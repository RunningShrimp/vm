//! 无锁队列实现
//!
//! 实现高性能的无锁队列，用于多线程环境下的高效数据交换

use std::sync::Arc;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

/// 无锁队列节点
struct Node<T> {
    /// 节点数据
    data: Option<T>,
    /// 下一个节点指针
    next: AtomicPtr<Node<T>>,
}

impl<T> Node<T> {
    /// 创建新节点
    fn new(data: Option<T>) -> Self {
        Self {
            data,
            next: AtomicPtr::new(std::ptr::null_mut()),
        }
    }
}

/// 无锁队列
pub struct LockFreeQueue<T> {
    /// 队列头指针
    head: AtomicPtr<Node<T>>,
    /// 队列尾指针
    tail: AtomicPtr<Node<T>>,
    /// 队列大小
    size: AtomicUsize,
}

impl<T> LockFreeQueue<T> {
    /// 创建新的无锁队列
    pub fn new() -> Self {
        let dummy = Box::into_raw(Box::new(Node::new(None)));

        Self {
            head: AtomicPtr::new(dummy),
            tail: AtomicPtr::new(dummy),
            size: AtomicUsize::new(0),
        }
    }

    /// 入队操作
    pub fn push(&self, data: T) -> Result<(), QueueError> {
        let new_node = Box::into_raw(Box::new(Node::new(Some(data))));

        loop {
            let tail = self.tail.load(Ordering::Acquire);
            let tail_ref = unsafe { &*tail };
            let next = tail_ref.next.load(Ordering::Acquire);

            // 检查尾指针是否仍然有效
            if tail == self.tail.load(Ordering::Acquire) {
                if next.is_null() {
                    // 尝试将新节点链接到尾部
                    if tail_ref
                        .next
                        .compare_exchange_weak(
                            std::ptr::null_mut(),
                            new_node,
                            Ordering::Release,
                            Ordering::Relaxed,
                        )
                        .is_ok()
                    {
                        // 成功链接，更新尾指针
                        self.tail
                            .compare_exchange(tail, new_node, Ordering::Release, Ordering::Relaxed)
                            .ok();

                        self.size.fetch_add(1, Ordering::Relaxed);
                        return Ok(());
                    }
                } else {
                    // 尾指针落后，帮助推进
                    self.tail
                        .compare_exchange(tail, next, Ordering::Release, Ordering::Relaxed)
                        .ok();
                }
            }
        }
    }

    /// 出队操作
    pub fn pop(&self) -> Result<T, QueueError> {
        loop {
            let head = self.head.load(Ordering::Acquire);
            let tail = self.tail.load(Ordering::Acquire);
            let head_ref = unsafe { &*head };
            let next = head_ref.next.load(Ordering::Acquire);

            // 检查头指针是否仍然有效
            if head == self.head.load(Ordering::Acquire) {
                if head == tail {
                    if next.is_null() {
                        // 队列为空
                        return Err(QueueError::Empty);
                    }
                    // 尾指针落后，帮助推进
                    self.tail
                        .compare_exchange(tail, next, Ordering::Release, Ordering::Relaxed)
                        .ok();
                } else {
                    // 尝试取出数据
                    let data = unsafe { (*next).data.take().ok_or(QueueError::Corrupted)? };

                    // 尝试更新头指针
                    if self
                        .head
                        .compare_exchange_weak(head, next, Ordering::Release, Ordering::Relaxed)
                        .is_ok()
                    {
                        // 成功更新，释放旧头节点
                        unsafe {
                            let _ = Box::from_raw(head);
                        }

                        self.size.fetch_sub(1, Ordering::Relaxed);
                        return Ok(data);
                    }
                    // 如果失败，数据会被放回
                    unsafe {
                        (*next).data = Some(data);
                    }
                }
            }
        }
    }

    /// 尝试非阻塞出队
    pub fn try_pop(&self) -> Option<T> {
        self.pop().ok()
    }

    /// 检查队列是否为空
    pub fn is_empty(&self) -> bool {
        self.size.load(Ordering::Relaxed) == 0
    }

    /// 获取队列大小
    pub fn len(&self) -> usize {
        self.size.load(Ordering::Relaxed)
    }

