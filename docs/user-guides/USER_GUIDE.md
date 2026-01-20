# VM CLI and GUI - Complete User Guide

## Table of Contents

1. [Quick Start](#quick-start)
2. [CLI Installation](#cli-installation)
3. [CLI Commands](#cli-commands)
4. [GUI Application](#gui-application)
5. [OS Installation Guide](#os-installation-guide)
6. [Advanced Configuration](#advanced-configuration)
7. [Troubleshooting](#troubleshooting)

## Quick Start

### Install and Run Ubuntu

```bash
# Download Ubuntu ISO
vm-cli download-iso --distro ubuntu --output ubuntu.iso

# Install Ubuntu
vm-cli install-ubuntu --iso ubuntu.iso

# Or use the GUI
cd vm-desktop && cargo tauri dev
```

## CLI Installation

### From Source

```bash
# Clone repository
git clone <repository-url>
cd vm

# Build release
cargo build --release --package vm-cli

# Install binary
sudo cp target/release/vm-cli /usr/local/bin/

# Enable shell completion
vm-cli completions bash > ~/.vm-cli-completion.bash
echo "source ~/.vm-cli-completion.bash" >> ~/.bashrc
```

### From Pre-built Binary

```bash
# Download latest release
wget https://releases.example/vm-cli/latest/vm-cli

# Install
sudo install vm-cli /usr/local/bin/

# Verify installation
vm-cli --version
```

## CLI Commands

### Installation Commands

#### Ubuntu

```bash
vm-cli install-ubuntu \
  --iso ~/Downloads/ubuntu-25.10-desktop-amd64.iso \
  --disk-size-gb 30 \
  --memory-mb 4096 \
  --vcpus 1
```

#### Debian

```bash
vm-cli install-debian \
  --iso ~/Downloads/debian-13.0.0-amd64-netinst.iso \
  --disk-size-gb 20 \
  --memory-mb 3072
```

#### Arch Linux

```bash
vm-cli install-arch \
  --iso ~/Downloads/archlinux-2025.01.01-x86_64.iso \
  --disk-size-gb 20 \
  --memory-mb 2048
```

#### Manjaro

```bash
vm-cli install-manjaro \
  --iso ~/Downloads/manjaro-kde-24.0.0.iso \
  --disk-size-gb 30 \
  --memory-mb 4096
```

#### Fedora

```bash
vm-cli install-fedora \
  --iso ~/Downloads/Fedora-Workstation-Live-x86_64-41.iso \
  --disk-size-gb 30 \
  --memory-mb 4096
```

#### CentOS/RHEL

```bash
vm-cli install-centos \
  --iso ~/Downloads/CentOS-Stream-9-latest-x86_64.iso \
  --disk-size-gb 25 \
  --memory-mb 4096
```

#### Windows

```bash
vm-cli install-windows \
  --iso ~/Downloads/Windows11.iso \
  --disk-size-gb 50 \
  --memory-mb 8192 \
  --vcpus 2
```

#### Generic Linux

```bash
vm-cli install-linux \
  --iso ~/Downloads/linux-mint.iso \
  --distro "Linux Mint" \
  --disk-size-gb 30 \
  --memory-mb 4096
```

### Download ISOs

```bash
# List available distributions
vm-cli download-iso --list

# Download Ubuntu
vm-cli download-iso --distro ubuntu --output ubuntu.iso

# Download Arch Linux
vm-cli download-iso --distro arch --output arch.iso

# Download Debian
vm-cli download-iso --distro debian --output debian.iso
```

### Run VM

```bash
# Basic run
vm-cli run --kernel vmlinuz

# With custom memory and CPUs
vm-cli run \
  --kernel vmlinuz \
  --memory 4G \
  --vcpus 2

# With JIT mode
vm-cli run --kernel vmlinuz --mode jit

# With hardware acceleration
vm-cli run --kernel vmlinuz --accel

# Verbose output
vm-cli run --kernel vmlinuz --verbose

# With timing
vm-cli run --kernel vmlinuz --timing
```

### System Information

```bash
# Show system info
vm-cli info

# List architectures
vm-cli list-arch

# Detect hardware
vm-cli detect-hw
```

### Configuration

```bash
# Generate config file
vm-cli config --generate > ~/.vm-cli.toml

# Show config location
vm-cli config --show-path

# View current config
vm-cli config
```

### Shell Completion

```bash
# Generate completions
vm-cli completions bash > ~/.vm-cli-completion.bash
vm-cli completions zsh > ~/.zsh-completion.zsh
vm-cli completions fish > ~/.config/fish/completions/vm-cli.fish

# Enable in bash
echo "source ~/.vm-cli-completion.bash" >> ~/.bashrc
```

## GUI Application

### Starting the GUI

```bash
# Development mode
cd vm-desktop
cargo tauri dev

# Production build
cargo tauri build
```

### GUI Features

1. **VM Management**
   - Create new VM configurations
   - Start, stop, pause, resume VMs
   - Delete VM configurations
   - Edit VM settings

2. **OS Installation**
   - Select from 10+ distributions
   - Automatic configuration
   - Progress tracking
   - Console output

3. **Monitoring**
   - Real-time CPU usage
   - Memory utilization
   - I/O statistics
   - System metrics

4. **Snapshots**
   - Create VM snapshots
   - Restore from snapshots
   - List snapshots

### GUI Installation Workflow

1. Select "Install OS" from menu
2. Choose distribution (Ubuntu, Debian, Arch, etc.)
3. Configure settings (disk size, memory, CPUs)
4. Select or download ISO
5. Start installation
6. Monitor progress in real-time
7. Boot installed system

## OS Installation Guide

### Ubuntu Installation

1. **Download ISO**
   ```bash
   vm-cli download-iso --distro ubuntu --output ubuntu.iso
   ```

2. **Start Installation**
   ```bash
   vm-cli install-ubuntu --iso ubuntu.iso
   ```

3. **Follow Installer**
   - Select language
   - Choose installation type
   - Configure disk partitioning
   - Create user account
   - Wait for installation to complete

4. **Boot Installed System**
   ```bash
   vm-cli run --disk ubuntu_disk.img
   ```

### Arch Linux Installation

1. **Download ISO**
   ```bash
   vm-cli download-iso --distro arch --output arch.iso
   ```

2. **Start Installation**
   ```bash
   vm-cli install-arch --iso arch.iso
   ```

3. **Follow Arch Wiki**
   - Partition disk
   - Format filesystems
   - Install base system
   - Configure bootloader
   - Setup network

4. **Alternative: Use archinstall**
   ```bash
   # In the VM console
   archinstall
   ```

### Windows Installation

1. **Download ISO**
   ```bash
   vm-cli download-iso --distro windows --output windows.iso
   ```

2. **Start Installation**
   ```bash
   vm-cli install-windows \
     --iso windows.iso \
     --disk-size-gb 50 \
     --memory-mb 8192
   ```

3. **Follow Windows Setup**
   - Select language
   - Enter product key (optional)
   - Choose edition
   - Accept license terms
   - Custom installation
   - Partition disk
   - Wait for installation
   - Create user account

## Advanced Configuration

### JIT Compilation

```bash
# Enable JIT with custom thresholds
vm-cli run \
  --kernel vmlinuz \
  --mode jit \
  --jit-min-threshold 1000 \
  --jit-max-threshold 10000 \
  --jit-sample-window 1000
```

### Hardware Acceleration

```bash
# KVM (Linux)
vm-cli run --kernel vmlinuz --accel

# HVF (macOS)
vm-cli run --kernel vmlinuz --accel

# WHPX (Windows)
vm-cli run --kernel vmlinuz --accel
```

### GPU Backend

```bash
# WGPU backend
vm-cli run --kernel vmlinuz --gpu-backend wgpu

# Passthrough mode
vm-cli run --kernel vmlinuz --gpu-backend passthrough
```

### Configuration File

Create `~/.vm-cli.toml`:

```toml
[default]
# Default architecture (riscv64, x8664, arm64)
arch = "x8664"

# Default memory size
memory = "4G"

# Default number of vCPUs
vcpus = 2

# Default execution mode
mode = "jit"

# Enable hardware acceleration
accel = false

# JIT configuration
jit_min_threshold = 1000
jit_max_threshold = 10000
jit_sample_window = 1000
jit_compile_weight = 0.5
jit_benefit_weight = 0.5
jit_share_pool = true
```

## Troubleshooting

### Common Issues

#### ISO Not Found

```
Error: ISO file not found: /path/to/iso
```

**Solution**: Verify the ISO path is correct
```bash
ls -l ~/Downloads/
vm-cli install-ubuntu --iso ~/Downloads/ubuntu.iso
```

#### Disk Creation Failed

```
Error: Failed to create disk
```

**Solution**: Check disk space and permissions
```bash
df -h .
sudo vm-cli install-ubuntu --iso ubuntu.iso
```

#### VM Won't Boot

```
Error: Boot failed
```

**Solution**: Enable verbose output
```bash
vm-cli install-ubuntu --iso ubuntu.iso --verbose
```

#### Kernel Not Loading

```
Error: Failed to load kernel
```

**Solution**: Check kernel architecture compatibility
```bash
vm-cli --arch x8664 install-ubuntu --iso ubuntu.iso
```

### Getting Help

```bash
# General help
vm-cli --help

# Command-specific help
vm-cli install-ubuntu --help
vm-cli run --help

# Examples
vm-cli examples

# System information
vm-cli info

# Hardware detection
vm-cli detect-hw
```

### Debug Mode

```bash
# Enable debug logging
RUST_LOG=debug vm-cli install-ubuntu --iso ubuntu.iso

# Trace execution
VM_TRACING=1 vm-cli run --kernel vmlinuz
```

## Performance Tips

1. **Use JIT mode** for better performance
   ```bash
   vm-cli run --kernel vmlinuz --mode jit
   ```

2. **Enable hardware acceleration** when available
   ```bash
   vm-cli run --kernel vmlinuz --accel
   ```

3. **Allocate sufficient memory** for the guest OS
   ```bash
   vm-cli install-ubuntu --iso ubuntu.iso --memory-mb 4096
   ```

4. **Use SSD storage** for better disk I/O

5. **Adjust JIT thresholds** based on workload
   ```bash
   vm-cli run \
     --kernel vmlinuz \
     --jit-min-threshold 500 \
     --jit-max-threshold 5000
   ```

## Support

- **Documentation**: See project README
- **Issues**: Report bugs on GitHub
- **Contributing**: See CONTRIBUTING.md

---

*Last Updated: 2026-01-11*
