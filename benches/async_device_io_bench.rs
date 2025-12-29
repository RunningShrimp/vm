/// 异步设备I/O基准测试
///
/// 专门测试异步设备I/O操作的性能
/// 包括块设备、网络设备、以及各种I/O模式
///
/// 运行: cargo bench --bench async_device_io_bench
use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use vm_device::async_block_device::{AsyncBlockDevice, BlockDeviceConfig};
use vm_device::async_buffer_pool::{AsyncBufferPool, BufferPoolConfig};

/// 基准测试：顺序块设备读取
fn sequential_block_reads(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();

    let mut group = c.benchmark_group("sequential_block_reads");

    let device = Arc::new(AsyncBlockDevice::new_memory(
        1000000,
        BufferPoolConfig::default(),
    ));

    for block_size in [512, 1024, 4096, 8192].iter() {
        group.throughput(Throughput::Bytes(*block_size as u64));
        group.bench_with_input(
            BenchmarkId::new("sequential_read", block_size),
            block_size,
            |b, &block_size| {
                b.to_async(&rt).iter(|| async {
                    let mut buf = vec![0u8; block_size];
                    let _ = device.read_async(0, &mut buf).await;
                    black_box(buf);
                });
            },
        );
    }

    group.finish();
}

/// 基准测试：顺序块设备写入
fn sequential_block_writes(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();

    let mut group = c.benchmark_group("sequential_block_writes");

    for block_size in [512, 1024, 4096, 8192].iter() {
        group.throughput(Throughput::Bytes(*block_size as u64));
        group.bench_with_input(
            BenchmarkId::new("sequential_write", block_size),
            block_size,
            |b, &block_size| {
                b.to_async(&rt).iter(|| async {
                    let device = AsyncBlockDevice::new_memory(1000000, BufferPoolConfig::default());
                    let buf = vec![0xABu8; block_size];
                    let _ = device.write_async(0, &buf).await;
                });
            },
        );
    }

    group.finish();
}

/// 基准测试：随机块设备I/O
fn random_block_io(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();

    let mut group = c.benchmark_group("random_block_io");

    let device = Arc::new(AsyncBlockDevice::new_memory(
        1000000,
        BufferPoolConfig::default(),
    ));

    // 随机读取
    group.throughput(Throughput::Bytes(4096));
    group.bench_function("random_read_4kb", |b| {
        b.to_async(&rt).iter(|| async {
            let offset = (black_box(12345u64) % 100000) * 4096;
            let mut buf = vec![0u8; 4096];
            let _ = device.read_async(offset, &mut buf).await;
            black_box(buf);
        });
    });

    // 随机写入
    group.throughput(Throughput::Bytes(4096));
    group.bench_function("random_write_4kb", |b| {
        b.to_async(&rt).iter(|| async {
            let offset = (black_box(54321u64) % 100000) * 4096;
            let buf = vec![0xCDu8; 4096];
            let _ = device.write_async(offset, &buf).await;
        });
    });

    group.finish();
}

/// 基准测试：混合读写负载
fn mixed_read_write_workload(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();

    let mut group = c.benchmark_group("mixed_workload");

    // 70% 读, 30% 写
    group.bench_function("70r_30w", |b| {
        b.to_async(&rt).iter(|| async {
            let device = AsyncBlockDevice::new_memory(1000000, BufferPoolConfig::default());

            // 7次读取
            for i in 0..7 {
                let mut buf = vec![0u8; 4096];
                let _ = device.read_async(i * 4096, &mut buf).await;
            }

            // 3次写入
            for i in 0..3 {
                let buf = vec![0xABu8; 4096];
                let _ = device.write_async(i * 4096, &buf).await;
            }
        });
    });

    // 50% 读, 50% 写
    group.bench_function("50r_50w", |b| {
        b.to_async(&rt).iter(|| async {
            let device = AsyncBlockDevice::new_memory(1000000, BufferPoolConfig::default());

            for i in 0..5 {
                // 读取
                let mut buf = vec![0u8; 4096];
                let _ = device.read_async(i * 4096, &mut buf).await;

                // 写入
                let buf = vec![0xABu8; 4096];
                let _ = device.write_async(i * 4096, &buf).await;
            }
        });
    });

    // 30% 读, 70% 写
    group.bench_function("30r_70w", |b| {
        b.to_async(&rt).iter(|| async {
            let device = AsyncBlockDevice::new_memory(1000000, BufferPoolConfig::default());

            // 3次读取
            for i in 0..3 {
                let mut buf = vec![0u8; 4096];
                let _ = device.read_async(i * 4096, &mut buf).await;
            }

            // 7次写入
            for i in 0..7 {
                let buf = vec![0xABu8; 4096];
                let _ = device.write_async(i * 4096, &buf).await;
            }
        });
    });

    group.finish();
}

