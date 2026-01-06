// ARM64 NEON Intrinsic 性能基准测试
//
// 目的: 测量NEON intrinsic vs 标量代码的性能差异
// 平台: Apple M4 Pro (ARM64)
//
// Round 34 Stage 3: ARM64 NEON专用测试

#![cfg(target_arch = "aarch64")]
#![allow(unsafe_op_in_unsafe_fn)]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::arch::aarch64::*;
use std::hint::black_box;
use std::time::Duration;

// 测试数据大小
const SIZES: &[usize] = &[4, 16, 64, 256, 1024];

/// NEON向量加法 (4×f32)
#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn neon_add_f32(a: &[f32], b: &[f32], result: &mut [f32]) {
    let chunks = a.len() / 4;
    for i in 0..chunks {
        let a_vec = vld1q_f32(a.as_ptr().add(i * 4));
        let b_vec = vld1q_f32(b.as_ptr().add(i * 4));
        let result_vec = vaddq_f32(a_vec, b_vec);
        vst1q_f32(result.as_mut_ptr().add(i * 4), result_vec);
    }
}

/// 标量向量加法 (for comparison)
#[inline(always)]
fn scalar_add_f32(a: &[f32], b: &[f32], result: &mut [f32]) {
    for i in 0..a.len() {
        result[i] = a[i] + b[i];
    }
}

/// NEON向量乘法 (4×f32)
#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn neon_mul_f32(a: &[f32], b: &[f32], result: &mut [f32]) {
    let chunks = a.len() / 4;
    for i in 0..chunks {
        let a_vec = vld1q_f32(a.as_ptr().add(i * 4));
        let b_vec = vld1q_f32(b.as_ptr().add(i * 4));
        let result_vec = vmulq_f32(a_vec, b_vec);
        vst1q_f32(result.as_mut_ptr().add(i * 4), result_vec);
    }
}

/// 标量向量乘法
#[inline(always)]
fn scalar_mul_f32(a: &[f32], b: &[f32], result: &mut [f32]) {
    for i in 0..a.len() {
        result[i] = a[i] * b[i];
    }
}

/// NEON融合乘加 (FMA) - a * b + c
#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn neon_fma_f32(a: &[f32], b: &[f32], c: &[f32], result: &mut [f32]) {
    let chunks = a.len() / 4;
    for i in 0..chunks {
        let a_vec = vld1q_f32(a.as_ptr().add(i * 4));
        let b_vec = vld1q_f32(b.as_ptr().add(i * 4));
        let c_vec = vld1q_f32(c.as_ptr().add(i * 4));
        let result_vec = vfmaq_f32(c_vec, a_vec, b_vec); // c + a * b
        vst1q_f32(result.as_mut_ptr().add(i * 4), result_vec);
    }
}

/// 标量融合乘加
#[inline(always)]
fn scalar_fma_f32(a: &[f32], b: &[f32], c: &[f32], result: &mut [f32]) {
    for i in 0..a.len() {
        result[i] = a[i] * b[i] + c[i];
    }
}

/// NEON内存拷贝
#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn neon_memcpy_u64(src: &[u64], dst: &mut [u64]) {
    let chunks = src.len() / 2;
    for i in 0..chunks {
        let vec = vld1q_u64(src.as_ptr().add(i * 2));
        vst1q_u64(dst.as_mut_ptr().add(i * 2), vec);
    }
}

/// 标量内存拷贝
#[inline(always)]
fn scalar_memcpy_u64(src: &[u64], dst: &mut [u64]) {
    dst.copy_from_slice(src);
}

/// NEON点积运算 (利用标准NEON实现)
#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn neon_dot_product_f32(a: &[f32], b: &[f32]) -> f32 {
    let mut sum = vdupq_n_f32(0.0);
    let chunks = a.len() / 4;
    for i in 0..chunks {
        let a_vec = vld1q_f32(a.as_ptr().add(i * 4));
        let b_vec = vld1q_f32(b.as_ptr().add(i * 4));

        // 标准NEON实现: 乘法然后累加
        let mul = vmulq_f32(a_vec, b_vec);
        sum = vaddq_f32(sum, mul);
    }

    // 提取结果
    let mut result = [0.0f32; 4];
    vst1q_f32(result.as_mut_ptr(), sum);
    result.iter().sum()
}

