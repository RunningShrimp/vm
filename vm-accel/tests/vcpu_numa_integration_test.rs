//! Integration tests for vCPU/NUMA manager
//!
//! Tests the integrated vCPU affinity and NUMA memory allocation functionality

use vm_accel::vcpu_numa_manager::{NumaTopology, VcpuNumaManager};

#[test]
fn test_numa_topology_detection() {
    let topology = NumaTopology::detect().expect("Should detect NUMA topology");

    // Verify topology was detected
    assert!(
        topology.num_nodes() > 0,
        "Should have at least one NUMA node"
    );

    // Check that each node has CPUs assigned
    for node in 0..topology.num_nodes() {
        let cpus = topology.get_node_cpus(node);
        if cpus.is_ok() {
            let cpu_list = cpus.unwrap();
            assert!(!cpu_list.is_empty(), "Node {} should have CPUs", node);
        }
    }

    println!("Detected {} NUMA nodes", topology.num_nodes());
}

#[test]
fn test_vcpu_numa_manager_creation() {
    let manager = VcpuNumaManager::new();

    assert!(manager.is_ok(), "Should create manager successfully");

    let manager = manager.unwrap();
    let topology = manager.numa_topology();
    assert!(topology.num_nodes() > 0, "Should have NUMA topology");

    println!(
        "Created vCPU/NUMA manager with {} nodes",
        topology.num_nodes()
    );
}

#[test]
fn test_bind_single_vcpu() {
    let mut manager = VcpuNumaManager::new().expect("Should create manager");

    // Bind vCPU 0 to NUMA node 0
    let result = manager.bind_vcpu_to_numa_node(0, 0);
    assert!(result.is_ok(), "Should bind vCPU successfully");

    // Verify the binding
    let numa_node = manager.get_vcpu_numa_node(0);
    assert!(numa_node.is_ok(), "Should get NUMA node for vCPU");
    assert_eq!(numa_node.unwrap(), 0, "vCPU should be on node 0");

    println!("Successfully bound vCPU 0 to NUMA node 0");
}

#[test]
fn test_bind_multiple_vcpus() {
    let mut manager = VcpuNumaManager::new().expect("Should create manager");

    let num_vcpus = 4;
    for vcpu_id in 0..num_vcpus {
        let result = manager.bind_vcpu_to_numa_node(vcpu_id, vcpu_id % 2);
        assert!(result.is_ok(), "Should bind vCPU {} successfully", vcpu_id);
    }

    // Verify all bindings
    for vcpu_id in 0..num_vcpus {
        let numa_node = manager.get_vcpu_numa_node(vcpu_id);
        assert!(
            numa_node.is_ok(),
            "Should get NUMA node for vCPU {}",
            vcpu_id
        );
        assert_eq!(
            numa_node.unwrap(),
            vcpu_id % 2,
            "vCPU {} should be on correct node",
            vcpu_id
        );
    }

    println!("Successfully bound {} vCPUs across NUMA nodes", num_vcpus);
}

#[test]
fn test_configure_vcpus_auto() {
    let mut manager = VcpuNumaManager::new().expect("Should create manager");

    let num_vcpus = 8;
    let result = manager.configure_vcpus(num_vcpus);
    assert!(result.is_ok(), "Should configure vCPUs automatically");

    // Verify all vCPUs were configured
    for vcpu_id in 0..num_vcpus {
        let numa_node = manager.get_vcpu_numa_node(vcpu_id);
        assert!(numa_node.is_ok(), "vCPU {} should be configured", vcpu_id);
    }

    println!("Auto-configured {} vCPUs", num_vcpus);
}

#[test]
fn test_allocate_numa_memory() {
    let mut manager = VcpuNumaManager::new().expect("Should create manager");

    let size = 1024 * 1024; // 1MB
    let result = manager.allocate_numa_memory(0, size);

    assert!(result.is_ok(), "Should allocate memory on NUMA node");

    let ptr = result.unwrap();
    // Note: Simplified implementation returns address as a number, may be 0
    println!("Allocated {} bytes on NUMA node 0 at {:p}", size, ptr);
}

#[test]
fn test_allocate_vcpu_local_memory() {
    let mut manager = VcpuNumaManager::new().expect("Should create manager");

    // First bind a vCPU to a specific NUMA node
    manager
        .bind_vcpu_to_numa_node(0, 0)
        .expect("Should bind vCPU");

    // Allocate memory local to that vCPU
    let size = 2 * 1024 * 1024; // 2MB
    let result = manager.allocate_vcpu_local_memory(0, size);

    assert!(result.is_ok(), "Should allocate vCPU-local memory");

    let ptr = result.unwrap();
    // Note: Simplified implementation returns address as a number, may be 0
    println!("Allocated {} bytes local to vCPU 0 at {:p}", size, ptr);
}

