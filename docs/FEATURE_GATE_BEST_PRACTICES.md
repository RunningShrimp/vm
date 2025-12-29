# Feature Gate Best Practices Guide

**Purpose**: Guide for using feature gates effectively in the VM codebase
**Status**: Active
**Last Updated**: 2025-12-28

---

## Quick Reference

### When to Use Feature Gates

✅ **Use feature gates for:**
- Entirely different implementations (e.g., interpreter vs JIT)
- Platform-specific code (e.g., KVM on Linux)
- Optional dependencies (e.g., performance profiling)
- Experimental features (e.g., new debugging capabilities)

❌ **Do NOT use feature gates for:**
- Configuration options (use runtime config instead)
- Minor optimizations (use profile-guided optimization)
- Conditional error messages (use log levels)
- Temporary debugging (use debug assertions)

---

## Core Principles

### 1. Module-Level Gating (Preferred)

**❌ Bad: Method-level gating**
```rust
impl VmService {
    #[cfg(feature = "async")]
    pub async fn load_kernel(&self, path: &str) -> Result<()> { ... }

    #[cfg(feature = "async")]
    pub async fn create_snapshot(&self) -> Result<Snapshot> { ... }

    #[cfg(feature = "async")]
    pub async fn restore_snapshot(&self, snap: Snapshot) -> Result<()> { ... }
}
```

**✅ Good: Module-level gating**
```rust
// vm_service.rs
#[cfg(feature = "async")]
pub mod async_ops {
    use super::*;

    impl VmService {
        pub async fn load_kernel(&self, path: &str) -> Result<()> { ... }
        pub async fn create_snapshot(&self) -> Result<Snapshot> { ... }
        pub async fn restore_snapshot(&self, snap: Snapshot) -> Result<()> { ... }
    }
}
```

**Benefits:**
- Single gate instead of N gates
- Clearer code organization
- Easier to test
- Better compile times

---

### 2. Trait Abstraction Over Conditional Compilation

**❌ Bad: Feature-gated implementations**
```rust
#[cfg(feature = "jit")]
pub fn execute_jit(&self, code: &[u8]) -> Result<()> { ... }

#[cfg(feature = "interpreter")]
pub fn execute_interpreter(&self, code: &[u8]) -> Result<()> { ... }
```

**✅ Good: Trait-based design**
```rust
pub trait ExecutionEngine {
    fn execute(&self, code: &[u8]) -> Result<()>;
}

#[cfg(feature = "jit")]
pub struct JitEngine { ... }

#[cfg(feature = "interpreter")]
pub struct Interpreter { ... }
```

**Benefits:**
- Runtime flexibility
- Easier to test
- Better abstraction
- Future-proof

---

### 3. Build-Time vs Runtime Configuration

**❌ Bad: Compile-time options**
```rust
#[cfg(feature = "high-performance")]
const CACHE_SIZE: usize = 1024;

#[cfg(not(feature = "high-performance"))]
const CACHE_SIZE: usize = 128;
```

**✅ Good: Runtime configuration**
```rust
const DEFAULT_CACHE_SIZE: usize = 1024;

pub struct Config {
    pub cache_size: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cache_size: std::env::var("CACHE_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_CACHE_SIZE),
        }
    }
}
```

**Benefits:**
- No need for multiple binaries
- Easier to tune
- Better user experience
- Fewer feature combinations to test

---

### 4. Platform Detection

**❌ Bad: Feature gates for platforms**
```rust
#[cfg(feature = "linux")]
fn platform_init() { ... }

#[cfg(feature = "windows")]
fn platform_init() { ... }
```

**✅ Good: Use `cfg(target_os)`**
```rust
#[cfg(target_os = "linux")]
fn platform_init() { ... }

#[cfg(target_os = "windows")]
fn platform_init() { ... }

#[cfg(not(any(target_os = "linux", target_os = "windows"))]
fn platform_init() { ... }
```

**Benefits:**
- No need for feature flags
- Standard Rust practice
- Better compiler optimization
- Fewer feature combinations

---

## Feature Organization

### Recommended File Structure

