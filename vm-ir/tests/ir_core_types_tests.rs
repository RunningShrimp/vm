//! vm-ir核心类型测试
//!
//! 测试IROp、Terminator、IRBlock、IRBuilder等核心类型

use vm_ir::DecodeCache;
use vm_ir::{AtomicOp, MemFlags, MemOrder};
use vm_ir::{GuestAddr, IRBlock, IRBuilder, IROp, Terminator};
use vm_ir::{Operand, RegisterFile, RegisterMode};

#[cfg(test)]
mod atomic_op_tests {
    use super::*;

    // Test 1: AtomicOp所有变体可以创建
    #[test]
    fn test_atomic_op_variants() {
        let ops = vec![
            AtomicOp::Add,
            AtomicOp::Sub,
            AtomicOp::And,
            AtomicOp::Or,
            AtomicOp::Xor,
            AtomicOp::Xchg,
            AtomicOp::CmpXchg,
            AtomicOp::Min,
            AtomicOp::Max,
            AtomicOp::MinS,
            AtomicOp::MaxS,
            AtomicOp::Minu,
            AtomicOp::Maxu,
        ];

        assert_eq!(ops.len(), 13);
    }

    // Test 2: AtomicOp相等性
    #[test]
    fn test_atomic_op_equality() {
        assert_eq!(AtomicOp::Add, AtomicOp::Add);
        assert_ne!(AtomicOp::Add, AtomicOp::Sub);
        assert_ne!(AtomicOp::CmpXchg, AtomicOp::Xchg);
    }

    // Test 3: AtomicOp Debug trait
    #[test]
    fn test_atomic_op_debug() {
        let op = AtomicOp::CmpXchg;
        let debug_str = format!("{:?}", op);

        assert!(debug_str.contains("CmpXchg"));
    }
}

#[cfg(test)]
mod mem_flags_tests {
    use super::*;

    // Test 4: MemFlags创建
    #[test]
    fn test_mem_flags_creation() {
        let flags = MemFlags {
            volatile: false,
            atomic: false,
            align: 0,
            fence_before: false,
            fence_after: false,
            order: MemOrder::default(),
        };

        assert!(!flags.volatile);
        assert!(!flags.atomic);
        assert_eq!(flags.align, 0);
    }

    // Test 5: MemFlags volatile设置
    #[test]
    fn test_mem_flags_volatile() {
        let flags = MemFlags {
            volatile: true,
            atomic: false,
            align: 0,
            fence_before: false,
            fence_after: false,
            order: MemOrder::default(),
        };

        assert!(flags.volatile);
    }

    // Test 6: MemFlags atomic设置
    #[test]
    fn test_mem_flags_atomic() {
        let flags = MemFlags {
            volatile: false,
            atomic: true,
            align: 0,
            fence_before: false,
            fence_after: false,
            order: MemOrder::default(),
        };

        assert!(flags.atomic);
    }

    // Test 7: MemFlags克隆
    #[test]
    fn test_mem_flags_clone() {
        let flags1 = MemFlags {
            volatile: true,
            atomic: false,
            align: 0,
            fence_before: false,
            fence_after: false,
            order: MemOrder::default(),
        };

        let flags2 = flags1.clone();
        assert!(flags2.volatile);
    }
}

#[cfg(test)]
mod mem_order_tests {
    use super::*;

    // Test 8: MemOrder所有变体
    #[test]
    fn test_mem_order_variants() {
        let orders = vec![
            MemOrder::None,
            MemOrder::Acquire,
            MemOrder::Release,
            MemOrder::AcqRel,
            MemOrder::SeqCst,
        ];

        assert_eq!(orders.len(), 5);
    }

    // Test 9: MemOrder相等性
    #[test]
    fn test_mem_order_equality() {
        assert_eq!(MemOrder::Acquire, MemOrder::Acquire);
        assert_ne!(MemOrder::Acquire, MemOrder::Release);
        assert_ne!(MemOrder::SeqCst, MemOrder::None);
    }

    // Test 10: MemOrder Debug trait
    #[test]
    fn test_mem_order_debug() {
        let order = MemOrder::SeqCst;
        let debug_str = format!("{:?}", order);

        assert!(debug_str.contains("SeqCst"));
    }

    // Test 11: MemOrder Default trait
    #[test]
    fn test_mem_order_default() {
        let order = MemOrder::default();
        assert_eq!(order, MemOrder::None);
    }
}

#[cfg(test)]
mod ir_op_tests {
    use super::*;

