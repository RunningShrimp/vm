//! VirtIO 设备集合
//!
//! 本模块包含所有 VirtIO 规范设备的实现。
//!
//! ## 设备类型
//!
//! - `block`: VirtIO Block 设备 - 块存储
//! - `console`: VirtIO Console 设备 - 串口控制台  
//! - `net`: VirtIO Network 设备 - 网络（通过 vhost）
//! - `gpu`: VirtIO GPU 设备 - 图形加速
//! - `ai`: VirtIO AI 设备 - AI 推理加速
//! - `scsi`: VirtIO SCSI 设备 - SCSI 存储

// 重导出现有模块
pub use crate::block::*;
#[cfg(feature = "async-io")]
pub use crate::block_async::*;
pub use crate::virtio_ai::*;
pub use crate::virtio_scsi::*;
pub use crate::vhost_net::*;

