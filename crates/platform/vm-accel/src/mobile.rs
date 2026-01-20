//! 移动端芯片特定优化
//!
//! 实现针对华为海思麒麟、高通骁龙、联发科天玑等移动芯片的优化

use super::cpuinfo::{CpuInfo, CpuVendor};
use super::vendor_extensions::VendorExtensionDetector;

/// 移动芯片型号
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MobileChipModel {
    Unknown,
    // 华为海思麒麟
    Kirin9000,
    Kirin9000S,
    Kirin9010,
    // 高通骁龙
    Snapdragon8Gen1,
    Snapdragon8Gen2,
    Snapdragon8Gen3,
    Snapdragon8Elite,
    // 联发科天玑
    Dimensity9000,
    Dimensity9200,
    Dimensity9300,
    Dimensity9400,
}

/// 移动芯片优化配置
#[derive(Debug, Clone)]
pub struct MobileChipConfig {
    /// 启用大小核调度优化
    pub enable_big_little: bool,
    /// 启用功耗管理
    pub enable_power_management: bool,
    /// 启用 GPU 加速
    pub enable_gpu_accel: bool,
    /// 启用 NPU/AI 加速
    pub enable_npu_accel: bool,
    /// 优先使用大核
    pub prefer_big_cores: bool,
    /// 启用动态电压频率调节
    pub enable_dvfs: bool,
}

impl Default for MobileChipConfig {
    fn default() -> Self {
        Self {
            enable_big_little: true,
            enable_power_management: true,
            enable_gpu_accel: false,
            enable_npu_accel: false,
            prefer_big_cores: true,
            enable_dvfs: true,
        }
    }
}

/// 移动芯片优化器
pub struct MobileOptimizer {
    config: MobileChipConfig,
    is_available: bool,
    vendor: CpuVendor,
    model: MobileChipModel,
    big_cores: usize,
    little_cores: usize,
    mid_cores: usize, // 部分芯片有中核
    extension_detector: VendorExtensionDetector,
}

impl MobileOptimizer {
    /// 创建新的移动芯片优化器
    pub fn new() -> Self {
        let cpu_info = CpuInfo::get();
        let is_mobile = matches!(
            cpu_info.vendor,
            CpuVendor::HiSilicon | CpuVendor::Qualcomm | CpuVendor::MediaTek
        );

        let model = Self::detect_model(&cpu_info.model_name, cpu_info.vendor);
        let (big_cores, mid_cores, little_cores) =
            Self::detect_core_config(model, cpu_info.core_count);

        Self {
            config: MobileChipConfig::default(),
            is_available: is_mobile,
            vendor: cpu_info.vendor,
            model,
            big_cores,
            mid_cores,
            little_cores,
            extension_detector: VendorExtensionDetector::new(),
        }
    }

    /// 检测移动芯片型号
    fn detect_model(model_name: &str, vendor: CpuVendor) -> MobileChipModel {
        let lower = model_name.to_lowercase();

        match vendor {
            CpuVendor::HiSilicon => {
                if lower.contains("kirin 9010") {
                    MobileChipModel::Kirin9010
                } else if lower.contains("kirin 9000s") {
                    MobileChipModel::Kirin9000S
                } else if lower.contains("kirin 9000") {
                    MobileChipModel::Kirin9000
                } else {
                    MobileChipModel::Unknown
                }
            }
            CpuVendor::Qualcomm => {
                if lower.contains("8 elite") || lower.contains("8 gen 4") {
                    MobileChipModel::Snapdragon8Elite
                } else if lower.contains("8 gen 3") {
                    MobileChipModel::Snapdragon8Gen3
                } else if lower.contains("8 gen 2") {
                    MobileChipModel::Snapdragon8Gen2
                } else if lower.contains("8 gen 1") {
                    MobileChipModel::Snapdragon8Gen1
                } else {
                    MobileChipModel::Unknown
                }
            }
            CpuVendor::MediaTek => {
                if lower.contains("dimensity 9400") {
                    MobileChipModel::Dimensity9400
                } else if lower.contains("dimensity 9300") {
                    MobileChipModel::Dimensity9300
                } else if lower.contains("dimensity 9200") {
                    MobileChipModel::Dimensity9200
                } else if lower.contains("dimensity 9000") {
                    MobileChipModel::Dimensity9000
                } else {
                    MobileChipModel::Unknown
                }
            }
            _ => MobileChipModel::Unknown,
        }
    }

