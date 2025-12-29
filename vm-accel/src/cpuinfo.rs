//! CPU Information and Feature Detection
//!
//! 检测 CPU 型号、厂商和硬件加速特性

#![cfg(feature = "acceleration")]

use std::sync::OnceLock;

/// CPU 厂商
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuVendor {
    Intel,
    AMD,
    Apple,
    Qualcomm,
    HiSilicon, // 华为海思
    MediaTek,  // 联发科
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
    pub vmx: bool,    // Intel VT-x
    pub svm: bool,    // AMD-V
    pub ept: bool,    // Intel Extended Page Tables
    pub npt: bool,    // AMD Nested Page Tables
    pub vpid: bool,   // Virtual Processor ID
    pub avic: bool,   // AMD Advanced Virtual Interrupt Controller
    pub x2avic: bool, // AMD x2AVIC (Extended AVIC)
    pub apicv: bool,  // Intel APICv

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
    pub neon: bool, // ARM NEON
    pub sve: bool,  // ARM SVE
    pub sve2: bool, // ARM SVE2

    // ARM 虚拟化特性
    pub el2: bool, // ARM Hypervisor mode
    pub vhe: bool, // Virtualization Host Extensions

    // 其他特性
    pub aes: bool, // AES-NI
    pub sha: bool, // SHA extensions
    pub crc32: bool,
    pub atomics: bool, // ARM LSE (Large System Extensions)

    // 厂商特有扩展
    pub amx: bool,         // Apple AMX (Apple Matrix Coprocessor)
    pub hexagon_dsp: bool, // Qualcomm Hexagon DSP
    pub apu: bool,         // MediaTek APU (AI Processing Unit)
    pub npu: bool,         // HiSilicon NPU (Neural Processing Unit)
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
        CPU_INFO.get_or_init(Self::detect)
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
            if let Some(ext_proc_info) = cpuid.get_extended_processor_and_feature_identifiers() {
                features.svm = ext_proc_info.has_svm();
            } else {
                features.svm = false;
            }
        }

        // 检测 EPT/VPID (需要读取 MSR，这里简化处理)
        if features.vmx {
            features.ept = true; // 现代 Intel CPU 都支持
            features.vpid = true;
        }

        // 检测 NPT/AVIC
        if features.svm {
            features.npt = true; // 现代 AMD CPU 都支持
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
        #[allow(unused_assignments)]
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
                        features.neon =
                            features_str.contains("asimd") || features_str.contains("neon");
                        features.sve = features_str.contains("sve");
                        features.sve2 = features_str.contains("sve2");
                        features.aes = features_str.contains("aes");
                        features.sha = features_str.contains("sha");
                        features.crc32 = features_str.contains("crc32");
                        features.atomics = features_str.contains("atomics");
                    }
                }

                // 检测厂商特有扩展
                match vendor {
                    CpuVendor::Qualcomm => {
                        // 高通骁龙8系列支持Hexagon DSP
                        if model_name.to_lowercase().contains("snapdragon") {
                            features.hexagon_dsp = true;
                        }
                    }
                    CpuVendor::MediaTek => {
                        // 联发科天玑系列支持APU
                        if model_name.to_lowercase().contains("dimensity") {
                            features.apu = true;
                        }
                    }
                    CpuVendor::HiSilicon => {
                        // 华为麒麟系列支持NPU
                        if model_name.to_lowercase().contains("kirin") {
                            features.npu = true;
                        }
                    }
                    _ => {}
                }
            }
        }

        // 在 macOS 上检测 Apple Silicon
        #[cfg(target_os = "macos")]
        {
            vendor = CpuVendor::Apple;

            // 使用 sysctl 获取 CPU 信息
            use std::process::Command;
            let output = Command::new("sysctl")
                .arg("-n")
                .arg("machdep.cpu.brand_string")
                .output();
            if let Ok(output) = output
                && let Ok(brand) = String::from_utf8(output.stdout)
            {
                model_name = brand.trim().to_string();
            }

            // Apple Silicon 特性
            features.neon = true;
            features.aes = true;
            features.sha = true;
            features.crc32 = true;
            features.atomics = true;
            features.el2 = true;
            features.vhe = true;

            // 检测 AMX 支持（Apple M1及以后支持）
            // AMX 通过系统寄存器访问，需要运行时检测
            // 这里基于型号推断：M1/M2/M3/M4都支持AMX
            let model_lower = model_name.to_lowercase();
            features.amx = model_lower.contains("m1")
                || model_lower.contains("m2")
                || model_lower.contains("m3")
                || model_lower.contains("m4");
        }

        // 检测 MediaTek (适用于所有平台)
        if model_name.to_lowercase().contains("mediatek")
            || model_name.to_lowercase().contains("mt")
        {
            vendor = CpuVendor::MediaTek;
        }

        // 如果在非Linux和非macOS平台上仍未确定vendor，尝试从model_name推断
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let model_lower = model_name.to_lowercase();
            if model_lower.contains("qualcomm") || model_lower.contains("snapdragon") {
                vendor = CpuVendor::Qualcomm;
            } else if model_lower.contains("mediatek") || model_lower.contains("dimensity") {
                vendor = CpuVendor::MediaTek;
            } else if model_lower.contains("kirin") {
                vendor = CpuVendor::HiSilicon;
            } else if model_lower.contains("apple")
                || model_lower.contains("m1")
                || model_lower.contains("m2")
                || model_lower.contains("m3")
                || model_lower.contains("m4")
            {
                vendor = CpuVendor::Apple;
            } else {
                vendor = CpuVendor::ARM; // 默认为ARM
            }
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

    /// 检测 AMX 支持（运行时检测）
    ///
    /// 在 Apple Silicon 上，AMX 通过系统寄存器访问
    /// 此函数尝试通过读取系统寄存器来检测 AMX 支持
    #[cfg(target_arch = "aarch64")]
    pub fn detect_amx_runtime() -> bool {
        #[cfg(target_os = "macos")]
        {
            // 在 macOS 上，尝试读取 AMX 相关系统寄存器
            // 注意：这需要特权访问，实际实现可能需要内核扩展
            // 这里使用启发式方法：检查是否为 Apple Silicon
            let cpu_info = Self::get();
            cpu_info.vendor == CpuVendor::Apple && cpu_info.features.amx
        }
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }

    #[cfg(not(target_arch = "aarch64"))]
    pub fn detect_amx_runtime() -> bool {
        false
    }

    /// 检测 Hexagon DSP 支持（运行时检测）
    ///
    /// 高通 Hexagon DSP 通常通过设备节点或系统属性访问
    #[cfg(target_arch = "aarch64")]
    pub fn detect_hexagon_dsp_runtime() -> bool {
        #[cfg(target_os = "linux")]
        {
            // 检查是否存在 Hexagon DSP 设备节点
            std::path::Path::new("/sys/kernel/debug/hexagon").exists()
                || std::path::Path::new("/dev/qdsp").exists()
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }

    #[cfg(not(target_arch = "aarch64"))]
    pub fn detect_hexagon_dsp_runtime() -> bool {
        false
    }

    /// 检测 APU 支持（运行时检测）
    ///
    /// 联发科 APU 通常通过设备节点访问
    #[cfg(target_arch = "aarch64")]
    pub fn detect_apu_runtime() -> bool {
        #[cfg(target_os = "linux")]
        {
            // 检查是否存在 APU 设备节点
            std::path::Path::new("/sys/class/misc/mtk_apu").exists()
                || std::path::Path::new("/dev/mtk_apu").exists()
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }

    #[cfg(not(target_arch = "aarch64"))]
    pub fn detect_apu_runtime() -> bool {
        false
    }

    /// 检测 NPU 支持（运行时检测）
    ///
    /// 华为 NPU 通常通过设备节点访问
    #[cfg(target_arch = "aarch64")]
    pub fn detect_npu_runtime() -> bool {
        #[cfg(target_os = "linux")]
        {
            // 检查是否存在 NPU 设备节点
            std::path::Path::new("/dev/davinci_manager").exists()
                || std::path::Path::new("/sys/class/davinci").exists()
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }

    #[cfg(not(target_arch = "aarch64"))]
    pub fn detect_npu_runtime() -> bool {
        false
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

        println!("\n=== Vendor Extensions ===");
        println!("AMX (Apple): {}", self.features.amx);
        println!("Hexagon DSP (Qualcomm): {}", self.features.hexagon_dsp);
        println!("APU (MediaTek): {}", self.features.apu);
        println!("NPU (HiSilicon): {}", self.features.npu);
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

    #[test]
    fn test_svm_detection_on_amd() {
        let info = CpuInfo::get();

        // On AMD CPUs, SVM detection should reflect actual CPU capability
        if info.vendor == CpuVendor::AMD {
            // SVM could be true or false depending on the actual CPU
            // The important thing is that it's not hardcoded
            println!("AMD CPU detected, SVM support: {}", info.features.svm);
        } else {
            // On non-AMD CPUs, SVM should be false
            assert_eq!(
                info.features.svm, false,
                "SVM should be false on non-AMD CPUs"
            );
        }
    }

    #[test]
    fn test_virtualization_features() {
        let info = CpuInfo::get();

        match info.vendor {
            CpuVendor::Intel => {
                // Intel should have VMX detection (could be true or false)
                println!("Intel CPU - VMX: {}", info.features.vmx);
                // SVM should be false on Intel
                assert_eq!(
                    info.features.svm, false,
                    "SVM should be false on Intel CPUs"
                );
            }
            CpuVendor::AMD => {
                // AMD should have SVM detection (could be true or false)
                println!("AMD CPU - SVM: {}", info.features.svm);
                // VMX should be false on AMD
                assert_eq!(info.features.vmx, false, "VMX should be false on AMD CPUs");
            }
            _ => {
                // Other architectures should have both false
                assert_eq!(
                    info.features.vmx, false,
                    "VMX should be false on non-x86 CPUs"
                );
                assert_eq!(
                    info.features.svm, false,
                    "SVM should be false on non-x86 CPUs"
                );
            }
        }
    }
}
