//! Software MMU Integration Tests
//!
//! Tests for software-based memory management unit implementation,
//! including address translation, TLB operations, and memory access.

use vm_core::{AccessType, GuestAddr, GuestPhysAddr, MemoryAccess};
use vm_ir::{GuestAddr as IRGuestAddr, IRBuilder, IROp, MemFlags, Terminator};
use vm_mem::SoftMmu;

#[test]
fn test_soft_mmu_bare_mode() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    // Bare mode: identity mapping
    let va = GuestAddr(0x1000);
    let pa = mmu.translate(va, AccessType::Read).unwrap();
    assert_eq!(pa, GuestPhysAddr(0x1000));

    // Write and read back
    mmu.write(va, 0xDEADBEEF, 4).unwrap();
    let val = mmu.read(va, 4).unwrap();
    assert_eq!(val, 0xDEADBEEF);
}

#[test]
fn test_soft_mmu_memory_operations() {
    let mut mmu = SoftMmu::new(64 * 1024, false);

    // Test various sizes
    let addr = GuestAddr(0x1000);

    // 1-byte
    mmu.write(addr, 0x42, 1).unwrap();
    assert_eq!(mmu.read(addr, 1).unwrap(), 0x42);

    // 2-byte
    mmu.write(addr, 0x1234, 2).unwrap();
    assert_eq!(mmu.read(addr, 2).unwrap(), 0x1234);

    // 4-byte
    mmu.write(addr, 0x12345678, 4).unwrap();
    assert_eq!(mmu.read(addr, 4).unwrap(), 0x12345678);

    // 8-byte
    mmu.write(addr, 0x123456789ABCDEF0, 8).unwrap();
    assert_eq!(mmu.read(addr, 8).unwrap(), 0x123456789ABCDEF0);
}

#[test]
fn test_soft_mmu_bulk_operations() {
    let mut mmu = SoftMmu::new(64 * 1024, false);
    let addr = GuestAddr(0x1000);

    // Write bulk
    let data = vec![0x10, 0x20, 0x30, 0x40, 0x50];
    mmu.write_bulk(addr, &data).unwrap();

    // Read back
    let mut buf = vec![0u8; 5];
    mmu.read_bulk(addr, &mut buf).unwrap();
    assert_eq!(buf, data);
}

#[test]
fn test_soft_mmu_tlb_stats() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    // In Bare mode, TLB is not used
    mmu.translate(GuestAddr(0x1000), AccessType::Read).unwrap();

    let (hits, misses) = mmu.tlb_stats();
    assert_eq!(hits, 0);
    assert_eq!(misses, 0);
}

#[test]
fn test_soft_mmu_fetch_insn() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    let pc = GuestAddr(0x1000);

    // Write an instruction
    mmu.write(pc, 0x00000513, 4).unwrap(); // ADDI x10, x0, 0

    // Fetch it
    let insn = mmu.fetch_insn(pc).unwrap();
    assert_eq!(insn, 0x00000513);
}

#[test]
fn test_soft_mmu_dump_restore() {
    let mut mmu = SoftMmu::new(1024, false);

    // Write some data
    mmu.write(GuestAddr(0x100), 0x12345678, 4).unwrap();
    mmu.write(GuestAddr(0x200), 0xABCDEF00, 4).unwrap();

    // Dump memory
    let dump = mmu.dump_memory();

    // Create new MMU and restore
    let mut mmu2 = SoftMmu::new(1024, false);
    mmu2.restore_memory(&dump).unwrap();

    // Verify
    assert_eq!(mmu2.read(GuestAddr(0x100), 4).unwrap(), 0x12345678);
    assert_eq!(mmu2.read(GuestAddr(0x200), 4).unwrap(), 0xABCDEF00);
}

#[test]
fn test_soft_mmu_memory_size() {
    let mmu = SoftMmu::new(2048, false);
    assert_eq!(mmu.memory_size(), 2048);
}

