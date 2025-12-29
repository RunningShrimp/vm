//! NPU (Neural Processing Unit) 加速支持
//!
//! 支持华为达芬奇、高通 Hexagon、联发科 APU、Apple Neural Engine 等

use super::PassthroughError;
use std::collections::HashMap;

/// NPU 厂商
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpuVendor {
    Huawei,   // 达芬奇架构
    Qualcomm, // Hexagon DSP/NPU
    MediaTek, // APU
    Apple,    // Neural Engine
    Intel,    // GNA/VPU
    Nvidia,   // Tensor Cores
    Google,   // TPU
    Unknown,
}

/// NPU 架构
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpuArchitecture {
    // 华为达芬奇
    DaVinci,
    DaVinci2,
    // 高通
    HexagonV65,
    HexagonV66,
    HexagonV68,
    HexagonV69,
    HexagonV73,
    // 联发科
    APU3,
    APU4,
    APU5,
    APU6,
    APU790,
    APU890,
    // Apple
    NeuralEngine,
    // Intel
    GNA,
    VPU,
    // NVIDIA
    TensorCore,
    // Google
    TPUv4,
    TPUv5,
    Unknown,
}

/// NPU 信息
#[derive(Debug, Clone)]
pub struct NpuInfo {
    pub vendor: NpuVendor,
    pub architecture: NpuArchitecture,
    pub name: String,
    pub tops: f32, // AI 算力 (TOPS)
    pub cores: usize,
    pub supports_int8: bool,
    pub supports_int4: bool,
    pub supports_fp16: bool,
    pub supports_bf16: bool,
}

/// NPU 设备
pub struct NpuDevice {
    pub info: NpuInfo,
    pub is_available: bool,
}

impl NpuDevice {
    /// 创建新的 NPU 设备
    pub fn new(
        vendor: NpuVendor,
        architecture: NpuArchitecture,
        name: String,
        tops: f32,
        cores: usize,
    ) -> Self {
        let info = NpuInfo {
            vendor,
            architecture,
            name,
            tops,
            cores,
            supports_int8: true,
            supports_int4: matches!(
                vendor,
                NpuVendor::Huawei | NpuVendor::Qualcomm | NpuVendor::MediaTek
            ),
            supports_fp16: true,
            supports_bf16: matches!(vendor, NpuVendor::Apple | NpuVendor::Nvidia),
        };

        Self {
            info,
            is_available: true,
        }
    }

    /// 检测华为达芬奇 NPU
    #[cfg(target_os = "linux")]
    pub fn detect_davinci() -> Option<Self> {
        use std::path::Path;

        // 检查 /dev/davinci* 设备节点
        for i in 0..8 {
            let dev_path = format!("/dev/davinci{}", i);
            if Path::new(&dev_path).exists() {
                log::info!("Detected Huawei DaVinci NPU at {}", dev_path);
                return Some(Self::new(
                    NpuVendor::Huawei,
                    NpuArchitecture::DaVinci2,
                    "Huawei DaVinci NPU".to_string(),
                    60.0, // 麒麟 9010 约 60 TOPS
                    2,
                ));
            }
        }

        None
    }

    #[cfg(not(target_os = "linux"))]
    pub fn detect_davinci() -> Option<Self> {
        None
    }

    /// 检测高通 Hexagon NPU
    #[cfg(target_os = "android")]
    pub fn detect_hexagon() -> Option<Self> {
        use std::process::Command;

        // 尝试通过 getprop 获取 SoC 信息
        if let Ok(output) = Command::new("getprop").arg("ro.hardware").output() {
            let hardware = String::from_utf8_lossy(&output.stdout)
                .trim()
                .to_lowercase();

            if hardware.contains("qcom") || hardware.contains("qualcomm") {
                // 根据 SoC 型号判断 Hexagon 版本
                let (architecture, tops) =
                    if hardware.contains("8gen3") || hardware.contains("8elite") {
                        (NpuArchitecture::HexagonV73, 45.0)
                    } else if hardware.contains("8gen2") {
                        (NpuArchitecture::HexagonV69, 35.0)
                    } else if hardware.contains("8gen1") {
                        (NpuArchitecture::HexagonV68, 27.0)
                    } else {
                        (NpuArchitecture::HexagonV66, 15.0)
                    };

                return Some(Self::new(
                    NpuVendor::Qualcomm,
                    architecture,
                    "Qualcomm Hexagon NPU".to_string(),
                    tops,
                    1,
                ));
            }
        }

        None
    }

    #[cfg(not(target_os = "android"))]
    pub fn detect_hexagon() -> Option<Self> {
        None
    }

    /// 检测联发科 APU
    #[cfg(target_os = "android")]
    pub fn detect_mediatek_apu() -> Option<Self> {
        use std::process::Command;

        if let Ok(output) = Command::new("getprop").arg("ro.hardware").output() {
            let hardware = String::from_utf8_lossy(&output.stdout)
                .trim()
                .to_lowercase();

            if hardware.contains("mt") || hardware.contains("mediatek") {
                let (architecture, tops) = if hardware.contains("9400") {
                    (NpuArchitecture::APU890, 50.0)
                } else if hardware.contains("9300") {
                    (NpuArchitecture::APU790, 45.0)
                } else if hardware.contains("9200") {
                    (NpuArchitecture::APU6, 35.0)
                } else {
                    (NpuArchitecture::APU5, 24.0)
                };

                return Some(Self::new(
                    NpuVendor::MediaTek,
                    architecture,
                    "MediaTek APU".to_string(),
                    tops,
                    1,
                ));
            }
        }

        None
    }

    #[cfg(not(target_os = "android"))]
    pub fn detect_mediatek_apu() -> Option<Self> {
        None
    }

