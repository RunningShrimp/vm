#!/bin/bash
# Quality Gates Check Script
# Run all quality checks locally before pushing

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Functions
print_header() {
    echo ""
    echo "=================================="
    echo "$1"
    echo "=================================="
    echo ""
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

# Check if required tools are installed
check_tool() {
    if ! command -v $1 &> /dev/null; then
        print_error "$1 is not installed"
        return 1
    fi
    return 0
}

# Main checks
print_header "VM Project Quality Gates"
echo "Running all quality checks locally..."
echo ""

# Track failures
FAILURES=0

# 1. Format check
print_header "1. Format Check (rustfmt)"
if check_tool rustfmt; then
    if cargo fmt --all -- --check; then
        print_success "Format check passed"
    else
        print_error "Format check failed"
        print_warning "Run 'cargo fmt' to fix"
        FAILURES=$((FAILURES + 1))
    fi
else
    print_error "rustfmt not found. Install with: rustup component add rustfmt"
    FAILURES=$((FAILURES + 1))
fi

# 2. Clippy check
print_header "2. Clippy Check (Strict Mode)"
if check_tool cargo; then
    if cargo clippy --workspace --all-features --all-targets -- \
        -D warnings \
        -W clippy::all \
        -W clippy::pedantic \
        -W clippy::cargo \
        -W clippy::unwrap_used \
        -W clippy::expect_used \
        -W clippy::panic; then
        print_success "Clippy check passed"
    else
        print_error "Clippy check failed"
        print_warning "Fix warnings or run: cargo clippy --workspace --all-features --all-targets -- --fix"
        FAILURES=$((FAILURES + 1))
    fi
fi

# 3. Compilation check
print_header "3. Compilation Check"
if cargo build --workspace --all-features; then
    print_success "Debug build passed"
else
    print_error "Debug build failed"
    FAILURES=$((FAILURES + 1))
fi

if cargo build --workspace --all-features --release; then
    print_success "Release build passed"
else
    print_error "Release build failed"
    FAILURES=$((FAILURES + 1))
fi

# 4. Test suite
print_header "4. Test Suite"
if cargo test --workspace --all-features --no-fail-fast; then
    print_success "All tests passed"
else
    print_error "Some tests failed"
    print_warning "Run with: cargo test --workspace --all-features -- --nocapture"
    FAILURES=$((FAILURES + 1))
fi

# 5. Documentation check
print_header "5. Documentation Check"
if cargo doc --no-deps --workspace --all-features; then
    print_success "Documentation build passed"
else
    print_error "Documentation build failed"
    FAILURES=$((FAILURES + 1))
fi

# 6. Coverage check (optional)
print_header "6. Coverage Check (Optional)"
if check_tool cargo-llvm-cov; then
    if cargo llvm-cov --workspace --all-features --summary; then
        print_success "Coverage check passed"
    else
        print_warning "Coverage check failed (non-blocking)"
    fi
else
    print_warning "cargo-llvm-cov not installed (optional)"
    print_warning "Install with: cargo install cargo-llvm-cov"
fi

# 7. Security audit (optional)
print_header "7. Security Audit (Optional)"
if check_tool cargo-audit; then
    if cargo audit; then
        print_success "Security audit passed"
    else
        print_warning "Security vulnerabilities found (non-blocking)"
    fi
else
    print_warning "cargo-audit not installed (optional)"
    print_warning "Install with: cargo install cargo-audit"
fi

# Final summary
print_header "Summary"
if [ $FAILURES -eq 0 ]; then
    print_success "All required quality gates passed!"
    echo ""
    echo "You're ready to push your changes."
    echo ""
    echo "To run coverage check (optional):"
    echo "  cargo llvm-cov --workspace --all-features --html"
    echo "  open target/llvm-cov/html/index.html"
    exit 0
else
    print_error "$FAILURES quality gate(s) failed"
    echo ""
    echo "Please fix the failures above before pushing."
    echo ""
    echo "Quick fixes:"
    echo "  Format:    cargo fmt"
    echo "  Clippy:    cargo clippy --workspace --all-features --all-targets -- --fix"
    echo "  Tests:     cargo test --workspace --all-features"
    echo "  Docs:      cargo doc --no-deps --workspace --all-features"
    exit 1
fi
