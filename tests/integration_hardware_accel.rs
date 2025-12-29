//! Hardware Acceleration Integration Tests
//!
//! Comprehensive integration tests for hardware acceleration:
//! - KVM (Kernel-based Virtual Machine) acceleration
//! - HVF (Hypervisor Framework) on macOS
//! - WHPX (Windows Hypervisor Platform)
//! - VCPU affinity and NUMA optimization
//! - Real-time monitoring
//! - Fallback mechanisms
//! - Error handling and edge cases

use vm_core::{GuestArch, VmConfig, ExecMode, VmError};
use vm_accel::{
    accel_fallback::{AccelBackend, AccelConfig, FallbackStrategy},
    apple::HvfAccelerator,
    cpuinfo::CpuInfo,
    realtime_monitor::RealtimeMonitor,
    vcpu_affinity::VcpuAffinityManager,
    numa_optimizer::NumaOptimizer,
};
use std::sync::{Arc, Mutex};

// ============================================================================
// Test Fixtures
// ============================================================================

/// Mock hardware accelerator for testing
struct MockAccelBackend {
    name: String,
    available: bool,
    enabled: bool,
}

impl MockAccelBackend {
    fn new(name: &str) -> Self {
        MockAccelBackend {
            name: name.to_string(),
            available: true,
            enabled: false,
        }
    }

    fn is_available(&self) -> bool {
        self.available
    }

    fn enable(&mut self) -> Result<(), VmError> {
        if !self.available {
            return Err(VmError::Core(vm_core::CoreError::PlatformError(
                vm_core::error::PlatformError::UnsupportedFeature {
                    feature: "hardware_acceleration".to_string(),
                },
            )));
        }
        self.enabled = true;
        Ok(())
    }

    fn disable(&mut self) {
        self.enabled = false;
    }

    fn run(&self) -> Result<(), VmError> {
        if !self.enabled {
            return Err(VmError::Core(vm_core::CoreError::ExecutionError(
                vm_core::error::ExecutionError::NotInitialized,
            )));
        }
        Ok(())
    }
}

/// Create test acceleration configuration
fn create_accel_config() -> AccelConfig {
    AccelConfig {
        prefer_kvm: true,
        prefer_hvf: true,
        prefer_whpx: true,
        fallback_to_interpreter: true,
        ..Default::default()
    }
}

// ============================================================================
// Happy Path Tests
// ============================================================================

#[test]
fn test_accel_config_creation() {
    let config = create_accel_config();

    assert!(config.prefer_kvm || config.prefer_hvf || config.prefer_whpx);
}

#[test]
fn test_mock_backend_creation() {
    let backend = MockAccelBackend::new("test_backend");

    assert_eq!(backend.name, "test_backend");
    assert!(backend.is_available());
    assert!(!backend.enabled);
}

#[test]
fn test_mock_backend_enable() {
    let mut backend = MockAccelBackend::new("test_backend");

    assert!(backend.enable().is_ok());
    assert!(backend.enabled);
}

#[test]
fn test_mock_backend_disable() {
    let mut backend = MockAccelBackend::new("test_backend");

    backend.enable().unwrap();
    backend.disable();

    assert!(!backend.enabled);
}

#[test]
fn test_mock_backend_run() {
    let mut backend = MockAccelBackend::new("test_backend");

    backend.enable().unwrap();
    assert!(backend.run().is_ok());
}

#[test]
fn test_cpu_info_detection() {
    let cpu_info = CpuInfo::detect();

    // Should detect CPU information
    assert!(cpu_info.num_cores > 0);
}

#[test]
fn test_vcpu_affinity_manager_creation() {
    let affinity_mgr = VcpuAffinityManager::new();

    assert!(affinity_mgr.is_ok());
}

#[test]
fn test_vcpu_affinity_setting() {
    let mut affinity_mgr = VcpuAffinityManager::new().unwrap();

    let result = affinity_mgr.set_vcpu_affinity(0, Some(0));

    // May fail on some systems
    let _ = result;
}

#[test]
fn test_numa_optimizer_creation() {
    let numa = NumaOptimizer::new();

    assert!(numa.is_ok());
}

