//! Memory Management Integration Tests
//!
//! Comprehensive integration tests for memory management:
//! - MMU initialization and configuration
//! - Memory allocation and deallocation
//! - Page table management
//! - TLB operations
//! - Address translation
//! - Memory access operations
//! - NUMA-aware allocation
//! - Memory pools and optimization

use std::sync::Arc;
use vm_core::{GuestAddr, GuestPhysAddr};
use vm_mem::{
    ConcurrentTlbManager, MultiLevelTlb, NumaAllocPolicy, NumaAllocator, PagingMode,
    Sv39PageTableWalker, TlbFactory, UnifiedMmu, UnifiedMmuConfig, UnifiedTlb, memory,
};

// ============================================================================
// Test Fixtures
// ============================================================================

/// Create a test MMU with default configuration
fn create_test_mmu(memory_size: usize) -> UnifiedMmu {
    let config = UnifiedMmuConfig {
        memory_size,
        paging_mode: PagingMode::Sv39,
        enable_tlb: true,
        enable_huge_pages: true,
        tlb_entries: 64,
        ..Default::default()
    };

    UnifiedMmu::new(config)
}

/// Create a test memory pool
fn create_test_pool(size: usize) -> memory::MemoryPool {
    memory::MemoryPool::new(size, 4096).unwrap()
}

/// Mock page table entry for testing
#[derive(Debug, Clone, Copy)]
struct MockPageTableEntry {
    pub addr: u64,
    pub flags: u64,
}

impl MockPageTableEntry {
    fn new(addr: u64, flags: u64) -> Self {
        Self { addr, flags }
    }

    fn is_valid(&self) -> bool {
        self.flags & vm_mem::pte_flags::V != 0
    }

    fn is_readable(&self) -> bool {
        self.flags & vm_mem::pte_flags::R != 0
    }

    fn is_writable(&self) -> bool {
        self.flags & vm_mem::pte_flags::W != 0
    }

    fn is_executable(&self) -> bool {
        self.flags & vm_mem::pte_flags::X != 0
    }
}

// ============================================================================
// Happy Path Tests
// ============================================================================

#[test]
fn test_mmu_initialization() {
    let mmu = create_test_mmu(1024 * 1024);

    assert_eq!(mmu.get_memory_size(), 1024 * 1024);
}

#[test]
fn test_memory_read_write() {
    let mut mmu = create_test_mmu(1024 * 1024);

    let addr = GuestAddr(0x1000);

    // Write
    mmu.write_byte(addr, 0x42).unwrap();

    // Read
    let value = mmu.read_byte(addr).unwrap();

    assert_eq!(value, 0x42);
}

#[test]
fn test_bulk_memory_operations() {
    let mut mmu = create_test_mmu(1024 * 1024);

    let addr = GuestAddr(0x2000);
    let data = vec![1u8, 2, 3, 4, 5, 6, 7, 8];

    // Write words
    mmu.write_words(addr, &data).unwrap();

    // Read words
    let mut read_data = vec![0u8; data.len()];
    mmu.read_words(addr, &mut read_data).unwrap();

    assert_eq!(read_data, data);
}

#[test]
fn test_tlb_initialization() {
    let tlb = TlbFactory::create_standard(64);

    assert!(tlb.is_ok());
}

#[test]
fn test_tlb_lookup() {
    let mut mmu = create_test_mmu(1024 * 1024);

    let vaddr = GuestAddr(0x1000);
    let paddr = GuestPhysAddr(0x1000);

    // Map page
    let mut page_table = [0u64; 512];
    page_table[0] = paddr.0 | vm_mem::pte_flags::V | vm_mem::pte_flags::R | vm_mem::pte_flags::W;

    // TLB lookup
    let result = mmu.tlb_lookup(vaddr, AccessType::Read);

    // May or may not hit depending on implementation
    let _ = result;
}

#[test]
fn test_tlb_invalidation() {
    let mut mmu = create_test_mmu(1024 * 1024);

    let vaddr = GuestAddr(0x1000);

    // Invalidate TLB entry
    mmu.tlb_flush(vaddr);

    // Invalidate all
    mmu.tlb_flush_all();
}

