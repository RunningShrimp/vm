//! PGO JIT编译器性能基准测试
//!
//! 测试PGO集成的性能提升

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use vm_engine::jit::pgo_integration::{JitWithPgo, OptimizationLevel};

fn bench_pgo_cold_path(c: &mut Criterion) {
    let jit = JitWithPgo::with_default_config();
    let block_data = vec![0x01, 0x02, 0x03, 0x04];

    // 冷路径：少量执行
    for _ in 0..5 {
        jit.record_execution(1, 10);
    }

    c.bench_function("pgo_cold_path_compile", |b| {
        b.iter(|| {
            black_box(jit.compile_block_with_pgo(1, &block_data));
        });
    });
}

fn bench_pgo_hot_path(c: &mut Criterion) {
    let jit = JitWithPgo::with_default_config();
    let block_data = vec![0x01, 0x02, 0x03, 0x04];

    // 热路径：大量执行
    for _ in 0..600 {
        jit.record_execution(1, 10);
    }

    c.bench_function("pgo_hot_path_compile", |b| {
        b.iter(|| {
            black_box(jit.compile_block_with_pgo(1, &block_data));
        });
    });
}

fn bench_pgo_disabled(c: &mut Criterion) {
    let mut jit = JitWithPgo::with_default_config();
    jit.disable_pgo();

    let block_data = vec![0x01, 0x02, 0x03, 0x04];

    // 记录一些执行，但PGO禁用
    for _ in 0..600 {
        jit.record_execution(1, 10);
    }

    c.bench_function("pgo_disabled_compile", |b| {
        b.iter(|| {
            black_box(jit.compile_block_with_pgo(1, &block_data));
        });
    });
}

fn bench_optimization_levels(c: &mut Criterion) {
    let jit = JitWithPgo::with_default_config();
    let block_data = vec![0x01, 0x02, 0x03, 0x04];

    // 创建热路径
    for _ in 0..600 {
        jit.record_execution(1, 10);
    }

    let mut group = c.benchmark_group("optimization_levels");

    group.bench_function("none", |b| {
        b.iter(|| {
            // 冷路径编译
            let result = jit.compile_block_with_pgo(2, &block_data);
            assert_eq!(result.optimization_level, OptimizationLevel::None);
        });
    });

    group.bench_function("standard", |b| {
        b.iter(|| {
            // 标准编译
            let result = jit.compile_block_with_pgo(3, &block_data);
            assert_eq!(result.optimization_level, OptimizationLevel::Standard);
        });
    });

    group.bench_function("aggressive", |b| {
        b.iter(|| {
            // 热路径编译
            let result = jit.compile_block_with_pgo(1, &block_data);
            assert_eq!(result.optimization_level, OptimizationLevel::Aggressive);
        });
    });

    group.finish();
}

fn bench_profile_collection(c: &mut Criterion) {
    let jit = JitWithPgo::with_default_config();

    c.bench_function("record_execution", |b| {
        b.iter(|| {
            black_box(jit.record_execution(1, 10));
        });
    });
}

fn bench_get_stats(c: &mut Criterion) {
    let jit = JitWithPgo::with_default_config();

    // 生成一些profile数据
    for i in 1..=100 {
        for _ in 0..(i * 10) {
            jit.record_execution(i, 10);
        }
    }

    c.bench_function("get_block_profile", |b| {
        b.iter(|| {
            black_box(jit.get_block_profile(50));
        });
    });

    c.bench_function("get_stats", |b| {
        b.iter(|| {
            black_box(jit.get_stats());
        });
    });
}

criterion_group!(benches, bench_pgo_cold_path, bench_pgo_hot_path, bench_pgo_disabled, bench_optimization_levels, bench_profile_collection, bench_get_stats);
criterion_main!(benches);
