# VM Performance Benchmarking Framework

Comprehensive performance benchmarking system for the VM project with automated regression detection, detailed reporting, and baseline tracking.

## Quick Start

### Run All Benchmarks
```bash
# Run all benchmarks with detailed reporting
./scripts/bench.sh

# Run with cargo
cargo bench --workspace
```

### Run Specific Benchmark Categories
```bash
./scripts/bench.sh --jit           # JIT compilation only
./scripts/bench.sh --memory        # Memory operations only
./scripts/bench.sh --gc            # Garbage collection only
./scripts/bench.sh --cross-arch    # Cross-arch translation only
```

### Baseline Management
```bash
# Save current results as baseline
./scripts/bench.sh --save-baseline

# Compare current results with baseline
./scripts/bench.sh --compare-baseline
```

## Overview

The benchmarking system provides:
- **Comprehensive Coverage**: JIT, Memory, GC, Cross-arch translation
- **Statistical Rigor**: Criterion.rs with confidence intervals
- **Regression Detection**: Automated performance change detection
- **Detailed Reporting**: Markdown + HTML reports with visualizations
- **Baseline Tracking**: Compare changes over time
- **CI Integration**: Automated testing in pull requests

## Benchmark Categories

### 1. JIT Compilation Performance (`perf-bench/benches/jit_performance.rs`)

**Purpose**: Measure JIT compiler performance across various scenarios

**Benchmarks**:
- `jit_ir_compilation` - IR block compilation time (10-1000 instructions)
- `jit_code_generation` - Code generation throughput
- `jit_optimization_passes` - Optimization level overhead (O0-O3)
- `jit_tiered_compilation` - Tier 1 vs Tier 2 compilation
- `jit_hot_block_recompile` - Hot block recompilation overhead
- `jit_function_compilation` - Multi-block function compilation

**Key Metrics**:
- Compilation time per instruction
- Code size expansion ratio
- Optimization overhead
- Tiered compilation speedup

**Running**:
```bash
cargo bench --bench jit_performance
```

### 2. Memory Operations (`perf-bench/benches/memory_operations.rs`)

**Purpose**: Analyze memory management and translation performance

**Benchmarks**:
- `memory_copy` - Copy speed (64B - 64KB)
- `mmu_translation` - MMU translation latency
- `tlb_hit_rate` - TLB hit/miss rates
- `tlb_lookup_latency` - TLB lookup performance
- `tlb_flush` - TLB flush overhead
- `memory_allocation` - Allocation/deallocation speed
- `memory_access_patterns` - Sequential/random/strided access

**Key Metrics**:
- Copy throughput (GB/s)
- Translation latency (ns)
- TLB hit rate (%)
- Allocation throughput (ops/ms)

**Running**:
```bash
cargo bench --bench memory_operations
```

### 3. Garbage Collection (`perf-bench/benches/gc_performance.rs`)

**Purpose**: Evaluate garbage collector performance and pause times

**Benchmarks**:
- `gc_minor_pause` - Minor GC pause time (various heap sizes)
- `gc_major_pause` - Major GC pause time
- `gc_allocation_throughput` - Allocation speed
- `gc_throughput` - Bytes reclaimed per ms
- `gc_generational` - Generational collection efficiency
- `gc_frequency` - Collection frequency impact
- `gc_live_data_ratio` - Live data ratio impact
- `gc_heap_size` - Heap size impact

**Key Metrics**:
- Pause time (ms)
- Throughput (ops/ms)
- Reclamation rate (MB/s)
- Collection frequency

**Running**:
```bash
cargo bench --bench gc_performance
```

### 4. Cross-Architecture Translation (`perf-bench/benches/cross_arch_translation.rs`)

**Purpose**: Measure binary translation performance across architectures

**Benchmarks**:
- `cross_arch_single_instruction` - Single instruction translation (all arch pairs)
- `cross_arch_block_translation` - Block translation throughput
- `cross_arch_cache_effectiveness` - Translation cache performance
- `cross_arch_instruction_density` - Instruction density impact
- `cross_arch_throughput` - Translation throughput (bytes/s)
- `cross_arch_complex_instructions` - Complex instruction handling
- `cross_arch_optimization` - Translation optimization levels

**Architecture Pairs**:
- x86_64 → ARM64
- x86_64 → RISC-V
- ARM64 → x86_64
- ARM64 → RISC-V
- RISC-V → x86_64
- RISC-V → ARM64

**Key Metrics**:
- Translation speed (instr/s)
- Cache hit rate (%)
- Code expansion ratio

**Running**:
```bash
cargo bench --bench cross_arch_translation
```

## Project Structure

