//! Lock-Free MMU Implementation
//!
//! This module implements a lock-free Memory Management Unit using atomic operations
//! and sharded data structures for concurrent access optimization.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU16, AtomicU64, Ordering};

use dashmap::DashMap;
use vm_core::error::CoreError;
use vm_core::mmu_traits::AddressTranslator;
use vm_core::{AccessType, Fault, MemoryAccess, MmioDevice, MmioManager, MmuAsAny, VmError};

use crate::{GuestAddr, GuestPhysAddr, HostAddr, PAGE_SHIFT, PAGE_SIZE, PhysicalMemory};

/// TLB entry
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // Reserved for future TLB optimization
struct TlbEntry {
    /// Virtual page number
    vpn: u64,
    /// Physical page number
    ppn: u64,
    /// Access flags
    flags: u64,
    /// Address space ID
    asid: u16,
}

/// Lock-Free MMU
pub struct LockFreeMMU {
    /// Physical memory backend
    phys_mem: Arc<PhysicalMemory>,
    /// Lock-free page table (using DashMap)
    page_table: DashMap<u64, u64>, // vpn -> ppn
    /// Global timestamp for TLB entries
    #[allow(dead_code)] // Reserved for future TLB optimization
    tlb_timestamp: AtomicU64,
    /// Page table base address
    page_table_base: AtomicU64,
    /// Current paging mode
    paging_mode: AtomicU8,
    /// Address space ID
    asid: AtomicU16,
    /// Statistics
    stats: LockFreeMMUStats,
}

/// Lock-free MMU statistics
#[derive(Debug, Default)]
struct LockFreeMMUStats {
    /// Total translations
    translations: AtomicU64,
    /// TLB hits
    tlb_hits: AtomicU64,
    /// TLB misses
    tlb_misses: AtomicU64,
    /// Page table updates
    page_table_updates: AtomicU64,
    /// TLB flushes
    tlb_flushes: AtomicU64,
}

impl LockFreeMMU {
    /// Create new lock-free MMU
    pub fn new(size: usize, use_hugepages: bool) -> Self {
        Self {
            phys_mem: Arc::new(PhysicalMemory::new(size, use_hugepages)),
            page_table: DashMap::new(),
            tlb_timestamp: AtomicU64::new(0),
            page_table_base: AtomicU64::new(0),
            paging_mode: AtomicU8::new(0), // 0 = Bare mode
            asid: AtomicU16::new(0),
            stats: LockFreeMMUStats::default(),
        }
    }

    /// Translate guest virtual address to host address (lock-free)
    /// This is the internal implementation that returns HostAddr
    fn translate_to_host(&self, guest_addr: GuestAddr) -> Result<HostAddr, VmError> {
        self.stats.translations.fetch_add(1, Ordering::Relaxed);

        let mode = self.paging_mode.load(Ordering::Acquire);

        // Bare mode: direct mapping
        if mode == 0 {
            return Ok(guest_addr.0);
        }

        let vpn = guest_addr.0 >> PAGE_SHIFT;

        // Lock-free page table lookup
        if let Some(entry) = self.page_table.get(&vpn) {
            self.stats.tlb_hits.fetch_add(1, Ordering::Relaxed);
            let ppn = *entry;
            let offset = guest_addr.0 & (PAGE_SIZE - 1);
            return Ok((ppn << PAGE_SHIFT) | offset);
        }

        self.stats.tlb_misses.fetch_add(1, Ordering::Relaxed);

        Err(VmError::from(Fault::PageFault {
            addr: guest_addr,
            access_type: AccessType::Read,
            is_write: false,
            is_user: false,
        }))
    }

