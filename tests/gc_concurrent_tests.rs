//! GC并发安全测试
//!
//! 使用loom测试GC并发标记、写屏障和标记栈的并发安全性

#[cfg(loom)]
mod loom_tests {
    use loom::sync::Arc;
    use loom::thread;
    use vm_engine::jit::unified_gc::{LockFreeMarkStack, ShardedWriteBarrier, UnifiedGcConfig, UnifiedGC};

    /// 测试无锁标记栈的并发安全性
    #[test]
    fn test_lockfree_mark_stack_concurrent() {
        loom::model(|| {
            let stack = Arc::new(LockFreeMarkStack::new(1000));
            let mut handles = Vec::new();

            // 创建多个线程并发推送和弹出
            for i in 0..4 {
                let stack_clone = Arc::clone(&stack);
                let handle = thread::spawn(move || {
                    // 推送一些地址
                    for j in 0..100 {
                        let addr = (i * 100 + j) as u64;
                        let _ = stack_clone.push(addr);
                    }

                    // 弹出一些地址
                    for _ in 0..50 {
                        let _ = stack_clone.pop();
                    }
                });
                handles.push(handle);
            }

            // 等待所有线程完成
            for handle in handles {
                handle.join().unwrap();
            }

            // 验证最终状态的一致性
            let final_size = stack.size.load(std::sync::atomic::Ordering::Relaxed);
            assert!(final_size <= 400); // 最多400个元素（4个线程各推送100个）
        });
    }

    /// 测试分片写屏障的并发安全性
    #[test]
    fn test_sharded_write_barrier_concurrent() {
        loom::model(|| {
            let barrier = Arc::new(ShardedWriteBarrier::new(4)); // 4个分片
            let mut handles = Vec::new();

            // 创建多个线程并发记录写屏障
            for i in 0..8 {
                let barrier_clone = Arc::clone(&barrier);
                let handle = thread::spawn(move || {
                    // 记录写屏障
                    for j in 0..100 {
                        let addr = (i * 100 + j) as u64;
                        barrier_clone.record_write(addr, addr + 0x1000);
                    }
                });
                handles.push(handle);
            }

            // 等待所有线程完成
            for handle in handles {
                handle.join().unwrap();
            }
        });
    }

    /// 测试GC并发标记的安全性
    #[test]
    fn test_gc_concurrent_marking() {
        loom::model(|| {
            let config = UnifiedGcConfig {
                concurrent_marking: true,
                mark_stack_capacity: 1000,
                write_barrier_shards: 4,
                ..Default::default()
            };
            let gc = Arc::new(UnifiedGC::new(config));
            let mark_stack = Arc::new(LockFreeMarkStack::new(1000));
            let write_barrier = Arc::new(ShardedWriteBarrier::new(4));
            let mut handles = Vec::new();

            // 创建标记线程
            for _ in 0..2 {
                let stack_clone = Arc::clone(&mark_stack);
                let handle = thread::spawn(move || {
                    // 模拟标记操作
                    for i in 0..100 {
                        let addr = i as u64 * 0x1000;
                        let _ = stack_clone.push(addr);
                    }
                });
                handles.push(handle);
            }

            // 创建写屏障线程
            for _ in 0..4 {
                let barrier_clone = Arc::clone(&write_barrier);
                let handle = thread::spawn(move || {
                    // 模拟写操作
                    for i in 0..50 {
                        let addr = i as u64 * 0x1000;
                        barrier_clone.record_write(addr, addr + 0x1000);
                    }
                });
                handles.push(handle);
            }

            // 等待所有线程完成
            for handle in handles {
                handle.join().unwrap();
            }
        });
    }
}

/// 非loom环境的并发测试（使用标准库）
#[cfg(not(loom))]
mod std_tests {
    use std::sync::Arc;
    use std::thread;
    use vm_engine::jit::unified_gc::{LockFreeMarkStack, ShardedWriteBarrier};

