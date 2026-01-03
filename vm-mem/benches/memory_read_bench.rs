//! Memory read performance benchmark - identifying 8-byte read anomaly
//!
//! This benchmark compares the performance of different-sized reads to identify
//! why 8-byte reads may be slower than expected.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use vm_mem::PhysicalMemory;

fn bench_reads(c: &mut Criterion) {
    let mem_size = 16 * 1024 * 1024; // 16MB
    let mem = PhysicalMemory::new(mem_size, false);

    // Initialize memory with some data
    for i in 0..1024 {
        let addr = i * 8;
        mem.write_u64(addr, 0xDEADBEEFCAFEBABE + addr as u64)
            .unwrap();
    }

    let mut group = c.benchmark_group("memory_read_sizes");
    // Increase sample size and warm-up time for more consistent results (P1-2 optimization)
    group.sample_size(200);
    group.warm_up_time(std::time::Duration::from_secs(5));
    group.measurement_time(std::time::Duration::from_secs(10));

    // Benchmark different read sizes
    for size in [1, 2, 4, 8].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let mut sum = 0u64;
                for i in 0..1024 {
                    let addr = i * size as usize;
                    let val = match size {
                        1 => mem.read_u8(black_box(addr)).unwrap() as u64,
                        2 => mem.read_u16(black_box(addr)).unwrap() as u64,
                        4 => mem.read_u32(black_box(addr)).unwrap() as u64,
                        8 => mem.read_u64(black_box(addr)).unwrap(),
                        _ => unreachable!(),
                    };
                    sum = sum.wrapping_add(val);
                }
                black_box(sum);
            });
        });
    }

    group.finish();
}

fn bench_sequential_reads(c: &mut Criterion) {
    let mem_size = 16 * 1024 * 1024; // 16MB
    let mem = PhysicalMemory::new(mem_size, false);

    // Initialize memory
    for i in 0..16384 {
        mem.write_u64(i * 8, i as u64).unwrap();
    }

    let mut group = c.benchmark_group("sequential_u64_reads");
    // Increase sample size for more consistent results (P1-2 optimization)
    group.sample_size(200);
    group.warm_up_time(std::time::Duration::from_secs(5));
    group.measurement_time(std::time::Duration::from_secs(10));

    // Test different access patterns
    group.bench_function("aligned_8byte", |b| {
        b.iter(|| {
            let mut sum = 0u64;
            for i in 0..16384 {
                sum = sum.wrapping_add(mem.read_u64(black_box(i * 8)).unwrap());
            }
            black_box(sum);
        });
    });

    group.bench_function("unaligned_8byte", |b| {
        b.iter(|| {
            let mut sum = 0u64;
            for i in 0..16384 {
                sum = sum.wrapping_add(mem.read_u64(black_box(i * 8 + 1)).unwrap());
            }
            black_box(sum);
        });
    });

    group.finish();
}

fn bench_read_throughput(c: &mut Criterion) {
    let mem_size = 16 * 1024 * 1024; // 16MB
    let mem = PhysicalMemory::new(mem_size, false);

    // Initialize memory
    for i in 0..(1024 * 1024 / 8) {
        mem.write_u64(i * 8, 0xABCD1234567890EF).unwrap();
    }

    let mut group = c.benchmark_group("read_throughput");
    // Increase sample size for more consistent results (P1-2 optimization)
    group.sample_size(200);
    group.warm_up_time(std::time::Duration::from_secs(5));
    group.measurement_time(std::time::Duration::from_secs(10));

    group.bench_function("u8_sequential_1mb", |b| {
        b.iter(|| {
            let mut sum = 0u8;
            for i in 0..1024 * 1024 {
                sum = sum.wrapping_add(mem.read_u8(black_box(i)).unwrap());
            }
            black_box(sum);
        });
    });

    group.bench_function("u16_sequential_1mb", |b| {
        b.iter(|| {
            let mut sum = 0u16;
            for i in 0..(1024 * 1024 / 2) {
                sum = sum.wrapping_add(mem.read_u16(black_box(i * 2)).unwrap());
            }
            black_box(sum);
        });
    });

    group.bench_function("u32_sequential_1mb", |b| {
        b.iter(|| {
            let mut sum = 0u32;
            for i in 0..(1024 * 1024 / 4) {
                sum = sum.wrapping_add(mem.read_u32(black_box(i * 4)).unwrap());
            }
            black_box(sum);
        });
    });

    group.bench_function("u64_sequential_1mb", |b| {
        b.iter(|| {
            let mut sum = 0u64;
            for i in 0..(1024 * 1024 / 8) {
                sum = sum.wrapping_add(mem.read_u64(black_box(i * 8)).unwrap());
            }
            black_box(sum);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_reads,
    bench_sequential_reads,
    bench_read_throughput
);
criterion_main!(benches);
