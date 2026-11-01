//! 蓝牙语义契约。
//!
//! 上层只认这些东西：**事件**（发生了什么）、**操作**（要它做什么）、
//! **错误**（为什么没成）。至于底层事件号是几、结构里 CID 在第几个字节、
//! 回调表要怎么装配，全部由后端消化。

use core::ffi::c_void;

/// 链路事件 —— 后端负责把固件的事件号翻译成这里
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Event {
    /// 连接确认（通道可用）。`cid` 为本地通道号
    ConnectConfirm { cid: u16 },
    /// 连接完成（可能带对端通道号与 MTU；后端拿不到就留 None）
    ConnectComplete {
        cid: u16,
        remote_cid: Option<u16>,
        mtu: Option<u16>,
    },
    /// 对端要求重新配置
    ConfigRequest { cid: u16 },
    /// 数据到达。`data` 指向后端拥有的缓冲区，**只在回调期间有效**
    Data { cid: u16, data: &'static [u8] },
    /// 断开
    Disconnected { cid: u16, reason: u32 },
    /// 后端认识这个事件号，但不打算暴露（例如内部的心跳类事件）
    Other { raw: u32 },
}

/// 后端错误 —— 与固件返回码解耦，模块不需要知道 -1 是"没内存"还是"参数错"
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum BtError {
    NotReady,
    NoMemory,
    NotFound,
    Rejected,
    Unsupported,
    Unknown(i32),
}

/// 一次 L2CAP 连接请求
#[derive(Copy, Clone)]
pub struct ConnectRequest {
    pub address: [u8; 6],
    /// 目标 PSM（动态 PSM 必须奇数）
    pub psm: u16,
    /// 本端接收 MTU
    pub rx_mtu: u16,
}

/// 配对过滤器句柄（替换原厂"只允许手机地址"的策略）
#[derive(Copy, Clone)]
pub struct PairFilter {
    pub allocation: usize,
    pub registration: u32,
}

/// 事件回调。
///
/// ★ 签名保持**裸参数**（`raw` + 载荷指针），不跨 FFI 传 [`Event`]：
///   `Event` 带 `&'static [u8]`，跨 FFI 传递既不确定布局也容易踩生命周期。
///   上层拿到 `raw` 后调 [`decode`] 换成语义 —— 翻译这件事仍然只发生在一处。
///
/// 后端保证：**只在协议栈上下文里调**，不要在里边写文件/阻塞。
pub type EventSink = unsafe extern "C" fn(raw: u32, arg: *mut c_void);

/// 把后端交给的裸事件换成语义（未识别返回 `None`）
pub fn decode(raw: u32, _arg: *mut c_void) -> Option<Event> {
    backend::map_event(raw)
}

// ── 接口 ───────────────────────────────────────────────────────
//
// 这些函数由后端提供实现；签名固定，语义固定，实现随设备不同。

/// 开始扫描（经典蓝牙 inquiry）
pub fn discovery_start() -> Result<(), BtError> {
    backend::discovery_start()
}

/// 停止扫描
pub fn discovery_stop() -> Result<(), BtError> {
    backend::discovery_stop()
}

/// 绑定（配对）
pub fn create_bond(address: &[u8; 6]) -> Result<(), BtError> {
    backend::create_bond(address)
}

/// 查询绑定状态（聚合值，已绑定的具体数值由后端归一）
pub fn bond_state(address: &[u8; 6]) -> Option<BondState> {
    backend::bond_state(address)
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum BondState {
    None,
    Bonding,
    Bonded,
}

/// 发起 L2CAP 连接；事件通过 `sink` 回来
pub fn connect(request: ConnectRequest, sink: EventSink) -> Result<(), BtError> {
    backend::connect(request, sink)
}

/// 发送一段数据到已建立的通道
pub fn send(cid: u16, data: &[u8]) -> Result<(), BtError> {
    backend::send(cid, data)
}

/// 断开通道
pub fn disconnect(cid: u16) -> Result<(), BtError> {
    backend::disconnect(cid)
}

// 后端实现 —— 一个 target 一份
#[cfg(feature = "backend-band10pro-043")]
use crate::backend::band10pro_043 as backend;

#[cfg(not(feature = "backend-band10pro-043"))]
mod backend {
    //! 没有后端时全部返回 Unsupported。
    //!
    //! ★ 故意**不用** `compile_error!`：HAL 本身要能在没有目标的情况下
    //!   被 `cargo check`（CI 会跑）。真正需要硬失败的是"声明了要用蓝牙
    //!   却选了一台没有后端的设备"，那由 target 的 capabilities 校验拦。
    use super::*;
    pub fn discovery_start() -> Result<(), BtError> {
        Err(BtError::Unsupported)
    }
    pub fn discovery_stop() -> Result<(), BtError> {
        Err(BtError::Unsupported)
    }
    pub fn create_bond(_a: &[u8; 6]) -> Result<(), BtError> {
        Err(BtError::Unsupported)
    }
    pub fn bond_state(_a: &[u8; 6]) -> Option<BondState> {
        None
    }
    pub fn connect(_r: ConnectRequest, _s: EventSink) -> Result<(), BtError> {
        Err(BtError::Unsupported)
    }
    pub fn send(_cid: u16, _d: &[u8]) -> Result<(), BtError> {
        Err(BtError::Unsupported)
    }
    pub fn disconnect(_cid: u16) -> Result<(), BtError> {
        Err(BtError::Unsupported)
    }
}
