# Quality Gates Quick Reference

**Quick guide to quality gates for the VM project**

---

## What Are Quality Gates?

Quality gates are automated checks that **must pass** before code can be merged. They ensure code quality, security, and maintainability.

---

## The 6 Required Gates

```
┌─────────────────────────────────────────────────────────────┐
│                    PUSH / PULL REQUEST                       │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│ 1. FORMAT CHECK (rustfmt)                                    │
│    ✅ All code must be formatted                             │
│    Command: cargo fmt --all -- --check                      │
└──────────────────────┬──────────────────────────────────────┘
                       │ Pass
                       ▼
┌─────────────────────────────────────────────────────────────┐
│ 2. CLIPPY CHECK (Strict Mode)                                │
│    ✅ Zero warnings tolerated                                │
│    ✅ No unwrap(), expect(), panic!()                         │
│    Command: cargo clippy --workspace --all-features          │
│             --all-targets -- -D warnings                     │
└──────────────────────┬──────────────────────────────────────┘
                       │ Pass
                       ▼
┌─────────────────────────────────────────────────────────────┐
│ 3. COMPILATION CHECK                                         │
│    ✅ Must compile on Linux, macOS, Windows                 │
│    ✅ Debug + Release builds                                 │
│    ✅ All features enabled                                   │
│    Command: cargo build --workspace --all-features           │
└──────────────────────┬──────────────────────────────────────┘
                       │ Pass
                       ▼
┌─────────────────────────────────────────────────────────────┐
│ 4. TEST SUITE                                                │
│    ✅ All tests must pass                                   │
│    ✅ Debug + Release modes                                  │
│    ✅ All platforms                                         │
│    Command: cargo test --workspace --all-features            │
└──────────────────────┬──────────────────────────────────────┘
                       │ Pass
                       ▼
┌─────────────────────────────────────────────────────────────┐
│ 5. DOCUMENTATION CHECK                                       │
│    ✅ All public APIs documented                            │
│    ✅ No broken links                                       │
│    Command: cargo doc --no-deps --workspace --all-features  │
└──────────────────────┬──────────────────────────────────────┘
                       │ Pass
                       ▼
┌─────────────────────────────────────────────────────────────┐
│ 6. COVERAGE CHECK                                            │
│    ✅ Minimum 50% code coverage                             │
│    Tool: cargo-llvm-cov                                     │
│    Command: cargo llvm-cov --workspace --all-features        │
└──────────────────────┬──────────────────────────────────────┘
                       │ Pass
                       ▼
┌─────────────────────────────────────────────────────────────┐
│                    ✅ READY TO MERGE                         │
└─────────────────────────────────────────────────────────────┘
```

---

## Quick Fix Commands

### Format Check Failed
```bash
cargo fmt
git add -A
git commit -m "fix: format code"
```

### Clippy Check Failed
```bash
# View warnings
cargo clippy --workspace --all-features --all-targets -- -D warnings

# Auto-fix (where possible)
cargo clippy --workspace --all-features --all-targets -- --fix
```

### Tests Failed
```bash
# Run tests with output
cargo test --workspace --all-features -- --nocapture

# Run specific test
cargo test --package vm-core --lib test_name

# With backtrace
RUST_BACKTRACE=1 cargo test --workspace
```

### Coverage Too Low
```bash
# View coverage report
cargo llvm-cov --workspace --all-features --html
open target/llvm-cov/html/index.html

# See summary
cargo llvm-cov --workspace --all-features --summary
```

### Build Failed
```bash
# Clean and rebuild
cargo clean
cargo build --workspace --all-features

# Check Rust version
rustc --version  # Should be 1.92+
```

---

## Run All Checks Locally

### Option 1: Use the Script
```bash
./scripts/check-quality.sh
```

### Option 2: Manual Commands
```bash
# 1. Format
cargo fmt

# 2. Clippy
cargo clippy --workspace --all-features --all-targets -- -D warnings

# 3. Build
cargo build --workspace --all-features

# 4. Test
cargo test --workspace --all-features

# 5. Docs
cargo doc --no-deps --workspace --all-features

# 6. Coverage (optional)
cargo llvm-cov --workspace --all-features --summary
```

---

## Quality Standards at a Glance

