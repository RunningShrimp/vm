//! VM-Mem Integration Tests
//!
//! Comprehensive integration tests for vm-mem that verify:
//! - Memory allocation and deallocation
//! - Address translation (virtual → physical)
//! - TLB functionality
//! - Page table operations
//! - NUMA-aware memory management
//! - Cross-component integration with vm-core
//! - Memory pool operations

use std::sync::Arc;

use vm_core::{AccessType, GuestAddr, GuestPhysAddr, MMU, MemoryAccess, MemoryError, PageSize};
use vm_mem::{
    memory::{self, MemoryPool},
    numa::{NumaAllocPolicy, NumaAllocator, NumaNode},
    paging_mode::{PagingMode, Sv39PageTable, Sv48PageTable},
    tlb::{Tlb, TlbEntry, TlbStats},
    unified_mmu::{UnifiedMmu, UnifiedMmuConfig},
};

// ============================================================================
// Test Helpers
// ============================================================================

/// Default memory size for tests (4 MB)
const DEFAULT_MEMORY_SIZE: usize = 4 * 1024 * 1024;

/// Default page size (4 KB)
const PAGE_SIZE: usize = 4096;

/// Create a test memory pool
fn create_test_pool(size: usize) -> MemoryPool {
    MemoryPool::new(size, PAGE_SIZE).expect("Failed to create memory pool")
}

/// Create a test TLB
fn create_test_tlb(entries: usize) -> Tlb<GuestAddr, GuestPhysAddr> {
    Tlb::new(entries)
}

/// Create a test NUMA allocator
fn create_test_numa(num_nodes: usize, policy: NumaAllocPolicy) -> NumaAllocator {
    NumaAllocator::new(num_nodes, policy)
}

// ============================================================================
// Memory Pool Integration Tests
// ============================================================================

#[cfg(test)]
mod memory_pool_integration {
    use super::*;

    /// Test memory pool creation and basic operations
    #[test]
    fn test_memory_pool_creation() {
        let pool = create_test_pool(DEFAULT_MEMORY_SIZE);

        assert_eq!(pool.size(), DEFAULT_MEMORY_SIZE);
        assert_eq!(pool.page_size(), PAGE_SIZE);
    }

    /// Test memory allocation and deallocation
    #[test]
    fn test_memory_allocation() {
        let mut pool = create_test_pool(DEFAULT_MEMORY_SIZE);

        // Allocate some memory
        let addr1 = pool.allocate(PAGE_SIZE).expect("Allocation should succeed");
        let addr2 = pool.allocate(PAGE_SIZE).expect("Allocation should succeed");

        assert_ne!(
            addr1, addr2,
            "Allocations should return different addresses"
        );

        // Write to allocated memory
        pool.write(addr1, &[1, 2, 3, 4])
            .expect("Write should succeed");
        pool.write(addr2, &[5, 6, 7, 8])
            .expect("Write should succeed");

        // Read back and verify
        let data1 = pool.read(addr1, 4).expect("Read should succeed");
        let data2 = pool.read(addr2, 4).expect("Read should succeed");

        assert_eq!(data1, vec![1, 2, 3, 4]);
        assert_eq!(data2, vec![5, 6, 7, 8]);

        // Deallocate
        pool.deallocate(addr1, PAGE_SIZE)
            .expect("Deallocation should succeed");
    }

    /// Test multiple sequential allocations
    #[test]
    fn test_multiple_allocations() {
        let mut pool = create_test_pool(DEFAULT_MEMORY_SIZE);

        let mut addrs = Vec::new();

        for _ in 0..10 {
            let addr = pool.allocate(PAGE_SIZE).expect("Allocation should succeed");
            addrs.push(addr);
        }

        // All allocations should be unique
        for (i, &addr1) in addrs.iter().enumerate() {
            for &addr2 in addrs.iter().skip(i + 1) {
                assert_ne!(addr1, addr2);
            }
        }
    }

