# 可移植性契约（branch `feat/target-portability`）

> 本文是我们这条自有分支的设计基准，不是上游路线图。上游的符号适配方法论见
> [`firmware-symbol-porting-plan.md`](firmware-symbol-porting-plan.md)，两者不冲突：
> 上游解决"地址从哪来"，本文解决"模块怎么写才不用知道地址"。

## 1. 问题陈述

目标：**同一个模块源码，装到任意受支持的固件上都能跑；模块里不出现任何固件地址、
版本号、私有符号名。** 新增一台设备时，只允许增加"设备侧数据"，不允许改模块。

现状与之的差距有三层，必须分开处理，否则会互相掩盖：

| 层 | 现状 | 影响 |
|---|---|---|
| ABI 层 | 15 个服务组中，`bt` 只有数据面（`l2_send` / `l2_register_rx` / `is_connected` / `adapter_state`），**没有控制面** | 扫描、配对、建链只能走 target-private 私有地址 → 模块被钉死在某一份固件 |
| 构建层 | 模块 `[targets] include` 手写；SDK 两个 crate 的 feature + `include!` 手写 | 新增设备要手改 3 处，漏一处就是"文件生成了但没接上"（030 现在就是这个状态） |
| 数据层 | 每个固件的符号地址需要独立恢复 | 无法用工程手段绕过；只能靠 BinDiff/证据链流程，缺符号就必须显式失败 |

## 2. 契约

### 2.1 模块只声明"我要什么"

模块 manifest 的 `[capabilities] required` 列能力名（如 `classic.discovery`），
不列函数名、不列地址。`canopus build` 用 `canopus_core::planner::expand`
把 required 与目标 pack 的 `capabilities` 求差集，缺一个就报错并列出缺哪些。

这套校验**已经存在**（`tools/canopus-core/src/planner.rs`），当前没用起来只是因为
模块侧 `required` 是空的。

### 2.2 目标 pack 只声明"这台机器给什么"

`targets/<id>/target.toml` 的 `capabilities` 是**证据结论**，不是愿望清单：
只能写在这份固件上已被证据/真机证明过的能力。043 已声明
`classic.adapter` / `classic.l2cap` / `classic.sdp` / `bt.timer` 等；
本分支补上模块实测用到的 `classic.discovery` 与 `classic.pair-request-filter`。

### 2.3 模块代码只调统一接口

模块侧唯一允许的调用面是 `canopus-runtime`（`fs` / `hook` / `log` / `ui` / … 已有 14 组）。
`bt` 是唯一缺口，两条路线见 §4。

## 3. 现状核账（2026-11-01，本分支实测）

`band10_pro.rs` 引用 96 个 `canopus_target_generated::*` 符号。各固件覆盖：

| 固件 | 覆盖 | 说明 |
|---|---|---|
| 3.101.043 | 96/96 | 模块当前唯一可编译目标，BandLink 在此真机跑通单向消息 |
| 3.101.036 | 67/96 | 缺 29 个（含全部 bt 控制面以外的 UI/fs 若干） |
| 3.101.030 | 63/96 | 缺 33 个，**bt 相关全缺**；且 bindings 文件已生成但从未接线（孤儿） |

030 缺失清单（33）：

```
CANOPUS_FW_LVX_CONTENT_PAD_BOTTOM_CALLABLE      CANOPUS_FW_LVX_LABEL_SET_LONG_MODE_CALLABLE
CANOPUS_FW_LV_OBJ_CLEAR_FLAG_CALLABLE          CANOPUS_FW_LV_OBJ_SET_LOCAL_STYLE_PROP_XIP_THUNK_CALLABLE
canopus_fw_bt_adapter_get_instance             canopus_fw_bt_adapter_get_scan_mode
canopus_fw_bt_adapter_get_state                canopus_fw_bt_adapter_register
canopus_fw_bt_adapter_unregister               canopus_fw_bt_get_pairing_state
canopus_fw_bt_l2cap_connect                    canopus_fw_bt_pair_display_reply
canopus_fw_bt_pair_request_reply               canopus_fw_core_bt_pair_request_callback
canopus_fw_interconnect_close                  canopus_fw_interconnect_connect
canopus_fw_interconnect_send                   canopus_fw_lseek
canopus_fw_lv_event_get_code                   canopus_fw_lv_event_get_user_data
canopus_fw_lv_obj_add_event_cb                 canopus_fw_lv_timer_create
canopus_fw_quickapp_register_app               canopus_fw_stock_bt_callback_descriptor
canopus_fw_style_misans_regular_24             canopus_fw_thirdparty_submit_message_content
canopus_interconnect_app_info                  canopus_interconnect_message
canopus_interconnect_recv_cb                   canopus_interconnect_send_done
canopus_thirdparty_message_content             canopus_thumb_callable
```

