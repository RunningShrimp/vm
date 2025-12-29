//! GC Performance Benchmarks
//!
//! Comprehensive benchmarks for garbage collection performance including:
//! - GC pause time (minor and major collections)
//! - GC throughput (objects/second)
//! - Memory reclamation efficiency
//! - Generation promotion rates
//! - Write barrier overhead
//! - Parallel marking performance
//! - Adaptive quota management

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::sync::Arc;
use std::time::Duration;
use vm_optimizers::gc::{
    AdaptiveQuota, LockFreeWriteBarrier, OptimizedGc, ParallelMarker, WriteBarrierType,
};

// Test parameters
const OBJECT_COUNTS: &[usize] = &[100, 1_000, 10_000, 100_000];
const HEAP_SIZES: &[usize] = &[1_000_000, 10_000_000, 100_000_000]; // 1MB, 10MB, 100MB
const WORKER_COUNTS: &[usize] = &[1, 2, 4, 8, 16];
const PAUSE_TARGETS: &[u64] = &[1_000, 5_000, 10_000, 50_000]; // 1ms, 5ms, 10ms, 50ms

/// Simulated object structure
#[derive(Debug, Clone)]
struct TestObject {
    id: u64,
    size: usize,
    data: Vec<u8>,
}

impl TestObject {
    fn new(id: u64, size: usize) -> Self {
        Self {
            id,
            size,
            data: vec![0u8; size],
        }
    }

    fn touch(&self) {
        black_box(&self.data);
    }
}

/// Benchmark GC pause time for minor collections
fn bench_gc_pause_time(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc_pause_time_minor");

    for &obj_count in OBJECT_COUNTS {
        group.throughput(Throughput::Elements(obj_count as u64));

        group.bench_with_input(
            BenchmarkId::new("minor_collection", obj_count),
            &obj_count,
            |b, &obj_count| {
                let gc = OptimizedGc::new(4, 10_000, WriteBarrierType::Atomic);

                // Allocate objects
                let objects: Vec<TestObject> = (0..obj_count)
                    .map(|i| TestObject::new(i as u64, 64))
                    .collect();

                b.iter(|| {
                    let bytes_collected = objects.iter().map(|o| o.size).sum::<usize>() as u64;
                    black_box(gc.collect_minor(bytes_collected).unwrap());
                });
            },
        );
    }

    group.finish();
}

/// Benchmark GC pause time for major collections
fn bench_gc_pause_time_major(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc_pause_time_major");
    group.sample_size(20);

    for &obj_count in OBJECT_COUNTS {
        group.throughput(Throughput::Elements(obj_count as u64));

        group.bench_with_input(
            BenchmarkId::new("major_collection", obj_count),
            &obj_count,
            |b, &obj_count| {
                let gc = OptimizedGc::new(4, 10_000, WriteBarrierType::Atomic);

                // Allocate objects
                let objects: Vec<TestObject> = (0..obj_count)
                    .map(|i| TestObject::new(i as u64, 256))
                    .collect();

                b.iter(|| {
                    let bytes_collected = objects.iter().map(|o| o.size).sum::<usize>() as u64;
                    black_box(gc.collect_major(bytes_collected).unwrap());
                });
            },
        );
    }

    group.finish();
}

/// Benchmark GC throughput (allocations + collections per second)
fn bench_gc_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc_throughput");

    group.bench_function("high_allocation_rate_small", |b| {
        let gc = Arc::new(OptimizedGc::new(4, 10_000, WriteBarrierType::Atomic));

        b.iter(|| {
            // High allocation rate
            for i in 0..10_000 {
                let obj = TestObject::new(i, 64);
                black_box(obj);

                // Trigger collection periodically
                if i % 1000 == 0 {
                    black_box(gc.collect_minor(64000).unwrap());
                }
            }
        });
    });

    group.bench_function("high_allocation_rate_medium", |b| {
        let gc = Arc::new(OptimizedGc::new(4, 10_000, WriteBarrierType::Atomic));

        b.iter(|| {
            for i in 0..5_000 {
                let obj = TestObject::new(i, 256);
                black_box(obj);

                if i % 500 == 0 {
                    black_box(gc.collect_minor(128000).unwrap());
                }
            }
        });
    });

    group.bench_function("high_allocation_rate_large", |b| {
        let gc = Arc::new(OptimizedGc::new(4, 10_000, WriteBarrierType::Atomic));

        b.iter(|| {
            for i in 0..1_000 {
                let obj = TestObject::new(i, 1024);
                black_box(obj);

                if i % 100 == 0 {
                    black_box(gc.collect_minor(102400).unwrap());
                }
            }
        });
    });

    group.finish();
}

