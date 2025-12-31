# Contributor CI/CD Handbook

A practical guide for contributors to understand and work with the VM project's CI/CD system.

## Quick Start

### Before You Push

```bash
# 1. Format your code
cargo fmt

# 2. Run linter
cargo clippy --workspace --all-targets --all-features -- -D warnings

# 3. Run tests
cargo test --workspace

# 4. Quick check (build only)
cargo check --workspace --all-features
```

### What Happens When You Push?

When you open a pull request or push to a branch, GitHub Actions automatically:

1. ✅ Checks code formatting
2. 🔍 Runs Clippy lints
3. 🧪 Executes all tests
4. 📊 Runs quick benchmarks
5. 📈 Calculates code coverage
6. 🔒 Audits dependencies for security
7. 🔨 Verifies builds on multiple platforms

**Typical runtime**: 15-20 minutes for all checks to complete.

## Understanding CI Results

### Reading the CI Dashboard

When your PR is ready, you'll see checks like this:

```
✅ CI / code-quality      (2 min)
✅ CI / test (ubuntu)     (8 min)
✅ CI / test (macos)      (10 min)
🟡 CI / quick-bench      (5 min)
✅ CI / coverage          (15 min)
✅ CI / security          (3 min)
✅ CI / build-check       (12 min)
```

**Legend**:
- ✅ **Green**: All checks passed
- 🟡 **Yellow**: Checks passed with warnings
- 🔴 **Red**: Something failed
- 🔵 **Blue**: Still running

### Click into Details

Each check shows:
- **Summary**: Pass/fail status
- **Logs**: Detailed output
- **Artifacts**: Generated files (test results, coverage reports, etc.)

### Common CI Failures

#### 1. Formatting Check Failed

```
Error: Code formatting check failed
```

**Fix**:
```bash
cargo fmt
git add -A
git commit -m "style: format code"
git push
```

#### 2. Clippy Warnings

```
Error: Clippy check failed
warning: this function is too long
```

**Fix**:
```bash
# See the warnings
cargo clippy --workspace --all-targets --all-features

# Fix them (cargo clippy --fix is experimental, review carefully)
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Or manually fix the issues
```

#### 3. Tests Failed

```
Error: Tests failed
FAIL vm_core::tests::test_memory
```

**Fix**:
```bash
# Run locally to see details
cargo test --workspace --all-features --no-fail-fast

# Run specific test
cargo test test_memory -- --nocapture

# Fix the issue, then push again
```

#### 4. Build Failed

```
Error: Build failed
error: use of undeclared type
```

**Fix**:
```bash
# Check locally
cargo check --workspace --all-features

# If it works locally but fails in CI:
# - Check Rust version matches (rust-toolchain.toml)
# - Verify all dependencies committed
# - Check for platform-specific code
```

## Performance Benchmarks

### Quick Benchmarks in PR

Every PR runs quick benchmarks (5-10 minutes):

```
🟡 Performance / benchmark (5 min)
```

**What it does**:
- Runs key benchmarks with reduced sampling
- Compares against main branch baseline
- Posts results as PR comment

**Example PR Comment**:
```markdown
## 📊 Performance Benchmark Results

### Summary
Your changes compared to the baseline from `main`.

### Comparison Details
- memory_optimization: +2.3% (stable)
- jit_compilation: -5.1% (improvement!)
- async_execution: +8.2% (warning)

### Thresholds
- 🟢 Improvement: >5% faster
- 🟡 Warning: >5% slower
- 🔴 Regression: >10% slower
```

### If Regression Detected

**Don't panic!** Here's what to do:

1. **Review the regression**:
   - Is it expected? (new feature, refactoring)
   - Is it significant? (>10%)
   - Is it a false positive? (CI noise)

2. **Investigate**:
   ```bash
   # Run locally to verify
   cargo bench --bench <affected_benchmark>
   ```

3. **Fix or explain**:
   - Fix the regression if unintended
   - Add comment to PR if intentional
   - Discuss with maintainers if uncertain

## Code Coverage

### Viewing Coverage

After CI completes, find the coverage check:

```
✅ CI / coverage (15 min)
```

**Download artifact**: `coverage-results-*.zip`

**View report**:
```bash
# Generate locally (more detailed)
cargo install cargo-llvm-cov
cargo llvm-cov --workspace --all-features --html
open target/llvm-cov/html/index.html
```

### Coverage Goals

**Current minimum**: 30% (warning only)

**Good targets**:
- Core code (vm-core, vm-engine): >60%
- Critical paths: >80%
- Overall: >50%

