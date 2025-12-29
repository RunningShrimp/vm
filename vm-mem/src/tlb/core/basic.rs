#![allow(unused_variables)]
#![allow(dead_code)]

//! 软件 TLB (Translation Lookaside Buffer)
//!
//!
use crate::GuestAddr;
use crate::mmu::{PageTableFlags, PageWalkResult};
use std::collections::{HashMap, VecDeque};

/// TLB 条目
#[derive(Debug, Clone)]
pub struct TlbEntry {
    /// Guest 虚拟地址（页对齐）
    pub gva: GuestAddr,
    /// Guest 物理地址
    pub gpa: GuestAddr,
    /// 页面大小
    pub page_size: u64,
    /// 页表标志
    pub flags: PageTableFlags,
    /// 访问计数（用于 LRU）
    pub access_count: u64,
    /// 访问频率（用于自适应算法）
    pub access_frequency: f64,
    /// 最后访问时间戳
    pub last_access: u64,
    /// ASID (Address Space ID)
    pub asid: u16,
    /// 引用位（用于时钟算法）
    pub reference_bit: bool,
}

impl TlbEntry {
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
}

/// TLB 替换策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlbReplacePolicy {
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

/// TLB性能配置
#[derive(Debug, Clone)]
pub struct TlbConfig {
    pub initial_capacity: usize,
    pub max_capacity: usize,
    pub policy: TlbReplacePolicy,
    pub enable_stats: bool,
    pub auto_resize: bool,
    pub resize_threshold: f64, // 命中率阈值，低于此值时扩容
}

impl Default for TlbConfig {
    fn default() -> Self {
        Self {
            initial_capacity: 1024,
            max_capacity: 8192,
            policy: TlbReplacePolicy::AdaptiveLru,
            enable_stats: true,
            auto_resize: true,
            resize_threshold: 0.85,
        }
    }
}

/// 软件 TLB
pub struct SoftwareTlb {
    /// TLB 条目
    entries: HashMap<(GuestAddr, u16), TlbEntry>,
    /// LRU 队列，存储键以实现高效的 LRU 替换
    lru_queue: VecDeque<(GuestAddr, u16)>,
    /// 时钟算法的时钟指针
    clock_hand: usize,
    /// 当前容量
    capacity: usize,
    /// 最大容量
    max_capacity: usize,
    /// 替换策略
    policy: TlbReplacePolicy,
    /// 配置
    config: TlbConfig,
    /// 全局访问计数
    global_access: u64,
    /// 全局时间戳（单调递增）
    global_timestamp: u64,
    /// 统计信息
    stats: TlbStats,
}

/// TLB 统计信息
#[derive(Debug, Clone, Default)]
pub struct TlbStats {
    pub hits: u64,
    pub misses: u64,
    pub flushes: u64,
    pub evictions: u64,
    pub inserts: u64,
    pub lookups: u64,
    pub resize_count: u64,
    pub total_access_time_ns: u64,
    pub avg_hit_rate_samples: Vec<f64>,
}

impl TlbStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }

    pub fn avg_access_time_ns(&self) -> f64 {
        if self.lookups == 0 {
            0.0
        } else {
            self.total_access_time_ns as f64 / self.lookups as f64
        }
    }

    pub fn recent_hit_rate(&self) -> f64 {
        if self.avg_hit_rate_samples.is_empty() {
            self.hit_rate()
        } else {
            // 返回最近10个样本的平均值
            let recent_count = self.avg_hit_rate_samples.len().min(10);
            let start_idx = self.avg_hit_rate_samples.len().saturating_sub(recent_count);
            let sum: f64 = self.avg_hit_rate_samples[start_idx..].iter().sum();
            sum / recent_count as f64
        }
    }

    pub fn efficiency_score(&self) -> f64 {
        // 综合效率评分：命中率权重70%，访问速度权重30%
        let hit_score = self.recent_hit_rate();
        let speed_score = 1.0 / (1.0 + self.avg_access_time_ns() / 100.0); // 归一化到0-1范围
        hit_score * 0.7 + speed_score * 0.3
    }
}

impl Default for SoftwareTlb {
    fn default() -> Self {
        Self::with_config(TlbConfig::default())
    }
}

impl SoftwareTlb {
    pub fn new(capacity: usize, policy: TlbReplacePolicy) -> Self {
        let config = TlbConfig {
            initial_capacity: capacity,
            max_capacity: capacity,
            policy,
            enable_stats: true,
            auto_resize: false,
            resize_threshold: 0.85,
        };
        Self::with_config(config)
    }

