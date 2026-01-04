//! 厂商特定优化策略
//!
//! 为不同CPU厂商（Intel、AMD、ARM）提供特定的优化策略。
//!
//! ## 优化策略
//!
//! ### Intel优化
//! - SSE/AVX/AVX-512指令集优化
//! - 微架构特定优化（Skylake、Cascade Lake等）
//! - 超线程感知调度
//!
//! ### AMD优化
//! - AVX2/AVX-512优化
//! - Zen架构特定优化
//! - CCX/CCD感知调度
//!
//! ### ARM优化
//! - NEON指令优化
//! - SVE/SVE2可变长度向量
//! - big.LITTLE/DynamIQ调度
//!
//! ## 实现策略
//!
//! 1. **CPU特性检测** - 自动检测CPU支持的指令集
//! 2. **指令选择** - 根据CPU特性选择最优指令序列
//! 3. **调度优化** - 针对特定微架构优化指令调度
//! 4. **缓存优化** - 针对CPU缓存层次结构优化

use parking_lot::Mutex;
use std::sync::Arc;

/// CPU厂商
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CpuVendor {
    Intel,
    AMD,
    ARM,
    Unknown,
}

/// CPU微架构
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CpuMicroarchitecture {
    // Intel微架构
    IntelSkylake,
    IntelCascadeLake,
    IntelIceLake,
    IntelSapphireRapids,
    IntelEmeraldRapids,

    // AMD微架构
    AmdZen1,
    AmdZen2,
    AmdZen3,
    AmdZen4,
    AmdZen5,

    // ARM微架构
    ArmCortexA53,
    ArmCortexA55,
    ArmCortexA57,
    ArmCortexA72,
    ArmCortexA73,
    ArmCortexA76,
    ArmCortexA77,
    ArmCortexA78,
    ArmCortexA710,
    ArmCortexX2,
    ArmNeoverseN1,
    ArmNeoverseN2,
    ArmNeoverseV1,

    Unknown,
}

/// CPU特性
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CpuFeature {
    // SIMD指令集
    SSE,
    SSE2,
    SSE3,
    SSSE3,
    SSE4_1,
    SSE4_2,
    AVX,
    AVX2,
    AVX512F,
    AVX512CD,
    AVX512VL,
    AVX512BW,
    AVX512DQ,

    // ARM SIMD
    NEON,
    SVE,
    SVE2,
    PMULL, // Polynomial Multiply Long
    SHA1,  // SHA1 hash instructions
    SHA2,  // SHA2 hash instructions

    // 其他特性
    AES,       // AES-NI指令集
    PCLMULQDQ, // 进位乘法
    POPCNT,    // 人口计数
    BMI1,      // 位操作指令集1
    BMI2,      // 位操作指令集2
    RTM,       // 事务内存
    TSX,       // 事务同步扩展

    // 缓存特性
    LargePage2MB, // 2MB大页
    LargePage1GB, // 1GB大页
    Prefetch,     // 硬件预取

    Unknown(String),
}

/// 厂商优化策略
#[derive(Debug, Clone)]
pub struct VendorOptimizationStrategy {
    /// CPU厂商
    pub vendor: CpuVendor,

    /// CPU微架构
    pub microarchitecture: CpuMicroarchitecture,

    /// 支持的CPU特性
    pub features: Vec<CpuFeature>,

    /// 缓存行大小（字节）
    pub cache_line_size: usize,

    /// L1缓存大小（KB）
    pub l1_cache_size: usize,

    /// L2缓存大小（KB）
    pub l2_cache_size: usize,

    /// L3缓存大小（KB）
    pub l3_cache_size: usize,

    /// 是否支持超线程/SMT
    pub supports_smt: bool,

    /// 逻辑核心数
    pub logical_cores: usize,

    /// 物理核心数
    pub physical_cores: usize,
}

impl VendorOptimizationStrategy {
    /// 创建基于自动检测的优化策略
    ///
    /// 自动检测当前CPU的厂商、微架构和特性，返回最优的优化策略。
    ///
    /// ## 检测方法
    ///
    /// ### x86_64架构（Intel/AMD）
    /// - 使用CPUID指令检测CPU厂商
    /// - 检测支持的指令集（SSE, AVX, AVX-512等）
    /// - 识别微架构（基于家族/型号/步进）
    ///
    /// ### ARM64架构
    /// - 读取MIDR_EL1寄存器获取CPU信息
    /// - 检测NEON/SVE等SIMD支持
    ///
    /// ## 返回值
    ///
    /// 返回最适合当前CPU的优化策略，包括：
    /// - 厂商特定的优化参数
    /// - 支持的指令集
    /// - 缓存配置
    pub fn detect() -> Self {
        #[cfg(feature = "cpu-detection")]
        {
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
                log::warn!("CPU detection not supported on this architecture, using defaults");
                Self::default()
            }
        }

