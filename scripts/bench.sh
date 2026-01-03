#!/bin/bash
# Comprehensive Benchmark Runner Script
# Runs all VM performance benchmarks with detailed reporting
#
# Usage:
#   ./scripts/bench.sh                    # Run all benchmarks
#   ./scripts/bench.sh --jit              # Run only JIT benchmarks
#   ./scripts/bench.sh --memory           # Run only memory benchmarks
#   ./scripts/bench.sh --baseline         # Save baseline
#   ./scripts/bench.sh --compare          # Compare with baseline

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color
BOLD='\033[1m'

# Configuration
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BENCH_RESULTS_DIR="${PROJECT_ROOT}/target/criterion"
BASELINE_DIR="${PROJECT_ROOT}/benches/baselines"
REPORT_DIR="${PROJECT_ROOT}/benchmark-reports"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
REPORT_FILE="${REPORT_DIR}/benchmark_report_${TIMESTAMP}.md"

# Create directories
mkdir -p "$BASELINE_DIR"
mkdir -p "$REPORT_DIR"

# Counter variables
TOTAL_BENCHMARKS=0
PASSED_BENCHMARKS=0
FAILED_BENCHMARKS=0

# Helper functions
print_header() {
    echo -e "${BLUE}${BOLD}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}${BOLD}$1${NC}"
    echo -e "${BLUE}${BOLD}═══════════════════════════════════════════════════════════════${NC}"
}

print_section() {
    echo ""
    echo -e "${YELLOW}${BOLD}▶ $1${NC}"
    echo ""
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

print_info() {
    echo -e "${BLUE}ℹ $1${NC}"
}

run_benchmark() {
    local bench_name=$1
    local bench_path=$2
    local extra_args=$3

    ((TOTAL_BENCHMARKS++)) || true

    echo -e "${YELLOW}Running: ${bench_name}${NC}"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

    local start_time=$(date +%s)

    if cargo bench --bench "$bench_path" $extra_args 2>&1 | tee "${REPORT_DIR}/${bench_name}_${TIMESTAMP}.log"; then
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))

        print_success "${bench_name} completed in ${duration}s"
        ((PASSED_BENCHMARKS++)) || true
        return 0
    else
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))

        print_error "${bench_name} failed after ${duration}s"
        ((FAILED_BENCHMARKS++)) || true
        return 1
    fi
}

