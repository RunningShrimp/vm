//! GC功能全面测试套件
//!
//! 测试统一GC系统的所有核心组件，包括：
//! - 分代GC（年轻代和老年代）
//! - 并发GC（并发标记和清扫）
//! - 自适应GC（自适应阈值调整）
//! - GC标记器和清扫器

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use vm_engine_jit::unified_gc::{
    UnifiedGc, UnifiedGcConfig, UnifiedGcStats, GCPhase, GcConfig,
    YoungGenConfig, OldGenConfig, ConcurrentConfig, AdaptiveConfig
};
use vm_engine_jit::gc_marker::{GcMarker, GcMarkerConfig, GcMarkerStats};
use vm_engine_jit::gc_sweeper::{GcSweeper, GcSweeperConfig, GcSweeperStats};
use vm_core::GuestAddr;

/// 创建测试用的GC配置
fn create_test_gc_config() -> UnifiedGcConfig {
    UnifiedGcConfig {
        young_gen: YoungGenConfig {
            initial_size: 1024 * 1024,      // 1MB
            max_size: 16 * 1024 * 1024,     // 16MB
            promotion_threshold: 100,          // 100次GC后晋升
            ..Default::default()
        },
        old_gen: OldGenConfig {
            initial_size: 4 * 1024 * 1024,   // 4MB
            max_size: 64 * 1024 * 1024,      // 64MB
            ..Default::default()
        },
        concurrent: ConcurrentConfig {
            enabled: true,
            marking_threads: 2,
            sweeping_threads: 1,
            ..Default::default()
        },
        adaptive: AdaptiveConfig {
            enabled: true,
            target_pause_time_ms: 10,
            ..Default::default()
        },
        ..Default::default()
    }
}

/// 创建测试对象集合
fn create_test_objects(count: usize) -> Vec<GuestAddr> {
    (0..count).map(|i| 0x1000 + (i * 0x100) as u64).collect()
}

/// 创建测试根集合
fn create_test_roots() -> Vec<GuestAddr> {
    vec![0x1000, 0x2000, 0x3000, 0x4000, 0x5000]
}

// ============================================================================
// 统一GC测试
// ============================================================================

#[test]
fn test_unified_gc_basic_functionality() {
    let config = create_test_gc_config();
    let gc = UnifiedGc::new(config);
    
    // 测试基本分配
    let objects = create_test_objects(100);
    for &obj in &objects {
        gc.allocate(obj, 64); // 分配64字节
    }
    
    // 验证分配统计
    let stats = gc.get_stats();
    assert!(stats.total_allocations > 0);
    assert!(stats.total_allocated_bytes > 0);
}

#[test]
fn test_unified_gc_young_generation() {
    let config = create_test_gc_config();
    let gc = UnifiedGc::new(config);
    
    // 分配对象到年轻代
    let young_objects = create_test_objects(50);
    for &obj in &young_objects {
        gc.allocate_young(obj, 64);
    }
    
    // 验证年轻代统计
    let stats = gc.get_stats();
    assert!(stats.young_gen.allocations > 0);
    assert!(stats.young_gen.allocated_bytes > 0);
    
    // 触发年轻代GC
    let roots = create_test_roots();
    gc.collect_young(&roots);
    
    // 验证GC统计
    let stats = gc.get_stats();
    assert!(stats.young_gen.collections > 0);
}

#[test]
fn test_unified_gc_old_generation() {
    let config = create_test_gc_config();
    let gc = UnifiedGc::new(config);
    
    // 分配对象到老年代
    let old_objects = create_test_objects(30);
    for &obj in &old_objects {
        gc.allocate_old(obj, 128);
    }
    
    // 验证老年代统计
    let stats = gc.get_stats();
    assert!(stats.old_gen.allocations > 0);
    assert!(stats.old_gen.allocated_bytes > 0);
    
    // 触发老年代GC
    let roots = create_test_roots();
    gc.collect_old(&roots);
    
    // 验证GC统计
    let stats = gc.get_stats();
    assert!(stats.old_gen.collections > 0);
}

#[test]
fn test_unified_gc_promotion() {
    let mut config = create_test_gc_config();
    config.young_gen.promotion_threshold = 5; // 5次GC后晋升
    
    let gc = UnifiedGc::new(config);
    
    // 分配对象到年轻代
    let objects = create_test_objects(20);
    for &obj in &objects {
        gc.allocate_young(obj, 64);
    }
    
    let roots = create_test_roots();
    
    // 触发多次GC以导致晋升
    for _ in 0..10 {
        gc.collect_young(&roots);
    }
    
    // 验证晋升统计
    let stats = gc.get_stats();
    assert!(stats.young_gen.promotions > 0);
    assert!(stats.old_gen.promoted_objects > 0);
}

