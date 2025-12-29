/// GC性能基准测试
///
/// 测试不同堆大小下的GC性能，包括：
/// - GC暂停时间分布
/// - 内存回收效率
/// - 并发GC性能
/// - 写屏障性能
/// - 动态配额调整效果
///
/// 运行: cargo bench --bench gc_performance_benchmark

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use vm_engine::jit::{UnifiedGC, UnifiedGcConfig, gc_sweeper::GcSweeper};
use std::time::Duration;
use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

/// 模拟对象分配（用于测试）
struct MockObject {
    data: Vec<u8>,
    references: Vec<u64>,
}

impl MockObject {
    fn new(size: usize) -> Self {
        Self {
            data: vec![0u8; size],
            references: Vec::new(),
        }
    }

    fn add_reference(&mut self, addr: u64) {
        self.references.push(addr);
    }
}

/// 基准测试：不同堆大小下的GC性能
fn gc_performance_by_heap_size(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc_heap_size");
    
    // 测试不同堆大小：1MB, 10MB, 100MB, 1GB
    let heap_sizes = vec![
        (1 * 1024 * 1024, "1MB"),
        (10 * 1024 * 1024, "10MB"),
        (100 * 1024 * 1024, "100MB"),
        (1024 * 1024 * 1024, "1GB"),
    ];
    
    for (heap_size, label) in heap_sizes {
        group.throughput(Throughput::Bytes(heap_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(label),
            &heap_size,
            |b, &heap_size| {
                let config = UnifiedGcConfig {
                    heap_size_limit: heap_size,
                    mark_quota_us: 1000,
                    sweep_quota_us: 500,
                    adaptive_quota: true,
                    write_barrier_shards: 0, // 自动计算
                    ..Default::default()
                };
                
                let gc = UnifiedGC::new(config);
                
                // 模拟分配对象
                let mut roots = Vec::new();
                let object_size = 1024; // 每个对象1KB
                let num_objects = heap_size / object_size;
                
                for i in 0..num_objects.min(10000) {
                    roots.push(i as u64 * object_size as u64);
                }
                
                b.iter(|| {
                    // 启动GC周期
                    let cycle_start = gc.start_gc(black_box(&roots));
                    
                    // 执行增量标记
                    let mut marked_total = 0;
                    loop {
                        let (complete, marked) = gc.incremental_mark();
                        marked_total += marked;
                        if complete {
                            break;
                        }
                    }
                    
                    // 完成标记
                    gc.terminate_marking();
                    
                    // 执行增量清扫
                    let mut freed_total = 0;
                    loop {
                        let (complete, freed) = gc.incremental_sweep();
                        freed_total += freed;
                        if complete {
                            break;
                        }
                    }
                    
                    // 完成GC周期
                    gc.finish_gc(cycle_start);
                    
                    black_box((marked_total, freed_total));
                });
            },
        );
    }
    
    group.finish();
}

/// 基准测试：GC暂停时间分布
fn gc_pause_time_distribution(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc_pause_time");
    
    let config = UnifiedGcConfig {
        heap_size_limit: 100 * 1024 * 1024, // 100MB
        mark_quota_us: 1000,
        sweep_quota_us: 500,
        adaptive_quota: true,
        write_barrier_shards: 0,
        ..Default::default()
    };
    
    let gc = UnifiedGC::new(config);
    
    // 准备根对象
    let roots: Vec<u64> = (0..1000).map(|i| i as u64 * 1024).collect();
    
    group.bench_function("mark_phase", |b| {
        b.iter(|| {
            let cycle_start = gc.start_gc(black_box(&roots));
            let (complete, _) = gc.incremental_mark();
            black_box(complete);
            gc.finish_gc(cycle_start);
        });
    });
    
    group.bench_function("sweep_phase", |b| {
        b.iter(|| {
            gc.terminate_marking();
            let (complete, _) = gc.incremental_sweep();
            black_box(complete);
        });
    });
    
    group.bench_function("full_cycle", |b| {
        b.iter(|| {
            let cycle_start = gc.start_gc(black_box(&roots));
            
            // 标记阶段
            loop {
                let (complete, _) = gc.incremental_mark();
                if complete {
                    break;
                }
            }
            gc.terminate_marking();
            
            // 清扫阶段
            loop {
                let (complete, _) = gc.incremental_sweep();
                if complete {
                    break;
                }
            }
            
            gc.finish_gc(cycle_start);
            
            // 获取统计信息
            let stats = gc.stats();
            black_box((
                stats.get_avg_pause_us(),
                stats.get_max_pause_us(),
                stats.get_last_pause_us(),
            ));
        });
    });
    
    group.finish();
}

/// 基准测试：写屏障性能
fn write_barrier_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("write_barrier");
    
    let config = UnifiedGcConfig {
        heap_size_limit: 100 * 1024 * 1024,
        write_barrier_shards: 0, // 自动计算
        ..Default::default()
    };
    
    let gc = UnifiedGC::new(config);
    
    // 启动GC周期以启用写屏障
    let roots: Vec<u64> = vec![0x1000, 0x2000, 0x3000];
    let _cycle_start = gc.start_gc(&roots);
    
    group.bench_function("write_barrier_1k", |b| {
        b.iter(|| {
            for i in 0..1000 {
                gc.write_barrier(
                    black_box(i as u64 * 1024),
                    black_box((i + 1) as u64 * 1024),
                );
            }
        });
    });
    
    group.bench_function("write_barrier_10k", |b| {
        b.iter(|| {
            for i in 0..10000 {
                gc.write_barrier(
                    black_box(i as u64 * 1024),
                    black_box((i + 1) as u64 * 1024),
                );
            }
        });
    });
    
    group.bench_function("write_barrier_100k", |b| {
        b.iter(|| {
            for i in 0..100000 {
                gc.write_barrier(
                    black_box(i as u64 * 1024),
                    black_box((i + 1) as u64 * 1024),
                );
            }
        });
    });
    
    group.finish();
}