    /// Test allocation failure on pool exhaustion
    #[test]
    fn test_pool_exhaustion() {
        let mut pool = create_test_pool(PAGE_SIZE * 10);

        // Allocate all pages
        let mut addrs = Vec::new();
        for _ in 0..10 {
            let addr = pool.allocate(PAGE_SIZE).expect("Allocation should succeed");
            addrs.push(addr);
        }

        // Next allocation should fail
        let result = pool.allocate(PAGE_SIZE);
        assert!(
            result.is_err(),
            "Allocation should fail when pool is exhausted"
        );
    }

    /// Test deallocation and reuse
    #[test]
    fn test_deallocation_reuse() {
        let mut pool = create_test_pool(PAGE_SIZE * 10);

        let addr1 = pool.allocate(PAGE_SIZE).expect("Allocation should succeed");
        pool.deallocate(addr1, PAGE_SIZE)
            .expect("Deallocation should succeed");

        // Should be able to allocate again
        let addr2 = pool.allocate(PAGE_SIZE).expect("Allocation should succeed");
        assert_eq!(addr1, addr2, "Deallocated memory should be reused");
    }
}

// ============================================================================
// TLB Integration Tests
// ============================================================================

#[cfg(test)]
mod tlb_integration {
    use super::*;

    /// Test TLB creation and basic operations
    #[test]
    fn test_tlb_creation() {
        let tlb = create_test_tlb(64);

        let stats = tlb.stats();
        assert_eq!(stats.entries, 0);
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
    }

    /// Test TLB insertion and lookup
    #[test]
    fn test_tlb_insert_lookup() {
        let mut tlb = create_test_tlb(64);

        let vaddr = GuestAddr(0x1000);
        let paddr = GuestPhysAddr(0x5000);
        let flags = 0x7; // Present, Readable, Writable

        tlb.insert(
            vaddr,
            TlbEntry {
                phys_addr: paddr,
                flags,
                access_count: 0,
            },
        );

        let result = tlb.lookup(vaddr, AccessType::Read);
        assert!(result.is_some(), "TLB lookup should find inserted entry");

        let entry = result.unwrap();
        assert_eq!(entry.phys_addr, paddr);
        assert_eq!(entry.flags, flags);
    }

    /// Test TLB miss on non-existent entry
    #[test]
    fn test_tlb_miss() {
        let mut tlb = create_test_tlb(64);

        let result = tlb.lookup(GuestAddr(0x1000), AccessType::Read);
        assert!(
            result.is_none(),
            "TLB lookup should return None for non-existent entry"
        );

        let stats = tlb.stats();
        assert_eq!(stats.misses, 1);
    }

    /// Test TLB hit tracking
    #[test]
    fn test_tlb_hit_tracking() {
        let mut tlb = create_test_tlb(64);

        let vaddr = GuestAddr(0x1000);
        tlb.insert(
            vaddr,
            TlbEntry {
                phys_addr: GuestPhysAddr(0x5000),
                flags: 0x7,
                access_count: 0,
            },
        );

        // Perform lookup
        let _ = tlb.lookup(vaddr, AccessType::Read);

        let stats = tlb.stats();
        assert_eq!(stats.hits, 1);
    }

    /// Test TLB invalidation
    #[test]
    fn test_tlb_invalidation() {
        let mut tlb = create_test_tlb(64);

        let vaddr = GuestAddr(0x1000);
        tlb.insert(
            vaddr,
            TlbEntry {
                phys_addr: GuestPhysAddr(0x5000),
                flags: 0x7,
                access_count: 0,
            },
        );

        // Invalidate
        tlb.invalidate(vaddr);

        // Lookup should now fail
        let result = tlb.lookup(vaddr, AccessType::Read);
        assert!(
            result.is_none(),
            "TLB lookup should fail after invalidation"
        );
    }

