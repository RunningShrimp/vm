//! Per-CPU TLB实现
//!
//! 为每个CPU核心提供独立的TLB，减少锁竞争和提高性能

use crate::GuestAddr;
use crate::mmu::{PageTableFlags, PageWalkResult};
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::thread::{self, ThreadId};
use std::time::Instant;
use vm_core::error::{MemoryError, VmError};

/// Type alias for TLB operations result
pub type PerCpuTlbResult<T> = Result<T, VmError>;

/// Per-CPU TLB条目
#[derive(Debug, Clone)]
pub struct PerCpuTlbEntry {
    /// Guest虚拟地址（页对齐）
    pub gva: GuestAddr,
    /// Guest物理地址
    pub gpa: GuestAddr,
    /// 页面大小
    pub page_size: u64,
    /// 页表标志
    pub flags: PageTableFlags,
    /// ASID
    pub asid: u16,
    /// 访问计数
    pub access_count: u64,
    /// 最后访问时间
    pub last_access: Instant,
    /// 引用位（用于时钟算法）
    pub reference_bit: bool,
    /// 是否为全局映射
    pub global: bool,
}

impl PerCpuTlbEntry {
    /// 检查地址是否在此条目范围内
    pub fn contains(&self, gva: GuestAddr) -> bool {
        let page_base = self.gva & !(self.page_size - 1);
        let gva_base = gva & !(self.page_size - 1);
        page_base == gva_base
    }

    /// 翻译地址
    pub fn translate(&self, gva: GuestAddr) -> GuestAddr {
        let offset = gva & (self.page_size - 1);
        self.gpa + offset
    }

    /// 更新访问信息
    pub fn update_access(&mut self) {
        self.access_count += 1;
        self.last_access = Instant::now();
        self.reference_bit = true;
    }
}

/// Per-CPU TLB配置
#[derive(Debug, Clone)]
pub struct PerCpuTlbConfig {
    /// 每个CPU的TLB大小
    pub entries_per_cpu: usize,
    /// 最大CPU数量
    pub max_cpus: usize,
    /// 替换策略
    pub replacement_policy: TlbReplacementPolicy,
    /// 是否启用统计信息
    pub enable_stats: bool,
    /// 是否启用自适应大小调整
    pub enable_adaptive_sizing: bool,
    /// 最小TLB大小
    pub min_size: usize,
    /// 最大TLB大小
    pub max_size: usize,
    /// 命中率阈值（用于自适应调整）
    pub hit_rate_threshold: f64,
}

impl Default for PerCpuTlbConfig {
    fn default() -> Self {
        Self {
            entries_per_cpu: 1024,
            max_cpus: 64,
            replacement_policy: TlbReplacementPolicy::AdaptiveLru,
            enable_stats: true,
            enable_adaptive_sizing: true,
            min_size: 256,
            max_size: 4096,
            hit_rate_threshold: 0.85,
        }
    }
}

/// TLB替换策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlbReplacementPolicy {
    /// 随机替换
    Random,
    /// 最近最少使用 (LRU)
    Lru,
    /// 先进先出 (FIFO)
    Fifo,
    /// 自适应LRU（结合访问频率和时间）
    AdaptiveLru,
    /// 时钟算法 (Clock Algorithm)
    Clock,
}

/// Per-CPU TLB统计信息
#[derive(Debug, Default)]
pub struct PerCpuTlbStats {
    /// 总访问次数
    pub total_accesses: AtomicU64,
    /// 命中次数
    pub hits: AtomicU64,
    /// 未命中次数
    pub misses: AtomicU64,
    /// 刷新次数
    pub flushes: AtomicU64,
    /// 替换次数
    pub evictions: AtomicU64,
    /// ASID切换次数
    pub asid_switches: AtomicU64,
    /// 全局映射次数
    pub global_mappings: AtomicU64,
    /// 当前条目数
    pub current_entries: AtomicUsize,
    /// 最大条目数
    pub max_entries: AtomicUsize,
}

impl PerCpuTlbStats {
    /// 获取统计信息快照
    pub fn snapshot(&self) -> PerCpuTlbStatsSnapshot {
        let total = self.total_accesses.load(Ordering::Relaxed);
        PerCpuTlbStatsSnapshot {
            cpu_id: 0,
            total_accesses: total,
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            flushes: self.flushes.load(Ordering::Relaxed),
            evictions: self.evictions.load(Ordering::Relaxed),
            asid_switches: self.asid_switches.load(Ordering::Relaxed),
            global_mappings: self.global_mappings.load(Ordering::Relaxed),
            current_entries: self.current_entries.load(Ordering::Relaxed),
            max_entries: self.max_entries.load(Ordering::Relaxed),
            hit_rate: if total > 0 {
                self.hits.load(Ordering::Relaxed) as f64 / total as f64
            } else {
                0.0
            },
        }
    }

