#!/bin/bash

# Format all Rust code in the workspace

set -e

# Color definitions
RED='\033[0;31m'
GREEN='\033[0;32m'
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

print_header "Formatting All Code"
print_info "Running rustfmt on all Rust files..."

echo ""

# Format all code
if cargo fmt --all; then
    echo ""
    print_success "Code formatted successfully! ✨"
    echo ""
    print_info "To check formatting without making changes, run:"
    echo "  cargo fmt --all -- --check"
    exit 0
else
    echo ""
    print_error "Formatting failed!"
    exit 1
fi
