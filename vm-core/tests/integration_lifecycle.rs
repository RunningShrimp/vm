//! VM Lifecycle Integration Tests
//!
//! Comprehensive integration tests for VM lifecycle management:
//! - VM creation and initialization
//! - Boot process
//! - Running state
//! - Pause/resume functionality
//! - Stop and cleanup
//! - Snapshot creation and restoration
//! - Error paths and edge cases

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use vm_core::{
    ExecMode, GuestAddr, GuestArch, GuestRegs, VmConfig, VmError as CoreVmError, VmLifecycleState,
    VmState,
};

// ============================================================================
// Test Fixtures
// ============================================================================

/// Test VM instance wrapper
struct TestVm {
    config: VmConfig,
    state: Arc<Mutex<VmState>>,
    lifecycle_state: VmLifecycleState,
    test_data_dir: PathBuf,
}

impl TestVm {
    /// Create a new test VM instance
    fn new(arch: GuestArch, memory_size: usize) -> Self {
        let config = VmConfig {
            guest_arch: arch,
            memory_size,
            vcpu_count: 1,
            exec_mode: ExecMode::Interpreter,
            kernel_path: None,
            initrd_path: None,
        };

        let state = Arc::new(Mutex::new(VmState {
            regs: GuestRegs::default(),
            memory: vec![0u8; memory_size],
            pc: GuestAddr(0),
        }));

        let mut test_data_dir = std::env::temp_dir();
        test_data_dir.push("vm_lifecycle_tests");
        let _ = fs::create_dir_all(&test_data_dir);

        TestVm {
            config,
            state,
            lifecycle_state: VmLifecycleState::Created,
            test_data_dir,
        }
    }

    /// Initialize VM state
    fn init(&mut self) -> Result<(), CoreVmError> {
        // Initialize memory with test pattern
        let mut state = self.state.lock().unwrap();
        for (i, byte) in state.memory.iter_mut().enumerate() {
            *byte = (i & 0xFF) as u8;
        }
        state.pc = GuestAddr(0x1000);
        Ok(())
    }

    /// Simulate boot process
    fn boot(&mut self) -> Result<(), CoreVmError> {
        if self.lifecycle_state != VmLifecycleState::Created {
            return Err(CoreVmError::ExecutionError(ExecutionError::InvalidState {
                expected: "Created".to_string(),
                actual: format!("{:?}", self.lifecycle_state),
            }));
        }

        self.init()?;
        self.lifecycle_state = VmLifecycleState::Running;
        Ok(())
    }

    /// Pause VM execution
    fn pause(&mut self) -> Result<(), CoreVmError> {
        if self.lifecycle_state != VmLifecycleState::Running {
            return Err(CoreVmError::ExecutionError(ExecutionError::InvalidState {
                expected: "Running".to_string(),
                actual: format!("{:?}", self.lifecycle_state),
            }));
        }

        self.lifecycle_state = VmLifecycleState::Paused;
        Ok(())
    }

    /// Resume VM execution
    fn resume(&mut self) -> Result<(), CoreVmError> {
        if self.lifecycle_state != VmLifecycleState::Paused {
            return Err(CoreVmError::ExecutionError(ExecutionError::InvalidState {
                expected: "Paused".to_string(),
                actual: format!("{:?}", self.lifecycle_state),
            }));
        }

        self.lifecycle_state = VmLifecycleState::Running;
        Ok(())
    }

    /// Stop VM execution
    fn stop(&mut self) -> Result<(), CoreVmError> {
        if self.lifecycle_state == VmLifecycleState::Stopped {
            return Err(CoreVmError::ExecutionError(ExecutionError::InvalidState {
                expected: "!Stopped".to_string(),
                actual: format!("{:?}", self.lifecycle_state),
            }));
        }

        self.lifecycle_state = VmLifecycleState::Stopped;
        Ok(())
    }

