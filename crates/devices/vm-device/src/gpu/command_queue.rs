//! GPU命令队列
//!
//! 提供GPU命令的提交、排队、执行和同步功能

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, Condvar};
use std::time::{Duration, Instant};

// 导入GpuCommand和GpuCommandType（从父模块）
use crate::{GpuCommand, GpuCommandType};

impl GpuCommandQueue {
    /// 处理命令队列（执行命令）
    pub fn process_command_queue<F>(&self, executor: F) -> usize
    where
        F: Fn(&GpuCommand) -> Result<(), String>,
    {
        let mut processed = 0;
        let max_process = 100; // 每次最多处理100个命令

        while processed < max_process {
            if let Some(command) = self.try_dequeue() {
                match executor(&command) {
                    Ok(_) => {
                        processed += 1;
                        let wait_time = command.submit_time.elapsed().as_micros() as u64;
                        self.mark_completed(wait_time);
                    }
                    Err(_) => {
                        // 命令执行失败，跳过
                        break;
                    }
                }
            } else {
                break;
            }
        }

        processed
    }
}

/// 命令队列状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandQueueState {
    /// 空闲
    Idle,
    /// 运行中
    Running,
    /// 暂停
    Paused,
    /// 错误
    Error,
}

/// GPU命令队列
pub struct GpuCommandQueue {
    /// 命令队列
    queue: Arc<Mutex<VecDeque<GpuCommand>>>,
    /// 队列状态
    state: Arc<Mutex<CommandQueueState>>,
    /// 条件变量（用于等待命令）
    condvar: Arc<Condvar>,
    /// 最大队列大小
    max_size: usize,
    /// 已提交的命令数
    submitted_count: Arc<Mutex<u64>>,
    /// 已完成的命令数
    completed_count: Arc<Mutex<u64>>,
    /// 队列统计
    stats: Arc<Mutex<CommandQueueStats>>,
}

/// 命令队列统计
#[derive(Debug, Clone, Default)]
pub struct CommandQueueStats {
    /// 总提交的命令数
    pub total_submitted: u64,
    /// 总完成的命令数
    pub total_completed: u64,
    /// 平均等待时间（微秒）
    pub avg_wait_time_us: u64,
    /// 最大队列深度
    pub max_queue_depth: usize,
    /// 队列溢出次数
    pub overflow_count: u64,
}

