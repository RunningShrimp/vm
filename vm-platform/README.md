# vm-platform

Platform abstraction layer providing unified cross-platform interfaces for OS detection, hardware feature detection, memory management, and platform-specific operations across Linux, macOS, Windows, iOS, and Android.

## Overview

`vm-platform` provides the foundational platform abstraction that enables the VM project to run seamlessly across multiple operating systems and hardware architectures. It abstracts platform-specific differences while exposing high-level interfaces for OS detection, CPU features, memory operations, and hardware capabilities.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│              vm-platform (Platform Abstraction)          │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐ │
│  │   OS Detect  │  │  Arch Detect │  │ Feature Detect│ │
│  │              │  │              │  │              │ │
│  │ • Linux      │  │ • x86_64     │  │ • CPUID      │ │
│  │ • macOS      │  │ • ARM64     │  │ • SIMD       │ │
│  │ • Windows    │  │ • RISC-V    │  │ • Virtualization│ │
│  │ • Android    │  │              │  │ • Cache      │ │
│  │ • iOS        │  │              │  │ • PMU        │ │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘ │
│         │                  │                  │         │
│         └──────────────────┼──────────────────┘         │
│                            │                            │
│                  ┌─────────▼──────────┐                 │
│                  │  Unified Platform  │                 │
│                  │   Information      │                 │
│                  │                    │                 │
│                  │ • PlatformInfo      │                 │
│                  │ • PlatformFeatures  │                 │
│                  │ • PlatformPaths     │                 │
│                  └─────────┬──────────┘                 │
│                            │                            │
│  ┌─────────────────────────┼─────────────────────────┐ │
│  │  ┌──────────────────────▼─────────────────────┐  │ │
│  │  │         Memory Operations                    │  │ │
│  │  │  • Page size detection                       │  │ │
│  │  │  • Allocation alignment                      │  │ │
│  │  │  • Memory protection (W^X)                   │  │ │
│  │  │  • NUMA memory policies                      │  │ │
│  │  └─────────────────────────────────────────────┘  │ │
│  │                                                   │ │
│  │  ┌─────────────────────────────────────────────┐  │ │
│  │  │         Hardware Operations                  │  │ │
│  │  │  • CPU affinity                              │  │ │
│  │  │  • Thread pinning                            │  │ │
│  │  │  • Cache operations                          │  │ │
│  │  │  • Performance monitoring                     │  │ │
│  │  └─────────────────────────────────────────────┘  │ │
│  │                                                   │ │
│  │  ┌─────────────────────────────────────────────┐  │ │
│  │  │         Device & Boot Operations              │  │ │
│  │  │  • PCI device enumeration                     │  │ │
│  │  │  • Device passthrough                         │  │ │
│  │  │  • Hotplug support                            │  │ │
│  │  │  • Boot configuration                         │  │ │
│  │  └─────────────────────────────────────────────┘  │ │
│  └───────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
```

## Key Components

### 1. OS Detection (`src/platform.rs`)
Cross-platform operating system detection.

**Supported Platforms**:
- **Linux**: Full support, KVM acceleration
- **macOS**: Full support, HVF acceleration
- **Windows**: Full support, WHPX acceleration
- **Android**: Experimental support
- **iOS/tvOS**: Partial support, VZ acceleration
- **HarmonyOS**: Experimental support

**Usage**:
```rust
use vm_platform::{host_os, OS};

let os = host_os();
match os {
    OS::Linux => println!("Running on Linux"),
    OS::MacOS => println!("Running on macOS"),
    OS::Windows => println!("Running on Windows"),
    OS::Android => println!("Running on Android"),
    OS::IOS => println!("Running on iOS"),
    OS::HarmonyOS => println!("Running on HarmonyOS"),
    OS::Unknown => println!("Unknown OS"),
}
```

### 2. Architecture Detection (`src/platform.rs`)
CPU architecture and feature detection.

**Supported Architectures**:
- **x86_64**: Intel, AMD (SSE, AVX, AVX2, AVX-512)
- **ARM64**: ARMv8, ARMv9 (NEON, SVE, SVE2)
- **RISC-V 64-bit**: RV64I/M/A/F/D/C/B
- **Unknown**: Fallback for unsupported architectures

**Usage**:
```rust
use vm_platform::{host_arch, Arch};

let arch = host_arch();
match arch {
    Arch::X86_64 => println!("x86_64 architecture"),
    Arch::ARM64 => println!("ARM64 architecture"),
    Arch::RISCV64 => println!("RISC-V 64-bit architecture"),
    Arch::Unknown => println!("Unknown architecture"),
}
```

### 3. Platform Features (`src/platform.rs`)
Comprehensive hardware and OS feature detection.

**Feature Detection**:
```rust
use vm_platform::PlatformFeatures;

let features = PlatformFeatures::detect();

// Check CPU features
if features.has_simd() {
    println!("SIMD supported");
}

if features.has_virtualization() {
    println!("Hardware virtualization available");
}

