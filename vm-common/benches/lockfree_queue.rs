//! 无锁队列性能基准测试
//!
//! 比较无锁队列与传统锁队列的性能差异
//!
//! 使用Throughput来精确测量不同场景下的吞吐量表现，包括：
//! - 元素数量级别的吞吐量变化
//! - 消息传递速率的性能影响
//! - 并发生产者/消费者的扩展性分析

use criterion::{
    Bencher, BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main,
};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use vm_common::lockfree::{
    BoundedLockFreeQueue, InstrumentedLockFreeQueue, LockFreeQueue, MpmcQueue,
};

/// 传统锁队列（用于比较）
struct LockedQueue<T> {
    queue: Arc<Mutex<Vec<T>>>,
}

impl<T> LockedQueue<T> {
    fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn push(&self, item: T) {
        let mut queue = self.queue.lock().unwrap();
        queue.push(item);
    }

    fn pop(&self) -> Option<T> {
        let mut queue = self.queue.lock().unwrap();
        queue.pop()
    }
}

/// 基准测试：无锁队列 vs 锁队列
fn bench_lockfree_vs_locked(c: &mut Criterion) {
    let mut group = c.benchmark_group("lockfree_vs_locked");

    // 无锁队列
    let lockfree_queue = Arc::new(LockFreeQueue::new());

    // 锁队列
    let locked_queue = Arc::new(LockedQueue::new());

    // 预热
    for i in 0..1000 {
        lockfree_queue.push(i).unwrap();
        let _ = lockfree_queue.pop();

        locked_queue.push(i);
        let _ = locked_queue.pop();
    }

    // 无锁队列基准
    group.bench_function("lockfree_queue", |b: &mut Bencher| {
        b.iter(|| {
            for i in 0..1000 {
                lockfree_queue.push(black_box(i)).unwrap();
                let _ = lockfree_queue.pop();
            }
        })
    });

    // 锁队列基准
    group.bench_function("locked_queue", |b: &mut Bencher| {
        b.iter(|| {
            for i in 0..1000 {
                locked_queue.push(black_box(i));
                let _ = locked_queue.pop();
            }
        })
    });

    group.finish();
}

/// 基准测试：不同队列类型的性能
fn bench_queue_types(c: &mut Criterion) {
    let mut group = c.benchmark_group("queue_types");

    // 基本无锁队列
    let basic_queue = Arc::new(LockFreeQueue::new());

    // 有界无锁队列
    let bounded_queue = Arc::new(BoundedLockFreeQueue::new(1000));

    // 带统计信息的无锁队列
    let instrumented_queue = Arc::new(InstrumentedLockFreeQueue::new());

    // 预热
    for i in 0..100 {
        basic_queue.push(i).unwrap();
        let _ = basic_queue.pop();

        bounded_queue.push(i).unwrap();
        let _ = bounded_queue.pop();

        instrumented_queue.push(i).unwrap();
        let _ = instrumented_queue.pop();
    }

    // 基本无锁队列基准
    group.bench_function("basic_lockfree", |b: &mut Bencher| {
        b.iter(|| {
            for i in 0..100 {
                basic_queue.push(black_box(i)).unwrap();
                let _ = basic_queue.pop();
            }
        })
    });

    // 有界无锁队列基准
    group.bench_function("bounded_lockfree", |b: &mut Bencher| {
        b.iter(|| {
            for i in 0..100 {
                bounded_queue.push(black_box(i)).unwrap();
                let _ = bounded_queue.pop();
            }
        })
    });

    // 带统计信息的无锁队列基准
    group.bench_function("instrumented_lockfree", |b: &mut Bencher| {
        b.iter(|| {
            for i in 0..100 {
                instrumented_queue.push(black_box(i)).unwrap();
                let _ = instrumented_queue.pop();
            }
        })
    });

    group.finish();
}