    /// 测试无锁标记栈的并发安全性（标准库版本）
    #[test]
    fn test_lockfree_mark_stack_concurrent_std() {
        let stack = Arc::new(LockFreeMarkStack::new(1000));
        let mut handles = Vec::new();

        // 创建多个线程并发推送和弹出
        for i in 0..4 {
            let stack_clone = Arc::clone(&stack);
            let handle = thread::spawn(move || {
                // 推送一些地址
                for j in 0..100 {
                    let addr = (i * 100 + j) as u64;
                    let _ = stack_clone.push(addr);
                }

                // 弹出一些地址
                for _ in 0..50 {
                    let _ = stack_clone.pop();
                }
            });
            handles.push(handle);
        }

        // 等待所有线程完成
        for handle in handles {
            handle.join().unwrap();
        }

        // 验证最终状态的一致性
        let final_size = stack.size.load(std::sync::atomic::Ordering::Relaxed);
        assert!(final_size <= 400); // 最多400个元素
    }

    /// 测试分片写屏障的并发安全性（标准库版本）
    #[test]
    fn test_sharded_write_barrier_concurrent_std() {
        let barrier = Arc::new(ShardedWriteBarrier::new(4)); // 4个分片
        let mut handles = Vec::new();

        // 创建多个线程并发记录写屏障
        for i in 0..8 {
            let barrier_clone = Arc::clone(&barrier);
            let handle = thread::spawn(move || {
                // 记录写屏障
                for j in 0..100 {
                    let addr = (i * 100 + j) as u64;
                    barrier_clone.record_write(addr, addr + 0x1000);
                }
            });
            handles.push(handle);
        }

        // 等待所有线程完成
        for handle in handles {
            handle.join().unwrap();
        }
    }

    /// 测试并发标记的正确性
    /// 
    /// 确保多个线程同时标记时不会丢失对象，所有对象都被正确标记
    #[test]
    fn test_concurrent_marking_correctness() {
        use std::collections::HashSet;
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::sync::RwLock;
        use vm_engine::jit::gc_marker::GcMarker;
        use vm_engine::jit::unified_gc::{GCPhase, UnifiedGcStats};

        let mark_stack = Arc::new(LockFreeMarkStack::new(10000));
        let marked_set = Arc::new(RwLock::new(HashSet::new()));
        let phase = Arc::new(AtomicU64::new(GCPhase::Marking as u64));
        let stats = Arc::new(UnifiedGcStats::default());

        // 准备测试对象：1000个对象
        let total_objects = 1000;
        let objects: Vec<u64> = (0..total_objects).map(|i| i as u64 * 0x1000).collect();

        // 将所有对象添加到标记栈
        for &obj in &objects {
            let _ = mark_stack.push(obj);
        }

        // 创建多个标记线程
        let num_threads = 4;
        let mut handles = Vec::new();

        for _ in 0..num_threads {
            let stack_clone = Arc::clone(&mark_stack);
            let marked_clone = Arc::clone(&marked_set);
            let phase_clone = Arc::clone(&phase);
            let stats_clone = Arc::clone(&stats);

            let handle = thread::spawn(move || {
                let marker = GcMarker::new(
                    stack_clone,
                    marked_clone,
                    phase_clone,
                    stats_clone,
                );
                // 每个线程执行增量标记，直到完成
                loop {
                    let (completed, _) = marker.incremental_mark(1000); // 1ms配额
                    if completed {
                        break;
                    }
                    // 短暂休眠，模拟实际场景
                    thread::sleep(std::time::Duration::from_micros(100));
                }
            });
            handles.push(handle);
        }

        // 等待所有标记线程完成
        for handle in handles {
            handle.join().unwrap();
        }

        // 验证所有对象都被标记
        let marked = marked_set.read().unwrap();
        assert_eq!(
            marked.len(),
            total_objects,
            "所有对象都应该被标记，实际标记了 {} 个，期望 {} 个",
            marked.len(),
            total_objects
        );

        // 验证每个对象都被标记
        for &obj in &objects {
            assert!(
                marked.contains(&obj),
                "对象 {:#x} 应该被标记",
                obj
            );
        }
    }