| Gate | Tool | Requirement | Time to Run |
|------|------|-------------|-------------|
| Format | rustfmt | 100% compliant | ~10s |
| Clippy | clippy | Zero warnings | ~2-5 min |
| Compile | cargo | No errors | ~5-10 min |
| Test | cargo test | All pass | ~5-15 min |
| Docs | cargo doc | No errors | ~2-5 min |
| Coverage | llvm-cov | Min 50% | ~10-15 min |

**Total: ~25-60 minutes** (varies by hardware)

---

## Pre-commit Checklist

Before pushing, ensure:

- [ ] Code formatted: `cargo fmt`
- [ ] No clippy warnings
- [ ] Builds without errors
- [ ] All tests pass
- [ ] Documentation builds
- [ ] Coverage ≥ 50% (recommended)

---

## Understanding CI Results

### In GitHub Actions

1. Go to your PR
2. Click "Checks" tab
3. Review each gate:
   - ✅ Green: Passed
   - ❌ Red: Failed (click to see logs)
   - ⚠️ Yellow: Warning (optional gates)

### Using GitHub CLI

```bash
# Check PR status
gh pr checks

# View workflow runs
gh run list

# Watch specific run
gh run watch
```

---

## What Happens If a Gate Fails?

### Required Gates (Block Merge)
- Format ❌
- Clippy ❌
- Compile ❌
- Test ❌
- Docs ❌
- Coverage ❌

**Action Required**: Fix the failure and push again.

### Optional Gates (Warning Only)
- Security vulnerabilities ⚠️
- Unsafe code detected ℹ️
- Outdated dependencies ℹ️

**Action**: Review but doesn't block merge.

---

## Common Mistakes

### ❌ Don't Do This
```rust
// Using unwrap() - will fail clippy
let value = some_option.unwrap();

// Using expect() - will fail clippy
let value = some_option.expect("msg");

// Using panic!() - will fail clippy
if error { panic!("Error!"); }

// Indexing directly - will fail clippy
let item = arr[10];
```

### ✅ Do This Instead
```rust
// Use ? operator
let value = some_option.ok_or_else(|| Error::NotFound)?;

// Proper error handling
if error {
    return Err(Error::Failed);
}

// Safe indexing
let item = arr.get(10).ok_or_else(|| Error::OutOfBounds)?;
```

---

## Coverage Requirements

### Current Threshold: 50%

**What counts toward coverage**:
- Unit tests
- Integration tests
- Property-based tests

**What doesn't count**:
- Test code itself
- Benchmark code
- Build scripts

**Improve coverage by**:
1. Adding unit tests for uncovered functions
2. Testing error paths
3. Testing edge cases
4. Using `proptest` for property testing

---

## Need Help?

### Documentation
- **Full Standards**: `docs/QUALITY_STANDARDS.md`
- **Contributing**: `docs/development/CONTRIBUTING.md`
- **CI/CD Guide**: `docs/CI_CD_GUIDE.md`

### Common Issues
See `docs/QUALITY_STANDARDS.md` → "Troubleshooting" section

### Ask Questions
- GitHub Issues: Report problems
- GitHub Discussions: Ask questions
- PR Comments: Request review

---

## Quality Gate Workflow Summary

```
┌──────────────────┐
│  Write Code      │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│  Run Locally     │
│  ./scripts/      │
│  check-quality   │
└────────┬─────────┘
         │
         ▼
    Pass?
         │
    ┌────┴────┐
    │ No      │ Yes
    ▼         ▼
┌───────┐  ┌──────────┐
│ Fix   │  │ Push to  │
│ Issues│  │ Branch   │
└───────┘  └─────┬────┘
                 │
                 ▼
          ┌─────────────┐
          │ CI Runs     │
          │ Quality     │
          │ Gates       │
          └──────┬──────┘
                 │
                 ▼
            All Pass?
                 │
            ┌────┴────┐
            │ No      │ Yes
            ▼         ▼
        ┌───────┐  ┌──────────┐
        │ Fix   │  │ Ready to │
        │ & Push│  │ Merge!   │
        └───────┘  └──────────┘
```

---

## Remember

- ✅ Quality gates ensure code quality
- ✅ Run checks locally before pushing
- ✅ Fix failures promptly
- ✅ Ask for help if needed
- ✅ Quality gates protect the codebase

**Happy coding! 🚀**
