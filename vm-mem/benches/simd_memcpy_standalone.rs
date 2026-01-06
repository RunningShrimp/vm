//! Standalone SIMD memcpy benchmark
//!
//! This benchmark only tests the SIMD memcpy functionality
//! without dependencies on other vm-mem modules.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;

// Import only the SIMD module directly
mod simd_memcpy {
    pub use vm_mem::simd_memcpy::*;
}

/// Standard library memcpy (baseline)
fn memcpy_std(dst: &mut [u8], src: &[u8]) {
    dst.copy_from_slice(src);
}

/// Raw pointer memcpy (baseline)
fn memcpy_raw_ptr(dst: &mut [u8], src: &[u8]) {
    unsafe {
        std::ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), src.len());
    }
}

fn bench_memcpy_comparison(c: &mut Criterion) {
    println!("\n=== SIMD Memcpy Benchmark ===");
    println!("Active SIMD feature: {}", simd_memcpy::simd_feature_name());
    println!("============================\n");

    let mut group = c.benchmark_group("memcpy_comparison");

    let size = 65536; // 64 KB
    let src = vec![42u8; size];

    group.throughput(Throughput::Bytes(size as u64));
    group.sample_size(100);

    group.bench_function("standard_copy_from_slice", |b| {
        let mut dst = vec![0u8; size];
        b.iter(|| {
            memcpy_std(black_box(&mut dst), black_box(&src));
            black_box(&mut dst);
        });
    });

    group.bench_function("raw_ptr_copy_nonoverlapping", |b| {
        let mut dst = vec![0u8; size];
        b.iter(|| {
            memcpy_raw_ptr(black_box(&mut dst), black_box(&src));
            black_box(&mut dst);
        });
    });

    group.bench_function("simd_memcpy_fast", |b| {
        let mut dst = vec![0u8; size];
        b.iter(|| {
            simd_memcpy::memcpy_fast(black_box(&mut dst), black_box(&src));
            black_box(&mut dst);
        });
    });

    group.finish();
}

fn bench_memcpy_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("memcpy_sizes");

    for size in [1024, 4096, 16384, 65536].iter() {
        let src = vec![42u8; *size];

        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::new("std", size), size, |b, &_size| {
            let mut dst = vec![0u8; *size];
            b.iter(|| {
                memcpy_std(black_box(&mut dst), black_box(&src));
                black_box(&mut dst);
            });
        });

        group.bench_with_input(BenchmarkId::new("simd", size), size, |b, &_size| {
            let mut dst = vec![0u8; *size];
            b.iter(|| {
                simd_memcpy::memcpy_fast(black_box(&mut dst), black_box(&src));
                black_box(&mut dst);
            });
        });
    }

    group.finish();
}

fn bench_memcpy_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memcpy_patterns");

    let size = 16384;

    // Pattern 1: Sequential
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
            simd_memcpy::memcpy_fast(black_box(&mut dst), black_box(&src_seq));
            black_box(&mut dst);
        });
    });

    // Pattern 2: Random-like
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
            simd_memcpy::memcpy_fast(black_box(&mut dst), black_box(&src_rand));
            black_box(&mut dst);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_memcpy_comparison,
    bench_memcpy_sizes,
    bench_memcpy_patterns
);

criterion_main!(benches);
