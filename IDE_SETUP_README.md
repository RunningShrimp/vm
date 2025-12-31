# VM Project - Development Environment Configuration

This directory contains comprehensive IDE and development environment configurations for the VM project.

## Quick Start

```bash
# Run the setup script
./scripts/setup_dev_env.sh

# Start developing
code .                    # VSCode
idea .                    # IntelliJ/RustRover
nvim .                    # Neovim
```

## Configuration Files

### Root Directory

- **`.editorconfig`**: Editor-agnostic code style settings
- **`.rustfmt.toml`**: Rust formatting configuration
- **`.clippy.toml`**: Clippy lint configuration
- **`.githooks/`**: Git hooks for code quality

### VSCode Configuration (`.vscode/`)

- **`settings.json`**: Rust Analyzer, formatting, and editor settings
- **`extensions.json`**: Recommended VSCode extensions
- **`tasks.json`**: Pre-configured Cargo tasks
- **`launch.json`**: Debug configurations

### Git Hooks (`.githooks/`)

- **`pre-commit`**: Comprehensive pre-commit checks (1-3 min)
- **`pre-commit-fast`**: Lightweight checks for rapid iteration (10-30s)
- **`README.md`**: Git hooks documentation

### Scripts (`scripts/`)

- **`setup_dev_env.sh`**: Automated development environment setup
- **`quick_test.sh`**: Quick test runner
- **`format_all.sh`**: Format all code
- **`clippy_check.sh`**: Run Clippy checks

### Documentation (`docs/`)

- **`DEVELOPER_SETUP.md`**: Comprehensive developer setup guide
- **`INTELLIJ_SETUP.md`**: IntelliJ/RustRover configuration
- **`VIM_SETUP.md`**: Vim/Neovim configuration

## Supported IDEs

### 1. Visual Studio Code (Recommended)

