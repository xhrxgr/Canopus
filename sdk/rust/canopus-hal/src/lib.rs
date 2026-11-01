//! # canopus-hal —— 模块与固件之间的硬件抽象层
//!
//! ## 它解决什么
//!
//! 现在的模块里到处都是这种代码：
//!
//! ```ignore
//! // 043 的 L2CAP 事件枚举
//! const EV_DATA: u32 = 7;
//! // 043 的 L2CAP_ConnectCompleteStru 布局
//! const CID: usize = 108;
//! const RCID: usize = 110;
//! ```
//!
//! 地址本身由符号包解决了（`canopus-target-private` 已经按 target 选表），
//! 但**语义**没有：同一个"数据到达"，043 是 7、别的固件可能是 9；
//! 同一个"本地 CID"，043 在 +108、别的可能是 +76。模块每支持一台新设备
//! 就要把这些数字重新查一遍 —— 这才是"广泛兼容"真正的成本。
//!
//! ## 分层
//!
//! ```text
//!  模块（bandlink）      ← 只写 canopus_hal::bt::connect() / send() / Event
//!       │
//!  canopus-hal          ← 语义契约：Event 枚举、BtLink trait、错误码
//!       │
//!  后端 backend/<target> ← 每台设备一份：事件号映射、结构偏移、回调装配方式
//!       │
//!  canopus-target-private ← 固件地址（已按 target 选好）
//! ```
//!
//! **上层不认识固件，下层不认识业务。** 模块代码里不再出现任何一个数字常量。
//!
//! ## 为什么不做成动态分发
//!
//! trait 只用来描述契约；实际调用在**链接期**就解析成直接调用
//! （backend 由 feature 选定，单态化，没有 vtable、没有运行期查表）。
//! 嵌入式模块的 ROM/RAM 都紧张，这一层必须是零成本的。
//!
//! ## 加一台新设备要做什么
//!
//! 1. `targets/<id>/symbols/*.json` 填好符号（已有工具链：`fw-match`、`symbol-generator`）
//! 2. 写一个 `backend/<id>.rs`，实现 [`bt`] 里的语义契约
//! 3. `Cargo.toml` 加一个 feature，把 target 链进来
//!
//! 模块代码 **一行都不用改** —— 包括 BandLink。

#![no_std]

pub mod bt;

/// 后端选择：这里只列"已实现语义层"的目标。
///
/// 没有实现的后端不会静默退化 —— 引用 [`bt`] 的函数会直接编译失败，
/// 而不是链到一堆错误的地址上。
pub mod backend {
    #[cfg(feature = "backend-band10pro-043")]
    #[path = "band10pro_043.rs"]
    pub mod band10pro_043;
}

/// 当前是否已经有语义层后端
pub const HAS_BT_BACKEND: bool = cfg!(feature = "backend-band10pro-043");