    // Test 12: IROp Nop创建
    #[test]
    fn test_ir_op_nop() {
        let op = IROp::Nop;

        match op {
            IROp::Nop => {}
            _ => panic!("Expected Nop"),
        }
    }

    // Test 13: IROp算术操作
    #[test]
    fn test_ir_op_arithmetic() {
        let add = IROp::Add {
            dst: 1,
            src1: 2,
            src2: 3,
        };
        let div = IROp::Div {
            dst: 1,
            src1: 2,
            src2: 3,
            signed: true,
        };
        let rem = IROp::Rem {
            dst: 1,
            src1: 2,
            src2: 3,
            signed: false,
        };

        match add {
            IROp::Add { dst, .. } => assert_eq!(dst, 1),
            _ => panic!("Expected Add"),
        }

        match div {
            IROp::Div { signed, .. } => assert!(signed),
            _ => panic!("Expected Div"),
        }

        match rem {
            IROp::Rem { signed, .. } => assert!(!signed),
            _ => panic!("Expected Rem"),
        }
    }

    // Test 14: IROp逻辑操作
    #[test]
    fn test_ir_op_logical() {
        let not = IROp::Not { dst: 1, src: 2 };

        match not {
            IROp::Not { dst, src } => {
                assert_eq!(dst, 1);
                assert_eq!(src, 2);
            }
            _ => panic!("Expected Not"),
        }
    }

    // Test 15: IROp移位操作
    #[test]
    fn test_ir_op_shifts() {
        let sll = IROp::Sll {
            dst: 1,
            src: 2,
            shreg: 3,
        };

        match sll {
            IROp::Sll { dst, src, shreg } => {
                assert_eq!(dst, 1);
                assert_eq!(src, 2);
                assert_eq!(shreg, 3);
            }
            _ => panic!("Expected Sll"),
        }
    }

    // Test 16: IROp立即数操作
    #[test]
    fn test_ir_op_immediates() {
        let add_imm = IROp::AddImm {
            dst: 1,
            src: 2,
            imm: 42,
        };
        let mov_imm = IROp::MovImm {
            dst: 1,
            imm: 0x1000,
        };

        match add_imm {
            IROp::AddImm { imm, .. } => {
                assert_eq!(imm, 42);
            }
            _ => panic!("Expected AddImm"),
        }

        match mov_imm {
            IROp::MovImm { imm, .. } => {
                assert_eq!(imm, 0x1000);
            }
            _ => panic!("Expected MovImm"),
        }
    }

    // Test 17: IROp比较操作
    #[test]
    fn test_ir_op_comparisons() {
        let cmp_eq = IROp::CmpEq {
            dst: 1,
            lhs: 2,
            rhs: 3,
        };

        match cmp_eq {
            IROp::CmpEq { dst, lhs, rhs } => {
                assert_eq!(dst, 1);
                assert_eq!(lhs, 2);
                assert_eq!(rhs, 3);
            }
            _ => panic!("Expected CmpEq"),
        }
    }

    // Test 18: IROp内存操作
    #[test]
    fn test_ir_op_memory() {
        let flags = MemFlags {
            volatile: false,
            atomic: false,
            align: 0,
            fence_before: false,
            fence_after: false,
            order: MemOrder::default(),
        };

        let load = IROp::Load {
            dst: 1,
            base: 2,
            offset: 0,
            size: 8,
            flags,
        };

        match load {
            IROp::Load {
                dst,
                base,
                offset,
                size,
                ..
            } => {
                assert_eq!(dst, 1);
                assert_eq!(base, 2);
                assert_eq!(offset, 0);
                assert_eq!(size, 8);
            }
            _ => panic!("Expected Load"),
        }
    }

    // Test 19: IROp原子操作
    #[test]
    fn test_ir_op_atomic() {
        let atomic_rmw = IROp::AtomicRMW {
            dst: 1,
            base: 2,
            src: 3,
            op: AtomicOp::Add,
            size: 8,
        };

        match atomic_rmw {
            IROp::AtomicRMW { op, .. } => {
                assert_eq!(op, AtomicOp::Add);
            }
            _ => panic!("Expected AtomicRMW"),
        }
    }

    // Test 20: IROp Select操作
    #[test]
    fn test_ir_op_select() {
        let select = IROp::Select {
            dst: 1,
            cond: 2,
            true_val: 3,
            false_val: 4,
        };

        match select {
            IROp::Select {
                dst,
                cond,
                true_val,
                false_val,
            } => {
                assert_eq!(dst, 1);
                assert_eq!(cond, 2);
                assert_eq!(true_val, 3);
                assert_eq!(false_val, 4);
            }
            _ => panic!("Expected Select"),
        }
    }