    pub fn with_config(config: TlbConfig) -> Self {
        Self {
            entries: HashMap::with_capacity(config.initial_capacity),
            lru_queue: VecDeque::with_capacity(config.initial_capacity),
            clock_hand: 0,
            capacity: config.initial_capacity,
            max_capacity: config.max_capacity,
            policy: config.policy,
            config,
            global_access: 0,
            global_timestamp: 0,
            stats: TlbStats::default(),
        }
    }

    /// 快速哈希函数，提高查找效率
    fn fast_hash(gva: GuestAddr, asid: u16) -> u64 {
        // 使用简单的哈希组合
        let hash = gva.0.wrapping_mul(31) ^ (asid as u64);
        hash.wrapping_mul(0x9e3779b9) // 黄金比例常数
    }

    fn update_lru(&mut self, key: &(GuestAddr, u16)) {
        if let Some(pos) = self.lru_queue.iter().position(|x| x == key)
            && let Some(k) = self.lru_queue.remove(pos)
        {
            self.lru_queue.push_back(k);
        }
    }

    pub fn lookup(&mut self, gva: GuestAddr, asid: u16) -> Option<&TlbEntry> {
        self.global_access += 1;
        // 假设页大小为 4096 进行页对齐
        let page_base = GuestAddr(gva.0 & !(4096 - 1));
        let key = (page_base, asid);

        if self.entries.contains_key(&key) {
            self.stats.hits += 1;
            self.update_lru(&key);
            // 我们不能在这里返回可变引用，因为它会与 update_lru 中的可变借用冲突
            // 所以我们再次获取它
            self.entries.get(&key)
        } else {
            self.stats.misses += 1;
            None
        }
    }

    pub fn insert(&mut self, walk_result: PageWalkResult, gva: GuestAddr, asid: u16) {
        let entry = TlbEntry {
            gva: GuestAddr(gva.0 & !(walk_result.page_size - 1)),
            gpa: GuestAddr(walk_result.gpa & !(walk_result.page_size - 1)),
            page_size: walk_result.page_size,
            flags: walk_result.flags,
            access_count: self.global_access,
            access_frequency: 1.0,
            last_access: self.global_timestamp,
            asid,
            reference_bit: true,
        };

        let key = (entry.gva, entry.asid);

        if self.entries.len() >= self.capacity
            && let Some(lru_key) = self.lru_queue.pop_front()
        {
            self.entries.remove(&lru_key);
        }
        self.entries.insert(key, entry);
        self.lru_queue.push_back(key);
    }

    pub fn flush_all(&mut self) {
        self.entries.clear();
        self.lru_queue.clear();
        self.stats.flushes += 1;
    }

    pub fn flush_asid(&mut self, asid: u16) {
        self.entries.retain(|_key, entry| entry.asid != asid);
        self.lru_queue.retain(|(_, entry_asid)| *entry_asid != asid);
        self.stats.flushes += 1;
    }

    pub fn flush_page(&mut self, gva: GuestAddr, asid: u16) {
        let page_base = GuestAddr(gva.0 & !(4096 - 1));
        let key = (page_base, asid);
        if self.entries.remove(&key).is_some() {
            self.lru_queue.retain(|k| *k != key);
        }
    }

    pub fn stats(&self) -> &TlbStats {
        &self.stats
    }

    pub fn reset_stats(&mut self) {
        self.stats = TlbStats::default();
    }

    pub fn used_entries(&self) -> usize {
        self.entries.len()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mmu::PageTableFlags;

    #[test]
    fn test_tlb_lookup() {
        let mut tlb = SoftwareTlb::new(4, TlbReplacePolicy::Lru);
        let walk_result = PageWalkResult {
            gpa: GuestAddr(0x1000),
            page_size: 4096,
            flags: PageTableFlags::default(),
        };
        tlb.insert(walk_result, GuestAddr(0x2000), 0);
        let entry = tlb.lookup(GuestAddr(0x2000), 0);
        assert!(entry.is_some());
        assert_eq!(tlb.stats().hits, 1);
        let entry = tlb.lookup(GuestAddr(0x3000), 0);
        assert!(entry.is_none());
        assert_eq!(tlb.stats().misses, 1);
    }

    #[test]
    fn test_tlb_flush() {
        let mut tlb = SoftwareTlb::new(4, TlbReplacePolicy::Lru);
        let walk_result = PageWalkResult {
            gpa: GuestAddr(0x1000),
            page_size: 4096,
            flags: PageTableFlags::default(),
        };
        tlb.insert(walk_result, GuestAddr(0x2000), 0);
        assert_eq!(tlb.used_entries(), 1);
        tlb.flush_all();
        assert_eq!(tlb.used_entries(), 0);
    }
}