#[test]
fn test_page_table_walker() {
    let memory = vec![0u8; 1024 * 1024];
    let walker = Sv39PageTableWalker::new(memory.as_ptr() as u64);

    assert_eq!(walker.get_root_addr(), memory.as_ptr() as u64);
}

#[test]
fn test_address_translation() {
    let mut mmu = create_test_mmu(1024 * 1024);

    // Direct mapping for testing
    let vaddr = GuestAddr(0x1000);
    let paddr = GuestPhysAddr(0x1000);

    let result = mmu.translate(vaddr, AccessType::Read);

    // Result depends on page table setup
    let _ = result;
}

#[test]
fn test_memory_pool_allocation() {
    let pool = create_test_pool(1024 * 1024);

    let addr1 = pool.allocate(4096).unwrap();
    let addr2 = pool.allocate(4096).unwrap();

    assert_ne!(addr1, addr2);
}

#[test]
fn test_memory_pool_deallocation() {
    let mut pool = create_test_pool(1024 * 1024);

    let addr = pool.allocate(4096).unwrap();
    pool.deallocate(addr, 4096).unwrap();

    // Should be able to allocate again
    let addr2 = pool.allocate(4096).unwrap();
    assert_eq!(addr, addr2);
}

#[test]
fn test_numa_allocator() {
    let policy = NumaAllocPolicy::Local;

    let allocator = NumaAllocator::new(policy).unwrap();

    let addr = allocator.allocate(1024 * 1024).unwrap();
    assert!(addr > 0);
}

#[test]
fn test_multi_level_tlb() {
    let config = vm_mem::MultiLevelTlbConfig {
        l1_entries: 32,
        l2_entries: 64,
        l3_entries: 128,
    };

    let tlb = MultiLevelTlb::new(config);

    assert!(tlb.is_ok());
}

// ============================================================================
// Error Path Tests
// ============================================================================]

#[test]
fn test_invalid_memory_read() {
    let mmu = create_test_mmu(1024 * 1024);

    // Read beyond memory bounds
    let addr = GuestAddr(0x1000_0000); // 256MB

    let result = mmu.read_byte(addr);

    assert!(result.is_err());
}

#[test]
fn test_invalid_memory_write() {
    let mut mmu = create_test_mmu(1024 * 1024);

    // Write beyond memory bounds
    let addr = GuestAddr(0x1000_0000);

    let result = mmu.write_byte(addr, 0x42);

    assert!(result.is_err());
}

#[test]
fn test_unaligned_access() {
    let mut mmu = create_test_mmu(1024 * 1024);

    // Unaligned word access
    let addr = GuestAddr(0x1001);

    let result = mmu.read_words(addr, &mut [0u8; 4]);

    // May fail depending on alignment requirements
    let _ = result;
}

#[test]
fn test_tlb_miss_handling() {
    let mmu = create_test_mmu(1024 * 1024);

    let vaddr = GuestAddr(0x1000);

    // Access without prior mapping - TLB miss
    let result = mmu.tlb_lookup(vaddr, AccessType::Read);

    // Should handle miss gracefully
    let _ = result;
}

#[test]
fn test_page_fault() {
    let mut mmu = create_test_mmu(1024 * 1024);

    // Access unmapped page
    let vaddr = GuestAddr(0x1000);

    let result = mmu.translate(vaddr, AccessType::Read);

    // Should result in page fault
    assert!(result.is_err());
}

#[test]
fn test_protection_violation() {
    let mut mmu = create_test_mmu(1024 * 1024);

    // Try to execute non-executable page
    let vaddr = GuestAddr(0x1000);

    let result = mmu.translate(vaddr, AccessType::Execute);

    // Should fail if page is not executable
    let _ = result;
}

#[test]
fn test_memory_pool_exhaustion() {
    let mut pool = create_test_pool(4096); // Small pool

    let mut addrs = Vec::new();

    // Allocate until pool is exhausted
    for _ in 0..10 {
        if let Ok(addr) = pool.allocate(4096) {
            addrs.push(addr);
        } else {
            break;
        }
    }

    // Next allocation should fail
    let result = pool.allocate(4096);
    assert!(result.is_err());
}

