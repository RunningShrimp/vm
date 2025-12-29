//! Memory Allocation Benchmarks
//!
//! Comprehensive benchmarks for memory allocation performance including:
//! - Small allocation throughput (8-64 bytes)
//! - Large allocation performance (1KB-1MB)
//! - Concurrent allocation scalability
//! - Memory pool efficiency
//! - Allocation/deallocation patterns

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use rayon::prelude::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

// Test allocation sizes
const SMALL_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256];
const LARGE_SIZES: &[usize] = &[1024, 4096, 16384, 65536, 262144, 1048576];
const THREAD_COUNTS: &[usize] = &[1, 2, 4, 8, 16];

/// Custom allocator for tracking statistics
struct TrackingAllocator {
    alloc_count: Arc<AtomicU64>,
    dealloc_count: Arc<AtomicU64>,
    total_bytes: Arc<AtomicU64>,
}

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.alloc_count.fetch_add(1, Ordering::Relaxed);
        self.total_bytes
            .fetch_add(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.dealloc_count.fetch_add(1, Ordering::Relaxed);
        self.total_bytes
            .fetch_sub(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
}

/// Benchmark small allocations (8-256 bytes)
fn bench_small_allocations(c: &mut Criterion) {
    let mut group = c.benchmark_group("small_allocations");

    for &size in SMALL_SIZES {
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(BenchmarkId::new("allocate", size), &size, |b, &size| {
            b.iter(|| {
                let data = vec![0u8; size];
                black_box(data);
            });
        });

        group.bench_with_input(
            BenchmarkId::new("alloc_dealloc", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    let data = vec![0u8; size];
                    black_box(data);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark large allocations (1KB-1MB)
fn bench_large_allocations(c: &mut Criterion) {
    let mut group = c.benchmark_group("large_allocations");
    group.sample_size(20); // Fewer samples for large allocations

    for &size in LARGE_SIZES {
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(BenchmarkId::new("allocate", size), &size, |b, &size| {
            b.iter(|| {
                let data = vec![0u8; size];
                black_box(data);
            });
        });

        group.bench_with_input(
            BenchmarkId::new("allocate_zeroed", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    let data: Vec<u8> = vec![0; size];
                    black_box(data);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark allocation throughput (allocations per second)
fn bench_allocation_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("allocation_throughput");

    group.bench_function("alloc_8_bytes_per_second", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                black_box(Box::new([0u8; 8]));
            }
        });
    });

    group.bench_function("alloc_64_bytes_per_second", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                black_box(Box::new([0u8; 64]));
            }
        });
    });

    group.bench_function("alloc_256_bytes_per_second", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                black_box(Box::new([0u8; 256]));
            }
        });
    });

    group.finish();
}