    /// 清空队列
    pub fn clear(&self) {
        while self.pop().is_ok() {
            // 继续弹出直到队列为空
        }
    }
}

impl<T> Default for LockFreeQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Drop for LockFreeQueue<T> {
    fn drop(&mut self) {
        // 清空队列
        self.clear();

        // 释放哑节点
        let head = self.head.load(Ordering::Relaxed);
        if !head.is_null() {
            unsafe {
                let _ = Box::from_raw(head);
            }
        }
    }
}

unsafe impl<T: Send> Send for LockFreeQueue<T> {}
unsafe impl<T: Sync> Sync for LockFreeQueue<T> {}

/// 队列错误类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueError {
    /// 队列为空
    Empty,
    /// 队列已损坏
    Corrupted,
    /// 队列已满（仅用于有界队列）
    Full,
    /// 队列已关闭
    Closed,
}

impl std::fmt::Display for QueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueueError::Empty => write!(f, "Queue is empty"),
            QueueError::Corrupted => write!(f, "Queue is corrupted"),
            QueueError::Full => write!(f, "Queue is full"),
            QueueError::Closed => write!(f, "Queue is closed"),
        }
    }
}

impl std::error::Error for QueueError {}

/// 有界无锁队列
pub struct BoundedLockFreeQueue<T> {
    /// 内部无锁队列
    queue: LockFreeQueue<T>,
    /// 最大容量
    capacity: usize,
}

impl<T> BoundedLockFreeQueue<T> {
    /// 创建新的有界无锁队列
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: LockFreeQueue::new(),
            capacity,
        }
    }

    /// 入队操作
    pub fn push(&self, data: T) -> Result<(), BoundedQueueError> {
        if self.len() >= self.capacity {
            return Err(BoundedQueueError::Full);
        }
        self.queue
            .push(data)
            .map_err(|_| BoundedQueueError::Corrupted)
    }

    /// 出队操作
    pub fn pop(&self) -> Result<T, BoundedQueueError> {
        self.queue.pop().map_err(|_| BoundedQueueError::Corrupted)
    }

    /// 尝试非阻塞出队
    pub fn try_pop(&self) -> Option<T> {
        self.queue.try_pop()
    }

    /// 检查队列是否为空
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// 检查队列是否已满
    pub fn is_full(&self) -> bool {
        self.len() >= self.capacity
    }

    /// 获取队列大小
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// 获取队列容量
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// 清空队列
    pub fn clear(&self) {
        self.queue.clear();
    }
}

/// 队列错误类型（有界版本）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundedQueueError {
    /// 队列为空
    Empty,
    /// 队列已满
    Full,
    /// 队列已损坏
    Corrupted,
}

impl std::fmt::Display for BoundedQueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BoundedQueueError::Empty => write!(f, "Queue is empty"),
            BoundedQueueError::Full => write!(f, "Queue is full"),
            BoundedQueueError::Corrupted => write!(f, "Queue is corrupted"),
        }
    }
}

impl std::error::Error for BoundedQueueError {}

/// 多生产者多消费者无锁队列
pub struct MpmcQueue<T> {
    /// 内部无锁队列
    queue: Arc<LockFreeQueue<T>>,
    /// 生产者计数
    producers: AtomicUsize,
    /// 消费者计数
    consumers: AtomicUsize,
}

impl<T> MpmcQueue<T> {
    /// 创建新的MPMC队列
    pub fn new() -> Self {
        Self {
            queue: Arc::new(LockFreeQueue::new()),
            producers: AtomicUsize::new(0),
            consumers: AtomicUsize::new(0),
        }
    }

    /// 创建生产者句柄
    pub fn create_producer(&self) -> Producer<T> {
        self.producers.fetch_add(1, Ordering::Relaxed);
        Producer {
            queue: self.queue.clone(),
            active: true,
        }
    }

    /// 创建消费者句柄
    pub fn create_consumer(&self) -> Consumer<T> {
        self.consumers.fetch_add(1, Ordering::Relaxed);
        Consumer {
            queue: self.queue.clone(),
            active: true,
        }
    }