    /// Create a snapshot
    fn create_snapshot(&self, name: &str) -> Result<PathBuf, CoreVmError> {
        let state = self.state.lock().unwrap();
        let snapshot_path = self.test_data_dir.join(format!("{}.snap", name));

        // Serialize state
        let serialized =
            bincode::encode_to_vec(&*state, bincode::config::standard()).map_err(|e| {
                CoreVmError::ExecutionError(ExecutionError::Other {
                    msg: format!("Failed to serialize snapshot: {}", e),
                })
            })?;

        fs::write(&snapshot_path, serialized).map_err(|e| {
            CoreVmError::ExecutionError(ExecutionError::Other {
                msg: format!("Failed to write snapshot: {}", e),
            })
        })?;

        Ok(snapshot_path)
    }

    /// Restore from a snapshot
    fn restore_snapshot(&mut self, path: &PathBuf) -> Result<(), CoreVmError> {
        let data = fs::read(path).map_err(|e| {
            CoreVmError::ExecutionError(ExecutionError::Other {
                msg: format!("Failed to read snapshot: {}", e),
            })
        })?;

        let restored: VmState = bincode::decode_from_slice(&data, bincode::config::standard())
            .map_err(|(e, _)| {
                CoreVmError::ExecutionError(ExecutionError::Other {
                    msg: format!("Failed to deserialize snapshot: {}", e),
                })
            })?
            .0;

        let mut state = self.state.lock().unwrap();
        *state = restored;

        Ok(())
    }

    /// Cleanup test resources
    fn cleanup(&self) {
        let _ = fs::remove_dir_all(&self.test_data_dir);
    }
}

// ============================================================================
// Happy Path Tests
// ============================================================================

#[test]
fn test_vm_lifecycle_full_flow() {
    let mut vm = TestVm::new(GuestArch::Riscv64, 1024 * 1024);

    // Initial state should be Created
    assert_eq!(vm.lifecycle_state, VmLifecycleState::Created);

    // Boot the VM
    assert!(vm.boot().is_ok());
    assert_eq!(vm.lifecycle_state, VmLifecycleState::Running);

    // Pause the VM
    assert!(vm.pause().is_ok());
    assert_eq!(vm.lifecycle_state, VmLifecycleState::Paused);

    // Resume the VM
    assert!(vm.resume().is_ok());
    assert_eq!(vm.lifecycle_state, VmLifecycleState::Running);

    // Stop the VM
    assert!(vm.stop().is_ok());
    assert_eq!(vm.lifecycle_state, VmLifecycleState::Stopped);

    vm.cleanup();
}

#[test]
fn test_vm_initialization() {
    let mut vm = TestVm::new(GuestArch::Arm64, 2 * 1024 * 1024);

    assert!(vm.init().is_ok());

    let state = vm.state.lock().unwrap();
    assert_eq!(state.memory.len(), 2 * 1024 * 1024);
    assert_eq!(state.pc, GuestAddr(0x1000));

    // Verify memory pattern
    assert_eq!(state.memory[0], 0);
    assert_eq!(state.memory[1], 1);
    assert_eq!(state.memory[255], 255);

    vm.cleanup();
}

#[test]
fn test_vm_snapshot_and_restore() {
    let mut vm = TestVm::new(GuestArch::X86_64, 1024 * 1024);

    // Boot and modify state
    assert!(vm.boot().is_ok());

    {
        let mut state = vm.state.lock().unwrap();
        state.regs.x[0] = 0xDEADBEEF;
        state.pc = GuestAddr(0x2000);
        state.memory[0] = 0x42;
    }

    // Create snapshot
    let snapshot_path = vm.create_snapshot("test_snapshot").unwrap();
    assert!(snapshot_path.exists());

    // Modify state again
    {
        let mut state = vm.state.lock().unwrap();
        state.regs.x[0] = 0xBADBADBAD;
        state.pc = GuestAddr(0x3000);
        state.memory[0] = 0x99;
    }

    // Restore from snapshot
    assert!(vm.restore_snapshot(&snapshot_path).is_ok());

    let state = vm.state.lock().unwrap();
    assert_eq!(state.regs.x[0], 0xDEADBEEF);
    assert_eq!(state.pc, GuestAddr(0x2000));
    assert_eq!(state.memory[0], 0x42);

    vm.cleanup();
}