**Low coverage is okay for**:
- Platform-specific code
- Error handling paths
- Deprecated code

## Local Development

### Mimic CI Environment

**Use the same Rust version**:
```bash
rustup show
# Should match rust-toolchain.toml
```

**Run full CI suite locally**:
```bash
# Clone this script
cat > run_local_ci.sh << 'EOF'
#!/bin/bash
set -e

echo "=== Running CI checks ==="

echo "1. Formatting check..."
cargo fmt -- --check

echo "2. Clippy check..."
cargo clippy --workspace --all-targets --all-features -- -D warnings

echo "3. Tests..."
cargo test --workspace --all-features

echo "4. Build check..."
cargo build --workspace --all-features

echo "✅ All checks passed!"
EOF

chmod +x run_local_ci.sh
./run_local_ci.sh
```

### Pre-commit Hook (Optional)

**Install pre-commit hooks**:
```bash
# .git/hooks/pre-commit
#!/bin/bash
echo "Running pre-commit checks..."

cargo fmt -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features --quiet

echo "✅ Pre-commit checks passed"
```

**Make executable**:
```bash
chmod +x .git/hooks/pre-commit
```

## Advanced Usage

### Skipping CI (Use Sparingly)

**Not recommended** unless absolutely necessary:

```yaml
# In commit message
[skip ci]
```

**Use cases**:
- Documentation-only changes
- Comment updates
- CI configuration fixes

**Don't use for**:
- Code changes
- Test updates
- Dependency changes

### Re-running Failed Jobs

**From GitHub UI**:
1. Go to your PR
2. Click on failed check
3. Click "Re-run jobs" button

**Or push empty commit**:
```bash
git commit --allow-empty -m "recheck: rerun CI"
git push
```

### Testing Across Platforms

**CI tests on**:
- Ubuntu Latest
- macOS Latest

**Not tested**:
- Windows (needs Windows machine or VM)

**To test locally**:
```bash
# On macOS
cargo test --workspace

# On Windows (if available)
cargo test --workspace --all-features
```

## Workflow Tips

### Small PRs, Fast Feedback

**Good**:
- PR: "Optimize memory allocation" (200 lines)
- CI: ~10 minutes
- Feedback: Quick

**Avoid**:
- PR: "Refactor everything" (5000 lines)
- CI: ~30 minutes
- Feedback: Slow, hard to review

### Draft PRs

**For work in progress**:
1. Create PR as "Draft"
2. CI still runs, but no rush to fix
3. Mark "Ready for review" when done

**Draft PR workflow**:
```bash
# Create draft PR
git push origin my-feature
# On GitHub: "Create pull request" -> "Mark as draft"

# When ready
git push
# On GitHub: "Ready for review"
```

### Addressing Review Feedback

**Process**:
1. Make requested changes
2. Push to same branch
3. CI re-runs automatically
4. Comment "Fixed" on review

**No need to**:
- Close and reopen PR
- Create new branch
- Squash commits (unless asked)

## Performance Guidelines

### When to Worry About Performance

**Worry** (>10% regression):
- In hot path (executed frequently)
- In user-facing code
- Without corresponding benefit

**Probably OK** (5-10% change):
- In cold path (rarely executed)
- With code clarity improvement
- In diagnostic/debug code

**Definitely OK** (<5% change):
- Normal measurement noise
- In tests/benchmarks
- With significant other improvements

### Adding Benchmarks

**When adding new features**:
```rust
// benches/new_feature_bench.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_new_feature(c: &mut Criterion) {
    let data = setup_test_data();

    c.bench_function("new_feature", |b| {
        b.iter(|| {
            new_feature(black_box(&data))
        })
    });
}

criterion_group!(benches, bench_new_feature);
criterion_main!(benches);
```

**Update benchmark list** in `scripts/run_benchmarks.sh`:
```bash
BENCHMARKS=(
    # ... existing benchmarks ...
    "new_feature_bench"  # Add here
)
```

## Troubleshooting

### CI Works Locally But Fails

**Common causes**:

1. **Rust version mismatch**:
   ```bash
   # Check CI version
   cat rust-toolchain.toml

   # Update local
   rustup update nightly
   rustup default nightly
   ```

2. **Environment differences**:
   - CI: Fresh environment
   - Local: Cached dependencies, old artifacts

   **Fix**:
   ```bash
   cargo clean
   cargo build --workspace --all-features
   ```

