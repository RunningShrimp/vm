/// 异步性能基准测试
///
/// 对比同步和异步I/O操作的性能表现
/// 使用criterion框架进行科学性能测试
///
/// 增强版: 添加更多异步操作基准测试,包括文件I/O、网络I/O、内存操作等
use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use vm_device::async_block_device::{AsyncBlockDevice, BlockDeviceConfig};
use vm_device::async_buffer_pool::{AsyncBufferPool, BufferPoolConfig};

/// 基准测试：简单异步执行
fn async_execution_basic(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("async_yield_1k", |b| {
        b.to_async(&rt).iter(|| async {
            for _ in 0..1000 {
                tokio::task::yield_now().await;
            }
        });
    });

    c.bench_function("async_yield_10k", |b| {
        b.to_async(&rt).iter(|| async {
            for _ in 0..10000 {
                tokio::task::yield_now().await;
            }
        });
    });

    c.bench_function("async_yield_100k", |b| {
        b.to_async(&rt).iter(|| async {
            for _ in 0..100000 {
                tokio::task::yield_now().await;
            }
        });
    });
}

/// 基准测试：异步执行扩展性
fn async_execution_scaling(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("async_scaling");

    for num_tasks in [1, 10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*num_tasks as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_tasks),
            num_tasks,
            |b, &num_tasks| {
                b.to_async(&rt).iter(|| async {
                    let mut handles = vec![];
                    for _ in 0..num_tasks {
                        let handle = tokio::spawn(async {
                            tokio::task::yield_now().await;
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

/// 基准测试：缓冲池性能
fn buffer_pool_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("buffer_pool");

    group.bench_function("pool_acquire_release", |b| {
        b.to_async(&rt).iter(|| async {
            let pool = AsyncBufferPool::new(BufferPoolConfig::default());
            let buf = pool.acquire().await.unwrap();
            black_box(buf);
        });
    });

    group.bench_function("pool_try_acquire", |b| {
        b.iter(|| {
            let pool = AsyncBufferPool::new(BufferPoolConfig::default());
            for _ in 0..100 {
                let _ = pool.try_acquire();
            }
        });
    });

    group.bench_function("pool_concurrent_acquire", |b| {
        b.to_async(&rt).iter(|| async {
            let pool = std::sync::Arc::new(AsyncBufferPool::new(BufferPoolConfig::default()));
            let mut handles = vec![];

            for _ in 0..10 {
                let pool_clone = pool.clone();
                let handle = tokio::spawn(async move {
                    let _ = pool_clone.acquire().await;
                });
                handles.push(handle);
            }

            for handle in handles {
                let _ = handle.await;
            }
        });
    });

    group.finish();
}

/// 基准测试：异步块设备读写
fn async_block_device_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("async_block_device");

    group.throughput(Throughput::Bytes(512));
    group.bench_function("read_512_bytes", |b| {
        b.to_async(&rt).iter(|| async {
            let device = AsyncBlockDevice::new_memory(10000, BufferPoolConfig::default());

            let mut buf = vec![0u8; 512];
            let _ = device.read_async(0, &mut buf).await;
            black_box(buf);
        });
    });

    group.throughput(Throughput::Bytes(512));
    group.bench_function("write_512_bytes", |b| {
        b.to_async(&rt).iter(|| async {
            let device = AsyncBlockDevice::new_memory(10000, BufferPoolConfig::default());

            let buf = vec![0xABu8; 512];
            let _ = device.write_async(0, &buf).await;
        });
    });

    group.throughput(Throughput::Bytes(512));
    group.bench_function("flush_device", |b| {
        b.to_async(&rt).iter(|| async {
            let device = AsyncBlockDevice::new_memory(10000, BufferPoolConfig::default());

            let _ = device.flush_async().await;
        });
    });

    group.finish();
}

/// 基准测试：并发I/O操作
fn concurrent_io_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("concurrent_io");

    for num_tasks in [1, 10, 100].iter() {
        group.throughput(Throughput::Elements(*num_tasks as u64 * 2)); // 每个任务执行读+写
        group.bench_with_input(
            BenchmarkId::new("read_write_tasks", num_tasks),
            num_tasks,
            |b, &num_tasks| {
                b.to_async(&rt).iter(|| async {
                    let device = std::sync::Arc::new(AsyncBlockDevice::new_memory(
                        100000,
                        BufferPoolConfig::default(),
                    ));

                    let mut handles = vec![];
                    for i in 0..num_tasks {
                        let device_clone = device.clone();
                        let handle = tokio::spawn(async move {
                            // 读操作
                            let mut buf = vec![0u8; 512];
                            let _ = device_clone.read_async((i as u64) * 100, &mut buf).await;

                            // 写操作
                            let buf = vec![(i as u8); 512];
                            let _ = device_clone.write_async((i as u64) * 100, &buf).await;
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

/// 基准测试：锁竞争模拟
fn lock_contention_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("lock_contention");

    group.bench_function("parking_lot_mutex_contention", |b| {
        b.to_async(&rt).iter(|| async {
            let counter = std::sync::Arc::new(parking_lot::Mutex::new(0u64));
            let mut handles = vec![];

            for _ in 0..10 {
                let counter_clone = counter.clone();
                let handle = tokio::spawn(async move {
                    for _ in 0..100 {
                        {
                            let mut c = counter_clone.lock();
                            *c += 1;
                        } // Guard dropped here
                        tokio::task::yield_now().await;
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                let _ = handle.await;
            }
        });
    });

    group.bench_function("rwlock_read_heavy", |b| {
        b.to_async(&rt).iter(|| async {
            let data = std::sync::Arc::new(parking_lot::RwLock::new(vec![0u64; 100]));
            let mut handles = vec![];

            for _ in 0..10 {
                let data_clone = data.clone();
                let handle = tokio::spawn(async move {
                    for _ in 0..100 {
                        {
                            let _d = data_clone.read();
                            // Use the data
                            black_box(&_d);
                        } // Guard dropped here
                        tokio::task::yield_now().await;
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                let _ = handle.await;
            }
        });
    });

    group.finish();
}

/// 基准测试：缓冲池命中率
fn buffer_pool_hit_rate_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("buffer_pool_hit_rate_100ops", |b| {
        b.to_async(&rt).iter(|| async {
            let pool = AsyncBufferPool::new(BufferPoolConfig {
                buffer_size: 4096,
                initial_pool_size: 10,
                max_pool_size: 100,
                max_pending_ops: 20,
            });

            for _ in 0..100 {
                let buf = pool.acquire().await.unwrap();
                black_box(buf);
            }

            let stats = pool.get_stats();
            black_box(stats);
        });
    });
}

/// 基准测试：异步文件I/O操作
fn async_file_io_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();

    let mut group = c.benchmark_group("async_file_io");

    // 准备测试文件
    let temp_dir = std::env::temp_dir();
    let test_file = temp_dir.join("benchmark_test.dat");

    // 预先创建测试文件
    rt.block_on(async {
        let mut file = tokio::fs::File::create(&test_file).await.unwrap();
        let data = vec![0u8; 1024 * 1024]; // 1MB
        for _ in 0..10 {
            file.write_all(&data).await.unwrap();
        }
        file.flush().await.unwrap();
    });

    // 小文件读取 (4KB)
    group.throughput(Throughput::Bytes(4096));
    group.bench_function("read_4kb", |b| {
        b.to_async(&rt).iter(|| async {
            let mut file = tokio::fs::File::open(&test_file).await.unwrap();
            let mut buf = vec![0u8; 4096];
            let _ = file.read_exact(&mut buf).await;
            black_box(buf);
        });
    });

    // 中等文件读取 (1MB)
    group.throughput(Throughput::Bytes(1024 * 1024));
    group.bench_function("read_1mb", |b| {
        b.to_async(&rt).iter(|| async {
            let mut file = tokio::fs::File::open(&test_file).await.unwrap();
            let mut buf = vec![0u8; 1024 * 1024];
            let _ = file.read_exact(&mut buf).await;
            black_box(buf);
        });
    });

    // 大文件读取 (10MB)
    group.throughput(Throughput::Bytes(10 * 1024 * 1024));
    group.bench_function("read_10mb", |b| {
        b.to_async(&rt).iter(|| async {
            let mut file = tokio::fs::File::open(&test_file).await.unwrap();
            let mut buf = vec![0u8; 10 * 1024 * 1024];
            let _ = file.read_exact(&mut buf).await;
            black_box(buf);
        });
    });

    // 清理测试文件
    rt.block_on(async {
        let _ = tokio::fs::remove_file(&test_file).await;
    });

    group.finish();
}

/// 基准测试：异步 vs 同步 I/O 性能对比
fn async_vs_sync_io_comparison(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();

    let mut group = c.benchmark_group("async_vs_sync");

    // 准备测试数据
    let data = vec![0xABu8; 1024 * 1024]; // 1MB

    // 同步写入
    group.throughput(Throughput::Bytes(1024 * 1024));
    group.bench_function("sync_write_1mb", |b| {
        b.iter(|| {
            let temp_dir = std::env::temp_dir();
            let test_file = temp_dir.join(format!(
                "sync_test_{}.dat",
                std::time::Instant::now().elapsed().as_nanos()
            ));

            let mut file = std::fs::File::create(&test_file).unwrap();
            std::io::Write::write_all(&mut file, black_box(&data)).unwrap();
            file.sync_all().unwrap();

            let _ = std::fs::remove_file(&test_file);
        });
    });

    // 异步写入
    group.throughput(Throughput::Bytes(1024 * 1024));
    group.bench_function("async_write_1mb", |b| {
        b.to_async(&rt).iter(|| async {
            let temp_dir = std::env::temp_dir();
            let test_file = temp_dir.join(format!(
                "async_test_{}.dat",
                std::time::Instant::now().elapsed().as_nanos()
            ));

            let mut file = tokio::fs::File::create(&test_file).await.unwrap();
            file.write_all(black_box(&data)).await.unwrap();
            file.flush().await.unwrap();

            let _ = tokio::fs::remove_file(&test_file).await;
        });
    });

    group.finish();
}

/// 基准测试：并发异步I/O操作
fn concurrent_async_io_operations(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(8)
        .enable_all()
        .build()
        .unwrap();

    let mut group = c.benchmark_group("concurrent_async_io");

    for concurrent_ops in [1, 4, 8, 16, 32].iter() {
        group.throughput(Throughput::Elements(*concurrent_ops as u64));
        group.bench_with_input(
            BenchmarkId::new("concurrent_reads", concurrent_ops),
            concurrent_ops,
            |b, &concurrent_ops| {
                b.to_async(&rt).iter(|| async {
                    // 创建内存块设备
                    let device = Arc::new(AsyncBlockDevice::new_memory(
                        100000,
                        BufferPoolConfig::default(),
                    ));

                    let mut handles = vec![];
                    for i in 0..concurrent_ops {
                        let device_clone = device.clone();
                        let handle = tokio::spawn(async move {
                            let mut buf = vec![0u8; 512];
                            let offset = (i * 100) as u64;
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

    group.finish();
}

/// 基准测试：异步任务调度开销
fn async_task_scheduling_overhead(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("task_scheduling");

    // 单个任务spawn开销
    group.bench_function("spawn_single_task", |b| {
        b.to_async(&rt).iter(|| async {
            let handle = tokio::spawn(async {
                black_box(1u64 + 1u64);
            });
            let _ = handle.await;
        });
    });

    // 批量任务spawn开销
    for task_count in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*task_count as u64));
        group.bench_with_input(
            BenchmarkId::new("spawn_batch", task_count),
            task_count,
            |b, &task_count| {
                b.to_async(&rt).iter(|| async {
                    let mut handles = vec![];
                    for _ in 0..task_count {
                        let handle = tokio::spawn(async {
                            black_box(1u64 + 1u64);
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

/// 基准测试：内存分配压力下的异步操作
fn async_memory_pressure_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_pressure");

    // 无内存压力
    group.bench_function("no_pressure", |b| {
        b.to_async(&rt).iter(|| async {
            let pool = AsyncBufferPool::new(BufferPoolConfig::default());
            let _buf = pool.acquire().await.unwrap();
        });
    });

    // 高内存压力
    group.bench_function("high_pressure", |b| {
        b.to_async(&rt).iter(|| async {
            let pool = Arc::new(AsyncBufferPool::new(BufferPoolConfig::default()));
            let mut handles = vec![];

            // 创建大量并发任务以产生内存压力
            for _ in 0..100 {
                let pool_clone = pool.clone();
                let handle = tokio::spawn(async move {
                    let _buf = pool_clone.acquire().await;
                    // 模拟一些工作
                    tokio::time::sleep(tokio::time::Duration::from_micros(10)).await;
                });
                handles.push(handle);
            }

            for handle in handles {
                let _ = handle.await;
            }
        });
    });

    group.finish();
}

/// 基准测试：不同异步运行时配置的性能
fn async_runtime_configuration_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("runtime_config");

    // 单线程运行时
    group.bench_function("single_threaded", |b| {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        b.to_async(&rt).iter(|| async {
            let mut handles = vec![];
            for _ in 0..100 {
                let handle = tokio::spawn(async {
                    black_box(1u64 + 1u64);
                });
                handles.push(handle);
            }
            for handle in handles {
                let _ = handle.await;
            }
        });
    });

    // 多线程运行时 (2线程)
    group.bench_function("multi_threaded_2", |b| {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();

        b.to_async(&rt).iter(|| async {
            let mut handles = vec![];
            for _ in 0..100 {
                let handle = tokio::spawn(async {
                    black_box(1u64 + 1u64);
                });
                handles.push(handle);
            }
            for handle in handles {
                let _ = handle.await;
            }
        });
    });

    // 多线程运行时 (4线程)
    group.bench_function("multi_threaded_4", |b| {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()
            .unwrap();

        b.to_async(&rt).iter(|| async {
            let mut handles = vec![];
            for _ in 0..100 {
                let handle = tokio::spawn(async {
                    black_box(1u64 + 1u64);
                });
                handles.push(handle);
            }
            for handle in handles {
                let _ = handle.await;
            }
        });
    });

    group.finish();
}

/// 基准测试：I/O吞吐量测试
fn io_throughput_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();

    let mut group = c.benchmark_group("io_throughput");

    let device = Arc::new(AsyncBlockDevice::new_memory(
        1000000, // 更大的设备
        BufferPoolConfig::default(),
    ));

    // 顺序读吞吐量
    for size in [512, 4096, 65536].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("sequential_read", size),
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

    // 顺序写吞吐量
    for size in [512, 4096, 65536].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("sequential_write", size),
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

criterion_group!(
    name = benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(10))
        .warm_up_time(Duration::from_secs(3))
        .sample_size(100);
    targets =
        async_execution_basic,
        async_execution_scaling,
        buffer_pool_benchmark,
        async_block_device_benchmark,
        concurrent_io_benchmark,
        lock_contention_benchmark,
        buffer_pool_hit_rate_benchmark,
        async_file_io_benchmark,
        async_vs_sync_io_comparison,
        concurrent_async_io_operations,
        async_task_scheduling_overhead,
        async_memory_pressure_benchmark,
        async_runtime_configuration_comparison,
        io_throughput_benchmark
);

criterion_main!(benches);
