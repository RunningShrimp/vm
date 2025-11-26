#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), feature = "cpuid"))]
use raw_cpuid::CpuId;

#[derive(Debug, Clone, Copy, Default)]
pub struct CpuFeatures {
    pub avx2: bool,
    pub avx512: bool,
    pub neon: bool,
    pub vmx: bool, // Intel VT-x
    pub svm: bool, // AMD-V
    pub arm_el2: bool, // ARM Virtualization
}

pub fn detect() -> CpuFeatures {
    let mut features = CpuFeatures::default();

    #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), feature = "cpuid"))]
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

// 新的统一 Accel trait
use vm_core::{GuestRegs, MMU};

#[derive(Debug, thiserror::Error)]
pub enum AccelError {
    #[error("Accelerator not available: {0}")]
    NotAvailable(String),
    #[error("Accelerator not initialized: {0}")]
    NotInitialized(String),
    #[error("Initialization failed: {0}")]
    InitFailed(String),
    #[error("Failed to create VM: {0}")]
    CreateVmFailed(String),
    #[error("Failed to create vCPU: {0}")]
    CreateVcpuFailed(String),
    #[error("Failed to map memory: {0}")]
    MapMemoryFailed(String),
    #[error("Failed to unmap memory: {0}")]
    UnmapMemoryFailed(String),
    #[error("Failed to run vCPU: {0}")]
    RunFailed(String),
    #[error("Failed to get registers: {0}")]
    GetRegsFailed(String),
    #[error("Failed to set registers: {0}")]
    SetRegsFailed(String),
    #[error("Invalid vCPU ID: {0}")]
    InvalidVcpuId(u32),
    #[error("Invalid address: {0}")]
    InvalidAddress(String),
    #[error("Not supported: {0}")]
    NotSupported(String),
}

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

pub trait VcpuAccel {}

#[derive(Debug, Clone, Copy)]
pub struct MemFlags {
    pub read: bool,
    pub write: bool,
    pub exec: bool,
}

#[derive(Debug)]
pub enum VmExitReason { Unknown }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccelKind {
    None,
    Kvm,
    Hvf,
    Whpx,
}

impl AccelKind {
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

        AccelKind::None
    }
}

pub struct NoAccel;
impl Accel for NoAccel {
    fn init(&mut self) -> Result<(), AccelError> {
        Err(AccelError::NotAvailable("No accelerator available".to_string()))
    }
    fn create_vcpu(&mut self, _id: u32) -> Result<(), AccelError> {
        Err(AccelError::NotSupported("No accelerator".to_string()))
    }
    fn map_memory(&mut self, _gpa: u64, _hva: u64, _size: u64, _flags: u32) -> Result<(), AccelError> {
        Err(AccelError::NotSupported("No accelerator".to_string()))
    }
    fn unmap_memory(&mut self, _gpa: u64, _size: u64) -> Result<(), AccelError> {
        Err(AccelError::NotSupported("No accelerator".to_string()))
    }
    fn run_vcpu(&mut self, _vcpu_id: u32, _mmu: &mut dyn MMU) -> Result<(), AccelError> {
        Err(AccelError::NotSupported("No accelerator".to_string()))
    }
    fn get_regs(&self, _vcpu_id: u32) -> Result<GuestRegs, AccelError> {
        Err(AccelError::NotSupported("No accelerator".to_string()))
    }
    fn set_regs(&mut self, _vcpu_id: u32, _regs: &GuestRegs) -> Result<(), AccelError> {
        Err(AccelError::NotSupported("No accelerator".to_string()))
    }
    fn name(&self) -> &str { "None" }
}

// 新的实现模块
#[cfg(target_os = "linux")]
mod kvm_impl;
#[cfg(target_os = "macos")]
mod hvf_impl;
#[cfg(target_os = "windows")]
mod whpx_impl;
#[cfg(any(target_os = "ios", target_os = "tvos"))]
mod vz_impl;

// 旧模块保留以保持兼容
#[cfg(target_os = "linux")]
mod kvm;
#[cfg(target_os = "macos")]
mod hvf;
#[cfg(target_os = "windows")]
mod whpx;

pub mod cpuinfo;
pub mod intel;
pub mod amd;
pub mod apple;
pub mod mobile;

// 新的 select 函数
pub fn select() -> (AccelKind, Box<dyn Accel>) {
    #[cfg(target_os = "linux")]
    {
        let mut a = kvm_impl::AccelKvm::new();
        if a.init().is_ok() { return (AccelKind::Kvm, Box::new(a)); }
    }
    #[cfg(target_os = "macos")]
    {
        let mut a = hvf_impl::AccelHvf::new();
        if a.init().is_ok() { return (AccelKind::Hvf, Box::new(a)); }
    }
    #[cfg(target_os = "windows")]
    {
        let mut a = whpx_impl::AccelWhpx::new();
        if a.init().is_ok() { return (AccelKind::Whpx, Box::new(a)); }
    }
    #[cfg(any(target_os = "ios", target_os = "tvos"))]
    {
        let mut a = vz_impl::AccelVz::new();
        if a.init().is_ok() { return (AccelKind::Hvf, Box::new(a)); } // 使用 Hvf 作为类型
    }
    (AccelKind::None, Box::new(NoAccel))
}

// 导出各平台的实现
#[cfg(target_os = "linux")]
pub use kvm_impl::AccelKvm;
#[cfg(target_os = "macos")]
pub use hvf_impl::AccelHvf;
#[cfg(target_os = "windows")]
pub use whpx_impl::AccelWhpx;
#[cfg(any(target_os = "ios", target_os = "tvos"))]
pub use vz_impl::AccelVz;

#[cfg(all(target_arch = "x86_64"))]
pub fn add_i32x8(a: [i32; 8], b: [i32; 8]) -> [i32; 8] {
    if std::is_x86_feature_detected!("avx2") {
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
        for i in 0..8 { out[i] = a[i] + b[i]; }
        out
    }
}

#[cfg(all(target_arch = "aarch64"))]
pub fn add_i32x4(a: [i32; 4], b: [i32; 4]) -> [i32; 4] {
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