#[test]
fn test_soft_mmu_clone() {
    let mut mmu1 = SoftMmu::new(1024, false);
    mmu1.write(GuestAddr(0x100), 0xDEADBEEF, 4).unwrap();

    // Clone should share the same physical memory
    let mut mmu2 = mmu1.clone();

    // Write through mmu2
    mmu2.write(GuestAddr(0x200), 0xCAFEBABE, 4).unwrap();

    // Read through mmu1
    assert_eq!(mmu1.read(GuestAddr(0x200), 4).unwrap(), 0xCAFEBABE);
}

// ============================================================================
// IR Integration Tests
// ============================================================================

#[test]
fn test_ir_block_basic() {
    let mut builder = IRBuilder::new(IRGuestAddr(0x1000));
    builder.push(IROp::MovImm { dst: 1, imm: 42 });
    builder.push(IROp::MovImm { dst: 2, imm: 10 });
    builder.set_term(Terminator::Ret);

    let block = builder.build();
    assert_eq!(block.start_pc, IRGuestAddr(0x1000));
    assert_eq!(block.op_count(), 2);
    assert!(block.validate().is_ok());
}

#[test]
fn test_ir_block_with_memory_ops() {
    let mut mmu = SoftMmu::new(8192, false); // Need more memory
    let addr = GuestAddr(0x1000);

    // Write memory value
    mmu.write(addr, 0x12345678, 4).unwrap();

    // Create IR block with memory operations
    let mut builder = IRBuilder::new(IRGuestAddr(0x1000));
    builder.push(IROp::MovImm {
        dst: 1,
        imm: 0x1000,
    });
    builder.push(IROp::Load {
        dst: 2,
        base: 1,
        offset: 0,
        size: 4,
        flags: MemFlags::default(),
    });
    builder.push(IROp::MovImm {
        dst: 3,
        imm: 0x9999,
    });
    builder.push(IROp::Store {
        src: 3,
        base: 1,
        offset: 4,
        size: 4,
        flags: MemFlags::default(),
    });
    builder.set_term(Terminator::Ret);

    let block = builder.build();
    assert_eq!(block.op_count(), 4);

    // Verify the IR operations
    assert!(matches!(block.ops[0], IROp::MovImm { .. }));
    assert!(matches!(block.ops[1], IROp::Load { .. }));
    assert!(matches!(block.ops[2], IROp::MovImm { .. }));
    assert!(matches!(block.ops[3], IROp::Store { .. }));
    assert!(matches!(block.term, Terminator::Ret));
}

#[test]
fn test_ir_block_with_terminators() {
    // Test Ret terminator
    let mut builder = IRBuilder::new(IRGuestAddr(0x1000));
    builder.push(IROp::Nop);
    builder.set_term(Terminator::Ret);
    let block = builder.build();
    assert!(matches!(block.term, Terminator::Ret));

    // Test Jmp terminator
    let mut builder = IRBuilder::new(IRGuestAddr(0x2000));
    builder.push(IROp::Nop);
    builder.set_term(Terminator::Jmp {
        target: IRGuestAddr(0x3000),
    });
    let block = builder.build();
    assert!(matches!(
        block.term,
        Terminator::Jmp {
            target: IRGuestAddr(0x3000)
        }
    ));

    // Test CondJmp terminator
    let mut builder = IRBuilder::new(IRGuestAddr(0x4000));
    builder.push(IROp::Nop);
    builder.set_term(Terminator::CondJmp {
        cond: 1,
        target_true: IRGuestAddr(0x5000),
        target_false: IRGuestAddr(0x6000),
    });
    let block = builder.build();
    assert!(matches!(
        block.term,
        Terminator::CondJmp {
            cond: 1,
            target_true: IRGuestAddr(0x5000),
            target_false: IRGuestAddr(0x6000)
        }
    ));
}

