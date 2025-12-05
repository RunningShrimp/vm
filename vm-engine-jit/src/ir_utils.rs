//! IR工具模块
//!
//! 提供IR操作的通用工具函数，避免代码重复

use vm_ir::{IROp, RegId};
use std::collections::HashSet;

/// IR操作分析工具
pub struct IrAnalyzer;

impl IrAnalyzer {
    /// 收集操作中读取的寄存器
    pub fn collect_read_regs(op: &IROp) -> Vec<RegId> {
        match op {
            IROp::Add { src1, src2, .. }
            | IROp::Sub { src1, src2, .. }
            | IROp::Mul { src1, src2, .. }
            | IROp::Div { src1, src2, .. }
            | IROp::Rem { src1, src2, .. }
            | IROp::And { src1, src2, .. }
            | IROp::Or { src1, src2, .. }
            | IROp::Xor { src1, src2, .. } => vec![*src1, *src2],

            IROp::Sll { src, shreg, .. }
            | IROp::Srl { src, shreg, .. }
            | IROp::Sra { src, shreg, .. } => vec![*src, *shreg],

            IROp::Not { src, .. } => vec![*src],
            IROp::Load { base, .. } => vec![*base],

            IROp::Store { src, base, .. } => vec![*src, *base],

            IROp::CmpEq { lhs, rhs, .. }
            | IROp::CmpNe { lhs, rhs, .. }
            | IROp::CmpLt { lhs, rhs, .. }
            | IROp::CmpLtU { lhs, rhs, .. }
            | IROp::CmpGe { lhs, rhs, .. }
            | IROp::CmpGeU { lhs, rhs, .. } => vec![*lhs, *rhs],

            IROp::Beq { cond, .. }
            | IROp::Bne { cond, .. }
            | IROp::Blt { cond, .. }
            | IROp::Bge { cond, .. }
            | IROp::Bltu { cond, .. }
            | IROp::Bgeu { cond, .. } => vec![*cond],

            _ => vec![],
        }
    }

    /// 收集操作中读取的寄存器（返回HashSet）
    pub fn collect_read_regs_set(op: &IROp) -> HashSet<RegId> {
        Self::collect_read_regs(op).into_iter().collect()
    }

    /// 收集操作中写入的寄存器
    pub fn collect_written_regs(op: &IROp) -> Vec<RegId> {
        match op {
            IROp::Add { dst, .. }
            | IROp::Sub { dst, .. }
            | IROp::Mul { dst, .. }
            | IROp::Div { dst, .. }
            | IROp::Rem { dst, .. }
            | IROp::AddImm { dst, .. }
            | IROp::MulImm { dst, .. }
            | IROp::MovImm { dst, .. }
            | IROp::And { dst, .. }
            | IROp::Or { dst, .. }
            | IROp::Xor { dst, .. }
            | IROp::Not { dst, .. }
            | IROp::Sll { dst, .. }
            | IROp::Srl { dst, .. }
            | IROp::Sra { dst, .. }
            | IROp::SllImm { dst, .. }
            | IROp::SrlImm { dst, .. }
            | IROp::SraImm { dst, .. }
            | IROp::Load { dst, .. }
            | IROp::CmpEq { dst, .. }
            | IROp::CmpNe { dst, .. }
            | IROp::CmpLt { dst, .. }
            | IROp::CmpLtU { dst, .. }
            | IROp::CmpGe { dst, .. }
            | IROp::CmpGeU { dst, .. } => vec![*dst],

            _ => vec![],
        }
    }

    /// 收集操作中写入的寄存器（返回HashSet）
    pub fn collect_written_regs_set(op: &IROp) -> HashSet<RegId> {
        Self::collect_written_regs(op).into_iter().collect()
    }

    /// 检查操作是否读取指定寄存器
    pub fn reads_reg(op: &IROp, reg: RegId) -> bool {
        Self::collect_read_regs(op).contains(&reg)
    }

    /// 检查操作是否写入指定寄存器
    pub fn writes_reg(op: &IROp, reg: RegId) -> bool {
        Self::collect_written_regs(op).contains(&reg)
    }

    /// 检查操作是否有副作用（如Store、Call等）
    pub fn has_side_effects(op: &IROp) -> bool {
        matches!(
            op,
            IROp::Store { .. }
                | IROp::Call { .. }
                | IROp::Syscall { .. }
                | IROp::Breakpoint
        )
    }

    /// 检查操作是否是内存访问
    pub fn is_memory_access(op: &IROp) -> bool {
        matches!(op, IROp::Load { .. } | IROp::Store { .. })
    }

    /// 检查操作是否是分支指令
    pub fn is_branch(op: &IROp) -> bool {
        matches!(
            op,
            IROp::Beq { .. }
                | IROp::Bne { .. }
                | IROp::Blt { .. }
                | IROp::Bge { .. }
                | IROp::Bltu { .. }
                | IROp::Bgeu { .. }
        )
    }

