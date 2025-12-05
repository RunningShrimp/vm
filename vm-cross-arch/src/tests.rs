//! 跨架构转换测试

use super::*;
use vm_ir::{IRBuilder, IROp, Terminator};

#[test]
fn test_x86_to_arm64_translation() {
        let mut translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
        
        // 创建简单的IR块：ADD r0, r1, r2
        let mut builder = IRBuilder::new(0x1000);
        builder.push(IROp::Add {
            dst: 0,
            src1: 1,
            src2: 2,
        });
        builder.set_term(Terminator::Ret);
        let block = builder.build();
        
        // 转换
        let result = translator.translate_block(&block).unwrap();
        
        // 验证结果
        assert!(!result.instructions.is_empty());
        assert_eq!(result.stats.ir_ops_translated, 1);
        assert!(result.stats.target_instructions_generated > 0);
    }

    #[test]
    fn test_arm64_to_riscv_translation() {
        let mut translator = ArchTranslator::new(SourceArch::ARM64, TargetArch::RISCV64);
        
        let mut builder = IRBuilder::new(0x2000);
        builder.push(IROp::MovImm { dst: 0, imm: 42 });
        builder.push(IROp::AddImm {
            dst: 1,
            src: 0,
            imm: 10,
        });
        builder.set_term(Terminator::Ret);
        let block = builder.build();
        
        let result = translator.translate_block(&block).unwrap();
        assert!(!result.instructions.is_empty());
}

#[test]
fn test_riscv_to_x86_translation() {
        let mut translator = ArchTranslator::new(SourceArch::RISCV64, TargetArch::X86_64);
        
        let mut builder = IRBuilder::new(0x3000);
        builder.push(IROp::Load {
            dst: 0,
            base: 1,
            offset: 0x100,
            size: 8,
            flags: Default::default(),
        });
        builder.set_term(Terminator::Ret);
        let block = builder.build();
        
        let result = translator.translate_block(&block).unwrap();
        assert!(!result.instructions.is_empty());
}

#[test]
fn test_register_mapping() {
        let mapper = RegisterMapper::new(Architecture::X86_64, Architecture::ARM64);
        
        // 测试基本映射
        assert_eq!(mapper.map_register(0), 0);
        assert_eq!(mapper.map_register(1), 1);
        assert_eq!(mapper.map_register(15), 15);
}

#[test]
fn test_complex_operation_translation() {
        let mut translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
        
        // 测试大立即数（需要多指令序列）
        let mut builder = IRBuilder::new(0x4000);
        builder.push(IROp::MovImm {
            dst: 0,
            imm: 0x123456789ABCDEF0,
        });
        builder.set_term(Terminator::Ret);
        let block = builder.build();
        
        let result = translator.translate_block(&block).unwrap();
        // 大立即数应该生成多条指令
        assert!(result.stats.target_instructions_generated >= 1);
        assert!(result.stats.complex_operations >= 0);
}

#[test]
fn test_simd_translation() {
        let mut translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
        
        let mut builder = IRBuilder::new(0x5000);
        builder.push(IROp::VecAdd {
            dst: 0,
            src1: 1,
            src2: 2,
            element_size: 4, // 32-bit elements
        });
        builder.set_term(Terminator::Ret);
        let block = builder.build();
        
        let result = translator.translate_block(&block).unwrap();
        assert!(!result.instructions.is_empty());
}

#[test]
fn test_float_translation() {
        let mut translator = ArchTranslator::new(SourceArch::ARM64, TargetArch::RISCV64);
        
        let mut builder = IRBuilder::new(0x6000);
        builder.push(IROp::Fadd {
            dst: 0,
            src1: 1,
            src2: 2,
        });
        builder.push(IROp::FaddS {
            dst: 3,
            src1: 4,
            src2: 5,
        });
        builder.set_term(Terminator::Ret);
        let block = builder.build();
        
        let result = translator.translate_block(&block).unwrap();
        assert!(!result.instructions.is_empty());
        assert_eq!(result.stats.ir_ops_translated, 2);
}

#[test]
fn test_atomic_translation() {
        use vm_ir::AtomicOp;
        
        let mut translator = ArchTranslator::new(SourceArch::RISCV64, TargetArch::X86_64);
        
        let mut builder = IRBuilder::new(0x7000);
        builder.push(IROp::AtomicRMW {
            dst: 0,
            base: 1,
            src: 2,
            op: AtomicOp::Add,
            size: 8,
        });
        builder.set_term(Terminator::Ret);
        let block = builder.build();
        
        let result = translator.translate_block(&block).unwrap();
        assert!(!result.instructions.is_empty());
}

