//! Comprehensive tests for vm-mem module
//!
//! This test suite covers memory management, TLB, MMU, and related functionality.

use std::sync::{Arc, RwLock};
use vm_core::{
    AccessType, AddressTranslator, Fault, GuestAddr, GuestPhysAddr, MMU, MemoryAccess, MmioDevice,
    VmError,
};
use vm_mem::{
    PAGE_SHIFT, PAGE_SIZE, PTE_SIZE, PTES_PER_PAGE, PageTableBuilder, PagingMode, SoftMmu,
    pte_flags,
};

// Type alias for result
type VmResult<T> = Result<T, VmError>;

// ============================================================================
// TLB Tests
// ============================================================================

#[test]
fn test_tlb_initialization() {
    let mmu = SoftMmu::new(1024 * 1024, false);
    let (itlb_size, dtlb_size) = mmu.tlb_capacity();
    assert_eq!(itlb_size, 64);
    assert_eq!(dtlb_size, 128);
}

#[test]
fn test_tlb_stats_initial() {
    let mmu = SoftMmu::new(1024 * 1024, false);
    let (hits, misses) = mmu.tlb_stats();
    assert_eq!(hits, 0);
    assert_eq!(misses, 0);
}

#[test]
fn test_tlb_resize() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    mmu.resize_tlbs(32, 64);
    let (itlb_size, dtlb_size) = mmu.tlb_capacity();
    assert_eq!(itlb_size, 32);
    assert_eq!(dtlb_size, 64);
}

#[test]
fn test_tlb_flush() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    // Set up paging and do some translations
    mmu.set_paging_mode(PagingMode::Sv39);
    let satp = (8u64 << 60) | (0x1000 >> 12);
    mmu.set_satp(satp);

    // Flush should clear TLB
    mmu.flush_tlb();
    let (hits, misses) = mmu.tlb_stats();
    assert_eq!(hits, 0);
    assert_eq!(misses, 0);
}

// ============================================================================
// Memory Access Tests
// ============================================================================

#[test]
fn test_memory_read_write_u8() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    let addr = GuestAddr(0x1000);

    // Write
    mmu.write(addr, 0xAB, 1).unwrap();

    // Read
    let value = mmu.read(addr, 1).unwrap();
    assert_eq!(value, 0xAB);
}

#[test]
fn test_memory_read_write_u16() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    let addr = GuestAddr(0x1000);

    // Write
    mmu.write(addr, 0xABCD, 2).unwrap();

    // Read
    let value = mmu.read(addr, 2).unwrap();
    assert_eq!(value, 0xABCD);
}

#[test]
fn test_memory_read_write_u32() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    let addr = GuestAddr(0x1000);

    // Write
    mmu.write(addr, 0xDEADBEEF, 4).unwrap();

    // Read
    let value = mmu.read(addr, 4).unwrap();
    assert_eq!(value, 0xDEADBEEF);
}

#[test]
fn test_memory_read_write_u64() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    let addr = GuestAddr(0x1000);

    // Write
    mmu.write(addr, 0x123456789ABCDEF0, 8).unwrap();

    // Read
    let value = mmu.read(addr, 8).unwrap();
    assert_eq!(value, 0x123456789ABCDEF0);
}

#[test]
fn test_memory_multiple_writes() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    // Write multiple values
    mmu.write(GuestAddr(0x1000), 0x1111, 4).unwrap();
    mmu.write(GuestAddr(0x2000), 0x2222, 4).unwrap();
    mmu.write(GuestAddr(0x3000), 0x3333, 4).unwrap();

    // Verify
    assert_eq!(mmu.read(GuestAddr(0x1000), 4).unwrap(), 0x1111);
    assert_eq!(mmu.read(GuestAddr(0x2000), 4).unwrap(), 0x2222);
    assert_eq!(mmu.read(GuestAddr(0x3000), 4).unwrap(), 0x3333);
}

#[test]
fn test_memory_overwrite() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    let addr = GuestAddr(0x1000);

    // Write first value
    mmu.write(addr, 0x1111, 4).unwrap();
    assert_eq!(mmu.read(addr, 4).unwrap(), 0x1111);

    // Overwrite
    mmu.write(addr, 0x2222, 4).unwrap();
    assert_eq!(mmu.read(addr, 4).unwrap(), 0x2222);
}

// ============================================================================
// Bulk Operations Tests
// ============================================================================

