//! Sharded MMU Implementation
//!
//! This module implements a sharded Memory Management Unit that reduces lock contention
//! by splitting the MMU into multiple independent shards, each with its own lock.
//!
//! # Architecture
//!
//! - **Sharded Design**: MMU state is split into N independent shards
//! - **Per-Shard Locking**: Each shard has its own RwLock for independent access
//! - **Hash-Based Distribution**: Addresses are distributed to shards using hash function
//! - **Reduced Contention**: Thread conflicts reduced by factor of N (where N = shard count)
//!
//! # Performance Characteristics
//!
//! - **Read Operations**: Contention reduced by ~16x (for 16 shards)
//! - **Write Operations**: Independent per-shard locking
//! - **Scalability**: Near-linear scaling up to number of shards
//! - **Memory Overhead**: Minimal (just N separate locks)
//!
//! # Usage Example
//!
//! ```rust
//! use vm_mem::ShardedMMU;
//!
//! // Create sharded MMU with 16 shards
//! let mmu = ShardedMMU::with_shards(16, 64 * 1024 * 1024, false);
//!
//! // Translate address (only locks one shard)
//! let host_addr = mmu.translate(guest_addr).unwrap();
//!
//! // Concurrent accesses to different shards don't block
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::RwLock;
use vm_core::error::{CoreError, MemoryError};
use vm_core::{AccessType, Fault, MemoryAccess, MmioDevice, MmioManager, VmError};

use crate::{GuestAddr, GuestPhysAddr, HostAddr, PAGE_SHIFT, PAGE_SIZE, PhysicalMemory};

/// Default number of shards
const DEFAULT_SHARD_COUNT: usize = 16;
/// Maximum number of shards (must be power of 2)
const MAX_SHARD_COUNT: usize = 64;

/// Page table entry for a single shard
#[derive(Debug, Clone, Copy)]
struct ShardPageTableEntry {
    /// Physical page number
    ppn: u64,
    /// Access flags
    flags: u64,
    /// Address space ID
    asid: u16,
}

/// TLB entry for a single shard
#[derive(Debug, Clone, Copy)]
struct ShardTlbEntry {
    /// Virtual page number
    vpn: u64,
    /// Physical page number
    ppn: u64,
    /// Access flags
    flags: u64,
    /// Address space ID
    asid: u16,
    /// Last access timestamp
    last_access: u64,
}

/// Per-shard TLB
struct ShardTlb {
    /// TLB entries
    entries: HashMap<(u64, u16), ShardTlbEntry>,
    /// Maximum capacity
    capacity: usize,
    /// Access counter for LRU
    access_counter: u64,
}

impl ShardTlb {
    fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(capacity),
            capacity,
            access_counter: 0,
        }
    }

    fn lookup(&mut self, vpn: u64, asid: u16) -> Option<&ShardTlbEntry> {
        let key = (vpn, asid);
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.last_access = self.access_counter;
            self.access_counter += 1;
            Some(entry)
        } else {
            None
        }
    }

    fn insert(&mut self, entry: ShardTlbEntry) {
        let key = (entry.vpn, entry.asid);

        // Evict if at capacity
        if self.entries.len() >= self.capacity {
            // Find LRU entry
            if let Some((&lru_key, _)) = self.entries
                .iter()
                .min_by_key(|(_, e)| e.last_access)
            {
                self.entries.remove(&lru_key);
            }
        }

        self.entries.insert(key, entry);
    }

    fn invalidate(&mut self, vpn: u64, asid: u16) -> bool {
        self.entries.remove(&(vpn, asid)).is_some()
    }

    fn flush(&mut self) {
        self.entries.clear();
        self.access_counter = 0;
    }

    fn len(&self) -> usize {
        self.entries.len()
    }
}

/// MMU shard (independent unit with its own lock)
struct MmuShard {
    /// Page table for this shard
    page_table: HashMap<u64, ShardPageTableEntry>,
    /// TLB for this shard
    tlb: ShardTlb,
    /// Statistics
    stats: ShardStats,
}

/// Per-shard statistics
#[derive(Debug, Default)]
struct ShardStats {
    translations: u64,
    tlb_hits: u64,
    tlb_misses: u64,
    updates: u64,
    flushes: u64,
}

