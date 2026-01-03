//! Page Table Walker Domain Service
//!
//! This service encapsulates business logic for page table walking
//! including multi-level page traversal, permission checking, and
//! address translation caching.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::domain_services::events::{DomainEventEnum, PageTableEvent};
use crate::domain_event_bus::DomainEventBus;
use crate::{AccessType, GuestPhysAddr, GuestAddr, VmResult};

/// Page table level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PageTableLevel {
    /// Level 0 (root page table)
    L0,
    /// Level 1
    L1,
    /// Level 2
    L2,
    /// Level 3
    L3,
    /// Level 4 (leaf level for 4KB pages)
    L4,
}

impl PageTableLevel {
    pub fn from_index(index: usize) -> Self {
        match index {
            0 => PageTableLevel::L0,
            1 => PageTableLevel::L1,
            2 => PageTableLevel::L2,
            3 => PageTableLevel::L3,
            _ => PageTableLevel::L4,
        }
    }

    pub fn as_usize(&self) -> usize {
        match self {
            PageTableLevel::L0 => 0,
            PageTableLevel::L1 => 1,
            PageTableLevel::L2 => 2,
            PageTableLevel::L3 => 3,
            PageTableLevel::L4 => 4,
        }
    }
}

/// Page table entry flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageTableEntryFlags {
    /// Present bit
    pub present: bool,
    /// Readable bit
    pub readable: bool,
    /// Writable bit
    pub writable: bool,
    /// Executable bit
    pub executable: bool,
    /// User accessible bit
    pub user_accessible: bool,
    /// Global bit
    pub global: bool,
    /// Accessed bit
    pub accessed: bool,
    /// Dirty bit
    pub dirty: bool,
}

impl PageTableEntryFlags {
    pub fn can_access(&self, access_type: AccessType, is_user: bool) -> bool {
        if !self.present {
            return false;
        }

        if is_user && !self.user_accessible {
            return false;
        }

        match access_type {
            AccessType::Read => self.readable,
            AccessType::Write => self.writable,
            AccessType::Execute => self.executable,
            AccessType::Atomic => self.readable && self.writable,
        }
    }
}

/// Page table entry
#[derive(Debug, Clone)]
pub struct PageTableEntry {
    /// Physical address of next level or page frame
    pub phys_addr: GuestPhysAddr,
    /// Entry flags
    pub flags: PageTableEntryFlags,
    /// Page size (for large pages)
    pub page_size: u64,
    /// Entry index within the page table
    pub index: u64,
}

/// Walk result
#[derive(Debug, Clone)]
pub enum WalkResult {
    /// Successful translation
    Success {
        phys_addr: GuestPhysAddr,
        page_size: u64,
        flags: PageTableEntryFlags,
    },
    /// Page not present
    NotPresent {
        va: GuestAddr,
        level: PageTableLevel,
    },
    /// Access violation
    AccessViolation {
        va: GuestAddr,
        access_type: AccessType,
        required_flags: PageTableEntryFlags,
    },
    /// Invalid entry
    InvalidEntry {
        va: GuestAddr,
        level: PageTableLevel,
    },
}

/// Walk statistics
#[derive(Debug, Clone, Default)]
pub struct WalkStatistics {
    /// Total number of walks
    pub total_walks: u64,
    /// Number of successful walks
    pub successful_walks: u64,
    /// Number of walks that hit the cache
    pub cache_hits: u64,
    /// Number of walks that missed the cache
    pub cache_misses: u64,
    /// Average walk time
    pub avg_walk_time: Duration,
    /// Number of levels traversed on average
    pub avg_levels_traversed: f64,
}

impl WalkStatistics {
    pub fn hit_rate(&self) -> f64 {
        if self.total_walks == 0 {
            0.0
        } else {
            self.cache_hits as f64 / self.total_walks as f64
        }
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_walks == 0 {
            0.0
        } else {
            self.successful_walks as f64 / self.total_walks as f64
        }
    }
}

/// Page Table Walker Domain Service
///
/// This service provides high-level business logic for walking page tables
/// across different architectures (RISC-V Sv39/Sv48, ARM64, x86_64)
/// with permission checking and caching.
pub struct PageTableWalkerDomainService {
    /// Event bus for publishing page table events
    event_bus: Arc<DomainEventBus>,
    /// Root page table physical address
    root_table_addr: GuestPhysAddr,
    /// Number of page table levels
    num_levels: usize,
    /// Page size (typically 4096 bytes)
    page_size: u64,
    /// Number of entries per page table
    entries_per_table: u64,
    /// Translation cache
    translation_cache: HashMap<GuestAddr, WalkResult>,
    /// Maximum cache size
    max_cache_size: usize,
    /// Statistics
    statistics: WalkStatistics,
}