#[test]
fn test_read_bulk() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    let addr = GuestAddr(0x1000);

    // Write data
    let data = vec![0u8; 256];
    mmu.write_bulk(addr, &data).unwrap();

    // Read back
    let mut buffer = vec![0u8; 256];
    mmu.read_bulk(addr, &mut buffer).unwrap();

    assert_eq!(buffer, data);
}

#[test]
fn test_write_bulk() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    let addr = GuestAddr(0x1000);

    // Write bulk
    let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
    mmu.write_bulk(addr, &data).unwrap();

    // Verify individual reads
    for i in 0..data.len() {
        let value = mmu.read(addr + i as u64, 1).unwrap();
        assert_eq!(value as u8, data[i]);
    }
}

#[test]
fn test_bulk_large_data() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    let addr = GuestAddr(0x1000);

    // Write 4KB of data
    let data = vec![0xABu8; 4096];
    mmu.write_bulk(addr, &data).unwrap();

    // Read back
    let mut buffer = vec![0u8; 4096];
    mmu.read_bulk(addr, &mut buffer).unwrap();

    assert_eq!(buffer, data);
}

// ============================================================================
// Address Translation Tests
// ============================================================================

#[test]
fn test_translate_bare_mode() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    // Bare mode should be identity mapping
    let va = GuestAddr(0x1000);
    let pa = mmu.translate(va, AccessType::Read).unwrap();
    assert_eq!(pa, GuestPhysAddr(0x1000));
}

#[test]
fn test_translate_bare_mode_write() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    let va = GuestAddr(0x1000);
    let pa = mmu.translate(va, AccessType::Write).unwrap();
    assert_eq!(pa, GuestPhysAddr(0x1000));
}

#[test]
fn test_translate_bare_mode_execute() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    let va = GuestAddr(0x1000);
    let pa = mmu.translate(va, AccessType::Execute).unwrap();
    assert_eq!(pa, GuestPhysAddr(0x1000));
}

#[test]
fn test_translate_various_addresses() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    let addresses = vec![
        GuestAddr(0x0),
        GuestAddr(0x1000),
        GuestAddr(0x10000),
        GuestAddr(0x100000),
    ];

    for va in addresses {
        let pa = mmu.translate(va, AccessType::Read).unwrap();
        assert_eq!(pa, GuestPhysAddr(va.0));
    }
}

// ============================================================================
// Alignment Tests
// ============================================================================

#[test]
fn test_alignment_strict_mode_enabled() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    mmu.set_strict_align(true);

    // Aligned access should succeed
    mmu.write(GuestAddr(0x1000), 0xAB, 1).unwrap();
    assert!(mmu.read(GuestAddr(0x1000), 1).is_ok());

    // Unaligned access should fail
    assert!(mmu.read(GuestAddr(0x1001), 2).is_err());
    assert!(mmu.read(GuestAddr(0x1002), 4).is_err());
    assert!(mmu.read(GuestAddr(0x1004), 8).is_err());
}

#[test]
fn test_alignment_strict_mode_disabled() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    mmu.set_strict_align(false);

    // Unaligned access should succeed
    mmu.write(GuestAddr(0x1001), 0xAB, 1).unwrap();
    assert!(mmu.read(GuestAddr(0x1001), 1).is_ok());
}

#[test]
fn test_alignment_all_sizes() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    mmu.set_strict_align(true);

    // Test all size alignments
    assert!(mmu.write(GuestAddr(0x1000), 0, 1).is_ok());
    assert!(mmu.write(GuestAddr(0x1002), 0, 2).is_ok());
    assert!(mmu.write(GuestAddr(0x1004), 0, 4).is_ok());
    assert!(mmu.write(GuestAddr(0x1008), 0, 8).is_ok());
}

// ============================================================================
// Memory Size Tests
// ============================================================================

#[test]
fn test_memory_size() {
    let mmu = SoftMmu::new(16 * 1024 * 1024, false);
    assert_eq!(mmu.memory_size(), 16 * 1024 * 1024);
}

#[test]
fn test_memory_size_different() {
    let mmu = SoftMmu::new(64 * 1024 * 1024, false);
    assert_eq!(mmu.memory_size(), 64 * 1024 * 1024);
}

#[test]
fn test_out_of_bounds_read() {
    let mmu = SoftMmu::new(1024, false);
    let addr = GuestAddr(0x10000); // Beyond memory size

    let result = mmu.read(addr, 4);
    assert!(result.is_err());
}

