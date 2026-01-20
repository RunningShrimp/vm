#!/bin/bash
# VM Project Test Execution Script
#
# This script runs the complete test suite for the VM project
# following the test plan defined in TEST_PLAN.md

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test counters
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0
SKIPPED_TESTS=0

# Log files
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
LOG_DIR="test-results/$TIMESTAMP"
mkdir -p "$LOG_DIR"

LOG_FILE="$LOG_DIR/test_run.log"
SUMMARY_FILE="$LOG_DIR/summary.txt"

echo "========================================" | tee -a "$LOG_FILE"
echo "VM Project Test Suite" | tee -a "$LOG_FILE"
echo "Started: $(date)" | tee -a "$LOG_FILE"
echo "========================================" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

# Function to print section header
print_section() {
    echo "" | tee -a "$LOG_FILE"
    echo -e "${YELLOW}>>> $1${NC}" | tee -a "$LOG_FILE"
    echo "----------------------------------------" | tee -a "$LOG_FILE"
}

# Function to run a test phase
run_test_phase() {
    local name="$1"
    local command="$2"
    local description="$3"

    print_section "$name"

    echo "Running: $description" | tee -a "$LOG_FILE"
    echo "Command: $command" | tee -a "$LOG_FILE"
    echo "" | tee -a "$LOG_FILE"

    local start_time=$(date +%s)

    if eval "$command" >> "$LOG_DIR/${name// /_}.log" 2>&1; then
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        echo -e "${GREEN}✓ PASSED${NC} (${duration}s)" | tee -a "$LOG_FILE"
        echo "$name: PASSED" >> "$SUMMARY_FILE"
        ((PASSED_TESTS++))
        return 0
    else
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        echo -e "${RED}✗ FAILED${NC} (${duration}s)" | tee -a "$LOG_FILE"
        echo "See: $LOG_DIR/${name// /_}.log for details" | tee -a "$LOG_FILE"
        echo "$name: FAILED" >> "$SUMMARY_FILE"
        ((FAILED_TESTS++))
        return 1
    fi
    ((TOTAL_TESTS++))
}

# Phase 1: Quick Health Check
print_section "Phase 1: Quick Health Check"

echo "Checking code formatting..." | tee -a "$LOG_FILE"
if cargo fmt --all -- --check >> "$LOG_FILE" 2>&1; then
    echo -e "${GREEN}✓ Code formatting OK${NC}" | tee -a "$LOG_FILE"
else
    echo -e "${YELLOW}⚠ Formatting issues found. Run 'cargo fmt --all' to fix.${NC}" | tee -a "$LOG_FILE"
fi

echo "" | tee -a "$LOG_FILE"
echo "Checking for compilation errors..." | tee -a "$LOG_FILE"
if cargo check --workspace >> "$LOG_FILE" 2>&1; then
    echo -e "${GREEN}✓ Code compiles${NC}" | tee -a "$LOG_FILE"
else
    echo -e "${RED}✗ Compilation errors found${NC}" | tee -a "$LOG_FILE"
    exit 1
fi

# Phase 2: Unit Tests
run_test_phase \
    "Unit Tests" \
    "cargo test --lib --workspace --no-fail-fast" \
    "Running unit tests across all crates"

# Phase 3: Integration Tests
run_test_phase \
    "Integration Tests" \
    "cargo test --test '*_integration*' --workspace --no-fail-fast" \
    "Running integration tests"

# Phase 4: Documentation Tests
run_test_phase \
    "Doc Tests" \
    "cargo test --doc --workspace" \
    "Running documentation tests"

# Phase 5: Clippy Analysis
print_section "Clippy Analysis"
echo "Running clippy linting..." | tee -a "$LOG_FILE"
if cargo clippy --all-targets --workspace -- -D warnings >> "$LOG_DIR/Clippy_Analysis.log" 2>&1; then
    echo -e "${GREEN}✓ No clippy warnings${NC}" | tee -a "$LOG_FILE"
    echo "Clippy: PASSED" >> "$SUMMARY_FILE"
else
    echo -e "${YELLOW}⚠ Clippy found issues (non-fatal)${NC}" | tee -a "$LOG_FILE"
    echo "Clippy: WARNINGS" >> "$SUMMARY_FILE"
fi

# Phase 6: Coverage (if available)
print_section "Code Coverage"
if command -v cargo-llvm-cov &> /dev/null; then
    echo "Generating coverage report..." | tee -a "$LOG_FILE"
    if cargo llvm-cov --workspace --html --output-dir "$LOG_DIR/coverage" >> "$LOG_FILE" 2>&1; then
        echo -e "${GREEN}✓ Coverage report generated${NC}" | tee -a "$LOG_FILE"
        echo "Coverage: See $LOG_DIR/coverage/index.html" >> "$SUMMARY_FILE"
    else
        echo -e "${YELLOW}⚠ Coverage generation failed${NC}" | tee -a "$LOG_FILE"
    fi
else
    echo "cargo-llvm-cov not installed. Skipping coverage." | tee -a "$LOG_FILE"
    echo "Install with: cargo install cargo-llvm-cov" | tee -a "$LOG_FILE"
fi

# Final Summary
print_section "Test Summary"

echo "Total test phases: $TOTAL_TESTS" | tee -a "$LOG_FILE"
echo "Passed: $PASSED_TESTS" | tee -a "$LOG_FILE"
echo "Failed: $FAILED_TESTS" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"
echo "Detailed logs available at: $LOG_DIR" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

if [ $FAILED_TESTS -eq 0 ]; then
    echo -e "${GREEN}All tests passed!${NC}" | tee -a "$LOG_FILE"
    exit 0
else
    echo -e "${RED}Some tests failed. Check the logs above.${NC}" | tee -a "$LOG_FILE"
    exit 1
fi