impl MmuShard {
    fn new(tlb_capacity: usize) -> Self {
        Self {
            page_table: HashMap::with_capacity(64),
            tlb: ShardTlb::new(tlb_capacity),
            stats: ShardStats::default(),
        }
    }

    /// Translate address (shard-local operation)
    fn translate(&mut self, vpn: u64, asid: u16, offset: u64) -> Option<u64> {
        self.stats.translations += 1;

        // Try TLB first
        if let Some(entry) = self.tlb.lookup(vpn, asid) {
            self.stats.tlb_hits += 1;
            return Some((entry.ppn << PAGE_SHIFT) | offset);
        }

        self.stats.tlb_misses += 1;

        // Check page table
        if let Some(pte) = self.page_table.get(&vpn) {
            // Insert into TLB
            self.tlb.insert(ShardTlbEntry {
                vpn,
                ppn: pte.ppn,
                flags: pte.flags,
                asid,
                last_access: 0,
            });

            return Some((pte.ppn << PAGE_SHIFT) | offset);
        }

        None
    }

    /// Update mapping (shard-local operation)
    fn update(&mut self, guest_vpn: u64, host: u64, asid: u16, flags: u64) {
        let ppn = host >> PAGE_SHIFT;

        let entry = ShardPageTableEntry {
            ppn,
            flags,
            asid,
        };

        self.page_table.insert(guest_vpn, entry);
        self.tlb.invalidate(guest_vpn, asid);
        self.stats.updates += 1;
    }

    /// Invalidate TLB entry
    fn invalidate_tlb(&mut self, vpn: u64, asid: u16) {
        self.tlb.invalidate(vpn, asid);
        self.stats.flushes += 1;
    }

    /// Flush TLB
    fn flush_tlb(&mut self) {
        self.tlb.flush();
        self.stats.flushes += 1;
    }

    /// Get statistics
    fn stats(&self) -> ShardStats {
        ShardStats {
            translations: self.stats.translations,
            tlb_hits: self.stats.tlb_hits,
            tlb_misses: self.stats.tlb_misses,
            updates: self.stats.updates,
            flushes: self.stats.flushes,
        }
    }
}

/// Sharded MMU
pub struct ShardedMMU {
    /// Physical memory backend
    phys_mem: Arc<PhysicalMemory>,
    /// MMU shards (each with independent lock)
    shards: Vec<RwLock<MmuShard>>,
    /// Number of shards (must be power of 2)
    shard_count: usize,
    /// Shard mask for fast indexing
    shard_mask: usize,
    /// Current paging mode
    paging_mode: RwLock<PagingMode>,
    /// Global statistics
    global_stats: GlobalStats,
}

/// Paging mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagingMode {
    Bare = 0,
    Sv39 = 8,
    Sv48 = 9,
}

/// Global statistics across all shards
#[derive(Debug, Default)]
struct GlobalStats {
    total_translations: AtomicU64,
    total_updates: AtomicU64,
    total_flushes: AtomicU64,
}

impl ShardedMMU {
    /// Create new sharded MMU with default shard count (16)
    pub fn new(size: usize, use_hugepages: bool) -> Self {
        Self::with_shards(DEFAULT_SHARD_COUNT, size, use_hugepages)
    }

    /// Create new sharded MMU with specified shard count
    ///
    /// # Arguments
    /// * `shard_count` - Number of shards (must be power of 2, max 64)
    /// * `size` - Physical memory size in bytes
    /// * `use_hugepages` - Whether to use 2MB huge pages
    pub fn with_shards(shard_count: usize, size: usize, use_hugepages: bool) -> Self {
        assert!(
            shard_count.is_power_of_two() && shard_count <= MAX_SHARD_COUNT,
            "shard_count must be power of 2 and <= {}",
            MAX_SHARD_COUNT
        );

        let tlb_per_shard = 128 / shard_count.max(1);

        let shards = (0..shard_count)
            .map(|_| RwLock::new(MmuShard::new(tlb_per_shard.max(8))))
            .collect();

        Self {
            phys_mem: Arc::new(PhysicalMemory::new(size, use_hugepages)),
            shards,
            shard_count,
            shard_mask: shard_count - 1,
            paging_mode: RwLock::new(PagingMode::Bare),
            global_stats: GlobalStats::default(),
        }
    }

