//! 厂商扩展集成测试

use vm_frontend_arm64::Arm64Decoder;
use vm_core::{MMU, GuestAddr, VmError};
use vm_accel::cpuinfo::CpuInfo;
use vm_accel::vendor_extensions::VendorExtensionDetector;

#[test]
fn test_vendor_extension_detection() {
    let detector = VendorExtensionDetector::new();
    let extensions = detector.extension_names();
    println!("Detected extensions: {:?}", extensions);
    
    let cpu_info = CpuInfo::get();
    println!("CPU vendor: {:?}", cpu_info.vendor);
    println!("AMX support: {}", cpu_info.features.amx);
    println!("Hexagon DSP support: {}", cpu_info.features.hexagon_dsp);
    println!("APU support: {}", cpu_info.features.apu);
    println!("NPU support: {}", cpu_info.features.npu);
}

#[test]
fn test_multi_vendor_coexistence() {
    // 测试多厂商扩展共存
    let detector = VendorExtensionDetector::new();
    let extensions = detector.available_extensions();
    
    // 验证不会冲突
    assert!(extensions.len() <= 4); // 最多4个厂商扩展
}


