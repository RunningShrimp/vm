# Benchmark Quick Start Guide

## Quick Reference

### Run Benchmarks Locally

```bash
# Option 1: Interactive quick start
./scripts/benchmark_quickstart.sh

# Option 2: Run all benchmarks
cargo bench --workspace --all-features

# Option 3: Run full suite with reports
./scripts/run_benchmarks.sh

# Option 4: Check for regressions
python3 scripts/detect_regression.py
```

### GitHub Actions

The CI/CD workflow automatically:
- ✅ Runs benchmarks on every PR
- ✅ Compares against main branch baseline
- ✅ Comments on PR with results
- ✅ Detects and alerts on regressions >10%
- ✅ Runs daily at 2 AM UTC

### Files Created

| File | Purpose |
|------|---------|
| `.github/workflows/benchmark.yml` | GitHub Actions workflow |
| `.cargo/config.toml` | Cargo benchmark configuration |
| `scripts/run_benchmarks.sh` | Benchmark runner script |
| `scripts/detect_regression.py` | Regression detection script |
| `scripts/generate_benchmark_report.py` | Report generator |
| `scripts/benchmark_quickstart.sh` | Interactive quick start |
| `benches/baselines/main.json` | Baseline performance data |
| `docs/BENCHMARKING.md` | Full documentation |

### First Time Setup

```bash
# 1. Run benchmarks to establish baseline
cargo bench --workspace --all-features

# 2. Check for regressions (will create baseline on first run)
python3 scripts/detect_regression.py

# 3. View results
open target/criterion/*/report/index.html
```

### Common Tasks

**View benchmark results:**
```bash
# HTML reports (in browser)
open target/criterion/

# Markdown report
cat benchmark-report.md
```

**Update baseline after optimization:**
```bash
cargo bench --workspace --all-features -- --save-baseline main
```

**Compare against baseline:**
```bash
cargo bench --workspace --all-features -- --baseline main
```

**Run specific benchmark:**
```bash
cargo bench --bench cross_arch_benchmark
```

### Understanding Results

- 🟢 **Improvement**: >5% faster than baseline
- ✅ **Stable**: Within acceptable range (±10%)
- 🟡 **Warning**: 10-20% slower than baseline
- 🔴 **Regression**: >20% slower than baseline

### CI/CD Integration

**On Pull Request:**
1. Benchmarks run automatically
2. Results compared to main branch
3. PR comment posted with summary
4. Fails if regressions detected

**On Push to Main:**
1. Benchmarks run automatically
2. Baseline updated
3. Results stored as artifacts
4. Daily tracking enabled

### Troubleshooting

**Benchmarks fail:**
```bash
# Check benchmark exists
cargo bench -- --list

# Run with verbose output
cargo bench -- --verbose
```

**Regression false positive:**
```bash
# Run multiple times
for i in {1..3}; do cargo bench; python3 scripts/detect_regression.py; done
```

**No baseline found:**
```bash
# Create baseline
cargo bench --workspace --all-features
python3 scripts/detect_regression.py
```

### Next Steps

1. Read full documentation: `docs/BENCHMARKING.md`
2. Check existing benchmarks: `ls benches/`
3. Add benchmarks for new features
4. Monitor CI results on PRs

### Need Help?

- Full documentation: `docs/BENCHMARKING.md`
- GitHub Issues: Report problems
- Discussions: Ask questions

---

*For detailed information, see [docs/BENCHMARKING.md](docs/BENCHMARKING.md)*
