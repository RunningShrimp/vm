//! Unified Executor Framework for VM
//!
//! This crate provides a comprehensive executor framework including:
//! - Async execution engines (JIT, interpreter, hybrid)
//! - Coroutine scheduling
//! - Distributed execution (requires `async` feature)
//!
//! # Modules
//!
//! - [`async_executor`]: Async execution engines for VM execution
//! - [`coroutine`]: GMP-style coroutine scheduler with work stealing
//! - [`distributed`]: Distributed execution across multiple VMs (requires `async` feature)
//!
//! # Examples
//!
//! ## Using the JIT Executor
//!
//! ```rust
//! use vm_executors::async_executor::JitExecutor;
//!
//! let mut executor = JitExecutor::new();
//! let result = executor.execute_block(1).unwrap();
//! ```
//!
//! ## Using the Coroutine Scheduler
//!
//! ```rust
//! use vm_executors::coroutine::Scheduler;
//!
//! let mut scheduler = Scheduler::new(4); // 4 vCPUs
//! let coroutine = scheduler.create_coroutine();
//! scheduler.submit_coroutine(coroutine);
//! ```
//!
//! ## Using Distributed Execution
//!
//! ```rust,ignore
//! use vm_executors::distributed::{DistributedArchitectureConfig, initialize_distributed_environment};
//!
//! let config = DistributedArchitectureConfig::default();
//! let coordinator = initialize_distributed_environment(config).await.unwrap();
//! ```

pub mod async_executor;
pub mod coroutine;

// Distributed execution (requires async feature)
#[cfg(feature = "async")]
pub mod distributed;

// Async Executor re-exports
pub use async_executor::{
    AsyncExecutionContext, ExecutionResult, ExecutionStats, ExecutorType, HybridExecutor,
    InterpreterExecutor, JitExecutor,
};
// Coroutine re-exports
pub use coroutine::{
    Coroutine, CoroutineId, CoroutineState, Scheduler, SchedulerStats, VCPU, VCPUState, VCPUStats,
};
#[cfg(feature = "async")]
pub use distributed::architecture::{FaultToleranceConfig, LoadBalancingStrategy};
#[cfg(feature = "async")]
pub use distributed::discovery::VmInfo;
#[cfg(feature = "async")]
pub use distributed::fault_tolerance::FaultToleranceManager;
#[cfg(feature = "async")]
pub use distributed::protocol::{TaskId, TaskInfo, TaskStatus, TaskType, VmCapabilities};
#[cfg(feature = "async")]
pub use distributed::scheduler::TaskScheduler;
// Distributed re-exports (only available with async feature)
#[cfg(feature = "async")]
pub use distributed::{
    DistributedArchitectureConfig, VmCoordinator, VmMessage, initialize_distributed_environment,
};