    /// 检测 Apple Neural Engine
    #[cfg(target_os = "macos")]
    pub fn detect_apple_neural_engine() -> Option<Self> {
        use std::process::Command;

        if let Ok(output) = Command::new("sysctl")
            .arg("-n")
            .arg("machdep.cpu.brand_string")
            .output()
        {
            let brand = String::from_utf8_lossy(&output.stdout).to_lowercase();

            if brand.contains("apple") {
                let (tops, cores) = if brand.contains("m4") {
                    (38.0, 16) // M4 Pro/Max
                } else if brand.contains("m3") {
                    (35.0, 16)
                } else if brand.contains("m2") {
                    (15.8, 16)
                } else {
                    (11.0, 16)
                };

                return Some(Self::new(
                    NpuVendor::Apple,
                    NpuArchitecture::NeuralEngine,
                    "Apple Neural Engine".to_string(),
                    tops,
                    cores,
                ));
            }
        }

        None
    }

    #[cfg(not(target_os = "macos"))]
    pub fn detect_apple_neural_engine() -> Option<Self> {
        None
    }

    /// 启用 NPU 加速
    pub fn enable_acceleration(&self) -> Result<(), PassthroughError> {
        log::info!("Enabling NPU acceleration: {}", self.info.name);
        log::info!("  - Architecture: {:?}", self.info.architecture);
        log::info!("  - Performance: {:.1} TOPS", self.info.tops);
        log::info!("  - Cores: {}", self.info.cores);

        match self.info.vendor {
            NpuVendor::Huawei => {
                log::info!("  - Using CANN (Compute Architecture for Neural Networks)");
                log::info!("  - Supports MindSpore framework");
            }
            NpuVendor::Qualcomm => {
                log::info!("  - Using SNPE (Snapdragon Neural Processing Engine)");
                log::info!("  - Supports TensorFlow Lite, ONNX");
            }
            NpuVendor::MediaTek => {
                log::info!("  - Using NeuroPilot SDK");
                log::info!("  - Supports TensorFlow Lite, ONNX");
            }
            NpuVendor::Apple => {
                log::info!("  - Using Core ML");
                log::info!("  - Optimized for on-device ML inference");
            }
            _ => {}
        }

        Ok(())
    }

    /// 获取支持的框架
    pub fn get_supported_frameworks(&self) -> Vec<String> {
        match self.info.vendor {
            NpuVendor::Huawei => vec![
                "MindSpore".to_string(),
                "CANN".to_string(),
                "ONNX".to_string(),
            ],
            NpuVendor::Qualcomm => vec![
                "SNPE".to_string(),
                "TensorFlow Lite".to_string(),
                "ONNX".to_string(),
                "PyTorch Mobile".to_string(),
            ],
            NpuVendor::MediaTek => vec![
                "NeuroPilot".to_string(),
                "TensorFlow Lite".to_string(),
                "ONNX".to_string(),
            ],
            NpuVendor::Apple => vec![
                "Core ML".to_string(),
                "BNNS".to_string(),
                "Metal Performance Shaders".to_string(),
            ],
            NpuVendor::Nvidia => vec![
                "TensorRT".to_string(),
                "cuDNN".to_string(),
                "CUDA".to_string(),
            ],
            _ => vec![],
        }
    }

    /// 获取推荐的量化策略
    pub fn get_quantization_hint(&self) -> QuantizationHint {
        QuantizationHint {
            prefer_int8: self.info.supports_int8,
            prefer_int4: self.info.supports_int4,
            prefer_fp16: self.info.supports_fp16,
            prefer_bf16: self.info.supports_bf16,
            mixed_precision: true,
        }
    }
}

/// 量化提示
#[derive(Debug, Clone)]
pub struct QuantizationHint {
    pub prefer_int8: bool,
    pub prefer_int4: bool,
    pub prefer_fp16: bool,
    pub prefer_bf16: bool,
    pub mixed_precision: bool,
}

/// NPU 管理器
pub struct NpuManager {
    devices: HashMap<String, NpuDevice>,
}

impl NpuManager {
    /// 创建新的 NPU 管理器
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
        }
    }

    /// 扫描所有可用的 NPU
    pub fn scan_devices(&mut self) {
        // 检测华为达芬奇
        if let Some(npu) = NpuDevice::detect_davinci() {
            self.devices.insert("davinci".to_string(), npu);
        }

        // 检测高通 Hexagon
        if let Some(npu) = NpuDevice::detect_hexagon() {
            self.devices.insert("hexagon".to_string(), npu);
        }

        // 检测联发科 APU
        if let Some(npu) = NpuDevice::detect_mediatek_apu() {
            self.devices.insert("apu".to_string(), npu);
        }

        // 检测 Apple Neural Engine
        if let Some(npu) = NpuDevice::detect_apple_neural_engine() {
            self.devices.insert("neural_engine".to_string(), npu);
        }
    }

    /// 获取所有 NPU 设备
    pub fn get_devices(&self) -> &HashMap<String, NpuDevice> {
        &self.devices
    }

    /// 打印 NPU 信息
    pub fn print_devices(&self) {
        println!("\n=== NPU Devices ===");
        for (name, device) in &self.devices {
            println!("{}: {}", name, device.info.name);
            println!("  - Architecture: {:?}", device.info.architecture);
            println!("  - Performance: {:.1} TOPS", device.info.tops);
            println!("  - Cores: {}", device.info.cores);
            println!(
                "  - Supported frameworks: {:?}",
                device.get_supported_frameworks()
            );
        }
    }
}

impl Default for NpuManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_npu_manager() {
        let mut manager = NpuManager::new();
        manager.scan_devices();
        manager.print_devices();
    }
}