    // Test 21: IROp Debug trait
    #[test]
    fn test_ir_op_debug() {
        let op = IROp::Add {
            dst: 1,
            src1: 2,
            src2: 3,
        };
        let debug_str = format!("{:?}", op);

        assert!(debug_str.contains("Add"));
    }
}

#[cfg(test)]
mod terminator_tests {
    use super::*;

    // Test 22: Terminator所有变体
    #[test]
    fn test_terminator_variants() {
        let ret = Terminator::Ret;
        let jmp = Terminator::Jmp {
            target: GuestAddr(0x1000),
        };

        match ret {
            Terminator::Ret => {}
            _ => panic!("Expected Ret"),
        }

        match jmp {
            Terminator::Jmp { target } => assert_eq!(target, GuestAddr(0x1000)),
            _ => panic!("Expected Jmp"),
        }
    }

    // Test 23: Terminator相等性
    #[test]
    fn test_terminator_equality() {
        assert_eq!(Terminator::Ret, Terminator::Ret);
        assert_eq!(
            Terminator::Jmp {
                target: GuestAddr(0x1000)
            },
            Terminator::Jmp {
                target: GuestAddr(0x1000)
            }
        );
        assert_ne!(
            Terminator::Jmp {
                target: GuestAddr(0x1000)
            },
            Terminator::Jmp {
                target: GuestAddr(0x2000)
            }
        );
    }

    // Test 24: Terminator Debug trait
    #[test]
    fn test_terminator_debug() {
        let term = Terminator::Ret;
        let debug_str = format!("{:?}", term);

        assert!(debug_str.contains("Ret"));
    }
}

#[cfg(test)]
mod ir_block_tests {
    use super::*;

    // Test 25: IRBlock创建
    #[test]
    fn test_ir_block_creation() {
        let block = IRBlock::new(GuestAddr(0x1000));

        assert_eq!(block.start_pc, GuestAddr(0x1000));
        assert_eq!(block.op_count(), 0);
        assert!(block.is_empty());
    }

    // Test 26: IRBlock操作计数
    #[test]
    fn test_ir_block_op_count() {
        let mut builder = IRBuilder::new(GuestAddr(0x1000));
        builder.push(IROp::Nop);
        builder.push(IROp::Nop);
        builder.push(IROp::Nop);

        let block = builder.build();
        assert_eq!(block.op_count(), 3);
        assert!(!block.is_empty());
    }

    // Test 27: IRBlock验证
    #[test]
    fn test_ir_block_validate() {
        let mut builder = IRBuilder::new(GuestAddr(0x1000));
        builder.push(IROp::Add {
            dst: 1,
            src1: 2,
            src2: 3,
        });
        builder.set_term(Terminator::Ret);

        let block = builder.build();
        assert!(block.validate().is_ok());
    }

    // Test 28: IRBlock估计大小
    #[test]
    fn test_ir_block_estimated_size() {
        let mut builder = IRBuilder::new(GuestAddr(0x1000));
        builder.push(IROp::Add {
            dst: 1,
            src1: 2,
            src2: 3,
        });
        builder.push(IROp::Sub {
            dst: 1,
            src1: 2,
            src2: 3,
        });
        builder.set_term(Terminator::Ret);

        let block = builder.build();
        let size = block.estimated_size();

        assert!(size > 0);
    }

    // Test 29: IRBlock迭代器
    #[test]
    fn test_ir_block_iter_ops() {
        let mut builder = IRBuilder::new(GuestAddr(0x1000));
        builder.push(IROp::Nop);
        builder.push(IROp::Nop);
        builder.set_term(Terminator::Ret);

        let block = builder.build();
        let ops: Vec<_> = block.iter_ops().collect();

        assert_eq!(ops.len(), 2);
    }

    // Test 30: IRBlock start_pc字段
    #[test]
    fn test_ir_block_start_pc_field() {
        let block1 = IRBlock::new(GuestAddr(0x1000));
        let block2 = IRBlock::new(GuestAddr(0x2000));

        assert_eq!(block1.start_pc, GuestAddr(0x1000));
        assert_eq!(block2.start_pc, GuestAddr(0x2000));
    }
}

#[cfg(test)]
mod ir_builder_tests {
    use super::*;

