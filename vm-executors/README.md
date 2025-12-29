# vm-executors

Unified Executor Framework for VM - Consolidates all executor implementations into a single package.

## Overview

This package provides a comprehensive executor framework that includes:

- **Async Execution Engines**: JIT, Interpreter, and Hybrid executors for VM instruction execution
- **Coroutine Scheduler**: GMP-style coroutine scheduler with work stealing and load balancing
- **Distributed Execution**: Distributed execution system for multiple VMs with fault tolerance

## Modules

### Async Executor (`async_executor`)

Provides three execution engines:

- **JitExecutor**: Just-in-time compilation for fast execution
- **InterpreterExecutor**: Interpreted execution for compatibility
- **HybridExecutor**: Adaptive selection between JIT and interpreter

Features:
- Code block caching
- Execution statistics
- Batch execution support

Example:
```rust
use vm_executors::JitExecutor;

let mut executor = JitExecutor::new();
let result = executor.execute_block(1)?;
let stats = executor.get_stats();
```

### Coroutine Scheduler (`coroutine`)

GMP-style coroutine scheduler with work stealing:

Features:
- Multiple vCPU support
- Local and global run queues
- Work stealing for load balancing
- Comprehensive statistics

Example:
```rust
use vm_executors::Scheduler;

let mut scheduler = Scheduler::new(4); // 4 vCPUs
let coroutine = scheduler.create_coroutine();
scheduler.submit_coroutine(coroutine);

// Execute coroutines
if let Some(coro) = scheduler.next_coroutine(0) {
    // Execute coroutine
}
```

### Distributed Execution (`distributed`)

Distributed execution system for multiple VMs:

Components:
- **VmCoordinator**: Main coordinator for distributed execution
- **VmDiscovery**: Service discovery for VM instances
- **TaskScheduler**: Task scheduling and management
- **FaultToleranceManager**: Fault tolerance and recovery

Example:
```rust
use vm_executors::distributed::{DistributedArchitectureConfig, initialize_distributed_environment};

let config = DistributedArchitectureConfig::default();
let coordinator = initialize_distributed_environment(config).await?;

// Submit tasks
let task_id = coordinator.submit_task(TaskType::CpuIntensive, vec![]).await?;
```

## Features

- `default`: Core executor functionality
- `distributed`: Enable distributed execution (requires async runtime)

## Dependencies

- `vm-engine-jit`: JIT compilation support
- `vm-common`: Common utilities
- `tokio`: Async runtime
- `parking_lot`: High-performance synchronization primitives
- `serde`/`serde_json`: Serialization
- `log`: Logging facade
- `uuid`: UUID generation
- `anyhow`: Error handling

## Testing

Run tests with:
```bash
cargo test -p vm-executors
```

Run with all features:
```bash
cargo test -p vm-executors --all-features
```

## Migration from Separate Packages

This package consolidates three previously separate packages:

1. **async-executor** → `vm-executors::async_executor`
2. **coroutine-scheduler** → `vm-executors::coroutine`
3. **distributed-executor** → `vm-executors::distributed`

### Migration Guide

Replace old imports:

```rust
// Old
use async_executor::JitExecutor;
use coroutine_scheduler::Scheduler;
use distributed_executor::VmCoordinator;

// New
use vm_executors::{JitExecutor, Scheduler, VmCoordinator};
```

## License

MIT OR Apache-2.0
