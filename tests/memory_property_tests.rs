//! Memory Property-Based Tests
//!
//! This module contains property-based tests for the memory management system using proptest.
//! These tests verify fundamental invariants and properties that should always hold true
//! regardless of the input values.

use proptest::prelude::*;
use proptest_derive::Arbitrary;
use std::sync::Arc;
use vm_core::{GuestAddr, GuestPhysAddr, MemoryError, VmError};
use vm_mem::{MemoryPool, PAGE_SIZE, PagingMode};

// ============================================================================
// Test Data Structures
// ============================================================================

/// Memory operation type for testing
#[derive(Debug, Clone, Copy, Arbitrary)]
enum MemOp {
    Read,
    Write,
    ReadWrite,
}

/// Memory region descriptor
#[derive(Debug, Clone)]
struct MemRegion {
    base: u64,
    size: u64,
    data: Vec<u8>,
}

impl MemRegion {
    fn new(base: u64, size: usize) -> Self {
        Self {
            base,
            size: size as u64,
            data: vec![0u8; size],
        }
    }

    fn contains(&self, addr: u64) -> bool {
        addr >= self.base && addr < self.base + self.size
    }

    fn is_aligned(&self) -> bool {
        self.base % PAGE_SIZE == 0 && self.size % PAGE_SIZE == 0
    }
}

// ============================================================================
// Property 1: Read-After-Write Consistency
// ============================================================================

proptest! {
    /// Property: Reading from an address after writing should return the written value
    ///
    /// This is a fundamental memory consistency property. When we write data to a specific
    /// address and then immediately read from that same address, we should get back exactly
    /// what we wrote.
    #[test]
    fn prop_read_after_write_consistency(
        addr in 0usize..(1usize << 20), // Up to 1MB address space
        value in any::<[u8; 8]>(),
    ) {
        prop_assume!(addr + 8 <= (1usize << 20));

        // Create a simple memory pool for testing
        let pool = MemoryPool::new(1 << 20); // 1MB pool

        // Write the value
        let write_result = pool.write(GuestAddr(addr as u64), &value);
        prop_assert!(write_result.is_ok(), "Write should succeed");

        // Read back the value
        let mut read_buffer = [0u8; 8];
        let read_result = pool.read(GuestAddr(addr as u64), &mut read_buffer);
        prop_assert!(read_result.is_ok(), "Read should succeed");

        // Verify consistency
        prop_assert_eq!(
            &read_buffer[..],
            &value[..],
            "Read value should match written value"
        );
    }
}

// ============================================================================
// Property 2: Write-Then-Read Consistency
// ============================================================================

proptest! {
    /// Property: Writing data and then reading it should preserve the exact data
    ///
    /// This property tests that memory writes are stored correctly and can be
    /// retrieved without corruption. It tests various data sizes and alignments.
    #[test]
    fn prop_write_read_preserves_data(
        addr in 0usize..(1usize << 20),
        data in prop::collection::vec(any::<u8>(), 1..256),
    ) {
        let end_addr = addr.checked_add(data.len()).unwrap();
        prop_assume!(end_addr <= (1usize << 20));

        let pool = MemoryPool::new(1 << 20);

        // Write data
        let write_result = pool.write(GuestAddr(addr as u64), &data);
        prop_assert!(write_result.is_ok(), "Write should succeed for valid range");

        // Read back
        let mut read_buffer = vec![0u8; data.len()];
        let read_result = pool.read(GuestAddr(addr as u64), &mut read_buffer);
        prop_assert!(read_result.is_ok(), "Read should succeed for valid range");

        // Verify data integrity
        prop_assert_eq!(
            read_buffer,
            data,
            "Read data should exactly match written data"
        );
    }
}

// ============================================================================
// Property 3: Cross-Boundary Access
// ============================================================================

proptest! {
    /// Property: Memory operations crossing page boundaries should be handled correctly
    ///
    /// This tests that the memory system correctly handles operations that span
    /// multiple pages, which is a common source of bugs in memory management.
    #[test]
    fn prop_cross_boundary_access(
        page_idx in 0usize..100usize,
        offset_in_page in 0usize..4096usize,
        size in 1usize..8192usize, // Can span multiple pages
    ) {
        let addr = page_idx * 4096 + offset_in_page;
        let pool_size = 101 * 4096; // Enough for 100 pages + buffer
        prop_assume!(addr + size <= pool_size);

        let pool = MemoryPool::new(pool_size as u64);

        // Create test data
        let data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();

        // Write across boundaries
        let write_result = pool.write(GuestAddr(addr as u64), &data);
        prop_assert!(
            write_result.is_ok(),
            "Cross-boundary write should succeed"
        );

        // Read back
        let mut read_buffer = vec![0u8; size];
        let read_result = pool.read(GuestAddr(addr as u64), &mut read_buffer);
        prop_assert!(
            read_result.is_ok(),
            "Cross-boundary read should succeed"
        );

        // Verify
        prop_assert_eq!(read_buffer, data, "Cross-boundary data should be preserved");
    }
}

