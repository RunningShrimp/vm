//! Distributed Task Scheduler
//!
//! This module provides task scheduling capabilities for distributed VMs.

use crate::executor::distributed::architecture::DistributedArchitectureConfig;
use crate::executor::distributed::protocol::{TaskId, TaskInfo, TaskStatus, TaskType};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

/// Task Scheduler
pub struct TaskScheduler {
    config: DistributedArchitectureConfig,
    tasks: Arc<Mutex<HashMap<TaskId, TaskInfo>>>,
}

impl TaskScheduler {
    /// Create a new task scheduler
    pub fn new(config: &DistributedArchitectureConfig) -> Result<Self, anyhow::Error> {
        Ok(TaskScheduler {
            config: config.clone(),
            tasks: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Start the task scheduler
    pub async fn start(&self) -> Result<(), anyhow::Error> {
        // Start task monitoring
        let tasks = self.tasks.clone();
        let config = self.config.clone();

        #[cfg(feature = "async")]
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(config.fault_tolerance.health_check_interval).await;

                // Check for timed out tasks
                let mut tasks = tasks.lock();

                for (_task_id, task_info) in tasks.iter_mut() {
                    if task_info.status == TaskStatus::Running
                        && task_info.update_time.elapsed() > config.fault_tolerance.task_timeout
                    {
                        // Task timed out
                        task_info.status = TaskStatus::Failed;
                        task_info.progress = 0;
                        task_info.update_time = std::time::Instant::now();
                    }
                }
            }
        });

        Ok(())
    }

    /// Get task status
    pub async fn get_task_status(&self, task_id: &TaskId) -> Result<TaskStatus, anyhow::Error> {
        let tasks = self.tasks.lock();

        tasks
            .get(task_id)
            .map(|task| task.status.clone())
            .ok_or_else(|| anyhow::anyhow!("Task not found"))
    }

    /// Get detailed task information
    pub async fn get_task_info(&self, task_id: &TaskId) -> Result<TaskInfo, anyhow::Error> {
        let tasks = self.tasks.lock();

        tasks
            .get(task_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Task not found"))
    }

    /// Update task status
    pub async fn update_task_status(
        &self,
        task_id: &TaskId,
        status: TaskStatus,
        progress: u8,
    ) -> Result<(), anyhow::Error> {
        let mut tasks = self.tasks.lock();

        if let Some(task) = tasks.get_mut(task_id) {
            task.status = status;
            task.progress = progress;
            task.update_time = std::time::Instant::now();
            Ok(())
        } else {
            Err(anyhow::anyhow!("Task not found"))
        }
    }

    /// Add a new task
    pub async fn add_task(
        &self,
        task_id: TaskId,
        task_type: TaskType,
    ) -> Result<(), anyhow::Error> {
        let mut tasks = self.tasks.lock();

        if tasks.contains_key(&task_id) {
            return Err(anyhow::anyhow!("Task already exists"));
        }

        let now = std::time::Instant::now();
        let task_info = TaskInfo {
            task_id: task_id.clone(),
            task_type,
            status: TaskStatus::Pending,
            progress: 0,
            vm_id: None,
            submission_time: now,
            update_time: now,
        };
        tasks.insert(task_id, task_info);

        Ok(())
    }

    /// Assign a task to a VM
    pub async fn assign_task(&self, task_id: &TaskId, vm_id: String) -> Result<(), anyhow::Error> {
        let mut tasks = self.tasks.lock();

        if let Some(task) = tasks.get_mut(task_id) {
            task.vm_id = Some(vm_id);
            task.status = TaskStatus::Running;
            task.progress = 0;
            task.update_time = std::time::Instant::now();
            Ok(())
        } else {
            Err(anyhow::anyhow!("Task not found"))
        }
    }
}
