//! TLB Manager 性能基准测试
//!
//! 测试 Round 24 实施的 TLB 优化:
//! - FxHashMap 代替 HashMap
//! - 分支预测优化 (flags != 0 检查)
//! - 预分配哈希表容量

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use vm_core::{AccessType, GuestAddr, GuestPhysAddr};
use vm_mem::tlb::management::manager::StandardTlbManager;
use vm_mem::tlb::management::TlbManager;

/// 创建测试 TLB 条目
fn create_test_entry(addr: u64, asid: u16) -> vm_core::TlbEntry {
    vm_core::TlbEntry {
        guest_addr: GuestAddr(addr),
        phys_addr: GuestPhysAddr(addr + 0x1000_0000),
        flags: 0x7, // V | R | W
        asid,
    }
}

/// 基准测试：TLB 查找性能
fn bench_tlb_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_lookup");

    // 创建容量为 256 的 TLB
    let mut tlb = StandardTlbManager::new(256);

    // 预填充 TLB 条目 (模拟真实工作负载)
    let addrs: Vec<u64> = (0..200).map(|i| 0x1000 + i * 0x1000).collect();
    for &addr in &addrs {
        let entry = create_test_entry(addr, 0);
        tlb.update(entry);
    }

    // 测试：TLB 查找性能 (主要命中情况)
    group.bench_function("mostly_hits", |b| {
        b.iter(|| {
            for i in 0..100 {
                let addr = GuestAddr(0x1000 + (i % 200) * 0x1000);
                black_box(tlb.lookup(addr, 0, AccessType::Read));
            }
        });
    });

    // 测试：TLB 查找性能 (全部未命中情况)
    group.bench_function("all_misses", |b| {
        b.iter(|| {
            for i in 200..300 {
                let addr = GuestAddr(0x1000 + i * 0x1000);
                black_box(tlb.lookup(addr, 0, AccessType::Read));
            }
        });
    });

    // 测试：TLB 查找性能 (混合工作负载)
    group.bench_function("mixed_workload", |b| {
        b.iter(|| {
            for i in 0..100 {
                let addr = if i % 10 < 8 {
                    // 80% 命中
                    GuestAddr(0x1000 + (i % 200) * 0x1000)
                } else {
                    // 20% 未命中
                    GuestAddr(0x1000 + (200 + i) * 0x1000)
                };
                black_box(tlb.lookup(addr, 0, AccessType::Read));
            }
        });
    });

    group.finish();
}

/// 基准测试：TLB 更新性能
fn bench_tlb_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_update");

    // 测试：TLB 更新性能
    group.bench_function("update", |b| {
        b.iter(|| {
            let mut tlb = StandardTlbManager::new(256);
            for i in 0..100 {
                let entry = create_test_entry(0x1000 + i * 0x1000, 0);
                black_box(tlb.update(entry));
            }
        });
    });

    group.finish();
}

/// 基准测试：TLB 刷新性能
fn bench_tlb_flush(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_flush");

    // 测试：TLB 全部刷新性能
    group.bench_function("flush_all", |b| {
        b.iter(|| {
            let mut tlb = StandardTlbManager::new(256);

            // 预填充
            for i in 0..200 {
                let entry = create_test_entry(0x1000 + i * 0x1000, 0);
                tlb.update(entry);
            }

            // 测试刷新
            black_box(tlb.flush());
        });
    });

    // 测试：TLB ASID 刷新性能
    group.bench_function("flush_asid", |b| {
        b.iter(|| {
            let mut tlb = StandardTlbManager::new(256);

            // 预填充多个 ASID
            for i in 0..100 {
                let entry = create_test_entry(0x1000 + i * 0x1000, 0);
                tlb.update(entry);

                let entry2 = create_test_entry(0x1000 + i * 0x1000, 1);
                tlb.update(entry2);
            }

            // 测试 ASID 刷新
            black_box(tlb.flush_asid(0));
        });
    });

    group.finish();
}

/// 基准测试：不同容量 TLB 性能
fn bench_tlb_capacities(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_capacity");

    for capacity in [64, 128, 256, 512] {
        let capacity_str = format!("capacity_{}", capacity);

        group.bench_function(capacity_str, |b| {
            b.iter(|| {
                let mut tlb = StandardTlbManager::new(capacity);

                // 填充到 80% 容量
                for i in 0..(capacity * 8 / 10) {
                    let entry = create_test_entry(0x1000 + i as u64 * 0x1000, 0);
                    tlb.update(entry);
                }

                // 执行 100 次查找
                for i in 0..100 {
                    let addr = GuestAddr(0x1000 + (i % (capacity * 8 / 10)) as u64 * 0x1000);
                    black_box(tlb.lookup(addr, 0, AccessType::Read));
                }
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_tlb_lookup,
    bench_tlb_update,
    bench_tlb_flush,
    bench_tlb_capacities
);

criterion_main!(benches);