/// 基准测试：并发性能
fn bench_concurrent_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_performance");

    let thread_counts = [1, 2, 4, 8, 16];

    for &thread_count in &thread_counts {
        // 无锁队列
        group.bench_with_input(
            BenchmarkId::new("lockfree_concurrent", thread_count),
            &thread_count,
            |b: &mut Bencher, &thread_count| {
                b.iter(|| {
                    let queue = Arc::new(LockFreeQueue::new());
                    let mut handles = Vec::new();

                    // 生产者线程
                    for i in 0..thread_count / 2 {
                        let queue = queue.clone();
                        let handle = thread::spawn(move || {
                            for j in 0..1000 {
                                queue.push(black_box(i * 1000 + j)).unwrap();
                            }
                        });
                        handles.push(handle);
                    }

                    // 消费者线程
                    for _ in 0..thread_count / 2 {
                        let queue = queue.clone();
                        let handle = thread::spawn(move || {
                            for _ in 0..1000 {
                                while queue.try_pop().is_none() {
                                    thread::yield_now();
                                }
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );

        // 锁队列
        group.bench_with_input(
            BenchmarkId::new("locked_concurrent", thread_count),
            &thread_count,
            |b: &mut Bencher, &thread_count| {
                b.iter(|| {
                    let queue = Arc::new(LockedQueue::new());
                    let mut handles = Vec::new();

                    // 生产者线程
                    for i in 0..thread_count / 2 {
                        let queue = queue.clone();
                        let handle = thread::spawn(move || {
                            for j in 0..1000 {
                                queue.push(black_box(i * 1000 + j));
                            }
                        });
                        handles.push(handle);
                    }

                    // 消费者线程
                    for _ in 0..thread_count / 2 {
                        let queue = queue.clone();
                        let handle = thread::spawn(move || {
                            for _ in 0..1000 {
                                while queue.pop().is_none() {
                                    thread::yield_now();
                                }
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

/// 基准测试：MPMC队列性能
fn bench_mpmc_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("mpmc_performance");

    let producer_counts = [1, 2, 4, 8];
    let consumer_counts = [1, 2, 4, 8];

    for &producer_count in &producer_counts {
        for &consumer_count in &consumer_counts {
            group.bench_with_input(
                BenchmarkId::new("mpmc", format!("{}p{}c", producer_count, consumer_count)),
                &(producer_count, consumer_count),
                |b: &mut Bencher, &(producer_count, consumer_count)| {
                    b.iter(|| {
                        let queue = MpmcQueue::new();
                        let mut handles = Vec::new();

                        // 生产者线程
                        for i in 0..producer_count {
                            let producer = queue.create_producer();
                            let handle = thread::spawn(move || {
                                for j in 0..1000 {
                                    producer.push(black_box(i * 1000 + j)).unwrap();
                                }
                            });
                            handles.push(handle);
                        }

                        // 消费者线程
                        for _ in 0..consumer_count {
                            let consumer = queue.create_consumer();
                            let handle = thread::spawn(move || {
                                for _ in 0..(producer_count * 1000 / consumer_count) {
                                    while consumer.try_pop().is_none() {
                                        thread::yield_now();
                                    }
                                }
                            });
                            handles.push(handle);
                        }

                        for handle in handles {
                            handle.join().unwrap();
                        }
                    });
                },
            );
        }
    }

    group.finish();
}

/// 基准测试：队列容量影响
fn bench_capacity_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("capacity_impact");

    let capacities = [10, 100, 1000, 10000];

    for &capacity in &capacities {
        group.bench_with_input(
            BenchmarkId::new("bounded_queue", capacity),
            &capacity,
            |b: &mut Bencher, &capacity| {
                b.iter(|| {
                    let queue = BoundedLockFreeQueue::new(capacity);

                    // 填充队列到80%容量
                    for i in 0..(capacity * 8 / 10) {
                        queue.push(black_box(i)).unwrap();
                    }

                    // 执行混合操作
                    for i in 0..1000 {
                        if i % 3 == 0 {
                            // 入队操作
                            queue.push(black_box(i)).ok();
                        } else {
                            // 出队操作
                            queue.try_pop();
                        }
                    }
                });
            },
        );
    }

    group.finish();
}

/// 基准测试：队列统计开销
fn bench_stats_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("stats_overhead");

    // 无统计信息的队列
    let basic_queue = Arc::new(LockFreeQueue::new());

    // 有统计信息的队列
    let instrumented_queue = Arc::new(InstrumentedLockFreeQueue::new());

    // 无统计信息队列基准
    group.bench_function("without_stats", |b: &mut Bencher| {
        b.iter(|| {
            for i in 0..1000 {
                basic_queue.push(black_box(i)).unwrap();
                let _ = basic_queue.pop();
            }
        })
    });

    // 有统计信息队列基准
    group.bench_function("with_stats", |b: &mut Bencher| {
        b.iter(|| {
            for i in 0..1000 {
                instrumented_queue.push(black_box(i)).unwrap();
                let _ = instrumented_queue.pop();
            }
        })
    });

    group.finish();
}

/// 基准测试：内存分配模式
fn bench_allocation_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("allocation_patterns");

    // 小对象
    group.bench_function("small_objects", |b: &mut Bencher| {
        b.iter(|| {
            let queue = LockFreeQueue::new();
            for i in 0..1000 {
                queue.push(black_box(i as u8)).unwrap();
                let _ = queue.pop();
            }
        })
    });

    // 中等对象
    group.bench_function("medium_objects", |b: &mut Bencher| {
        b.iter(|| {
            let queue = LockFreeQueue::new();
            for i in 0..1000 {
                queue.push(black_box([i as u8; 64])).unwrap();
                let _ = queue.pop();
            }
        })
    });

    // 大对象
    group.bench_function("large_objects", |b: &mut Bencher| {
        b.iter(|| {
            let queue = LockFreeQueue::new();
            for i in 0..100 {
                queue.push(black_box([i as u8; 4096])).unwrap();
                let _ = queue.pop();
            }
        })
    });

    group.finish();
}

/// 基准测试：吞吐量测量（使用Throughput）
///
/// 使用Throughput精确测量不同数据规模下的队列吞吐量性能
fn bench_queue_throughput_measurement(c: &mut Criterion) {
    let mut group = c.benchmark_group("queue_throughput_measurement");

    let element_counts = [100, 1000, 10000, 100000];

    for &element_count in &element_counts {
        let throughput = Throughput::Elements(element_count);

        // 无锁队列吞吐量测试
        group.throughput(throughput);
        group.bench_with_input(
            BenchmarkId::new("lockfree_queue_throughput", element_count),
            &element_count,
            |b: &mut Bencher, &element_count| {
                b.iter(|| {
                    let queue = LockFreeQueue::new();

                    // 执行指定数量的入队和出队操作
                    for i in 0..element_count {
                        queue.push(black_box(i)).unwrap();
                    }

                    for _ in 0..element_count {
                        let _ = queue.try_pop();
                    }
                });
            },
        );

        // 传统锁队列吞吐量测试
        group.throughput(Throughput::Elements(element_count));
        group.bench_with_input(
            BenchmarkId::new("locked_queue_throughput", element_count),
            &element_count,
            |b: &mut Bencher, &element_count| {
                b.iter(|| {
                    let queue = LockedQueue::new();

                    // 执行指定数量的入队和出队操作
                    for i in 0..element_count {
                        queue.push(black_box(i));
                    }

                    for _ in 0..element_count {
                        let _ = queue.pop();
                    }
                });
            },
        );
    }

    group.finish();
}

/// 基准测试：字节吞吐量测量
///
/// 测试不同消息大小下的字节吞吐量，评估消息传递效率
fn bench_queue_bytes_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("queue_bytes_throughput");

    let message_sizes = [64, 256, 1024, 4096];

    for &message_size in &message_sizes {
        let bytes_per_element = message_size;
        let element_count = 1000;
        let total_bytes = bytes_per_element * element_count;

        group.throughput(Throughput::Bytes(total_bytes as u64));
        group.bench_with_input(
            BenchmarkId::new("lockfree_queue_bytes_throughput", message_size),
            &message_size,
            |b: &mut Bencher, &message_size| {
                b.iter(|| {
                    let queue = LockFreeQueue::new();

                    // 插入大消息
                    for _i in 0..element_count {
                        let message = vec![0u8; message_size];
                        queue.push(black_box(message)).unwrap();
                    }

                    // 弹出大消息
                    for _ in 0..element_count {
                        let _ = queue.try_pop();
                    }
                });
            },
        );
    }

    group.finish();
}

/// 基准测试：持续消息吞吐量
///
/// 模拟生产环境中的持续消息流处理能力
fn bench_sustained_message_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("sustained_message_throughput");

    let messages_per_second = 100_000;
    group.throughput(Throughput::Elements(messages_per_second));

    group.bench_function("sustained_processing", |b: &mut Bencher| {
        let queue = Arc::new(LockFreeQueue::new());

        b.iter(|| {
            // 模拟持续消息处理
            for i in 0..1000 {
                queue.push(black_box(i)).unwrap();
                let _ = queue.try_pop();
            }
        });
    });

    group.finish();
}

/// 基准测试：MPMC队列吞吐量分析
///
/// 使用Throughput分析多生产者多消费者场景的吞吐量特性
fn bench_mpmc_throughput_analysis(c: &mut Criterion) {
    let mut group = c.benchmark_group("mpmc_throughput");

    let configurations = [(2, 2), (4, 4), (8, 8)];

    for &(producers, consumers) in &configurations {
        let total_ops = producers * 1000;
        group.throughput(Throughput::Elements(total_ops));

        group.bench_with_input(
            BenchmarkId::new("mpmc_throughput", format!("{}p{}c", producers, consumers)),
            &(producers, consumers),
            |b: &mut Bencher, &(producers, consumers)| {
                b.iter(|| {
                    let queue = Arc::new(MpmcQueue::new());
                    let mut handles = Vec::new();

                    // 生产者线程
                    for p in 0..producers {
                        let queue = queue.clone();
                        let producer = queue.create_producer();
                        let handle = thread::spawn(move || {
                            for i in 0..1000 {
                                producer.push(black_box(p * 1000 + i)).unwrap();
                            }
                        });
                        handles.push(handle);
                    }

                    // 消费者线程
                    for _ in 0..consumers {
                        let queue = queue.clone();
                        let consumer = queue.create_consumer();
                        let handle = thread::spawn(move || {
                            for _ in 0..(producers * 1000 / consumers) {
                                while consumer.try_pop().is_none() {
                                    thread::yield_now();
                                }
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

/// 性能监控辅助结构
///
/// 提供队列性能指标的详细分析
#[allow(dead_code)]
struct QueuePerformanceMetrics {
    messages_processed: usize,
    total_bytes: u64,
    latency_ms: f64,
}

#[allow(dead_code)]
impl QueuePerformanceMetrics {
    fn new(messages_processed: usize, message_size: usize, duration: Duration) -> Self {
        let total_bytes = (messages_processed * message_size) as u64;
        let latency_ms = duration.as_secs_f64() / messages_processed.max(1) as f64 * 1000.0;

        Self {
            messages_processed,
            total_bytes,
            latency_ms,
        }
    }

    fn calculate_throughput(&self, duration_ns: u128) -> f64 {
        if duration_ns > 0 {
            (self.messages_processed as f64) / (duration_ns as f64 / 1_000_000_000.0)
        } else {
            0.0
        }
    }

    fn log_summary(&self) {
        log::info!(
            "队列性能指标: 消息数={}, 总字节={}, 延迟={}ms",
            self.messages_processed,
            self.total_bytes,
            self.latency_ms
        );
    }
}

criterion_group!(
    benches,
    bench_lockfree_vs_locked,
    bench_queue_types,
    bench_concurrent_performance,
    bench_mpmc_performance,
    bench_capacity_impact,
    bench_stats_overhead,
    bench_allocation_patterns,
    bench_queue_throughput_measurement,
    bench_queue_bytes_throughput,
    bench_sustained_message_throughput,
    bench_mpmc_throughput_analysis
);
criterion_main!(benches);