```
vm/
├── perf-bench/                    # Performance benchmark crate
│   ├── benches/
│   │   ├── jit_performance.rs           # JIT benchmarks
│   │   ├── memory_operations.rs         # Memory benchmarks
│   │   ├── gc_performance.rs            # GC benchmarks
│   │   └── cross_arch_translation.rs    # Cross-arch benchmarks
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs                       # Benchmark utilities
├── benches/                       # Legacy benchmarks (being migrated)
│   ├── jit_compilation_bench.rs
│   ├── tlb_cache_benchmark.rs
│   └── ...
├── scripts/
│   ├── bench.sh                      # Main benchmark runner
│   ├── benchmark_quickstart.sh       # Interactive quickstart
│   ├── run_benchmarks.sh             # Legacy runner
│   ├── detect_regression.py          # Regression detection
│   └── generate_benchmark_report.py  # Report generator
├── docs/
│   └── BENCHMARKING.md               # This document
└── target/
    └── criterion/                    # Benchmark results (HTML reports)
```

## Benchmark Runner Scripts

### `scripts/bench.sh` (Recommended)

Comprehensive benchmark runner with detailed reporting:

```bash
# Run all benchmarks
./scripts/bench.sh

# Run specific category
./scripts/bench.sh --jit
./scripts/bench.sh --memory

# Baseline operations
./scripts/bench.sh --save-baseline
./scripts/bench.sh --compare-baseline
```

**Features**:
- Color-coded output
- Progress tracking
- Detailed markdown reports
- Baseline management
- System information collection
- Summary statistics

### `scripts/benchmark_quickstart.sh`

Interactive menu-driven quickstart:

```bash
./scripts/benchmark_quickstart.sh
```

**Options**:
1. Run all benchmarks (quick)
2. Run all benchmarks with full reports
3. Run specific benchmark
4. Check for regressions only
5. Update baseline
6. Compare with baseline

### `scripts/run_benchmarks.sh`

Legacy runner (being replaced by bench.sh):

```bash
./scripts/run_benchmarks.sh --all
./scripts/run_benchmarks.sh --jit --update-baseline
```

## Results and Reporting

### Viewing Results

**HTML Reports** (Recommended):
```bash
# View in browser
open target/criterion/<benchmark_name>/report/index.html

# Or open specific report
open target/criterion/jit_ir_compilation/report/index.html
```

**Markdown Reports**:
```bash
# Generated automatically by bench.sh
cat benchmark-reports/benchmark_report_*.md
```

**Raw Data**:
```bash
# Criterion raw data
ls target/criterion/<benchmark_name>/
```

### Report Structure

Each benchmark report includes:
- **Mean**: Average execution time
- **Std Dev**: Standard deviation
- **95% CI**: Confidence interval
- **Median**: Median value
- **Mad**: Median absolute deviation
- **Throughput**: Operations per second
- **Comparison**: Change from baseline (if comparing)

### Interpreting Results

**Performance Indicators**:
- 🔴 **High severity**: >20% slowdown
- 🟡 **Medium severity**: >10% slowdown
- 🟢 **Improvement**: >5% speedup
- ✅ **Stable**: Within ±5%

**Confidence Intervals**:
- Narrow CI = Consistent performance
- Wide CI = Noisy measurement (run longer)
- Overlapping CIs = No significant difference

## Baseline Management

### Creating Baselines

```bash
# Save current performance as baseline
./scripts/bench.sh --save-baseline

# Or manually
cargo bench -- --save-baseline main
```

### Comparing with Baselines

```bash
# Compare current run with baseline
./scripts/bench.sh --compare-baseline

# Or manually
cargo bench -- --baseline main

# Compare with custom baseline
cargo bench -- --baseline my_experiment
```

### Managing Multiple Baselines

```bash
# List baselines
ls -la target/criterion/

# Remove old baseline
rm -rf target/criterion/baseline/
```

## Regression Detection

### Automated Detection

Python script detects significant performance changes:

```bash
python3 scripts/detect_regression.py
```

**Thresholds**:
- Regression alert: >10% slowdown
- Improvement note: >5% speedup
- High severity: >20% slowdown

### Manual Detection

Compare two Criterion runs:

```bash
# Run baseline
cargo bench -- --save-baseline before

# Make changes...

# Compare
cargo bench -- --baseline before
```

## CI Integration

### GitHub Actions

The project includes CI workflows for automated benchmarking:

**Location**: `.github/workflows/benchmark.yml`

**Triggers**:
- Pull requests to main/master
- Push to main/master
- Daily schedule (2 AM UTC)
- Manual workflow dispatch

**Workflow**:
1. Runs all benchmarks
2. Compares with baseline (PRs only)
3. Comments on PR with results
4. Fails if regressions detected
5. Generates performance reports

### Running Benchmarks in CI

```yaml
- name: Run Benchmarks
  run: ./scripts/bench.sh --all

- name: Check for Regressions
  run: python3 scripts/detect_regression.py
```

## Writing Benchmarks

### Basic Benchmark Template

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

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

### Parameterized Benchmarks

```rust
fn bench_with_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("my_benchmarks");

    for size in [10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, &size| {
                b.iter(|| {
                    black_box(process_data(size))
                })
            },
        );
    }

    group.finish();
}
```