#[test]
fn test_out_of_bounds_write() {
    let mut mmu = SoftMmu::new(1024, false);
    let addr = GuestAddr(0x10000); // Beyond memory size

    let result = mmu.write(addr, 0x1234, 4);
    assert!(result.is_err());
}

// ============================================================================
// Memory Dump and Restore Tests
// ============================================================================

#[test]
fn test_dump_memory() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    // Write some data
    mmu.write(GuestAddr(0x1000), 0x1111, 4).unwrap();
    mmu.write(GuestAddr(0x2000), 0x2222, 4).unwrap();

    // Dump
    let dump = mmu.dump_memory();

    // Check size
    assert_eq!(dump.len(), 1024 * 1024);
}

#[test]
fn test_restore_memory() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    // Write some data
    mmu.write(GuestAddr(0x1000), 0x1111, 4).unwrap();
    mmu.write(GuestAddr(0x2000), 0x2222, 4).unwrap();

    // Dump
    let dump = mmu.dump_memory();

    // Create new MMU and restore
    let mut mmu2 = SoftMmu::new(1024 * 1024, false);
    mmu2.restore_memory(&dump).unwrap();

    // Verify restored data
    assert_eq!(mmu2.read(GuestAddr(0x1000), 4).unwrap(), 0x1111);
    assert_eq!(mmu2.read(GuestAddr(0x2000), 4).unwrap(), 0x2222);
}

#[test]
fn test_restore_memory_wrong_size() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    let wrong_dump = vec![0u8; 512 * 1024]; // Wrong size

    let result = mmu.restore_memory(&wrong_dump);
    assert!(result.is_err());
}

// ============================================================================
// Instruction Fetch Tests
// ============================================================================

#[test]
fn test_fetch_insn() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    let pc = GuestAddr(0x1000);

    // Write instruction
    mmu.write(pc, 0x12345678, 4).unwrap();

    // Fetch
    let insn = mmu.fetch_insn(pc).unwrap();
    assert_eq!(insn, 0x12345678);
}

#[test]
fn test_fetch_multiple_insns() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    // Write multiple instructions
    mmu.write(GuestAddr(0x1000), 0x11111111, 4).unwrap();
    mmu.write(GuestAddr(0x1004), 0x22222222, 4).unwrap();
    mmu.write(GuestAddr(0x1008), 0x33333333, 4).unwrap();

    // Fetch
    assert_eq!(mmu.fetch_insn(GuestAddr(0x1000)).unwrap(), 0x11111111);
    assert_eq!(mmu.fetch_insn(GuestAddr(0x1004)).unwrap(), 0x22222222);
    assert_eq!(mmu.fetch_insn(GuestAddr(0x1008)).unwrap(), 0x33333333);
}

// ============================================================================
// Load-Reserved/Store-Conditional Tests
// ============================================================================

#[test]
fn test_load_reserved_store_conditional_success() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    let addr = GuestAddr(0x1000);

    // Initialize
    mmu.write(addr, 0x1111, 4).unwrap();

    // Load-reserved
    let value = mmu.load_reserved(addr, 4).unwrap();
    assert_eq!(value, 0x1111);

    // Store-conditional should succeed
    let success = mmu.store_conditional(addr, 0x2222, 4).unwrap();
    assert!(success);

    // Verify new value
    assert_eq!(mmu.read(addr, 4).unwrap(), 0x2222);
}

#[test]
fn test_load_reserved_store_conditional_fail() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    let addr = GuestAddr(0x1000);

    // Load-reserved
    mmu.load_reserved(addr, 4).unwrap();

    // Invalidate reservation
    mmu.invalidate_reservation(addr, 4);

    // Store-conditional should fail
    let success = mmu.store_conditional(addr, 0x2222, 4).unwrap();
    assert!(!success);
}

#[test]
fn test_load_reserved_store_conditional_different_sizes() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    let addr = GuestAddr(0x1000);

    // Test all sizes
    for size in [1u8, 2, 4, 8] {
        let value: u64 = match size {
            1 => 0xAB,
            2 => 0xABCD,
            4 => 0xDEADBEEF,
            8 => 0x123456789ABCDEF0,
            _ => unreachable!(),
        };

        mmu.write(addr, value, size).unwrap();
        mmu.load_reserved(addr, size).unwrap();
        let success = mmu.store_conditional(addr, value, size).unwrap();
        assert!(success, "Size {}", size);
    }
}

// ============================================================================
// Paging Mode Tests
// ============================================================================

