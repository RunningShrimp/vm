//! Concurrent MMU Performance Benchmarks
//!
//! This benchmark compares the performance of different MMU implementations
//! under concurrent access patterns:
//!
//! - **SoftMMU**: Baseline mutex-based MMU
//! - **LockFreeMMU**: Lock-free RCU-style MMU
//! - **ShardedMMU**: Sharded MMU with per-shard locks
//!
//! Expected results:
//! - LockFreeMMU: 3-5x improvement over mutex MMU
//! - ShardedMMU: 2-3x improvement over mutex MMU

use std::sync::Arc;
use std::thread;

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use vm_mem::{GuestAddr, LockFreeMMU, ShardedMMU, SoftMmu};

/// Benchmark single-threaded translation performance
fn bench_single_threaded_translation(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_threaded_translation");

    // Different address counts
    for addr_count in [100, 1000, 10000].iter() {
        let size = *addr_count * 0x1000;

        group.throughput(Throughput::Elements(*addr_count as u64));

        // SoftMMU baseline
        group.bench_with_input(
            BenchmarkId::new("SoftMMU", addr_count),
            addr_count,
            |b, &_addr_count| {
                let mmu = SoftMmu::new(size, false);

                b.iter(|| {
                    for i in 0..*addr_count {
                        let addr = GuestAddr((i * 0x1000) as u64);
                        black_box(mmu.translate(addr, vm_core::AccessType::Read).ok());
                    }
                });
            },
        );

        // LockFreeMMU
        group.bench_with_input(
            BenchmarkId::new("LockFreeMMU", addr_count),
            addr_count,
            |b, &_addr_count| {
                let mmu = LockFreeMMU::new(size, false);

                b.iter(|| {
                    for i in 0..*addr_count {
                        let addr = GuestAddr((i * 0x1000) as u64);
                        black_box(mmu.translate(addr).ok());
                    }
                });
            },
        );

        // ShardedMMU
        group.bench_with_input(
            BenchmarkId::new("ShardedMMU", addr_count),
            addr_count,
            |b, &_addr_count| {
                let mmu = ShardedMMU::new(size, false);

                b.iter(|| {
                    for i in 0..*addr_count {
                        let addr = GuestAddr((i * 0x1000) as u64);
                        black_box(mmu.translate(addr).ok());
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark concurrent translation with multiple threads
fn bench_concurrent_translation(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_translation");

    // Different thread counts
    for thread_count in [2, 4, 8, 16].iter() {
        let addr_count = 1000;
        let size = addr_count * 0x1000 * *thread_count;

        group.throughput(Throughput::Elements((addr_count * thread_count) as u64));

        // SoftMMU baseline (NOT thread-safe, but included for comparison)
        group.bench_with_input(
            BenchmarkId::new("SoftMMU", thread_count),
            &thread_count,
            |b, &_thread_count| {
                let mmu = Arc::new(SoftMmu::new(size, false));

                b.iter(|| {
                    let barrier = Arc::new(std::sync::Barrier::new(*thread_count));
                    let mut handles = vec![];

                    for thread_id in 0..*thread_count {
                        let mmu_clone = Arc::clone(&mmu);
                        let barrier_clone = Arc::clone(&barrier);

                        handles.push(thread::spawn(move || {
                            barrier_clone.wait();

                            for i in 0..addr_count {
                                let addr =
                                    GuestAddr(((thread_id * addr_count + i) * 0x1000) as u64);
                                black_box(
                                    mmu_clone.translate(addr, vm_core::AccessType::Read).ok(),
                                );
                            }
                        }));
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );

        // LockFreeMMU
        group.bench_with_input(
            BenchmarkId::new("LockFreeMMU", thread_count),
            &thread_count,
            |b, &_thread_count| {
                let mmu = Arc::new(LockFreeMMU::new(size, false));

                b.iter(|| {
                    let barrier = Arc::new(std::sync::Barrier::new(*thread_count));
                    let mut handles = vec![];

                    for thread_id in 0..*thread_count {
                        let mmu_clone = Arc::clone(&mmu);
                        let barrier_clone = Arc::clone(&barrier);

                        handles.push(thread::spawn(move || {
                            barrier_clone.wait();

                            for i in 0..addr_count {
                                let addr =
                                    GuestAddr(((thread_id * addr_count + i) * 0x1000) as u64);
                                black_box(mmu_clone.translate(addr).ok());
                            }
                        }));
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );

        // ShardedMMU
        group.bench_with_input(
            BenchmarkId::new("ShardedMMU", thread_count),
            &thread_count,
            |b, &_thread_count| {
                let mmu = Arc::new(ShardedMMU::new(size, false));

                b.iter(|| {
                    let barrier = Arc::new(std::sync::Barrier::new(*thread_count));
                    let mut handles = vec![];

                    for thread_id in 0..*thread_count {
                        let mmu_clone = Arc::clone(&mmu);
                        let barrier_clone = Arc::clone(&barrier);

                        handles.push(thread::spawn(move || {
                            barrier_clone.wait();

                            for i in 0..addr_count {
                                let addr =
                                    GuestAddr(((thread_id * addr_count + i) * 0x1000) as u64);
                                black_box(mmu_clone.translate(addr).ok());
                            }
                        }));
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark concurrent update operations
fn bench_concurrent_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_updates");

    for thread_count in [2, 4, 8].iter() {
        let addr_count = 500;
        let size = addr_count * 0x1000 * *thread_count;

        group.throughput(Throughput::Elements((addr_count * thread_count) as u64));

        // LockFreeMMU
        group.bench_with_input(
            BenchmarkId::new("LockFreeMMU", thread_count),
            &thread_count,
            |b, &_thread_count| {
                let mmu = Arc::new(LockFreeMMU::new(size, false));

                b.iter(|| {
                    let barrier = Arc::new(std::sync::Barrier::new(*thread_count));
                    let mut handles = vec![];

                    for thread_id in 0..*thread_count {
                        let mmu_clone = Arc::clone(&mmu);
                        let barrier_clone = Arc::clone(&barrier);

                        handles.push(thread::spawn(move || {
                            barrier_clone.wait();

                            for i in 0..addr_count {
                                let addr =
                                    GuestAddr(((thread_id * addr_count + i) * 0x1000) as u64);
                                mmu_clone.update_mapping(addr, addr.0);
                            }
                        }));
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );

        // ShardedMMU
        group.bench_with_input(
            BenchmarkId::new("ShardedMMU", thread_count),
            &thread_count,
            |b, &_thread_count| {
                let mmu = Arc::new(ShardedMMU::new(size, false));

                b.iter(|| {
                    let barrier = Arc::new(std::sync::Barrier::new(*thread_count));
                    let mut handles = vec![];

                    for thread_id in 0..*thread_count {
                        let mmu_clone = Arc::clone(&mmu);
                        let barrier_clone = Arc::clone(&barrier);

                        handles.push(thread::spawn(move || {
                            barrier_clone.wait();

                            for i in 0..addr_count {
                                let addr =
                                    GuestAddr(((thread_id * addr_count + i) * 0x1000) as u64);
                                mmu_clone.update_mapping(addr, addr.0);
                            }
                        }));
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark mixed read-write workload
fn bench_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_workload");

    for thread_count in [4, 8].iter() {
        let addr_count = 1000;
        let size = addr_count * 0x1000 * *thread_count;

        group.throughput(Throughput::Elements((addr_count * thread_count * 2) as u64));

        // LockFreeMMU
        group.bench_with_input(
            BenchmarkId::new("LockFreeMMU", thread_count),
            &thread_count,
            |b, &_thread_count| {
                let mmu = Arc::new(LockFreeMMU::new(size, false));

                b.iter(|| {
                    let barrier = Arc::new(std::sync::Barrier::new(*thread_count));
                    let mut handles = vec![];

                    for thread_id in 0..*thread_count {
                        let mmu_clone = Arc::clone(&mmu);
                        let barrier_clone = Arc::clone(&barrier);

                        handles.push(thread::spawn(move || {
                            barrier_clone.wait();

                            for i in 0..addr_count {
                                let addr =
                                    GuestAddr(((thread_id * addr_count + i) * 0x1000) as u64);

                                // 90% reads, 10% writes
                                if i % 10 == 0 {
                                    mmu_clone.update_mapping(addr, addr.0);
                                } else {
                                    black_box(mmu_clone.translate(addr).ok());
                                }
                            }
                        }));
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );

        // ShardedMMU
        group.bench_with_input(
            BenchmarkId::new("ShardedMMU", thread_count),
            &thread_count,
            |b, &_thread_count| {
                let mmu = Arc::new(ShardedMMU::new(size, false));

                b.iter(|| {
                    let barrier = Arc::new(std::sync::Barrier::new(*thread_count));
                    let mut handles = vec![];

                    for thread_id in 0..*thread_count {
                        let mmu_clone = Arc::clone(&mmu);
                        let barrier_clone = Arc::clone(&barrier);

                        handles.push(thread::spawn(move || {
                            barrier_clone.wait();

                            for i in 0..addr_count {
                                let addr =
                                    GuestAddr(((thread_id * addr_count + i) * 0x1000) as u64);

                                // 90% reads, 10% writes
                                if i % 10 == 0 {
                                    mmu_clone.update_mapping(addr, addr.0);
                                } else {
                                    black_box(mmu_clone.translate(addr).ok());
                                }
                            }
                        }));
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark TLB hit rate effectiveness
fn bench_tlb_hit_rate(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_hit_rate");

    let addr_count = 100;
    let iterations = 10000;

    // LockFreeMMU
    group.bench_function("LockFreeMMU", |b| {
        let mmu = LockFreeMMU::new(1024 * 1024, false);
        mmu.set_paging_mode(8);

        // Pre-populate mappings
        for i in 0..addr_count {
            let addr = GuestAddr((i * 0x1000) as u64);
            mmu.update_mapping(addr, addr.0);
        }

        b.iter(|| {
            for _ in 0..iterations {
                for i in 0..addr_count {
                    let addr = GuestAddr((i * 0x1000) as u64);
                    black_box(mmu.translate(addr).ok());
                }
            }
        });

        let stats = mmu.stats();
        println!("LockFreeMMU hit rate: {:.2}%", stats.hit_rate() * 100.0);
    });

    // ShardedMMU
    group.bench_function("ShardedMMU", |b| {
        let mmu = ShardedMMU::new(1024 * 1024, false);
        mmu.set_paging_mode(vm_mem::ShardedPagingMode::Sv39);

        // Pre-populate mappings
        for i in 0..addr_count {
            let addr = GuestAddr((i * 0x1000) as u64);
            mmu.update_mapping(addr, addr.0);
        }

        b.iter(|| {
            for _ in 0..iterations {
                for i in 0..addr_count {
                    let addr = GuestAddr((i * 0x1000) as u64);
                    black_box(mmu.translate(addr).ok());
                }
            }
        });

        let hit_rate = mmu.hit_rate();
        println!("ShardedMMU hit rate: {:.2}%", hit_rate * 100.0);
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_single_threaded_translation,
    bench_concurrent_translation,
    bench_concurrent_updates,
    bench_mixed_workload,
    bench_tlb_hit_rate
);
criterion_main!(benches);
