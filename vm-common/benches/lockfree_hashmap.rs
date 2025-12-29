//! 无锁哈希表性能基准测试
//!
//! 比较无锁哈希表与传统锁哈希表的性能差异
//!
//! 使用Throughput来精确测量不同场景下的吞吐量表现，包括：
//! - 元素数量级别的吞吐量变化
//! - 操作密集度的性能影响
//! - 并发程度的扩展性分析

use criterion::{
    Bencher, BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main,
};
use std::collections::HashMap as StdHashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use vm_common::lockfree::{
    CacheAwareHashMap, InstrumentedLockFreeHashMap, LockFreeHashMap, StripedHashMap,
};

/// 传统锁哈希表（用于比较）
type LockedHashMap<K, V> = Arc<Mutex<StdHashMap<K, V>>>;

/// 基准测试：无锁哈希表 vs 锁哈希表
fn bench_lockfree_vs_locked(c: &mut Criterion) {
    let mut group = c.benchmark_group("lockfree_vs_locked");

    // 无锁哈希表
    let lockfree_map = Arc::new(LockFreeHashMap::new());

    // 锁哈希表
    let locked_map = LockedHashMap::new(Mutex::new(StdHashMap::new()));

    // 预热
    for i in 0..1000 {
        lockfree_map.insert(i, i).unwrap();
        let _ = lockfree_map.get(&i);

        let mut map = locked_map.lock().unwrap();
        map.insert(i, i);
        let _ = map.get(&i);
    }

    // 无锁哈希表基准
    group.bench_function("lockfree_hashmap", |b: &mut Bencher| {
        b.iter(|| {
            for i in 0..1000 {
                lockfree_map.insert(black_box(i), black_box(i)).unwrap();
                let _ = lockfree_map.get(&black_box(i));
            }
        })
    });

    // 锁哈希表基准
    group.bench_function("locked_hashmap", |b: &mut Bencher| {
        b.iter(|| {
            for i in 0..1000 {
                let mut map = locked_map.lock().unwrap();
                map.insert(black_box(i), black_box(i));
                let _ = map.get(&black_box(i));
            }
        })
    });

    group.finish();
}

