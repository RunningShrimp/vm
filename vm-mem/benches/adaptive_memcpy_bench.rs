//! Adaptive memcpy performance benchmark
//!
//! Compares adaptive strategy vs standard library vs pure SIMD
//! to verify the adaptive approach provides optimal performance.

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use vm_mem::simd_memcpy::{memcpy_adaptive, memcpy_fast};

/// Standard library memcpy (baseline)
fn memcpy_std(dst: &mut [u8], src: &[u8]) {
    dst.copy_from_slice(src);
}

fn bench_adaptive_comparison(c: &mut Criterion) {
    println!("\n=== Adaptive Memcpy Benchmark ===");
    println!("Testing strategy: Adaptive (SIMD < 4KB, Standard >= 4KB)");
    println!("===============================\n");

    let mut group = c.benchmark_group("adaptive_comparison");

    // Test different sizes to show crossover point
    for size in [512, 1024, 2048, 4096, 8192, 16384].iter() {
        let src = vec![42u8; *size];

        group.throughput(Throughput::Bytes(*size as u64));

        // Standard library
        group.bench_with_input(BenchmarkId::new("std", size), size, |b, &_size| {
            let mut dst = vec![0u8; *size];
            b.iter(|| {
                memcpy_std(black_box(&mut dst), black_box(&src));
                black_box(&mut dst);
            });
        });

        // Pure SIMD
        group.bench_with_input(BenchmarkId::new("simd", size), size, |b, &_size| {
            let mut dst = vec![0u8; *size];
            b.iter(|| {
                memcpy_fast(black_box(&mut dst), black_box(&src));
                black_box(&mut dst);
            });
        });

        // Adaptive (automatic selection)
        group.bench_with_input(BenchmarkId::new("adaptive", size), size, |b, &_size| {
            let mut dst = vec![0u8; *size];
            b.iter(|| {
                memcpy_adaptive(black_box(&mut dst), black_box(&src));
                black_box(&mut dst);
            });
        });
    }

    group.finish();
}

fn bench_adaptive_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("adaptive_patterns");

    let size = 16384;

    // Pattern 1: Sequential (SIMD-friendly)
    let src_seq: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
    group.throughput(Throughput::Bytes(size as u64));

    group.bench_function("sequential_std", |b| {
        let mut dst = vec![0u8; size];
        b.iter(|| {
            memcpy_std(black_box(&mut dst), black_box(&src_seq));
            black_box(&mut dst);
        });
    });

    group.bench_function("sequential_simd", |b| {
        let mut dst = vec![0u8; size];
        b.iter(|| {
            memcpy_fast(black_box(&mut dst), black_box(&src_seq));
            black_box(&mut dst);
        });
    });

    group.bench_function("sequential_adaptive", |b| {
        let mut dst = vec![0u8; size];
        b.iter(|| {
            memcpy_adaptive(black_box(&mut dst), black_box(&src_seq));
            black_box(&mut dst);
        });
    });

    // Pattern 2: Random-like (less SIMD-friendly)
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let src_rand: Vec<u8> = (0..size)
        .map(|i| {
            let mut hasher = DefaultHasher::new();
            i.hash(&mut hasher);
            (hasher.finish() & 0xFF) as u8
        })
        .collect();

    group.bench_function("random_std", |b| {
        let mut dst = vec![0u8; size];
        b.iter(|| {
            memcpy_std(black_box(&mut dst), black_box(&src_rand));
            black_box(&mut dst);
        });
    });

    group.bench_function("random_simd", |b| {
        let mut dst = vec![0u8; size];
        b.iter(|| {
            memcpy_fast(black_box(&mut dst), black_box(&src_rand));
            black_box(&mut dst);
        });
    });

    group.bench_function("random_adaptive", |b| {
        let mut dst = vec![0u8; size];
        b.iter(|| {
            memcpy_adaptive(black_box(&mut dst), black_box(&src_rand));
            black_box(&mut dst);
        });
    });

    group.finish();
}

fn bench_real_world_workloads(c: &mut Criterion) {
    let mut group = c.benchmark_group("real_workloads");

    // Workload 1: Small structure copying (typical in VM)
    let src_struct = vec![0u8; 256]; // 256 bytes
    group.throughput(Throughput::Bytes(256));

    group.bench_function("small_struct_adaptive", |b| {
        let mut dst = vec![0u8; 256];
        b.iter(|| {
            memcpy_adaptive(black_box(&mut dst), black_box(&src_struct));
            black_box(&mut dst);
        });
    });

    // Workload 2: Page copying (4KB - threshold boundary)
    let src_page = vec![0u8; 4096]; // 4KB page
    group.throughput(Throughput::Bytes(4096));

    group.bench_function("page_copy_adaptive", |b| {
        let mut dst = vec![0u8; 4096];
        b.iter(|| {
            memcpy_adaptive(black_box(&mut dst), black_box(&src_page));
            black_box(&mut dst);
        });
    });

    // Workload 3: Large buffer (typical in memory operations)
    let src_buffer = vec![0u8; 65536]; // 64KB
    group.throughput(Throughput::Bytes(65536));

    group.bench_function("large_buffer_adaptive", |b| {
        let mut dst = vec![0u8; 65536];
        b.iter(|| {
            memcpy_adaptive(black_box(&mut dst), black_box(&src_buffer));
            black_box(&mut dst);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_adaptive_comparison,
    bench_adaptive_patterns,
    bench_real_world_workloads
);

criterion_main!(benches);
