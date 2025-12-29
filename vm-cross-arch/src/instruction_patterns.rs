#![allow(dead_code)]
//! 指令模式匹配模块
//!
//! 用于识别和优化常见的指令模式

use vm_ir::IROp;

/// 指令模式
#[derive(Debug, Clone, PartialEq)]
pub enum InstructionPattern {
    /// 寄存器到寄存器的移动
    MoveRegToReg,
    /// 立即数加载
    LoadImmediate,
    /// 内存加载
    LoadFromMemory,
    /// 内存存储
    StoreToMemory,
    /// 算术运算
    ArithmeticOp,
    /// 逻辑运算
    LogicalOp,
    /// 比较操作
    CompareOp,
    /// 条件分支
    ConditionalBranch,
    /// 无条件跳转
    UnconditionalJump,
}

/// 识别IR操作的指令模式
pub fn identify_pattern(op: &IROp) -> Option<InstructionPattern> {
    match op {
        IROp::MovImm { .. } => Some(InstructionPattern::LoadImmediate),
        IROp::Load { .. } => Some(InstructionPattern::LoadFromMemory),
        IROp::Store { .. } => Some(InstructionPattern::StoreToMemory),
        IROp::Add { .. } | IROp::Sub { .. } | IROp::Mul { .. } | IROp::Div { .. } => {
            Some(InstructionPattern::ArithmeticOp)
        }
        IROp::And { .. } | IROp::Or { .. } | IROp::Xor { .. } | IROp::Not { .. } => {
            Some(InstructionPattern::LogicalOp)
        }
        IROp::CmpEq { .. } | IROp::CmpNe { .. } | IROp::CmpLt { .. } | IROp::CmpGe { .. } => {
            Some(InstructionPattern::CompareOp)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_identification() {
        let add_op = IROp::Add {
            dst: 0,
            src1: 1,
            src2: 2,
        };
        assert_eq!(
            identify_pattern(&add_op),
            Some(InstructionPattern::ArithmeticOp)
        );
    }
}
