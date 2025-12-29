#!/bin/bash
# Benchmark Runner Script
# Runs all VM benchmarks and generates reports

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
BENCHMARK_DIR="benches"
RESULTS_DIR="benchmark-results"
BASELINE_DIR="$BENCHMARK_DIR/baselines"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

# Create results directory
mkdir -p "$RESULTS_DIR"
mkdir -p "$BASELINE_DIR"

echo -e "${GREEN}=== VM Benchmark Runner ===${NC}"
echo "Starting at $(date)"
echo ""

# Function to run a single benchmark
run_benchmark() {
    local bench_name=$1
    local bench_file=$2

    echo -e "${YELLOW}Running $bench_name...${NC}"

    if cargo bench --bench "$bench_file" -- --save-baseline main --output-format bencher | tee "$RESULTS_DIR/${bench_name}_${TIMESTAMP}.txt"; then
        echo -e "${GREEN}✓ $bench_name completed${NC}"
        return 0
    else
        echo -e "${RED}✗ $bench_name failed${NC}"
        return 1
    fi
}

# Function to compare with baseline
compare_baseline() {
    local bench_name=$1

    if [ -f "$BASELINE_DIR/baselines.json" ]; then
        echo -e "${YELLOW}Comparing $bench_name with baseline...${NC}"
        cargo bench --bench "$bench_name" -- --baseline main
    fi
}

# Parse command line arguments
BENCHMARKS=""
UPDATE_BASELINE=false
PLOT_RESULTS=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --all)
            BENCHMARKS="all"
            shift
            ;;
        --cross-arch)
            BENCHMARKS="$BENCHMARKS cross_arch_translation_bench"
            shift
            ;;
        --snapshot)
            BENCHMARKS="$BENCHMARKS snapshot_bench"
            shift
            ;;
        --jit)
            BENCHMARKS="$BENCHMARKS jit_compilation_bench"
            shift
            ;;
        --gc)
            BENCHMARKS="$BENCHMARKS gc_bench"
            shift
            ;;
        --memory)
            BENCHMARKS="$BENCHMARKS memory_allocation_bench"
            shift
            ;;
        --device-io)
            BENCHMARKS="$BENCHMARKS device_io_bench"
            shift
            ;;
        --update-baseline)
            UPDATE_BASELINE=true
            shift
            ;;
        --plot)
            PLOT_RESULTS=true
            shift
            ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --all                  Run all benchmarks (default)"
            echo "  --cross-arch          Run cross-architecture translation benchmarks"
            echo "  --snapshot            Run snapshot performance benchmarks"
            echo "  --jit                 Run JIT compilation benchmarks"
            echo "  --gc                  Run GC performance benchmarks"
            echo "  --memory              Run memory allocation benchmarks"
            echo "  --device-io           Run device I/O benchmarks"
            echo "  --update-baseline     Update baseline values"
            echo "  --plot                Generate plots from results"
            echo "  --help                Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Default to all benchmarks if none specified
if [ -z "$BENCHMARKS" ]; then
    BENCHMARKS="all"
fi

# Run benchmarks
SUCCESS_COUNT=0
FAIL_COUNT=0

if [ "$BENCHMARKS" = "all" ]; then
    echo -e "${GREEN}Running all benchmarks...${NC}"
    echo ""

    benchmarks=(
        "cross_arch_translation_bench"
        "snapshot_bench"
        "jit_compilation_bench"
        "gc_bench"
        "memory_allocation_bench"
        "device_io_bench"
    )

    for bench in "${benchmarks[@]}"; do
        if run_benchmark "$bench" "$bench"; then
            ((SUCCESS_COUNT++))
        else
            ((FAIL_COUNT++))
        fi
        echo ""
    done
else
    for bench in $BENCHMARKS; do
        if run_benchmark "$bench" "$bench"; then
            ((SUCCESS_COUNT++))
        else
            ((FAIL_COUNT++))
        fi
        echo ""
    done
fi

# Update baseline if requested
if [ "$UPDATE_BASELINE" = true ]; then
    echo -e "${YELLOW}Updating baseline...${NC}"
    cp -r target/criterion/main "$BASELINE_DIR/"
    echo -e "${GREEN}✓ Baseline updated${NC}"
fi

# Generate summary
echo ""
echo -e "${GREEN}=== Benchmark Summary ===${NC}"
echo "Successful: $SUCCESS_COUNT"
echo "Failed: $FAIL_COUNT"
echo "Completed at $(date)"
echo ""
echo "Results saved to: $RESULTS_DIR/"

# Generate plots if requested
if [ "$PLOT_RESULTS" = true ]; then
    echo -e "${YELLOW}Generating plots...${NC}"
    # Note: This requires criterion plotting tools
    # You may need to install additional tools for this
    echo "Plots would be generated here"
fi

exit $FAIL_COUNT
