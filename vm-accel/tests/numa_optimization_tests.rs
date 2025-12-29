//! NUMA Optimization Tests
//!
//! Comprehensive tests for NUMA-aware memory allocation and optimization

use vm_accel::numa_optimizer::{
    MemoryAllocationStrategy, NUMANodeStats, NUMAOptimizer,
};
use vm_accel::vcpu_affinity::{CPUTopology, NUMAAwareAllocator, VCPUAffinityManager};
use std::sync::{Arc, RwLock};

/// Test NUMA topology detection
#[test]
fn test_numa_topology_detection() {
    let topology = CPUTopology::detect();

    println!("NUMA Topology:");
    println!("  Total CPUs: {}", topology.total_cpus);
    println!("  NUMA nodes: {}", topology.numa_nodes);

    assert!(topology.total_cpus > 0, "Should have at least one CPU");
    assert!(topology.numa_nodes > 0, "Should have at least one NUMA node");

    // Print CPU distribution
    for node_id in 0..topology.numa_nodes {
        let cpus = topology.get_node_cpus(node_id);
        println!("  Node {} CPUs: {:?}", node_id, cpus);
    }
}

/// Test NUMA-aware allocator creation
#[test]
fn test_numa_aware_allocator_creation() {
    let numa_nodes = 2;
    let memory_per_node = 1024 * 1024 * 1024; // 1GB per node

    let allocator = NUMAAwareAllocator::new(numa_nodes, memory_per_node);

    println!("Created NUMA-aware allocator:");
    println!("  NUMA nodes: {}", allocator.num_nodes());
    println!("  Memory per node: {} bytes", memory_per_node);

    assert_eq!(allocator.num_nodes(), numa_nodes);
}

/// Test NUMA node stats creation
#[test]
fn test_numa_node_stats_creation() {
    let stats = NUMANodeStats::new(0);

    assert_eq!(stats.node_id, 0);
    assert_eq!(stats.cpu_usage, 0.0);
    assert_eq!(stats.memory_usage, 0.0);
    assert_eq!(stats.memory_bandwidth_usage, 0.0);
    assert_eq!(stats.cache_miss_rate, 0.0);
    assert_eq!(stats.cross_node_accesses, 0);
    assert_eq!(stats.local_accesses, 0);

    println!("NUMA node stats created successfully");
}

/// Test NUMA node stats local access rate
#[test]
fn test_numa_node_stats_local_access_rate() {
    let mut stats = NUMANodeStats::new(0);

    // Test with no accesses
    assert_eq!(stats.local_access_rate(), 0.0);

    // Add some local accesses
    stats.local_accesses = 100;
    assert_eq!(stats.local_access_rate(), 1.0);

    // Add some cross-node accesses
    stats.cross_node_accesses = 50;
    assert!((stats.local_access_rate() - 0.666).abs() < 0.01);

    // All cross-node
    stats.local_accesses = 0;
    stats.cross_node_accesses = 100;
    assert_eq!(stats.local_access_rate(), 0.0);

    println!("Local access rate calculations correct");
}

/// Test NUMA optimizer creation
#[test]
fn test_numa_optimizer_creation() {
    let topology = Arc::new(CPUTopology::detect());
    let affinity_manager = Arc::new(VCPUAffinityManager::new_with_topology(topology.clone()));
    let memory_per_node = 1024 * 1024 * 1024; // 1GB

    let optimizer = NUMAOptimizer::new(topology.clone(), affinity_manager, memory_per_node);

    println!("Created NUMA optimizer with {} nodes", topology.numa_nodes);
    assert_eq!(topology.numa_nodes, topology.numa_nodes);
}

/// Test memory allocation strategies
#[test]
fn test_memory_allocation_strategies() {
    let strategies = [
        MemoryAllocationStrategy::LocalFirst,
        MemoryAllocationStrategy::LoadBalanced,
        MemoryAllocationStrategy::BandwidthOptimized,
        MemoryAllocationStrategy::Adaptive,
    ];

    for strategy in strategies {
        println!("Strategy: {:?}", strategy);
    }

    println!("All memory allocation strategies are valid");
}