/// 基准测试：动态配额调整效果
fn dynamic_quota_adjustment(c: &mut Criterion) {
    let mut group = c.benchmark_group("dynamic_quota");
    
    let heap_sizes = vec![
        (10 * 1024 * 1024, "10MB"),
        (100 * 1024 * 1024, "100MB"),
        (1024 * 1024 * 1024, "1GB"),
    ];
    
    for (heap_size, label) in heap_sizes {
        group.bench_with_input(
            BenchmarkId::new("quota_adjustment", label),
            &heap_size,
            |b, &heap_size| {
                let config = UnifiedGcConfig {
                    heap_size_limit: heap_size,
                    adaptive_quota: true,
                    ..Default::default()
                };
                
                let gc = UnifiedGC::new(config);
                
                b.iter(|| {
                    // 更新堆使用量，触发配额调整
                    for usage_ratio in [0.3, 0.5, 0.7, 0.9] {
                        let used = (heap_size as f64 * usage_ratio) as u64;
                        gc.update_heap_usage(black_box(used));
                        
                        // 执行一次增量标记以触发配额调整
                        let roots: Vec<u64> = vec![0x1000];
                        let _cycle_start = gc.start_gc(&roots);
                        let (_, _) = gc.incremental_mark();
                        gc.finish_gc(_cycle_start);
                    }
                });
            },
        );
    }
    
    group.finish();
}

/// 基准测试：并发GC性能（模拟多线程场景）
fn concurrent_gc_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_gc");
    
    let config = UnifiedGcConfig {
        heap_size_limit: 100 * 1024 * 1024,
        concurrent_marking: true,
        write_barrier_shards: 0, // 自动计算
        ..Default::default()
    };
    
    let gc = Arc::new(UnifiedGC::new(config));
    
    group.bench_function("concurrent_write_barrier", |b| {
        b.iter(|| {
            use std::sync::atomic::{AtomicU64, Ordering};
            use std::thread;
            
            let roots: Vec<u64> = vec![0x1000, 0x2000, 0x3000];
            let _cycle_start = gc.start_gc(&roots);
            
            let counter = Arc::new(AtomicU64::new(0));
            let num_threads = 4;
            let ops_per_thread = 10000;
            
            let mut handles = Vec::new();
            for thread_id in 0..num_threads {
                let gc_clone = Arc::clone(&gc);
                let counter_clone = Arc::clone(&counter);
                
                let handle = thread::spawn(move || {
                    for i in 0..ops_per_thread {
                        let obj_addr = (thread_id * ops_per_thread + i) as u64 * 1024;
                        let child_addr = (obj_addr + 1024) as u64;
                        gc_clone.write_barrier(obj_addr, child_addr);
                        counter_clone.fetch_add(1, Ordering::Relaxed);
                    }
                });
                handles.push(handle);
            }
            
            for handle in handles {
                handle.join().unwrap();
            }
            
            black_box(counter.load(Ordering::Relaxed));
        });
    });
    
    group.finish();
}

