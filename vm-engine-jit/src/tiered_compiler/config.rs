use serde::{Deserialize, Serialize};

/// 分层编译器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TieredCompilerConfig {
    /// 解释器配置
    pub interpreter_config: InterpreterConfig,
    /// 基础JIT配置
    pub baseline_config: BaselineJITConfig,
    /// 优化JIT配置
    pub optimized_config: OptimizedJITConfig,
    /// 热点检测配置
    pub hotspot_config: HotspotConfig,
    /// 基础JIT编译阈值（执行次数）
    pub baseline_threshold: u32,
    /// 优化JIT编译阈值（执行次数）
    pub optimized_threshold: u32,
}

/// 解释器配置（Tier 1）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterpreterConfig {
    /// 最大指令数
    pub max_instructions: usize,
    /// 寄存器数量
    pub register_count: usize,
    /// 内存大小（字节）
    pub memory_size: usize,
    /// 启用指令计数
    pub enable_instruction_count: bool,
}

/// 基础JIT配置（Tier 2）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineJITConfig {
    /// 优化级别
    pub optimization_level: u8,
    /// 启用寄存器分配
    pub enable_register_allocation: bool,
    /// 启用基础优化
    pub enable_basic_optimizations: bool,
    /// 最大代码块大小
    pub max_block_size: usize,
}

/// 优化JIT配置（Tier 3）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizedJITConfig {
    /// 优化级别
    pub optimization_level: u8,
    /// 启用内联
    pub enable_inlining: bool,
    /// 内联阈值
    pub inline_threshold: usize,
    /// 启用逃逸分析
    pub enable_escape_analysis: bool,
    /// 启用循环优化
    pub enable_loop_optimizations: bool,
    /// 启用SIMD优化
    pub enable_simd_optimization: bool,
    /// 启用死代码消除
    pub enable_dead_code_elimination: bool,
    /// 启用常量折叠
    pub enable_constant_folding: bool,
    /// 最大内联深度
    pub max_inline_depth: usize,
}

/// 热点检测配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotspotConfig {
    /// 采样间隔（纳秒）
    pub sampling_interval_ns: u64,
    /// 热点阈值
    pub hotspot_threshold: u32,
    /// 热点窗口大小
    pub hotspot_window_size: usize,
    /// 启用自适应检测
    pub enable_adaptive_detection: bool,
}

impl Default for InterpreterConfig {
    fn default() -> Self {
        Self {
            max_instructions: 1000000,
            register_count: 32,
            memory_size: 1024 * 1024, // 1MB
            enable_instruction_count: true,
        }
    }
}

impl Default for BaselineJITConfig {
    fn default() -> Self {
        Self {
            optimization_level: 1,
            enable_register_allocation: true,
            enable_basic_optimizations: true,
            max_block_size: 1024,
        }
    }
}

impl Default for OptimizedJITConfig {
    fn default() -> Self {
        Self {
            optimization_level: 3,
            enable_inlining: true,
            inline_threshold: 50,
            enable_escape_analysis: true,
            enable_loop_optimizations: true,
            enable_simd_optimization: false,
            enable_dead_code_elimination: true,
            enable_constant_folding: true,
            max_inline_depth: 3,
        }
    }
}

impl Default for HotspotConfig {
    fn default() -> Self {
        Self {
            sampling_interval_ns: 1000000, // 1ms
            hotspot_threshold: 100,
            hotspot_window_size: 1000,
            enable_adaptive_detection: true,
        }
    }
}
