# Performance Monitoring Guide

This guide explains how to monitor, analyze, and improve the performance of the VM project.

## Table of Contents

- [Overview](#overview)
- [Benchmarking Infrastructure](#benchmarking-infrastructure)
- [Running Benchmarks](#running-benchmarks)
- [Analyzing Results](#analyzing-results)
- [Detecting Regressions](#detecting-regressions)
- [Performance Profiling](#performance-profiling)
- [Best Practices](#best-practices)

## Overview

The VM project uses a comprehensive performance monitoring system to:

- **Track performance over time**: Daily automated benchmarking
- **Detect regressions**: Automatic comparison against baselines
- **Identify bottlenecks**: Detailed profiling and analysis
- **Guide optimizations**: Data-driven improvement decisions

### Key Components

1. **Criterion.rs**: Statistical benchmarking framework
2. **GitHub Actions**: Automated benchmark execution
3. **Custom Scripts**: Analysis and reporting tools
4. **Baseline Storage**: Git-tracked performance history

## Benchmarking Infrastructure

### Criterion.rs Configuration

**Location**: `criterion.toml`

**Key Settings**:
```toml
# Sample size for accuracy
sample_size = 100

# Warm-up time (allows CPU optimizations)
warm_up_time = 5.0  # seconds

# Measurement time per benchmark
measurement_time = 60.0  # seconds

# Statistical significance
alpha = 0.05
confidence_level = 0.95
```

**Modes**:

1. **Quick Mode** (CI/PR feedback):
   - Sample size: 10
   - Warm-up: 1s
   - Measurement: 3s
   - Purpose: Fast feedback, lower accuracy

2. **Full Mode** (Main branch):
   - Sample size: 100
   - Warm-up: 5s
   - Measurement: 60s
   - Purpose: High accuracy, trend analysis

### Benchmark Categories

#### 1. Memory Performance

**Benchmarks**:
- `memory_optimization_benchmark`: Memory allocation strategies
- `tlb_cache_benchmark`: TLB cache efficiency
- `memory_allocation_bench`: Pool-based allocation

**Metrics**:
- Allocation/deallocation time
- Cache hit/miss ratios
- Memory usage patterns
- Pool efficiency

**Location**: `vm-mem/`, `benches/memory_*.rs`

#### 2. JIT Compilation

**Benchmarks**:
- `jit_compilation_bench`: Code generation speed
- `comprehensive_jit_benchmark`: End-to-end JIT performance
- `pgo_jit_bench`: Profile-guided optimization

**Metrics**:
- Compilation time per instruction
- Code generation throughput
- Hot path optimization effectiveness
- Generated code quality

**Location**: `vm-engine/`, `benches/jit_*.rs`

#### 3. Async Runtime

**Benchmarks**:
- `async_performance_benchmark`: Task execution
- `async_device_io_bench`: Async I/O operations
- `comprehensive_async_benchmark`: Full async workflow

**Metrics**:
- Task spawn time
- Await latency
- I/O throughput
- Concurrent operation efficiency

**Location**: `vm-engine/`, `benches/async_*.rs`

#### 4. Device I/O

**Benchmarks**:
- `device_io_bench`: Block device performance
- `async_device_io_bench`: Async device operations
- `virtioblock_performance`: VirtIO block device

**Metrics**:
- Read/write throughput
- I/O latency
- Queue efficiency
- Interrupt handling

**Location**: `vm-device/`, `benches/*io*.rs`

#### 5. Cross-Architecture

**Benchmarks**:
- `cross_arch_translation_bench`: Instruction translation
- `cross_arch_comprehensive_bench`: Full translation pipeline

**Metrics**:
- Translation speed
- Cache efficiency
- Branch prediction impact

**Location**: `vm-cross-arch-support/`, `benches/cross_arch_*.rs`

## Running Benchmarks

### Local Execution

#### Run All Benchmarks

```bash
# Full benchmark suite (slow, accurate)
BENCHMARK_MODE=full ./scripts/run_benchmarks.sh

# Quick benchmark suite (fast, less accurate)
BENCHMARK_MODE=quick ./scripts/run_benchmarks.sh

# CI mode (matches GitHub Actions)
BENCHMARK_MODE=ci ./scripts/run_benchmarks.sh
```

#### Run Specific Benchmark

```bash
# Memory benchmarks
cargo bench --bench memory_optimization_benchmark

# JIT benchmarks
cargo bench --bench jit_compilation_bench

# With custom parameters
cargo bench --bench jit_compilation_bench -- \
  --sample-size 50 \
  --warm-up-time 3 \
  --measurement-time 10
```

#### Compare with Baseline

```bash
# Save current run as baseline
cargo bench -- --save-baseline my_baseline

# Compare against baseline
cargo bench -- --baseline my_baseline

# Use critcmp for detailed comparison
critcmp main my_baseline
```

### CI Execution

#### Automatic Triggers

- **Push to main**: Full benchmark suite
- **Pull requests**: Comparison with baseline
- **Daily schedule**: Trend analysis
- **Manual dispatch**: On-demand

#### View Results

1. **GitHub Actions UI**:
   - Navigate to "Actions" tab
   - Select "Performance Monitoring" workflow
   - View job logs and artifacts

2. **PR Comments**:
   - Automatic performance comparison posted
   - Shows regressions/improvements
   - Links to detailed reports

3. **Artifacts**:
   - Download `benchmark-results-*.zip`
   - Contains Criterion reports and data

## Analyzing Results

### Criterion Reports

**Location**: `target/criterion/<benchmark_name>/report/index.html`

**Contents**:
- **Mean**: Average execution time
- **Median**: Middle value (50th percentile)
- **Std Dev**: Variability measure
- **Confidence Interval**: Statistical range
- **PDF/CDF graphs**: Distribution visualization

**Key Metrics**:
- **Mean point estimate**: Best single-value estimate
- **Standard error**: Estimate precision
- **Slope**: Performance trend (if applicable)

### Performance Trends

**Historical Data**: `.github/perf-data/metrics.csv`

**Format**:
```csv
benchmark,mean_time_ns,timestamp
memory_optimization_benchmark,1234567,2025-12-30
jit_compilation_bench,2345678,2025-12-30
...
```

**Visualize Trends**:

```python
# Simple trend analysis script
import pandas as pd
import matplotlib.pyplot as plt

df = pd.read_csv('.github/perf-data/metrics.csv')
df['timestamp'] = pd.to_datetime(df['timestamp'])

for bench in df['benchmark'].unique():
    data = df[df['benchmark'] == bench]
    plt.plot(data['timestamp'], data['mean_time_ns'], label=bench)

plt.legend()
plt.xlabel('Time')
plt.ylabel('Mean Time (ns)')
plt.title('Performance Trend')
plt.show()
```

### Comparison Tools

#### critcmp

**Installation**:
```bash
cargo install critcmp
```

**Usage**:
```bash
# Compare two baselines
critcmp main previous

# Show only changed benchmarks
critcmp --only-changed main previous

# Output in different formats
critcmp --output-format table main previous
critcmp --output-format markdown main previous
```

#### cargo-criterion

**Installation**:
```bash
cargo install cargo-criterion
```

**Features**:
- HTML report generation
- Comparison views
- Interactive charts

**Usage**:
```bash
# Generate HTML report
cargo criterion --bench jit_compilation_bench

# Open in browser
open target/criterion/jit_compilation_benchmark/report/index.html
```

## Detecting Regressions

### Automatic Detection

**Script**: `scripts/detect_regression.sh`

**Thresholds**:
- **Regression**: >10% slower (fail CI)
- **Warning**: >5% slower (comment only)
- **Improvement**: >5% faster (celebrate!)

**Process**:

1. **Run current benchmarks**
   ```bash
   cargo bench --workspace --all-features
   ```

2. **Run detection script**
   ```bash
   ./scripts/detect_regression.sh
   ```

3. **Review report**
   - `regression-report.md` generated
   - Shows all comparisons
   - Highlights regressions

### Manual Investigation

#### Identify Regression Source

1. **Nrow down affected benchmark**:
   ```bash
   grep "REGRESSION" regression-report.md
   ```

2. **Compare detailed metrics**:
   ```bash
   critcmp --baseline main <affected_benchmark>
   ```

3. **Profile the code**:
   ```bash
   cargo flamegraph --bench <affected_benchmark>
   ```

4. **Check recent changes**:
   ```bash
   git log --oneline --since="2 weeks ago" -- <affected_module>
   ```

#### Fix Regression

1. **Understand the cause**:
   - Algorithm change?
   - New overhead?
   - Cache issue?

2. **Implement fix**:
   - Optimize algorithm
   - Reduce allocations
   - Improve cache locality

3. **Verify improvement**:
   ```bash
   cargo bench --bench <affected_benchmark>
   ./scripts/detect_regression.sh
   ```

4. **Update baseline** (if improvement):
   ```bash
   cargo bench -- --save-baseline main
   git add benches/baselines/
   git commit -m "perf: update baseline after optimization"
   ```

## Performance Profiling

### Flame Graphs

**Tool**: `cargo-flamegraph`

**Installation**:
```bash
cargo install flamegraph
```

**Usage**:
```bash
# Generate flamegraph for benchmark
cargo flamegraph --bench jit_compilation_benchmark

# View result
open flamegraph.svg
```

**Interpretation**:
- Width = time spent in function
- Height = call stack depth
- Identify hot paths and bottlenecks

### Memory Profiling

**Tool**: `valgrind` + `massif`

**Usage**:
```bash
# Run with memory profiling
valgrind --tool=massif \
  cargo bench --bench memory_optimization_benchmark

# Analyze results
ms_print massif.out.<pid>
```

**Alternative**: `dhat` (heap profiling)

```bash
cargo install dhat

# Run with DHAT
cargo bench --bench memory_optimization_benchmark -- \
  --profile-time=10 -- --dhat
```

### CPU Profiling

**Tool**: `perf` (Linux)

**Usage**:
```bash
# Record performance data
perf record --call-graph dwarf \
  cargo bench --bench jit_compilation_benchmark

# Analyze
perf report

# Annotated source
perf annotate
```

**Alternative**: `Instruments` (macOS)

```bash
# Run with Instruments
instruments -t "Time Profiler" \
  cargo bench --bench jit_compilation_benchmark
```

## Best Practices

### Writing Benchmarks

#### DO:

1. **Use meaningful inputs**:
   ```rust
   // Good: Real-world workload
   #[bench]
   fn bench_real_workload(b: &mut Bencher) {
       let data = generate_realistic_data(1024);
       b.iter(|| {
           process_data(&data)
       });
   }
   ```

2. **Avoid dead code elimination**:
   ```rust
   // Good: Use test::black_box
   use std::hint::black_box;

   #[bench]
   fn bench_computation(b: &mut Bencher) {
       b.iter(|| {
           black_box(expensive_computation())
       });
   }
   ```

3. **Account for setup cost**:
   ```rust
   // Good: Separate setup from measurement
   #[bench]
   fn bench_with_setup(b: &mut Bencher) {
       let data = setup_expensive_data();
       b.iter(|| {
           process_data(&data)
       });
   }
   ```

#### DON'T:

1. **Don't measure I/O in tight loops**:
   ```rust
   // Bad: I/O dominates measurement
   #[bench]
   fn bench_io(b: &mut Bencher) {
       b.iter(|| {
           File::open("test.txt").unwrap()
       });
   }
   ```

2. **Don't use too-small inputs**:
   ```rust
   // Bad: Measurement noise
   #[bench]
   fn bench_tiny(b: &mut Bencher) {
       b.iter(|| {
           compute([1, 2, 3])
       });
   }
   ```

3. **Don't forget to verify**:
   ```rust
   // Good: Verify correctness
   #[bench]
   fn bench_verified(b: &mut Bencher) {
       let result = b.iter(|| {
           compute()
       });
       assert!(verify(result));
   }
   ```

### Interpreting Results

#### Statistical Significance

**Confidence Intervals**:
- Narrow CI = Precise measurement
- Wide CI = Noisy measurement (increase samples)

**Comparisons**:
- Overlapping CIs = Not significantly different
- Non-overlapping CIs = Significantly different

**Rule of Thumb**:
- If difference < 5%: Probably noise
- If difference 5-10%: Needs investigation
- If difference > 10%: Significant change

#### Common Pitfalls

1. **Benchmarking in debug mode**:
   - Always use `--release`
   - Debug builds are 10-100x slower

2. **System noise**:
   - Close other applications
   - Disable power saving
   - Run multiple times

3. **Cold vs hot cache**:
   - First run may be slower (cold cache)
   - Take average of multiple runs

4. **Platform differences**:
   - Results vary by OS/CPU
   - Compare on same system
   - Use CI for consistency

### Continuous Improvement

#### Optimization Workflow

1. **Measure first**:
   ```bash
   cargo bench --bench <target> -- --save-baseline before
   ```

2. **Make changes**:
   - Optimize algorithm
   - Reduce allocations
   - Improve cache locality

3. **Measure again**:
   ```bash
   cargo bench --bench <target> -- --save-baseline after
   critcmp before after
   ```

4. **Verify improvement**:
   - Check for regressions
   - Verify correctness
   - Update baseline

5. **Document**:
   - Comment on improvement
   - Update benchmarks if needed
   - Share findings

#### Tracking Progress

**Performance Dashboard**:
- Maintain trend data
- Plot key metrics over time
- Set performance goals

**Regular Reviews**:
- Weekly: Review daily benchmark results
- Monthly: Analyze performance trends
- Quarterly: Set performance targets

## Related Documentation

- [CI/CD Guide](./CI_CD_GUIDE.md)
- [Contributing Guide](../CONTRIBUTING.md)
- [Benchmark Implementation](../benches/README.md)

## Support

For performance issues:
1. Check benchmark results in CI
2. Run benchmarks locally to reproduce
3. Profile with appropriate tools
4. Create issue with benchmark data
