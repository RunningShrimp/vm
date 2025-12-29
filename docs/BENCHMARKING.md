# Automated Performance Benchmarking System

This document describes the automated performance benchmarking system for the VM project.

## Overview

The benchmarking system automatically tracks performance metrics, detects regressions, and generates reports for all pull requests and daily builds.

## Components

### 1. GitHub Actions Workflow

**Location:** `.github/workflows/benchmark.yml`

The workflow runs on:
- Push to `main`/`master` branches
- Pull requests to `main`/`master`
- Daily schedule (2 AM UTC)
- Manual workflow dispatch

**Jobs:**

1. **benchmark** - Runs all benchmarks and collects results
2. **compare** - Compares PR results against baseline (PRs only)
3. **notify** - Generates summary and notifications

### 2. Benchmark Configuration

**Location:** `.cargo/config.toml`

Configures the benchmark profile with:
- Release optimizations
- Debug symbols for profiling
- LTO enabled
- Single codegen unit for maximum optimization

### 3. Benchmark Runner Script

**Location:** `scripts/run_benchmarks.sh`

Automated script that runs all benchmark suites:
- Cross-architecture translation
- JIT compilation
- Memory management
- Garbage collection
- Async operations
- Concurrency/locking
- And more...

**Usage:**
```bash
./scripts/run_benchmarks.sh
```

### 4. Regression Detection

**Location:** `scripts/detect_regression.py`

Automatically detects performance regressions by comparing results against baseline.

**Thresholds:**
- Regression alert: >10% slowdown
- Improvement note: >5% improvement
- High severity: >20% slowdown

**Usage:**
```bash
python3 scripts/detect_regression.py
```

### 5. Report Generator

**Location:** `scripts/generate_benchmark_report.py`

Generates comprehensive markdown reports from Criterion output.

**Features:**
- Categorizes benchmarks by type
- Shows mean times with 95% confidence intervals
- Compares with previous runs
- Identifies fastest/slowest benchmarks
- Performance insights and recommendations

**Usage:**
```bash
# Generate standard report
python3 scripts/generate_benchmark_report.py

# Generate comparison report
python3 scripts/generate_benchmark_report.py --compare

# Specify output file
python3 scripts/generate_benchmark_report.py --output my-report.md
```

### 6. Baseline Tracking

**Location:** `benches/baselines/main.json`

Stores baseline performance metrics for comparison. Automatically updated after each run.

**Format:**
```json
{
  "benchmark_name": {
    "value": 1.55,
    "unit": "ms",
    "description": "Benchmark description",
    "date": "2025-12-28"
  }
}
```

## Usage

### Local Development

**Run all benchmarks:**
```bash
cargo bench --workspace --all-features
```

**Run specific benchmark:**
```bash
cargo bench --bench cross_arch_benchmark
```

**Save baseline:**
```bash
cargo bench --workspace --all-features -- --save-baseline main
```

**Compare against baseline:**
```bash
cargo bench --workspace --all-features -- --baseline main
```

**Run full benchmark suite with reports:**
```bash
./scripts/run_benchmarks.sh
```

**Check for regressions:**
```bash
# First run benchmarks
cargo bench --workspace --all-features

# Then check for regressions
python3 scripts/detect_regression.py
```

### CI/CD Integration

The workflow automatically:

1. **On Pull Requests:**
   - Runs all benchmarks
   - Compares against main branch baseline
   - Comments on PR with results
   - Fails if regressions detected

2. **On Push to Main:**
   - Runs all benchmarks
   - Updates baseline
   - Stores results as artifacts
   - Generates summary report

3. **Daily Schedule:**
   - Runs full benchmark suite
   - Tracks long-term performance trends
   - Generates historical reports

## Benchmark Categories

### 1. JIT Compilation
- Tier 1 compilation time
- Tier 2 compilation time
- Optimization pass overhead
- Code size

### 2. Cross-Architecture Translation
- x86_64 to ARM64
- x86_64 to RISC-V
- Translation overhead
- Instruction throughput

### 3. Memory Management
- Allocation speed (small/large)
- Deallocation speed
- Memory pool performance
- NUMA-aware allocation

### 4. Garbage Collection
- Pause times (small/large heaps)
- Throughput
- Collection frequency
- Generational performance

### 5. TLB/MMU
- Hit rate
- Lookup time
- Flush overhead
- Prefetch effectiveness

### 6. Async Operations
- Task spawn overhead
- Await overhead
- Executor throughput
- Concurrent task performance

### 7. Concurrency
- Lock contention (low/high)
- Mutex performance
- Read-write lock performance
- Lock-free structures

