//! TLB Management Domain Service
//!
//! This service encapsulates business logic for Translation Lookaside Buffer (TLB) management
//! including TLB invalidation strategies, multi-level TLB coordination, ASID-based management,
//! and TLB statistics tracking.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::jit::domain_services::events::{DomainEventBus, DomainEventEnum, TlbEvent};
use crate::{AccessType, TlbEntry, VmError, VmResult};

/// TLB level (ITLB, DTLB, L2 TLB, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TlbLevel {
    /// Instruction TLB (L1)
    ITlb,
    /// Data TLB (L1)
    DTlb,
    /// Unified L2 TLB
    L2Tlb,
    /// L3 TLB (if present)
    L3Tlb,
}

impl TlbLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            TlbLevel::ITlb => "ITLB",
            TlbLevel::DTlb => "DTLB",
            TlbLevel::L2Tlb => "L2TLB",
            TlbLevel::L3Tlb => "L3TLB",
        }
    }

    pub fn from_usize(value: usize) -> Self {
        match value {
            0 => TlbLevel::ITlb,
            1 => TlbLevel::DTlb,
            2 => TlbLevel::L2Tlb,
            _ => TlbLevel::L3Tlb,
        }
    }

    pub fn as_usize(&self) -> usize {
        match self {
            TlbLevel::ITlb => 0,
            TlbLevel::DTlb => 1,
            TlbLevel::L2Tlb => 2,
            TlbLevel::L3Tlb => 3,
        }
    }
}

/// TLB replacement policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlbReplacementPolicy {
    /// Least Recently Used
    LRU,
    /// Least Frequently Used
    LFU,
    /// First In First Out
    FIFO,
    /// Random replacement
    Random,
}

/// TLB entry with metadata
#[derive(Debug, Clone)]
pub struct TlbManagedEntry {
    /// Virtual address (page-aligned)
    pub va: u64,
    /// Physical address (page-aligned)
    pub pa: u64,
    /// Address Space Identifier
    pub asid: u16,
    /// Access type permissions
    pub access_type: AccessType,
    /// Entry size
    pub page_size: u64,
    /// Last access timestamp
    pub last_access: Instant,
    /// Access count
    pub access_count: u64,
    /// Entry creation timestamp
    pub created_at: Instant,
}

impl TlbManagedEntry {
    pub fn new(va: u64, pa: u64, asid: u16, access_type: AccessType, page_size: u64) -> Self {
        let now = Instant::now();
        Self {
            va: va & !(page_size - 1),
            pa: pa & !(page_size - 1),
            asid,
            access_type,
            page_size,
            last_access: now,
            access_count: 0,
            created_at: now,
        }
    }
}

/// TLB statistics
#[derive(Debug, Clone, Default)]
pub struct TlbStatistics {
    /// Total number of lookups
    pub total_lookups: u64,
    /// Number of hits
    pub hits: u64,
    /// Number of misses
    pub misses: u64,
    /// Number of evictions
    pub evictions: u64,
    /// Number of flushes
    pub flushes: u64,
    /// Number of ASID flushes
    pub asid_flushes: u64,
    /// Average lookup time
    pub avg_lookup_time: Duration,
}

impl TlbStatistics {
    pub fn hit_rate(&self) -> f64 {
        if self.total_lookups == 0 {
            0.0
        } else {
            self.hits as f64 / self.total_lookups as f64
        }
    }

    pub fn miss_rate(&self) -> f64 {
        if self.total_lookups == 0 {
            0.0
        } else {
            self.misses as f64 / self.total_lookups as f64
        }
    }
}

/// TLB Management Domain Service
///
/// This service provides high-level business logic for managing Translation Lookaside Buffers
/// across different levels (ITLB, DTLB, L2TLB, etc.) with coordinated invalidation
/// and statistics tracking.
pub struct TlbManagementDomainService {
    /// Event bus for publishing TLB events
    event_bus: Arc<DomainEventBus>,
    /// ITLB entries
    itlb: HashMap<(u64, u16), TlbManagedEntry>,
    /// DTLB entries
    dtlb: HashMap<(u64, u16), TlbManagedEntry>,
    /// L2TLB entries
    l2tlb: HashMap<(u64, u16), TlbManagedEntry>,
    /// L3TLB entries
    l3tlb: HashMap<(u64, u16), TlbManagedEntry>,
    /// Replacement policy
    replacement_policy: TlbReplacementPolicy,
    /// LRU tracking
    lru_queues: HashMap<TlbLevel, VecDeque<(u64, u16)>>,
    /// Maximum entries per TLB level
    max_entries: HashMap<TlbLevel, usize>,
    /// Statistics per TLB level
    statistics: HashMap<TlbLevel, TlbStatistics>,
}

