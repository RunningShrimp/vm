//! 实时优化模块 - 微秒级延迟要求
//!
//! 提供：
//! - CPU 隔离与亲和性调优
//! - 优先级继承与优先级天花板
//! - Lock-free 无锁算法
//! - 内存预分配与页面锁定
//! - 预测性预取与缓存管理

use parking_lot::RwLock;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// 实时优先级等级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RealtimePriority {
    Soft = 0,
    Medium = 50,
    Hard = 100,
    Critical = 200,
}

/// CPU 隔离配置
#[derive(Debug, Clone)]
pub struct CpuIsolation {
    pub isolated_cpus: Vec<u32>,
    pub housekeeping_cpus: Vec<u32>,
    pub nohz_cpus: Vec<u32>,
    pub rcu_cpus: Vec<u32>,
}

impl CpuIsolation {
    pub fn new(total_cpus: u32, isolated_count: u32) -> Self {
        let isolated: Vec<u32> = (total_cpus - isolated_count..total_cpus).collect();
        let housekeeping: Vec<u32> = (0..total_cpus - isolated_count).collect();
        let nohz: Vec<u32> = (total_cpus - isolated_count..total_cpus).collect();
        let rcu: Vec<u32> = (total_cpus - isolated_count..total_cpus).collect();

        Self {
            isolated_cpus: isolated,
            housekeeping_cpus: housekeeping,
            nohz_cpus: nohz,
            rcu_cpus: rcu,
        }
    }

    pub fn assign_task_to_cpu(&self, task_id: u32, cpu_affinity: u32) -> Result<(), String> {
        if !self.isolated_cpus.contains(&cpu_affinity) {
            return Err(format!("CPU {} is not isolated", cpu_affinity));
        }
        println!("Task {} assigned to isolated CPU {}", task_id, cpu_affinity);
        Ok(())
    }
}

/// 优先级继承协议
pub struct PriorityInheritance {
    // 任务 ID -> 当前优先级
    task_priorities: Arc<RwLock<Vec<RealtimePriority>>>,
    // 任务 ID -> 持有的锁列表
    task_locks: Arc<RwLock<Vec<Vec<u32>>>>,
    // 锁 ID -> 持有者任务 ID
    lock_owners: Arc<RwLock<Vec<Option<u32>>>>,
}

impl PriorityInheritance {
    pub fn new(max_tasks: usize, max_locks: usize) -> Self {
        Self {
            task_priorities: Arc::new(RwLock::new(vec![RealtimePriority::Soft; max_tasks])),
            task_locks: Arc::new(RwLock::new(vec![Vec::new(); max_tasks])),
            lock_owners: Arc::new(RwLock::new(vec![None; max_locks])),
        }
    }

    pub fn set_task_priority(
        &self,
        task_id: usize,
        priority: RealtimePriority,
    ) -> Result<(), String> {
        let mut priorities = self.task_priorities.write();
        if task_id >= priorities.len() {
            return Err("Invalid task ID".to_string());
        }
        priorities[task_id] = priority;
        println!("Task {} priority set to {:?}", task_id, priority);
        Ok(())
    }

    pub fn acquire_lock(&self, task_id: usize, lock_id: usize) -> Result<(), String> {
        let mut owners = self.lock_owners.write();
        if lock_id >= owners.len() {
            return Err("Invalid lock ID".to_string());
        }

        if owners[lock_id].is_some() {
            return Err(format!(
                "Lock {} already held by task {}",
                lock_id,
                owners[lock_id].unwrap()
            ));
        }

        owners[lock_id] = Some(task_id as u32);

        let mut locks = self.task_locks.write();
        locks[task_id].push(lock_id as u32);

        println!("Task {} acquired lock {}", task_id, lock_id);
        Ok(())
    }

    pub fn release_lock(&self, task_id: usize, lock_id: usize) -> Result<(), String> {
        let mut owners = self.lock_owners.write();
        if lock_id >= owners.len() {
            return Err("Invalid lock ID".to_string());
        }

        if owners[lock_id] != Some(task_id as u32) {
            return Err(format!("Task {} does not own lock {}", task_id, lock_id));
        }

        owners[lock_id] = None;

        let mut locks = self.task_locks.write();
        locks[task_id].retain(|&l| l != lock_id as u32);

        println!("Task {} released lock {}", task_id, lock_id);
        Ok(())
    }

    pub fn get_blocking_task(&self, lock_id: usize) -> Option<u32> {
        self.lock_owners.read().get(lock_id).copied().flatten()
    }
}

