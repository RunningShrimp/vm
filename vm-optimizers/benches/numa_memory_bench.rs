//! NUMA Memory Benchmarks
//!
//! Comprehensive benchmarks for NUMA-aware memory operations including:
//! - Local vs remote memory access latency
//! - NUMA-aware allocation benefits
//! - Cross-socket memory bandwidth
//! - TLB translation performance
//! - Prefetching effectiveness
//! - Page table traversal
//! - Memory access patterns

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use rand;
use std::time::Duration;
use vm_optimizers::memory::{
    AsyncPrefetchingTlb, MemoryOptimizer, NumaAllocator, NumaConfig, ParallelPageTable,
};

// Test parameters
const NODE_COUNTS: &[usize] = &[2, 4, 8];
const MEMORY_SIZES: &[usize] = &[1024, 4096, 16384, 65536, 262144]; // 1KB to 256KB
const ACCESS_COUNTS: &[usize] = &[100, 1_000, 10_000, 100_000];
const TLB_SIZES: &[usize] = &[64, 256, 1024, 4096];

/// Benchmark local vs remote memory access latency
fn bench_local_remote_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("numa_local_remote_latency");

    for &node_count in NODE_COUNTS {
        let config = NumaConfig {
            num_nodes: node_count,
            mem_per_node: 1024 * 1024 * 1024, // 1GB per node
        };

        // Local allocation (on node 0)
        group.bench_with_input(
            BenchmarkId::new("local_alloc", node_count),
            &config,
            |b, config| {
                let allocator = NumaAllocator::new(*config);

                b.iter(|| {
                    // Simulate local allocation
                    let size = black_box(4096);
                    let addr = allocator.allocate(size).unwrap();

                    // Extract node from address (upper bits)
                    let _node = (addr >> 48) as usize;
                    black_box(addr);
                });
            },
        );

        // Remote allocation simulation
        group.bench_with_input(
            BenchmarkId::new("remote_access_simulation", node_count),
            &config,
            |b, config| {
                let allocator = NumaAllocator::new(*config);
                // Pre-allocate on different nodes
                let mut addrs = Vec::new();
                for _i in 0..node_count {
                    let addr = allocator.allocate(4096).unwrap();
                    addrs.push(addr);
                }

                b.iter(|| {
                    // Access all nodes (some will be remote)
                    for &addr in &addrs {
                        black_box(addr);
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark NUMA-aware allocation benefits
fn bench_numa_allocation_benefits(c: &mut Criterion) {
    let mut group = c.benchmark_group("numa_allocation_benefits");

    for &mem_size in MEMORY_SIZES {
        group.throughput(Throughput::Bytes(mem_size as u64));

        // NUMA-aware allocation
        group.bench_with_input(
            BenchmarkId::new("numa_aware", mem_size),
            &mem_size,
            |b, &mem_size| {
                let config = NumaConfig {
                    num_nodes: 4,
                    mem_per_node: 1024 * 1024 * 1024,
                };
                let allocator = NumaAllocator::new(config);

                b.iter(|| {
                    let addr = allocator.allocate(mem_size).unwrap();
                    black_box(addr);
                });
            },
        );

        // Uniform allocation (baseline)
        group.bench_with_input(
            BenchmarkId::new("uniform_baseline", mem_size),
            &mem_size,
            |b, &_mem_size| {
                b.iter(|| {
                    // Simple allocation without NUMA awareness
                    let data = vec![0u8; mem_size];
                    black_box(data);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark cross-socket memory bandwidth
fn bench_cross_socket_bandwidth(c: &mut Criterion) {
    let mut group = c.benchmark_group("cross_socket_bandwidth");
    group.sample_size(20);

    for &mem_size in MEMORY_SIZES {
        group.throughput(Throughput::Bytes(mem_size as u64));

        group.bench_with_input(
            BenchmarkId::new("sequential_access", mem_size),
            &mem_size,
            |b, &mem_size| {
                let config = NumaConfig {
                    num_nodes: 2,
                    mem_per_node: 1024 * 1024 * 1024,
                };
                let allocator = NumaAllocator::new(config);

                // Allocate on both nodes
                let addr1 = allocator.allocate(mem_size).unwrap();
                let addr2 = allocator.allocate(mem_size).unwrap();

                b.iter(|| {
                    // Sequential access pattern
                    black_box(addr1);
                    black_box(addr2);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("random_access", mem_size),
            &mem_size,
            |b, &mem_size| {
                let config = NumaConfig {
                    num_nodes: 2,
                    mem_per_node: 1024 * 1024 * 1024,
                };
                let allocator = NumaAllocator::new(config);

                // Allocate multiple regions
                let addrs: Vec<u64> = (0..10)
                    .map(|_| allocator.allocate(mem_size / 10).unwrap())
                    .collect();

                b.iter(|| {
                    // Random access pattern
                    for i in 0..addrs.len() {
                        let idx = (i * 3) % addrs.len();
                        black_box(addrs[idx]);
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark TLB translation performance
fn bench_tlb_translation(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_translation");

    for &access_count in ACCESS_COUNTS {
        group.throughput(Throughput::Elements(access_count as u64));

        // Cold cache (all misses)
        group.bench_with_input(
            BenchmarkId::new("cold_cache", access_count),
            &access_count,
            |b, &access_count| {
                let tlb = AsyncPrefetchingTlb::new(false);

                b.iter(|| {
                    for i in 0..access_count {
                        let addr = 0x1000 + (i as u64) * 4096;
                        black_box(tlb.translate(addr).unwrap());
                    }
                });
            },
        );

        // Warm cache (mostly hits)
        group.bench_with_input(
            BenchmarkId::new("warm_cache", access_count),
            &access_count,
            |b, &access_count| {
                let tlb = AsyncPrefetchingTlb::new(false);

                // Warm up cache
                for i in 0..access_count {
                    let addr = 0x1000 + (i as u64) * 4096;
                    tlb.translate(addr).unwrap();
                }

                b.iter(|| {
                    for i in 0..access_count {
                        let addr = 0x1000 + (i as u64) * 4096;
                        black_box(tlb.translate(addr).unwrap());
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark TLB with prefetching
fn bench_tlb_prefetching(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_prefetching");

    // Without prefetching
    group.bench_function("no_prefetch", |b| {
        let tlb = AsyncPrefetchingTlb::new(false);

        b.iter(|| {
            // Sequential access pattern
            for i in 0..1000 {
                let addr = 0x1000 + (i as u64) * 4096;
                black_box(tlb.translate(addr).unwrap());
            }
        });
    });

    // With prefetching
    group.bench_function("with_prefetch", |b| {
        let tlb = AsyncPrefetchingTlb::new(true);

        b.iter(|| {
            // Sequential access pattern
            for i in 0..1000 {
                let addr = 0x1000 + (i as u64) * 4096;
                black_box(tlb.translate(addr).unwrap());
            }
        });
    });

    // Prefetch effectiveness
    group.bench_function("prefetch_effectiveness", |b| {
        let tlb = AsyncPrefetchingTlb::new(true);

        b.iter(|| {
            // Access pattern that benefits from prefetching
            for i in 0..100 {
                let addr = 0x1000 + (i as u64) * 4096;
                tlb.translate(addr).unwrap();

                // Process prefetch queue
                tlb.process_prefetch();
            }

            let stats = tlb.get_stats();
            black_box(stats);
        });
    });

    group.finish();
}

/// Benchmark batch translation
fn bench_batch_translation(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_translation");

    for &batch_size in &[10, 100, 1000] {
        group.throughput(Throughput::Elements(batch_size as u64));

        group.bench_with_input(
            BenchmarkId::new("batch_translate", batch_size),
            &batch_size,
            |b, &batch_size| {
                let tlb = AsyncPrefetchingTlb::new(false);
                let addrs: Vec<u64> = (0..batch_size)
                    .map(|i| 0x1000 + (i as u64) * 4096)
                    .collect();

                b.iter(|| {
                    black_box(tlb.translate_batch(&addrs).unwrap());
                });
            },
        );

        // Compare with individual translations
        group.bench_with_input(
            BenchmarkId::new("individual_translate", batch_size),
            &batch_size,
            |b, &batch_size| {
                let tlb = AsyncPrefetchingTlb::new(false);
                let addrs: Vec<u64> = (0..batch_size)
                    .map(|i| 0x1000 + (i as u64) * 4096)
                    .collect();

                b.iter(|| {
                    for &addr in &addrs {
                        black_box(tlb.translate(addr).unwrap());
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark page table traversal
fn bench_page_table_traversal(c: &mut Criterion) {
    let mut group = c.benchmark_group("page_table_traversal");

    for &page_count in &[100, 1000, 10000] {
        group.throughput(Throughput::Elements(page_count as u64));

        group.bench_with_input(
            BenchmarkId::new("single_lookup", page_count),
            &page_count,
            |b, &page_count| {
                let pt = ParallelPageTable::new();

                // Populate page table
                for i in 0..page_count {
                    let vaddr = (i as u64) * 4096;
                    let paddr = 0x1000_0000 + vaddr;
                    pt.insert(vaddr, paddr);
                }

                b.iter(|| {
                    for i in 0..page_count {
                        let vaddr = (i as u64) * 4096;
                        black_box(pt.lookup(vaddr));
                    }
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("batch_lookup", page_count),
            &page_count,
            |b, &page_count| {
                let pt = ParallelPageTable::new();

                // Populate page table
                for i in 0..page_count {
                    let vaddr = (i as u64) * 4096;
                    let paddr = 0x1000_0000 + vaddr;
                    pt.insert(vaddr, paddr);
                }

                let vaddrs: Vec<u64> = (0..page_count).map(|i| (i as u64) * 4096).collect();

                b.iter(|| {
                    black_box(pt.batch_lookup(&vaddrs));
                });
            },
        );
    }

    group.finish();
}

/// Benchmark memory access patterns
fn bench_access_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("access_patterns");

    // Sequential pattern
    group.bench_function("sequential", |b| {
        let tlb = AsyncPrefetchingTlb::new(true);

        b.iter(|| {
            for i in 0..1000 {
                let addr = 0x1000 + (i as u64) * 4096;
                black_box(tlb.translate(addr).unwrap());
            }
        });
    });

    // Strided pattern
    group.bench_function("strided", |b| {
        let tlb = AsyncPrefetchingTlb::new(true);

        b.iter(|| {
            for i in 0..1000 {
                let addr = 0x1000 + (i as u64) * 16384; // 4-page stride
                black_box(tlb.translate(addr).unwrap());
            }
        });
    });

    // Random pattern
    group.bench_function("random", |b| {
        let tlb = AsyncPrefetchingTlb::new(true);
        let mut rng = rand::thread_rng();

        b.iter(|| {
            for _ in 0..1000 {
                let random_val: u64 = rand::random();
                let addr = 0x1000 + (random_val % 1_000_000) * 4096;
                black_box(tlb.translate(addr).unwrap());
            }
        });
    });

    group.finish();
}

/// Benchmark NUMA memory rebalancing
fn bench_numa_rebalancing(c: &mut Criterion) {
    let mut group = c.benchmark_group("numa_rebalancing");

    for &node_count in NODE_COUNTS {
        group.bench_with_input(
            BenchmarkId::new("rebalance", node_count),
            &node_count,
            |b, &node_count| {
                let config = NumaConfig {
                    num_nodes: node_count,
                    mem_per_node: 1024 * 1024 * 1024,
                };
                let allocator = NumaAllocator::new(config);

                // Create imbalance
                for _i in 0..node_count * 100 {
                    allocator.allocate(1024 * 1024).unwrap();
                }

                b.iter(|| {
                    let moved = black_box(allocator.rebalance());
                    black_box(moved);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark integrated memory optimizer
fn bench_memory_optimizer(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_optimizer");

    for &access_count in ACCESS_COUNTS {
        group.throughput(Throughput::Elements(access_count as u64));

        group.bench_with_input(
            BenchmarkId::new("integrated_optimized", access_count),
            &access_count,
            |b, &access_count| {
                let config = NumaConfig {
                    num_nodes: 4,
                    mem_per_node: 1024 * 1024 * 1024,
                };
                let optimizer = MemoryOptimizer::new(config);

                // Pre-allocate and translate
                let addrs: Vec<u64> = (0..access_count)
                    .map(|i| 0x1000 + (i as u64) * 4096)
                    .collect();

                b.iter(|| {
                    // Translate with prefetching
                    for &addr in &addrs {
                        black_box(optimizer.translate(addr).unwrap());
                    }

                    // Process background prefetching
                    optimizer.process_prefetch();

                    let stats = optimizer.get_tlb_stats();
                    black_box(stats);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark TLB statistics collection overhead
fn bench_tlb_stats_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_stats_overhead");

    group.bench_function("stats_collection", |b| {
        let tlb = AsyncPrefetchingTlb::new(true);

        b.iter(|| {
            for i in 0..1000 {
                let addr = 0x1000 + (i as u64) * 4096;
                tlb.translate(addr).unwrap();
            }

            let stats = black_box(tlb.get_stats());
            black_box(stats);
        });
    });

    group.finish();
}

/// Benchmark NUMA allocation scalability
fn bench_numa_scalability(c: &mut Criterion) {
    let mut group = c.benchmark_group("numa_scalability");

    for &node_count in NODE_COUNTS {
        group.bench_with_input(
            BenchmarkId::new("allocation_scalability", node_count),
            &node_count,
            |b, &node_count| {
                let config = NumaConfig {
                    num_nodes: node_count,
                    mem_per_node: 1024 * 1024 * 1024,
                };
                let allocator = NumaAllocator::new(config);

                b.iter(|| {
                    // Allocate across all nodes
                    for _ in 0..1000 {
                        let addr = allocator.allocate(4096).unwrap();
                        black_box(addr);
                    }

                    // Get stats
                    let stats = allocator.get_stats();
                    black_box(stats);
                });
            },
        );
    }

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(10))
        .sample_size(100);
    targets =
        bench_local_remote_latency,
        bench_numa_allocation_benefits,
        bench_cross_socket_bandwidth,
        bench_tlb_translation,
        bench_tlb_prefetching,
        bench_batch_translation,
        bench_page_table_traversal,
        bench_access_patterns,
        bench_numa_rebalancing,
        bench_memory_optimizer,
        bench_tlb_stats_overhead,
        bench_numa_scalability
}

criterion_main!(benches);
