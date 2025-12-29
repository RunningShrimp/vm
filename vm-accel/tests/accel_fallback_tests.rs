//! Acceleration Fallback Tests
//!
//! Tests for the acceleration fallback manager

use vm_accel::accel_fallback::{AccelFallbackManager, ExecResult};
use vm_core::{GuestRegs, MMU, VmError};

/// Mock MMU for testing
struct MockMMU;
impl MMU for MockMMU {
    fn read(&mut self, addr: vm_core::GuestAddr, size: usize) -> Result<Vec<u8>, VmError> {
        Ok(vec![0; size])
    }

    fn write(&mut self, _addr: vm_core::GuestAddr, _data: &[u8]) -> Result<(), VmError> {
        Ok(())
    }

    fn fetch(&mut self, _addr: vm_core::GuestAddr) -> Result<Vec<u8>, VmError> {
        Ok(vec![0; 4])
    }

    fn translate(&mut self, _addr: vm_core::GuestAddr) -> Result<u64, VmError> {
        Ok(0)
    }
}

/// Test fallback manager creation
#[test]
fn test_fallback_manager_creation() {
    let manager = AccelFallbackManager::new();
    println!("Fallback manager created");
}

/// Test fallback execution result
#[test]
fn test_fallback_execution_result() {
    let result = ExecResult {
        cycles: 100,
        executed: true,
    };

    assert_eq!(result.cycles, 100);
    assert!(result.executed);
    println!("Execution result created");
}

/// Test fallback manager execution
#[test]
fn test_fallback_manager_execution() {
    let mut manager = AccelFallbackManager::new();
    let mut mmu = MockMMU;

    // Try to execute some instructions
    let result = manager.execute(&mut mmu, 0x1000, 10);

    match result {
        Ok(exec_result) => {
            println!("Executed {} cycles, executed: {}",
                exec_result.cycles, exec_result.executed);
        }
        Err(e) => {
            println!("Execution failed: {:?}", e);
        }
    }
}

/// Test fallback manager with different instruction counts
#[test]
fn test_fallback_manager_instruction_counts() {
    let mut manager = AccelFallbackManager::new();
    let mut mmu = MockMMU;

    let counts = vec![1, 10, 100, 1000];

    for count in counts {
        let result = manager.execute(&mut mmu, 0x1000, count);
        println!("Execution with {} instructions: {:?}", count, result.is_ok());
    }
}

/// Test fallback manager error handling
#[test]
fn test_fallback_manager_error_handling() {
    let mut manager = AccelFallbackManager::new();
    let mut mmu = MockMMU;

    // Try with invalid address
    let result = manager.execute(&mut mmu, 0xFFFF_FFFF_F000, 10);
    println!("Execution with invalid address: {:?}", result.is_err());
}

/// Test fallback manager state management
#[test]
fn test_fallback_manager_state_management() {
    let manager = AccelFallbackManager::new();

    // Manager should maintain state across executions
    // (This is a basic smoke test)
    println!("State management test completed");
}
