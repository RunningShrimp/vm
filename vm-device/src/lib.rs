//! # vm-device - 设备虚拟化层
//!
//! 提供虚拟机设备的完整实现，包括 VirtIO 设备、中断控制器和硬件检测。
//!
//! ## 模块组织
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

// ============================================================
// VirtIO 核心定义
// ============================================================
pub mod virtio;

// ============================================================
// VirtIO 设备实现（保持向后兼容的直接导出）
// ============================================================
pub mod block;
#[cfg(feature = "async-io")]
pub mod block_async;
pub mod virtio_ai;
pub mod virtio_scsi;
pub mod vhost_net;

// ============================================================
// 中断控制器（保持向后兼容的直接导出）
// ============================================================
pub mod clint;
pub mod plic;

// ============================================================
// 硬件检测与实用工具
// ============================================================
pub mod hw_detect;
pub mod mmu_util;

// ============================================================
// GPU 虚拟化
// ============================================================
pub mod virgl;
pub mod gpu_virt;
pub mod gpu_passthrough;
pub mod gpu_mdev;
pub mod gpu_manager;

// ============================================================
// 设备服务
// ============================================================
pub mod device_service;

// ============================================================
// 新的模块化组织（推荐使用）
// ============================================================
pub mod virtio_devices;
pub mod interrupt;