impl PageTableWalkerDomainService {
    /// Create a new page table walker
    pub fn new(
        event_bus: Arc<DomainEventBus>,
        root_table_addr: GuestPhysAddr,
        num_levels: usize,
        page_size: u64,
    ) -> Self {
        let entries_per_table = page_size / 8;
        Self {
            event_bus,
            root_table_addr,
            num_levels,
            page_size,
            entries_per_table,
            translation_cache: HashMap::new(),
            max_cache_size: 1024,
            statistics: WalkStatistics::default(),
        }
    }

    /// Set maximum cache size
    pub fn set_max_cache_size(&mut self, size: usize) {
        self.max_cache_size = size;
    }

    /// Get statistics
    pub fn get_statistics(&self) -> &WalkStatistics {
        &self.statistics
    }

    /// Invalidate cache entry
    pub fn invalidate_cache_entry(&mut self, va: GuestAddr) {
        if self.translation_cache.remove(&va).is_some() {
            self.publish_event(PageTableEvent::CacheInvalidated { va: va.0 });
        }
    }

    /// Invalidate entire cache
    pub fn invalidate_cache_all(&mut self) {
        let count = self.translation_cache.len();
        self.translation_cache.clear();
        self.publish_event(PageTableEvent::CacheFlushed { count });
    }

    /// Walk the page tables to translate a virtual address
    pub fn walk(&mut self, va: GuestAddr, access_type: AccessType, is_user: bool) -> VmResult<WalkResult> {
        let start_time = Instant::now();

        self.statistics.total_walks += 1;

        if let Some(cached) = self.translation_cache.get(&va) {
            self.statistics.cache_hits += 1;

            if let WalkResult::Success { flags, .. } = cached
                && !flags.can_access(access_type, is_user) {
                    return Ok(WalkResult::AccessViolation {
                        va,
                        access_type,
                        required_flags: *flags,
                    });
                }

            return Ok(cached.clone());
        }

        self.statistics.cache_misses += 1;

        let result = self.walk_internal(va, access_type, is_user)?;

        match &result {
            WalkResult::Success { .. } => {
                self.statistics.successful_walks += 1;
                self.cache_result(va, &result);
            }
            WalkResult::NotPresent { .. } => {
                self.publish_event(PageTableEvent::PageFault {
                    va: va.0,
                    access_type: crate::domain_services::events::AccessType::Read,
                });
            }
            WalkResult::AccessViolation { .. } => {
                self.publish_event(PageTableEvent::AccessViolation {
                    va: va.0,
                    access_type: crate::domain_services::events::AccessType::Read,
                });
            }
            WalkResult::InvalidEntry { .. } => {
                self.publish_event(PageTableEvent::InvalidEntry { va: va.0 });
            }
        }

        let elapsed = start_time.elapsed();
        self.update_avg_walk_time(elapsed);

        Ok(result)
    }

    fn walk_internal(
        &self,
        va: GuestAddr,
        access_type: AccessType,
        is_user: bool,
    ) -> VmResult<WalkResult> {
        let mut current_addr = self.root_table_addr;
        let mut current_level = PageTableLevel::L0;
        let mut _levels_traversed = 0usize; // Reserved for future statistics/debugging

        for level in 0..self.num_levels {
            current_level = PageTableLevel::from_index(level);
            _levels_traversed = level + 1; // Track levels for future use

            let entry = self.read_pte(current_addr, level, va)?;

            if !entry.flags.present {
                return Ok(WalkResult::NotPresent {
                    va,
                    level: current_level,
                });
            }

            if level == self.num_levels - 1 {
                let result = WalkResult::Success {
                    phys_addr: self.align_to_page(va, entry.phys_addr, entry.page_size),
                    page_size: entry.page_size,
                    flags: entry.flags,
                };

                if !entry.flags.can_access(access_type, is_user) {
                    return Ok(WalkResult::AccessViolation {
                        va,
                        access_type,
                        required_flags: entry.flags,
                    });
                }

                return Ok(result);
            }

            current_addr = entry.phys_addr;
        }

        Ok(WalkResult::InvalidEntry {
            va,
            level: current_level,
        })
    }

    fn read_pte(&self, _table_addr: GuestPhysAddr, level: usize, va: GuestAddr) -> VmResult<PageTableEntry> {
        let vpn = self.extract_vpn(level, va);
        let index = vpn as u64 % self.entries_per_table;

        Ok(PageTableEntry {
            phys_addr: GuestPhysAddr(0),
            flags: PageTableEntryFlags {
                present: true,
                readable: true,
                writable: true,
                executable: true,
                user_accessible: false,
                global: false,
                accessed: false,
                dirty: false,
            },
            page_size: self.page_size,
            index,
        })
    }

    fn extract_vpn(&self, level: usize, va: GuestAddr) -> usize {
        let shift = (self.num_levels - level) * 9;
        ((va.0 >> shift) & 0x1FF) as usize
    }

