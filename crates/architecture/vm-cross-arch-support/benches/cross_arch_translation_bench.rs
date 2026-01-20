// 跨架构翻译性能基准测试
//
// 测试不同架构之间的指令翻译性能：
// - x86_64 ↔ ARM64
// - x86_64 ↔ RISC-V
// - ARM64 ↔ RISC-V
//
// 测试指标：
// - 翻译速度（指令/秒）
// - 缓存命中率
// - 内存开销

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use vm_cross_arch_support::encoding_cache::{
    Arch as CacheArch, Instruction, InstructionEncodingCache,
};
use vm_cross_arch_support::translation_pipeline::CrossArchTranslationPipeline;

/// 创建测试指令集
fn create_test_instructions(count: usize) -> Vec<Instruction> {
    (0..count)
        .map(|i| Instruction {
            arch: CacheArch::X86_64,
            opcode: (i % 256) as u32,
            operands: vec![
                vm_cross_arch_support::encoding_cache::Operand::Register((i % 16) as u8),
                vm_cross_arch_support::encoding_cache::Operand::Immediate(i as i64),
            ],
        })
        .collect()
}

/// 基准测试：x86_64 → ARM64 翻译（单条指令）
fn bench_x86_to_arm_single(c: &mut Criterion) {
    let mut pipeline = CrossArchTranslationPipeline::new();
    let instruction = create_test_instructions(1).pop().unwrap();

    c.bench_function("x86_to_arm_single", |b| {
        b.iter(|| {
            black_box(
                pipeline
                    .translate_instruction(CacheArch::X86_64, CacheArch::ARM64, &instruction)
                    .unwrap(),
            )
        })
    });
}

/// 基准测试：x86_64 → ARM64 翻译（批量指令）
fn bench_x86_to_arm_batch(c: &mut Criterion) {
    let sizes = vec![10, 100, 1000];

    let mut group = c.benchmark_group("x86_to_arm_batch");

    for size in sizes {
        let instructions = create_test_instructions(size);
        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let mut pipeline = CrossArchTranslationPipeline::new();
                black_box(
                    pipeline
                        .translate_block(CacheArch::X86_64, CacheArch::ARM64, &instructions)
                        .unwrap(),
                )
            })
        });
    }

    group.finish();
}

/// 基准测试：x86_64 → RISC-V 翻译（批量指令）
fn bench_x86_to_riscv_batch(c: &mut Criterion) {
    let sizes = vec![10, 100, 1000];

    let mut group = c.benchmark_group("x86_to_riscv_batch");

    for size in sizes {
        let instructions = create_test_instructions(size);
        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let mut pipeline = CrossArchTranslationPipeline::new();
                black_box(
                    pipeline
                        .translate_block(CacheArch::X86_64, CacheArch::Riscv64, &instructions)
                        .unwrap(),
                )
            })
        });
    }

    group.finish();
}

/// 基准测试：ARM64 → RISC-V 翻译（批量指令）
fn bench_arm_to_riscv_batch(c: &mut Criterion) {
    let sizes = vec![10, 100, 1000];

    let mut group = c.benchmark_group("arm_to_riscv_batch");

    for size in sizes {
        let mut arm_instructions = create_test_instructions(size);
        // 修改为ARM64架构
        for insn in &mut arm_instructions {
            insn.arch = CacheArch::ARM64;
        }

        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let mut pipeline = CrossArchTranslationPipeline::new();
                black_box(
                    pipeline
                        .translate_block(CacheArch::ARM64, CacheArch::Riscv64, &arm_instructions)
                        .unwrap(),
                )
            })
        });
    }

    group.finish();
}

/// 基准测试：并行翻译多个块
fn bench_parallel_block_translation(c: &mut Criterion) {
    let block_counts = vec![2, 4, 8, 16];

    let mut group = c.benchmark_group("parallel_blocks");

    for &block_count in &block_counts {
        let blocks: Vec<Vec<Instruction>> = (0..block_count)
            .map(|_| create_test_instructions(100))
            .collect();

        let total_instructions = block_count * 100;
        group.throughput(Throughput::Elements(total_instructions as u64));

        group.bench_with_input(
            BenchmarkId::new("blocks", block_count),
            &blocks,
            |b, blocks| {
                b.iter(|| {
                    let mut pipeline = CrossArchTranslationPipeline::new();
                    black_box(
                        pipeline
                            .translate_blocks_parallel(CacheArch::X86_64, CacheArch::ARM64, blocks)
                            .unwrap(),
                    )
                })
            },
        );
    }

    group.finish();
}

/// 基准测试：编码缓存性能
fn bench_encoding_cache(c: &mut Criterion) {
    let cache = InstructionEncodingCache::new();
    let instruction = create_test_instructions(1).pop().unwrap();

    // 首次编码（缓存未命中）
    c.bench_function("encoding_cache_miss", |b| {
        b.iter(|| {
            let cache = InstructionEncodingCache::new();
            black_box(cache.encode_or_lookup(&instruction).unwrap())
        })
    });

    // 后续编码（缓存命中）
    c.bench_function("encoding_cache_hit", |b| {
        b.iter(|| black_box(cache.encode_or_lookup(&instruction).unwrap()))
    });
}

/// 基准测试：模式匹配缓存性能
fn bench_pattern_cache(c: &mut Criterion) {
    let instructions = create_test_instructions(100);

    c.bench_function("pattern_cache_hit_rate", |b| {
        b.iter(|| {
            let mut pipeline = CrossArchTranslationPipeline::new();
            // 预热模式缓存
            for _ in 0..10 {
                let _ =
                    pipeline.translate_block(CacheArch::X86_64, CacheArch::ARM64, &instructions);
            }
            black_box(
                pipeline
                    .translate_block(CacheArch::X86_64, CacheArch::ARM64, &instructions)
                    .unwrap(),
            )
        })
    });
}

/// 基准测试：翻译质量（生成的代码效率）
fn bench_translation_quality(c: &mut Criterion) {
    let instructions = create_test_instructions(100);

    c.bench_function("translation_quality", |b| {
        b.iter(|| {
            let mut pipeline = CrossArchTranslationPipeline::new();
            let translated = pipeline
                .translate_block(CacheArch::X86_64, CacheArch::ARM64, &instructions)
                .unwrap();

            // 评估翻译质量：
            // 1. 翻译后的指令数量应该与源指令相同
            assert_eq!(translated.len(), instructions.len());

            // 2. 翻译后的指令应该有效
            for insn in &translated {
                assert!(insn.arch == CacheArch::ARM64);
            }

            black_box(translated)
        })
    });
}

criterion_group!(
    benches,
    bench_x86_to_arm_single,
    bench_x86_to_arm_batch,
    bench_x86_to_riscv_batch,
    bench_arm_to_riscv_batch,
    bench_parallel_block_translation,
    bench_encoding_cache,
    bench_pattern_cache,
    bench_translation_quality
);

criterion_main!(benches);
