//! VM Coordinator
//!
//! This module provides the main VM coordinator that manages VM discovery,
//! task distribution, and fault tolerance.

use crate::executor::distributed::architecture::DistributedArchitectureConfig;
use crate::executor::distributed::discovery::{VmDiscovery, VmInfo};
use crate::executor::distributed::protocol::{TaskId, TaskInfo, TaskStatus, TaskType, VmMessage};
use crate::executor::distributed::scheduler::TaskScheduler;
use std::sync::Arc;

/// VM Coordinator
pub struct VmCoordinator {
    config: DistributedArchitectureConfig,
    discovery: VmDiscovery,
    scheduler: TaskScheduler,
    next_vm_index: Arc<std::sync::Mutex<usize>>,
}

impl VmCoordinator {
    /// Create a new VM coordinator
    pub async fn new(config: DistributedArchitectureConfig) -> Result<Self, anyhow::Error> {
        let discovery = VmDiscovery::new(&config)?;
        let scheduler = TaskScheduler::new(&config)?;

        Ok(VmCoordinator {
            config,
            discovery,
            scheduler,
            next_vm_index: Arc::new(std::sync::Mutex::new(0)),
        })
    }

    /// Start the VM discovery service
    pub async fn start_discovery(&self) -> Result<(), anyhow::Error> {
        self.discovery.start().await
    }

    /// Start the task scheduler
    pub async fn start_scheduler(&self) -> Result<(), anyhow::Error> {
        self.scheduler.start().await
    }

    /// Submit a task to the distributed system
    pub async fn submit_task(
        &self,
        task_type: TaskType,
        task_data: Vec<u8>,
    ) -> Result<TaskId, anyhow::Error> {
        // Get active VMs
        let active_vms = self.discovery.get_active_vms().await;

        if active_vms.is_empty() {
            return Err(anyhow::anyhow!("No active VMs available"));
        }

        // Select a VM based on task type and load balancing strategy
        let selected_vm = match task_type {
            TaskType::CpuIntensive => self.select_cpu_intensive(&active_vms, &task_type).await,
            TaskType::MemoryIntensive => {
                self.select_memory_intensive(&active_vms, &task_type).await
            }
            TaskType::IoIntensive => self.select_least_loaded(&active_vms).await,
            _ => self.select_round_robin(&active_vms).await,
        };

        // Create and submit the task
        let task_id = uuid::Uuid::new_v4().to_string();
        let _task_submit = VmMessage::TaskSubmit {
            task_id: task_id.clone(),
            task_type,
            task_data,
        };

        // In a real implementation, we would send the task to the selected VM
        log::info!("Task {} submitted to VM {}", task_id, selected_vm.vm_id);

        Ok(task_id)
    }

    /// Get task status
    pub async fn get_task_status(&self, task_id: &TaskId) -> Result<TaskStatus, anyhow::Error> {
        self.scheduler.get_task_status(task_id).await
    }

    /// Get detailed task information
    pub async fn get_task_info(&self, task_id: &TaskId) -> Result<TaskInfo, anyhow::Error> {
        self.scheduler.get_task_info(task_id).await
    }

    /// Get active VM list
    pub async fn get_active_vms(&self) -> Vec<VmInfo> {
        self.discovery.get_active_vms().await
    }

    /// Get the coordinator configuration
    pub fn get_config(&self) -> &DistributedArchitectureConfig {
        &self.config
    }

    // Note: These methods have been simplified to not return references
    // This makes them compatible with async contexts

    /// Round robin VM selection
    async fn select_round_robin(&self, active_vms: &[VmInfo]) -> VmInfo {
        let mut index = self
            .next_vm_index
            .lock()
            .expect("Next VM index mutex should not be poisoned");
        let selected = active_vms[*index % active_vms.len()].clone();
        *index += 1;
        selected
    }

    /// Least loaded VM selection
    async fn select_least_loaded(&self, active_vms: &[VmInfo]) -> VmInfo {
        // Calculate load score (higher = more loaded)
        active_vms
            .iter()
            .min_by_key(|vm| vm.cpu_usage + vm.memory_usage)
            .expect("Active VMs list should not be empty when selecting least loaded")
            .clone()
    }

    /// CPU intensive task selection
    async fn select_cpu_intensive(&self, active_vms: &[VmInfo], _task_type: &TaskType) -> VmInfo {
        // For CPU intensive tasks, select VM with lowest CPU usage
        active_vms
            .iter()
            .min_by_key(|vm| vm.cpu_usage)
            .expect("Active VMs list should not be empty when selecting for CPU intensive tasks")
            .clone()
    }

    /// Memory intensive task selection
    async fn select_memory_intensive(
        &self,
        active_vms: &[VmInfo],
        _task_type: &TaskType,
    ) -> VmInfo {
        // For memory intensive tasks, select VM with lowest memory usage
        active_vms
            .iter()
            .min_by_key(|vm| vm.memory_usage)
            .expect("Active VMs list should not be empty when selecting for memory intensive tasks")
            .clone()
    }
}