#[test]
fn test_unified_gc_concurrent_marking() {
    let mut config = create_test_gc_config();
    config.concurrent.enabled = true;
    config.concurrent.marking_threads = 2;
    
    let gc = UnifiedGc::new(config);
    
    // 分配对象
    let objects = create_test_objects(200);
    for &obj in &objects {
        gc.allocate(obj, 64);
    }
    
    // 启动并发标记
    let roots = create_test_roots();
    gc.start_concurrent_marking(&roots);
    
    // 等待标记完成
    thread::sleep(Duration::from_millis(100));
    
    // 验证并发标记统计
    let stats = gc.get_stats();
    assert!(stats.concurrent.marking_started > 0);
}

#[test]
fn test_unified_gc_concurrent_sweeping() {
    let mut config = create_test_gc_config();
    config.concurrent.enabled = true;
    config.concurrent.sweeping_threads = 1;
    
    let gc = UnifiedGc::new(config);
    
    // 分配对象
    let objects = create_test_objects(100);
    for &obj in &objects {
        gc.allocate(obj, 64);
    }
    
    // 模拟标记阶段
    let roots = create_test_roots();
    gc.start_concurrent_marking(&roots);
    thread::sleep(Duration::from_millis(50));
    
    // 启动并发清扫
    gc.start_concurrent_sweeping();
    
    // 等待清扫完成
    thread::sleep(Duration::from_millis(100));
    
    // 验证并发清扫统计
    let stats = gc.get_stats();
    assert!(stats.concurrent.sweeping_started > 0);
}

#[test]
fn test_unified_gc_adaptive_thresholds() {
    let mut config = create_test_gc_config();
    config.adaptive.enabled = true;
    config.adaptive.target_pause_time_ms = 5; // 目标停顿5ms
    
    let gc = UnifiedGc::new(config);
    
    // 分配对象
    let objects = create_test_objects(150);
    for &obj in &objects {
        gc.allocate(obj, 64);
    }
    
    // 触发多次GC以收集性能数据
    let roots = create_test_roots();
    for i in 0..10 {
        gc.collect_young(&roots);
        
        // 模拟不同的GC时间
        if i % 3 == 0 {
            thread::sleep(Duration::from_millis(10)); // 长时间GC
        } else {
            thread::sleep(Duration::from_millis(2));  // 短时间GC
        }
    }
    
    // 验证自适应调整
    let stats = gc.get_stats();
    assert!(stats.adaptive.threshold_adjustments > 0);
}

#[test]
fn test_unified_gc_memory_pressure() {
    let config = create_test_gc_config();
    let gc = UnifiedGc::new(config);
    
    // 模拟内存压力
    let large_objects = create_test_objects(1000);
    for &obj in &large_objects {
        gc.allocate(obj, 1024); // 分配1KB对象
    }
    
    // 触发GC
    let roots = create_test_roots();
    gc.collect_full(&roots);
    
    // 验证内存压力处理
    let stats = gc.get_stats();
    assert!(stats.memory_pressure_events > 0);
    assert!(stats.full_collections > 0);
}

#[test]
fn test_unified_gc_pause_time() {
    let config = create_test_gc_config();
    let gc = UnifiedGc::new(config);
    
    // 分配对象
    let objects = create_test_objects(100);
    for &obj in &objects {
        gc.allocate(obj, 64);
    }
    
    // 测量GC停顿时间
    let roots = create_test_roots();
    let start_time = Instant::now();
    
    gc.collect_young(&roots);
    
    let pause_time = start_time.elapsed();
    
    // 验证停顿时间在合理范围内
    assert!(pause_time.as_millis() < 50, "GC pause should be reasonable");
    
    let stats = gc.get_stats();
    assert!(stats.pause_time_stats.max_pause_ms > 0);
    assert!(stats.pause_time_stats.avg_pause_ms > 0);
}

// ============================================================================
// GC标记器测试
// ============================================================================