    // Test 31: IRBuilder创建
    #[test]
    fn test_ir_builder_creation() {
        let builder = IRBuilder::new(GuestAddr(0x1000));

        assert_eq!(builder.pc(), GuestAddr(0x1000));
        assert_eq!(builder.op_count(), 0);
        assert!(builder.is_empty());
    }

    // Test 32: IRBuilder push操作
    #[test]
    fn test_ir_builder_push() {
        let mut builder = IRBuilder::new(GuestAddr(0x1000));
        builder.push(IROp::Nop);
        builder.push(IROp::MovImm { dst: 1, imm: 42 });

        assert_eq!(builder.op_count(), 2);
        assert!(!builder.is_empty());
    }

    // Test 33: IRBuilder push_all操作
    #[test]
    fn test_ir_builder_push_all() {
        let ops = vec![IROp::Nop, IROp::Nop, IROp::Nop];

        let mut builder = IRBuilder::new(GuestAddr(0x1000));
        builder.push_all(ops);

        assert_eq!(builder.op_count(), 3);
    }

    // Test 34: IRBuilder set_term操作
    #[test]
    fn test_ir_builder_set_term() {
        let mut builder = IRBuilder::new(GuestAddr(0x1000));
        builder.set_term(Terminator::Ret);

        let block = builder.build();
        assert_eq!(block.start_pc, GuestAddr(0x1000));
    }

    // Test 35: IRBuilder build操作
    #[test]
    fn test_ir_builder_build() {
        let mut builder = IRBuilder::new(GuestAddr(0x1000));
        builder.push(IROp::Add {
            dst: 1,
            src1: 2,
            src2: 3,
        });
        builder.set_term(Terminator::Ret);

        let block = builder.build();

        assert_eq!(block.op_count(), 1);
        assert_eq!(block.start_pc, GuestAddr(0x1000));
    }

    // Test 36: IRBuilder build_ref操作
    #[test]
    fn test_ir_builder_build_ref() {
        let mut builder = IRBuilder::new(GuestAddr(0x1000));
        builder.push(IROp::Nop);
        builder.set_term(Terminator::Ret);

        let block1 = builder.build_ref();
        let block2 = builder.build_ref();

        assert_eq!(block1.op_count(), block2.op_count());
        assert_eq!(block1.start_pc, block2.start_pc);
    }
}

#[cfg(test)]
mod register_file_tests {
    use super::*;

    // Test 37: RegisterFile创建
    #[test]
    fn test_register_file_creation() {
        let reg_file = RegisterFile::new(32, RegisterMode::Standard);

        // 应该成功创建
        let _ = reg_file;
    }

    // Test 38: RegisterFile读写
    #[test]
    fn test_register_file_read_write() {
        let mut reg_file = RegisterFile::new(32, RegisterMode::Standard);

        let reg_id = reg_file.write(0);
        let value = reg_file.read(0);

        assert_eq!(reg_id, value);
    }

    // Test 39: RegisterFile分配临时寄存器
    #[test]
    fn test_register_file_alloc_temp() {
        let mut reg_file = RegisterFile::new(32, RegisterMode::Standard);

        let temp1 = reg_file.alloc_temp();
        let temp2 = reg_file.alloc_temp();

        // 临时寄存器应该不同
        assert_ne!(temp1, temp2);
    }

    // Test 40: RegisterFile SSA模式
    #[test]
    fn test_register_file_ssa_mode() {
        let reg_file = RegisterFile::new(32, RegisterMode::SSA);

        // 应该成功创建
        let _ = reg_file;
    }
}

#[cfg(test)]
mod operand_tests {
    use super::*;

    // Test 41: Operand Register variant
    #[test]
    fn test_operand_register() {
        let reg = Operand::Register(5);

        match reg {
            Operand::Register(id) => assert_eq!(id, 5),
            _ => panic!("Expected Register"),
        }
    }

    // Test 42: Operand Immediate variant
    #[test]
    fn test_operand_immediate() {
        let imm = Operand::Immediate(42);

        match imm {
            Operand::Immediate(val) => assert_eq!(val, 42),
            _ => panic!("Expected Immediate"),
        }
    }

    // Test 43: Operand Memory variant
    #[test]
    fn test_operand_memory() {
        let mem = Operand::Memory {
            base: 1,
            offset: 8,
            size: 4,
        };

        match mem {
            Operand::Memory { base, offset, size } => {
                assert_eq!(base, 1);
                assert_eq!(offset, 8);
                assert_eq!(size, 4);
            }
            _ => panic!("Expected Memory"),
        }
    }