#[test]
fn test_multiple_pause_resume_cycles() {
    let mut vm = TestVm::new(GuestArch::Riscv64, 1024 * 1024);

    assert!(vm.boot().is_ok());

    // Multiple pause/resume cycles
    for _ in 0..10 {
        assert!(vm.pause().is_ok());
        assert_eq!(vm.lifecycle_state, VmLifecycleState::Paused);

        assert!(vm.resume().is_ok());
        assert_eq!(vm.lifecycle_state, VmLifecycleState::Running);
    }

    assert!(vm.stop().is_ok());
    vm.cleanup();
}

// ============================================================================
// Error Path Tests
// ============================================================================

#[test]
fn test_boot_twice_should_fail() {
    let mut vm = TestVm::new(GuestArch::Riscv64, 1024 * 1024);

    assert!(vm.boot().is_ok());

    // Second boot should fail
    let result = vm.boot();
    assert!(result.is_err());

    if let Err(CoreVmError::ExecutionError(ExecutionError::InvalidState { expected, actual })) =
        result
    {
        assert_eq!(expected, "Created");
        assert!(actual.contains("Running"));
    } else {
        panic!("Expected InvalidState error");
    }

    vm.cleanup();
}

#[test]
fn test_pause_when_not_running_should_fail() {
    let mut vm = TestVm::new(GuestArch::Arm64, 1024 * 1024);

    // Try to pause without booting
    let result = vm.pause();
    assert!(result.is_err());

    // Boot then stop, then try to pause
    assert!(vm.boot().is_ok());
    assert!(vm.stop().is_ok());

    let result = vm.pause();
    assert!(result.is_err());

    vm.cleanup();
}

#[test]
fn test_resume_when_not_paused_should_fail() {
    let mut vm = TestVm::new(GuestArch::X86_64, 1024 * 1024);

    // Try to resume without pausing
    assert!(vm.boot().is_ok());
    let result = vm.resume();
    assert!(result.is_err());

    vm.cleanup();
}

#[test]
fn test_stop_when_already_stopped_should_fail() {
    let mut vm = TestVm::new(GuestArch::Riscv64, 1024 * 1024);

    assert!(vm.boot().is_ok());
    assert!(vm.stop().is_ok());

    // Second stop should fail
    let result = vm.stop();
    assert!(result.is_err());

    vm.cleanup();
}

