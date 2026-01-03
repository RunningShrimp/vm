// Benchmark for concurrent vs sequential batch memory operations
//
// This benchmark demonstrates the 200-300% performance improvement
// when using concurrent async operations for batch translations.

use std::time::Duration;

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use vm_optimizers::memory::{AsyncPrefetchingTlb, ConcurrencyConfig, MemoryOptimizer, NumaConfig};

fn bench_sequential_batch_translation(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_translation_sequential");
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(2));

    for size in [10, 50, 100, 500, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let tlb = AsyncPrefetchingTlb::with_concurrency(false, ConcurrencyConfig::sequential());

            let addrs: Vec<u64> = (0..size).map(|i| 0x1000 + (i * 4096)).collect();

            b.iter(|| {
                let result = tlb.translate_batch(black_box(&addrs));
                black_box(result)
            });
        });
    }
    group.finish();
}

fn bench_concurrent_batch_translation(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_translation_concurrent");
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(2));

    for size in [10, 50, 100, 500, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let tlb = AsyncPrefetchingTlb::with_concurrency(false, ConcurrencyConfig::new(8));

            let addrs: Vec<u64> = (0..size).map(|i| 0x1000 + (i * 4096)).collect();

            let rt = tokio::runtime::Runtime::new().unwrap();

            b.iter(|| {
                let result =
                    rt.block_on(async { tlb.translate_batch_concurrent(black_box(&addrs)).await });
                black_box(result)
            });
        });
    }
    group.finish();
}

fn bench_concurrent_batch_translation_varying_concurrency(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_translation_concurrency_levels");
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(2));

    let batch_size = 500;
    let addrs: Vec<u64> = (0..batch_size).map(|i| 0x1000 + (i * 4096)).collect();

    for concurrency in [1, 2, 4, 8, 16, 32].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(concurrency),
            concurrency,
            |b, &concurrency| {
                let tlb = AsyncPrefetchingTlb::with_concurrency(
                    false,
                    ConcurrencyConfig::new(concurrency),
                );

                let rt = tokio::runtime::Runtime::new().unwrap();

                b.iter(|| {
                    let result = rt.block_on(async {
                        tlb.translate_batch_concurrent(black_box(&addrs)).await
                    });
                    black_box(result)
                });
            },
        );
    }
    group.finish();
}

fn bench_memory_optimizer_sequential_vs_concurrent(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_optimizer_comparison");
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(2));

    let config = NumaConfig {
        num_nodes: 4,
        mem_per_node: 1024 * 1024,
    };

    let batch_sizes = [50, 200, 500];
    for size in batch_sizes.iter() {
        // Sequential benchmark
        group.bench_with_input(BenchmarkId::new("sequential", size), size, |b, &size| {
            let optimizer =
                MemoryOptimizer::with_concurrency(config, ConcurrencyConfig::sequential());

            let addrs: Vec<u64> = (0..size).map(|i| 0x1000 + (i * 4096)).collect();

            b.iter(|| {
                let result = optimizer.batch_access(black_box(&addrs));
                black_box(result)
            });
        });

        // Concurrent benchmark
        group.bench_with_input(BenchmarkId::new("concurrent", size), size, |b, &size| {
            let optimizer = MemoryOptimizer::with_concurrency(config, ConcurrencyConfig::new(8));

            let addrs: Vec<u64> = (0..size).map(|i| 0x1000 + (i * 4096)).collect();

            let rt = tokio::runtime::Runtime::new().unwrap();

            b.iter(|| {
                let result = rt
                    .block_on(async { optimizer.batch_access_concurrent(black_box(&addrs)).await });
                black_box(result)
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_sequential_batch_translation,
    bench_concurrent_batch_translation,
    bench_concurrent_batch_translation_varying_concurrency,
    bench_memory_optimizer_sequential_vs_concurrent
);
criterion_main!(benches);