/// 标量点积运算
#[inline(always)]
fn scalar_dot_product_f32(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

// ============================================================================
// 基准测试: 向量加法
// ============================================================================

fn bench_vector_add(c: &mut Criterion) {
    let mut group = c.benchmark_group("vector_add");

    for size in SIZES {
        let input_a: Vec<f32> = (0..*size).map(|i| i as f32).collect();
        let input_b: Vec<f32> = (0..*size).map(|i| i as f32 * 2.0).collect();
        let mut result_neon = vec![0.0f32; *size];
        let mut result_scalar = vec![0.0f32; *size];

        group.throughput(Throughput::Elements(*size as u64));

        // NEON版本
        group.bench_with_input(BenchmarkId::new("neon", size), size, |b, _| {
            b.iter(|| {
                unsafe {
                    neon_add_f32(&input_a, &input_b, &mut result_neon);
                }
                black_box(&result_neon[..]);
            });
        });

        // 标量版本
        group.bench_with_input(BenchmarkId::new("scalar", size), size, |b, _| {
            b.iter(|| {
                scalar_add_f32(&input_a, &input_b, &mut result_scalar);
                black_box(&result_scalar[..]);
            });
        });
    }

    group.finish();
}

// ============================================================================
// 基准测试: 向量乘法
// ============================================================================

fn bench_vector_mul(c: &mut Criterion) {
    let mut group = c.benchmark_group("vector_mul");

    for size in SIZES {
        let input_a: Vec<f32> = (0..*size).map(|i| i as f32).collect();
        let input_b: Vec<f32> = (0..*size).map(|i| i as f32 * 2.0).collect();
        let mut result_neon = vec![0.0f32; *size];
        let mut result_scalar = vec![0.0f32; *size];

        group.throughput(Throughput::Elements(*size as u64));

        // NEON版本
        group.bench_with_input(BenchmarkId::new("neon", size), size, |b, _| {
            b.iter(|| {
                unsafe {
                    neon_mul_f32(&input_a, &input_b, &mut result_neon);
                }
                black_box(&result_neon[..]);
            });
        });

        // 标量版本
        group.bench_with_input(BenchmarkId::new("scalar", size), size, |b, _| {
            b.iter(|| {
                scalar_mul_f32(&input_a, &input_b, &mut result_scalar);
                black_box(&result_scalar[..]);
            });
        });
    }

    group.finish();
}

// ============================================================================
// 基准测试: 融合乘加 (FMA)
// ============================================================================

fn bench_vector_fma(c: &mut Criterion) {
    let mut group = c.benchmark_group("vector_fma");

    for size in SIZES {
        let input_a: Vec<f32> = (0..*size).map(|i| i as f32).collect();
        let input_b: Vec<f32> = (0..*size).map(|i| i as f32 * 2.0).collect();
        let input_c: Vec<f32> = (0..*size).map(|i| i as f32 * 3.0).collect();
        let mut result_neon = vec![0.0f32; *size];
        let mut result_scalar = vec![0.0f32; *size];

        group.throughput(Throughput::Elements(*size as u64));

        // NEON版本
        group.bench_with_input(BenchmarkId::new("neon", size), size, |b, _| {
            b.iter(|| {
                unsafe {
                    neon_fma_f32(&input_a, &input_b, &input_c, &mut result_neon);
                }
                black_box(&result_neon[..]);
            });
        });

        // 标量版本
        group.bench_with_input(BenchmarkId::new("scalar", size), size, |b, _| {
            b.iter(|| {
                scalar_fma_f32(&input_a, &input_b, &input_c, &mut result_scalar);
                black_box(&result_scalar[..]);
            });
        });
    }

    group.finish();
}

// ============================================================================
// 基准测试: 内存拷贝
// ============================================================================

fn bench_memcpy(c: &mut Criterion) {
    let mut group = c.benchmark_group("memcpy_u64");

    for size in SIZES {
        let src: Vec<u64> = (0..*size).map(|i| i as u64).collect();
        let mut dst_neon = vec![0u64; *size];
        let mut dst_scalar = vec![0u64; *size];

        group.throughput(Throughput::Bytes((*size as u64) * 8));

        // NEON版本
        group.bench_with_input(BenchmarkId::new("neon", size), size, |b, _| {
            b.iter(|| {
                unsafe {
                    neon_memcpy_u64(&src, &mut dst_neon);
                }
                black_box(&dst_neon[..]);
            });
        });

        // 标量版本
        group.bench_with_input(BenchmarkId::new("scalar", size), size, |b, _| {
            b.iter(|| {
                scalar_memcpy_u64(&src, &mut dst_scalar);
                black_box(&dst_scalar[..]);
            });
        });
    }

    group.finish();
}

// ============================================================================
// 基准测试: 点积运算
// ============================================================================

fn bench_dot_product(c: &mut Criterion) {
    let mut group = c.benchmark_group("dot_product");

    for size in SIZES {
        let input_a: Vec<f32> = (0..*size).map(|i| i as f32).collect();
        let input_b: Vec<f32> = (0..*size).map(|i| i as f32 * 2.0).collect();

        group.throughput(Throughput::Elements(*size as u64));

        // NEON版本
        group.bench_with_input(BenchmarkId::new("neon", size), size, |b, _| {
            b.iter(|| unsafe {
                black_box(neon_dot_product_f32(&input_a, &input_b));
            });
        });

        // 标量版本
        group.bench_with_input(BenchmarkId::new("scalar", size), size, |b, _| {
            b.iter(|| {
                black_box(scalar_dot_product_f32(&input_a, &input_b));
            });
        });
    }

    group.finish();
}

// ============================================================================
// 自定义配置
// ============================================================================

fn custom_criterion() -> Criterion {
    Criterion::default()
        .measurement_time(Duration::from_secs(10))
        .sample_size(100)
        .warm_up_time(Duration::from_secs(3))
}

criterion_group! {
    name = neon_benches;
    config = custom_criterion();
    targets =
        bench_vector_add,
        bench_vector_mul,
        bench_vector_fma,
        bench_memcpy,
        bench_dot_product
}

criterion_main!(neon_benches);
