//! Distributed Execution Engine for VM
//!
//! This module provides distributed execution capabilities for multiple VMs,
//! including VM discovery, task distribution, and fault tolerance.
//!
//! 此模块需要 `async` feature 支持。

#[cfg(feature = "async")]
use std::sync::Arc;

#[cfg(feature = "async")]
pub mod architecture;
#[cfg(feature = "async")]
pub mod coordinator;
#[cfg(feature = "async")]
pub mod discovery;
#[cfg(feature = "async")]
pub mod fault_tolerance;
#[cfg(feature = "async")]
pub mod protocol;
#[cfg(feature = "async")]
pub mod scheduler;

#[cfg(feature = "async")]
pub use architecture::DistributedArchitectureConfig;
#[cfg(feature = "async")]
pub use coordinator::VmCoordinator;
#[cfg(feature = "async")]
pub use protocol::VmMessage;

/// Initialize distributed execution environment
#[cfg(feature = "async")]
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
