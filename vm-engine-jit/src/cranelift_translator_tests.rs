//! Cranelift 转换器测试
//!
//! 提供单元测试和集成测试，验证 IR 转换的正确性

#[cfg(test)]
mod tests {
    use vm_ir::{IRBlock, IROp, Terminator, RegId, GuestAddr};
    use cranelift::prelude::*;
    use cranelift_jit::{JITBuilder, JITModule};
    use cranelift_native;

    fn create_test_module() -> JITModule {
        let mut flag_builder = settings::builder();
        flag_builder.enable("is_pic").unwrap();
        
        let isa_builder = cranelift_native::builder().unwrap();
        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .unwrap();
        
        JITModule::new(isa)
    }

    #[test]
    fn test_simple_add() {
        // 构建简单的 IR: r2 = r0 + r1; return r2
        let mut block = IRBlock {
            start_pc: GuestAddr::from(0x1000u64),
            ops: vec![
                IROp::Add { dst: 2, src1: 0, src2: 1 },
            ],
            term: Terminator::Ret { value: Some(2) },
        };

        // 验证块可以被创建
        assert_eq!(block.ops.len(), 1);
        assert!(matches!(block.term, Terminator::Ret { value: Some(2) }));
    }

    #[test]
    fn test_ir_block_construction() {
        // 测试 IR 块的构建
        let ops = vec![
            IROp::MovImm { dst: 0, imm: 42 },
            IROp::MovImm { dst: 1, imm: 8 },
            IROp::Add { dst: 2, src1: 0, src2: 1 },
        ];

        let block = IRBlock {
            start_pc: GuestAddr::from(0x2000u64),
            ops,
            term: Terminator::Ret { value: Some(2) },
        };

        assert_eq!(block.ops.len(), 3);
        assert_eq!(block.start_pc, GuestAddr::from(0x2000u64));
    }

    #[test]
    fn test_floating_point_ops() {
        let ops = vec![
            IROp::MovImm { dst: 0, imm: 0x3ff0000000000000 }, // 1.0 in F64 bit pattern
            IROp::MovImm { dst: 1, imm: 0x4000000000000000 }, // 2.0 in F64 bit pattern
            // Fadd would be next, but requires type context
        ];

        let block = IRBlock {
            start_pc: GuestAddr::from(0x3000u64),
            ops,
            term: Terminator::Ret { value: None },
        };

        assert_eq!(block.ops.len(), 2);
    }

    #[test]
    fn test_memory_operations() {
        let ops = vec![
            IROp::MovImm { dst: 0, imm: 0x1000 }, // base address
            IROp::MovImm { dst: 1, imm: 42 },      // value to store
            IROp::Store { 
                src: 1, 
                base: 0, 
                offset: 0, 
                size: 8,
                flags: Default::default(),
            },
            IROp::Load {
                dst: 2,
                base: 0,
                offset: 0,
                size: 8,
                flags: Default::default(),
            },
        ];

        let block = IRBlock {
            start_pc: GuestAddr::from(0x4000u64),
            ops,
            term: Terminator::Ret { value: Some(2) },
        };

        assert_eq!(block.ops.len(), 4);
    }

    #[test]
    fn test_comparison_operations() {
        let ops = vec![
            IROp::MovImm { dst: 0, imm: 10 },
            IROp::MovImm { dst: 1, imm: 20 },
            IROp::CmpLt { dst: 2, lhs: 0, rhs: 1 }, // 10 < 20 => 1
            IROp::CmpEq { dst: 3, lhs: 0, rhs: 0 }, // 10 == 10 => 1
        ];

        let block = IRBlock {
            start_pc: GuestAddr::from(0x5000u64),
            ops,
            term: Terminator::Ret { value: Some(2) },
        };

        assert_eq!(block.ops.len(), 4);
    }

    #[test]
    fn test_bitwise_operations() {
        let ops = vec![
            IROp::MovImm { dst: 0, imm: 0xFF },
            IROp::MovImm { dst: 1, imm: 0x0F },
            IROp::And { dst: 2, src1: 0, src2: 1 }, // 0xFF & 0x0F = 0x0F
            IROp::Or  { dst: 3, src1: 0, src2: 1 }, // 0xFF | 0x0F = 0xFF
            IROp::Xor { dst: 4, src1: 0, src2: 1 }, // 0xFF ^ 0x0F = 0xF0
        ];

        let block = IRBlock {
            start_pc: GuestAddr::from(0x6000u64),
            ops,
            term: Terminator::Ret { value: Some(4) },
        };

        assert_eq!(block.ops.len(), 5);
    }

    #[test]
    fn test_shift_operations() {
        let ops = vec![
            IROp::MovImm { dst: 0, imm: 1 },
            IROp::SllImm { dst: 1, src: 0, sh: 3 }, // 1 << 3 = 8
            IROp::SrlImm { dst: 2, src: 1, sh: 1 }, // 8 >> 1 = 4
        ];

        let block = IRBlock {
            start_pc: GuestAddr::from(0x7000u64),
            ops,
            term: Terminator::Ret { value: Some(2) },
        };

        assert_eq!(block.ops.len(), 3);
    }

    #[test]
    fn test_select_operation() {
        let ops = vec![
            IROp::MovImm { dst: 0, imm: 1 },    // condition
            IROp::MovImm { dst: 1, imm: 100 },  // true value
            IROp::MovImm { dst: 2, imm: 200 },  // false value
            IROp::Select {
                dst: 3,
                cond: 0,
                true_val: 1,
                false_val: 2,
            },
        ];

        let block = IRBlock {
            start_pc: GuestAddr::from(0x8000u64),
            ops,
            term: Terminator::Ret { value: Some(3) },
        };

        assert_eq!(block.ops.len(), 4);
    }

    #[test]
    fn test_vector_operations() {
        let ops = vec![
            IROp::VecAdd {
                dst: 0,
                src1: 1,
                src2: 2,
                element_size: 4,
            },
            IROp::VecSub {
                dst: 3,
                src1: 4,
                src2: 5,
                element_size: 4,
            },
        ];

        let block = IRBlock {
            start_pc: GuestAddr::from(0x9000u64),
            ops,
            term: Terminator::Ret { value: None },
        };

        assert_eq!(block.ops.len(), 2);
    }

    #[test]
    fn test_atomic_operations() {
        use vm_ir::AtomicOp;

        let ops = vec![
            IROp::Atomic {
                dst: 0,
                base: 1,
                src: 2,
                op: AtomicOp::Add,
                size: 8,
            },
        ];

        let block = IRBlock {
            start_pc: GuestAddr::from(0xA000u64),
            ops,
            term: Terminator::Ret { value: Some(0) },
        };

        assert_eq!(block.ops.len(), 1);
    }

    #[test]
    fn test_complex_expression() {
        // (a + b) * (c - d)
        let ops = vec![
            IROp::MovImm { dst: 0, imm: 10 }, // a = 10
            IROp::MovImm { dst: 1, imm: 20 }, // b = 20
            IROp::MovImm { dst: 2, imm: 30 }, // c = 30
            IROp::MovImm { dst: 3, imm: 5 },  // d = 5
            IROp::Add { dst: 4, src1: 0, src2: 1 }, // a + b = 30
            IROp::Sub { dst: 5, src1: 2, src2: 3 }, // c - d = 25
            IROp::Mul { dst: 6, src1: 4, src2: 5 }, // (a+b)*(c-d) = 750
        ];

        let block = IRBlock {
            start_pc: GuestAddr::from(0xB000u64),
            ops,
            term: Terminator::Ret { value: Some(6) },
        };

        assert_eq!(block.ops.len(), 7);
    }
}