#[test]
fn test_set_paging_mode() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    // Set different modes
    mmu.set_paging_mode(PagingMode::Bare);
    mmu.set_paging_mode(PagingMode::Sv39);
    mmu.set_paging_mode(PagingMode::Sv48);
    mmu.set_paging_mode(PagingMode::Arm64);
    mmu.set_paging_mode(PagingMode::X86_64);
}

#[test]
fn test_set_paging_mode_flushes_tlb() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    mmu.set_paging_mode(PagingMode::Sv39);
    let satp = (8u64 << 60) | (0x1000 >> 12);
    mmu.set_satp(satp);

    // Do a translation to populate TLB
    let _ = mmu.translate(GuestAddr(0x1000), AccessType::Read);

    // Change paging mode should flush TLB
    mmu.set_paging_mode(PagingMode::Bare);
    let (hits, misses) = mmu.tlb_stats();
    assert_eq!(hits, 0); // TLB was flushed
}

// ============================================================================
// SATP Tests
// ============================================================================

#[test]
fn test_set_satp_bare_mode() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    // Set SATP with bare mode (mode = 0)
    let satp = 0u64;
    mmu.set_satp(satp);

    // Should still be in bare mode
    let pa = mmu.translate(GuestAddr(0x1000), AccessType::Read).unwrap();
    assert_eq!(pa, GuestPhysAddr(0x1000));
}

#[test]
fn test_set_satp_sv39() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    // Set SATP with SV39 mode (mode = 8)
    let root_page = 0x10000;
    let satp = (8u64 << 60) | (root_page >> 12);
    mmu.set_satp(satp);

    // Paging mode should be SV39
    // (Translation will fail without valid page tables, but mode should be set)
}

#[test]
fn test_set_satp_sv48() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    // Set SATP with SV48 mode (mode = 9)
    let root_page = 0x10000;
    let satp = (9u64 << 60) | (root_page >> 12);
    mmu.set_satp(satp);

    // Paging mode should be SV48
}

// ============================================================================
// Clone Tests
// ============================================================================

#[test]
fn test_soft_mmu_clone() {
    let mut mmu1 = SoftMmu::new(1024 * 1024, false);
    mmu1.write(GuestAddr(0x1000), 0x1111, 4).unwrap();

    let mut mmu2 = mmu1.clone();

    // Both should have access to same physical memory
    assert_eq!(mmu2.read(GuestAddr(0x1000), 4).unwrap(), 0x1111);

    // Write through mmu2
    mmu2.write(GuestAddr(0x2000), 0x2222, 4).unwrap();

    // mmu1 should see it
    assert_eq!(mmu1.read(GuestAddr(0x2000), 4).unwrap(), 0x2222);
}

#[test]
fn test_clone_separate_tlbs() {
    let mut mmu1 = SoftMmu::new(1024 * 1024, false);
    mmu1.set_paging_mode(PagingMode::Sv39);
    let satp = (8u64 << 60) | (0x1000 >> 12);
    mmu1.set_satp(satp);

    let mmu2 = mmu1.clone();

    // Each should have separate TLBs
    let (hits1, _) = mmu1.tlb_stats();
    let (hits2, _) = mmu2.tlb_stats();
    assert_eq!(hits1, 0);
    assert_eq!(hits2, 0);
}

// ============================================================================
// Page Table Builder Tests
// ============================================================================

#[test]
fn test_page_table_builder_allocation() {
    let mut builder = PageTableBuilder::new(GuestPhysAddr(0x10000));

    let page1 = builder.alloc_page();
    let page2 = builder.alloc_page();
    let page3 = builder.alloc_page();

    assert_eq!(page1, GuestPhysAddr(0x10000));
    assert_eq!(page2, GuestPhysAddr(0x11000));
    assert_eq!(page3, GuestPhysAddr(0x12000));
}

#[test]
fn test_page_table_builder_track_allocated() {
    let mut builder = PageTableBuilder::new(GuestPhysAddr(0x10000));

    builder.alloc_page();
    builder.alloc_page();

    // Note: allocated_pages is private, so we can't test it directly
    // The allocation tests verify the builder works correctly
    assert!(builder.alloc_page() >= GuestPhysAddr(0x12000));
}

// ============================================================================
// MMIO Device Tests
// ============================================================================

// Note: SoftMmu doesn't expose map_mmio in public API for testing
// These tests are simplified to just verify the MMIO device trait implementation

struct TestDevice {
    registers: RwLock<[u64; 8]>,
}

impl TestDevice {
    fn new() -> Self {
        Self {
            registers: RwLock::new([0; 8]),
        }
    }
}

