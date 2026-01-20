//! 异步批处理性能基准测试
//!
//! 测试异步执行和批处理的性能特征

use std::time::Duration;

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use vm_engine::executor::async_executor::{HybridExecutor, InterpreterExecutor, JitExecutor};

/// 批量执行基准测试
fn bench_batch_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_execution");

    // JIT批量执行
    for batch_size in [10, 50, 100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("jit", batch_size),
            batch_size,
            |b, &size| {
                let mut executor = JitExecutor::new();
                let _block_ids: Vec<u64> = (0..size).collect();

                b.iter(|| {
                    black_box(executor.execute_blocks(black_box(&block_ids)));
                });
            },
        );
    }

    // 解释器批量执行
    for batch_size in [10, 50, 100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("interpreter", batch_size),
            batch_size,
            |b, &size| {
                let mut executor = InterpreterExecutor::new();
                let _block_ids: Vec<u64> = (0..size).collect();

                b.iter(|| {
                    black_box(executor.execute_block(black_box(size)));
                });
            },
        );
    }

    group.finish();
}

/// 单次执行性能基准测试
fn bench_single_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_execution");

    // JIT单次执行（有缓存）
    group.bench_function("jit_cached", |b| {
        let mut executor = JitExecutor::new();

        // 预热缓存
        let _ = executor.execute_block(1);

        b.iter(|| {
            black_box(executor.execute_block(black_box(1)));
        });
    });

    // JIT单次执行（无缓存）
    group.bench_function("jit_uncached", |b| {
        let mut executor = JitExecutor::new();
        let mut block_id = 0;

        b.iter(|| {
            block_id += 1;
            black_box(executor.execute_block(black_box(block_id)));
        });
    });

    // 解释器单次执行
    group.bench_function("interpreter", |b| {
        let mut executor = InterpreterExecutor::new();

        b.iter(|| {
            black_box(executor.execute_block(black_box(1)));
        });
    });

    // 混合模式（JIT优先）
    group.bench_function("hybrid_jit", |b| {
        let mut executor = HybridExecutor::new();
        executor.set_prefer_jit(true);

        b.iter(|| {
            black_box(executor.execute_block(black_box(1)));
        });
    });

    // 混合模式（解释器优先）
    group.bench_function("hybrid_interpreter", |b| {
        let mut executor = HybridExecutor::new();
        executor.set_prefer_jit(false);

        b.iter(|| {
            black_box(executor.execute_block(black_box(1)));
        });
    });

    group.finish();
}

/// 缓存效果基准测试
fn bench_cache_effects(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_effects");

    // 测试不同缓存命中率
    for hit_rate in [0.0, 0.25, 0.5, 0.75, 1.0].iter() {
        group.bench_with_input(
            BenchmarkId::new("hit_rate", hit_rate),
            hit_rate,
            |b, &rate| {
                let mut executor = JitExecutor::new();
                let unique_blocks = 100;
                let batch_size = 1000;

                // 预填充缓存
                let cached_count = (unique_blocks as f64 * rate) as u64;
                for block_id in 0..cached_count {
                    let _ = executor.execute_block(block_id);
                }

                let block_ids: Vec<u64> = (0..batch_size).map(|i| i % unique_blocks).collect();

                b.iter(|| {
                    for &block_id in &block_ids {
                        black_box(executor.execute_block(black_box(block_id)));
                    }
                });
            },
        );
    }

    group.finish();
}

/// 缓存大小影响基准测试
fn bench_cache_size_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_size");

    // 不同工作集大小
    for working_set in [10, 50, 100, 500, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(working_set),
            working_set,
            |b, &size| {
                let mut executor = JitExecutor::new();
                let _block_ids: Vec<u64> = (0..size).collect();

                // 第一次执行（编译并缓存）
                for &block_id in &block_ids {
                    let _ = executor.execute_block(block_id);
                }

                // 第二次执行（使用缓存）
                b.iter(|| {
                    for &block_id in &block_ids {
                        black_box(executor.execute_block(black_box(block_id)));
                    }
                });
            },
        );
    }

    group.finish();
}

/// 吞吐量基准测试
fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");

    // JIT吞吐量
    group.bench_function("jit", |b| {
        let mut executor = JitExecutor::new();
        let iterations = 10000;

        // 预热
        for i in 0..10 {
            let _ = executor.execute_block(i);
        }

        b.iter(|| {
            for i in 0..iterations {
                black_box(executor.execute_block(black_box(i % 10)));
            }
        });
    });

    // 解释器吞吐量
    group.bench_function("interpreter", |b| {
        let mut executor = InterpreterExecutor::new();
        let iterations = 10000;

        b.iter(|| {
            for i in 0..iterations {
                black_box(executor.execute_block(black_box(i % 10)));
            }
        });
    });

    // 混合模式吞吐量
    group.bench_function("hybrid", |b| {
        let mut executor = HybridExecutor::new();
        let iterations = 10000;

        b.iter(|| {
            for i in 0..iterations {
                black_box(executor.execute_block(black_box(i % 10)));
            }
        });
    });

    group.finish();
}