    /// 获取操作的延迟（cycles）
    pub fn get_latency(op: &IROp) -> u32 {
        match op {
            IROp::Load { .. } | IROp::Store { .. } => 3,
            IROp::Mul { .. } | IROp::Div { .. } | IROp::Rem { .. } => 3,
            _ => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_read_regs() {
        let op = IROp::Add {
            dst: 1,
            src1: 2,
            src2: 3,
        };
        let reads = IrAnalyzer::collect_read_regs(&op);
        assert_eq!(reads, vec![2, 3]);
    }

    #[test]
    fn test_collect_written_regs() {
        let op = IROp::Add {
            dst: 1,
            src1: 2,
            src2: 3,
        };
        let writes = IrAnalyzer::collect_written_regs(&op);
        assert_eq!(writes, vec![1]);
    }

    #[test]
    fn test_is_memory_access() {
        assert!(IrAnalyzer::is_memory_access(&IROp::Load {
            dst: 1,
            base: 2,
            offset: 0,
            size: 8,
            flags: Default::default(),
        }));
        assert!(IrAnalyzer::is_memory_access(&IROp::Store {
            src: 1,
            base: 2,
            offset: 0,
            size: 8,
            flags: Default::default(),
        }));
        assert!(!IrAnalyzer::is_memory_access(&IROp::Add {
            dst: 1,
            src1: 2,
            src2: 3
        }));
    }

    #[test]
    fn test_collect_read_regs_set() {
        let op = IROp::Add {
            dst: 1,
            src1: 2,
            src2: 3,
        };
        let reads_set = IrAnalyzer::collect_read_regs_set(&op);
        assert_eq!(reads_set.len(), 2);
        assert!(reads_set.contains(&2));
        assert!(reads_set.contains(&3));
    }

    #[test]
    fn test_collect_written_regs_set() {
        let op = IROp::Add {
            dst: 1,
            src1: 2,
            src2: 3,
        };
        let writes_set = IrAnalyzer::collect_written_regs_set(&op);
        assert_eq!(writes_set.len(), 1);
        assert!(writes_set.contains(&1));
    }

    #[test]
    fn test_reads_reg() {
        let op = IROp::Add {
            dst: 1,
            src1: 2,
            src2: 3,
        };
        assert!(IrAnalyzer::reads_reg(&op, 2));
        assert!(IrAnalyzer::reads_reg(&op, 3));
        assert!(!IrAnalyzer::reads_reg(&op, 1));
    }

    #[test]
    fn test_writes_reg() {
        let op = IROp::Add {
            dst: 1,
            src1: 2,
            src2: 3,
        };
        assert!(IrAnalyzer::writes_reg(&op, 1));
        assert!(!IrAnalyzer::writes_reg(&op, 2));
        assert!(!IrAnalyzer::writes_reg(&op, 3));
    }

    #[test]
    fn test_has_side_effects() {
        assert!(IrAnalyzer::has_side_effects(&IROp::Store {
            src: 1,
            base: 2,
            offset: 0,
            size: 8,
            flags: Default::default(),
        }));
        assert!(IrAnalyzer::has_side_effects(&IROp::Call {
            target: 0x1000,
            args: vec![],
        }));
        assert!(!IrAnalyzer::has_side_effects(&IROp::Add {
            dst: 1,
            src1: 2,
            src2: 3
        }));
    }

    #[test]
    fn test_is_branch() {
        assert!(IrAnalyzer::is_branch(&IROp::Beq {
            cond: 1,
            target: 0x1000,
        }));
        assert!(IrAnalyzer::is_branch(&IROp::Bne {
            cond: 1,
            target: 0x2000,
        }));
        assert!(!IrAnalyzer::is_branch(&IROp::Add {
            dst: 1,
            src1: 2,
            src2: 3
        }));
    }

    #[test]
    fn test_get_latency() {
        assert_eq!(
            IrAnalyzer::get_latency(&IROp::Load {
                dst: 1,
                base: 2,
                offset: 0,
                size: 8,
                flags: Default::default(),
            }),
            3
        );
        assert_eq!(
            IrAnalyzer::get_latency(&IROp::Mul {
                dst: 1,
                src1: 2,
                src2: 3
            }),
            3
        );
        assert_eq!(
            IrAnalyzer::get_latency(&IROp::Add {
                dst: 1,
                src1: 2,
                src2: 3
            }),
            1
        );
    }

    #[test]
    fn test_collect_read_regs_comprehensive() {
        // 测试各种操作类型的寄存器读取
        let ops = vec![
            IROp::Not { dst: 1, src: 2 },
            IROp::Load {
                dst: 3,
                base: 4,
                offset: 0,
                size: 8,
                flags: Default::default(),
            },
            IROp::Store {
                src: 5,
                base: 6,
                offset: 0,
                size: 8,
                flags: Default::default(),
            },
            IROp::CmpEq {
                dst: 7,
                lhs: 8,
                rhs: 9,
            },
        ];

        assert_eq!(IrAnalyzer::collect_read_regs(&ops[0]), vec![2]);
        assert_eq!(IrAnalyzer::collect_read_regs(&ops[1]), vec![4]);
        assert_eq!(IrAnalyzer::collect_read_regs(&ops[2]), vec![5, 6]);
        assert_eq!(IrAnalyzer::collect_read_regs(&ops[3]), vec![8, 9]);
    }

    #[test]
    fn test_collect_written_regs_comprehensive() {
        // 测试各种操作类型的寄存器写入
        let ops = vec![
            IROp::Not { dst: 1, src: 2 },
            IROp::Load {
                dst: 3,
                base: 4,
                offset: 0,
                size: 8,
                flags: Default::default(),
            },
            IROp::CmpEq {
                dst: 7,
                lhs: 8,
                rhs: 9,
            },
        ];

        assert_eq!(IrAnalyzer::collect_written_regs(&ops[0]), vec![1]);
        assert_eq!(IrAnalyzer::collect_written_regs(&ops[1]), vec![3]);
        assert_eq!(IrAnalyzer::collect_written_regs(&ops[2]), vec![7]);
    }
}

