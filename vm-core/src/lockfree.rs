//! 无锁数据结构实现
//!
//! 提供高性能的无锁数据结构，用于多线程/多vCPU场景中的无竞争访问。
//!
//! # 标识: 服务接口
//!
//! 支持的无锁数据结构:
//! - `LockFreeQueue`: 无锁FIFO队列，适用于事件分发
//! - `LockFreeCounter`: 无锁计数器，用于统计
//! - `LockFreeStack`: 无锁栈，用于内存池
//! - `LockFreeHashMap`: 无锁哈希表，用于键值存储
//! - `LockFreeSharedState`: 无锁共享状态管理

// 重新导出vm-common中的无锁数据结构
pub use vm_common::lockfree::*;

use std::sync::atomic::{AtomicU64, Ordering};

/// 无锁计数器
///
/// 高效的无锁计数器，用于并发统计场景。
///
/// 标识: 数据模型
pub struct LockFreeCounter {
    value: AtomicU64,
}

impl LockFreeCounter {
    /// 创建新的无锁计数器
    pub fn new(initial: u64) -> Self {
        Self {
            value: AtomicU64::new(initial),
        }
    }

    /// 原子地递增计数器
    ///
    /// # 返回
    /// 递增前的值
    pub fn increment(&self) -> u64 {
        self.value.fetch_add(1, Ordering::SeqCst)
    }

    /// 原子地减少计数器
    ///
    /// # 返回
    /// 递减前的值
    pub fn decrement(&self) -> u64 {
        self.value.fetch_sub(1, Ordering::SeqCst)
    }

    /// 原子地加上指定值
    pub fn add(&self, delta: u64) -> u64 {
        self.value.fetch_add(delta, Ordering::SeqCst)
    }

    /// 获取当前值
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::SeqCst)
    }

    /// 设置值
    pub fn set(&self, new_value: u64) {
        self.value.store(new_value, Ordering::SeqCst);
    }

    /// 重置为零
    pub fn reset(&self) {
        self.set(0);
    }
}

impl Default for LockFreeCounter {
    fn default() -> Self {
        Self::new(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_lockfree_counter() {
        let counter = Arc::new(LockFreeCounter::new(0));
        let mut handles = vec![];

        for _ in 0..10 {
            let c = Arc::clone(&counter);
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    c.increment();
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(counter.get(), 1000);
    }

    #[test]
    fn test_lockfree_counter_operations() {
        let counter = LockFreeCounter::new(100);

        assert_eq!(counter.get(), 100);
        assert_eq!(counter.increment(), 100);
        assert_eq!(counter.get(), 101);
        assert_eq!(counter.add(50), 101);
        assert_eq!(counter.get(), 151);
        counter.reset();
        assert_eq!(counter.get(), 0);
    }

    #[test]
    fn test_lockfree_queue_basic() {
        let queue: LockFreeQueue<i32> = LockFreeQueue::new();

        queue.push(1).unwrap();
        queue.push(2).unwrap();
        queue.push(3).unwrap();

        assert_eq!(queue.pop().unwrap(), 1);
        assert_eq!(queue.pop().unwrap(), 2);
        assert_eq!(queue.pop().unwrap(), 3);
        assert!(queue.try_pop().is_none());
    }

    #[test]
    fn test_lockfree_hashmap_basic() {
        let map = LockFreeHashMap::new();

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
    }

    #[test]
    fn test_lockfree_shared_state() {
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
    }
}