// Check OS features
if features.has_huge_pages() {
    println!("Huge pages supported");
}

if features.has_numa() {
    println!("NUMA available");
}
```

**Available Features**:
- **CPU Features**: SIMD, virtualization, PMU, cache info
- **Memory Features**: Huge pages, NUMA, memory protection keys
- **OS Features**: I/OURING (Linux), Hypervisor APIs, Hotplug
- **Device Features**: GPU acceleration, passthrough support

### 4. Memory Operations (`src/memory.rs`)
Cross-platform memory management.

**Features**:
- Page size detection
- Memory protection (W^X policy)
- Aligned allocation
- NUMA memory policies

**Usage**:
```rust
use vm_platform::memory::{page_size, allocate_aligned, protect_memory};

// Get page size
let size = page_size()?;

// Allocate aligned memory
let ptr = allocate_aligned(4096, 4096)?;

// Protect memory (W^X aware)
protect_memory(ptr, size, MemoryProtection::READ_WRITE)?;
```

**Memory Protection**:
```rust
pub enum MemoryProtection {
    NONE,
    READ,
    WRITE,
    EXECUTE,
    READ_WRITE,
    READ_EXECUTE,
    READ_WRITE_EXECUTE,  // May fail on systems with W^X
}
```

### 5. Hardware Operations (`src/hotplug.rs`, `src/pci.rs`)

#### CPU Affinity (`src/runtime.rs`)
```rust
use vm_platform::runtime::{set_cpu_affinity, CpuSet};

let mut cpuset = CpuSet::new();
cpuset.set(0);

// Pin thread to CPU 0
set_cpu_affinity(cpuset)?;
```

#### PCI Device Enumeration (`src/pci.rs`)
```rust
use vm_platform::pci::{PciDevice, PciBus};

// Enumerate PCI devices
let bus = PciBus::new();
for device in bus.devices()? {
    println!("Device: {:04x}:{:04x}", device.vendor_id, device.device_id);
}
```

#### Device Hotplug (`src/hotplug.rs`)
```rust
use vm_platform::hotplug::{HotplugController, HotplugDevice};

let controller = HotplugController::new()?;

// Add device
controller.add_device(HotplugDevice::Pci)?;
```

### 6. Boot Configuration (`src/boot.rs`, `src/iso.rs`)

#### Boot Configuration
```rust
use vm_platform::boot::{BootConfig, BootMethod};

let config = BootConfig {
    method: BootMethod::Direct,
    kernel_path: "/path/to/kernel",
    initrd_path: Some("/path/to/initrd"),
    cmdline: "console=ttyS0",
};
```

#### ISO Boot (El Torito)
```rust
use vm_platform::iso::{ElToritoBoot, IsoImage};

let iso = IsoImage::open("disk.iso")?;
let boot = ElToritoBoot::new(&iso)?;

// Get boot catalog
let catalog = boot.boot_catalog()?;
```

### 7. GPU Detection (`src/gpu.rs`)
Graphics hardware detection and capabilities.

```rust
use vm_platform::gpu::{GpuInfo, GpuType};

let gpus = GpuInfo::detect_all()?;
for gpu in gpus {
    match gpu.gpu_type {
        GpuType::Integrated => println!("Integrated GPU"),
        GpuType::Discrete => println!("Discrete GPU"),
        GpuType::Virtual => println!("Virtual GPU"),
    }
}
```

### 8. Device Passthrough (`src/passthrough.rs`)
Hardware passthrough support detection.

```rust
use vm_platform::passthrough::{PassthroughCapability, check_iommu};

// Check IOMMU support
let iommu = check_iommu()?;
if iommu.available {
    println!("IOMMU available: {}", iommu.backend);
}

// Check device passthrough
let caps = PassthroughCapability::detect()?;
if caps.gpu_passthrough {
    println!("GPU passthrough supported");
}
```

## Platform Information

### PlatformInfo Structure

```rust
pub struct PlatformInfo {
    pub os: OS,
    pub arch: Arch,
    pub features: PlatformFeatures,
    pub num_cpus: usize,
    pub page_size: usize,
}
```

**Usage**:
```rust
use vm_platform::platform_info;

let info = platform_info()?;
println!("Platform: {}-{} with {} CPUs",
    info.os, info.arch, info.num_cpus);
```

### Platform Paths

```rust
use vm_platform::PlatformPaths;

