//! 厂商扩展检测模块
//!
//! 提供统一的接口来检测和使用不同厂商的处理器扩展

use super::cpuinfo::{CpuInfo, CpuVendor};

/// 厂商扩展 trait
///
/// 所有厂商扩展检测器必须实现此 trait
pub trait VendorExtension {
    /// 扩展名称
    fn name(&self) -> &'static str;

    /// 检查扩展是否可用
    fn is_available(&self) -> bool;

    /// 获取扩展的详细特性信息
    fn get_features(&self) -> ExtensionFeatures;

    /// 获取推荐的优化策略
    fn get_optimization_hints(&self) -> OptimizationHints;
}

/// 扩展特性信息
#[derive(Debug, Clone)]
pub struct ExtensionFeatures {
    /// 是否支持矩阵运算
    pub supports_matrix_ops: bool,
    /// 是否支持向量运算
    pub supports_vector_ops: bool,
    /// 是否支持AI推理
    pub supports_ai_inference: bool,
    /// 支持的精度类型
    pub supported_precisions: Vec<Precision>,
    /// 最大向量宽度（位）
    pub max_vector_width: usize,
}

/// 数值精度类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precision {
    Int8,
    Int16,
    Int32,
    Fp16,
    Fp32,
    Fp64,
}

/// 优化提示
#[derive(Debug, Clone)]
pub struct OptimizationHints {
    /// 推荐的SIMD宽度
    pub recommended_simd_width: usize,
    /// 是否启用批处理
    pub enable_batching: bool,
    /// 批处理大小
    pub batch_size: usize,
    /// 是否启用流水线
    pub enable_pipelining: bool,
}

/// Apple AMX 扩展检测器
pub struct AppleExtension {
    cpu_info: &'static CpuInfo,
}

impl AppleExtension {
    pub fn new() -> Self {
        Self {
            cpu_info: CpuInfo::get(),
        }
    }

    /// 检查是否为 Apple Silicon
    fn is_apple_silicon(&self) -> bool {
        self.cpu_info.vendor == CpuVendor::Apple
    }
}

impl VendorExtension for AppleExtension {
    fn name(&self) -> &'static str {
        "Apple AMX"
    }

    fn is_available(&self) -> bool {
        self.is_apple_silicon() && self.cpu_info.features.amx
    }

    fn get_features(&self) -> ExtensionFeatures {
        ExtensionFeatures {
            supports_matrix_ops: true,
            supports_vector_ops: true,
            supports_ai_inference: true,
            supported_precisions: vec![
                Precision::Int8,
                Precision::Int16,
                Precision::Fp16,
                Precision::Fp32,
            ],
            max_vector_width: 1024, // AMX 支持大矩阵运算
        }
    }

    fn get_optimization_hints(&self) -> OptimizationHints {
        OptimizationHints {
            recommended_simd_width: 128, // NEON 宽度
            enable_batching: true,
            batch_size: 64, // AMX 矩阵块大小
            enable_pipelining: true,
        }
    }
}

impl Default for AppleExtension {
    fn default() -> Self {
        Self::new()
    }
}

/// 高通 Hexagon DSP 扩展检测器
pub struct QualcommExtension {
    cpu_info: &'static CpuInfo,
}

impl QualcommExtension {
    pub fn new() -> Self {
        Self {
            cpu_info: CpuInfo::get(),
        }
    }

    /// 检查是否为高通芯片
    fn is_qualcomm(&self) -> bool {
        self.cpu_info.vendor == CpuVendor::Qualcomm
    }
}

impl VendorExtension for QualcommExtension {
    fn name(&self) -> &'static str {
        "Qualcomm Hexagon DSP"
    }

    fn is_available(&self) -> bool {
        self.is_qualcomm() && self.cpu_info.features.hexagon_dsp
    }

    fn get_features(&self) -> ExtensionFeatures {
        ExtensionFeatures {
            supports_matrix_ops: false,
            supports_vector_ops: true,
            supports_ai_inference: true,
            supported_precisions: vec![
                Precision::Int8,
                Precision::Int16,
                Precision::Int32,
                Precision::Fp16,
                Precision::Fp32,
            ],
            max_vector_width: 1024, // Hexagon 支持VLIW向量运算
        }
    }

    fn get_optimization_hints(&self) -> OptimizationHints {
        OptimizationHints {
            recommended_simd_width: 128,
            enable_batching: true,
            batch_size: 32,
            enable_pipelining: true, // VLIW架构支持并行执行
        }
    }
}

impl Default for QualcommExtension {
    fn default() -> Self {
        Self::new()
    }
}

/// 联发科 APU 扩展检测器
pub struct MediaTekExtension {
    cpu_info: &'static CpuInfo,
}

impl MediaTekExtension {
    pub fn new() -> Self {
        Self {
            cpu_info: CpuInfo::get(),
        }
    }

    /// 检查是否为联发科芯片
    fn is_mediatek(&self) -> bool {
        self.cpu_info.vendor == CpuVendor::MediaTek
    }
}