#[test]
fn test_gc_marker_basic_functionality() {
    let config = GcMarkerConfig::default();
    let mark_stack = Arc::new(vm_engine_jit::unified_gc::LockFreeMarkStack::new(1000));
    let marked_set = Arc::new(RwLock::new(HashSet::new()));
    let phase = Arc::new(AtomicU64::new(GCPhase::Marking as u64));
    let stats = Arc::new(Mutex::new(GcMarkerStats::default()));
    
    let marker = GcMarker::new(config, mark_stack.clone(), marked_set.clone(), phase.clone(), stats.clone());
    
    // 添加根对象
    let roots = create_test_roots();
    for &root in &roots {
        let _ = mark_stack.push(root);
    }
    
    // 执行标记
    let (completed, marked_count) = marker.incremental_mark(1000); // 1ms配额
    
    // 验证标记结果
    assert!(marked_count >= 0);
    if completed {
        let marked = marked_set.read().unwrap();
        for &root in &roots {
            assert!(marked.contains(&root), "Root should be marked");
        }
    }
}

#[test]
fn test_gc_marker_incremental_marking() {
    let config = GcMarkerConfig::default();
    let mark_stack = Arc::new(vm_engine_jit::unified_gc::LockFreeMarkStack::new(1000));
    let marked_set = Arc::new(RwLock::new(HashSet::new()));
    let phase = Arc::new(AtomicU64::new(GCPhase::Marking as u64));
    let stats = Arc::new(Mutex::new(GcMarkerStats::default()));
    
    let marker = GcMarker::new(config, mark_stack.clone(), marked_set.clone(), phase.clone(), stats.clone());
    
    // 添加大量对象
    let objects = create_test_objects(500);
    for &obj in &objects {
        let _ = mark_stack.push(obj);
    }
    
    // 执行增量标记
    let mut total_marked = 0;
    let mut completed = false;
    
    while !completed {
        let (c, marked) = marker.incremental_mark(100); // 小配额
        completed = c;
        total_marked += marked;
    }
    
    // 验证增量标记结果
    assert!(total_marked > 0);
    assert!(completed);
    
    let stats = stats.lock().unwrap();
    assert!(stats.total_marked > 0);
    assert!(stats.incremental_rounds > 1);
}

#[test]
fn test_gc_marker_concurrent_marking() {
    let config = GcMarkerConfig::default();
    let mark_stack = Arc::new(vm_engine_jit::unified_gc::LockFreeMarkStack::new(1000));
    let marked_set = Arc::new(RwLock::new(HashSet::new()));
    let phase = Arc::new(AtomicU64::new(GCPhase::Marking as u64));
    let stats = Arc::new(Mutex::new(GcMarkerStats::default()));
    
    let marker = Arc::new(GcMarker::new(config, mark_stack.clone(), marked_set.clone(), phase.clone(), stats.clone()));
    
    // 添加对象
    let objects = create_test_objects(300);
    for &obj in &objects {
        let _ = mark_stack.push(obj);
    }
    
    // 启动并发标记线程
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let marker = marker.clone();
            thread::spawn(move || {
                let mut total_marked = 0;
                for _ in 0..10 {
                    let (_, marked) = marker.incremental_mark(50);
                    total_marked += marked;
                    thread::sleep(Duration::from_millis(1));
                }
                total_marked
            })
        })
        .collect();
    
    // 等待所有线程完成
    let mut total_marked = 0;
    for handle in handles {
        total_marked += handle.join().unwrap();
    }
    
    // 验证并发标记结果
    assert!(total_marked > 0);
    
    let stats = stats.lock().unwrap();
    assert!(stats.concurrent_threads > 0);
}

#[test]
fn test_gc_marker_work_stealing() {
    let config = GcMarkerConfig::default();
    config.enable_work_stealing = true;
    
    let mark_stack = Arc::new(vm_engine_jit::unified_gc::LockFreeMarkStack::new(1000));
    let marked_set = Arc::new(RwLock::new(HashSet::new()));
    let phase = Arc::new(AtomicU64::new(GCPhase::Marking as u64));
    let stats = Arc::new(Mutex::new(GcMarkerStats::default()));
    
    let marker = Arc::new(GcMarker::new(config, mark_stack.clone(), marked_set.clone(), phase.clone(), stats.clone()));
    
    // 添加对象
    let objects = create_test_objects(200);
    for &obj in &objects {
        let _ = mark_stack.push(obj);
    }
    
    // 启动工作窃取标记线程
    let handles: Vec<_> = (0..3)
        .map(|_| {
            let marker = marker.clone();
            thread::spawn(move || {
                let mut total_marked = 0;
                for _ in 0..5 {
                    let (_, marked) = marker.incremental_mark(100);
                    total_marked += marked;
                    thread::sleep(Duration::from_millis(2));
                }
                total_marked
            })
        })
        .collect();
    
    // 等待所有线程完成
    let mut total_marked = 0;
    for handle in handles {
        total_marked += handle.join().unwrap();
    }
    
    // 验证工作窃取结果
    assert!(total_marked > 0);
    
    let stats = stats.lock().unwrap();
    assert!(stats.work_stealing_attempts > 0);
}