/// 基准测试：并发块设备访问
fn concurrent_block_device_access(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(8)
        .enable_all()
        .build()
        .unwrap();

    let mut group = c.benchmark_group("concurrent_device_access");

    for concurrent_ops in [1, 2, 4, 8, 16].iter() {
        group.throughput(Throughput::Elements(*concurrent_ops as u64));
        group.bench_with_input(
            BenchmarkId::new("concurrent_readers", concurrent_ops),
            concurrent_ops,
            |b, &concurrent_ops| {
                b.to_async(&rt).iter(|| async {
                    let device = Arc::new(AsyncBlockDevice::new_memory(
                        1000000,
                        BufferPoolConfig::default(),
                    ));

                    let mut handles = vec![];
                    for i in 0..concurrent_ops {
                        let device_clone = device.clone();
                        let handle = tokio::spawn(async move {
                            let mut buf = vec![0u8; 4096];
                            let offset = (i * 1000) as u64;
                            let _ = device_clone.read_async(offset, &mut buf).await;
                            black_box(buf);
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        let _ = handle.await;
                    }
                });
            },
        );
    }

    // 并发写入者
    for concurrent_ops in [1, 2, 4, 8].iter() {
        group.throughput(Throughput::Elements(*concurrent_ops as u64));
        group.bench_with_input(
            BenchmarkId::new("concurrent_writers", concurrent_ops),
            concurrent_ops,
            |b, &concurrent_ops| {
                b.to_async(&rt).iter(|| async {
                    let device = Arc::new(AsyncBlockDevice::new_memory(
                        1000000,
                        BufferPoolConfig::default(),
                    ));

                    let mut handles = vec![];
                    for i in 0..concurrent_ops {
                        let device_clone = device.clone();
                        let handle = tokio::spawn(async move {
                            let buf = vec![0xABu8; 4096];
                            let offset = (i * 10000) as u64; // 分离写入位置
                            let _ = device_clone.write_async(offset, &buf).await;
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        let _ = handle.await;
                    }
                });
            },
        );
    }

    group.finish();
}

/// 基准测试：缓冲池性能对设备I/O的影响
fn buffer_pool_impact_on_device_io(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();

    let mut group = c.benchmark_group("buffer_pool_impact");

    // 小缓冲池
    group.bench_function("small_pool", |b| {
        b.to_async(&rt).iter(|| async {
            let device = AsyncBlockDevice::new_memory(
                1000000,
                BufferPoolConfig {
                    buffer_size: 4096,
                    initial_pool_size: 5,
                    max_pool_size: 10,
                    max_pending_ops: 5,
                },
            );

            for i in 0..100 {
                let mut buf = vec![0u8; 4096];
                let _ = device.read_async(i * 4096, &mut buf).await;
            }
        });
    });

    // 中等缓冲池
    group.bench_function("medium_pool", |b| {
        b.to_async(&rt).iter(|| async {
            let device = AsyncBlockDevice::new_memory(
                1000000,
                BufferPoolConfig {
                    buffer_size: 4096,
                    initial_pool_size: 50,
                    max_pool_size: 100,
                    max_pending_ops: 20,
                },
            );

            for i in 0..100 {
                let mut buf = vec![0u8; 4096];
                let _ = device.read_async(i * 4096, &mut buf).await;
            }
        });
    });

    // 大缓冲池
    group.bench_function("large_pool", |b| {
        b.to_async(&rt).iter(|| async {
            let device = AsyncBlockDevice::new_memory(
                1000000,
                BufferPoolConfig {
                    buffer_size: 4096,
                    initial_pool_size: 200,
                    max_pool_size: 500,
                    max_pending_ops: 50,
                },
            );

            for i in 0..100 {
                let mut buf = vec![0u8; 4096];
                let _ = device.read_async(i * 4096, &mut buf).await;
            }
        });
    });

    group.finish();
}

/// 基准测试：设备刷新性能
fn device_flush_performance(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("device_flush");

    // 无刷新
    group.bench_function("no_flush", |b| {
        b.to_async(&rt).iter(|| async {
            let device = AsyncBlockDevice::new_memory(1000000, BufferPoolConfig::default());

            for i in 0..10 {
                let buf = vec![0xABu8; 4096];
                let _ = device.write_async(i * 4096, &buf).await;
            }
        });
    });

    // 每次写入后刷新
    group.bench_function("flush_every_write", |b| {
        b.to_async(&rt).iter(|| async {
            let device = AsyncBlockDevice::new_memory(1000000, BufferPoolConfig::default());

            for i in 0..10 {
                let buf = vec![0xABu8; 4096];
                let _ = device.write_async(i * 4096, &buf).await;
                let _ = device.flush_async().await;
            }
        });
    });

    // 批量写入后刷新
    group.bench_function("flush_after_batch", |b| {
        b.to_async(&rt).iter(|| async {
            let device = AsyncBlockDevice::new_memory(1000000, BufferPoolConfig::default());

            for i in 0..10 {
                let buf = vec![0xABu8; 4096];
                let _ = device.write_async(i * 4096, &buf).await;
            }
            let _ = device.flush_async().await;
        });
    });

    group.finish();
}

/// 基准测试：大块传输性能
fn large_transfer_performance(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();

    let mut group = c.benchmark_group("large_transfers");

    let device = Arc::new(AsyncBlockDevice::new_memory(
        10000000,
        BufferPoolConfig::default(),
    ));

    for size in [64 * 1024, 256 * 1024, 1024 * 1024, 4 * 1024 * 1024].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("large_read", size / 1024), // KB
            size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let mut buf = vec![0u8; size];
                    let _ = device.read_async(0, &mut buf).await;
                    black_box(buf);
                });
            },
        );
    }

    for size in [64 * 1024, 256 * 1024, 1024 * 1024, 4 * 1024 * 1024].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("large_write", size / 1024), // KB
            size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let buf = vec![0xABu8; size];
                    let _ = device.write_async(0, &buf).await;
                });
            },
        );
    }

    group.finish();
}

