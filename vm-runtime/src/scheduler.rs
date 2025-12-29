// VM调度器（Scheduler）
//
// 本模块提供VM的调度器服务：
// - 任务调度
// - 优先级管理
// - 调度器配置
//

use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// 调度器配置
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// 最大任务队列大小
    pub max_tasks: usize,
    /// 优先级级别数
    pub priority_levels: usize,
    /// 时间片（毫秒）
    pub time_slice: Option<u64>,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            max_tasks: 1024,
            priority_levels: 4,
            time_slice: Some(10), //10ms
        }
    }
}

/// 任务优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    High = 3,
    Medium = 2,
    Low = 1,
    Idle = 0,
}

impl TaskPriority {
    /// 创建新的优先级
    pub fn new(level: u8) -> Self {
        match level {
            3 => TaskPriority::High,
            2 => TaskPriority::Medium,
            1 => TaskPriority::Low,
            0 => TaskPriority::Idle,
            _ => TaskPriority::Low,
        }
    }
}

/// VM任务
#[derive(Debug, Clone)]
pub struct VmTask {
    /// 任务ID
    pub task_id: u64,
    /// 任务名称
    pub name: String,
    /// 任务优先级
    pub priority: TaskPriority,
}

impl VmTask {
    /// 创建新的VM任务
    pub fn new(task_id: u64, name: String, priority: TaskPriority) -> Self {
        Self {
            task_id,
            name,
            priority,
        }
    }
}

impl PartialOrd for VmTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for VmTask {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority.cmp(&other.priority)
    }
}

impl PartialEq for VmTask {
    fn eq(&self, other: &Self) -> bool {
        self.task_id == other.task_id
    }
}

impl Eq for VmTask {}

/// VM调度器
#[derive(Debug, Clone)]
pub struct VmScheduler {
    /// 调度器配置
    pub config: SchedulerConfig,
    /// 任务队列
    pub task_queue: BinaryHeap<VmTask>,
    /// 下一个任务ID
    pub next_task_id: u64,
}

impl VmScheduler {
    /// 创建新的VM调度器
    ///
    /// # 参数
    /// - `config`: 调度器配置
    ///
    /// # 示例
    /// ```ignore
    /// let config = SchedulerConfig::default();
    /// let scheduler = VmScheduler::new(config);
    /// ```
    pub fn new(config: SchedulerConfig) -> Self {
        Self {
            config: config.clone(),
            task_queue: BinaryHeap::with_capacity(config.max_tasks),
            next_task_id: 1,
        }
    }

    /// 添加任务到队列
    ///
    /// # 参数
    /// - `task`: VM任务
    ///
    /// # 示例
    /// ```ignore
    /// let task = VmTask::new(1, "test_task".to_string(), TaskPriority::High);
    /// scheduler.add_task(task);
    /// ```
    pub fn add_task(&mut self, task: VmTask) {
        self.task_queue.push(task);
    }

    /// 获取下一个任务
    ///
    /// # 返回
    /// - `Some(task)`: 有任务
    /// - `None`: 无任务
    ///
    /// # 示例
    /// ```ignore
    /// if let Some(task) = scheduler.get_next_task() {
    ///     println!("Executing task: {}", task.name);
    /// }
    /// ```
    pub fn get_next_task(&mut self) -> Option<VmTask> {
        self.task_queue.pop()
    }

    /// 检查是否有待处理的任务
    ///
    /// # 返回
    /// - `true`: 有任务
    /// - `false`: 无任务
    pub fn has_pending_tasks(&self) -> bool {
        !self.task_queue.is_empty()
    }

    /// 获取队列大小
    pub fn queue_size(&self) -> usize {
        self.task_queue.len()
    }

    /// 清空任务队列
    pub fn clear_queue(&mut self) {
        self.task_queue.clear();
    }