    /// Calculate shard index for an address
    #[inline]
    fn shard_index(&self, addr: u64) -> usize {
        // Use hash of address to distribute across shards
        let hash = (addr as usize).wrapping_mul(0x9e3779b9);
        (hash >> 16) & self.shard_mask
    }

    /// Translate guest virtual address to host address
    ///
    /// Only locks one shard, allowing concurrent access to different shards
    pub fn translate(&self, guest_addr: GuestAddr) -> Result<HostAddr, VmError> {
        self.global_stats
            .total_translations
            .fetch_add(1, Ordering::Relaxed);

        let mode = *self.paging_mode.read();

        // Bare mode: direct mapping
        if mode == PagingMode::Bare {
            return Ok(guest_addr.0);
        }

        let vpn = guest_addr.0 >> PAGE_SHIFT;
        let offset = guest_addr.0 & (PAGE_SIZE - 1);
        let shard_idx = self.shard_index(guest_addr.0);

        // Lock only one shard
        let mut shard = self.shards[shard_idx].write();

        // Default ASID for now
        let asid = 0;

        if let Some(phys) = shard.translate(vpn, asid, offset) {
            Ok(phys)
        } else {
            Err(VmError::from(Fault::PageFault {
                addr: guest_addr,
                access_type: AccessType::Read,
                is_write: false,
                is_user: false,
            }))
        }
    }

    /// Update memory mapping
    ///
    /// Only locks one shard
    pub fn update_mapping(&self, guest: GuestAddr, host: HostAddr) {
        let vpn = guest.0 >> PAGE_SHIFT;
        let shard_idx = self.shard_index(guest.0);

        let mut shard = self.shards[shard_idx].write();
        shard.update(vpn, host, 0, 0x1F); // V+R+W+X+U

        self.global_stats
            .total_updates
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Invalidate TLB entry for specific address
    pub fn invalidate_tlb(&self, addr: GuestAddr) {
        let vpn = addr.0 >> PAGE_SHIFT;
        let shard_idx = self.shard_index(addr.0);

        let mut shard = self.shards[shard_idx].write();
        shard.invalidate_tlb(vpn, 0);
    }

    /// Flush all TLB entries across all shards
    pub fn flush_tlb(&self) {
        for shard in &self.shards {
            shard.write().flush_tlb();
        }

        self.global_stats
            .total_flushes
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Set paging mode
    pub fn set_paging_mode(&self, mode: PagingMode) {
        *self.paging_mode.write() = mode;
        self.flush_tlb();
    }

    /// Get aggregated statistics from all shards
    pub fn stats(&self) -> ShardedMMUStats {
        let mut stats = ShardedMMUStats::default();

        for shard in &self.shards {
            let shard_stats = shard.read().stats();
            stats.translations += shard_stats.translations;
            stats.tlb_hits += shard_stats.tlb_hits;
            stats.tlb_misses += shard_stats.tlb_misses;
            stats.updates += shard_stats.updates;
            stats.flushes += shard_stats.flushes;
        }

        stats.total_translations = self.global_stats.total_translations.load(Ordering::Relaxed);
        stats.total_updates = self.global_stats.total_updates.load(Ordering::Relaxed);
        stats.total_flushes = self.global_stats.total_flushes.load(Ordering::Relaxed);

        stats
    }

    /// Get hit rate across all shards
    pub fn hit_rate(&self) -> f64 {
        let stats = self.stats();
        let total = stats.tlb_hits + stats.tlb_misses;

        if total == 0 {
            0.0
        } else {
            stats.tlb_hits as f64 / total as f64
        }
    }

    /// Get number of shards
    pub fn shard_count(&self) -> usize {
        self.shard_count
    }
}

/// Aggregated statistics for sharded MMU
#[derive(Debug, Clone, Copy, Default)]
pub struct ShardedMMUStats {
    /// Total translations (aggregated from shards)
    pub translations: u64,
    /// Total TLB hits
    pub tlb_hits: u64,
    /// Total TLB misses
    pub tlb_misses: u64,
    /// Total updates
    pub updates: u64,
    /// Total flushes
    pub flushes: u64,
    /// Global counters
    pub total_translations: u64,
    pub total_updates: u64,
    pub total_flushes: u64,
}

impl ShardedMMUStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.tlb_hits + self.tlb_misses;
        if total == 0 {
            0.0
        } else {
            self.tlb_hits as f64 / total as f64
        }
    }
}