/// Benchmark memory reclamation efficiency
fn bench_memory_reclamation(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_reclamation");

    for &heap_size in HEAP_SIZES {
        group.bench_with_input(
            BenchmarkId::new("reclaim_efficiency", heap_size),
            &heap_size,
            |b, &heap_size| {
                let gc = OptimizedGc::new(4, 10_000, WriteBarrierType::Atomic);

                b.iter(|| {
                    // Allocate to heap size
                    let obj_count = heap_size / 256;
                    let objects: Vec<TestObject> = (0..obj_count)
                        .map(|i| TestObject::new(i as u64, 256))
                        .collect();

                    // Collect
                    let bytes_collected = objects.iter().map(|o| o.size).sum::<usize>() as u64;

                    black_box(gc.collect_major(bytes_collected).unwrap());

                    let stats_after = gc.get_stats();

                    // Calculate efficiency: bytes collected / pause time
                    let pause_time = stats_after.current_pause_time_us;
                    black_box((bytes_collected, pause_time));
                });
            },
        );
    }

    group.finish();
}

/// Benchmark generational GC (young vs old generation)
fn bench_gc_generational(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc_generational");

    // Young generation (short-lived objects)
    group.bench_function("young_gen_survival_rate", |b| {
        let gc = OptimizedGc::new(4, 10_000, WriteBarrierType::Atomic);

        b.iter(|| {
            // Create short-lived objects
            let _objects: Vec<TestObject> = (0..10_000).map(|i| TestObject::new(i, 128)).collect();

            // Minor collection
            black_box(gc.collect_minor(1_280_000).unwrap());

            let stats = gc.get_stats();
            black_box(stats);
        });
    });

    // Old generation (long-lived objects)
    group.bench_function("old_gen_promotion", |b| {
        let gc = OptimizedGc::new(4, 10_000, WriteBarrierType::Atomic);

        b.iter(|| {
            let mut objects: Vec<TestObject> = Vec::new();

            // Create objects with varying lifetimes
            for i in 0..10 {
                let batch: Vec<TestObject> = (0..1_000)
                    .map(|j| TestObject::new((i * 1000 + j) as u64, 256))
                    .collect();
                objects.extend(batch);

                // Minor collection promotes surviving objects
                let bytes = objects.iter().map(|o| o.size).sum::<usize>() as u64;
                black_box(gc.collect_minor(bytes).unwrap());
            }

            // Final major collection
            let bytes = objects.iter().map(|o| o.size).sum::<usize>() as u64;
            black_box(gc.collect_major(bytes).unwrap());

            let stats = gc.get_stats();
            black_box(stats);
        });
    });

    // Mixed workload
    group.bench_function("mixed_gen_workload", |b| {
        let gc = OptimizedGc::new(4, 10_000, WriteBarrierType::Atomic);

        b.iter(|| {
            let mut long_lived: Vec<TestObject> = Vec::new();

            for i in 0..100 {
                // Create some long-lived objects (10%)
                if i % 10 == 0 {
                    long_lived.push(TestObject::new(i as u64, 512));
                }

                // Create short-lived objects (90%)
                let _short_lived: Vec<TestObject> = (0..100)
                    .map(|j| TestObject::new((i * 100 + j) as u64, 64))
                    .collect();

                // Periodic minor collection
                if i % 10 == 0 {
                    let bytes = long_lived.iter().map(|o| o.size).sum::<usize>() as u64;
                    black_box(gc.collect_minor(bytes).unwrap());
                }
            }

            // Major collection
            let bytes = long_lived.iter().map(|o| o.size).sum::<usize>() as u64;
            black_box(gc.collect_major(bytes).unwrap());

            let stats = gc.get_stats();
            black_box(stats);
        });
    });

    group.finish();
}

/// Benchmark write barrier overhead
fn bench_write_barrier_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("write_barrier_overhead");

    // No write barrier (baseline)
    group.bench_function("no_barrier_baseline", |b| {
        b.iter(|| {
            let mut data: Vec<u64> = vec![0; 10_000];
            for i in 0..10_000 {
                data[i] = i as u64;
                black_box(&mut data);
            }
        });
    });

    // Lock-free write barrier
    group.bench_function("lock_free_barrier", |b| {
        let barrier = LockFreeWriteBarrier::new();

        b.iter(|| {
            for i in 0..10_000 {
                barrier.record_write(i as u64);
                black_box(&barrier);
            }
        });
    });

    // Write barrier in tight loop
    group.bench_function("barrier_tight_loop", |b| {
        let barrier = Arc::new(LockFreeWriteBarrier::new());

        b.iter(|| {
            for i in 0..100_000 {
                barrier.record_write(i as u64);
            }
            black_box(barrier.get_dirty_set());
        });
    });

    // Concurrent write barriers
    group.bench_function("concurrent_barriers", |b| {
        let barrier = Arc::new(LockFreeWriteBarrier::new());

        b.iter(|| {
            use rayon::prelude::*;
            (0..8).into_par_iter().for_each(|thread_id| {
                let start = thread_id * 10_000;
                for i in start..start + 10_000 {
                    barrier.record_write(i as u64);
                }
            });
            black_box(barrier.overhead_us());
        });
    });

    group.finish();
}

