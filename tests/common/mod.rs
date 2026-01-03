//! Common test utilities for integration tests
//!
//! This module provides reusable helpers, fixtures, and test infrastructure
//! for integration tests across the entire VM workspace.

#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use vm_core::{
    Config, CoreError, DeviceError, ExecMode, ExecutionError, GuestAddr, GuestArch,
    GuestPhysAddr, GuestRegs, MemoryError, VmConfig, VmError, VmLifecycleState,
};

use vm_engine::jit::{JITCompiler, JITConfig};

use vm_ir::{IRBlock, IROp, MemFlags, RegId, Terminator};

// ============================================================================
// Test Configuration
// ============================================================================

/// Default test memory size (1 MB)
pub const DEFAULT_MEMORY_SIZE: usize = 1024 * 1024;

/// Default test timeout in seconds
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Get the test data directory
pub fn test_data_dir() -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push("vm_integration_tests");
    let _ = fs::create_dir_all(&dir);
    dir
}

/// Clean up test data directory
pub fn cleanup_test_data() {
    let dir = test_data_dir();
    let _ = fs::remove_dir_all(&dir);
}

// ============================================================================
// Test Fixtures
// ============================================================================

/// Basic VM configuration for testing
pub struct TestVmConfig {
    pub arch: GuestArch,
    pub memory_size: usize,
    pub vcpu_count: usize,
    pub exec_mode: ExecMode,
}

impl Default for TestVmConfig {
    fn default() -> Self {
        Self {
            arch: GuestArch::Riscv64,
            memory_size: DEFAULT_MEMORY_SIZE,
            vcpu_count: 1,
            exec_mode: ExecMode::Interpreter,
        }
    }
}

/// Simple VM state for testing
#[derive(Debug, Clone)]
pub struct TestVmState {
    pub regs: GuestRegs,
    pub memory: Vec<u8>,
    pub pc: GuestAddr,
}

impl Default for TestVmState {
    fn default() -> Self {
        Self {
            regs: GuestRegs::default(),
            memory: vec![0u8; DEFAULT_MEMORY_SIZE],
            pc: GuestAddr(0),
        }
    }
}

/// Test VM instance wrapper
pub struct TestVm {
    pub config: VmConfig,
    pub state: Arc<Mutex<TestVmState>>,
    pub lifecycle_state: VmLifecycleState,
    pub test_data_dir: PathBuf,
}

impl TestVm {
    /// Create a new test VM instance
    pub fn new(config: TestVmConfig) -> Self {
        let vm_config = VmConfig {
            guest_arch: config.arch,
            memory_size: config.memory_size,
            vcpu_count: config.vcpu_count,
            exec_mode: config.exec_mode,
            kernel_path: None,
            initrd_path: None,
        };

        let state = Arc::new(Mutex::new(TestVmState::default()));

        let test_data_dir = test_data_dir();

        TestVm {
            config: vm_config,
            state,
            lifecycle_state: VmLifecycleState::Created,
            test_data_dir,
        }
    }

    /// Initialize VM with test pattern
    pub fn init(&mut self) -> Result<(), VmError> {
        let mut state = self.state.lock().unwrap();
        for (i, byte) in state.memory.iter_mut().enumerate() {
            *byte = (i & 0xFF) as u8;
        }
        state.pc = GuestAddr(0x1000);
        Ok(())
    }

    /// Write to guest memory
    pub fn write_memory(&self, addr: GuestAddr, data: &[u8]) -> Result<(), MemoryError> {
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

    /// Read from guest memory
    pub fn read_memory(&self, addr: GuestAddr, size: usize) -> Result<Vec<u8>, MemoryError> {
        let state = self.state.lock().unwrap();
        let start = addr.0 as usize;
        let end = start + size;

        if end > state.memory.len() {
            return Err(MemoryError::InvalidAddress {
                addr: addr.0,
                size,
            });
        }

        Ok(state.memory[start..end].to_vec())
    }

    /// Set PC
    pub fn set_pc(&self, pc: GuestAddr) {
        let mut state = self.state.lock().unwrap();
        state.pc = pc;
    }

    /// Get PC
    pub fn pc(&self) -> GuestAddr {
        let state = self.state.lock().unwrap();
        state.pc
    }
}

// ============================================================================
// IR Builder Utilities
// ============================================================================

/// Helper to build IR blocks for testing
pub struct TestIRBuilder {
    block: IRBlock,
}

impl TestIRBuilder {
    /// Create a new IR builder
    pub fn new(start_pc: u64) -> Self {
        Self {
            block: IRBlock {
                start_pc: GuestAddr(start_pc),
                ops: Vec::new(),
                term: Terminator::Ret,
            },
        }
    }

    /// Add an operation
    pub fn push(mut self, op: IROp) -> Self {
        self.block.ops.push(op);
        self
    }

    /// Set terminator
    pub fn terminator(mut self, term: Terminator) -> Self {
        self.block.term = term;
        self
    }

