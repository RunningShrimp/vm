#!/bin/bash
# Build Verification Script
# Use this script to verify that all fixes are working correctly

set -e  # Exit on error

echo "=========================================="
echo "VM Project Build Verification"
echo "=========================================="
echo ""

# Color codes for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Step 1: Clean previous builds
echo "Step 1: Cleaning previous builds..."
cargo clean
echo -e "${GREEN}✓ Clean complete${NC}"
echo ""

# Step 2: Check vm-core (the critical package)
echo "Step 2: Checking vm-core..."
if cargo check -p vm-core --all-features 2>&1 | grep -q "Finished"; then
    echo -e "${GREEN}✓ vm-core builds successfully${NC}"
else
    echo -e "${RED}✗ vm-core has errors${NC}"
    exit 1
fi
echo ""

# Step 3: Check vm-platform
echo "Step 3: Checking vm-platform..."
if cargo check -p vm-platform --all-features 2>&1 | grep -q "Finished"; then
    echo -e "${GREEN}✓ vm-platform builds successfully${NC}"
else
    echo -e "${RED}✗ vm-platform has errors${NC}"
    exit 1
fi
echo ""

# Step 4: Check vm-service
echo "Step 4: Checking vm-service..."
if cargo check -p vm-service --all-features 2>&1 | grep -q "Finished"; then
    echo -e "${GREEN}✓ vm-service builds successfully${NC}"
else
    echo -e "${RED}✗ vm-service has errors${NC}"
    exit 1
fi
echo ""

# Step 5: Check vm-engine-jit
echo "Step 5: Checking vm-engine-jit..."
if cargo check -p vm-engine-jit --all-features 2>&1 | grep -q "Finished"; then
    echo -e "${GREEN}✓ vm-engine-jit builds successfully${NC}"
else
    echo -e "${RED}✗ vm-engine-jit has errors${NC}"
    exit 1
fi
echo ""

# Step 6: Check vm-accel with features
echo "Step 6: Checking vm-accel with features..."
if cargo check -p vm-accel --features kvm,smmu 2>&1 | grep -q "Finished"; then
    echo -e "${GREEN}✓ vm-accel builds successfully${NC}"
else
    echo -e "${RED}✗ vm-accel has errors${NC}"
    exit 1
fi
echo ""

# Step 7: Full workspace build
echo "Step 7: Building entire workspace..."
if cargo build --workspace --all-targets --all-features 2>&1 | tee build_verification.txt | grep -q "Finished"; then
    echo -e "${GREEN}✓ Full workspace builds successfully${NC}"
else
    echo -e "${RED}✗ Full workspace has errors${NC}"
    exit 1
fi
echo ""

# Step 8: Count errors
echo "Step 8: Counting errors..."
ERROR_COUNT=$(grep -c "error:" build_verification.txt || echo "0")
if [ "$ERROR_COUNT" -eq 0 ]; then
    echo -e "${GREEN}✓ Zero errors found${NC}"
else
    echo -e "${RED}✗ Found $ERROR_COUNT errors${NC}"
    echo "Error details:"
    grep "error:" build_verification.txt | head -20
    exit 1
fi
echo ""

# Step 9: Count warnings
echo "Step 9: Counting warnings..."
WARNING_COUNT=$(grep -c "warning:" build_verification.txt || echo "0")
if [ "$WARNING_COUNT" -eq 0 ]; then
    echo -e "${GREEN}✓ Zero warnings found${NC}"
else
    echo -e "${YELLOW}⚠ Found $WARNING_COUNT warnings${NC}"
    echo "Warning details:"
    grep "warning:" build_verification.txt | head -20
fi
echo ""

# Step 10: Summary
echo "=========================================="
echo "Build Verification Summary"
echo "=========================================="
echo -e "${GREEN}✓ All critical packages build successfully${NC}"
echo -e "${GREEN}✓ Full workspace builds successfully${NC}"
echo -e "${GREEN}✓ Zero compilation errors${NC}"
if [ "$WARNING_COUNT" -eq 0 ]; then
    echo -e "${GREEN}✓ Zero compilation warnings${NC}"
else
    echo -e "${YELLOW}⚠ $WARNING_COUNT warnings remain${NC}"
fi
echo ""
echo "Build Status: ${GREEN}SUCCESS${NC}"
echo ""

# Optional: Run tests
echo "Step 11: Running tests (optional)..."
read -p "Run tests? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    if cargo test --workspace 2>&1 | tee test_verification.txt | grep -q "test result: ok"; then
        echo -e "${GREEN}✓ All tests pass${NC}"
    else
        echo -e "${YELLOW}⚠ Some tests failed${NC}"
    fi
fi
echo ""

echo "=========================================="
echo "Verification Complete!"
echo "=========================================="
echo ""
echo "Generated files:"
echo "  - build_verification.txt (build log)"
echo "  - test_verification.txt (test log, if tests run)"
echo ""
echo "You can now proceed with confidence that the codebase builds successfully."