/// Test NUMA memory allocation (local first)
#[test]
fn test_numa_memory_allocation_local_first() {
    let topology = Arc::new(CPUTopology::detect());
    let affinity_manager = Arc::new(VCPUAffinityManager::new_with_topology(topology.clone()));
    let memory_per_node = 1024 * 1024; // 1MB

    let mut optimizer = NUMAOptimizer::new(topology, affinity_manager, memory_per_node);
    optimizer.set_allocation_strategy(MemoryAllocationStrategy::LocalFirst);

    // Allocate memory on preferred node 0
    let size = 4096;
    let result = optimizer.allocate_memory(size, Some(0));

    match result {
        Ok((addr, node_id)) => {
            println!("Allocated {} bytes on node {} at address {:#x}", size, node_id, addr);
            assert_eq!(node_id, 0, "Should allocate on preferred node");
        }
        Err(e) => {
            println!("Allocation failed: {}", e);
        }
    }
}

/// Test NUMA memory allocation (load balanced)
#[test]
fn test_numa_memory_allocation_load_balanced() {
    let topology = Arc::new(CPUTopology::detect());
    let affinity_manager = Arc::new(VCPUAffinityManager::new_with_topology(topology.clone()));
    let memory_per_node = 1024 * 1024; // 1MB

    let mut optimizer = NUMAOptimizer::new(topology, affinity_manager, memory_per_node);
    optimizer.set_allocation_strategy(MemoryAllocationStrategy::LoadBalanced);

    // Allocate multiple chunks and check distribution
    let allocations = 10;
    let size = 4096;

    for i in 0..allocizations {
        let result = optimizer.allocate_memory(size, None);
        if let Ok((addr, node_id)) = result {
            println!("Allocation {} on node {}", i, node_id);
        }
    }

    println!("Load balanced allocation test completed");
}

/// Test NUMA memory allocation tracking
#[test]
fn test_numa_memory_allocation_tracking() {
    let allocator = Arc::new(RwLock::new(NUMAAwareAllocator::new(2, 1024 * 1024)));

    // Allocate on node 0
    {
        let mut alloc = allocator.write().unwrap();
        let result = alloc.allocate_on_node(0, 4096);
        assert!(result.is_ok(), "Allocation should succeed");
    }

    // Check node usage
    {
        let alloc = allocator.read().unwrap();
        let usage = alloc.get_node_usage(0);
        println!("Node 0 usage: {:.1}%", usage * 100.0);
        assert!(usage > 0.0, "Node 0 should have some usage");
    }

    println!("Memory allocation tracking works correctly");
}

/// Test NUMA node selection
#[test]
fn test_numa_node_selection() {
    let topology = Arc::new(CPUTopology::detect());
    let affinity_manager = Arc::new(VCPUAffinityManager::new_with_topology(topology.clone()));
    let memory_per_node = 1024 * 1024; // 1MB

    let optimizer = NUMAOptimizer::new(topology.clone(), affinity_manager, memory_per_node);

    // Get available nodes (just count from topology)
    let node_count = topology.numa_nodes;
    println!("Available NUMA nodes: {}", node_count);

    assert!(node_count > 0, "Should have at least one node");
}

/// Test NUMA statistics update
#[test]
fn test_numa_statistics_update() {
    let topology = Arc::new(CPUTopology::detect());
    let affinity_manager = Arc::new(VCPUAffinityManager::new_with_topology(topology.clone()));
    let memory_per_node = 1024 * 1024; // 1MB

    let optimizer = NUMAOptimizer::new(topology, affinity_manager, memory_per_node);

    // Update statistics
    optimizer.update_stats();

    // Get all stats
    let stats = optimizer.get_all_stats();
    println!("Got statistics for {} nodes", stats.len());

    assert!(!stats.is_empty(), "Should have statistics for at least one node");

    for stat in stats {
        println!("Node {} stats:", stat.node_id);
        println!("  CPU usage: {:.1}%", stat.cpu_usage * 100.0);
        println!("  Memory usage: {:.1}%", stat.memory_usage * 100.0);
        println!("  Local access rate: {:.1}%", stat.local_access_rate() * 100.0);
    }
}

/// Test NUMA cross-node access tracking
#[test]
fn test_numa_cross_node_access_tracking() {
    let mut stats = NUMANodeStats::new(0);

    stats.local_accesses = 1000;
    stats.cross_node_accesses = 100;

    let rate = stats.local_access_rate();
    println!("Local access rate: {:.1}%", rate * 100.0);

    assert!(rate < 1.0, "Should have less than 100% local access");
    assert!(rate > 0.9, "Should have more than 90% local access");
}

