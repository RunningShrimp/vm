//! vm-platform: 平台相关功能的统一抽象层
//!
//! 提供：
//! - 操作系统抽象（内存映射、线程、信号、计时器）
//! - 硬件直通（PCIe、GPU、NPU、SR-IOV）
//! - 虚拟机启动和运行时
//!
//! # 架构设计
//! 模块按职责划分，提供清晰的接口和简单的依赖关系
//!
//! # 使用示例
//! ```ignore
//! use vm_platform::{
//!     memory::MappedMemory,
//!     boot::BootManager,
//!     platform::host_os,
//!     pci::VfioDevice,
//! };
//!
//! // 分配内存
//! let mem = MappedMemory::allocate(4096, MemoryProtection::READ_WRITE)?;
//!
//! // 获取平台信息
//! let os = host_os();
//! println!("Platform: {}", os);
//!
//! // 创建启动管理器
//! let boot_mgr = SimpleBootManager::new();
//!
//! // 扫描IOMMU组
//! let mut iommu_mgr = IommuManager::new();
//! let _ = iommu_mgr.scan_groups();
//! ```

// ========== 重新导出vm-osal功能 ==========
pub mod memory;
pub mod platform;
pub mod signals;
pub mod threading;
pub mod timer;

// ========== 重新导出vm-passthrough功能 ==========
pub mod gpu;
pub mod passthrough;
pub mod pci;

// ========== 重新导出vm-boot功能 ==========
pub mod boot;
pub mod hotplug;
pub mod iso;
pub mod runtime;
pub mod snapshot;

// ========== 公共接口：内存相关 ==========
pub use memory::{
    JitMemory, MappedMemory, MemoryError, MemoryProtection, barrier_acquire, barrier_full,
    barrier_release,
};

// ========== 公共接口：线程相关 ==========
pub use threading::{set_thread_affinity_big, set_thread_affinity_little, set_thread_cpu};

// ========== 公共接口：信号相关 ==========
pub use signals::{SignalHandler, register_sigsegv_handler};

// ========== 公共接口：计时器相关 ==========
pub use timer::{measure, timestamp_ns};

// ========== 公共接口：平台检测相关 ==========
pub use platform::{PlatformFeatures, PlatformInfo, PlatformPaths, host_arch, host_os};

// ========== 公共接口：硬件直通 ==========
pub use passthrough::{
    DeviceType, PassthroughDevice, PassthroughError, PassthroughManager, PciAddress, PciDeviceInfo,
};

pub use pci::{IommuGroup, IommuManager, VfioDevice};

pub use gpu::{AmdGpuPassthrough, GpuConfig, NvidiaGpuPassthrough};

// ========== 公共接口：虚拟机启动和运行时 ==========
pub use boot::{BootConfig, BootManager, BootMethod, BootStatus, SimpleBootManager};

pub use runtime::{
    Runtime, RuntimeCommand, RuntimeEvent, RuntimeState, RuntimeStats, SimpleRuntimeController,
};

pub use snapshot::{
    SimpleSnapshotManager, SnapshotManager, SnapshotMetadata, SnapshotOptions, VmSnapshot,
};

pub use hotplug::{
    DeviceInfo, DeviceState as HotplugDeviceState, DeviceType as HotplugDeviceType, HotplugEvent,
    HotplugManager, SimpleHotplugManager,
};

pub use iso::{Iso9660, IsoDirectory, IsoEntry, IsoVolumeInfo, SimpleIso9660};
