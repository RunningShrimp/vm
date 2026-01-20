# Platform-Specific Testing Guide

This document describes how platform-specific tests are handled in `vm-accel` to ensure tests are portable across different operating systems.

## Overview

The `vm-accel` crate provides hardware virtualization acceleration support for multiple platforms:
- **Linux**: KVM (Kernel-based Virtual Machine)
- **macOS**: Hypervisor.framework (HVF)
- **Windows**: Windows Hypervisor Platform (WHPX)
- **iOS/tvOS**: Virtualization.framework

## Test Portability Strategy

### 1. Compile-Time Platform Detection

Platform-specific tests are conditionally compiled using `#[cfg()]` attributes:

```rust
#[cfg(target_os = "macos")]
mod hvf_tests {
    // HVF-specific tests only compiled on macOS
}

#[cfg(target_os = "linux")]
mod kvm_tests {
    // KVM-specific tests only compiled on Linux
}
```

### 2. Graceful Runtime Failure Handling

Tests that depend on hardware availability handle failures gracefully instead of asserting success:

**Before (non-portable):**
```rust
#[test]
fn test_hvf_init() {
    let mut hvf = AccelHvf::new();
    assert!(hvf.init().is_ok()); // ❌ Will fail without entitlements
}
```

**After (portable):**
```rust
#[test]
fn test_hvf_init() {
    let mut hvf = AccelHvf::new();
    match hvf.init() {
        Ok(()) => println!("HVF initialized successfully"),
        Err(e) => {
            println!("HVF initialization failed (expected in some environments): {:?}", e);
            // Acceptable - HVF requires specific entitlements
        }
    }
}
```

### 3. Platform Detection Utilities

The `platform_detect` module provides runtime platform detection:

```rust
use vm_accel::{Platform, is_kvm_available, is_hvf_available, is_whpx_available};

let platform = Platform::current();
println!("Current platform: {}", platform.name());

if platform.supports_kvm() && is_kvm_available() {
    // Run KVM-specific tests
}
```

## Platform Requirements

### Linux (KVM)

**Requirements:**
- `/dev/kvm` device node exists
- Current user has read/write permissions to `/dev/kvm`
- KVM kernel module is loaded (`kvm_intel` or `kvm_amd`)

**Typical setup:**
```bash
# Add user to kvm group
sudo usermod -aG kvm $USER

# Verify permissions
ls -l /dev/kvm
```

**Test behavior:**
- Tests check for `/dev/kvm` before attempting initialization
- Tests handle permission failures gracefully
- Tests print informative messages about why initialization may fail

### macOS (Hypervisor.framework)

**Requirements:**
- macOS 10.10 or later
- `com.apple.security.hypervisor` entitlement in code signature
- Proper code signing

**Typical setup:**
```xml
<!-- In entitlements file -->
<key>com.apple.security.hypervisor</key>
<true/>
```

**Test behavior:**
- Tests handle missing entitlements gracefully
- Tests explain why initialization may fail
- No assertions that require entitlements to be present

### Windows (WHPX)

**Requirements:**
- Windows 10 version 1803 or later
- "Windows Hypervisor Platform" feature enabled
- Hyper-V compatible hardware

**Typical setup:**
```powershell
# Enable WHPX
Enable-WindowsOptionalFeature -Online -FeatureName VirtualMachinePlatform
```

**Test behavior:**
- Tests check for Hyper-V availability
- Tests handle missing Hyper-V gracefully
- Feature-gated behind `whpx` Cargo feature

## Running Tests

### Run All Available Tests

```bash
# Runs all tests compatible with current platform
cargo test -p vm-accel
```

### Run Platform-Specific Tests

```bash
# Run only HVF tests (macOS only)
cargo test -p vm-accel --test hvf_backend_tests

# Run only KVM tests (Linux only)
cargo test -p vm-accel --test kvm_backend_tests

# Run lib tests (cross-platform)
cargo test -p vm-accel --lib
```