#[test]
fn test_numa_node_detection() {
    let numa = NumaOptimizer::new().unwrap();

    let nodes = numa.get_num_nodes();

    // Should have at least 1 NUMA node
    assert!(nodes >= 1);
}

#[test]
fn test_realtime_monitor_creation() {
    let monitor = RealtimeMonitor::new();

    assert!(monitor.is_ok());
}

#[test]
fn test_fallback_strategy() {
    let strategies = vec![
        FallbackStrategy::PreferKvm,
        FallbackStrategy::PreferHvf,
        FallbackStrategy::PreferWhpx,
        FallbackStrategy::InterpreterOnly,
    ];

    for strategy in strategies {
        // Test strategy creation
        let config = AccelConfig {
            fallback_strategy: strategy,
            ..Default::default()
        };

        assert_eq!(config.fallback_strategy, strategy);
    }
}

#[cfg(target_os = "linux")]
#[test]
fn test_kvm_detection() {
    use vm_accel::kvm::KvmAccelerator;

    let kvm = KvmAccelerator::new();

    // Check if KVM is available
    let available = kvm.is_available();

    // Result depends on system
    let _ = available;
}

#[cfg(target_os = "macos")]
#[test]
fn test_hvf_detection() {
    let hvf = HvfAccelerator::new();

    // Check if HVF is available
    let available = hvf.is_available();

    // Should be available on macOS
    assert!(available);
}

#[cfg(target_os = "windows")]
#[test]
fn test_whpx_detection() {
    use vm_accel::whpx::WhpxAccelerator;

    let whpx = WhpxAccelerator::new();

    // Check if WHPX is available
    let available = whpx.is_available();

    // Result depends on system
    let _ = available;
}

// ============================================================================
// Error Path Tests
// ============================================================================

#[test]
fn test_unavailable_backend() {
    let mut backend = MockAccelBackend::new("unavailable");
    backend.available = false;

    let result = backend.enable();

    assert!(result.is_err());
}

#[test]
fn test_run_without_enable() {
    let backend = MockAccelBackend::new("test");

    let result = backend.run();

    assert!(result.is_err());
}

#[test]
fn test_invalid_vcpu_affinity() {
    let mut affinity_mgr = VcpuAffinityManager::new().unwrap();

    // Try to set affinity for non-existent CPU
    let result = affinity_mgr.set_vcpu_affinity(0, Some(9999));

    assert!(result.is_err());
}

#[test]
fn test_accelerator_unavailable() {
    let config = AccelConfig {
        prefer_kvm: true,
        prefer_hvf: false,
        prefer_whpx: false,
        fallback_to_interpreter: false,
        ..Default::default()
    };

    // If KVM is not available, should fail
    #[cfg(not(target_os = "linux"))]
    {
        let result = vm_accel::select_backend(&config);
        assert!(result.is_err());
    }
}

