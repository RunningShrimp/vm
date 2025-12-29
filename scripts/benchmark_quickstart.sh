#!/bin/bash
# Quick-start script for running benchmarks locally

set -e

echo "🚀 VM Benchmark Quick Start"
echo "=========================="
echo ""

# Check if we're in project root
if [ ! -f "Cargo.toml" ]; then
    echo "❌ Error: Must be run from project root directory"
    exit 1
fi

# Check if Python 3 is available
if ! command -v python3 &> /dev/null; then
    echo "❌ Error: Python 3 is required for regression detection"
    exit 1
fi

echo "✓ Environment checks passed"
echo ""

# Menu
echo "Select an option:"
echo "1) Run all benchmarks (quick)"
echo "2) Run all benchmarks with full reports"
echo "3) Run specific benchmark"
echo "4) Check for regressions only"
echo "5) Update baseline"
echo "6) Compare with baseline"
echo ""
read -p "Enter choice [1-6]: " choice

case $choice in
    1)
        echo ""
        echo "Running all benchmarks..."
        cargo bench --workspace --all-features
        ;;
    2)
        echo ""
        echo "Running full benchmark suite..."
        ./scripts/run_benchmarks.sh
        echo ""
        echo "Generating reports..."
        python3 scripts/generate_benchmark_report.py
        echo ""
        echo "✓ Full report generated: benchmark-report.md"
        ;;
    3)
        echo ""
        echo "Available benchmarks:"
        cargo bench --workspace --all-features -- --list
        echo ""
        read -p "Enter benchmark name: " bench_name
        echo ""
        echo "Running benchmark: $bench_name"
        cargo bench --bench "$bench_name" --all-features
        ;;
    4)
        echo ""
        echo "Checking for regressions..."
        if [ -d "target/criterion" ]; then
            python3 scripts/detect_regression.py
        else
            echo "❌ Error: No benchmark results found"
            echo "   Run benchmarks first with option 1 or 2"
            exit 1
        fi
        ;;
    5)
        echo ""
        echo "Updating baseline..."
        cargo bench --workspace --all-features -- --save-baseline main
        echo ""
        echo "✓ Baseline updated"
        ;;
    6)
        echo ""
        echo "Comparing with baseline..."
        cargo bench --workspace --all-features -- --baseline main
        ;;
    *)
        echo "❌ Invalid choice"
        exit 1
        ;;
esac

echo ""
echo "=========================="
echo "✓ Done!"
echo ""