    /// 重置统计信息
    pub fn reset(&self) {
        self.total_accesses.store(0, Ordering::Relaxed);
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
        self.flushes.store(0, Ordering::Relaxed);
        self.evictions.store(0, Ordering::Relaxed);
        self.asid_switches.store(0, Ordering::Relaxed);
        self.global_mappings.store(0, Ordering::Relaxed);
        self.max_entries.store(
            self.current_entries.load(Ordering::Relaxed),
            Ordering::Relaxed,
        );
    }
}

/// Per-CPU TLB统计信息快照
#[derive(Debug, Clone)]
pub struct PerCpuTlbStatsSnapshot {
    pub cpu_id: usize,
    pub total_accesses: u64,
    pub hits: u64,
    pub misses: u64,
    pub flushes: u64,
    pub evictions: u64,
    pub asid_switches: u64,
    pub global_mappings: u64,
    pub current_entries: usize,
    pub max_entries: usize,
    pub hit_rate: f64,
}

/// 单个CPU的TLB
struct SingleCpuTlb {
    /// CPU ID
    cpu_id: usize,
    /// TLB条目
    entries: HashMap<(GuestAddr, u16), PerCpuTlbEntry>,
    /// LRU列表，存储键以实现高效的LRU替换
    lru_list: Vec<(GuestAddr, u16)>,
    /// 时钟算法的时钟指针
    clock_hand: usize,
    /// 当前容量
    capacity: usize,
    /// 替换策略
    replacement_policy: TlbReplacementPolicy,
    /// 统计信息
    stats: PerCpuTlbStats,
    /// 当前ASID
    current_asid: u16,
}

impl SingleCpuTlb {
    /// 创建新的单CPU TLB
    fn new(cpu_id: usize, capacity: usize, replacement_policy: TlbReplacementPolicy) -> Self {
        Self {
            cpu_id,
            entries: HashMap::with_capacity(capacity),
            lru_list: Vec::with_capacity(capacity),
            clock_hand: 0,
            capacity,
            replacement_policy,
            stats: PerCpuTlbStats::default(),
            current_asid: 0,
        }
    }

    /// 查找TLB条目
    fn lookup(&mut self, gva: GuestAddr, asid: u16) -> Option<&mut PerCpuTlbEntry> {
        self.stats.total_accesses.fetch_add(1, Ordering::Relaxed);

        // 检查ASID是否切换
        if self.current_asid != asid {
            self.stats.asid_switches.fetch_add(1, Ordering::Relaxed);
            self.current_asid = asid;
        }

        let page_base = GuestAddr(gva.0 & !(4096 - 1)); // 页对齐
        let key = (page_base, asid);

        if self.entries.contains_key(&key) {
            self.stats.hits.fetch_add(1, Ordering::Relaxed);

            // 更新LRU列表
            self.update_lru(&key);

            // 获取并更新条目
            if let Some(entry) = self.entries.get_mut(&key) {
                entry.access_count += 1;
                entry.last_access = Instant::now();
                entry.reference_bit = true;
                Some(entry)
            } else {
                None
            }
        } else {
            self.stats.misses.fetch_add(1, Ordering::Relaxed);
            None
        }
    }

    /// 插入TLB条目
    fn insert(&mut self, walk_result: PageWalkResult, gva: GuestAddr, asid: u16) {
        let page_base = GuestAddr(gva.0 & !(walk_result.page_size - 1));
        let key = (page_base, asid);

        let entry = PerCpuTlbEntry {
            gva: page_base,
            gpa: GuestAddr(walk_result.gpa & !(walk_result.page_size - 1)),
            page_size: walk_result.page_size,
            flags: walk_result.flags,
            asid,
            access_count: 1,
            last_access: Instant::now(),
            reference_bit: true,
            global: walk_result.flags.global,
        };

        // 如果是全局映射，增加计数
        if entry.global {
            self.stats.global_mappings.fetch_add(1, Ordering::Relaxed);
        }

        // 检查是否需要替换
        if self.entries.len() >= self.capacity {
            self.evict_entry();
        }

        // 插入新条目
        self.entries.insert(key, entry);
        self.update_lru(&key);

        // 更新当前条目数
        let current = self.entries.len();
        self.stats.current_entries.store(current, Ordering::Relaxed);

        // 更新最大条目数
        let max = self.stats.max_entries.load(Ordering::Relaxed);
        if current > max {
            self.stats.max_entries.store(current, Ordering::Relaxed);
        }
    }

