mod backend;
mod compiler;
mod core;

// 声明所有JIT子模块
mod branch_target_cache;
mod code_cache;
mod codegen;
pub mod executable_memory;
mod instruction_scheduler;
mod optimizer;
mod tiered_cache;
pub mod translation_optimizer;

pub mod branch_prediction;
pub mod register_allocator;
pub mod translation_cache;

pub use backend::{
    BackendType, CompiledCode, JITBackend, JITBackendImpl, JITConfig, JITStats, OptLevel,
};
pub use compiler::JITCompiler;

/// 自适应阈值配置
#[derive(Debug, Clone, Default)]
pub struct AdaptiveThresholdConfig {
    pub hot_threshold: u64,
    pub cold_threshold: u64,
    pub enable_adaptive: bool,
}

/// 自适应阈值统计（占位符）
#[derive(Debug, Clone, Default)]
pub struct AdaptiveThresholdStats {
    pub hits: usize,
    pub misses: usize,
    pub execution_count: u64,
}

/// 代码指针（占位符）
pub type CodePtr = u64;

/// JIT类型别名（占位符）
pub type Jit = JITCompiler;
