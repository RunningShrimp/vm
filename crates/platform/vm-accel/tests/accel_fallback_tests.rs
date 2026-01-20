//! Acceleration Fallback Tests
//!
//! Tests for the acceleration fallback manager

use vm_accel::accel_fallback::{AccelFallbackManager, ExecResult};
use vm_core::{AccessType, GuestAddr, GuestPhysAddr, MmioDevice, MmuAsAny};
use vm_core::{AddressTranslator, MMU, MemoryAccess, MmioManager, VmError};

/// Mock MMU for testing
struct MockMMU;

impl vm_core::MmuAsAny for MockMMU {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl AddressTranslator for MockMMU {
    fn translate(&mut self, _va: GuestAddr, _access: AccessType) -> Result<GuestPhysAddr, VmError> {
        Ok(GuestPhysAddr(0))
    }

    fn flush_tlb(&mut self) {}
}

impl MemoryAccess for MockMMU {
    fn read(&self, _pa: GuestAddr, size: u8) -> Result<u64, VmError> {
        Ok(0)
    }

    fn write(&mut self, _pa: GuestAddr, _val: u64, _size: u8) -> Result<(), VmError> {
        Ok(())
    }

    fn fetch_insn(&self, _pc: GuestAddr) -> Result<u64, VmError> {
        Ok(0)
    }

    fn memory_size(&self) -> usize {
        0
    }

    fn dump_memory(&self) -> Vec<u8> {
        Vec::new()
    }

    fn restore_memory(&mut self, _data: &[u8]) -> Result<(), String> {
        Ok(())
    }
}

impl MmioManager for MockMMU {
    fn map_mmio(&self, _base: GuestAddr, _size: u64, _device: Box<dyn MmioDevice>) {
        // Mock implementation
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
        success: true,
        error: None,
        pc: GuestAddr(0),
    };

    assert!(result.success);
    assert!(result.error.is_none());
    println!("Execution result created");
}

/// Test fallback manager execution
#[test]
fn test_fallback_manager_execution() {
    let manager = AccelFallbackManager::new();
    // Test recording a failure
    manager.record_failure(FallbackError::UnsupportedInstruction);
    println!("Fallback manager failure recording test completed");
}

/// Test fallback manager with different instruction counts
#[test]
fn test_fallback_manager_instruction_counts() {
    let manager = AccelFallbackManager::new();

    // Test recording multiple failures
    let errors = vec![
        FallbackError::UnsupportedInstruction,
        FallbackError::MemoryError,
        FallbackError::IoError,
    ];

    for error in errors {
        manager.record_failure(error);
        println!("Recorded error: {:?}", error);
    }
}

/// Test fallback manager error handling
#[test]
fn test_fallback_manager_error_handling() {
    let manager = AccelFallbackManager::new();

    // Test error handling
    manager.record_failure(FallbackError::InterruptError);
    manager.record_failure(FallbackError::Other("Test error".to_string()));

    println!("Error handling test completed");
}

/// Test fallback manager state management
#[test]
fn test_fallback_manager_state_management() {
    let manager = AccelFallbackManager::new();

    // Manager should maintain state across executions
    // (This is a basic smoke test)
    println!("State management test completed");
}