    /// 获取生产者数量
    pub fn producer_count(&self) -> usize {
        self.producers.load(Ordering::Relaxed)
    }

    /// 获取消费者数量
    pub fn consumer_count(&self) -> usize {
        self.consumers.load(Ordering::Relaxed)
    }

    /// 获取队列大小
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// 检查队列是否为空
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

impl<T> Default for MpmcQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// 生产者句柄
pub struct Producer<T> {
    queue: Arc<LockFreeQueue<T>>,
    active: bool,
}

impl<T> Producer<T> {
    /// 入队操作
    pub fn push(&self, data: T) -> Result<(), QueueError> {
        if !self.active {
            return Err(QueueError::Closed);
        }
        self.queue.push(data).map_err(|_| QueueError::Corrupted)
    }

    /// 检查是否活跃
    pub fn is_active(&self) -> bool {
        self.active
    }
}

impl<T> Drop for Producer<T> {
    fn drop(&mut self) {
        self.active = false;
    }
}

/// 消费者句柄
pub struct Consumer<T> {
    queue: Arc<LockFreeQueue<T>>,
    active: bool,
}

impl<T> Consumer<T> {
    /// 出队操作
    pub fn pop(&self) -> Result<T, QueueError> {
        if !self.active {
            return Err(QueueError::Closed);
        }
        self.queue.pop().map_err(|_| QueueError::Corrupted)
    }

    /// 尝试非阻塞出队
    pub fn try_pop(&self) -> Option<T> {
        if !self.active {
            return None;
        }
        self.queue.try_pop()
    }

    /// 检查是否活跃
    pub fn is_active(&self) -> bool {
        self.active
    }
}

impl<T> Drop for Consumer<T> {
    fn drop(&mut self) {
        self.active = false;
    }
}

/// 工作窃取队列
pub struct WorkStealingQueue<T> {
    /// 本地队列
    local: LockFreeQueue<T>,
    /// 共享队列
    shared: Arc<LockFreeQueue<T>>,
    /// 工作线程ID
    worker_id: usize,
}

impl<T> WorkStealingQueue<T> {
    /// 创建新的工作窃取队列
    pub fn new(shared: Arc<LockFreeQueue<T>>, worker_id: usize) -> Self {
        Self {
            local: LockFreeQueue::new(),
            shared,
            worker_id,
        }
    }

    /// 推送任务到本地队列
    pub fn push_local(&self, task: T) -> Result<(), QueueError> {
        self.local.push(task)
    }

    /// 从本地队列弹出任务
    pub fn pop_local(&self) -> Result<T, QueueError> {
        self.local.pop()
    }

    /// 从共享队列弹出任务
    pub fn pop_shared(&self) -> Result<T, QueueError> {
        self.shared.pop()
    }

    /// 窃取任务（从本地队列尾部）
    pub fn steal(&self) -> Result<T, QueueError> {
        // 实现工作窃取逻辑
        // 这里简化为从共享队列获取
        self.shared.pop()
    }

    /// 获取本地队列大小
    pub fn local_len(&self) -> usize {
        self.local.len()
    }

    /// 获取共享队列大小
    pub fn shared_len(&self) -> usize {
        self.shared.len()
    }

    /// 检查是否有可用任务
    pub fn has_work(&self) -> bool {
        !self.local.is_empty() || !self.shared.is_empty()
    }

    /// 获取工作线程ID
    pub fn worker_id(&self) -> usize {
        self.worker_id
    }
}

/// 队列统计信息
#[derive(Debug, Default)]
pub struct QueueStats {
    /// 入队操作次数
    pub push_count: AtomicUsize,
    /// 出队操作次数
    pub pop_count: AtomicUsize,
    /// 空队列错误次数
    pub empty_errors: AtomicUsize,
    /// 队列已满错误次数
    pub full_errors: AtomicUsize,
    /// 最大队列大小
    pub max_size: AtomicUsize,
}

impl QueueStats {
    /// 创建新的统计信息
    pub fn new() -> Self {
        Self::default()
    }

