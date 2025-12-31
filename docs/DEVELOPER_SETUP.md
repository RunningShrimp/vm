# VM Project Developer Setup Guide

This guide will help you set up your development environment for the VM project.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Quick Start](#quick-start)
- [IDE Configuration](#ide-configuration)
- [Pre-commit Hooks](#pre-commit-hooks)
- [Common Development Commands](#common-development-commands)
- [Troubleshooting](#troubleshooting)
- [Additional Resources](#additional-resources)

---

## Prerequisites

### Required Tools

- **Rust**: 1.85 or later
  - Install from [rustup.rs](https://rustup.rs/)
  - Or run: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`

- **Git**: Latest version
  - macOS: `brew install git`
  - Linux: `sudo apt-get install git` (Ubuntu/Debian)
  - Windows: Download from [git-scm.com](https://git-scm.com/)

### Optional but Recommended Tools

- **cargo-watch**: Watch for changes and recompile automatically
- **cargo-edit**: Manage dependencies from the command line
- **cargo-audit**: Check for security vulnerabilities
- **cargo-nextest**: Faster test runner
- **cargo-tarpaulin**: Code coverage tool

---

## Quick Start

### 1. Clone the Repository

```bash
git clone <repository-url>
cd vm
```

### 2. Run the Setup Script

```bash
./scripts/setup_dev_env.sh
```

This script will:
- Install Git hooks
- Check your Rust toolchain
- Install development tools
- Verify project configuration
- Create helper scripts

### 3. Verify Your Setup

```bash
# Build the project
cargo build --workspace

# Run tests
cargo test --workspace

# Format code
cargo fmt --all

# Run clippy
cargo clippy --workspace --all-targets -- -D warnings
```

---

## IDE Configuration

### Visual Studio Code

#### 1. Install Recommended Extensions

Open the project in VSCode. You'll be prompted to install recommended extensions, or manually install:

```bash
code --install-extension rust-lang.rust-analyzer
code --install-extension tamasfe.even-better-toml
code --install-extension serayuzgur.crates
code --install-extension vadimcn.vscode-lldb
code --install-extension eamodio.gitlens
```

#### 2. Configure Settings

The project includes VSCode configuration in `.vscode/`:

- **settings.json**: Rust analyzer settings, formatting, editor preferences
- **extensions.json**: Recommended extensions
- **tasks.json**: Pre-configured build tasks
- **launch.json**: Debug configurations

#### 3. Key Features

- **Auto-format on save**: Code is automatically formatted when you save
- **Inline errors**: Errors are shown directly in the editor
- **IntelliCode**: Enhanced code completion
- **Test Explorer**: Visual test runner
- **Cargo tasks**: Quick access to common Cargo commands (Ctrl+Shift+B / Cmd+Shift+B)

#### 4. Useful Keybindings

| Command | macOS | Linux/Windows |
|---------|-------|---------------|
| Run Task | Cmd+Shift+B | Ctrl+Shift+B |
| Format Document | Cmd+Shift+F | Ctrl+Shift+F |
| Go to Definition | F12 | F12 |
| Peek Definition | Opt+Shift+F12 | Alt+Shift+F12 |
| Toggle Terminal | Ctrl+\` | Ctrl+\` |

### IntelliJ IDEA / RustRover

#### 1. Open the Project

- File → Open → Select the VM project directory

#### 2. Configure Rust Settings

- **Settings → Languages & Frameworks → Rust**
  - Rust toolchain location: Auto-detected
  - External linter: Clippy
  - Format tool: Rustfmt

#### 3. Enable Actions on Save

- **Settings → Tools → Actions on Save**
  - ✅ Reformat code
  - ✅ Run cargo check
  - ✅ Organize imports

#### 4. Configure Build Configuration

- Run → Edit Configurations
- Add new "Cargo" configuration:
  - Command: `check`
  - Options: `--workspace`

### Vim/Neovim

#### Using coc.nvim

```vim
" Install coc.nvim
:Plug 'neoclide/coc.nvim'

" Install Rust extension
:CocInstall coc-rust-analyzer
```

#### Configuration (.vimrc / init.vim)

```vim
" Rust settings
let g:coc_global_extensions = ['coc-rust-analyzer']

" Format on save
autocmd BufWritePre *.rs :call CocAction('command', 'rust-analyzer.runCodeAction')

" Signcolumn
set signcolumn=yes

" Complete options
set completeopt=menuone,noinsert,noselect
```

#### Using nvim-lsp

```lua
-- init.lua
local lspconfig = require('lspconfig')

lspconfig.rust_analyzer.setup({
  settings = {
    ['rust-analyzer'] = {
      checkOnSave = {
        command = "clippy",
        extraArgs = { "--all-targets", "-D", "warnings" }
      },
      cargo = {
        features = "all"
      }
    }
  }
})
```

---

## Pre-commit Hooks

### What They Do

Pre-commit hooks run automatically before each commit and check:

1. **Code Formatting**: Ensures code is properly formatted with `cargo fmt`
2. **Clippy Checks**: Runs linter to catch common mistakes
3. **Compilation**: Verifies code compiles with `cargo check`
4. **Unit Tests**: Runs library tests with `cargo test --lib`
5. **Documentation Tests**: Checks doc examples with `cargo test --doc`
6. **Large Files**: Warns about files > 10MB
7. **Sensitive Information**: Warns about potential secrets/passwords
8. **TODO/FIXME**: Lists TODO and FIXME comments

### Skipping Hooks

If you need to bypass hooks temporarily:

```bash
git commit --no-verify -m "WIP: work in progress"
```

### Running Hooks Manually

```bash
# Run pre-commit checks without committing
.git/hooks/pre-commit
```

### Customizing Hooks

Edit `.githooks/pre-commit` to customize the checks.

---

## Common Development Commands

### Building

```bash
# Debug build
cargo build --workspace

# Release build
cargo build --workspace --release

# Check without building (faster)
cargo check --workspace

# Check with all features
cargo check --workspace --all-features
```

### Testing

```bash
# Run all tests
cargo test --workspace

# Run library tests only (faster)
cargo test --workspace --lib

# Run specific test
cargo test --workspace test_name

# Run tests with output
cargo test --workspace -- --nocapture

# Run tests in parallel
cargo nextest run --workspace

# Run quick tests (helper script)
./scripts/quick_test.sh
```

### Formatting

```bash
# Format all code
cargo fmt --all

# Check formatting without making changes
cargo fmt --all -- --check

# Format all code (helper script)
./scripts/format_all.sh
```

### Linting

```bash
# Run clippy with default settings
cargo clippy --workspace

# Run clippy with strict warnings
cargo clippy --workspace --all-targets -- -D warnings

# Run clippy check (helper script)
./scripts/clippy_check.sh
```

### Documentation

```bash
# Generate documentation
cargo doc --workspace --no-deps

# Include private items
cargo doc --workspace --no-deps --document-private-items

# Open documentation in browser
cargo doc --workspace --no-deps --open
```

### Dependency Management

```bash
# Update dependencies
cargo update

# Check for outdated dependencies
cargo outdated

# Add a dependency
cargo add crate_name

# Search for a crate
cargo search crate_name

# View dependency tree
cargo tree
```

### Security & Compliance

```bash
# Check for security vulnerabilities
cargo audit

# Check license compliance
cargo deny check

# Check bans (multiple versions, etc.)
cargo deny check bans
```

### Watching for Changes

```bash
# Watch and rebuild on changes
cargo watch -x check

# Watch and run tests
cargo watch -x test

# Watch and run clippy
cargo watch -x clippy

# Watch with multiple commands
cargo watch -x check -x test -x clippy
```

### Benchmarking

```bash
# Run benchmarks
cargo bench --workspace

# Run specific benchmark
cargo bench --workspace bench_name

# Run with additional output
cargo bench --workspace -- --save-baseline main
```

### Code Coverage

```bash
# Generate coverage report
cargo tarpaulin --workspace --out Html

# Generate coverage for specific crate
cargo tarpaulin -p vm-core --out Html
```

---

## Troubleshooting

### Rust Toolchain Issues

**Problem**: "Rust not found" or version too old

**Solution**:
```bash
# Update Rust
rustup update

# Install specific version
rustup install 1.85
rustup default 1.85

# Verify installation
rustc --version
```

### Build Errors

**Problem**: Compilation fails with cryptic errors

**Solutions**:
```bash
# Clean build artifacts
cargo clean

# Update dependencies
cargo update

# Check for issues
cargo check --workspace --all-features 2>&1 | tee build.log
```

### Rust Analyzer Issues

**Problem**: Rust Analyzer shows errors or doesn't work

**Solutions**:
1. **VSCode**: Restart Rust Analyzer
   - Command Palette → "Rust Analyzer: Reload workspace"

2. **Clear cache**:
   ```bash
   rm -rf ~/.local/state/rust-analyzer
   rm -rf target/
   ```

3. **Check settings**:
   - Ensure `rust-analyzer.linkedProjects` points to `./Cargo.toml`

### Pre-commit Hook Issues

**Problem**: Pre-commit hook fails but you want to commit anyway

**Solution**:
```bash
# Skip hooks temporarily
git commit --no-verify -m "message"
```

**Problem**: Hook permission denied

**Solution**:
```bash
chmod +x .githooks/pre-commit
```

### Memory Issues

**Problem**: Build runs out of memory

**Solutions**:
```bash
# Limit parallel jobs
export CARGO_BUILD_JOBS=2
cargo build --workspace

# Use debug build for development
cargo build --workspace
```

### Slow Builds

**Solutions**:
```bash
# Use incremental compilation (already enabled in dev profile)
# Use rustcache
export RUSTC_WRAPPER=sccache

# Build only what you need
cargo build -p vm-core
```

### IDE-specific Issues

**VSCode**: Extensions not working
- Reload window: Cmd+Shift+P → "Developer: Reload Window"
- Check Rust Analyzer logs: View → Output → Rust Analyzer Language Server

**IntelliJ**: External linter not working
- Settings → Rust → External linter
- Ensure Clippy is installed: `rustup component add clippy`

---

## Additional Resources

### Project Documentation

- **README.md**: Project overview
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

### Tools & Utilities

- **crates.io**: Crate registry
- **docs.rs**: Crate documentation
- **lib.rs**: Crate discovery
- **cargo-debstats**: Cargo download statistics

---

## Best Practices

### Code Style

- **Follow Rust naming conventions**: camelCase for variables, snake_case for functions
- **Use rustfmt**: Format code before committing
- **Fix clippy warnings**: Aim for zero warnings
- **Write tests**: Aim for high test coverage
- **Document code**: Use doc comments for public APIs

### Git Workflow

1. **Create a branch**: `git checkout -b feature/your-feature`
2. **Make changes**: Code, test, format
3. **Pre-commit checks**: Run automatically
4. **Commit**: `git commit -m "Description"`
5. **Push**: `git push origin feature/your-feature`
6. **Create PR**: Review and merge

### Testing Strategy

- **Unit tests**: Test individual functions and modules
- **Integration tests**: Test interaction between components
- **Doc tests**: Test examples in documentation
- **Benchmarks**: Track performance over time

### Performance Tips

- **Use `cargo check` during development**: Faster than full build
- **Enable incremental compilation**: Already enabled in dev profile
- **Use `cargo watch`**: Automatic rebuilding on file changes
- **Profile with flamegraph**: Identify bottlenecks

---

## Getting Help

If you encounter issues not covered in this guide:

1. **Check existing issues**: Look for similar problems in the project's issue tracker
2. **Ask for help**: Reach out to the team via your communication channel
3. **Consult Rust documentation**: [doc.rust-lang.org](https://doc.rust-lang.org/)
4. **Search Stack Overflow**: Many common issues have solutions there

---

## Quick Reference Card

```
# Build
cargo build --workspace          # Debug build
cargo check --workspace          # Fast check
cargo clean                      # Clean artifacts

# Test
cargo test --workspace           # All tests
cargo test --lib                 # Library tests only
./scripts/quick_test.sh          # Quick tests

# Format & Lint
cargo fmt --all                  # Format
cargo clippy --workspace         # Lint

# Docs
cargo doc --workspace --open     # Generate & open docs

# Watch
cargo watch -x check             # Auto-rebuild
cargo watch -x test              # Auto-test

# Dependencies
cargo update                     # Update deps
cargo add crate_name             # Add crate
cargo tree                       # Dependency tree

# Git
git commit --no-verify           # Skip hooks
.git/hooks/pre-commit            # Run hooks manually

# IDE
code .                           # Open in VSCode
idea .                           # Open in IntelliJ
```

---

**Happy Hacking! 🚀**
