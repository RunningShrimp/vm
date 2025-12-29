# Cross-Architecture Benchmark Quick Start Guide

## Quick Reference

### Basic Commands

```bash
# Run all cross-arch benchmarks
cargo bench --bench cross_arch_benchmark
cargo bench --bench cross_arch_comprehensive_bench

# Run specific benchmark groups
cargo bench --bench cross_arch_comprehensive_bench translation_pairs
cargo bench --bench cross_arch_comprehensive_bench performance_metrics
cargo bench --bench cross_arch_comprehensive_bench workloads
cargo bench --bench cross_arch_comprehensive_bench optimization
cargo bench --bench cross_arch_comprehensive_bench accuracy
```

### View Results

Results are saved in HTML format:
- `target/criterion/cross_arch_benchmark/report/index.html`
- `target/criterion/cross_arch_comprehensive_bench/report/index.html`

Open in browser:
```bash
open target/criterion/cross_arch_comprehensive_bench/report/index.html
```

## Benchmark Files

1. **cross_arch_benchmark.rs** - Original enhanced benchmark
   - Location: `/Users/wangbiao/Desktop/project/vm/benches/cross_arch_benchmark.rs`
   - 9 test groups covering basic translation performance

2. **cross_arch_comprehensive_bench.rs** - New comprehensive suite
   - Location: `/Users/wangbiao/Desktop/project/vm/benches/cross_arch_comprehensive_bench.rs`
   - 6 test groups covering all translation pairs and metrics

## Test Coverage

### Translation Pairs (6)
- x86_64 → ARM64 ✅
- x86_64 → RISC-V64 ✅ (NEW)
- ARM64 → x86_64 ✅ (NEW)
- ARM64 → RISC-V64 ✅
- RISC-V64 → x86_64 ✅
- RISC-V64 → ARM64 ✅ (NEW)

### Metrics (4)
- Translation Speed (instructions/sec) ✅ (NEW)
- Translation Overhead (%) ✅ (NEW)
- Code Size Ratio ✅ (NEW)
- Translation Accuracy (%) ✅ (NEW)

### Workloads (7)
- Basic Arithmetic ✅
- Memory Operations ✅
- Control Flow ✅
- Function Prologue/Epilogue ✅ (NEW)
- Loop Structures ✅ (NEW)
- Switch Statements ✅ (NEW)
- Complex Mixed ✅ (NEW)

## Example Output

```
Translation Pairs:
-------------------
x86_64_to_arm64/10
    time:   [1.2345 µs 1.2500 µs 1.2655 µs]
x86_64_to_arm64/50
    time:   [5.6789 µs 5.7000 µs 5.7211 µs]

Performance Metrics:
--------------------
x86_64_to_arm64: 800000.00 instructions/second
x86_64_to_arm64 code size ratio: 1.25x (100 IR ops -> 125 target instructions)

Translation Accuracy:
---------------------
✓ x86_64 -> ARM64 for basic block: 125 instructions generated
✓ x86_64 -> ARM64 for memory block: 130 instructions generated
✓ x86_64 -> ARM64 for control_flow block: 95 instructions generated

Translation Accuracy: 95.24% (40/42)
```

## Customization

### Change Instruction Counts
Edit the benchmark files and modify:
```rust
let instruction_counts = [10, 50, 100, 500, 1000];
```

### Change Thread Counts
```rust
let thread_counts = [1, 2, 4, 8];
```

### Change Cache Sizes
```rust
let cache_sizes = [256, 1024, 4096, 16384, 65536];
```

## Tips

1. **For Quick Testing**: Use smaller instruction counts [10, 50, 100]
2. **For Production Benchmarks**: Use [100, 500, 1000, 5000]
3. **For Cache Analysis**: Test multiple cache sizes
4. **For Accuracy**: Run multiple iterations and use medians

## Troubleshooting

### Build Issues
```bash
cargo clean
cargo update
cargo build --benches
```

### Feature Conflicts
```bash
cargo tree --features all  # Check for conflicts
```

### View Detailed Logs
```bash
cargo bench --bench cross_arch_comprehensive_bench -- --verbose
```

## Next Steps

1. Run the benchmarks to establish baseline
2. Save baseline: `cargo bench -- --save-baseline main`
3. Make code changes
4. Compare: `cargo bench -- --baseline main`
5. Check HTML reports for regressions

## Files Summary

| File | Lines | Test Groups | Status |
|------|-------|-------------|--------|
| cross_arch_benchmark.rs | 515 | 9 | ✅ Enhanced |
| cross_arch_comprehensive_bench.rs | 650 | 6 | ✅ New |

Total: **1165 lines** of comprehensive benchmark code