```
vm-component/
├── Cargo.toml              # Feature definitions
├── src/
│   ├── lib.rs              # Public API (minimal gates)
│   ├── core.rs             # Core functionality (0 gates)
│   ├── opt/
│   │   ├── mod.rs          # Optional features module
│   │   ├── performance.rs  # Performance features (1 gate)
│   │   ├── debug.rs        # Debug features (1 gate)
│   │   └── async.rs        # Async features (1 gate)
│   └── platform/
│       ├── linux.rs        # Linux-specific (target_os gate)
│       └── macos.rs        # macOS-specific (target_os gate)
```

### Cargo.toml Feature Definitions

**❌ Bad: Overlapping features**
```toml
[features]
default = []
async = []
performance = []
async-perf = ["async", "performance"]
full = ["async", "performance", "async-perf"]
```

**✅ Good: Clear, independent features**
```toml
[features]
default = ["std"]

# Core features
std = ["dep:parking_lot"]
async = ["dep:tokio"]
performance = ["dep:rayon"]

# Platform-specific
kvm = ["dep:kvm", "platform/linux"]
hvf = ["dep:apple-hvf", "platform/macos"]

# Optional components
debugger = ["dep:gdb-protocol"]
profiler = ["dep:flamegraph"]

# Composite features (for convenience)
all-features = ["async", "performance", "debugger", "profiler"]
```

---

## Common Patterns

### Pattern 1: Conditional Struct Fields

**❌ Bad: Multiple field-level gates**
```rust
pub struct VmService {
    #[cfg(feature = "performance")]
    perf_stats: PerformanceStats,

    #[cfg(feature = "performance")]
    jit_engine: Option<JitEngine>,

    #[cfg(feature = "performance")]
    profiler: Profiler,

    #[cfg(feature = "async")]
    async_runtime: AsyncRuntime,
}
```

**✅ Good: Grouped in sub-struct**
```rust
#[cfg(feature = "performance")]
pub struct PerformanceContext {
    stats: PerformanceStats,
    jit_engine: Option<JitEngine>,
    profiler: Profiler,
}

#[cfg(feature = "async")]
pub struct AsyncContext {
    runtime: AsyncRuntime,
}

pub struct VmService {
    #[cfg(feature = "performance")]
    perf: PerformanceContext,

    #[cfg(feature = "async")]
    async_ctx: AsyncContext,
}
```

---

### Pattern 2: Conditional Module Exports

**❌ Bad: Conditional re-exports**
```rust
#[cfg(feature = "performance")]
pub use performance::PerformanceStats;

#[cfg(feature = "debug")]
pub use debug::Debugger;

#[cfg(feature = "async")]
pub use async_runtime::AsyncExecutor;
```

**✅ Good: Feature-specific modules**
```rust
// lib.rs
#[cfg(feature = "performance")]
pub mod performance;

#[cfg(feature = "debug")]
pub mod debug;

#[cfg(feature = "async")]
pub mod async_runtime;

// Users explicitly import what they need
// use vm_component::performance::PerformanceStats;
```

---

### Pattern 3: Conditional Trait Implementations

**❌ Bad: Duplicated impl blocks**
```rust
impl VmService {
    pub fn basic_method(&self) { ... }
}

#[cfg(feature = "async")]
impl VmService {
    pub async fn async_method(&self) { ... }
}

#[cfg(feature = "performance")]
impl VmService {
    pub fn perf_method(&self) { ... }
}
```

**✅ Good: Feature-specific traits**
```rust
pub trait VmServiceCore {
    fn basic_method(&self);
}

#[cfg(feature = "async")]
pub trait VmServiceAsync {
    async fn async_method(&self);
}

#[cfg(feature = "performance")]
pub trait VmServicePerf {
    fn perf_method(&self);
}
```

---

## Testing Feature Gates

### Feature Matrix Testing

**Include in CI/CD:**
```bash
#!/bin/bash
# test_all_features.sh

# Test no default features
cargo test --no-default-features

# Test each feature individually
for feature in async performance debug kvm; do
    cargo test --no-default-features --features $feature
done

# Test feature combinations
cargo test --features "async,performance"
cargo test --features "async,debug"
cargo test --features "performance,debug,kvm"

# Test all features
cargo test --all-features
```

### Compile-Time Checking

```bash
# Check which files use feature gates
grep -r "#\[cfg(feature" --include="*.rs" | wc -l

# Find files with too many gates
grep -r "#\[cfg(feature" --include="*.rs" | cut -d: -f1 | \
    sort | uniq -c | sort -rn | \
    awk '$1 > 10 {print $0}'
```

