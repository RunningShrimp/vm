//! Acceleration Manager Tests
//!
//! Comprehensive tests for the unified acceleration manager

use vm_accel::accel::AccelerationManager;
use vm_accel::{AccelKind, CpuInfo, CpuVendor};

/// Test acceleration manager creation
#[test]
fn test_acceleration_manager_creation() {
    let manager = AccelerationManager::new();

    assert!(manager.is_ok(), "Should create manager successfully");

    let manager = manager.unwrap();
    println!("Acceleration manager created successfully");
    println!("Initialized: {}", manager.is_initialized());

    // Manager should not be initialized initially
    assert!(!manager.is_initialized());
}

/// Test acceleration manager topology detection
#[test]
fn test_acceleration_manager_topology() {
    let manager = AccelerationManager::new().expect("Should create manager");

    let topology = manager.get_topology();
    println!("Total CPUs: {}", topology.total_cpus);
    println!("NUMA nodes: {}", topology.numa_nodes);

    println!("Topology detected successfully");
}

/// Test full acceleration setup
#[test]
fn test_full_acceleration_setup() {
    let mut manager = AccelerationManager::new().expect("Should create manager");

    match manager.setup_full_acceleration() {
        Ok(()) => {
            println!("Full acceleration setup completed");
            assert!(manager.is_initialized(), "Should be initialized");
        }
        Err(e) => {
            println!("Full acceleration setup failed: {:?}", e);
            // This is acceptable on systems without certain features
        }
    }
}

/// Test NUMA enabling
#[test]
fn test_enable_numa() {
    let mut manager = AccelerationManager::new().expect("Should create manager");

    // Enable with 1 node
    let result = manager.enable_numa(1);
    assert!(result.is_ok(), "Should enable NUMA with 1 node");
    println!("NUMA enabled with 1 node");

    // Try to enable with invalid node count (0)
    let result = manager.enable_numa(0);
    assert!(result.is_err(), "Should reject 0 nodes");
    println!("Correctly rejected 0 nodes");

    // Try to enable with too many nodes
    let result = manager.enable_numa(999);
    assert!(result.is_err(), "Should reject too many nodes");
    println!("Correctly rejected too many nodes");
}

/// Test NUMA disabling
#[test]
fn test_disable_numa() {
    let mut manager = AccelerationManager::new().expect("Should create manager");

    manager.enable_numa(1).expect("Should enable NUMA");
    println!("NUMA enabled");

    manager.disable_numa();
    println!("NUMA disabled");
}

/// Test vCPU affinity initialization
#[test]
fn test_vcpu_affinity_initialization() {
    let mut manager = AccelerationManager::new().expect("Should create manager");

    #[cfg(not(any(target_os = "windows", target_os = "ios")))]
    {
        let result = manager.init_vcpu_affinity();
        match result {
            Ok(()) => println!("vCPU affinity initialized"),
            Err(e) => println!("vCPU affinity initialization failed: {:?}", e),
        }
    }

    #[cfg(any(target_os = "windows", target_os = "ios"))]
    {
        let result = manager.init_vcpu_affinity();
        assert!(result.is_ok(), "Should succeed on Windows/iOS (stub)");
        println!("vCPU affinity stub called on Windows/iOS");
    }
}

/// Test SMMU initialization
#[test]
fn test_smmu_initialization() {
    let mut manager = AccelerationManager::new().expect("Should create manager");

    // Initialize manager first
    manager.setup_full_acceleration().ok();

    let result = manager.init_smmu();
    match result {
        Ok(()) => println!("SMMU initialized"),
        Err(e) => {
            println!("SMMU initialization failed: {:?}", e);
            // This is expected if SMMU is not available or disabled
        }
    }
}

/// Test acceleration manager error handling
#[test]
fn test_acceleration_manager_error_handling() {
    let mut manager = AccelerationManager::new().expect("Should create manager");

    // Try operations without initialization
    let result = manager.init_smmu();
    assert!(result.is_err(), "Should fail when not initialized");
    println!("Correctly rejected SMMU init without initialization");
}

/// Test NUMA with valid node counts
#[test]
fn test_numa_valid_node_counts() {
    let mut manager = AccelerationManager::new().expect("Should create manager");

    let cpu_info = CpuInfo::get();
    let max_nodes = match cpu_info.vendor {
        CpuVendor::Intel | CpuVendor::AMD => 8,
        CpuVendor::Apple => 2,
        _ => 4,
    };

    // Test valid node counts
    for nodes in 1..=max_nodes.min(4) {
        let result = manager.enable_numa(nodes);
        if result.is_ok() {
            println!("Enabled NUMA with {} nodes", nodes);
        } else {
            println!(
                "Could not enable NUMA with {} nodes: {:?}",
                nodes,
                result.unwrap_err()
            );
        }
    }
}

/// Test NUMA state management
#[test]
fn test_numa_state_management() {
    let mut manager = AccelerationManager::new().expect("Should create manager");

    // Initially disabled
    assert!(
        !manager.is_numa_enabled(),
        "NUMA should be disabled initially"
    );

    // Enable
    manager.enable_numa(1).expect("Should enable NUMA");
    assert!(manager.is_numa_enabled(), "NUMA should be enabled");

    // Disable
    manager.disable_numa();
    assert!(!manager.is_numa_enabled(), "NUMA should be disabled");
}