// ============================================================================
// Property 4: Batch Operations Equivalence
// ============================================================================

proptest! {
    /// Property: Batch operations should be equivalent to individual operations
    ///
    /// This tests that performing multiple small operations produces the same
    /// result as one large operation, which is important for optimization correctness.
    #[test]
    fn prop_batch_equivalence(
        base_addr in 0usize..(1usize << 20),
        chunk_sizes in prop::collection::vec(1usize..1024usize, 2..10),
    ) {
        let total_size: usize = chunk_sizes.iter().sum();
        let end_addr = base_addr.checked_add(total_size).unwrap();
        prop_assume!(end_addr <= (1usize << 20));

        let pool1 = MemoryPool::new(1 << 20);
        let pool2 = MemoryPool::new(1 << 20);

        // Create test data
        let data: Vec<u8> = (0..total_size).map(|i| (i % 256) as u8).collect();

        // Approach 1: Single large write
        let write1 = pool1.write(GuestAddr(base_addr as u64), &data);
        prop_assert!(write1.is_ok());

        // Approach 2: Multiple chunked writes
        let mut offset = 0;
        for chunk_size in chunk_sizes {
            let chunk_data = &data[offset..offset + chunk_size];
            let write2 = pool2.write(GuestAddr((base_addr + offset) as u64), chunk_data);
            prop_assert!(write2.is_ok(), "Chunked write should succeed");
            offset += chunk_size;
        }

        // Verify both approaches produce same result
        let mut buffer1 = vec![0u8; total_size];
        let mut buffer2 = vec![0u8; total_size];

        let read1 = pool1.read(GuestAddr(base_addr as u64), &mut buffer1);
        let read2 = pool2.read(GuestAddr(base_addr as u64), &mut buffer2);

        prop_assert!(read1.is_ok() && read2.is_ok());
        prop_assert_eq!(buffer1, buffer2, "Single write should equal chunked writes");
    }
}

// ============================================================================
// Property 5: Independence of Non-Overlapping Regions
// ============================================================================

proptest! {
    /// Property: Operations on non-overlapping memory regions should be independent
    ///
    /// This tests that writing to one region doesn't affect another region,
    /// which is a fundamental isolation property.
    #[test]
    fn prop_region_independence(
        addr1 in 0usize..(1usize << 19),
        addr2 in 0usize..(1usize << 19),
        size in 1usize..4096usize,
    ) {
        // Ensure regions don't overlap
        let end1 = addr1.checked_add(size).unwrap();
        let end2 = addr2.checked_add(size).unwrap();
        prop_assume!(end1 <= addr2 || end2 <= addr1);

        let pool = MemoryPool::new(1 << 20);

        // Write different patterns to each region
        let data1: Vec<u8> = vec![0xAA; size];
        let data2: Vec<u8> = vec![0xBB; size];

        pool.write(GuestAddr(addr1 as u64), &data1).unwrap();
        pool.write(GuestAddr(addr2 as u64), &data2).unwrap();

        // Read back both regions
        let mut buffer1 = vec![0u8; size];
        let mut buffer2 = vec![0u8; size];

        pool.read(GuestAddr(addr1 as u64), &mut buffer1).unwrap();
        pool.read(GuestAddr(addr2 as u64), &mut buffer2).unwrap();

        // Verify independence
        prop_assert_eq!(buffer1, data1, "Region 1 should contain only its data");
        prop_assert_eq!(buffer2, data2, "Region 2 should contain only its data");
        prop_assert!(buffer1.iter().all(|&b| b == 0xAA));
        prop_assert!(buffer2.iter().all(|&b| b == 0xBB));
    }
}

// ============================================================================
// Property 6: Address Alignment
// ============================================================================

