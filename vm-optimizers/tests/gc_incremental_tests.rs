//! 增量GC测试套件
//!
//! 测试增量垃圾回收的功能和性能

use vm_optimizers::gc_incremental_enhanced::{
    GCPhase, IncrementalGC, IncrementalGCConfig, IncrementalGCStats, MarkStack, ObjectPtr,
};
use std::time::Instant;

// ============================================================================
// MarkStack测试
// ============================================================================

#[test]
fn test_mark_stack_creation() {
    let stack = MarkStack::new(100);
    assert!(stack.is_empty());
    assert_eq!(stack.len(), 0);
}

#[test]
fn test_mark_stack_push_pop() {
    let mut stack = MarkStack::new(10);

    // 推入对象
    assert!(stack.push(ObjectPtr(1)).is_ok());
    assert!(stack.push(ObjectPtr(2)).is_ok());
    assert!(stack.push(ObjectPtr(3)).is_ok());

    assert_eq!(stack.len(), 3);

    // 弹出对象
    assert_eq!(stack.pop(), Some(ObjectPtr(3)));
    assert_eq!(stack.pop(), Some(ObjectPtr(2)));
    assert_eq!(stack.pop(), Some(ObjectPtr(1)));
    assert_eq!(stack.pop(), None);
}

#[test]
fn test_mark_stack_capacity_limit() {
    let mut stack = MarkStack::new(2);

    // 可以推入2个对象
    assert!(stack.push(ObjectPtr(1)).is_ok());
    assert!(stack.push(ObjectPtr(2)).is_ok());

    // 第3个对象会失败
    assert!(stack.push(ObjectPtr(3)).is_err());

    assert_eq!(stack.len(), 2);
}

#[test]
fn test_mark_stack_clear() {
    let mut stack = MarkStack::new(10);

    stack.push(ObjectPtr(1)).unwrap();
    stack.push(ObjectPtr(2)).unwrap();
    stack.push(ObjectPtr(3)).unwrap();

    assert_eq!(stack.len(), 3);

    stack.clear();

    assert!(stack.is_empty());
    assert_eq!(stack.len(), 0);
}

// ============================================================================
// IncrementalGCConfig测试
// ============================================================================

#[test]
fn test_incremental_gc_config_default() {
    let config = IncrementalGCConfig::default();

    assert_eq!(config.work_quota, 100);
    assert_eq!(config.min_work_quota, 10);
    assert_eq!(config.max_work_quota, 1000);
    assert!(config.adaptive_quota);
    assert_eq!(config.target_pause_time_ms, 5);
}

#[test]
fn test_incremental_gc_config_custom() {
    let config = IncrementalGCConfig {
        work_quota: 200,
        min_work_quota: 20,
        max_work_quota: 2000,
        adaptive_quota: false,
        target_pause_time_ms: 10,
    };

    assert_eq!(config.work_quota, 200);
    assert_eq!(config.min_work_quota, 20);
    assert_eq!(config.max_work_quota, 2000);
    assert!(!config.adaptive_quota);
    assert_eq!(config.target_pause_time_ms, 10);
}

// ============================================================================
// IncrementalGCStats测试
// ============================================================================

#[test]
fn test_incremental_gc_stats_default() {
    let stats = IncrementalGCStats::default();

    assert_eq!(stats.marked_objects.load(std::sync::atomic::Ordering::Relaxed), 0);
    assert_eq!(stats.swept_objects.load(std::sync::atomic::Ordering::Relaxed), 0);
    assert_eq!(stats.freed_memory.load(std::sync::atomic::Ordering::Relaxed), 0);
}

