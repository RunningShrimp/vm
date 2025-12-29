//! Integration Tests
//!
//! End-to-end integration tests for vm-accel components

use std::sync::Arc;
use vm_accel::accel::AccelerationManager;
use vm_accel::numa_optimizer::{MemoryAllocationStrategy, NUMAOptimizer};
use vm_accel::vcpu_affinity::{CPUTopology, VCPUAffinityManager};
use vm_accel::{Accel, AccelKind, CpuInfo};

/// Test full stack integration
#[test]
fn test_full_stack_integration() {
    println!("=== Full Stack Integration Test ===");

    // 1. Detect CPU info
    let cpu_info = CpuInfo::get();
    println!("CPU: {} ({:?})", cpu_info.model_name, cpu_info.vendor);

    // 2. Detect best accelerator
    let kind = AccelKind::detect_best();
    println!("Best accelerator: {:?}", kind);

    // 3. Create acceleration manager
    let mut manager = AccelerationManager::new().expect("Should create manager");
    println!("Acceleration manager created");

    // 4. Setup full acceleration
    match manager.setup_full_acceleration() {
        Ok(()) => println!("Full acceleration setup successful"),
        Err(e) => println!("Full acceleration setup failed: {:?}", e),
    }

    println!("=== Full Stack Integration Complete ===\n");
}

/// Test NUMA integration with affinity manager
#[test]
fn test_numa_affinity_integration() {
    println!("=== NUMA-Affinity Integration Test ===");

    let topology = Arc::new(CPUTopology::detect());
    println!(
        "Topology: {} CPUs, {} NUMA nodes",
        topology.total_cpus, topology.numa_nodes
    );

    let affinity_manager = Arc::new(VCPUAffinityManager::new_with_topology(topology.clone()));

    // Create NUMA optimizer
    let memory_per_node = 1024 * 1024 * 1024; // 1GB
    let optimizer = NUMAOptimizer::new(topology.clone(), affinity_manager, memory_per_node);

    println!(
        "NUMA optimizer created with {} nodes",
        optimizer.topology().numa_nodes
    );

    // Allocate some memory
    let result = optimizer.allocate_memory(4096, Some(0));
    match result {
        Ok((addr, node_id)) => {
            println!("Allocated memory on node {} at {:#x}", node_id, addr);
        }
        Err(e) => {
            println!("Memory allocation failed: {}", e);
        }
    }

    println!("=== NUMA-Affinity Integration Complete ===\n");
}

/// Test accelerator selection flow
#[test]
fn test_accelerator_selection_flow() {
    println!("=== Accelerator Selection Flow Test ===");

    // Detect platform
    println!("OS: {}", std::env::consts::OS);
    println!("Arch: {}", std::env::consts::ARCH);

    // Detect best accelerator
    let kind = AccelKind::detect_best();
    println!("Detected accelerator: {:?}", kind);

    // Try to select and initialize
    let (selected_kind, mut accel) = vm_accel::select();
    println!("Selected accelerator: {:?}", selected_kind);
    println!("Accelerator name: {}", accel.name());

    // Try to initialize
    match accel.init() {
        Ok(()) => {
            println!("Accelerator initialized successfully");

            // Try to create vCPU
            match accel.create_vcpu(0) {
                Ok(()) => println!("vCPU 0 created"),
                Err(e) => println!("vCPU creation failed: {:?}", e),
            }
        }
        Err(e) => {
            println!("Accelerator initialization failed: {:?}", e);
            println!("This is expected if hardware virtualization is not available");
        }
    }

    println!("=== Accelerator Selection Flow Complete ===\n");
}