    // Test 44: Operand None variant
    #[test]
    fn test_operand_none() {
        let none = Operand::None;

        match none {
            Operand::None => {}
            _ => panic!("Expected None"),
        }
    }

    // Test 45: Operand相等性
    #[test]
    fn test_operand_equality() {
        assert_eq!(Operand::Register(1), Operand::Register(1));
        assert_eq!(Operand::Immediate(42), Operand::Immediate(42));
        assert_ne!(Operand::Register(1), Operand::Register(2));
    }
}

#[cfg(test)]
mod decode_cache_tests {
    use super::*;

    // Test 46: DecodeCache创建
    #[test]
    fn test_decode_cache_creation() {
        let cache = DecodeCache::new(256);

        // 应该成功创建
        let _ = cache;
    }

    // Test 47: DecodeCache插入和获取
    #[test]
    fn test_decode_cache_insert_get() {
        let mut cache = DecodeCache::new(256);
        let ops = vec![IROp::Nop, IROp::Nop];

        cache.insert(0x1000, 8, ops);

        let result = cache.get(0x1000, 8);
        assert!(result.is_some());
    }

    // Test 48: DecodeCache misses
    #[test]
    fn test_decode_cache_miss() {
        let mut cache = DecodeCache::new(256);

        let result = cache.get(0x2000, 4);
        assert!(result.is_none());
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    // Test 49: 完整的IR块构建
    #[test]
    fn test_complete_ir_block() {
        let mut builder = IRBuilder::new(GuestAddr(0x1000));

        // 构建一个简单的计算: (10 + 20) + 42
        builder.push(IROp::MovImm { dst: 1, imm: 10 });
        builder.push(IROp::MovImm { dst: 2, imm: 20 });
        builder.push(IROp::Add {
            dst: 3,
            src1: 1,
            src2: 2,
        });
        builder.push(IROp::AddImm {
            dst: 4,
            src: 3,
            imm: 42,
        });
        builder.set_term(Terminator::Ret);

        let block = builder.build();

        assert_eq!(block.op_count(), 4);
        assert!(block.validate().is_ok());
    }

    // Test 50: 内存操作块
    #[test]
    fn test_memory_operations_block() {
        let flags = MemFlags {
            volatile: false,
            atomic: false,
            align: 8,
            fence_before: false,
            fence_after: false,
            order: MemOrder::default(),
        };

        let mut builder = IRBuilder::new(GuestAddr(0x2000));
        builder.push(IROp::MovImm {
            dst: 1,
            imm: 0x1000,
        });
        builder.push(IROp::Load {
            dst: 2,
            base: 1,
            offset: 0,
            size: 8,
            flags,
        });
        builder.push(IROp::AddImm {
            dst: 2,
            src: 2,
            imm: 10,
        });
        builder.set_term(Terminator::Ret);

        let block = builder.build();

        assert_eq!(block.op_count(), 3);
    }

    // Test 51: 比较和分支
    #[test]
    fn test_comparison_and_branch() {
        let mut builder = IRBuilder::new(GuestAddr(0x3000));

        builder.push(IROp::MovImm { dst: 1, imm: 10 });
        builder.push(IROp::MovImm { dst: 2, imm: 20 });
        builder.push(IROp::CmpLt {
            dst: 3,
            lhs: 1,
            rhs: 2,
        });
        builder.push(IROp::Select {
            dst: 4,
            cond: 3,
            true_val: 1,
            false_val: 2,
        });
        builder.set_term(Terminator::Jmp {
            target: GuestAddr(0x4000),
        });

        let block = builder.build();

        assert_eq!(block.op_count(), 4);
    }

    // Test 52: 原子操作块
    #[test]
    fn test_atomic_operations_block() {
        let mut builder = IRBuilder::new(GuestAddr(0x4000));

        builder.push(IROp::MovImm {
            dst: 1,
            imm: 0x5000,
        });
        builder.push(IROp::MovImm { dst: 2, imm: 1 });
        builder.push(IROp::AtomicRMW {
            dst: 3,
            base: 1,
            src: 2,
            op: AtomicOp::Add,
            size: 8,
        });
        builder.set_term(Terminator::Ret);

        let block = builder.build();

        assert_eq!(block.op_count(), 3);
    }

    // Test 53: 复杂的位操作
    #[test]
    fn test_complex_bit_operations() {
        let mut builder = IRBuilder::new(GuestAddr(0x5000));

        builder.push(IROp::MovImm { dst: 1, imm: 0xFF });
        builder.push(IROp::MovImm { dst: 2, imm: 0x0F });
        builder.push(IROp::And {
            dst: 3,
            src1: 1,
            src2: 2,
        });
        builder.push(IROp::Or {
            dst: 4,
            src1: 3,
            src2: 2,
        });
        builder.push(IROp::Xor {
            dst: 5,
            src1: 4,
            src2: 2,
        });
        builder.push(IROp::Not { dst: 6, src: 5 });
        builder.set_term(Terminator::Ret);

        let block = builder.build();

        assert_eq!(block.op_count(), 6);
    }

    // Test 54: 移位操作
    #[test]
    fn test_shift_operations() {
        let mut builder = IRBuilder::new(GuestAddr(0x6000));

        builder.push(IROp::MovImm { dst: 1, imm: 0x100 });
        builder.push(IROp::MovImm { dst: 2, imm: 4 });
        builder.push(IROp::Sll {
            dst: 3,
            src: 1,
            shreg: 2,
        });
        builder.push(IROp::Srl {
            dst: 4,
            src: 3,
            shreg: 2,
        });
        builder.push(IROp::Sra {
            dst: 5,
            src: 4,
            shreg: 2,
        });
        builder.set_term(Terminator::Ret);

        let block = builder.build();

        assert_eq!(block.op_count(), 5);
    }

    // Test 55: 多个基本块连接
    #[test]
    fn test_multiple_blocks_linked() {
        // 块1
        let mut builder1 = IRBuilder::new(GuestAddr(0x1000));
        builder1.push(IROp::MovImm { dst: 1, imm: 10 });
        builder1.push(IROp::MovImm { dst: 2, imm: 20 });
        builder1.set_term(Terminator::Jmp {
            target: GuestAddr(0x2000),
        });

        let block1 = builder1.build();

        // 块2
        let mut builder2 = IRBuilder::new(GuestAddr(0x2000));
        builder2.push(IROp::Add {
            dst: 3,
            src1: 1,
            src2: 2,
        });
        builder2.set_term(Terminator::Ret);

        let block2 = builder2.build();

        assert_eq!(block1.start_pc, GuestAddr(0x1000));
        assert_eq!(block2.start_pc, GuestAddr(0x2000));
    }
}

#[cfg(test)]
mod edge_case_tests {
    use super::*;