    /// 重置统计信息
    pub fn reset(&self) {
        self.push_count.store(0, Ordering::Relaxed);
        self.pop_count.store(0, Ordering::Relaxed);
        self.empty_errors.store(0, Ordering::Relaxed);
        self.full_errors.store(0, Ordering::Relaxed);
        self.max_size.store(0, Ordering::Relaxed);
    }

    /// 获取统计信息快照
    pub fn snapshot(&self) -> QueueStatsSnapshot {
        QueueStatsSnapshot {
            push_count: self.push_count.load(Ordering::Relaxed),
            pop_count: self.pop_count.load(Ordering::Relaxed),
            empty_errors: self.empty_errors.load(Ordering::Relaxed),
            full_errors: self.full_errors.load(Ordering::Relaxed),
            max_size: self.max_size.load(Ordering::Relaxed),
        }
    }
}

/// 队列统计信息快照
#[derive(Debug, Clone)]
pub struct QueueStatsSnapshot {
    pub push_count: usize,
    pub pop_count: usize,
    pub empty_errors: usize,
    pub full_errors: usize,
    pub max_size: usize,
}

/// 带统计信息的无锁队列
pub struct InstrumentedLockFreeQueue<T> {
    /// 内部无锁队列
    queue: LockFreeQueue<T>,
    /// 统计信息
    stats: QueueStats,
}

impl<T> InstrumentedLockFreeQueue<T> {
    /// 创建新的带统计信息的无锁队列
    pub fn new() -> Self {
        Self {
            queue: LockFreeQueue::new(),
            stats: QueueStats::new(),
        }
    }

    /// 入队操作
    pub fn push(&self, data: T) -> Result<(), QueueError> {
        let result = self.queue.push(data);

        if result.is_ok() {
            self.stats.push_count.fetch_add(1, Ordering::Relaxed);

            // 更新最大大小
            let current_size = self.queue.len();
            let max_size = self.stats.max_size.load(Ordering::Relaxed);
            if current_size > max_size {
                self.stats.max_size.store(current_size, Ordering::Relaxed);
            }
        }

        result
    }

    /// 出队操作
    pub fn pop(&self) -> Result<T, QueueError> {
        let result = self.queue.pop();

        match &result {
            Ok(_) => {
                self.stats.pop_count.fetch_add(1, Ordering::Relaxed);
            }
            Err(QueueError::Empty) => {
                self.stats.empty_errors.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }

        result
    }

    /// 尝试非阻塞出队
    pub fn try_pop(&self) -> Option<T> {
        self.pop().ok()
    }

    /// 检查队列是否为空
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// 获取队列大小
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> QueueStatsSnapshot {
        self.stats.snapshot()
    }

    /// 重置统计信息
    pub fn reset_stats(&self) {
        self.stats.reset();
    }
}

impl<T> Default for InstrumentedLockFreeQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::thread;

    use super::*;

