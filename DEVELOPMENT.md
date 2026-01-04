# Development Guide

Comprehensive guide for developing and debugging the Rust VM project.

---

## 📋 Table of Contents

- [Development Environment](#development-environment)
- [Project Architecture](#project-architecture)
- [Building and Compilation](#building-and-compilation)
- [Testing Strategies](#testing-strategies)
- [Debugging Techniques](#debugging-techniques)
- [Performance Profiling](#performance-profiling)
- [Common Development Tasks](#common-development-tasks)
- [Domain-Driven Design Patterns](#domain-driven-design-patterns)
- [Troubleshooting](#troubleshooting)

---

## 🔧 Development Environment

### Recommended Tools

#### Editors and IDEs

**VS Code** (Recommended):
```bash
# Install extensions
code --install-extension rust-lang.rust-analyzer
code --install-extension vadimcn.vscode-lldb
code --install-extension tamasfe.even-better-toml
code --install-extension usernamehw.errorlens
```

**IntelliJ IDEA / RustRover**:
- Built-in Rust support
- Excellent debugging experience
- Integrated terminal

**Neovim / Vim**:
```lua
-- init.lua
require('rust-tools').setup({
  server = {
    on_attach = function(_, bufnr)
      -- LSP keybindings
    end
  }
})
```

### Development Configuration

**`.cargo/config.toml`**:
```toml
[build]
jobs = 4  # Parallel jobs

[target.x86_64-apple-darwin]
rustflags = ["-C", "link-arg=-undefined", "-C", "link-arg=dynamic_lookup"]

[target.aarch64-apple-darwin]
rustflags = ["-C", "link-arg=-undefined", "-C", "link-arg=dynamic_lookup"]

[profile.dev]
opt-level = 0      # No optimizations for faster compilation
debug = true
split-debuginfo = "unpacked"

[profile.dev.package."*"]
opt-level = 2      # Optimize dependencies

[profile.test]
opt-level = 2      # Faster tests
debug = true

[profile.bench]
inherits = "release"
debug = true
```

### Shell Aliases

Add to your `~/.bashrc` or `~/.zshrc`:

```bash
# Cargo aliases
alias cb='cargo build'
alias cbr='cargo build --release'
alias ct='cargo test'
alias ctw='cargo test --workspace'
alias cc='cargo check'
alias cf='cargo fmt'
alias cfc='cargo fmt -- --check'
alias cl='cargo clippy -- -D warnings'

# Workspace-specific
alias cb-core='cargo build -p vm-core'
alias cb-engine='cargo build -p vm-engine'
alias cb-jit='cargo build -p vm-engine-jit'

# Testing aliases
alias ct-core='cargo test -p vm-core'
alias ct-mem='cargo test -p vm-mem'
alias ct-gc='cargo test -p vm-core gc::'

# Development aliases
alias dev-verify='bash scripts/verify_zero_warnings.sh'
alias dev-clean='cargo clean && cargo build --workspace'
alias dev-test='cargo test --workspace --all-features'
```

---

## 🏗️ Project Architecture

### Layer Overview

```
┌─────────────────────────────────────────────────────────────┐
│                   Application Layer                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │ vm-cli       │  │ vm-desktop   │  │ vm-service   │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    Domain Layer                              │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  vm-core (Domain Services & Aggregates)             │    │
│  │  - GC Management                                    │    │
│  │  - TLB Management                                   │    │
│  │  - Translation Strategy                             │    │
│  │  - Resource Management                              │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                  Infrastructure Layer                        │
│  ┌───────────┐ ┌──────────┐ ┌─────────┐ ┌─────────────┐    │
│  │vm-engine  │ │ vm-mem   │ │vm-accel │ │ vm-device   │    │
│  │(JIT/Int)  │ │(MMU/TLB) │ │(KVM/HVF)│ │(VirtIO)     │    │
│  └───────────┘ └──────────┘ └─────────┘ └─────────────┘    │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                   Interface Layer                            │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  vm-frontend (Instruction Decoders)                   │   │
│  │  - RISC-V 64-bit                                      │   │
│  │  - ARM64                                              │   │
│  │  - x86_64                                             │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### Crate Responsibilities

**vm-core**: Domain logic
```rust
// Domain service example
pub struct GcManagementService {
    config: GcConfig,
    strategy: Arc<dyn GcStrategy>,
}

impl GcManagementService {
    pub fn manage_gc_cycle(&mut self) -> Result<GcStats, GcError> {
        // Domain logic for GC cycle management
    }
}
```

**vm-engine**: Execution engines
```rust
// JIT compiler implementation
pub struct JITEngine {
    pub backend: Box<dyn CodegenBackend>,
    pub cache: TranslationCache,
}

impl JITEngine for JITEngine {
    fn compile_block(&mut self, block: &BasicBlock) -> Result<JitCode, CompileError> {
        // JIT compilation logic
    }
}
```

**vm-mem**: Memory subsystem
```rust
// MMU implementation
pub struct MMU {
    pub tlb: Arc<TLB>,
    pub page_table: PageTable,
}

impl MMU {
    pub fn translate(&mut self, vaddr: u64) -> Result<u64, MMUError> {
        // Address translation logic
    }
}
```

---

## 🔨 Building and Compilation

### Build Profiles

**Development Build** (Fastest compilation):
```bash
cargo build --workspace
# Debug symbols, no optimizations
```

**Check-Only Build** (Fastest feedback):
```bash
cargo check --workspace
# Type checking only, no code generation
```

**Release Build** (Best performance):
```bash
cargo build --workspace --release
# Full optimizations, no debug symbols
```

**Custom Profile** (Debug + Optimized):
```bash
# Add to Cargo.toml:
[profile.dev-opt]
inherits = "dev"
opt-level = 2

# Build:
cargo build --workspace --profile dev-opt
```

### Incremental Compilation

Cargo automatically uses incremental compilation:

```bash
# First build: slower
time cargo build --workspace  # ~30s

# Subsequent builds: faster
time cargo build --workspace  # ~5s (after small change)
```

**Limit cache size** (if disk space is low):
```bash
# In .cargo/config.toml
[build]
incremental = true
```

### Compiler Explainer

Understand compiler optimizations:

```bash
# Show LLVM IR
cargo rustc --release -- --emit=llvm-ir

# Show assembly
cargo rustc --release -- --emit=asm

# Show time spent in each crate
cargo build --workspace --timings
```

---

## 🧪 Testing Strategies

### Test Organization

```
vm-core/
├── src/
│   ├── gc.rs                    # Implementation
│   └── lib.rs
│       └── #[cfg(test)]         # Unit tests
├── tests/
│   ├── gc_integration_tests.rs  # Integration tests
│   └── property_tests.rs        # Property-based tests
└── benches/
    └── gc_bench.rs              # Benchmarks
```

### Unit Tests

Test individual functions and methods:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocation() {
        let mut gc = Gc::new(1024);
        let ptr = gc.allocate(512).unwrap();
        assert!(!ptr.is_null());
    }

    #[test]
    fn test_allocation_fails() {
        let mut gc = Gc::new(512);
        let result = gc.allocate(1024);
        assert_matches!(result, Err(GcError::OutOfMemory));
    }

    // Parameterized test
    #[test]
    fn test_various_sizes() {
        for size in &[1, 8, 64, 512, 1024] {
            let mut gc = Gc::new(2048);
            let ptr = gc.allocate(*size);
            assert!(ptr.is_ok(), "Failed for size {}", size);
        }
    }
}
```

### Integration Tests

Test module interactions:

```rust
// tests/gc_integration_tests.rs

use vm_core::gc::{Gc, UnifiedGc};
use vm_core::mmu::MMU;

#[test]
fn test_gc_with_mmu_integration() {
    let mut mmu = MMU::new();
    let mut gc = UnifiedGc::new(1024 * 1024);

    // Allocate memory through MMU
    let addr = mmu.alloc(1024).unwrap();

    // Trigger GC
    gc.collect();

    // Verify MMU state
    assert!(mmu.is_valid(addr));
}
```

### Property-Based Testing

Use `proptest` for exhaustive testing:

```rust
// Add to Cargo.toml:
[dev-dependencies]
proptest = "1.0"

// In tests:
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_allocation_roundtrip(size in 1usize..1024) {
        let mut gc = Gc::new(2048);
        let ptr = gc.allocate(size).unwrap();

        // Write and read back
        unsafe {
            *ptr = 0xAB_u8;
            assert_eq!(*ptr, 0xAB);
        }

        gc.deallocate(ptr);
    }
}
```

### Running Tests

```bash
# Run all tests
cargo test --workspace

# Run specific test
cargo test test_allocation

# Run tests in a crate
cargo test -p vm-core

# Run tests with output
cargo test -- --nocapture

# Run tests with logging
RUST_LOG=debug cargo test --workspace

# Run tests in parallel
cargo test -- --test-threads=4

# Run specific test module
cargo test gc::tests::test_gc_allocation

# Run ignored tests
cargo test -- --ignored
```

### Test Utilities

**Custom test helpers**:

```rust
// tests/common/mod.rs
pub struct TestGc {
    gc: UnifiedGc,
}

impl TestGc {
    pub fn new() -> Self {
        Self {
            gc: UnifiedGc::new(1024 * 1024),
        }
    }

    pub fn allocate(&mut self, size: usize) -> *mut u8 {
        self.gc.allocate(size).unwrap()
    }

    pub fn assert_allocated(&self, ptr: *mut u8) {
        assert!(!ptr.is_null(), "Allocation failed");
    }
}

// Use in tests:
use tests::common::TestGc;

#[test]
fn test_with_helper() {
    let mut test_gc = TestGc::new();
    let ptr = test_gc.allocate(512);
    test_gc.assert_allocated(ptr);
}
```

---

## 🐛 Debugging Techniques

### Logging

**Structured logging with tracing**:

```rust
use tracing::{info, debug, error, instrument};

#[instrument]
pub fn allocate(&mut self, size: usize) -> Result<*mut u8, GcError> {
    info!("Allocating {} bytes", size);
    debug!("Heap usage: {} / {}", self.used, self.heap_size);

    if size > self.available() {
        error!("Out of memory: requested {}, available {}", size, self.available());
        return Err(GcError::OutOfMemory);
    }

    let ptr = self.allocate_internal(size);
    info!("Allocated at {:p}", ptr);
    Ok(ptr)
}
```

**Enable logging**:

```bash
# Set log level
RUST_LOG=debug cargo test

# Filter by module
RUST_LOG=vm_core::gc=debug cargo test

# Filter by crate
RUST_LOG=vm_core=trace cargo run
```

### GDB/LLDB Debugging

**Prepare debug build**:

```bash
# Build with debug symbols
cargo build

# Run with debugger
lldb target/debug/vm-cli

# LLDB commands:
(lldb) breakpoint set --name main
(lldb) breakpoint set --file gc.rs --line 123
(lldb) run
(lldb) bt           # Backtrace
(lldb) frame select 0
(lldb) print size
(lldb) print *ptr
(lldb) continue
```

### Rust-Specific Debugging

**Print with `Debug` trait**:

```rust
#[derive(Debug)]
pub struct GcStats {
    collections: usize,
    bytes_collected: usize,
}

println!("{:?}", stats);
```

**Custom debug output**:

```rust
impl std::fmt::Debug for GcStats {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "GcStats {{ collections: {}, bytes: {} }}",
            self.collections, self.bytes_collected
        )
    }
}
```

### Memory Debugging

**Valgrind** (Linux):

```bash
# Install Valgrind
sudo apt-get install valgrind

# Run with Valgrind
valgrind --leak-check=full \
         --show-leak-kinds=all \
         --track-origins=yes \
         target/debug/vm-cli
```

**Address Sanitizer**:

```bash
# Run with ASAN
RUSTFLAGS="-Z sanitizer=address" cargo run

# With leak detection
RUSTFLAGS="-Z sanitizer=address -Z sanitizer=leak" cargo run
```

### Performance Debugging

**Flamegraphs**:

```bash
# Install flamegraph
cargo install flamegraph

# Generate flamegraph
cargo flamegraph --bin vm-cli

# Open result
open flamegraph.svg
```

**perf** (Linux):

```bash
# Record performance
perf record -g ./target/release/vm-cli

# Analyze
perf report

# Annotate source
perf annotate
```

---

## ⚡ Performance Profiling

### CPU Profiling

**Using flamegraph**:

```bash
# Install
cargo install flamegraph

# Profile
cargo flamegraph --bin vm-cli -- --workload

# Result: flamegraph.svg
```

**Using perf** (Linux):

```bash
# Record
perf record -F 99 -g -- ./target/release/vm-cli

# Report
perf report

# Annotate
perf annotate vm_core::gc::allocate
```

### Memory Profiling

**Using heaptrack** (Linux):

```bash
# Install heaptrack
sudo apt-get install heaptrack

# Profile
heaptrack ./target/release/vm-cli

# Analyze
heaptrack_print heaptrack.$PID.gz
```

**Using dhat**:

```bash
# Add to Cargo.toml:
[profile.dhat]
inherits = "release"
debug = 1

# Build and run
cargo build --profile dhat
cargo test --profile dhat

# Analyze heap
```

### Benchmarking

**Criterion** (recommended):

```rust
// benches/gc_bench.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_allocation(c: &mut Criterion) {
    c.bench_function("allocate", |b| {
        let mut gc = Gc::new(1024 * 1024);
        b.iter(|| {
            black_box(gc.allocate(1024).unwrap())
        })
    });
}

criterion_group!(benches, benchmark_allocation);
criterion_main!(benches);
```

**Run benchmarks**:

```bash
# Run all benchmarks
cargo bench --workspace

# Run specific benchmark
cargo bench --bench gc_bench

# Compare against baseline
cargo bench -- --save-baseline main
cargo bench -- --baseline main

# Generate plots
cargo bench -- --output-format bencher | tee bench.txt
```

---

## 🛠️ Common Development Tasks

### Adding a New Crate

```bash
# Create new crate
cargo new --lib vm-new-thing

# Add to workspace
# Edit Cargo.toml:
[workspace.members]
    # ...
    vm-new-thing

# Add dependency
cargo add -p vm-core --path vm-new-thing
```

### Refactoring Code

**Extract function**:
```rust
// Before
pub fn complex_function(&mut self) {
    // ... 50 lines of code
}

// After
pub fn complex_function(&mut self) {
    self.step1();
    self.step2();
    self.step3();
}

fn step1(&mut self) {
    // Extracted logic
}
```

**Extract module**:
```rust
// Before: lib.rs (500 lines)
// After:
mod gc;
mod mmu;
mod tlb;

// gc/mod.rs
pub mod unified;
pub mod generational;
```

### Adding Dependencies

```bash
# Add runtime dependency
cargo add serde

# Add dev dependency
cargo add --dev proptest

# Add specific version
cargo add tokio --features "full" --version "1.49"

# Add workspace dependency
cargo add --workspace log
```

### Updating Dependencies

```bash
# Update all dependencies
cargo update

# Update specific dependency
cargo update -p tokio

# Check for security vulnerabilities
cargo audit
```

---

## 🎨 Domain-Driven Design Patterns

### Repository Pattern

```rust
// src/repository.rs
pub trait EventStore: Send + Sync {
    fn append(&self, event: DomainEvent) -> Result<(), StoreError>;
    fn read(&self, id: &AggregateId) -> Result<Vec<DomainEvent>, StoreError>;
}

pub struct PostgresEventStore {
    pool: PgPool,
}

impl EventStore for PostgresEventStore {
    fn append(&self, event: DomainEvent) -> Result<(), StoreError> {
        // Implementation
    }

    fn read(&self, id: &AggregateId) -> Result<Vec<DomainEvent>, StoreError> {
        // Implementation
    }
}
```

### Domain Service Pattern

```rust
// src/domain_services/gc_management_service.rs
pub struct GcManagementService {
    event_bus: Arc<dyn EventBus>,
    repository: Arc<dyn EventStore>,
}

impl GcManagementService {
    pub fn manage_gc_cycle(&mut self) -> Result<GcStats, GcError> {
        // Complex operation spanning multiple aggregates

        // 1. Check if GC needed
        if !self.should_collect() {
            return Ok(GcStats::default());
        }

        // 2. Trigger GC
        let stats = self.trigger_gc()?;

        // 3. Publish event
        self.event_bus.publish(DomainEvent::GcCompleted(stats));

        // 4. Save to store
        self.repository.append(DomainEvent::GcCompleted(stats))?;

        Ok(stats)
    }
}
```

### Aggregate Root Pattern

```rust
// src/aggregate_root.rs
pub struct VirtualMachine {
    id: VmId,
    state: VmState,
    events: Vec<DomainEvent>,
}

impl VirtualMachine {
    pub fn new(config: VmConfig) -> Self {
        Self {
            id: VmId::new(),
            state: VmState::Stopped,
            events: vec![DomainEvent::VmCreated],
        }
    }

    pub fn start(&mut self) -> Result<(), VmError> {
        if self.state != VmState::Stopped {
            return Err(VmError::InvalidState);
        }

        self.state = VmState::Running;
        self.events.push(DomainEvent::VmStarted);
        Ok(())
    }

    pub fn take_events(&mut self) -> Vec<DomainEvent> {
        std::mem::take(&mut self.events)
    }
}
```

---

## 🔍 Troubleshooting

### Common Build Issues

**"error: linker `cc` not found"**:
```bash
# Ubuntu/Debian
sudo apt-get install build-essential

# macOS
xcode-select --install

# Fedora
sudo dnf install gcc
```

**"out of memory" during build**:
```bash
# Reduce parallel jobs
CARGO_BUILD_JOBS=2 cargo build --workspace

# Or in .cargo/config.toml:
[build]
jobs = 2
```

### Common Test Issues

**Tests timeout**:
```bash
# Increase test timeout
cargo test -- --test-threads=1

# Or add timeout to specific test
#[test]
#[timeout(10000)] // 10 seconds
fn test_slow_operation() {
    // ...
}
```

**Flaky tests**:
```bash
# Run tests multiple times
cargo test -- --test-threads=1 --repeat 10

# Identify flaky tests
cargo test -- --nocapture -- --exact test_flaky
```

### Common Runtime Issues

**Segfaults**:
```bash
# Run with address sanitizer
RUSTFLAGS="-Z sanitizer=address" cargo run

# Use GDB
lldb target/debug/vm-cli
(lldb) run
(lldb) bt  # Get backtrace
```

**Deadlocks**:
```bash
# Run with thread sanitization
RUSTFLAGS="-Z sanitizer=thread" cargo run

# Check for deadlocks
# (Deadlock detection requires manual inspection)
```

---

## 📚 Additional Resources

### Internal Documentation

- [README.md](README.md) - Project overview
- [CONTRIBUTING.md](CONTRIBUTING.md) - Contribution guidelines
- [Architecture](docs/DDD_ARCHITECTURE_CLARIFICATION.md) - DDD architecture
- [Quick Start](docs/QUICK_START.md) - Get started quickly

### External Resources

- [The Rust Book](https://doc.rust-lang.org/book/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Performance Book](https://nnethercote.github.io/perf-book/)

---

**Happy coding!** 🚀

*Last Updated: 2025-01-03*
*Maintained By: VM Development Team*
