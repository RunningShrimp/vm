// 快速优化验证基准测试
//
// 用于验证SIMD和其他优化的实际效果

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use vm_core::{GuestAddr, Fault};
use vm_mem::MMU;

/// 内存复制性能基准 - 验证SIMD优化效果
fn bench_memory_copy(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_copy");

    // 不同大小的内存块
    for size in [1024, 4096, 16384].iter() {
        let mut data = vec![0u8; *size];

        group.bench_with_input(
            BenchmarkId::new("sequential", size),
            |b, data| {
                b.iter(|| {
                    let mut dest = vec![0u8; data.len()];
                    dest.copy_from_slice(black_box(data));
                    dest
                })
            },
        );
    }

    group.finish();
}

/// 内存清零性能基准
fn bench_memory_zero(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_zero");

    for size in [1024, 4096, 16384].iter() {
        group.bench_with_input(
            BenchmarkId::new("zero_fill", size),
            |b, size| {
                b.iter(|| {
                    let mut data = vec![0u8; *size];
                    // 清零操作
                    for byte in &mut data {
                        *byte = 0;
                    }
                    black_box(&data)
                })
            },
        );
    }

    group.finish();
}

/// 简单计算密集型基准 - 验证SIMD向量化
fn bench_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("computation");

    // 简单的数组操作
    group.bench_function("array_sum", |b| {
        let data: Vec<i32> = (0..1000).collect();
        b.iter(|| {
            let sum: i32 = black_box(&data).iter().sum();
            black_box(sum)
        })
    });

    // 数组乘法
    group.bench_function("array_multiply", |b| {
        let data: Vec<i32> = (0..1000).collect();
        let multiplier = 2;
        b.iter(|| {
            let result: Vec<i32> = black_box(&data)
                .iter()
                .map(|x| x * multiplier)
                .collect();
            black_box(result)
        })
    });

    group.finish();
}

/// MMU操作基准
fn bench_mmu_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("mmu");

    // 地址转换（模拟）
    group.bench_function("address_translation", |b| {
        let base_addr = GuestAddr(0x1000);
        b.iter(|| {
            let addr = GuestAddr::from(black_box(base_addr).0 + black_box(0x100));
            black_box(addr)
        })
    });

    group.finish();
}

/// 故障处理基准
fn bench_fault_handling(c: &mut Criterion) {
    let mut group = c.benchmark_group("fault");

    // 创建故障（性能测试）
    group.bench_function("fault_creation", |b| {
        b.iter(|| {
            let fault = Fault::PageFault {
                addr: GuestAddr(0x1000),
                is_write: black_box(true),
                is_present: black_box(false),
            };
            black_box(fault)
        })
    });

    group.finish();
}

criterion_group!(
    optimization_benches,
    bench_memory_copy,
    bench_memory_zero,
    bench_computation,
    bench_mmu_operations,
    bench_fault_handling
);

criterion_main!(optimization_benches);