impl GpuCommandQueue {
    /// 创建新的命令队列
    pub fn new(max_size: usize) -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            state: Arc::new(Mutex::new(CommandQueueState::Idle)),
            condvar: Arc::new(Condvar::new()),
            max_size,
            submitted_count: Arc::new(Mutex::new(0)),
            completed_count: Arc::new(Mutex::new(0)),
            stats: Arc::new(Mutex::new(CommandQueueStats::default())),
        }
    }

    // Helper methods for safe lock operations
    fn lock_queue(&self) -> Result<std::sync::MutexGuard<VecDeque<GpuCommand>>, CommandQueueError> {
        self.queue.lock().map_err(|_| CommandQueueError::QueueError)
    }

    fn lock_state(&self) -> Result<std::sync::MutexGuard<CommandQueueState>, CommandQueueError> {
        self.state.lock().map_err(|_| CommandQueueError::QueueError)
    }

    fn lock_stats(&self) -> Result<std::sync::MutexGuard<CommandQueueStats>, CommandQueueError> {
        self.stats.lock().map_err(|_| CommandQueueError::QueueError)
    }

    fn lock_submitted_count(&self) -> Result<std::sync::MutexGuard<u64>, CommandQueueError> {
        self.submitted_count.lock().map_err(|_| CommandQueueError::QueueError)
    }

    fn lock_completed_count(&self) -> Result<std::sync::MutexGuard<u64>, CommandQueueError> {
        self.completed_count.lock().map_err(|_| CommandQueueError::QueueError)
    }

    /// 提交命令到队列
    pub fn submit(&self, command: GpuCommand) -> Result<(), CommandQueueError> {
        let mut queue = self.lock_queue()?;
        let mut stats = self.lock_stats()?;

        // 检查队列是否已满
        if queue.len() >= self.max_size {
            stats.overflow_count += 1;
            return Err(CommandQueueError::QueueFull);
        }

        // 检查队列状态
        let state = *self.lock_state()?;
        if state == CommandQueueState::Error {
            return Err(CommandQueueError::QueueError);
        }

        // 添加到队列
        queue.push_back(command);
        *self.lock_submitted_count()? += 1;
        stats.total_submitted += 1;
        stats.max_queue_depth = stats.max_queue_depth.max(queue.len());

        // 通知等待的线程
        self.condvar.notify_one();

        Ok(())
    }

    /// 批量提交命令
    pub fn submit_batch(&self, commands: Vec<GpuCommand>) -> Result<usize, CommandQueueError> {
        let mut queue = self.lock_queue()?;
        let mut stats = self.lock_stats()?;
        let state = *self.lock_state()?;

        if state == CommandQueueState::Error {
            return Err(CommandQueueError::QueueError);
        }

        let mut submitted = 0;
        for command in commands {
            if queue.len() >= self.max_size {
                stats.overflow_count += 1;
                break;
            }
            queue.push_back(command);
            submitted += 1;
        }

        *self.lock_submitted_count()? += submitted as u64;
        stats.total_submitted += submitted as u64;
        stats.max_queue_depth = stats.max_queue_depth.max(queue.len());

        if submitted > 0 {
            self.condvar.notify_all();
        }

        Ok(submitted)
    }

    /// 从队列获取下一个命令（阻塞）
    pub fn dequeue(&self, timeout: Option<Duration>) -> Option<GpuCommand> {
        let mut queue = self.lock_queue().ok()?;
        let start_time = Instant::now();

        loop {
            // 检查队列状态
            let state = match self.lock_state() {
                Ok(s) => *s,
                Err(_) => return None,
            };
            if state == CommandQueueState::Error {
                return None;
            }

            // 尝试获取命令
            if let Some(command) = queue.pop_front() {
                return Some(command);
            }

            // 检查超时
            if let Some(timeout) = timeout {
                if start_time.elapsed() >= timeout {
                    return None;
                }
            }

            // 等待命令到达
            let wait_timeout = timeout.map(|t| {
                let elapsed = start_time.elapsed();
                if elapsed < t {
                    t - elapsed
                } else {
                    Duration::from_millis(0)
                }
            });

            if let Some(timeout) = wait_timeout {
                let (guard, _) = self.condvar.wait_timeout(queue, timeout).ok()?;
                queue = guard;
            } else {
                queue = self.condvar.wait(queue).ok()?;
            }
        }
    }

    /// 从队列获取下一个命令（非阻塞）
    pub fn try_dequeue(&self) -> Option<GpuCommand> {
        let mut queue = self.lock_queue().ok()?;
        queue.pop_front()
    }

    /// 标记命令完成
    pub fn mark_completed(&self, wait_time_us: u64) {
        if let Ok(mut completed) = self.lock_completed_count() {
            *completed += 1;
        }
        if let Ok(mut stats) = self.lock_stats() {
            stats.total_completed += 1;

            // 更新平均等待时间（简单移动平均）
            let completed = stats.total_completed;
            if completed > 0 {
                stats.avg_wait_time_us = (stats.avg_wait_time_us * (completed - 1) + wait_time_us) / completed;
            }
        }
    }

    /// 获取队列大小
    pub fn size(&self) -> usize {
        match self.lock_queue() {
            Ok(queue) => queue.len(),
            Err(_) => 0,
        }
    }

    /// 获取最大队列大小
    pub fn max_size(&self) -> usize {
        self.max_size
    }

    /// 检查队列是否为空
    pub fn is_empty(&self) -> bool {
        match self.lock_queue() {
            Ok(queue) => queue.is_empty(),
            Err(_) => true,
        }
    }

    /// 清空队列
    pub fn clear(&self) {
        if let Ok(mut queue) = self.lock_queue() {
            queue.clear();
        }
    }

    /// 设置队列状态
    pub fn set_state(&self, state: CommandQueueState) {
        if let Ok(mut s) = self.lock_state() {
            *s = state;
            self.condvar.notify_all();
        }
    }

    /// 获取队列状态
    pub fn get_state(&self) -> CommandQueueState {
        match self.lock_state() {
            Ok(state) => *state,
            Err(_) => CommandQueueState::Error,
        }
    }

    /// 启动队列
    pub fn start(&self) {
        self.set_state(CommandQueueState::Running);
    }

    /// 暂停队列
    pub fn pause(&self) {
        self.set_state(CommandQueueState::Paused);
    }

    /// 恢复队列
    pub fn resume(&self) {
        self.set_state(CommandQueueState::Running);
    }

    /// 停止队列
    pub fn stop(&self) {
        self.set_state(CommandQueueState::Idle);
        self.clear();
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> CommandQueueStats {
        match self.lock_stats() {
            Ok(stats) => stats.clone(),
            Err(_) => CommandQueueStats::default(),
        }
    }

    /// 重置统计信息
    pub fn reset_stats(&self) {
        if let Ok(mut stats) = self.lock_stats() {
            *stats = CommandQueueStats::default();
        }
        if let Ok(mut submitted) = self.lock_submitted_count() {
            *submitted = 0;
        }
        if let Ok(mut completed) = self.lock_completed_count() {
            *completed = 0;
        }
    }

    /// 获取提交的命令数
    pub fn get_submitted_count(&self) -> u64 {
        match self.lock_submitted_count() {
            Ok(count) => *count,
            Err(_) => 0,
        }
    }

    /// 获取完成的命令数
    pub fn get_completed_count(&self) -> u64 {
        match self.lock_completed_count() {
            Ok(count) => *count,
            Err(_) => 0,
        }
    }

    /// 等待队列为空（阻塞直到队列为空或超时）
    pub fn wait_empty(&self, timeout: Option<Duration>) -> bool {
        let start_time = Instant::now();
        loop {
            if self.is_empty() {
                return true;
            }

            if let Some(timeout) = timeout {
                if start_time.elapsed() >= timeout {
                    return false;
                }
            }

            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// 命令队列错误
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandQueueError {
    /// 队列已满
    QueueFull,
    /// 队列错误
    QueueError,
    /// 无效命令
    InvalidCommand,
}

impl std::fmt::Display for CommandQueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandQueueError::QueueFull => write!(f, "Command queue is full"),
            CommandQueueError::QueueError => write!(f, "Command queue is in error state"),
            CommandQueueError::InvalidCommand => write!(f, "Invalid command"),
        }
    }
}