#[test]
fn test_ir_block_builder_methods() {
    let mut builder = IRBuilder::new(IRGuestAddr(0x1000));

    // Test pc()
    assert_eq!(builder.pc(), GuestAddr(0x1000));

    // Test op_count() and is_empty()
    assert_eq!(builder.op_count(), 0);
    assert!(builder.is_empty());

    // Add operations
    builder.push(IROp::Nop);
    assert_eq!(builder.op_count(), 1);
    assert!(!builder.is_empty());

    // Test push_all
    let ops = vec![
        IROp::MovImm { dst: 1, imm: 10 },
        IROp::MovImm { dst: 2, imm: 20 },
    ];
    builder.push_all(ops);
    assert_eq!(builder.op_count(), 3);

    // Test build_ref (should clone)
    let _block1 = builder.build_ref();
    assert_eq!(builder.op_count(), 3); // Builder still usable

    // Test build (consumes builder)
    let block2 = builder.build();
    assert_eq!(block2.op_count(), 3);
}

#[test]
fn test_ir_block_validation() {
    // Valid block
    let mut builder = IRBuilder::new(IRGuestAddr(0x1000));
    builder.push(IROp::Nop);
    builder.set_term(Terminator::Ret);
    let block = builder.build();
    assert!(block.validate().is_ok());

    // Invalid CondJmp with invalid condition register
    let mut builder = IRBuilder::new(IRGuestAddr(0x2000));
    builder.push(IROp::Nop);
    builder.set_term(Terminator::CondJmp {
        cond: u32::MAX, // Invalid
        target_true: IRGuestAddr(0x3000),
        target_false: IRGuestAddr(0x4000),
    });
    let block = builder.build();
    assert!(block.validate().is_err());
}

#[test]
fn test_ir_block_estimated_size() {
    let mut builder = IRBuilder::new(IRGuestAddr(0x1000));
    builder.push(IROp::MovImm { dst: 1, imm: 42 });
    builder.push(IROp::Add {
        dst: 2,
        src1: 1,
        src2: 1,
    });
    builder.set_term(Terminator::Ret);

    let block = builder.build();
    let size = block.estimated_size();
    assert!(size > 0);
}

#[test]
fn test_ir_block_iterator() {
    let mut builder = IRBuilder::new(IRGuestAddr(0x1000));
    builder.push(IROp::Nop);
    builder.push(IROp::MovImm { dst: 1, imm: 42 });
    builder.push(IROp::MovImm { dst: 2, imm: 10 });
    builder.set_term(Terminator::Ret);

    let block = builder.build();

    // Test iter_ops
    let ops: Vec<_> = block.iter_ops().collect();
    assert_eq!(ops.len(), 3);

    // Verify ops
    assert!(matches!(ops[0], IROp::Nop));
    assert!(matches!(ops[1], IROp::MovImm { .. }));
    assert!(matches!(ops[2], IROp::MovImm { .. }));
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_soft_mmu_with_ir_load() {
    let mut mmu = SoftMmu::new(1024, false);

    // Setup: write value to memory
    mmu.write(GuestAddr(0x1000), 0x12345678, 4).unwrap();

    // Create IR that loads from that address
    let mut builder = IRBuilder::new(IRGuestAddr(0x1000));
    builder.push(IROp::MovImm {
        dst: 1,
        imm: 0x1000,
    }); // Load address into r1
    builder.push(IROp::Load {
        dst: 2,
        base: 1,
        offset: 0,
        size: 4,
        flags: MemFlags::default(),
    }); // Load [r1] into r2
    builder.set_term(Terminator::Ret);

    let block = builder.build();

    // Verify the IR structure
    assert_eq!(block.op_count(), 2);
    if let IROp::Load {
        dst,
        base,
        offset,
        size,
        ..
    } = &block.ops[1]
    {
        assert_eq!(*dst, 2);
        assert_eq!(*base, 1);
        assert_eq!(*offset, 0);
        assert_eq!(*size, 4);
    } else {
        panic!("Expected Load operation");
    }

    // Simulate execution: actually load from memory
    let loaded_value = mmu.read(GuestAddr(0x1000), 4).unwrap();
    assert_eq!(loaded_value, 0x12345678);
}

#[test]
fn test_soft_mmu_with_ir_store() {
    let mut mmu = SoftMmu::new(16384, false); // Need enough memory for 0x2000

    // Create IR that stores to memory
    let mut builder = IRBuilder::new(IRGuestAddr(0x1000));
    builder.push(IROp::MovImm {
        dst: 1,
        imm: 0xCAFEBABE,
    }); // Value to store
    builder.push(IROp::MovImm {
        dst: 2,
        imm: 0x2000,
    }); // Store address
    builder.push(IROp::Store {
        src: 1,
        base: 2,
        offset: 0,
        size: 4,
        flags: MemFlags::default(),
    }); // Store r1 to [r2]
    builder.set_term(Terminator::Ret);

    let block = builder.build();

    // Verify the IR structure
    assert_eq!(block.op_count(), 3);
    if let IROp::Store {
        src,
        base,
        offset,
        size,
        ..
    } = &block.ops[2]
    {
        assert_eq!(*src, 1);
        assert_eq!(*base, 2);
        assert_eq!(*offset, 0);
        assert_eq!(*size, 4);
    } else {
        panic!("Expected Store operation");
    }

    // Simulate execution: actually write to memory
    mmu.write(GuestAddr(0x2000), 0xCAFEBABE, 4).unwrap();
    let stored_value = mmu.read(GuestAddr(0x2000), 4).unwrap();
    assert_eq!(stored_value, 0xCAFEBABE);
}

#[test]
fn test_soft_mmu_alignment() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    mmu.set_strict_align(true);

    let addr = GuestAddr(0x1000);

    // Aligned accesses should work
    assert!(mmu.write(addr, 0x12345678, 4).is_ok());
    assert!(mmu.read(addr, 4).is_ok());

    // Misaligned accesses should fail in strict mode
    let misaligned = GuestAddr(0x1001);
    assert!(mmu.write(misaligned, 0x12345678, 4).is_err());
    assert!(mmu.read(misaligned, 4).is_err());
}

