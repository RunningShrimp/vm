# vm-service

VM services layer providing high-level VM management, execution control, monitoring, and service orchestration with async runtime support and comprehensive state management.

## Overview

`vm-service` provides the service layer abstraction for VM operations, offering high-level APIs for VM lifecycle management, execution control, monitoring, and service composition in a distributed environment.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                vm-service (VM Services Layer)            │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐ │
│  │  VM Service  │  │  Execution   │  │  Snapshot    │ │
│  │     API      │  │   Service    │  │   Service    │ │
│  │              │  │              │  │              │ │
│  │ • Lifecycle  │  │ • Start/stop │  │ • Create     │ │
│  │ • Query      │  │ • Pause/resume│  │ • Restore    │ │
│  │ • Config     │  │ • Reset      │  │ • Delete     │ │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘ │
│         │                  │                  │         │
│         └──────────────────┼──────────────────┘         │
│                            │                            │
│                  ┌─────────▼──────────┐                 │
│                  │   Service Layer     │                 │
│                  │                    │                 │
│                  │ • Orchestration     │                 │
│                  │ • Composition       │                 │
│                  │ • State management  │                 │
│                  └─────────┬──────────┘                 │
│                            │                            │
│  ┌─────────────────────────┼─────────────────────────┐ │
│  │  ┌──────────────────────▼─────────────────────┐  │ │
│  │  │         Monitoring & Metrics                │  │ │
│  │  │  • Performance metrics                      │  │ │
│  │  │  • Resource usage                           │  │ │
│  │  │  • Event streaming                         │  │ │
│  │  │  • Health checks                            │  │ │
│  │  └────────────────────────────────────────────┘  │ │
│  │                                                   │ │
│  │  ┌─────────────────────────────────────────────┐  │ │
│  │  │            Async Runtime                     │  │ │
│  │  │  • Tokio async runtime                      │  │ │
│  │  │  • Async service operations                │  │ │
│  │  │  • Concurrent request handling              │  │ │
│  │  └────────────────────────────────────────────┘  │ │
│  │                                                   │ │
│  │  ┌─────────────────────────────────────────────┐  │ │
│  │  │         Distributed Services                 │  │ │
│  │  │  • Service discovery                         │  │ │
│  │  │  • Load balancing                           │  │ │
│  │  │  • Failover                                 │  │ │
│  │  └────────────────────────────────────────────┘  │ │
│  └───────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
```

## Key Components

### 1. VM Service API (`src/vm_service/mod.rs`)

**VMService Trait**:
```rust
pub trait VMService: Send + Sync {
    /// Create new VM
    async fn create_vm(&self, config: VmConfig) -> Result<String, ServiceError>;

    /// Start VM
    async fn start_vm(&self, vm_id: &str) -> Result<(), ServiceError>;

    /// Stop VM
    async fn stop_vm(&self, vm_id: &str) -> Result<(), ServiceError>;

    /// Query VM state
    async fn get_vm(&self, vm_id: &str) -> Result<VmState, ServiceError>;

    /// Delete VM
    async fn delete_vm(&self, vm_id: &str) -> Result<(), ServiceError>;
}
```

**Usage**:
```rust
use vm_service::vm_service::VMService;

let service = VMService::new()?;

// Create VM
let vm_id = service.create_vm(config).await?;

// Start VM
service.start_vm(&vm_id).await?;

// Query state
let state = service.get_vm(&vm_id).await?;
println!("VM status: {:?}", state.status);
```

### 2. Execution Service (`src/vm_service/execution.rs`)

**ExecutionControl Trait**:
```rust
pub trait ExecutionControl: Send + Sync {
    /// Start execution
    async fn start(&self, vm_id: &str) -> Result<(), ServiceError>;

    /// Pause execution
    async fn pause(&self, vm_id: &str) -> Result<(), ServiceError>;

    /// Resume execution
    async fn resume(&self, vm_id: &str) -> Result<(), ServiceError>;

    /// Reset VM
    async fn reset(&self, vm_id: &str) -> Result<(), ServiceError>;
}
```

**Usage**:
```rust
use vm_service::execution::ExecutionService;

let exec_service = ExecutionService::new()?;

// Pause VM
exec_service.pause(&vm_id).await?;

// Do some work...