    /// 刷新所有条目
    fn flush_all(&mut self) {
        self.entries.clear();
        self.lru_list.clear();
        self.clock_hand = 0;
        self.stats.flushes.fetch_add(1, Ordering::Relaxed);
        self.stats.current_entries.store(0, Ordering::Relaxed);
    }

    /// 刷新特定ASID的条目
    fn flush_asid(&mut self, asid: u16) {
        self.entries.retain(|_key, entry| entry.asid != asid);
        self.lru_list.retain(|key| key.1 != asid);
        self.stats.flushes.fetch_add(1, Ordering::Relaxed);
        self.stats
            .current_entries
            .store(self.entries.len(), Ordering::Relaxed);
    }

    /// 刷新特定页面的条目
    fn flush_page(&mut self, gva: GuestAddr, asid: u16) {
        let page_base = GuestAddr(gva.0 & !(4096 - 1));
        let key = (page_base, asid);

        if self.entries.remove(&key).is_some() {
            self.lru_list.retain(|k| *k != key);
            self.stats
                .current_entries
                .store(self.entries.len(), Ordering::Relaxed);
        }
    }

    /// 更新LRU列表
    fn update_lru(&mut self, key: &(GuestAddr, u16)) {
        if let Some(pos) = self.lru_list.iter().position(|k| k == key) {
            self.lru_list.remove(pos);
        }
        self.lru_list.push(*key);
    }

    /// 替换条目
    fn evict_entry(&mut self) {
        match self.replacement_policy {
            TlbReplacementPolicy::Random => self.evict_random(),
            TlbReplacementPolicy::Lru => self.evict_lru(),
            TlbReplacementPolicy::Fifo => self.evict_fifo(),
            TlbReplacementPolicy::AdaptiveLru => self.evict_adaptive_lru(),
            TlbReplacementPolicy::Clock => self.evict_clock(),
        }
    }

