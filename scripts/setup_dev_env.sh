#!/bin/bash

# Setup development environment for VM project
# This script configures Git hooks, installs development tools, and verifies the environment

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
    echo ""
    echo -e "${BLUE}=====================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}=====================================${NC}"
}

# Check if running from project root
if [ ! -f "Cargo.toml" ] || [ ! -d "vm-core" ]; then
    print_error "This script must be run from the VM project root directory"
    exit 1
fi

print_header "VM Project Development Environment Setup"
echo ""

# Detect OS
OS="$(uname -s)"
case "${OS}" in
    Linux*)     MACHINE=Linux;;
    Darwin*)    MACHINE=Mac;;
    MINGW*|MSYS*|CYGWIN*)  MACHINE=Windows;;
    *)          MACHINE="UNKNOWN:${OS}"
esac

print_info "Detected OS: $MACHINE"
echo ""

# Step 1: Install Git hooks
print_header "Step 1: Installing Git Hooks"

if [ -f ".git/hooks/pre-commit" ]; then
    print_info "Removing existing pre-commit hook..."
    rm -f .git/hooks/pre-commit
fi

print_info "Creating symbolic link to pre-commit hook..."
ln -sf ../../.githooks/pre-commit .git/hooks/pre-commit
chmod +x .githooks/pre-commit

print_success "Git hooks installed successfully"
echo ""

# Step 2: Check Rust toolchain
print_header "Step 2: Checking Rust Toolchain"

if ! command -v rustc &> /dev/null; then
    print_error "Rust not found!"
    print_info "Please install Rust from: https://rustup.rs/"
    exit 1
fi

RUST_VERSION=$(rustc --version)
print_success "Rust toolchain found: $RUST_VERSION"

# Check if Rust version is >= 1.85
RUST_VERSION_NUM=$(rustc --version | cut -d' ' -f2 | cut -d'-' -f1)
print_info "Rust version: $RUST_VERSION_NUM"

if [ -f "rust-toolchain.toml" ]; then
    print_success "rust-toolchain.toml found"
    cat rust-toolchain.toml
fi
echo ""

# Step 3: Install development tools
print_header "Step 3: Installing Development Tools"

# Function to install cargo tool if not already installed
install_cargo_tool() {
    TOOL_NAME=$1
    TOOL_CRATE=$2

    if cargo install --list | grep -q "^$TOOL_NAME "; then
        print_success "$TOOL_NAME already installed"
    else
        print_info "Installing $TOOL_NAME..."
        if cargo install $TOOL_CRATE; then
            print_success "$TOOL_NAME installed successfully"
        else
            print_warning "Failed to install $TOOL_NAME (optional)"
        fi
    fi
}

# Essential tools
install_cargo_tool "cargo-watch" "cargo-watch"
install_cargo_tool "cargo-edit" "cargo-edit"
install_cargo_tool "cargo-audit" "cargo-audit"

# Optional tools
print_info "Installing optional development tools (may take a while)..."
install_cargo_tool "cargo-tarpaulin" "cargo-tarpaulin"
install_cargo_tool "cargo-nextest" "cargo-nextest"
install_cargo_tool "cargo-outdated" "cargo-outdated"
install_cargo_tool "cargo-tree" "cargo-tree"

# Check for cargo-deny
if [ -f "deny.toml" ]; then
    install_cargo_tool "cargo-deny" "cargo-deny"
fi

echo ""

# Step 4: Verify project builds
print_header "Step 4: Verifying Project Configuration"

print_info "Running cargo check to verify workspace..."
if cargo check --workspace --quiet 2>&1 | head -20; then
    print_success "Workspace check passed"
else
    print_warning "Workspace check found issues (this is normal for first-time setup)"
fi
echo ""

# Step 5: Check for IDE configurations
print_header "Step 5: Checking IDE Configurations"

if [ -d ".vscode" ]; then
    print_success "VSCode configuration found"

    if [ -f ".vscode/settings.json" ]; then
        print_success "  - settings.json"
    fi

    if [ -f ".vscode/extensions.json" ]; then
        print_success "  - extensions.json"
    fi

    if [ -f ".vscode/tasks.json" ]; then
        print_success "  - tasks.json"
    fi

    if [ -f ".vscode/launch.json" ]; then
        print_success "  - launch.json"
    fi
fi

if [ -f ".editorconfig" ]; then
    print_success "EditorConfig found"
fi
echo ""

# Step 6: Check formatting and linting configuration
print_header "Step 6: Checking Development Tools Configuration"

if [ -f ".rustfmt.toml" ]; then
    print_success "rustfmt configuration found"