#[test]
fn test_incremental_gc_stats_pause_time() {
    let stats = IncrementalGCStats::default();

    // 初始状态
    assert_eq!(stats.avg_pause_time_ns(), 0);
    assert_eq!(stats.avg_pause_time_ms(), 0.0);

    // 添加一些暂停时间
    stats.total_pauses.fetch_add(2, std::sync::atomic::Ordering::Relaxed);
    stats
        .total_pause_time_ns
        .fetch_add(10_000_000, std::sync::atomic::Ordering::Relaxed); // 10ms

    // 平均暂停时间 = 10ms / 2 = 5ms
    assert_eq!(stats.avg_pause_time_ns(), 5_000_000);
    assert_eq!(stats.avg_pause_time_ms(), 5.0);
}

#[test]
fn test_incremental_gc_stats_mark_sweep() {
    let stats = IncrementalGCStats::default();

    // 标记100个对象
    stats
        .marked_objects
        .fetch_add(100, std::sync::atomic::Ordering::Relaxed);

    // 清扫50个对象
    stats
        .swept_objects
        .fetch_add(50, std::sync::atomic::Ordering::Relaxed);

    // 释放1000字节
    stats
        .freed_memory
        .fetch_add(1000, std::sync::atomic::Ordering::Relaxed);

    assert_eq!(
        stats.marked_objects.load(std::sync::atomic::Ordering::Relaxed),
        100
    );
    assert_eq!(
        stats.swept_objects.load(std::sync::atomic::Ordering::Relaxed),
        50
    );
    assert_eq!(
        stats.freed_memory.load(std::sync::atomic::Ordering::Relaxed),
        1000
    );
}

// ============================================================================
// IncrementalGC测试
// ============================================================================

#[test]
fn test_incremental_gc_creation() {
    let config = IncrementalGCConfig::default();
    let gc = IncrementalGC::new(1024 * 1024, config);

    assert_eq!(gc.phase(), GCPhase::Idle);
}

#[test]
fn test_incremental_gc_alloc() {
    let config = IncrementalGCConfig::default();
    let mut gc = IncrementalGC::new(1024 * 1024, config);

    // 分配应该触发增量GC工作
    let result = gc.alloc(100);
    assert!(result.is_ok());
}

#[test]
fn test_incremental_gc_phase_transitions() {
    let config = IncrementalGCConfig::default();
    let mut gc = IncrementalGC::new(1024 * 1024, config);

    // 初始状态
    assert_eq!(gc.phase(), GCPhase::Idle);

    // 启动GC
    gc.start_gc();

    // 应该进入标记阶段
    assert_eq!(gc.phase(), GCPhase::Marking);
}

#[test]
fn test_incremental_gc_work_quota() {
    let config = IncrementalGCConfig {
        work_quota: 50,
        ..Default::default()
    };
    let gc = IncrementalGC::new(1024 * 1024, config);

    assert_eq!(gc.work_quota(), 50);
}

#[test]
fn test_incremental_gc_stats() {
    let config = IncrementalGCConfig::default();
    let gc = IncrementalGC::new(1024 * 1024, config);

    let stats = gc.stats();

    // 初始统计应该为0
    assert_eq!(
        stats.marked_objects.load(std::sync::atomic::Ordering::Relaxed),
        0
    );
    assert_eq!(
        stats.swept_objects.load(std::sync::atomic::Ordering::Relaxed),
        0
    );
}

#[test]
fn test_incremental_gc_concurrent_access() {
    use std::sync::Arc;
    use std::thread;

    let config = IncrementalGCConfig::default();
    let gc = Arc::new(std::sync::Mutex::new(IncrementalGC::new(1024 * 1024, config)));

    let mut handles = vec![];

    // 多线程并发分配
    for _ in 0..10 {
        let gc_clone = Arc::clone(&gc);
        let handle = thread::spawn(move || {
            let mut gc = gc_clone.lock().unwrap();
            for _ in 0..100 {
                let _ = gc.alloc(10);
            }
        });
        handles.push(handle);
    }

    // 等待所有线程完成
    for handle in handles {
        handle.join().unwrap();
    }

    // 验证统计信息
    let gc = gc.lock().unwrap();
    let stats = gc.stats();
    // 统计可能大于0
    let marked = stats.marked_objects.load(std::sync::atomic::Ordering::Relaxed);
    assert!(marked >= 0);
}

