# CI/CD Guide for VM Project

This guide explains how to use and configure the Continuous Integration and Continuous Deployment (CI/CD) pipelines for the VM project.

## Table of Contents

- [Overview](#overview)
- [CI Pipeline](#ci-pipeline)
- [Performance Monitoring](#performance-monitoring)
- [Quality Gates](#quality-gates)
- [Running Locally](#running-locally)
- [Troubleshooting](#troubleshooting)

## Overview

The VM project uses GitHub Actions for CI/CD with the following workflows:

1. **CI Pipeline** (`.github/workflows/ci.yml`)
   - Code quality checks (formatting, linting)
   - Unit tests across multiple platforms
   - Code coverage reporting
   - Security audits
   - Build verification

2. **Performance Monitoring** (`.github/workflows/performance.yml`)
   - Comprehensive benchmark suite
   - Performance regression detection
   - Trend analysis
   - Baseline comparison

## CI Pipeline

### Triggers

The CI pipeline runs on:
- Push to `master`, `main`, or `develop` branches
- Pull requests to `master`, `main`, or `develop`
- Manual workflow dispatch

### Jobs

#### 1. Code Quality Checks

**Purpose**: Ensure code meets quality standards

**Checks performed**:
- Code formatting verification (`cargo fmt`)
- Clippy linting with pedantic checks
- Documentation generation
- Documentation link validation

**Runtime**: ~5-10 minutes

**Configuration**:
```toml
# .clippy.toml
warn-on-all-wildcard-imports = true

# Cargo.toml workspace lints
[workspace.lints.clippy]
all = "warn"
pedantic = "warn"
cargo = "warn"
```

**Failure conditions**:
- Unformatted code
- Clippy warnings treated as errors (`-D warnings`)
- Broken documentation links

#### 2. Unit Tests

**Purpose**: Verify code correctness across platforms

**Matrix**:
- OS: Ubuntu Latest, macOS Latest
- Rust: Nightly

**Runtime**: ~10-15 minutes per platform

**Configuration**:
- All features enabled
- Parallel test execution (4 threads)
- JSON output for result parsing

**Failure conditions**:
- Any test failure
- Test panics

#### 3. Quick Benchmarks (PR only)

**Purpose**: Fast performance feedback for pull requests

**Runtime**: ~5-10 minutes

**Configuration**:
- Reduced sample size (10 iterations)
- Short warm-up time (1 second)
- Minimal measurement time (3 seconds)

**Note**: Results are for quick feedback only. Full benchmarks run on main branch.

#### 4. Code Coverage

**Purpose**: Track test coverage

**Tool**: `cargo-llvm-cov`

**Minimum threshold**: 30% (warning only, not enforced)

**Runtime**: ~15-20 minutes

**Output**:
- Coverage reports uploaded as artifacts
- Optional Codecov integration

#### 5. Security Audit

**Purpose**: Detect security vulnerabilities

**Tools**:
- `cargo-audit`: Checks for known vulnerabilities
- `cargo-deny`: License and dependency checks

**Runtime**: ~5 minutes

**Failure conditions**:
- Critical security vulnerabilities (fail-fast disabled for gradual rollout)

#### 6. Build Check

**Purpose**: Ensure code builds successfully

**Matrix**:
- OS: Ubuntu Latest, macOS Latest
- Target: x86_64-unknown-linux-gnu, x86_64-apple-darwin

**Configuration**:
- Debug build with all features
- Release build with optimizations

**Runtime**: ~10-15 minutes per platform

### CI Report

After all jobs complete, a final report summarizes:
- Status of all jobs
- Links to artifacts
- Test coverage summary
- Build verification results

## Performance Monitoring

### Overview

Performance monitoring runs comprehensive benchmarks and tracks performance over time.

### Triggers

- **Push to main/master**: Full benchmark suite
- **Pull requests**: Comparison against baseline
- **Schedule**: Daily at 2 AM UTC (trend analysis)
- **Manual**: On-demand benchmarking

### Benchmark Categories

#### 1. Memory Benchmarks

**Target**: `vm-mem`, memory management

**Benchmarks**:
- Memory allocation/deallocation
- TLB cache performance
- Memory optimization strategies

**Runtime**: ~10 minutes

#### 2. JIT Compilation

**Target**: `vm-engine`, JIT compiler

**Benchmarks**:
- Code generation time
- Compilation throughput
- Hot path optimization

**Runtime**: ~15 minutes

#### 3. Async Performance

**Target**: Async runtime, device I/O

**Benchmarks**:
- Async task execution
- Device I/O throughput
- Concurrent operations

**Runtime**: ~10 minutes

#### 4. Comprehensive Benchmarks

**Target**: Full system performance

**Benchmarks**:
- End-to-end execution
- Cross-component interactions
- Real-world workloads

**Runtime**: ~20 minutes

### Baseline Management

**Baselines**: Reference performance measurements

**Storage**:
- `benches/baselines/` directory
- Git-tracked in `.github/perf-data/`

**Naming**:
- `main`: Primary baseline (master branch)
- `previous`: Previous successful run
- Custom names for specific experiments

**Updating baselines**:
```bash
# Run benchmarks with new baseline
cargo bench -- --save-baseline my_baseline

# Compare baselines
critcmp main my_baseline
```

### Regression Detection

**Thresholds**:
- **Regression**: >10% slower (🔴)
- **Warning**: >5% slower (🟡)
- **Improvement**: >5% faster (🟢)
- **Stable**: Within ±5% (✅)

**Configuration**:
```toml
# criterion.toml or environment variables
REGRESSION_THRESHOLD = 10  # percentage
WARNING_THRESHOLD = 5       # percentage
```

**Detection**:
- Automatic comparison with baseline
- PR comments with detailed results
- Trend tracking over time

## Quality Gates

### Pre-commit Checks

Run these before pushing:

```bash
# Format code
cargo fmt

# Run clippy
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Run tests
cargo test --workspace

# Quick check
cargo check --workspace --all-features
```

### Pre-merge Requirements

Before merging to main:

1. **All CI jobs must pass** (except security warnings)
2. **No performance regressions** >10%
3. **Code coverage maintained** (no significant drops)
4. **Documentation builds** without errors

### Exception Process

If a job fails but should be allowed:

1. **Evaluate impact**:
   - Is it a false positive?
   - Does it affect correctness or performance?
   - Is it a known issue?

2. **Document decision**:
   - Add comment to PR
   - Create tracking issue
   - Update CI configuration if needed

3. **Team approval**:
   - Get approval from maintainer
   - Document in commit message

## Running Locally

### Reproduce CI Environment

```bash
# Run all CI checks
./scripts/run_ci.sh

# Run specific checks
cargo fmt -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

### Run Benchmarks Locally

```bash
# Quick benchmarks (like CI)
BENCHMARK_MODE=quick ./scripts/run_benchmarks.sh

# Full benchmarks (like main branch)
BENCHMARK_MODE=full ./scripts/run_benchmarks.sh

# Specific benchmark
cargo bench --bench memory_optimization_benchmark

# Compare with baseline
cargo bench --bench memory_optimization_benchmark -- --baseline main
```

### Check Coverage Locally

```bash
# Install cargo-llvm-cov
cargo install cargo-llvm-cov

# Generate coverage
cargo llvm-cov --workspace --all-features

# Generate HTML report
cargo llvm-cov --workspace --all-features --html
```

### Performance Regression Detection

```bash
# Run regression detection
./scripts/detect_regression.sh

# With custom threshold
REGRESSION_THRESHOLD=5 ./scripts/detect_regression.sh
```

## Troubleshooting

### CI Failures

#### Formatting Issues

**Error**: `Code formatting check failed`

**Solution**:
```bash
cargo fmt
git add -A
git commit -m "fix: format code"
```

#### Clippy Warnings

**Error**: `Clippy check failed`

**Solution**:
1. Review warnings locally:
   ```bash
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   ```

2. Fix warnings or add `#[allow(...)]` with justification

3. If warning is false positive, file issue

#### Test Failures

**Error**: `Tests failed`

**Solution**:
1. Run tests locally:
   ```bash
   cargo test --workspace --all-features --no-fail-fast
   ```

2. Check for platform-specific issues
3. Verify dependencies are up to date

#### Build Failures

**Error**: `Build failed`

**Solution**:
1. Clean build:
   ```bash
   cargo clean
   cargo build --workspace --all-features
   ```

2. Check Rust version matches `rust-toolchain.toml`
3. Verify system dependencies installed

### Performance Issues

#### Benchmark Failures

**Error**: `Benchmarks failed`

**Solution**:
1. Run benchmark locally to reproduce
2. Check for system resource issues
3. Verify benchmark code is correct

#### Performance Regression Detected

**Error**: `Performance regression detected`

**Solution**:
1. Review regression report
2. Identify cause of regression
3. Fix or justify regression
4. Update baseline if intentional

### Timeouts

#### CI Job Timeout

**Error**: `Job timed out after X minutes`

**Solution**:
1. Check if job is consistently slow
2. Optimize job (caching, parallelization)
3. Increase timeout if justified
4. Split job into smaller parts

#### Benchmark Timeout

**Error**: `Benchmark timed out`

**Solution**:
1. Reduce measurement time
2. Decrease sample size
3. Check for infinite loops in benchmark

## Best Practices

### For Contributors

1. **Run checks locally first**
   - Pre-commit hooks recommended
   - At least run `cargo fmt` and `cargo clippy`

2. **Keep PRs focused**
   - Smaller PRs faster to review and test
   - Easier to identify issues

3. **Monitor performance**
   - Check benchmark results in PR comments
   - Address regressions promptly

4. **Update documentation**
   - Document API changes
   - Update README if needed

### For Maintainers

1. **Review CI results**
   - Don't merge failing workflows
   - Investigate flaky tests

2. **Monitor trends**
   - Review scheduled benchmark reports
   - Track coverage trends

3. **Update baselines**
   - When performance improves intentionally
   - After major refactoring

4. **Tune CI**
   - Adjust thresholds as needed
   - Optimize for speed vs. accuracy

## Configuration Files

### `.github/workflows/ci.yml`

Main CI pipeline configuration.

### `.github/workflows/performance.yml`

Performance monitoring and benchmarking.

### `criterion.toml`

Benchmark configuration.

### `.clippy.toml`

Lint configuration.

### `.rustfmt.toml`

Formatting configuration.

### `rust-toolchain.toml`

Rust version and components.

## Related Documentation

- [Performance Monitoring Guide](./PERFORMANCE_MONITORING.md)
- [Contributing Guide](../CONTRIBUTING.md)
- [Benchmark Implementation](../benches/README.md)

## Support

For CI/CD issues:
1. Check this guide
2. Review GitHub Actions logs
3. Search existing issues
4. Create new issue with details
