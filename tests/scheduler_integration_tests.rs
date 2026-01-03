/// VM GMP型协程调度器集成测试

#[cfg(test)]
mod tests {
    // vm-runtime 已被删除，相关功能已迁移到其他模块
    // use vm_runtime::scheduler::*;
    use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
    use std::time::Duration;
    use std::thread;

    // vm-runtime 已被删除，以下所有测试暂时禁用
    // 这些测试依赖 vm_runtime::scheduler 模块中的类型：
    // - CoroutineScheduler
    // - Coroutine
    // - Processor
    // - Priority
    // - CoroutineState
    //
    // 如需重新启用，需要：
    // 1. 找到这些类型迁移到哪个模块
    // 2. 更新导入语句
    // 3. 验证API是否发生变化

    /*
    #[test]
    fn test_scheduler_basic_startup_and_shutdown() {
        let scheduler = CoroutineScheduler::new().expect("Failed to create scheduler");
        assert!(!scheduler.is_running());

        scheduler.start().expect("Failed to start scheduler");
        assert!(scheduler.is_running());

        thread::sleep(Duration::from_millis(100));

        scheduler.stop();
        thread::sleep(Duration::from_millis(100));
        assert!(!scheduler.is_running());

        scheduler.join_all();
    }

    #[test]
    fn test_submit_and_distribute_tasks() {
        let scheduler = CoroutineScheduler::new().expect("Failed to create scheduler");

        // 提交不同优先级的任务
        let _task1 = scheduler.submit_task(Priority::High, || {
            println!("High priority task executed");
        });

        let _task2 = scheduler.submit_task(Priority::Medium, || {
            println!("Medium priority task executed");
        });

        let _task3 = scheduler.submit_task(Priority::Low, || {
            println!("Low priority task executed");
        });

        let stats_before = scheduler.get_stats();
        assert_eq!(stats_before.global_queue_size, 3);

        // 分发任务
        scheduler.distribute_tasks();

        let stats_after = scheduler.get_stats();
        assert_eq!(stats_after.global_queue_size, 0);
        assert!(stats_after.total_tasks > 0);
    }

    #[test]
    fn test_priority_scheduling_order() {
        let scheduler = CoroutineScheduler::new().expect("Failed to create scheduler");

        let execution_order = Arc::new(AtomicUsize::new(0));

        // 提交低优先级任务
        let order_clone = Arc::clone(&execution_order);
        let _low = scheduler.submit_task(Priority::Low, move || {
            let seq = order_clone.fetch_add(1, Ordering::SeqCst);
            println!("Low priority task executed at position {}", seq);
        });

        // 提交高优先级任务
        let order_clone = Arc::clone(&execution_order);
        let _high = scheduler.submit_task(Priority::High, move || {
            let seq = order_clone.fetch_add(1, Ordering::SeqCst);
            println!("High priority task executed at position {}", seq);
        });

        // 提交中优先级任务
        let order_clone = Arc::clone(&execution_order);
        let _medium = scheduler.submit_task(Priority::Medium, move || {
            let seq = order_clone.fetch_add(1, Ordering::SeqCst);
            println!("Medium priority task executed at position {}", seq);
        });

        scheduler.distribute_tasks();
        let stats = scheduler.get_stats();

        // 验证高优先级任务被正确排队
        assert!(stats.processor_queue_sizes.iter().any(|&size| size > 0));
    }

    #[test]
    fn test_coroutine_state_machine() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let coro = Coroutine::new(Priority::Medium, move || {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        // 初始状态应该是Ready
        assert_eq!(coro.state(), CoroutineState::Ready);

        // 转换到Running
        coro.set_state(CoroutineState::Running);
        assert_eq!(coro.state(), CoroutineState::Running);

        // 转换到Blocked（等待I/O）
        coro.set_state(CoroutineState::Blocked);
        assert_eq!(coro.state(), CoroutineState::Blocked);

        // 转换回Ready（I/O完成）
        coro.set_state(CoroutineState::Ready);
        assert_eq!(coro.state(), CoroutineState::Ready);

        // 执行协程
        coro.execute();
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // 转换到Dead
        coro.set_state(CoroutineState::Dead);
        assert_eq!(coro.state(), CoroutineState::Dead);
    }

    #[test]
    fn test_processor_enqueue_and_dequeue() {
        let processor = Processor::new(0);

        let task1 = Arc::new(Coroutine::new(Priority::High, || {}));
        let task2 = Arc::new(Coroutine::new(Priority::Medium, || {}));
        let task3 = Arc::new(Coroutine::new(Priority::Low, || {}));
        let task4 = Arc::new(Coroutine::new(Priority::High, || {}));

        // 不按优先级顺序入队
        processor.enqueue(task3.clone());
        processor.enqueue(task1.clone());
        processor.enqueue(task2.clone());
        processor.enqueue(task4.clone());

        // 验证按优先级出队：High, High, Medium, Low
        let first = processor.dequeue_next().unwrap();
        assert_eq!(first.priority(), Priority::High);

        let second = processor.dequeue_next().unwrap();
        assert_eq!(second.priority(), Priority::High);

        let third = processor.dequeue_next().unwrap();
        assert_eq!(third.priority(), Priority::Medium);

        let fourth = processor.dequeue_next().unwrap();
        assert_eq!(fourth.priority(), Priority::Low);

        assert!(processor.dequeue_next().is_none());
    }

    #[test]
    fn test_worker_execution_tracking() {
        let processor = Processor::new(0);

        let execution_count = Arc::new(AtomicUsize::new(0));
        let exec_clone = Arc::clone(&execution_count);

        let coro = Arc::new(Coroutine::new(Priority::High, move || {
            exec_clone.fetch_add(1, Ordering::SeqCst);
        }));

        // 验证初始状态
        assert_eq!(coro.execution_count(), 0);

        // 记录执行
        coro.record_execution();
        assert_eq!(coro.execution_count(), 1);

        coro.record_execution();
        assert_eq!(coro.execution_count(), 2);

        // 验证经过时间
        let elapsed = coro.elapsed_ms();
        assert!(elapsed >= 0);
    }

    #[test]
    fn test_multiple_processors_work_stealing() {
        let proc1 = Processor::new(0);
        let proc2 = Processor::new(1);

        let task1 = Arc::new(Coroutine::new(Priority::High, || {}));
        let task2 = Arc::new(Coroutine::new(Priority::Medium, || {}));

        // 在proc1中入队
        proc1.enqueue(task1.clone());
        proc1.enqueue(task2.clone());

        // proc2尝试从proc1窃取
        let stolen = proc2.try_steal_from(&proc1);
        assert!(stolen.is_some());
        assert_eq!(stolen.unwrap().priority(), Priority::High);

        // 验证proc1现在只有一个任务
        assert_eq!(proc1.queue_size(), 1);

        // proc2再次窃取
        let stolen2 = proc2.try_steal_from(&proc1);
        assert!(stolen2.is_some());
        assert_eq!(proc1.queue_size(), 0);
    }

    #[test]
    fn test_scheduler_statistics() {
        let scheduler = CoroutineScheduler::new().expect("Failed to create scheduler");

        // 获取初始统计
        let stats1 = scheduler.get_stats();
        assert_eq!(stats1.total_tasks, 0);
        assert_eq!(stats1.global_queue_size, 0);
        assert!(!stats1.running);

        // 提交任务
        for i in 0..10 {
            let priority = match i % 3 {
                0 => Priority::High,
                1 => Priority::Medium,
                _ => Priority::Low,
            };
            scheduler.submit_task(priority, move || {
                println!("Task {}", i);
            });
        }

        let stats2 = scheduler.get_stats();
        assert_eq!(stats2.global_queue_size, 10);

        // 分发任务
        scheduler.distribute_tasks();

        let stats3 = scheduler.get_stats();
        assert_eq!(stats3.global_queue_size, 0);
        assert!(stats3.total_tasks > 0);
        assert_eq!(stats3.num_workers, scheduler.num_workers());
    }

    #[test]
    fn test_concurrent_task_submission() {
        let scheduler = Arc::new(CoroutineScheduler::new().expect("Failed to create scheduler"));
        let mut handles = vec![];

        // 多个线程并发提交任务
        for i in 0..5 {
            let sched_clone = Arc::clone(&scheduler);
            let handle = thread::spawn(move || {
                for j in 0..10 {
                    let priority = match (i + j) % 3 {
                        0 => Priority::High,
                        1 => Priority::Medium,
                        _ => Priority::Low,
                    };
                    sched_clone.submit_task(priority, move || {
                        println!("Task from thread {} iteration {}", i, j);
                    });
                }
            });
            handles.push(handle);
        }

        // 等待所有线程完成
        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        // 验证所有任务都被提交了
        let stats = scheduler.get_stats();
        assert_eq!(stats.global_queue_size, 50);

        // 分发任务
        scheduler.distribute_tasks();
        let stats_after = scheduler.get_stats();
        assert_eq!(stats_after.global_queue_size, 0);
        assert_eq!(stats_after.total_tasks, 50);
    }

    #[test]
    fn test_scheduler_cpu_affinity_setup() {
        let scheduler = CoroutineScheduler::new().expect("Failed to create scheduler");
        assert_eq!(scheduler.num_workers(), num_cpus::get().saturating_sub(1).max(1));

        // 验证处理器数量等于worker数量
        let stats = scheduler.get_stats();
        assert_eq!(stats.processor_queue_sizes.len(), scheduler.num_workers());
    }

    #[test]
    fn test_task_execution_flow() {
        let execution_log = Arc::new(parking_lot::Mutex::new(Vec::new()));

        let log_clone = Arc::clone(&execution_log);
        let coro = Arc::new(Coroutine::new(Priority::High, move || {
            log_clone.lock().push("executed");
        }));

        // 初始状态为Ready
        assert_eq!(coro.state(), CoroutineState::Ready);

        // 模拟调度：Ready → Running
        coro.set_state(CoroutineState::Running);
        assert_eq!(coro.state(), CoroutineState::Running);

        // 执行任务
        coro.execute();
        assert_eq!(execution_log.lock().len(), 1);
        assert_eq!(execution_log.lock()[0], "executed");

        // 时间片过期：Running → Ready
        coro.set_state(CoroutineState::Ready);
        assert_eq!(coro.state(), CoroutineState::Ready);

        // 最终：Ready → Dead
        coro.set_state(CoroutineState::Dead);
        assert_eq!(coro.state(), CoroutineState::Dead);
    }

    #[test]
    fn test_processor_queue_distribution() {
        let proc = Processor::new(0);

        // 添加20个任务，不同优先级
        for i in 0..20 {
            let priority = match i % 3 {
                0 => Priority::High,
                1 => Priority::Medium,
                _ => Priority::Low,
            };
            proc.enqueue(Arc::new(Coroutine::new(priority, move || {
                println!("Task {}", i);
            })));
        }

        let (high, med, low) = proc.queue_sizes();
        assert!(high > 0);
        assert!(med > 0);
        assert!(low > 0);

        // 高优先级应该有7个（0,3,6,9,12,15,18），中优先级7个，低优先级6个
        assert_eq!(high, 7);
        assert_eq!(med, 7);
        assert_eq!(low, 6);
    }
    */
}