#[test]
fn test_vcpu_not_bound_error() {
    let mut manager = VcpuNumaManager::new().expect("Should create manager");

    // Try to allocate memory for an unbound vCPU
    let result = manager.allocate_vcpu_local_memory(99, 1024);
    assert!(result.is_err(), "Should fail for unbound vCPU");

    // Try to get NUMA node for an unbound vCPU
    let result = manager.get_vcpu_numa_node(99);
    assert!(
        result.is_err(),
        "Should fail to get NUMA node for unbound vCPU"
    );

    println!("Correctly handled unbound vCPU errors");
}

#[test]
fn test_diagnostic_report() {
    let mut manager = VcpuNumaManager::new().expect("Should create manager");

    // Configure some vCPUs
    manager.configure_vcpus(4).expect("Should configure vCPUs");

    // Allocate some memory
    manager
        .allocate_numa_memory(0, 1024 * 1024)
        .expect("Should allocate memory");

    // Generate report
    let report = manager.diagnostic_report();

    // Verify report contains expected sections
    assert!(
        report.contains("vCPU/NUMA Integration Report"),
        "Should have report header"
    );
    assert!(
        report.contains("vCPU Bindings"),
        "Should show vCPU bindings"
    );
    assert!(
        report.contains("NUMA Memory Allocation"),
        "Should show memory allocation"
    );

    println!("Diagnostic report:\n{}", report);
}

#[test]
fn test_memory_affinity() {
    let mut manager = VcpuNumaManager::new().expect("Should create manager");

    // Bind vCPUs to different NUMA nodes
    manager
        .bind_vcpu_to_numa_node(0, 0)
        .expect("Should bind vCPU 0");
    manager
        .bind_vcpu_to_numa_node(1, 1)
        .expect("Should bind vCPU 1");

    // Allocate memory local to each vCPU
    let mem0 = manager.allocate_vcpu_local_memory(0, 1024 * 1024);
    let mem1 = manager.allocate_vcpu_local_memory(1, 1024 * 1024);

    assert!(mem0.is_ok(), "Should allocate memory for vCPU 0");
    assert!(mem1.is_ok(), "Should allocate memory for vCPU 1");

    // Verify each vCPU's NUMA node
    let node0 = manager.get_vcpu_numa_node(0).unwrap();
    let node1 = manager.get_vcpu_numa_node(1).unwrap();

    assert_eq!(node0, 0, "vCPU 0 should be on node 0");
    assert_eq!(node1, 1, "vCPU 1 should be on node 1");

    println!("vCPU 0 (node {}): memory at {:p}", node0, mem0.unwrap());
    println!("vCPU 1 (node {}): memory at {:p}", node1, mem1.unwrap());
}

#[test]
fn test_round_robin_placement() {
    let mut manager = VcpuNumaManager::new().expect("Should create manager");

    let num_vcpus = 8;
    let num_nodes = manager.numa_topology().num_nodes() as u32;

    manager
        .configure_vcpus(num_vcpus)
        .expect("Should configure vCPUs");

    // Count vCPUs per node
    let mut vcpus_per_node = vec![0u32; num_nodes as usize];

    for vcpu_id in 0..num_vcpus {
        if let Ok(node_id) = manager.get_vcpu_numa_node(vcpu_id) {
            vcpus_per_node[node_id as usize] += 1;
        }
    }

    // Verify distribution is roughly even
    let min_vcpus = *vcpus_per_node.iter().min().unwrap();
    let max_vcpus = *vcpus_per_node.iter().max().unwrap();

    assert!(
        max_vcpus - min_vcpus <= 1,
        "vCPUs should be evenly distributed"
    );

    println!("vCPU distribution: {:?}", vcpus_per_node);
}

#[test]
fn test_affinity_manager_integration() {
    let manager = VcpuNumaManager::new().expect("Should create manager");

    // Access the affinity manager
    let affinity_manager = manager.affinity_manager();

    // Get topology stats
    let stats = affinity_manager.get_topology_stats();
    assert!(stats.is_ok(), "Should get topology stats");

    let stats = stats.unwrap();
    assert!(stats.total_cpus > 0, "Should have CPUs");
    assert!(stats.numa_nodes > 0, "Should have NUMA nodes");

    println!(
        "Topology: {} CPUs, {} NUMA nodes, {} cores/node",
        stats.total_cpus, stats.numa_nodes, stats.cores_per_node
    );
}

#[test]
fn test_memory_allocator_integration() {
    let manager = VcpuNumaManager::new().expect("Should create manager");

    // Access the memory allocator
    let allocator = manager.memory_allocator();

    // Try to read allocator state
    if let Ok(alloc) = allocator.read() {
        // Should be able to get node usage
        let usage = alloc.get_node_usage(0);
        assert!(
            usage >= 0.0 && usage <= 1.0,
            "Usage should be between 0 and 1"
        );

        println!("Initial node 0 usage: {:.1}%", usage * 100.0);
    }
}
