//! SIMD memcpy performance benchmarks
//!
//! Compares SIMD-optimized memcpy implementations against standard library copy.
//!
//! Run benchmarks:
//! ```bash
//! cargo bench -p vm-mem --bench simd_memcpy
//! ```
//!
//! Expected results:
//! - AVX-512: 8-10x faster for large aligned copies
//! - AVX2: 5-7x faster for large aligned copies
//! - NEON: 4-6x faster for large aligned copies

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use vm_mem::simd_memcpy::{memcpy_fast, simd_feature_name};

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

fn bench_memcpy_small(c: &mut Criterion) {
    let mut group = c.benchmark_group("memcpy_small");

    for size in [1, 4, 8, 16, 32, 64, 128, 256].iter() {
        let src = vec![42u8; *size];

        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::new("std", size), size, |b, &_size| {
            let mut dst = vec![0u8; *size];
            b.iter(|| {
                memcpy_std(black_box(&mut dst), black_box(&src));
                black_box(&mut dst);
            });
        });

        group.bench_with_input(BenchmarkId::new("raw_ptr", size), size, |b, &_size| {
            let mut dst = vec![0u8; *size];
            b.iter(|| {
                memcpy_raw_ptr(black_box(&mut dst), black_box(&src));
                black_box(&mut dst);
            });
        });

        group.bench_with_input(BenchmarkId::new("simd", size), size, |b, &_size| {
            let mut dst = vec![0u8; *size];
            b.iter(|| {
                memcpy_fast(black_box(&mut dst), black_box(&src));
                black_box(&mut dst);
            });
        });
    }

    group.finish();
}

fn bench_memcpy_medium(c: &mut Criterion) {
    let mut group = c.benchmark_group("memcpy_medium");

    for size in [512, 1024, 2048, 4096].iter() {
        let src = vec![42u8; *size];

        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::new("std", size), size, |b, &_size| {
            let mut dst = vec![0u8; *size];
            b.iter(|| {
                memcpy_std(black_box(&mut dst), black_box(&src));
                black_box(&mut dst);
            });
        });

        group.bench_with_input(BenchmarkId::new("raw_ptr", size), size, |b, &_size| {
            let mut dst = vec![0u8; *size];
            b.iter(|| {
                memcpy_raw_ptr(black_box(&mut dst), black_box(&src));
                black_box(&mut dst);
            });
        });

        group.bench_with_input(BenchmarkId::new("simd", size), size, |b, &_size| {
            let mut dst = vec![0u8; *size];
            b.iter(|| {
                memcpy_fast(black_box(&mut dst), black_box(&src));
                black_box(&mut dst);
            });
        });
    }

    group.finish();
}

fn bench_memcpy_large(c: &mut Criterion) {
    let mut group = c.benchmark_group("memcpy_large");

    for size in [8192, 16384, 32768, 65536, 131072].iter() {
        let src = vec![42u8; *size];

        group.throughput(Throughput::Bytes(*size as u64));
        group.sample_size(50); // Reduce samples for large copies

        group.bench_with_input(BenchmarkId::new("std", size), size, |b, &_size| {
            let mut dst = vec![0u8; *size];
            b.iter(|| {
                memcpy_std(black_box(&mut dst), black_box(&src));
                black_box(&mut dst);
            });
        });

        group.bench_with_input(BenchmarkId::new("raw_ptr", size), size, |b, &_size| {
            let mut dst = vec![0u8; *size];
            b.iter(|| {
                memcpy_raw_ptr(black_box(&mut dst), black_box(&src));
                black_box(&mut dst);
            });
        });

        group.bench_with_input(BenchmarkId::new("simd", size), size, |b, &_size| {
            let mut dst = vec![0u8; *size];
            b.iter(|| {
                memcpy_fast(black_box(&mut dst), black_box(&src));
                black_box(&mut dst);
            });
        });
    }

    group.finish();
}

fn bench_memcpy_aligned(c: &mut Criterion) {
    let mut group = c.benchmark_group("memcpy_aligned");

    // Test different SIMD alignment boundaries
    for size in [16, 32, 64, 128, 256, 512].iter() {
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
                memcpy_fast(black_box(&mut dst), black_box(&src));
                black_box(&mut dst);
            });
        });
    }

    group.finish();
}

fn bench_memcpy_unaligned(c: &mut Criterion) {
    let mut group = c.benchmark_group("memcpy_unaligned");

    // Test unaligned access patterns
    let size = 4096;
    let src = vec![42u8; size + 16];

    group.throughput(Throughput::Bytes(size as u64));

    for offset in [1, 3, 5, 7, 9].iter() {
        group.bench_with_input(BenchmarkId::new("std", offset), offset, |b, &_offset| {
            let mut dst = vec![0u8; size];
            let src_slice = &src[*offset..*offset + size];
            b.iter(|| {
                memcpy_std(black_box(&mut dst), black_box(src_slice));
                black_box(&mut dst);
            });
        });

        group.bench_with_input(BenchmarkId::new("simd", offset), offset, |b, &_offset| {
            let mut dst = vec![0u8; size];
            let src_slice = &src[*offset..*offset + size];
            b.iter(|| {
                memcpy_fast(black_box(&mut dst), black_box(src_slice));
                black_box(&mut dst);
            });
        });
    }

    group.finish();
}

fn bench_memcpy_pattern(c: &mut Criterion) {
    let mut group = c.benchmark_group("memcpy_pattern");

    // Test realistic data patterns
    let size = 16384;

    // Pattern 1: Sequential
    let src_seq: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
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

    // Pattern 2: All zeros
    let src_zero = vec![0u8; size];
    group.bench_function("zero_std", |b| {
        let mut dst = vec![0u8; size];
        b.iter(|| {
            memcpy_std(black_box(&mut dst), black_box(&src_zero));
            black_box(&mut dst);
        });
    });

    group.bench_function("zero_simd", |b| {
        let mut dst = vec![0u8; size];
        b.iter(|| {
            memcpy_fast(black_box(&mut dst), black_box(&src_zero));
            black_box(&mut dst);
        });
    });

    // Pattern 3: Random-like
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

    group.finish();
}

fn bench_memcpy_comparison(c: &mut Criterion) {
    println!("\n=== SIMD Memcpy Benchmark ===");
    println!("Active SIMD feature: {}", simd_feature_name());
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
            memcpy_fast(black_box(&mut dst), black_box(&src));
            black_box(&mut dst);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_memcpy_small,
    bench_memcpy_medium,
    bench_memcpy_large,
    bench_memcpy_aligned,
    bench_memcpy_unaligned,
    bench_memcpy_pattern,
    bench_memcpy_comparison
);

criterion_main!(benches);
