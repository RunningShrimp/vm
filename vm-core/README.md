# vm-core

Virtual machine core domain layer implementing DDD (Domain-Driven Design) principles with comprehensive business logic, event sourcing, and dependency injection.

## Overview

`vm-core` is the heart of the Rust VM project, providing the foundational domain models, business logic, and infrastructure for virtual machine management. It implements strict anemic domain model principles with business logic properly encapsulated in domain services.

## Architecture

### Domain-Driven Design (DDD)

The crate follows DDD principles with these key components:

- **Aggregates**: `VirtualMachineAggregate` - root entity managing VM lifecycle
- **Domain Events**: Event-driven architecture with `DomainEventBus`
- **Repositories**: Aggregate, event, and snapshot repositories
- **Domain Services**: 12 specialized services for business logic
- **Dependency Injection**: Comprehensive DI framework with 11 modules

### Key Components

#### 1. Domain Models
- **`VirtualMachineAggregate`**: Root aggregate managing VM state
- **`DomainEventEnum`**: Comprehensive domain events (state changes, execution, devices)
- **`VmError`**: Unified error handling with error chaining and context

#### 2. Event Sourcing
- **`EventStore`**: Append-only event storage
- **`Snapshot`**: State snapshots for performance optimization
- **`DomainEventBus`**: Async and sync event bus implementations
- **Event Replay**: Reconstruction of aggregate state from events

#### 3. Dependency Injection
- **`ServiceContainer`**: DI container for service management
- **`ServiceFactory`**: Lazy service initialization
- **`LifecycleManager`**: Service lifecycle control
- **Scoped and transient services**: Flexible registration options

#### 4. Domain Services
- **`DeviceHotplugService`**: Runtime device management
- **`ExecutionService`**: VM execution control
- **`SnapshotService`**: VM state snapshot management
- **`MigrationService`**: Live migration support
- **And 8 more specialized services**

#### 5. Memory Management
- **`AsyncMmu`**: Async memory management unit
- **NUMA-aware allocation**: Optimize for multi-socket systems
- **Memory protection**: W^X policy enforcement

#### 6. Device Emulation
- **`DeviceEmulation`**: Unified device emulation interface
- **Hot-plug support**: Runtime device addition/removal
- **Device manager**: Centralized device lifecycle management

#### 7. Debugger Integration
- **GDB server**: Remote debugging protocol
- **Breakpoint management**: Software and hardware breakpoints
- **Register access**: CPU state inspection and modification

## Features

### Default Features
- **`std`**: Standard library support (enabled by default)

### Optional Features
- **`async`**: Async runtime support (tokio, futures)
- **`enhanced-event-sourcing`**: Advanced event sourcing with chrono and async
- **`optimization_application`**: Optimization application framework
- **GPU acceleration**: `cuda`, `rocm`, `gpu` (placeholder, see vm-passthrough)

### Architecture Features
- **`x86_64`**: x86_64 architecture support
- **`arm64`**: ARM64 architecture support
- **`riscv64`**: RISC-V 64-bit architecture support

## Usage

### Basic VM Creation

```rust
use vm_core::{VirtualMachineAggregate, VmConfig};

// Create VM aggregate
let mut vm = VirtualMachineAggregate::new(VmConfig::default())?;

// Start VM
vm.start()?;

// Execute instructions
vm.execute()?;

// Stop VM
vm.stop()?;
```

### Event Sourcing

```rust
use vm_core::{EventStore, DomainEventEnum};

// Create event store
let event_store = EventStore::new()?;

// Append events
let event = DomainEventEnum::VmStarted {
    timestamp: std::time::SystemTime::now(),
};
event_store.append("vm-123", event)?;

// Replay events
let events = event_store.read("vm-123")?;
for event in events {
    // Process event
}
```

### Dependency Injection

```rust
use vm_core::di::{ServiceContainer, ServiceFactory};

// Create container
let mut container = ServiceContainer::new();

// Register service
container.register_singleton::<MyService>()?;

// Resolve service
let service: MyService = container.resolve()?;
```

### Domain Events

```rust
use vm_core::{DomainEventBus, DomainEventEnum};

// Subscribe to events
let bus = DomainEventBus::new();
bus.subscribe(|event: &DomainEventEnum| {
    match event {
        DomainEventEnum::VmStarted { timestamp } => {
            println!("VM started at {:?}", timestamp);
        }
        _ => {}
    }
});

// Publish event
bus.publish(DomainEventEnum::VmStarted {
    timestamp: std::time::SystemTime::now(),
})?;
```

## Error Handling

All errors use the unified `VmError` type:

