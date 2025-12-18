//! Communication Protocol for Distributed VM
//!
//! This module defines the message format for communication between VMs and the coordinator.

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Unique identifier for VM instances
pub type VmId = String;

/// Unique identifier for tasks
pub type TaskId = String;

/// Message types for VM communication
#[derive(Debug, Serialize, Deserialize)]
pub enum VmMessage {
    /// Hello message for VM discovery
    Hello {
        vm_id: VmId,
        vm_addr: SocketAddr,
        capabilities: VmCapabilities,
    },

    /// Ping message for health check
    Ping,

    /// Pong response to ping
    Pong { vm_id: VmId },

    /// Task submission from coordinator to VM
    TaskSubmit {
        task_id: TaskId,
        task_type: TaskType,
        task_data: Vec<u8>,
    },

    /// Task status update from VM to coordinator
    TaskStatus {
        task_id: TaskId,
        status: TaskStatus,
        progress: u8, // 0-100%
        message: Option<String>,
    },

    /// Task result from VM to coordinator
    TaskResult {
        task_id: TaskId,
        result: Result<Vec<u8>, String>,
    },

    /// Resource status update from VM to coordinator
    ResourceStatus {
        vm_id: VmId,
        cpu_usage: u8,
        memory_usage: u8,
        disk_usage: u8,
    },

    /// Control message from coordinator to VM
    Control {
        vm_id: VmId,
        command: ControlCommand,
    },
}

/// VM capabilities
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VmCapabilities {
    /// CPU count
    pub cpu_count: usize,

    /// Memory size in MB
    pub memory_mb: u64,

    /// Supported instruction sets
    pub instruction_sets: Vec<String>,

    /// GPU support
    pub has_gpu: bool,
}

/// Task types
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum TaskType {
    /// CPU intensive task
    CpuIntensive,

    /// Memory intensive task
    MemoryIntensive,

    /// I/O intensive task
    IoIntensive,

    /// GPU accelerated task
    GpuAccelerated,
}

/// Task status
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum TaskStatus {
    /// Task is pending
    Pending,

    /// Task is running
    Running,

    /// Task is completed successfully
    Completed,

    /// Task failed
    Failed,

    /// Task was cancelled
    Cancelled,
}

/// Task information
#[derive(Debug, Clone)]
pub struct TaskInfo {
    pub task_id: TaskId,
    pub task_type: TaskType,
    pub status: TaskStatus,
    pub progress: u8,
    pub vm_id: Option<String>,
    pub submission_time: std::time::Instant,
    pub update_time: std::time::Instant,
}

/// Control commands
#[derive(Debug, Serialize, Deserialize)]
pub enum ControlCommand {
    /// Shutdown the VM
    Shutdown,

    /// Restart the VM
    Restart,

    /// Pause the VM
    Pause,

    /// Resume the VM
    Resume,

    /// Update VM configuration
    UpdateConfig { config: serde_json::Value },
}