impl std::error::Error for CommandQueueError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GpuCommand;

    #[test]
    fn test_command_queue_creation() {
        let queue = GpuCommandQueue::new(100);
        assert_eq!(queue.size(), 0);
        assert!(queue.is_empty());
        assert_eq!(queue.get_state(), CommandQueueState::Idle);
    }

    #[test]
    fn test_command_submit() {
        let queue = GpuCommandQueue::new(10);
        let command = GpuCommand {
            command_type: GpuCommandType::KernelLaunch,
            parameters: vec![1, 2, 3],
            submit_time: Instant::now(),
        };

        assert!(queue.submit(command).is_ok());
        assert_eq!(queue.size(), 1);
        assert!(!queue.is_empty());
    }

    #[test]
    fn test_command_dequeue() {
        let queue = GpuCommandQueue::new(10);
        let command = GpuCommand {
            command_type: GpuCommandType::KernelLaunch,
            parameters: vec![1, 2, 3],
            submit_time: Instant::now(),
        };

        queue.submit(command.clone()).unwrap();
        let dequeued = queue.try_dequeue();
        assert!(dequeued.is_some());
        assert_eq!(queue.size(), 0);
    }

    #[test]
    fn test_queue_full() {
        let queue = GpuCommandQueue::new(2);
        let command = GpuCommand {
            command_type: GpuCommandType::KernelLaunch,
            parameters: vec![],
            submit_time: Instant::now(),
        };

        queue.submit(command.clone()).unwrap();
        queue.submit(command.clone()).unwrap();
        assert!(queue.submit(command).is_err());
    }

    #[test]
    fn test_batch_submit() {
        let queue = GpuCommandQueue::new(10);
        let commands: Vec<GpuCommand> = (0..5)
            .map(|i| GpuCommand {
                command_type: GpuCommandType::KernelLaunch,
                parameters: vec![i],
                submit_time: Instant::now(),
            })
            .collect();

        let submitted = queue.submit_batch(commands).unwrap();
        assert_eq!(submitted, 5);
        assert_eq!(queue.size(), 5);
    }

    #[test]
    fn test_queue_state() {
        let queue = GpuCommandQueue::new(10);
        assert_eq!(queue.get_state(), CommandQueueState::Idle);

        queue.start();
        assert_eq!(queue.get_state(), CommandQueueState::Running);

        queue.pause();
        assert_eq!(queue.get_state(), CommandQueueState::Paused);

        queue.resume();
        assert_eq!(queue.get_state(), CommandQueueState::Running);

        queue.stop();
        assert_eq!(queue.get_state(), CommandQueueState::Idle);
    }

    #[test]
    fn test_stats() {
        let queue = GpuCommandQueue::new(10);
        let command = GpuCommand {
            command_type: GpuCommandType::KernelLaunch,
            parameters: vec![],
            submit_time: Instant::now(),
        };

        queue.submit(command).unwrap();
        queue.mark_completed(100);

        let stats = queue.get_stats();
        assert_eq!(stats.total_submitted, 1);
        assert_eq!(stats.total_completed, 1);
    }
}