/// Benchmark deallocation speed
fn bench_deallocation_speed(c: &mut Criterion) {
    let mut group = c.benchmark_group("deallocation_speed");

    group.bench_function("dealloc_8_bytes", |b| {
        b.iter_batched(
            || vec![Box::new([0u8; 8]); 1000],
            |mut allocs| {
                allocs.clear();
                black_box(allocs);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("dealloc_64_bytes", |b| {
        b.iter_batched(
            || vec![Box::new([0u8; 64]); 1000],
            |mut allocs| {
                allocs.clear();
                black_box(allocs);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("dealloc_256_bytes", |b| {
        b.iter_batched(
            || vec![Box::new([0u8; 256]); 1000],
            |mut allocs| {
                allocs.clear();
                black_box(allocs);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// Benchmark concurrent allocations
fn bench_concurrent_allocations(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_allocations");

    for &thread_count in THREAD_COUNTS {
        group.bench_with_input(
            BenchmarkId::new("parallel_alloc_256_bytes", thread_count),
            &thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    (0..thread_count).into_par_iter().for_each(|_| {
                        for _ in 0..100 {
                            black_box(Box::new([0u8; 256]));
                        }
                    });
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("parallel_alloc_dealloc_256_bytes", thread_count),
            &thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    (0..thread_count).into_par_iter().for_each(|_| {
                        for _ in 0..100 {
                            let data = vec![0u8; 256];
                            black_box(data);
                        }
                    });
                });
            },
        );
    }

    group.finish();
}

/// Benchmark memory pool efficiency
fn bench_memory_pool_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_pool_efficiency");

    // Simulate memory pool behavior with reuse
    group.bench_function("pool_reuse_8_bytes", |b| {
        let mut pool: Vec<Box<[u8; 8]>> = Vec::with_capacity(1000);

        b.iter(|| {
            // Allocate
            for _ in 0..1000 {
                pool.push(Box::new([0u8; 8]));
            }
            // Deallocate all
            pool.clear();
            black_box(&pool);
        });
    });

    group.bench_function("pool_reuse_64_bytes", |b| {
        let mut pool: Vec<Box<[u8; 64]>> = Vec::with_capacity(1000);

        b.iter(|| {
            for _ in 0..1000 {
                pool.push(Box::new([0u8; 64]));
            }
            pool.clear();
            black_box(&pool);
        });
    });

    group.bench_function("pool_reuse_256_bytes", |b| {
        let mut pool: Vec<Box<[u8; 256]>> = Vec::with_capacity(1000);

        b.iter(|| {
            for _ in 0..1000 {
                pool.push(Box::new([0u8; 256]));
            }
            pool.clear();
            black_box(&pool);
        });
    });

    group.finish();
}

/// Benchmark allocation patterns
fn bench_allocation_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("allocation_patterns");

    // Sequential allocation
    group.bench_function("sequential_alloc_256", |b| {
        b.iter(|| {
            let mut allocs = Vec::with_capacity(100);
            for i in 0..100 {
                allocs.push(Box::new([i as u8; 256]));
            }
            black_box(allocs);
        });
    });

    // Random allocation sizes
    group.bench_function("random_alloc_sizes", |b| {
        b.iter(|| {
            let mut allocs = Vec::with_capacity(100);
            for i in 0..100 {
                let size = 8 + (i % 8) * 32; // Varying sizes from 8 to 256
                allocs.push(vec![0u8; size]);
            }
            black_box(allocs);
        });
    });

    // Burst allocation pattern
    group.bench_function("burst_alloc_256", |b| {
        b.iter(|| {
            let allocs: Vec<Vec<u8>> = (0..10)
                .flat_map(|_| (0..100).map(|_| vec![0u8; 256]).collect::<Vec<Vec<u8>>>())
                .collect();
            black_box(allocs);
        });
    });

    // Interleaved alloc/dealloc
    group.bench_function("interleaved_alloc_dealloc", |b| {
        b.iter(|| {
            let mut allocs = Vec::with_capacity(100);
            for i in 0..200 {
                if i % 2 == 0 {
                    allocs.push(vec![0u8; 256]);
                } else if !allocs.is_empty() {
                    allocs.remove(0);
                }
            }
            black_box(allocs);
        });
    });

    group.finish();
}

/// Benchmark fragmentation
fn bench_fragmentation(c: &mut Criterion) {
    let mut group = c.benchmark_group("fragmentation");

    group.bench_function("fragmented_allocations", |b| {
        b.iter(|| {
            let mut allocs: Vec<Box<[u8]>> = Vec::new();

            // Allocate various sizes
            for size in [8, 16, 32, 64, 128, 256, 512, 1024] {
                match size {
                    8 => allocs.push(Box::new([0u8; 8])),
                    16 => allocs.push(Box::new([0u8; 16])),
                    32 => allocs.push(Box::new([0u8; 32])),
                    64 => allocs.push(Box::new([0u8; 64])),
                    128 => allocs.push(Box::new([0u8; 128])),
                    256 => allocs.push(Box::new([0u8; 256])),
                    512 => allocs.push(Box::new([0u8; 512])),
                    1024 => allocs.push(Box::new([0u8; 1024])),
                    _ => unreachable!(),
                }
            }

            // Free every other allocation
            let mut i = 0;
            allocs.retain(|_| {
                i += 1;
                i % 2 == 0
            });

            // Allocate more
            for size in [16, 32, 64, 128] {
                match size {
                    16 => allocs.push(Box::new([0u8; 16])),
                    32 => allocs.push(Box::new([0u8; 32])),
                    64 => allocs.push(Box::new([0u8; 64])),
                    128 => allocs.push(Box::new([0u8; 128])),
                    _ => unreachable!(),
                }
            }

            black_box(allocs);
        });
    });

    group.finish();
}

/// Benchmark allocation latency (time to allocate)
fn bench_allocation_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("allocation_latency");

    group.bench_function("latency_8_bytes", |b| {
        b.iter(|| {
            let start = std::time::Instant::now();
            let ptr = Box::new([0u8; 8]);
            let elapsed = start.elapsed();
            black_box((ptr, elapsed));
        });
    });

    group.bench_function("latency_256_bytes", |b| {
        b.iter(|| {
            let start = std::time::Instant::now();
            let ptr = Box::new([0u8; 256]);
            let elapsed = start.elapsed();
            black_box((ptr, elapsed));
        });
    });

    group.bench_function("latency_4096_bytes", |b| {
        b.iter(|| {
            let start = std::time::Instant::now();
            let ptr = Box::new([0u8; 4096]);
            let elapsed = start.elapsed();
            black_box((ptr, elapsed));
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
        bench_small_allocations,
        bench_large_allocations,
        bench_allocation_throughput,
        bench_deallocation_speed,
        bench_concurrent_allocations,
        bench_memory_pool_efficiency,
        bench_allocation_patterns,
        bench_fragmentation,
        bench_allocation_latency
}

criterion_main!(benches);