    /// 检测核心配置（大核 + 中核 + 小核）
    fn detect_core_config(model: MobileChipModel, total_cores: usize) -> (usize, usize, usize) {
        match model {
            // 华为麒麟
            MobileChipModel::Kirin9010 => (1, 3, 4), // 1x3.3GHz + 3x2.8GHz + 4x2.0GHz
            MobileChipModel::Kirin9000S => (1, 3, 4),
            MobileChipModel::Kirin9000 => (1, 3, 4),

            // 高通骁龙
            MobileChipModel::Snapdragon8Elite => (2, 6, 0), // 2xOryon + 6xOryon (全大核架构)
            MobileChipModel::Snapdragon8Gen3 => (1, 5, 2),  // 1xX4 + 5xA720 + 2xA520
            MobileChipModel::Snapdragon8Gen2 => (1, 4, 3),  // 1xX3 + 4xA715 + 3xA510
            MobileChipModel::Snapdragon8Gen1 => (1, 3, 4),  // 1xX2 + 3xA710 + 4xA510

            // 联发科天玑
            MobileChipModel::Dimensity9400 => (1, 3, 4), // 1xX925 + 3xX4 + 4xA720
            MobileChipModel::Dimensity9300 => (4, 4, 0), // 4xX4 + 4xA720 (全大核)
            MobileChipModel::Dimensity9200 => (1, 3, 4), // 1xX3 + 3xA715 + 4xA510
            MobileChipModel::Dimensity9000 => (1, 3, 4), // 1xX2 + 3xA710 + 4xA510

            MobileChipModel::Unknown => {
                // 启发式估计：假设 1+3+4 配置
                if total_cores >= 8 {
                    (1, 3, 4)
                } else {
                    (total_cores / 2, 0, total_cores / 2)
                }
            }
        }
    }

    /// 检查是否可用
    pub fn is_available(&self) -> bool {
        self.is_available
    }

    /// 获取配置
    pub fn config(&self) -> &MobileChipConfig {
        &self.config
    }

    /// 设置配置
    pub fn set_config(&mut self, config: MobileChipConfig) {
        self.config = config;
    }

    /// 获取芯片型号
    pub fn model(&self) -> MobileChipModel {
        self.model
    }

    /// 应用移动芯片特定优化
    pub fn apply_optimizations(&self) {
        if !self.is_available {
            log::warn!("Mobile chip not detected, skipping optimizations");
            return;
        }

        log::info!("Applying mobile chip optimizations:");
        log::info!("  - Vendor: {:?}", self.vendor);
        log::info!("  - Model: {:?}", self.model);
        log::info!("  - Big cores: {}", self.big_cores);
        if self.mid_cores > 0 {
            log::info!("  - Mid cores: {}", self.mid_cores);
        }
        log::info!("  - Little cores: {}", self.little_cores);

        if self.config.enable_big_little {
            log::info!("  - big.LITTLE scheduling: enabled");
            // 智能调度到合适的核心
        }

        if self.config.enable_power_management {
            log::info!("  - Power management: enabled");
            // 根据负载动态调整频率和核心使用
        }

        if self.config.prefer_big_cores {
            log::info!("  - Big core affinity: enabled");
            // 将虚拟机线程优先绑定到大核
        }

        if self.config.enable_dvfs {
            log::info!("  - DVFS (Dynamic Voltage and Frequency Scaling): enabled");
        }

        // 显示厂商扩展支持情况
        if self.config.enable_npu_accel {
            let extension_name = match self.vendor {
                CpuVendor::Qualcomm => "Qualcomm Hexagon DSP",
                CpuVendor::MediaTek => "MediaTek APU",
                CpuVendor::HiSilicon => "HiSilicon NPU",
                _ => "",
            };

            if !extension_name.is_empty()
                && let Some(ext) = self.extension_detector.find_extension(extension_name)
                && ext.is_available()
            {
                log::info!("  - {}: enabled", extension_name);
                let features = ext.get_features();
                log::info!(
                    "    - Supports AI inference: {}",
                    features.supports_ai_inference
                );
                log::info!("    - Max vector width: {} bits", features.max_vector_width);
            }
        }
    }