generate_summary_report() {
    local report_file=$1

    cat > "$report_file" << EOF
# VM Performance Benchmark Report

**Generated:** $(date '+%Y-%m-%d %H:%M:%S')
**Commit:** $(git rev-parse --short HEAD 2>/dev/null || echo "N/A")
**Branch:** $(git branch --show-current 2>/dev/null || echo "N/A")

## Summary

- **Total Benchmarks:** ${TOTAL_BENCHMARKS}
- **Passed:** ${PASSED_BENCHMARKS}
- **Failed:** ${FAILED_BENCHMARKS}
- **Success Rate:** $(( PASSED_BENCHMARKS * 100 / (TOTAL_BENCHMARKS > 0 ? TOTAL_BENCHMARKS : 1) ))%

## Benchmark Categories

### 1. JIT Compilation Performance

**Location:** \`perf-bench/benches/jit_performance.rs\`

**Benchmarks:**
- IR block compilation time (various block sizes)
- Code generation throughput
- Optimization pass overhead (O0-O3)
- Tiered compilation (Tier 1 vs Tier 2)
- Hot block recompilation
- Function compilation (multi-block)

**Key Metrics:**
- Compilation time per instruction
- Code size expansion ratio
- Optimization overhead

**Baseline:**
- Small blocks (10 instr): ~TBD μs
- Medium blocks (100 instr): ~TBD μs
- Large blocks (1000 instr): ~TBD μs

### 2. Memory Operations

**Location:** \`perf-bench/benches/memory_operations.rs\`

**Benchmarks:**
- Memory copy speed (64B - 64KB)
- MMU translation latency
- TLB hit/miss rates
- TLB lookup latency
- TLB flush overhead
- Memory allocation/deallocation
- Memory access patterns (sequential, random, strided)

**Key Metrics:**
- Copy throughput (GB/s)
- Translation latency (ns)
- TLB hit rate (%)
- Allocation throughput (ops/ms)

**Baseline:**
- Sequential copy (4KB): ~TBD GB/s
- MMU translation: ~TBD ns
- TLB hit rate: ~TBD%

### 3. Garbage Collection

**Location:** \`perf-bench/benches/gc_performance.rs\`

**Benchmarks:**
- Minor GC pause time
- Major GC pause time
- Allocation throughput
- GC throughput (bytes reclaimed/ms)
- Generational efficiency
- Collection frequency
- Live data ratio impact
- Heap size impact

**Key Metrics:**
- Pause time (ms)
- Throughput (ops/ms)
- Reclamation rate (MB/s)

**Baseline:**
- Minor GC pause: ~TBD ms
- Major GC pause: ~TBD ms
- Allocation throughput: ~TBD ops/ms

### 4. Cross-Architecture Translation

**Location:** \`perf-bench/benches/cross_arch_translation.rs\`

**Benchmarks:**
- Single instruction translation (all arch pairs)
- Block translation throughput
- Translation cache effectiveness
- Instruction density impact
- Translation throughput (bytes/s)
- Complex instructions (arithmetic, memory, SIMD)
- Translation optimization levels

**Architecture Pairs:**
- x86_64 → ARM64
- x86_64 → RISC-V
- ARM64 → x86_64
- ARM64 → RISC-V
- RISC-V → x86_64
- RISC-V → ARM64

**Key Metrics:**
- Translation speed (instr/s)
- Cache hit rate (%)
- Code expansion ratio

**Baseline:**
- x86_64 → ARM64: ~TBD instr/s
- Cache hit rate: ~TBD%

## How to Run Benchmarks

### Run All Benchmarks
\`\`\`bash
./scripts/bench.sh
\`\`\`

### Run Specific Category
\`\`\`bash
./scripts/bench.sh --jit           # JIT compilation only
./scripts/bench.sh --memory        # Memory operations only
./scripts/bench.sh --gc            # GC only
./scripts/bench.sh --cross-arch    # Cross-arch only
\`\`\`

### Save Baseline
\`\`\`bash
./scripts/bench.sh --save-baseline
\`\`\`

### Compare with Baseline
\`\`\`bash
./scripts/bench.sh --compare-baseline
\`\`\`

## Detailed Results

HTML reports are available in \`target/criterion/\`:
\`\`\`
open target/criterion/<benchmark_name>/report/index.html
\`\`\`

## Interpreting Results

### Performance Metrics

- **Mean:** Average execution time
- **Std Dev:** Variability (lower is better)
- **95% CI:** Confidence interval
- **Throughput:** Operations per time unit

### Regression Detection

- 🔴 **High severity:** >20% slowdown
- 🟡 **Medium severity:** >10% slowdown
- 🟢 **Improvement:** >5% speedup
- ✅ **Stable:** Within ±5%

## System Information

**CPU:** $(sysctl -n machdep.cpu.brand_string 2>/dev/null || uname -m)
**Memory:** $(sysctl -n hw.memsize 2>/dev/null | awk '{printf "%.2f GB", $1/1024/1024/1024}' || echo "N/A")
**OS:** $(uname -s) $(uname -r)
**Rust:** $(rustc --version 2>/dev/null || echo "N/A")

## Notes

- All benchmarks run with statistical analysis via Criterion.rs
- Results include warm-up and multiple iterations for accuracy
- For reliable comparisons, use consistent hardware and load conditions
- See \`docs/BENCHMARKING.md\` for detailed documentation

---

*Generated by VM Benchmark Framework v1.0*
EOF

    print_success "Report generated: ${report_file}"
}

# Parse command line arguments
RUN_ALL=true
RUN_JIT=false
RUN_MEMORY=false
RUN_GC=false
RUN_CROSS_ARCH=false
SAVE_BASELINE=false
COMPARE_BASELINE=false
GENERATE_REPORT=true

while [[ $# -gt 0 ]]; do
    case $1 in
        --all)
            RUN_ALL=true
            shift
            ;;
        --jit)
            RUN_ALL=false
            RUN_JIT=true
            shift
            ;;
        --memory)
            RUN_ALL=false
            RUN_MEMORY=true
            shift
            ;;
        --gc)
            RUN_ALL=false
            RUN_GC=true
            shift
            ;;
        --cross-arch)
            RUN_ALL=false
            RUN_CROSS_ARCH=true
            shift
            ;;
        --save-baseline)
            SAVE_BASELINE=true
            shift
            ;;
        --compare-baseline)
            COMPARE_BASELINE=true
            shift
            ;;
        --no-report)
            GENERATE_REPORT=false
            shift
            ;;
        --help)
            echo "VM Benchmark Runner - Comprehensive Performance Testing"
            echo ""
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --all              Run all benchmarks (default)"
            echo "  --jit              Run JIT compilation benchmarks"
            echo "  --memory           Run memory operation benchmarks"
            echo "  --gc               Run garbage collection benchmarks"
            echo "  --cross-arch       Run cross-architecture translation benchmarks"
            echo "  --save-baseline    Save results as baseline"
            echo "  --compare-baseline Compare current results with baseline"
            echo "  --no-report        Skip generating report"
            echo "  --help             Show this help message"
            echo ""
            echo "Examples:"
            echo "  $0                           # Run all benchmarks"
            echo "  $0 --jit --save-baseline     # Run JIT and save as baseline"
            echo "  $0 --compare-baseline        # Compare with saved baseline"
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Main execution
print_header "VM Performance Benchmark Suite"
echo ""
print_info "Started at: $(date)"
print_info "Working directory: ${PROJECT_ROOT}"
print_info "Results directory: ${REPORT_DIR}"
echo ""

# Check prerequisites
print_section "Checking Prerequisites"

if ! command -v cargo &> /dev/null; then
    print_error "cargo not found. Please install Rust toolchain."
    exit 1
fi
print_success "Rust toolchain found"

if ! command -v git &> /dev/null; then
    print_info "git not found (optional, for report metadata)"
else
    print_success "git found"
fi

# Run benchmarks
print_section "Running Benchmarks"

# Benchmark extra arguments
EXTRA_ARGS=""
if [ "$SAVE_BASELINE" = true ]; then
    EXTRA_ARGS="-- --save-baseline main"
    print_info "Will save baseline"
fi

if [ "$COMPARE_BASELINE" = true ]; then
    EXTRA_ARGS="-- --baseline main"
    print_info "Will compare with baseline"
fi

# Run selected benchmarks
if [ "$RUN_ALL" = true ] || [ "$RUN_JIT" = true ]; then
    print_section "JIT Compilation Performance"
    run_benchmark "JIT Performance" "jit_performance" "$EXTRA_ARGS"
fi

if [ "$RUN_ALL" = true ] || [ "$RUN_MEMORY" = true ]; then
    print_section "Memory Operations"
    run_benchmark "Memory Operations" "memory_operations" "$EXTRA_ARGS"
fi

if [ "$RUN_ALL" = true ] || [ "$RUN_GC" = true ]; then
    print_section "Garbage Collection"
    run_benchmark "GC Performance" "gc_performance" "$EXTRA_ARGS"
fi

if [ "$RUN_ALL" = true ] || [ "$RUN_CROSS_ARCH" = true ]; then
    print_section "Cross-Architecture Translation"
    run_benchmark "Cross-Arch Translation" "cross_arch_translation" "$EXTRA_ARGS"
fi

# Print summary
print_header "Benchmark Summary"
echo ""
echo "Total Benchmarks:  ${TOTAL_BENCHMARKS}"
echo -e "Passed:            ${GREEN}${PASSED_BENCHMARKS}${NC}"
echo -e "Failed:            ${RED}${FAILED_BENCHMARKS}${NC}"
echo ""
echo "Completed at: $(date)"

# Generate report
if [ "$GENERATE_REPORT" = true ]; then
    print_section "Generating Report"
    generate_summary_report "$REPORT_FILE"
fi

# Exit with appropriate code
if [ $FAILED_BENCHMARKS -gt 0 ]; then
    print_error "Some benchmarks failed"
    exit 1
else
    print_success "All benchmarks passed successfully!"
    exit 0
fi