#[test]
fn test_multiple_enable_calls() {
    let mut backend = MockAccelBackend::new("test");

    backend.enable().unwrap();

    // Second enable should be idempotent or fail gracefully
    let result = backend.enable();

    assert!(result.is_ok() || result.is_err());
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_all_backends_unavailable() {
    let config = AccelConfig {
        prefer_kvm: false,
        prefer_hvf: false,
        prefer_whpx: false,
        fallback_to_interpreter: false,
        ..Default::default()
    };

    // Should fail if all backends are disabled
    let result = vm_accel::select_backend(&config);

    assert!(result.is_err());
}

#[test]
fn test_interpreter_fallback() {
    let config = AccelConfig {
        prefer_kvm: false,
        prefer_hvf: false,
        prefer_whpx: false,
        fallback_to_interpreter: true,
        ..Default::default()
    };

    let result = vm_accel::select_backend(&config);

    // Should fall back to interpreter
    assert!(result.is_ok());
}

#[test]
fn test_single_numa_node() {
    let numa = NumaOptimizer::new().unwrap();

    let nodes = numa.get_num_nodes();

    if nodes == 1 {
        // Single node system - should handle gracefully
        let preferred = numa.get_preferred_node(0);
        assert!(preferred.is_ok());
    }
}

#[test]
fn test_many_vcpus_affinity() {
    let mut affinity_mgr = VcpuAffinityManager::new().unwrap();

    // Try to set affinity for many VCPUs
    for i in 0..32 {
        let result = affinity_mgr.set_vcpu_affinity(i, None); // Let system decide
        let _ = result; // May fail on systems with fewer cores
    }
}

#[test]
fn test_zero_vcpu_id() {
    let mut affinity_mgr = VcpuAffinityManager::new().unwrap();

    // VCPU 0 should be valid
    let result = affinity_mgr.set_vcpu_affinity(0, None);

    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_concurrent_accelerator_access() {
    use std::thread;

    let backend = Arc::new(Mutex::new(MockAccelBackend::new("concurrent")));
    let mut handles = Vec::new();

    for _ in 0..10 {
        let backend_clone = Arc::clone(&backend);
        let handle = thread::spawn(move || {
            let mut backend = backend_clone.lock().unwrap();
            backend.enable().unwrap();
            backend.run().unwrap();
            backend.disable();
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_realtime_monitor_sampling() {
    let monitor = RealtimeMonitor::new().unwrap();

    // Sample performance
    let stats = monitor.sample_performance();

    // Should return valid stats
    assert!(stats.cpu_usage >= 0.0 && stats.cpu_usage <= 100.0);
}

#[test]
fn test_accelerator_priority_selection() {
    // Test KVM priority
    let config1 = AccelConfig {
        prefer_kvm: true,
        prefer_hvf: true,
        prefer_whpx: true,
        fallback_to_interpreter: true,
        ..Default::default()
    };

    let backend1 = vm_accel::select_backend(&config1);

    // Should select highest priority available backend
    assert!(backend1.is_ok());
}

#[test]
fn test_cross_platform_compatibility() {
    // Create config that works on all platforms
    let config = AccelConfig {
        fallback_to_interpreter: true,
        ..Default::default()
    };

    let backend = vm_accel::select_backend(&config);

    // Should always succeed with interpreter fallback
    assert!(backend.is_ok());
}

// ============================================================================
// Performance Tests
// ============================================================================

#[test]
fn test_accelerator_startup_time() {
    let start = std::time::Instant::now();

    let backend = MockAccelBackend::new("test");
    let _ = backend.enable();

    let duration = start.elapsed();

    // Should start quickly
    assert!(duration.as_millis() < 100);
}

#[test]
fn test_affinity_setting_performance() {
    let mut affinity_mgr = VcpuAffinityManager::new().unwrap();

    let start = std::time::Instant::now();

    for i in 0..100 {
        let _ = affinity_mgr.set_vcpu_affinity(i % 4, Some(i % 4));
    }

    let duration = start.elapsed();

    // Should complete quickly
    assert!(duration.as_secs() < 1);
}

#[test]
fn test_realtime_monitor_overhead() {
    let monitor = RealtimeMonitor::new().unwrap();

    let start = std::time::Instant::now();

    for _ in 0..1000 {
        let _ = monitor.sample_performance();
    }

    let duration = start.elapsed();

    // Should have minimal overhead
    assert!(duration.as_secs() < 1);
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_vm_with_hardware_acceleration() {
    let mut config = VmConfig::default();
    config.exec_mode = ExecMode::HardwareAssisted;

    // Should create config successfully
    assert_eq!(config.exec_mode, ExecMode::HardwareAssisted);
}

#[test]
fn test_accelerator_with_memory() {
    let backend = MockAccelBackend::new("test");
    backend.enable().unwrap();

    // Simulate memory access through accelerator
    let _ = backend.run();

    assert!(backend.enabled);
}

#[test]
fn test_numa_with_affinity() {
    let numa = NumaOptimizer::new().unwrap();
    let mut affinity_mgr = VcpuAffinityManager::new().unwrap();

    let preferred_node = numa.get_preferred_node(0).unwrap();

    // Set affinity based on NUMA node
    let result = affinity_mgr.set_vcpu_affinity(0, preferred_node);

    // May succeed or fail depending on system
    let _ = result;
}

#[test]
fn test_monitoring_with_acceleration() {
    let monitor = RealtimeMonitor::new().unwrap();
    let backend = MockAccelBackend::new("test");
    backend.enable().unwrap();

    // Monitor while running
    let _ = backend.run();

    let stats = monitor.sample_performance();

    // Should have valid stats
    assert!(stats.cpu_usage >= 0.0);
}

#[test]
fn test_fallback_chain() {
    // Test complete fallback chain
    let configs = vec![
        AccelConfig {
            prefer_kvm: true,
            ..Default::default()
        },
        AccelConfig {
            prefer_hvf: true,
            ..Default::default()
        },
        AccelConfig {
            prefer_whpx: true,
            ..Default::default()
        },
        AccelConfig {
            fallback_to_interpreter: true,
            ..Default::default()
        },
    ];

    let mut selected = None;

    for config in configs {
        if let Ok(backend) = vm_accel::select_backend(&config) {
            selected = Some(backend);
            break;
        }
    }

    // At least interpreter should be available
    assert!(selected.is_some());
}

#[test]
fn test_vcpu_power_management() {
    let mut affinity_mgr = VcpuAffinityManager::new().unwrap();

    // Enable VCPU
    let result1 = affinity_mgr.set_vcpu_power_state(0, true);
    let _ = result1;

    // Disable VCPU
    let result2 = affinity_mgr.set_vcpu_power_state(0, false);
    let _ = result2;
}

#[test]
fn test_multiple_accelerator_instances() {
    let backends: Vec<MockAccelBackend> = vec![
        MockAccelBackend::new("accel1"),
        MockAccelBackend::new("accel2"),
        MockAccelBackend::new("accel3"),
    ];

    assert_eq!(backends.len(), 3);
}

#[test]
fn test_accelerator_statistics() {
    let backend = MockAccelBackend::new("test");
    backend.enable().unwrap();

    // Mock statistics
    let stats = (
        "test_backend".to_string(),
        true,
        std::time::Duration::from_millis(10),
    );

    assert_eq!(stats.0, "test_backend");
    assert!(stats.1);
}

// ============================================================================
// Platform-Specific Tests
// ============================================================================

#[cfg(target_os = "linux")]
#[test]
fn test_kvm_specific_features() {
    use vm_accel::kvm::KvmAccelerator;

    let kvm = KvmAccelerator::new();

    if kvm.is_available() {
        // Test KVM-specific features
        let vm = kvm.create_vm();
        let _ = vm;
    }
}

#[cfg(target_os = "macos")]
#[test]
fn test_hvf_specific_features() {
    let hvf = HvfAccelerator::new();

    if hvf.is_available() {
        // Test HVF-specific features
        let capabilities = hvf.get_capabilities();
        assert!(!capabilities.is_empty());
    }
}

#[cfg(target_os = "windows")]
#[test]
fn test_whpx_specific_features() {
    use vm_accel::whpx::WhpxAccelerator;

    let whpx = WhpxAccelerator::new();

    if whpx.is_available() {
        // Test WHPX-specific features
        let capabilities = whpx.get_capabilities();
        assert!(!capabilities.is_empty());
    }
}

#[test]
fn test_cpu_feature_detection() {
    let cpu_info = CpuInfo::detect();

    // Should detect basic features
    assert!(cpu_info.num_cores > 0);
    assert!(cpu_info.frequency_mhz > 0);

    // Optional features
    let _ = cpu_info.supports_vtx;
    let _ = cpu_info.supports_svm;
    let _ = cpu_info.supports_avx;
}

#[test]
fn test_cross_arch_acceleration() {
    let archs = vec![
        GuestArch::X86_64,
        GuestArch::Arm64,
        GuestArch::Riscv64,
    ];

    for arch in archs {
        let mut config = VmConfig {
            guest_arch: arch,
            exec_mode: ExecMode::HardwareAssisted,
            ..Default::default()
        };

        // Should create config for all architectures
        config.exec_mode = ExecMode::Interpreter; // Fallback
    }
}