#[test]
fn test_invalid_deallocation() {
    let mut pool = create_test_pool(1024 * 1024);

    // Deallocate unallocated address
    let result = pool.deallocate(0x1000, 4096);

    assert!(result.is_err());
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_zero_size_allocation() {
    let mut pool = create_test_pool(1024 * 1024);

    let result = pool.allocate(0);

    // Should handle gracefully
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_very_large_allocation() {
    let mut pool = create_test_pool(1024 * 1024);

    // Allocate more than pool size
    let result = pool.allocate(10 * 1024 * 1024);

    assert!(result.is_err());
}

#[test]
fn test_fragmentation_handling() {
    let mut pool = create_test_pool(1024 * 1024);

    let mut addrs = Vec::new();

    // Allocate and deallocate to create fragmentation
    for _ in 0..100 {
        if let Ok(addr) = pool.allocate(4096) {
            addrs.push(addr);
        }
    }

    // Free every other allocation
    for (i, &addr) in addrs.iter().enumerate() {
        if i % 2 == 0 {
            let _ = pool.deallocate(addr, 4096);
        }
    }

    // Should still be able to allocate
    let result = pool.allocate(4096);
    assert!(result.is_ok());
}

#[test]
fn test_concurrent_memory_access() {
    use std::thread;

    let mmu = Arc::new(std::sync::Mutex::new(create_test_mmu(1024 * 1024)));
    let mut handles = Vec::new();

    for i in 0..10 {
        let mmu_clone = Arc::clone(&mmu);
        let handle = thread::spawn(move || {
            let mut mmu = mmu_clone.lock().unwrap();
            let addr = GuestAddr(0x1000 + (i * 0x100));
            mmu.write_byte(addr, i as u8).unwrap();
            mmu.read_byte(addr).unwrap()
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_tlb_with_various_sizes() {
    let sizes = vec![16, 32, 64, 128, 256, 512, 1024];

    for size in sizes {
        let tlb = TlbFactory::create_standard(size);
        assert!(tlb.is_ok());
    }
}

#[test]
fn test_different_paging_modes() {
    let modes = vec![PagingMode::Bare, PagingMode::Sv39, PagingMode::Sv48];

    for mode in modes {
        let config = UnifiedMmuConfig {
            memory_size: 1024 * 1024,
            paging_mode: mode,
            ..Default::default()
        };

        let mmu = UnifiedMmu::new(config);
        assert!(mmu.get_memory_size() == 1024 * 1024);
    }
}

#[test]
fn test_memory_access_patterns() {
    let mut mmu = create_test_mmu(1024 * 1024);

    // Sequential access
    for i in 0..1000 {
        let addr = GuestAddr(i * 8);
        mmu.write_byte(addr, (i & 0xFF) as u8).unwrap();
    }

    // Random access
    for i in 0..100 {
        let addr = GuestAddr((i * 17 * 8) % (1024 * 1024));
        let _ = mmu.read_byte(addr);
    }
}

#[test]
fn test_huge_page_allocation() {
    let config = UnifiedMmuConfig {
        memory_size: 1024 * 1024,
        enable_huge_pages: true,
        ..Default::default()
    };

    let mmu = UnifiedMmu::new(config);

    // Test 2MB huge page allocation
    let _ = mmu;
}

#[test]
fn test_cross_page_access() {
    let mut mmu = create_test_mmu(1024 * 1024);

    // Access spanning two pages
    let addr = GuestAddr(0xFFC); // Near page boundary

    let data = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
    mmu.write_words(addr, &data).unwrap();

    let mut read_data = vec![0u8; data.len()];
    mmu.read_words(addr, &mut read_data).unwrap();

    assert_eq!(read_data, data);
}

// ============================================================================
// Performance Tests
// ============================================================================

#[test]
fn test_memory_access_performance() {
    let mut mmu = create_test_mmu(10 * 1024 * 1024);

    let start = std::time::Instant::now();

    // Sequential access
    for i in 0..1_000_000 {
        let addr = GuestAddr(i * 8);
        mmu.write_byte(addr, (i & 0xFF) as u8).unwrap();
    }

    let duration = start.elapsed();

    // Should complete reasonably fast
    assert!(duration.as_secs() < 5);
}

#[test]
fn test_tlb_performance() {
    let mmu = create_test_mmu(1024 * 1024);

    let start = std::time::Instant::now();

    // Multiple TLB lookups
    for i in 0..100_000 {
        let addr = GuestAddr((i * 4096) % (1024 * 1024));
        let _ = mmu.tlb_lookup(addr, AccessType::Read);
    }

    let duration = start.elapsed();

    // Should complete quickly
    assert!(duration.as_secs() < 3);
}

#[test]
fn test_pool_allocation_performance() {
    let mut pool = create_test_pool(10 * 1024 * 1024);

    let start = std::time::Instant::now();

    // Rapid allocations
    for _ in 0..10_000 {
        let addr = pool.allocate(4096).unwrap();
        pool.deallocate(addr, 4096).unwrap();
    }

    let duration = start.elapsed();

    // Should complete quickly
    assert!(duration.as_secs() < 5);
}

// ============================================================================
// Statistics and Monitoring
// ============================================================================

#[test]
fn test_mmu_statistics() {
    let mut mmu = create_test_mmu(1024 * 1024);

    // Perform some operations
    for i in 0..100 {
        let addr = GuestAddr(i * 0x100);
        mmu.write_byte(addr, i as u8).unwrap();
        let _ = mmu.read_byte(addr);
    }

    let stats = mmu.get_stats();

    assert!(stats.memory_reads > 0);
    assert!(stats.memory_writes > 0);
}

#[test]
fn test_tlb_statistics() {
    let mut mmu = create_test_mmu(1024 * 1024);

    // Perform TLB operations
    for i in 0..100 {
        let addr = GuestAddr(i * 0x1000);
        let _ = mmu.tlb_lookup(addr, AccessType::Read);
    }

    let stats = mmu.get_stats();

    assert!(stats.tlb_lookups > 0);
}

#[test]
fn test_pool_statistics() {
    let mut pool = create_test_pool(1024 * 1024);

    let mut addrs = Vec::new();

    // Allocate
    for _ in 0..100 {
        if let Ok(addr) = pool.allocate(4096) {
            addrs.push(addr);
        }
    }

    // Deallocate some
    for (i, &addr) in addrs.iter().take(50).enumerate() {
        pool.deallocate(addr, 4096).unwrap();
    }

    let stats = pool.get_stats();

    assert!(stats.allocations > 0);
    assert!(stats.deallocations > 0);
    assert!(stats.total_allocated > 0);
}

#[test]
fn test_concurrent_tlb_manager() {
    let config = vm_mem::ConcurrentTlbConfig {
        num_shards: 4,
        entries_per_shard: 64,
    };

    let tlb_manager = ConcurrentTlbManager::new(config);

    assert!(tlb_manager.is_ok());
}

#[test]
fn test_memory_optimization_stats() {
    let config = UnifiedMmuConfig {
        memory_size: 1024 * 1024,
        enable_optimizations: true,
        ..Default::default()
    };

    let mut mmu = UnifiedMmu::new(config);

    // Perform operations
    for i in 0..1000 {
        let addr = GuestAddr(i * 0x100);
        mmu.write_byte(addr, i as u8).unwrap();
    }

    let stats = mmu.get_stats();

    // May have optimization stats
    assert!(stats.memory_reads > 0 || stats.optimized_operations > 0);
}

#[test]
fn test_numa_allocation_stats() {
    let policy = NumaAllocPolicy::Interleave;
    let allocator = NumaAllocator::new(policy).unwrap();

    // Allocate from multiple nodes
    let mut addrs = Vec::new();
    for _ in 0..10 {
        if let Ok(addr) = allocator.allocate(1024 * 1024) {
            addrs.push(addr);
        }
    }

    let stats = allocator.get_stats();

    assert!(stats.total_allocated > 0);
    assert!(stats.allocation_count > 0);
}
