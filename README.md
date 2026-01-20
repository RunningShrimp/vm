# VM Project

A high-performance, cross-architecture virtual machine written in Rust.

## Overview

This is a comprehensive virtual machine implementation supporting multiple CPU architectures (x86_64, ARM64, RISC-V) with advanced features including:

- **Multi-architecture support**: x86_64, ARM64, and RISC-V
- **JIT compilation**: Tiered JIT with Cranelift and LLVM backends
- **GPU acceleration**: Virtualized GPU and passthrough support
- **Advanced memory management**: Lock-free MMU with NUMA optimization
- **Cross-platform**: Linux, macOS, and Windows support
- **Hotplug and snapshots**: Runtime device hotplug and incremental snapshots
- **GUI and CLI**: Both desktop GUI application and command-line interface

## Quick Start

### Prerequisites

- Rust 1.92 or later
- Platform-specific dependencies (KVM on Linux, HVF on macOS, WHP on Windows)

### Build

```bash
# Build all workspace members
cargo build --release

# Build CLI only
cargo build --release --package vm-cli

# Build desktop GUI
cd vm-desktop && cargo tauri build
```

### Run

```bash
# CLI: Quick start with Debian
vm-cli install-debian

# GUI: Desktop application
cd vm-desktop && cargo tauri dev
```

## Documentation

- [User Guide](docs/user-guides/USER_GUIDE.md) - Complete user manual for CLI and GUI
- [Multi-OS Support](docs/user-guides/MULTI_OS_SUPPORT.md) - Supported operating systems
- [Development](docs/development/) - Development guides and reports
- [API Documentation](docs/api/) - Module API documentation

## Project Structure

```
.
├── Cargo.toml        # Workspace configuration
├── Cargo.lock        # Dependency lock file
├── README.md         # This file
│
├── crates/           # Core libraries (26 modules organized by function)
│   ├── core/         # Core VM components
│   │   ├── vm-core          # Core VM engine and domain logic
│   │   ├── vm-ir            # Intermediate representation
│   │   └── vm-boot          # Boot and runtime services
│   │
│   ├── execution/    # Execution engines
│   │   ├── vm-engine        # Execution engine (interpreter + JIT)
│   │   ├── vm-engine-jit    # Advanced JIT implementation
│   │   └── vm-frontend      # Frontend decoders (x86_64, ARM64, RISC-V)
│   │
│   ├── memory/       # Memory management
│   │   ├── vm-mem           # Memory management and MMU
│   │   ├── vm-gc            # Garbage collection
│   │   └── vm-optimizers    # Performance optimizers
│   │
│   ├── platform/     # Platform abstraction
│   │   ├── vm-accel         # Hardware acceleration (KVM, HVF, WHP)
│   │   ├── vm-platform       # Platform-specific code
│   │   └── vm-osal          # OS abstraction layer
│   │
│   ├── devices/      # Device emulation
│   │   ├── vm-device         # Device emulation framework
│   │   ├── vm-graphics       # Graphics devices
│   │   ├── vm-smmu          # IOMMU/SMMU support
│   │   └── vm-soc           # System-on-chip devices
│   │
│   ├── runtime/      # Runtime services
│   │   ├── vm-service        # VM service orchestration
│   │   ├── vm-plugin        # Plugin system
│   │   └── vm-monitor       # Monitoring and metrics
│   │
│   ├── compatibility/ # Compatibility layer
│   │   ├── security-sandbox  # Security sandboxing
│   │   └── syscall-compat   # System call compatibility
│   │
│   └── architecture/  # Architecture support
│       ├── vm-cross-arch-support  # Cross-architecture support
│       ├── vm-codegen             # Code generation tools
│       └── vm-build-deps          # Build dependencies
│
├── tools/            # User-facing tools
│   ├── cli/          # Command-line interface (vm-cli)
│   ├── desktop/      # Desktop GUI application (vm-desktop)
│   ├── debug/        # Debugging tools (vm-debug)
│   └── passthrough/  # Device passthrough (vm-passthrough)
│
├── research/         # Research and experiments
│   ├── perf-bench/       # Performance benchmarks
│   ├── tiered-compiler/  # Tiered compiler experiments
│   ├── parallel-jit/     # Parallel JIT research
│   └── benches/          # Benchmark suites
│
├── docs/             # Documentation
│   ├── api/          # API documentation
│   ├── architecture/  # Architecture docs
│   ├── development/  # Development guides
│   └── user-guides/  # User guides
│
├── tests/            # Test suites
├── scripts/          # Helper scripts
├── plans/            # Planning documents
└── fixtures/         # Test fixtures (ISOs, kernels, etc.)
```

## Features

### Architecture

- **DDD Architecture**: Domain-driven design with aggregates and services
- **Event Sourcing**: Domain events for reproducible state
- **Async Execution**: Tokio-based async runtime with lock-free data structures
- **Plugin System**: Extensible architecture with sandboxed plugins

### Performance

- **Lock-free MMU**: High-performance memory management
- **SIMD Optimization**: NEON/SSE optimization for critical paths
- **NUMA Support**: Multi-socket optimization
- **Tiered JIT**: Adaptive compilation with profile-guided optimization
- **Cache Optimization**: Smart caching for translation and compilation

### Devices

- **GPU**: Virtualized GPU with passthrough support (NVIDIA, AMD, Intel)
- **VirtIO**: Full virtio device stack (9p, balloon, console, crypto, etc.)
- **Block Devices**: AHCI, virtio-blk with async I/O
- **Network**: Virtio-net with multi-queue support
- **Input**: Keyboard, mouse, and gamepad support

### Platform Support

| Platform | Status | Notes |
|----------|--------|-------|
| x86_64 Linux | ✅ Full | KVM acceleration |
| ARM64 Linux | ✅ Full | KVM acceleration |
| ARM64 macOS | ✅ Full | HVF acceleration |
| x86_64 Windows | ✅ Full | WHP acceleration |
| RISC-V | 🚧 In Progress | JIT and device support |

## Contributing

Please see [CONTRIBUTING.md](docs/development/CONTRIBUTING.md) for contribution guidelines.

## License

MIT OR Apache-2.0

## Authors

VM Development Team

## Repository

https://github.com/example/vm