    /// Test TLB flush
    #[test]
    fn test_tlb_flush() {
        let mut tlb = create_test_tlb(64);

        // Insert multiple entries
        for i in 0..10 {
            let vaddr = GuestAddr(0x1000 + i * PAGE_SIZE as u64);
            tlb.insert(
                vaddr,
                TlbEntry {
                    phys_addr: GuestPhysAddr(0x5000 + i * PAGE_SIZE as u64),
                    flags: 0x7,
                    access_count: 0,
                },
            );
        }

        let stats_before = tlb.stats();
        assert_eq!(stats_before.entries, 10);

        // Flush all
        tlb.flush();

        let stats_after = tlb.stats();
        assert_eq!(stats_after.entries, 0);

        // All lookups should fail
        for i in 0..10 {
            let vaddr = GuestAddr(0x1000 + i * PAGE_SIZE as u64);
            let result = tlb.lookup(vaddr, AccessType::Read);
            assert!(result.is_none());
        }
    }

    /// Test TLB capacity and eviction
    #[test]
    fn test_tlb_capacity() {
        let mut tlb = create_test_tlb(4); // Small TLB with 4 entries

        // Insert 5 entries
        for i in 0..5 {
            let vaddr = GuestAddr(0x1000 + i * PAGE_SIZE as u64);
            tlb.insert(
                vaddr,
                TlbEntry {
                    phys_addr: GuestPhysAddr(0x5000 + i * PAGE_SIZE as u64),
                    flags: 0x7,
                    access_count: 0,
                },
            );
        }

        let stats = tlb.stats();
        // TLB should have at most 4 entries
        assert!(stats.entries <= 4, "TLB should respect capacity");
    }
}

// ============================================================================
// Unified MMU Integration Tests
// ============================================================================

#[cfg(test)]
mod unified_mmu_integration {
    use super::*;

    /// Create a test Unified MMU
    fn create_test_mmu(size: usize) -> UnifiedMmu {
        let config = UnifiedMmuConfig {
            memory_size: size,
            paging_mode: PagingMode::Sv39,
            enable_tlb: true,
            enable_huge_pages: false,
            tlb_entries: 64,
            ..Default::default()
        };

        UnifiedMmu::new(size, false, config)
    }

    /// Test Unified MMU creation
    #[test]
    fn test_unified_mmu_creation() {
        let mmu = create_test_mmu(DEFAULT_MEMORY_SIZE);
        assert_eq!(mmu.size(), DEFAULT_MEMORY_SIZE);
    }

    /// Test basic memory read/write through MMU
    #[test]
    fn test_mmu_memory_operations() {
        let mut mmu = create_test_mmu(DEFAULT_MEMORY_SIZE);

        let addr = GuestAddr(0x1000);
        let data = vec![1u8, 2, 3, 4, 5];

        // Write
        let write_result = mmu.write(addr, &data.len(), &data);
        assert!(write_result.is_ok(), "MMU write should succeed");

        // Read back
        let mut read_data = vec![0u8; data.len()];
        let read_result = mmu.read(addr, &data.len(), &mut read_data);
        assert!(read_result.is_ok(), "MMU read should succeed");

        assert_eq!(read_data, data);
    }

    /// Test multiple sequential operations
    #[test]
    fn test_mmu_sequential_operations() {
        let mut mmu = create_test_mmu(DEFAULT_MEMORY_SIZE);

        let operations = vec![
            (GuestAddr(0x1000), vec![1u8, 2, 3]),
            (GuestAddr(0x2000), vec![4u8, 5, 6]),
            (GuestAddr(0x3000), vec![7u8, 8, 9]),
        ];

        for (addr, data) in &operations {
            let _ = mmu.write(*addr, &data.len(), data);
        }

        for (addr, expected) in &operations {
            let mut data = vec![0u8; expected.len()];
            let _ = mmu.read(*addr, &expected.len(), &mut data);
            assert_eq!(data, *expected);
        }
    }