### Expected Test Results

**macOS:**
- ✅ All lib tests pass (69 tests)
- ✅ HVF backend tests pass (12 tests)
- ⚠️ Some HVF tests may print initialization failures (expected)

**Linux:**
- ✅ All lib tests pass (69 tests)
- ✅ KVM backend tests pass (may skip if /dev/kvm unavailable)
- ⚠️ Some KVM tests may skip if /dev/kvm doesn't exist

**Windows:**
- ✅ All lib tests pass (69 tests)
- ⚠️ WHPX tests (if present) require Hyper-V to be enabled

## CI/CD Considerations

### GitHub Actions Example

```yaml
name: Test vm-accel

on: [push, pull_request]

jobs:
  test-macos:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Run tests
        run: cargo test -p vm-accel

  test-linux:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Install KVM
        run: |
          sudo modprobe kvm
          sudo chmod 666 /dev/kvm
      - name: Run tests
        run: cargo test -p vm-accel

  test-windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Enable WHPX
        run: |
          Enable-WindowsOptionalFeature -Online -FeatureName VirtualMachinePlatform
      - name: Run tests
        run: cargo test -p vm-accel
```

## Adding New Platform-Specific Tests

When adding new platform-specific tests, follow these guidelines:

1. **Use conditional compilation** for platform-specific code
2. **Check hardware availability at runtime** before asserting
3. **Handle failures gracefully** with informative messages
4. **Document requirements** in test comments
5. **Use platform detection utilities** from `platform_detect` module

### Example Template

```rust
#[cfg(target_os = "your_platform")]
mod platform_tests {
    use vm_accel::PlatformSpecificAccel;

    /// Test initialization
    ///
    /// Note: This test requires:
    /// - Specific hardware feature
    /// - Proper permissions/entitlements
    #[test]
    fn test_init() {
        let mut accel = PlatformSpecificAccel::new();

        match accel.init() {
            Ok(()) => {
                println!("Accelerator initialized successfully");
                // Run additional tests
            }
            Err(e) => {
                println!("Initialization failed (expected without proper setup): {:?}", e);
                // Don't fail the test
            }
        }
    }
}

#[cfg(not(target_os = "your_platform"))]
mod platform_tests {
    /// Test that platform is not available
    #[test]
    fn test_platform_not_available() {
        println!("Platform-specific accelerator only available on your_platform");
    }
}
```

## Troubleshooting

### macOS: HVF Tests Fail

**Symptom:** Tests panic with "assertion failed: accel.init().is_ok()"

**Solution:**
- Ensure your binary is signed with hypervisor entitlement
- Add `com.apple.security.hypervisor` to your entitlements file
- Sign the binary: `codesign --entitlements entitlements.plist --force --sign - target/debug/deps/vm_accel-*`

### Linux: KVM Tests Fail

**Symptom:** Tests fail with "Permission denied" or "/dev/kvm not found"

**Solution:**
```bash
# Check if /dev/kvm exists
ls -l /dev/kvm

# Add user to kvm group
sudo usermod -aG kvm $USER

# Reload group (new login session required)
```

### Windows: WHPX Tests Fail

**Symptom:** Tests fail with "WHPX not available"

**Solution:**
```powershell
# Check Hyper-V status
Get-ComputerInfo -Property HyperVisorPresent

# Enable WHPX
Enable-WindowsOptionalFeature -Online -FeatureName VirtualMachinePlatform
# Restart required
```

## References

- [Hypervisor Framework Documentation](https://developer.apple.com/documentation/hypervisor)
- [KVM API Documentation](https://www.kernel.org/doc/html/latest/virt/kvm/api.html)
- [Windows Hypervisor Platform API](https://docs.microsoft.com/en-us/virtualization/api/)
- [Rust cfg() documentation](https://doc.rust-lang.org/reference/conditional-compilation.html)