/// 基准测试：IOPS性能（每秒I/O操作数）
fn iops_performance(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();

    let mut group = c.benchmark_group("iops");

    let device = Arc::new(AsyncBlockDevice::new_memory(
        1000000,
        BufferPoolConfig::default(),
    ));

    // 4K随机读IOPS
    group.throughput(Throughput::Elements(1));
    group.bench_function("4k_random_read", |b| {
        b.to_async(&rt).iter(|| async {
            let offset = black_box(12345u64) % 100000 * 4096;
            let mut buf = vec![0u8; 4096];
            let _ = device.read_async(offset, &mut buf).await;
        });
    });

    // 4K随机写IOPS
    group.throughput(Throughput::Elements(1));
    group.bench_function("4k_random_write", |b| {
        b.to_async(&rt).iter(|| async {
            let offset = black_box(54321u64) % 100000 * 4096;
            let buf = vec![0xABu8; 4096];
            let _ = device.write_async(offset, &buf).await;
        });
    });

    // 4K顺序读IOPS
    group.throughput(Throughput::Elements(1));
    group.bench_function("4k_sequential_read", |b| {
        b.to_async(&rt).iter(|| async {
            let mut buf = vec![0u8; 4096];
            let offset = black_box(12345u64 % 100000) * 4096;
            let _ = device.read_async(offset, &mut buf).await;
        });
    });

    // 4K顺序写IOPS
    group.throughput(Throughput::Elements(1));
    group.bench_function("4k_sequential_write", |b| {
        b.to_async(&rt).iter(|| async {
            let buf = vec![0xABu8; 4096];
            let offset = black_box(54321u64 % 100000) * 4096;
            let _ = device.write_async(offset, &buf).await;
        });
    });

    group.finish();
}

criterion_group!(
    name = benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(10))
        .warm_up_time(Duration::from_secs(3))
        .sample_size(100);
    targets =
        sequential_block_reads,
        sequential_block_writes,
        random_block_io,
        mixed_read_write_workload,
        concurrent_block_device_access,
        buffer_pool_impact_on_device_io,
        device_flush_performance,
        large_transfer_performance,
        iops_performance
);

criterion_main!(benches);
