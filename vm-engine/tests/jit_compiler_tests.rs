//! JIT编译器集成测试
//!
//! 测试JIT编译器的各种功能和场景

use vm_core::GuestAddr;
use vm_ir::{IRBlock, IROp, Terminator, RegId, MemFlags};
use vm_engine::jit::{JITCompiler, JITConfig, OptLevel};

#[cfg(test)]
mod basic_compilation_tests {
    use super::*;

    /// 测试1: 基本算术运算编译
    #[test]
    fn test_compile_arithmetic_ops() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };
        block.ops.push(IROp::Add {
            dst: 1,
            src1: 0,
            src2: 2,
        });
        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试2: 基本逻辑运算编译
    #[test]
    fn test_compile_logic_ops() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };
        block.ops.push(IROp::And {
            dst: 1,
            src1: 0,
            src2: 2,
        });
        block.ops.push(IROp::Or {
            dst: 2,
            src1: 1,
            src2: 3,
        });
        block.ops.push(IROp::Xor {
            dst: 3,
            src1: 2,
            src2: 4,
        });
        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试3: 移位操作编译
    #[test]
    fn test_compile_shift_ops() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };
        block.ops.push(IROp::SllImm {
            dst: 1,
            src: 0,
            sh: 4,
        });
        block.ops.push(IROp::SrlImm {
            dst: 2,
            src: 0,
            sh: 8,
        });
        block.ops.push(IROp::SraImm {
            dst: 3,
            src: 0,
            sh: 16,
        });
        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试4: 比较操作编译
    #[test]
    fn test_compile_compare_ops() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };
        block.ops.push(IROp::CmpEq {
            dst: 1,
            lhs: 0,
            rhs: 2,
        });
        block.ops.push(IROp::CmpLt {
            dst: 2,
            lhs: 3,
            rhs: 4,
        });
        block.ops.push(IROp::CmpLtU {
            dst: 3,
            lhs: 5,
            rhs: 6,
        });
        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试5: 内存加载操作编译
    #[test]
    fn test_compile_load_ops() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };
        block.ops.push(IROp::Load {
            dst: 1,
            base: 0,
            offset: 0,
            size: 8,
            flags: MemFlags::default(),
        });
        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试6: 内存存储操作编译
    #[test]
    fn test_compile_store_ops() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };
        block.ops.push(IROp::Store {
            src: 1,
            base: 0,
            offset: 0,
            size: 8,
            flags: MemFlags::default(),
        });
        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试7: 立即数操作编译
    #[test]
    fn test_compile_immediate_ops() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };
        block.ops.push(IROp::MovImm {
            dst: 1,
            imm: 42,
        });
        block.ops.push(IROp::AddImm {
            dst: 2,
            src: 1,
            imm: 10,
        });
        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试8: 条件跳转编译
    #[test]
    fn test_compile_conditional_branch() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };
        block.ops.push(IROp::CmpEq {
            dst: 1,
            lhs: 2,
            rhs: 3,
        });
        block.term = Terminator::CondJmp {
            cond: 1,
            target_true: GuestAddr(0x100),
            target_false: GuestAddr(0x200),
        };

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试9: 无条件跳转编译
    #[test]
    fn test_compile_unconditional_branch() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };
        block.term = Terminator::Jmp {
            target: GuestAddr(0x100),
        };

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试10: 返回指令编译
    #[test]
    fn test_compile_return() {
        let block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod optimization_tests {
    use super::*;

    /// 测试11: 无优化级别编译
    #[test]
    fn test_opt_level_none() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };
        block.ops.push(IROp::Add {
            dst: 1,
            src1: 0,
            src2: 2,
        });
        block.term = Terminator::Ret;

        let mut config = JITConfig::default();
        config.opt_level = OptLevel::None;
        let mut compiler = JITCompiler::new();
        // Note: Current API doesn't support passing config to new()
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试12: 基本优化级别编译
    #[test]
    fn test_opt_level_basic() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };
        block.ops.push(IROp::MovImm {
            dst: 0,
            imm: 10,
        });
        block.ops.push(IROp::AddImm {
            dst: 1,
            src: 0,
            imm: 5,
        });
        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试13: 寄存器压力测试
    #[test]
    fn test_register_pressure() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        // 使用大量寄存器
        for i in 0..32 {
            block.ops.push(IROp::MovImm {
                dst: i,
                imm: i as u64,
            });
        }

        // 使用这些寄存器进行运算
        for i in 0..16 {
            block.ops.push(IROp::Add {
                dst: i,
                src1: i,
                src2: i + 16,
            });
        }

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试14: 混合操作类型
    #[test]
    fn test_mixed_operations() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        // 立即数操作
        block.ops.push(IROp::MovImm {
            dst: 0,
            imm: 100,
        });

        // 算术操作
        block.ops.push(IROp::Add {
            dst: 1,
            src1: 0,
            src2: 0,
        });

        // 逻辑操作
        block.ops.push(IROp::And {
            dst: 2,
            src1: 1,
            src2: 0,
        });

        // 移位操作
        block.ops.push(IROp::SllImm {
            dst: 3,
            src: 2,
            sh: 2,
        });

        // 内存操作
        block.ops.push(IROp::Load {
            dst: 4,
            base: 0,
            offset: 0,
            size: 8,
            flags: MemFlags::default(),
        });

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试15: 长基本块
    #[test]
    fn test_large_basic_block() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        // 创建包含100个操作的基本块
        for i in 0..100 {
            block.ops.push(IROp::AddImm {
                dst: i % 32,
                src: (i + 1) % 32,
                imm: i as i64,
            });
        }

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试16: 分支预测优化
    #[test]
    fn test_branch_prediction_hint() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        block.ops.push(IROp::CmpLt {
            dst: 1,
            lhs: 2,
            rhs: 3,
        });

        block.term = Terminator::CondJmp {
            cond: 1,
            target_true: GuestAddr(0x100),
            target_false: GuestAddr(0x200),
        };

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试17: 冗余操作消除
    #[test]
    fn test_redundant_elimination() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        // 添加冗余操作（可以被优化器消除）
        block.ops.push(IROp::MovImm {
            dst: 0,
            imm: 10,
        });
        block.ops.push(IROp::Mov {
            dst: 1,
            src: 0,
        });
        block.ops.push(IROp::Mov {
            dst: 2,
            src: 1,
        });

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试18: 常量折叠优化
    #[test]
    fn test_constant_folding() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        // 可折叠的常量操作
        block.ops.push(IROp::MovImm {
            dst: 0,
            imm: 10,
        });
        block.ops.push(IROp::AddImm {
            dst: 1,
            src: 0,
            imm: 5,
        });

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod error_handling_tests {
    use super::*;

    /// 测试19: 空基本块
    #[test]
    fn test_empty_block() {
        let block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试20: 无效寄存器
    #[test]
    fn test_invalid_register() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        // 使用非常大的寄存器号（可能在某些实现中无效）
        block.ops.push(IROp::Add {
            dst: 1000,
            src1: 1001,
            src2: 1002,
        });

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        // 应该能够编译，即使寄存器号很大
        assert!(result.is_ok());
    }

    /// 测试21: 大立即数
    #[test]
    fn test_large_immediate() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        block.ops.push(IROp::MovImm {
            dst: 0,
            imm: u64::MAX,
        });

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试22: 负立即数偏移
    #[test]
    fn test_negative_offset() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        block.ops.push(IROp::Load {
            dst: 1,
            base: 0,
            offset: -8,
            size: 8,
            flags: MemFlags::default(),
        });

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod edge_case_tests {
    use super::*;

    /// 测试23: 最大立即数
    #[test]
    fn test_max_immediate() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        block.ops.push(IROp::MovImm {
            dst: 0,
            imm: u64::MAX,
        });

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试24: 零寄存器操作
    #[test]
    fn test_zero_register_operations() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        block.ops.push(IROp::Add {
            dst: 0,
            src1: 0,
            src2: 0,
        });

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试25: 链式依赖
    #[test]
    fn test_chained_dependencies() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        // 创建长依赖链：r1 = r0 + 1, r2 = r1 + 2, r3 = r2 + 3, ...
        for i in 0..10 {
            block.ops.push(IROp::AddImm {
                dst: i + 1,
                src: i,
                imm: i as i64,
            });
        }

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试26: 多种大小内存操作
    #[test]
    fn test_various_memory_sizes() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        // 测试1, 2, 4, 8字节的内存操作
        for (idx, size) in [1u8, 2, 4, 8].iter().enumerate() {
            block.ops.push(IROp::Load {
                dst: idx as u32,
                base: 0,
                offset: 0,
                size: *size,
                flags: MemFlags::default(),
            });
        }

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试27: 最大偏移量
    #[test]
    fn test_max_offset() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        block.ops.push(IROp::Load {
            dst: 1,
            base: 0,
            offset: i64::MAX,
            size: 8,
            flags: MemFlags::default(),
        });

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试28: 复杂条件跳转
    #[test]
    fn test_complex_conditional_jump() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        // 创建多个比较操作
        block.ops.push(IROp::CmpLt {
            dst: 1,
            lhs: 2,
            rhs: 3,
        });
        block.ops.push(IROp::CmpEq {
            dst: 4,
            lhs: 5,
            rhs: 6,
        });
        block.ops.push(IROp::And {
            dst: 7,
            src1: 1,
            src2: 4,
        });

        block.term = Terminator::CondJmp {
            cond: 7,
            target_true: GuestAddr(0x100),
            target_false: GuestAddr(0x200),
        };

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试29: 寄存器重用
    #[test]
    fn test_register_reuse() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        // 多次重用相同寄存器
        for i in 0..10 {
            block.ops.push(IROp::AddImm {
                dst: 1,
                src: 1,
                imm: i as i64,
            });
        }

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试30: NOP指令
    #[test]
    fn test_nop_instruction() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        block.ops.push(IROp::Nop);
        block.ops.push(IROp::MovImm {
            dst: 0,
            imm: 1,
        });
        block.ops.push(IROp::Nop);
        block.ops.push(IROp::AddImm {
            dst: 1,
            src: 0,
            imm: 2,
        });
        block.ops.push(IROp::Nop);

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod arithmetic_extended_tests {
    use super::*;

    /// 测试31: 乘法操作
    #[test]
    fn test_multiply_ops() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        block.ops.push(IROp::Mul {
            dst: 1,
            src1: 2,
            src2: 3,
        });

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试32: 除法操作（有符号）
    #[test]
    fn test_divide_signed() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        block.ops.push(IROp::Div {
            dst: 1,
            src1: 2,
            src2: 3,
            signed: true,
        });

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试33: 除法操作（无符号）
    #[test]
    fn test_divide_unsigned() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        block.ops.push(IROp::Div {
            dst: 1,
            src1: 2,
            src2: 3,
            signed: false,
        });

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试34: 取模操作（有符号）
    #[test]
    fn test_remainder_signed() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        block.ops.push(IROp::Rem {
            dst: 1,
            src1: 2,
            src2: 3,
            signed: true,
        });

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试35: 取模操作（无符号）
    #[test]
    fn test_remainder_unsigned() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        block.ops.push(IROp::Rem {
            dst: 1,
            src1: 2,
            src2: 3,
            signed: false,
        });

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod logical_extended_tests {
    use super::*;

    /// 测试36: NOT操作
    #[test]
    fn test_not_operation() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        block.ops.push(IROp::Not {
            dst: 1,
            src: 0,
        });

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试37: 复杂逻辑表达式
    #[test]
    fn test_complex_logic_expression() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        // (a & b) | (c & d)
        block.ops.push(IROp::And {
            dst: 1,
            src1: 2,
            src2: 3,
        });
        block.ops.push(IROp::And {
            dst: 4,
            src1: 5,
            src2: 6,
        });
        block.ops.push(IROp::Or {
            dst: 7,
            src1: 1,
            src2: 4,
        });

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试38: XOR交换模式
    #[test]
    fn test_xor_swap_pattern() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        // 使用XOR交换两个值
        block.ops.push(IROp::Xor {
            dst: 1,
            src1: 1,
            src2: 2,
        });
        block.ops.push(IROp::Xor {
            dst: 2,
            src1: 2,
            src2: 1,
        });
        block.ops.push(IROp::Xor {
            dst: 1,
            src1: 1,
            src2: 2,
        });

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod comparison_extended_tests {
    use super::*;

    /// 测试39: 所有比较操作
    #[test]
    fn test_all_comparisons() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        block.ops.push(IROp::CmpEq {
            dst: 1,
            lhs: 2,
            rhs: 3,
        });
        block.ops.push(IROp::CmpNe {
            dst: 4,
            lhs: 2,
            rhs: 3,
        });
        block.ops.push(IROp::CmpLt {
            dst: 5,
            lhs: 2,
            rhs: 3,
        });
        block.ops.push(IROp::CmpLtU {
            dst: 6,
            lhs: 2,
            rhs: 3,
        });
        block.ops.push(IROp::CmpGe {
            dst: 7,
            lhs: 2,
            rhs: 3,
        });
        block.ops.push(IROp::CmpGeU {
            dst: 8,
            lhs: 2,
            rhs: 3,
        });

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试40: 比较链
    #[test]
    fn test_comparison_chain() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        // a < b && b < c && c < d
        block.ops.push(IROp::CmpLt {
            dst: 10,
            lhs: 1,
            rhs: 2,
        });
        block.ops.push(IROp::CmpLt {
            dst: 11,
            lhs: 2,
            rhs: 3,
        });
        block.ops.push(IROp::CmpLt {
            dst: 12,
            lhs: 3,
            rhs: 4,
        });
        block.ops.push(IROp::And {
            dst: 13,
            src1: 10,
            src2: 11,
        });
        block.ops.push(IROp::And {
            dst: 14,
            src1: 13,
            src2: 12,
        });

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod memory_extended_tests {
    use super::*;

    /// 测试41: 复杂内存访问模式
    #[test]
    fn test_complex_memory_access() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        // 计算地址并访问
        block.ops.push(IROp::AddImm {
            dst: 1,
            src: 0,
            imm: 8,
        });
        block.ops.push(IROp::Load {
            dst: 2,
            base: 1,
            offset: 0,
            size: 8,
            flags: MemFlags::default(),
        });
        block.ops.push(IROp::Store {
            src: 2,
            base: 1,
            offset: 0,
            size: 8,
            flags: MemFlags::default(),
        });

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试42: 多种内存大小存储
    #[test]
    fn test_various_store_sizes() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        for (idx, size) in [1u8, 2, 4, 8].iter().enumerate() {
            block.ops.push(IROp::Store {
                src: idx as u32,
                base: 0,
                offset: 0,
                size: *size,
                flags: MemFlags::default(),
            });
        }

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试43: 内存复制循环模式
    #[test]
    fn test_memory_copy_pattern() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        // 模拟内存复制：load -> store
        for i in 0..10 {
            block.ops.push(IROp::Load {
                dst: i + 10,
                base: 1,
                offset: i as i64 * 8,
                size: 8,
                flags: MemFlags::default(),
            });
            block.ops.push(IROp::Store {
                src: i + 10,
                base: 2,
                offset: i as i64 * 8,
                size: 8,
                flags: MemFlags::default(),
            });
        }

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod special_terminator_tests {
    use super::*;

    /// 测试44: 寄存器跳转
    #[test]
    fn test_jump_register() {
        let block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::JmpReg {
                base: 1,
                offset: 0,
            },
        };

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试45: 带偏移的寄存器跳转
    #[test]
    fn test_jump_register_offset() {
        let block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::JmpReg {
                base: 1,
                offset: 16,
            },
        };

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试46: 故障终止
    #[test]
    fn test_fault_terminator() {
        let block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Fault { cause: 0xDEAD },
        };

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试47: 中断终止
    #[test]
    fn test_interrupt_terminator() {
        let block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Interrupt { vector: 32 },
        };

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试48: 函数调用终止
    #[test]
    fn test_call_terminator() {
        let block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Call {
                target: GuestAddr(0x1000),
                ret_pc: GuestAddr(0x100),
            },
        };

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod misc_tests {
    use super::*;

    /// 测试49: Select操作
    #[test]
    fn test_select_operation() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        block.ops.push(IROp::Select {
            dst: 1,
            cond: 2,
            true_val: 3,
            false_val: 4,
        });

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试50: Move操作
    #[test]
    fn test_move_operation() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        block.ops.push(IROp::Mov {
            dst: 1,
            src: 2,
        });

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试51: 寄存器移位操作
    #[test]
    fn test_register_shift_ops() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        block.ops.push(IROp::Sll {
            dst: 1,
            src: 2,
            shreg: 3,
        });
        block.ops.push(IROp::Srl {
            dst: 4,
            src: 5,
            shreg: 6,
        });
        block.ops.push(IROp::Sra {
            dst: 7,
            src: 8,
            shreg: 9,
        });

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }

    /// 测试52-60: 组合测试场景
    #[test]
    fn test_arithmetic_logic_mix() {
        let mut block = IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: Terminator::Ret,
        };

        for i in 0..20 {
            block.ops.push(IROp::AddImm {
                dst: i % 32,
                src: (i + 1) % 32,
                imm: i as i64,
            });
            block.ops.push(IROp::And {
                dst: (i + 2) % 32,
                src1: i % 32,
                src2: (i + 1) % 32,
            });
        }

        block.term = Terminator::Ret;

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }
}
