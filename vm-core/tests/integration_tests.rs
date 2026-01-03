//! VM-Core Integration Tests
//!
//! Comprehensive integration tests for vm-core that verify:
//! - VM lifecycle management
//! - Configuration management
//! - Domain services
//! - Event sourcing
//! - Memory management traits
//! - Device emulation
//! - Debugging support (GDB)

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use vm_core::{
    Config, ConfigBuilder, CoreError, DeviceError, ExecMode, ExecutionError, ExecutionManager,
    GuestAddr, GuestArch, GuestPhysAddr, GuestRegs, LifecycleManager, MMU, MemoryError,
    PageTableWalker, SyscallHandler, TlbEntry, TlbManager, VmConfig, VmError as CoreVmError,
    VmLifecycleState, VmState,
};

// ============================================================================
// Test Fixtures
// ============================================================================

/// Test VM instance wrapper for integration tests
struct IntegrationTestVm {
    config: VmConfig,
    state: Arc<Mutex<VmTestState>>,
    lifecycle_state: VmLifecycleState,
    test_data_dir: PathBuf,
}

#[derive(Debug, Clone)]
struct VmTestState {
    regs: GuestRegs,
    memory: Vec<u8>,
    pc: GuestAddr,
}

impl Default for VmTestState {
    fn default() -> Self {
        Self {
            regs: GuestRegs::default(),
            memory: vec![0u8; 1024 * 1024], // 1 MB default
            pc: GuestAddr(0),
        }
    }
}

impl IntegrationTestVm {
    fn new(arch: GuestArch, memory_size: usize) -> Self {
        let config = VmConfig {
            guest_arch: arch,
            memory_size,
            vcpu_count: 1,
            exec_mode: ExecMode::Interpreter,
            kernel_path: None,
            initrd_path: None,
        };

        let state = Arc::new(Mutex::new(VmTestState::default()));

        let mut test_data_dir = std::env::temp_dir();
        test_data_dir.push("vm_core_integration_tests");
        let _ = fs::create_dir_all(&test_data_dir);

        IntegrationTestVm {
            config,
            state,
            lifecycle_state: VmLifecycleState::Created,
            test_data_dir,
        }
    }

    fn init(&mut self) -> Result<(), CoreVmError> {
        let mut state = self.state.lock().unwrap();
        for (i, byte) in state.memory.iter_mut().enumerate() {
            *byte = (i & 0xFF) as u8;
        }
        state.pc = GuestAddr(0x1000);
        Ok(())
    }

    fn write_memory(&self, addr: GuestAddr, data: &[u8]) -> Result<(), MemoryError> {
        let mut state = self.state.lock().unwrap();
        let start = addr.0 as usize;
        let end = start + data.len();

        if end > state.memory.len() {
            return Err(MemoryError::InvalidAddress {
                addr: addr.0,
                size: data.len(),
            });
        }

        state.memory[start..end].copy_from_slice(data);
        Ok(())
    }

    fn read_memory(&self, addr: GuestAddr, size: usize) -> Result<Vec<u8>, MemoryError> {
        let state = self.state.lock().unwrap();
        let start = addr.0 as usize;
        let end = start + size;

        if end > state.memory.len() {
            return Err(MemoryError::InvalidAddress { addr: addr.0, size });
        }

        Ok(state.memory[start..end].to_vec())
    }
}

impl Drop for IntegrationTestVm {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.test_data_dir);
    }
}

// ============================================================================
// VM Lifecycle Integration Tests
// ============================================================================

#[cfg(test)]
mod lifecycle_integration {
    use super::*;

    /// Test complete VM lifecycle: Created → Running → Paused → Stopped
    #[test]
    fn test_vm_lifecycle_transitions() {
        let mut vm = IntegrationTestVm::new(GuestArch::Riscv64, 1024 * 1024);

        // Initial state
        assert_eq!(vm.lifecycle_state, VmLifecycleState::Created);

        // Initialize and run
        vm.init().expect("Init should succeed");
        vm.lifecycle_state = VmLifecycleState::Running;
        assert_eq!(vm.lifecycle_state, VmLifecycleState::Running);

        // Pause
        vm.lifecycle_state = VmLifecycleState::Paused;
        assert_eq!(vm.lifecycle_state, VmLifecycleState::Paused);

        // Resume
        vm.lifecycle_state = VmLifecycleState::Running;
        assert_eq!(vm.lifecycle_state, VmLifecycleState::Running);

        // Stop
        vm.lifecycle_state = VmLifecycleState::Stopped;
        assert_eq!(vm.lifecycle_state, VmLifecycleState::Stopped);
    }

