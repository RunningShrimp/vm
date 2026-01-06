//! 缓存优化性能基准测试
//!
//! 测试 Round 25 导出的缓存优化:
//! - SIMD 内存拷贝 (128/256/512位)
//! - 缓存行对齐
//! - 预取指令
//! - 非临时存储

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use vm_mem::optimization::{CopyStrategy, FastMemoryCopier, MemoryCopyConfig};

/// 创建测试数据
fn create_test_data(size: usize) -> Vec<u8> {
    (0..size).map(|i| (i % 256) as u8).collect()
}

/// 基准测试：不同拷贝策略性能
fn bench_copy_strategies(c: &mut Criterion) {
    let mut group = c.benchmark_group("copy_strategies");

    // 小块拷贝 (64字节)
    group.bench_function("small_64bytes", |b| {
        let src = create_test_data(64);
        let mut dst = vec![0u8; 64];
        let copier = FastMemoryCopier::with_default_config();

        b.iter(|| unsafe {
            black_box(copier.copy_memory(src.as_ptr(), dst.as_mut_ptr(), 64)).unwrap();
        });
    });

    // 中等块拷贝 (1KB)
    group.bench_function("medium_1kb", |b| {
        let src = create_test_data(1024);
        let mut dst = vec![0u8; 1024];
        let copier = FastMemoryCopier::with_default_config();

        b.iter(|| unsafe {
            black_box(copier.copy_memory(src.as_ptr(), dst.as_mut_ptr(), 1024)).unwrap();
        });
    });

    // 大块拷贝 (64KB)
    group.bench_function("large_64kb", |b| {
        let src = create_test_data(65536);
        let mut dst = vec![0u8; 65536];
        let copier = FastMemoryCopier::with_default_config();

        b.iter(|| unsafe {
            black_box(copier.copy_memory(src.as_ptr(), dst.as_mut_ptr(), 65536)).unwrap();
        });
    });

    // 超大块拷贝 (1MB)
    group.bench_function("xlarge_1mb", |b| {
        let src = create_test_data(1024 * 1024);
        let mut dst = vec![0u8; 1024 * 1024];
        let copier = FastMemoryCopier::with_default_config();

        b.iter(|| unsafe {
            black_box(copier.copy_memory(src.as_ptr(), dst.as_mut_ptr(), 1024 * 1024)).unwrap();
        });
    });

    group.finish();
}

/// 基准测试：对齐vs不对齐拷贝性能
fn bench_aligned_vs_unaligned(c: &mut Criterion) {
    let mut group = c.benchmark_group("alignment");

    let size = 4096; // 4KB

    // 对齐拷贝
    group.bench_function("aligned_4kb", |b| {
        // 使用aligned_alloc创建对齐内存
        let src = vec![0u8; size + 128];
        let mut dst = vec![0u8; size + 128];

        // 计算对齐地址
        let src_ptr = ((src.as_ptr() as usize + 63) & !63) as *const u8;
        let dst_ptr = ((dst.as_mut_ptr() as usize + 63) & !63) as *mut u8;

        // 初始化数据
        unsafe {
            std::ptr::write_bytes(src_ptr as *mut u8, 0xAB, size);
        }

        let copier = FastMemoryCopier::with_default_config();

        b.iter(|| unsafe {
            black_box(copier.copy_memory(src_ptr, dst_ptr, size)).unwrap();
        });
    });

    // 不对齐拷贝
    group.bench_function("unaligned_4kb", |b| {
        let src = create_test_data(size);
        let mut dst = vec![0u8; size];
        let copier = FastMemoryCopier::with_default_config();

        b.iter(|| unsafe {
            black_box(copier.copy_memory(src.as_ptr(), dst.as_mut_ptr(), size)).unwrap();
        });
    });

    group.finish();
}

