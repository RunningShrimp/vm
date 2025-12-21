//! 无锁哈希表测试
//!
//! 验证无锁哈希表的正确性和性能

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use vm_common::lockfree::{
    CacheAwareHashMap, HashMapError, InstrumentedLockFreeHashMap, LockFreeHashMap, StripedHashMap,
};

/// 测试基本无锁哈希表功能
#[test]
fn test_basic_lockfree_hashmap() {
    println!("=== 基本无锁哈希表测试 ===");

    let map: LockFreeHashMap<i32, i32> = LockFreeHashMap::new();

    // 测试空表
    assert!(map.is_empty());
    assert_eq!(map.len(), 0);
    assert!(map.get(&1).is_none());

    // 测试插入
    map.insert(1, "one").unwrap();
    map.insert(2, "two").unwrap();
    map.insert(3, "three").unwrap();

    assert!(!map.is_empty());
    assert_eq!(map.len(), 3);

    // 测试获取
    assert_eq!(map.get(&1), Some("one"));
    assert_eq!(map.get(&2), Some("two"));
    assert_eq!(map.get(&3), Some("three"));
    assert!(map.get(&4).is_none());

    // 测试删除
    assert_eq!(map.remove(&2), Some("two"));
    assert_eq!(map.get(&2), None);
    assert_eq!(map.len(), 2);

    // 测试包含键
    assert!(map.contains_key(&1));
    assert!(!map.contains_key(&2));

    println!("基本无锁哈希表测试通过");
}

/// 测试分片哈希表功能
#[test]
fn test_striped_hashmap() {
    println!("=== 分片哈希表测试 ===");

    let map = StripedHashMap::with_shards(4);

    // 测试插入
    map.insert(1, "one").unwrap();
    map.insert(2, "two").unwrap();
    map.insert(3, "three").unwrap();

    // 测试获取
    assert_eq!(map.get(&1), Some("one"));
    assert_eq!(map.get(&2), Some("two"));
    assert_eq!(map.get(&3), Some("three"));

    // 测试大小
    assert_eq!(map.len(), 3);
    assert!(!map.is_empty());

    // 测试删除
    assert_eq!(map.remove(&2), Some("two"));
    assert_eq!(map.get(&2), None);
    assert_eq!(map.len(), 2);

    println!("分片哈希表测试通过");
}

/// 测试缓存感知哈希表功能
#[test]
fn test_cache_aware_hashmap() {
    println!("=== 缓存感知哈希表测试 ===");

    let map = CacheAwareHashMap::new(2);

    // 测试插入
    map.insert(1, "one").unwrap();
    map.insert(2, "two").unwrap();
    map.insert(3, "three").unwrap();

    // 测试获取
    assert_eq!(map.get(&1), Some("one"));
    assert_eq!(map.get(&2), Some("two"));
    assert_eq!(map.get(&3), Some("three"));

    // 测试热点键
    let hot_keys = map.get_hot_keys();
    println!("热点键: {:?}", hot_keys);

    // 测试删除
    assert_eq!(map.remove(&2), Some("two"));
    assert_eq!(map.get(&2), None);

    println!("缓存感知哈希表测试通过");
}

/// 测试带统计信息的哈希表功能
#[test]
fn test_instrumented_hashmap() {
    println!("=== 带统计信息的哈希表测试 ===");

    let map = InstrumentedLockFreeHashMap::new();

    // 执行一些操作
    map.insert(1, "one").unwrap();
    map.insert(2, "two").unwrap();
    map.get(&1);
    map.get(&2);
    map.remove(&1);

    // 检查统计信息
    let stats = map.get_stats();
    assert_eq!(stats.insert_count, 2);
    assert_eq!(stats.get_count, 2);
    assert_eq!(stats.remove_count, 1);

    println!("带统计信息的哈希表测试通过");
}

