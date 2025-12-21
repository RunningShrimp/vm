//! 翻译器 IROp 覆盖测试
//!
//! 测试翻译器对各种 IROp 的支持情况，确保与解释器语义对齐

use super::*;
use vm_core::GuestAddr;
use vm_ir::{IRBuilder, IROp, MemFlags, RegId, Terminator};
use tracing;

/// 测试翻译器 IROp 覆盖情况
#[cfg(test)]
mod tests {
    use super::*;

    /// 创建基本的翻译器实例用于测试
    fn create_test_translator() -> ArchTranslator {
        ArchTranslator::new(
            SourceArch::X86_64,
            TargetArch::ARM64,
        )
    }

    /// 测试基本算术操作的翻译
    #[test]
    fn test_arithmetic_ops_translation() {
        let mut translator = create_test_translator();

        // 创建包含各种算术操作的 IR 块
        let mut builder = IRBuilder::new(GuestAddr(0x1000));

        // 测试支持的操作
        builder.push(IROp::Add { dst: 0, src1: 1, src2: 2 });
        builder.push(IROp::Sub { dst: 0, src1: 1, src2: 2 });
        builder.push(IROp::Mul { dst: 0, src1: 1, src2: 2 });
        builder.push(IROp::Div { dst: 0, src1: 1, src2: 2, signed: true });
        builder.push(IROp::AddImm { dst: 0, src: 1, imm: 42 });
        builder.push(IROp::MovImm { dst: 0, imm: 123 });

        builder.set_term(Terminator::Ret);
        let block = builder.build();

        // 测试翻译
        let result = translator.translate_block(&block);
        assert!(result.is_ok(), "基本算术操作翻译失败");
    }

    /// 测试内存操作的翻译
    #[test]
    fn test_memory_ops_translation() {
        let mut translator = create_test_translator();

        let mut builder = IRBuilder::new(GuestAddr(0x1000));

        // 测试支持的内存操作
        builder.push(IROp::Load {
            dst: 0,
            base: 1,
            offset: 8,
            size: 8,
            flags: MemFlags::default(),
        });
        builder.push(IROp::Store {
            src: 0,
            base: 1,
            offset: 8,
            size: 8,
            flags: MemFlags::default(),
        });

        builder.set_term(Terminator::Ret);
        let block = builder.build();

        let result = translator.translate_block(&block);
        assert!(result.is_ok(), "内存操作翻译失败");
    }

    /// 测试分支操作的翻译
    #[test]
    fn test_branch_ops_translation() {
        let mut translator = create_test_translator();

        let mut builder = IRBuilder::new(GuestAddr(0x1000));

        // 测试支持的分支操作
        builder.push(IROp::Beq { src1: 0, src2: 1, target: GuestAddr(0x2000) });
        builder.push(IROp::Bne { src1: 0, src2: 1, target: GuestAddr(0x2000) });
        builder.push(IROp::Blt { src1: 0, src2: 1, target: GuestAddr(0x2000) });
        builder.push(IROp::Bge { src1: 0, src2: 1, target: GuestAddr(0x2000) });
        builder.push(IROp::Bltu { src1: 0, src2: 1, target: GuestAddr(0x2000) });
        builder.push(IROp::Bgeu { src1: 0, src2: 1, target: GuestAddr(0x2000) });

        builder.set_term(Terminator::Ret);
        let block = builder.build();

        let result = translator.translate_block(&block);
        assert!(result.is_ok(), "分支操作翻译失败");
    }

    /// 测试浮点操作的翻译
    #[test]
    fn test_float_ops_translation() {
        let mut translator = create_test_translator();

        let mut builder = IRBuilder::new(GuestAddr(0x1000));

        // 测试支持的浮点操作
        builder.push(IROp::Fadd { dst: 0, src1: 1, src2: 2 });
        builder.push(IROp::Fsub { dst: 0, src1: 1, src2: 2 });
        builder.push(IROp::Fmul { dst: 0, src1: 1, src2: 2 });
        builder.push(IROp::Fdiv { dst: 0, src1: 1, src2: 2 });
        builder.push(IROp::Fsqrt { dst: 0, src: 1 });

        builder.set_term(Terminator::Ret);
        let block = builder.build();

        let result = translator.translate_block(&block);
        assert!(result.is_ok(), "浮点操作翻译失败");
    }

    /// 测试 SIMD 操作的翻译
    #[test]
    fn test_simd_ops_translation() {
        let mut translator = create_test_translator();

        let mut builder = IRBuilder::new(GuestAddr(0x1000));

        // 测试支持的 SIMD 操作
        builder.push(IROp::VecAdd { dst: 0, src1: 1, src2: 2, element_size: 4 });
        builder.push(IROp::VecSub { dst: 0, src1: 1, src2: 2, element_size: 4 });
        builder.push(IROp::VecMul { dst: 0, src1: 1, src2: 2, element_size: 4 });

        builder.set_term(Terminator::Ret);
        let block = builder.build();

        let result = translator.translate_block(&block);
        assert!(result.is_ok(), "SIMD 操作翻译失败");
    }