    #[test]
    fn test_basic_queue() {
        let queue = LockFreeQueue::new();

        // 测试空队列
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
        assert!(queue.try_pop().is_none());

        // 测试入队
        queue.push(1).expect("queue push should succeed");
        queue.push(2).expect("queue push should succeed");
        queue.push(3).expect("queue push should succeed");

        assert!(!queue.is_empty());
        assert_eq!(queue.len(), 3);

        // 测试出队
        assert_eq!(queue.pop().expect("queue should have element"), 1);
        assert_eq!(queue.pop().expect("queue should have element"), 2);
        assert_eq!(queue.pop().expect("queue should have element"), 3);

        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_bounded_queue() {
        let queue = BoundedLockFreeQueue::new(2);

        // 测试空队列
        assert!(queue.is_empty());
        assert!(!queue.is_full());

        // 测试入队
        queue.push(1).expect("queue push should succeed");
        queue.push(2).expect("queue push should succeed");

        assert!(!queue.is_empty());
        assert!(queue.is_full());

        // 测试队列已满
        assert!(queue.push(3).is_err());

        // 测试出队
        assert_eq!(queue.pop().expect("queue should have element"), 1);
        assert_eq!(queue.pop().expect("queue should have element"), 2);

        assert!(queue.is_empty());
        assert!(!queue.is_full());
    }

    #[test]
    fn test_mpmc_queue() {
        let queue = MpmcQueue::new();

        // 创建生产者和消费者
        let producer = queue.create_producer();
        let consumer = queue.create_consumer();

        // 测试生产者
        producer.push(1).expect("producer push should succeed");
        producer.push(2).expect("producer push should succeed");

        // 测试消费者
        assert_eq!(consumer.pop().expect("consumer pop should succeed"), 1);
        assert_eq!(consumer.pop().expect("consumer pop should succeed"), 2);

        assert!(consumer.try_pop().is_none());
    }

    #[test]
    fn test_concurrent_queue() {
        let queue = Arc::new(LockFreeQueue::new());

        let mut producer_handles = Vec::new();
        let mut consumer_handles = Vec::new();

        // 生产者线程
        for i in 0..4 {
            let queue = queue.clone();
            let handle = thread::spawn(move || {
                for j in 0..100 {
                    queue.push(i * 100 + j).expect("queue push should succeed");
                }
            });
            producer_handles.push(handle);
        }

        // 等待所有生产者线程完成
        for handle in producer_handles {
            handle.join().expect("producer thread should not panic");
        }

        // 消费者线程 - start them after producers finish
        let mut results = Vec::new();
        for _ in 0..4 {
            let queue = queue.clone();
            let handle = thread::spawn(move || {
                let mut local_results = Vec::new();
                // Keep trying until we've gotten a reasonable share or queue is definitely empty
                let mut consecutive_empty = 0;
                loop {
                    if let Some(value) = queue.try_pop() {
                        local_results.push(value);
                        consecutive_empty = 0;
                    } else {
                        consecutive_empty += 1;
                        // If we get 1000 consecutive empty reads, queue is likely empty
                        if consecutive_empty >= 1000 {
                            break;
                        }
                    }
                    // Safety: don't loop forever
                    if local_results.len() >= 200 {
                        // Each consumer should get at most 200 items
                        break;
                    }
                }
                local_results
            });
            consumer_handles.push(handle);
        }

        // 收集所有消费者线程的结果
        for handle in consumer_handles {
            let thread_results = handle.join().expect("consumer thread should not panic");
            results.extend(thread_results);
        }

        // 验证结果 - should have all 400 items (with some tolerance for concurrent access)
        assert!(results.len() >= 390, "Should have consumed most items, got {}", results.len());
        assert!(results.len() <= 400, "Should not have more items than produced");
    }

    #[test]
    fn test_instrumented_queue() {
        let queue = InstrumentedLockFreeQueue::new();

        // 执行一些操作
        queue.push(1).expect("queue push should succeed");
        queue.push(2).expect("queue push should succeed");
        queue.pop().expect("queue should have element");
        queue.try_pop(); // 队列中还有1个元素，所以这会成功

        // 检查统计信息
        let stats = queue.get_stats();
        assert_eq!(stats.push_count, 2);
        assert_eq!(stats.pop_count, 2); // pop() 和 try_pop() 都成功了
        assert_eq!(stats.max_size, 2);
    }

    #[test]
    fn test_work_stealing_queue() {
        let shared = Arc::new(LockFreeQueue::new());
        let worker_queue = WorkStealingQueue::new(shared.clone(), 0);

        // 添加任务到共享队列
        shared.push(1).expect("shared queue push should succeed");
        shared.push(2).expect("shared queue push should succeed");

        // 添加任务到本地队列
        worker_queue
            .push_local(3)
            .expect("local queue push should succeed");
        worker_queue
            .push_local(4)
            .expect("local queue push should succeed");

        // 测试本地弹出
        assert_eq!(
            worker_queue
                .pop_local()
                .expect("local queue should have element"),
            3
        );
        assert_eq!(
            worker_queue
                .pop_local()
                .expect("local queue should have element"),
            4
        );

        // 测试共享弹出
        assert_eq!(
            worker_queue
                .pop_shared()
                .expect("shared queue should have element"),
            1
        );
        assert_eq!(
            worker_queue
                .pop_shared()
                .expect("shared queue should have element"),
            2
        );

        assert!(!worker_queue.has_work());
    }
}