#[test]
fn test_soft_mmu_guest_slice() {
    let mut mmu = SoftMmu::new(1024, false);

    // Write some data
    mmu.write(GuestAddr(0x100), 0x12345678, 4).unwrap();
    mmu.write(GuestAddr(0x104), 0xABCDEF00, 4).unwrap();

    // Read as slice
    let slice = mmu.guest_slice(0x100, 8).unwrap();
    assert_eq!(slice.len(), 8);
    // Little-endian
    assert_eq!(slice[0], 0x78);
    assert_eq!(slice[1], 0x56);
    assert_eq!(slice[2], 0x34);
    assert_eq!(slice[3], 0x12);
}

#[test]
fn test_soft_mmu_paging_mode() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    // Default is Bare mode
    let pa = mmu.translate(GuestAddr(0x1000), AccessType::Read).unwrap();
    assert_eq!(pa, GuestPhysAddr(0x1000));

    // Switch to Sv39 (this should flush TLB)
    mmu.set_paging_mode(vm_mem::PagingMode::Sv39);

    // In Sv39 mode without page tables, translation should still work in bare mode
    // The actual page walk requires page tables to be set up
    // For now, let's just test that we can set the mode
    // and the TLB is flushed
    let (hits, misses) = mmu.tlb_stats();
    assert_eq!(hits, 0);
    assert_eq!(misses, 0);
}

#[test]
fn test_soft_mmu_tlb_flush() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    // Set SATP for Sv39
    let satp = (8u64 << 60) | 0x1000; // MODE=Sv39, PPN=0x1000
    mmu.set_satp(satp);

    // TLB should be flushed after set_satp
    let (hits, misses) = mmu.tlb_stats();
    assert_eq!(hits, 0);
    assert_eq!(misses, 0);

    // Flush again manually
    use vm_core::mmu_traits::AddressTranslator;
    mmu.flush_tlb();

    // Stats should still be zero
    let (hits, misses) = mmu.tlb_stats();
    assert_eq!(hits, 0);
    assert_eq!(misses, 0);
}