    /// Test boundary conditions
    #[test]
    fn test_mmu_boundaries() {
        let mut mmu = create_test_mmu(DEFAULT_MEMORY_SIZE);

        // Write at end of memory
        let last_addr = GuestAddr((DEFAULT_MEMORY_SIZE - 4) as u64);
        let data = vec![1u8, 2, 3, 4];

        let write_result = mmu.write(last_addr, &data.len(), &data);
        assert!(write_result.is_ok());

        // Read back
        let mut read_data = vec![0u8; 4];
        let read_result = mmu.read(last_addr, &4, &mut read_data);
        assert!(read_result.is_ok());
        assert_eq!(read_data, data);

        // Out of bounds should fail
        let oob_addr = GuestAddr(DEFAULT_MEMORY_SIZE as u64);
        let oob_result = mmu.write(oob_addr, &4, &data);
        assert!(oob_result.is_err());
    }
}

// ============================================================================
// NUMA Integration Tests
// ============================================================================

#[cfg(test)]
mod numa_integration {
    use super::*;

    /// Test NUMA allocator creation
    #[test]
    fn test_numa_creation() {
        let allocator = create_test_numa(4, NumaAllocPolicy::Interleave);

        // Verify allocator is created
        assert!(allocator.total_memory() > 0);
    }

    /// Test NUMA memory allocation with different policies
    #[test]
    fn test_numa_allocation_policies() {
        let policies = vec![
            NumaAllocPolicy::Local,
            NumaAllocPolicy::Interleave,
            NumaAllocPolicy::Bind(0),
        ];

        for policy in policies {
            let allocator = create_test_numa(2, policy);

            // Allocate from each node
            for node in 0..2 {
                let addr = allocator.allocate_from_node(node, PAGE_SIZE);
                assert!(addr.is_ok(), "Allocation from node {} should succeed", node);
            }
        }
    }

    /// Test NUMA node statistics
    #[test]
    fn test_numa_statistics() {
        let allocator = create_test_numa(4, NumaAllocPolicy::Interleave);

        // Allocate some memory
        let _ = allocator.allocate(PAGE_SIZE * 10);

        // Check stats
        let stats = allocator.stats();
        assert!(stats.total_nodes == 4);
        assert!(stats.allocated_memory > 0);
    }
}

// ============================================================================
// Cross-Component Integration Tests
// ============================================================================

#[cfg(test)]
mod cross_component_integration {
    use super::*;

    /// Test memory pool + TLB integration
    #[test]
    fn test_pool_tlb_integration() {
        let pool = create_test_pool(DEFAULT_MEMORY_SIZE);
        let mut tlb = create_test_tlb(64);

        // Allocate memory
        let paddr = pool.allocate(PAGE_SIZE).expect("Allocation failed");

        // Write to memory
        pool.write(paddr, &[1, 2, 3, 4]).expect("Write failed");

        // Create TLB entry
        let vaddr = GuestAddr(0x1000);
        tlb.insert(
            vaddr,
            TlbEntry {
                phys_addr: paddr,
                flags: 0x7,
                access_count: 0,
            },
        );

        // Lookup in TLB
        let entry = tlb.lookup(vaddr, AccessType::Read);
        assert!(entry.is_some());

        let entry = entry.unwrap();
        assert_eq!(entry.phys_addr, paddr);

        // Verify we can access the actual memory
        let data = pool.read(paddr, 4).expect("Read failed");
        assert_eq!(data, vec![1, 2, 3, 4]);
    }

    /// Test MMU + memory pool integration
    #[test]
    fn test_mmu_pool_integration() {
        let pool = create_test_pool(DEFAULT_MEMORY_SIZE);

        // Allocate physical memory
        let paddr = pool.allocate(PAGE_SIZE).expect("Allocation failed");

        // Write data
        let test_data = vec![10u8, 20, 30, 40];
        pool.write(paddr, &test_data).expect("Write failed");

        // Verify data in pool
        let read_from_pool = pool.read(paddr, test_data.len()).expect("Read failed");
        assert_eq!(read_from_pool, test_data);

        // The physical address should be valid
        assert!(paddr.0 < DEFAULT_MEMORY_SIZE as u64);
    }