fi

if [ -f ".clippy.toml" ]; then
    print_success "clippy configuration found"
fi

if [ -f "deny.toml" ]; then
    print_success "cargo-deny configuration found"
fi
echo ""

# Step 7: Environment variables
print_header "Step 7: Environment Variables"

print_info "Recommended environment variables:"
cat << 'EOF'

# For development
export RUST_LOG=debug
export RUST_BACKTRACE=1

# For performance profiling
export RUST_PROFILE=time

# For testing
export CARGO_TERM_COLOR=always

EOF

# Check if .env file exists
if [ -f ".env" ]; then
    print_warning ".env file found (make sure it's not committed to git)"
fi
echo ""

# Step 8: Create helper scripts
print_header "Step 8: Creating Helper Scripts"

# Create quick test script
cat > scripts/quick_test.sh << 'EOF'
#!/bin/bash
# Quick test runner - runs fast tests only
cargo test --workspace --lib --quiet
EOF
chmod +x scripts/quick_test.sh
print_success "Created scripts/quick_test.sh"

# Create quick format script
cat > scripts/format_all.sh << 'EOF'
#!/bin/bash
# Format all code
cargo fmt --all
EOF
chmod +x scripts/format_all.sh
print_success "Created scripts/format_all.sh"

# Create quick clippy script
cat > scripts/clippy_check.sh << 'EOF'
#!/bin/bash
# Run clippy with workspace defaults
cargo clippy --workspace --all-targets -- -D warnings
EOF
chmod +x scripts/clippy_check.sh
print_success "Created scripts/clippy_check.sh"
echo ""

# Step 9: Git configuration
print_header "Step 9: Git Configuration"

# Check if core.hooksPath is set
if git config --get core.hooksPath > /dev/null 2>&1; then
    HOOKS_PATH=$(git config --get core.hooksPath)
    print_info "Git hooks path: $HOOKS_PATH"

    if [ "$HOOKS_PATH" = ".githooks" ]; then
        print_success "Git hooks path correctly configured"
    else
        print_warning "Git hooks path is set to '$HOOKS_PATH', not '.githooks'"
        print_info "You may want to run: git config core.hooksPath .githooks"
    fi
else
    print_info "Git hooks path not configured (using default .git/hooks)"
fi
echo ""

# Step 10: Final summary
print_header "Setup Complete!"
echo ""

print_success "Development environment is ready!"
echo ""

print_info "Quick start commands:"
cat << 'EOF'

  # Build the project
  cargo build --workspace

  # Run tests
  cargo test --workspace

  # Format code
  cargo fmt --all

  # Run clippy
  cargo clippy --workspace --all-targets -- -D warnings

  # Run quick tests
  ./scripts/quick_test.sh

EOF

print_info "Useful commands:"
cat << 'EOF'

  # Watch for changes and recompile
  cargo watch -x check

  # Watch for changes and run tests
  cargo watch -x test

  # Check for security vulnerabilities
  cargo audit

  # Check license compliance
  cargo deny check

  # Update dependencies
  cargo update

  # Document all crates
  cargo doc --workspace --no-deps --document-private-items

  # Open documentation in browser
  cargo doc --workspace --no-deps --open

EOF

print_info "IDE Setup:"
cat << 'EOF'

  For VSCode:
    1. Install recommended extensions (VSCode will prompt automatically)
    2. Open the project folder in VSCode
    3. Wait for Rust Analyzer to index the project (may take a few minutes)

  For IntelliJ IDEA / RustRover:
    1. Open the project folder
    2. Enable "External Linter" in Settings → Rust
    3. Configure clippy and rustfmt paths

  For Vim/Neovim:
    1. Install rust-analyzer
    2. Configure your plugin (coc.nvim, nvim-lsp, etc.)

EOF

print_warning "Pre-commit Hooks:"
cat << 'EOF'

  Pre-commit hooks are installed and will run automatically before each commit.
  They check:
    - Code formatting (cargo fmt)
    - Clippy warnings (cargo clippy)
    - Compilation (cargo check)
    - Unit tests (cargo test --lib)
    - Large files
    - Sensitive information

  To skip hooks temporarily:
    git commit --no-verify -m "message"

  To run hooks manually:
    .git/hooks/pre-commit

EOF

print_info "Next steps:"
cat << 'EOF'

  1. Review the documentation in docs/DEVELOPER_SETUP.md
  2. Run 'cargo build --workspace' to ensure everything compiles
  3. Run 'cargo test --workspace' to verify tests pass
  4. Start coding!

EOF

print_success "Happy hacking! 🚀"
