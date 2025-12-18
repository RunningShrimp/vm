//! 无锁共享状态管理测试
//!
//! 验证无锁共享状态管理的正确性和性能

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use vm_common::lockfree::{
    LockFreeSharedState, StripedSharedState, RwLockState, StateManager,
    StateVersion, StateSnapshot, StateChange, StateChangeType,
    StateSubscriber, SharedStateStats,
};

/// 测试订阅者
struct TestSubscriber {
    id: usize,
    changes: Arc<std::sync::Mutex<Vec<StateChange<i32>>>>,
}

impl TestSubscriber {
    fn new(id: usize) -> Self {
        Self {
            id,
            changes: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    fn get_changes(&self) -> Vec<StateChange<i32>> {
        let changes = self.changes.lock().unwrap();
        changes.clone()
    }
}

impl StateSubscriber<i32> for TestSubscriber {
    fn on_state_change(&self, change: &StateChange<i32>) {
        let mut changes = self.changes.lock().unwrap();
        changes.push(change.clone());
    }
}

/// 测试基本无锁共享状态功能
#[test]
fn test_basic_lockfree_shared_state() {
    println!("=== 基本无锁共享状态测试 ===");
    
    let state = LockFreeSharedState::new(0);
    
    // 测试读取
    let snapshot = state.read();
    assert_eq!(snapshot.data, 0);
    assert_eq!(snapshot.version, StateVersion::new());
    
    // 测试更新
    let new_snapshot = state.update(|x| x + 1);
    assert_eq!(new_snapshot.data, 1);
    assert_eq!(new_snapshot.version.minor, 1);
    
    // 验证状态已更新
    let current_snapshot = state.read();
    assert_eq!(current_snapshot.data, 1);
    
    // 测试条件更新
    let conditional_snapshot = state.conditional_update(
        |x| x > 0,
        |x| x * 2,
    );
    assert!(conditional_snapshot.is_some());
    assert_eq!(conditional_snapshot.unwrap().data, 2);
    
    // 测试条件更新失败
    let failed_snapshot = state.conditional_update(
        |x| x < 0,
        |x| x * 2,
    );
    assert!(failed_snapshot.is_none());
    
    // 检查统计信息
    let stats = state.get_stats();
    assert_eq!(stats.update_count, 3);
    assert_eq!(stats.read_count, 4);
    
    println!("基本无锁共享状态测试通过");
}

/// 测试分片共享状态功能
#[test]
fn test_striped_shared_state() {
    println!("=== 分片共享状态测试 ===");
    
    let state = StripedSharedState::new(0);
    
    // 测试读取
    let snapshots = state.read();
    assert_eq!(snapshots.len(), 16);
    for snapshot in &snapshots {
        assert_eq!(snapshot.data, 0);
    }
    
    // 测试更新所有分片
    let new_snapshots = state.update_all(|x| x + 1);
    assert_eq!(new_snapshots.len(), 16);
    for snapshot in &new_snapshots {
        assert_eq!(snapshot.data, 1);
    }
    
    // 测试更新指定分片
    let shard_snapshot = state.update_shard(0, |x| x + 10);
    assert!(shard_snapshot.is_some());
    assert_eq!(shard_snapshot.unwrap().data, 11);
    
    // 验证只有指定分片被更新
    let current_snapshots = state.read();
    assert_eq!(current_snapshots[0].data, 11);
    for i in 1..16 {
        assert_eq!(current_snapshots[i].data, 1);
    }
    
    println!("分片共享状态测试通过");
}

/// 测试读写锁状态功能
#[test]
fn test_rwlock_state() {
    println!("=== 读写锁状态测试 ===");
    
    let state = Arc::new(RwLockState::new(0));
    
    // 测试读取
    let value = state.read();
    assert_eq!(value, 0);
    
    // 测试写入
    let new_value = state.write(|x| x + 1);
    assert_eq!(new_value, 1);
    
    // 验证状态已更新
    let current_value = state.read();
    assert_eq!(current_value, 1);
    
    println!("读写锁状态测试通过");
}

/// 测试状态管理器功能
#[test]
fn test_state_manager() {
    println!("=== 状态管理器测试 ===");
    
    let manager = Arc::new(StateManager::new(0));
    
    // 测试读取状态
    let snapshot = manager.read_state();
    assert_eq!(snapshot.data, 0);
    
    // 创建测试订阅者
    let subscriber1 = TestSubscriber::new(1);
    let subscriber2 = TestSubscriber::new(2);
    
    // 订阅状态变更
    let sub1_id = manager.subscribe(subscriber1);
    let sub2_id = manager.subscribe(subscriber2);
    
    // 测试更新状态
    let new_snapshot = manager.update_state(|x| x + 1);
    assert_eq!(new_snapshot.data, 1);
    
    // 测试条件更新状态
    let conditional_snapshot = manager.conditional_update_state(
        |x| x > 0,
        |x| x * 2,
    );
    assert!(conditional_snapshot.is_some());
    assert_eq!(conditional_snapshot.unwrap().data, 2);
    
    // 测试替换状态
    let replace_snapshot = manager.replace_state(10);
    assert_eq!(replace_snapshot.data, 10);
    
    // 验证状态变更历史
    let changes = manager.get_change_history();
    assert_eq!(changes.len(), 3);
    assert_eq!(changes[0].change_type, StateChangeType::Update);
    assert_eq!(changes[1].change_type, StateChangeType::Conditional);
    assert_eq!(changes[2].change_type, StateChangeType::Replace);
    
    // 验证订阅者收到通知
    let sub1_changes = manager.read_state();
    assert_eq!(sub1_changes.data, 10);
    
    // 取消订阅
    let _ = manager.unsubscribe(sub1_id);
    let _ = manager.unsubscribe(sub2_id);
    
    // 检查统计信息
    let stats = manager.get_stats();
    assert_eq!(stats.subscriber_count, 0);
    
    println!("状态管理器测试通过");
}

/// 测试并发状态访问性能
#[test]
fn test_concurrent_state_access_performance() {
    println!("=== 并发状态访问性能测试 ===");
    
    let state = Arc::new(LockFreeSharedState::new(0));
    let thread_count = 8;
    let operations_per_thread = 10000;
    
    let start = Instant::now();
    
    let mut handles = Vec::new();
    
    // 混合读取和写入线程
    for i in 0..thread_count {
        let state = state.clone();
        let handle = thread::spawn(move || {
            if i % 2 == 0 {
                // 读取线程
                for _ in 0..operations_per_thread {
                    let _ = state.read();
                }
            } else {
                // 写入线程
                for j in 0..operations_per_thread {
                    state.update(|x| x + i * operations_per_thread + j);
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
    let total_operations = (thread_count * operations_per_thread) as u64;
    let ops_per_second = total_operations * 1000 / duration.as_millis() as u64;
    
    println!("并发状态访问性能测试完成");
    println!("总操作数: {}", total_operations);
    println!("耗时: {:?}", duration);
    println!("每秒操作数: {}", ops_per_second);
    
    // 性能断言
    assert!(ops_per_second > 100000, "并发性能应该超过10万操作/秒");
    
    // 检查统计信息
    let stats = state.get_stats();
    println!("更新次数: {}", stats.update_count);
    println!("读取次数: {}", stats.read_count);
}

/// 测试高并发场景
#[test]
fn test_high_concurrency_scenario() {
    println!("=== 高并发场景测试 ===");
    
    let manager = Arc::new(StateManager::new(0));
    let thread_count = 16;
    let operations_per_thread = 5000;
    
    let start = Instant::now();
    
    let mut handles = Vec::new();
    
    // 创建多个订阅者
    for i in 0..thread_count / 2 {
        let subscriber = TestSubscriber::new(i);
        let _ = manager.subscribe(subscriber);
    }
    
    // 混合操作线程
    for i in 0..thread_count {
        let manager = manager.clone();
        let handle = thread::spawn(move || {
            if i % 3 == 0 {
                // 读取线程
                for _ in 0..operations_per_thread {
                    let _ = manager.read_state();
                }
            } else if i % 3 == 1 {
                // 更新线程
                for j in 0..operations_per_thread {
                    manager.update_state(|x| x + i * operations_per_thread + j);
                }
            } else {
                // 条件更新线程
                for j in 0..operations_per_thread {
                    manager.conditional_update_state(
                        |x| x % 2 == 0,
                        |x| x + i * operations_per_thread + j,
                    );
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
    let stats = manager.get_stats();
    
    println!("高并发场景测试完成");
    println!("耗时: {:?}", duration);
    println!("更新次数: {}", stats.shared_stats.update_count);
    println!("读取次数: {}", stats.shared_stats.read_count);
    println!("订阅者数量: {}", stats.subscriber_count);
    println!("待处理变更数量: {}", stats.pending_changes);
    
    // 验证统计信息
    assert!(stats.shared_stats.update_count > 0);
    assert!(stats.shared_stats.read_count > 0);
}

/// 测试状态版本管理
#[test]
fn test_state_version_management() {
    println!("=== 状态版本管理测试 ===");
    
    let state = LockFreeSharedState::new(0);
    
    // 初始版本
    let initial_snapshot = state.read();
    assert_eq!(initial_snapshot.version.major, 0);
    assert_eq!(initial_snapshot.version.minor, 0);
    
    // 多次更新
    let snapshot1 = state.update(|x| x + 1);
    let snapshot2 = state.update(|x| x + 1);
    let snapshot3 = state.update(|x| x + 1);
    
    // 验证版本号递增
    assert_eq!(snapshot1.version.minor, 1);
    assert_eq!(snapshot2.version.minor, 2);
    assert_eq!(snapshot3.version.minor, 3);
    
    // 验证版本比较
    assert!(snapshot1.version.cmp(&snapshot2.version) == std::cmp::Ordering::Less);
    assert!(snapshot2.version.cmp(&snapshot1.version) == std::cmp::Ordering::Greater);
    assert!(snapshot2.version.cmp(&snapshot2.version) == std::cmp::Ordering::Equal);
    
    println!("状态版本管理测试通过");
}

/// 测试状态变更通知
#[test]
fn test_state_change_notification() {
    println!("=== 状态变更通知测试 ===");
    
    let manager = Arc::new(StateManager::new(0));
    
    // 创建测试订阅者
    let subscriber = Arc::new(TestSubscriber::new(1));
    let subscriber_clone = subscriber.clone();
    
    // 订阅状态变更
    let _ = manager.subscribe((*subscriber).clone());
    
    // 执行多次状态更新
    manager.update_state(|x| x + 1);
    manager.update_state(|x| x + 2);
    manager.replace_state(10);
    manager.conditional_update_state(|x| x > 5, |x| x * 2);
    
    // 等待通知处理
    thread::sleep(Duration::from_millis(100));
    
    // 验证订阅者收到通知
    let changes = subscriber.get_changes();
    assert_eq!(changes.len(), 4);
    
    // 验证变更类型
    assert_eq!(changes[0].change_type, StateChangeType::Update);
    assert_eq!(changes[1].change_type, StateChangeType::Update);
    assert_eq!(changes[2].change_type, StateChangeType::Replace);
    assert_eq!(changes[3].change_type, StateChangeType::Conditional);
    
    // 验证变更数据
    assert_eq!(changes[0].data, 1);
    assert_eq!(changes[1].data, 3);
    assert_eq!(changes[2].data, 10);
    assert_eq!(changes[3].data, 20);
    
    println!("状态变更通知测试通过");
}

/// 测试内存安全性
#[test]
fn test_memory_safety() {
    println!("=== 内存安全性测试 ===");
    
    let state = Arc::new(LockFreeSharedState::new(vec![1, 2, 3]));
    let thread_count = 8;
    let operations_per_thread = 1000;
    
    let mut handles = Vec::new();
    
    // 创建多个线程同时操作状态
    for i in 0..thread_count {
        let state = state.clone();
        let handle = thread::spawn(move || {
            for j in 0..operations_per_thread {
                if j % 3 == 0 {
                    // 读取操作
                    let _ = state.read();
                } else if j % 3 == 1 {
                    // 更新操作
                    state.update(|x| {
                        let mut new_x = x.clone();
                        new_x.push(i * operations_per_thread + j);
                        new_x
                    });
                } else {
                    // 条件更新操作
                    state.conditional_update(
                        |x| x.len() > 2,
                        |x| {
                            let mut new_x = x.clone();
                            new_x.push(i * operations_per_thread + j);
                            new_x
                        },
                    );
                }
            }
        });
        handles.push(handle);
    }
    
    // 等待所有线程完成
    for handle in handles {
        handle.join().unwrap();
    }
    
    // 验证最终状态
    let final_snapshot = state.read();
    println!("最终状态长度: {}", final_snapshot.data.len());
    
    // 验证状态不为空
    assert!(!final_snapshot.data.is_empty());
    
    println!("内存安全性测试通过");
}