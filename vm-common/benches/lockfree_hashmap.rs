//! 无锁哈希表性能基准测试
//!
//! 比较无锁哈希表与传统锁哈希表的性能差异

use criterion::{Bencher, BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::collections::HashMap as StdHashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use vm_common::lockfree::{
    LockFreeHashMap, StripedHashMap, CacheAwareHashMap, InstrumentedLockFreeHashMap,
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
                                map.insert(black_box(i * 1000 + j), black_box(i * 1000 + j)).unwrap();
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
                map.insert(black_box(i + 1000), black_box(i + 1000)).unwrap();
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
                    map.insert(black_box(i + 2000), black_box(i + 2000)).unwrap();
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
                map.insert(black_box(i), black_box([i as u8; 4096])).unwrap();
                let _ = map.get(&black_box(i));
            }
        })
    });
    
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
    bench_allocation_patterns
);
criterion_main!(benches);