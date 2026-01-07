# vm-accel

Hardware acceleration layer providing unified hypervisor abstraction across KVM (Linux), HVF (macOS), WHPX (Windows), and VZ (iOS/tvOS) with CPU feature detection, NUMA optimization, and SIMD acceleration.

## Overview

`vm-accel` provides platform-specific hardware acceleration through a unified interface, enabling high-performance virtualization across multiple operating systems and hypervisors.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│              vm-accel (Hardware Acceleration)            │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐ │
│  │  Linux KVM   │  │  macOS HVF   │  │ Windows WHPX │ │
│  │              │  │              │  │              │ │
│  │ • Intel VT-x │  │ • Hypervisor │  │ • Hyper-V    │ │
│  │ • AMD-V      │  │   Framework  │  │   Platform   │ │
│  │ • ARM VHE    │  │ • ARM VE     │  │ • SLAT       │ │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘ │
│         │                  │                  │         │
│         └──────────────────┼──────────────────┘         │
│                            │                            │
│                  ┌─────────▼──────────┐                 │
│                  │ Unified Hypervisor│                 │
│                  │   Abstraction      │                 │
│                  │                    │                 │
│                  │ • vCPU creation    │                 │
│                  │ • Memory mapping   │                 │
│                  │ • Interrupt injection│                │
│                  └─────────┬──────────┘                 │
│                            │                            │
│  ┌─────────────────────────┼─────────────────────────┐ │
│  │  ┌──────────────────────▼─────────────────────┐  │ │
│  │  │        CPU Feature Detection                │  │ │
│  │  │  • Raw CPUID                               │  │ │
│  │  │  • Feature flags                           │  │ │
│  │  │  • Capabilities query                      │  │ │
│  │  └────────────────────────────────────────────┘  │ │
│  │                                                   │ │
│  │  ┌────────────────────────────────────────────┐  │ │
│  │  │         NUMA & vCPU Management              │  │ │
│  │  │  • NUMA-aware vCPU pinning                 │  │ │
│  │  │  • CPU affinity                            │  │ │
│  │  │  • vCPU hotplug                            │  │ │
│  │  └────────────────────────────────────────────┘  │ │
│  │                                                   │ │
│  │  ┌────────────────────────────────────────────┐  │ │
│  │  │         Performance Monitoring              │  │ │
│  │  │  • Real-time metrics                        │  │ │
│  │  │  • JIT execution rate                       │  │ │
│  │  │  • Performance snapshots                    │  │ │
│  │  └────────────────────────────────────────────┘  │ │
│  │                                                   │ │
│  │  ┌────────────────────────────────────────────┐  │ │
│  │  │            SIMD Acceleration                │  │ │
│  │  │  • x86 SSE/AVX/AVX-512                      │  │ │
│  │  │  • ARM NEON/SVE                            │  │ │
│  │  │  • Auto-detection                           │  │ │
│  │  └────────────────────────────────────────────┘  │ │
│  └───────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
```

## Platform Support

### Linux KVM (Kernel-based Virtual Machine)
- **Architectures**: x86_64 (Intel VT-x, AMD-V), ARM64 (VHE)
- **Features**:
  - Full virtualization support
  - Nested virtualization
  - SR-IOV support
  - vCPU hotplug
  - Memory ballooning
- **Dependencies**: `kvm-ioctls`, `kvm-bindings`

### macOS HVF (Hypervisor Framework)
- **Architectures**: x86_64, ARM64 (Apple Silicon)
- **Features**:
  - Native hypervisor framework
  - Low-overhead virtualization
  - ARM Virtualization Extensions
- **Dependencies**: `libc` (hypervisor syscalls)

### Windows WHPX (Windows Hypervisor Platform)
- **Architectures**: x86_64
- **Features**:
  - Hyper-V based virtualization
  - SLAT (Second Level Address Translation)
  - Virtual TPM
- **Dependencies**: `windows` crate

### iOS/tvOS VZ (Virtualization Framework)
- **Architectures**: ARM64 (Apple Silicon)
- **Features**:
  - Platform-specific virtualization
  - Resource limits
  - Sandbox-aware

## Key Components

### 1. Hypervisor Abstraction

**Unified Interface**:
```rust
pub trait Hypervisor: Send + Sync {
    /// Create new virtual machine
    fn create_vm(&mut self) -> Result<VmHandle, HypervisorError>;

    /// Create virtual CPU
    fn create_vcpu(&mut self, vm: VmHandle) -> Result<VcpuHandle, HypervisorError>;

    /// Map guest memory
    fn map_memory(&mut self, vm: VmHandle, guest_addr: u64, size: usize, host_addr: u64)
        -> Result<(), HypervisorError>;

    /// Run virtual CPU
    fn run_vcpu(&mut self, vcpu: VcpuHandle) -> Result<VcpuExit, HypervisorError>;
}
```

**Platform Implementations**:
- `src/linux/kvm.rs`: KVM hypervisor
- `src/macos/hvf.rs`: HVF hypervisor
- `src/windows/whpx.rs`: WHPX hypervisor
- `src/ios/vz.rs`: VZ hypervisor

### 2. CPU Feature Detection

**Feature Detection**:
```rust
use vm_accel::cpu::CpuFeatures;

