//! 3.101.043 的蓝牙语义后端。
//!
//! 这一层是**唯一**允许出现"固件数字"的地方：
//! 事件号、结构偏移、原厂策略、适配器实例怎么取。
//!
//! 现在的实现直接转给 `canopus-target-private`（地址已按 target 选好）。
//! 真正的价值在那些**不能被地址解决**的差异上，见每段注释。

use crate::bt::{BondState, BtError, ConnectRequest, Event, EventSink};
use canopus_target_private as fw;

/// 经典蓝牙传输类型。043 上是 1；有的固件是 0（BR/EDR 默认），
/// 写错的表现是配对请求一直失败但没有任何错误码 —— 必须由后端归一。
const TRANSPORT_CLASSIC: u32 = 1;

/// 扫描超时（inquiry 的 duration，单位 1.28 s）
const INQUIRY_TIMEOUT: i32 = 8;

pub fn discovery_start() -> Result<(), BtError> {
    let adapter = unsafe { fw::bt_adapter_get_instance() };
    if adapter.is_null() {
        return Err(BtError::NotReady);
    }
    match unsafe { fw::bt_discovery_start(adapter, INQUIRY_TIMEOUT) } {
        0 => Ok(()),
        rc => Err(BtError::Unknown(rc)),
    }
}

pub fn discovery_stop() -> Result<(), BtError> {
    let adapter = unsafe { fw::bt_adapter_get_instance() };
    if adapter.is_null() {
        return Err(BtError::NotReady);
    }
    match unsafe { fw::bt_discovery_stop(adapter) } {
        0 => Ok(()),
        rc => Err(BtError::Unknown(rc)),
    }
}

pub fn create_bond(address: &[u8; 6]) -> Result<(), BtError> {
    match unsafe { fw::bt_create_bond(address.as_ptr(), TRANSPORT_CLASSIC) } {
        0 => Ok(()),
        rc => Err(BtError::Unknown(rc)),
    }
}

/// 把固件的绑定状态归一成三态。
///
/// ★ 043 有两个来源：聚合状态里"已绑定"=3，设备级里"已绑定"=2。
///   别的固件可能只有其中一个，甚至用 1 表示已绑定 —— 归一化只能在这一层做，
///   否则每个模块都要写一遍"两个都认"。
pub fn bond_state(address: &[u8; 6]) -> Option<BondState> {
    const BOND_NONE: u32 = 0;
    const BOND_BONDING: u32 = 1;
    const BOND_DEVICE_BONDED: u32 = 2;
    const BOND_AGGREGATED_BONDED: u32 = 3;
    match unsafe { fw::bt_get_bond_state(address.as_ptr()) } {
        BOND_NONE => Some(BondState::None),
        BOND_BONDING => Some(BondState::Bonding),
        BOND_DEVICE_BONDED | BOND_AGGREGATED_BONDED => Some(BondState::Bonded),
        _ => None,
    }
}

/// 事件号 → 语义。**这是 HAL 存在的核心理由。**
///
/// 043 的枚举来自固件 DWARF（`stack_adapter_gap.c`）：
/// `CONNECT_IND=1 CONNECT_CFM=2 CONNECT_COMPLETE=3 CONNECT_PARAM=4
///  CONFIG_IND=5 DISCONNECT_COMPLETE=6 DATA_IND=7 DATA_CFM=8`。
/// 换固件时这张表要重做，而模块不该知道它存在。
pub fn map_event(raw: u32) -> Option<Event> {
    match raw {
        2 => Some(Event::ConnectConfirm { cid: 0 }),
        3 => Some(Event::ConnectComplete {
            cid: 0,
            remote_cid: None,
            mtu: None,
        }),
        5 => Some(Event::ConfigRequest { cid: 0 }),
        7 => Some(Event::Data {
            cid: 0,
            data: &[],
        }),
        6 => Some(Event::Disconnected { cid: 0, reason: 0 }),
        _ => None,
    }
}

/// 连接请求：**还没搬进来**。
///
/// 现在这段逻辑（68 字节结构、`config` 内联在 +24、`in_mtu` 在 +28）
/// 还留在 bandlink 的 `platform.rs` 里。搬进来的意义不是"少一个文件"，
/// 而是让 036 / 030 用各自的结构布局，而 BandLink 一行都不改。
/// 这是迁移的第 2 步。
pub fn connect(_request: ConnectRequest, _sink: EventSink) -> Result<(), BtError> {
    Err(BtError::Unsupported)
}

/// 发送：同样待迁移（需要 `StockBuffer` 前缀 12 字节 + `+4+offset` 的布局知识）
pub fn send(_cid: u16, _data: &[u8]) -> Result<(), BtError> {
    Err(BtError::Unsupported)
}

pub fn disconnect(_cid: u16) -> Result<(), BtError> {
    Err(BtError::Unsupported)
}