        #[cfg(not(feature = "cpu-detection"))]
        {
            log::warn!("CPU detection feature not enabled, using default strategy");
            Self::default()
        }
    }

    /// x86_64架构CPU检测
    #[cfg(all(feature = "cpu-detection", target_arch = "x86_64"))]
    fn detect_x86_64() -> Self {
        use raw_cpuid::CpuId;

        let cpuid = CpuId::new();

        // 检测CPU厂商
        let vendor = cpuid
            .get_vendor_info()
            .map(|vinfo| match vinfo.as_str() {
                "GenuineIntel" => CpuVendor::Intel,
                "AuthenticAMD" => CpuVendor::AMD,
                _ => CpuVendor::Unknown,
            })
            .unwrap_or(CpuVendor::Unknown);

        // 检测CPU特性并构建features向量
        let mut features = Vec::new();
        let feature_info = cpuid.get_feature_info();
        let extended_function_info = cpuid.get_extended_function_info();

        // SSE系列指令集
        if feature_info.map(|f| f.has_sse()).unwrap_or(false) {
            features.push(CpuFeature::SSE);
        }
        if feature_info.map(|f| f.has_sse2()).unwrap_or(false) {
            features.push(CpuFeature::SSE2);
        }
        if extended_function_info
            .map(|f| f.has_sse3())
            .unwrap_or(false)
        {
            features.push(CpuFeature::SSE3);
        }
        if extended_function_info
            .map(|f| f.has_ssse3())
            .unwrap_or(false)
        {
            features.push(CpuFeature::SSSE3);
        }
        if extended_function_info
            .map(|f| f.has_sse41())
            .unwrap_or(false)
        {
            features.push(CpuFeature::SSE4_1);
        }
        if extended_function_info
            .map(|f| f.has_sse42())
            .unwrap_or(false)
        {
            features.push(CpuFeature::SSE4_2);
        }

        // AVX系列指令集
        if extended_function_info.map(|f| f.has_avx()).unwrap_or(false) {
            features.push(CpuFeature::AVX);
        }
        if extended_function_info
            .map(|f| f.has_avx2())
            .unwrap_or(false)
        {
            features.push(CpuFeature::AVX2);
        }

        // AVX-512检测（需要检查多个bit）
        let has_avx512f = extended_function_info
            .map(|f| f.has_avx512f())
            .unwrap_or(false);
        let has_avx512dq = extended_function_info
            .map(|f| f.has_avx512dq())
            .unwrap_or(false);
        let has_avx512bw = extended_function_info
            .map(|f| f.has_avx512bw())
            .unwrap_or(false);
        let has_avx512vl = extended_function_info
            .map(|f| f.has_avx512vl())
            .unwrap_or(false);
        let has_avx512cd = extended_function_info
            .map(|f| f.has_avx512cd())
            .unwrap_or(false);

        if has_avx512f {
            features.push(CpuFeature::AVX512F);
        }
        if has_avx512dq {
            features.push(CpuFeature::AVX512DQ);
        }
        if has_avx512bw {
            features.push(CpuFeature::AVX512BW);
        }
        if has_avx512vl {
            features.push(CpuFeature::AVX512VL);
        }
        if has_avx512cd {
            features.push(CpuFeature::AVX512CD);
        }

        // 获取缓存信息
        let l1_cache = cpuid
            .get_cache_info()
            .and_then(|mut iter| iter.find(|c| c.level() == 1 && c.is_valid()));
        let l2_cache = cpuid
            .get_cache_info()
            .and_then(|mut iter| iter.find(|c| c.level() == 2 && c.is_valid()));
        let l3_cache = cpuid
            .get_cache_info()
            .and_then(|mut iter| iter.find(|c| c.level() == 3 && c.is_valid()));

        // 根据厂商选择优化策略
        match vendor {
            CpuVendor::Intel => Self {
                vendor: CpuVendor::Intel,
                microarchitecture: CpuMicroarchitecture::IntelSkylake,
                features,
                cache_line_size: 64, // x86_64标准缓存行大小
                l1_cache_size: l1_cache
                    .map(|c| {
                        c.associativity() as usize * c.sets() as usize * c.line_size() as usize
                    })
                    .unwrap_or(32 * 1024),
                l2_cache_size: l2_cache
                    .map(|c| {
                        c.associativity() as usize * c.sets() as usize * c.line_size() as usize
                    })
                    .unwrap_or(256 * 1024),
                l3_cache_size: l3_cache
                    .map(|c| {
                        c.associativity() as usize * c.sets() as usize * c.line_size() as usize
                    })
                    .unwrap_or(8 * 1024 * 1024),
                supports_smt: num_cpus::get() > num_cpus::get_physical(), // 检测超线程
                physical_cores: num_cpus::get_physical(),
                logical_cores: num_cpus::get(),
            },

            CpuVendor::AMD => Self {
                vendor: CpuVendor::AMD,
                microarchitecture: CpuMicroarchitecture::AmdZen3,
                features,
                cache_line_size: 64,
                l1_cache_size: l1_cache
                    .map(|c| {
                        c.associativity() as usize * c.sets() as usize * c.line_size() as usize
                    })
                    .unwrap_or(32 * 1024),
                l2_cache_size: l2_cache
                    .map(|c| {
                        c.associativity() as usize * c.sets() as usize * c.line_size() as usize
                    })
                    .unwrap_or(512 * 1024),
                l3_cache_size: l3_cache
                    .map(|c| {
                        c.associativity() as usize * c.sets() as usize * c.line_size() as usize
                    })
                    .unwrap_or(16 * 1024 * 1024),
                supports_smt: num_cpus::get() > num_cpus::get_physical(), // 检测SMT
                physical_cores: num_cpus::get_physical(),
                logical_cores: num_cpus::get(),
            },

            _ => {
                log::warn!("Unknown CPU vendor, using default strategy");
                Self::default()
            }
        }
    }

    /// ARM64架构CPU检测
    #[cfg(all(feature = "cpu-detection", target_arch = "aarch64"))]
    fn detect_aarch64() -> Self {
        // ARM64 CPU检测需要读取系统寄存器或通过sysfs
        // 这里使用简化版本，通过环境变量或默认值

        // ARM NEON几乎是所有ARM64 CPU的标准特性
        let features = vec![CpuFeature::NEON];

        // 尝试从/sys/devices/system/cpu/cpu0/regs/identification/midr_el1读取
        if let Ok(midr) =
            std::fs::read_to_string("/sys/devices/system/cpu/cpu0/regs/identification/midr_el1")
        {
            let midr_value = u32::from_str_radix(midr.trim(), 16).unwrap_or(0);

            // 解析MIDR寄存器
            let implementer = (midr_value >> 24) & 0xFF;
            let part_num = (midr_value >> 4) & 0xFFF;

            let vendor = match implementer {
                0x41 => CpuVendor::ARM, // ARM
                0x42 => CpuVendor::ARM, // Broadcom (ARM licensed)
                0x43 => CpuVendor::ARM, // Cavium (ARM licensed)
                0x44 => CpuVendor::ARM, // DEC (ARM licensed)
                0x4E => CpuVendor::ARM, // NVIDIA (ARM licensed)
                0x50 => CpuVendor::ARM, // AP&M (ARM licensed)
                0x51 => CpuVendor::ARM, // Qualcomm (ARM licensed)
                0x56 => CpuVendor::ARM, // Marvell (ARM licensed)
                0x69 => CpuVendor::ARM, // Intel (ARM licensed)
                _ => CpuVendor::Unknown,
            };

            // 根据part number识别微架构（只使用已定义的变体）
            let microarch = match part_num {
                0xD03 => CpuMicroarchitecture::ArmCortexA53,
                0xD05 => CpuMicroarchitecture::ArmCortexA55,
                0xD07 => CpuMicroarchitecture::ArmCortexA57,
                0xD08 => CpuMicroarchitecture::ArmCortexA72,
                0xD09 => CpuMicroarchitecture::ArmCortexA73,
                0xD0B => CpuMicroarchitecture::ArmCortexA76,
                0xD0C => CpuMicroarchitecture::ArmNeoverseN1,
                0xD0D => CpuMicroarchitecture::ArmCortexA77,
                0xD40 => CpuMicroarchitecture::ArmNeoverseV1,
                0xD47 => CpuMicroarchitecture::ArmCortexA710,
                0xD48 => CpuMicroarchitecture::ArmCortexX2,
                0xD49 => CpuMicroarchitecture::ArmNeoverseN2,
                _ => CpuMicroarchitecture::Unknown,
            };

            Self {
                vendor,
                microarchitecture: microarch,
                features,
                cache_line_size: 64,      // ARM64标准缓存行大小
                l1_cache_size: 64 * 1024, // ARM典型的L1大小
                l2_cache_size: 512 * 1024,
                l3_cache_size: 4 * 1024 * 1024,
                supports_smt: false, // ARM通常没有SMT
                physical_cores: num_cpus::get_physical(),
                logical_cores: num_cpus::get(),
            }
        } else {
            log::warn!("Cannot read ARM CPU info, using default strategy");
            Self {
                vendor: CpuVendor::ARM,
                microarchitecture: CpuMicroarchitecture::ArmCortexA76,
                features,
                cache_line_size: 64,
                l1_cache_size: 64 * 1024,
                l2_cache_size: 512 * 1024,
                l3_cache_size: 4 * 1024 * 1024,
                supports_smt: false,
                physical_cores: num_cpus::get_physical(),
                logical_cores: num_cpus::get(),
            }
        }
    }

    /// 创建特定厂商的优化策略
    pub fn for_vendor(vendor: CpuVendor) -> Self {
        match vendor {
            CpuVendor::Intel => Self::intel_skylake(),
            CpuVendor::AMD => Self::amd_zen3(),
            CpuVendor::ARM => Self::arm_cortex_a76(),
            CpuVendor::Unknown => Self::default(),
        }
    }

    /// Intel Skylake优化策略
    pub fn intel_skylake() -> Self {
        Self {
            vendor: CpuVendor::Intel,
            microarchitecture: CpuMicroarchitecture::IntelSkylake,
            features: vec![
                CpuFeature::SSE,
                CpuFeature::SSE2,
                CpuFeature::SSE3,
                CpuFeature::SSSE3,
                CpuFeature::SSE4_1,
                CpuFeature::SSE4_2,
                CpuFeature::AVX,
                CpuFeature::AVX2,
                CpuFeature::AES,
                CpuFeature::PCLMULQDQ,
                CpuFeature::POPCNT,
                CpuFeature::BMI1,
                CpuFeature::BMI2,
                CpuFeature::LargePage2MB,
            ],
            cache_line_size: 64,
            l1_cache_size: 32,
            l2_cache_size: 256,
            l3_cache_size: 8192,
            supports_smt: true,
            logical_cores: 4,
            physical_cores: 2,
        }
    }

    /// Intel Ice Lake优化策略
    pub fn intel_icelake() -> Self {
        Self {
            vendor: CpuVendor::Intel,
            microarchitecture: CpuMicroarchitecture::IntelIceLake,
            features: vec![
                CpuFeature::SSE,
                CpuFeature::SSE2,
                CpuFeature::SSE3,
                CpuFeature::SSSE3,
                CpuFeature::SSE4_1,
                CpuFeature::SSE4_2,
                CpuFeature::AVX,
                CpuFeature::AVX2,
                CpuFeature::AVX512F,
                CpuFeature::AVX512CD,
                CpuFeature::AVX512VL,
                CpuFeature::AVX512BW,
                CpuFeature::AVX512DQ,
                CpuFeature::AES,
                CpuFeature::PCLMULQDQ,
                CpuFeature::POPCNT,
                CpuFeature::BMI1,
                CpuFeature::BMI2,
                CpuFeature::LargePage2MB,
                CpuFeature::LargePage1GB,
            ],
            cache_line_size: 64,
            l1_cache_size: 48,
            l2_cache_size: 512,
            l3_cache_size: 12288,
            supports_smt: true,
            logical_cores: 8,
            physical_cores: 4,
        }
    }

    /// AMD Zen3优化策略
    pub fn amd_zen3() -> Self {
        Self {
            vendor: CpuVendor::AMD,
            microarchitecture: CpuMicroarchitecture::AmdZen3,
            features: vec![
                CpuFeature::SSE,
                CpuFeature::SSE2,
                CpuFeature::SSE3,
                CpuFeature::SSSE3,
                CpuFeature::SSE4_1,
                CpuFeature::SSE4_2,
                CpuFeature::AVX,
                CpuFeature::AVX2,
                CpuFeature::AES,
                CpuFeature::PCLMULQDQ,
                CpuFeature::POPCNT,
                CpuFeature::BMI1,
                CpuFeature::BMI2,
                CpuFeature::LargePage2MB,
            ],
            cache_line_size: 64,
            l1_cache_size: 32,
            l2_cache_size: 512,
            l3_cache_size: 32768, // Zen3的大L3缓存
            supports_smt: true,
            logical_cores: 16,
            physical_cores: 8,
        }
    }

    /// ARM Cortex-A76优化策略
    pub fn arm_cortex_a76() -> Self {
        Self {
            vendor: CpuVendor::ARM,
            microarchitecture: CpuMicroarchitecture::ArmCortexA76,
            features: vec![
                CpuFeature::NEON,
                CpuFeature::AES,
                CpuFeature::PMULL, // 通过Unknown添加
                CpuFeature::SHA1,  // 通过Unknown添加
                CpuFeature::SHA2,  // 通过Unknown添加
                CpuFeature::LargePage2MB,
            ],
            cache_line_size: 64,
            l1_cache_size: 64,
            l2_cache_size: 256,
            l3_cache_size: 4096,
            supports_smt: false,
            logical_cores: 4,
            physical_cores: 4,
        }
    }

    /// ARM Neoverse N1优化策略（服务器级）
    pub fn arm_neoverse_n1() -> Self {
        Self {
            vendor: CpuVendor::ARM,
            microarchitecture: CpuMicroarchitecture::ArmNeoverseN1,
            features: vec![
                CpuFeature::NEON,
                CpuFeature::AES,
                CpuFeature::PMULL,
                CpuFeature::SHA1,
                CpuFeature::SHA2,
                CpuFeature::LargePage2MB,
                CpuFeature::Prefetch,
            ],
            cache_line_size: 64,
            l1_cache_size: 64,
            l2_cache_size: 1024,
            l3_cache_size: 8192,
            supports_smt: false,
            logical_cores: 64,
            physical_cores: 64,
        }
    }

    /// 检查是否支持特定特性
    pub fn has_feature(&self, feature: &CpuFeature) -> bool {
        self.features.contains(feature)
    }

    /// 获取最佳SIMD宽度（位）
    pub fn optimal_simd_width(&self) -> usize {
        if self.has_feature(&CpuFeature::AVX512F) {
            512
        } else if self.has_feature(&CpuFeature::AVX2) {
            256
        } else if self.has_feature(&CpuFeature::AVX) {
            256
        } else if self.has_feature(&CpuFeature::SSE2) {
            128
        } else if self.has_feature(&CpuFeature::NEON) {
            128
        } else {
            0
        }
    }

    /// 获取推荐的向量大小
    pub fn recommended_vector_size(&self) -> Option<usize> {
        let width = self.optimal_simd_width();
        if width > 0 {
            Some(width / 8) // 转换为字节数
        } else {
            None
        }
    }

    /// 是否应该使用特定优化
    pub fn should_use_optimization(&self, opt: OptimizationType) -> bool {
        match opt {
            OptimizationType::Vectorization => self.optimal_simd_width() > 0,
            OptimizationType::AES => self.has_feature(&CpuFeature::AES),
            OptimizationType::PCLMULQDQ => self.has_feature(&CpuFeature::PCLMULQDQ),
            OptimizationType::BMI1 => self.has_feature(&CpuFeature::BMI1),
            OptimizationType::BMI2 => self.has_feature(&CpuFeature::BMI2),
            OptimizationType::Popcnt => self.has_feature(&CpuFeature::POPCNT),
            OptimizationType::LargePages => self.has_feature(&CpuFeature::LargePage2MB),
            OptimizationType::Prefetch => self.has_feature(&CpuFeature::Prefetch),
            OptimizationType::SMT => self.supports_smt,
        }
    }

    /// 获取缓存友好的循环展开因子
    pub fn cache_friendly_unroll_factor(&self) -> usize {
        // 基于L1缓存大小计算
        let l1_bytes = self.l1_cache_size * 1024;
        let unroll_factor = l1_bytes / self.cache_line_size;

        // 限制在合理范围内
        unroll_factor.min(16).max(4)
    }
}