#[test]
fn test_restore_invalid_snapshot_should_fail() {
    let mut vm = TestVm::new(GuestArch::Riscv64, 1024 * 1024);

    let invalid_path = PathBuf::from("/nonexistent/path/to/snapshot.snap");
    let result = vm.restore_snapshot(&invalid_path);
    assert!(result.is_err());

    // Create a file with invalid data
    let invalid_file = vm.test_data_dir.join("invalid.snap");
    fs::write(&invalid_file, b"invalid snapshot data").unwrap();

    let result = vm.restore_snapshot(&invalid_file);
    assert!(result.is_err());

    vm.cleanup();
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_vm_with_minimal_memory() {
    let mut vm = TestVm::new(GuestArch::Riscv64, 4096); // 1 page

    assert!(vm.boot().is_ok());

    let state = vm.state.lock().unwrap();
    assert_eq!(state.memory.len(), 4096);

    vm.cleanup();
}

#[test]
fn test_vm_with_large_memory() {
    let mut vm = TestVm::new(GuestArch::Arm64, 1024 * 1024 * 1024); // 1GB

    assert!(vm.boot().is_ok());

    let state = vm.state.lock().unwrap();
    assert_eq!(state.memory.len(), 1024 * 1024 * 1024);

    vm.cleanup();
}

#[test]
fn test_all_architectures() {
    let archs = [
        GuestArch::Riscv64,
        GuestArch::Arm64,
        GuestArch::X86_64,
        GuestArch::PowerPC64,
    ];

    for arch in archs {
        let mut vm = TestVm::new(arch, 1024 * 1024);
        assert!(vm.boot().is_ok());
        assert!(vm.stop().is_ok());
        vm.cleanup();
    }
}

#[test]
fn test_multiple_snapshots() {
    let mut vm = TestVm::new(GuestArch::Riscv64, 1024 * 1024);

    assert!(vm.boot().is_ok());

    // Create multiple snapshots
    let snapshots = vec!["snap1", "snap2", "snap3"];
    let mut snapshot_paths = Vec::new();

    for snap_name in &snapshots {
        // Modify state
        {
            let mut state = vm.state.lock().unwrap();
            state.pc = GuestAddr(state.pc.0 + 0x1000);
        }

        let path = vm.create_snapshot(snap_name).unwrap();
        snapshot_paths.push(path);
    }

    // Restore from each snapshot and verify
    for (i, path) in snapshot_paths.iter().enumerate() {
        assert!(vm.restore_snapshot(path).is_ok());

        let state = vm.state.lock().unwrap();
        let expected_pc = 0x1000 + ((i + 1) as u64 * 0x1000);
        assert_eq!(state.pc, GuestAddr(expected_pc));
    }

    vm.cleanup();
}

#[test]
fn test_state_transitions_graph() {
    let mut vm = TestVm::new(GuestArch::Riscv64, 1024 * 1024);

    // Created -> Running (via boot)
    assert_eq!(vm.lifecycle_state, VmLifecycleState::Created);
    assert!(vm.boot().is_ok());
    assert_eq!(vm.lifecycle_state, VmLifecycleState::Running);

    // Running -> Paused
    assert!(vm.pause().is_ok());
    assert_eq!(vm.lifecycle_state, VmLifecycleState::Paused);

    // Paused -> Running
    assert!(vm.resume().is_ok());
    assert_eq!(vm.lifecycle_state, VmLifecycleState::Running);

    // Running -> Stopped
    assert!(vm.stop().is_ok());
    assert_eq!(vm.lifecycle_state, VmLifecycleState::Stopped);

    vm.cleanup();
}

#[test]
fn test_concurrent_state_access() {
    use std::thread;

    let vm = Arc::new(Mutex::new(TestVm::new(GuestArch::Riscv64, 1024 * 1024)));
    let vm_clone = Arc::clone(&vm);

    // Boot in main thread
    {
        let mut vm_guard = vm.lock().unwrap();
        vm_guard.boot().unwrap();
    }

    // Spawn thread to access state
    let handle = thread::spawn(move || {
        let vm = vm_clone.lock().unwrap();
        let state = vm.state.lock().unwrap();
        assert_eq!(state.memory.len(), 1024 * 1024);
    });

    handle.join().unwrap();

    // Cleanup
    let vm_guard = vm.lock().unwrap();
    vm_guard.cleanup();
}

#[test]
fn test_snapshot_persistence() {
    let mut vm1 = TestVm::new(GuestArch::Arm64, 1024 * 1024);

    // Create VM and snapshot
    assert!(vm1.boot().is_ok());

    {
        let mut state = vm1.state.lock().unwrap();
        state.regs.x[0] = 0x12345678;
        state.regs.x[1] = 0x87654321;
        state.pc = GuestAddr(0x5000);
    }

    let snapshot_path = vm1.create_snapshot("persist_test").unwrap();

    // Create new VM instance and restore
    let mut vm2 = TestVm::new(GuestArch::Arm64, 1024 * 1024);
    assert!(vm2.restore_snapshot(&snapshot_path).is_ok());

    let state = vm2.state.lock().unwrap();
    assert_eq!(state.regs.x[0], 0x12345678);
    assert_eq!(state.regs.x[1], 0x87654321);
    assert_eq!(state.pc, GuestAddr(0x5000));

    vm1.cleanup();
    vm2.cleanup();
}

// ============================================================================
// Performance and Stress Tests
// ============================================================================

#[test]
fn test_rapid_snapshot_creation() {
    let vm = TestVm::new(GuestArch::Riscv64, 1024 * 1024);
    assert!(vm.boot().is_ok());

    // Create 100 snapshots rapidly
    for i in 0..100 {
        let _ = vm.create_snapshot(&format!("rapid_{}", i));
    }

    vm.cleanup();
}

#[test]
fn test_large_memory_snapshot() {
    let mut vm = TestVm::new(GuestArch::Arm64, 10 * 1024 * 1024); // 10MB
    assert!(vm.boot().is_ok());

    let snapshot_path = vm.create_snapshot("large_mem").unwrap();
    assert!(snapshot_path.exists());

    // Check file size
    let metadata = fs::metadata(&snapshot_path).unwrap();
    assert!(metadata.len() > 10 * 1024 * 1024);

    vm.cleanup();
}