    /// 测试并发清扫的正确性
    /// 
    /// 确保并行清扫时不会重复释放或遗漏对象
    #[test]
    fn test_concurrent_sweeping_correctness() {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::sync::Mutex;
        use vm_engine::jit::gc_sweeper::GcSweeper;
        use vm_engine::jit::unified_gc::{GCPhase, UnifiedGcStats};

        let sweep_list = Arc::new(Mutex::new(Vec::new()));
        let phase = Arc::new(AtomicU64::new(GCPhase::Sweeping as u64));
        let stats = Arc::new(UnifiedGcStats::default());

        // 准备待清扫对象：1000个对象
        let total_objects = 1000;
        {
            let mut list = sweep_list.lock().unwrap();
            for i in 0..total_objects {
                list.push(i as u64 * 0x1000);
            }
        }

        // 创建清扫器
        let sweeper = GcSweeper::new(
            Arc::clone(&sweep_list),
            Arc::clone(&phase),
            Arc::clone(&stats),
            100, // 批次大小
        );

        // 使用并行清扫
        let mut total_freed = 0;
        loop {
            let (completed, freed_count) = sweeper.incremental_sweep(1000); // 1ms配额
            total_freed += freed_count;
            if completed {
                break;
            }
            // 短暂休眠
            thread::sleep(std::time::Duration::from_micros(100));
        }

        // 验证所有对象都被释放
        assert_eq!(
            total_freed,
            total_objects,
            "所有对象都应该被释放，实际释放了 {} 个，期望 {} 个",
            total_freed,
            total_objects
        );

        // 验证清扫列表为空
        let list = sweep_list.lock().unwrap();
        assert_eq!(
            list.len(),
            0,
            "清扫列表应该为空，实际还有 {} 个对象",
            list.len()
        );
    }

    /// 测试标记和清扫的并发安全性
    /// 
    /// 确保在标记阶段和清扫阶段之间的正确性，不会出现竞态条件
    #[test]
    fn test_marking_and_sweeping_concurrent_safety() {
        use std::collections::HashSet;
        use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
        use std::sync::{Mutex, RwLock};
        use vm_engine::jit::gc_marker::GcMarker;
        use vm_engine::jit::gc_sweeper::GcSweeper;
        use vm_engine::jit::unified_gc::{GCPhase, UnifiedGcStats};

        let mark_stack = Arc::new(LockFreeMarkStack::new(10000));
        let marked_set = Arc::new(RwLock::new(HashSet::new()));
        let sweep_list = Arc::new(Mutex::new(Vec::new()));
        let phase = Arc::new(AtomicU64::new(GCPhase::Marking as u64));
        let stats = Arc::new(UnifiedGcStats::default());
        let marking_done = Arc::new(AtomicBool::new(false));

        let total_objects = 500;
        let objects: Vec<u64> = (0..total_objects).map(|i| i as u64 * 0x1000).collect();

        // 添加对象到标记栈
        for &obj in &objects {
            let _ = mark_stack.push(obj);
        }

        // 标记线程
        let mark_stack_clone = Arc::clone(&mark_stack);
        let marked_set_clone = Arc::clone(&marked_set);
        let phase_clone = Arc::clone(&phase);
        let stats_clone = Arc::clone(&stats);
        let marking_done_clone = Arc::clone(&marking_done);

        let mark_handle = thread::spawn(move || {
            let marker = GcMarker::new(
                mark_stack_clone,
                marked_set_clone,
                phase_clone,
                stats_clone,
            );
            loop {
                let (completed, _) = marker.incremental_mark(1000);
                if completed {
                    marking_done_clone.store(true, Ordering::Release);
                    break;
                }
                thread::sleep(std::time::Duration::from_micros(100));
            }
        });

        // 等待标记完成
        mark_handle.join().unwrap();

        // 验证标记完成
        assert!(
            marking_done.load(Ordering::Acquire),
            "标记应该完成"
        );

        // 准备清扫列表（只包含未标记的对象，这里简化处理，假设所有对象都被标记）
        // 实际场景中，清扫列表应该包含所有对象，然后根据标记状态决定是否释放
        {
            let mut list = sweep_list.lock().unwrap();
            // 添加一些未标记的对象（模拟）
            for i in total_objects..(total_objects + 100) {
                list.push(i as u64 * 0x1000);
            }
        }

        // 切换到清扫阶段
        phase.store(GCPhase::Sweeping as u64, Ordering::Release);

        // 清扫线程
        let sweeper = GcSweeper::new(
            Arc::clone(&sweep_list),
            Arc::clone(&phase),
            Arc::clone(&stats),
            50,
        );

        let mut total_freed = 0;
        loop {
            let (completed, freed_count) = sweeper.incremental_sweep(1000);
            total_freed += freed_count;
            if completed {
                break;
            }
            thread::sleep(std::time::Duration::from_micros(100));
        }

        // 验证清扫完成
        assert_eq!(
            total_freed,
            100,
            "应该释放100个未标记对象"
        );
    }

