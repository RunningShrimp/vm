//! IR处理模块
//!
//! 处理IR块的优化、转换和准备

use vm_ir::{IRBlock, IROp};
use vm_ir_lift::optimizer::{OptimizationLevel, PassManager};

use crate::optimizer::apply_optimization_passes;

/// IR处理器
pub struct IrProcessor {
    /// 优化级别
    optimization_level: OptimizationLevel,
}

impl IrProcessor {
    /// 创建新的IR处理器
    pub fn new(optimization_level: OptimizationLevel) -> Self {
        Self {
            optimization_level,
        }
    }

    /// 处理IR块（优化）
    ///
    /// 对IR块应用优化Pass
    pub fn process_ir_block(&self, block: &IRBlock) -> IRBlock {
        if self.optimization_level == OptimizationLevel::O0 {
            // O0级别，不优化
            return block.clone();
        }

        let pass_manager = PassManager::new(self.optimization_level);
        let mut ops = block.ops.clone();
        
        // 执行优化Pass
        apply_optimization_passes(&mut ops, &pass_manager);

        IRBlock {
            start_pc: block.start_pc,
            ops,
            term: block.term.clone(),
        }
    }

    /// 处理IR操作列表
    pub fn process_ir_ops(&self, ops: &[IROp]) -> Vec<IROp> {
        if self.optimization_level == OptimizationLevel::O0 {
            return ops.to_vec();
        }

        let pass_manager = PassManager::new(self.optimization_level);
        let mut optimized_ops = ops.to_vec();
        
        apply_optimization_passes(&mut optimized_ops, &pass_manager);

        optimized_ops
    }

    /// 验证IR块的有效性
    pub fn validate_ir_block(block: &IRBlock) -> Result<(), String> {
        // 检查基本有效性
        if block.ops.is_empty() && matches!(block.term, vm_ir::Terminator::Ret) {
            return Err("Empty block with Ret terminator".to_string());
        }

        // 检查终结符的有效性
        match &block.term {
            vm_ir::Terminator::Jmp { target } => {
                if *target == 0 {
                    return Err("Invalid jump target: 0".to_string());
                }
            }
            vm_ir::Terminator::CondJmp {
                target_true,
                target_false,
                ..
            } => {
                if *target_true == 0 || *target_false == 0 {
                    return Err("Invalid conditional jump target: 0".to_string());
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// 统计IR块的复杂度
    pub fn calculate_complexity(block: &IRBlock) -> u32 {
        let mut complexity = block.ops.len() as u32;

        // 根据操作类型增加复杂度
        for op in &block.ops {
            match op {
                IROp::Div { .. } | IROp::Mul { .. } => complexity += 2,
                IROp::Load { .. } | IROp::Store { .. } => complexity += 1,
                _ => {}
            }
        }

        // 根据终结符类型增加复杂度
        match &block.term {
            vm_ir::Terminator::CondJmp { .. } => complexity += 1,
            vm_ir::Terminator::Call { .. } => complexity += 2,
            _ => {}
        }

        complexity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_ir::{IRBuilder, Terminator};

    #[test]
    fn test_process_ir_block() {
        let mut builder = IRBuilder::new(0x1000);
        builder.add_op(IROp::MovImm { dst: 1, imm: 10 });
        builder.add_op(IROp::Add {
            dst: 2,
            src1: 1,
            src2: 1,
        });
        builder.set_terminator(Terminator::Ret);
        let block = builder.build();

        let processor = IrProcessor::new(OptimizationLevel::O1);
        let processed = processor.process_ir_block(&block);

        // 处理后应该仍然有效
        assert_eq!(processed.start_pc, block.start_pc);
        assert!(!processed.ops.is_empty());
    }

    #[test]
    fn test_validate_ir_block() {
        let mut builder = IRBuilder::new(0x1000);
        builder.add_op(IROp::MovImm { dst: 1, imm: 10 });
        builder.set_terminator(Terminator::Jmp { target: 0x2000 });
        let block = builder.build();

        assert!(IrProcessor::validate_ir_block(&block).is_ok());
    }

    #[test]
    fn test_calculate_complexity() {
        let mut builder = IRBuilder::new(0x1000);
        builder.add_op(IROp::MovImm { dst: 1, imm: 10 });
        builder.add_op(IROp::Add {
            dst: 2,
            src1: 1,
            src2: 1,
        });
        builder.add_op(IROp::Load {
            dst: 3,
            base: 1,
            offset: 0,
            size: 8,
            flags: Default::default(),
        });
        builder.set_terminator(Terminator::Ret);
        let block = builder.build();

        let complexity = IrProcessor::calculate_complexity(&block);
        assert!(complexity >= 3); // 至少3个操作
    }

    #[test]
    fn test_process_ir_ops() {
        let ops = vec![
            IROp::MovImm { dst: 1, imm: 10 },
            IROp::Add {
                dst: 2,
                src1: 1,
                src2: 1,
            },
        ];

        let processor = IrProcessor::new(OptimizationLevel::O1);
        let processed = processor.process_ir_ops(&ops);

        assert_eq!(processed.len(), ops.len());
    }

    #[test]
    fn test_process_ir_block_o0() {
        let mut builder = IRBuilder::new(0x1000);
        builder.add_op(IROp::MovImm { dst: 1, imm: 10 });
        builder.set_terminator(Terminator::Ret);
        let block = builder.build();

        let processor = IrProcessor::new(OptimizationLevel::O0);
        let processed = processor.process_ir_block(&block);

        // O0级别应该不优化，结果应该相同
        assert_eq!(processed.ops.len(), block.ops.len());
    }

    #[test]
    fn test_validate_ir_block_invalid_jump() {
        let mut builder = IRBuilder::new(0x1000);
        builder.add_op(IROp::MovImm { dst: 1, imm: 10 });
        builder.set_terminator(Terminator::Jmp { target: 0 });
        let block = builder.build();

        let result = IrProcessor::validate_ir_block(&block);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_ir_block_invalid_cond_jump() {
        let mut builder = IRBuilder::new(0x1000);
        builder.add_op(IROp::CmpEq { dst: 1, lhs: 2, rhs: 3 });
        builder.set_terminator(Terminator::CondJmp {
            cond: 1,
            target_true: 0,
            target_false: 0x3000,
        });
        let block = builder.build();

        let result = IrProcessor::validate_ir_block(&block);
        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_complexity_with_div() {
        let mut builder = IRBuilder::new(0x1000);
        builder.add_op(IROp::Div {
            dst: 1,
            src1: 2,
            src2: 3,
        });
        builder.set_terminator(Terminator::Ret);
        let block = builder.build();

        let complexity = IrProcessor::calculate_complexity(&block);
        // Div操作应该增加额外复杂度
        assert!(complexity >= 3); // 1个操作 + 2（Div的额外复杂度）
    }

    #[test]
    fn test_calculate_complexity_with_call() {
        let mut builder = IRBuilder::new(0x1000);
        builder.add_op(IROp::MovImm { dst: 1, imm: 10 });
        builder.set_terminator(Terminator::Call {
            target: 0x5000,
            args: vec![],
        });
        let block = builder.build();

        let complexity = IrProcessor::calculate_complexity(&block);
        // Call终结符应该增加额外复杂度
        assert!(complexity >= 3); // 1个操作 + 1（操作） + 2（Call的额外复杂度）
    }
}