    // Test 56: 空块
    #[test]
    fn test_empty_block() {
        let block = IRBlock::new(GuestAddr(0x1000));

        assert!(block.is_empty());
        assert_eq!(block.op_count(), 0);
        assert_eq!(block.start_pc, GuestAddr(0x1000));
    }

    // Test 57: 只有终止符的块
    #[test]
    fn test_block_with_only_terminator() {
        let mut builder = IRBuilder::new(GuestAddr(0x1000));
        builder.set_term(Terminator::Ret);

        let block = builder.build();

        assert_eq!(block.op_count(), 0);
        assert!(block.validate().is_ok());
    }

    // Test 58: 大立即数
    #[test]
    fn test_large_immediate() {
        let mut builder = IRBuilder::new(GuestAddr(0x1000));
        builder.push(IROp::MovImm {
            dst: 1,
            imm: u64::MAX,
        });
        builder.set_term(Terminator::Ret);

        let block = builder.build();

        assert_eq!(block.op_count(), 1);
    }

    // Test 59: 负立即数
    #[test]
    fn test_negative_immediate() {
        let mut builder = IRBuilder::new(GuestAddr(0x1000));
        builder.push(IROp::MovImm { dst: 1, imm: 42 });
        builder.push(IROp::AddImm {
            dst: 2,
            src: 1,
            imm: -42,
        });
        builder.set_term(Terminator::Ret);

        let block = builder.build();

        assert_eq!(block.op_count(), 2);
    }

    // Test 60: 最大对齐的内存操作
    #[test]
    fn test_max_aligned_memory_op() {
        let flags = MemFlags {
            volatile: false,
            atomic: false,
            align: 16,
            fence_before: false,
            fence_after: false,
            order: MemOrder::default(),
        };

        let mut builder = IRBuilder::new(GuestAddr(0x1000));
        builder.push(IROp::MovImm {
            dst: 2,
            imm: 0x1000,
        });
        builder.push(IROp::Load {
            dst: 1,
            base: 2,
            offset: 0,
            size: 16,
            flags,
        });
        builder.set_term(Terminator::Ret);

        let block = builder.build();

        assert_eq!(block.op_count(), 2);
    }
}
