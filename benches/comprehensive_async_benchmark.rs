/// 异步性能基准测试
///
/// 对比同步和异步I/O操作的性能表现
/// 使用criterion框架进行科学性能测试

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use vm_device::async_buffer_pool::{AsyncBufferPool, BufferPoolConfig};
use vm_device::async_block_device::{AsyncBlockDevice, BlockDeviceConfig};
use std::time::Duration;

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
            let device = AsyncBlockDevice::new_memory(
                10000,
                BufferPoolConfig::default(),
            );
            
            let mut buf = vec![0u8; 512];
            let _ = device.read_async(0, &mut buf).await;
            black_box(buf);
        });
    });

    group.throughput(Throughput::Bytes(512));
    group.bench_function("write_512_bytes", |b| {
        b.to_async(&rt).iter(|| async {
            let device = AsyncBlockDevice::new_memory(
                10000,
                BufferPoolConfig::default(),
            );
            
            let buf = vec![0xABu8; 512];
            let _ = device.write_async(0, &buf).await;
        });
    });

    group.throughput(Throughput::Bytes(512));
    group.bench_function("flush_device", |b| {
        b.to_async(&rt).iter(|| async {
            let device = AsyncBlockDevice::new_memory(
                10000,
                BufferPoolConfig::default(),
            );
            
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
                        let mut c = counter_clone.lock();
                        *c += 1;
                        drop(c);
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
                        let _d = data_clone.read();
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
        buffer_pool_hit_rate_benchmark
);

criterion_main!(benches);