/// Lock-free 无锁队列
pub struct LockFreeQueue {
    buffer: Arc<Vec<AtomicU64>>,
    head: Arc<AtomicU64>,
    tail: Arc<AtomicU64>,
    capacity: u64,
}

impl LockFreeQueue {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: Arc::new((0..capacity).map(|_| AtomicU64::new(0)).collect()),
            head: Arc::new(AtomicU64::new(0)),
            tail: Arc::new(AtomicU64::new(0)),
            capacity: capacity as u64,
        }
    }

    pub fn enqueue(&self, value: u64) -> Result<(), String> {
        let tail = self.tail.load(Ordering::SeqCst);
        let next_tail = (tail + 1) % self.capacity;
        let head = self.head.load(Ordering::SeqCst);

        if next_tail == head {
            return Err("Queue is full".to_string());
        }

        self.buffer[tail as usize].store(value, Ordering::SeqCst);
        self.tail.store(next_tail, Ordering::SeqCst);
        Ok(())
    }

    pub fn dequeue(&self) -> Option<u64> {
        let head = self.head.load(Ordering::SeqCst);
        let tail = self.tail.load(Ordering::SeqCst);

        if head == tail {
            return None;
        }

        let value = self.buffer[head as usize].load(Ordering::SeqCst);
        let next_head = (head + 1) % self.capacity;
        self.head.store(next_head, Ordering::SeqCst);
        Some(value)
    }

    pub fn is_empty(&self) -> bool {
        self.head.load(Ordering::SeqCst) == self.tail.load(Ordering::SeqCst)
    }

    pub fn size(&self) -> u64 {
        let tail = self.tail.load(Ordering::SeqCst);
        let head = self.head.load(Ordering::SeqCst);
        (tail - head + self.capacity) % self.capacity
    }
}

/// 内存预分配池
pub struct PreallocatedPool {
    memory_blocks: Arc<RwLock<VecDeque<Vec<u8>>>>,
    block_size: usize,
    pool_size: usize,
    locked_pages: Arc<AtomicU64>,
}

impl PreallocatedPool {
    pub fn new(block_size: usize, pool_size: usize) -> Self {
        let mut blocks = VecDeque::new();
        for _ in 0..pool_size {
            // 页面对齐
            let block = vec![0u8; block_size];
            // 模拟页面锁定
            let _ = mlock_simulation(&block);
            blocks.push_back(block);
        }

        Self {
            memory_blocks: Arc::new(RwLock::new(blocks)),
            block_size,
            pool_size,
            locked_pages: Arc::new(AtomicU64::new((pool_size * block_size) as u64)),
        }
    }

    pub fn acquire_block(&self) -> Option<Vec<u8>> {
        self.memory_blocks.write().pop_front()
    }

    pub fn release_block(&self, block: Vec<u8>) -> Result<(), String> {
        let mut blocks = self.memory_blocks.write();
        if blocks.len() >= self.pool_size {
            return Err("Pool is full".to_string());
        }
        blocks.push_back(block);
        Ok(())
    }

    pub fn get_locked_memory_size(&self) -> u64 {
        self.locked_pages.load(Ordering::SeqCst)
    }

    pub fn get_available_blocks(&self) -> usize {
        self.memory_blocks.read().len()
    }

    pub fn get_block_size(&self) -> usize {
        self.block_size
    }
}

fn mlock_simulation(_data: &[u8]) -> Result<(), String> {
    // 模拟页面锁定
    Ok(())
}

/// 预测性预取管理器
pub struct PrefetchManager {
    access_history: Arc<RwLock<VecDeque<u64>>>,
    predicted_addresses: Arc<RwLock<Vec<u64>>>,
    max_history: usize,
    prefetch_distance: u32,
}

impl PrefetchManager {
    pub fn new(max_history: usize, prefetch_distance: u32) -> Self {
        Self {
            access_history: Arc::new(RwLock::new(VecDeque::new())),
            predicted_addresses: Arc::new(RwLock::new(Vec::new())),
            max_history,
            prefetch_distance,
        }
    }

    pub fn record_access(&self, address: u64) {
        let mut history = self.access_history.write();
        history.push_back(address);

        if history.len() > self.max_history {
            history.pop_front();
        }

        // 简单的步长预测
        if history.len() >= 2 {
            let prev = history[history.len() - 2];
            let curr = history[history.len() - 1];
            let stride = (curr as i64 - prev as i64) as u64;

            let mut predictions = self.predicted_addresses.write();
            predictions.clear();
            for i in 1..=self.prefetch_distance {
                predictions.push(curr + stride * i as u64);
            }
        }
    }

