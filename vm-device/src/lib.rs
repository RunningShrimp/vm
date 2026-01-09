//! # vm-device - 设备虚拟化层
//!
//! 提供虚拟机设备的完整实现，包括 VirtIO 设备、中断控制器和硬件检测。
//!
//! ## 模块组织

#![cfg_attr(test, allow(dead_code))]
#![cfg_attr(test, allow(unused_imports))]
#![cfg_attr(test, allow(unused_variables))]
#![cfg_attr(test, allow(unused_mut))]
//!
//! ```text
//! vm-device
//! ├── virtio_devices/     # VirtIO 设备集合
//! │   ├── block           # 块存储设备
//! │   ├── block_async     # 异步块存储（需要 async-io feature）
//! │   ├── ai              # AI 加速设备
//! │   ├── scsi            # SCSI 存储设备
//! │   └── vhost_net       # vhost 网络
//! │
//! ├── interrupt/          # 中断控制器
//! │   ├── clint           # Core Local Interruptor
//! │   └── plic            # Platform Level Interrupt Controller
//! │
//! └── gpu/                # GPU 虚拟化
//!     ├── gpu_virt        # GPU 虚拟化管理
//!     ├── gpu_passthrough # GPU 直通
//!     ├── gpu_mdev        # GPU 介质设备
//!     └── virgl           # Virgl 3D 渲染
//! ```
//!
//! ## 使用方式
//!
//! ### 基础使用（推荐）
//! ```rust,ignore
//! use vm_device::virtio_devices;  // VirtIO 设备
//! use vm_device::interrupt;       // 中断控制器
//! ```
//!
//! ### 直接模块访问（向后兼容）
//! ```rust,ignore
//! use vm_device::block::VirtioBlock;
//! use vm_device::clint::CLINT;
//! use vm_device::plic::PLIC;
//! ```
//!
//! ## 特性标志
//!
//! - `async-io`: 启用异步 IO 支持（使用 tokio）
//! - `simple-devices`: 启用简化版设备实现（适用于快速原型和测试）

// ============================================================
// VirtIO 核心定义
// ============================================================
pub mod virtio;

// ============================================================
// VirtIO 设备实现（保持向后兼容的直接导出）
// ============================================================
pub mod async_block_device; // 真正的异步块设备
pub mod async_buffer_pool; // 异步I/O缓冲池
pub mod block;
// async-io feature removed - block_async is now always compiled
pub mod block_async;
pub mod block_service; // DDD服务层
pub mod net;
pub mod vhost_net;
pub mod virtio_9p;
pub mod virtio_ai;
pub mod virtio_balloon;
pub mod virtio_console;
pub mod virtio_crypto;
pub mod virtio_input;
pub mod virtio_memory;
pub mod virtio_performance;
pub mod virtio_rng;
pub mod virtio_scsi;
pub mod virtio_sound;
pub mod virtio_watchdog;

// ============================================================
// 中断控制器（保持向后兼容的直接导出）
// ============================================================
pub mod clint;
pub mod plic;

// ============================================================
// 磁盘镜像创建
// ============================================================
pub mod disk_image;

// ============================================================
// SATA/AHCI 控制器
// ============================================================
pub mod ahci;

// ============================================================
// 通用块设备接口
// ============================================================
pub mod block_device;

// ============================================================
// ATAPI CD-ROM 设备
// ============================================================
pub mod atapi;

// ============================================================
// VGA 显示设备
// ============================================================
pub mod vga;

// ============================================================
// 硬件检测与实用工具
// ============================================================
pub mod hw_detect;
pub mod mmu_util;

// ============================================================
// 零复制 I/O 与 DMA
// ============================================================
pub mod dma;
pub mod io_multiplexing;
pub mod io_scheduler;
pub mod mmap_io;
#[cfg(feature = "smmu")]
pub mod smmu_device;
pub mod vhost_protocol;
pub mod virtio_zerocopy;
#[cfg(test)]
pub mod zero_copy_io;
pub mod zero_copy_optimizer;
pub mod zerocopy; // 仅用于测试和基准测试

pub use io_multiplexing::{IoEventLoop, IoLatencyOptimizer, IoThroughputOptimizer};
pub use vhost_protocol::{VhostFeature, VhostFrontend, VhostMemoryMap, VhostServiceManager};
pub use virtio_zerocopy::{
    BufferPool, DirectMemoryAccess, ScatterGatherList as VirtioSgList, VirtioZeroCopyManager,
    ZeroCopyChain,
};
pub use zero_copy_optimizer::{ZeroCopyConfig, ZeroCopyIoOptimizer, ZeroCopyStats};
#[cfg(target_os = "linux")]
pub use zerocopy::MemoryMappedBuffer;
pub use zerocopy::{DirectBuffer, ScatterGatherElement, ScatterGatherList, ZeroCopyIoManager};

// ============================================================
// GPU 虚拟化
// ============================================================
pub mod gpu_manager;
pub mod gpu_mdev;
pub mod gpu_passthrough;
pub mod gpu_virt;
pub mod virgl;

// ============================================================
// 设备服务
// ============================================================
pub mod device_service;

// ============================================================
// SMMU 设备支持
// ============================================================
#[cfg(feature = "smmu")]
pub use smmu_device::{SmmuDeviceInfo, SmmuDeviceManager};

// ============================================================
// 简化设备实现（simple-devices feature removed - always available）
// ============================================================
pub mod simple_devices;

// ============================================================
// 新的模块化组织（推荐使用）
// ============================================================
pub mod interrupt;
pub mod virtio_devices;

// ============================================================
// 高级网络功能
// ============================================================
pub mod dpdk;
pub mod network_qos;
pub mod sriov;