let features = CpuFeatures::detect()?;

if features.has_avx512f() {
    println!("AVX-512 Foundation supported");
}

if features.has_sve() {
    println!("ARM SVE supported");
}
```

**Supported Features**:

**x86_64**:
- SIMD: SSE, SSE2, SSE4.1, SSE4.2, AVX, AVX2, AVX-512
- Virtualization: VMX, SVM
- Other: AES-NI, RDRAND, RDSEED, CLWB

**ARM64**:
- SIMD: NEON, SVE, SVE2
- Virtualization: VE, VHE
- Other: PMU, SPE

### 3. NUMA Optimization

**NUMA-Aware vCPU Pinning**:
```rust
use vm_accel::numa::{NumaPolicy, VcpuPinner};

let policy = NumaPolicy::Local;
let pinner = VcpuPinner::new(policy)?;

// Pin vCPU to local NUMA node
pinner.pin_vcpu(vcpu_id, numa_node)?;
```

**Features**:
- NUMA topology detection
- CPU topology query
- vCPU affinity management
- Memory locality optimization

### 4. vCPU Management

**vCPU Creation**:
```rust
use vm_accel::vcpu::VcpuManager;

let manager = VcpuManager::new()?;

// Create vCPU
let vcpu = manager.create_vcpu(vm_handle)?;

// Configure vCPU
vcpu.set_cpuid(cpuid)?;
vcpu.set_msr(msr_index, msr_value)?;

// Run vCPU
match vcpu.run()? {
    VcpuExit::Hlt => { /* Handle HLT */ }
    VcpuExit::IoIn(port, data) => { /* Handle I/O */ }
    VcpuExit::MmioRead(addr, data) => { /* Handle MMIO */ }
    // ... more exit reasons
}
```

### 5. Performance Monitoring

**Real-time Metrics**:
```rust
use vm_accel::monitor::PerformanceMonitor;

let monitor = PerformanceMonitor::new()?;

// Record metrics
monitor.record_execution(vcpu_id, duration_ns)?;

// Get snapshot
let snapshot = monitor.snapshot()?;
println!("JIT execution rate: {:.2}%", snapshot.jit_execution_rate);
```

**Metrics**:
- JIT execution rate
- TLB hit rate
- Memory usage
- CPU utilization
- I/O throughput

### 6. SIMD Acceleration

**SIMD Detection**:
```rust
use vm_accel::simd::SimdCapabilities;

let caps = SimdCapabilities::detect();

match caps.recommended_strategy() {
    SimdStrategy::Avx512 => { /* Use AVX-512 */ }
    SimdStrategy::Avx2 => { /* Use AVX2 */ }
    SimdStrategy::Neon => { /* Use NEON */ }
    SimdStrategy::Scalar => { /* Use scalar */ }
}
```

## Features

### Default Features
- **`acceleration`**: Hardware acceleration enabled (includes KVM, HVF, WHPX support)

### Platform-Specific Features
- **`kvm`**: Linux KVM support (auto-enabled on Linux)
- **`hvf`**: macOS HVF support (auto-enabled on macOS)
- **`whpx`**: Windows WHPX support (auto-enabled on Windows)
- **`vz`**: iOS/tvOS VZ support (auto-enabled on iOS/tvOS)

### Optional Features
- **`smmu`**: SMMU/IOMMU integration

## Usage

### Linux KVM

```rust
use vm_accel::linux::KvmHypervisor;

let hypervisor = KvmHypervisor::new()?;
let vm = hypervisor.create_vm()?;

let vcpu = hypervisor.create_vcpu(vm)?;
vcpu.run()?;
```

### macOS HVF

```rust
use vm_accel::macos::HvfHypervisor;

let hypervisor = HvfHypervisor::new()?;
let vm = hypervisor.create_vm()?;

let vcpu = hypervisor.create_vcpu(vm)?;
vcpu.run()?;
```

### Windows WHPX

```rust
use vm_accel::windows::WhpxHypervisor;

let hypervisor = WhpxHypervisor::new()?;
let vm = hypervisor.create_vm()?;

let vcpu = hypervisor.create_vcpu(vm)?;
vcpu.run()?;
```

### Cross-Platform Code

```rust
use vm_accel::create_hypervisor;

// Platform-agnostic hypervisor creation
let hypervisor = create_hypervisor()?;
let vm = hypervisor.create_vm()?;
```

## NUMA Optimization

### Topology Detection

```rust
use vm_accel::numa::NumaTopology;

let topology = NumaTopology::detect()?;

println!("NUMA nodes: {}", topology.num_nodes());
for node in topology.nodes() {
    println!("Node {}: {} CPUs", node.id(), node.cpu_count());
}
```

### vCPU Pinning

```rust
use vm_accel::numa::VcpuPinner;