    /// Test end-to-end: allocation → TLB → access
    #[test]
    fn test_end_to_end_memory_access() {
        let pool = create_test_pool(DEFAULT_MEMORY_SIZE);
        let mut tlb = create_test_tlb(64);

        // 1. Allocate physical memory
        let paddr = pool.allocate(PAGE_SIZE).expect("Allocation failed");

        // 2. Write to physical memory
        let test_data = vec![0xABu8; 256];
        pool.write(paddr, &test_data).expect("Write failed");

        // 3. Create TLB mapping
        let vaddr = GuestAddr(0x1000);
        tlb.insert(
            vaddr,
            TlbEntry {
                phys_addr: paddr,
                flags: 0x7,
                access_count: 0,
            },
        );

        // 4. Translate virtual to physical via TLB
        let entry = tlb
            .lookup(vaddr, AccessType::Read)
            .expect("TLB lookup failed");

        // 5. Access memory using translated address
        let read_data = pool
            .read(entry.phys_addr, test_data.len())
            .expect("Read failed");

        // 6. Verify data
        assert_eq!(read_data, test_data);
    }
}

// ============================================================================
// Performance Integration Tests
// ============================================================================

#[cfg(test)]
mod performance_integration {
    use std::time::Instant;

    use super::*;

    /// Benchmark memory pool operations
    #[test]
    fn test_memory_pool_performance() {
        let mut pool = create_test_pool(DEFAULT_MEMORY_SIZE);

        let iterations = 1000;
        let start = Instant::now();

        let mut addrs = Vec::new();
        for _ in 0..iterations {
            let addr = pool.allocate(PAGE_SIZE).expect("Allocation failed");
            addrs.push(addr);

            pool.write(addr, &[1, 2, 3, 4]).expect("Write failed");
            let _ = pool.read(addr, 4).expect("Read failed");
        }

        let elapsed = start.elapsed();
        let avg_time = elapsed / iterations;

        println!(
            "Memory pool: {} iterations in {:?} (avg: {:?}/iter)",
            iterations, elapsed, avg_time
        );

        // Cleanup
        for addr in addrs {
            let _ = pool.deallocate(addr, PAGE_SIZE);
        }
    }

    /// Benchmark TLB operations
    #[test]
    fn test_tlb_performance() {
        let mut tlb = create_test_tlb(64);

        let iterations = 10000;
        let start = Instant::now();

        for i in 0..iterations {
            let vaddr = GuestAddr(0x1000 + (i % 100) * PAGE_SIZE as u64);
            tlb.insert(
                vaddr,
                TlbEntry {
                    phys_addr: GuestPhysAddr(0x5000 + i as u64 * PAGE_SIZE as u64),
                    flags: 0x7,
                    access_count: 0,
                },
            );

            let _ = tlb.lookup(vaddr, AccessType::Read);
        }

        let elapsed = start.elapsed();
        let avg_time = elapsed / iterations;

        println!(
            "TLB operations: {} iterations in {:?} (avg: {:?}/iter)",
            iterations, elapsed, avg_time
        );

        assert!(avg_time.as_micros() < 100, "TLB operations should be fast");
    }

    /// Test TLB hit rate
    #[test]
    fn test_tlb_hit_rate() {
        let mut tlb = create_test_tlb(64);

        // Insert entries
        for i in 0..10 {
            let vaddr = GuestAddr(0x1000 + i * PAGE_SIZE as u64);
            tlb.insert(
                vaddr,
                TlbEntry {
                    phys_addr: GuestPhysAddr(0x5000 + i * PAGE_SIZE as u64),
                    flags: 0x7,
                    access_count: 0,
                },
            );
        }

        // Perform lookups with locality
        for _ in 0..1000 {
            for i in 0..10 {
                let _ = tlb.lookup(GuestAddr(0x1000 + i * PAGE_SIZE as u64), AccessType::Read);
            }
        }

        let stats = tlb.stats();
        let hit_rate = if stats.hits + stats.misses > 0 {
            stats.hits as f64 / (stats.hits + stats.misses) as f64
        } else {
            0.0
        };

        println!("TLB hit rate: {:.2}%", hit_rate * 100.0);
        assert!(hit_rate > 0.9, "TLB hit rate should be > 90%");
    }
}