    /// 获取调度器状态
    pub fn get_status(&self) -> SchedulerStatus {
        SchedulerStatus {
            queue_size: self.queue_size(),
            max_queue_size: self.config.max_tasks,
            next_task_id: self.next_task_id,
        }
    }
}

impl Default for VmScheduler {
    fn default() -> Self {
        Self::new(SchedulerConfig::default())
    }
}

/// 调度器状态
#[derive(Debug, Clone)]
pub struct SchedulerStatus {
    /// 当前队列大小
    pub queue_size: usize,
    /// 最大队列大小
    pub max_queue_size: usize,
    /// 下一个任务ID
    pub next_task_id: u64,
}

impl std::fmt::Display for SchedulerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "VM调度器状态")?;
        writeln!(f, "  队列大小: {}/{}", self.queue_size, self.max_queue_size)?;
        writeln!(f, "  下一个任务ID: {}", self.next_task_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_config_default() {
        let config = SchedulerConfig::default();
        assert_eq!(config.max_tasks, 1024);
        assert_eq!(config.priority_levels, 4);
        assert_eq!(config.time_slice, Some(10));
    }

    #[test]
    fn test_task_priority() {
        let high = TaskPriority::High;
        let low = TaskPriority::Low;
        assert!(high > low);
        assert_eq!(high.cmp(&low), Ordering::Greater);
    }

    #[test]
    fn test_task_creation() {
        let task = VmTask::new(1, "test".to_string(), TaskPriority::High);
        assert_eq!(task.task_id, 1);
        assert_eq!(task.name, "test");
        assert_eq!(task.priority, TaskPriority::High);
    }

    #[test]
    fn test_scheduler_creation() {
        let config = SchedulerConfig::default();
        let scheduler = VmScheduler::new(config);

        assert_eq!(scheduler.next_task_id, 1);
        assert!(!scheduler.has_pending_tasks());
    }

    #[test]
    fn test_add_task() {
        let config = SchedulerConfig::default();
        let mut scheduler = VmScheduler::new(config);

        let task = VmTask::new(1, "test".to_string(), TaskPriority::High);
        scheduler.add_task(task);

        assert!(scheduler.has_pending_tasks());
        assert_eq!(scheduler.queue_size(), 1);
    }

    #[test]
    fn test_get_next_task() {
        let config = SchedulerConfig::default();
        let mut scheduler = VmScheduler::new(config);

        let task = VmTask::new(1, "test".to_string(), TaskPriority::High);
        scheduler.add_task(task);

        let next = scheduler.get_next_task();
        assert!(next.is_some());
        let next_task = next.expect("Task should be available");
        assert_eq!(next_task.name, "test");
    }

    #[test]
    fn test_scheduler_status() {
        let config = SchedulerConfig::default();
        let scheduler = VmScheduler::new(config);

        let status = scheduler.get_status();
        let display = format!("{}", status);

        assert!(display.contains("VM调度器状态"));
    }

    #[test]
    fn test_task_comparison() {
        let high_task = VmTask::new(1, "high".to_string(), TaskPriority::High);
        let low_task = VmTask::new(2, "low".to_string(), TaskPriority::Low);

        assert!(high_task > low_task);
        assert_eq!(high_task.cmp(&low_task), Ordering::Greater);
    }

    #[test]
    fn test_task_equality() {
        let task1 = VmTask::new(1, "test".to_string(), TaskPriority::High);
        let task2 = VmTask::new(1, "test".to_string(), TaskPriority::High);

        assert_eq!(task1, task2);
    }

    #[test]
    fn test_clear_queue() {
        let config = SchedulerConfig::default();
        let mut scheduler = VmScheduler::new(config);

        let task = VmTask::new(1, "test".to_string(), TaskPriority::High);
        scheduler.add_task(task);

        assert_eq!(scheduler.queue_size(), 1);

        scheduler.clear_queue();

        assert!(!scheduler.has_pending_tasks());
        assert_eq!(scheduler.queue_size(), 0);
    }
}