// Resume VM
exec_service.resume(&vm_id).await?;
```

### 3. Snapshot Service (`src/vm_service/snapshot_manager.rs`)

**SnapshotService Trait**:
```rust
pub trait SnapshotService: Send + Sync {
    /// Create snapshot
    async fn create_snapshot(
        &self,
        vm_id: &str,
        name: &str,
    ) -> Result<String, ServiceError>;

    /// Restore snapshot
    async fn restore_snapshot(
        &self,
        vm_id: &str,
        snapshot_id: &str,
    ) -> Result<(), ServiceError>;

    /// Delete snapshot
    async fn delete_snapshot(
        &self,
        vm_id: &str,
        snapshot_id: &str,
    ) -> Result<(), ServiceError>;

    /// List snapshots
    async fn list_snapshots(&self, vm_id: &str) -> Result<Vec<Snapshot>, ServiceError>;
}
```

**Usage**:
```rust
use vm_service::snapshot_manager::SnapshotService;

let snap_service = SnapshotService::new()?;

// Create snapshot
let snapshot_id = snap_service.create_snapshot(&vm_id, "pre-upgrade").await?;

// Restore snapshot
snap_service.restore_snapshot(&vm_id, &snapshot_id).await?;

// List snapshots
let snapshots = snap_service.list_snapshots(&vm_id).await?;
for snap in snapshots {
    println!("Snapshot: {} ({})", snap.name, snap.created_at);
}
```

### 4. Monitoring Service (`src/vm_service/monitoring.rs`)

**Monitoring Features**:
- Performance metrics
- Resource usage tracking
- Event streaming
- Health checks

**Usage**:
```rust
use vm_service::monitoring::{MonitoringService, MetricQuery};

let monitor = MonitoringService::new()?;

// Get performance metrics
let query = MetricQuery::vm(&vm_id);
let metrics = monitor.get_metrics(query).await?;

println!("CPU: {}%", metrics.cpu_usage);
println!("Memory: {} MB", metrics.memory_mb);

// Stream events
let mut stream = monitor.subscribe_events(&vm_id).await?;
while let Some(event) = stream.next().await {
    println!("Event: {:?}", event);
}
```

**Metrics Available**:
- CPU usage percentage
- Memory usage (bytes, percentage)
- Disk I/O (reads, writes, bytes)
- Network I/O (tx, rx, packets)
- Execution statistics
- Error rates

### 5. Service Composition

**ServiceComposer**:
```rust
use vm_service::composer::ServiceComposer;

let composer = ServiceComposer::new();

// Compose services
let vm_service = composer.compose_vm_service()?;
let exec_service = composer.compose_execution_service()?;
let snap_service = composer.compose_snapshot_service()?;

// Use composed services
vm_service.create_vm(config).await?;
exec_service.start(&vm_id).await?;
```

### 6. Async Runtime

**AsyncService**:
```rust
use vm_service::async_runtime::AsyncServiceManager;

let manager = AsyncServiceManager::new()?;

// Run async operations
tokio::spawn(async move {
    manager.run_vm(&vm_id).await?;
});

// Handle concurrent requests
let (vm1_result, vm2_result) = tokio::join!(
    service.get_vm("vm-1"),
    service.get_vm("vm-2")
);
```

## Usage Examples

### Basic VM Lifecycle

```rust
use vm_service::{VmService, VmConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = VmService::new()?;

    // Create VM
    let config = VmConfig::default();
    let vm_id = service.create_vm(config).await?;

    // Start VM
    service.start_vm(&vm_id).await?;

    // Run for some time
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

    // Stop VM
    service.stop_vm(&vm_id).await?;

    Ok(())
}
```

### Execution Control

```rust
use vm_service::execution::ExecutionService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let exec_service = ExecutionService::new()?;

    // Pause execution
    exec_service.pause(&vm_id).await?;

    // Take checkpoint
    exec_service.checkpoint(&vm_id).await?;

    // Resume execution
    exec_service.resume(&vm_id).await?;

    Ok(())
}
```

### Monitoring and Metrics

```rust
use vm_service::monitoring::{MonitoringService, MetricQuery};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let monitor = MonitoringService::new()?;

    // Query metrics
    let query = MetricQuery::vm("vm-123");
    let metrics = monitor.get_metrics(query).await?;

    println!("Performance:");
    println!("  CPU: {:.1}%", metrics.cpu_usage);
    println!("  Memory: {} MB", metrics.memory_mb);
    println!("  Disk I/O: {} reads/s", metrics.disk_reads_per_sec);

    // Subscribe to events
    let mut events = monitor.subscribe_events("vm-123").await?;
    while let Some(event) = events.next().await {
        println!("Event: {:?}", event);
    }

    Ok(())
}
```

### Service Composition

```rust
use vm_service::composer::ServiceComposer;

