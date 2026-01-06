#!/bin/bash
# Round 14: Performance Verification Script
#
# This script runs performance benchmarks to verify the memcpy optimization impact

echo "=========================================="
echo "Round 14: Performance Benchmark Verification"
echo "=========================================="
echo ""

# Check if benchmark is still running
if pgrep -f "adaptive_memcpy_bench" > /dev/null; then
    echo "⏳ Benchmark still running, waiting for completion..."
    wait
fi

echo "📊 Running adaptive memcpy benchmark..."
echo ""

# Run the benchmark
cargo bench --bench adaptive_memcpy_bench 2>&1 | tee /tmp/bench_output.txt

echo ""
echo "=========================================="
echo "Benchmark Results Summary"
echo "=========================================="
echo ""

# Extract and display key metrics
echo "Key Performance Metrics:"
echo "-------------------------"
grep -A 5 "adaptive_comparison" /tmp/bench_output.txt | head -10

echo ""
echo "✅ Round 14: Performance benchmark verification complete!"
echo ""
echo "📁 Full results saved to: /tmp/bench_output.txt"
echo ""