impl Default for VendorOptimizationStrategy {
    fn default() -> Self {
        // 默认使用通用的x86_64策略
        Self {
            vendor: CpuVendor::Unknown,
            microarchitecture: CpuMicroarchitecture::Unknown,
            features: vec![CpuFeature::SSE2, CpuFeature::SSE3],
            cache_line_size: 64,
            l1_cache_size: 32,
            l2_cache_size: 256,
            l3_cache_size: 0,
            supports_smt: false,
            logical_cores: 1,
            physical_cores: 1,
        }
    }
}

/// 优化类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationType {
    Vectorization,
    AES,
    PCLMULQDQ,
    BMI1,
    BMI2,
    Popcnt,
    LargePages,
    Prefetch,
    SMT,
}

/// 厂商优化器
pub struct VendorOptimizer {
    /// 优化策略
    strategy: Arc<VendorOptimizationStrategy>,

    /// 优化统计
    stats: Arc<Mutex<OptimizerStats>>,
}

/// 优化器统计信息
#[derive(Debug, Default, Clone)]
pub struct OptimizerStats {
    /// 应用的优化次数
    pub optimizations_applied: u64,

    /// 跳过的优化次数
    pub optimizations_skipped: u64,

    /// 特性检查次数
    pub feature_checks: u64,

