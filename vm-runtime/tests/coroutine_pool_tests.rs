//! 协程调度器测试套件

use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use vm_runtime::CoroutineScheduler;
use vm_runtime::Priority;

#[tokio::test]
fn test_coroutine_scheduler_basic() {
    let scheduler = CoroutineScheduler::new().expect("Failed to create scheduler");
    scheduler.start().expect("Failed to start scheduler");

    // 提交任务
    let task = || {
        std::thread::sleep(Duration::from_millis(10));
    };

    let coroutine = scheduler.submit_task(Priority::Medium, task);
    assert!(!coroutine.id().is_empty());
    
    // 等待任务完成
    std::thread::sleep(Duration::from_millis(20));
    
    scheduler.stop();
    scheduler.join_all();
}

#[tokio::test]
fn test_coroutine_scheduler_multiple_tasks() {
    let scheduler = CoroutineScheduler::new().expect("Failed to create scheduler");
    scheduler.start().expect("Failed to start scheduler");

    // 提交多个任务
    for i in 0..5 {
        let task = move || {
            std::thread::sleep(Duration::from_millis(50));
        };
        let _coroutine = scheduler.submit_task(Priority::Medium, task);
    }

    // 等待所有任务完成
    std::thread::sleep(Duration::from_millis(300));
    
    let stats = scheduler.get_stats();
    println!("Scheduler stats: {:?}", stats);
    
    scheduler.stop();
    scheduler.join_all();
}

#[tokio::test]
fn test_coroutine_scheduler_priority() {
    let scheduler = CoroutineScheduler::new().expect("Failed to create scheduler");
    scheduler.start().expect("Failed to start scheduler");

    // 提交不同优先级的任务
    let high_task = || {
        std::thread::sleep(Duration::from_millis(10));
    };
    
    let low_task = || {
        std::thread::sleep(Duration::from_millis(10));
    };

    let _high_coro = scheduler.submit_task(Priority::High, high_task);
    let _low_coro = scheduler.submit_task(Priority::Low, low_task);

    // 等待任务完成
    std::thread::sleep(Duration::from_millis(30));
    
    scheduler.stop();
    scheduler.join_all();
}

#[tokio::test]
fn test_coroutine_scheduler_stats() {
    let scheduler = CoroutineScheduler::new().expect("Failed to create scheduler");
    
    // 提交一些任务
    for i in 0..3 {
        let task = move || {
            std::thread::sleep(Duration::from_millis(10));
        };
        let _coroutine = scheduler.submit_task(Priority::Medium, task);
    }

    let stats = scheduler.get_stats();
    assert_eq!(stats.global_queue_size, 3);
    assert_eq!(stats.total_tasks, 3);
    assert!(!stats.running);
}