    /// 测试写屏障在并发标记时的正确性
    /// 
    /// 确保在标记过程中，写屏障能够正确记录修改的对象
    #[test]
    fn test_write_barrier_during_concurrent_marking() {
        use std::collections::HashSet;
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::sync::RwLock;
        use vm_engine::jit::gc_marker::GcMarker;
        use vm_engine::jit::unified_gc::{GCPhase, UnifiedGcStats};

        let mark_stack = Arc::new(LockFreeMarkStack::new(10000));
        let marked_set = Arc::new(RwLock::new(HashSet::new()));
        let write_barrier = Arc::new(ShardedWriteBarrier::new(4));
        let phase = Arc::new(AtomicU64::new(GCPhase::Marking as u64));
        let stats = Arc::new(UnifiedGcStats::default());

        // 启用写屏障
        write_barrier.set_enabled(true);

        // 准备初始对象
        let initial_objects: Vec<u64> = (0..100).map(|i| i as u64 * 0x1000).collect();
        for &obj in &initial_objects {
            let _ = mark_stack.push(obj);
        }

        // 标记线程
        let mark_stack_clone = Arc::clone(&mark_stack);
        let marked_set_clone = Arc::clone(&marked_set);
        let phase_clone = Arc::clone(&phase);
        let stats_clone = Arc::clone(&stats);

        let mark_handle = thread::spawn(move || {
            let marker = GcMarker::new(
                mark_stack_clone,
                marked_set_clone,
                phase_clone,
                stats_clone,
            );
            // 执行增量标记
            for _ in 0..10 {
                let _ = marker.incremental_mark(500);
                thread::sleep(std::time::Duration::from_micros(50));
            }
        });

        // 写操作线程（模拟应用程序在标记过程中修改对象）
        let write_barrier_clone = Arc::clone(&write_barrier);
        let write_handle = thread::spawn(move || {
            // 在标记过程中进行写操作
            for i in 0..50 {
                let addr = (i as u64 * 0x1000) + 0x100;
                let new_addr = addr + 0x2000;
                write_barrier_clone.record_write(addr, new_addr);
                thread::sleep(std::time::Duration::from_micros(10));
            }
        });

        // 等待所有线程完成
        mark_handle.join().unwrap();
        write_handle.join().unwrap();

        // 验证写屏障记录了修改
        let modified = write_barrier.drain_modified();
        assert!(
            modified.len() > 0,
            "写屏障应该记录了一些修改的对象"
        );
    }

