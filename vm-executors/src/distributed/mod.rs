//! Distributed Execution Engine for VM
//!
//! This module provides distributed execution capabilities for multiple VMs,
//! including VM discovery, task distribution, and fault tolerance.

use std::sync::Arc;

pub mod architecture;
pub mod coordinator;
pub mod discovery;
pub mod fault_tolerance;
pub mod protocol;
pub mod scheduler;

pub use architecture::DistributedArchitectureConfig;
pub use coordinator::VmCoordinator;
pub use protocol::VmMessage;

/// Initialize distributed execution environment
pub async fn initialize_distributed_environment(
    config: DistributedArchitectureConfig,
) -> Result<Arc<VmCoordinator>, anyhow::Error> {
    // Initialize the VM coordinator
    let coordinator = VmCoordinator::new(config).await?;

    // Start discovery service
    coordinator.start_discovery().await?;

    // Start task scheduler
    coordinator.start_scheduler().await?;

    Ok(Arc::new(coordinator))
}