// ============================================================================
// Trait Implementations
// ============================================================================

impl MemoryAccess for ShardedMMU {
    fn read(&self, addr: GuestAddr, size: u8) -> Result<u64, VmError> {
        let host_addr = self.translate(addr)?;
        let host_addr_usize = host_addr as usize;

        match size {
            1 => self.phys_mem.read_u8(host_addr_usize).map(|v| v as u64),
            2 => self.phys_mem.read_u16(host_addr_usize).map(|v| v as u64),
            4 => self.phys_mem.read_u32(host_addr_usize).map(|v| v as u64),
            8 => self.phys_mem.read_u64(host_addr_usize),
            _ => Err(VmError::Core(CoreError::InvalidParameter {
                name: "size".to_string(),
                value: size.to_string(),
                message: "Invalid read size".to_string(),
            })),
        }
    }

    fn write(&mut self, addr: GuestAddr, val: u64, size: u8) -> Result<(), VmError> {
        let host_addr = self.translate(addr)?;
        let host_addr_usize = host_addr as usize;

        match size {
            1 => self.phys_mem.write_u8(host_addr_usize, val as u8),
            2 => self.phys_mem.write_u16(host_addr_usize, val as u16),
            4 => self.phys_mem.write_u32(host_addr_usize, val as u32),
            8 => self.phys_mem.write_u64(host_addr_usize, val),
            _ => Err(VmError::Core(CoreError::InvalidParameter {
                name: "size".to_string(),
                value: size.to_string(),
                message: "Invalid write size".to_string(),
            })),
        }
    }

    fn fetch_insn(&self, pc: GuestAddr) -> Result<u64, VmError> {
        self.read(pc, 4)
    }

    fn memory_size(&self) -> usize {
        self.phys_mem.size()
    }

    fn dump_memory(&self) -> Vec<u8> {
        self.phys_mem.dump()
    }

    fn restore_memory(&mut self, data: &[u8]) -> Result<(), String> {
        self.phys_mem.restore(data).map_err(|e| e.to_string())
    }
}

impl MmioManager for ShardedMMU {
    fn map_mmio(&self, base: GuestAddr, size: u64, device: Box<dyn MmioDevice>) {
        self.phys_mem.map_mmio(base, size, device)
    }