impl TlbManagementDomainService {
    pub fn new(event_bus: Arc<DomainEventBus>) -> Self {
        let mut service = Self {
            event_bus,
            itlb: HashMap::new(),
            dtlb: HashMap::new(),
            l2tlb: HashMap::new(),
            l3tlb: HashMap::new(),
            replacement_policy: TlbReplacementPolicy::LRU,
            lru_queues: HashMap::new(),
            max_entries: Self::default_max_entries(),
            statistics: HashMap::new(),
        };

        service.initialize_statistics();
        service
    }

    fn default_max_entries() -> HashMap<TlbLevel, usize> {
        let mut map = HashMap::new();
        map.insert(TlbLevel::ITlb, 128);
        map.insert(TlbLevel::DTlb, 128);
        map.insert(TlbLevel::L2Tlb, 1024);
        map.insert(TlbLevel::L3Tlb, 4096);
        map
    }

    fn initialize_statistics(&mut self) {
        for level in [TlbLevel::ITlb, TlbLevel::DTlb, TlbLevel::L2Tlb, TlbLevel::L3Tlb] {
            self.statistics.insert(level, TlbStatistics::default());
            self.lru_queues.insert(level, VecDeque::new());
        }
    }

    /// Set replacement policy
    pub fn set_replacement_policy(&mut self, policy: TlbReplacementPolicy) {
        self.replacement_policy = policy;
    }

    /// Set maximum entries for a TLB level
    pub fn set_max_entries(&mut self, level: TlbLevel, max: usize) {
        self.max_entries.insert(level, max);
    }

    /// Get statistics for a TLB level
    pub fn get_statistics(&self, level: TlbLevel) -> &TlbStatistics {
        self.statistics.get(&level).unwrap_or(&TlbStatistics::default())
    }

    /// Get combined statistics for all TLB levels
    pub fn get_combined_statistics(&self) -> TlbStatistics {
        let mut combined = TlbStatistics::default();
        for stats in self.statistics.values() {
            combined.total_lookups += stats.total_lookups;
            combined.hits += stats.hits;
            combined.misses += stats.misses;
            combined.evictions += stats.evictions;
            combined.flushes += stats.flushes;
            combined.asid_flushes += stats.asid_flushes;
        }
        combined
    }

    /// Lookup in ITLB
    pub fn lookup_itlb(&mut self, va: u64, asid: u16) -> Option<&TlbManagedEntry> {
        self.lookup_internal(TlbLevel::ITlb, va, asid)
    }

    /// Lookup in DTLB
    pub fn lookup_dtlb(&mut self, va: u64, asid: u16) -> Option<&TlbManagedEntry> {
        self.lookup_internal(TlbLevel::DTlb, va, asid)
    }

    /// Lookup in L2TLB
    pub fn lookup_l2tlb(&mut self, va: u64, asid: u16) -> Option<&TlbManagedEntry> {
        self.lookup_internal(TlbLevel::L2Tlb, va, asid)
    }

    /// Lookup in L3TLB
    pub fn lookup_l3tlb(&mut self, va: u64, asid: u16) -> Option<&TlbManagedEntry> {
        self.lookup_internal(TlbLevel::L3Tlb, va, asid)
    }

    fn lookup_internal(&mut self, level: TlbLevel, va: u64, asid: u16) -> Option<&TlbManagedEntry> {
        let key = (va, asid);
        let tlb = self.get_tlb_mut(level);
        let stats = self.statistics.get_mut(&level)?;

        stats.total_lookups += 1;

        if let Some(entry) = tlb.get_mut(&key) {
            stats.hits += 1;
            entry.last_access = Instant::now();
            entry.access_count += 1;
            self.update_lru(level, &key);
            Some(entry)
        } else {
            stats.misses += 1;
            None
        }
    }