```rust
use vm_core::{VmError, CoreError};

// Error with context
let result: Result<(), VmError> = Err(VmError::Core(CoreError::Config {
    message: "Invalid configuration".to_string(),
    path: Some("/etc/vm/config.toml".to_string()),
}));

// Error chaining
let result = result.context("Failed to load VM configuration")?;
```

## Performance Considerations

### Memory
- **NUMA-aware allocation**: Optimized for multi-socket systems
- **Memory pools**: Reduce allocation overhead
- **Lazy evaluation**: Services initialized on first use

### Concurrency
- **Lock-free structures**: DashMap for concurrent access
- **Async executors**: Tokio-based async execution
- **Parallel execution**: Multi-core instruction execution

### Event Sourcing
- **Snapshots**: Reduce replay overhead
- **Event batching**: Optimize storage I/O
- **Caching**: Aggregate state caching

## Testing

```bash
# Run all tests
cargo test -p vm-core

# Run with coverage
cargo tarpaulin -p vm-core --out Html

# Run specific test
cargo test -p vm-core test_vm_creation
```

## Dependencies

### Core Dependencies
- `parking_lot`: High-performance locks
- `serde`: Serialization framework
- `thiserror`: Error handling
- `uuid`: Unique identifiers
- `dashmap`: Concurrent hashmap

### Optional Dependencies
- `tokio`: Async runtime
- `chrono`: Date and time
- `async-trait`: Async trait support

## Architecture Diagram

```
┌─────────────────────────────────────────────────┐
│           vm-core (Domain Layer)                │
├─────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐            │
│  │  Aggregates  │  │Domain Events │            │
│  │              │  │              │            │
│  │ VirtualMac   │  │EventBus      │            │
│  │ hineAggregate│  │EventStore    │            │
│  └──────┬───────┘  └──────┬───────┘            │
│         │                  │                     │
│  ┌──────▼──────────────────▼───────┐           │
│  │     Domain Services (12)         │           │
│  │  • DeviceHotplugService          │           │
│  │  • ExecutionService              │           │
│  │  • SnapshotService               │           │
│  │  • MigrationService              │           │
│  └──────┬──────────────────┬────────┘           │
│         │                  │                     │
│  ┌──────▼──────────────────▼───────┐           │
│  │    Dependency Injection (DI)     │           │
│  │  • ServiceContainer              │           │
│  │  • ServiceFactory                │           │
│  │  • LifecycleManager              │           │
│  └──────┬──────────────────┬────────┘           │
│         │                  │                     │
│  ┌──────▼──────────────────▼───────┐           │
│  │      Repositories & Stores       │           │
│  │  • AggregateRepository           │           │
│  │  • EventRepository               │           │
│  │  • SnapshotRepository            │           │
│  └──────────────────────────────────┘           │
└─────────────────────────────────────────────────┘
```

## Design Patterns

### 1. Repository Pattern
Data access abstraction for aggregates, events, and snapshots.

### 2. Event Sourcing
State reconstruction from event log.

### 3. Dependency Injection
Inversion of control for loosely coupled components.

### 4. Observer Pattern
Domain event subscription and notification.

### 5. Aggregate Pattern
Consistency boundary around domain entities.

## Best Practices

1. **Always use aggregates**: Access domain models through aggregates
2. **Embrace events**: Use domain events for cross-aggregate communication
3. **DI for services**: Register services in DI container
4. **Error context**: Chain errors with context using `.context()`
5. **Immutability**: Prefer immutable data structures where possible

## Performance Tips

1. **Enable snapshots**: Reduce event replay overhead
2. **Use async**: For I/O-bound operations
3. **NUMA-aware**: Bind memory to NUMA nodes for performance
4. **Batch events**: Group events for efficient storage
5. **Cache judiciously**: Balance memory vs computation

## Related Crates

- **vm-mem**: Memory management subsystem
- **vm-device**: Device emulation
- **vm-accel**: Hardware acceleration (KVM, HVF, WHPX)
- **vm-engine**: Unified execution engine
- **vm-gc**: Garbage collection

## License

[Your License Here]

## Contributing

Contributions are welcome! Please ensure:
- All tests pass: `cargo test -p vm-core`
- Code is formatted: `cargo fmt -p vm-core`
- No clippy warnings: `cargo clippy -p vm-core`

## Authors

[Author Names]

## See Also

- [VM_COMPREHENSIVE_REVIEW_REPORT.md](../VM_COMPREHENSIVE_REVIEW_REPORT.md) - Comprehensive project review
- [Domain-Driven Design](https://domainlanguage.com/ddd/) - DDD reference
- [Event Sourcing](https://martinfowler.com/eaaDev/EventSourcing.html) - Event sourcing pattern
