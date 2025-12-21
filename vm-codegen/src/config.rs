//! 代码生成配置
//!
//! 定义AOT代码生成模式和编译选项

/// 代码生成模式
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum CodegenMode {
    /// 直接生成机器码
    #[default]
    Direct,
    /// 通过 Cranelift 生成
    Cranelift,
    /// 通过 LLVM 生成（可选）
    #[cfg(feature = "llvm-backend")]
    LLVM,
}

impl std::fmt::Display for CodegenMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodegenMode::Direct => write!(f, "direct"),
            CodegenMode::Cranelift => write!(f, "cranelift"),
            #[cfg(feature = "llvm-backend")]
            CodegenMode::LLVM => write!(f, "llvm"),
        }
    }
}

impl std::str::FromStr for CodegenMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "direct" => Ok(CodegenMode::Direct),
            "cranelift" => Ok(CodegenMode::Cranelift),
            #[cfg(feature = "llvm-backend")]
            "llvm" => Ok(CodegenMode::LLVM),
            _ => Err(format!("无效的代码生成模式: {}", s)),
        }
    }
}

/// 优化级别
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum OptimizationLevel {
    /// 无优化
    None = 0,
    /// 基础优化
    Basic = 1,
    /// 标准优化
    #[default]
    Standard = 2,
    /// 激进优化
    Aggressive = 3,
}

impl std::fmt::Display for OptimizationLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OptimizationLevel::None => write!(f, "none"),
            OptimizationLevel::Basic => write!(f, "basic"),
            OptimizationLevel::Standard => write!(f, "standard"),
            OptimizationLevel::Aggressive => write!(f, "aggressive"),
        }
    }
}

impl std::str::FromStr for OptimizationLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "none" | "0" => Ok(OptimizationLevel::None),
            "basic" | "1" => Ok(OptimizationLevel::Basic),
            "standard" | "2" => Ok(OptimizationLevel::Standard),
            "aggressive" | "3" => Ok(OptimizationLevel::Aggressive),
            _ => Err(format!("无效的优化级别: {}", s)),
        }
    }
}

use vm_core::TargetArch;

/// 编译选项
#[derive(Clone, Debug, PartialEq)]
pub struct CompilationOptions {
    /// 代码生成模式
    pub codegen_mode: CodegenMode,
    /// 优化级别
    pub optimization_level: OptimizationLevel,
    /// 目标架构
    pub target_arch: TargetArch,
    /// 是否启用调试信息
    pub debug_info: bool,
    /// 是否启用符号信息
    pub symbols: bool,
    /// 是否启用LTO (Link Time Optimization)
    pub lto: bool,
    /// 是否启用增量编译
    pub incremental: bool,
    /// 输出路径
    pub output_path: Option<String>,
}

impl Default for CompilationOptions {
    fn default() -> Self {
        Self {
            codegen_mode: CodegenMode::default(),
            optimization_level: OptimizationLevel::default(),
            target_arch: TargetArch::default(),
            debug_info: false,
            symbols: false,
            lto: false,
            incremental: true,
            output_path: None,
        }
    }
}

impl CompilationOptions {
    /// 创建新的编译选项
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置代码生成模式
    pub fn with_codegen_mode(mut self, mode: CodegenMode) -> Self {
        self.codegen_mode = mode;
        self
    }

    /// 设置优化级别
    pub fn with_optimization_level(mut self, level: OptimizationLevel) -> Self {
        self.optimization_level = level;
        self
    }

    /// 设置目标架构
    pub fn with_target_arch(mut self, arch: TargetArch) -> Self {
        self.target_arch = arch;
        self
    }

    /// 启用调试信息
    pub fn with_debug_info(mut self, enabled: bool) -> Self {
        self.debug_info = enabled;
        self
    }

    /// 启用符号信息
    pub fn with_symbols(mut self, enabled: bool) -> Self {
        self.symbols = enabled;
        self
    }

    /// 启用LTO
    pub fn with_lto(mut self, enabled: bool) -> Self {
        self.lto = enabled;
        self
    }

    /// 启用增量编译
    pub fn with_incremental(mut self, enabled: bool) -> Self {
        self.incremental = enabled;
        self
    }

    /// 设置输出路径
    pub fn with_output_path<S: Into<String>>(mut self, path: S) -> Self {
        self.output_path = Some(path.into());
        self
    }
}

/// 代码生成统计信息
#[derive(Clone, Debug, Default)]
pub struct CodegenStats {
    /// 已处理的块数量
    pub blocks_processed: u64,
    /// 生成的指令数量
    pub instructions_generated: u64,
    /// 总代码大小（字节）
    pub code_size_bytes: u64,
    /// 编译时间（毫秒）
    pub compile_time_ms: u64,
    /// 优化时间（毫秒）
    pub optimization_time_ms: u64,
    /// 缓存命中次数
    pub cache_hits: u64,
    /// 缓存未命中次数
    pub cache_misses: u64,
}

impl CodegenStats {
    /// 创建新的统计信息
    pub fn new() -> Self {
        Self::default()
    }

    /// 获取缓存命中率
    pub fn cache_hit_rate(&self) -> f64 {
        if self.cache_hits + self.cache_misses == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / (self.cache_hits + self.cache_misses) as f64
    }

    /// 重置统计信息
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
