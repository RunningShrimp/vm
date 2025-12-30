//! 异步MMU性能基准测试
//!
//! 测试异步MMU操作的延迟和吞吐量

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::sync::Arc;
use tokio::runtime::Runtime;

use vm_core::{AccessType, GuestAddr};
use vm_mem::SoftMmu;
use vm_mem::async_mmu::async_impl::{AsyncMmuWrapper, AsyncMMU};

// Use std::hint::black_box instead of criterion's deprecated version
use std::hint::black_box;

fn create_runtime() -> Runtime {
    Runtime::new().unwrap()
}

fn create_test_mmu() -> AsyncMmuWrapper {
    let mmu = SoftMmu::new(1024 * 1024 * 1024, false); // 1GB memory, bare mode
    AsyncMmuWrapper::new(Box::new(mmu))
}

/// 基准测试: 异步地址翻译延迟
fn bench_async_translate_latency(c: &mut Criterion) {
    let rt = create_runtime();
    let async_mmu = create_test_mmu();

    c.bench_function("async_translate_latency", |b| {
        b.to_async(&rt).iter(|| {
            let addr = black_box(GuestAddr(0x1000));
            async_mmu.translate_async(addr, AccessType::Read)
        });
    });
}

/// 基准测试: 批量异步翻译性能
fn bench_async_batch_translate(c: &mut Criterion) {
    let rt = create_runtime();
    let mut group = c.benchmark_group("async_batch_translate");

    for batch_size in [10, 50, 100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &batch_size| {
                let async_mmu = create_test_mmu();
                let requests: Vec<_> = (0..batch_size)
                    .map(|i| (GuestAddr((i + 1) * 0x1000), AccessType::Read))
                    .collect();

                b.to_async(&rt).iter(|| {
                    async_mmu.translate_bulk_async(black_box(&requests))
                });
            },
        );
    }

    group.finish();
}

/// 基准测试: 异步内存读取性能
fn bench_async_memory_read(c: &mut Criterion) {
    let rt = create_runtime();
    let mut group = c.benchmark_group("async_memory_read");

    let async_mmu = create_test_mmu();

    // 预写入测试数据
    rt.block_on(async {
        for i in 0..1000 {
            let addr = GuestAddr((i + 1) * 0x1000);
            let _ = async_mmu.write_async(addr, i as u64, 8).await;
        }
    });

    for size in [1u8, 2, 4, 8].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.to_async(&rt).iter(|| {
                let addr = black_box(GuestAddr(0x1000));
                async_mmu.read_async(addr, size)
            });
        });
    }

    group.finish();
}

/// 基准测试: 异步内存写入性能
fn bench_async_memory_write(c: &mut Criterion) {
    let rt = create_runtime();
    let mut group = c.benchmark_group("async_memory_write");

    for size in [1u8, 2, 4, 8].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let async_mmu = create_test_mmu();
            b.to_async(&rt).iter(|| {
                let addr = black_box(GuestAddr(0x1000));
                let value = black_box(0xDEADBEEF_u64);
                async_mmu.write_async(addr, value, size)
            });
        });
    }

    group.finish();
}

/// 基准测试: 批量异步内存操作
fn bench_async_bulk_operations(c: &mut Criterion) {
    let rt = create_runtime();
    let mut group = c.benchmark_group("async_bulk_operations");

    for size in [256, 1024, 4096, 16384].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let async_mmu = create_test_mmu();
            let data = vec![0xABu8; *size];
            let addr = GuestAddr(0x1000);

            b.to_async(&rt).iter(|| {
                async_mmu.write_bulk_async(addr, black_box(&data))
            });
        });
    }

    group.finish();
}

/// 基准测试: 并发异步翻译
fn bench_async_concurrent_translate(c: &mut Criterion) {
    let rt = create_runtime();
    let mut group = c.benchmark_group("async_concurrent_translate");

    for concurrency in [10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(concurrency),
            concurrency,
            |b, &concurrency| {
                b.to_async(&rt).iter(|| {
                    let async_mmu = Arc::new(create_test_mmu());

                    async move {
                        let mut tasks = Vec::with_capacity(concurrency);

                        for i in 0..concurrency {
                            let mmu_clone = Arc::clone(&async_mmu);
                            tasks.push(tokio::spawn(async move {
                                let addr = GuestAddr((i + 1) * 0x1000);
                                mmu_clone.translate_async(addr, AccessType::Read).await
                            }));
                        }

                        // 等待所有任务完成
                        for task in tasks {
                            black_box(task.await.unwrap().unwrap());
                        }

                        Ok::<(), vm_core::VmError>(())
                    }
                });
            },
        );
    }

    group.finish();
}

/// 基准测试: 异步TLB刷新
fn bench_async_tlb_flush(c: &mut Criterion) {
    let rt = create_runtime();
    let async_mmu = create_test_mmu();

    c.bench_function("async_tlb_flush", |b| {
        b.to_async(&rt).iter(|| {
            async_mmu.flush_tlb_async()
        });
    });
}

/// 基准测试: 混合异步操作
fn bench_async_mixed_operations(c: &mut Criterion) {
    let rt = create_runtime();
    let mut group = c.benchmark_group("async_mixed_operations");

    for operation_count in [10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(operation_count),
            operation_count,
            |b, &operation_count| {
                let async_mmu = create_test_mmu();

                b.to_async(&rt).iter(|| {
                    let async_mmu = &async_mmu;

                    async move {
                        // 混合操作: 翻译、读取、写入
                        for i in 0..operation_count {
                            let addr = GuestAddr((i + 1) * 0x1000);

                            // 翻译
                            let _ = async_mmu
                                .translate_async(addr, AccessType::Read)
                                .await?;

                            // 写入
                            let _ = async_mmu.write_async(addr, i as u64, 8).await?;

                            // 读取
                            let _ = async_mmu.read_async(addr, 8).await?;

                            Ok::<(), vm_core::VmError>(())
                        }

                        Ok::<(), vm_core::VmError>(())
                    }
                });
            },
        );
    }

    group.finish();
}

/// 基准测试: 异步指令预取
fn bench_async_insn_fetch(c: &mut Criterion) {
    let rt = create_runtime();
    let async_mmu = create_test_mmu();

    c.bench_function("async_insn_fetch", |b| {
        b.to_async(&rt).iter(|| {
            let addr = black_box(GuestAddr(0x1000));
            async_mmu.fetch_insn_async(addr)
        });
    });
}

criterion_group!(
    benches,
    bench_async_translate_latency,
    bench_async_batch_translate,
    bench_async_memory_read,
    bench_async_memory_write,
    bench_async_bulk_operations,
    bench_async_concurrent_translate,
    bench_async_tlb_flush,
    bench_async_mixed_operations,
    bench_async_insn_fetch,
);

criterion_main!(benches);