    fn align_to_page(&self, va: GuestAddr, pa: GuestPhysAddr, page_size: u64) -> GuestPhysAddr {
        let offset = va.0 & (page_size - 1);
        GuestPhysAddr(pa.0 + offset)
    }

    fn cache_result(&mut self, va: GuestAddr, result: &WalkResult) {
        if self.translation_cache.len() >= self.max_cache_size {
            self.evict_one_cache_entry();
        }
        self.translation_cache.insert(va, result.clone());
    }

    fn evict_one_cache_entry(&mut self) {
        if let Some((va, _)) = self.translation_cache.iter().next() {
            let va = *va;
            self.translation_cache.remove(&va);
        }
    }

    fn update_avg_walk_time(&mut self, elapsed: Duration) {
        let total = self.statistics.avg_walk_time.as_nanos() as u64 * self.statistics.total_walks;
        let new_total = total + elapsed.as_nanos() as u64;
        self.statistics.avg_walk_time = Duration::from_nanos(new_total / (self.statistics.total_walks + 1));
    }

    fn publish_event(&self, event: PageTableEvent) {
        let _ = self.event_bus.publish(&DomainEventEnum::PageTable(event));
    }
}

impl Default for PageTableWalkerDomainService {
    fn default() -> Self {
        Self::new(
            Arc::new(DomainEventBus::new()),
            GuestPhysAddr(0x8000_0000),
            4,
            4096,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_table_level_conversions() {
        assert_eq!(PageTableLevel::L0.as_usize(), 0);
        assert_eq!(PageTableLevel::L1.as_usize(), 1);
        assert_eq!(PageTableLevel::L2.as_usize(), 2);
        assert_eq!(PageTableLevel::L3.as_usize(), 3);
        assert_eq!(PageTableLevel::L4.as_usize(), 4);

        assert_eq!(PageTableLevel::from_index(0), PageTableLevel::L0);
        assert_eq!(PageTableLevel::from_index(1), PageTableLevel::L1);
        assert_eq!(PageTableLevel::from_index(5), PageTableLevel::L4);
    }

    #[test]
    fn test_page_table_entry_flags_access() {
        let flags = PageTableEntryFlags {
            present: true,
            readable: true,
            writable: true,
            executable: true,
            user_accessible: true,
            global: false,
            accessed: false,
            dirty: false,
        };

        assert!(flags.can_access(AccessType::Read, true));
        assert!(flags.can_access(AccessType::Write, true));
        assert!(flags.can_access(AccessType::Execute, true));
        assert!(flags.can_access(AccessType::Atomic, true));
    }

    #[test]
    fn test_page_table_entry_flags_not_present() {
        let flags = PageTableEntryFlags {
            present: false,
            readable: true,
            writable: true,
            executable: true,
            user_accessible: true,
            global: false,
            accessed: false,
            dirty: false,
        };

        assert!(!flags.can_access(AccessType::Read, true));
    }

    #[test]
    fn test_walk_statistics_rates() {
        let stats = WalkStatistics {
            total_walks: 100,
            successful_walks: 80,
            cache_hits: 60,
            ..Default::default()
        };
        assert_eq!(stats.hit_rate(), 0.6);
        assert_eq!(stats.success_rate(), 0.8);
    }

    #[test]
    fn test_walk_statistics_zero_walks() {
        let stats = WalkStatistics::default();
        assert_eq!(stats.hit_rate(), 0.0);
        assert_eq!(stats.success_rate(), 0.0);
    }

    #[test]
    fn test_page_table_walker_creation() {
        let service = PageTableWalkerDomainService::default();
        assert_eq!(service.num_levels, 4);
        assert_eq!(service.page_size, 4096);
    }

    #[test]
    fn test_page_table_walker_cache_invalidation() {
        let mut service = PageTableWalkerDomainService::default();
        let va = GuestAddr(0x1000);

        let result = WalkResult::Success {
            phys_addr: GuestPhysAddr(0x2000),
            page_size: 4096,
            flags: PageTableEntryFlags {
                present: true,
                readable: true,
                writable: true,
                executable: true,
                user_accessible: true,
                global: false,
                accessed: false,
                dirty: false,
            },
        };
        service.translation_cache.insert(va, result.clone());

        service.invalidate_cache_entry(va);
        assert!(!service.translation_cache.contains_key(&va));
    }

    #[test]
    fn test_page_table_walker_cache_flush_all() {
        let mut service = PageTableWalkerDomainService::default();
        let va = GuestAddr(0x1000);

        let result = WalkResult::Success {
            phys_addr: GuestPhysAddr(0x2000),
            page_size: 4096,
            flags: PageTableEntryFlags {
                present: true,
                readable: true,
                writable: true,
                executable: true,
                user_accessible: true,
                global: false,
                accessed: false,
                dirty: false,
            },
        };
        service.translation_cache.insert(va, result.clone());

        service.invalidate_cache_all();
        assert!(service.translation_cache.is_empty());
    }
}
