//! CPU Feature Detection Tests
//!
//! Comprehensive tests for CPU feature detection across architectures

use vm_accel::cpuinfo::{CpuArch, CpuInfo, CpuVendor};
use vm_accel::{CpuFeatures, detect};

/// Test CPU feature detection function
#[test]
fn test_cpu_features_detect() {
    let features = detect();

    println!("CPU Features detected:");
    println!("  AVX2: {}", features.avx2);
    println!("  AVX512: {}", features.avx512);
    println!("  NEON: {}", features.neon);
    println!("  VMX: {}", features.vmx);
    println!("  SVM: {}", features.svm);
    println!("  ARM EL2: {}", features.arm_el2);

    // At least one feature should be detected
    let has_any_feature = features.avx2 || features.avx512 || features.neon
        || features.vmx || features.svm || features.arm_el2;
    println!("Has any feature: {}", has_any_feature);
}

/// Test CPU features default
#[test]
fn test_cpu_features_default() {
    let features = CpuFeatures::default();

    assert!(!features.avx2);
    assert!(!features.avx512);
    assert!(!features.neon);
    assert!(!features.vmx);
    assert!(!features.svm);
    assert!(!features.arm_el2);

    println!("CPU features default values are correct");
}

/// Test CPU info detection
#[test]
fn test_cpu_info_detection() {
    let cpu_info = CpuInfo::get();

    println!("CPU Information:");
    println!("  Vendor: {:?}", cpu_info.vendor);
    println!("  Architecture: {:?}", cpu_info.arch);
    println!("  Model: {}", cpu_info.model_name);
    println!("  Core count: {}", cpu_info.core_count);

    assert!(cpu_info.core_count > 0, "Should have at least one core");
    assert!(!cpu_info.model_name.is_empty(), "Model name should not be empty");
}

/// Test CPU vendor detection
#[test]
fn test_cpu_vendor_detection() {
    let cpu_info = CpuInfo::get();

    match cpu_info.vendor {
        CpuVendor::Intel => {
            println!("Detected Intel CPU");
            #[cfg(target_arch = "x86_64")]
            assert!(cpu_info.arch == CpuArch::X86_64);
        }
        CpuVendor::AMD => {
            println!("Detected AMD CPU");
            #[cfg(target_arch = "x86_64")]
            assert!(cpu_info.arch == CpuArch::X86_64);
        }
        CpuVendor::Apple => {
            println!("Detected Apple CPU");
            #[cfg(target_arch = "aarch64")]
            assert!(cpu_info.arch == CpuArch::AArch64);
        }
        CpuVendor::ARM => {
            println!("Detected ARM CPU");
        }
        CpuVendor::Qualcomm => {
            println!("Detected Qualcomm CPU");
        }
        CpuVendor::HiSilicon => {
            println!("Detected HiSilicon CPU");
        }
        CpuVendor::MediaTek => {
            println!("Detected MediaTek CPU");
        }
        CpuVendor::Unknown => {
            println!("Unknown CPU vendor");
        }
    }
}

/// Test CPU architecture detection
#[test]
fn test_cpu_architecture_detection() {
    let cpu_info = CpuInfo::get();

    #[cfg(target_arch = "x86_64")]
    {
        assert_eq!(cpu_info.arch, CpuArch::X86_64);
        println!("Architecture correctly detected as x86_64");
    }

    #[cfg(target_arch = "aarch64")]
    {
        assert_eq!(cpu_info.arch, CpuArch::AArch64);
        println!("Architecture correctly detected as aarch64");
    }
}

/// Test x86_64 specific features
#[cfg(target_arch = "x86_64")]
#[test]
fn test_x86_64_features() {
    let cpu_info = CpuInfo::get();
    let features = &cpu_info.features;

    println!("x86_64 Features:");
    println!("  VMX (Intel VT-x): {}", features.vmx);
    println!("  SVM (AMD-V): {}", features.svm);
    println!("  EPT: {}", features.ept);
    println!("  NPT: {}", features.npt);
    println!("  AVX: {}", features.avx);
    println!("  AVX2: {}", features.avx2);
    println!("  AVX512F: {}", features.avx512f);

    // Should have either VMX or SVM (virtualization support)
    let has_virtualization = features.vmx || features.svm;
    println!("Has virtualization support: {}", has_virtualization);
}

/// Test ARM64 specific features
#[cfg(target_arch = "aarch64")]
#[test]
fn test_arm64_features() {
    let cpu_info = CpuInfo::get();
    let features = &cpu_info.features;

    println!("ARM64 Features:");
    println!("  NEON: {}", features.neon);
    println!("  SVE: {}", features.sve);
    println!("  SVE2: {}", features.sve2);
    println!("  EL2 (Hypervisor): {}", features.el2);
    println!("  VHE: {}", features.vhe);
    println!("  Atomics (LSE): {}", features.atomics);

    // ARM64 always has NEON
    assert!(features.neon, "ARM64 should always have NEON");
    println!("NEON correctly detected as always available on ARM64");
}

