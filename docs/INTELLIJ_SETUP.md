# IntelliJ IDEA / RustRover Setup Guide

This guide covers setting up IntelliJ IDEA or JetBrains RustRover for the VM project.

## Table of Contents

- [Installation](#installation)
- [Project Setup](#project-setup)
- [Configuration](#configuration)
- [Run Configurations](#run-configurations)
- [Key Features](#key-features)
- [Tips and Tricks](#tips-and-tricks)

---

## Installation

### RustRover (Recommended)

RustRover is JetBrains' dedicated Rust IDE.

1. Download from [jetbrains.com/rust](https://www.jetbrains.com/rust/)
2. Install using the installer
3. Launch and activate your license

### IntelliJ IDEA with Rust Plugin

1. Install IntelliJ IDEA (Community or Ultimate)
2. Install the Rust plugin:
   - File → Settings → Plugins
   - Search for "Rust"
   - Install "Rust" by JetBrains

---

## Project Setup

### Opening the Project

1. **File → Open**
2. Select the VM project directory
3. Click "Open"
4. Wait for indexing to complete (may take a few minutes)

### Initial Configuration

When opening the project for the first time:

1. **Rust Toolchain Detection**
   - RustRover should auto-detect your Rust installation
   - If not: File → Settings → Rust → Detection
   - Click "Detect" and select your Rust toolchain

2. **External Linter Setup**
   - Settings → Languages & Frameworks → Rust → External Linter
   - Select "Clippy"
   - Path: `clippy` (auto-detected)
   - Arguments: `--all-targets -D warnings`

3. **Format Tool Setup**
   - Settings → Languages & Frameworks → Rust → Format Tool
   - Select "Rustfmt"
   - Path: `rustfmt` (auto-detected)

---

## Configuration

### Editor Settings

**Settings → Editor → Code Style → Rust**

```yaml
# Recommended settings
Indent: 4 spaces
Continuation indent: 8 spaces
Keep max blank lines: 2
Line width: 100
```

### Actions on Save

**Settings → Tools → Actions on Save**

Enable the following:
- ✅ Reformat code
- ✅ Optimize imports
- ✅ Run cargo check
- ✅ Inspect and fix: Rust

### Inspections

**Settings → Editor → Inspections → Rust**

Enable recommended inspections:
- ✅ Clippy lints (warning)
- ✅ Unused declarations (warning)
- ✅ Dead code (warning)
- ✅ Missing documentation (warning for public items)

### Build Configuration

**Settings → Languages & Frameworks → Rust**

- **Cargo project directory**: `$PROJECT_DIR$`
- **Cargo command**: `cargo`
- **Build, test and run cargo commands in**: `.`
- **All features**: ✅ Checked

---

## Run Configurations

### Build Configuration

1. **Run → Edit Configurations**
2. Click **+** → **Cargo**
3. Configure:
   - **Name**: `cargo build`
   - **Command**: `build`
   - **Options**: `--workspace`
   - **Working directory**: `$PROJECT_DIR$`

### Test Configuration

1. **Run → Edit Configurations**
2. Click **+** → **Cargo**
3. Configure:
   - **Name**: `cargo test`
   - **Command**: `test`
   - **Options**: `--workspace`
   - **Working directory**: `$PROJECT_DIR$`
   - **Environment**: `RUST_BACKTRACE=1`

### Clippy Configuration

1. **Run → Edit Configurations**
2. Click **+** → **Cargo**
3. Configure:
   - **Name**: `cargo clippy`
   - **Command**: `clippy`
   - **Options**: `--workspace --all-targets -- -D warnings`
   - **Working directory**: `$PROJECT_DIR$`

### Executable Configuration (e.g., vm-cli)

1. **Run → Edit Configurations**
2. Click **+** → **Cargo**
3. Configure:
   - **Name**: `Run vm-cli`
   - **Command**: `run`
   - **Options**: `--package vm-cli`
   - **Working directory**: `$PROJECT_DIR$`
   - **Before launch**: Build `vm-cli`

### Test Specific Test

1. **Run → Edit Configurations**
2. Click **+** → **Cargo Test**
3. Configure:
   - **Name**: `Test specific test`
   - **Command**: `test`
   - **Options**: `--package vm-core -- test_name`
   - **Working directory**: `$PROJECT_DIR$`

---

## Key Features

### Code Completion

- **Basic completion**: Ctrl+Space
- **Smart completion**: Ctrl+Shift+Space
- **Member completion**: Ctrl+Space after `.`
- **Macro expansion**: Automatic

### Navigation

- **Go to definition**: Ctrl+B / Cmd+B
- **Go to implementation**: Ctrl+Alt+B / Cmd+Alt+B
- **Go to usage**: Alt+F7 / Cmd+F7
- **Find in files**: Shift+Shift (Search Everywhere)
- **Symbol navigation**: Ctrl+Shift+Alt+N / Cmd+Shift+Alt+N

### Refactoring

- **Rename**: Shift+F6
- **Extract variable**: Ctrl+Alt+V / Cmd+Alt+V
- **Extract function**: Ctrl+Alt+M / Cmd+Alt+M
- **Inline variable**: Ctrl+Alt+N / Cmd+Alt+N
- **Introduce struct field**: Refactor → Introduce → Field

### Code Generation

- **Generate code**: Alt+Insert / Cmd+N
- **Implement trait**: Right-click → Generate → Implement
- **Derive macros**: Alt+Enter on struct name

### Testing

- **Run test**: Right-click → Run (or Ctrl+Shift+F10)
- **Debug test**: Right-click → Debug (or Ctrl+Shift+F9)
- **Run all tests**: Right-click on package → Run Tests
- **Test results**: View in Test Runner tool window

### Documentation

- **Quick documentation**: Ctrl+Q / Ctrl+J
- **View documentation**: Ctrl+Shift+Q / Ctrl+Shift+J
- **Generate doc**: Type `///` or `//!` above item

---

## Tips and Tricks

### Performance Optimization

1. **Increase Memory**:
   - Help → Edit Custom VM Options
   - Set `-Xmx8g` (or more for large projects)

2. **Exclude directories from indexing**:
   - Settings → Editor → File Types → Ignored Files and Folders
   - Add: `target;Cargo.lock`

3. **Use power save mode**:
   - File → Power Save Mode
   - Disables background inspections

### Keyboard Shortcuts

| Action | Windows/Linux | macOS |
|--------|---------------|-------|
| Build Project | Ctrl+F9 | Cmd+F9 |
| Run | Shift+F10 | Ctrl+R |
| Debug | Shift+F9 | Ctrl+D |
| Stop | Ctrl+F2 | Cmd+F2 |
| Format Code | Ctrl+Alt+L | Cmd+Alt+L |
| Optimize Imports | Ctrl+Alt+O | Ctrl+Alt+O |
| Reformat | Ctrl+Alt+Shift+L | Cmd+Alt+Shift+L |
| Find Action | Ctrl+Shift+A | Cmd+Shift+A |
| Recent Files | Ctrl+E | Cmd+E |
| Navigate to Class | Ctrl+N | Cmd+O |
| Navigate to File | Ctrl+Shift+N | Cmd+Shift+O |

### Git Integration

1. **Commit**: Ctrl+K / Cmd+K
2. **Push**: Ctrl+Shift+K / Cmd+Shift+K
3. **Pull**: Ctrl+T / Cmd+T
4. **View History**: Alt+9 / Cmd+9
5. **Diff**: Right-click → Show Diff

### Terminal Integration

1. Open terminal: Alt+F12 / Cmd+F12
2. Split terminal: Right-click → Split
3. Run Cargo commands directly in terminal

### Code Quality Tools

1. **Run Clippy**:
   - Right-click on project → Run External Linter
   - Or use your run configuration

2. **Format with Rustfmt**:
   - Right-click → Reformat with Rustfmt
   - Or: Ctrl+Alt+Shift+L / Cmd+Alt+Shift+L

3. **Run Cargo Check**:
   - Tools → Cargo → Check
   - Or use Actions on Save

### Plugin Recommendations

Install these plugins for enhanced functionality:

1. **String Manipulation**: Advanced string operations
2. **Rainbow Brackets**: Color-coded brackets
3. **GitToolBox**: Enhanced Git features
4. **Key Promoter X**: Learn shortcuts
5. **Translation**: Translate documentation
6. **TODO Highlight**: Highlight TODO comments

### Customizing the IDE

#### Color Scheme

1. Settings → Editor → Color Scheme
2. Choose from presets or create custom
3. Recommended: "Darcula" or "Light"

#### Font

1. Settings → Editor → Font
2. Recommended monospace fonts:
   - JetBrains Mono
   - Fira Code
   - Source Code Pro

#### Keymap

1. Settings → Keymap
2. Choose from presets:
   - Windows (default)
   - macOS (default)
   - Customizable

---

## Troubleshooting

### Rust Toolchain Not Detected

**Problem**: RustRover can't find Rust

**Solution**:
1. Ensure Rust is installed: `rustc --version`
2. Restart RustRover
3. File → Settings → Rust → Detection → Detect
4. Manually specify path if needed

### External Linter Not Working

**Problem**: Clippy errors not showing

**Solution**:
1. Ensure Clippy is installed: `rustup component add clippy`
2. Check path: Settings → Rust → External Linter
3. Test in terminal: `cargo clippy --version`

### Slow Indexing

**Problem**: IDE is slow/frozen

**Solution**:
1. Wait for initial indexing to complete
2. Exclude `target/` from indexing
3. Increase memory: Help → Edit Custom VM Options
4. Enable power save mode during editing

### Build Errors Not Showing

**Problem**: Cargo build errors not visible

**Solution**:
1. Check "Build" tool window: Alt+4 / Cmd+4
2. Enable "Build Make Project" on save
3. Reload Cargo project: Tools → Cargo → Reload

### Tests Not Running

**Problem**: Tests fail to run

**Solution**:
1. Ensure workspace is loaded: Tools → Cargo → Reload
2. Check test configuration: Run → Edit Configurations
3. Run in terminal to see full output

### Code Completion Not Working

**Problem**: No suggestions when typing

**Solution**:
1. Wait for indexing to complete
2. Invalidate caches: File → Invalidate Caches
3. Restart RustRover
4. Check Rust plugin is enabled

---

## Advanced Configuration

### Custom Build Scripts

If your project has custom build scripts (`build.rs`):

1. Settings → Languages & Frameworks → Rust
2. Enable "Run build scripts"
3. Configure environment variables if needed

### Macro Expansion

For procedural macro debugging:

1. Settings → Languages & Frameworks → Rust → Macros
2. Enable "Expand procedural macros"
3. Right-click on macro → Expand Macro

### Remote Development

For remote development via SSH:

1. File → Settings → Build, Execution, Deployment → Remote SSH
2. Configure SSH connection
3. Use Remote Development

### Docker Integration

For development inside Docker:

1. Install Docker plugin
2. Configure Docker connection
3. Set up SDK in Docker

---

## Community and Support

- **JetBrains RustRover Documentation**: [jetbrains.com/rust](https://www.jetbrains.com/rust/)
- **Issue Tracker**: [youtrack.jetbrains.com](https://youtrack.jetbrains.com/)
- **Community Forum**: [intellij-support.jetbrains.com](https://intellij-support.jetbrains.com/)

---

**Happy coding with RustRover! 🦀**
