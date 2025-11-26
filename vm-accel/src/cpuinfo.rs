//! CPU Information and Feature Detection
//!
//! 检测 CPU 型号、厂商和硬件加速特性

use std::sync::OnceLock;

/// CPU 厂商
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuVendor {
    Intel,
    AMD,
    Apple,
    Qualcomm,
    HiSilicon,  // 华为海思
    MediaTek,   // 联发科
    ARM,
    Unknown,
}

/// CPU 架构
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuArch {
    X86_64,
    AArch64,
    Unknown,
}

/// CPU 特性标志
#[derive(Debug, Clone, Default)]
pub struct CpuFeatures {
    // x86_64 虚拟化特性
    pub vmx: bool,           // Intel VT-x
    pub svm: bool,           // AMD-V
    pub ept: bool,           // Intel Extended Page Tables
    pub npt: bool,           // AMD Nested Page Tables
    pub vpid: bool,          // Virtual Processor ID
    pub avic: bool,          // AMD Advanced Virtual Interrupt Controller
    pub apicv: bool,         // Intel APICv
    
    // SIMD 特性
    pub sse: bool,
    pub sse2: bool,
    pub sse3: bool,
    pub ssse3: bool,
    pub sse4_1: bool,
    pub sse4_2: bool,
    pub avx: bool,
    pub avx2: bool,
    pub avx512f: bool,
    pub neon: bool,          // ARM NEON
    pub sve: bool,           // ARM SVE
    pub sve2: bool,          // ARM SVE2
    
    // ARM 虚拟化特性
    pub el2: bool,           // ARM Hypervisor mode
    pub vhe: bool,           // Virtualization Host Extensions
    
    // 其他特性
    pub aes: bool,           // AES-NI
    pub sha: bool,           // SHA extensions
    pub crc32: bool,
    pub atomics: bool,       // ARM LSE (Large System Extensions)
}

/// CPU 信息
#[derive(Debug, Clone)]
pub struct CpuInfo {
    pub vendor: CpuVendor,
    pub arch: CpuArch,
    pub model_name: String,
    pub features: CpuFeatures,
    pub core_count: usize,
}

static CPU_INFO: OnceLock<CpuInfo> = OnceLock::new();