/// 基准测试：预取效果
fn bench_prefetch_effect(c: &mut Criterion) {
    let mut group = c.benchmark_group("prefetch");

    // 启用预取
    group.bench_function("with_prefetch", |b| {
        let config = MemoryCopyConfig {
            enable_prefetch: true,
            prefetch_distance: 64,
            ..Default::default()
        };
        let copier = FastMemoryCopier::new(config);

        let src = create_test_data(16384);
        let mut dst = vec![0u8; 16384];

        b.iter(|| unsafe {
            black_box(copier.copy_memory(src.as_ptr(), dst.as_mut_ptr(), 16384)).unwrap();
        });
    });

    // 禁用预取
    group.bench_function("without_prefetch", |b| {
        let config = MemoryCopyConfig {
            enable_prefetch: false,
            ..Default::default()
        };
        let copier = FastMemoryCopier::new(config);

        let src = create_test_data(16384);
        let mut dst = vec![0u8; 16384];

        b.iter(|| unsafe {
            black_box(copier.copy_memory(src.as_ptr(), dst.as_mut_ptr(), 16384)).unwrap();
        });
    });

    group.finish();
}

/// 基准测试：非临时存储效果
fn bench_non_temporal(c: &mut Criterion) {
    let mut group = c.benchmark_group("non_temporal");

    let large_size = 256 * 1024; // 256KB

    // 使用非临时存储
    group.bench_function("non_temporal_256kb", |b| {
        let config = MemoryCopyConfig {
            use_non_temporal: true,
            non_temporal_threshold: 65536, // 64KB
            ..Default::default()
        };
        let copier = FastMemoryCopier::new(config);

        let src = create_test_data(large_size);
        let mut dst = vec![0u8; large_size];

        b.iter(|| unsafe {
            black_box(copier.copy_memory(src.as_ptr(), dst.as_mut_ptr(), large_size)).unwrap();
        });
    });

    // 使用普通存储
    group.bench_function("temporal_256kb", |b| {
        let config = MemoryCopyConfig {
            use_non_temporal: false,
            ..Default::default()
        };
        let copier = FastMemoryCopier::new(config);

        let src = create_test_data(large_size);
        let mut dst = vec![0u8; large_size];

        b.iter(|| unsafe {
            black_box(copier.copy_memory(src.as_ptr(), dst.as_mut_ptr(), large_size)).unwrap();
        });
    });

    group.finish();
}

/// 基准测试：SIMD级别性能对比
fn bench_simd_levels(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_levels");

    let size = 8192; // 8KB

    // SIMD128 (SSE2/NEON)
    group.bench_function("simd128_8kb", |b| {
        let config = MemoryCopyConfig {
            default_strategy: CopyStrategy::Simd128,
            ..Default::default()
        };
        let copier = FastMemoryCopier::new(config);

        let src = create_test_data(size);
        let mut dst = vec![0u8; size];

        b.iter(|| unsafe {
            black_box(copier.copy_memory(src.as_ptr(), dst.as_mut_ptr(), size)).unwrap();
        });
    });

    // SIMD256 (AVX2)
    group.bench_function("simd256_8kb", |b| {
        let config = MemoryCopyConfig {
            default_strategy: CopyStrategy::Simd256,
            ..Default::default()
        };
        let copier = FastMemoryCopier::new(config);

        let src = create_test_data(size);
        let mut dst = vec![0u8; size];

        b.iter(|| unsafe {
            black_box(copier.copy_memory(src.as_ptr(), dst.as_mut_ptr(), size)).unwrap();
        });
    });

    // 自适应策略
    group.bench_function("adaptive_8kb", |b| {
        let config = MemoryCopyConfig {
            default_strategy: CopyStrategy::Adaptive,
            ..Default::default()
        };
        let copier = FastMemoryCopier::new(config);

        let src = create_test_data(size);
        let mut dst = vec![0u8; size];

        b.iter(|| unsafe {
            black_box(copier.copy_memory(src.as_ptr(), dst.as_mut_ptr(), size)).unwrap();
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_copy_strategies,
    bench_aligned_vs_unaligned,
    bench_prefetch_effect,
    bench_non_temporal,
    bench_simd_levels
);

criterion_main!(benches);
