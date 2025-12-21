//! TLB刷新基准测试
//!
//! 测试TLB刷新性能

use criterion::{Criterion, criterion_group, criterion_main};
use parking_lot::RwLock;
use std::hint::black_box;
use std::sync::Arc;
use vm_core::{AccessType, GuestAddr};
use vm_mem::SoftMmu;

/// 基准测试：基础刷新 vs 高级刷新
fn bench_basic_flush(c: &mut Criterion) {
    let mut group = c.benchmark_group("basic_vs_advanced_flush");

    // 创建一个简单的MMU用于测试
    let mmu = Arc::new(RwLock::new(SoftMmu::new(64 * 1024 * 1024, false)));

    // 测试地址
    let test_addresses: Vec<u64> = (0..1000).map(|i| i * 4096).collect();

    // 基础刷新基准
    group.bench_function("basic_flush", |b| {
        b.iter(|| {
            for &addr in &test_addresses {
                let mut mmu_guard = mmu.write();
                let _ = mmu_guard.translate(GuestAddr(black_box(addr)), AccessType::Read);
            }
        })
    });

    group.finish();
}

/// 基准测试：不同刷新策略比较
fn bench_flush_strategies(c: &mut Criterion) {
    let mut group = c.benchmark_group("flush_strategies");

    // 创建一个简单的MMU用于测试
    let mmu = Arc::new(RwLock::new(SoftMmu::new(64 * 1024 * 1024, false)));

    // 测试地址
    let test_addresses: Vec<u64> = (0..1000).map(|i| i * 4096).collect();

    // 刷新基准
    group.bench_function("flush", |b| {
        b.iter(|| {
            for &addr in &test_addresses {
                let mut mmu_guard = mmu.write();
                let _ = mmu_guard.translate(GuestAddr(black_box(addr)), AccessType::Read);
            }
        })
    });

    group.finish();
}

criterion_group!(
    basic_vs_advanced_flush,
    bench_basic_flush,
    bench_flush_strategies,
);

criterion_main!(basic_vs_advanced_flush);
