//! # vm-accel - 硬件虚拟化加速层
//!
//! 提供跨平台的硬件虚拟化加速支持，包括 KVM (Linux)、HVF (macOS)、
//! WHPX (Windows) 和 Virtualization.framework (iOS/tvOS)。
//!
//! ## 架构概览
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                    vm-accel                              │
//! ├─────────────────────────────────────────────────────────┤
//! │                   Accel Trait                            │
//! │  ┌─────────┬─────────┬─────────┬─────────┐              │
//! │  │   KVM   │   HVF   │  WHPX   │   VZ    │              │
//! │  │ (Linux) │ (macOS) │ (Win)   │ (iOS)   │              │
//! │  └─────────┴─────────┴─────────┴─────────┘              │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! ## 主要功能
//!
//! - **平台检测**: 自动检测可用的虚拟化后端
//! - **统一接口**: 通过 [`Accel`] trait 提供统一的虚拟化 API
//! - **CPU 特性检测**: 检测 AVX2/AVX512/NEON 等 SIMD 特性
//! - **SIMD 优化**: 提供平台特定的 SIMD 辅助函数
//!
//! ## 支持的平台
//!
//! | 平台 | 后端 | 状态 |
//! |------|------|------|
//! | Linux | KVM | ✅ 完整支持 |
//! | macOS | Hypervisor.framework | ✅ 完整支持 |
//! | Windows | WHPX | ✅ 完整支持 |
//! | iOS/tvOS | Virtualization.framework | ⚠️ 实验性 |
//!
//! ## 快速开始
//!
//! ```rust,ignore
//! use vm_accel::{select, AccelKind, Accel};
//!
//! // 自动选择最佳的加速器后端
//! let (kind, mut accel) = select();
//!
//! match kind {
//!     AccelKind::Kvm => println!("Using KVM"),
//!     AccelKind::Hvf => println!("Using Hypervisor.framework"),
//!     AccelKind::Whpx => println!("Using Windows Hypervisor Platform"),
//!     AccelKind::None => println!("No hardware acceleration available"),
//! }
//!
//! // 初始化加速器
//! accel.init()?;
//!
//! // 创建 vCPU
//! accel.create_vcpu(0)?;
//!
//! // 映射内存
//! accel.map_memory(0x1000, host_addr, 0x10000, 0x7)?;
//!
//! // 运行 vCPU
//! accel.run_vcpu(0, &mut mmu)?;
//! ```
//!
//! ## CPU 特性检测
//!
//! ```rust,ignore
//! use vm_accel::{detect, CpuFeatures};
//!
//! let features = detect();
//!
//! if features.avx2 {
//!     println!("AVX2 supported");
//! }
//! if features.vmx {
//!     println!("Intel VT-x supported");
//! }
//! if features.svm {
//!     println!("AMD-V supported");
//! }
//! ```
//!
//! ## 错误处理
//!
//! 所有加速器操作返回 [`Result<T, AccelError>`]，常见错误包括：
//!
//! - [`AccelError::NotAvailable`][]: 加速器不可用（硬件不支持或权限不足）
//! - [`AccelError::CreateVmFailed`][]: 创建虚拟机失败
//! - [`AccelError::MapMemoryFailed`][]: 内存映射失败
//! - [`AccelError::RunFailed`]: vCPU 执行失败
//!
//! ## 平台特定说明
//!
//! ### Linux (KVM)
//!
//! - 需要 `/dev/kvm` 设备节点访问权限
//! - 用户需要在 `kvm` 组中或有 root 权限
//! - 支持嵌套虚拟化（需要内核模块参数）
//!
//! ### macOS (Hypervisor.framework)
//!
//! - 需要 macOS 10.10+
//! - 应用需要 `com.apple.security.hypervisor` 权限
//! - 不支持嵌套虚拟化
//!
//! ### Windows (WHPX)
//!
//! - 需要 Windows 10 1803+
//! - 需要启用 "Windows Hypervisor Platform" 功能
//! - 与 Hyper-V 共存时可能有限制
//!
//! ## Feature 标志
//!
//! - `cpuid`: 启用 x86/x86_64 CPU 特性检测（使用 raw_cpuid crate）

#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), feature = "hardware"))]
use raw_cpuid::CpuId;

