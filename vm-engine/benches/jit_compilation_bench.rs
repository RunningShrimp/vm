//! JIT编译器性能基准测试
//!
//! 使用Criterion框架全面测试JIT编译器的性能指标

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use vm_engine::jit::core::{JITEngine, JITConfig};
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator};
use std::time::Duration;

/// 创建测试IR块
fn create_test_ir_block(size: usize) -> IRBlock {
    let mut builder = IRBuilder::new(0x1000);

    for i in 0..size {
        let dst = (i % 16) as u32;
        let src1 = ((i + 1) % 16) as u32;
        let src2 = ((i + 2) % 16) as u32;

        match i % 4 {
            0 => builder.push(IROp::MovImm { dst, imm: i as u64 }),
            1 => builder.push(IROp::Add { dst, src1, src2 }),
            2 => builder.push(IROp::Mul { dst, src1, src2 }),
            _ => builder.push(IROp::Load {
                dst,
                base: 0,
                offset: (i * 8) as i64,
                size: 8,
                flags: vm_ir::MemFlags::default(),
            }),
        }
    }

    builder.set_term(Terminator::Ret);
    builder.build()
}

/// JIT编译速度基准测试
fn bench_jit_compilation_speed(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_compilation");

    // 测试不同IR块大小的编译速度
    for size in [10, 50, 100, 500, 1000, 5000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let ir_block = create_test_ir_block(size);
            let config = JITConfig::default();
            let mut jit_engine = JITEngine::new(config);

            b.iter(|| {
                black_box(jit_engine.compile(black_box(&ir_block)));
            });
        });
    }

    group.finish();
}

/// JIT编译内存使用基准测试
fn bench_jit_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_memory");

    // 测试不同代码大小的内存使用
    for size in [100, 500, 1000, 5000, 10000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let ir_blocks: Vec<_> = (0..10)
                .map(|i| create_test_ir_block(size))
                .collect();

            let config = JITConfig::default();
            let mut jit_engine = JITEngine::new(config);

            b.iter(|| {
                for ir_block in &ir_blocks {
                    black_box(jit_engine.compile(black_box(ir_block)));
                }
            });
        });
    }

    group.finish();
}

/// JIT代码缓存性能基准测试
fn bench_jit_cache_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_cache");

    let ir_block = create_test_ir_block(1000);
    let config = JITConfig::default();
    let mut jit_engine = JITEngine::new(config);

    // 首次编译（缓存未命中）
    group.bench_function("cache_miss", |b| {
        b.iter(|| {
            let test_block = create_test_ir_block(1000);
            black_box(jit_engine.compile(black_box(&test_block)));
        });
    });

    // 重复编译（缓存命中）
    group.bench_function("cache_hit", |b| {
        b.iter(|| {
            black_box(jit_engine.compile(black_box(&ir_block)));
        });
    });

    group.finish();
}

/// JIT优化级别性能对比基准测试
fn bench_jit_optimization_levels(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_optimization");

    let ir_block = create_test_ir_block(500);

    // 无优化
    group.bench_function("no_optimization", |b| {
        let mut config = JITConfig::default();
        config.optimization_level = 0;
        let mut jit_engine = JITEngine::new(config);

        b.iter(|| {
            black_box(jit_engine.compile(black_box(&ir_block)));
        });
    });

    // 标准优化
    group.bench_function("standard_optimization", |b| {
        let mut config = JITConfig::default();
        config.optimization_level = 1;
        let mut jit_engine = JITEngine::new(config);

        b.iter(|| {
            black_box(jit_engine.compile(black_box(&ir_block)));
        });
    });

    // 激进优化
    group.bench_function("aggressive_optimization", |b| {
        let mut config = JITConfig::default();
        config.optimization_level = 2;
        let mut jit_engine = JITEngine::new(config);

        b.iter(|| {
            black_box(jit_engine.compile(black_box(&ir_block)));
        });
    });

    group.finish();
}

