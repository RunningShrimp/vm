//! 编译器后端抽象层
//!
//! 定义统一的编译器接口，支持多种后端实现，包括Cranelift和LLVM。

use vm_ir::{IROp, IRBlock};
use std::fmt;

/// 编译器错误类型
#[derive(Debug, Clone)]
pub enum CompilerError {
    /// 编译失败
    CompilationFailed(String),
    /// 不支持的操作
    UnsupportedOperation(String),
    /// 优化失败
    OptimizationFailed(String),
    /// 后端不可用
    BackendUnavailable(String),
}

impl fmt::Display for CompilerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CompilerError::CompilationFailed(msg) => write!(f, "Compilation failed: {}", msg),
            CompilerError::UnsupportedOperation(msg) => write!(f, "Unsupported operation: {}", msg),
            CompilerError::OptimizationFailed(msg) => write!(f, "Optimization failed: {}", msg),
            CompilerError::BackendUnavailable(msg) => write!(f, "Backend unavailable: {}", msg),
        }
    }
}

impl std::error::Error for CompilerError {}

/// 编译器特性
#[derive(Debug, Clone, PartialEq)]
pub enum CompilerFeature {
    /// SIMD支持
    Simd,
    /// 向量化
    Vectorization,
    /// 循环优化
    LoopOptimization,
    /// 内联汇编
    InlineAssembly,
    /// 高级优化
    AdvancedOptimization,
    /// 调试信息
    DebugInfo,
}

/// 优化级别
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OptimizationLevel {
    /// 无优化
    O0,
    /// 基本优化
    O1,
    /// 标准优化
    O2,
    /// 高级优化
    O3,
}

/// 编译器后端类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CompilerBackendType {
    /// Cranelift后端
    Cranelift,
    /// LLVM后端
    #[cfg(feature = "llvm-backend")]
    LLVM,
}

/// 统一编译器接口
pub trait CompilerBackend: Send + Sync {
    /// 编译IR块到机器码
    fn compile(&mut self, block: &IRBlock) -> Result<Vec<u8>, CompilerError>;
    
    /// 获取后端名称
    fn name(&self) -> &str;
    
    /// 获取支持的特性
    fn supported_features(&self) -> Vec<CompilerFeature>;
    
    /// 应用优化
    fn optimize(&mut self, block: &mut IRBlock, level: OptimizationLevel) -> Result<(), CompilerError>;
    
    /// 检查是否支持特定特性
    fn supports_feature(&self, feature: &CompilerFeature) -> bool {
        self.supported_features().contains(feature)
    }
    
    /// 获取后端类型
    fn backend_type(&self) -> CompilerBackendType;
}

/// 编译器统计信息
#[derive(Debug, Clone, Default)]
pub struct CompilerStats {
    /// 编译的块数
    pub compiled_blocks: u64,
    /// 总编译时间（纳秒）
    pub total_compile_time_ns: u64,
    /// 平均编译时间（纳秒）
    pub avg_compile_time_ns: u64,
    /// 生成的代码大小（字节）
    pub generated_code_size: u64,
    /// 优化次数
    pub optimization_passes: u64,
}

impl CompilerStats {
    /// 创建新的统计信息
    pub fn new() -> Self {
        Self::default()
    }
    
    /// 更新编译统计
    pub fn update_compile(&mut self, compile_time_ns: u64, code_size: usize) {
        self.compiled_blocks += 1;
        self.total_compile_time_ns += compile_time_ns;
        self.generated_code_size += code_size as u64;
        
        if self.compiled_blocks > 0 {
            self.avg_compile_time_ns = self.total_compile_time_ns / self.compiled_blocks;
        }
    }
    
    /// 更新优化统计
    pub fn update_optimization(&mut self, passes: u64) {
        self.optimization_passes += passes;
    }
    
    /// 生成统计报告
    pub fn report(&self) -> String {
        format!(
            r#"Compiler Statistics:
  - Compiled Blocks: {}
  - Total Compile Time: {} ms
  - Average Compile Time: {} μs
  - Generated Code Size: {} bytes
  - Optimization Passes: {}
"#,
            self.compiled_blocks,
            self.total_compile_time_ns / 1_000_000,
            self.avg_compile_time_ns / 1_000,
            self.generated_code_size,
            self.optimization_passes
        )
    }
}
