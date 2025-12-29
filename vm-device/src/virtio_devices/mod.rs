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
//! - `9p`: VirtIO 9P 设备 - 文件系统
//! - `balloon`: VirtIO Balloon 设备 - 内存气球
//! - `input`: VirtIO Input 设备 - 输入设备（键盘、鼠标）
//! - `rng`: VirtIO RNG 设备 - 随机数生成器
//! - `crypto`: VirtIO Crypto 设备 - 加密加速
//! - `sound`: VirtIO Sound 设备 - 音频设备
//! - `watchdog`: VirtIO Watchdog 设备 - 看门狗
//! - `memory`: VirtIO Memory 设备 - 内存设备（热插拔）
//! - `performance`: VirtIO 性能统计 - 统一的性能监控

// 重导出现有模块
pub use crate::block::*;
// async-io feature removed - block_async is now always compiled
pub use crate::block_async::*;
pub use crate::vhost_net::*;
pub use crate::virtio_9p::*;
pub use crate::virtio_ai::*;
pub use crate::virtio_balloon::*;
pub use crate::virtio_console::*;
pub use crate::virtio_crypto::*;
pub use crate::virtio_input::*;
pub use crate::virtio_memory::*;
pub use crate::virtio_performance::*;
pub use crate::virtio_rng::*;
pub use crate::virtio_scsi::*;
pub use crate::virtio_sound::*;