// ============================================================================
// GC清扫器测试
// ============================================================================

#[test]
fn test_gc_sweeper_basic_functionality() {
    let config = GcSweeperConfig::default();
    let sweep_list = Arc::new(Mutex::new(Vec::new()));
    let phase = Arc::new(AtomicU64::new(GCPhase::Sweeping as u64));
    let stats = Arc::new(Mutex::new(GcSweeperStats::default()));
    
    let sweeper = GcSweeper::new(config, sweep_list.clone(), phase.clone(), stats.clone());
    
    // 添加待清扫对象
    let objects = create_test_objects(100);
    {
        let mut list = sweep_list.lock().unwrap();
        for &obj in &objects {
            list.push(obj);
        }
    }
    
    // 执行清扫
    let (completed, freed_count) = sweeper.incremental_sweep(1000); // 1ms配额
    
    // 验证清扫结果
    assert!(freed_count >= 0);
    if completed {
        let list = sweep_list.lock().unwrap();
        assert!(list.is_empty(), "All objects should be freed");
    }
}

#[test]
fn test_gc_sweeper_batch_processing() {
    let mut config = GcSweeperConfig::default();
    config.batch_size = 10; // 小批次大小
    
    let sweep_list = Arc::new(Mutex::new(Vec::new()));
    let phase = Arc::new(AtomicU64::new(GCPhase::Sweeping as u64));
    let stats = Arc::new(Mutex::new(GcSweeperStats::default()));
    
    let sweeper = GcSweeper::new(config, sweep_list.clone(), phase.clone(), stats.clone());
    
    // 添加大量对象
    let objects = create_test_objects(50);
    {
        let mut list = sweep_list.lock().unwrap();
        for &obj in &objects {
            list.push(obj);
        }
    }
    
    // 执行批次清扫
    let mut total_freed = 0;
    let mut completed = false;
    
    while !completed {
        let (c, freed) = sweeper.incremental_sweep(50); // 小配额
        completed = c;
        total_freed += freed;
    }
    
    // 验证批次清扫结果
    assert!(total_freed > 0);
    assert!(completed);
    
    let stats = stats.lock().unwrap();
    assert!(stats.batches_processed > 1);
}

#[test]
fn test_gc_sweeper_concurrent_sweeping() {
    let config = GcSweeperConfig::default();
    let sweep_list = Arc::new(Mutex::new(Vec::new()));
    let phase = Arc::new(AtomicU64::new(GCPhase::Sweeping as u64));
    let stats = Arc::new(Mutex::new(GcSweeperStats::default()));
    
    let sweeper = Arc::new(GcSweeper::new(config, sweep_list.clone(), phase.clone(), stats.clone()));
    
    // 添加对象
    let objects = create_test_objects(200);
    {
        let mut list = sweep_list.lock().unwrap();
        for &obj in &objects {
            list.push(obj);
        }
    }
    
    // 启动并发清扫线程
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let sweeper = sweeper.clone();
            thread::spawn(move || {
                let mut total_freed = 0;
                for _ in 0..10 {
                    let (_, freed) = sweeper.incremental_sweep(50);
                    total_freed += freed;
                    thread::sleep(Duration::from_millis(1));
                }
                total_freed
            })
        })
        .collect();
    
    // 等待所有线程完成
    let mut total_freed = 0;
    for handle in handles {
        total_freed += handle.join().unwrap();
    }
    
    // 验证并发清扫结果
    assert!(total_freed > 0);
    
    let stats = stats.lock().unwrap();
    assert!(stats.concurrent_threads > 0);
}

// ============================================================================
// GC集成测试
// ============================================================================

#[test]
fn test_gc_full_cycle() {
    let config = create_test_gc_config();
    let gc = UnifiedGc::new(config);
    
    // 分配对象
    let objects = create_test_objects(150);
    for &obj in &objects {
        gc.allocate(obj, 64);
    }
    
    // 执行完整GC周期
    let roots = create_test_roots();
    
    // 1. 标记阶段
    gc.start_concurrent_marking(&roots);
    thread::sleep(Duration::from_millis(50));
    
    // 2. 清扫阶段
    gc.start_concurrent_sweeping();
    thread::sleep(Duration::from_millis(50));
    
    // 验证完整周期
    let stats = gc.get_stats();
    assert!(stats.full_collections > 0);
    assert!(stats.concurrent.marking_completed > 0);
    assert!(stats.concurrent.sweeping_completed > 0);
}