/// 测试并发性能
#[test]
fn test_concurrent_performance() {
    println!("=== 并发性能测试 ===");

    let map = Arc::new(LockFreeHashMap::new());
    let thread_count = 8;
    let operations_per_thread = 10000;

    let start = Instant::now();

    // 生产者线程
    let mut producer_handles = Vec::new();
    for i in 0..thread_count / 2 {
        let map = map.clone();
        let handle = thread::spawn(move || {
            for j in 0..operations_per_thread {
                map.insert(i * operations_per_thread + j, i * operations_per_thread + j)
                    .unwrap();
            }
        });
        producer_handles.push(handle);
    }

    // 消费者线程
    let mut consumer_handles = Vec::new();
    for i in 0..thread_count / 2 {
        let map = map.clone();
        let handle = thread::spawn(move || {
            for j in 0..operations_per_thread {
                map.get(&(i * operations_per_thread + j));
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

    let map = Arc::new(InstrumentedLockFreeHashMap::new());
    let thread_count = 16;
    let operations_per_thread = 5000;

    let start = Instant::now();

    let mut handles = Vec::new();

    // 混合生产者和消费者线程
    for i in 0..thread_count {
        let map = map.clone();
        let handle = thread::spawn(move || {
            if i % 2 == 0 {
                // 生产者
                for j in 0..operations_per_thread {
                    map.insert(i * operations_per_thread + j, i * operations_per_thread + j)
                        .unwrap();
                }
            } else {
                // 消费者
                for j in 0..operations_per_thread {
                    map.get(&(i * operations_per_thread + j));
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
    let stats = map.get_stats();

    println!("高并发场景测试完成");
    println!("耗时: {:?}", duration);
    println!("插入次数: {}", stats.insert_count);
    println!("查找次数: {}", stats.get_count);
    println!("删除次数: {}", stats.remove_count);

    // 验证统计信息
    assert_eq!(
        stats.insert_count,
        (thread_count / 2 * operations_per_thread) as usize
    );
    assert_eq!(
        stats.get_count,
        (thread_count / 2 * operations_per_thread) as usize
    );
}

/// 测试哈希表的内存安全性
#[test]
fn test_memory_safety() {
    println!("=== 内存安全性测试 ===");

    let map = Arc::new(LockFreeHashMap::new());
    let thread_count = 8;
    let operations_per_thread = 1000;

    let mut handles = Vec::new();

    // 创建多个线程同时操作哈希表
    for i in 0..thread_count {
        let map = map.clone();
        let handle = thread::spawn(move || {
            for j in 0..operations_per_thread {
                if j % 2 == 0 {
                    // 偶数操作：插入
                    map.insert(i * operations_per_thread + j, i * operations_per_thread + j)
                        .unwrap();
                } else {
                    // 奇数操作：尝试删除
                    map.remove(&(i * operations_per_thread + j));
                }
            }
        });
        handles.push(handle);
    }

    // 等待所有线程完成
    for handle in handles {
        handle.join().unwrap();
    }

    // 清空哈希表
    map.clear();

    assert!(map.is_empty());

    println!("内存安全性测试通过");
}

/// 测试哈希表的异常处理
#[test]
fn test_error_handling() {
    println!("=== 异常处理测试 ===");

    let map: LockFreeHashMap<i32, i32> = LockFreeHashMap::new();

    // 测试空表删除
    assert!(map.remove(&1).is_none());

    // 测试获取不存在的键
    assert!(map.get(&1).is_none());

    // 测试包含不存在的键
    assert!(!map.contains_key(&1));

    println!("异常处理测试通过");
}

/// 测试不同哈希表类型的性能比较
#[test]
fn test_hashmap_types_performance() {
    println!("=== 哈希表类型性能比较测试 ===");

    let operations = 10000;

    // 基本无锁哈希表
    let basic_map = LockFreeHashMap::new();
    let start = Instant::now();
    for i in 0..operations {
        basic_map.insert(i, i).unwrap();
        basic_map.get(&i);
    }
    let basic_duration = start.elapsed();

    // 分片哈希表
    let striped_map = StripedHashMap::with_shards(4);
    let start = Instant::now();
    for i in 0..operations {
        striped_map.insert(i, i).unwrap();
        striped_map.get(&i);
    }
    let striped_duration = start.elapsed();

    // 缓存感知哈希表
    let cache_map = CacheAwareHashMap::new(100);
    let start = Instant::now();
    for i in 0..operations {
        cache_map.insert(i, i).unwrap();
        cache_map.get(&i);
    }
    let cache_duration = start.elapsed();

    // 带统计信息的哈希表
    let instrumented_map = InstrumentedLockFreeHashMap::new();
    let start = Instant::now();
    for i in 0..operations {
        instrumented_map.insert(i, i).unwrap();
        instrumented_map.get(&i);
    }
    let instrumented_duration = start.elapsed();

    println!("基本无锁哈希表: {:?}", basic_duration);
    println!("分片哈希表: {:?}", striped_duration);
    println!("缓存感知哈希表: {:?}", cache_duration);
    println!("带统计信息的哈希表: {:?}", instrumented_duration);

    // 性能比较
    let basic_ops = operations as f64 * 2.0 / basic_duration.as_secs_f64();
    let striped_ops = operations as f64 * 2.0 / striped_duration.as_secs_f64();
    let cache_ops = operations as f64 * 2.0 / cache_duration.as_secs_f64();
    let instrumented_ops = operations as f64 * 2.0 / instrumented_duration.as_secs_f64();

    println!("基本无锁哈希表每秒操作数: {:.0}", basic_ops);
    println!("分片哈希表每秒操作数: {:.0}", striped_ops);
    println!("缓存感知哈希表每秒操作数: {:.0}", cache_ops);
    println!("带统计信息的哈希表每秒操作数: {:.0}", instrumented_ops);

    // 性能断言
    assert!(
        basic_ops > 100000.0,
        "基本无锁哈希表性能应该超过10万操作/秒"
    );
    assert!(striped_ops > 100000.0, "分片哈希表性能应该超过10万操作/秒");
}

/// 测试哈希表的扩容行为
#[test]
fn test_hashmap_resize() {
    println!("=== 哈希表扩容测试 ===");

    let map = LockFreeHashMap::with_capacity(4);

    // 插入大量元素触发扩容
    for i in 0..100 {
        map.insert(i, i).unwrap();
    }

    // 验证所有元素都存在
    for i in 0..100 {
        assert_eq!(map.get(&i), Some(i));
    }

    // 验证大小
    assert_eq!(map.len(), 100);

    // 验证桶数量增加
    assert!(map.bucket_count() > 4);

    println!("哈希表扩容测试通过");
}
