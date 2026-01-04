# Contributing to Rust VM

Thank you for your interest in contributing to the Rust VM project! This document provides guidelines and instructions for contributing effectively.

---

## 📋 Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Workflow](#development-workflow)
- [Coding Standards](#coding-standards)
- [Testing Guidelines](#testing-guidelines)
- [Commit Conventions](#commit-conventions)
- [Pull Request Process](#pull-request-process)
- [Developer Certificate of Origin](#developer-certificate-of-origin)

---

## 🤝 Code of Conduct

We are committed to providing a welcoming and inclusive environment. Please:

- Be respectful and considerate
- Use inclusive language
- Focus on constructive feedback
- Help others learn and grow

If you encounter any issues, please contact the maintainers.

---

## 🚀 Getting Started

### Prerequisites

1. **Rust Toolchain**: Install Rust 1.92.0 or later
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   rustc --version  # Verify: 1.92.0 or later
   ```

2. **Development Tools**:
   - Git: Version control
   - Editor: VS Code, IntelliJ IDEA, or your preferred editor
   - (Optional) LLVM: For advanced JIT features

3. **Clone and Setup**:
   ```bash
   # Fork the repository on GitHub
   # Clone your fork
   git clone https://github.com/YOUR_USERNAME/vm.git
   cd vm

   # Add upstream remote
   git remote add upstream https://github.com/your-org/vm.git

   # Install hooks (if available)
   cp .githooks/pre-commit .git/hooks/pre-commit
   chmod +x .git/hooks/pre-commit
   ```

### First-Time Build

```bash
# Build the workspace
cargo build --workspace

# Run tests to verify setup
cargo test --workspace

# Run clippy to check code quality
cargo clippy --workspace -- -D warnings

# Format code
cargo fmt --all
```

### Recommended Development Environment

**VS Code Extensions**:
- `rust-analyzer`: Rust language server
- `CodeLLDB`: Debugging support
- `Even Better TOML`: Cargo.toml syntax
- `Error Lens`: Inline error display

**IntelliJ IDEA**:
- Rust plugin (built-in)
- Excellent debugging integration
- Built-in terminal

---

## 🔄 Development Workflow

### 1. Choose an Issue

Browse [GitHub Issues](https://github.com/your-org/vm/issues) and find something to work on:

- **Good First Issues**: Labeled `good-first-issue` - start here!
- **Help Wanted**: Labeled `help-wanted` - needs contributors
- **Priority Tasks**: Labeled `P1`, `P2`, `P3` - by importance

### 2. Create a Branch

```bash
# Update your master branch
git checkout master
git fetch upstream
git rebase upstream/master

# Create a feature branch
git checkout -b feat/your-feature-name

# Or a fix branch
git checkout -b fix/issue-description

# Branch naming conventions:
# feat/     - New features
# fix/      - Bug fixes
# docs/     - Documentation changes
# test/     - Test additions or changes
# refactor/ - Code refactoring
# perf/     - Performance improvements
# chore/    - Maintenance tasks
```

### 3. Make Changes

```bash
# Edit code
vim vm-core/src/gc/unified.rs

# Incremental compilation (faster)
cargo check -p vm-core

# Run affected tests
cargo test -p vm-core gc::

# Format code
cargo fmt

# Check for issues
cargo clippy -p vm-core -- -D warnings
```

### 4. Commit Changes

```bash
# Stage files
git add vm-core/src/gc/unified.rs
git add tests/test_gc.rs

# Commit with conventional commit message
git commit -m "feat: implement parallel GC sweeping

- Add parallel sweeping algorithm
- Implement work stealing for load balancing
- Add thread-safe heap marking
- Include 5 unit tests

Closes #123"
```

### 5. Push and Create PR

```bash
# Push to your fork
git push origin feat/parallel-gc-sweeping

# Create Pull Request on GitHub
# Or use gh CLI:
gh pr create --title "feat: implement parallel GC sweeping" \
             --body "See description below"
```

---

## 📐 Coding Standards

### Rust Style Guide

We follow the official [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/):

**1. Code Formatting**:
```bash
# Always format your code
cargo fmt --all

# Check formatting before committing
cargo fmt -- --check
```

**2. Linting**:
```bash
# Run clippy with strict checks
cargo clippy --workspace --all-targets -- -D warnings -W clippy::all -W clippy::pedantic

# Fix auto-fixable issues
cargo clippy --fix --allow-dirty -- -D warnings
```

**3. Common Patterns**:

✅ **DO**:
```rust
// Use Result for error handling
pub fn allocate(&mut self, size: usize) -> Result<*mut u8, GcError> {
    if size > self.heap_size {
        return Err(GcError::OutOfMemory);
    }
    Ok(self.allocate_internal(size))
}

// Use builders for complex structs
impl GcConfig {
    pub fn builder() -> GcConfigBuilder {
        GcConfigBuilder::default()
    }
}

// Use const assertions for compile-time checks
const { assert!(MIN_HEAP_SIZE <= DEFAULT_HEAP_SIZE) }

// Use Default trait
let config = GcConfig {
    heap_size: 1024,
    ..Default::default()
};
```

❌ **DON'T**:
```rust
// Don't use unwrap() in library code
let ptr = self.allocate(size).unwrap();  // ❌

// Don't ignore errors
let _ = self.collect();  // ❌

// Don't use field reassignment
let mut config = GcConfig::default();
config.heap_size = 1024;  // ❌

// Don't use runtime assertions for constants
assert!(MIN_HEAP_SIZE <= DEFAULT_HEAP_SIZE);  // ❌
```

### Documentation Standards

**All public APIs must be documented**:

```rust
/// Unified garbage collector with adaptive strategy selection.
///
/// The GC automatically selects the best collection strategy based on
/// heap size, allocation rate, and pause time goals.
///
/// # Examples
///
/// ```rust
/// use vm_core::gc::UnifiedGc;
///
/// let gc = UnifiedGc::new(1024 * 1024); // 1MB heap
/// let obj = gc.allocate(1024)?;
/// ```
///
/// # Errors
///
/// Returns [`GcError::OutOfMemory`] if the heap is exhausted.
///
/// # Panics
///
/// Panics if `heap_size` is less than [`MIN_HEAP_SIZE`].
///
/// [`GcError::OutOfMemory`]: enum.GcError.html#variant.OutOfMemory
/// [`MIN_HEAP_SIZE`]: constant.MIN_HEAP_SIZE.html
pub struct UnifiedGc { ... }
```

### Testing Standards

**Write tests for all new functionality**:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocation_succeeds() {
        let mut gc = UnifiedGc::new(1024);
        let ptr = gc.allocate(512).unwrap();
        assert!(!ptr.is_null());
    }

    #[test]
    fn test_allocation_fails_when_full() {
        let mut gc = UnifiedGc::new(512);
        let result = gc.allocate(1024);
        assert_matches!(result, Err(GcError::OutOfMemory));
    }

    #[test]
    fn test_gc_collects_frees_memory() {
        let mut gc = UnifiedGc::new(1024);
        let ptr1 = gc.allocate(512).unwrap();

        gc.collect();

        let ptr2 = gc.allocate(512).unwrap();
        assert!(!ptr2.is_null(), "Memory should be freed after GC");
    }
}
```

---

## 🧪 Testing Guidelines

### Test Organization

```
vm-core/
├── src/
│   └── gc.rs              # Implementation code
├── tests/
│   ├── gc_tests.rs        # Integration tests
│   └── property_tests.rs  # Property-based tests
└── src/
    └── gc.rs              # Unit tests (in #[cfg(test)] mod tests)
```

### Running Tests

```bash
# Run all tests
cargo test --workspace

# Run specific test
cargo test test_allocation_succeeds

# Run tests with output
cargo test -- --nocapture

# Run tests in release mode (faster)
cargo test --release

# Run specific crate tests
cargo test -p vm-core

# Run ignored tests
cargo test -- --ignored

# Run tests with specific filter
cargo test gc::
```

### Test Coverage

We aim for **>80% code coverage**. Check your coverage:

```bash
# Install cargo-llvm-cov
cargo install cargo-llvm-cov

# Generate coverage report
cargo llvm-cov --workspace --html --output-dir coverage

# Open report
open coverage/index.html
```

### Test Categories

1. **Unit Tests**: Test individual functions and methods
2. **Integration Tests**: Test interactions between modules
3. **Property-Based Tests**: Use `proptest` for exhaustive testing
4. **Benchmark Tests**: Performance critical code

---

## 📝 Commit Conventions

We follow [Conventional Commits](https://www.conventionalcommits.org/):

### Format

```
<type>(<scope>): <subject>

<body>

<footer>
```

### Types

- **feat**: New feature
- **fix**: Bug fix
- **docs**: Documentation changes
- **style**: Code style changes (formatting)
- **refactor**: Code refactoring
- **perf**: Performance improvement
- **test**: Adding or updating tests
- **chore**: Maintenance tasks
- **build**: Build system changes
- **ci**: CI/CD changes

### Examples

```bash
# Feature
feat(gc): add parallel sweeping algorithm

# Bug fix
fix(mmu): resolve TLB flush race condition

# Documentation
docs(readme): update build instructions for macOS

# Performance
perf(jit): optimize register allocation pass

# Refactoring
refactor(core): extract error handling to separate module

# Test
test(gc): add unit tests for generational collection
```

### Good Commit Messages

✅ **Good**:
```
feat(gc): implement generational garbage collection

- Add nursery and old generations
- Implement generation promotion heuristic
- Add write barrier for tracking inter-generational pointers
- Include 15 unit tests

This improves GC pause times by 60% for allocate-heavy workloads.

Closes #123
```

❌ **Bad**:
```
fix bugs
update stuff
wip
```

---

## 🔀 Pull Request Process

### Before Submitting

1. **Code Quality**:
   ```bash
   # Format code
   cargo fmt --all

   # Run clippy
   cargo clippy --workspace -- -D warnings

   # Run tests
   cargo test --workspace
   ```

2. **Documentation**:
   - Update relevant documentation
   - Add doc comments to public APIs
   - Update CHANGELOG.md if applicable

3. **Commits**:
   - Squash related commits
   - Use conventional commit messages
   - Remove fixup! or squash! commits

### Creating the PR

Use the PR template (`.github/PULL_REQUEST_TEMPLATE.md`):

```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update

## Testing
- [ ] Unit tests added/updated
- [ ] Integration tests pass
- [ ] Manual testing completed

## Checklist
- [ ] Code follows style guidelines
- [ ] Self-review completed
- [ ] Comments added to complex code
- [ ] Documentation updated
- [ ] No new warnings generated
- [ ] Tests pass locally
- [ ] Conventional commits used

## Related Issues
Fixes #123
Closes #456
```

### PR Review Process

1. **Automated Checks**: CI must pass
2. **Code Review**: At least one maintainer approval
3. **Changes**: Address review feedback
4. **Approval**: Ready to merge

### Merge Policy

- **Squash Merge**: For feature branches
- **Merge Commit**: For long-running branches
- **Rebase**: For maintaining clean history

Maintainers will handle the merge after approval.

---

## 📜 Developer Certificate of Origin

By contributing, you agree to the DCO:

```
Developer Certificate of Origin
Version 1.1

Copyright (C) 2004, 2006 The Linux Foundation and its contributors.
1 Letterman Drive
Suite D4700
San Francisco, CA, 94123

Everyone is permitted to copy and distribute verbatim copies of this
license document, but changing it is not allowed.

Developer's Certificate of Origin 1.1

By making a contribution to this project, I certify that:

(a) The contribution was created in whole or in part by me and I
    have the right to submit it under the open source license
    indicated in the file; or

(b) The contribution is based upon previous work that, to the best
    of my knowledge, is covered under an appropriate open source
    license and I have the right under that license to submit that
    work with modifications, whether created in whole or in part
    by me, under the same open source license (unless I am
    permitted to submit under a different license), as indicated
    in the file; or

(c) The contribution was provided directly to me by some other
    person who certified (a), (b) or (c) and I have not modified
    it.

(d) I understand and agree that this project and the contribution
    are public and that a record of the contribution (including all
    personal information I submit with it, including my sign-off) is
    maintained indefinitely and may be redistributed consistent with
    this project or the open source license(s) involved.
```

### Sign-off Your Commits

Add `Signed-off-by` to your commit messages:

```bash
git commit -s -m "feat: add new feature

Description here.

Signed-off-by: Your Name <your.email@example.com>"
```

Or configure Git to automatically sign off:

```bash
git config --global commit.signOff true
```

---

## 🆘 Getting Help

### Resources

- **Documentation**: Check [README.md](README.md) and [docs/](docs/)
- **Issues**: Search [GitHub Issues](https://github.com/your-org/vm/issues)
- **Discussions**: Ask questions in [GitHub Discussions](https://github.com/your-org/vm/discussions)

### Asking Good Questions

1. **Search first**: Check if your question was already asked
2. **Be specific**: Include code snippets, error messages, and context
3. **Minimal example**: Provide a minimal reproducible example
4. **Format code**: Use markdown code blocks

### Example Question

```markdown
## Issue: TLB flush panic in concurrent scenario

### Context
I'm seeing a panic when running concurrent TLB flushes on vm-mem v0.1.0.

### Minimal Reproducer
```rust
#[test]
fn test_concurrent_flush() {
    let tlb = Arc::new(Tlb::new());
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let tlb = tlb.clone();
            thread::spawn(move || {
                tlb.flush(0x1000);
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}
```

### Error
```
thread 'main' panicked at 'called Result::unwrap() on an Err value: ..'
```

### Environment
- Rust 1.92.0
- macOS 14.0
- vm-mem from commit abc123

Any help would be appreciated!
```

---

## 📚 Additional Resources

### Learning Rust

- [The Rust Programming Language](https://doc.rust-lang.org/book/)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)

### Project-Specific

- [Architecture Overview](docs/DDD_ARCHITECTURE_CLARIFICATION.md)
- [Quick Start Guide](docs/QUICK_START.md)
- [Development Workflow](docs/QUICK_START.md#development-workflow)

---

## ✨ Recognition

Contributors are recognized in:

- **CONTRIBUTORS.md**: List of all contributors
- **Release Notes**: Notable contributions mentioned
- **GitHub**: Contributor statistics

---

**Thank you for contributing to Rust VM!** 🎉

Every contribution, no matter how small, is valued and appreciated. Together we're building a high-performance, production-ready virtual machine!

---

*Last Updated: 2025-01-03*
*Maintained By: VM Development Team*
