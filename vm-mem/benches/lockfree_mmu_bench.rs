//! Lock-free MMU 性能基准测试
//!
//! 测试目标：
//! - 验证并发性能提升 3-5x
//! - 测试 2/4/8/16 线程性能
//! - 对比单线程和多线程性能

use std::sync::{Arc, Barrier};
use std::thread;
use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use vm_core::{GuestAddr, MemoryAccess};
use vm_mem::LockFreeMMU;

/// 基准测试：单线程地址翻译
fn bench_single_thread_translation(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_thread_translation");

    for num_ops in [100, 1000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_ops),
            num_ops,
            |b, &num_ops| {
                b.iter(|| {
                    let mmu = LockFreeMMU::new(16 * 1024 * 1024, false);
                    for i in 0..num_ops {
                        let addr = GuestAddr(0x1000 + i * 8);
                        let _ = black_box(&mmu).read(addr, 8);
                    }
                });
            },
        );
    }

    group.finish();
}

/// 基准测试：多线程地址翻译
fn bench_multi_thread_translation(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_thread_translation");

    for num_threads in [2, 4, 8, 16].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_threads),
            num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let mmu = Arc::new(LockFreeMMU::new(16 * 1024 * 1024, false));
                    let barrier = Arc::new(Barrier::new(num_threads));
                    let mut handles = vec![];

                    for thread_id in 0..num_threads {
                        let mmu_clone = Arc::clone(&mmu);
                        let barrier_clone: Arc<Barrier> = Arc::clone(&barrier);

                        handles.push(thread::spawn(move || {
                            barrier_clone.wait();

                            for i in 0..1000 {
                                let addr = GuestAddr((thread_id * 0x10000 + i * 8) as u64);
                                let _ = mmu_clone.read(addr, 8);
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

/// 基准测试：TLB 命中率性能
fn bench_tlb_hit_rate(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_hit_rate");

    // 冷启动（TLB 未命中）
    group.bench_function("cold_cache", |b| {
        b.iter(|| {
            let mmu = LockFreeMMU::new(1024 * 1024, false);
            mmu.set_paging_mode(8);
            mmu.update_mapping(GuestAddr(0x1000), 0x2000);

            let addr = GuestAddr(0x1000);
            let _ = black_box(&mmu).read(addr, 8);
        });
    });

    // 热缓存（TLB 命中）
    group.bench_function("hot_cache", |b| {
        b.iter(|| {
            let mmu = LockFreeMMU::new(1024 * 1024, false);
            mmu.set_paging_mode(8);
            mmu.update_mapping(GuestAddr(0x1000), 0x2000);

            // 预热缓存
            for _ in 0..10 {
                let _ = mmu.read(GuestAddr(0x1000), 8);
            }

            let addr = GuestAddr(0x1000);
            let _ = black_box(&mmu).read(addr, 8);
        });
    });

    group.finish();
}

/// 基准测试：并发映射更新
fn bench_concurrent_mapping_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_mapping_updates");

    for num_threads in [2, 4, 8, 16].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_threads),
            num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let mmu = Arc::new(LockFreeMMU::new(16 * 1024 * 1024, false));
                    mmu.set_paging_mode(8);
                    let barrier = Arc::new(Barrier::new(num_threads));
                    let mut handles = vec![];

                    for thread_id in 0..num_threads {
                        let mmu_clone = Arc::clone(&mmu);
                        let barrier_clone: Arc<Barrier> = Arc::clone(&barrier);

                        handles.push(thread::spawn(move || {
                            barrier_clone.wait();

                            for i in 0..1000 {
                                let guest_addr = GuestAddr((thread_id * 0x10000 + i * 8) as u64);
                                mmu_clone.update_mapping(guest_addr, guest_addr.0);

                                let host = mmu_clone.read(guest_addr, 8);
                                assert!(host.is_ok());
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

/// 基准测试：TLB 刷新性能
fn bench_tlb_flush(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_flush");

    group.bench_function("flush_full_tlb", |b| {
        b.iter(|| {
            let mmu = LockFreeMMU::new(16 * 1024 * 1024, false);
            mmu.set_paging_mode(8);

            // 添加 1000 个映射
            for i in 0..1000 {
                mmu.update_mapping(GuestAddr(i * 0x1000), i * 0x1000);
            }

            black_box(&mmu).flush_tlb();
        });
    });

    group.bench_function("flush_single_page", |b| {
        b.iter(|| {
            let mut mmu = LockFreeMMU::new(16 * 1024 * 1024, false);
            mmu.set_paging_mode(8);
            mmu.update_mapping(GuestAddr(0x1000), 0x2000);

            use vm_core::AddressTranslator;
            black_box(&mut mmu).flush_tlb_page(GuestAddr(0x1000));
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_single_thread_translation,
    bench_multi_thread_translation,
    bench_tlb_hit_rate,
    bench_concurrent_mapping_updates,
    bench_tlb_flush
);
criterion_main!(benches);