    /// Update memory mapping (lock-free)
    pub fn update_mapping(&self, guest: GuestAddr, host: HostAddr) {
        let vpn = guest.0 >> PAGE_SHIFT;
        let ppn = host >> PAGE_SHIFT;

        self.page_table.insert(vpn, ppn);
        self.stats
            .page_table_updates
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Invalidate TLB entry for specific address
    pub fn invalidate_tlb(&self, addr: GuestAddr) {
        let vpn = addr.0 >> PAGE_SHIFT;
        self.page_table.remove(&vpn);
        self.stats.tlb_flushes.fetch_add(1, Ordering::Relaxed);
    }

    /// Flush entire TLB (internal implementation)
    fn flush_tlb_internal(&self) {
        self.page_table.clear();
        self.stats.tlb_flushes.fetch_add(1, Ordering::Relaxed);
    }

    /// Flush entire TLB (public API)
    pub fn flush_tlb(&self) {
        self.flush_tlb_internal();
    }

    /// Set paging mode (0=Bare, 8=Sv39, 9=Sv48)
    pub fn set_paging_mode(&self, mode: u8) {
        self.paging_mode.store(mode, Ordering::Release);
        self.flush_tlb_internal();
    }

    /// Set SATP register (RISC-V)
    pub fn set_satp(&self, satp: u64) {
        let mode = ((satp >> 60) & 0xF) as u8;
        let asid = ((satp >> 44) & 0xFFFF) as u16;
        let ppn = satp & ((1u64 << 44) - 1);

        self.paging_mode.store(mode, Ordering::Release);
        self.asid.store(asid, Ordering::Release);
        self.page_table_base
            .store(ppn << PAGE_SHIFT, Ordering::Release);
        self.flush_tlb_internal();
    }

    /// Get statistics snapshot
    pub fn stats(&self) -> LockFreeMMUStatsSnapshot {
        LockFreeMMUStatsSnapshot {
            translations: self.stats.translations.load(Ordering::Relaxed),
            tlb_hits: self.stats.tlb_hits.load(Ordering::Relaxed),
            tlb_misses: self.stats.tlb_misses.load(Ordering::Relaxed),
            page_table_updates: self.stats.page_table_updates.load(Ordering::Relaxed),
            tlb_flushes: self.stats.tlb_flushes.load(Ordering::Relaxed),
        }
    }

    /// Get hit rate (0.0 to 1.0)
    pub fn hit_rate(&self) -> f64 {
        let hits = self.stats.tlb_hits.load(Ordering::Relaxed);
        let misses = self.stats.tlb_misses.load(Ordering::Relaxed);
        let total = hits + misses;

        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }
}

/// Statistics snapshot
#[derive(Debug, Clone, Copy)]
pub struct LockFreeMMUStatsSnapshot {
    pub translations: u64,
    pub tlb_hits: u64,
    pub tlb_misses: u64,
    pub page_table_updates: u64,
    pub tlb_flushes: u64,
}

impl LockFreeMMUStatsSnapshot {
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

impl MemoryAccess for LockFreeMMU {
    fn read(&self, addr: GuestAddr, size: u8) -> Result<u64, VmError> {
        let host_addr = self.translate_to_host(addr)?;
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
        let host_addr = self.translate_to_host(addr)?;
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

impl MmioManager for LockFreeMMU {
    fn map_mmio(&self, base: GuestAddr, size: u64, device: Box<dyn MmioDevice>) {
        self.phys_mem.map_mmio(base, size, device)
    }

    fn poll_devices(&self) {
        self.phys_mem.poll_devices()
    }
}

impl AddressTranslator for LockFreeMMU {
    fn translate(&mut self, va: GuestAddr, _access: AccessType) -> Result<GuestPhysAddr, VmError> {
        // Call the internal translate_to_host method and convert to GuestPhysAddr
        let host_addr = self.translate_to_host(va)?;
        Ok(GuestPhysAddr(host_addr))
    }

    fn flush_tlb(&mut self) {
        // Call the internal flush_tlb method
        self.flush_tlb_internal();
    }

    fn flush_tlb_asid(&mut self, _asid: u16) {
        self.flush_tlb_internal();
    }

    fn flush_tlb_page(&mut self, va: GuestAddr) {
        self.invalidate_tlb(va);
    }
}

impl MmuAsAny for LockFreeMMU {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_translation() {
        let mmu = LockFreeMMU::new(1024 * 1024, false);

        // Bare mode: direct mapping
        let host = mmu.translate_to_host(GuestAddr(0x1000)).unwrap();
        assert_eq!(host, 0x1000);
    }

    #[test]
    fn test_update_mapping() {
        let mmu = LockFreeMMU::new(1024 * 1024, false);

        // Set Sv39 mode first
        mmu.set_paging_mode(8);

        // Create mapping (page-aligned)
        mmu.update_mapping(GuestAddr(0x1000), 0x2000);

        // Translation should use the mapping
        let host = mmu.translate_to_host(GuestAddr(0x1000)).unwrap();
        assert_eq!(host, 0x2000);

        // Test with offset
        let host_with_offset = mmu.translate_to_host(GuestAddr(0x1100)).unwrap();
        assert_eq!(host_with_offset, 0x2100);
    }

    #[test]
    fn test_tlb_invalidation() {
        let mmu = LockFreeMMU::new(1024 * 1024, false);

        mmu.set_paging_mode(8);
        mmu.update_mapping(GuestAddr(0x1000), 0x2000);

        // First access
        let host1 = mmu.translate_to_host(GuestAddr(0x1000)).unwrap();
        assert_eq!(host1, 0x2000);

        // Invalidate TLB
        mmu.invalidate_tlb(GuestAddr(0x1000));

        // Update mapping
        mmu.update_mapping(GuestAddr(0x1000), 0x3000);

        // Should get new mapping
        let host2 = mmu.translate_to_host(GuestAddr(0x1000)).unwrap();
        assert_eq!(host2, 0x3000);
    }

    #[test]
    fn test_concurrent_access() {
        use std::thread;

        let mmu = Arc::new(LockFreeMMU::new(16 * 1024 * 1024, false));
        let barrier = Arc::new(std::sync::Barrier::new(8));
        let mut handles = vec![];

        // 8 threads concurrently accessing MMU
        for thread_id in 0..8 {
            let mmu_clone = Arc::clone(&mmu);
            let barrier_clone = Arc::clone(&barrier);

            handles.push(thread::spawn(move || {
                barrier_clone.wait();

                for i in 0..1000 {
                    let guest_addr = GuestAddr((thread_id * 0x1000 + i * 8) as u64);
                    mmu_clone.update_mapping(guest_addr, guest_addr.0);

                    let host = mmu_clone.translate_to_host(guest_addr).unwrap();
                    assert_eq!(host, guest_addr.0);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify statistics
        let stats = mmu.stats();
        assert!(stats.translations >= 8000);
        assert!(stats.page_table_updates >= 8000);
    }

    #[test]
    fn test_hit_rate() {
        let mmu = LockFreeMMU::new(1024 * 1024, false);

        mmu.set_paging_mode(8);
        mmu.update_mapping(GuestAddr(0x1000), 0x2000);

        // First access (TLB miss)
        mmu.translate_to_host(GuestAddr(0x1000)).unwrap();

        // Subsequent accesses (TLB hits)
        for _ in 0..10 {
            mmu.translate_to_host(GuestAddr(0x1000)).unwrap();
        }

        let hit_rate = mmu.hit_rate();
        assert!(hit_rate > 0.8); // Should be ~0.91 (10/11)
    }

    #[test]
    fn test_flush_tlb() {
        let mmu = LockFreeMMU::new(1024 * 1024, false);

        mmu.set_paging_mode(8);
        mmu.update_mapping(GuestAddr(0x1000), 0x2000);

        // Access to populate TLB
        mmu.translate_to_host(GuestAddr(0x1000)).unwrap();

        // Flush TLB
        mmu.flush_tlb();

        let stats = mmu.stats();
        assert!(stats.tlb_flushes > 0);
    }
}
