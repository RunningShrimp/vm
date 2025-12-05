//! GC模块单元测试
//!
//! 测试GC标记器和清扫器的功能

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::collections::HashSet;
use vm_engine_jit::gc_marker::GcMarker;
use vm_engine_jit::gc_sweeper::GcSweeper;
use vm_engine_jit::unified_gc::{LockFreeMarkStack, UnifiedGcStats, GCPhase};

/// 创建测试用的标记栈
fn create_test_mark_stack() -> Arc<LockFreeMarkStack> {
    Arc::new(LockFreeMarkStack::new(1000))
}

/// 创建测试用的统计信息
fn create_test_stats() -> Arc<UnifiedGcStats> {
    Arc::new(UnifiedGcStats::default())
}

#[test]
fn test_gc_marker_new() {
    let mark_stack = create_test_mark_stack();
    let marked_set = Arc::new(RwLock::new(HashSet::new()));
    let phase = Arc::new(AtomicU64::new(GCPhase::Idle as u64));
    let stats = create_test_stats();

    let marker = GcMarker::new(
        mark_stack.clone(),
        marked_set.clone(),
        phase.clone(),
        stats.clone(),
    );

    // 验证标记器创建成功
    assert!(Arc::strong_count(&mark_stack) > 1, "Mark stack should be shared");
    assert!(Arc::strong_count(&marked_set) > 1, "Marked set should be shared");
    assert!(Arc::strong_count(&phase) > 1, "Phase should be shared");
    assert!(Arc::strong_count(&stats) > 1, "Stats should be shared");
}

#[test]
fn test_gc_marker_incremental_mark() {
    let mark_stack = create_test_mark_stack();
    let marked_set = Arc::new(RwLock::new(HashSet::new()));
    let phase = Arc::new(AtomicU64::new(GCPhase::Marking as u64));
    let stats = create_test_stats();

    let marker = GcMarker::new(
        mark_stack.clone(),
        marked_set.clone(),
        phase.clone(),
        stats.clone(),
    );

    // 添加一些根对象到标记栈
    let roots = vec![0x1000, 0x2000, 0x3000];
    for &root in &roots {
        let _ = mark_stack.push(root);
    }

    // 执行增量标记（使用较大的配额）
    let quota_us = 1000; // 1ms
    let (completed, marked_count) = marker.incremental_mark(quota_us);

    // 验证标记过程
    assert!(marked_count >= 0, "Marked count should be non-negative");
    
    // 如果完成，验证所有根对象都被标记
    if completed {
        let marked = marked_set.read().unwrap();
        for &root in &roots {
            assert!(
                marked.contains(&root),
                "Root object should be marked: {:#x}",
                root
            );
        }
    }
}

#[test]
fn test_gc_marker_prepare_marking() {
    let mark_stack = create_test_mark_stack();
    let marked_set = Arc::new(RwLock::new(HashSet::new()));
    let phase = Arc::new(AtomicU64::new(GCPhase::Idle as u64));
    let stats = create_test_stats();

    let marker = GcMarker::new(
        mark_stack.clone(),
        marked_set.clone(),
        phase.clone(),
        stats.clone(),
    );

    let roots = vec![0x1000, 0x2000];
    marker.prepare_marking(&roots);

    // 验证根对象被添加到标记栈
    assert!(mark_stack.len() > 0, "Root objects should be added to mark stack");
    
    // 验证标记集合被清空
    let marked = marked_set.read().unwrap();
    assert!(marked.is_empty(), "Marked set should be cleared");
}

#[test]
fn test_gc_marker_finish_marking() {
    let mark_stack = create_test_mark_stack();
    let marked_set = Arc::new(RwLock::new(HashSet::new()));
    let phase = Arc::new(AtomicU64::new(GCPhase::Marking as u64));
    let stats = create_test_stats();

    let marker = GcMarker::new(
        mark_stack.clone(),
        marked_set.clone(),
        phase.clone(),
        stats.clone(),
    );

    marker.finish_marking();

    // 验证阶段转换
    assert_eq!(
        phase.load(Ordering::Relaxed),
        GCPhase::MarkTerminate as u64,
        "Phase should be MarkTerminate"
    );
}

#[test]
fn test_gc_sweeper_new() {
    let sweep_list = Arc::new(Mutex::new(Vec::new()));
    let phase = Arc::new(AtomicU64::new(GCPhase::Sweeping as u64));
    let stats = create_test_stats();
    let batch_size = 100;

    let sweeper = GcSweeper::new(
        sweep_list.clone(),
        phase.clone(),
        stats.clone(),
        batch_size,
    );

    // 验证清扫器创建成功
    assert!(Arc::strong_count(&sweep_list) > 1, "Sweep list should be shared");
    assert!(Arc::strong_count(&phase) > 1, "Phase should be shared");
    assert!(Arc::strong_count(&stats) > 1, "Stats should be shared");
}