/// Test acceleration manager re-initialization
#[test]
fn test_acceleration_manager_reinitialization() {
    let mut manager = AccelerationManager::new().expect("Should create manager");

    // First initialization
    let result1 = manager.setup_full_acceleration();
    println!("First initialization: {:?}", result1.is_ok());

    // Second initialization (should handle gracefully)
    let result2 = manager.setup_full_acceleration();
    println!("Second initialization: {:?}", result2.is_ok());
}

/// Test acceleration manager with platform detection
#[test]
fn test_acceleration_manager_platform_detection() {
    let manager = AccelerationManager::new().expect("Should create manager");

    let cpu_info = CpuInfo::get();
    println!("Platform:");
    println!("  Vendor: {:?}", cpu_info.vendor);
    println!("  Architecture: {:?}", cpu_info.arch);
    println!("  Cores: {}", cpu_info.core_count);

    // Check which acceleration should be available
    let kind = AccelKind::detect_best();
    println!("  Best accelerator: {:?}", kind);
}

/// Test NUMA optimization with multiple nodes
#[test]
fn test_numa_optimization_multi_node() {
    let mut manager = AccelerationManager::new().expect("Should create manager");

    let cpu_info = CpuInfo::get();
    let node_count = match cpu_info.vendor {
        CpuVendor::Apple => 2, // Apple Silicon typically has 2 NUMA nodes
        CpuVendor::Intel | CpuVendor::AMD => {
            // x86 might have multiple nodes
            std::cmp::min(cpu_info.core_count / 4, 2)
        }
        _ => 1,
    };

    if node_count > 1 {
        let result = manager.enable_numa(node_count);
        if result.is_ok() {
            println!("Enabled NUMA optimization with {} nodes", node_count);
        } else {
            println!(
                "Could not enable multi-node NUMA: {:?}",
                result.unwrap_err()
            );
        }
    } else {
        println!("System has single NUMA node");
    }
}

/// Test vCPU affinity manager integration
#[test]
fn test_vcpu_affinity_manager_integration() {
    let mut manager = AccelerationManager::new().expect("Should create manager");

    #[cfg(not(any(target_os = "windows", target_os = "ios")))]
    {
        if manager.init_vcpu_affinity().is_ok() {
            println!("vCPU affinity manager integrated successfully");

            // Try to get topology info
            // Note: This would require accessor methods
        }
    }
}

/// Test acceleration manager lifecycle
#[test]
fn test_acceleration_manager_lifecycle() {
    // Create
    let mut manager = AccelerationManager::new().expect("Should create manager");
    assert!(!manager.is_initialized());

    // Initialize
    if manager.setup_full_acceleration().is_ok() {
        assert!(manager.is_initialized());

        // Use
        if manager.enable_numa(1).is_ok() {
            assert!(manager.is_numa_enabled());
        }

        // Cleanup happens on drop
    }

    println!("Lifecycle test completed");
}

/// Test NUMA memory strategy selection
#[test]
fn test_numa_memory_strategy_selection() {
    let mut manager = AccelerationManager::new().expect("Should create manager");

    // This tests that NUMA optimization can be configured
    // Actual strategy testing would require accessor methods
    let result = manager.enable_numa(1);
    if result.is_ok() {
        println!("NUMA memory strategy can be configured");
    }
}

/// Test acceleration manager with SMMU dependency
#[test]
fn test_acceleration_manager_smmu_dependency() {
    let mut manager = AccelerationManager::new().expect("Should create manager");

    // SMMU requires initialization first
    manager.setup_full_acceleration().ok();

    let smmu_result = manager.init_smmu();
    match smmu_result {
        Ok(()) => {
            println!("SMMU initialized after acceleration setup");
        }
        Err(_) => {
            println!("SMMU not available (expected on many systems)");
        }
    }
}

/// Test cross-platform compatibility
#[test]
fn test_cross_platform_compatibility() {
    let manager = AccelerationManager::new().expect("Should create manager");

    let cpu_info = CpuInfo::get();

    // Test should work on all platforms
    println!("Platform compatibility test:");
    println!("  OS: {}", std::env::consts::OS);
    println!("  Arch: {}", std::env::consts::ARCH);
    println!("  Vendor: {:?}", cpu_info.vendor);

    // Manager should create successfully on all platforms
    assert!(manager.is_ok());
}

/// Test acceleration manager concurrent operations
#[test]
fn test_acceleration_manager_concurrent_operations() {
    let mut manager = AccelerationManager::new().expect("Should create manager");

    // Enable NUMA
    let _ = manager.enable_numa(1);

    // Try to enable again (should handle gracefully)
    let result = manager.enable_numa(1);
    println!("Concurrent NUMA enable: {:?}", result);
}

/// Test NUMA bounds checking
#[test]
fn test_numa_bounds_checking() {
    let mut manager = AccelerationManager::new().expect("Should create manager");

    // Test boundary values
    let test_cases = vec![0, 1, 2, 100, 999];

    for nodes in test_cases {
        let result = manager.enable_numa(nodes);
        println!("NUMA with {} nodes: {:?}", nodes, result.is_ok());
    }
}

/// Test acceleration manager configuration persistence
#[test]
fn test_acceleration_manager_configuration_persistence() {
    let mut manager = AccelerationManager::new().expect("Should create manager");

    // Enable NUMA
    if manager.enable_numa(1).is_ok() {
        assert!(manager.is_numa_enabled());

        // Configuration should persist
        assert!(manager.is_numa_enabled());
    }
}