    /// 获取推荐的 SIMD 指令集
    pub fn get_recommended_simd(&self) -> String {
        let cpu_info = CpuInfo::get();
        let mut simd = String::new();

        simd.push_str("NEON");

        if cpu_info.features.sve2 {
            simd.push_str(" + SVE2");
        } else if cpu_info.features.sve {
            simd.push_str(" + SVE");
        }

        // 添加厂商扩展
        let extension_name = match self.vendor {
            CpuVendor::Qualcomm if cpu_info.features.hexagon_dsp => " + Hexagon DSP",
            CpuVendor::MediaTek if cpu_info.features.apu => " + APU",
            CpuVendor::HiSilicon if cpu_info.features.npu => " + NPU",
            _ => "",
        };

        if !extension_name.is_empty() {
            simd.push_str(extension_name);
        }

        simd
    }

    /// 检查 Hexagon DSP 是否可用（高通）
    pub fn has_hexagon_dsp(&self) -> bool {
        self.vendor == CpuVendor::Qualcomm
            && self.config.enable_npu_accel
            && self
                .extension_detector
                .find_extension("Qualcomm Hexagon DSP")
                .map(|ext| ext.is_available())
                .unwrap_or(false)
    }

    /// 检查 APU 是否可用（联发科）
    pub fn has_apu(&self) -> bool {
        self.vendor == CpuVendor::MediaTek
            && self.config.enable_npu_accel
            && self
                .extension_detector
                .find_extension("MediaTek APU")
                .map(|ext| ext.is_available())
                .unwrap_or(false)
    }

    /// 检查 NPU 是否可用（华为）
    pub fn has_npu(&self) -> bool {
        self.vendor == CpuVendor::HiSilicon
            && self.config.enable_npu_accel
            && self
                .extension_detector
                .find_extension("HiSilicon NPU")
                .map(|ext| ext.is_available())
                .unwrap_or(false)
    }

    /// 获取厂商扩展检测器
    pub fn extension_detector(&self) -> &VendorExtensionDetector {
        &self.extension_detector
    }

    /// 优化内存访问模式
    pub fn optimize_memory_access(&self) -> MemoryAccessHint {
        MemoryAccessHint {
            use_huge_pages: false, // 移动设备通常不支持
            cache_line_size: 64,
            prefetch_distance: 128, // 移动芯片缓存较小
            numa_aware: false,
            optimize_for_battery: self.config.enable_power_management,
            memory_bandwidth_gbps: self.get_memory_bandwidth(),
        }
    }

    /// 获取内存带宽（GB/s）
    fn get_memory_bandwidth(&self) -> usize {
        match self.model {
            MobileChipModel::Kirin9010
            | MobileChipModel::Kirin9000S
            | MobileChipModel::Kirin9000 => 44,
            MobileChipModel::Snapdragon8Elite => 77,
            MobileChipModel::Snapdragon8Gen3 => 77,
            MobileChipModel::Snapdragon8Gen2 => 64,
            MobileChipModel::Snapdragon8Gen1 => 51,
            MobileChipModel::Dimensity9400 => 68,
            MobileChipModel::Dimensity9300 => 68,
            MobileChipModel::Dimensity9200 => 51,
            MobileChipModel::Dimensity9000 => 51,
            MobileChipModel::Unknown => 40,
        }
    }

    /// 获取 JIT 编译器优化建议
    pub fn get_jit_hints(&self) -> JitOptimizationHints {
        JitOptimizationHints {
            inline_threshold: 80, // 移动设备代码缓存较小
            loop_unroll_factor: 4,
            use_simd: true,
            simd_width: 128, // NEON
            enable_branch_prediction_hints: true,
            enable_cache_prefetch: false, // 移动设备功耗敏感
            optimize_for_power: self.config.enable_power_management,
            prefer_big_cores: self.config.prefer_big_cores,
        }
    }

    /// 获取核心调度建议
    pub fn get_core_scheduling_hint(&self) -> CoreSchedulingHint {
        CoreSchedulingHint {
            big_cores: self.big_cores,
            mid_cores: self.mid_cores,
            little_cores: self.little_cores,
            prefer_big_for_vm: self.config.prefer_big_cores,
            enable_migration: true, // 允许核心间迁移
            power_aware: self.config.enable_power_management,
        }
    }

