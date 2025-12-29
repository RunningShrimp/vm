//! 无锁队列测试
//!
//! 验证无锁队列的正确性和性能

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use vm_common::lockfree::{
    BoundedLockFreeQueue, InstrumentedLockFreeQueue, LockFreeQueue, MpmcQueue, QueueError,
    WorkStealingQueue,
};

/// 测试基本无锁队列功能
#[test]
fn test_basic_lockfree_queue() {
    println!("=== 基本无锁队列测试 ===");

    let queue = LockFreeQueue::new();

    // 测试空队列
    assert!(queue.is_empty());
    assert_eq!(queue.len(), 0);
    assert!(queue.try_pop().is_none());

    // 测试入队
    queue.push(1).unwrap();
    queue.push(2).unwrap();
    queue.push(3).unwrap();

    assert!(!queue.is_empty());
    assert_eq!(queue.len(), 3);

    // 测试出队
    assert_eq!(queue.pop().unwrap(), 1);
    assert_eq!(queue.pop().unwrap(), 2);
    assert_eq!(queue.pop().unwrap(), 3);

    assert!(queue.is_empty());
    assert_eq!(queue.len(), 0);

    println!("基本无锁队列测试通过");
}

/// 测试有界无锁队列功能
#[test]
fn test_bounded_lockfree_queue() {
    println!("=== 有界无锁队列测试 ===");

    let queue = BoundedLockFreeQueue::new(2);

    // 测试空队列
    assert!(queue.is_empty());
    assert!(!queue.is_full());
    assert_eq!(queue.len(), 0);
    assert_eq!(queue.capacity(), 2);

    // 测试入队
    queue.push(1).unwrap();
    queue.push(2).unwrap();

    assert!(!queue.is_empty());
    assert!(queue.is_full());
    assert_eq!(queue.len(), 2);

    // 测试队列已满
    assert!(queue.push(3).is_err());

    // 测试出队
    assert_eq!(queue.pop().unwrap(), 1);
    assert_eq!(queue.pop().unwrap(), 2);

    assert!(queue.is_empty());
    assert!(!queue.is_full());

    println!("有界无锁队列测试通过");
}

/// 测试多生产者多消费者队列
#[test]
fn test_mpmc_queue() {
    println!("=== 多生产者多消费者队列测试 ===");

    let queue = MpmcQueue::new();

    // 创建生产者和消费者
    let producer1 = queue.create_producer();
    let producer2 = queue.create_producer();
    let consumer1 = queue.create_consumer();
    let consumer2 = queue.create_consumer();

    // 测试生产者
    producer1.push(1).unwrap();
    producer1.push(2).unwrap();
    producer2.push(3).unwrap();
    producer2.push(4).unwrap();

    // 测试消费者
    let mut results = Vec::new();
    while let Some(value) = consumer1.try_pop() {
        results.push(value);
    }
    while let Some(value) = consumer2.try_pop() {
        results.push(value);
    }

    // 验证结果
    assert_eq!(results.len(), 4);
    assert!(results.contains(&1));
    assert!(results.contains(&2));
    assert!(results.contains(&3));
    assert!(results.contains(&4));

    println!("多生产者多消费者队列测试通过");
}

/// 测试工作窃取队列
#[test]
fn test_work_stealing_queue() {
    println!("=== 工作窃取队列测试 ===");

    let shared = Arc::new(LockFreeQueue::new());
    let worker_queue = WorkStealingQueue::new(shared.clone(), 0);

    // 添加任务到共享队列
    shared.push(1).unwrap();
    shared.push(2).unwrap();

    // 添加任务到本地队列
    worker_queue.push_local(3).unwrap();
    worker_queue.push_local(4).unwrap();

    // 测试本地弹出
    assert_eq!(worker_queue.pop_local().unwrap(), 3);
    assert_eq!(worker_queue.pop_local().unwrap(), 4);

    // 测试共享弹出
    assert_eq!(worker_queue.pop_shared().unwrap(), 1);
    assert_eq!(worker_queue.pop_shared().unwrap(), 2);

    assert!(!worker_queue.has_work());

    println!("工作窃取队列测试通过");
}

/// 测试带统计信息的无锁队列
#[test]
fn test_instrumented_lockfree_queue() {
    println!("=== 带统计信息的无锁队列测试 ===");

    let queue = InstrumentedLockFreeQueue::new();

    // 执行一些操作
    queue.push(1).unwrap();
    queue.push(2).unwrap();
    queue.pop().unwrap();
    queue.try_pop();

    // 检查统计信息
    let stats = queue.get_stats();
    assert_eq!(stats.push_count, 2);
    assert_eq!(stats.pop_count, 1);
    assert_eq!(stats.max_size, 2);

    println!("带统计信息的无锁队列测试通过");
}