proptest! {
    /// Property: Memory operations should work correctly at various alignments
    ///
    /// This tests that the memory system handles unaligned accesses correctly,
    /// which is crucial for architectures that support unaligned memory access.
    #[test]
    fn varrious_alignments(
        addr in 0usize..(1usize << 20),
        alignment in 1usize..16usize, // Test alignments from 1 to 16 bytes
        size in 1usize..128usize,
    ) {
        // Align address
        let aligned_addr = addr & !(alignment - 1);
        prop_assume!(aligned_addr % alignment == 0);

        let end_addr = aligned_addr.checked_add(size).unwrap();
        prop_assume!(end_addr <= (1usize << 20));

        let pool = MemoryPool::new(1 << 20);

        // Create patterned data based on alignment
        let data: Vec<u8> = (0..size)
            .map(|i| ((aligned_addr + i) % 256) as u8)
            .collect();

        // Write and read back
        pool.write(GuestAddr(aligned_addr as u64), &data).unwrap();

        let mut buffer = vec![0u8; size];
        pool.read(GuestAddr(aligned_addr as u64), &mut buffer).unwrap();

        prop_assert_eq!(buffer, data, "Data should be preserved at any alignment");
    }
}

// ============================================================================
// Property 7: Zero Preservation
// ============================================================================

proptest! {
    /// Property: Unwritten memory should read as zeros
    ///
    /// This tests that newly allocated memory is zero-initialized, which is
    /// a security and correctness requirement.
    #[test]
    fn prop_zero_initialization(
        addr in 0usize..(1usize << 20),
        size in 1usize..4096usize,
    ) {
        let end_addr = addr.checked_add(size).unwrap();
        prop_assume!(end_addr <= (1usize << 20));

        let pool = MemoryPool::new(1 << 20);

        // Read without writing
        let mut buffer = vec![0u8; size];
        let read_result = pool.read(GuestAddr(addr as u64), &mut buffer);

        prop_assert!(read_result.is_ok(), "Read should succeed");

        // Verify it's all zeros
        prop_assert!(
            buffer.iter().all(|&b| b == 0),
            "Unwritten memory should be zero-initialized"
        );
    }
}

// ============================================================================
// Property 8: Multiple Overwrite Consistency
// ============================================================================

proptest! {
    /// Property: Multiple writes to the same location should preserve the last value
    ///
    /// This tests that overwriting memory correctly replaces the old value
    /// without affecting other locations.
    #[test]
    fn prop_multiple_overwrites(
        addr in 0usize..(1usize << 20),
        iterations in 2usize..10usize,
    ) {
        let pool = MemoryPool::new(1 << 20);

        let mut expected_value = [0u8; 8];

        // Perform multiple overwrites
        for i in 0..iterations {
            expected_value = [(i % 256) as u8; 8];
            pool.write(GuestAddr(addr as u64), &expected_value).unwrap();
        }

        // Final read should get the last written value
        let mut buffer = [0u8; 8];
        pool.read(GuestAddr(addr as u64), &mut buffer).unwrap();

        prop_assert_eq!(
            buffer,
            expected_value,
            "Last write should be preserved"
        );
    }
}

// ============================================================================
// Property 9: Large Data Transfer
// ============================================================================

proptest! {
    /// Property: Large data transfers should complete without corruption
    ///
    /// This tests the memory system's ability to handle large contiguous transfers,
    /// which is important for DMA-like operations.
    #[test]
    fn prop_large_transfer(
        addr in 0usize..(1usize << 20),
        size in 16384usize..65536usize, // 16KB to 64KB
    ) {
        let end_addr = addr.checked_add(size).unwrap();
        prop_assume!(end_addr <= (1usize << 20));

        let pool = MemoryPool::new(1 << 20);

        // Create patterned data for easy verification
        let data: Vec<u8> = (0..size)
            .map(|i| ((addr + i) % 256) as u8)
            .collect();

        // Write large block
        pool.write(GuestAddr(addr as u64), &data).unwrap();

        // Read back
        let mut buffer = vec![0u8; size];
        pool.read(GuestAddr(addr as u64), &mut buffer).unwrap();

        // Verify entire block
        prop_assert_eq!(buffer, data, "Large transfer should preserve data");
    }
}

// ============================================================================
// Property 10: Memory Pool Bounds
// ============================================================================

proptest! {
    /// Property: Operations within pool bounds should succeed, outside should fail
    ///
    /// This tests proper bounds checking, which is critical for memory safety.
    #[test]
    fn prop_bounds_checking(
        addr in 0usize..(1usize << 21),
        size in 1usize..4096usize,
    ) {
        let pool_size = 1usize << 20; // 1MB
        let pool = MemoryPool::new(pool_size as u64);

        let data = vec![0xABu8; size];
        let write_result = pool.write(GuestAddr(addr as u64), &data);

        let end_addr = addr.checked_add(size).unwrap();

        if end_addr <= pool_size {
            // Within bounds: should succeed
            prop_assert!(write_result.is_ok(), "Write within bounds should succeed");
        } else {
            // Out of bounds: should fail
            prop_assert!(
                write_result.is_err(),
                "Write out of bounds should fail"
            );
        }
    }
}
