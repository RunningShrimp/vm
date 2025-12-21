//! 自适应编译队列
//!
//! 根据 CPU 核心数动态调整编译队列大小和优先级

use parking_lot::Mutex;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::Arc;
use std::time::Instant;
use vm_core::GuestAddr;

/// 编译任务优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CompilePriority {
    /// 低优先级（预编译）
    Low = 1,
    /// 中优先级（常规编译）
    Medium = 2,
    /// 高优先级（热点代码）
    High = 3,
    /// 紧急优先级（当前执行路径）
    Urgent = 4,
}

/// 编译任务
#[derive(Debug, Clone)]
pub struct CompileTask {
    /// 代码地址
    pub pc: GuestAddr,
    /// 优先级
    pub priority: CompilePriority,
    /// 执行次数（用于优先级调整）
    pub execution_count: u32,
    /// 创建时间
    pub created_at: Instant,
}

impl PartialEq for CompileTask {
    fn eq(&self, other: &Self) -> bool {
        self.pc == other.pc
    }
}

impl Eq for CompileTask {}

impl PartialOrd for CompileTask {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CompileTask {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // 首先按优先级比较
        match self.priority.cmp(&other.priority) {
            std::cmp::Ordering::Equal => {
                // 优先级相同，按执行次数比较
                match self.execution_count.cmp(&other.execution_count) {
                    std::cmp::Ordering::Equal => {
                        // 执行次数相同，按地址比较（稳定排序）
                        self.pc.cmp(&other.pc)
                    }
                    other => other,
                }
            }
            other => other,
        }
    }
}

impl CompileTask {
    /// 创建新的编译任务
    pub fn new(pc: GuestAddr, priority: CompilePriority) -> Self {
        Self {
            pc,
            priority,
            execution_count: 0,
            created_at: Instant::now(),
        }
    }

    /// 增加执行次数
    pub fn increment_execution(&mut self) {
        self.execution_count += 1;
        // 根据执行次数自动提升优先级
        if self.execution_count > 100 {
            self.priority = CompilePriority::Urgent;
        } else if self.execution_count > 50 {
            self.priority = CompilePriority::High;
        } else if self.execution_count > 10 {
            self.priority = CompilePriority::Medium;
        }
    }
}

/// 自适应编译队列配置
#[derive(Debug, Clone)]
pub struct AdaptiveQueueConfig {
    /// 基础队列大小（每个CPU核心）
    pub base_queue_size_per_core: usize,
    /// 最大队列大小
    pub max_queue_size: usize,
    /// 最小队列大小
    pub min_queue_size: usize,
    /// 优先级提升阈值（执行次数）
    pub priority_threshold: u32,
}

impl Default for AdaptiveQueueConfig {
    fn default() -> Self {
        let num_cores = num_cpus::get();
        Self {
            base_queue_size_per_core: 10,
            max_queue_size: num_cores * 50, // 每个核心50个任务
            min_queue_size: num_cores * 5,  // 每个核心至少5个任务
            priority_threshold: 10,
        }
    }
}

/// 自适应编译队列
pub struct AdaptiveCompileQueue {
    /// 任务队列（使用 BinaryHeap 实现优先级队列）
    queue: Arc<Mutex<BinaryHeap<Reverse<CompileTask>>>>,
    /// 配置
    config: AdaptiveQueueConfig,
    /// 当前队列大小限制
    queue_size_limit: Arc<Mutex<usize>>,
    /// 统计信息
    stats: Arc<Mutex<QueueStats>>,
}

/// 队列统计信息
#[derive(Debug, Clone, Default)]
pub struct QueueStats {
    /// 总任务数
    pub total_tasks: u64,
    /// 已完成任务数
    pub completed_tasks: u64,
    /// 高优先级任务数
    pub high_priority_tasks: u64,
    /// 平均等待时间（毫秒）
    pub avg_wait_time_ms: f64,
}

impl AdaptiveCompileQueue {
    /// 创建新的自适应编译队列
    pub fn new(config: AdaptiveQueueConfig) -> Self {
        let num_cores = num_cpus::get();
        let queue_size = (config.base_queue_size_per_core * num_cores)
            .min(config.max_queue_size)
            .max(config.min_queue_size);

        Self {
            queue: Arc::new(Mutex::new(BinaryHeap::new())),
            config: config.clone(),
            queue_size_limit: Arc::new(Mutex::new(queue_size)),
            stats: Arc::new(Mutex::new(QueueStats::default())),
        }
    }

    /// 根据 CPU 核心数调整队列大小
    pub fn adjust_for_cpu_cores(&self) {
        let num_cores = num_cpus::get();
        let new_size = (self.config.base_queue_size_per_core * num_cores)
            .min(self.config.max_queue_size)
            .max(self.config.min_queue_size);

        *self.queue_size_limit.lock() = new_size;

        // 如果队列超过新的大小限制，移除低优先级任务
        let mut queue = self.queue.lock();
        while queue.len() > new_size {
            if let Some(Reverse(task)) = queue.pop() {
                // 只移除低优先级任务
                if task.priority == CompilePriority::Low {
                    continue;
                }
                // 如果是高优先级任务，重新插入（不应该被移除）
                queue.push(Reverse(task));
                break;
            } else {
                break;
            }
        }
    }