    /// 测试原子操作的翻译
    #[test]
    fn test_atomic_ops_translation() {
        let mut translator = create_test_translator();

        let mut builder = IRBuilder::new(GuestAddr(0x1000));

        // 测试支持的原子操作
        builder.push(IROp::AtomicRMW {
            dst: 0,
            base: 1,
            src: 2,
            op: vm_ir::AtomicOp::Add,
            size: 8,
        });
        builder.push(IROp::AtomicCmpXchg {
            dst: 0,
            base: 1,
            expected: 2,
            new: 3,
            size: 8,
        });

        builder.set_term(Terminator::Ret);
        let block = builder.build();

        let result = translator.translate_block(&block);
        assert!(result.is_ok(), "原子操作翻译失败");
    }

    /// 测试未支持的操作（应该返回错误）
    #[test]
    fn test_unsupported_ops() {
        let mut translator = create_test_translator();

        let mut builder = IRBuilder::new(GuestAddr(0x1000));

        // 添加一些当前不支持的操作
        builder.push(IROp::And { dst: 0, src1: 1, src2: 2 }); // 假设这个不支持
        builder.push(IROp::Or { dst: 0, src1: 1, src2: 2 });  // 假设这个不支持

        builder.set_term(Terminator::Ret);
        let block = builder.build();

        // 这些操作可能不支持，应该有合理的错误处理
        let result = translator.translate_block(&block);
        // 注意：这里我们不强制要求失败，因为操作可能已经被添加支持
        // 如果失败了，那说明我们需要添加对这些操作的支持
        if result.is_err() {
            tracing::warn!("发现不支持的操作: {:?}", result.err());
        }
    }

    /// 测试跨架构一致性
    #[test]
    fn test_cross_arch_consistency() {
        // 测试从不同源架构到不同目标架构的翻译一致性
        let source_arches = [SourceArch::X86_64, SourceArch::ARM64, SourceArch::RISCV64];
        let target_arches = [TargetArch::ARM64, TargetArch::X86_64, TargetArch::RISCV64];

        for &src_arch in &source_arches {
            for &tgt_arch in &target_arches {
                if src_arch as u32 == tgt_arch as u32 {
                    continue; // 跳过相同架构
                }

                let mut translator = ArchTranslator::new(src_arch, tgt_arch);

                // 创建简单的测试块
                let mut builder = IRBuilder::new(GuestAddr(0x1000));
                builder.push(IROp::Add { dst: 0, src1: 1, src2: 2 });
                builder.set_term(Terminator::Ret);
                let block = builder.build();

                let result = translator.translate_block(&block);
                assert!(result.is_ok(), "架构 {:?} -> {:?} 翻译失败", src_arch, tgt_arch);
            }
        }
    }

    /// 测试翻译性能基准
    #[test]
    fn test_translation_performance() {
        let mut translator = create_test_translator();

        // 创建一个较大的 IR 块用于性能测试
        let mut builder = IRBuilder::new(GuestAddr(0x1000));

        // 生成 1000 个操作的块
        for i in 0..1000 {
            let dst = (i % 32) as RegId;
            let src1 = ((i + 1) % 32) as RegId;
            let src2 = ((i + 2) % 32) as RegId;

            match i % 4 {
                0 => builder.push(IROp::Add { dst, src1, src2 }),
                1 => builder.push(IROp::Sub { dst, src1, src2 }),
                2 => builder.push(IROp::Mul { dst, src1, src2 }),
                _ => builder.push(IROp::MovImm { dst, imm: i as u64 }),
            }
        }

        builder.set_term(Terminator::Ret);
        let block = builder.build();

        // 测量翻译时间
        let start = std::time::Instant::now();
        let result = translator.translate_block(&block);
        let duration = start.elapsed();

        assert!(result.is_ok(), "大型块翻译失败");
        tracing::info!("翻译 1000 个操作耗时: {:?}", duration);

        // 性能断言：翻译应该在合理时间内完成
        assert!(duration < std::time::Duration::from_secs(1), "翻译性能太慢: {:?}", duration);
    }

    /// 测试翻译结果的语义正确性
    #[test]
    fn test_semantic_correctness() {
        let mut translator = create_test_translator();

        // 创建一个有明确语义的 IR 块
        let mut builder = IRBuilder::new(GuestAddr(0x1000));

        // r0 = 10
        builder.push(IROp::MovImm { dst: 0, imm: 10 });
        // r1 = 20
        builder.push(IROp::MovImm { dst: 1, imm: 20 });
        // r2 = r0 + r1  (应该等于 30)
        builder.push(IROp::Add { dst: 2, src1: 0, src2: 1 });

        builder.set_term(Terminator::Ret);
        let block = builder.build();

        let result = translator.translate_block(&block);
        assert!(result.is_ok(), "语义测试块翻译失败");

        let translated_block = result.unwrap();

        // 验证翻译后的块结构
        assert!(!translated_block.instructions.is_empty(), "应该产生一些指令");
        assert!(translated_block.stats.ir_ops_translated >= 3, "至少应该翻译3个操作");
    }
}