---

## Migration Guide

### Converting Existing Code

**Step 1: Identify Scope**
```rust
// Find all related gates
grep "#\[cfg(feature = \"X\"\)" file.rs
```

**Step 2: Create Module**
```rust
// Create file_x.rs
#[cfg(feature = "X")]
pub mod file_x {
    // Move all gated code here
}
```

**Step 3: Update Main File**
```rust
// Replace scattered gates with single module import
#[cfg(feature = "X")]
pub mod file_x;
```

**Step 4: Test**
```bash
cargo test --features "X"
cargo test --no-default-features
```

---

## Metrics and Goals

### Feature Gate Health Metrics

**Current Targets:**
- Total feature gates: <150 (currently 254)
- Files with >8 gates: <5 (currently 14)
- Average gates per file: <4 (currently 5.52)

**Per-File Limits:**
- 0-3 gates: Excellent ✅
- 4-7 gates: Good ⚠️
- 8-10 gates: Needs review ⚠️
- 10+ gates: Must refactor ❌

### Review Process

**Before Adding New Feature Gates:**
1. Can this be runtime configuration?
2. Can existing features be used instead?
3. Is this a temporary or permanent feature?
4. What's the maintenance cost?

**Feature Gate Addition Checklist:**
- [ ] Feature documented in Cargo.toml
- [ ] Feature tested in CI
- [ ] Feature added to feature matrix
- [ ] Documentation updated
- [ ] Code review approved

---

## Examples from Codebase

### Excellent Examples

**vm-mem/src/async_mmu.rs** (24 → 1 gate)
```rust
#[cfg(feature = "async")]
pub mod async_mmu {
    // All async MMU code here
    // No internal feature gates
}
```

**vm-mem/src/tlb/unified_tlb.rs** (13 → 6 gates)
```rust
pub mod tlb {
    // Core TLB (0 gates)

    #[cfg(feature = "async")]
    pub mod async_ops {
        // Async TLB operations (1 gate)
    }

    #[cfg(feature = "performance")]
    pub mod optimization {
        // Performance optimizations (1 gate)
    }
}
```

### Needs Improvement Examples

**vm-service/src/vm_service.rs** (23 gates → target 8)
- Issue: Scattered method-level gates
- Plan: Extract to performance_mod.rs and smmu_mod.rs

**vm-service/src/vm_service/execution.rs** (21 gates → target 5)
- Issue: Multiple nested gated modules
- Plan: Extract JIT and coroutine to separate files

---

## Additional Resources

### Internal Documentation
- [FEATURE_GATE_PROGRESS.md](../FEATURE_GATE_PROGRESS.md) - Current status and statistics
- [FEATURE_GATE_OPTIMIZATION_ROADMAP.md](../FEATURE_GATE_OPTIMIZATION_ROADMAP.md) - Detailed optimization plan
- [COMPREHENSIVE_ARCHITECTURE_REVIEW_2025-12-28.md](../COMPREHENSIVE_ARCHITECTURE_REVIEW_2025-12-28.md) - Original architecture review

### External Resources
- [Rust Reference: Conditional Compilation](https://doc.rust-lang.org/reference/conditional-compilation.html)
- [Cargo Features: The Guide](https://doc.rust-lang.org/cargo/reference/features.html)
- [API Guidelines: Feature Gated APIs](https://rust-lang.github.io/api-guidelines/feature-gated.html)

---

## FAQ

**Q: When should I add a new feature gate?**
A: Only when you need to compile different implementations. Use runtime config for options.

**Q: Can I remove feature gates?**
A: Yes! If a feature is stable or always used, consider removing the gate and making it default.

**Q: What's the cost of feature gates?**
A: Increased build time, more test combinations, higher cognitive load. Minimize where possible.

**Q: How many feature gates are too many?**
A: Generally, >10 gates in a single file indicates need for refactoring. Target <5 per file.

**Q: Should I use features or target_os for platform code?**
A: Use `target_os`/`target_arch` for platform detection. Use features for optional functionality.

---

**Document Version**: 1.0
**Maintainer**: Architecture Team
**Feedback**: Create issue or PR for suggestions