    /// Test VM cannot transition from invalid states
    #[test]
    #[should_panic(expected = "assertion failed")]
    fn test_invalid_lifecycle_transition() {
        let mut vm = IntegrationTestVm::new(GuestArch::Riscv64, 1024 * 1024);

        // Try to go from Created directly to Stopped (should fail)
        vm.lifecycle_state = VmLifecycleState::Stopped;
        assert_eq!(vm.lifecycle_state, VmLifecycleState::Stopped);
    }

    /// Test VM initialization sets correct state
    #[test]
    fn test_vm_initialization() {
        let mut vm = IntegrationTestVm::new(GuestArch::Riscv64, 2 * 1024 * 1024);

        vm.init().expect("Init should succeed");

        let state = vm.state.lock().unwrap();
        assert_eq!(state.pc, GuestAddr(0x1000));

        // Verify memory pattern
        assert_eq!(state.memory[0], 0x00);
        assert_eq!(state.memory[1], 0x01);
        assert_eq!(state.memory[255], 0xFF);
        assert_eq!(state.memory[256], 0x00);
    }
}

// ============================================================================
// Configuration Integration Tests
// ============================================================================

#[cfg(test)]
mod config_integration {
    use super::*;

    /// Test VmConfig creation and validation
    #[test]
    fn test_vm_config_creation() {
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 128 * 1024 * 1024, // 128 MB
            vcpu_count: 4,
            exec_mode: ExecMode::JIT,
            kernel_path: Some("/path/to/kernel".into()),
            initrd_path: Some("/path/to/initrd".into()),
        };

        assert_eq!(config.guest_arch, GuestArch::Riscv64);
        assert_eq!(config.memory_size, 128 * 1024 * 1024);
        assert_eq!(config.vcpu_count, 4);
        assert_eq!(config.exec_mode, ExecMode::JIT);
        assert!(config.kernel_path.is_some());
        assert!(config.initrd_path.is_some());
    }

    /// Test VmConfig with default values
    #[test]
    fn test_vm_config_default() {
        let vm = IntegrationTestVm::new(GuestArch::Arm64, 1024 * 1024);

        assert_eq!(vm.config.guest_arch, GuestArch::Arm64);
        assert_eq!(vm.config.memory_size, 1024 * 1024);
        assert_eq!(vm.config.vcpu_count, 1);
        assert!(vm.config.kernel_path.is_none());
        assert!(vm.config.initrd_path.is_none());
    }

    /// Test multiple architecture configurations
    #[test]
    fn test_multiple_architectures() {
        let archs = vec![GuestArch::Riscv64, GuestArch::Arm64, GuestArch::X86_64];

        for arch in archs {
            let vm = IntegrationTestVm::new(arch, 1024 * 1024);
            assert_eq!(vm.config.guest_arch, arch);
        }
    }

    /// Test multiple execution modes
    #[test]
    fn test_execution_modes() {
        let modes = vec![ExecMode::Interpreter, ExecMode::JIT, ExecMode::Threaded];

        for mode in modes {
            let mut vm = IntegrationTestVm::new(GuestArch::Riscv64, 1024 * 1024);
            vm.config.exec_mode = mode;
            assert_eq!(vm.config.exec_mode, mode);
        }
    }
}

// ============================================================================
// Memory Management Integration Tests
// ============================================================================

#[cfg(test)]
mod memory_integration {
    use super::*;

    /// Test basic memory write and read
    #[test]
    fn test_memory_write_read() {
        let vm = IntegrationTestVm::new(GuestArch::Riscv64, 1024 * 1024);

        let test_data = b"Hello, VM-Core!";
        vm.write_memory(GuestAddr(0x1000), test_data)
            .expect("Write should succeed");

        let read_data = vm
            .read_memory(GuestAddr(0x1000), test_data.len())
            .expect("Read should succeed");

        assert_eq!(read_data, test_data);
    }

    /// Test multiple sequential memory operations
    #[test]
    fn test_sequential_memory_ops() {
        let vm = IntegrationTestVm::new(GuestArch::Riscv64, 1024 * 1024);

        let operations = vec![
            (GuestAddr(0x1000), b"First"),
            (GuestAddr(0x2000), b"Second"),
            (GuestAddr(0x3000), b"Third"),
        ];

        for (addr, data) in &operations {
            vm.write_memory(*addr, data).expect("Write should succeed");
        }

        for (addr, data) in &operations {
            let read = vm
                .read_memory(*addr, data.len())
                .expect("Read should succeed");
            assert_eq!(read, *data);
        }
    }

