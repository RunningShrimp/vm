//! 异步接口定义

use crate::{TaskId, TaskResult, TaskStatus, VmError};

/// 异步执行上下文
#[async_trait::async_trait]
pub trait AsyncExecutionContext {
    /// 获取异步运行时
    fn runtime(&self) -> &tokio::runtime::Runtime;

    /// 获取任务调度器
    fn scheduler(&self) -> &dyn TaskScheduler;

    /// 生成任务ID
    fn generate_task_id(&self) -> TaskId;
}

/// 任务调度器接口
#[async_trait::async_trait]
pub trait TaskScheduler {
    /// 提交任务
    async fn submit_task(&self, task: Box<dyn AsyncTask>) -> TaskId;

    /// 取消任务
    async fn cancel_task(&self, task_id: TaskId) -> Result<(), VmError>;

    /// 获取任务状态
    async fn get_task_status(&self, task_id: TaskId) -> TaskStatus;

    /// 等待任务完成
    async fn wait_task(&self, task_id: TaskId) -> Result<TaskResult, VmError>;
}

/// 异步任务trait
#[async_trait::async_trait]
pub trait AsyncTask: Send + Sync {
    /// 执行任务
    async fn execute(&mut self) -> Result<(), VmError>;

    /// 获取任务描述
    fn description(&self) -> &str;
}

/// 默认任务调度器实现
pub struct DefaultTaskScheduler {
    runtime: tokio::runtime::Runtime,
    task_counter: std::sync::atomic::AtomicU64,
}

impl DefaultTaskScheduler {
    pub fn new() -> Result<Self, VmError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| VmError::Io(std::io::Error::other(format!("{}", e)).to_string()))?;

        Ok(Self {
            runtime,
            task_counter: std::sync::atomic::AtomicU64::new(0),
        })
    }
}

#[async_trait::async_trait]
impl TaskScheduler for DefaultTaskScheduler {
    async fn submit_task(&self, mut task: Box<dyn AsyncTask>) -> TaskId {
        let task_id = self
            .task_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        self.runtime.spawn(async move {
            let result = task.execute().await;
            // 这里可以存储结果或发送到通道
            match result {
                Ok(_) => println!("Task {} completed successfully", task_id),
                Err(e) => println!("Task {} failed: {:?}", task_id, e),
            }
        });

        task_id
    }

    async fn cancel_task(&self, _task_id: TaskId) -> Result<(), VmError> {
        // 简化实现：实际实现需要任务取消机制
        Ok(())
    }

    async fn get_task_status(&self, _task_id: TaskId) -> TaskStatus {
        // 简化实现：实际实现需要任务状态跟踪
        TaskStatus::Completed
    }

    async fn wait_task(&self, _task_id: TaskId) -> Result<TaskResult, VmError> {
        // 简化实现
        Ok(TaskResult::Success)
    }
}

/// 默认异步执行上下文
pub struct DefaultAsyncContext {
    runtime: tokio::runtime::Runtime,
    scheduler: DefaultTaskScheduler,
}

impl DefaultAsyncContext {
    pub fn new() -> Result<Self, VmError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| VmError::Io(std::io::Error::other(format!("{}", e)).to_string()))?;

        let scheduler = DefaultTaskScheduler::new()?;

        Ok(Self { runtime, scheduler })
    }
}

#[async_trait::async_trait]
impl AsyncExecutionContext for DefaultAsyncContext {
    fn runtime(&self) -> &tokio::runtime::Runtime {
        &self.runtime
    }

    fn scheduler(&self) -> &dyn TaskScheduler {
        &self.scheduler
    }

    fn generate_task_id(&self) -> TaskId {
        self.scheduler
            .task_counter
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}