**Installation**:
1. Install [VSCode](https://code.visualstudio.com/)
2. Open the VM project: `code .`
3. Install recommended extensions (you'll be prompted)

**Features**:
- Full Rust Analyzer integration
- Auto-format on save
- Inline errors and warnings
- Integrated testing
- Debugging support

**Documentation**: See [docs/DEVELOPER_SETUP.md](docs/DEVELOPER_SETUP.md#visual-studio-code)

### 2. JetBrains RustRover / IntelliJ IDEA

**Installation**:
1. Install [RustRover](https://www.jetbrains.com/rust/) or [IntelliJ IDEA](https://www.jetbrains.com/idea/)
2. Install the Rust plugin (for IntelliJ IDEA)
3. Open the VM project

**Features**:
- Advanced code analysis
- Refactoring tools
- Built-in debugger
- Test runner
- Database tools (Ultimate)

**Documentation**: See [docs/INTELLIJ_SETUP.md](docs/INTELLIJ_SETUP.md)

### 3. Vim / Neovim

**Installation**:
1. Install [Neovim](https://neovim.io/) (0.9+) or [Vim](https://www.vim.org/) (9.0+)
2. Install rust-analyzer: `cargo install rust-analyzer`
3. Configure LSP (see below)

**Features**:
- Lightweight and fast
- Highly customizable
- Terminal-based
- Powerful text editing

**Documentation**: See [docs/VIM_SETUP.md](docs/VIM_SETUP.md)

### 4. Other Editors

The project can be used with any editor supporting:
- **Language Server Protocol (LSP)**
- **rust-analyzer** as the LSP server

Examples: Emacs, Sublime Text, Atom, Kakoune

## Pre-commit Hooks

### Standard Hook (Default)

Comprehensive checks for quality assurance:
- Code formatting
- Clippy linting
- Compilation check
- Unit tests
- Documentation tests
- Large file detection
- Sensitive information scanning

**Runtime**: 1-3 minutes

### Fast Hook

Lightweight checks for active development:
- Format on changed files
- Clippy on affected packages
- Quick compilation

**Runtime**: 10-30 seconds

### Switching Hooks

```bash
# Use fast hook
ln -sf ../../.githooks/pre-commit-fast .git/hooks/pre-commit

# Use standard hook
ln -sf ../../.githooks/pre-commit .git/hooks/pre-commit
```

### Skipping Hooks

```bash
# Skip for one commit
git commit --no-verify -m "WIP: work in progress"
```

## Development Workflow

### 1. Initial Setup

```bash
# Clone repository
git clone <repository-url>
cd vm

# Run setup script
./scripts/setup_dev_env.sh

# Verify setup
cargo build --workspace
cargo test --workspace
```

### 2. Daily Development

```bash
# Create a branch
git checkout -b feature/my-feature

# Make changes
# ... edit code ...

# Check formatting
cargo fmt --all

# Run linter
cargo clippy --workspace --all-targets -- -D warnings

# Run tests
cargo test --workspace

# Commit (pre-commit hooks run automatically)
git add .
git commit -m "Add my feature"

# Push
git push origin feature/my-feature
```

### 3. Quick Iteration

```bash
# Switch to fast hook for rapid development
ln -sf ../../.githooks/pre-commit-fast .git/hooks/pre-commit

# Use watch for automatic rebuilding
cargo watch -x check

# Format and lint in background
cargo watch -x 'fmt --all' -x clippy

# Run tests on save
cargo watch -x test
```

## Code Quality Tools

### Formatting

```bash
# Format all code
cargo fmt --all

# Check formatting without changes
cargo fmt --all -- --check

# Format specific crate
cargo fmt -p vm-core
```

### Linting

```bash
# Run Clippy with workspace settings
cargo clippy --workspace --all-targets -- -D warnings

# Run Clippy on specific package
cargo clippy -p vm-core --all-targets

# Fix automatically
cargo clippy --workspace --fix --allow-dirty
```

### Testing

```bash
# Run all tests
cargo test --workspace

# Run library tests only (faster)
cargo test --workspace --lib

# Run tests with output
cargo test --workspace -- --nocapture

# Run specific test
cargo test --workspace test_name

# Quick tests
./scripts/quick_test.sh
```

### Documentation

```bash
# Generate documentation
cargo doc --workspace --no-deps

# Include private items
cargo doc --workspace --no-deps --document-private-items

# Open in browser
cargo doc --workspace --no-deps --open
```

## Environment Variables

Create a `.env` file (not committed to git):

```bash
# Rust logging
export RUST_LOG=debug

# Backtrace for debugging
export RUST_BACKTRACE=1

# Terminal color
export CARGO_TERM_COLOR=always

# Profile performance
export RUST_PROFILE=time
```

## Troubleshooting

### Rust Analyzer Issues

**VSCode not showing errors**:
1. Command Palette → "Rust Analyzer: Reload workspace"
2. Check `.vscode/settings.json`
3. Restart VSCode

**IntelliJ not indexing**:
1. File → Invalidate Caches
2. File → Sync with Cargo.toml
3. Restart IDE

### Build Errors

```bash
# Clean and rebuild
cargo clean
cargo build --workspace

# Update dependencies
cargo update
```

### Pre-commit Hook Failures

```bash
# Skip temporarily
git commit --no-verify -m "message"

# Run manually to debug
.git/hooks/pre-commit
```

### Performance Issues

```bash
# Use fast hook
ln -sf ../../.githooks/pre-commit-fast .git/hooks/pre-commit

# Limit build parallelism
export CARGO_BUILD_JOBS=2

# Use incremental compilation (already enabled)
```

## Best Practices

### Code Style

- Follow Rust naming conventions
- Use `rustfmt` before committing
- Fix all `clippy` warnings
- Document public APIs
- Write tests for new code

### Git Workflow

1. Create feature branches
2. Commit frequently
3. Use meaningful commit messages
4. Pull before pushing
5. Create pull requests for review

### Testing Strategy

- Write unit tests for logic
- Write integration tests for interactions
- Use doc tests for examples
- Benchmark critical paths

### Performance

- Use `cargo check` during development
- Enable incremental compilation
- Profile bottlenecks
- Optimize based on measurements

## Additional Resources

### Project Documentation

- **README.md**: Project overview and usage
- **CONTRIBUTING.md**: Contribution guidelines
- **CHANGELOG.md**: Version history
- **docs/**: Detailed documentation

### External Resources

- [Rust Book](https://doc.rust-lang.org/book/)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/)
- [The Cargo Book](https://doc.rust-lang.org/cargo/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Rust Standard Library](https://doc.rust-lang.org/std/)

### Community

- [Rust Users Forum](https://users.rust-lang.org/)
- [Rust Discord](https://discord.gg/rust-lang)
- [Stack Overflow - Rust Tag](https://stackoverflow.com/questions/tagged/rust)

## Getting Help

If you encounter issues:

1. Check the relevant setup guide in `docs/`
2. Search existing issues in the project
3. Ask the team via your communication channel
4. Consult Rust documentation
5. Search Stack Overflow

---

## Quick Reference

### Essential Commands

```bash
# Build
cargo build --workspace          # Debug build
cargo check --workspace          # Fast check

# Test
cargo test --workspace           # All tests
./scripts/quick_test.sh          # Quick tests

# Format & Lint
cargo fmt --all                  # Format
cargo clippy --workspace         # Lint

# Docs
cargo doc --workspace --open     # View docs

# Watch
cargo watch -x check             # Auto-rebuild
cargo watch -x test              # Auto-test

# Git
git commit --no-verify           # Skip hooks
git status                       # Check status
```

### IDE Shortcuts

| Action | VSCode | IntelliJ | Vim |
|--------|--------|----------|-----|
| Go to Definition | F12 | Ctrl+B | gd |
| Find References | Shift+F12 | Alt+F7 | gr |
| Format Document | Shift+Alt+F | Ctrl+Alt+L | = |
| Run Tests | None | Ctrl+Shift+F10 | :wa<CR>:!cargo test |
| Toggle Terminal | Ctrl+\` | Alt+F12 | :terminal |

---

**Happy Hacking! 🚀**

For detailed setup instructions, see:
- [docs/DEVELOPER_SETUP.md](docs/DEVELOPER_SETUP.md)
- [docs/INTELLIJ_SETUP.md](docs/INTELLIJ_SETUP.md)
- [docs/VIM_SETUP.md](docs/VIM_SETUP.md)