### Throughput Benchmarks

```rust
use criterion::Throughput;

fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");

    group.throughput(Throughput::Bytes(1024));
    group.bench_function("process_1kb", |b| {
        b.iter(|| {
            black_box(process_1kb_data())
        })
    });

    group.finish();
}
```

### Benchmarking Best Practices

1. **Use `black_box`**: Prevent compiler optimization
   ```rust
   black_box(input_data);
   let result = black_box(compute());
   ```

2. **Avoid I/O**: Don't print or read files in hot loop
3. **Warm-up**: Criterion handles automatically
4. **Measurement time**: Adjust for slow/fast benchmarks
   ```rust
   let mut group = c.benchmark_group("slow_bench");
   group.measurement_time(Duration::from_secs(30));
   ```

5. **Sample size**: Let Criterion determine automatically

## Performance Targets

### Baseline Performance (to be measured)

**JIT Compilation**:
- Small block (10 instr): < 10 μs
- Medium block (100 instr): < 100 μs
- Large block (1000 instr): < 1 ms

**Memory Operations**:
- Sequential copy (4KB): > 1 GB/s
- MMU translation: < 100 ns
- TLB hit rate: > 95%

**Garbage Collection**:
- Minor GC pause: < 5 ms
- Major GC pause: < 50 ms
- Allocation throughput: > 1M ops/s

**Cross-Arch Translation**:
- x86_64 → ARM64: > 10M instr/s
- Cache hit rate: > 90%
- Code expansion: < 2x

### Setting Performance Targets

When optimizing, update targets:

1. Run benchmarks to establish baseline
2. Document in `docs/BENCHMARKING.md`
3. Add regression detection thresholds
4. Update CI checks accordingly

## Troubleshooting

### Benchmarks are too noisy

**Solutions**:
- Increase measurement time
- Close background applications
- Use performance CPU mode
- Disable power saving

```bash
# Increase measurement time
let mut group = c.benchmark_group("noisy_bench");
group.measurement_time(Duration::from_secs(60));
```

### Benchmarks are too slow

**Solutions**:
- Reduce measurement time
- Decrease sample size
- Use smaller test cases
- Profile to find bottleneck

```rust
// Faster but less precise
let mut group = c.benchmark_group("fast_bench");
group.sample_size(10);  // Fewer iterations
```

### False positive regressions

**Solutions**:
- Run multiple times to confirm
- Check system load during run
- Verify stable test environment
- Increase sample size for better CI

```bash
# Run multiple times
for i in {1..5}; do
    cargo bench --bench my_bench
done
```

### No regression detected

**Solutions**:
- Verify baseline is current
- Check benchmark ran successfully
- Compare against correct baseline
- Review Criterion output

```bash
# Verify baseline exists
ls target/criterion/baseline/

# Check current results
ls target/criterion/
```

## Advanced Usage

### Custom Benchmark Config

```toml
# .cargo/config.toml
[bench]
debug = true
# More aggressive optimizations
[profile.bench]
opt-level = 3
lto = true
codegen-units = 1
```

### Filtering Benchmarks

```bash
# Run specific benchmark
cargo bench --bench jit_performance

# Run by name pattern
cargo bench jit

# Skip benchmarks
cargo bench -- --skip-filter gc
```

### Output Formats

```bash
# HTML (default)
cargo bench -- --output-format html

# Quiet output
cargo bench -- --output-format quiet

# Verbose output
cargo bench -- --verbose
```

### Profiling Integration

```bash
# With perf (Linux)
perf record cargo bench --bench my_bench
perf report

# With Instruments (macOS)
cargo bench --bench my_bench
# Then open Instruments.app and attach
```

## Contributing

When adding performance-sensitive changes:

1. **Before**: Run benchmarks and save results
2. **Implement**: Make your changes
3. **After**: Run benchmarks and compare
4. **Document**: Include results in PR description
5. **Baseline**: Update if improvement is intentional
6. **Add tests**: Add benchmarks for new features

**PR Template**:
```markdown
## Performance Impact

### Benchmarks Run
- JIT compilation
- Memory operations
- GC performance

### Results
- JIT compilation: +15% improvement
- Memory copy: +5% improvement
- GC pause time: No change

### Notes
- Optimized hot path in IR compiler
- Improved memory allocator
- Added benchmark for new feature
```

## References

- [Criterion.rs Documentation](https://bheisler.github.io/criterion.rs/book/)
- [Rust Benchmarking Guidelines](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#benchmarks)
- [Statistical Analysis for Benchmarks](https://bheisler.github.io/criterion.rs/book/user_guide/command_line_options.html)
- [Performance Testing Best Practices](https://github.com/philschmid/machine-learning-performance-testing)

## Support

For benchmarking issues or questions:
- Check existing GitHub issues
- Start a discussion in GitHub
- Review this documentation
- Examine existing benchmarks

---

*Last updated: 2025-12-31*
*Maintained by: VM Project Team*

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
