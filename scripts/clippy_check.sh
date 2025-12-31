#!/bin/bash

# Run Clippy with workspace defaults
# This script runs Clippy with the project's recommended settings

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

print_warning() {
    echo -e "${YELLOW}[⚠]${NC} $1"
}

print_error() {
    echo -e "${RED}[✗]${NC} $1"
}

print_header() {
    echo -e "${BLUE}=====================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}=====================================${NC}"
}

print_header "Running Clippy"
print_info "Checking code with Clippy linter..."
print_info "Using: cargo clippy --workspace --all-targets -- -D warnings"

echo ""

# Run clippy with strict warnings
if cargo clippy --workspace --all-targets -- -D warnings; then
    echo ""
    print_success "No Clippy warnings found! 🎉"
    echo ""
    print_info "Your code is clean!"
    exit 0
else
    echo ""
    print_error "Clippy found issues!"
    echo ""
    print_info "To auto-fix some issues, run:"
    echo "  cargo clippy --workspace --fix --allow-dirty"
    echo ""
    print_info "For help with specific warnings, visit:"
    echo "  https://rust-lang.github.io/rust-clippy/"
    exit 1
fi
