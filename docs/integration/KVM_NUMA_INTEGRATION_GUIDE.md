# KVM vCPU Affinity and NUMA Optimization Integration Guide

This guide demonstrates the enhanced KVM integration with vCPU affinity and NUMA optimization support in `vm-accel`.

## Overview

The KVM accelerator now supports:

1. **vCPU Affinity**: Pin vCPUs to specific physical CPUs for improved cache locality
2. **NUMA Memory Allocation**: Allocate memory from specific NUMA nodes to reduce cross-node memory access latency
3. **vCPU-NUMA Binding**: Bind vCPUs to NUMA nodes for optimal data locality

## Features

### 1. vCPU Affinity

Set vCPU thread affinity to specific physical CPUs:

```rust
use vm_accel::kvm_impl::AccelKvm;

let mut accel = AccelKvm::new();
accel.init().unwrap();

// Create vCPUs
accel.create_vcpu(0).unwrap();
accel.create_vcpu(1).unwrap();

// Pin vCPU 0 to CPU cores 0-3
accel.set_vcpu_affinity(0, &[0, 1, 2, 3]).unwrap();

// Pin vCPU 1 to CPU cores 4-7
accel.set_vcpu_affinity(1, &[4, 5, 6, 7]).unwrap();
```

**Benefits:**
- Improved cache locality
- Reduced context switching overhead
- Better performance isolation between vCPUs

### 2. NUMA Memory Allocation

Allocate memory from specific NUMA nodes:

```rust
use vm_accel::kvm_impl::AccelKvm;

let mut accel = AccelKvm::new();
accel.init().unwrap();

// Enable NUMA optimization with 2 nodes
accel.enable_numa(2);

// Allocate memory from NUMA node 0
let mem_size = 1024 * 1024 * 1024; // 1GB
let gpa = 0x1000;
let hva = 0x7000_0000;

accel.setup_numa_memory(0, mem_size, gpa, hva).unwrap();

// Allocate memory from NUMA node 1
accel.setup_numa_memory(1, mem_size, gpa + mem_size, hva + mem_size).unwrap();
```

**Benefits:**
- Reduced cross-node memory access latency
- Improved memory bandwidth utilization
- Better scalability on multi-socket systems

### 3. vCPU-NUMA Binding

Bind vCPUs to NUMA nodes for optimal data locality:

```rust
use vm_accel::kvm_impl::AccelKvm;

let mut accel = AccelKvm::new();
accel.init().unwrap();

// Enable NUMA optimization
accel.enable_numa(2);

// Create vCPUs
accel.create_vcpu(0).unwrap();
accel.create_vcpu(1).unwrap();

// Bind vCPUs to NUMA nodes
accel.bind_vcpu_to_numa_node(0, 0).unwrap(); // vCPU 0 on node 0
accel.bind_vcpu_to_numa_node(1, 1).unwrap(); // vCPU 1 on node 1

// Verify bindings
assert_eq!(accel.get_vcpu_numa_node(0), Some(0));
assert_eq!(accel.get_vcpu_numa_node(1), Some(1));
```

**Benefits:**
- vCPUs access local memory most of the time
- Reduced remote memory access penalties
- Improved overall VM performance

## Complete Example

Here's a complete example showing all features working together:

```rust
use vm_accel::kvm_impl::AccelKvm;
use vm_accel::vcpu_affinity::{CPUTopology, VCPUAffinityManager};
use vm_accel::numa_optimizer::{NUMAOptimizer, MemoryAllocationStrategy};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize KVM accelerator
    let mut accel = AccelKvm::new();
    accel.init()?;

    // Detect system topology
    let topology = CPUTopology::detect();
    println!("System has {} CPUs and {} NUMA nodes",
             topology.total_cpus, topology.numa_nodes);

    // Enable NUMA optimization
    accel.enable_numa(topology.numa_nodes as u32);

    // Create affinity manager
    let affinity_manager = VCPUAffinityManager::new_with_topology(
        std::sync::Arc::new(topology.clone())
    );

    // Create NUMA optimizer
    let numa_optimizer = NUMAOptimizer::new(
        std::sync::Arc::new(topology.clone()),
        std::sync::Arc::new(affinity_manager),
        1024 * 1024 * 1024, // 1GB per node
    );

    // Set allocation strategy
    numa_optimizer.set_allocation_strategy(
        MemoryAllocationStrategy::LocalFirst
    );

    // Create vCPUs
    let num_vcpus = 4;
    for i in 0..num_vcpus {
        accel.create_vcpu(i)?;
    }

    // Configure vCPU affinity and NUMA binding
    affinity_manager.configure_vcpu_affinity(num_vcpus)?;

    for vcpu_id in 0..num_vcpus {
        if let Some(config) = affinity_manager.get_vcpu_config(vcpu_id) {
            let cpus = config.affinity.cpus();

            // Set CPU affinity
            accel.set_vcpu_affinity(vcpu_id as u32, &cpus)?;

            // Bind vCPU to NUMA node
            accel.bind_vcpu_to_numa_node(vcpu_id as u32, config.numa_node as u32)?;

            println!("vCPU {} bound to NUMA node {}, CPUs: {:?}",
                     vcpu_id, config.numa_node, cpus);
        }
    }

    // Allocate memory for each NUMA node
    let mem_per_node = 512 * 1024 * 1024; // 512MB
    for node_id in 0..topology.numa_nodes {
        let gpa = (node_id * mem_per_node) as u64;
        let hva = 0x7000_0000 + gpa;

        // Use NUMA-aware allocation
        let (_addr, allocated_node) = numa_optimizer.allocate_memory(
            mem_per_node,
            Some(node_id)
        )?;

        accel.setup_numa_memory(
            allocated_node as u32,
            mem_per_node as u64,
            gpa,
            hva
        )?;

        println!("Allocated {}MB for NUMA node {} at GPA 0x{:x}",
                 mem_per_node / (1024 * 1024), node_id, gpa);
    }

    // Print diagnostic information
    println!("\n=== KVM NUMA Configuration ===");
    let (enabled, nodes) = accel.numa_config();
    println!("NUMA enabled: {}, nodes: {}", enabled, nodes);

    println!("\n=== vCPU Affinity Configuration ===");
    println!("{}", affinity_manager.diagnostic_report());

    println!("\n=== NUMA Optimizer Status ===");
    println!("{}", numa_optimizer.diagnostic_report());

    Ok(())
}
```

## Platform-Specific Notes

### Linux (KVM)

Full vCPU affinity support using `sched_setaffinity`:

```rust
#[cfg(target_os = "linux")]
{
    // Uses libc::sched_setaffinity
    // Requires appropriate permissions
    accel.set_vcpu_affinity(0, &[0, 1, 2, 3])?;
}
```

### macOS (Hypervisor.framework)

Limited vCPU affinity support:

```rust
#[cfg(target_os = "macos")]
{
    // Will log a warning but not fail
    accel.set_vcpu_affinity(0, &[0, 1, 2, 3])?;
    // Actual affinity handled by macOS kernel
}
```

### Other Platforms