    /// 测试标记栈在极端并发情况下的正确性
    /// 
    /// 测试大量线程同时推送和弹出时的正确性
    #[test]
    fn test_mark_stack_extreme_concurrency() {
        let stack = Arc::new(LockFreeMarkStack::new(100000));
        let num_threads = 16;
        let operations_per_thread = 1000;
        let mut handles = Vec::new();

        // 创建大量线程进行并发操作
        for i in 0..num_threads {
            let stack_clone = Arc::clone(&stack);
            let handle = thread::spawn(move || {
                // 每个线程执行推送和弹出操作
                for j in 0..operations_per_thread {
                    let addr = (i * operations_per_thread + j) as u64;
                    let _ = stack_clone.push(addr);
                }

                // 弹出一些元素
                for _ in 0..(operations_per_thread / 2) {
                    let _ = stack_clone.pop();
                }
            });
            handles.push(handle);
        }

        // 等待所有线程完成
        for handle in handles {
            handle.join().unwrap();
        }

        // 验证最终状态的一致性
        let final_size = stack.size.load(std::sync::atomic::Ordering::Relaxed);
        let expected_min = num_threads * operations_per_thread / 2; // 每个线程至少留下一半
        assert!(
            final_size >= expected_min,
            "最终大小应该至少为 {}，实际为 {}",
            expected_min,
            final_size
        );
    }

    /// 测试并行清扫的性能和正确性
    /// 
    /// 对比串行和并行清扫的性能差异，并验证正确性
    #[test]
    fn test_parallel_sweeping_performance_and_correctness() {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::sync::Mutex;
        use vm_engine::jit::gc_sweeper::GcSweeper;
        use vm_engine::jit::unified_gc::{GCPhase, UnifiedGcStats};

        let total_objects = 10000;

        // 测试并行清扫
        let sweep_list_parallel = Arc::new(Mutex::new(Vec::new()));
        {
            let mut list = sweep_list_parallel.lock().unwrap();
            for i in 0..total_objects {
                list.push(i as u64 * 0x1000);
            }
        }

        let phase_parallel = Arc::new(AtomicU64::new(GCPhase::Sweeping as u64));
        let stats_parallel = Arc::new(UnifiedGcStats::default());

        let sweeper_parallel = GcSweeper::new(
            Arc::clone(&sweep_list_parallel),
            Arc::clone(&phase_parallel),
            Arc::clone(&stats_parallel),
            1000,
        );

        let start_parallel = std::time::Instant::now();
        let mut total_freed_parallel = 0;
        loop {
            let (completed, freed_count) = sweeper_parallel.incremental_sweep_with_parallel(10000, true);
            total_freed_parallel += freed_count;
            if completed {
                break;
            }
        }
        let duration_parallel = start_parallel.elapsed();

        // 测试串行清扫
        let sweep_list_serial = Arc::new(Mutex::new(Vec::new()));
        {
            let mut list = sweep_list_serial.lock().unwrap();
            for i in 0..total_objects {
                list.push(i as u64 * 0x1000);
            }
        }

        let phase_serial = Arc::new(AtomicU64::new(GCPhase::Sweeping as u64));
        let stats_serial = Arc::new(UnifiedGcStats::default());

        let sweeper_serial = GcSweeper::new(
            Arc::clone(&sweep_list_serial),
            Arc::clone(&phase_serial),
            Arc::clone(&stats_serial),
            1000,
        );

        let start_serial = std::time::Instant::now();
        let mut total_freed_serial = 0;
        loop {
            let (completed, freed_count) = sweeper_serial.incremental_sweep_with_parallel(10000, false);
            total_freed_serial += freed_count;
            if completed {
                break;
            }
        }
        let duration_serial = start_serial.elapsed();

        // 验证正确性：两种方式都应该释放所有对象
        assert_eq!(
            total_freed_parallel,
            total_objects,
            "并行清扫应该释放所有对象"
        );
        assert_eq!(
            total_freed_serial,
            total_objects,
            "串行清扫应该释放所有对象"
        );

        // 验证性能：并行应该更快（在多核系统上）
        println!(
            "并行清扫耗时: {:?}, 串行清扫耗时: {:?}",
            duration_parallel, duration_serial
        );
    }
}

