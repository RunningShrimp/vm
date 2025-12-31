#!/bin/bash
# Performance Regression Detection Script
# This script detects performance regressions by comparing benchmark results

set -e

# Configuration
REGRESSION_THRESHOLD=${REGRESSION_THRESHOLD:-10}  # 10% regression threshold
WARNING_THRESHOLD=${WARNING_THRESHOLD:-5}           # 5% warning threshold
BASELINE_DIR="${BASELINE_DIR:-target/criterion}"
CURRENT_DIR="${CURRENT_DIR:-target/criterion}"
OUTPUT_FILE="${OUTPUT_FILE:-regression-report.md}"

# Colors for output
RED='\033[0;31m'
YELLOW='\033[1;33m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color

echo "# Performance Regression Report" > "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"
echo "Generated at: $(date -u)" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"
echo "## Configuration" >> "$OUTPUT_FILE"
echo "- Regression Threshold: ${REGRESSION_THRESHOLD}%" >> "$OUTPUT_FILE"
echo "- Warning Threshold: ${WARNING_THRESHOLD}%" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"

# Function to compare benchmark results
compare_benchmarks() {
    local benchmark_name=$1
    local baseline_file="$BASELINE_DIR/$benchmark_name/new/estimates.json"
    local current_file="$CURRENT_DIR/$benchmark_name/new/estimates.json"

    if [[ ! -f "$baseline_file" ]] || [[ ! -f "$current_file" ]]; then
        echo "Skipping $benchmark_name (missing data)" >> "$OUTPUT_FILE"
        return
    fi

    # Extract mean times (in nanoseconds)
    local baseline_mean=$(jq -r '.mean.point_estimate' "$baseline_file" 2>/dev/null || echo "0")
    local current_mean=$(jq -r '.mean.point_estimate' "$current_file" 2>/dev/null || echo "0")

    if [[ "$baseline_mean" == "0" ]] || [[ "$current_mean" == "0" ]]; then
        echo "Skipping $benchmark_name (invalid data)" >> "$OUTPUT_FILE"
        return
    fi

    # Calculate percentage change
    local change=$(echo "scale=2; (($current_mean - $baseline_mean) / $baseline_mean) * 100" | bc)

    echo "### $benchmark_name" >> "$OUTPUT_FILE"
    echo "- Baseline: ${baseline_mean} ns" >> "$OUTPUT_FILE"
    echo "- Current: ${current_mean} ns" >> "$OUTPUT_FILE"
    echo "- Change: ${change}%" >> "$OUTPUT_FILE"

    # Determine status
    local is_regression=$(echo "$change > $REGRESSION_THRESHOLD" | bc -l)
    local is_warning=$(echo "$change > $WARNING_THRESHOLD" | bc -l)
    local is_improvement=$(echo "$change < -$WARNING_THRESHOLD" | bc -l)

    if [[ "$is_regression" == "1" ]]; then
        echo "- Status: 🔴 REGRESSION" >> "$OUTPUT_FILE"
        echo -e "${RED}❌ REGRESSION${NC}: $benchmark_name (+${change}%)"
        RETURN_CODE=1
    elif [[ "$is_warning" == "1" ]]; then
        echo "- Status: 🟡 WARNING" >> "$OUTPUT_FILE"
        echo -e "${YELLOW}⚠️  WARNING${NC}: $benchmark_name (+${change}%)"
    elif [[ "$is_improvement" == "1" ]]; then
        echo "- Status: 🟢 IMPROVEMENT" >> "$OUTPUT_FILE"
        echo -e "${GREEN}✅ IMPROVEMENT${NC}: $benchmark_name (${change}%)"
    else
        echo "- Status: ✅ STABLE" >> "$OUTPUT_FILE"
    fi

    echo "" >> "$OUTPUT_FILE"
}

# Find all benchmark directories
echo "## Benchmark Results" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"

RETURN_CODE=0

# Iterate through all benchmark directories
for benchmark_dir in "$CURRENT_DIR"/*/; do
    if [[ -d "$benchmark_dir" ]]; then
        benchmark_name=$(basename "$benchmark_dir")
        compare_benchmarks "$benchmark_name"
    fi
done

# Print summary
echo "" >> "$OUTPUT_FILE"
echo "## Summary" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"

if [[ $RETURN_CODE -eq 0 ]]; then
    echo "✅ No performance regressions detected!" >> "$OUTPUT_FILE"
    echo -e "${GREEN}✅ No performance regressions detected!${NC}"
else
    echo "❌ Performance regressions detected!" >> "$OUTPUT_FILE"
    echo -e "${RED}❌ Performance regressions detected!${NC}"
fi

# Print report to stdout
cat "$OUTPUT_FILE"

exit $RETURN_CODE
