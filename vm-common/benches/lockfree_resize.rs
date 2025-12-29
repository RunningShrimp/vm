//! 无锁扩容性能基准测试
//!
//! 专门测试无锁哈希表在扩容场景下的性能表现

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::sync::Arc;
use std::thread;
use vm_common::lockfree::LockFreeHashMap;

/// 基准测试：单线程扩容性能
fn bench_single_thread_resize(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_thread_resize");

    let initial_sizes = [4, 8, 16, 32];
    let elements_per_size = [100, 500, 1000, 2000];

    for &initial_size in &initial_sizes {
        for &element_count in &elements_per_size {
            group.bench_with_input(
                BenchmarkId::new("resize", format!("{}_{}", initial_size, element_count)),
                &(initial_size, element_count),
                |b, &(initial_size, element_count)| {
                    b.iter(|| {
                        let map = LockFreeHashMap::with_capacity(initial_size);

                        for i in 0..element_count {
                            map.insert(black_box(i), black_box(i)).unwrap();
                        }

                        // 验证数据完整性
                        assert_eq!(map.len(), element_count);
                    });
                },
            );
        }
    }

    group.finish();
}

/// 基准测试：多线程并发扩容性能
fn bench_concurrent_resize(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_resize");

    let thread_counts = [2, 4, 8, 16];
    let initial_capacity = 8;

    for &thread_count in &thread_counts {
        group.throughput(Throughput::Elements(thread_count as u64 * 100));

        group.bench_with_input(
            BenchmarkId::new("concurrent_resize", thread_count),
            &thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let map = Arc::new(LockFreeHashMap::with_capacity(initial_capacity));
                    let mut handles = Vec::new();

                    // 启动多个线程并发插入
                    for i in 0..thread_count {
                        let map = map.clone();
                        let handle = thread::spawn(move || {
                            for j in 0..100 {
                                let key = i * 100 + j;
                                map.insert(black_box(key), black_box(key)).unwrap();
                            }
                        });
                        handles.push(handle);
                    }

                    // 等待所有线程完成
                    for handle in handles {
                        handle.join().unwrap();
                    }

                    // 验证数据完整性
                    assert_eq!(map.len(), thread_count * 100);
                });
            },
        );
    }

    group.finish();
}

/// 基准测试：扩容期间的读取性能
fn bench_read_during_resize(c: &mut Criterion) {
    let mut group = c.benchmark_group("read_during_resize");

    let reader_counts = [1, 2, 4, 8];

    for &reader_count in &reader_counts {
        group.bench_with_input(
            BenchmarkId::new("read_during_resize", reader_count),
            &reader_count,
            |b, &reader_count| {
                b.iter(|| {
                    let map = Arc::new(LockFreeHashMap::with_capacity(4));
                    let mut handles = Vec::new();

                    // 预填充一些数据
                    for i in 0..10 {
                        map.insert(i, i).unwrap();
                    }

                    // 启动读取线程
                    for _ in 0..reader_count {
                        let map = map.clone();
                        let handle = thread::spawn(move || {
                            for _ in 0..1000 {
                                let key = black_box(5); // 读取已存在的键
                                let _ = map.get(&key);
                            }
                        });
                        handles.push(handle);
                    }

                    // 启动写入线程触发扩容
                    let writer_handle = {
                        let map = map.clone();
                        thread::spawn(move || {
                            for i in 10..100 {
                                map.insert(black_box(i), black_box(i)).unwrap();
                            }
                        })
                    };

                    // 等待所有线程完成
                    for handle in handles {
                        handle.join().unwrap();
                    }
                    writer_handle.join().unwrap();
                });
            },
        );
    }

    group.finish();
}

/// 基准测试：多次连续扩容
fn bench_multiple_resizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("multiple_resizes");

    let resize_counts = [1, 2, 3, 5];

    for &resize_count in &resize_counts {
        group.throughput(Throughput::Elements((resize_count * 50) as u64));

        group.bench_with_input(
            BenchmarkId::new("multiple_resizes", resize_count),
            &resize_count,
            |b, &resize_count| {
                b.iter(|| {
                    let map = LockFreeHashMap::with_capacity(2);

                    for batch in 0..resize_count {
                        let start = batch * 50;
                        for i in start..start + 50 {
                            map.insert(black_box(i), black_box(i)).unwrap();
                        }

                        // 验证这批数据
                        for i in start..start + 50 {
                            assert_eq!(map.get(&i), Some(i));
                        }
                    }

                    assert_eq!(map.len(), resize_count * 50);
                });
            },
        );
    }

    group.finish();
}

/// 基准测试：混合工作负载下的扩容
fn bench_mixed_workload_resize(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_workload_resize");

    let operation_ratios = [(90, 10), (70, 30), (50, 50)]; // (read%, write%)

    for &(read_pct, write_pct) in &operation_ratios {
        group.bench_with_input(
            BenchmarkId::new("mixed_workload", format!("{}_{}", read_pct, write_pct)),
            &(read_pct, write_pct),
            |b, &(read_pct, write_pct)| {
                b.iter(|| {
                    let map = Arc::new(LockFreeHashMap::with_capacity(4));
                    let mut handles = Vec::new();

                    // 预填充
                    for i in 0..20 {
                        map.insert(i, i).unwrap();
                    }

                    // 启动工作线程
                    for thread_id in 0..4 {
                        let map = map.clone();
                        let handle = thread::spawn(move || {
                            let total_ops = 1000;
                            let write_count = total_ops * write_pct / 100;

                            for i in 0..total_ops {
                                if i < write_count {
                                    // 写操作
                                    let key = thread_id * 1000 + i;
                                    map.insert(black_box(key), black_box(key)).unwrap();
                                } else {
                                    // 读操作
                                    let key = i % 20;
                                    let _ = map.get(&black_box(key));
                                }
                            }
                        });
                        handles.push(handle);
                    }

                    // 等待所有线程完成
                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

/// 基准测试：扩容期间的吞吐量
fn bench_resize_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("resize_throughput");

    let element_counts = [100, 500, 1000, 2000, 5000];

    for &element_count in &element_counts {
        group.throughput(Throughput::Elements(element_count as u64));

        group.bench_with_input(
            BenchmarkId::new("throughput", element_count),
            &element_count,
            |b, &element_count| {
                b.iter(|| {
                    let map = LockFreeHashMap::with_capacity(8);

                    for i in 0..element_count {
                        map.insert(black_box(i), black_box(i)).unwrap();
                    }

                    assert_eq!(map.len(), element_count);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_single_thread_resize,
    bench_concurrent_resize,
    bench_read_during_resize,
    bench_multiple_resizes,
    bench_mixed_workload_resize,
    bench_resize_throughput
);
criterion_main!(benches);