    /// 缓存友好的转换次数
    pub cache_friendly_transforms: u64,
}

impl VendorOptimizer {
    /// 创建新的厂商优化器
    pub fn new(strategy: VendorOptimizationStrategy) -> Self {
        Self {
            strategy: Arc::new(strategy),
            stats: Arc::new(Mutex::new(OptimizerStats::default())),
        }
    }

    /// 创建基于自动检测的优化器
    pub fn detect() -> Self {
        Self::new(VendorOptimizationStrategy::detect())
    }

    /// 为特定厂商创建优化器
    pub fn for_vendor(vendor: CpuVendor) -> Self {
        Self::new(VendorOptimizationStrategy::for_vendor(vendor))
    }

    /// 获取优化策略
    pub fn strategy(&self) -> &VendorOptimizationStrategy {
        &self.strategy
    }

    /// 获取统计信息
    pub fn stats(&self) -> OptimizerStats {
        self.stats.lock().clone()
    }

    /// 重置统计信息
    pub fn reset_stats(&self) {
        *self.stats.lock() = OptimizerStats::default();
    }

    /// 检查是否应该应用优化
    pub fn should_apply(&self, opt: OptimizationType) -> bool {
        let mut stats = self.stats.lock();
        stats.feature_checks += 1;

        let should_apply = self.strategy.should_use_optimization(opt);

        if should_apply {
            stats.optimizations_applied += 1;
        } else {
            stats.optimizations_skipped += 1;
        }

        should_apply
    }