/// 并发执行基准测试（模拟）
fn bench_concurrent_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_execution");

    // 模拟单线程顺序执行
    group.bench_function("sequential", |b| {
        let mut executor = JitExecutor::new();
        let block_ids: Vec<u64> = (0..1000).collect();

        b.iter(|| {
            for &block_id in &block_ids {
                black_box(executor.execute_block(black_box(block_id)));
            }
        });
    });

    // 模拟交错执行（模拟并发）
    group.bench_function("interleaved", |b| {
        let mut executor1 = JitExecutor::new();
        let mut executor2 = JitExecutor::new();

        b.iter(|| {
            for i in 0..1000 {
                if i % 2 == 0 {
                    black_box(executor1.execute_block(black_box(i)));
                } else {
                    black_box(executor2.execute_block(black_box(i)));
                }
            }
        });
    });

    group.finish();
}

/// 冷启动vs热启动基准测试
fn bench_cold_vs_hot_start(c: &mut Criterion) {
    let mut group = c.benchmark_group("startup_performance");

    // 冷启动（空缓存）
    group.bench_function("cold_start", |b| {
        b.iter(|| {
            let mut executor = JitExecutor::new();
            for i in 0..100 {
                black_box(executor.execute_block(black_box(i)));
            }
        });
    });

    // 热启动（预热缓存）
    group.bench_function("hot_start", |b| {
        let mut executor = JitExecutor::new();

        // 预热缓存
        for i in 0..100 {
            let _ = executor.execute_block(i);
        }

        b.iter(|| {
            for i in 0..100 {
                black_box(executor.execute_block(black_box(i)));
            }
        });
    });

    group.finish();
}

/// 执行模式切换基准测试
fn bench_execution_mode_switching(c: &mut Criterion) {
    let mut group = c.benchmark_group("mode_switching");

    // 频繁切换JIT/解释器
    group.bench_function("frequent_switch", |b| {
        let mut executor = HybridExecutor::new();

        b.iter(|| {
            for i in 0..1000 {
                executor.set_prefer_jit(i % 2 == 0);
                black_box(executor.execute_block(black_box(i)));
            }
        });
    });

    // 偶尔切换
    group.bench_function("rare_switch", |b| {
        let mut executor = HybridExecutor::new();

        b.iter(|| {
            for i in 0..1000 {
                if i % 100 == 0 {
                    executor.set_prefer_jit(i % 200 == 0);
                }
                black_box(executor.execute_block(black_box(i)));
            }
        });
    });

    group.finish();
}

/// 内存使用基准测试
fn bench_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage");

    // 测试不同数量块缓存的内存使用
    for block_count in [100, 1000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(block_count),
            block_count,
            |b, &count| {
                b.iter(|| {
                    let mut executor = JitExecutor::new();
                    for i in 0..count {
                        let _ = executor.execute_block(i);
                    }
                    black_box(executor.get_stats());
                });
            },
        );
    }

    group.finish();
}

/// 批处理大小优化基准测试
fn bench_batch_size_optimization(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_optimization");

    // 测试最优批处理大小
    for batch_size in [1, 10, 50, 100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &size| {
                let mut executor = JitExecutor::new();
                let total_blocks = 10000;
                let batches = total_blocks / size;

                b.iter(|| {
                    for _ in 0..batches {
                        let batch_start = std::time::Instant::now();
                        let _block_ids: Vec<u64> = (0..size).collect();
                        let _ = executor.execute_blocks(&block_ids);
                        black_box(batch_start.elapsed());
                    }
                });
            },
        );
    }

    group.finish();
}

/// 统计信息收集开销基准测试
fn bench_stats_collection_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("stats_overhead");

    // 无统计
    group.bench_function("no_stats", |b| {
        let mut executor = JitExecutor::new();

        b.iter(|| {
            for _ in 0..1000 {
                let _ = executor.execute_block(1);
            }
        });
    });

    // 有统计
    group.bench_function("with_stats", |b| {
        let mut executor = JitExecutor::new();

        b.iter(|| {
            for _ in 0..1000 {
                let _ = executor.execute_block(1);
                black_box(executor.get_stats());
            }
        });
    });

    group.finish();
}

/// 配置基准测试
fn configure_criterion() -> Criterion {
    Criterion::default()
        .warm_up_time(Duration::from_secs(2))
        .measurement_time(Duration::from_secs(5))
        .sample_size(100)
}

criterion_group! {
    name = async_batch_benches;
    config = configure_criterion();
    targets =
        bench_batch_execution,
        bench_single_execution,
        bench_cache_effects,
        bench_cache_size_impact,
        bench_throughput,
        bench_concurrent_execution,
        bench_cold_vs_hot_start,
        bench_execution_mode_switching,
        bench_memory_usage,
        bench_batch_size_optimization,
        bench_stats_collection_overhead
}

criterion_main!(async_batch_benches);