/// 测试并发性能
#[test]
fn test_concurrent_performance() {
    println!("=== 并发性能测试 ===");

    let queue = Arc::new(LockFreeQueue::new());
    let thread_count = 8;
    let operations_per_thread = 10000;

    let start = Instant::now();

    // 生产者线程
    let mut producer_handles = Vec::new();
    for i in 0..thread_count / 2 {
        let queue = queue.clone();
        let handle = thread::spawn(move || {
            for j in 0..operations_per_thread {
                queue.push(i * operations_per_thread + j).unwrap();
            }
        });
        producer_handles.push(handle);
    }

    // 消费者线程
    let mut consumer_handles = Vec::new();
    for _ in 0..thread_count / 2 {
        let queue = queue.clone();
        let handle = thread::spawn(move || {
            let mut count = 0;
            while count < operations_per_thread {
                if queue.try_pop().is_some() {
                    count += 1;
                }
            }
        });
        consumer_handles.push(handle);
    }

    // 等待所有线程完成
    for handle in producer_handles {
        handle.join().unwrap();
    }
    for handle in consumer_handles {
        handle.join().unwrap();
    }

    let duration = start.elapsed();
    let total_operations = (thread_count * operations_per_thread) as u64;
    let ops_per_second = total_operations * 1000 / duration.as_millis() as u64;

    println!("并发性能测试完成");
    println!("总操作数: {}", total_operations);
    println!("耗时: {:?}", duration);
    println!("每秒操作数: {}", ops_per_second);

    // 性能断言
    assert!(ops_per_second > 100000, "并发性能应该超过10万操作/秒");
}

/// 测试高并发场景
#[test]
fn test_high_concurrency() {
    println!("=== 高并发场景测试 ===");

    let queue = Arc::new(InstrumentedLockFreeQueue::new());
    let thread_count = 16;
    let operations_per_thread = 5000;

    let start = Instant::now();

    let mut handles = Vec::new();

    // 混合生产者和消费者线程
    for i in 0..thread_count {
        let queue = queue.clone();
        let handle = thread::spawn(move || {
            if i % 2 == 0 {
                // 生产者
                for j in 0..operations_per_thread {
                    queue.push(i * operations_per_thread + j).unwrap();
                }
            } else {
                // 消费者
                let mut count = 0;
                while count < operations_per_thread {
                    if queue.try_pop().is_some() {
                        count += 1;
                    }
                }
            }
        });
        handles.push(handle);
    }

    // 等待所有线程完成
    for handle in handles {
        handle.join().unwrap();
    }

    let duration = start.elapsed();
    let stats = queue.get_stats();

    println!("高并发场景测试完成");
    println!("耗时: {:?}", duration);
    println!("入队次数: {}", stats.push_count);
    println!("出队次数: {}", stats.pop_count);
    println!("最大队列大小: {}", stats.max_size);

    // 验证统计信息
    assert_eq!(
        stats.push_count,
        (thread_count / 2 * operations_per_thread) as usize
    );
    assert_eq!(
        stats.pop_count,
        (thread_count / 2 * operations_per_thread) as usize
    );
}

/// 测试队列的内存安全性
#[test]
fn test_memory_safety() {
    println!("=== 内存安全性测试 ===");

    let queue = Arc::new(LockFreeQueue::new());
    let thread_count = 8;
    let operations_per_thread = 1000;

    let mut handles = Vec::new();

    // 创建多个线程同时操作队列
    for i in 0..thread_count {
        let queue = queue.clone();
        let handle = thread::spawn(move || {
            for j in 0..operations_per_thread {
                if j % 2 == 0 {
                    // 偶数操作：入队
                    queue.push(i * operations_per_thread + j).unwrap();
                } else {
                    // 奇数操作：尝试出队
                    queue.try_pop();
                }
            }
        });
        handles.push(handle);
    }

    // 等待所有线程完成
    for handle in handles {
        handle.join().unwrap();
    }

    // 清空队列
    while queue.try_pop().is_some() {
        // 继续弹出直到队列为空
    }

    println!("内存安全性测试通过");
}

/// 测试队列的异常处理
#[test]
fn test_error_handling() {
    println!("=== 异常处理测试 ===");

    // 测试空队列出队
    let queue = LockFreeQueue::new();
    let result = queue.pop();
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), QueueError::Empty);

    // 测试有界队列已满
    let bounded_queue = BoundedLockFreeQueue::new(1);
    bounded_queue.push(1).unwrap();
    let result = bounded_queue.push(2);
    assert!(result.is_err());

    println!("异常处理测试通过");
}