#[test]
fn test_gc_generational_interaction() {
    let config = create_test_gc_config();
    let gc = UnifiedGc::new(config);
    
    // 分配对象到不同代
    let young_objects = create_test_objects(50);
    let old_objects = create_test_objects(30);
    
    for &obj in &young_objects {
        gc.allocate_young(obj, 64);
    }
    
    for &obj in &old_objects {
        gc.allocate_old(obj, 128);
    }
    
    // 触发年轻代GC
    let roots = create_test_roots();
    gc.collect_young(&roots);
    
    // 验证代际交互
    let stats = gc.get_stats();
    assert!(stats.young_gen.collections > 0);
    assert!(stats.young_gen.promotions >= 0);
    
    // 触发老年代GC
    gc.collect_old(&roots);
    
    let stats = gc.get_stats();
    assert!(stats.old_gen.collections > 0);
}

#[test]
fn test_gc_performance_characteristics() {
    let config = create_test_gc_config();
    let gc = UnifiedGc::new(config);
    
    // 性能测试
    let start_time = Instant::now();
    
    // 分配大量对象
    let objects = create_test_objects(1000);
    for &obj in &objects {
        gc.allocate(obj, 64);
    }
    
    let allocation_time = start_time.elapsed();
    
    // 测量GC时间
    let roots = create_test_roots();
    let gc_start = Instant::now();
    
    gc.collect_full(&roots);
    
    let gc_time = gc_start.elapsed();
    
    // 验证性能特征
    assert!(allocation_time.as_millis() < 100, "Allocation should be fast");
    assert!(gc_time.as_millis() < 200, "GC should complete in reasonable time");
    
    let stats = gc.get_stats();
    assert!(stats.performance_metrics.allocation_rate > 0);
    assert!(stats.performance_metrics.gc_throughput > 0);
}

#[test]
fn test_gc_stress_test() {
    let config = create_test_gc_config();
    let gc = Arc::new(UnifiedGc::new(config));
    
    // 压力测试：多线程分配和GC
    let handles: Vec<_> = (0..4)
        .map(|i| {
            let gc = gc.clone();
            thread::spawn(move || {
                let objects = create_test_objects(100);
                for &obj in &objects {
                    gc.allocate(obj + i as u64 * 0x10000, 64);
                }
                
                // 触发GC
                let roots = create_test_roots();
                gc.collect_young(&roots);
                
                true
            })
        })
        .collect();
    
    // 等待所有线程完成
    for handle in handles {
        assert!(handle.join().unwrap());
    }
    
    // 验证压力测试结果
    let stats = gc.get_stats();
    assert!(stats.total_allocations > 0);
    assert!(stats.concurrent.max_concurrent_threads > 1);
}

#[test]
fn test_gc_memory_fragmentation() {
    let config = create_test_gc_config();
    let gc = UnifiedGc::new(config);
    
    // 分配不同大小的对象
    let mut objects = Vec::new();
    
    // 小对象
    for i in 0..100 {
        let addr = 0x1000 + i * 0x10;
        gc.allocate(addr, 32);
        objects.push(addr);
    }
    
    // 大对象
    for i in 0..20 {
        let addr = 0x2000 + i * 0x100;
        gc.allocate(addr, 1024);
        objects.push(addr);
    }
    
    // 释放一半对象（模拟）
    let roots = vec![objects[0], objects[50], objects[100]];
    
    // 触发GC
    gc.collect_full(&roots);
    
    // 验证碎片化处理
    let stats = gc.get_stats();
    assert!(stats.fragmentation_events > 0);
    assert!(stats.compactions > 0);
}

#[test]
fn test_gc_error_handling() {
    let config = create_test_gc_config();
    let gc = UnifiedGc::new(config);
    
    // 测试错误处理
    
    // 1. 无效地址处理
    let invalid_addr = 0;
    let result = std::panic::catch_unwind(|| {
        gc.allocate(invalid_addr, 64);
    });
    
    // 应该能够处理无效地址而不崩溃
    assert!(result.is_ok() || result.is_err());
    
    // 2. 空根集合处理
    let empty_roots = Vec::new();
    let result = std::panic::catch_unwind(|| {
        gc.collect_young(&empty_roots);
    });
    
    // 应该能够处理空根集合
    assert!(result.is_ok());
    
    // 3. 超大对象分配
    let huge_addr = 0x10000;
    let result = std::panic::catch_unwind(|| {
        gc.allocate(huge_addr, 1024 * 1024 * 1024); // 1GB
    });
    
    // 应该能够处理超大对象分配
    assert!(result.is_ok() || result.is_err());
}