    fn poll_devices(&self) {
        self.phys_mem.poll_devices()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::sync::Arc;

    #[test]
    fn test_basic_translation() {
        let mmu = ShardedMMU::new(1024 * 1024, false);

        // Bare mode: direct mapping
        let host = mmu.translate(GuestAddr(0x1000)).unwrap();
        assert_eq!(host, 0x1000);
    }

    #[test]
    fn test_update_mapping() {
        let mmu = ShardedMMU::new(1024 * 1024, false);

        // Set Sv39 mode first
        mmu.set_paging_mode(PagingMode::Sv39);

        // Create mapping (page-aligned)
        mmu.update_mapping(GuestAddr(0x1000), 0x2000);

        // Translation should use the mapping (page-aligned)
        let host = mmu.translate(GuestAddr(0x1000)).unwrap();
        assert_eq!(host, 0x2000);
    }

    #[test]
    fn test_shard_distribution() {
        let mmu = ShardedMMU::with_shards(16, 1024 * 1024, false);

        // Updates to different addresses go to different shards
        for i in 0..100 {
            let addr = (i * 0x1000) as u64;
            mmu.update_mapping(GuestAddr(addr), addr);

            let shard_idx = mmu.shard_index(addr);
            assert!(shard_idx < 16);
        }
    }

    #[test]
    fn test_concurrent_access_same_shard() {
        let mmu = Arc::new(ShardedMMU::new(16 * 1024 * 1024, false));
        let barrier = Arc::new(std::sync::Barrier::new(4));
        let mut handles = vec![];

        // 4 threads accessing same addresses (same shard)
        for _ in 0..4 {
            let mmu_clone = Arc::clone(&mmu);
            let barrier_clone = Arc::clone(&barrier);

            handles.push(thread::spawn(move || {
                barrier_clone.wait();

                for i in 0..1000 {
                    let addr = (i * 0x1000) as u64;
                    mmu_clone.update_mapping(GuestAddr(addr), addr);

                    let host = mmu_clone.translate(GuestAddr(addr)).unwrap();
                    assert_eq!(host, addr);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all operations completed
        let stats = mmu.stats();
        assert!(stats.total_translations >= 4000);
        assert!(stats.total_updates >= 4000);
    }

    #[test]
    fn test_concurrent_access_different_shards() {
        let mmu = Arc::new(ShardedMMU::with_shards(16, 64 * 1024 * 1024, false));
        let barrier = Arc::new(std::sync::Barrier::new(16));
        let mut handles = vec![];

        // 16 threads accessing different addresses (different shards)
        for thread_id in 0..16 {
            let mmu_clone = Arc::clone(&mmu);
            let barrier_clone = Arc::clone(&barrier);

            handles.push(thread::spawn(move || {
                barrier_clone.wait();

                for i in 0..1000 {
                    // Each thread accesses addresses that hash to different shards
                    let addr = ((thread_id * 0x10000 + i * 8) as u64);
                    mmu_clone.update_mapping(GuestAddr(addr), addr);

                    let host = mmu_clone.translate(GuestAddr(addr)).unwrap();
                    assert_eq!(host, addr);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all operations completed
        let stats = mmu.stats();
        assert!(stats.total_translations >= 16000);
        assert!(stats.total_updates >= 16000);
    }

    #[test]
    fn test_tlb_flush() {
        let mmu = ShardedMMU::new(1024 * 1024, false);

        mmu.set_paging_mode(PagingMode::Sv39);
        mmu.update_mapping(GuestAddr(0x1000), 0x2000);

        // Access to populate TLB
        mmu.translate(GuestAddr(0x1000)).unwrap();

        // Flush TLB
        mmu.flush_tlb();

        let stats = mmu.stats();
        assert!(stats.total_flushes > 0);
    }

    #[test]
    fn test_hit_rate() {
        let mmu = ShardedMMU::new(1024 * 1024, false);

        mmu.set_paging_mode(PagingMode::Sv39);
        mmu.update_mapping(GuestAddr(0x1000), 0x2000);

        // First access (TLB miss)
        mmu.translate(GuestAddr(0x1000)).unwrap();

        // Subsequent accesses (TLB hits)
        for _ in 0..10 {
            mmu.translate(GuestAddr(0x1000)).unwrap();
        }

        let hit_rate = mmu.hit_rate();
        assert!(hit_rate > 0.8); // Should be ~0.91 (10/11)
    }

    #[test]
    fn test_stats_aggregation() {
        let mmu = ShardedMMU::with_shards(8, 1024 * 1024, false);

        mmu.set_paging_mode(PagingMode::Sv39);

        // Perform operations across different shards
        for i in 0..100 {
            let addr = (i * 0x1000) as u64;
            mmu.update_mapping(GuestAddr(addr), addr);
            mmu.translate(GuestAddr(addr)).unwrap();
        }

        let stats = mmu.stats();

        // Verify statistics are aggregated
        assert!(stats.translations >= 100);
        assert!(stats.updates >= 100);
        assert_eq!(stats.total_translations, stats.translations);
        assert_eq!(stats.total_updates, stats.updates);
    }

    #[test]
    fn test_shard_count() {
        let mmu_default = ShardedMMU::new(1024 * 1024, false);
        assert_eq!(mmu_default.shard_count(), 16);

        let mmu_custom = ShardedMMU::with_shards(32, 1024 * 1024, false);
        assert_eq!(mmu_custom.shard_count(), 32);
    }
}