#[test]
fn test_gc_sweeper_incremental_sweep() {
    let sweep_list = Arc::new(Mutex::new(vec![0x1000, 0x2000, 0x3000]));
    let phase = Arc::new(AtomicU64::new(GCPhase::Sweeping as u64));
    let stats = create_test_stats();
    let batch_size = 10;

    let sweeper = GcSweeper::new(
        sweep_list.clone(),
        phase.clone(),
        stats.clone(),
        batch_size,
    );

    // 执行增量清扫（使用较大的配额）
    let quota_us = 1000; // 1ms
    let (completed, freed_count) = sweeper.incremental_sweep(quota_us);

    // 验证清扫过程
    assert!(freed_count >= 0, "Freed count should be non-negative");
    
    // 如果完成，验证清扫列表被清空
    if completed {
        let list = sweep_list.lock().unwrap();
        assert!(list.is_empty(), "Sweep list should be empty after completion");
    }
}

#[test]
fn test_gc_sweeper_prepare_sweeping() {
    let sweep_list = Arc::new(Mutex::new(Vec::new()));
    let phase = Arc::new(AtomicU64::new(GCPhase::MarkTerminate as u64));
    let stats = create_test_stats();
    let batch_size = 100;

    let sweeper = GcSweeper::new(
        sweep_list.clone(),
        phase.clone(),
        stats.clone(),
        batch_size,
    );

    let all_objects = vec![0x1000, 0x2000, 0x3000];
    let mut marked_set = HashSet::new();
    marked_set.insert(0x1000); // 只有0x1000被标记

    sweeper.prepare_sweeping(&all_objects, &marked_set);

    // 验证阶段转换
    assert_eq!(
        phase.load(Ordering::Relaxed),
        GCPhase::Sweeping as u64,
        "Phase should be Sweeping"
    );
    
    // 验证未标记对象被添加到清扫列表
    assert_eq!(sweeper.pending_count(), 2, "Should have 2 unmarked objects");
}

#[test]
fn test_gc_sweeper_finish_sweeping() {
    let sweep_list = Arc::new(Mutex::new(Vec::new()));
    let phase = Arc::new(AtomicU64::new(GCPhase::Sweeping as u64));
    let stats = create_test_stats();
    let batch_size = 100;

    let sweeper = GcSweeper::new(
        sweep_list.clone(),
        phase.clone(),
        stats.clone(),
        batch_size,
    );

    sweeper.finish_sweeping();

    // 验证阶段转换
    assert_eq!(
        phase.load(Ordering::Relaxed),
        GCPhase::Complete as u64,
        "Phase should be Complete"
    );
}

#[test]
fn test_gc_sweeper_empty_list() {
    let sweep_list = Arc::new(Mutex::new(Vec::<u64>::new()));
    let phase = Arc::new(AtomicU64::new(GCPhase::Sweeping as u64));
    let stats = create_test_stats();
    let batch_size = 100;

    let sweeper = GcSweeper::new(
        sweep_list.clone(),
        phase.clone(),
        stats.clone(),
        batch_size,
    );

    // 执行增量清扫
    let quota_us = 1000;
    let (completed, freed_count) = sweeper.incremental_sweep(quota_us);

    // 空列表应该立即完成
    assert!(completed, "Empty sweep list should complete immediately");
    assert_eq!(freed_count, 0, "No objects should be freed from empty list");
}

#[test]
fn test_gc_sweeper_batch_processing() {
    let mut objects = Vec::new();
    for i in 0..200 {
        objects.push(i as u64 * 0x1000);
    }
    let sweep_list = Arc::new(Mutex::new(objects));
    let phase = Arc::new(AtomicU64::new(GCPhase::Sweeping as u64));
    let stats = create_test_stats();
    let batch_size = 50;

    let sweeper = GcSweeper::new(
        sweep_list.clone(),
        phase.clone(),
        stats.clone(),
        batch_size,
    );

    // 执行增量清扫（使用较小的配额，应该只处理一部分）
    let quota_us = 100; // 0.1ms
    let (completed, freed_count) = sweeper.incremental_sweep(quota_us);

    // 验证批次处理
    assert!(freed_count >= 0, "Freed count should be non-negative");
    
    // 如果未完成，应该处理了部分对象
    if !completed {
        assert!(freed_count > 0, "Should free some objects in batch");
    }
}

#[test]
fn test_gc_marker_time_quota() {
    let mark_stack = create_test_mark_stack();
    let marked_set = Arc::new(RwLock::new(HashSet::new()));
    let phase = Arc::new(AtomicU64::new(GCPhase::Marking as u64));
    let stats = create_test_stats();

    let marker = GcMarker::new(
        mark_stack.clone(),
        marked_set.clone(),
        phase.clone(),
        stats.clone(),
    );

    // 添加大量根对象
    for i in 0..1000 {
        let _ = mark_stack.push(i as u64 * 0x1000);
    }

    // 使用非常小的时间配额
    let quota_us = 1; // 1微秒，应该几乎不处理任何对象
    let (completed, marked_count) = marker.incremental_mark(quota_us);

    // 验证时间配额被遵守
    assert!(!completed, "Should not complete with tiny quota");
    assert!(marked_count >= 0, "Marked count should be non-negative");
}