    /// Insert into ITLB
    pub fn insert_itlb(&mut self, entry: TlbManagedEntry) -> VmResult<()> {
        self.insert_internal(TlbLevel::ITlb, entry)
    }

    /// Insert into DTLB
    pub fn insert_dtlb(&mut self, entry: TlbManagedEntry) -> VmResult<()> {
        self.insert_internal(TlbLevel::DTlb, entry)
    }

    /// Insert into L2TLB
    pub fn insert_l2tlb(&mut self, entry: TlbManagedEntry) -> VmResult<()> {
        self.insert_internal(TlbLevel::L2Tlb, entry)
    }

    /// Insert into L3TLB
    pub fn insert_l3tlb(&mut self, entry: TlbManagedEntry) -> VmResult<()> {
        self.insert_internal(TlbLevel::L3Tlb, entry)
    }

    fn insert_internal(&mut self, level: TlbLevel, entry: TlbManagedEntry) -> VmResult<()> {
        let key = (entry.va, entry.asid);
        let tlb = self.get_tlb_mut(level);
        let max = *self.max_entries.get(&level).unwrap_or(&1024);

        if tlb.len() >= max {
            self.evict_one(level)?;
        }

        tlb.insert(key, entry.clone());
        self.add_to_lru(level, &key);

        self.publish_event(TlbEvent::EntryInserted {
            level,
            va: entry.va,
            asid: entry.asid,
        });

        Ok(())
    }

    fn evict_one(&mut self, level: TlbLevel) -> VmResult<()> {
        match self.replacement_policy {
            TlbReplacementPolicy::LRU => self.evict_lru(level),
            TlbReplacementPolicy::LFU => self.evict_lfu(level),
            TlbReplacementPolicy::FIFO => self.evict_fifo(level),
            TlbReplacementPolicy::Random => self.evict_random(level),
        }
    }

    fn evict_lru(&mut self, level: TlbLevel) -> VmResult<()> {
        let queue = self.lru_queues.get_mut(&level).ok_or(VmError::InvalidState)?;
        if let Some(key) = queue.pop_front() {
            let tlb = self.get_tlb_mut(level);
            if let Some(entry) = tlb.remove(&key) {
                let stats = self.statistics.get_mut(&level).ok_or(VmError::InvalidState)?;
                stats.evictions += 1;

                self.publish_event(TlbEvent::EntryEvicted {
                    level,
                    va: entry.va,
                    asid: entry.asid,
                });
            }
        }
        Ok(())
    }

    fn evict_lfu(&mut self, level: TlbLevel) -> VmResult<()> {
        let tlb = self.get_tlb_mut(level);
        if let Some((&key, _)) = tlb.iter().min_by_key(|(_, e)| e.access_count) {
            let entry = tlb.remove(&key).ok_or(VmError::InvalidState)?;
            let stats = self.statistics.get_mut(&level).ok_or(VmError::InvalidState)?;
            stats.evictions += 1;

            self.publish_event(TlbEvent::EntryEvicted {
                level,
                va: entry.va,
                asid: entry.asid,
            });
        }
        Ok(())
    }

    fn evict_fifo(&mut self, level: TlbLevel) -> VmResult<()> {
        self.evict_lru(level)
    }

    fn evict_random(&mut self, level: TlbLevel) -> VmResult<()> {
        use std::collections::hash_map::RandomState;
        let tlb = self.get_tlb_mut(level);
        if let Some((&key, _)) = tlb.iter().next() {
            let entry = tlb.remove(&key).ok_or(VmError::InvalidState)?;
            let stats = self.statistics.get_mut(&level).ok_or(VmError::InvalidState)?;
            stats.evictions += 1;

            self.publish_event(TlbEvent::EntryEvicted {
                level,
                va: entry.va,
                asid: entry.asid,
            });
        }
        Ok(())
    }