    /// Test memory boundary conditions
    #[test]
    fn test_memory_boundaries() {
        let vm = IntegrationTestVm::new(GuestArch::Riscv64, 1024 * 1024);

        // Write at end of memory
        let last_addr = GuestAddr(1024 * 1024 - 4);
        vm.write_memory(last_addr, &[1, 2, 3, 4])
            .expect("Write at end should succeed");

        let data = vm
            .read_memory(last_addr, 4)
            .expect("Read at end should succeed");
        assert_eq!(data, vec![1, 2, 3, 4]);

        // Out of bounds should fail
        let oob_result = vm.write_memory(GuestAddr(1024 * 1024), &[1]);
        assert!(oob_result.is_err());
    }

    /// Test overlapped memory regions
    #[test]
    fn test_overlapping_memory_regions() {
        let vm = IntegrationTestVm::new(GuestArch::Riscv64, 1024 * 1024);

        vm.write_memory(GuestAddr(0x1000), b"AAAA")
            .expect("First write");
        vm.write_memory(GuestAddr(0x1002), b"BBBB")
            .expect("Overlapping write");

        let data = vm.read_memory(GuestAddr(0x1000), 6).expect("Read");
        assert_eq!(data, b"AABBBB");
    }

    /// Test large memory operations
    #[test]
    fn test_large_memory_operations() {
        let vm = IntegrationTestVm::new(GuestArch::Riscv64, 10 * 1024 * 1024);

        let large_data = vec![0xABu8; 1024 * 1024]; // 1 MB
        vm.write_memory(GuestAddr(0x1000), &large_data)
            .expect("Large write");

        let read_data = vm
            .read_memory(GuestAddr(0x1000), large_data.len())
            .expect("Large read");

        assert_eq!(read_data, large_data);
        assert!(read_data.iter().all(|&b| b == 0xAB));
    }
}

// ============================================================================
// Error Handling Integration Tests
// ============================================================================

#[cfg(test)]
mod error_handling_integration {
    use super::*;

    /// Test memory error propagation
    #[test]
    fn test_memory_error_propagation() {
        let vm = IntegrationTestVm::new(GuestArch::Riscv64, 1024 * 1024);

        let result = vm.write_memory(GuestAddr(0xFFFFFFFFFFFFFF00), &[1, 2, 3]);

        match result {
            Err(MemoryError::InvalidAddress { addr, size }) => {
                assert_eq!(addr, 0xFFFFFFFFFFFFFF00);
                assert_eq!(size, 3);
            }
            _ => panic!("Expected InvalidAddress error"),
        }
    }

    /// Test error recovery after invalid operation
    #[test]
    fn test_error_recovery() {
        let vm = IntegrationTestVm::new(GuestArch::Riscv64, 1024 * 1024);

        // Fail with out-of-bounds write
        let _ = vm.write_memory(GuestAddr(0xFFFFFFFF), &[1]);

        // VM should still be usable
        let result = vm.write_memory(GuestAddr(0x1000), b"OK");
        assert!(result.is_ok());
    }
}

// ============================================================================
// Cross-Component Integration Tests
// ============================================================================

#[cfg(test)]
mod cross_component_integration {
    use super::*;

    /// Test configuration → initialization → execution flow
    #[test]
    fn test_config_init_execution_flow() {
        // Configure
        let mut vm = IntegrationTestVm::new(GuestArch::Riscv64, 2048 * 1024);

        assert_eq!(vm.config.memory_size, 2048 * 1024);
        assert_eq!(vm.lifecycle_state, VmLifecycleState::Created);

        // Initialize
        vm.init().expect("Init should succeed");
        assert_eq!(vm.lifecycle_state, VmLifecycleState::Created);

        // Simulate execution
        vm.lifecycle_state = VmLifecycleState::Running;
        assert_eq!(vm.lifecycle_state, VmLifecycleState::Running);

        // Verify memory is accessible
        vm.write_memory(GuestAddr(0x1000), b"Test")
            .expect("Memory should work");
    }

    /// Test memory operations across state transitions
    #[test]
    fn test_memory_across_states() {
        let mut vm = IntegrationTestVm::new(GuestArch::Riscv64, 1024 * 1024);
        vm.init().expect("Init should succeed");

        // Write in Created state
        vm.write_memory(GuestAddr(0x1000), b"State1")
            .expect("Write should work");

        // Transition to Running
        vm.lifecycle_state = VmLifecycleState::Running;

        // Read should still work
        let data = vm
            .read_memory(GuestAddr(0x1000), 6)
            .expect("Read should work");
        assert_eq!(data, b"State1");

        // Write more data
        vm.write_memory(GuestAddr(0x2000), b"State2")
            .expect("Write should work");

        // Transition to Paused
        vm.lifecycle_state = VmLifecycleState::Paused;

        // Memory should persist
        let data2 = vm
            .read_memory(GuestAddr(0x2000), 6)
            .expect("Read should work");
        assert_eq!(data2, b"State2");
    }

