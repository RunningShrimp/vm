use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use vm_core::AddressTranslator;
use vm_mem::optimization::unified::MemoryPool;

// 使用std::hint::black_box而不是criterion::black_box
use std::hint::black_box;

/// 内存分配性能基准测试
fn bench_memory_pool_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_pool_allocation");

    // 测试不同大小的分配
    for size in [1024u64, 4096, 65536].iter() {
        group.throughput(Throughput::Bytes(*size));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let mut pool = MemoryPool::new();
            b.iter(|| {
                let addr = pool.allocate(size as usize).unwrap();
                pool.deallocate(addr).ok();
                black_box(addr)
            });
        });
    }

    group.finish();
}

/// 内存重用性能基准测试
fn bench_memory_pool_reuse(c: &mut Criterion) {
    c.bench_function("memory_pool_reuse", |b| {
        let mut pool = MemoryPool::new();
        // 预分配一些地址
        let mut addrs = Vec::new();
        for _ in 0..10 {
            addrs.push(pool.allocate(4096).unwrap());
        }
        // 释放所有
        for addr in addrs {
            pool.deallocate(addr).unwrap();
        }

        b.iter(|| {
            let addr = pool.allocate(4096).unwrap();
            pool.deallocate(addr).ok();
            black_box(addr)
        });
    });
}

/// 批量分配性能基准测试
fn bench_memory_pool_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_pool_batch");

    for count in [10, 50, 100].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            b.iter(|| {
                let mut pool = MemoryPool::new();
                let mut addrs = Vec::with_capacity(count);
                for _ in 0..count {
                    addrs.push(pool.allocate(4096).unwrap());
                }
                // 清理
                for addr in &addrs {
                    pool.deallocate(*addr).ok();
                }
                black_box(addrs.len())
            });
        });
    }

    group.finish();
}

criterion_group!(
    name = memory_benches;
    config = Criterion::default().sample_size(100);
    targets =
        bench_memory_pool_allocation,
        bench_memory_pool_reuse,
        bench_memory_pool_batch
);
criterion_main!(memory_benches);
