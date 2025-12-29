// 性能基准测试 - 异步执行引擎
//
// 这个文件包含用于测试VM执行性能的基准测试
// 运行: cargo bench --bench async_performance_benchmark
//
// 增强版: 添加了更全面的异步性能测试,包括统计分析和对比基准

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::sync::Arc;
use std::time::Duration;

// 模拟的基准测试场景
fn execute_simple_instructions(iterations: u64) -> u64 {
    let mut result = 0u64;
    for i in 0..iterations {
        result = result.wrapping_add(i);
        result = result.wrapping_mul(i.wrapping_add(1));
    }
    result
}

fn async_execution_basic(c: &mut Criterion) {
    c.bench_function("simple_execution_1k", |b| {
        b.iter(|| execute_simple_instructions(black_box(1000)))
    });

    c.bench_function("simple_execution_10k", |b| {
        b.iter(|| execute_simple_instructions(black_box(10000)))
    });

    c.bench_function("simple_execution_100k", |b| {
        b.iter(|| execute_simple_instructions(black_box(100000)))
    });
}

fn async_execution_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("execution_scaling");

    for iterations in [1000, 10000, 100000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(iterations),
            iterations,
            |b, &iterations| b.iter(|| execute_simple_instructions(black_box(iterations))),
        );
    }
    group.finish();
}

fn lock_contention_simulation(lock_count: u32, operations: u32) -> u64 {
    use parking_lot::Mutex;
    use std::sync::Arc;

    let counter = Arc::new(Mutex::new(0u64));
    let mut handles = vec![];

    for _ in 0..lock_count {
        let counter_clone = Arc::clone(&counter);
        let handle = std::thread::spawn(move || {
            for _ in 0..operations {
                let mut val = counter_clone.lock();
                *val = val.wrapping_add(1);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.join();
    }

    *counter.lock()
}

fn lock_contention_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("lock_contention");

    for thread_count in [2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("threads", thread_count),
            thread_count,
            |b, &thread_count| {
                b.iter(|| lock_contention_simulation(black_box(thread_count), black_box(100)))
            },
        );
    }
    group.finish();
}

/// 基准测试：异步yield性能 - 与同步版本对比
fn async_yield_performance(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("async_yield");

    // 同步版本 - 简单循环
    group.bench_function("sync_loop_10k", |b| {
        b.iter(|| {
            for _ in 0..10000 {
                black_box(1u64 + 1u64);
            }
        });
    });

    // 异步版本 - 带yield
    group.bench_function("async_yield_10k", |b| {
        b.to_async(&rt).iter(|| async {
            for _ in 0..10000 {
                black_box(1u64 + 1u64);
                tokio::task::yield_now().await;
            }
        });
    });

    // 异步版本 - 每100次yield一次
    group.bench_function("async_yield_100_10k", |b| {
        b.to_async(&rt).iter(|| async {
            for i in 0..10000 {
                black_box(1u64 + 1u64);
                if i % 100 == 0 {
                    tokio::task::yield_now().await;
                }
            }
        });
    });

    group.finish();
}

/// 基准测试：异步任务创建和销毁开销
fn async_task_overhead(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("task_overhead");

    // 单个任务创建和等待
    group.bench_function("single_task_spawn", |b| {
        b.to_async(&rt).iter(|| async {
            let handle = tokio::spawn(async {
                black_box(42u64);
            });
            black_box(handle.await.unwrap());
        });
    });

    // 批量任务创建和等待
    for count in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(
            BenchmarkId::new("batch_spawn", count),
            count,
            |b, &count| {
                b.to_async(&rt).iter(|| async {
                    let mut handles = Vec::with_capacity(count);
                    for _ in 0..count {
                        handles.push(tokio::spawn(async {
                            black_box(42u64);
                        }));
                    }
                    for handle in handles {
                        black_box(handle.await.unwrap());
                    }
                });
            },
        );
    }

    group.finish();
}

/// 基准测试：异步与同步内存访问对比
fn memory_access_comparison(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_access");

    // 同步版本 - 顺序访问
    group.bench_function("sync_sequential_1mb", |b| {
        b.iter(|| {
            let data: Vec<u64> = (0..16384).collect();
            let mut sum = 0u64;
            for &val in &data {
                sum = sum.wrapping_add(val);
            }
            black_box(sum);
        });
    });

    // 异步版本 - 顺序访问
    group.bench_function("async_sequential_1mb", |b| {
        b.to_async(&rt).iter(|| async {
            let data: Vec<u64> = (0..16384).collect();
            let mut sum = 0u64;
            for &val in &data {
                sum = sum.wrapping_add(val);
            }
            black_box(sum);
        });
    });

    // 同步版本 - 随机访问
    group.bench_function("sync_random_1mb", |b| {
        b.iter(|| {
            let data: Vec<u64> = (0..16384).collect();
            let mut sum = 0u64;
            for i in (0..16384).step_by(7) {
                sum = sum.wrapping_add(data[i]);
            }
            black_box(sum);
        });
    });

    // 异步版本 - 随机访问
    group.bench_function("async_random_1mb", |b| {
        b.to_async(&rt).iter(|| async {
            let data: Vec<u64> = (0..16384).collect();
            let mut sum = 0u64;
            for i in (0..16384).step_by(7) {
                sum = sum.wrapping_add(data[i]);
            }
            black_box(sum);
        });
    });

    group.finish();
}

