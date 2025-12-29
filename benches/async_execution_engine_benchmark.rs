/// Week 3 - 异步执行性能基准测试
///
/// 对比异步和同步执行的性能指标
use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::time::Duration;

/// 基准测试：异步指令执行
fn async_instruction_execution(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("async_instruction_1k", |b| {
        b.to_async(&rt).iter(|| async {
            for _ in 0..1000 {
                // 模拟指令执行
                black_box(1u64 + 1u64);
                tokio::task::yield_now().await;
            }
        });
    });

    c.bench_function("async_instruction_10k", |b| {
        b.to_async(&rt).iter(|| async {
            for _ in 0..10000 {
                black_box(1u64 + 1u64);
            }
        });
    });

    c.bench_function("async_instruction_100k", |b| {
        b.to_async(&rt).iter(|| async {
            for _ in 0..100000 {
                black_box(1u64 + 1u64);
            }
        });
    });
}

/// 基准测试：多vCPU并发执行
fn multi_vcpu_concurrent_execution(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("multi_vcpu_concurrent");

    for vcpu_count in [1, 2, 4, 8].iter() {
        group.throughput(Throughput::Elements(*vcpu_count as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}vcpu", vcpu_count)),
            vcpu_count,
            |b, &vcpu_count| {
                b.to_async(&rt).iter(|| async {
                    let mut handles = vec![];

                    for _ in 0..vcpu_count {
                        let handle = tokio::spawn(async {
                            // 模拟vcpu执行
                            for _ in 0..1000 {
                                black_box(1u64 + 1u64);
                            }
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

/// 基准测试：异步上下文切换开销
fn async_context_switch_overhead(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("context_switch");

    group.bench_function("yield_overhead_low", |b| {
        b.to_async(&rt).iter(|| async {
            for _ in 0..1000 {
                tokio::task::yield_now().await;
            }
        });
    });

    group.bench_function("yield_overhead_medium", |b| {
        b.to_async(&rt).iter(|| async {
            for _ in 0..10000 {
                tokio::task::yield_now().await;
            }
        });
    });

    group.bench_function("yield_overhead_high", |b| {
        b.to_async(&rt).iter(|| async {
            for _ in 0..100000 {
                tokio::task::yield_now().await;
            }
        });
    });

    group.finish();
}

/// 基准测试：中断处理延迟
fn interrupt_handling_latency(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("interrupt_dispatch_latency", |b| {
        b.to_async(&rt).iter(|| async {
            // 模拟中断投递
            let start = std::time::Instant::now();

            for _ in 0..100 {
                black_box(std::time::Instant::now());
            }

            let _elapsed = start.elapsed();
        });
    });
}

/// 基准测试：细粒度锁竞争
fn fine_grained_lock_contention(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("fine_grained_locks");

    group.bench_function("multiple_locks_low_contention", |b| {
        b.to_async(&rt).iter(|| async {
            let locks: Vec<_> = (0..10)
                .map(|_| Arc::new(parking_lot::Mutex::new(0u64)))
                .collect();

            let mut handles = vec![];
            for lock in locks {
                let lock_clone = lock.clone();
                let handle = tokio::spawn(async move {
                    for _ in 0..100 {
                        let mut val = lock_clone.lock();
                        *val += 1;
                        drop(val);
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                let _ = handle.await;
            }
        });
    });

    group.bench_function("multiple_locks_high_contention", |b| {
        b.to_async(&rt).iter(|| async {
            let lock = Arc::new(parking_lot::Mutex::new(0u64));

            let mut handles = vec![];
            for _ in 0..10 {
                let lock_clone = lock.clone();
                let handle = tokio::spawn(async move {
                    for _ in 0..100 {
                        let mut val = lock_clone.lock();
                        *val += 1;
                        drop(val);
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

/// 基准测试：吞吐量对比（同步 vs 异步）
fn throughput_comparison(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("throughput");

    // 同步版本
    group.throughput(Throughput::Elements(1_000_000));
    group.bench_function("sync_instruction_throughput", |b| {
        b.iter(|| {
            let mut sum = 0u64;
            for i in 0..1_000_000 {
                sum = sum.wrapping_add(i);
            }
            black_box(sum);
        });
    });

    // 异步版本（有yield）
    group.throughput(Throughput::Elements(100_000));
    group.bench_function("async_instruction_throughput", |b| {
        b.to_async(&rt).iter(|| async {
            let mut sum = 0u64;
            for i in 0..100_000 {
                sum = sum.wrapping_add(i);
                if i % 1000 == 0 {
                    tokio::task::yield_now().await;
                }
            }
            black_box(sum);
        });
    });

    group.finish();
}

/// 基准测试：内存访问模式
fn memory_access_patterns(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("sequential_memory_access", |b| {
        b.to_async(&rt).iter(|| async {
            let data: Vec<u64> = vec![0; 10000];
            let mut sum = 0u64;
            for val in &data {
                sum = sum.wrapping_add(*val);
            }
            black_box(sum);
        });
    });

    c.bench_function("random_memory_access", |b| {
        b.to_async(&rt).iter(|| async {
            let data: Vec<u64> = vec![1; 10000];
            let mut sum = 0u64;
            for i in (0..10000).step_by(73) {
                // 质数步长以模拟随机访问
                sum = sum.wrapping_add(data[i % 10000]);
            }
            black_box(sum);
        });
    });
}

/// 基准测试：缩放效率
fn scaling_efficiency(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("scaling");

    for num_tasks in [1, 2, 4, 8, 16].iter() {
        group.throughput(Throughput::Elements(*num_tasks as u64));
        group.bench_with_input(
            BenchmarkId::new("concurrent_tasks", num_tasks),
            num_tasks,
            |b, &num_tasks| {
                b.to_async(&rt).iter(|| async {
                    let mut handles = vec![];

                    for task_id in 0..num_tasks {
                        let handle = tokio::spawn(async move {
                            let mut result = task_id as u64;
                            for _ in 0..100 {
                                result = result.wrapping_mul(2).wrapping_add(1);
                            }
                            black_box(result);
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

use std::sync::Arc;

criterion_group!(
    name = benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(10))
        .warm_up_time(Duration::from_secs(3))
        .sample_size(100);
    targets =
        async_instruction_execution,
        multi_vcpu_concurrent_execution,
        async_context_switch_overhead,
        interrupt_handling_latency,
        fine_grained_lock_contention,
        throughput_comparison,
        memory_access_patterns,
        scaling_efficiency
);

criterion_main!(benches);
