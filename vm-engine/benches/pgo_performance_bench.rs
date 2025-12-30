//! PGO（Profile-Guided Optimization）性能基准测试
//!
//! 测试基于profiling的优化对JIT编译性能的影响

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::time::Duration;
use vm_core::GuestAddr;
use vm_engine::jit::pgo_integration::{CompileResult, JitWithPgo, OptimizationLevel};
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator};

/// 创建热路径IR块（高频执行）
fn create_hot_path_ir_block() -> IRBlock {
    let mut builder = IRBuilder::new(0x1000);

    // 创建复杂但频繁执行的代码
    for i in 0..1000 {
        builder.push(IROp::MovImm {
            dst: (i % 16) as u32,
            imm: i as u64,
        });
        builder.push(IROp::Mul {
            dst: (i % 16) as u32,
            src1: ((i + 1) % 16) as u32,
            src2: ((i + 2) % 16) as u32,
            signed: false,
        });
        builder.push(IROp::Add {
            dst: (i % 16) as u32,
            src1: ((i + 1) % 16) as u32,
            src2: ((i + 2) % 16) as u32,
        });
    }

    builder.set_term(Terminator::Ret);
    builder.build()
}

/// 创建冷路径IR块（低频执行）
fn create_cold_path_ir_block() -> IRBlock {
    let mut builder = IRBuilder::new(0x2000);

    // 简单但很少执行的代码
    for i in 0..100 {
        builder.push(IROp::MovImm {
            dst: (i % 16) as u32,
            imm: i as u64,
        });
    }

    builder.set_term(Terminator::Ret);
    builder.build()
}

/// 创建温路径IR块（中等频率）
fn create_warm_path_ir_block() -> IRBlock {
    let mut builder = IRBuilder::new(0x3000);

    for i in 0..500 {
        builder.push(IROp::Add {
            dst: (i % 16) as u32,
            src1: ((i + 1) % 16) as u32,
            src2: ((i + 2) % 16) as u32,
        });
    }

    builder.set_term(Terminator::Ret);
    builder.build()
}

/// PGO vs 非PGO编译性能对比
fn bench_pgo_vs_no_pgo(c: &mut Criterion) {
    let mut group = c.benchmark_group("pgo_comparison");

    // 无PGO的编译
    group.bench_function("no_pgo", |b| {
        let mut jit = JitWithPgo::new();
        jit.disable_pgo();

        let block_data = vec![0x01, 0x02, 0x03, 0x04];

        b.iter(|| {
            black_box(jit.compile_block_with_pgo(black_box(1), black_box(&block_data)));
        });
    });

    // 有PGO的编译（冷路径）
    group.bench_function("pgo_cold", |b| {
        let jit = JitWithPgo::new();
        let block_data = vec![0x01, 0x02, 0x03, 0x04];

        // 记录少量执行（冷路径）
        for _ in 0..10 {
            jit.record_execution(1, 10);
        }

        b.iter(|| {
            black_box(jit.compile_block_with_pgo(black_box(1), black_box(&block_data)));
        });
    });

    // 有PGO的编译（温路径）
    group.bench_function("pgo_warm", |b| {
        let jit = JitWithPgo::new();
        let block_data = vec![0x01, 0x02, 0x03, 0x04];

        // 记录中等执行次数（温路径）
        for _ in 0..300 {
            jit.record_execution(1, 10);
        }

        b.iter(|| {
            black_box(jit.compile_block_with_pgo(black_box(1), black_box(&block_data)));
        });
    });

    // 有PGO的编译（热路径）
    group.bench_function("pgo_hot", |b| {
        let jit = JitWithPgo::new();
        let block_data = vec![0x01, 0x02, 0x03, 0x04];

        // 记录大量执行（热路径）
        for _ in 0..1000 {
            jit.record_execution(1, 10);
        }

        b.iter(|| {
            black_box(jit.compile_block_with_pgo(black_box(1), black_box(&block_data)));
        });
    });

    group.finish();
}

/// PGO优化级别性能测试
fn bench_pgo_optimization_levels(c: &mut Criterion) {
    let mut group = c.benchmark_group("pgo_optimization_levels");

    let block_data = vec![0x01, 0x02, 0x03, 0x04];

    // 无优化
    group.bench_function("level_none", |b| {
        let jit = JitWithPgo::new();

        b.iter(|| {
            let result = jit.compile_block_with_pgo(black_box(1), black_box(&block_data));
            assert_eq!(result.optimization_level, OptimizationLevel::None);
            black_box(result);
        });
    });

    // 标准优化
    group.bench_function("level_standard", |b| {
        let jit = JitWithPgo::new();

        // 记录中等执行次数
        for _ in 0..300 {
            jit.record_execution(1, 10);
        }

        b.iter(|| {
            let result = jit.compile_block_with_pgo(black_box(1), black_box(&block_data));
            black_box(result);
        });
    });

    // 激进优化
    group.bench_function("level_aggressive", |b| {
        let jit = JitWithPgo::new();

        // 记录大量执行
        for _ in 0..1000 {
            jit.record_execution(1, 10);
        }

        b.iter(|| {
            let result = jit.compile_block_with_pgo(black_box(1), black_box(&block_data));
            assert_eq!(result.optimization_level, OptimizationLevel::Aggressive);
            black_box(result);
        });
    });

    group.finish();
}

