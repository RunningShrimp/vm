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

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// 无锁FIFO队列
///
/// 基于环形缓冲区的无锁队列实现，支持多生产者多消费者场景。
///
/// # 特性
/// - O(1) 入队和出队
/// - 无全局锁
/// - 预分配容量
///
/// 标识: 数据模型
pub struct LockFreeQueue<T> {
    buffer: Box<[UnsafeCell<Option<T>>]>,
    head: AtomicUsize,
    tail: AtomicUsize,
    capacity: usize,
}

impl<T> LockFreeQueue<T> {
    /// 创建指定容量的无锁队列
    ///
    /// # 参数
    /// - `capacity`: 队列容量（应为 2 的幂次）
    ///
    /// # 返回
    /// 新的无锁队列实例
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.next_power_of_two();
        let mut buffer = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            buffer.push(UnsafeCell::new(None));
        }

        Self {
            buffer: buffer.into_boxed_slice(),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            capacity,
        }
    }

    /// 入队操作
    ///
    /// # 返回
    /// - `Ok(())` 如果入队成功
    /// - `Err(item)` 如果队列满
    pub fn enqueue(&self, item: T) -> Result<(), T> {
        loop {
            let tail = self.tail.load(Ordering::Relaxed);
            let next_tail = (tail + 1) % self.capacity;
            let head = self.head.load(Ordering::Acquire);

            if next_tail == head {
                return Err(item);
            }

            // 尝试CAS操作更新tail
            match self
                .tail
                .compare_exchange(tail, next_tail, Ordering::Release, Ordering::Relaxed)
            {
                Ok(_) => {
                    // 安全地写入buffer（因为我们已经抢占了该位置）
                    unsafe {
                        *self.buffer[tail].get() = Some(item);
                    }
                    return Ok(());
                }
                Err(_) => {
                    // CAS失败，重试
                    continue;
                }
            }
        }
    }

    /// 出队操作
    ///
    /// # 返回
    /// - `Some(item)` 如果队列非空
    /// - `None` 如果队列为空
    pub fn dequeue(&self) -> Option<T> {
        loop {
            let head = self.head.load(Ordering::Relaxed);
            let tail = self.tail.load(Ordering::Acquire);

            if head == tail {
                return None;
            }

            let next_head = (head + 1) % self.capacity;

            // 尝试CAS操作更新head
            match self
                .head
                .compare_exchange(head, next_head, Ordering::Release, Ordering::Relaxed)
            {
                Ok(_) => {
                    // 安全地读取buffer
                    unsafe {
                        return (*self.buffer[head].get()).take();
                    }
                }
                Err(_) => {
                    // CAS失败，重试
                    continue;
                }
            }
        }
    }

    /// 检查队列是否为空
    pub fn is_empty(&self) -> bool {
        self.head.load(Ordering::Acquire) == self.tail.load(Ordering::Acquire)
    }

    /// 获取队列当前元素数量（近似值）
    pub fn len(&self) -> usize {
        let tail = self.tail.load(Ordering::Acquire);
        let head = self.head.load(Ordering::Acquire);
        (tail + self.capacity - head) % self.capacity
    }
}

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

/// 无锁栈
///
/// 用于内存池和对象池的无锁栈实现。
///
/// 标识: 数据模型
pub struct LockFreeStack<T> {
    data: Vec<T>,
    top: AtomicUsize,
    capacity: usize,
}

impl<T> LockFreeStack<T> {
    /// 创建指定容量的无锁栈
    pub fn new(capacity: usize) -> Self {
        let data = Vec::with_capacity(capacity);
        Self {
            data,
            top: AtomicUsize::new(0),
            capacity,
        }
    }

    /// 压栈操作
    ///
    /// # 返回
    /// - `Ok(())` 如果压栈成功
    /// - `Err(item)` 如果栈满
    pub fn push(&mut self, item: T) -> Result<(), T> {
        let top = self.top.load(Ordering::Relaxed);
        if top >= self.capacity {
            return Err(item);
        }

        // 注意: 这个实现假设只有单个线程修改，或需要更复杂的同步
        if top < self.data.len() {
            unsafe {
                let ptr = self.data.as_mut_ptr().add(top);
                *ptr = item;
            }
        } else if top == self.data.len() {
            self.data.push(item);
        }

        self.top.store(top + 1, Ordering::Release);
        Ok(())
    }

    /// 出栈操作
    pub fn pop(&mut self) -> Option<T> {
        let mut top = self.top.load(Ordering::Acquire);

        loop {
            if top == 0 {
                return None;
            }

            let new_top = top - 1;
            match self
                .top
                .compare_exchange(top, new_top, Ordering::Release, Ordering::Acquire)
            {
                Ok(_) => {
                    return unsafe {
                        let ptr = self.data.as_mut_ptr().add(new_top);
                        Some(std::ptr::read(ptr))
                    };
                }
                Err(actual) => {
                    top = actual;
                }
            }
        }
    }

    /// 检查栈是否为空
    pub fn is_empty(&self) -> bool {
        self.top.load(Ordering::Acquire) == 0
    }

    /// 获取栈的当前大小
    pub fn len(&self) -> usize {
        self.top.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_lockfree_queue_basic() {
        let queue: Arc<LockFreeQueue<i32>> = Arc::new(LockFreeQueue::new(8));

        queue.enqueue(1).unwrap();
        queue.enqueue(2).unwrap();
        queue.enqueue(3).unwrap();

        assert_eq!(queue.dequeue(), Some(1));
        assert_eq!(queue.dequeue(), Some(2));
        assert_eq!(queue.dequeue(), Some(3));
        assert_eq!(queue.dequeue(), None);
    }

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
}
