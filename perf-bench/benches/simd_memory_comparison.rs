// SIMD内存操作性能对比基准测试
//
// 对比不同内存操作方式的性能：
// - volatile读写 vs 普通读写
// - SIMD优化 vs 标准库实现
//
// 运行方式:
// cargo bench --bench simd_memory_comparison

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use std::time::Duration;

/// 基准测试1: volatile读写 (当前实现)
fn bench_volatile_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("volatile_vs_normal");
    group.measurement_time(Duration::from_secs(10));

    // volatile读写
    group.bench_function("volatile_1kb", |b| {
        b.iter(|| {
            let mut memory = vec![0u8; 1024];
            for i in 0..256 {
                unsafe {
                    std::ptr::write_volatile(memory.as_mut_ptr().add(i * 4) as *mut u32, i as u32);
                }
            }
            let mut sum = 0u32;
            for i in 0..256 {
                unsafe {
                    sum += std::ptr::read_volatile(memory.as_ptr().add(i * 4) as *const u32);
                }
            }
            black_box(sum)
        })
    });

    // 普通读写
    group.bench_function("normal_1kb", |b| {
        b.iter(|| {
            let mut memory = vec![0u8; 1024];
            for i in 0..256 {
                unsafe {
                    *(memory.as_mut_ptr().add(i * 4) as *mut u32) = i as u32;
                }
            }
            let mut sum = 0u32;
            for i in 0..256 {
                unsafe {
                    sum += *(memory.as_ptr().add(i * 4) as *const u32);
                }
            }
            black_box(sum)
        })
    });

    group.finish();
}

/// 基准测试2: 不同大小的memcpy对比
fn bench_memcpy_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("memcpy_sizes");
    group.measurement_time(Duration::from_secs(10));

    // 测试不同大小
    for size in [64, 256, 1024, 4096].iter() {
        group.bench_with_input(format!("std_{}bytes", size), size, |b, size| {
            let src = vec![42u8; *size];
            let mut dst = vec![0u8; *size];

            b.iter(|| {
                unsafe {
                    std::ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), *size);
                }
                black_box(dst[0])
            })
        });

        group.bench_with_input(format!("slice_{}bytes", size), size, |b, size| {
            let src = vec![42u8; *size];

            b.iter(|| {
                let mut dst = vec![0u8; *size];
                dst.copy_from_slice(&src);
                black_box(dst[0])
            })
        });
    }

    group.finish();
}

/// 基准测试3: 批量复制 vs 逐个复制
fn bench_batch_vs_loop(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_vs_loop");
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("loop_256_bytes", |b| {
        let src = vec![42u8; 256];
        b.iter(|| {
            let mut dst = vec![0u8; 256];
            for i in 0..256 {
                dst[i] = src[i];
            }
            black_box(dst[0])
        })
    });

    group.bench_function("copy_from_slice_256", |b| {
        let src = vec![42u8; 256];
        b.iter(|| {
            let mut dst = vec![0u8; 256];
            dst.copy_from_slice(&src);
            black_box(dst[0])
        })
    });

    group.bench_function("std_copy_256", |b| {
        let src = vec![42u8; 256];
        b.iter(|| {
            let mut dst = vec![0u8; 256];
            unsafe {
                std::ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), 256);
            }
            black_box(dst[0])
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_volatile_memory,
    bench_memcpy_sizes,
    bench_batch_vs_loop
);

criterion_main!(benches);