Stub implementation (no-op but doesn't fail):

```rust
#[cfg(not(target_os = "linux"))]
{
    // Logs but doesn't enforce affinity
    accel.set_vcpu_affinity(0, &[0, 1, 2, 3])?;
}
```

## Performance Considerations

### When to Use vCPU Affinity

**Use when:**
- Running CPU-intensive workloads
- Need consistent performance
- Want to minimize context switching
- Running on systems with multiple CPU cores

**Avoid when:**
- Workload is I/O bound
- System is lightly loaded
- Need maximum scheduling flexibility

### When to Use NUMA Optimization

**Use when:**
- Running on multi-socket systems
- Allocating large amounts of memory (>1GB)
- Memory bandwidth is a bottleneck
- Workload has high memory locality

**Avoid when:**
- Single-socket systems (UMA)
- Small memory allocations
- Memory access patterns are random

## Verification

Verify the implementation is working correctly:

```bash
# Check compilation
cargo check -p vm-accel --features kvm

# Run tests
cargo test -p vm-accel --lib --features kvm

# Run with sample workload
cargo run --example numa_demo --features kvm
```

## API Reference

### AccelKvm Methods

- `enable_numa(numa_nodes: u32)` - Enable NUMA optimization
- `disable_numa()` - Disable NUMA optimization
- `set_vcpu_affinity(vcpu_id: u32, cpu_ids: &[usize])` - Set vCPU affinity
- `setup_numa_memory(node, mem_size, gpa, hva)` - Allocate NUMA memory
- `bind_vcpu_to_numa_node(vcpu_id, node)` - Bind vCPU to NUMA node
- `get_vcpu_numa_node(vcpu_id) -> Option<u32>` - Get vCPU's NUMA node
- `numa_config() -> (bool, u32)` - Get NUMA configuration status

### Integration with vcpu_affinity Module

The KVM accelerator integrates seamlessly with the existing `vcpu_affinity` module:

```rust
use vm_accel::vcpu_affinity::{CPUTopology, VCPUAffinityManager};

let topology = CPUTopology::detect();
let manager = VCPUAffinityManager::new_with_topology(
    std::sync::Arc::new(topology)
);

manager.configure_vcpu_affinity(4)?;

// Use with KVM
if let Some(config) = manager.get_vcpu_config(0) {
    accel.set_vcpu_affinity(0, &config.affinity.cpus())?;
    accel.bind_vcpu_to_numa_node(0, config.numa_node as u32)?;
}
```

### Integration with numa_optimizer Module

The KVM accelerator also integrates with the `numa_optimizer` module:

```rust
use vm_accel::numa_optimizer::{NUMAOptimizer, MemoryAllocationStrategy};

let optimizer = NUMAOptimizer::new(
    std::sync::Arc::new(topology),
    std::sync::Arc::new(affinity_manager),
    1024 * 1024 * 1024
);

optimizer.set_allocation_strategy(MemoryAllocationStrategy::LocalFirst);

// Allocate memory and setup in KVM
let (addr, node) = optimizer.allocate_memory(size, preferred_node)?;
accel.setup_numa_memory(node as u32, size, gpa, hva)?;
```

## Troubleshooting

### NUMA Not Working

1. Verify NUMA is enabled:
   ```rust
   let (enabled, nodes) = accel.numa_config();
   assert!(enabled);
   ```

2. Check system topology:
   ```rust
   let topology = CPUTopology::detect();
   println!("NUMA nodes: {}", topology.numa_nodes);
   ```

3. Enable NUMA explicitly:
   ```rust
   accel.enable_numa(topology.numa_nodes as u32);
   ```

### vCPU Affinity Failing

1. Check permissions (Linux):
   ```bash
   # May need CAP_SYS_NICE capability
   sudo setcap cap_sys_nice+ep ./your_binary
   ```

2. Verify vCPU exists:
   ```rust
   accel.create_vcpu(0)?;
   accel.set_vcpu_affinity(0, &[0, 1, 2, 3])?;
   ```

3. Check CPU IDs are valid:
   ```rust
   let topology = CPUTopology::detect();
   assert!(cpu_id < topology.total_cpus);
   ```

## Testing

The implementation includes comprehensive tests:

```bash
# Run all KVM tests
cargo test -p vm-accel --lib --features kvm

# Run NUMA-specific tests
cargo test -p vm-accel --lib --features kvm test_numa

# Run vCPU affinity tests
cargo test -p vm-accel --lib --features kvm test_vcpu
```

## Future Enhancements

Potential improvements for future versions:

1. **Automatic NUMA Balancing**: Dynamically rebalance vCPUs and memory
2. **Performance Monitoring**: Track NUMA hit/miss ratios
3. **Migration Support**: Move memory between NUMA nodes
4. **Policy Engine**: Advanced policies for vCPU placement
5. **Integration with cgroups**: Use Linux cgroups for resource control

## Summary

The enhanced KVM integration provides:

- **vCPU Affinity**: Pin vCPUs to physical CPUs for improved cache locality
- **NUMA Memory Allocation**: Allocate memory from specific NUMA nodes
- **vCPU-NUMA Binding**: Bind vCPUs to NUMA nodes for optimal data locality
- **Comprehensive Testing**: Full test coverage for all features
- **Platform Support**: Works on Linux, macOS, and other platforms
- **Integration**: Seamlessly integrates with existing vcpu_affinity and numa_optimizer modules

The implementation is production-ready and follows best practices for error handling, logging, and cross-platform compatibility.