let pinner = VcpuPinner::new()?;

// Pin to specific node
pinner.pin_vcpu_to_node(vcpu_id, node_id)?;

// Pin to specific CPU
pinner.pin_vcpu_to_cpu(vcpu_id, cpu_id)?;

// Get affinity
let affinity = pinner.get_affinity(vcpu_id)?;
```

## Performance Considerations

### Best Practices

1. **Use KVM on Linux**: Best performance and feature support
2. **Enable NUMA awareness**: For multi-socket systems
3. **Pin vCPUs**: Reduce context switching overhead
4. **Use SIMD**: For compute-intensive workloads
5. **Monitor metrics**: Track performance in real-time

### Performance Tips

**For High Throughput**:
- Use local NUMA allocation
- Enable vCPU pinning
- Use SIMD operations

**For Low Latency**:
- Pin vCPUs to isolated CPUs
- Disable CPU frequency scaling
- Use real-time kernel (PREEMPT_RT)

**For Power Efficiency**:
- Use CPU frequency scaling
- Consolidate vCPUs on fewer sockets
- Use power management features

## Platform Comparison

| Feature | KVM (Linux) | HVF (macOS) | WHPX (Windows) |
|---------|-------------|-------------|----------------|
| **Performance** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ |
| **Features** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐ |
| **Stability** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ |
| **Nested Virt** | ✅ | ❌ | ⚠️ Partial |
| **vCPU Hotplug** | ✅ | ❌ | ⚠️ Partial |
| **SR-IOV** | ✅ | ❌ | ✅ |

## Architecture Details

### KVM (Linux)

```rust
pub struct KvmHypervisor {
    fd: Arc<Kvm>,
    vms: HashMap<VmHandle, Arc<Vm>>,
    vcpus: HashMap<VcpuHandle, Arc<Vcpu>>,
}
```

**Flow**:
1. Open `/dev/kvm`
2. Create VM via `KVM_CREATE_VM` ioctl
3. Create vCPU via `KVM_CREATE_VCPU` ioctl
4. Map memory via `KVM_SET_USER_MEMORY_REGION` ioctl
5. Run vCPU via `KVM_RUN` ioctl

### HVF (macOS)

```rust
pub struct HvfHypervisor {
    vm_slots: HashMap<VmHandle, hvf_vm_t>,
    vcpus: HashMap<VcpuHandle, hvf_vcpu_t>,
}
```

**Flow**:
1. `hv_vm_create()` syscall
2. `hv_vcpu_create()` syscall
3. `hv_vm_map()` / `hv_vcpu_read/write()` syscalls
4. `hv_vcpu_run()` syscall

### WHPX (Windows)

```rust
pub struct WhpxHypervisor {
    partition: WHV_PARTITION_HANDLE,
    vcpus: HashMap<VcpuHandle, WHV_VCPU_HANDLE>,
}
```

**Flow**:
1. `WHvCreatePartition()` API
2. `WHvCreateVirtualProcessor()` API
3. `WHvMapGpaRange()` API
4. `WHvRunVirtualProcessor()` API

## Error Handling

All errors use `HypervisorError`:

```rust
pub enum HypervisorError {
    /// Platform-specific error
    Platform(String),
    /// Invalid configuration
    InvalidConfig(String),
    /// Resource exhausted
    ResourceExhausted(String),
    /// Not supported on this platform
    NotSupported(String),
    /// I/O error
    Io(std::io::Error),
}
```

## Testing

```bash
# Run all tests
cargo test -p vm-accel

# Run platform-specific tests
cargo test -p vm-accel --lib kvm --features kvm
cargo test -p vm-accel --lib hvf --features hvf
cargo test -p vm-accel --lib whpx --features whpx

# Run CPU feature detection
cargo test -p vm-accel --lib cpu_features
```

## Related Crates

- **vm-core**: Domain models and error handling
- **vm-device**: Device emulation
- **vm-engine**: Execution engine
- **vm-smmu**: SMMU/IOMMU support

## Platform Support Matrix

| Platform | Arch | Status | Notes |
|----------|------|--------|-------|
| Linux KVM | x86_64 | ✅ Full | Best support |
| Linux KVM | ARM64 | ✅ Full | VHE required |
| macOS HVF | x86_64 | ✅ Good | Deprecated on x86_64 |
| macOS HVF | ARM64 | ✅ Full | Apple Silicon |
| Windows WHPX | x86_64 | ✅ Good | Hyper-V required |
| iOS/tvOS VZ | ARM64 | ⚠️ Partial | Platform-specific |

## License

[Your License Here]

## Contributing

Contributions welcome! Please:
- Test on all supported platforms
- Add feature detection for new CPU features
- Benchmark performance changes
- Document platform-specific quirks

## See Also

- [KVM API Documentation](https://www.kernel.org/doc/html/latest/virt/kvm/api.html)
- [HVF Documentation](https://developer.apple.com/documentation/hypervisor)
- [WHPX API](https://docs.microsoft.com/en-us/virtualization/hypervisor-on-windows/)