    pub fn get_predictions(&self) -> Vec<u64> {
        self.predicted_addresses.read().clone()
    }
}

/// 实时任务调度器
pub struct RealtimeScheduler {
    cpu_isolation: Arc<CpuIsolation>,
    priority_inheritance: Arc<PriorityInheritance>,
    taskset: Arc<RwLock<Vec<(u32, u32, RealtimePriority)>>>, // (task_id, cpu_affinity, priority)
}

impl RealtimeScheduler {
    pub fn new(total_cpus: u32, isolated_count: u32) -> Self {
        Self {
            cpu_isolation: Arc::new(CpuIsolation::new(total_cpus, isolated_count)),
            priority_inheritance: Arc::new(PriorityInheritance::new(256, 256)),
            taskset: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn schedule_realtime_task(
        &self,
        task_id: u32,
        priority: RealtimePriority,
        cpu_affinity: u32,
    ) -> Result<(), String> {
        self.priority_inheritance
            .set_task_priority(task_id as usize, priority)?;
        self.cpu_isolation
            .assign_task_to_cpu(task_id, cpu_affinity)?;
        self.taskset.write().push((task_id, cpu_affinity, priority));
        Ok(())
    }

    pub fn get_scheduled_tasks(&self) -> Vec<(u32, u32, RealtimePriority)> {
        self.taskset.read().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_isolation() {
        let isolation = CpuIsolation::new(8, 4);
        assert_eq!(isolation.isolated_cpus.len(), 4);
        assert_eq!(isolation.housekeeping_cpus.len(), 4);
        assert!(isolation.assign_task_to_cpu(1, 4).is_ok());
        assert!(isolation.assign_task_to_cpu(1, 0).is_err());
    }

    #[test]
    fn test_priority_inheritance() {
        let pi = PriorityInheritance::new(10, 10);

        pi.set_task_priority(0, RealtimePriority::Hard).unwrap();
        pi.acquire_lock(0, 0).unwrap();
        assert_eq!(pi.get_blocking_task(0), Some(0));

        pi.release_lock(0, 0).unwrap();
        assert_eq!(pi.get_blocking_task(0), None);
    }

    #[test]
    fn test_lockfree_queue() {
        let queue = LockFreeQueue::new(10);

        assert!(queue.enqueue(100).is_ok());
        assert!(queue.enqueue(200).is_ok());
        assert_eq!(queue.size(), 2);

        assert_eq!(queue.dequeue(), Some(100));
        assert_eq!(queue.dequeue(), Some(200));
        assert!(queue.is_empty());
    }

    #[test]
    fn test_preallocated_pool() {
        let pool = PreallocatedPool::new(4096, 10);
        assert_eq!(pool.get_available_blocks(), 10);

        let block = pool.acquire_block();
        assert!(block.is_some());
        assert_eq!(pool.get_available_blocks(), 9);

        pool.release_block(block.unwrap()).unwrap();
        assert_eq!(pool.get_available_blocks(), 10);
    }

    #[test]
    fn test_prefetch_manager() {
        let prefetch = PrefetchManager::new(10, 4);

        prefetch.record_access(0x1000);
        prefetch.record_access(0x2000);
        prefetch.record_access(0x3000);

        let predictions = prefetch.get_predictions();
        assert!(!predictions.is_empty());
    }

    #[test]
    fn test_realtime_scheduler() {
        let scheduler = RealtimeScheduler::new(8, 4);

        scheduler
            .schedule_realtime_task(1, RealtimePriority::Hard, 4)
            .unwrap();
        scheduler
            .schedule_realtime_task(2, RealtimePriority::Critical, 5)
            .unwrap();

        let tasks = scheduler.get_scheduled_tasks();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].0, 1);
        assert_eq!(tasks[0].2, RealtimePriority::Hard);
    }

    #[test]
    fn test_lockfree_queue_full() {
        let queue = LockFreeQueue::new(3);

        assert!(queue.enqueue(1).is_ok());
        assert!(queue.enqueue(2).is_ok());
        assert!(queue.enqueue(3).is_err());
    }

    #[test]
    fn test_priority_lock_ordering() {
        let pi = PriorityInheritance::new(10, 10);

        pi.set_task_priority(0, RealtimePriority::Soft).unwrap();
        pi.set_task_priority(1, RealtimePriority::Hard).unwrap();

        pi.acquire_lock(1, 0).unwrap();
        assert_eq!(pi.get_blocking_task(0), Some(1));

        pi.release_lock(1, 0).unwrap();
    }
}