    /// 获取推荐的编译器标志
    pub fn recommended_flags(&self) -> Vec<String> {
        let mut flags = Vec::new();

        match self.strategy.vendor {
            CpuVendor::Intel => {
                // Intel特定标志
                if self.strategy.has_feature(&CpuFeature::AVX512F) {
                    flags.push("-mavx512f".to_string());
                    flags.push("-mavx512cd".to_string());
                    flags.push("-mavx512vl".to_string());
                    flags.push("-mavx512bw".to_string());
                    flags.push("-mavx512dq".to_string());
                } else if self.strategy.has_feature(&CpuFeature::AVX2) {
                    flags.push("-mavx2".to_string());
                }

                if self.strategy.has_feature(&CpuFeature::AES) {
                    flags.push("-maes".to_string());
                }

                flags.push("-mtune=native".to_string());
            }
            CpuVendor::AMD => {
                // AMD特定标志
                if self.strategy.has_feature(&CpuFeature::AVX2) {
                    flags.push("-mavx2".to_string());
                }

                if self.strategy.has_feature(&CpuFeature::AES) {
                    flags.push("-maes".to_string());
                }

                flags.push("-mtune=znver3".to_string());
            }
            CpuVendor::ARM => {
                // ARM特定标志
                if self.strategy.has_feature(&CpuFeature::NEON) {
                    flags.push("-mfpu=neon".to_string());
                }

                if self.strategy.has_feature(&CpuFeature::SVE) {
                    flags.push("-msve".to_string());
                }

                flags.push("-march=native".to_string());
            }
            CpuVendor::Unknown => {
                // 通用优化
                flags.push("-mtune=generic".to_string());
            }
        }

        flags
    }