/// PGO编译时间开销测试
fn bench_pgo_compilation_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("pgo_compilation_overhead");

    let block_sizes = [100, 500, 1000, 5000];

    for size in &block_sizes {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("cold_path", size), size, |b, &size| {
            let jit = JitWithPgo::new();
            let block_data = vec![0x01; size];

            // 冷路径：少量执行
            for _ in 0..10 {
                jit.record_execution(1, 10);
            }

            b.iter(|| {
                black_box(jit.compile_block_with_pgo(black_box(1), black_box(&block_data)));
            });
        });

        group.bench_with_input(BenchmarkId::new("hot_path", size), size, |b, &size| {
            let jit = JitWithPgo::new();
            let block_data = vec![0x01; size];

            // 热路径：大量执行
            for _ in 0..1000 {
                jit.record_execution(1, 10);
            }

            b.iter(|| {
                black_box(jit.compile_block_with_pgo(black_box(1), black_box(&block_data)));
            });
        });
    }

    group.finish();
}

/// PGO执行时间收益测试
fn bench_pgo_execution_benefit(c: &mut Criterion) {
    let mut group = c.benchmark_group("pgo_execution_benefit");

    // 模拟执行时间
    let block_data = vec![0x01, 0x02, 0x03, 0x04];

    // 无PGO执行
    group.bench_function("no_pgo_execution", |b| {
        let mut jit = JitWithPgo::new();
        jit.disable_pgo();

        let result = jit.compile_block_with_pgo(1, &block_data);

        b.iter(|| {
            // 模拟执行编译后的代码
            black_box(result.compile_time_us);
        });
    });

    // PGO热路径执行（应该更快）
    group.bench_function("pgo_hot_execution", |b| {
        let jit = JitWithPgo::new();

        // 记录大量执行
        for _ in 0..1000 {
            jit.record_execution(1, 10);
        }

        let result = jit.compile_block_with_pgo(1, &block_data);

        b.iter(|| {
            // 模拟执行编译后的代码
            black_box(result.compile_time_us);
        });
    });

    group.finish();
}

/// PGO内存使用测试
fn bench_pgo_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("pgo_memory");

    let block_sizes = [100, 1000, 10000];

    for size in &block_sizes {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let jit = JitWithPgo::new();
            let block_data = vec![0x01; *size];

            // 收集profiling数据
            for block_id in 0..100 {
                for _ in 0..100 {
                    jit.record_execution(block_id, 10);
                }
            }

            b.iter(|| {
                black_box(jit.compile_block_with_pgo(black_box(1), black_box(&block_data)));
            });
        });
    }

    group.finish();
}

/// PGO Profile收集开销测试
fn bench_pgo_profile_collection(c: &mut Criterion) {
    let mut group = c.benchmark_group("pgo_profile_collection");

    let jit = JitWithPgo::new();

    // 单次记录
    group.bench_function("single_record", |b| {
        b.iter(|| {
            black_box(jit.record_execution(black_box(1), black_box(10)));
        });
    });

    // 批量记录
    for batch_size in [10, 100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    for block_id in 0..size {
                        black_box(jit.record_execution(black_box(block_id), black_box(10)));
                    }
                });
            },
        );
    }

    group.finish();
}

/// PGO热路径识别准确性测试
fn bench_pgo_hot_path_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("pgo_hot_path_detection");

    let jit = JitWithPgo::new();

    // 创建混合热度的路径
    for block_id in 0..100 {
        let execution_count = match block_id {
            0..=10 => 1000, // 热路径
            11..=30 => 300, // 温路径
            _ => 10,        // 冷路径
        };

        for _ in 0..execution_count {
            jit.record_execution(block_id, 10);
        }
    }

    // 测试热路径查询性能
    group.bench_function("query_hot_blocks", |b| {
        b.iter(|| {
            let collector = jit.get_profile_collector();
            black_box(collector.get_hot_blocks(500));
        });
    });

    // 测试冷路径查询性能
    group.bench_function("query_cold_blocks", |b| {
        b.iter(|| {
            let collector = jit.get_profile_collector();
            black_box(collector.get_cold_blocks(500));
        });
    });

    group.finish();
}

/// PGO自适应编译测试
fn bench_pgo_adaptive_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("pgo_adaptive");

    let jit = JitWithPgo::new();
    let block_data = vec![0x01, 0x02, 0x03, 0x04];

    // 测试从冷到热的转换
    group.bench_function("cold_to_hot_transition", |b| {
        let mut jit = JitWithPgo::new();

        b.iter(|| {
            // 初始编译（冷）
            let result1 = jit.compile_block_with_pgo(1, &block_data);
            assert!(!result1.used_pgo);

            // 执行一些次数
            for _ in 0..600 {
                jit.record_execution(1, 10);
            }

            // 重新编译（热）
            let result2 = jit.compile_block_with_pgo(1, &block_data);
            assert!(result2.used_pgo);

            black_box((result1, result2));
        });
    });

    group.finish();
}

/// 配置基准测试
fn configure_criterion() -> Criterion {
    Criterion::default()
        .warm_up_time(Duration::from_secs(2))
        .measurement_time(Duration::from_secs(8))
        .sample_size(100)
}

criterion_group! {
    name = pgo_benches;
    config = configure_criterion();
    targets =
        bench_pgo_vs_no_pgo,
        bench_pgo_optimization_levels,
        bench_pgo_compilation_overhead,
        bench_pgo_execution_benefit,
        bench_pgo_memory_usage,
        bench_pgo_profile_collection,
        bench_pgo_hot_path_detection,
        bench_pgo_adaptive_compilation
}

criterion_main!(pgo_benches);
