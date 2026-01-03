//! Integration test for LockFreeMMU
//!
//! This test verifies that LockFreeMMU implements all required MMU traits

use vm_core::{AccessType, AddressTranslator, GuestAddr, MMU, MemoryAccess};
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
    // In bare mode, addresses should translate directly
    let result = mmu.translate(GuestAddr(0x1000), AccessType::Read);
    assert!(result.is_ok());

    // Test TLB flush
    mmu.flush_tlb();

    // Test page flush
    mmu.flush_tlb_page(GuestAddr(0x2000));
}

#[test]
fn test_lockfree_mmu_memory_operations() {
    let mut mmu = LockFreeMMU::new(1024 * 1024, false);

    // Test read operations
    let addr = GuestAddr(0x1000);
    let result = mmu.read(addr, 4);
    assert!(result.is_ok());

    // Test write operations
    let write_result = mmu.write(addr, 0x12345678, 4);
    assert!(write_result.is_ok());

    // Test fetch instruction
    let pc = GuestAddr(0x1000);
    let insn = mmu.fetch_insn(pc);
    assert!(insn.is_ok());
}