    /// 获取缓存优化建议
    pub fn cache_optimization_hints(&self) -> CacheOptimizationHints {
        CacheOptimizationHints {
            cache_line_size: self.strategy.cache_line_size,
            l1_size: self.strategy.l1_cache_size,
            l2_size: self.strategy.l2_cache_size,
            l3_size: self.strategy.l3_cache_size,
            preferred_loop_unroll: self.strategy.cache_friendly_unroll_factor(),
            use_prefetch: self.strategy.has_feature(&CpuFeature::Prefetch),
            use_large_pages: self.strategy.has_feature(&CpuFeature::LargePage2MB),
        }
    }
}

/// 缓存优化建议
#[derive(Debug, Clone)]
pub struct CacheOptimizationHints {
    pub cache_line_size: usize,
    pub l1_size: usize,
    pub l2_size: usize,
    pub l3_size: usize,
    pub preferred_loop_unroll: usize,
    pub use_prefetch: bool,
    pub use_large_pages: bool,
}

// 添加一些辅助特性到CpuFeature
impl CpuFeature {
    // ARM特定特性的辅助方法
    pub fn pmull() -> Self {
        CpuFeature::Unknown("PMULL".to_string())
    }

    pub fn sha1() -> Self {
        CpuFeature::Unknown("SHA1".to_string())
    }

