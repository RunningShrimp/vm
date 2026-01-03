//! 多级 TLB 管理实现
//!
//! 提供 ITLB、DTLB、L2TLB、L3TLB 等多级 TLB 的协调管理。
//! 这是基础设施层的实现，具体的技术细节应在此模块中。

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use vm_core::{
    AccessType, CoreError, GuestAddr, GuestPhysAddr, TlbEntry, TlbManager, TlbStats, VmResult,
};

/// TLB 级别
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

/// TLB 替换策略
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

/// TLB 条目（带元数据）
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

    /// 转换为 TlbEntry
    pub fn to_tlb_entry(&self) -> TlbEntry {
        TlbEntry {
            guest_addr: GuestAddr(self.va),
            phys_addr: GuestPhysAddr(self.pa),
            flags: 0, // 需要从其他地方获取标志
            asid: self.asid,
        }
    }
}

/// TLB 统计信息
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

    /// 转换为 TlbStats（领域层类型）
    pub fn to_tlb_stats(&self, current_entries: usize, capacity: usize) -> TlbStats {
        TlbStats {
            total_lookups: self.total_lookups,
            hits: self.hits,
            misses: self.misses,
            hit_rate: self.hit_rate(),
            current_entries,
            capacity,
        }
    }
}

/// 多级 TLB 管理器（基础设施层实现）
///
/// 管理 ITLB、DTLB、L2TLB、L3TLB 等多级 TLB。
/// 这是基础设施层的具体实现，包含技术细节。
pub struct MultiLevelTlbManager {
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

impl MultiLevelTlbManager {
    pub fn new() -> Self {
        let mut manager = Self {
            itlb: HashMap::new(),
            dtlb: HashMap::new(),
            l2tlb: HashMap::new(),
            l3tlb: HashMap::new(),
            replacement_policy: TlbReplacementPolicy::LRU,
            lru_queues: HashMap::new(),
            max_entries: Self::default_max_entries(),
            statistics: HashMap::new(),
        };

        manager.initialize_statistics();
        manager
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
        for level in [
            TlbLevel::ITlb,
            TlbLevel::DTlb,
            TlbLevel::L2Tlb,
            TlbLevel::L3Tlb,
        ] {
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
    pub fn get_statistics(&self, level: TlbLevel) -> TlbStatistics {
        self.statistics.get(&level).cloned().unwrap_or_default()
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

        // 使用原始的get_mut_mut来避免借用冲突
        let tlb_ptr: *mut HashMap<(u64, u16), TlbManagedEntry> = match level {
            TlbLevel::ITlb => &mut self.itlb as *mut _,
            TlbLevel::DTlb => &mut self.dtlb as *mut _,
            TlbLevel::L2Tlb => &mut self.l2tlb as *mut _,
            TlbLevel::L3Tlb => &mut self.l3tlb as *mut _,
        };

        // 更新统计信息
        if let Some(stats) = self.statistics.get_mut(&level) {
            stats.total_lookups += 1;
        }

        // 查找条目
        unsafe {
            let tlb = &mut *tlb_ptr;
            if let Some(entry) = tlb.get_mut(&key) {
                if let Some(stats) = self.statistics.get_mut(&level) {
                    stats.hits += 1;
                }
                entry.last_access = Instant::now();
                entry.access_count += 1;

                // 返回引用
                Some(entry)
            } else {
                if let Some(stats) = self.statistics.get_mut(&level) {
                    stats.misses += 1;
                }
                None
            }
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

        // 先获取最大条目数
        let max = *self.max_entries.get(&level).unwrap_or(&1024);

        // 获取当前TLB长度
        let current_len = {
            let tlb = self.get_tlb_ref(level);
            tlb.len()
        };

        // 如果需要驱逐
        if current_len >= max {
            self.evict_one(level)?;
        }

        // 插入新条目
        {
            let tlb = self.get_tlb_mut(level);
            tlb.insert(key, entry.clone());
        }

        // 更新LRU
        self.add_to_lru(level, &key);

        Ok(())
    }

    fn evict_one(&mut self, level: TlbLevel) -> VmResult<()> {
        match self.replacement_policy {
            TlbReplacementPolicy::LRU => self.evict_lru(level),
            TlbReplacementPolicy::LFU => self.evict_lfu(level),
            TlbReplacementPolicy::FIFO => self.evict_lru(level), // FIFO 使用 LRU 队列
            TlbReplacementPolicy::Random => self.evict_random(level),
        }
    }

    fn evict_lru(&mut self, level: TlbLevel) -> VmResult<()> {
        let queue = self
            .lru_queues
            .get_mut(&level)
            .ok_or_else(|| CoreError::InvalidState {
                message: format!("TLB level {:?} not initialized", level),
                current: "Unknown".to_string(),
                expected: "initialized".to_string(),
            })?;
        if let Some(key) = queue.pop_front() {
            let tlb = self.get_tlb_mut(level);
            if let Some(_entry) = tlb.remove(&key) {
                let stats =
                    self.statistics
                        .get_mut(&level)
                        .ok_or_else(|| CoreError::InvalidState {
                            message: format!("TLB level {:?} not found", level),
                            current: "Unknown".to_string(),
                            expected: "initialized".to_string(),
                        })?;
                stats.evictions += 1;
            }
        }
        Ok(())
    }

    fn evict_lfu(&mut self, level: TlbLevel) -> VmResult<()> {
        let tlb = self.get_tlb_mut(level);
        if let Some((&key, _)) = tlb.iter().min_by_key(|(_, e)| e.access_count) {
            let _entry = tlb.remove(&key).ok_or_else(|| CoreError::InvalidState {
                message: format!("TLB level {:?} not found", level),
                current: "Unknown".to_string(),
                expected: "initialized".to_string(),
            })?;
            let stats = self
                .statistics
                .get_mut(&level)
                .ok_or_else(|| CoreError::InvalidState {
                    message: format!("TLB level {:?} not found", level),
                    current: "Unknown".to_string(),
                    expected: "initialized".to_string(),
                })?;
            stats.evictions += 1;
        }
        Ok(())
    }

    fn evict_random(&mut self, level: TlbLevel) -> VmResult<()> {
        let tlb = self.get_tlb_mut(level);
        if let Some((&key, _)) = tlb.iter().next() {
            let _entry = tlb.remove(&key).ok_or_else(|| CoreError::InvalidState {
                message: format!("TLB level {:?} not found", level),
                current: "Unknown".to_string(),
                expected: "initialized".to_string(),
            })?;
            let stats = self
                .statistics
                .get_mut(&level)
                .ok_or_else(|| CoreError::InvalidState {
                    message: format!("TLB level {:?} not found", level),
                    current: "Unknown".to_string(),
                    expected: "initialized".to_string(),
                })?;
            stats.evictions += 1;
        }
        Ok(())
    }

    /// Flush all entries in a TLB level
    pub fn flush_all(&mut self, level: TlbLevel) -> VmResult<()> {
        let tlb = self.get_tlb_mut(level);
        let count = tlb.len();
        tlb.clear();
        self.lru_queues.entry(level).or_default().clear();

        let stats = self
            .statistics
            .get_mut(&level)
            .ok_or_else(|| CoreError::InvalidState {
                message: format!("TLB level {:?} not found", level),
                current: "Unknown".to_string(),
                expected: "initialized".to_string(),
            })?;
        stats.flushes += count as u64;
        Ok(())
    }

    /// Flush all TLB levels
    pub fn flush_all_levels(&mut self) -> VmResult<()> {
        for level in [
            TlbLevel::ITlb,
            TlbLevel::DTlb,
            TlbLevel::L2Tlb,
            TlbLevel::L3Tlb,
        ] {
            self.flush_all(level)?;
        }
        Ok(())
    }

    /// Flush by ASID
    pub fn flush_asid(&mut self, asid: u16) -> VmResult<()> {
        for level in [
            TlbLevel::ITlb,
            TlbLevel::DTlb,
            TlbLevel::L2Tlb,
            TlbLevel::L3Tlb,
        ] {
            let tlb = self.get_tlb_mut(level);
            let keys: Vec<_> = tlb.keys().filter(|(_, a)| *a == asid).copied().collect();
            for key in keys {
                tlb.remove(&key);
            }

            let stats = self
                .statistics
                .get_mut(&level)
                .ok_or_else(|| CoreError::InvalidState {
                    message: format!("TLB level {:?} not found", level),
                    current: "Unknown".to_string(),
                    expected: "initialized".to_string(),
                })?;
            stats.asid_flushes += 1;
        }
        Ok(())
    }

    fn get_tlb_ref(&self, level: TlbLevel) -> &HashMap<(u64, u16), TlbManagedEntry> {
        match level {
            TlbLevel::ITlb => &self.itlb,
            TlbLevel::DTlb => &self.dtlb,
            TlbLevel::L2Tlb => &self.l2tlb,
            TlbLevel::L3Tlb => &self.l3tlb,
        }
    }

    fn get_tlb_mut(&mut self, level: TlbLevel) -> &mut HashMap<(u64, u16), TlbManagedEntry> {
        match level {
            TlbLevel::ITlb => &mut self.itlb,
            TlbLevel::DTlb => &mut self.dtlb,
            TlbLevel::L2Tlb => &mut self.l2tlb,
            TlbLevel::L3Tlb => &mut self.l3tlb,
        }
    }

    #[allow(dead_code)] // Helper for LRU cache management
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
}

impl Default for MultiLevelTlbManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 实现 TlbManager trait（领域层接口）
///
/// 将多级 TLB 管理器适配到领域层定义的 TlbManager trait。
/// 默认使用 L2TLB 进行查找和更新（作为统一 TLB）。
impl TlbManager for MultiLevelTlbManager {
    fn lookup(&mut self, addr: GuestAddr, asid: u16, access: AccessType) -> Option<TlbEntry> {
        let va = addr.0;

        // 根据访问类型选择 TLB 级别
        let level = match access {
            AccessType::Execute => TlbLevel::ITlb,
            AccessType::Read | AccessType::Write | AccessType::Atomic => TlbLevel::DTlb,
        };

        // 先查找指定级别的 TLB
        if let Some(entry) = self.lookup_internal(level, va, asid) {
            return Some(TlbEntry {
                guest_addr: GuestAddr(entry.va),
                phys_addr: GuestPhysAddr(entry.pa),
                flags: 0, // 需要从其他地方获取标志
                asid: entry.asid,
            });
        }

        // 如果未命中，查找 L2TLB（统一 TLB）
        if let Some(entry) = self.lookup_internal(TlbLevel::L2Tlb, va, asid) {
            return Some(TlbEntry {
                guest_addr: GuestAddr(entry.va),
                phys_addr: GuestPhysAddr(entry.pa),
                flags: 0,
                asid: entry.asid,
            });
        }

        // 最后查找 L3TLB
        if let Some(entry) = self.lookup_internal(TlbLevel::L3Tlb, va, asid) {
            return Some(TlbEntry {
                guest_addr: GuestAddr(entry.va),
                phys_addr: GuestPhysAddr(entry.pa),
                flags: 0,
                asid: entry.asid,
            });
        }

        None
    }

    fn update(&mut self, entry: TlbEntry) {
        let va = entry.guest_addr.0;
        let pa = entry.phys_addr.0;
        let asid = entry.asid;

        // 创建 TlbManagedEntry
        let managed_entry = TlbManagedEntry::new(
            va,
            pa,
            asid,
            AccessType::Read, // 默认访问类型，实际应该从 flags 推断
            4096,             // 默认页面大小
        );

        // 更新到 L2TLB（统一 TLB）
        let _ = self.insert_internal(TlbLevel::L2Tlb, managed_entry);
    }

    fn flush(&mut self) {
        let _ = self.flush_all_levels();
    }

    fn flush_asid(&mut self, asid: u16) {
        // 调用公共方法刷新所有级别的 ASID
        let _ = MultiLevelTlbManager::flush_asid(self, asid);
    }

    fn flush_page(&mut self, va: GuestAddr) {
        let va = va.0;
        // 刷新所有级别的 TLB 中匹配的条目
        for level in [
            TlbLevel::ITlb,
            TlbLevel::DTlb,
            TlbLevel::L2Tlb,
            TlbLevel::L3Tlb,
        ] {
            let tlb = self.get_tlb_mut(level);
            let keys: Vec<_> = tlb.keys().filter(|(v, _)| *v == va).copied().collect();
            for key in keys {
                tlb.remove(&key);
            }
        }
    }

    fn get_stats(&self) -> Option<TlbStats> {
        let combined = self.get_combined_statistics();
        let total_capacity: usize = self.max_entries.values().sum();
        let total_entries: usize =
            self.itlb.len() + self.dtlb.len() + self.l2tlb.len() + self.l3tlb.len();

        Some(combined.to_tlb_stats(total_entries, total_capacity))
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
    }

    #[test]
    fn test_multilevel_tlb_manager_creation() {
        let manager = MultiLevelTlbManager::new();
        assert_eq!(manager.replacement_policy, TlbReplacementPolicy::LRU);
    }

    #[test]
    fn test_multilevel_tlb_insert_and_lookup() {
        let mut manager = MultiLevelTlbManager::new();
        let entry = TlbManagedEntry::new(0x1000, 0x2000, 1, AccessType::Read, 0x1000);

        assert!(manager.insert_itlb(entry).is_ok());
        assert!(manager.lookup_itlb(0x1000, 1).is_some());
    }

    #[test]
    fn test_multilevel_tlb_flush_all() {
        let mut manager = MultiLevelTlbManager::new();
        let entry = TlbManagedEntry::new(0x1000, 0x2000, 1, AccessType::Read, 0x1000);

        assert!(manager.insert_itlb(entry).is_ok());
        assert!(manager.flush_all(TlbLevel::ITlb).is_ok());
        assert!(manager.lookup_itlb(0x1000, 1).is_none());
    }
}
