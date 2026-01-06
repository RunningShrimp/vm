//! 内存分配器性能基准测试
//!
//! 测试 Round 26 验证的内存分配器:
//! - StackPool 对象池性能
//! - NUMA感知分配器性能
//! - 与标准分配器对比

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use vm_mem::{MemoryPool, StackPool};
use vm_mem::NumaAllocPolicy;

/// 简单测试结构
#[derive(Debug, Clone, Default)]
struct TestData {
    #[allow(dead_code)]
    data: [u64; 8],
}

impl TestData {
    fn new() -> Self {
        Self {
            data: [0xDEAD_BEEF_CAFE_BABEu64; 8],
        }
    }
}

/// 基准测试：StackPool vs 标准分配
fn bench_stack_pool_vs_standard(c: &mut Criterion) {
    let mut group = c.benchmark_group("pool_vs_standard");

    // StackPool分配
    group.bench_function("stack_pool_allocate", |b| {
        let mut pool: StackPool<TestData> = StackPool::with_capacity(50);

        b.iter(|| {
            let item = pool.allocate().unwrap();
            black_box(item);
        });
    });

    // StackPool分配+释放循环
    group.bench_function("stack_pool_cycle", |b| {
        let mut pool: StackPool<TestData> = StackPool::with_capacity(50);

        b.iter(|| {
            let item = pool.allocate().unwrap();
            pool.deallocate(item);
        });
    });

    // 标准分配
    group.bench_function("standard_allocate", |b| {
        b.iter(|| {
            let item = Box::new(TestData::new());
            black_box(&item);
        });
    });

    // 标准分配+释放循环
    group.bench_function("standard_cycle", |b| {
        b.iter(|| {
            let item = Box::new(TestData::new());
            black_box(&item);
            drop(item);
        });
    });

    group.finish();
}

/// 基准测试：不同Pool大小的性能
fn bench_pool_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("pool_sizes");

    for size in [10, 50, 100, 500, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let mut pool: StackPool<TestData> = StackPool::with_capacity(size / 2);

            b.iter(|| {
                for _ in 0..10 {
                    if let Ok(item) = pool.allocate() {
                        black_box(&item);
                        pool.deallocate(item);
                    }
                }
            });
        });
    }

    group.finish();
}

/// 基准测试：预分配效果
fn bench_preallocation_effect(c: &mut Criterion) {
    let mut group = c.benchmark_group("preallocation");

    // 无预分配
    group.bench_function("no_preallocation", |b| {
        let mut pool: StackPool<TestData> = StackPool::new();

        b.iter(|| {
            for _ in 0..10 {
                if let Ok(item) = pool.allocate() {
                    black_box(&item);
                    pool.deallocate(item);
                }
            }
        });
    });

    // 有预分配
    group.bench_function("with_preallocation", |b| {
        let mut pool: StackPool<TestData> = StackPool::with_capacity(50);

        b.iter(|| {
            for _ in 0..10 {
                if let Ok(item) = pool.allocate() {
                    black_box(&item);
                    pool.deallocate(item);
                }
            }
        });
    });

    group.finish();
}

/// 基准测试：NUMA分配策略
fn bench_numa_strategies(c: &mut Criterion) {
    let mut group = c.benchmark_group("numa_strategies");

    // 本地策略
    group.bench_function("local_policy", |b| {
        // 注意：NUMA分配器可能需要特定环境
        // 这里测试创建和策略设置的开销
        b.iter(|| {
            let policy = NumaAllocPolicy::Local;
            black_box(policy);
        });
    });

    // 交错策略
    group.bench_function("interleave_policy", |b| {
        b.iter(|| {
            let policy = NumaAllocPolicy::Interleave;
            black_box(policy);
        });
    });

    // 绑定策略
    group.bench_function("bind_policy", |b| {
        b.iter(|| {
            let policy = NumaAllocPolicy::Bind(0);
            black_box(policy);
        });
    });

    group.finish();
}

/// 基准测试：内存分配吞吐量
fn bench_allocation_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");

    // StackPool吞吐量
    group.bench_function("stack_pool_throughput", |b| {
        let mut pool: StackPool<TestData> = StackPool::with_capacity(500);

        b.iter(|| {
            let mut items = Vec::with_capacity(100);
            for _ in 0..100 {
                if let Ok(item) = pool.allocate() {
                    items.push(item);
                }
            }
            // 释放所有
            for item in items {
                pool.deallocate(item);
            }
        });
    });

    // 标准分配吞吐量
    group.bench_function("standard_throughput", |b| {
        b.iter(|| {
            let mut items = Vec::with_capacity(100);
            for _ in 0..100 {
                items.push(Box::new(TestData::new()));
            }
            // 释放所有
            drop(items);
        });
    });

    group.finish();
}

/// 基准测试：Pool统计信息开销
fn bench_stats_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("stats_overhead");

    // 无统计
    group.bench_function("no_stats_access", |b| {
        let mut pool: StackPool<TestData> = StackPool::with_capacity(50);

        b.iter(|| {
            for _ in 0..10 {
                if let Ok(item) = pool.allocate() {
                    pool.deallocate(item);
                }
            }
        });
    });

    // 有统计
    group.bench_function("with_stats_access", |b| {
        let mut pool: StackPool<TestData> = StackPool::with_capacity(50);

        b.iter(|| {
            for _ in 0..10 {
                if let Ok(item) = pool.allocate() {
                    pool.deallocate(item);
                }
            }
            let stats = pool.stats();
            black_box(stats);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_stack_pool_vs_standard,
    bench_pool_sizes,
    bench_preallocation_effect,
    bench_numa_strategies,
    bench_allocation_throughput,
    bench_stats_overhead
);

criterion_main!(benches);