    /// Build the block
    pub fn build(self) -> IRBlock {
        self.block
    }
}

/// Create a simple arithmetic IR block
pub fn create_arithmetic_block() -> IRBlock {
    TestIRBuilder::new(0x1000)
        .push(IROp::MovImm {
            dst: 1,
            imm: 10,
        })
        .push(IROp::MovImm {
            dst: 2,
            imm: 20,
        })
        .push(IROp::Add {
            dst: 3,
            src1: 1,
            src2: 2,
        })
        .terminator(Terminator::Ret)
        .build()
}

/// Create a memory access IR block
pub fn create_memory_block() -> IRBlock {
    TestIRBuilder::new(0x1000)
        .push(IROp::MovImm {
            dst: 1,
            imm: 0x1000,
        })
        .push(IROp::Load {
            dst: 2,
            addr: 1,
            flags: MemFlags::default(),
        })
        .push(IROp::MovImm {
            dst: 3,
            imm: 42,
        })
        .push(IROp::Store {
            addr: 1,
            src: 3,
            flags: MemFlags::default(),
        })
        .terminator(Terminator::Ret)
        .build()
}

/// Create a control flow IR block
pub fn create_control_flow_block() -> IRBlock {
    TestIRBuilder::new(0x1000)
        .push(IROp::MovImm {
            dst: 1,
            imm: 10,
        })
        .push(IROp::MovImm {
            dst: 2,
            imm: 20,
        })
        .terminator(Terminator::BranchCond {
            src1: 1,
            src2: 2,
            target: GuestAddr(0x2000),
        })
        .build()
}

// ============================================================================
// Assertion Helpers
// ============================================================================

/// Assert that an operation completes within a timeout
pub fn assert_timeout<F>(f: F, timeout: Duration, message: &str)
where
    F: FnOnce() -> Result<(), Box<dyn std::error::Error>>,
{
    let start = std::time::Instant::now();
    let result = f();

    let elapsed = start.elapsed();
    assert!(
        elapsed < timeout,
        "{}: Operation took {}ms, expected < {}ms",
        message,
        elapsed.as_millis(),
        timeout.as_millis()
    );

    result.expect("Operation should succeed");
}

/// Assert that memory contains expected pattern
pub fn assert_memory_pattern(memory: &[u8], addr: usize, pattern: &[u8]) {
    let end = addr + pattern.len();
    assert!(
        end <= memory.len(),
        "Memory range {:x}..{:x} out of bounds",
        addr,
        end
    );

    let actual = &memory[addr..end];
    assert_eq!(
        actual, pattern,
        "Memory at {:x} does not match expected pattern",
        addr
    );
}

/// Assert VM is in expected lifecycle state
pub fn assert_lifecycle_state(vm: &TestVm, expected: VmLifecycleState) {
    assert_eq!(
        vm.lifecycle_state, expected,
        "VM should be in {:?} state, but is in {:?}",
        expected, vm.lifecycle_state
    );
}

// ============================================================================
// Property-Based Testing Helpers
// ============================================================================

/// Generate a random valid memory address
pub fn random_memory_addr(size: usize) -> u64 {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    rng.gen_range(0..(size as u64))
}

/// Generate random byte sequence
pub fn random_bytes(len: usize) -> Vec<u8> {
    use rand::Rng;
    let mut bytes = vec![0u8; len];
    rand::thread_rng().fill(&mut bytes[..]);
    bytes
}

/// Generate random register value
pub fn random_reg_value() -> u64 {
    use rand::Rng;
    rand::thread_rng().gen()
}

/// Generate random valid register ID (0-31 for RISC-V)
pub fn random_reg_id() -> RegId {
    use rand::Rng;
    rand::thread_rng().gen_range(0..32)
}

// ============================================================================
// Error Matching Helpers
// ============================================================================

/// Check if error is a memory error
pub fn is_memory_error(err: &VmError) -> bool {
    matches!(err, VmError::MemoryError(_))
}

/// Check if error is an execution error
pub fn is_execution_error(err: &VmError) -> bool {
    matches!(err, VmError::ExecutionError(_))
}

/// Check if error is a device error
pub fn is_device_error(err: &VmError) -> bool {
    matches!(err, VmError::DeviceError(_))
}

// ============================================================================
// Benchmark Utilities
// ============================================================================

/// Measure execution time of a function
pub fn measure_time<F, R>(f: F) -> (R, Duration)
where
    F: FnOnce() -> R,
{
    let start = std::time::Instant::now();
    let result = f();
    let elapsed = start.elapsed();
    (result, elapsed)
}

/// Simple performance stats
#[derive(Debug, Clone)]
pub struct PerfStats {
    pub iterations: usize,
    pub total_time: Duration,
    pub avg_time: Duration,
    pub min_time: Duration,
    pub max_time: Duration,
}

/// Run a function multiple times and collect performance stats
pub fn benchmark<F>(mut f: F, iterations: usize) -> PerfStats
where
    F: FnMut() -> (),
{
    let mut times = Vec::with_capacity(iterations);
    let total_start = std::time::Instant::now();

    for _ in 0..iterations {
        let start = std::time::Instant::now();
        f();
        times.push(start.elapsed());
    }

    let total_time = total_start.elapsed();
    let min_time = times.iter().min().copied().unwrap_or_default();
    let max_time = times.iter().max().copied().unwrap_or_default();
    let avg_time = total_time / iterations as u32;

    PerfStats {
        iterations,
        total_time,
        avg_time,
        min_time,
        max_time,
    }
}

// ============================================================================
// Test Cleanup Utilities
// ============================================================================

/// Test cleanup guard that runs cleanup on drop
pub struct CleanupGuard;

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        cleanup_test_data();
    }
}

/// Create a cleanup guard
pub fn cleanup_guard() -> CleanupGuard {
    CleanupGuard
}