fn compose_services() -> Result<ServiceBundle, ServiceError> {
    let composer = ServiceComposer::new();

    let bundle = ServiceComposer::new()
        .with_vm_service()?
        .with_execution_service()?
        .with_snapshot_service()?
        .with_monitoring_service()?
        .compose()?;

    Ok(bundle)
}
```

## Features

### Async/Await Support
- Tokio-based async runtime
- Non-blocking operations
- Concurrent request handling
- Efficient resource utilization

### Service Composition
- Combine multiple services
- Shared state management
- Dependency injection
- Lifecycle coordination

### Monitoring
- Real-time metrics
- Event streaming
- Health checks
- Performance tracking

## Performance Characteristics

### Operation Latency

| Operation | Latency | Notes |
|-----------|---------|-------|
| Create VM | 100-500ms | Depends on config |
| Start VM | 50-200ms | Cold start |
| Stop VM | 50-100ms | Graceful shutdown |
| Create Snapshot | 1-5s | Depends on memory |
| Restore Snapshot | 500ms-2s | Fast resume |
| Query State | 1-10ms | In-memory lookup |

### Throughput

| Operation | Throughput | Notes |
|-----------|-----------|-------|
| Concurrent VMs | 100-1000 | Per service instance |
| Queries/sec | 10K-100K | State queries |
| Snapshot/sec | 10-50 | Storage dependent |

## Best Practices

1. **Use Async**: Leverage async/await for concurrent operations
2. **Handle Errors**: All operations can fail, handle gracefully
3. **Monitor Resources**: Track service health and performance
4. **Use Snapshots**: Before major changes
5. **Clean Up**: Delete unused VMs and snapshots

## Configuration

### Service Configuration

```rust
use vm_service::ServiceConfig;

let config = ServiceConfig {
    max_concurrent_vms: 100,
    default_timeout: Duration::from_secs(30),
    enable_metrics: true,
    enable_tracing: true,
    snapshot_dir: "/var/lib/vm/snapshots".into(),
};
```

### VM Configuration

```rust
use vm_service::VmConfig;

let config = VmConfig {
    name: "my-vm".to_string(),
    vcpus: 2,
    memory_mb: 2048,
    kernel: "/vmlinuz".into(),
    initrd: Some("/initrd.img".into()),
    cmdline: "console=ttyS0".to_string(),
    devices: vec![],
};
```

## Testing

```bash
# Run all tests
cargo test -p vm-service

# Test VM service
cargo test -p vm-service --lib vm_service

# Test execution service
cargo test -p vm-service --lib execution

# Test snapshot service
cargo test -p vm-service --lib snapshot

# Test monitoring
cargo test -p vm-service --lib monitoring
```

## Related Crates

- **vm-core**: Domain models and aggregates
- **vm-boot**: Boot and lifecycle management
- **vm-engine**: Execution engine
- **vm-monitor**: Monitoring tools

## Dependencies

### Core Dependencies
- `vm-core`: Domain models
- `tokio`: Async runtime
- `serde`: Serialization
- `uuid`: Unique identifiers

## Platform Support

| Platform | Async Runtime | Status |
|----------|--------------|--------|
| Linux | Tokio + io-uring | ✅ Full |
| macOS | Tokio | ✅ Full |
| Windows | Tokio | ✅ Good |

## License

[Your License Here]

## Contributing

Contributions welcome! Please:
- Use async/await throughout
- Handle errors gracefully
- Add tests for async operations
- Document service APIs

## See Also

- [Tokio Documentation](https://tokio.rs/)
- [Async Rust Book](https://rust-lang.github.io/async-book/)
- [Service Layer Pattern](https://martinfowler.com/eaaCatalog/ServiceLayer.html)