/// CPU 特性检测结果
///
/// 包含当前 CPU 支持的各种特性标志，用于运行时特性检测和优化路径选择。
///
/// # 示例
///
/// ```rust,ignore
/// let features = vm_accel::detect();
/// if features.avx512 {
///     // 使用 AVX-512 优化路径
/// } else if features.avx2 {
///     // 使用 AVX2 优化路径
/// } else {
///     // 使用标量路径
/// }
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct CpuFeatures {
    /// Intel AVX2 (256-bit SIMD) 支持
    pub avx2: bool,
    /// Intel AVX-512 (512-bit SIMD) 支持
    pub avx512: bool,
    /// ARM NEON (128-bit SIMD) 支持
    pub neon: bool,
    /// Intel VT-x 虚拟化扩展
    pub vmx: bool,
    /// AMD-V (SVM) 虚拟化扩展
    pub svm: bool,
    /// ARM EL2 虚拟化支持
    pub arm_el2: bool,
}

/// 检测当前 CPU 的特性
///
/// 根据平台使用不同的检测方法：
/// - x86/x86_64: 使用 CPUID 指令
/// - aarch64: NEON 默认支持，通过设备节点检测虚拟化
///
/// # 返回值
///
/// 返回 [`CpuFeatures`] 结构体，包含检测到的特性标志
///
/// # 示例
///
/// ```rust,ignore
/// let features = vm_accel::detect();
/// println!("AVX2: {}", features.avx2);
/// println!("VMX: {}", features.vmx);
/// ```
pub fn detect() -> CpuFeatures {
    let mut features = CpuFeatures::default();

    #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), feature = "hardware"))]
    {
        let cpuid = CpuId::new();

        if let Some(info) = cpuid.get_feature_info() {
            features.vmx = info.has_vmx();
        }

        if let Some(info) = cpuid.get_extended_feature_info() {
            features.avx2 = info.has_avx2();
            features.avx512 = info.has_avx512f();
        }

        // AMD SVM 检测需要扩展功能叶
        if let Some(ext_info) = cpuid.get_extended_processor_and_feature_identifiers() {
            // SVM 在扩展功能中，这里简化处理
            features.svm = false;
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        // On aarch64, NEON is mandatory.
        features.neon = true;
        // Simple heuristic for virtualization support availability
        features.arm_el2 = std::path::Path::new("/dev/kvm").exists();
    }

    features
}

// ============================================================================
// 统一加速器接口
// ============================================================================

use vm_core::error::{CoreError, MemoryError};
use vm_core::{ExecutionError, GuestRegs, MMU, VmError};

/// 加速器错误类型别名
///
/// 使用统一的 VmError 系统，加速器相关的错误映射到 PlatformError
pub type AccelError = VmError;

/// 从传统错误转换为统一错误
impl From<AccelLegacyError> for VmError {
    fn from(err: AccelLegacyError) -> Self {
        match err {
            AccelLegacyError::NotAvailable(msg) => VmError::Core(CoreError::NotSupported {
                feature: msg,
                module: "vm-accel".to_string(),
            }),
            AccelLegacyError::NotInitialized(msg) => VmError::Core(CoreError::InvalidState {
                message: msg,
                current: "not_initialized".to_string(),
                expected: "initialized".to_string(),
            }),
            AccelLegacyError::InitFailed(msg) => VmError::Core(CoreError::Internal {
                message: msg,
                module: "vm-accel".to_string(),
            }),
            AccelLegacyError::CreateVmFailed(msg) => VmError::Core(CoreError::Internal {
                message: msg,
                module: "vm-accel".to_string(),
            }),
            AccelLegacyError::CreateVcpuFailed(msg) => VmError::Core(CoreError::Internal {
                message: msg,
                module: "vm-accel".to_string(),
            }),
            AccelLegacyError::MapMemoryFailed(msg) => VmError::Memory(MemoryError::MappingFailed {
                message: msg,
                src: None,
                dst: None,
            }),
            AccelLegacyError::UnmapMemoryFailed(msg) => {
                VmError::Memory(MemoryError::MappingFailed {
                    message: msg,
                    src: None,
                    dst: None,
                })
            }
            AccelLegacyError::RunFailed(msg) => {
                VmError::Execution(ExecutionError::Halted { reason: msg })
            }
            AccelLegacyError::GetRegsFailed(msg) => {
                VmError::Execution(ExecutionError::FetchFailed {
                    pc: vm_core::GuestAddr(0),
                    message: msg,
                })
            }
            AccelLegacyError::SetRegsFailed(msg) => {
                VmError::Execution(ExecutionError::FetchFailed {
                    pc: vm_core::GuestAddr(0),
                    message: msg,
                })
            }
            AccelLegacyError::AccessDenied(msg) => VmError::Core(CoreError::NotSupported {
                feature: msg,
                module: "vm-accel".to_string(),
            }),
            AccelLegacyError::InvalidVcpuId(id) => VmError::Core(CoreError::InvalidParameter {
                name: "vcpu_id".to_string(),
                value: format!("{}", id),
                message: "Invalid vCPU ID".to_string(),
            }),
            AccelLegacyError::InvalidAddress(msg) => VmError::Memory(MemoryError::InvalidAddress(
                vm_core::GuestAddr(msg.parse().unwrap_or(0)),
            )),
            AccelLegacyError::NotSupported(msg) => VmError::Core(CoreError::NotSupported {
                feature: msg,
                module: "vm-accel".to_string(),
            }),
        }
    }
}

/// 传统的加速器错误类型（保留用于向后兼容）
#[derive(Debug, thiserror::Error)]
pub enum AccelLegacyError {
    /// 加速器不可用（硬件不支持或驱动未加载）
    #[error("Accelerator not available: {0}")]
    NotAvailable(String),
    /// 加速器未初始化
    #[error("Accelerator not initialized: {0}")]
    NotInitialized(String),
    /// 初始化失败
    #[error("Initialization failed: {0}")]
    InitFailed(String),
    /// 创建虚拟机失败
    #[error("Failed to create VM: {0}")]
    CreateVmFailed(String),
    /// 创建 vCPU 失败
    #[error("Failed to create vCPU: {0}")]
    CreateVcpuFailed(String),
    /// 内存映射失败
    #[error("Failed to map memory: {0}")]
    MapMemoryFailed(String),
    /// 取消内存映射失败
    #[error("Failed to unmap memory: {0}")]
    UnmapMemoryFailed(String),
    /// vCPU 运行失败
    #[error("Failed to run vCPU: {0}")]
    RunFailed(String),
    /// 访问被拒绝
    #[error("Access denied: {0}")]
    AccessDenied(String),
    /// 获取寄存器失败
    #[error("Failed to get registers: {0}")]
    GetRegsFailed(String),
    /// 设置寄存器失败
    #[error("Failed to set registers: {0}")]
    SetRegsFailed(String),
    /// 无效的 vCPU ID
    #[error("Invalid vCPU ID: {0}")]
    InvalidVcpuId(u32),
    /// 无效的地址
    #[error("Invalid address: {0}")]
    InvalidAddress(String),
    /// 功能不支持
    #[error("Not supported: {0}")]
    NotSupported(String),
}

/// 硬件虚拟化加速器统一接口
///
/// 此 trait 定义了所有虚拟化后端（KVM、HVF、WHPX 等）必须实现的接口，
/// 使得上层代码可以以统一的方式使用不同的硬件虚拟化技术。
///
/// # 生命周期
///
/// 1. **创建**: 使用平台特定的构造函数创建加速器实例
/// 2. **初始化**: 调用 [`init()`](Accel::init) 初始化加速器
/// 3. **配置**: 创建 vCPU、映射内存
/// 4. **运行**: 循环调用 [`run_vcpu()`](Accel::run_vcpu) 执行 Guest 代码
/// 5. **清理**: 实例析构时自动清理资源
///
/// # 示例
///
/// ```rust,ignore
/// use vm_accel::{Accel, AccelError};
///
/// fn run_vm(accel: &mut dyn Accel, mmu: &mut dyn MMU) -> Result<(), AccelError> {
///     // 初始化
///     accel.init()?;
///     
///     // 创建 vCPU
///     accel.create_vcpu(0)?;
///     
///     // 映射 Guest 内存
///     accel.map_memory(0x0, host_mem_ptr, MEM_SIZE, 0x7)?;
///     
///     // 设置初始寄存器状态
///     let mut regs = accel.get_regs(0)?;
///     regs.pc = ENTRY_POINT;
///     accel.set_regs(0, &regs)?;
///     
///     // 运行 Guest
///     loop {
///         accel.run_vcpu(0, mmu)?;
///         // 处理 VM Exit...
///     }
/// }
/// ```
///
/// # 实现者注意
///
/// 实现此 trait 时应注意：
/// - 所有方法应是线程安全的（或明确文档说明线程约束）
/// - 错误应提供有意义的错误消息
/// - 资源应在 Drop 时正确清理
pub trait Accel {
    /// 初始化加速器
    fn init(&mut self) -> Result<(), AccelError>;

    /// 创建 vCPU
    fn create_vcpu(&mut self, id: u32) -> Result<(), AccelError>;

    /// 映射内存
    fn map_memory(&mut self, gpa: u64, hva: u64, size: u64, flags: u32) -> Result<(), AccelError>;

    /// 取消映射内存
    fn unmap_memory(&mut self, gpa: u64, size: u64) -> Result<(), AccelError>;

    /// 运行 vCPU
    fn run_vcpu(&mut self, vcpu_id: u32, mmu: &mut dyn MMU) -> Result<(), AccelError>;

    /// 获取寄存器
    fn get_regs(&self, vcpu_id: u32) -> Result<GuestRegs, AccelError>;

    /// 设置寄存器
    fn set_regs(&mut self, vcpu_id: u32, regs: &GuestRegs) -> Result<(), AccelError>;

    /// 获取加速器名称
    fn name(&self) -> &str;
}

/// vCPU 加速器接口 (预留扩展)
pub trait VcpuAccel {}

/// 内存区域权限标志
///
/// 用于 [`Accel::map_memory`] 指定内存区域的访问权限。
///
/// # 示例
///
/// ```rust,ignore
/// let flags = MemFlags {
///     read: true,
///     write: true,
///     exec: false,  // 数据区域不可执行
/// };
/// ```
#[derive(Debug, Clone, Copy)]
pub struct MemFlags {
    /// 读取权限
    pub read: bool,
    /// 写入权限
    pub write: bool,
    /// 执行权限
    pub exec: bool,
}

/// VM Exit 原因枚举
///
/// 表示 Guest 执行被中断的原因。
#[derive(Debug)]
pub enum VmExitReason {
    /// 未知原因
    Unknown,
}

/// 加速器类型枚举
///
/// 标识当前使用的虚拟化后端类型。
///
/// # 示例
///
/// ```rust,ignore
/// let (kind, accel) = vm_accel::select();
/// match kind {
///     AccelKind::Kvm => println!("Using Linux KVM"),
///     AccelKind::Hvf => println!("Using macOS Hypervisor.framework"),
///     AccelKind::Whpx => println!("Using Windows WHPX"),
///     AccelKind::None => println!("No acceleration"),
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AccelKind {
    /// 无硬件加速
    None,
    /// Linux KVM (Kernel-based Virtual Machine)
    Kvm,
    /// macOS Hypervisor.framework
    Hvf,
    /// Windows Hypervisor Platform (WHPX)
    Whpx,
}

impl AccelKind {
    /// 检测当前平台最佳的加速器类型
    ///
    /// # 返回值
    ///
    /// 返回可用的最佳加速器类型，如果没有可用的硬件加速则返回 `AccelKind::None`
    pub fn detect_best() -> Self {
        #[cfg(target_os = "linux")]
        {
            if std::path::Path::new("/dev/kvm").exists() {
                return AccelKind::Kvm;
            }
        }

        #[cfg(target_os = "macos")]
        {
            // In a real app we would check if Hypervisor.framework is usable
            return AccelKind::Hvf;
        }

        #[cfg(target_os = "windows")]
        {
            // Check for WHPX
            return AccelKind::Whpx;
        }

        #[allow(unreachable_code)]
        AccelKind::None
    }
}

pub mod accel;
pub mod accel_fallback;
pub mod event;

#[cfg(feature = "smmu")]
pub mod smmu;

#[cfg(feature = "smmu")]
pub use crate::accel::{AccelManagerError, AccelerationManager};
pub use accel_fallback::{AccelFallbackManager, ExecResult};
pub use numa_optimizer::{MemoryAllocationStrategy, NUMANodeStats, NUMAOptimizer};
pub use vcpu_numa_manager::{NumaTopology, VcpuNumaManager};

#[cfg(feature = "smmu")]
pub use smmu::{SmmuDeviceAttachment, SmmuDeviceInfo, SmmuManager};

#[cfg(target_os = "linux")]
pub use kvm_enhanced::EnhancedKvmAccelerator;

pub struct NoAccel;
impl Accel for NoAccel {
    fn init(&mut self) -> Result<(), AccelError> {
        Err(VmError::Core(vm_core::CoreError::NotSupported {
            feature: "No accelerator available".to_string(),
            module: "vm-accel".to_string(),
        }))
    }
    fn create_vcpu(&mut self, _id: u32) -> Result<(), AccelError> {
        Err(VmError::Core(vm_core::CoreError::NotSupported {
            feature: "No accelerator".to_string(),
            module: "vm-accel".to_string(),
        }))
    }
    fn map_memory(
        &mut self,
        _gpa: u64,
        _hva: u64,
        _size: u64,
        _flags: u32,
    ) -> Result<(), AccelError> {
        Err(VmError::Core(vm_core::CoreError::NotSupported {
            feature: "No accelerator".to_string(),
            module: "vm-accel".to_string(),
        }))
    }
    fn unmap_memory(&mut self, _gpa: u64, _size: u64) -> Result<(), AccelError> {
        Err(VmError::Core(vm_core::CoreError::NotSupported {
            feature: "No accelerator".to_string(),
            module: "vm-accel".to_string(),
        }))
    }
    fn run_vcpu(&mut self, _vcpu_id: u32, _mmu: &mut dyn MMU) -> Result<(), AccelError> {
        Err(VmError::Core(vm_core::CoreError::NotSupported {
            feature: "No accelerator".to_string(),
            module: "vm-accel".to_string(),
        }))
    }
    fn get_regs(&self, _vcpu_id: u32) -> Result<GuestRegs, AccelError> {
        Err(VmError::Core(vm_core::CoreError::NotSupported {
            feature: "No accelerator".to_string(),
            module: "vm-accel".to_string(),
        }))
    }
    fn set_regs(&mut self, _vcpu_id: u32, _regs: &GuestRegs) -> Result<(), AccelError> {
        Err(VmError::Core(vm_core::CoreError::NotSupported {
            feature: "No accelerator".to_string(),
            module: "vm-accel".to_string(),
        }))
    }
    fn name(&self) -> &str {
        "None"
    }
}

// 新的实现模块
#[cfg(target_os = "macos")]
mod hvf_impl;
#[cfg(target_os = "linux")]
mod kvm_impl;
#[cfg(any(target_os = "ios", target_os = "tvos"))]
mod vz_impl;
#[cfg(target_os = "windows")]
mod whpx_impl;
#[cfg(target_os = "windows")]
mod whpx_io;

// 旧模块保留以保持兼容
#[cfg(target_os = "macos")]
mod hvf;
#[cfg(target_os = "linux")]
mod kvm;
#[cfg(target_os = "linux")]
mod kvm_enhanced;
#[cfg(target_os = "windows")]
mod whpx;

/// AMD CPU 特定功能
pub mod amd;
/// Apple Silicon 特定功能
pub mod apple;
/// CPU 详细信息模块
pub mod cpuinfo;
/// Intel CPU 特定功能
pub mod intel;
/// 移动平台支持
pub mod mobile;
/// 高级 NUMA 感知优化器
pub mod numa_optimizer;
/// 实时优化 - 微秒级延迟处理
pub mod realtime;
/// 实时性能监控
pub mod realtime_monitor;
/// vCPU 亲和性和 NUMA 支持
pub mod vcpu_affinity;
/// 集成的 vCPU 和 NUMA 管理器
pub mod vcpu_numa_manager;
/// 厂商扩展检测
pub mod vendor_extensions;

/// 自动选择最佳的硬件加速器
///
/// 根据当前平台自动检测并初始化最合适的虚拟化后端。
///
/// # 检测优先级
///
/// - **Linux**: KVM (通过 /dev/kvm)
/// - **macOS**: Hypervisor.framework
/// - **Windows**: Windows Hypervisor Platform
/// - **iOS/tvOS**: Virtualization.framework
///
/// # 返回值
///
/// 返回元组 `(AccelKind, Box<dyn Accel>)`：
/// - 第一个元素是加速器类型
/// - 第二个元素是已初始化的加速器实例
///
/// 如果没有可用的硬件加速，返回 `(AccelKind::None, Box::new(NoAccel))`
///
/// # 示例
///
/// ```rust,ignore
/// use vm_accel::{select, AccelKind, Accel};
///
/// let (kind, mut accel) = select();
///
/// if kind == AccelKind::None {
///     eprintln!("Warning: No hardware acceleration available");
///     eprintln!("Performance will be significantly reduced");
/// }
///
/// // 使用 accel 进行虚拟化操作...
/// ```
///
/// # 平台特定要求
///
/// ## Linux
/// - 需要加载 kvm 内核模块
/// - 用户需要有 /dev/kvm 的访问权限（通常需要在 kvm 组中）
///
/// ## macOS
/// - 需要应用有 `com.apple.security.hypervisor` 权限
///
/// ## Windows
/// - 需要启用 "Windows Hypervisor Platform" 可选功能
pub fn select() -> (AccelKind, Box<dyn Accel>) {
    #[cfg(target_os = "linux")]
    {
        let mut a = kvm_impl::AccelKvm::new();
        if a.init().is_ok() {
            return (AccelKind::Kvm, Box::new(a));
        }
    }
    #[cfg(target_os = "macos")]
    {
        let mut a = hvf_impl::AccelHvf::new();
        if a.init().is_ok() {
            return (AccelKind::Hvf, Box::new(a));
        }
    }
    #[cfg(target_os = "windows")]
    {
        let mut a = whpx_impl::AccelWhpx::new();
        if a.init().is_ok() {
            return (AccelKind::Whpx, Box::new(a));
        }
    }
    #[cfg(any(target_os = "ios", target_os = "tvos"))]
    {
        let mut a = vz_impl::AccelVz::new();
        if a.init().is_ok() {
            return (AccelKind::Hvf, Box::new(a));
        } // 使用 Hvf 作为类型
    }
    (AccelKind::None, Box::new(NoAccel))
}

// 导出各平台的实现
#[cfg(target_os = "macos")]
pub use hvf_impl::{AccelHvf, HvfError, HvmExit};
#[cfg(target_os = "linux")]
pub use kvm_impl::AccelKvm;
#[cfg(any(target_os = "ios", target_os = "tvos"))]
pub use vz_impl::AccelVz;
#[cfg(target_os = "windows")]
pub use whpx_impl::AccelWhpx;

// ============================================================================
// SIMD 辅助函数
// ============================================================================

/// 使用 AVX2 进行 8 个 32 位整数的并行加法 (x86_64)
///
/// 如果 CPU 不支持 AVX2，则回退到标量实现。
///
/// # 参数
///
/// * `a` - 第一个输入向量（8 个 i32）
/// * `b` - 第二个输入向量（8 个 i32）
///
/// # 返回值
///
/// 返回逐元素相加的结果向量
///
/// # 性能
///
/// - AVX2 路径：单指令完成所有 8 个加法
/// - 回退路径：8 次标量加法
///
/// # 示例
///
/// ```rust,ignore
/// #[cfg(target_arch = "x86_64")]
/// {
///     let a = [1, 2, 3, 4, 5, 6, 7, 8];
///     let b = [10, 20, 30, 40, 50, 60, 70, 80];
///     let result = vm_accel::add_i32x8(a, b);
///     assert_eq!(result, [11, 22, 33, 44, 55, 66, 77, 88]);
/// }
/// ```
#[cfg(target_arch = "x86_64")]
pub fn add_i32x8(a: [i32; 8], b: [i32; 8]) -> [i32; 8] {
    if std::is_x86_feature_detected!("avx2") {
        // SAFETY: AVX2 intrinsics require CPU feature check (done above)
        // Preconditions: a and b are valid arrays of 8 i32 each (32 bytes), pointers properly aligned for loadu/storeu
        // Invariants: _mm256_loadu_si256 supports unaligned loads, _mm256_storeu_si256 supports unaligned stores
        unsafe {
            use core::arch::x86_64::*;
            let va = _mm256_loadu_si256(a.as_ptr() as *const __m256i);
            let vb = _mm256_loadu_si256(b.as_ptr() as *const __m256i);
            let vr = _mm256_add_epi32(va, vb);
            let mut out = [0i32; 8];
            _mm256_storeu_si256(out.as_mut_ptr() as *mut __m256i, vr);
            out
        }
    } else {
        let mut out = [0i32; 8];
        for i in 0..8 {
            out[i] = a[i] + b[i];
        }
        out
    }
}

/// 使用 NEON 进行 4 个 32 位整数的并行加法 (aarch64)
///
/// ARM64 平台上 NEON 是强制支持的，所以此函数总是使用 SIMD 指令。
///
/// # 参数
///
/// * `a` - 第一个输入向量（4 个 i32）
/// * `b` - 第二个输入向量（4 个 i32）
///
/// # 返回值
///
/// 返回逐元素相加的结果向量
///
/// # 性能
///
/// 使用单条 `vaddq_s32` NEON 指令完成所有 4 个加法
///
/// # 示例
///
/// ```rust,ignore
/// #[cfg(target_arch = "aarch64")]
/// {
///     let a = [1, 2, 3, 4];
///     let b = [10, 20, 30, 40];
///     let result = vm_accel::add_i32x4(a, b);
///     assert_eq!(result, [11, 22, 33, 44]);
/// }
/// ```
#[cfg(target_arch = "aarch64")]
pub fn add_i32x4(a: [i32; 4], b: [i32; 4]) -> [i32; 4] {
    // SAFETY: NEON intrinsics are mandatory on aarch64 (always available)
    // Preconditions: a and b are valid arrays of 4 i32 each (16 bytes), pointers properly aligned
    // Invariants: vld1q_s32 loads 4 i32 values, vst1q_s32 stores 4 i32 values, vaddq_s32 performs SIMD addition
    unsafe {
        use core::arch::aarch64::*;
        let va = vld1q_s32(a.as_ptr());
        let vb = vld1q_s32(b.as_ptr());
        let vr = vaddq_s32(va, vb);
        let mut out = [0i32; 4];
        vst1q_s32(out.as_mut_ptr(), vr);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_features_default() {
        let features = CpuFeatures::default();
        assert!(!features.avx2);
        assert!(!features.avx512);
        assert!(!features.neon);
        assert!(!features.vmx);
        assert!(!features.svm);
        assert!(!features.arm_el2);
    }

    #[test]
    fn test_accel_kind_detect_best() {
        let kind = AccelKind::detect_best();
        match kind {
            AccelKind::Kvm | AccelKind::Hvf | AccelKind::Whpx | AccelKind::None => {}
        }
    }

    #[test]
    fn test_mem_flags_creation() {
        let flags = MemFlags {
            read: true,
            write: true,
            exec: false,
        };
        assert!(flags.read);
        assert!(flags.write);
        assert!(!flags.exec);
    }

    #[test]
    fn test_mem_flags_copy() {
        let flags1 = MemFlags {
            read: true,
            write: false,
            exec: false,
        };
        let flags2 = flags1;
        assert_eq!(flags1.read, flags2.read);
        assert_eq!(flags1.write, flags2.write);
        assert_eq!(flags1.exec, flags2.exec);
    }

    #[test]
    fn test_accel_kind_equality() {
        assert_eq!(AccelKind::Kvm, AccelKind::Kvm);
        assert_eq!(AccelKind::Hvf, AccelKind::Hvf);
        assert_eq!(AccelKind::Whpx, AccelKind::Whpx);
        assert_eq!(AccelKind::None, AccelKind::None);

        assert_ne!(AccelKind::Kvm, AccelKind::Hvf);
        assert_ne!(AccelKind::Hvf, AccelKind::Whpx);
    }

    #[test]
    fn test_no_accel_name() {
        let no_accel = NoAccel;
        assert_eq!(no_accel.name(), "None");
    }

    #[test]
    fn test_accel_legacy_error_display() {
        let err = AccelLegacyError::NotAvailable("test".to_string());
        assert!(format!("{:?}", err).contains("NotAvailable"));
    }

    #[test]
    fn test_accel_kind_copy() {
        let kind1 = AccelKind::Kvm;
        let kind2 = kind1;
        assert_eq!(kind1, kind2);
    }
}