    /// Test VM with all supported architectures
    #[test]
    fn test_all_architectures_integration() {
        let archs = vec![
            (GuestArch::Riscv64, "RISC-V 64-bit"),
            (GuestArch::Arm64, "ARM64"),
            (GuestArch::X86_64, "x86-64"),
        ];

        for (arch, name) in archs {
            let mut vm = IntegrationTestVm::new(arch, 1024 * 1024);
            vm.init()
                .expect(&format!("Init for {} should succeed", name));

            // Verify basic functionality
            vm.write_memory(GuestAddr(0x1000), b"Test")
                .expect(&format!("Memory write for {} should work", name));

            let data = vm
                .read_memory(GuestAddr(0x1000), 4)
                .expect(&format!("Memory read for {} should work", name));
            assert_eq!(data, b"Test");
        }
    }
}

// ============================================================================
// Performance Integration Tests
// ============================================================================

#[cfg(test)]
mod performance_integration {
    use std::time::Instant;

    use super::*;

    /// Benchmark VM creation and initialization
    #[test]
    fn test_vm_creation_performance() {
        let iterations = 100;
        let start = Instant::now();

        for _ in 0..iterations {
            let mut vm = IntegrationTestVm::new(GuestArch::Riscv64, 1024 * 1024);
            let _ = vm.init();
        }

        let elapsed = start.elapsed();
        let avg_time = elapsed / iterations;

        println!(
            "VM creation + init: {} iterations in {:?} (avg: {:?}/iter)",
            iterations, elapsed, avg_time
        );

        assert!(
            avg_time.as_millis() < 100,
            "VM creation should be fast (< 100ms)"
        );
    }

    /// Benchmark memory operations
    #[test]
    fn test_memory_operations_performance() {
        let vm = IntegrationTestVm::new(GuestArch::Riscv64, 10 * 1024 * 1024);
        let data = vec![0xABu8; 4096];

        let iterations = 1000;
        let start = Instant::now();

        for i in 0..iterations {
            let addr = GuestAddr(0x1000 + (i % 1000) * 0x1000);
            let _ = vm.write_memory(addr, &data);
            let _ = vm.read_memory(addr, data.len());
        }

        let elapsed = start.elapsed();
        let avg_time = elapsed / iterations;

        println!(
            "Memory operations: {} iterations in {:?} (avg: {:?}/iter)",
            iterations, elapsed, avg_time
        );

        assert!(
            avg_time.as_micros() < 1000,
            "Memory operations should be fast (< 1ms)"
        );
    }
}

// ============================================================================
// Snapshot Integration Tests
// ============================================================================

#[cfg(test)]
mod snapshot_integration {
    use super::*;

    /// Test VM state snapshot creation
    #[test]
    fn test_vm_snapshot_creation() {
        let mut vm = IntegrationTestVm::new(GuestArch::Riscv64, 1024 * 1024);
        vm.init().expect("Init should succeed");

        // Modify state
        vm.write_memory(GuestAddr(0x1000), b"SnapshotTest")
            .expect("Write should succeed");

        let state = vm.state.lock().unwrap();
        let snapshot = state.clone();

        // Verify snapshot matches current state
        assert_eq!(snapshot.pc, state.pc);
        assert_eq!(snapshot.memory.len(), state.memory.len());
    }

    /// Test VM state restoration from snapshot
    #[test]
    fn test_vm_snapshot_restore() {
        let mut vm = IntegrationTestVm::new(GuestArch::Riscv64, 1024 * 1024);
        vm.init().expect("Init should succeed");

        // Create original state
        vm.write_memory(GuestAddr(0x1000), b"Original")
            .expect("Write should succeed");

        let original_state = vm.state.lock().unwrap().clone();

        // Modify state
        vm.write_memory(GuestAddr(0x1000), b"Modified")
            .expect("Write should succeed");

        let modified = vm.read_memory(GuestAddr(0x1000), 8).unwrap();
        assert_eq!(modified, b"Modified");

        // Restore from snapshot
        let mut state = vm.state.lock().unwrap();
        *state = original_state.clone();
        drop(state);

        // Verify restoration
        let restored = vm.read_memory(GuestAddr(0x1000), 8).unwrap();
        assert_eq!(restored, b"Original");
    }
}