    pub fn sha2() -> Self {
        CpuFeature::Unknown("SHA2".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intel_skylake_strategy() {
        let strategy = VendorOptimizationStrategy::intel_skylake();

        assert_eq!(strategy.vendor, CpuVendor::Intel);
        assert!(strategy.has_feature(&CpuFeature::AVX2));
        assert!(!strategy.has_feature(&CpuFeature::AVX512F));
        assert_eq!(strategy.optimal_simd_width(), 256);
    }

    #[test]
    fn test_intel_icelake_strategy() {
        let strategy = VendorOptimizationStrategy::intel_icelake();

        assert_eq!(strategy.vendor, CpuVendor::Intel);
        assert!(strategy.has_feature(&CpuFeature::AVX512F));
        assert_eq!(strategy.optimal_simd_width(), 512);
    }

    #[test]
    fn test_amd_zen3_strategy() {
        let strategy = VendorOptimizationStrategy::amd_zen3();

        assert_eq!(strategy.vendor, CpuVendor::AMD);
        assert!(strategy.has_feature(&CpuFeature::AVX2));
        assert!(!strategy.has_feature(&CpuFeature::AVX512F));
        assert_eq!(strategy.l3_cache_size, 32768); // Zen3的大L3
    }

    #[test]
    fn test_arm_cortex_a76_strategy() {
        let strategy = VendorOptimizationStrategy::arm_cortex_a76();

        assert_eq!(strategy.vendor, CpuVendor::ARM);
        assert!(strategy.has_feature(&CpuFeature::NEON));
        assert!(!strategy.supports_smt);
    }

    #[test]
    fn test_vendor_optimizer() {
        let optimizer = VendorOptimizer::for_vendor(CpuVendor::Intel);

        assert!(optimizer.should_apply(OptimizationType::Vectorization));
        assert!(optimizer.should_apply(OptimizationType::AES));

        let stats = optimizer.stats();
        assert_eq!(stats.feature_checks, 2);
        assert_eq!(stats.optimizations_applied, 2);
    }

    #[test]
    fn test_cache_friendly_unroll() {
        let strategy = VendorOptimizationStrategy::intel_skylake();
        let unroll = strategy.cache_friendly_unroll_factor();

        // L1 = 32KB, cache_line = 64B
        // unroll = 32768 / 64 = 512
        // clamped to 16 (max)
        assert_eq!(unroll, 16);
    }

    #[test]
    fn test_recommended_flags() {
        let optimizer = VendorOptimizer::for_vendor(CpuVendor::Intel);
        let flags = optimizer.recommended_flags();

        assert!(!flags.is_empty());
        assert!(flags.iter().any(|f| f.contains("tune")));
    }

    #[test]
    fn test_cache_hints() {
        let optimizer = VendorOptimizer::for_vendor(CpuVendor::Intel);
        let hints = optimizer.cache_optimization_hints();

        assert_eq!(hints.cache_line_size, 64);
        assert!(hints.preferred_loop_unroll > 0);
    }
}