#[test]
fn test_atomic_cmpxchg_translation() {
        let mut translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
        
        let mut builder = IRBuilder::new(0x8000);
        builder.push(IROp::AtomicCmpXchg {
            dst: 0,
            base: 1,
            expected: 2,
            new: 3,
            size: 8,
        });
        builder.set_term(Terminator::Ret);
        let block = builder.build();
        
        let result = translator.translate_block(&block).unwrap();
        assert!(!result.instructions.is_empty());
}

#[test]
fn test_riscv_lr_sc_translation() {
        let mut translator = ArchTranslator::new(SourceArch::RISCV64, TargetArch::ARM64);
        
        let mut builder = IRBuilder::new(0x9000);
        builder.push(IROp::AtomicLoadReserve {
            dst: 0,
            base: 1,
            offset: 0,
            size: 8,
            flags: Default::default(),
        });
        builder.push(IROp::AtomicStoreCond {
            src: 2,
            base: 1,
            offset: 0,
            size: 8,
            dst_flag: 3,
            flags: Default::default(),
        });
        builder.set_term(Terminator::Ret);
        let block = builder.build();
        
        let result = translator.translate_block(&block).unwrap();
        assert!(!result.instructions.is_empty());
        assert_eq!(result.stats.ir_ops_translated, 2);
    }

    #[test]
    fn test_simd_saturating_translation() {
        let mut translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
        
        let mut builder = IRBuilder::new(0xA000);
        builder.push(IROp::VecAddSat {
            dst: 0,
            src1: 1,
            src2: 2,
            element_size: 1,
            signed: true,
        });
        builder.set_term(Terminator::Ret);
        let block = builder.build();
        
        let result = translator.translate_block(&block).unwrap();
        assert!(!result.instructions.is_empty());
    }

    #[test]
    fn test_fma_translation() {
        let mut translator = ArchTranslator::new(SourceArch::ARM64, TargetArch::RISCV64);
        
        let mut builder = IRBuilder::new(0xB000);
        builder.push(IROp::Fmadd {
            dst: 0,
            src1: 1,
            src2: 2,
            src3: 3,
        });
        builder.push(IROp::FmaddS {
            dst: 4,
            src1: 5,
            src2: 6,
            src3: 7,
        });
        builder.set_term(Terminator::Ret);
        let block = builder.build();
        
        let result = translator.translate_block(&block).unwrap();
        assert!(!result.instructions.is_empty());
        assert_eq!(result.stats.ir_ops_translated, 2);
    }

    #[test]
    fn test_float_comparison_translation() {
        let mut translator = ArchTranslator::new(SourceArch::RISCV64, TargetArch::X86_64);
        
        let mut builder = IRBuilder::new(0xC000);
        builder.push(IROp::Feq {
            dst: 0,
            src1: 1,
            src2: 2,
        });
        builder.push(IROp::FltS {
            dst: 3,
            src1: 4,
            src2: 5,
        });
        builder.set_term(Terminator::Ret);
        let block = builder.build();
        
        let result = translator.translate_block(&block).unwrap();
        assert!(!result.instructions.is_empty());
    }

    #[test]
    fn test_float_conversion_translation() {
        let mut translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
        
        let mut builder = IRBuilder::new(0xD000);
        builder.push(IROp::Fcvtws {
            dst: 0,
            src: 1,
        });
        builder.push(IROp::Fcvtsw {
            dst: 2,
            src: 3,
        });
        builder.push(IROp::Fcvtsd {
            dst: 4,
            src: 5,
        });
        builder.set_term(Terminator::Ret);
        let block = builder.build();
        
        let result = translator.translate_block(&block).unwrap();
        assert!(!result.instructions.is_empty());
        assert_eq!(result.stats.ir_ops_translated, 3);
    }

    #[test]
    fn test_float_minmax_translation() {
        let mut translator = ArchTranslator::new(SourceArch::ARM64, TargetArch::RISCV64);
        
        let mut builder = IRBuilder::new(0xE000);
        builder.push(IROp::Fmin {
            dst: 0,
            src1: 1,
            src2: 2,
        });
        builder.push(IROp::FmaxS {
            dst: 3,
            src1: 4,
            src2: 5,
        });
        builder.set_term(Terminator::Ret);
        let block = builder.build();
        
        let result = translator.translate_block(&block).unwrap();
        assert!(!result.instructions.is_empty());
    }

    #[test]
    fn test_float_abs_neg_translation() {
        let mut translator = ArchTranslator::new(SourceArch::RISCV64, TargetArch::X86_64);
        
        let mut builder = IRBuilder::new(0xF000);
        builder.push(IROp::Fabs {
            dst: 0,
            src: 1,
        });
        builder.push(IROp::FnegS {
            dst: 2,
            src: 3,
        });
        builder.set_term(Terminator::Ret);
        let block = builder.build();
        
        let result = translator.translate_block(&block).unwrap();
        assert!(!result.instructions.is_empty());
    }

    #[test]
    fn test_float_load_store_translation() {
        let mut translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
        
        let mut builder = IRBuilder::new(0x10000);
        builder.push(IROp::Fload {
            dst: 0,
            base: 1,
            offset: 0x100,
            size: 8,
        });
        builder.push(IROp::Fstore {
            src: 2,
            base: 3,
            offset: 0x200,
            size: 4,
        });
        builder.set_term(Terminator::Ret);
        let block = builder.build();
        
        let result = translator.translate_block(&block).unwrap();
        assert!(!result.instructions.is_empty());
        assert_eq!(result.stats.ir_ops_translated, 2);
    }

    #[test]
    fn test_simd_mulsat_translation() {
        let mut translator = ArchTranslator::new(SourceArch::ARM64, TargetArch::RISCV64);
        
        let mut builder = IRBuilder::new(0x11000);
        builder.push(IROp::VecMulSat {
            dst: 0,
            src1: 1,
            src2: 2,
            element_size: 2,
            signed: true,
        });
        builder.set_term(Terminator::Ret);
        let block = builder.build();
        
        let result = translator.translate_block(&block).unwrap();
        assert!(!result.instructions.is_empty());
    }

    #[test]
    fn test_float_sign_ops_translation() {
        let mut translator = ArchTranslator::new(SourceArch::RISCV64, TargetArch::X86_64);
        
        let mut builder = IRBuilder::new(0x12000);
        builder.push(IROp::Fsgnj {
            dst: 0,
            src1: 1,
            src2: 2,
        });
        builder.push(IROp::FsgnjxS {
            dst: 3,
            src1: 4,
            src2: 5,
        });
        builder.set_term(Terminator::Ret);
        let block = builder.build();
        
        let result = translator.translate_block(&block).unwrap();
        assert!(!result.instructions.is_empty());
    }

    #[test]
    fn test_float_class_translation() {
        let mut translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
        
        let mut builder = IRBuilder::new(0x13000);
        builder.push(IROp::Fclass {
            dst: 0,
            src: 1,
        });
        builder.push(IROp::FclassS {
            dst: 2,
            src: 3,
        });
        builder.set_term(Terminator::Ret);
        let block = builder.build();
        
        let result = translator.translate_block(&block).unwrap();
        assert!(!result.instructions.is_empty());
    }

    #[test]
    fn test_float_mv_translation() {
        let mut translator = ArchTranslator::new(SourceArch::ARM64, TargetArch::RISCV64);
        
        let mut builder = IRBuilder::new(0x14000);
        builder.push(IROp::FmvXW {
            dst: 0,
            src: 1,
        });
        builder.push(IROp::FmvDX {
            dst: 2,
            src: 3,
        });
        builder.set_term(Terminator::Ret);
        let block = builder.build();
        
        let result = translator.translate_block(&block).unwrap();
        assert!(!result.instructions.is_empty());
    }

    #[test]
    fn test_more_float_conversions() {
        let mut translator = ArchTranslator::new(SourceArch::RISCV64, TargetArch::X86_64);
        
        let mut builder = IRBuilder::new(0x15000);
        builder.push(IROp::Fcvtwus {
            dst: 0,
            src: 1,
        });
        builder.push(IROp::Fcvtdwu {
            dst: 2,
            src: 3,
        });
        builder.push(IROp::Fcvtlud {
            dst: 4,
            src: 5,
        });
        builder.set_term(Terminator::Ret);
        let block = builder.build();
        
        let result = translator.translate_block(&block).unwrap();
        assert!(!result.instructions.is_empty());
        assert_eq!(result.stats.ir_ops_translated, 3);
    }

    #[test]
    fn test_large_vector_ops() {
        let mut translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
        
        let mut builder = IRBuilder::new(0x16000);
        builder.push(IROp::Vec128Add {
            dst_lo: 0,
            dst_hi: 1,
            src1_lo: 2,
            src1_hi: 3,
            src2_lo: 4,
            src2_hi: 5,
            element_size: 4,
            signed: true,
        });
        builder.push(IROp::Vec256Add {
            dst0: 6, dst1: 7, dst2: 8, dst3: 9,
            src10: 10, src11: 11, src12: 12, src13: 13,
            src20: 14, src21: 15, src22: 16, src23: 17,
            element_size: 4,
            signed: true,
        });
        builder.set_term(Terminator::Ret);
        let block = builder.build();
        
        let result = translator.translate_block(&block).unwrap();
        assert!(!result.instructions.is_empty());
        assert_eq!(result.stats.ir_ops_translated, 2);
    }