    /// Flush a specific entry
    pub fn flush_entry(&mut self, level: TlbLevel, va: u64, asid: u16) -> VmResult<()> {
        let key = (va, asid);
        let tlb = self.get_tlb_mut(level);
        if tlb.remove(&key).is_some() {
            let stats = self.statistics.get_mut(&level).ok_or(VmError::InvalidState)?;
            stats.flushes += 1;

            self.publish_event(TlbEvent::EntryFlushed {
                level,
                va,
                asid,
            });
        }
        Ok(())
    }

    /// Flush all entries in a TLB level
    pub fn flush_all(&mut self, level: TlbLevel) -> VmResult<()> {
        let tlb = self.get_tlb_mut(level);
        let count = tlb.len();
        tlb.clear();
        self.lru_queues.entry(level).or_default().clear();

        let stats = self.statistics.get_mut(&level).ok_or(VmError::InvalidState)?;
        stats.flushes += count as u64;

        self.publish_event(TlbEvent::FlushAll { level });
        Ok(())
    }

    /// Flush all TLB levels
    pub fn flush_all_levels(&mut self) -> VmResult<()> {
        for level in [TlbLevel::ITlb, TlbLevel::DTlb, TlbLevel::L2Tlb, TlbLevel::L3Tlb] {
            self.flush_all(level)?;
        }
        Ok(())
    }

    /// Flush by ASID
    pub fn flush_asid(&mut self, asid: u16) -> VmResult<()> {
        for level in [TlbLevel::ITlb, TlbLevel::DTlb, TlbLevel::L2Tlb, TlbLevel::L3Tlb] {
            let tlb = self.get_tlb_mut(level);
            let keys: Vec<_> = tlb.keys().filter(|(_, a)| *a == asid).copied().collect();
            for key in keys {
                tlb.remove(&key);
            }

            let stats = self.statistics.get_mut(&level).ok_or(VmError::InvalidState)?;
            stats.asid_flushes += 1;
        }

        self.publish_event(TlbEvent::FlushAsid { asid });
        Ok(())
    }

    /// Flush by virtual address range
    pub fn flush_range(&mut self, start_va: u64, end_va: u64) -> VmResult<()> {
        for level in [TlbLevel::ITlb, TlbLevel::DTlb, TlbLevel::L2Tlb, TlbLevel::L3Tlb] {
            let tlb = self.get_tlb_mut(level);
            let keys: Vec<_> = tlb.keys()
                .filter(|(va, _)| *va >= start_va && *va < end_va)
                .copied()
                .collect();
            for key in keys {
                tlb.remove(&key);
            }

            let stats = self.statistics.get_mut(&level).ok_or(VmError::InvalidState)?;
            stats.flushes += keys.len() as u64;
        }

        self.publish_event(TlbEvent::FlushRange { start_va, end_va });
        Ok(())
    }

    /// Invalidate by physical address (when page tables change)
    pub fn invalidate_by_pa(&mut self, pa: u64) -> VmResult<()> {
        for level in [TlbLevel::ITlb, TlbLevel::DTlb, TlbLevel::L2Tlb, TlbLevel::L3Tlb] {
            let tlb = self.get_tlb_mut(level);
            let keys: Vec<_> = tlb.iter()
                .filter(|(_, e)| e.pa == pa)
                .map(|(k, _)| *k)
                .collect();
            for key in keys {
                tlb.remove(&key);
            }

            let stats = self.statistics.get_mut(&level).ok_or(VmError::InvalidState)?;
            stats.flushes += keys.len() as u64;
        }

        self.publish_event(TlbEvent::InvalidatePa { pa });
        Ok(())
    }

    fn get_tlb_mut(&mut self, level: TlbLevel) -> &mut HashMap<(u64, u16), TlbManagedEntry> {
        match level {
            TlbLevel::ITlb => &mut self.itlb,
            TlbLevel::DTlb => &mut self.dtlb,
            TlbLevel::L2Tlb => &mut self.l2tlb,
            TlbLevel::L3Tlb => &mut self.l3tlb,
        }
    }

    fn update_lru(&mut self, level: TlbLevel, key: &(u64, u16)) {
        let queue = self.lru_queues.entry(level).or_default();
        if let Some(pos) = queue.iter().position(|k| k == key) {
            queue.remove(pos);
        }
        queue.push_back(*key);
    }