/// 基准测试：不同哈希表类型的性能
fn bench_hashmap_types(c: &mut Criterion) {
    let mut group = c.benchmark_group("hashmap_types");

    // 基本无锁哈希表
    let basic_map = Arc::new(LockFreeHashMap::new());

    // 分片无锁哈希表
    let striped_map = Arc::new(StripedHashMap::with_shards(4));

    // 缓存感知无锁哈希表
    let cache_map = Arc::new(CacheAwareHashMap::new(100));

    // 带统计信息的无锁哈希表
    let instrumented_map = Arc::new(InstrumentedLockFreeHashMap::new());

    // 预热
    for i in 0..100 {
        basic_map.insert(i, i).unwrap();
        let _ = basic_map.get(&i);

        striped_map.insert(i, i).unwrap();
        let _ = striped_map.get(&i);

        cache_map.insert(i, i).unwrap();
        let _ = cache_map.get(&i);

        instrumented_map.insert(i, i).unwrap();
        let _ = instrumented_map.get(&i);
    }

    // 基本无锁哈希表基准
    group.bench_function("basic_lockfree", |b: &mut Bencher| {
        b.iter(|| {
            for i in 0..100 {
                basic_map.insert(black_box(i), black_box(i)).unwrap();
                let _ = basic_map.get(&black_box(i));
            }
        })
    });

    // 分片无锁哈希表基准
    group.bench_function("striped_lockfree", |b: &mut Bencher| {
        b.iter(|| {
            for i in 0..100 {
                striped_map.insert(black_box(i), black_box(i)).unwrap();
                let _ = striped_map.get(&black_box(i));
            }
        })
    });

    // 缓存感知无锁哈希表基准
    group.bench_function("cache_aware_lockfree", |b: &mut Bencher| {
        b.iter(|| {
            for i in 0..100 {
                cache_map.insert(black_box(i), black_box(i)).unwrap();
                let _ = cache_map.get(&black_box(i));
            }
        })
    });

    // 带统计信息的无锁哈希表基准
    group.bench_function("instrumented_lockfree", |b: &mut Bencher| {
        b.iter(|| {
            for i in 0..100 {
                instrumented_map.insert(black_box(i), black_box(i)).unwrap();
                let _ = instrumented_map.get(&black_box(i));
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
        // 无锁哈希表
        group.bench_with_input(
            BenchmarkId::new("lockfree_concurrent", thread_count),
            &thread_count,
            |b: &mut Bencher, &thread_count| {
                b.iter(|| {
                    let map = Arc::new(LockFreeHashMap::new());
                    let mut handles = Vec::new();

                    // 生产者线程
                    for i in 0..thread_count / 2 {
                        let map = map.clone();
                        let handle = thread::spawn(move || {
                            for j in 0..1000 {
                                map.insert(black_box(i * 1000 + j), black_box(i * 1000 + j))
                                    .unwrap();
                            }
                        });
                        handles.push(handle);
                    }

                    // 消费者线程
                    for _ in 0..thread_count / 2 {
                        let map = map.clone();
                        let handle = thread::spawn(move || {
                            for j in 0..1000 {
                                while map.get(&black_box(j)).is_none() {
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

        // 锁哈希表
        group.bench_with_input(
            BenchmarkId::new("locked_concurrent", thread_count),
            &thread_count,
            |b: &mut Bencher, &thread_count| {
                b.iter(|| {
                    let map = LockedHashMap::new(Mutex::new(StdHashMap::new()));
                    let mut handles = Vec::new();

                    // 生产者线程
                    for i in 0..thread_count / 2 {
                        let map = map.clone();
                        let handle = thread::spawn(move || {
                            for j in 0..1000 {
                                let mut m = map.lock().unwrap();
                                m.insert(black_box(i * 1000 + j), black_box(i * 1000 + j));
                            }
                        });
                        handles.push(handle);
                    }

                    // 消费者线程
                    for _ in 0..thread_count / 2 {
                        let map = map.clone();
                        let handle = thread::spawn(move || {
                            for j in 0..1000 {
                                let m = map.lock().unwrap();
                                let _ = m.get(&black_box(j));
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

/// 基准测试：分片数量影响
fn bench_shard_count_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("shard_count_impact");

    let shard_counts = [1, 2, 4, 8, 16, 32];

    for &shard_count in &shard_counts {
        group.bench_with_input(
            BenchmarkId::new("striped_hashmap", shard_count),
            &shard_count,
            |b: &mut Bencher, &shard_count| {
                b.iter(|| {
                    let map = StripedHashMap::with_shards(shard_count);

                    // 执行混合操作
                    for i in 0..1000 {
                        map.insert(black_box(i), black_box(i)).unwrap();
                        let _ = map.get(&black_box(i));
                    }
                });
            },
        );
    }

    group.finish();
}

/// 基准测试：缓存大小影响
fn bench_cache_size_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_size_impact");

    let cache_sizes = [10, 50, 100, 500, 1000];

    for &cache_size in &cache_sizes {
        group.bench_with_input(
            BenchmarkId::new("cache_aware_hashmap", cache_size),
            &cache_size,
            |b: &mut Bencher, &cache_size| {
                b.iter(|| {
                    let map = CacheAwareHashMap::new(cache_size);

                    // 执行混合操作
                    for i in 0..1000 {
                        map.insert(black_box(i), black_box(i)).unwrap();
                        let _ = map.get(&black_box(i));
                    }
                });
            },
        );
    }

    group.finish();
}

/// 基准测试：哈希表容量影响
fn bench_capacity_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("capacity_impact");

    let capacities = [16, 64, 256, 1024, 4096];

    for &capacity in &capacities {
        group.bench_with_input(
            BenchmarkId::new("lockfree_hashmap", capacity),
            &capacity,
            |b: &mut Bencher, &capacity| {
                b.iter(|| {
                    let map = LockFreeHashMap::with_capacity(capacity);

                    // 执行混合操作
                    for i in 0..1000 {
                        map.insert(black_box(i), black_box(i)).unwrap();
                        let _ = map.get(&black_box(i));
                    }
                });
            },
        );
    }

    group.finish();
}

/// 基准测试：统计开销
fn bench_stats_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("stats_overhead");

    // 无统计信息的哈希表
    let basic_map = Arc::new(LockFreeHashMap::new());

    // 有统计信息的哈希表
    let instrumented_map = Arc::new(InstrumentedLockFreeHashMap::new());

    // 无统计信息哈希表基准
    group.bench_function("without_stats", |b: &mut Bencher| {
        b.iter(|| {
            for i in 0..1000 {
                basic_map.insert(black_box(i), black_box(i)).unwrap();
                let _ = basic_map.get(&black_box(i));
            }
        })
    });

    // 有统计信息哈希表基准
    group.bench_function("with_stats", |b: &mut Bencher| {
        b.iter(|| {
            for i in 0..1000 {
                instrumented_map.insert(black_box(i), black_box(i)).unwrap();
                let _ = instrumented_map.get(&black_box(i));
            }
        })
    });

    group.finish();
}

/// 基准测试：不同操作类型的性能
fn bench_operation_types(c: &mut Criterion) {
    let mut group = c.benchmark_group("operation_types");

    let map = Arc::new(LockFreeHashMap::new());

    // 预热
    for i in 0..1000 {
        map.insert(i, i).unwrap();
    }

    // 插入操作基准
    group.bench_function("insert_operations", |b: &mut Bencher| {
        b.iter(|| {
            for i in 0..1000 {
                map.insert(black_box(i + 1000), black_box(i + 1000))
                    .unwrap();
            }
        })
    });

    // 查找操作基准
    group.bench_function("lookup_operations", |b: &mut Bencher| {
        b.iter(|| {
            for i in 0..1000 {
                let _ = map.get(&black_box(i));
            }
        })
    });

    // 删除操作基准
    group.bench_function("remove_operations", |b: &mut Bencher| {
        b.iter(|| {
            for i in 0..1000 {
                let _ = map.remove(&black_box(i));
            }
        })
    });

    // 混合操作基准
    group.bench_function("mixed_operations", |b: &mut Bencher| {
        b.iter(|| {
            for i in 0..1000 {
                if i % 3 == 0 {
                    map.insert(black_box(i + 2000), black_box(i + 2000))
                        .unwrap();
                } else if i % 3 == 1 {
                    let _ = map.get(&black_box(i));
                } else {
                    let _ = map.remove(&black_box(i));
                }
            }
        })
    });

    group.finish();
}

/// 基准测试：内存分配模式
fn bench_allocation_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("allocation_patterns");

    // 小键值对
    group.bench_function("small_key_value_pairs", |b: &mut Bencher| {
        b.iter(|| {
            let map = LockFreeHashMap::new();
            for i in 0..1000 {
                map.insert(black_box(i as u8), black_box(i as u8)).unwrap();
                let _ = map.get(&black_box(i as u8));
            }
        })
    });

    // 中等键值对
    group.bench_function("medium_key_value_pairs", |b: &mut Bencher| {
        b.iter(|| {
            let map = LockFreeHashMap::new();
            for i in 0..1000 {
                map.insert(black_box(i), black_box([i as u8; 64])).unwrap();
                let _ = map.get(&black_box(i));
            }
        })
    });

    // 大键值对
    group.bench_function("large_key_value_pairs", |b: &mut Bencher| {
        b.iter(|| {
            let map = LockFreeHashMap::new();
            for i in 0..100 {
                map.insert(black_box(i), black_box([i as u8; 4096]))
                    .unwrap();
                let _ = map.get(&black_box(i));
            }
        })
    });

    group.finish();
}

/// 基准测试：吞吐量测量（使用Throughput）
///
/// 使用Throughput精确测量不同数据规模下的吞吐量性能
/// 这是clippy提示的Throughput导入的实际使用场景
fn bench_throughput_measurement(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput_measurement");

    let element_counts = [100, 1000, 10000, 100000];

    for &element_count in &element_counts {
        let throughput = Throughput::Elements(element_count);

        // 无锁哈希表吞吐量测试
        group.throughput(throughput);
        group.bench_with_input(
            BenchmarkId::new("lockfree_hashmap_throughput", element_count),
            &element_count,
            |b: &mut Bencher, &element_count| {
                b.iter(|| {
                    let map = LockFreeHashMap::new();

                    // 执行指定数量的插入和查找操作
                    for i in 0..element_count {
                        map.insert(black_box(i), black_box(i)).unwrap();
                    }

                    for i in 0..element_count {
                        let _ = map.get(&black_box(i));
                    }
                });
            },
        );

        // 传统锁哈希表吞吐量测试
        group.throughput(Throughput::Elements(element_count));
        group.bench_with_input(
            BenchmarkId::new("locked_hashmap_throughput", element_count),
            &element_count,
            |b: &mut Bencher, &element_count| {
                b.iter(|| {
                    let map = Mutex::new(StdHashMap::new());

                    // 执行指定数量的插入和查找操作
                    for i in 0..element_count {
                        let mut m = map.lock().unwrap();
                        m.insert(black_box(i), black_box(i));
                    }

                    for i in 0..element_count {
                        let m = map.lock().unwrap();
                        let _ = m.get(&black_box(i));
                    }
                });
            },
        );
    }

    group.finish();
}

/// 基准测试：字节吞吐量测量
///
/// 测试不同数据大小下的字节吞吐量，评估内存带宽利用率
fn bench_bytes_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("bytes_throughput");

    let data_sizes = [64, 256, 1024, 4096];

    for &data_size in &data_sizes {
        let bytes_per_element = data_size * 2; // 键和值的大小
        let element_count = 1000;
        let total_bytes = bytes_per_element * element_count;

        group.throughput(Throughput::Bytes(total_bytes as u64));
        group.bench_with_input(
            BenchmarkId::new("lockfree_bytes_throughput", data_size),
            &data_size,
            |b: &mut Bencher, &data_size| {
                b.iter(|| {
                    let map = LockFreeHashMap::new();

                    // 插入大数据对象
                    for i in 0..element_count {
                        let key = i;
                        let value = vec![0u8; data_size];
                        map.insert(black_box(key), black_box(value)).unwrap();
                    }

                    // 查询大数据对象
                    for i in 0..element_count {
                        let _ = map.get(&black_box(i));
                    }
                });
            },
        );
    }

    group.finish();
}

/// 基准测试：实时吞吐量分析
///
/// 模拟实际应用场景中的实时吞吐量需求
fn bench_realtime_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("realtime_throughput");

    // 使用Throughput::Bytes测量实时数据吞吐
    let ops_per_second = 100_000;
    group.throughput(Throughput::Elements(ops_per_second));

    group.bench_function("sustained_throughput", |b: &mut Bencher| {
        let map = Arc::new(LockFreeHashMap::new());

        // 预填充数据
        for i in 0..10_000 {
            map.insert(i, i).unwrap();
        }

        b.iter(|| {
            // 模拟持续吞吐量工作负载
            for i in 0..1000 {
                let key = i % 10_000;
                if i % 2 == 0 {
                    map.insert(black_box(key), black_box(key)).unwrap();
                } else {
                    let _ = map.get(&black_box(key));
                }
            }
        });
    });

    group.finish();
}

/// 性能分析辅助函数
///
/// 提供详细的性能指标分析，增强基准测试的可观测性
#[allow(dead_code)]
struct PerformanceMetrics {
    total_operations: usize,
    duration_ns: u128,
    throughput_ops_per_sec: f64,
}

#[allow(dead_code)]
impl PerformanceMetrics {
    fn new(total_operations: usize, duration: Duration) -> Self {
        let duration_ns = duration.as_nanos();
        let throughput_ops_per_sec = if duration_ns > 0 {
            (total_operations as f64) / (duration_ns as f64 / 1_000_000_000.0)
        } else {
            0.0
        };

        Self {
            total_operations,
            duration_ns,
            throughput_ops_per_sec,
        }
    }

    fn log(&self) {
        log::info!(
            "性能指标: 总操作数={}, 耗时={}ns, 吞吐量={:.2} ops/sec",
            self.total_operations,
            self.duration_ns,
            self.throughput_ops_per_sec
        );
    }
}

/// 辅助基准测试：混合负载吞吐量
///
/// 测试读写混合场景下的吞吐量表现
fn bench_mixed_workload_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_workload");

    let read_ratios = [0.1, 0.5, 0.9];

    for &read_ratio in &read_ratios {
        let operation_count = 10_000;
        group.throughput(Throughput::Elements(operation_count));

        group.bench_with_input(
            BenchmarkId::new("mixed_read_write_ratio", read_ratio),
            &read_ratio,
            |b: &mut Bencher, &read_ratio| {
                b.iter(|| {
                    let map = Arc::new(LockFreeHashMap::new());

                    // 预填充
                    for i in 0..1000 {
                        map.insert(i, i).unwrap();
                    }

                    // 执行混合操作
                    for i in 0..operation_count {
                        if (i as f64) / (operation_count as f64) < read_ratio {
                            // 读操作
                            let _ = map.get(&black_box(i % 1000));
                        } else {
                            // 写操作
                            map.insert(black_box(i), black_box(i)).unwrap();
                        }
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_lockfree_vs_locked,
    bench_hashmap_types,
    bench_concurrent_performance,
    bench_shard_count_impact,
    bench_cache_size_impact,
    bench_capacity_impact,
    bench_stats_overhead,
    bench_operation_types,
    bench_allocation_patterns,
    bench_throughput_measurement,
    bench_bytes_throughput,
    bench_realtime_throughput,
    bench_mixed_workload_throughput
);
criterion_main!(benches);