结论：**030/036 现在不可能跑 BandLink**，缺的不是胶水而是地址。谁把"全兼容"理解成
"改改构建就能跑 030"，谁就会在下一步撞墙。构建层能做的只有一件事：让它在编译前
就明确拒绝，并说清楚缺什么。

## 4. 蓝牙控制面：两条路线

### 路线 A — 框架拥有 BT（目标是"一个 .bin 通吃"）

在 `struct canopus_bluetooth_obj` 尾部追加控制面成员（沿用 `canopus_bus_obj` 的
"追加字段 + `group_version` 升版"惯例），`SVC_BLUETOOTH` 从 1 升到 2：

```c
int32_t (*discover_start)(ctx, uint32_t scan_mode);
int32_t (*discover_stop)(ctx);
int32_t (*peer_count)(ctx);
int32_t (*peer_at)(ctx, uint32_t index, struct canopus_bt_peer_v1 *out);
int32_t (*bond_create)(ctx, const uint8_t addr[6], uint32_t transport);
int32_t (*bond_state)(ctx, const uint8_t addr[6]);
int32_t (*pair_request_reply)(ctx, const uint8_t addr[6], int accept);
int32_t (*l2_open)(ctx, uint16_t psm, uint16_t *out_cid);
int32_t (*l2_close)(ctx, uint16_t cid);
int32_t (*l2_submit_cid)(ctx, uint16_t cid);
int32_t (*subscribe)(ctx, canopus_bt_event_callback_t cb, void *user_data);
```

Supervisor 侧承担全部固件差异，模块变成纯二进制、可跨设备分发。

- 代价：`canopus_supervisor_platform` 目前只有 `bt_adapter_state` / `bt_is_connected`
  两个钩子，控制面要新写（C，且每个 target 仍要一份地址），估计 800–1500 行 + 真机回归。
- 收益：模块彻底脱离 target-private；`canopus build` 不再需要 `--target`；
  应用商店式分发成为可能。

### 路线 B — SDK 门面（每设备一份 .bin，源码不变）

`canopus-target-private` 已经是"同函数集 + 缺符号返回 `ERR_UNSUPPORTED`"的门面
（见 `tests/api_parity.rs`），把 030/036 的缺口按同约定补成桩，模块源码零改动，
每台设备各编一份。

- 代价：`band10_pro.rs` 要为缺失符号加 per-version `cfg` 桩（约 30 处）；
  每台设备仍需一次构建；桩让"跑起来但什么都不做"的风险变高，必须靠能力声明兜住。
- 收益：一周内可交付；不动 supervisor，不动 ABI，不与上游冲突。

**建议：先 B 后 A。** B 让现有产品线立刻多设备可用（能跑的设备真跑，不能跑的编译期
就拒绝），A 作为长期方向单独推进，两者共用同一份能力词表，不返工。

## 5. 本分支的提交与后续

已落地（`feat/target-portability`，基于 `origin/main` 4928f1e）：

1. `feat(targets)` 030 pack 重新接线到 band10-pro family（此前只存在于 git 历史）
2. `fix(bt)` SSP 应答发到 core adapter；配对过滤器可逆撤销；`nuttx_open_mode`
3. `fix(cli)` Rust 模块链接加 `--gc-sections`（ELF 388 KB → 23 KB）
4. `fix(cli)` 无符号链接权限时 `sdk update` 退化为指针文件

后续按顺序：

- P1 构建层：`canopus target generate --check` 同时校验"bindings 已接线"，
  消灭孤儿文件；模块 `[targets] include` 由 pack 推导
- P2 能力层：043 补 `classic.discovery` / `classic.pair-request-filter`；
  BandLink manifest 写 `required`，让 030 在编译前失败并列出缺口
- P3 门面层：路线 B 的桩 + `canopus_runtime::bt` 统一接口（先包住已有 4 个数据面方法）
- P4 控制面：路线 A 的 ABI 追加 + supervisor 实现（需真机回归，单独立项）

## 6. 不做的事

- 不为"看起来兼容"给缺失符号编地址。缺就是缺，编译期拒绝优于运行期崩溃。
- 不在本分支改上游的符号恢复流程（`fw-match` / 证据链），那是另一条线。
- 不把 BandLink 的私有需求（BandLink 自己发明的协议、表盘 id 规则）写进框架。