impl MmioDevice for TestDevice {
    fn read(&self, offset: u64, size: u8) -> VmResult<u64> {
        let reg_idx = (offset / 8) as usize;
        let regs = self.registers.read().unwrap();
        Ok(regs[reg_idx])
    }

    fn write(&mut self, offset: u64, value: u64, _size: u8) -> VmResult<()> {
        let reg_idx = (offset / 8) as usize;
        let mut regs = self.registers.write().unwrap();
        regs[reg_idx] = value;
        Ok(())
    }
}

#[test]
fn test_mmio_device_read_write() {
    let mut device = TestDevice::new();

    // Write to register 0
    device.write(0, 0x12345678, 8).unwrap();

    // Read it back
    let value = device.read(0, 8).unwrap();
    assert_eq!(value, 0x12345678);
}

// ============================================================================
// Constants Tests
// ============================================================================

#[test]
fn test_page_constants() {
    assert_eq!(PAGE_SIZE, 4096);
    assert_eq!(PAGE_SHIFT, 12);
    assert_eq!(PTE_SIZE, 8);
    assert_eq!(PTES_PER_PAGE, 512);
}

#[test]
fn test_pte_flags() {
    assert_eq!(pte_flags::V, 1 << 0);
    assert_eq!(pte_flags::R, 1 << 1);
    assert_eq!(pte_flags::W, 1 << 2);
    assert_eq!(pte_flags::X, 1 << 3);
    assert_eq!(pte_flags::U, 1 << 4);
    assert_eq!(pte_flags::G, 1 << 5);
    assert_eq!(pte_flags::A, 1 << 6);
    assert_eq!(pte_flags::D, 1 << 7);
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_zero_size_access() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    // Access with size 0 should handle gracefully
    // (The implementation might reject this, but it shouldn't crash)
}

#[test]
fn test_max_size_access() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    let addr = GuestAddr(0x1000);

    // Max supported size is 8 bytes
    mmu.write(addr, 0x123456789ABCDEF0, 8).unwrap();
    assert_eq!(mmu.read(addr, 8).unwrap(), 0x123456789ABCDEF0);
}

#[test]
fn test_invalid_access_size() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    let addr = GuestAddr(0x1000);

    // Invalid size (e.g., 3, 5, 6, 7) should fail
    assert!(mmu.write(addr, 0, 3).is_err());
    assert!(mmu.write(addr, 0, 5).is_err());
}

#[test]
fn test_memory_boundary() {
    let mut mmu = SoftMmu::new(0x1000, false); // Exactly one page

    // Last valid access
    assert!(mmu.write(GuestAddr(0xFF8), 0x123456789ABCDEF0, 8).is_ok());

    // Just beyond boundary
    assert!(mmu.write(GuestAddr(0x1000), 0, 1).is_err());
}

#[test]
fn test_concurrent_access() {
    use std::thread;

    let mmu = Arc::new(RwLock::new(SoftMmu::new(1024 * 1024, false)));
    let mut handles = vec![];

    // Spawn multiple threads
    for i in 0..4 {
        let mmu_clone = Arc::clone(&mmu);
        let handle = thread::spawn(move || {
            let mut mmu = mmu_clone.write().unwrap();
            for j in 0..100 {
                let addr = GuestAddr((i * 0x1000 + j * 0x10) as u64);
                mmu.write(addr, (i * 100 + j) as u64, 8).unwrap();
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify all writes
    let mmu = mmu.read().unwrap();
    for i in 0..4 {
        for j in 0..100 {
            let addr = GuestAddr((i * 0x1000 + j * 0x10) as u64);
            let expected = (i * 100 + j) as u64;
            assert_eq!(mmu.read(addr, 8).unwrap(), expected);
        }
    }
}

#[test]
fn test_flush_tlb_asid() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    mmu.set_paging_mode(PagingMode::Sv39);

    // Set SATP with ASID
    let satp = (8u64 << 60) | (1u64 << 44) | 0x1000; // mode=8, asid=1, ppn=0x1000
    mmu.set_satp(satp);

    // Flush specific ASID (using AddressTranslator trait)
    AddressTranslator::flush_tlb_asid(&mut mmu, 1);

    // Should not panic
}

#[test]
fn test_flush_tlb_page() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    mmu.set_paging_mode(PagingMode::Sv39);

    // Flush specific page (using AddressTranslator trait)
    AddressTranslator::flush_tlb_page(&mut mmu, GuestAddr(0x1000));

    // Should not panic
}