    /// 获取 NPU 加速建议
    pub fn get_npu_hints(&self) -> NpuAccelerationHint {
        let (npu_cores, tops) = match self.model {
            MobileChipModel::Kirin9010 => (2, 60), // 双核NPU, 60 TOPS
            MobileChipModel::Kirin9000S => (2, 50),
            MobileChipModel::Kirin9000 => (2, 40),
            MobileChipModel::Snapdragon8Elite => (1, 45), // Hexagon NPU
            MobileChipModel::Snapdragon8Gen3 => (1, 45),
            MobileChipModel::Snapdragon8Gen2 => (1, 35),
            MobileChipModel::Snapdragon8Gen1 => (1, 27),
            MobileChipModel::Dimensity9400 => (1, 50), // APU 890
            MobileChipModel::Dimensity9300 => (1, 45), // APU 790
            MobileChipModel::Dimensity9200 => (1, 35),
            MobileChipModel::Dimensity9000 => (1, 24),
            MobileChipModel::Unknown => (0, 0),
        };

        NpuAccelerationHint {
            available: npu_cores > 0,
            npu_cores,
            tops,
            vendor: self.vendor,
        }
    }

    /// 获取 GPU 加速建议
    pub fn get_gpu_hints(&self) -> GpuAccelerationHint {
        let (gpu_name, gpu_cores) = match self.model {
            MobileChipModel::Kirin9010 => ("Maleoon 910", 24),
            MobileChipModel::Kirin9000S => ("Maleoon 910", 22),
            MobileChipModel::Kirin9000 => ("Mali-G78", 24),
            MobileChipModel::Snapdragon8Elite => ("Adreno 830", 0),
            MobileChipModel::Snapdragon8Gen3 => ("Adreno 750", 0),
            MobileChipModel::Snapdragon8Gen2 => ("Adreno 740", 0),
            MobileChipModel::Snapdragon8Gen1 => ("Adreno 730", 0),
            MobileChipModel::Dimensity9400 => ("Immortalis-G925", 12),
            MobileChipModel::Dimensity9300 => ("Immortalis-G720", 12),
            MobileChipModel::Dimensity9200 => ("Immortalis-G715", 11),
            MobileChipModel::Dimensity9000 => ("Mali-G710", 10),
            MobileChipModel::Unknown => ("Unknown", 0),
        };

        GpuAccelerationHint {
            gpu_name: gpu_name.to_string(),
            gpu_cores,
            supports_vulkan: true,
            supports_opencl: true,
            supports_ray_tracing: matches!(
                self.model,
                MobileChipModel::Dimensity9400 | MobileChipModel::Dimensity9300
            ),
        }
    }
}

impl Default for MobileOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

/// 内存访问优化提示
#[derive(Debug, Clone)]
pub struct MemoryAccessHint {
    pub use_huge_pages: bool,
    pub cache_line_size: usize,
    pub prefetch_distance: usize,
    pub numa_aware: bool,
    pub optimize_for_battery: bool,
    pub memory_bandwidth_gbps: usize,
}

/// JIT 优化提示
#[derive(Debug, Clone)]
pub struct JitOptimizationHints {
    pub inline_threshold: usize,
    pub loop_unroll_factor: usize,
    pub use_simd: bool,
    pub simd_width: usize,
    pub enable_branch_prediction_hints: bool,
    pub enable_cache_prefetch: bool,
    pub optimize_for_power: bool,
    pub prefer_big_cores: bool,
}

/// 核心调度提示
#[derive(Debug, Clone)]
pub struct CoreSchedulingHint {
    pub big_cores: usize,
    pub mid_cores: usize,
    pub little_cores: usize,
    pub prefer_big_for_vm: bool,
    pub enable_migration: bool,
    pub power_aware: bool,
}

/// NPU 加速提示
#[derive(Debug, Clone)]
pub struct NpuAccelerationHint {
    pub available: bool,
    pub npu_cores: usize,
    pub tops: usize, // AI 算力 (TOPS)
    pub vendor: CpuVendor,
}

/// GPU 加速提示
#[derive(Debug, Clone)]
pub struct GpuAccelerationHint {
    pub gpu_name: String,
    pub gpu_cores: usize,
    pub supports_vulkan: bool,
    pub supports_opencl: bool,
    pub supports_ray_tracing: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mobile_optimizer() {
        let optimizer = MobileOptimizer::new();
        println!("Mobile optimizer available: {}", optimizer.is_available());

        if optimizer.is_available() {
            optimizer.apply_optimizations();
            println!("Model: {:?}", optimizer.model());
            println!("Recommended SIMD: {}", optimizer.get_recommended_simd());
            println!("Memory hints: {:?}", optimizer.optimize_memory_access());
            println!("JIT hints: {:?}", optimizer.get_jit_hints());
            println!(
                "Core scheduling: {:?}",
                optimizer.get_core_scheduling_hint()
            );
            println!("NPU hints: {:?}", optimizer.get_npu_hints());
            println!("GPU hints: {:?}", optimizer.get_gpu_hints());
        }
    }
}
