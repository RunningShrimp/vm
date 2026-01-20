//! 优化策略实现（基础设施层）
//!
//! 提供优化策略的基础设施层实现，适配领域层定义的 OptimizationStrategy trait。
//! 这是基础设施层的实现，具体的技术细节应在此模块中。

use std::sync::Arc;

use vm_core::domain::{OptimizationStrategy, OptimizationType};
use vm_core::{VmError, VmResult};
use vm_ir::IRBlock;

use crate::jit::optimizer::{DefaultIROptimizer, IROptimizer};

/// 优化策略实现（基础设施层）
///
/// 将 vm-engine 的优化器适配到领域层定义的 OptimizationStrategy trait。
pub struct OptimizationStrategyImpl {
    /// 内部优化器
    optimizer: Arc<tokio::sync::Mutex<DefaultIROptimizer>>,
    /// 优化级别
    opt_level: u32,
}

impl OptimizationStrategyImpl {
    /// 创建新的优化策略实现
    pub fn new(opt_level: u32) -> Self {
        // 创建默认配置
        let config = crate::jit::core::JITConfig {
            optimization_level: opt_level as u8,
            ..Default::default()
        };

        let optimizer = DefaultIROptimizer::new(config);

        Self {
            optimizer: Arc::new(tokio::sync::Mutex::new(optimizer)),
            opt_level,
        }
    }

    /// 从现有优化器创建
    pub fn from_optimizer(optimizer: DefaultIROptimizer, opt_level: u32) -> Self {
        Self {
            optimizer: Arc::new(tokio::sync::Mutex::new(optimizer)),
            opt_level,
        }
    }
}

impl OptimizationStrategy for OptimizationStrategyImpl {
    fn optimize_ir(&self, ir: &[u8]) -> VmResult<Vec<u8>> {
        // 反序列化 IR 块
        // 注意：这里使用 bincode serde 进行序列化/反序列化
        // 实际实现中应该使用更高效的序列化方式
        let (block, _): (IRBlock, usize) =
            bincode::serde::decode_from_slice(ir, bincode::config::standard()).map_err(|e| {
                VmError::Core(vm_core::CoreError::InvalidState {
                    message: format!("Failed to deserialize IR: {}", e),
                    current: "".to_string(),
                    expected: "".to_string(),
                })
            })?;

        // 使用同步方式调用优化器
        // 注意：这里需要处理 async 上下文
        let rt = tokio::runtime::Handle::try_current();

        if let Ok(handle) = rt {
            // 在 async 上下文中
            let optimizer = self.optimizer.clone();
            handle.block_on(async move {
                let mut opt = optimizer.lock().await;
                let optimized = opt.optimize(&block)?;

                // 序列化优化后的 IR
                bincode::serde::encode_to_vec(&optimized, bincode::config::standard()).map_err(
                    |e| {
                        VmError::Core(vm_core::CoreError::InvalidState {
                            message: format!("Failed to serialize IR: {}", e),
                            current: "".to_string(),
                            expected: "".to_string(),
                        })
                    },
                )
            })
        } else {
            // 同步上下文 - 创建临时 runtime
            let rt = tokio::runtime::Runtime::new().map_err(|e| {
                VmError::Core(vm_core::CoreError::InvalidState {
                    message: format!("Failed to create tokio runtime: {}", e),
                    current: "".to_string(),
                    expected: "".to_string(),
                })
            })?;

            let optimizer = self.optimizer.clone();
            rt.block_on(async move {
                let mut opt = optimizer.lock().await;
                let optimized = opt.optimize(&block)?;

                // 序列化优化后的 IR
                bincode::serde::encode_to_vec(&optimized, bincode::config::standard()).map_err(
                    |e| {
                        VmError::Core(vm_core::CoreError::InvalidState {
                            message: format!("Failed to serialize IR: {}", e),
                            current: "".to_string(),
                            expected: "".to_string(),
                        })
                    },
                )
            })
        }
    }

    fn optimization_level(&self) -> u32 {
        self.opt_level
    }

    fn supports_optimization(&self, opt_type: OptimizationType) -> bool {
        match opt_type {
            OptimizationType::ConstantFolding => true,
            OptimizationType::DeadCodeElimination => true,
            OptimizationType::InstructionCombining => true,
            OptimizationType::LoopOptimization => self.opt_level >= 2,
            OptimizationType::RegisterAllocation => self.opt_level >= 1,
            OptimizationType::InstructionScheduling => self.opt_level >= 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimization_strategy_creation() {
        let strategy = OptimizationStrategyImpl::new(2);
        assert_eq!(strategy.optimization_level(), 2);
    }

    #[test]
    fn test_supports_optimization() {
        let strategy = OptimizationStrategyImpl::new(2);
        assert!(strategy.supports_optimization(OptimizationType::ConstantFolding));
        assert!(strategy.supports_optimization(OptimizationType::LoopOptimization));
    }
}
