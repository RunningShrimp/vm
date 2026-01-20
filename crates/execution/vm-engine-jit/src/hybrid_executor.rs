//! 混合执行器占位实现

use crate::aot_format::AotError;

#[derive(Debug, Clone)]
pub struct AotFailureReason;

#[derive(Debug, Clone)]
pub enum CodeSource {
    JIT,
    AOT,
}

#[derive(Debug, Clone)]
pub struct ExecutionStats;

/// 混合执行器配置
#[derive(Debug, Clone, Default)]
pub struct HybridConfig {
    /// JIT编译阈值
    pub jit_threshold: usize,
    /// AOT回退阈值
    pub aot_threshold: usize,
    /// 是否启用自适应切换
    pub enable_adaptive: bool,
}

pub struct HybridExecutor {
    pub config: HybridConfig,
}

impl HybridExecutor {
    pub fn new(config: HybridConfig) -> Result<Self, AotError> {
        Ok(Self { config })
    }
}
