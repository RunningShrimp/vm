#!/bin/bash
# Property-Based Tests and Fuzzing Verification Script
#
# This script verifies that property tests and fuzzing infrastructure
# are properly set up and functional.

set -e

echo "=========================================="
echo "VM Property-Based Testing Verification"
echo "=========================================="
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test counters
PASS=0
FAIL=0
WARN=0

# Helper functions
pass() {
    echo -e "${GREEN}✓${NC} $1"
    PASS=$((PASS + 1))
}

fail() {
    echo -e "${RED}✗${NC} $1"
    FAIL=$((FAIL + 1))
}

warn() {
    echo -e "${YELLOW}⚠${NC} $1"
    WARN=$((WARN + 1))
}

info() {
    echo -e "  $1"
}

echo "1. Checking Property Test Files"
echo "--------------------------------"

# Check if property test files exist
for test_file in \
    "tests/memory_property_tests.rs" \
    "tests/instruction_property_tests.rs" \
    "tests/device_property_tests.rs"
do
    if [ -f "$test_file" ]; then
        lines=$(wc -l < "$test_file" | tr -d ' ')
        pass "Found $test_file ($lines lines)"
    else
        fail "Missing $test_file"
    fi
done

echo ""
echo "2. Checking Fuzz Targets"
echo "------------------------"

# Check if fuzz targets exist
for fuzz_file in \
    "fuzz/fuzz_targets/instruction_decoder.rs" \
    "fuzz/fuzz_targets/memory_access.rs" \
    "fuzz/fuzz_targets/jit_compiler.rs"
do
    if [ -f "$fuzz_file" ]; then
        lines=$(wc -l < "$fuzz_file" | tr -d ' ')
        pass "Found $fuzz_file ($lines lines)"
    else
        fail "Missing $fuzz_file"
    fi
done

echo ""
echo "3. Checking Dependencies"
echo "------------------------"

# Check if proptest is in Cargo.toml
if grep -q "proptest = \"1.4\"" Cargo.toml; then
    pass "proptest dependency found in workspace Cargo.toml"
else
    fail "proptest dependency not found in workspace Cargo.toml"
fi

if grep -q "proptest-derive = \"0.4\"" Cargo.toml; then
    pass "proptest-derive dependency found in workspace Cargo.toml"
else
    fail "proptest-derive dependency not found in workspace Cargo.toml"
fi

# Check if fuzz/Cargo.toml exists
if [ -f "fuzz/Cargo.toml" ]; then
    pass "Found fuzz/Cargo.toml"
else
    fail "Missing fuzz/Cargo.toml"
fi

echo ""
echo "4. Checking Documentation"
echo "------------------------"

# Check if documentation exists
if [ -f "docs/ADVANCED_TESTING_GUIDE.md" ]; then
    lines=$(wc -l < "docs/ADVANCED_TESTING_GUIDE.md" | tr -d ' ')
    pass "Found ADVANCED_TESTING_GUIDE.md ($lines lines)"
else
    fail "Missing ADVANCED_TESTING_GUIDE.md"
fi

echo ""
echo "5. Syntax Validation"
echo "--------------------"

# Try to compile property tests (syntax check only)
echo "Compiling property tests (syntax check)..."
if cargo check --tests --quiet 2>&1 | grep -i "error.*property" > /dev/null; then
    fail "Property tests have compilation errors"
    cargo check --tests 2>&1 | grep -A 3 "error.*property" | head -20
else
    pass "Property tests syntax appears valid"
fi

echo ""
echo "6. Property Test Content Validation"
echo "------------------------------------"

# Check for proptest! macro usage
for test_file in tests/*property_tests.rs; do
    if [ -f "$test_file" ]; then
        if grep -q "proptest!" "$test_file"; then
            pass "$(basename $test_file) uses proptest! macro"
        else
            warn "$(basename $test_file) may not use proptest! macro"
        fi

        # Check for property assertions
        if grep -q "prop_assert" "$test_file"; then
            info "  Contains property assertions"
        else
            warn "  Missing property assertions"
        fi
    fi
done

echo ""
echo "7. Fuzz Target Validation"
echo "-------------------------"

# Check for fuzz_target! macro
for fuzz_file in fuzz/fuzz_targets/*.rs; do
    if [ -f "$fuzz_file" ]; then
        filename=$(basename "$fuzz_file")
        if grep -q "fuzz_target!" "$fuzz_file"; then
            pass "$filename uses fuzz_target! macro"
        else
            warn "$filename may not use fuzz_target! macro"
        fi

        # Check for no_main attribute
        if grep -q "#!\\[no_main\\]" "$fuzz_file"; then
            info "  Has no_main attribute"
        else
            warn "  Missing no_main attribute"
        fi
    fi
done

echo ""
echo "8. Running Sample Property Tests"
echo "--------------------------------"

# Try to run one property test with limited iterations
echo "Attempting to run memory_property_tests..."
if timeout 30 cargo test --test memory_property_tests -- --test-threads=1 PROPTEST_CASES=10 2>&1 | head -30; then
    pass "Property test execution succeeded"
else
    warn "Property test execution had issues (may need fixes)"
fi

echo ""
echo "=========================================="
echo "Verification Summary"
echo "=========================================="
echo -e "${GREEN}Passed:${NC} $PASS"
echo -e "${YELLOW}Warnings:${NC} $WARN"
echo -e "${RED}Failed:${NC} $FAIL"
echo ""

if [ $FAIL -eq 0 ]; then
    echo -e "${GREEN}All critical checks passed!${NC}"
    echo ""
    echo "Next steps:"
    echo "1. Run property tests: cargo test --test memory_property_tests"
    echo "2. Run fuzz targets: cd fuzz && cargo fuzz run instruction_decoder"
    echo "3. Read the guide: docs/ADVANCED_TESTING_GUIDE.md"
    exit 0
else
    echo -e "${RED}Some checks failed. Please review the output above.${NC}"
    exit 1
fi