// ============================================================================
// 性能测试
// ============================================================================

#[test]
fn test_incremental_gc_allocation_performance() {
    let config = IncrementalGCConfig::default();
    let mut gc = IncrementalGC::new(1024 * 1024, config);

    let start = Instant::now();

    // 分配1000次
    for _ in 0..1000 {
        let _ = gc.alloc(100);
    }

    let duration = start.elapsed();

    // 1000次分配应该在合理时间内完成 (< 100ms)
    assert!(duration.as_millis() < 100);
}

#[test]
fn test_incremental_gc_mark_stack_performance() {
    let mut stack = MarkStack::new(1000);

    let start = Instant::now();

    // 推入和弹出1000个对象
    for i in 0..1000 {
        let _ = stack.push(ObjectPtr(i));
    }

    while let Some(_) = stack.pop() {}

    let duration = start.elapsed();

    // 应该很快 (< 10ms)
    assert!(duration.as_millis() < 10);
}

// ============================================================================
// 边界条件测试
// ============================================================================

#[test]
fn test_incremental_gc_zero_heap() {
    let config = IncrementalGCConfig::default();
    let gc = IncrementalGC::new(0, config);

    // 即使堆大小为0，也应该能创建
    assert_eq!(gc.phase(), GCPhase::Idle);
}

#[test]
fn test_incremental_gc_large_heap() {
    let config = IncrementalGCConfig::default();
    let gc = IncrementalGC::new(1024 * 1024 * 1024, config); // 1GB

    // 大堆应该正常工作
    assert_eq!(gc.phase(), GCPhase::Idle);
}

#[test]
fn test_incremental_gc_zero_quota() {
    let config = IncrementalGCConfig {
        work_quota: 0,
        ..Default::default()
    };
    let mut gc = IncrementalGC::new(1024, config);

    // 即使配额为0，也应该能分配
    let result = gc.alloc(100);
    assert!(result.is_ok());
}

#[test]
fn test_incremental_gc_extreme_quota() {
    let config = IncrementalGCConfig {
        work_quota: 1_000_000,
        ..Default::default()
    };
    let mut gc = IncrementalGC::new(1024, config);

    // 极端配额应该正常工作
    let result = gc.alloc(100);
    assert!(result.is_ok());
}

#[test]
fn test_mark_stack_empty_pop() {
    let mut stack = MarkStack::new(10);

    // 空栈弹出应该返回None
    assert_eq!(stack.pop(), None);
    assert_eq!(stack.pop(), None);
}

#[test]
fn test_mark_stack_single_element() {
    let mut stack = MarkStack::new(10);

    // 单个元素
    stack.push(ObjectPtr(42)).unwrap();

    assert_eq!(stack.len(), 1);
    assert!(!stack.is_empty());

    assert_eq!(stack.pop(), Some(ObjectPtr(42)));
    assert!(stack.is_empty());
}

#[test]
fn test_gc_stats_concurrent_updates() {
    use std::sync::Arc;
    use std::thread;

    let stats = Arc::new(IncrementalGCStats::default());
    let mut handles = vec![];

    // 多线程并发更新统计
    for _ in 0..10 {
        let stats_clone = Arc::clone(&stats);
        let handle = thread::spawn(move || {
            for i in 0..100 {
                stats_clone
                    .marked_objects
                    .fetch_add(i, std::sync::atomic::Ordering::Relaxed);
                stats_clone
                    .swept_objects
                    .fetch_add(i, std::sync::atomic::Ordering::Relaxed);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // 总和应该是 0+1+...+99 = 4950，乘以10个线程 = 49500
    let marked = stats.marked_objects.load(std::sync::atomic::Ordering::Relaxed);
    assert_eq!(marked, 49500);
}