3. **Platform-specific code**:
   ```rust
   #[cfg(target_os = "linux")]
   fn linux_specific() { }

   #[cfg(target_os = "macos")]
   fn macos_specific() { }

   #[cfg(not(any(target_os = "linux", target_os = "macos")))]
   fn fallback() { }
   ```

### Flaky Tests

**Symptoms**: Test passes locally, fails intermittently in CI

**Common causes**:
- Race conditions
- Timing-dependent code
- Resource exhaustion

**Fix**:
```rust
// Add retries
#[test]
fn flaky_test() {
    let mut attempts = 0;
    loop {
        attempts += 1;
        match test_logic() {
            Ok(_) => return,
            Err(_) if attempts < 3 => continue,
            Err(e) => panic!("Test failed: {}", e),
        }
    }
}
```

### Timeout Issues

**If job times out**:

1. **Check for infinite loops**:
   ```rust
   // Bad
   loop {
       // Never exits
   }

   // Good
   for _ in 0..MAX_ITERATIONS {
       if condition { break; }
   }
   ```

2. **Reduce work in tests**:
   ```rust
   // Bad: Testing with 1M items
   #[test]
   fn test_large() {
       test_with_data(1_000_000)
   }

   // Good: Testing with 1K items
   #[test]
   fn test_reasonable() {
       test_with_data(1_000)
   }
   ```

## Getting Help

### Resources

1. **This handbook**: Quick reference
2. **CI/CD Guide**: `docs/CI_CD_GUIDE.md` (detailed)
3. **Performance Guide**: `docs/PERFORMANCE_MONITORING.md` (performance focus)
4. **Contributing Guide**: `CONTRIBUTING.md` (general contribution)

### When Stuck

1. **Search existing issues**:
   ```bash
   # In GitHub repo
   # Search: "CI failed clippy" or similar
   ```

2. **Ask in PR comments**:
   ```markdown
   @maintainers I'm getting a CI failure for clippy warnings.
   I've tried X, Y, Z. Any suggestions?
   ```

3. **Create discussion**:
   - GitHub Discussions: "CI/CD"
   - Describe issue clearly
   - Include logs/error messages

### Maintainer Communication

**Good PR comment**:
```markdown
## CI Help Needed

### Issue
Clippy failing with warning: `must_use_method`

### What I tried
- Ran `cargo clippy` locally (works)
- Updated Rust version (still fails)
- Reviewed Clippy documentation (unclear)

### Logs
[Attach relevant logs]

### Question
Should I add `#[must_use]` or suppress the warning?
```

## Best Practices

### Before Submitting PR

✅ **Do**:
- Run `cargo fmt`
- Fix all Clippy warnings
- Ensure tests pass locally
- Add tests for new features
- Update documentation
- Check performance for sensitive changes

❌ **Don't**:
- Ignore CI warnings
- Skip tests ("works on my machine")
- Make massive PRs without discussion
- Forget to update docs
- Suppress Clippy without justification

### During Review

✅ **Do**:
- Respond to all review comments
- Push fixes promptly
- Ask questions if unclear
- Explain trade-offs
- Keep PR focused

❌ **Don't**:
- Abandon PR without notice
- Argue against every suggestion
- Make unrequested changes
- Ignore performance concerns

### After Merge

✅ **Do**:
- Delete your branch (if not reused)
- Update related documentation
- Celebrate! 🎉

❌ **Don't**:
- Immediately open conflicting PR
- Forget to thank reviewers
- Leave merged branches forever

## Quick Reference

### Essential Commands

```bash
# Format
cargo fmt

# Lint
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Test
cargo test --workspace --all-features

# Check build
cargo check --workspace --all-features

# Benchmarks (quick)
BENCHMARK_MODE=quick ./scripts/run_benchmarks.sh

# Benchmarks (full)
BENCHMARK_MODE=full ./scripts/run_benchmarks.sh

# Coverage
cargo llvm-cov --workspace --all-features
```

### CI Status Checks

```bash
# Check workflow status (gh CLI)
gh run list --workflow=ci.yml

# Watch specific PR
gh pr view 123 --json statusCheckRollup

# Re-run failed jobs
gh run rerun <run-id>
```

### Useful Links

- **CI/CD Guide**: [docs/CI_CD_GUIDE.md](./CI_CD_GUIDE.md)
- **Performance Guide**: [docs/PERFORMANCE_MONITORING.md](./PERFORMANCE_MONITORING.md)
- **Contributing**: [CONTRIBUTING.md](../CONTRIBUTING.md)
- **Project README**: [README.md](../README.md)

---

**Happy contributing!** 🚀

Questions? Open an issue or discussion, and we'll help you out.