// ============================================================================
// Stress Tests
// ============================================================================

#[cfg(test)]
mod stress_tests {
    use super::*;

    /// Test many small allocations
    #[test]
    fn test_many_small_allocations() {
        let mut pool = create_test_pool(DEFAULT_MEMORY_SIZE);

        let iterations = 1000;
        let mut addrs = Vec::new();

        for _ in 0..iterations {
            let addr = pool.allocate(64).expect("Allocation failed");
            addrs.push(addr);
        }

        // Verify all allocations are unique
        for (i, &addr1) in addrs.iter().enumerate() {
            for &addr2 in addrs.iter().skip(i + 1) {
                assert_ne!(addr1, addr2);
            }
        }

        // Cleanup
        for addr in addrs {
            let _ = pool.deallocate(addr, 64);
        }
    }

    /// Test TLB with many entries
    #[test]
    fn test_tlb_many_entries() {
        let mut tlb = create_test_tlb(1024);

        for i in 0..1000 {
            let vaddr = GuestAddr(0x1000 + i * PAGE_SIZE as u64);
            tlb.insert(
                vaddr,
                TlbEntry {
                    phys_addr: GuestPhysAddr(0x5000 + i * PAGE_SIZE as u64),
                    flags: 0x7,
                    access_count: 0,
                },
            );
        }

        let stats = tlb.stats();
        assert!(stats.entries <= 1024, "TLB should respect capacity");
    }

    /// Test alternating allocations and deallocations
    #[test]
    fn test_alternating_alloc_dealloc() {
        let mut pool = create_test_pool(PAGE_SIZE * 100);

        for _ in 0..100 {
            let addr1 = pool.allocate(PAGE_SIZE).expect("Alloc failed");
            let addr2 = pool.allocate(PAGE_SIZE).expect("Alloc failed");

            pool.deallocate(addr1, PAGE_SIZE).expect("Dealloc failed");
            let addr3 = pool.allocate(PAGE_SIZE).expect("Alloc failed");

            pool.deallocate(addr2, PAGE_SIZE).expect("Dealloc failed");
            pool.deallocate(addr3, PAGE_SIZE).expect("Dealloc failed");
        }
    }
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[cfg(test)]
mod error_handling_tests {
    use super::*;

    /// Test allocation failure handling
    #[test]
    fn test_allocation_failure() {
        let mut pool = create_test_pool(PAGE_SIZE * 10);

        // Exhaust the pool
        for _ in 0..10 {
            pool.allocate(PAGE_SIZE).expect("Allocation should succeed");
        }

        // Next allocation should fail
        let result = pool.allocate(PAGE_SIZE);
        assert!(result.is_err(), "Should fail when pool is exhausted");

        if let Err(MemoryError::OutOfMemory { requested, .. }) = result {
            assert_eq!(requested, PAGE_SIZE);
        } else {
            panic!("Expected OutOfMemory error");
        }
    }

    /// Test invalid address handling
    #[test]
    fn test_invalid_address_handling() {
        let pool = create_test_pool(DEFAULT_MEMORY_SIZE);

        let invalid_addr = GuestPhysAddr(0xFFFFFFFFFFFFFFF0);
        let result = pool.read(invalid_addr, 16);

        assert!(result.is_err(), "Read to invalid address should fail");
    }

    /// Test size overflow handling
    #[test]
    fn test_size_overflow_handling() {
        let pool = create_test_pool(DEFAULT_MEMORY_SIZE);

        let valid_addr = GuestPhysAddr(0x1000);
        let huge_size = usize::MAX;

        let result = pool.read(valid_addr, huge_size);
        assert!(result.is_err(), "Huge size should cause error");
    }
}
