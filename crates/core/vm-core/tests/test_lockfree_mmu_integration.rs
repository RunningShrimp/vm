//! Integration test for LockFreeMMU
//!
//! This test verifies that LockFreeMMU can be used with VirtualMachine

use vm_core::{MMU, VmConfig};
use vm_mem::LockFreeMMU;

#[test]
fn test_lockfree_mmu_implements_mmu_trait() {
    // Create a LockFreeMMU instance
    let _mmu: Box<dyn MMU> = Box::new(LockFreeMMU::new(1024 * 1024, false));
    // If this compiles, LockFreeMMU successfully implements MMU trait
}

#[test]
fn test_lockfree_mmu_basic_operations() {
    let mut mmu = LockFreeMMU::new(1024 * 1024, false);

    // Test basic operations through MMU trait
    use vm_core::{AccessType, GuestAddr};

    // In bare mode, addresses should translate directly
    let result = mmu.translate(GuestAddr(0x1000), AccessType::Read);
    assert!(result.is_ok());
}
