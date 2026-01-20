# API Documentation

This directory contains API documentation for VM project modules.

## Core Modules

### [vm-core](../../vm-core/README.md)
Core VM engine, domain logic, and aggregate roots. Contains:
- Domain models and aggregates
- Event sourcing infrastructure
- Dependency injection
- Configuration management

### [vm-engine](../../vm-engine/README.md)
Execution engine with interpreter and JIT support. Contains:
- Instruction execution logic
- Baseline interpreter
- Unified execution framework

### [vm-engine-jit](../../vm-engine-jit/README.md)
Advanced JIT compiler implementation. Contains:
- Tiered compilation
- Cranelift and LLVM backends
- Profile-guided optimization
- ML-guided optimization

## Architecture Support

### [vm-frontend](../../vm-frontend/README.md)
Frontend instruction decoders for multiple architectures:
- x86_64 decoder
- ARM64 decoder
- RISC-V64 decoder
- Translation layer

### [vm-ir](../../vm-ir/README.md)
Intermediate representation and instruction lifting:
- IR data structures
- Instruction patterns
- Translation pipeline

### [vm-cross-arch-support](../../vm-cross-arch-support/README.md)
Cross-architecture translation support:
- Encoding cache
- Instruction patterns
- Memory access abstraction

## Memory Management

### [vm-mem](../../vm-mem/README.md)
Memory management and MMU:
- Lock-free MMU
- TLB management
- NUMA optimization
- SIMD-optimized operations

### [vm-gc](../../vm-gc/README.md)
Garbage collection:
- Generational GC
- Incremental GC
- Concurrent GC
- Adaptive strategies

## Devices and Acceleration

### [vm-device](../../vm-device/README.md)
Device emulation:
- Block devices (AHCI, virtio-blk)
- Network devices (virtio-net)
- GPU emulation
- Input devices

### [vm-accel](../../vm-accel/README.md)
Hardware acceleration backends:
- KVM (Linux)
- HVF (macOS)
- WHP (Windows)
- Vendor-specific optimizations

### [vm-passthrough](../../vm-passthrough/README.md)
Device passthrough support:
- GPU passthrough
- NPU passthrough
- PCIe passthrough
- SR-IOV support

### [vm-smmu](../../vm-smmu/README.md)
SMMU/IOMMU support:
- Address translation
- Interrupt handling
- TLB management

## Platform and Services

### [vm-platform](../../vm-platform/README.md)
Platform abstraction layer:
- Boot sequence
- Hotplug support
- Snapshot management
- Signal handling

### [vm-boot](../../vm-boot/README.md)
Boot and runtime services:
- Fast boot optimization
- ISO9660 support
- Hotplug initialization
- Snapshot handling

### [vm-service](../../vm-service/README.md)
VM service orchestration:
- Service lifecycle
- Device management
- Event-driven architecture

## Tools and Utilities

### [vm-cli](../../vm-cli/README.md)
Command-line interface:
- OS installation commands
- VM management
- Configuration tools

### [vm-desktop](../../vm-desktop/README.md)
Desktop GUI application:
- Tauri-based GUI
- VM controller
- Monitoring interface

### [vm-monitor](../../vm-monitor/README.md)
Monitoring and metrics:
- Real-time monitoring
- Performance metrics
- Alerting system

### [vm-debug](../../vm-debug/README.md)
Debugging tools:
- GDB stub
- Tracing utilities
- Inspection tools

## Optimization and Extension

### [vm-optimizers](../../vm-optmizers/README.md)
Performance optimizers:
- Memory allocation optimization
- GC optimization
- PGO support
- NUMA optimization

### [vm-plugin](../../vm-plugin/README.md)
Plugin system:
- Plugin loader
- Extension points
- Sandbox isolation

### [vm-osal](../../vm-osal/README.md)
Operating system abstraction layer:
- Platform detection
- System calls
- Threading abstraction

## Code Generation

### [vm-codegen](../../vm-codegen/README.md)
Code generation tools:
- Frontend code generators
- IR transformers
- Build utilities

### [vm-build-deps](../../vm-build-deps/README.md)
Build dependencies:
- Shared dependencies
- Hakari configuration

## Building Documentation

To generate Rustdoc for all modules:

```bash
# Build documentation for all packages
cargo doc --all --no-deps --open

# Build with private items
cargo doc --all --no-deps --document-private-items --open
```

## Module Organization

Modules are organized by responsibility:

1. **Core**: Essential VM functionality
2. **Architecture**: CPU architecture support
3. **Memory**: Memory management and GC
4. **Devices**: Hardware emulation
5. **Platform**: Platform abstraction
6. **Tools**: CLI, GUI, monitoring
7. **Optimization**: Performance enhancements
8. **Extension**: Plugin system and codegen

## API Design Principles

- **Type Safety**: Leverage Rust's type system
- **Error Handling**: Use `Result<T, E>` consistently
- **Async/Await**: Use Tokio for async operations
- **Documentation**: Public APIs must have doc comments
- **Testing**: Comprehensive test coverage

## Additional Resources

- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [DDD Architecture](../architecture/)
- [Development Guide](../development/CONTRIBUTING.md)
