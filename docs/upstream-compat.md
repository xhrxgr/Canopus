# 向上兼容承诺（我们的分支 vs 上游模块）

> 一句话：**我们的 fork 只能"加东西"，不能改上游模块依赖的任何契约。**
> 这份文档写清承诺范围，以及怎么验证承诺还算数。

## 1. 承诺保持不变的东西

| 契约 | 状态 | 为什么不能动 |
|---|---|---|
| 模块 ABI（`canopus-abi`、`canopus_services.h` 的服务对象布局） | 未改 | 上游编出来的模块要在我们的 Supervisor 上跑 |
| C SDK 头文件 | 未改 | 同上 |
| 包格式（`.canopus` tar + manifest schema） | 未改 | 上游产物要能被我们的 CLI 校验、安装 |
| `canopus build` / `package` 的**默认**链接行为 | 已回退一处改动（见 §3） | 它会改变**所有**模块的产物 |
| 能力校验 | 只对**声明了** `required` 的模块生效 | `required` 为空 = 不校验，老模块行为不变 |
| Supervisor / manager 运行时 | 未改（只读不改） | 现场设备上装的是它 |

## 2. 我们加的东西都是"旁路"

- `canopus-hal`：新增 crate。**没有任何上游代码依赖它**，不用它就不进产物（LTO + `--gc-sections` 保证）。
- `docs/portability-contract.md`、`docs/hal-design.md`：纯文档。
- 030 的 target pack 重连：只影响"要不要给 030 出包"，不影响已注册的其他 target。
- `sdk update` 的 Windows 退化路径：只在没有 symlink 权限时触发。

## 3. 已经回退的一处改动

`cli/canopus/src/project.rs` 里曾加过 `--discard-locals`（想缩 `.strtab`）。
实测对 BandLink 体积**零效果**（71,608 → 71,608），但它会改变**所有**模块的链接命令。
为没有收益的改动去动上游模块的产物，不划算 —— 已移除，只留 `--gc-sections`。

这是本次"兼容性优先"的唯一实际代价：放弃了一个本来也没用的优化。

## 4. 现状是"能证明"的，不是"觉得"

用我们的 CLI 编上游自带的示例模块（`modules/examples/hello-portable`）：

```
$ canopus build
built ./build/hello_portable.elf
summary: PASS (72 sections, 810 resident bytes, 0 undefined, 38 relocs, 1 ctors, 0 dtors)
ok: hello_portable (portable) built and verified

$ canopus package --build --installer
wrote installer ./dist/installer/xiaomi-band-11
```

即：**编译、verify、出包、生成安装件**四个环节对上游模块全部照旧。

## 5. 还差一个正式的回归门

现在这次验证是手动的。要让它长期成立，应该加进 CI：

```
for m in modules/examples/*/; do
  canopus build   --manifest "$m/Canopus.toml"   || fail
  canopus package --manifest "$m/Canopus.toml"   || fail
done
```

**没有这条门，兼容性迟早会被下一次"顺手改一下链接参数"破坏** —— §3 就是活例子。

## 6. 边界：什么情况我们会不兼容

只有一种：上游改了 ABI 而我们跟进时漏了一处。
所以每次 rebase 上游，跑第 5 节那条循环就够了 —— 它比读 diff 靠谱。