/// JIT热点检测性能基准测试
fn bench_jit_hotspot_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_hotspot");

    let ir_block = create_test_ir_block(1000);
    let config = JITConfig::default();
    let mut jit_engine = JITEngine::new(config);

    // 模拟热点检测：多次执行同一块
    for execution_count in [10, 50, 100, 500, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("executions", execution_count),
            execution_count,
            |b, &count| {
                b.iter(|| {
                    for _ in 0..count {
                        black_box(jit_engine.compile(black_box(&ir_block)));
                    }
                });
            },
        );
    }

    group.finish();
}

/// JIT编译吞吐量基准测试
fn bench_jit_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_throughput");

    // 测试批量编译的吞吐量
    for batch_size in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &size| {
                let ir_blocks: Vec<_> = (0..size)
                    .map(|i| create_test_ir_block(100))
                    .collect();

                let config = JITConfig::default();
                let mut jit_engine = JITEngine::new(config);

                b.iter(|| {
                    for ir_block in &ir_blocks {
                        black_box(jit_engine.compile(black_box(ir_block)));
                    }
                });
            },
        );
    }

    group.finish();
}

/// JIT不同类型操作的性能基准测试
fn bench_jit_operation_types(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_operations");

    // 算术操作密集
    group.bench_function("arithmetic_heavy", |b| {
        let mut builder = IRBuilder::new(0x1000);
        for i in 0..1000 {
            builder.push(IROp::Add {
                dst: (i % 16) as u32,
                src1: ((i + 1) % 16) as u32,
                src2: ((i + 2) % 16) as u32,
            });
        }
        builder.set_term(Terminator::Ret);
        let ir_block = builder.build();

        let config = JITConfig::default();
        let mut jit_engine = JITEngine::new(config);

        b.iter(|| {
            black_box(jit_engine.compile(black_box(&ir_block)));
        });
    });

    // 内存操作密集
    group.bench_function("memory_heavy", |b| {
        let mut builder = IRBuilder::new(0x1000);
        for i in 0..1000 {
            builder.push(IROp::Load {
                dst: (i % 16) as u32,
                base: 0,
                offset: (i * 8) as i64,
                size: 8,
                flags: vm_ir::MemFlags::default(),
            });
        }
        builder.set_term(Terminator::Ret);
        let ir_block = builder.build();

        let config = JITConfig::default();
        let mut jit_engine = JITEngine::new(config);

        b.iter(|| {
            black_box(jit_engine.compile(black_box(&ir_block)));
        });
    });

    // 分支密集
    group.bench_function("branch_heavy", |b| {
        let mut builder = IRBuilder::new(0x1000);
        for i in 0..500 {
            builder.push(IROp::CmpEq {
                dst: (i % 16) as u32,
                lhs: ((i + 1) % 16) as u32,
                rhs: ((i + 2) % 16) as u32,
            });
            builder.push(IROp::Beq {
                src1: ((i + 1) % 16) as u32,
                src2: ((i + 2) % 16) as u32,
                target: 0x1000 + (i * 4) as u64,
            });
        }
        builder.set_term(Terminator::Ret);
        let ir_block = builder.build();

        let config = JITConfig::default();
        let mut jit_engine = JITEngine::new(config);

        b.iter(|| {
            black_box(jit_engine.compile(black_box(&ir_block)));
        });
    });

    group.finish();
}

/// 配置基准测试参数
fn configure_criterion() -> Criterion {
    Criterion::default()
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(10))
        .sample_size(100)
        .significance_level(0.05) // 95% 置信度
}

criterion_group! {
    name = jit_benches;
    config = configure_criterion();
    targets =
        bench_jit_compilation_speed,
        bench_jit_memory_usage,
        bench_jit_cache_performance,
        bench_jit_optimization_levels,
        bench_jit_hotspot_detection,
        bench_jit_throughput,
        bench_jit_operation_types
}

criterion_main!(jit_benches);