/// Benchmark parallel marking performance
fn bench_parallel_marking(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_marking");

    for &worker_count in WORKER_COUNTS {
        group.bench_with_input(
            BenchmarkId::new("mark_throughput", worker_count),
            &worker_count,
            |b, &worker_count| {
                b.iter(|| {
                    let marker = ParallelMarker::new(worker_count);
                    // Distribute work across workers
                    for i in 0..100_000 {
                        marker.mark(i, (i as usize) % worker_count);
                    }
                    let marked = black_box(marker.process_marks());
                    black_box(marked);
                });
            },
        );
    }

    // Work stealing efficiency
    group.bench_function("work_stealing_imbalance", |b| {
        b.iter(|| {
            let marker = ParallelMarker::new(4);
            // Create imbalanced workload (all in worker 0)
            for i in 0..10_000 {
                marker.mark(i, 0);
            }
            let marked = black_box(marker.process_marks());
            black_box(marked);
        });
    });

    group.finish();
}

/// Benchmark adaptive quota management
fn bench_adaptive_quota(c: &mut Criterion) {
    let mut group = c.benchmark_group("adaptive_quota");

    for &target_pause in PAUSE_TARGETS {
        group.bench_with_input(
            BenchmarkId::new("quota_adjustment", target_pause),
            &target_pause,
            |b, &target_pause| {
                b.iter_batched(
                    || AdaptiveQuota::new(target_pause),
                    |quota| {
                        // Simulate varying pause times
                        for pause in &[500, 1500, 3000, 2000, 8000, 4000, 12000, 6000] {
                            quota.record_pause(*pause);
                        }
                        black_box(quota.get_quota());
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    // Quota convergence
    group.bench_function("quota_convergence", |b| {
        let quota = AdaptiveQuota::new(10_000);

        b.iter(|| {
            // Simulate system converging to target
            for i in 0..100 {
                let pause = 10_000 + ((i as i64 - 50) * 100).abs() as u64;
                quota.record_pause(pause);
            }
            black_box(quota.get_quota());
        });
    });

    group.finish();
}

/// Benchmark GC statistics collection overhead
fn bench_gc_stats_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc_stats_overhead");

    group.bench_function("stats_collection", |b| {
        let gc = OptimizedGc::new(4, 10_000, WriteBarrierType::Atomic);

        b.iter(|| {
            // Perform some operations
            for _ in 0..1000 {
                gc.record_write(0x1000);
            }
            gc.collect_minor(64000).unwrap();

            // Collect stats
            let stats = black_box(gc.get_stats());
            black_box(stats);
        });
    });

    group.finish();
}

/// Benchmark GC under memory pressure
fn bench_gc_memory_pressure(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc_memory_pressure");

    group.bench_function("low_pressure", |b| {
        let gc = OptimizedGc::new(4, 10_000, WriteBarrierType::Atomic);

        b.iter(|| {
            // Low allocation rate
            for i in 0..100 {
                let obj = TestObject::new(i, 64);
                black_box(obj);

                if i % 20 == 0 {
                    gc.collect_minor(1280).unwrap();
                }
            }
        });
    });

    group.bench_function("medium_pressure", |b| {
        let gc = OptimizedGc::new(4, 10_000, WriteBarrierType::Atomic);

        b.iter(|| {
            // Medium allocation rate
            for i in 0..1000 {
                let obj = TestObject::new(i, 256);
                black_box(obj);

                if i % 100 == 0 {
                    gc.collect_minor(25600).unwrap();
                }
            }
        });
    });

    group.bench_function("high_pressure", |b| {
        let gc = OptimizedGc::new(4, 10_000, WriteBarrierType::Atomic);

        b.iter(|| {
            // High allocation rate
            for i in 0..10_000 {
                let obj = TestObject::new(i, 1024);
                black_box(obj);

                if i % 1000 == 0 {
                    gc.collect_major(1024000).unwrap();
                }
            }
        });
    });

    group.finish();
}

/// Benchmark collection frequency impact
fn bench_collection_frequency(c: &mut Criterion) {
    let mut group = c.benchmark_group("collection_frequency");

    group.bench_function("frequent_small_collections", |b| {
        let gc = OptimizedGc::new(4, 10_000, WriteBarrierType::Atomic);

        b.iter(|| {
            for i in 0..10_000 {
                let obj = TestObject::new(i, 128);
                black_box(obj);

                // Collect every 100 objects
                if i % 100 == 0 {
                    gc.collect_minor(12800).unwrap();
                }
            }
        });
    });

    group.bench_function("infrequent_large_collections", |b| {
        let gc = OptimizedGc::new(4, 10_000, WriteBarrierType::Atomic);

        b.iter(|| {
            for i in 0..10_000 {
                let obj = TestObject::new(i, 128);
                black_box(obj);

                // Collect every 5000 objects
                if i % 5000 == 0 {
                    gc.collect_major(640000).unwrap();
                }
            }
        });
    });

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(10))
        .sample_size(100);
    targets =
        bench_gc_pause_time,
        bench_gc_pause_time_major,
        bench_gc_throughput,
        bench_memory_reclamation,
        bench_gc_generational,
        bench_write_barrier_overhead,
        bench_parallel_marking,
        bench_adaptive_quota,
        bench_gc_stats_overhead,
        bench_gc_memory_pressure,
        bench_collection_frequency
}

criterion_main!(benches);