/// 基准测试：GC统计信息收集性能
fn gc_stats_collection(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc_stats");
    
    let config = UnifiedGcConfig::default();
    let gc = UnifiedGC::new(config);
    
    // 执行几次GC周期以产生统计数据
    let roots: Vec<u64> = (0..100).map(|i| i as u64 * 1024).collect();
    for _ in 0..10 {
        let cycle_start = gc.start_gc(&roots);
        loop {
            let (complete, _) = gc.incremental_mark();
            if complete {
                break;
            }
        }
        gc.terminate_marking();
        loop {
            let (complete, _) = gc.incremental_sweep();
            if complete {
                break;
            }
        }
        gc.finish_gc(cycle_start);
    }
    
    group.bench_function("get_stats", |b| {
        b.iter(|| {
            let stats = gc.stats();
            black_box((
                stats.get_avg_pause_us(),
                stats.get_max_pause_us(),
                stats.get_total_pause_us(),
                stats.get_last_pause_us(),
            ));
        });
    });
    
    group.finish();
}

/// 基准测试：并行清扫 vs 串行清扫性能对比
fn parallel_sweep_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_sweep");
    
    // 准备测试数据：不同大小的清扫列表
    let sweep_list_sizes = vec![
        (1000, "1k"),
        (10000, "10k"),
        (100000, "100k"),
        (1000000, "1M"),
    ];
    
    for (size, label) in sweep_list_sizes {
        // 创建测试用的清扫列表
        let sweep_list: Vec<u64> = (0..size).map(|i| i as u64 * 1024).collect();
        let sweep_list_arc = Arc::new(Mutex::new(sweep_list.clone()));
        let phase = Arc::new(AtomicU64::new(4)); // Sweeping phase
        let stats = Arc::new(vm_engine::jit::UnifiedGcStats::default());
        
        let sweeper = GcSweeper::new(
            sweep_list_arc.clone(),
            phase.clone(),
            stats.clone(),
            1000, // batch_size
        );
        
        // 准备清扫列表（模拟标记阶段后的状态）
        let marked_set = std::collections::HashSet::new(); // 所有对象都未标记
        sweeper.prepare_sweeping(&sweep_list, &marked_set);
        
        // 测试串行清扫
        group.bench_with_input(
            BenchmarkId::new("serial", label),
            &sweeper,
            |b, sweeper| {
                // 重置清扫列表
                sweeper.prepare_sweeping(&sweep_list, &marked_set);
                
                b.iter(|| {
                    let quota_us = 10_000_000; // 10ms配额
                    let (complete, freed) = sweeper.incremental_sweep_with_parallel(quota_us, false);
                    black_box((complete, freed));
                });
            },
        );
        
        // 测试并行清扫
        group.bench_with_input(
            BenchmarkId::new("parallel", label),
            &sweeper,
            |b, sweeper| {
                // 重置清扫列表
                sweeper.prepare_sweeping(&sweep_list, &marked_set);
                
                b.iter(|| {
                    let quota_us = 10_000_000; // 10ms配额
                    let (complete, freed) = sweeper.incremental_sweep_with_parallel(quota_us, true);
                    black_box((complete, freed));
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
        gc_performance_by_heap_size,
        gc_pause_time_distribution,
        write_barrier_performance,
        dynamic_quota_adjustment,
        concurrent_gc_performance,
        gc_stats_collection,
        parallel_sweep_comparison
);

criterion_main!(benches);

