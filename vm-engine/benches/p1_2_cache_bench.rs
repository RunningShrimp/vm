//! P1-2 缓存性能基准测试
//!
//! 验证跨架构翻译缓存系统的性能效果

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use std::time::Duration;
use vm_core::GuestAddr;
use vm_cross_arch_support::{
    encoding_cache::{InstructionEncodingCache, Arch as CacheArch, Instruction, Operand},
    pattern_cache::{PatternMatchCache, Arch as PatternArch},
    translation_pipeline::{CrossArchTranslationPipeline, RegisterMappingCache},
};

/// 创建测试指令
fn create_test_instruction(arch: CacheArch, opcode: u32) -> Instruction {
    Instruction {
        arch,
        opcode,
        operands: vec![Operand::Register(1), Operand::Immediate(42)],
    }
}

/// 基准测试：编码缓存性能
fn bench_encoding_cache(c: &mut Criterion) {
    let mut cache = InstructionEncodingCache::new();

    let mut group = c.benchmark_group("encoding_cache");
    group.measurement_time(Duration::from_secs(10));

    // 预填充缓存
    for i in 0..100 {
        let insn = create_test_instruction(CacheArch::Riscv64, i);
        let _ = cache.encode_or_lookup(&insn);
    }

    // 测试缓存命中性能
    group.bench_function("cache_hit", |b| {
        let insn = create_test_instruction(CacheArch::Riscv64, 50);
        b.iter(|| {
            black_box(cache.encode_or_lookup(black_box(&insn)).unwrap())
        })
    });

    // 测试缓存未命中性能
    group.bench_function("cache_miss", |b| {
        b.iter(|| {
            let insn = create_test_instruction(CacheArch::Riscv64, 1000 + black_box(0));
            black_box(cache.encode_or_lookup(&insn).unwrap())
        })
    });

    group.finish();
}

/// 基准测试：模式匹配缓存性能
fn bench_pattern_cache(c: &mut Criterion) {
    let mut cache = PatternMatchCache::new(10_000);

    let mut group = c.benchmark_group("pattern_cache");
    group.measurement_time(Duration::from_secs(10));

    // 预填充RISC-V指令模式
    for i in 0..100 {
        let insn: u32 = 0x00000333 + i; // ADD指令变体
        let bytes = insn.to_le_bytes();
        cache.match_or_analyze(PatternArch::Riscv64, &bytes[..4]);
    }

    // 测试缓存命中性能
    group.bench_function("pattern_hit", |b| {
        let insn: u32 = 0x00000383; // LB加载指令
        let bytes = insn.to_le_bytes();
        b.iter(|| {
            black_box(cache.match_or_analyze(PatternArch::Riscv64, black_box(&bytes[..4])))
        })
    });

    // 测试RISC-V加载指令检测
    group.bench_function("riscv_load_detect", |b| {
        let insn: u32 = 0x00000303; // LB指令
        let bytes = insn.to_le_bytes();
        b.iter(|| {
            black_box(cache.match_or_analyze(PatternArch::Riscv64, black_box(&bytes[..4])))
        })
    });

    // 测试RISC-V存储指令检测
    group.bench_function("riscv_store_detect", |b| {
        let insn: u32 = 0x0010A023; // SB指令
        let bytes = insn.to_le_bytes();
        b.iter(|| {
            black_box(cache.match_or_analyze(PatternArch::Riscv64, black_box(&bytes[..4])))
        })
    });

    // 测试RISC-V分支指令检测
    group.bench_function("riscv_branch_detect", |b| {
        let insn: u32 = 0x00000063; // BEQ指令
        let bytes = insn.to_le_bytes();
        b.iter(|| {
            black_box(cache.match_or_analyze(PatternArch::Riscv64, black_box(&bytes[..4])))
        })
    });

    group.finish();
}

/// 基准测试：寄存器映射缓存性能
fn bench_register_mapping(c: &mut Criterion) {
    let mut cache = RegisterMappingCache::new();

    let mut group = c.benchmark_group("register_mapping");
    group.measurement_time(Duration::from_secs(10));

    // 测试x86_64 → RISC-V映射（预填充）
    group.bench_function("x86_to_riscv", |b| {
        b.iter(|| {
            black_box(cache.map_or_compute(
                CacheArch::X86_64,
                CacheArch::Riscv64,
                vm_cross_arch_support::translation_pipeline::RegId::X86(black_box(5)),
            ))
        })
    });

    // 测试ARM → RISC-V映射（预填充）
    group.bench_function("arm_to_riscv", |b| {
        b.iter(|| {
            black_box(cache.map_or_compute(
                CacheArch::ARM64,
                CacheArch::Riscv64,
                vm_cross_arch_support::translation_pipeline::RegId::Arm(black_box(10)),
            ))
        })
    });

    // 测试RISC-V → x86_64映射（预填充）
    group.bench_function("riscv_to_x86", |b| {
        b.iter(|| {
            black_box(cache.map_or_compute(
                CacheArch::Riscv64,
                CacheArch::X86_64,
                vm_cross_arch_support::translation_pipeline::RegId::Riscv(black_box(3)),
            ))
        })
    });

    group.finish();
}

/// 基准测试：完整翻译管线性能
fn bench_translation_pipeline(c: &mut Criterion) {
    let mut pipeline = CrossArchTranslationPipeline::new();

    let mut group = c.benchmark_group("translation_pipeline");
    group.measurement_time(Duration::from_secs(10));

    // 预热
    let warmup_insns = vec![
        create_test_instruction(CacheArch::Riscv64, 0x00000333),
        create_test_instruction(CacheArch::Riscv64, 0x00000303),
    ];
    pipeline.warmup(&warmup_insns);

    // 测试单条指令翻译
    group.bench_function("single_instruction", |b| {
        let insn = create_test_instruction(CacheArch::X86_64, 0x90);
        b.iter(|| {
            black_box(
                pipeline
                    .translate_instruction(CacheArch::X86_64, CacheArch::Riscv64, black_box(&insn))
                    .unwrap(),
            )
        })
    });

    // 测试指令块翻译
    for size in [10, 50, 100].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let instructions: Vec<_> = (0..size)
                .map(|i| create_test_instruction(CacheArch::X86_64, 0x90 + i as u32))
                .collect();

            b.iter(|| {
                black_box(
                    pipeline
                        .translate_block(CacheArch::X86_64, CacheArch::Riscv64, black_box(&instructions))
                        .unwrap(),
                )
            })
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_encoding_cache,
    bench_pattern_cache,
    bench_register_mapping,
    bench_translation_pipeline
);
criterion_main!(benches);