    fn add_to_lru(&mut self, level: TlbLevel, key: &(u64, u16)) {
        let queue = self.lru_queues.entry(level).or_default();
        queue.push_back(*key);
    }

    fn publish_event(&self, event: TlbEvent) {
        if let Ok(bus) = self.event_bus.lock() {
            let _ = bus.publish(DomainEventEnum::Tlb(event));
        }
    }
}

impl Default for TlbManagementDomainService {
    fn default() -> Self {
        Self::new(Arc::new(DomainEventBus::new()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tlb_level_conversions() {
        assert_eq!(TlbLevel::ITlb.as_str(), "ITLB");
        assert_eq!(TlbLevel::DTlb.as_str(), "DTLB");
        assert_eq!(TlbLevel::L2Tlb.as_str(), "L2TLB");
        assert_eq!(TlbLevel::L3Tlb.as_str(), "L3TLB");

        assert_eq!(TlbLevel::ITlb.as_usize(), 0);
        assert_eq!(TlbLevel::DTlb.as_usize(), 1);
        assert_eq!(TlbLevel::L2Tlb.as_usize(), 2);
        assert_eq!(TlbLevel::L3Tlb.as_usize(), 3);

        assert_eq!(TlbLevel::from_usize(0), TlbLevel::ITlb);
        assert_eq!(TlbLevel::from_usize(1), TlbLevel::DTlb);
        assert_eq!(TlbLevel::from_usize(2), TlbLevel::L2Tlb);
        assert_eq!(TlbLevel::from_usize(5), TlbLevel::L3Tlb);
    }

    #[test]
    fn test_tlb_replacement_policy_equality() {
        assert_eq!(TlbReplacementPolicy::LRU, TlbReplacementPolicy::LRU);
        assert_eq!(TlbReplacementPolicy::LFU, TlbReplacementPolicy::LFU);
        assert_eq!(TlbReplacementPolicy::FIFO, TlbReplacementPolicy::FIFO);
        assert_eq!(TlbReplacementPolicy::Random, TlbReplacementPolicy::Random);

        assert_ne!(TlbReplacementPolicy::LRU, TlbReplacementPolicy::LFU);
    }

    #[test]
    fn test_tlb_managed_entry_creation() {
        let entry = TlbManagedEntry::new(0x1000, 0x2000, 1, AccessType::Read, 0x1000);
        assert_eq!(entry.va, 0x1000);
        assert_eq!(entry.pa, 0x2000);
        assert_eq!(entry.asid, 1);
        assert_eq!(entry.access_type, AccessType::Read);
        assert_eq!(entry.page_size, 0x1000);
        assert_eq!(entry.access_count, 0);
    }

    #[test]
    fn test_tlb_statistics_hit_rate() {
        let stats = TlbStatistics {
            total_lookups: 100,
            hits: 80,
            misses: 20,
            ..Default::default()
        };
        assert_eq!(stats.hit_rate(), 0.8);
        assert_eq!(stats.miss_rate(), 0.2);
    }

    #[test]
    fn test_tlb_statistics_zero_lookups() {
        let stats = TlbStatistics::default();
        assert_eq!(stats.hit_rate(), 0.0);
        assert_eq!(stats.miss_rate(), 0.0);
    }

    #[test]
    fn test_tlb_manager_creation() {
        let service = TlbManagementDomainService::default();
        assert_eq!(service.replacement_policy, TlbReplacementPolicy::LRU);
    }

    #[test]
    fn test_tlb_manager_insert_and_lookup() {
        let mut service = TlbManagementDomainService::default();
        let entry = TlbManagedEntry::new(0x1000, 0x2000, 1, AccessType::Read, 0x1000);

        assert!(service.insert_itlb(entry).is_ok());
        assert!(service.lookup_itlb(0x1000, 1).is_some());
    }

    #[test]
    fn test_tlb_manager_flush_all() {
        let mut service = TlbManagementDomainService::default();
        let entry = TlbManagedEntry::new(0x1000, 0x2000, 1, AccessType::Read, 0x1000);

        assert!(service.insert_itlb(entry).is_ok());
        assert!(service.flush_all(TlbLevel::ITlb).is_ok());
        assert!(service.lookup_itlb(0x1000, 1).is_none());
    }
}