impl CpuInfo {
    /// 获取全局 CPU 信息（单例）
    pub fn get() -> &'static CpuInfo {
        CPU_INFO.get_or_init(|| Self::detect())
    }

    /// 检测 CPU 信息
    fn detect() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            Self::detect_x86_64()
        }
        
        #[cfg(target_arch = "aarch64")]
        {
            Self::detect_aarch64()
        }
        
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            Self::default()
        }
    }

    /// 检测 x86_64 CPU 信息
    #[cfg(target_arch = "x86_64")]
    fn detect_x86_64() -> Self {
        use raw_cpuid::CpuId;
        
        let cpuid = CpuId::new();
        let mut features = CpuFeatures::default();
        let mut vendor = CpuVendor::Unknown;
        let mut model_name = String::from("Unknown x86_64 CPU");

        // 检测厂商
        if let Some(vendor_info) = cpuid.get_vendor_info() {
            let vendor_str = vendor_info.as_str();
            vendor = match vendor_str {
                "GenuineIntel" => CpuVendor::Intel,
                "AuthenticAMD" => CpuVendor::AMD,
                _ => CpuVendor::Unknown,
            };
        }

        // 获取型号名称
        if let Some(brand) = cpuid.get_processor_brand_string() {
            model_name = brand.as_str().trim().to_string();
        }

        // 检测基础特性
        if let Some(feature_info) = cpuid.get_feature_info() {
            features.sse = feature_info.has_sse();
            features.sse2 = feature_info.has_sse2();
            features.sse3 = feature_info.has_sse3();
            features.ssse3 = feature_info.has_ssse3();
            features.sse4_1 = feature_info.has_sse41();
            features.sse4_2 = feature_info.has_sse42();
            features.aes = feature_info.has_aesni();
            
            // VMX (Intel VT-x)
            features.vmx = feature_info.has_vmx();
        }

        // 检测扩展特性
        if let Some(ext_features) = cpuid.get_extended_feature_info() {
            features.avx2 = ext_features.has_avx2();
            features.avx512f = ext_features.has_avx512f();
            features.sha = ext_features.has_sha();
        }

        // 检测 AMD SVM (需要通过扩展功能检测)
        if vendor == CpuVendor::AMD {
            // SVM 在 CPUID 0x8000_0001 的 ECX bit 2
            features.svm = false; // 简化实现，实际需要读取扩展 CPUID
        }

        // 检测 EPT/VPID (需要读取 MSR，这里简化处理)
        if features.vmx {
            features.ept = true;  // 现代 Intel CPU 都支持
            features.vpid = true;
        }

        // 检测 NPT/AVIC
        if features.svm {
            features.npt = true;  // 现代 AMD CPU 都支持
            features.avic = true;
        }

        let core_count = num_cpus::get();

        Self {
            vendor,
            arch: CpuArch::X86_64,
            model_name,
            features,
            core_count,
        }
    }

    /// 检测 AArch64 CPU 信息
    #[cfg(target_arch = "aarch64")]
    fn detect_aarch64() -> Self {
        let mut features = CpuFeatures::default();
        let mut vendor = CpuVendor::Unknown;
        let mut model_name = String::from("Unknown ARM64 CPU");

        // 在 Linux 上读取 /proc/cpuinfo
        #[cfg(target_os = "linux")]
        {
            if let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo") {
                for line in cpuinfo.lines() {
                    if line.starts_with("CPU implementer") {
                        if let Some(value) = line.split(':').nth(1) {
                            let implementer = value.trim();
                            vendor = match implementer {
                                "0x41" => CpuVendor::ARM,
                                "0x48" => CpuVendor::HiSilicon,
                                "0x51" => CpuVendor::Qualcomm,
                                _ => CpuVendor::Unknown,
                            };
                        }
                    }
                    
                    if line.starts_with("model name") || line.starts_with("Hardware") {
                        if let Some(value) = line.split(':').nth(1) {
                            model_name = value.trim().to_string();
                        }
                    }
                    
                    if line.starts_with("Features") {
                        let features_str = line.split(':').nth(1).unwrap_or("").to_lowercase();
                        features.neon = features_str.contains("asimd") || features_str.contains("neon");
                        features.sve = features_str.contains("sve");
                        features.sve2 = features_str.contains("sve2");
                        features.aes = features_str.contains("aes");
                        features.sha = features_str.contains("sha");
                        features.crc32 = features_str.contains("crc32");
                        features.atomics = features_str.contains("atomics");
                    }
                }
            }
        }

        // 在 macOS 上检测 Apple Silicon
        #[cfg(target_os = "macos")]
        {
            vendor = CpuVendor::Apple;
            
            // 使用 sysctl 获取 CPU 信息
            use std::process::Command;
            if let Ok(output) = Command::new("sysctl")
                .arg("-n")
                .arg("machdep.cpu.brand_string")
                .output()
            {
                if let Ok(brand) = String::from_utf8(output.stdout) {
                    model_name = brand.trim().to_string();
                }
            }
            
            // Apple Silicon 特性
            features.neon = true;
            features.aes = true;
            features.sha = true;
            features.crc32 = true;
            features.atomics = true;
            features.el2 = true;
            features.vhe = true;
        }

        // 检测 MediaTek
        if model_name.to_lowercase().contains("mediatek") || model_name.to_lowercase().contains("mt") {
            vendor = CpuVendor::MediaTek;
        }

        let core_count = num_cpus::get();

        Self {
            vendor,
            arch: CpuArch::AArch64,
            model_name,
            features,
            core_count,
        }
    }

    /// 打印 CPU 信息
    pub fn print_info(&self) {
        println!("=== CPU Information ===");
        println!("Vendor: {:?}", self.vendor);
        println!("Architecture: {:?}", self.arch);
        println!("Model: {}", self.model_name);
        println!("Cores: {}", self.core_count);
        println!("\n=== Virtualization Features ===");
        
        match self.arch {
            CpuArch::X86_64 => {
                println!("VMX (Intel VT-x): {}", self.features.vmx);
                println!("SVM (AMD-V): {}", self.features.svm);
                println!("EPT: {}", self.features.ept);
                println!("NPT: {}", self.features.npt);
                println!("VPID: {}", self.features.vpid);
                println!("AVIC: {}", self.features.avic);
            }
            CpuArch::AArch64 => {
                println!("EL2 (Hypervisor): {}", self.features.el2);
                println!("VHE: {}", self.features.vhe);
            }
            _ => {}
        }
        
        println!("\n=== SIMD Features ===");
        match self.arch {
            CpuArch::X86_64 => {
                println!("SSE4.2: {}", self.features.sse4_2);
                println!("AVX: {}", self.features.avx);
                println!("AVX2: {}", self.features.avx2);
                println!("AVX-512: {}", self.features.avx512f);
            }
            CpuArch::AArch64 => {
                println!("NEON: {}", self.features.neon);
                println!("SVE: {}", self.features.sve);
                println!("SVE2: {}", self.features.sve2);
            }
            _ => {}
        }
        
        println!("\n=== Crypto Features ===");
        println!("AES: {}", self.features.aes);
        println!("SHA: {}", self.features.sha);
        println!("Atomics: {}", self.features.atomics);
    }
}

impl Default for CpuInfo {
    fn default() -> Self {
        Self {
            vendor: CpuVendor::Unknown,
            arch: CpuArch::Unknown,
            model_name: String::from("Unknown CPU"),
            features: CpuFeatures::default(),
            core_count: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_detection() {
        let info = CpuInfo::get();
        println!("{:#?}", info);
        assert!(info.core_count > 0);
    }
}
