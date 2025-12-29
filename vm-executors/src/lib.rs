//! Unified Executor Framework for VM
//!
//! This crate provides a comprehensive executor framework including:
//! - Async execution engines (JIT, interpreter, hybrid)
//! - Coroutine scheduling
//! - Distributed execution
//!
//! # Modules
//!
//! - [`async_executor`]: Async execution engines for VM execution
//! - [`coroutine`]: GMP-style coroutine scheduler with work stealing
//! - [`distributed`]: Distributed execution across multiple VMs
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
//! ```rust
//! use vm_executors::distributed::{DistributedArchitectureConfig, initialize_distributed_environment};
//!
//! let config = DistributedArchitectureConfig::default();
//! let coordinator = initialize_distributed_environment(config).await.unwrap();
//! ```

pub mod async_executor;
pub mod coroutine;
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

// Distributed re-exports
pub use distributed::{
    DistributedArchitectureConfig, VmCoordinator, VmMessage, initialize_distributed_environment,
};

pub use distributed::architecture::{FaultToleranceConfig, LoadBalancingStrategy};
pub use distributed::discovery::VmInfo;
pub use distributed::fault_tolerance::FaultToleranceManager;
pub use distributed::protocol::{TaskId, TaskInfo, TaskStatus, TaskType, VmCapabilities};
pub use distributed::scheduler::TaskScheduler;
