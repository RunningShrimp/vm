///
/// 测试物理内存管理器的读写吞吐量
// Use std::hint::black_box instead of criterion's deprecated version
use std::hint::black_box;
/// 内存操作性能基准测试
use vm_core::AddressTranslator;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use vm_core::mmu_traits::MemoryAccess;
use vm_mem::PhysicalMemory;

/// 基准测试: 内存读取性能
fn bench_memory_read_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_read_throughput");

    let mut mem = PhysicalMemory::new(1024 * 1024 * 1024, false);

    // 预分配并写入测试数据
    let test_addr = vm_core::GuestAddr(0x1000);
    for i in 0..(1024 * 1024 / 8) {
        let _ = mem.write(test_addr + i * 8, i as u64, 8);
    }

    for size in [1u8, 2, 4, 8].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let addr = black_box(test_addr);
                mem.read(addr, size)
            });
        });
    }

    group.finish();
}

/// 基准测试: 内存写入性能
fn bench_memory_write_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_write_throughput");

    for size in [1u8, 2, 4, 8].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let mut mem = PhysicalMemory::new(1024 * 1024 * 1024, false);
            let test_addr = vm_core::GuestAddr(0x1000);

            b.iter(|| {
                let addr = black_box(test_addr);
                let value = black_box(0xDEADBEEF_u64);
                mem.write(addr, value, size)
            });
        });
    }

    group.finish();
}

/// 基准测试: 批量内存读取
fn bench_bulk_memory_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("bulk_memory_read");

    let mut mem = PhysicalMemory::new(1024 * 1024 * 1024, false);
    let test_addr = vm_core::GuestAddr(0x1000);

    // 预先写入测试数据
    for i in 0..(65536 / 8) {
        let _ = mem.write(test_addr + i * 8, 0xABCD1234567890EF_u64, 8);
    }

    for size in [256, 1024, 4096, 16384, 65536].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, size| {
            let mut buffer = vec![0u8; *size];

            b.iter(|| {
                let addr = black_box(test_addr);
                mem.read_bulk(addr, &mut buffer)
            });
        });
    }

    group.finish();
}

/// 基准测试: 批量内存写入
fn bench_bulk_memory_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("bulk_memory_write");

    let mut mem = PhysicalMemory::new(1024 * 1024 * 1024, false);
    let test_addr = vm_core::GuestAddr(0x1000);

    for size in [256, 1024, 4096, 16384, 65536].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, size| {
            let data = vec![0xABu8; *size];

            b.iter(|| {
                let addr = black_box(test_addr);
                mem.write_bulk(addr, &data)
            });
        });
    }

    group.finish();
}

/// 基准测试: 不同地址的访问模式
fn bench_memory_access_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_access_patterns");

    let mem = PhysicalMemory::new(1024 * 1024 * 1024, false);

    // 顺序访问
    group.bench_function("sequential", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let addr = vm_core::GuestAddr(i * 0x1000);
                black_box(mem.read(addr, 8));
            }
        });
    });

    // 随机访问
    group.bench_function("random", |b| {
        let addrs: Vec<_> = (0..1000).map(|i| vm_core::GuestAddr(i * 0x1000)).collect();
        b.iter(|| {
            for addr in &addrs {
                black_box(mem.read(*addr, 8));
            }
        });
    });

    // 跨页边界访问
    group.bench_function("cross_page", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let addr = vm_core::GuestAddr(i * 0x1000 + 0xFF8);
                black_box(mem.read(addr, 8));
            }
        });
    });

    group.finish();
}

/// 基准测试: 不同对齐要求的访问
fn bench_aligned_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("aligned_access");

    let mem = PhysicalMemory::new(1024 * 1024 * 1024, false);

    for alignment in [1, 2, 4, 8, 16, 32, 64].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(alignment),
            alignment,
            |b, &alignment| {
                b.iter(|| {
                    let addr = vm_core::GuestAddr(alignment);
                    mem.read(addr, 8)
                });
            },
        );
    }

    group.finish();
}

/// 基准测试: 高地址区域访问性能 (模拟MMIO)
fn bench_mmio_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("mmio_access");

    let mem = PhysicalMemory::new(1024 * 1024 * 1024, false);

    // 高地址访问 (模拟MMIO场景)
    group.bench_function("high_addr_read", |b| {
        b.iter(|| {
            let addr = vm_core::GuestAddr(0xF0000000);
            black_box(mem.read(addr, 4));
        });
    });

    // 普通内存访问对比
    group.bench_function("normal_read", |b| {
        b.iter(|| {
            let addr = vm_core::GuestAddr(0x1000);
            black_box(mem.read(addr, 4));
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_memory_read_throughput,
    bench_memory_write_throughput,
    bench_bulk_memory_read,
    bench_bulk_memory_write,
    bench_memory_access_patterns,
    bench_aligned_access,
    bench_mmio_access,
);

criterion_main!(benches);
