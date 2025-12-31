#!/bin/bash

# Quick test runner - runs fast tests only
# This script runs library tests only, skipping integration and doc tests for speed

set -e

# Color definitions
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[✓]${NC} $1"
}

print_error() {
    echo -e "${RED}[✗]${NC} $1"
}

print_header() {
    echo -e "${BLUE}=====================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}=====================================${NC}"
}

print_header "Running Quick Tests"
print_info "Running library tests only (skipping integration and doc tests)..."

echo ""

# Run library tests only (fastest)
if cargo test --workspace --lib --quiet; then
    echo ""
    print_success "All quick tests passed! 🎉"
    exit 0
else
    echo ""
    print_error "Some tests failed!"
    echo ""
    print_info "For full tests, run: cargo test --workspace"
    exit 1
fi
