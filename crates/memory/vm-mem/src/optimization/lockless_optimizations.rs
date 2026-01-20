//! 无锁并发优化
//!
//! 在关键路径使用原子操作，拆分粗粒度锁，使用 parking_lot::RwLock

use parking_lot::{Mutex, RwLock};
use std::sync::Arc;
use std::sync::atomic::{AtomicPtr, AtomicU64, AtomicUsize, Ordering};

/// 原子计数器
///
/// 使用原子操作实现无锁计数器
pub struct AtomicCounter {
    value: AtomicU64,
}

impl AtomicCounter {
    pub fn new(initial: u64) -> Self {
        Self {
            value: AtomicU64::new(initial),
        }
    }

    pub fn increment(&self) -> u64 {
        self.value.fetch_add(1, Ordering::Relaxed)
    }

    pub fn decrement(&self) -> u64 {
        self.value.fetch_sub(1, Ordering::Relaxed)
    }

    pub fn load(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    pub fn store(&self, value: u64) {
        self.value.store(value, Ordering::Release);
    }

    pub fn compare_and_swap(&self, expected: u64, new: u64) -> u64 {
        self.value
            .compare_exchange(expected, new, Ordering::AcqRel, Ordering::Relaxed)
            .unwrap_or_else(|x| x)
    }
}

/// 细粒度锁管理器
///
/// 将粗粒度锁拆分为多个细粒度锁，减少锁竞争
pub struct FineGrainedLockManager<T> {
    /// 分片数组
    shards: Vec<Arc<RwLock<T>>>,
    /// 分片掩码
    shard_mask: usize,
}

impl<T> FineGrainedLockManager<T> {
    /// 创建新的细粒度锁管理器
    pub fn new(shard_count: usize, factory: impl Fn() -> T) -> Self {
        let mut shards = Vec::with_capacity(shard_count);
        for _ in 0..shard_count {
            shards.push(Arc::new(RwLock::new(factory())));
        }

        Self {
            shards,
            shard_mask: shard_count.next_power_of_two() - 1,
        }
    }

    /// 根据键计算分片索引
    fn shard_index(&self, key: usize) -> usize {
        key & self.shard_mask
    }

    /// 获取读锁
    pub fn read<F, R>(&self, key: usize, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        let shard = &self.shards[self.shard_index(key)];
        let guard = shard.read();
        f(&guard)
    }

    /// 获取写锁
    pub fn write<F, R>(&self, key: usize, f: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        let shard = &self.shards[self.shard_index(key)];
        let mut guard = shard.write();
        f(&mut guard)
    }

    /// 获取所有分片的读锁（用于全局操作）
    pub fn read_all<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&[Arc<RwLock<T>>]) -> R,
    {
        f(&self.shards)
    }
}

/// 无锁队列节点
struct Node<T> {
    value: Option<T>,
    next: AtomicPtr<Node<T>>,
}

/// 无锁队列
///
/// 使用原子操作实现无锁的FIFO队列
pub struct LockFreeQueue<T> {
    head: AtomicPtr<Node<T>>,
    tail: AtomicPtr<Node<T>>,
}

unsafe impl<T: Send> Send for LockFreeQueue<T> {}
unsafe impl<T: Send> Sync for LockFreeQueue<T> {}

impl<T> Default for LockFreeQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> LockFreeQueue<T> {
    pub fn new() -> Self {
        let dummy = Box::into_raw(Box::new(Node {
            value: None,
            next: AtomicPtr::new(std::ptr::null_mut()),
        }));

        Self {
            head: AtomicPtr::new(dummy),
            tail: AtomicPtr::new(dummy),
        }
    }

    /// 入队
    pub fn enqueue(&self, value: T) {
        let node = Box::into_raw(Box::new(Node {
            value: Some(value),
            next: AtomicPtr::new(std::ptr::null_mut()),
        }));

        loop {
            let tail = self.tail.load(Ordering::Acquire);
            let next = unsafe { (*tail).next.load(Ordering::Acquire) };

            if next.is_null() {
                // 尝试链接新节点
                if unsafe {
                    (*tail)
                        .next
                        .compare_exchange(
                            std::ptr::null_mut(),
                            node,
                            Ordering::Release,
                            Ordering::Relaxed,
                        )
                        .is_ok()
                } {
                    // 更新尾指针
                    let _ = self.tail.compare_exchange(
                        tail,
                        node,
                        Ordering::Release,
                        Ordering::Relaxed,
                    );
                    return;
                }
            } else {
                // 帮助其他线程更新尾指针
                let _ =
                    self.tail
                        .compare_exchange(tail, next, Ordering::Release, Ordering::Relaxed);
            }
        }
    }