/// 基准测试：异步锁竞争
fn async_lock_contention(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("async_lock_contention");

    // 低竞争 - 多个锁
    group.bench_function("low_contention_multiple_locks", |b| {
        b.to_async(&rt).iter(|| async {
            let locks: Vec<_> = (0..10)
                .map(|_| Arc::new(parking_lot::Mutex::new(0u64)))
                .collect();

            let mut handles = vec![];
            for lock in &locks {
                let lock_clone = lock.clone();
                handles.push(tokio::spawn(async move {
                    for _ in 0..100 {
                        let mut val = lock_clone.lock();
                        *val += 1;
                    }
                }));
            }

            for handle in handles {
                handle.await.unwrap();
            }
        });
    });

    // 高竞争 - 单个锁
    group.bench_function("high_contention_single_lock", |b| {
        b.to_async(&rt).iter(|| async {
            let lock = Arc::new(parking_lot::Mutex::new(0u64));

            let mut handles = vec![];
            for _ in 0..10 {
                let lock_clone = lock.clone();
                handles.push(tokio::spawn(async move {
                    for _ in 0..100 {
                        let mut val = lock_clone.lock();
                        *val += 1;
                    }
                }));
            }

            for handle in handles {
                handle.await.unwrap();
            }
        });
    });

    // RwLock - 读多写少
    group.bench_function("rwlock_read_heavy", |b| {
        b.to_async(&rt).iter(|| async {
            let lock = Arc::new(parking_lot::RwLock::new(vec![0u64; 1000]));
            let mut handles = vec![];

            // 10个读任务
            for _ in 0..10 {
                let lock_clone = lock.clone();
                handles.push(tokio::spawn(async move {
                    for _ in 0..100 {
                        let r = lock_clone.read();
                        black_box(&*r);
                    }
                }));
            }

            // 1个写任务
            let lock_clone = lock.clone();
            handles.push(tokio::spawn(async move {
                for _ in 0..10 {
                    let mut w = lock_clone.write();
                    w[0] += 1;
                }
            }));

            for handle in handles {
                handle.await.unwrap();
            }
        });
    });

    group.finish();
}

/// 基准测试：并发扩展性
fn concurrency_scalability(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(8)
        .enable_all()
        .build()
        .unwrap();

    let mut group = c.benchmark_group("concurrency_scalability");

    for concurrent_tasks in [1, 2, 4, 8, 16, 32].iter() {
        group.throughput(Throughput::Elements(*concurrent_tasks as u64));
        group.bench_with_input(
            BenchmarkId::new("concurrent_tasks", concurrent_tasks),
            concurrent_tasks,
            |b, &task_count| {
                b.to_async(&rt).iter(|| async {
                    let mut handles = vec![];

                    for task_id in 0..task_count {
                        handles.push(tokio::spawn(async move {
                            let mut result = task_id as u64;
                            // 模拟一些计算
                            for _ in 0..1000 {
                                result = result.wrapping_mul(2).wrapping_add(1);
                            }
                            black_box(result);
                        }));
                    }

                    for handle in handles {
                        handle.await.unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

/// 基准测试：异步I/O模拟
fn async_io_simulation(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();

    let mut group = c.benchmark_group("async_io_simulation");

    // 模拟I/O延迟
    group.bench_function("io_delay_1ms", |b| {
        b.to_async(&rt).iter(|| async {
            tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
            black_box(42u64);
        });
    });

    // 模拟批量I/O操作
    group.bench_function("batch_io_10ops", |b| {
        b.to_async(&rt).iter(|| async {
            let mut handles = vec![];
            for i in 0..10 {
                handles.push(tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_micros(100)).await;
                    black_box(i)
                }));
            }

            for handle in handles {
                handle.await.unwrap();
            }
        });
    });

    // 模拟并发I/O
    for concurrent_ops in [1, 5, 10, 20].iter() {
        group.throughput(Throughput::Elements(*concurrent_ops as u64));
        group.bench_with_input(
            BenchmarkId::new("concurrent_io", concurrent_ops),
            concurrent_ops,
            |b, &op_count| {
                b.to_async(&rt).iter(|| async {
                    let mut handles = vec![];

                    for i in 0..op_count {
                        handles.push(tokio::spawn(async move {
                            // 模拟I/O操作
                            tokio::time::sleep(tokio::time::Duration::from_micros(500)).await;
                            black_box(i)
                        }));
                    }

                    for handle in handles {
                        handle.await.unwrap();
                    }
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
        lock_contention_benchmark,
        async_yield_performance,
        async_task_overhead,
        memory_access_comparison,
        async_lock_contention,
        concurrency_scalability,
        async_io_simulation
);

criterion_main!(benches);