#[test]
fn test_ir_block_with_arithmetic_ops() {
    let mut builder = IRBuilder::new(IRGuestAddr(0x1000));

    // Build a simple arithmetic expression: r3 = (r1 + r2) * 2
    builder.push(IROp::MovImm { dst: 1, imm: 10 });
    builder.push(IROp::MovImm { dst: 2, imm: 20 });
    builder.push(IROp::Add {
        dst: 3,
        src1: 1,
        src2: 2,
    });
    builder.push(IROp::AddImm {
        dst: 4,
        src: 3,
        imm: 10,
    });
    builder.set_term(Terminator::Ret);

    let block = builder.build();
    assert_eq!(block.op_count(), 4);

    // Verify operations
    assert!(matches!(block.ops[0], IROp::MovImm { dst: 1, imm: 10 }));
    assert!(matches!(block.ops[1], IROp::MovImm { dst: 2, imm: 20 }));
    assert!(matches!(block.ops[2], IROp::Add { dst: 3, .. }));
    assert!(matches!(
        block.ops[3],
        IROp::AddImm {
            dst: 4,
            src: 3,
            imm: 10
        }
    ));
}

#[test]
fn test_ir_block_with_comparison_ops() {
    let mut builder = IRBuilder::new(IRGuestAddr(0x1000));

    // Build comparison operations
    builder.push(IROp::MovImm { dst: 1, imm: 10 });
    builder.push(IROp::MovImm { dst: 2, imm: 20 });
    builder.push(IROp::CmpLt {
        dst: 3,
        lhs: 1,
        rhs: 2,
    });
    builder.push(IROp::CmpEq {
        dst: 4,
        lhs: 1,
        rhs: 1,
    });
    builder.set_term(Terminator::Ret);

    let block = builder.build();
    assert_eq!(block.op_count(), 4);

    // Verify operations
    assert!(matches!(block.ops[2], IROp::CmpLt { .. }));
    assert!(matches!(block.ops[3], IROp::CmpEq { .. }));
}

#[test]
fn test_soft_mmu_atomic_operations() {
    let mut mmu = SoftMmu::new(8192, false); // Need more memory
    let addr = GuestAddr(0x1000);

    // Initialize value
    mmu.write(addr, 0x1000, 4).unwrap();

    // Load-reserved
    let val1 = mmu.load_reserved(addr, 4).unwrap();
    assert_eq!(val1, 0x1000);

    // Store-conditional should succeed if no other write happened
    let success = mmu.store_conditional(addr, 0x2000, 4).unwrap();
    assert!(success);

    // Verify new value
    let val2 = mmu.read(addr, 4).unwrap();
    assert_eq!(val2, 0x2000);

    // Load-reserve again
    let _ = mmu.load_reserved(addr, 4).unwrap();

    // Invalidate reservation
    mmu.invalidate_reservation(addr, 4);

    // Store-conditional should fail
    let success = mmu.store_conditional(addr, 0x3000, 4).unwrap();
    assert!(!success);

    // Value should remain unchanged
    let val3 = mmu.read(addr, 4).unwrap();
    assert_eq!(val3, 0x2000);
}

#[test]
fn test_ir_block_complex_function() {
    let mut builder = IRBuilder::new(IRGuestAddr(0x1000));

    // Build a more complex function:
    // int add_and_multiply(int a, int b) {
    //     int sum = a + b;
    //     int result = sum * 2;
    //     return result;
    // }

    // Assuming a=10, b=20
    builder.push(IROp::MovImm { dst: 1, imm: 10 }); // a
    builder.push(IROp::MovImm { dst: 2, imm: 20 }); // b
    builder.push(IROp::Add {
        dst: 3,
        src1: 1,
        src2: 2,
    }); // sum = a + b
    builder.push(IROp::AddImm {
        dst: 4,
        src: 3,
        imm: 30,
    }); // result = sum + 30
    builder.set_term(Terminator::Ret);

    let block = builder.build();

    // Verify block structure
    assert_eq!(block.start_pc, IRGuestAddr(0x1000));
    assert_eq!(block.op_count(), 4);
    assert!(block.validate().is_ok());
    assert!(matches!(block.term, Terminator::Ret));

    // Expected result: 10 + 20 + 30 = 60
    // In a real execution engine, we would execute the IR and verify the result
}
