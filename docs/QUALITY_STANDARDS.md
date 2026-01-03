# Quality Standards for VM Project

This document defines the quality gates and standards enforced by the CI/CD pipeline to ensure code quality, security, and maintainability.

## Table of Contents

- [Overview](#overview)
- [Quality Gates](#quality-gates)
- [Code Style Requirements](#code-style-requirements)
- [Testing Requirements](#testing-requirements)
- [Documentation Requirements](#documentation-requirements)
- [Security Requirements](#security-requirements)
- [Performance Requirements](#performance-requirements)
- [Local Development](#local-development)
- [CI/CD Pipeline](#cicd-pipeline)

---

## Overview

The VM project enforces strict quality gates through automated CI/CD pipelines. All code must pass these gates before being merged into the main branch.

### Quality Gate Levels

1. **Required** - Must pass to merge
2. **Warning** - Fails but doesn't block merge (with justification)
3. **Informational** - For tracking and awareness only

---

## Quality Gates

### 1. Code Formatting (Required)

**Tool**: `rustfmt`

**Configuration**: `.rustfmt.toml`

**Check Command**:
```bash
cargo fmt --all -- --check
```

**Requirements**:
- All code must be formatted with rustfmt
- Maximum line width: 100 characters
- 4 spaces for indentation
- Unix line endings
- Imports grouped: std, external crates, local crates

**How to Fix**:
```bash
cargo fmt
git add -A
git commit -m "fix: format code"
```

**Enforcement**: CI will fail if formatting doesn't match. This is a **hard requirement**.

---

### 2. Clippy Linting (Required)

**Tool**: `clippy`

**Configuration**: `.clippy.toml`

**Check Command**:
```bash
cargo clippy --workspace --all-features --all-targets -- \
  -D warnings \
  -W clippy::all \
  -W clippy::pedantic \
  -W clippy::cargo \
  -W clippy::unwrap_used \
  -W clippy::expect_used \
  -W clippy::panic \
  -W clippy::unimplemented \
  -W clippy::todo \
  -W clippy::unreachable \
  -W clippy::indexing_slicing
```

**Requirements**:
- Zero warnings tolerated
- Pedantic checks enabled
- No `unwrap()` or `expect()` in production code
- No `panic!()`, `unimplemented!()`, or `todo!()` in production code
- Proper error handling with `Result` and `Option`

**Common Issues and Fixes**:

| Issue | Bad Practice | Good Practice |
|-------|-------------|---------------|
| Unwrap | `let x = val.unwrap()` | `let x = val?` or `let x = val.ok_or_else(...)?` |
| Expect | `val.expect("msg")` | `val.ok_or_else(|| Error::...)?` |
| Index | `arr[i]` | `arr.get(i).ok_or_else(...)?` |
| Clone | `val.clone()` | `&val` or `Arc::clone(&val)` |

**Allowing Lints**:

If a lint must be allowed, add an inline comment with justification:

```rust
#[allow(clippy::too_many_arguments)] // Needed for device configuration
pub fn configure_device(
    arg1: Type1,
    arg2: Type2,
    // ... many args
) -> Result<()> {
    // ...
}
```

**Enforcement**: CI will fail with any clippy warnings. This is a **hard requirement**.

---

### 3. Compilation (Required)

**Platforms Tested**:
- Ubuntu Linux (x86_64)
- macOS (x86_64 and ARM64)
- Windows (x86_64)

**Build Modes**:
- Debug (with all features)
- Release (with all features)

**Check Commands**:
```bash
# Debug build
cargo build --workspace --all-features

# Release build
cargo build --workspace --all-features --release
```

**Requirements**:
- Must compile without errors on all platforms
- Must compile with `--all-features`
- No compiler warnings (`RUSTFLAGS=-D warnings` enforced)
- MSRV: Rust 1.92+ (defined in `rust-toolchain.toml`)

**Enforcement**: CI will fail if compilation fails on any platform. This is a **hard requirement**.

---

### 4. Test Suite (Required)

**Platforms Tested**:
- Ubuntu Linux
- macOS
- Windows

**Test Modes**:
- Debug mode tests
- Release mode tests

**Check Command**:
```bash
# All tests
cargo test --workspace --all-features --verbose

# Specific test
cargo test --package vm-core --lib vm::tests::test_name

# With output
cargo test --workspace --all-features -- --nocapture --test-threads=1
```

**Requirements**:
- All tests must pass
- No `#[ignore]` tests without justification
- Test coverage must be maintained (see below)
- Each crate should have:
  - Unit tests in `src/` or `tests/` module
  - Integration tests in `tests/` directory
  - Property-based tests where applicable

**Test Organization**:
```
vm-core/
├── src/
│   └── lib.rs
│       └── #[cfg(test)]
│           └── mod tests { ... }
├── tests/
│   ├── integration_tests.rs
│   └── property_tests.rs
└── benches/
    └── performance_bench.rs
```

**Enforcement**: CI will fail if any test fails. This is a **hard requirement**.

---

### 5. Code Coverage (Required)

**Tool**: `cargo-llvm-cov`

**Minimum Threshold**: 50%

**Check Command**:
```bash
# Install
cargo install cargo-llvm-cov

# Generate report
cargo llvm-cov --workspace --all-features --html

# Summary only
cargo llvm-cov --workspace --all-features --summary
```

**Coverage Targets**:
- **Overall minimum**: 50%
- **Critical paths**: 90%+ (core execution logic)
- **Error handling**: 80%+
- **Public APIs**: 100%

**Exemptions**:
- Platform-specific code (counted separately)
- Test helper functions
- Benchmark code

**Viewing Reports**:
```bash
# HTML report
open target/llvm-cov/html/index.html

# Terminal summary
cargo llvm-cov --workspace --all-features --summary
```

**Enforcement**: CI will fail if overall coverage drops below 50%. This is a **hard requirement**.

---

### 6. Documentation (Required)

**Check Command**:
```bash
# Build docs
cargo doc --no-deps --workspace --all-features

# Check for broken links
cargo doc --no-deps --workspace --all-features 2>&1 | grep "broken"
```

**Requirements**:
- All public APIs must have documentation
- Documentation must build without errors
- No broken intra-documentation links
- Examples for complex APIs
- `#![warn(missing_docs)]` at crate level

**Documentation Template**:
```rust
/// Brief one-line description.
///
/// More detailed explanation if needed.
///
/// # Arguments
///
/// * `arg1` - Description of arg1
/// * `arg2` - Description of arg2
///
/// # Returns
///
/// Description of return value
///
/// # Errors
///
/// * `Error::Variant` - When this happens
///
/// # Examples
///
/// ```
/// use vm_core::Function;
///
/// let result = Function::new(arg1, arg2)?;
/// assert_eq!(result.value(), expected);
/// # Ok::<(), vm_core::Error>(())
/// ```
pub fn function(arg1: Type1, arg2: Type2) -> Result<Output> {
    // ...
}
```

**Enforcement**: CI will fail if documentation has errors or broken links. This is a **hard requirement**.

---

### 7. Security Audit (Warning)

**Tools**:
- `cargo-audit`: Vulnerability scanning
- `cargo-deny`: License and dependency checks

**Check Commands**:
```bash
# Install
cargo install cargo-audit cargo-deny

# Run audit
cargo audit

# Check licenses
cargo deny check licenses

# Check advisories
cargo deny check advisories
```

**Requirements**:
- No critical vulnerabilities
- No GPL-licensed dependencies (unless exception granted)
- Allowed licenses: MIT, Apache-2.0, BSD-3-Clause

**Configuration**: `deny.toml`

**Enforcement**: Security vulnerabilities are reported but don't block merge (warning only).

---

### 8. Dependency Management (Informational)

**Checks**:
- Duplicate dependencies detection
- Outdated dependency tracking
- Dependency tree analysis

**Check Command**:
```bash
# Check duplicates
cargo tree --duplicates

# Check outdated
cargo outdated --workspace

# Tree size
cargo tree | wc -l
```

**Requirements**:
- Minimize duplicate dependencies
- Keep dependencies up to date
- Prefer minimal, focused dependencies

**Enforcement**: Informational only, for awareness.

---

## Code Style Requirements

### Rust Edition

**Current Edition**: 2024

**Features Used**:
- Let chains
- Generic defaults improvements
- Enhanced capture rules

### Naming Conventions

| Type | Convention | Example |
|------|-----------|---------|
| Struct | `PascalCase` | `VirtualMachine` |
| Enum | `PascalCase` | `MemoryError` |
| Function | `snake_case` | `create_vm` |
| Variable | `snake_case` | `vm_count` |
| Constant | `SCREAMING_SNAKE_CASE` | `MAX_VCPUS` |
| Module | `snake_case` | `device_emulation` |
| Trait | `PascalCase` | `JITCompiler` |
| Macro | `snake_case!` | `vm_debug!` |

### Error Handling

**Use `Result` for fallible operations**:
```rust
pub fn read_memory(&self, addr: u64) -> Result<u64, MemoryError> {
    if addr >= self.size {
        return Err(MemoryError::OutOfBounds { addr, size: self.size });
    }
    Ok(self.memory[addr as usize])
}
```

**Prefer `?` operator**:
```rust
pub fn execute(&mut self) -> Result<(), ExecutionError> {
    self.fetch()?;
    self.decode()?;
    self.execute_instruction()?;
    Ok(())
}
```

**Define errors with `thiserror`**:
```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("out of bounds access: addr={addr}, size={size}")]
    OutOfBounds { addr: u64, size: u64 },

    #[error("alignment error: addr={addr} must be {required}-byte aligned")]
    AlignmentError { addr: u64, required: usize },
}
```

### Unsafe Code

**Guidelines**:
- Minimize use of `unsafe`
- Document safety invariants
- Wrap unsafe code in safe abstractions
- Review all unsafe code carefully

**Example**:
```rust
/// Safe wrapper for unsafe pointer access
///
/// # Safety
///
/// Caller must ensure:
/// - Pointer is properly aligned
/// - Pointer points to valid memory
/// - Memory remains valid for lifetime 'a
pub unsafe fn read_u32(ptr: *const u8) -> u32 {
    ptr.cast::<u32>().read_unaligned()
}
```

**Audit**: All unsafe code is tracked and reported in CI.

---

## Testing Requirements

### Test Types

#### 1. Unit Tests

**Location**: In each module's `#[cfg(test)]` module

**Purpose**: Test individual functions and methods

**Example**:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_allocation() {
        let mem = Memory::new(4096);
        assert_eq!(mem.size(), 4096);
    }

    #[test]
    fn test_memory_write_read() {
        let mut mem = Memory::new(4096);
        mem.write(0, 42).unwrap();
        assert_eq!(mem.read(0).unwrap(), 42);
    }

    #[test]
    fn test_out_of_bounds() {
        let mem = Memory::new(4096);
        assert!(matches!(
            mem.read(4096),
            Err(MemoryError::OutOfBounds { .. })
        ));
    }
}
```

#### 2. Integration Tests

**Location**: `tests/` directory in workspace root or crate root

**Purpose**: Test component interactions

**Example**:
```rust
// tests/integration_tests.rs
use vm_core::{VmConfig, VmId};
use vm_engine::ExecutionEngine;

#[test]
fn test_vm_lifecycle() {
    let config = VmConfig::default();
    let vm = VmId::new("test-vm");
    let mut engine = ExecutionEngine::new(vm, config).unwrap();

    assert!(engine.start().is_ok());
    assert!(engine.execute().is_ok());
    assert!(engine.stop().is_ok());
}
```

#### 3. Property-Based Tests

**Tool**: `proptest` crate

**Purpose**: Test invariants across many inputs

**Example**:
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_roundtrip(addr in 0u64..4096, value in 0u64..u64::MAX) {
        let mut mem = Memory::new(4096);
        mem.write(addr, value).unwrap();
        assert_eq!(mem.read(addr).unwrap(), value);
    }
}
```

#### 4. Benchmark Tests

**Tool**: `criterion` crate

**Location**: `benches/` directory

**Purpose**: Performance regression detection

**Example**:
```rust
// benches/memory_bench.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use vm_core::Memory;

fn bench_memory_read(c: &mut Criterion) {
    let mem = Memory::new(4096);
    c.bench_function("memory_read", |b| {
        b.iter(|| {
            black_box(mem.read(0))
        })
    });
}

criterion_group!(benches, bench_memory_read);
criterion_main!(benches);
```

### Test Quality Standards

- **No test should rely on external state** (filesystem, network, etc.)
- **Tests should be deterministic** (no randomness or fixed seed)
- **Tests should be fast** (unit tests < 100ms each)
- **Tests should be isolated** (no shared state between tests)
- **Use assertions appropriately**:
  - `assert!` for boolean conditions
  - `assert_eq!` for equality checks
  - `assert_matches!` for pattern matching
  - `assert!(!...)` for negative checks

---

## Documentation Requirements

### Code Documentation

**Required for**:
- All `pub` items (structs, enums, functions, modules, traits)
- Complex algorithms
- Safety invariants (for `unsafe` code)
- Platform-specific behavior

**Not required for**:
- Private items (unless complex)
- Trivial getters/setters
- Test code

### Architecture Documentation

**Required locations**:
- `docs/architecture/` - System architecture docs
- `docs/api/` - API documentation
- `README.md` in each crate - Crate overview
- `ARCHITECTURE.md` in complex crates

### Examples

**Provide examples for**:
- All public APIs
- Common use cases
- Error handling patterns
- Integration scenarios

---

## Security Requirements

### Dependency Security

**Tools**:
- `cargo-audit` - Check for known vulnerabilities
- `cargo-deny` - License and advisory checks

**Requirements**:
- No critical vulnerabilities
- No high-severity vulnerabilities without mitigation
- License compliance (MIT/Apache-2.0/BSD-3-Clause)

### Code Security

**Practices**:
- No hardcoded secrets or credentials
- Validate all external input
- Use constant-time comparisons for secrets
- Proper error handling (don't leak sensitive info)
- Memory safety (Rust provides this, but careful with `unsafe`)

### Unsafe Code Review

**Required for all `unsafe` blocks**:
- Safety documentation
- Invariant documentation
- Careful review before merge

---

## Performance Requirements

### Benchmarks

**Required for**:
- Hot paths (JIT compilation, memory operations)
- Algorithm changes
- Data structure changes
- Cross-cutting concerns (e.g., locking strategies)

**Performance Regressions**:
- >10% slower: Fail (must fix or justify)
- >5% slower: Warning (review required)
- >5% faster: Improvement

### Profiling

**Tools**:
- `flamegraph` - Flame graph generation
- `perf` - Linux profiling
- `Instruments` - macOS profiling
- `cargo-criterion` - Benchmark statistics

---

## Local Development

### Pre-commit Hook (Recommended)

Create `.git/hooks/pre-commit`:

```bash
#!/bin/bash
set -e

echo "Running pre-commit checks..."

# Format check
echo "Checking formatting..."
cargo fmt --all -- --check

# Clippy
echo "Running clippy..."
cargo clippy --workspace --all-features --all-targets -- -D warnings

# Quick tests
echo "Running tests..."
cargo test --workspace --lib

echo "All checks passed!"
```

Make it executable:
```bash
chmod +x .git/hooks/pre-commit
```

### Pre-push Hook (Optional)

Create `.git/hooks/pre-push`:

```bash
#!/bin/bash
set -e

echo "Running pre-push checks..."

# Full test suite
echo "Running full test suite..."
cargo test --workspace --all-features

# Coverage check (optional, requires cargo-llvm-cov)
if command -v cargo-llvm-cov &> /dev/null; then
    echo "Checking coverage..."
    cargo llvm-cov --workspace --all-features --summary
fi

echo "All checks passed!"
```

### Local CI Reproduction

Run all CI checks locally:

```bash
# Format
cargo fmt --all -- --check

# Clippy
cargo clippy --workspace --all-features --all-targets -- -D warnings

# Build
cargo build --workspace --all-features

# Tests
cargo test --workspace --all-features

# Docs
cargo doc --no-deps --workspace --all-features

# Coverage (requires cargo-llvm-cov)
cargo llvm-cov --workspace --all-features --summary
```

---

## CI/CD Pipeline

### Workflow Files

1. **`quality-gates.yml`** - Main quality enforcement (this document)
2. **`ci.yml`** - Comprehensive CI pipeline
3. **`code-quality.yml`** - Additional quality checks
4. **`coverage.yml`** - Coverage reporting
5. **`performance.yml`** - Performance monitoring

### Trigger Events

Quality gates run on:
- Push to `master`, `main`, or `develop` branches
- Pull requests to `master`, `main`, or `develop`
- Manual workflow dispatch
- Merge group (before final merge)

### Required Gates for Merge

All of these must pass:
1. ✅ Format check
2. ✅ Clippy check (strict mode)
3. ✅ Compilation (all platforms)
4. ✅ Test suite (all platforms)
5. ✅ Documentation build
6. ✅ Coverage threshold (50%)

### Optional Gates

These don't block merge but are monitored:
- Security audit (warnings only)
- Unsafe code audit (informational)
- Dependency analysis (informational)
- Performance benchmarks (separate workflow)

### Checking CI Results

```bash
# Using GitHub CLI
gh pr checks

# View workflow runs
gh run list

# View specific run
gh run view <run-id>

# Watch logs in real-time
gh run watch
```

---

## Quality Metrics

### Code Health Indicators

Track these metrics over time:
- **Test coverage**: Percentage of code covered by tests
- **Clippy warnings**: Count of clippy warnings (should be 0)
- **Test failures**: Count of failing tests (should be 0)
- **Unsafe lines**: Count of unsafe lines (minimize)
- **Dependencies**: Total dependency count (minimize)
- **Build time**: Time to compile (optimize)

### Trend Analysis

CI tracks trends for:
- Coverage changes
- Test performance
- Build duration
- Dependency growth

---

## Troubleshooting

### Common Issues

#### Format Check Failed

**Solution**:
```bash
cargo fmt
git add -A
git commit -m "fix: format code"
```

#### Clippy Warnings

**Solution**:
```bash
# View warnings
cargo clippy --workspace --all-features --all-targets -- -D warnings

# Fix automatically where possible
cargo clippy --workspace --all-features --all-targets -- --fix
```

#### Test Failures

**Solution**:
```bash
# Run tests locally
cargo test --workspace --all-features --no-fail-fast -- --nocapture

# Run specific test
cargo test --package vm-core --lib tests::test_name

# Run with backtrace
RUST_BACKTRACE=1 cargo test --workspace
```

#### Coverage Below Threshold

**Solution**:
1. Identify uncovered code:
   ```bash
   cargo llvm-cov --workspace --all-features --html
   open target/llvm-cov/html/index.html
   ```
2. Add tests for uncovered paths
3. Re-run coverage check

#### Build Failures

**Solution**:
```bash
# Clean build
cargo clean
cargo build --workspace --all-features

# Check Rust version
rustc --version  # Should match rust-toolchain.toml

# Update toolchain
rustup update stable
```

---

## Resources

### Documentation

- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Effective Rust](https://doc.rust-lang.org/book/ch10-00-generics.html)
- [The Rustonomicon](https://doc.rust-lang.org/nomicon/)
- [Clippy Lints](https://rust-lang.github.io/rust-clippy/master/)

### Tools

- `rustfmt`: Code formatting
- `clippy`: Linting
- `cargo-llvm-cov`: Coverage
- `cargo-audit`: Security
- `cargo-deny`: Dependency checks
- `criterion`: Benchmarking

### Project-Specific

- [CONTRIBUTING.md](./development/CONTRIBUTING.md) - Contribution guide
- [CI_CD_GUIDE.md](./CI_CD_GUIDE.md) - CI/CD details
- [ARCHITECTURE.md](./architecture/) - Architecture docs
- [BENCHMARKING.md](./BENCHMARKING.md) - Performance guide

---

## Version History

- **v1.0.0** (2025-01-03): Initial quality standards definition
  - Establish 50% coverage threshold
  - Enforce strict clippy checks
  - Require documentation for all public APIs
  - Define quality gate process

---

For questions or suggestions about quality standards, please open an issue or PR.