### 8. Snapshots
- Creation time
- Restore time
- Compression ratio
- Memory overhead

## Results

### Viewing Results

**GitHub Artifacts:**
- Results stored for 30 days
- Download from Actions tab
- Format: HTML reports + JSON data

**Local Reports:**
- Location: `target/criterion/`
- Open in browser: `target/criterion/<benchmark>/report/index.html`

**Markdown Reports:**
- Generated automatically
- Location: `benchmark-report.md`
- Attached to PR comments

### Interpreting Results

**Criterion Output:**
- **Mean:** Average time across iterations
- **Std Dev:** Variability in measurements
- **95% CI:** Confidence interval (true mean lies here 95% of the time)
- **Iterations:** Number of samples measured

**Regression Indicators:**
- 🔴 High severity: >20% slowdown
- 🟡 Medium severity: >10% slowdown
- 🟢 Improvement: >5% speedup
- ✅ Stable: Within acceptable range

## Best Practices

### Writing Benchmarks

1. **Use Criterion.rs:**
   ```rust
   use criterion::{black_box, criterion_group, criterion_main, Criterion};
   
   fn bench_function(c: &mut Criterion) {
       c.bench_function("my_function", |b| {
           b.iter(|| {
               // Code to benchmark
               black_box(my_function())
           })
       });
   }
   
   criterion_group!(benches, bench_function);
   criterion_main!(benches);
   ```

2. **Use `black_box`:**
   - Prevents compiler optimizations
   - Ensures code isn't eliminated
   - Use on inputs and outputs

3. **Avoid I/O in benchmarks:**
   - Don't print to stdout/stderr
   - Don't read/write files
   - Focus on computation

4. **Warm-up:**
   - Criterion handles warm-up automatically
   - No manual warm-up needed

5. **Parameterized benchmarks:**
   ```rust
   fn bench_with_sizes(c: &mut Criterion) {
       for size in [1024, 4096, 16384].iter() {
           c.bench_with_input(
               BenchmarkId::new("my_bench", size),
               size,
               |b, &size| {
                   b.iter(|| {
                       black_box(process_data(size))
                   })
               },
           );
       }
   }
   ```

### Running Benchmarks

1. **Consistent Environment:**
   - Use same machine for comparison
   - Close unnecessary applications
   - Ensure stable power/battery

2. **Multiple Runs:**
   - Run at least 3 times
   - Take median result
   - Watch for outliers

3. **Update Baselines:**
   - After intentional optimizations
   - After significant refactors
   - When improving other metrics

### Troubleshooting

**Benchmarks are too noisy:**
- Increase sample size in Criterion
- Close background applications
- Use performance mode (disable power saving)

**Benchmarks are too slow:**
- Reduce warm-up time
- Decrease sample size
- Use shorter test cases

**Regression false positives:**
- Run multiple times to confirm
- Check system load during run
- Verify no background processes

**No regression detected:**
- Check baseline is current
- Verify benchmark ran successfully
- Compare against correct baseline

## Advanced Usage

### Custom Baselines

```bash
# Create named baseline
cargo bench -- --save-baseline my_experiment

# Compare against custom baseline
cargo bench -- --baseline my_experiment
```

### Filtering Benchmarks

```bash
# Run specific benchmark
cargo bench --bench jit_benchmarks

# Run benchmarks matching pattern
cargo bench jit

# Exclude benchmarks
cargo bench -- --skip-filter gc
```

### Output Formats

```bash
# HTML output (default)
cargo bench -- --output-format html

# Quiet output
cargo bench -- --output-format quiet

# Verbose output
cargo bench -- --verbose
```

### Integration with profilers

```bash
# Run with perf (Linux)
cargo bench -- --profile-time 5

# Run with Instruments (macOS)
cargo bench -- --sample-rate 1000
```

## Continuous Improvement

The benchmark system evolves with the project. Regular updates:

- Add new benchmarks as features are added
- Update baselines after optimizations
- Adjust thresholds as needed
- Improve report formatting
- Add performance targets

## Contributing

When adding performance-sensitive changes:

1. Run benchmarks before and after
2. Include results in PR description
3. Document intentional regressions
4. Update baselines if needed
5. Add benchmarks for new features

## References

- [Criterion.rs Documentation](https://bheisler.github.io/criterion.rs/book/)
- [Rust Benchmarking Guidelines](https://doc.rust-lang.org/1.70.0/cargo/reference/cargo-targets.html#benchmarks)
- [Performance Testing Best Practices](https://github.com/philschmid/machine-learning-performance-testing)

## Support

For issues or questions:
- Check existing issues
- Start a discussion
- Contact maintainers

---

*Last updated: 2025-12-28*