let paths = PlatformPaths::new();
println!("Config: {}", paths.config_dir);
println!("Cache: {}", paths.cache_dir);
println!("Runtime: {}", paths.runtime_dir);
```

## Platform Support Matrix

| Platform | OS | Arch | Status | Notes |
|----------|----|----|--------|-------|
| **Linux** | Linux | x86_64 | ✅ Full | KVM, I/OURING, huge pages |
| **Linux** | Linux | ARM64 | ✅ Full | KVM (VHE), huge pages |
| **Linux** | Linux | RISC-V | ⚠️ Experimental | Limited hypervisor support |
| **macOS** | macOS | x86_64 | ✅ Good | HVF, deprecated by Apple |
| **macOS** | macOS | ARM64 | ✅ Full | HVF, Apple Silicon |
| **Windows** | Windows | x86_64 | ✅ Good | WHPX, Hyper-V required |
| **Android** | Android | ARM64 | ⚠️ Experimental | Limited testing |
| **iOS** | iOS | ARM64 | ⚠️ Partial | VZ, sandbox restrictions |
| **tvOS** | tvOS | ARM64 | ⚠️ Partial | VZ, sandbox restrictions |

## Usage Examples

### Cross-Platform Code

```rust
use vm_platform::{host_os, host_arch, PlatformFeatures};

fn main() {
    let os = host_os();
    let arch = host_arch();
    let features = PlatformFeatures::detect();

    println!("VM Platform");
    println!("  OS: {:?}", os);
    println!("  Arch: {:?}", arch);
    println!("  Features:");
    println!("    SIMD: {}", features.has_simd());
    println!("    Virtualization: {}", features.has_virtualization());
    println!("    NUMA: {}", features.has_numa());
}
```

### Platform-Specific Optimization

```rust
use vm_platform::{host_os, PlatformFeatures};

fn optimize_for_platform() {
    let os = host_os();
    let features = PlatformFeatures::detect();

    match os {
        OS::Linux => {
            // Use Linux-specific optimizations
            if features.has_io_uring() {
                println!("Using io_uring");
            }
        }
        OS::MacOS => {
            // Use macOS-specific optimizations
            println!("Using optimized mach syscalls");
        }
        OS::Windows => {
            // Use Windows-specific optimizations
            println!("Using Windows APIs");
        }
        _ => {}
    }
}
```

### Memory Management

```rust
use vm_platform::memory::{MappedMemory, MemoryProtection};

// Allocate executable memory (W^X aware)
let result = MappedMemory::allocate(4096, MemoryProtection::READ_EXECUTE);

match result {
    Ok(mem) => {
        println!("Allocated executable memory");
    }
    Err(_) => {
        // Fall back to RW + RX on W^X systems
        println!("W^X enforced, using separate mappings");
    }
}
```

## Performance Considerations

### Feature Detection Caching
- Results cached after first detection
- Minimal overhead for repeated queries
- Thread-safe access

### Memory Operations
- Page-aligned allocations for optimal performance
- NUMA-local allocations when available
- Huge pages for large memory regions

### CPU Operations
- Cache-line size awareness
- CPU topology detection
- Affinity for performance-critical threads

## Configuration

### Compile-Time Features

No explicit features needed - platform detection is automatic.

### Runtime Configuration

```rust
use vm_platform::PlatformConfig;

let config = PlatformConfig {
    enable_numa: true,
    enable_huge_pages: true,
    enable_simd: true,
    cpu_affinity_policy: CpuAffinityPolicy::Local,
};
```

## Best Practices

1. **Always check platform**: Use feature detection before using platform-specific features
2. **Handle W^X**: Gracefully handle systems with W^X policy
3. **Use abstractions**: Prefer platform APIs over direct syscalls
4. **Test everywhere**: Test on all supported platforms
5. **Document quirks**: Note platform-specific behaviors

## Platform-Specific Notes

### Linux
- Best hypervisor support (KVM)
- I/OURING for fast I/O
- Huge pages: 2MB and 1GB
- Comprehensive NUMA support

### macOS
- HVF on both x86_64 and ARM64
- x86_64 deprecated by Apple
- Mach-O binaries required
- Limited NUMA (single socket)

### Windows
- WHPX requires Hyper-V
- Admin privileges required
- SLAT support varies
- Limited NUMA support

### ARM64 Platforms
- NEON always available
- SVE/SVE2 optional
- VHE for KVM
- Apple Silicon specific features

## Testing

```bash
# Run all tests
cargo test -p vm-platform

# Run platform detection tests
cargo test -p vm-platform --lib platform

# Run memory tests
cargo test -p vm-platform --lib memory
```

## Related Crates

- **vm-core**: Domain models and error handling
- **vm-accel**: Hardware acceleration (uses platform detection)
- **vm-mem**: Memory management (uses platform operations)
- **vm-engine**: Execution engine (uses platform features)

## Dependencies

### Core Dependencies
- `vm-core`: Domain models
- `num_cpus`: CPU count detection
- `serde_json`: Serialization

### Platform-Specific Dependencies
- **Unix**: `libc`
- **Windows**: `windows-sys`

## License

[Your License Here]

## Contributing

Contributions welcome! Please:
- Test on all supported platforms
- Add feature detection for new hardware
- Document platform-specific quirks
- Handle errors gracefully

## See Also

- [Platform Detection in Rust](https://doc.rust-lang.org/std/env/constant.ARCH.html)
- [CPUID Documentation](https://en.wikipedia.org/wiki/CPUID)
- [W^X Wikipedia](https://en.wikipedia.org/wiki/W%5EX)