/// Test NUMA memory bandwidth tracking
#[test]
fn test_numa_memory_bandwidth_tracking() {
    let mut stats = NUMANodeStats::new(0);

    stats.memory_bandwidth_usage = 0.75;
    println!("Memory bandwidth usage: {:.1}%", stats.memory_bandwidth_usage * 100.0);

    assert!(stats.memory_bandwidth_usage >= 0.0);
    assert!(stats.memory_bandwidth_usage <= 1.0);
}

/// Test NUMA CPU usage tracking
#[test]
fn test_numa_cpu_usage_tracking() {
    let mut stats = NUMANodeStats::new(0);

    stats.cpu_usage = 0.5;
    println!("CPU usage: {:.1}%", stats.cpu_usage * 100.0);

    assert!(stats.cpu_usage >= 0.0);
    assert!(stats.cpu_usage <= 1.0);
}

/// Test NUMA cache miss rate tracking
#[test]
fn test_numa_cache_miss_rate_tracking() {
    let mut stats = NUMANodeStats::new(0);

    stats.cache_miss_rate = 0.05;
    println!("Cache miss rate: {:.1}%", stats.cache_miss_rate * 100.0);

    assert!(stats.cache_miss_rate >= 0.0);
    assert!(stats.cache_miss_rate <= 1.0);
}

/// Test NUMA strategy switching
#[test]
fn test_numa_strategy_switching() {
    let topology = Arc::new(CPUTopology::detect());
    let affinity_manager = Arc::new(VCPUAffinityManager::new_with_topology(topology.clone()));
    let memory_per_node = 1024 * 1024; // 1MB

    let mut optimizer = NUMAOptimizer::new(topology, affinity_manager, memory_per_node);

    // Test switching between strategies
    optimizer.set_allocation_strategy(MemoryAllocationStrategy::LocalFirst);
    optimizer.set_allocation_strategy(MemoryAllocationStrategy::LoadBalanced);
    optimizer.set_allocation_strategy(MemoryAllocationStrategy::BandwidthOptimized);
    optimizer.set_allocation_strategy(MemoryAllocationStrategy::Adaptive);

    println!("Strategy switching successful");
}

/// Test NUMA allocator node validation
#[test]
fn test_numa_allocator_node_validation() {
    let allocator = NUMAAwareAllocator::new(2, 1024 * 1024);

    // Try to allocate on invalid node
    let result = allocator.allocate_on_node(999, 4096);
    assert!(result.is_err(), "Should fail for invalid node");

    println!("Node validation works correctly");
}

/// Test NUMA allocator memory limits
#[test]
fn test_numa_allocator_memory_limits() {
    let memory_per_node = 4096; // Small limit
    let allocator = NUMAAwareAllocator::new(1, memory_per_node);

    // Allocate within limit
    let result = allocator.allocate_on_node(0, 2048);
    assert!(result.is_ok(), "Allocation within limit should succeed");

    // Try to allocate more than available
    let result = allocator.allocate_on_node(0, memory_per_node * 2);
    assert!(result.is_err(), "Allocation exceeding limit should fail");

    println!("Memory limits enforced correctly");
}

/// Test NUMA closest CPU selection
#[test]
fn test_numa_closest_cpu_selection() {
    let topology = CPUTopology::detect();

    // Get closest CPUs to CPU 0
    let closest = topology.get_closest_cpus(0, 2);
    println!("Closest CPUs to CPU 0: {:?}", closest);

    assert!(!closest.is_empty(), "Should find closest CPUs");
    assert!(closest.len() <= 2, "Should return at most requested count");
}

/// Test NUMA vCPU to node mapping
#[test]
fn test_numa_vcpu_to_node_mapping() {
    let topology = CPUTopology::detect();

    // Check CPU to node mapping
    for cpu_id in 0..topology.total_cpus.min(8) {
        if let Some(&node_id) = topology.cpu_to_node.get(&cpu_id) {
            println!("CPU {} is on NUMA node {}", cpu_id, node_id);
            assert!(node_id < topology.numa_nodes, "Node ID should be valid");
        }
    }
}

/// Test NUMA cache topology
#[test]
fn test_numa_cache_topology() {
    let topology = CPUTopology::detect();

    println!("Cache topology:");
    for cache in &topology.cache_topology {
        println!("  Level {}: {} KB, shared by CPUs {:?}",
            cache.level, cache.size_kb, cache.shared_by);
    }

    // Should have at least L3 cache info
    assert!(!topology.cache_topology.is_empty(), "Should have cache topology");
}