    /// 随机替换
    fn evict_random(&mut self) {
        if let Some(key) = self.entries.keys().next().cloned() {
            self.entries.remove(&key);
            self.lru_list.retain(|k| k != &key);
            self.stats.evictions.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// LRU替换
    fn evict_lru(&mut self) {
        if let Some(key) = self.lru_list.first().cloned() {
            self.entries.remove(&key);
            self.lru_list.remove(0);
            self.stats.evictions.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// FIFO替换
    fn evict_fifo(&mut self) {
        if let Some(key) = self.lru_list.first().cloned() {
            self.entries.remove(&key);
            self.lru_list.remove(0);
            self.stats.evictions.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// 自适应LRU替换
    fn evict_adaptive_lru(&mut self) {
        // 找到访问频率最低的条目
        if let Some((&key, _)) = self.entries.iter().min_by(|(_, entry1), (_, entry2)| {
            // 综合考虑访问次数和最后访问时间
            let time_factor1 = entry1.last_access.elapsed().as_secs() as f64;
            let access_factor1 = entry1.access_count as f64;
            let score1 = time_factor1 / (access_factor1 + 1.0);

            let time_factor2 = entry2.last_access.elapsed().as_secs() as f64;
            let access_factor2 = entry2.access_count as f64;
            let score2 = time_factor2 / (access_factor2 + 1.0);

            score1
                .partial_cmp(&score2)
                .unwrap_or(std::cmp::Ordering::Equal)
        }) {
            self.entries.remove(&key);
            self.lru_list.retain(|k| k != &key);
            self.stats.evictions.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// 时钟算法替换
    fn evict_clock(&mut self) {
        if self.entries.is_empty() {
            return;
        }

        let keys: Vec<_> = self.entries.keys().cloned().collect();
        let start_hand = self.clock_hand;

        loop {
            let key = keys[self.clock_hand % keys.len()];

            if let Some(entry) = self.entries.get_mut(&key) {
                if entry.reference_bit {
                    entry.reference_bit = false;
                    self.clock_hand = (self.clock_hand + 1) % keys.len();

                    // 避免无限循环
                    if self.clock_hand == start_hand {
                        break;
                    }
                } else {
                    // 找到未引用的条目，替换它
                    self.entries.remove(&key);
                    self.lru_list.retain(|k| k != &key);
                    self.stats.evictions.fetch_add(1, Ordering::Relaxed);
                    break;
                }
            } else {
                // 条目已被其他地方移除，继续
                self.clock_hand = (self.clock_hand + 1) % keys.len();
            }
        }
    }

    /// 获取统计信息
    fn get_stats(&self) -> PerCpuTlbStatsSnapshot {
        let mut snapshot = self.stats.snapshot();
        snapshot.cpu_id = self.cpu_id;
        snapshot
    }

    /// 重置统计信息
    fn reset_stats(&mut self) {
        self.stats.reset();
    }

    /// 调整容量
    fn resize(&mut self, new_capacity: usize) {
        if new_capacity < self.entries.len() {
            // 需要删除一些条目
            let to_remove = self.entries.len() - new_capacity;
            for _ in 0..to_remove {
                self.evict_entry();
            }
        }

        self.capacity = new_capacity;
        self.lru_list.reserve(new_capacity);
    }
}

/// Per-CPU TLB管理器
pub struct PerCpuTlbManager {
    /// 配置
    config: PerCpuTlbConfig,
    /// 每个CPU的TLB
    cpu_tlbs: Vec<Mutex<SingleCpuTlb>>,
    /// 线程ID到CPU ID的映射
    thread_to_cpu: Mutex<HashMap<ThreadId, usize>>,
    /// 下一个可用的CPU ID
    next_cpu_id: AtomicUsize,
    /// 全局统计信息
    global_stats: PerCpuTlbStats,
}

impl PerCpuTlbManager {
    /// 创建新的Per-CPU TLB管理器
    pub fn new(config: PerCpuTlbConfig) -> Self {
        let mut cpu_tlbs = Vec::with_capacity(config.max_cpus);

        for cpu_id in 0..config.max_cpus {
            let tlb = SingleCpuTlb::new(cpu_id, config.entries_per_cpu, config.replacement_policy);
            cpu_tlbs.push(Mutex::new(tlb));
        }

        Self {
            config,
            cpu_tlbs,
            thread_to_cpu: Mutex::new(HashMap::new()),
            next_cpu_id: AtomicUsize::new(0),
            global_stats: PerCpuTlbStats::default(),
        }
    }

    /// 使用默认配置创建Per-CPU TLB管理器
    pub fn with_default_config() -> Self {
        Self::new(PerCpuTlbConfig::default())
    }

    /// Helper method to lock thread_to_cpu mapping with error handling
    fn lock_thread_to_cpu(
        &self,
    ) -> PerCpuTlbResult<std::sync::MutexGuard<'_, HashMap<ThreadId, usize>>> {
        self.thread_to_cpu.lock().map_err(|e| {
            VmError::Memory(MemoryError::MmuLockFailed {
                message: format!("Failed to lock thread_to_cpu mapping: {}", e),
            })
        })
    }

    /// Helper method to lock a specific CPU TLB with error handling
    fn lock_cpu_tlb(
        &self,
        cpu_id: usize,
    ) -> PerCpuTlbResult<std::sync::MutexGuard<'_, SingleCpuTlb>> {
        if cpu_id >= self.cpu_tlbs.len() {
            return Err(VmError::Memory(MemoryError::InvalidAddress(GuestAddr(
                cpu_id as u64,
            ))));
        }
        self.cpu_tlbs[cpu_id].lock().map_err(|e| {
            VmError::Memory(MemoryError::MmuLockFailed {
                message: format!("Failed to lock CPU TLB {}: {}", cpu_id, e),
            })
        })
    }

    /// 获取当前线程的CPU ID
    fn get_current_cpu_id(&self) -> PerCpuTlbResult<usize> {
        let thread_id = thread::current().id();

        // Check if thread already has a CPU ID assigned
        {
            let mapping = self.lock_thread_to_cpu()?;
            if let Some(&cpu_id) = mapping.get(&thread_id) {
                return Ok(cpu_id);
            }
        }

        // Allocate new CPU ID
        let cpu_id = self.next_cpu_id.fetch_add(1, Ordering::Relaxed) % self.config.max_cpus;

        {
            let mut mapping = self.lock_thread_to_cpu()?;
            mapping.insert(thread_id, cpu_id);
        }

        Ok(cpu_id)
    }

    /// 查找TLB条目
    pub fn lookup(&self, gva: GuestAddr, asid: u16) -> Option<GuestAddr> {
        // Get CPU ID, return None on error
        let cpu_id = match self.get_current_cpu_id() {
            Ok(id) => id,
            Err(_) => return None,
        };

        // Lock TLB, return None on error
        let mut tlb = match self.lock_cpu_tlb(cpu_id) {
            Ok(tlb_guard) => tlb_guard,
            Err(_) => return None,
        };

        self.global_stats
            .total_accesses
            .fetch_add(1, Ordering::Relaxed);

        if let Some(entry) = tlb.lookup(gva, asid) {
            self.global_stats.hits.fetch_add(1, Ordering::Relaxed);
            Some(entry.translate(gva))
        } else {
            self.global_stats.misses.fetch_add(1, Ordering::Relaxed);
            None
        }
    }

    /// 插入TLB条目
    pub fn insert(&self, walk_result: PageWalkResult, gva: GuestAddr, asid: u16) {
        // Get CPU ID, silently fail on error
        let cpu_id = match self.get_current_cpu_id() {
            Ok(id) => id,
            Err(_) => return,
        };

        // Lock TLB, silently fail on error
        let mut tlb = match self.lock_cpu_tlb(cpu_id) {
            Ok(tlb_guard) => tlb_guard,
            Err(_) => return,
        };

        tlb.insert(walk_result, gva, asid);
    }

    /// 刷新所有CPU的TLB
    pub fn flush_all(&self) {
        for tlb_mutex in &self.cpu_tlbs {
            // Silently fail if lock cannot be acquired
            if let Ok(mut tlb) = tlb_mutex.lock() {
                tlb.flush_all();
            }
        }
        self.global_stats.flushes.fetch_add(1, Ordering::Relaxed);
    }

    /// 刷新特定ASID的所有TLB条目
    pub fn flush_asid(&self, asid: u16) {
        for tlb_mutex in &self.cpu_tlbs {
            // Silently fail if lock cannot be acquired
            if let Ok(mut tlb) = tlb_mutex.lock() {
                tlb.flush_asid(asid);
            }
        }
        self.global_stats.flushes.fetch_add(1, Ordering::Relaxed);
    }

    /// 刷新特定页面的所有TLB条目
    pub fn flush_page(&self, gva: GuestAddr, asid: u16) {
        for tlb_mutex in &self.cpu_tlbs {
            // Silently fail if lock cannot be acquired
            if let Ok(mut tlb) = tlb_mutex.lock() {
                tlb.flush_page(gva, asid);
            }
        }
    }

    /// 获取特定CPU的统计信息
    pub fn get_cpu_stats(&self, cpu_id: usize) -> Option<PerCpuTlbStatsSnapshot> {
        if cpu_id >= self.cpu_tlbs.len() {
            return None;
        }

        match self.lock_cpu_tlb(cpu_id) {
            Ok(tlb) => Some(tlb.get_stats()),
            Err(_) => None,
        }
    }

    /// 获取所有CPU的统计信息
    pub fn get_all_stats(&self) -> Vec<PerCpuTlbStatsSnapshot> {
        let mut all_stats = Vec::with_capacity(self.cpu_tlbs.len());

        for tlb_mutex in &self.cpu_tlbs {
            // Skip CPUs where lock cannot be acquired
            if let Ok(tlb) = tlb_mutex.lock() {
                all_stats.push(tlb.get_stats());
            }
        }

        all_stats
    }

    /// 获取全局统计信息
    pub fn get_global_stats(&self) -> PerCpuTlbStatsSnapshot {
        self.global_stats.snapshot()
    }

    /// 重置所有统计信息
    pub fn reset_all_stats(&self) {
        for tlb_mutex in &self.cpu_tlbs {
            // Silently fail if lock cannot be acquired
            if let Ok(mut tlb) = tlb_mutex.lock() {
                tlb.reset_stats();
            }
        }
        self.global_stats.reset();
    }

    /// 调整特定CPU的TLB大小
    pub fn resize_cpu_tlb(&self, cpu_id: usize, new_size: usize) {
        if cpu_id >= self.cpu_tlbs.len() {
            return;
        }

        // Silently fail if lock cannot be acquired
        if let Ok(mut tlb) = self.lock_cpu_tlb(cpu_id) {
            tlb.resize(new_size);
        }
    }

    /// 自适应调整所有CPU的TLB大小
    pub fn adaptive_resize(&self) {
        if !self.config.enable_adaptive_sizing {
            return;
        }

        let all_stats = self.get_all_stats();

        for (cpu_id, stats) in all_stats.iter().enumerate() {
            // 根据命中率调整大小
            if stats.hit_rate < self.config.hit_rate_threshold {
                // 命中率低，增加大小
                let current_size = self.config.entries_per_cpu;
                let new_size = (current_size * 2).min(self.config.max_size);
                self.resize_cpu_tlb(cpu_id, new_size);
            } else if stats.hit_rate > 0.95 {
                // 命中率很高，可能可以减小大小
                let current_size = self.config.entries_per_cpu;
                let new_size = (current_size / 2).max(self.config.min_size);
                self.resize_cpu_tlb(cpu_id, new_size);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mmu::PageTableFlags;

    #[test]
    fn test_per_cpu_tlb_entry() {
        let mut entry = PerCpuTlbEntry {
            gva: GuestAddr(0x1000),
            gpa: GuestAddr(0x2000),
            page_size: 4096,
            flags: PageTableFlags::default(),
            asid: 0,
            access_count: 0,
            last_access: Instant::now(),
            reference_bit: false,
            global: false,
        };

        assert!(entry.contains(GuestAddr(0x1000)));
        assert!(entry.contains(GuestAddr(0x1FFF)));
        assert!(!entry.contains(GuestAddr(0x2000)));

        assert_eq!(entry.translate(GuestAddr(0x1000)), GuestAddr(0x2000));
        assert_eq!(entry.translate(GuestAddr(0x1FFF)), GuestAddr(0x2FFF));

        entry.update_access();
        assert_eq!(entry.access_count, 1);
        assert!(entry.reference_bit);
    }

    #[test]
    fn test_single_cpu_tlb() {
        let mut tlb = SingleCpuTlb::new(0, 4, TlbReplacementPolicy::Lru);

        // 初始状态
        assert_eq!(tlb.entries.len(), 0);

        // 插入条目
        let walk_result = PageWalkResult {
            gpa: GuestAddr(0x2000),
            page_size: 4096,
            flags: PageTableFlags::default(),
        };
        tlb.insert(walk_result, GuestAddr(0x1000), 0);

        assert_eq!(tlb.entries.len(), 1);

        // 查找条目
        let entry = tlb.lookup(GuestAddr(0x1000), 0);
        assert!(entry.is_some());

        // 查找不存在的条目
        let entry = tlb.lookup(GuestAddr(0x3000), 0);
        assert!(entry.is_none());

        // 刷新所有条目
        tlb.flush_all();
        assert_eq!(tlb.entries.len(), 0);
    }

    #[test]
    fn test_per_cpu_tlb_manager() {
        let manager = PerCpuTlbManager::with_default_config();

        // 插入条目
        let walk_result = PageWalkResult {
            gpa: GuestAddr(0x2000),
            page_size: 4096,
            flags: PageTableFlags::default(),
        };
        manager.insert(walk_result, GuestAddr(0x1000), 0);

        // 查找条目
        let gpa = manager.lookup(GuestAddr(0x1000), 0);
        assert_eq!(gpa, Some(GuestAddr(0x2000)));

        // 查找不存在的条目
        let gpa = manager.lookup(GuestAddr(0x3000), 0);
        assert_eq!(gpa, None);

        // 获取统计信息
        let stats = manager.get_global_stats();
        assert_eq!(stats.total_accesses, 2);
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
    }

    #[test]
    fn test_tlb_replacement_policies() {
        let mut tlb = SingleCpuTlb::new(0, 2, TlbReplacementPolicy::Lru);

        // 插入两个条目，填满TLB
        let walk_result1 = PageWalkResult {
            gpa: GuestAddr(0x2000),
            page_size: 4096,
            flags: PageTableFlags::default(),
        };
        tlb.insert(walk_result1, GuestAddr(0x1000), 0);

        let walk_result2 = PageWalkResult {
            gpa: GuestAddr(0x3000),
            page_size: 4096,
            flags: PageTableFlags::default(),
        };
        tlb.insert(walk_result2, GuestAddr(0x2000), 0);

        assert_eq!(tlb.entries.len(), 2);

        // 插入第三个条目，应该替换一个
        let walk_result3 = PageWalkResult {
            gpa: GuestAddr(0x4000),
            page_size: 4096,
            flags: PageTableFlags::default(),
        };
        tlb.insert(walk_result3, GuestAddr(0x3000), 0);

        assert_eq!(tlb.entries.len(), 2);
    }
}