    /// 出队
    pub fn dequeue(&self) -> Option<T> {
        loop {
            let head = self.head.load(Ordering::Acquire);
            let tail = self.tail.load(Ordering::Acquire);
            let next = unsafe { (*head).next.load(Ordering::Acquire) };

            if head == tail {
                if next.is_null() {
                    return None; // 队列为空
                }
                // 帮助其他线程更新尾指针
                let _ =
                    self.tail
                        .compare_exchange(tail, next, Ordering::Release, Ordering::Relaxed);
            } else {
                if next.is_null() {
                    continue;
                }

                // 尝试更新头指针
                if self
                    .head
                    .compare_exchange(head, next, Ordering::Release, Ordering::Relaxed)
                    .is_ok()
                {
                    unsafe {
                        let value = (*next).value.take();
                        // 释放旧头节点
                        let _ = Box::from_raw(head);
                        return value;
                    }
                }
            }
        }
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        head == tail && unsafe { (*head).next.load(Ordering::Acquire).is_null() }
    }
}

impl<T> Drop for LockFreeQueue<T> {
    fn drop(&mut self) {
        while self.dequeue().is_some() {}
        let head = self.head.load(Ordering::Acquire);
        if !head.is_null() {
            unsafe {
                let _ = Box::from_raw(head);
            }
        }
    }
}

/// 原子引用计数
///
/// 使用原子操作实现引用计数
pub struct AtomicRefCount {
    count: AtomicUsize,
}

impl AtomicRefCount {
    pub fn new(initial: usize) -> Self {
        Self {
            count: AtomicUsize::new(initial),
        }
    }

    pub fn increment(&self) -> usize {
        self.count.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub fn decrement(&self) -> usize {
        let old = self.count.fetch_sub(1, Ordering::AcqRel);
        if old == 1 {
            // 最后一个引用被释放，添加内存屏障
            std::sync::atomic::fence(Ordering::Acquire);
        }
        old - 1
    }

    pub fn load(&self) -> usize {
        self.count.load(Ordering::Acquire)
    }
}

/// 优化的读写锁包装器
///
/// 使用 parking_lot::RwLock 替代 std::sync::RwLock
pub type OptimizedRwLock<T> = RwLock<T>;

/// 优化的互斥锁包装器
///
/// 使用 parking_lot::Mutex 替代 std::sync::Mutex
pub type OptimizedMutex<T> = Mutex<T>;

/// 分片原子计数器
///
/// 将计数器分散到多个分片，减少原子操作的竞争
pub struct ShardedAtomicCounter {
    shards: Vec<AtomicU64>,
    shard_mask: usize,
}

impl ShardedAtomicCounter {
    pub fn new(shard_count: usize) -> Self {
        let mut shards = Vec::with_capacity(shard_count);
        for _ in 0..shard_count {
            shards.push(AtomicU64::new(0));
        }

        Self {
            shards,
            shard_mask: shard_count.next_power_of_two() - 1,
        }
    }

    /// 增加计数（使用线程ID选择分片）
    pub fn increment(&self, thread_id: usize) -> u64 {
        let shard_index = thread_id & self.shard_mask;
        self.shards[shard_index].fetch_add(1, Ordering::Relaxed)
    }

    /// 获取总和
    pub fn sum(&self) -> u64 {
        self.shards.iter().map(|s| s.load(Ordering::Relaxed)).sum()
    }

    /// 重置所有分片
    pub fn reset(&self) {
        for shard in &self.shards {
            shard.store(0, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_counter() {
        let counter = AtomicCounter::new(0);
        assert_eq!(counter.increment(), 0);
        assert_eq!(counter.increment(), 1);
        assert_eq!(counter.load(), 2);
    }

    #[test]
    fn test_lock_free_queue() {
        let queue = LockFreeQueue::new();
        assert!(queue.is_empty());

        queue.enqueue(1);
        queue.enqueue(2);
        queue.enqueue(3);

        assert_eq!(queue.dequeue(), Some(1));
        assert_eq!(queue.dequeue(), Some(2));
        assert_eq!(queue.dequeue(), Some(3));
        assert_eq!(queue.dequeue(), None);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_fine_grained_lock_manager() {
        let manager = FineGrainedLockManager::new(4, || 0u64);

        manager.write(1, |v| *v = 10);
        assert_eq!(manager.read(1, |v| *v), 10);
    }

    #[test]
    fn test_sharded_atomic_counter() {
        let counter = ShardedAtomicCounter::new(4);

        counter.increment(0);
        counter.increment(1);
        counter.increment(2);

        assert_eq!(counter.sum(), 3);
    }
}