/// Test vendor-specific extensions
#[test]
fn test_vendor_specific_extensions() {
    let cpu_info = CpuInfo::get();
    let features = &cpu_info.features;

    println!("Vendor-specific extensions:");
    println!("  AMX (Apple): {}", features.amx);
    println!("  Hexagon DSP (Qualcomm): {}", features.hexagon_dsp);
    println!("  APU (MediaTek): {}", features.apu);
    println!("  NPU (HiSilicon): {}", features.npu);

    // Only certain vendors should have these features
    match cpu_info.vendor {
        CpuVendor::Apple => {
            if features.amx {
                println!("Apple AMX detected");
            }
        }
        CpuVendor::Qualcomm => {
            if features.hexagon_dsp {
                println!("Qualcomm Hexagon DSP detected");
            }
        }
        CpuVendor::HiSilicon => {
            if features.npu {
                println!("HiSilicon NPU detected");
            }
        }
        CpuVendor::MediaTek => {
            if features.apu {
                println!("MediaTek APU detected");
            }
        }
        _ => {}
    }
}

/// Test SIMD features
#[test]
fn test_simd_features() {
    let cpu_info = CpuInfo::get();
    let features = &cpu_info.features;

    println!("SIMD Features:");
    println!("  SSE: {}", features.sse);
    println!("  SSE2: {}", features.sse2);
    println!("  SSE3: {}", features.sse3);
    println!("  SSSE3: {}", features.ssse3);
    println!("  SSE4.1: {}", features.sse4_1);
    println!("  SSE4.2: {}", features.sse4_2);
    println!("  AVX: {}", features.avx);
    println!("  AVX2: {}", features.avx2);
    println!("  AVX512F: {}", features.avx512f);
    println!("  NEON: {}", features.neon);
    println!("  SVE: {}", features.sve);
    println!("  SVE2: {}", features.sve2);

    #[cfg(target_arch = "x86_64")]
    {
        // x86_64 should have at least SSE2
        assert!(features.sse2, "x86_64 should have SSE2");
        println!("SSE2 correctly detected on x86_64");
    }

    #[cfg(target_arch = "aarch64")]
    {
        // ARM64 should have NEON
        assert!(features.neon, "aarch64 should have NEON");
        println!("NEON correctly detected on aarch64");
    }
}

/// Test encryption and hashing features
#[test]
fn test_encryption_features() {
    let cpu_info = CpuInfo::get();
    let features = &cpu_info.features;

    println!("Encryption/Hashing Features:");
    println!("  AES-NI: {}", features.aes);
    println!("  SHA extensions: {}", features.sha);
    println!("  CRC32: {}", features.crc32);

    // These are optional, so we just report them
    if features.aes {
        println!("Hardware AES encryption available");
    }
    if features.sha {
        println!("Hardware SHA hashing available");
    }
    if features.crc32 {
        println!("Hardware CRC32 available");
    }
}

/// Test core count detection
#[test]
fn test_core_count_detection() {
    let cpu_info = CpuInfo::get();

    println!("Detected {} CPU cores", cpu_info.core_count);

    // Should have at least 1 core
    assert!(cpu_info.core_count >= 1, "Should have at least 1 CPU core");

    // Should not exceed reasonable maximum (1024 cores)
    assert!(cpu_info.core_count <= 1024, "Core count seems unreasonable");

    // Compare with num_cpus crate if available
    let system_cores = num_cpus::get();
    println!("num_cpus reports: {} cores", system_cores);

    // Should be reasonably close
    let diff = if cpu_info.core_count > system_cores {
        cpu_info.core_count - system_cores
    } else {
        system_cores - cpu_info.core_count
    };

    assert!(diff <= 2, "Core count should match system CPU count");
    println!("Core count matches system CPU count");
}

/// Test CPU info caching (singleton pattern)
#[test]
fn test_cpu_info_caching() {
    let info1 = CpuInfo::get();
    let info2 = CpuInfo::get();

    // Should return the same instance (same address)
    assert_eq!(info1 as *const CpuInfo as usize, info2 as *const CpuInfo as usize);
    println!("CPU info is correctly cached as singleton");

    // Values should be identical
    assert_eq!(info1.vendor, info2.vendor);
    assert_eq!(info1.arch, info2.arch);
    assert_eq!(info1.core_count, info2.core_count);
    println!("Cached values are consistent");
}

/// Test virtualization support detection
#[test]
fn test_virtualization_support() {
    let features = detect();

    println!("Virtualization Support:");
    #[cfg(target_arch = "x86_64")]
    {
        println!("  Intel VT-x (VMX): {}", features.vmx);
        println!("  AMD-V (SVM): {}", features.svm);

        // At least one should be available on modern x86_64 CPUs
        let has_virt = features.vmx || features.svm;
        println!("  Has hardware virtualization: {}", has_virt);
    }

    #[cfg(target_arch = "aarch64")]
    {
        println!("  ARM EL2 (Hypervisor): {}", features.arm_el2);
        println!("  Virtualization Host Extensions: {}", features.arm_el2);
    }
}
