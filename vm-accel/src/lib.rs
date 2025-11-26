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
             features.svm = info.has_svm();
             features.avx2 = info.has_avx2();
             features.avx512 = info.has_avx512f();
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

pub trait Accel {
    fn init(&mut self) -> bool;
    fn map_memory(&mut self, guest_pa: u64, size: u64) -> bool { let _ = (guest_pa, size); false }
    fn create_vcpu(&mut self, id: u32) -> bool { let _ = id; false }
    fn run(&mut self) -> bool { false }
    fn inject_interrupt(&mut self, vector: u32) -> bool { let _ = vector; false }
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
    fn init(&mut self) -> bool { false }
}

#[cfg(target_os = "linux")]
mod kvm;
#[cfg(target_os = "macos")]
mod hvf;
#[cfg(target_os = "windows")]
mod whpx;

pub fn select() -> (AccelKind, Box<dyn Accel>) {
    #[cfg(target_os = "linux")]
    {
        let mut a = kvm::AccelKvm::new();
        if a.init() { return (AccelKind::Kvm, Box::new(a)); }
    }
    #[cfg(target_os = "macos")]
    {
        let mut a = hvf::AccelHvf::new();
        if a.init() { return (AccelKind::Hvf, Box::new(a)); }
    }
    #[cfg(target_os = "windows")]
    {
        let mut a = whpx::AccelWhpx::new();
        if a.init() { return (AccelKind::Whpx, Box::new(a)); }
    }
    (AccelKind::None, Box::new(NoAccel))
}

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