    /// 添加编译任务
    pub fn enqueue(&self, task: CompileTask) -> bool {
        let mut queue = self.queue.lock();
        let size_limit = *self.queue_size_limit.lock();

        // 检查队列是否已满
        if queue.len() >= size_limit {
            // 如果新任务优先级更高，移除最低优先级的任务
            if let Some(Reverse(lowest)) = queue.peek() {
                if task.priority > lowest.priority {
                    queue.pop();
                } else {
                    return false; // 队列已满且新任务优先级不够高
                }
            }
        }

        queue.push(Reverse(task.clone()));
        let mut stats = self.stats.lock();
        stats.total_tasks += 1;
        if task.priority >= CompilePriority::High {
            stats.high_priority_tasks += 1;
        }

        true
    }

    /// 获取下一个编译任务
    pub fn dequeue(&self) -> Option<CompileTask> {
        let mut queue = self.queue.lock();
        if let Some(Reverse(task)) = queue.pop() {
            let mut stats = self.stats.lock();
            stats.completed_tasks += 1;
            let wait_time = Instant::now().duration_since(task.created_at);
            stats.avg_wait_time_ms = (stats.avg_wait_time_ms * (stats.completed_tasks - 1) as f64
                + wait_time.as_millis() as f64)
                / stats.completed_tasks as f64;
            Some(task)
        } else {
            None
        }
    }

    /// 批量获取任务
    pub fn dequeue_batch(&self, max_count: usize) -> Vec<CompileTask> {
        let mut tasks = Vec::with_capacity(max_count);
        let mut queue = self.queue.lock();

        for _ in 0..max_count {
            if let Some(Reverse(task)) = queue.pop() {
                tasks.push(task);
            } else {
                break;
            }
        }

        if !tasks.is_empty() {
            let mut stats = self.stats.lock();
            stats.completed_tasks += tasks.len() as u64;
            for task in &tasks {
                let wait_time = Instant::now().duration_since(task.created_at);
                stats.avg_wait_time_ms = (stats.avg_wait_time_ms
                    * (stats.completed_tasks - tasks.len() as u64) as f64
                    + wait_time.as_millis() as f64)
                    / stats.completed_tasks as f64;
            }
        }

        tasks
    }

    /// 更新任务的执行次数（提升优先级）
    pub fn update_task_execution(&self, pc: GuestAddr, execution_count: u32) {
        let mut queue = self.queue.lock();
        let mut tasks: Vec<CompileTask> = queue.drain().map(|Reverse(t)| t).collect();

        // 查找并更新任务
        for task in &mut tasks {
            if task.pc == pc {
                task.execution_count = execution_count;
                task.increment_execution();
            }
        }

        // 重新插入队列
        for task in tasks {
            queue.push(Reverse(task));
        }
    }

    /// 获取队列统计信息
    pub fn stats(&self) -> QueueStats {
        let stats = self.stats.lock();
        let queue = self.queue.lock();
        QueueStats {
            total_tasks: stats.total_tasks,
            completed_tasks: stats.completed_tasks,
            high_priority_tasks: stats.high_priority_tasks,
            avg_wait_time_ms: stats.avg_wait_time_ms,
            ..*stats
        }
    }

    /// 获取当前队列大小
    pub fn len(&self) -> usize {
        self.queue.lock().len()
    }

    /// 检查队列是否为空
    pub fn is_empty(&self) -> bool {
        self.queue.lock().is_empty()
    }

    /// 清空队列
    pub fn clear(&self) {
        self.queue.lock().clear();
    }
}

impl Clone for AdaptiveCompileQueue {
    fn clone(&self) -> Self {
        Self {
            queue: Arc::clone(&self.queue),
            config: self.config.clone(),
            queue_size_limit: Arc::clone(&self.queue_size_limit),
            stats: Arc::clone(&self.stats),
        }
    }
}

impl Default for AdaptiveCompileQueue {
    fn default() -> Self {
        Self::new(AdaptiveQueueConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adaptive_queue_enqueue_dequeue() {
        let queue = AdaptiveCompileQueue::default();

        let task1 = CompileTask::new(GuestAddr(0x1000), CompilePriority::Low);
        let task2 = CompileTask::new(GuestAddr(0x2000), CompilePriority::High);

        assert!(queue.enqueue(task1));
        assert!(queue.enqueue(task2));

        // 高优先级任务应该先出队
        let dequeued = queue.dequeue().unwrap();
        assert_eq!(dequeued.priority, CompilePriority::High);
        assert_eq!(dequeued.pc, GuestAddr(0x2000));
    }

    #[test]
    fn test_adaptive_queue_priority_update() {
        let queue = AdaptiveCompileQueue::default();

        let mut task = CompileTask::new(GuestAddr(0x1000), CompilePriority::Low);
        task.execution_count = 100;
        task.increment_execution();

        assert_eq!(task.priority, CompilePriority::Urgent);
    }

    #[test]
    fn test_adaptive_queue_adjust_for_cores() {
        let queue = AdaptiveCompileQueue::default();
        let initial_limit = *queue.queue_size_limit.lock();

        queue.adjust_for_cpu_cores();

        let new_limit = *queue.queue_size_limit.lock();
        assert_eq!(initial_limit, new_limit); // 应该保持不变（CPU核心数未变）
    }
}