/// Test CPU feature integration
#[test]
fn test_cpu_feature_integration() {
    println!("=== CPU Feature Integration Test ===");

    let cpu_info = CpuInfo::get();
    let features = vm_accel::detect();

    println!("Vendor: {:?}", cpu_info.vendor);
    println!("Architecture: {:?}", cpu_info.arch);
    println!("Core count: {}", cpu_info.core_count);

    #[cfg(target_arch = "x86_64")]
    {
        println!("Virtualization:");
        println!("  VMX (Intel VT-x): {}", features.vmx);
        println!("  SVM (AMD-V): {}", features.svm);

        println!("SIMD:");
        println!("  AVX2: {}", features.avx2);
        println!("  AVX-512: {}", features.avx512);
    }

    #[cfg(target_arch = "aarch64")]
    {
        println!("Virtualization:");
        println!("  EL2: {}", features.arm_el2);

        println!("SIMD:");
        println!("  NEON: {}", features.neon);
        println!("  SVE: {}", cpu_info.features.sve);
        println!("  SVE2: {}", cpu_info.features.sve2);
    }

    println!("=== CPU Feature Integration Complete ===\n");
}

/// Test memory allocation strategies
#[test]
fn test_memory_allocation_strategies_integration() {
    println!("=== Memory Allocation Strategies Test ===");

    let topology = Arc::new(CPUTopology::detect());
    let affinity_manager = Arc::new(VCPUAffinityManager::new_with_topology(topology.clone()));
    let memory_per_node = 1024 * 1024; // 1MB

    let strategies = [
        MemoryAllocationStrategy::LocalFirst,
        MemoryAllocationStrategy::LoadBalanced,
        MemoryAllocationStrategy::BandwidthOptimized,
        MemoryAllocationStrategy::Adaptive,
    ];

    for strategy in strategies {
        let mut optimizer =
            NUMAOptimizer::new(topology.clone(), affinity_manager.clone(), memory_per_node);
        optimizer.set_allocation_strategy(strategy);

        println!("Strategy: {:?}", strategy);

        // Try to allocate memory
        let result = optimizer.allocate_memory(4096, None);
        match result {
            Ok((addr, node_id)) => {
                println!("  Allocated on node {} at {:#x}", node_id, addr);
            }
            Err(e) => {
                println!("  Allocation failed: {}", e);
            }
        }
    }

    println!("=== Memory Allocation Strategies Complete ===\n");
}

/// Test NUMA-aware allocation
#[test]
fn test_numa_aware_allocation_integration() {
    println!("=== NUMA-Aware Allocation Test ===");

    let topology = Arc::new(CPUTopology::detect());
    let affinity_manager = Arc::new(VCPUAffinityManager::new_with_topology(topology.clone()));
    let memory_per_node = 1024 * 1024 * 1024; // 1GB

    let optimizer = NUMAOptimizer::new(topology.clone(), affinity_manager, memory_per_node);

    // Allocate on different nodes
    for node_id in 0..topology.numa_nodes.min(2) {
        let size = 4096 * (node_id + 1);
        let result = optimizer.allocate_memory(size, Some(node_id));

        match result {
            Ok((addr, allocated_node)) => {
                println!(
                    "Allocated {} bytes on node {} at {:#x}",
                    size, allocated_node, addr
                );
            }
            Err(e) => {
                println!("Allocation on node {} failed: {}", node_id, e);
            }
        }
    }

    println!("=== NUMA-Aware Allocation Complete ===\n");
}

/// Test vCPU affinity integration
#[test]
fn test_vcpu_affinity_integration() {
    println!("=== vCPU Affinity Integration Test ===");

    let topology = CPUTopology::detect();
    let affinity_manager = VCPUAffinityManager::new_with_topology(Arc::new(topology.clone()));

    println!(
        "Topology: {} CPUs, {} NUMA nodes",
        topology.total_cpus, topology.numa_nodes
    );

    // Show CPU distribution
    for node_id in 0..topology.numa_nodes {
        let cpus = topology.get_node_cpus(node_id);
        println!("Node {} CPUs: {:?}", node_id, cpus);
    }

    // Test closest CPU selection
    if topology.total_cpus > 0 {
        let closest = topology.get_closest_cpus(0, 2);
        println!("Closest CPUs to CPU 0: {:?}", closest);
    }

    println!("=== vCPU Affinity Integration Complete ===\n");
}