impl VendorExtension for MediaTekExtension {
    fn name(&self) -> &'static str {
        "MediaTek APU"
    }

    fn is_available(&self) -> bool {
        self.is_mediatek() && self.cpu_info.features.apu
    }

    fn get_features(&self) -> ExtensionFeatures {
        ExtensionFeatures {
            supports_matrix_ops: true,
            supports_vector_ops: true,
            supports_ai_inference: true,
            supported_precisions: vec![
                Precision::Int8,
                Precision::Int16,
                Precision::Fp16,
                Precision::Fp32,
            ],
            max_vector_width: 512,
        }
    }

    fn get_optimization_hints(&self) -> OptimizationHints {
        OptimizationHints {
            recommended_simd_width: 128,
            enable_batching: true,
            batch_size: 16,
            enable_pipelining: true,
        }
    }
}

impl Default for MediaTekExtension {
    fn default() -> Self {
        Self::new()
    }
}

/// 华为海思 NPU 扩展检测器
pub struct HiSiliconExtension {
    cpu_info: &'static CpuInfo,
}

impl HiSiliconExtension {
    pub fn new() -> Self {
        Self {
            cpu_info: CpuInfo::get(),
        }
    }

    /// 检查是否为华为芯片
    fn is_hisilicon(&self) -> bool {
        self.cpu_info.vendor == CpuVendor::HiSilicon
    }
}

impl VendorExtension for HiSiliconExtension {
    fn name(&self) -> &'static str {
        "HiSilicon NPU"
    }

    fn is_available(&self) -> bool {
        self.is_hisilicon() && self.cpu_info.features.npu
    }

    fn get_features(&self) -> ExtensionFeatures {
        ExtensionFeatures {
            supports_matrix_ops: true,
            supports_vector_ops: true,
            supports_ai_inference: true,
            supported_precisions: vec![
                Precision::Int8,
                Precision::Int16,
                Precision::Fp16,
                Precision::Fp32,
            ],
            max_vector_width: 1024, // 达芬奇架构支持大矩阵
        }
    }

    fn get_optimization_hints(&self) -> OptimizationHints {
        OptimizationHints {
            recommended_simd_width: 128,
            enable_batching: true,
            batch_size: 32,
            enable_pipelining: true, // 达芬奇架构支持流水线
        }
    }
}

impl Default for HiSiliconExtension {
    fn default() -> Self {
        Self::new()
    }
}

/// 统一的扩展检测接口
pub struct VendorExtensionDetector {
    extensions: Vec<Box<dyn VendorExtension>>,
}

impl VendorExtensionDetector {
    /// 创建新的检测器，自动检测所有可用的扩展
    pub fn new() -> Self {
        let mut extensions: Vec<Box<dyn VendorExtension>> = Vec::new();

        // 检测 Apple AMX
        let apple_ext = AppleExtension::new();
        if apple_ext.is_available() {
            extensions.push(Box::new(apple_ext));
        }

        // 检测 Qualcomm Hexagon DSP
        let qualcomm_ext = QualcommExtension::new();
        if qualcomm_ext.is_available() {
            extensions.push(Box::new(qualcomm_ext));
        }

        // 检测 MediaTek APU
        let mediatek_ext = MediaTekExtension::new();
        if mediatek_ext.is_available() {
            extensions.push(Box::new(mediatek_ext));
        }

        // 检测 HiSilicon NPU
        let hisilicon_ext = HiSiliconExtension::new();
        if hisilicon_ext.is_available() {
            extensions.push(Box::new(hisilicon_ext));
        }

        Self { extensions }
    }

    /// 获取所有可用的扩展
    pub fn available_extensions(&self) -> &[Box<dyn VendorExtension>] {
        &self.extensions
    }

    /// 根据名称查找扩展
    pub fn find_extension(&self, name: &str) -> Option<&dyn VendorExtension> {
        self.extensions
            .iter()
            .find(|ext| ext.name() == name)
            .map(|ext| ext.as_ref())
    }

    /// 获取所有扩展的名称
    pub fn extension_names(&self) -> Vec<&'static str> {
        self.extensions.iter().map(|ext| ext.name()).collect()
    }
}

impl Default for VendorExtensionDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vendor_extension_detector() {
        let detector = VendorExtensionDetector::new();
        println!("Available extensions: {:?}", detector.extension_names());

        for ext in detector.available_extensions() {
            println!("Extension: {}", ext.name());
            println!("  Available: {}", ext.is_available());
            println!("  Features: {:?}", ext.get_features());
            println!("  Optimization hints: {:?}", ext.get_optimization_hints());
        }
    }

    #[test]
    fn test_apple_extension() {
        let ext = AppleExtension::new();
        println!("Apple extension available: {}", ext.is_available());
        if ext.is_available() {
            println!("Features: {:?}", ext.get_features());
        }
    }

    #[test]
    fn test_qualcomm_extension() {
        let ext = QualcommExtension::new();
        println!("Qualcomm extension available: {}", ext.is_available());
        if ext.is_available() {
            println!("Features: {:?}", ext.get_features());
        }
    }

    #[test]
    fn test_mediatek_extension() {
        let ext = MediaTekExtension::new();
        println!("MediaTek extension available: {}", ext.is_available());
        if ext.is_available() {
            println!("Features: {:?}", ext.get_features());
        }
    }

    #[test]
    fn test_hisilicon_extension() {
        let ext = HiSiliconExtension::new();
        println!("HiSilicon extension available: {}", ext.is_available());
        if ext.is_available() {
            println!("Features: {:?}", ext.get_features());
        }
    }
}