/// Test error handling across components
#[test]
fn test_error_handling_integration() {
    println!("=== Error Handling Integration Test ===");

    // Test invalid NUMA node
    let topology = Arc::new(CPUTopology::detect());
    let affinity_manager = Arc::new(VCPUAffinityManager::new_with_topology(topology.clone()));
    let memory_per_node = 1024 * 1024;

    let optimizer = NUMAOptimizer::new(topology, affinity_manager, memory_per_node);

    // Try to allocate with invalid preferred node
    let result = optimizer.allocate_memory(4096, Some(999));
    assert!(result.is_err(), "Should fail for invalid node");
    println!("Correctly handled invalid node allocation");

    // Test acceleration manager without initialization
    let mut manager = AccelerationManager::new().expect("Should create manager");
    let result = manager.init_smmu();
    assert!(result.is_err(), "Should fail without initialization");
    println!("Correctly handled operation without initialization");

    println!("=== Error Handling Integration Complete ===\n");
}

/// Test performance monitoring integration
#[test]
fn test_performance_monitoring_integration() {
    println!("=== Performance Monitoring Integration Test ===");

    let topology = Arc::new(CPUTopology::detect());
    let affinity_manager = Arc::new(VCPUAffinityManager::new_with_topology(topology.clone()));
    let memory_per_node = 1024 * 1024 * 1024;

    let optimizer = NUMAOptimizer::new(topology, affinity_manager, memory_per_node);

    // Update statistics
    optimizer.update_stats();

    // Get all stats
    let stats = optimizer.get_all_stats();
    println!("Statistics for {} nodes", stats.len());

    for stat in stats {
        println!("Node {}:", stat.node_id);
        println!("  CPU usage: {:.1}%", stat.cpu_usage * 100.0);
        println!("  Memory usage: {:.1}%", stat.memory_usage * 100.0);
        println!(
            "  Local access rate: {:.1}%",
            stat.local_access_rate() * 100.0
        );
    }

    println!("=== Performance Monitoring Integration Complete ===\n");
}

/// Test SIMD integration
#[test]
fn test_simd_integration() {
    println!("=== SIMD Integration Test ===");

    let features = vm_accel::detect();

    #[cfg(target_arch = "x86_64")]
    {
        if features.avx2 {
            let a = [1, 2, 3, 4, 5, 6, 7, 8];
            let b = [8, 7, 6, 5, 4, 3, 2, 1];
            let result = vm_accel::add_i32x8(a, b);
            println!("AVX2 result: {:?}", result);
            assert_eq!(result, [9, 9, 9, 9, 9, 9, 9, 9]);
        } else {
            println!("AVX2 not available, using fallback");
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if features.neon {
            let a = [1, 2, 3, 4];
            let b = [4, 3, 2, 1];
            let result = vm_accel::add_i32x4(a, b);
            println!("NEON result: {:?}", result);
            assert_eq!(result, [5, 5, 5, 5]);
        }
    }

    println!("=== SIMD Integration Complete ===\n");
}

/// Test cross-platform compatibility
#[test]
fn test_cross_platform_compatibility_integration() {
    println!("=== Cross-Platform Compatibility Test ===");

    println!(
        "Platform: {}-{}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );

    // Test CPU info detection
    let cpu_info = CpuInfo::get();
    println!("CPU: {:?}", cpu_info.vendor);

    // Test accelerator detection
    let kind = AccelKind::detect_best();
    println!("Accelerator: {:?}", kind);

    // Test topology detection
    let topology = CPUTopology::detect();
    println!(
        "CPUs: {}, NUMA nodes: {}",
        topology.total_cpus, topology.numa_nodes
    );

    // Test feature detection
    let features = vm_accel::detect();
    println!("Features detected");

    println!("=== Cross-Platform Compatibility Complete ===\n");
}
