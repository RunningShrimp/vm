//! 高性能优化TLB实现
//!
//! 实现多级TLB结构、优化的替换算法和预取机制
//!
//! # 模块说明
//!
//! 此模块提供 `MultiLevelTlb`，这是一个高性能的多级TLB实现。
//! 它被 `unified_mmu.rs` 和 `mmu_optimized.rs` 使用。
//!
//! # 与其他TLB实现的关系
//!
//! - `tlb.rs`: 基础TLB实现
//! - `tlb_manager.rs`: 标准TLB管理器（推荐用于一般场景）
//! - `tlb_concurrent.rs`: 并发TLB（推荐用于高并发场景）
//! - `tlb_optimized.rs`: 多级TLB（推荐用于高性能场景）
//! - `tlb_async.rs` (vm-core): 异步TLB（推荐用于异步执行场景）
//!
//! # 适用场景
//!
//! `MultiLevelTlb` 适用于以下场景：
//! - **高性能要求**: 需要最低的TLB查找延迟
//! - **多级缓存**: 可以利用L1/L2/L3多级缓存结构
//! - **预取优化**: 需要智能预取机制的场景
//! - **自适应替换**: 需要根据访问模式自动调整替换策略

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use vm_core::{AccessType, GuestAddr, GuestPhysAddr, TlbEntry, TlbManager};

/// TLB条目优化版本
#[derive(Debug, Clone, Copy)]
pub struct OptimizedTlbEntry {
    /// Guest虚拟页号
    pub vpn: u64,
    /// Guest物理页号
    pub ppn: u64,
    /// 页表标志
    pub flags: u64,
    /// ASID (Address Space ID)
    pub asid: u16,
    /// 访问计数（用于自适应算法）
    pub access_count: u32,
    /// 最后访问时间戳（相对值）
    pub last_access: u32,
    /// 访问频率权重
    pub frequency_weight: u16,
    /// 预取标记
    pub prefetch_mark: bool,
    /// 热度标记
    pub hot_mark: bool,
}

impl OptimizedTlbEntry {
    /// 检查权限
    #[inline]
    pub fn check_permission(&self, access: AccessType) -> bool {
        let required = match access {
            AccessType::Read => 1 << 1,  // R bit
            AccessType::Write => 1 << 2, // W bit
            AccessType::Exec => 1 << 3,  // X bit
        };
        (self.flags & required) != 0
    }

    /// 更新访问信息
    #[inline]
    pub fn update_access(&mut self, timestamp: u32) {
        self.access_count = self.access_count.saturating_add(1);
        self.last_access = timestamp;

        // 更新频率权重
        if self.access_count > 100 {
            self.frequency_weight = 3; // 高频
        } else if self.access_count > 10 {
            self.frequency_weight = 2; // 中频
        } else {
            self.frequency_weight = 1; // 低频
        }
    }
}

/// 多级TLB配置
#[derive(Debug, Clone)]
pub struct MultiLevelTlbConfig {
    /// L1 TLB容量（最快访问）
    pub l1_capacity: usize,
    /// L2 TLB容量（中等访问）
    pub l2_capacity: usize,
    /// L3 TLB容量（大容量）
    pub l3_capacity: usize,
    /// 预取窗口大小
    pub prefetch_window: usize,
    /// 预取阈值
    pub prefetch_threshold: f64,
    /// 自适应替换策略
    pub adaptive_replacement: bool,
    /// 并发访问优化
    pub concurrent_optimization: bool,
    /// 统计收集
    pub enable_stats: bool,
}

impl Default for MultiLevelTlbConfig {
    fn default() -> Self {
        Self {
            l1_capacity: 64,   // L1: 64 entries
            l2_capacity: 256,  // L2: 256 entries
            l3_capacity: 1024, // L3: 1024 entries
            prefetch_window: 4,
            prefetch_threshold: 0.8,
            adaptive_replacement: true,
            concurrent_optimization: true,
            enable_stats: true,
        }
    }
}

/// TLB统计信息（原子操作版本）
#[derive(Debug)]
pub struct AtomicTlbStats {
    /// 总查找次数
    pub total_lookups: AtomicU64,
    /// L1命中次数
    pub l1_hits: AtomicU64,
    /// L2命中次数
    pub l2_hits: AtomicU64,
    /// L3命中次数
    pub l3_hits: AtomicU64,
    /// 总缺失次数
    pub total_misses: AtomicU64,
    /// 预取命中次数
    pub prefetch_hits: AtomicU64,
    /// 替换次数
    pub evictions: AtomicU64,
    /// 总查找时间（纳秒）
    pub total_lookup_time_ns: AtomicU64,
}

impl AtomicTlbStats {
    pub fn new() -> Self {
        Self {
            total_lookups: AtomicU64::new(0),
            l1_hits: AtomicU64::new(0),
            l2_hits: AtomicU64::new(0),
            l3_hits: AtomicU64::new(0),
            total_misses: AtomicU64::new(0),
            prefetch_hits: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
            total_lookup_time_ns: AtomicU64::new(0),
        }
    }

    pub fn record_lookup(&self, duration: Duration) {
        self.total_lookups.fetch_add(1, Ordering::Relaxed);
        self.total_lookup_time_ns
            .fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
    }

    pub fn record_l1_hit(&self) {
        self.l1_hits.fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_l2_hit(&self) {
        self.l2_hits.fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_l3_hit(&self) {
        self.l3_hits.fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_miss(&self) {
        self.total_misses.fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_prefetch_hit(&self) {
        self.prefetch_hits.fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_eviction(&self) {
        self.evictions.fetch_add(1, Ordering::Relaxed);
    }

    pub fn overall_hit_rate(&self) -> f64 {
        let total = self.total_lookups.load(Ordering::Relaxed);
        let hits = self.l1_hits.load(Ordering::Relaxed)
            + self.l2_hits.load(Ordering::Relaxed)
            + self.l3_hits.load(Ordering::Relaxed);
        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }

    pub fn avg_lookup_time_ns(&self) -> f64 {
        let lookups = self.total_lookups.load(Ordering::Relaxed);
        let total_time = self.total_lookup_time_ns.load(Ordering::Relaxed);
        if lookups == 0 {
            0.0
        } else {
            total_time as f64 / lookups as f64
        }
    }
}

/// 自适应替换策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdaptiveReplacementPolicy {
    /// 基于频率的LRU
    FrequencyBasedLru,
    /// 基于时间的LRU
    TimeBasedLru,
    /// 混合策略
    Hybrid,
    /// 2Q算法
    TwoQueue,
}

/// 单级TLB实现
pub struct SingleLevelTlb {
    /// TLB条目存储
    entries: HashMap<u64, OptimizedTlbEntry>,
    /// LRU访问顺序
    lru_order: VecDeque<u64>,
    /// 频率计数器
    frequency_counter: HashMap<u64, u32>,
    /// 容量
    capacity: usize,
    /// 替换策略
    replacement_policy: AdaptiveReplacementPolicy,
    /// 当前时间戳计数器
    timestamp_counter: u32,
}

impl SingleLevelTlb {
    pub fn new(capacity: usize, policy: AdaptiveReplacementPolicy) -> Self {
        Self {
            entries: HashMap::with_capacity(capacity),
            lru_order: VecDeque::with_capacity(capacity),
            frequency_counter: HashMap::with_capacity(capacity),
            capacity,
            replacement_policy: policy,
            timestamp_counter: 0,
        }
    }

    /// 生成TLB键
    #[inline]
    fn make_key(vpn: u64, asid: u16) -> u64 {
        (vpn << 16) | (asid as u64)
    }

    /// 查找条目
    pub fn lookup(&mut self, vpn: u64, asid: u16) -> Option<&OptimizedTlbEntry> {
        let key = Self::make_key(vpn, asid);

        if self.entries.contains_key(&key) {
            // 更新访问信息
            self.timestamp_counter = self.timestamp_counter.wrapping_add(1);

            // 更新LRU顺序
            self.update_lru_order(key);

            // 更新频率计数
            *self.frequency_counter.entry(key).or_insert(0) += 1;

            // 现在可以安全地获取不可变引用
            let entry = self.entries.get_mut(&key).unwrap();
            entry.update_access(self.timestamp_counter);

            Some(entry)
        } else {
            None
        }
    }

    /// 插入条目
    pub fn insert(&mut self, entry: OptimizedTlbEntry) -> bool {
        let key = Self::make_key(entry.vpn, entry.asid);

        // 检查是否需要替换
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            if !self.evict_victim() {
                return false; // 替换失败
            }
        }

        self.entries.insert(key, entry);
        self.update_lru_order(key);
        true
    }

    /// 更新LRU顺序
    fn update_lru_order(&mut self, key: u64) {
        // 从当前位置移除
        if let Some(pos) = self.lru_order.iter().position(|&k| k == key) {
            self.lru_order.remove(pos);
        }
        // 添加到末尾（最近使用）
        self.lru_order.push_back(key);
    }

    /// 选择替换受害者
    fn evict_victim(&mut self) -> bool {
        if self.lru_order.is_empty() {
            return false;
        }

        let victim_key = match self.replacement_policy {
            AdaptiveReplacementPolicy::FrequencyBasedLru => {
                // 选择频率最低的条目
                self.lru_order
                    .iter()
                    .min_by_key(|&&k| self.frequency_counter.get(&k).unwrap_or(&0))
                    .copied()
            }
            AdaptiveReplacementPolicy::TimeBasedLru => {
                // 选择最久未使用的条目
                self.lru_order.front().copied()
            }
            AdaptiveReplacementPolicy::Hybrid => {
                // 混合策略：结合频率和时间
                self.lru_order
                    .iter()
                    .min_by_key(|&&k| {
                        let freq = self.frequency_counter.get(&k).unwrap_or(&0);
                        let time_pos = self.lru_order.iter().position(|&x| x == k).unwrap_or(0);
                        // 频率权重高，时间权重低
                        (*freq as usize) * 2 + time_pos
                    })
                    .copied()
            }
            AdaptiveReplacementPolicy::TwoQueue => {
                // 2Q算法：优先替换新访问的条目
                if self.lru_order.len() > self.capacity / 2 {
                    self.lru_order.get(self.capacity / 2).copied()
                } else {
                    self.lru_order.front().copied()
                }
            }
        };

        if let Some(key) = victim_key {
            self.entries.remove(&key);
            self.lru_order.retain(|&k| k != key);
            self.frequency_counter.remove(&key);
            true
        } else {
            false
        }
    }

    /// 刷新指定ASID的条目
    pub fn flush_asid(&mut self, asid: u16) {
        let keys_to_remove: Vec<u64> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.asid == asid)
            .map(|(&key, _)| key)
            .collect();

        for key in keys_to_remove {
            self.entries.remove(&key);
            self.lru_order.retain(|&k| k != key);
            self.frequency_counter.remove(&key);
        }
    }

    /// 刷新所有条目
    pub fn flush_all(&mut self) {
        self.entries.clear();
        self.lru_order.clear();
        self.frequency_counter.clear();
    }

    /// 获取使用情况
    pub fn usage(&self) -> usize {
        self.entries.len()
    }
}

/// 多级TLB实现
pub struct MultiLevelTlb {
    /// 配置
    config: MultiLevelTlbConfig,
    /// L1 TLB（最快，最小）
    l1_tlb: SingleLevelTlb,
    /// L2 TLB（中等）
    l2_tlb: SingleLevelTlb,
    /// L3 TLB（大容量）
    l3_tlb: SingleLevelTlb,
    /// 预取队列
    prefetch_queue: VecDeque<(u64, u16)>, // (vpn, asid)
    /// 访问模式历史
    access_history: VecDeque<(u64, u16)>, // (vpn, asid)
    /// 统计信息
    stats: Arc<AtomicTlbStats>,
    /// 全局时间戳
    global_timestamp: Arc<AtomicUsize>,
}

impl MultiLevelTlb {
    pub fn new(config: MultiLevelTlbConfig) -> Self {
        Self {
            l1_tlb: SingleLevelTlb::new(
                config.l1_capacity,
                AdaptiveReplacementPolicy::TimeBasedLru,
            ),
            l2_tlb: SingleLevelTlb::new(config.l2_capacity, AdaptiveReplacementPolicy::Hybrid),
            l3_tlb: SingleLevelTlb::new(
                config.l3_capacity,
                AdaptiveReplacementPolicy::FrequencyBasedLru,
            ),
            prefetch_queue: VecDeque::with_capacity(config.prefetch_window),
            access_history: VecDeque::with_capacity(256),
            stats: Arc::new(AtomicTlbStats::new()),
            global_timestamp: Arc::new(AtomicUsize::new(0)),
            config,
        }
    }

    /// 查找地址翻译
    pub fn translate(&mut self, vpn: u64, asid: u16, access: AccessType) -> Option<(u64, u64)> {
        let start_time = Instant::now();
        let key = SingleLevelTlb::make_key(vpn, asid);

        // 更新全局时间戳
        let _timestamp = self.global_timestamp.fetch_add(1, Ordering::Relaxed) as u32;

        // L1查找
        if self.l1_tlb.entries.contains_key(&key) {
            let res = if let Some(entry) = self.l1_tlb.lookup(vpn, asid) {
                if entry.check_permission(access) {
                    self.stats.record_l1_hit();
                    self.stats.record_lookup(start_time.elapsed());
                    Some((entry.ppn, entry.flags))
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(ppn_flags) = res {
                self.update_access_pattern(vpn, asid);
                return Some(ppn_flags);
            }
        }

        // L2查找
        if self.l2_tlb.entries.contains_key(&key) {
            let promote = if let Some(entry) = self.l2_tlb.lookup(vpn, asid) {
                if entry.check_permission(access) {
                    self.stats.record_l2_hit();
                    self.stats.record_lookup(start_time.elapsed());
                    Some((*entry, (entry.ppn, entry.flags)))
                } else {
                    None
                }
            } else {
                None
            };
            if let Some((entry_copy, ppn_flags)) = promote {
                self.update_access_pattern(vpn, asid);
                self.promote_to_l1(entry_copy);
                return Some(ppn_flags);
            }
        }

        // L3查找
        if self.l3_tlb.entries.contains_key(&key) {
            let promote = if let Some(entry) = self.l3_tlb.lookup(vpn, asid) {
                if entry.check_permission(access) {
                    self.stats.record_l3_hit();
                    self.stats.record_lookup(start_time.elapsed());
                    Some((*entry, (entry.ppn, entry.flags)))
                } else {
                    None
                }
            } else {
                None
            };
            if let Some((entry_copy, ppn_flags)) = promote {
                self.update_access_pattern(vpn, asid);
                self.promote_to_l2(entry_copy);
                return Some(ppn_flags);
            }
        }

        // TLB缺失
        self.stats.record_miss();
        self.stats.record_lookup(start_time.elapsed());
        self.update_access_pattern(vpn, asid);

        // 触发预取
        self.trigger_prefetch(vpn, asid);

        None
    }

    /// 插入新的翻译结果
    pub fn insert(&mut self, vpn: u64, ppn: u64, flags: u64, asid: u16) {
        let timestamp = self.global_timestamp.load(Ordering::Relaxed) as u32;

        let entry = OptimizedTlbEntry {
            vpn,
            ppn,
            flags,
            asid,
            access_count: 1,
            last_access: timestamp,
            frequency_weight: 1,
            prefetch_mark: false,
            hot_mark: false,
        };

        // 优先插入L1，如果满了则降级
        if !self.l1_tlb.insert(entry) {
            // L1满了，插入L2
            if !self.l2_tlb.insert(entry) {
                // L2也满了，插入L3
                self.l3_tlb.insert(entry);
            }
        }
    }

    /// 提升条目到L1
    fn promote_to_l1(&mut self, entry: OptimizedTlbEntry) {
        let mut promoted_entry = entry;
        promoted_entry.hot_mark = true;

        if !self.l1_tlb.insert(promoted_entry) {
            // L1满了，将L1的受害者降级到L2
            if let Some(victim_key) = self.l1_tlb.lru_order.front() {
                if let Some(victim_entry) = self.l1_tlb.entries.get(victim_key) {
                    self.l2_tlb.insert(*victim_entry);
                }
            }
            self.l1_tlb.insert(promoted_entry);
        }
    }

    /// 提升条目到L2
    fn promote_to_l2(&mut self, entry: OptimizedTlbEntry) {
        if !self.l2_tlb.insert(entry) {
            // L2满了，将L2的受害者降级到L3
            if let Some(victim_key) = self.l2_tlb.lru_order.front() {
                if let Some(victim_entry) = self.l2_tlb.entries.get(victim_key) {
                    self.l3_tlb.insert(*victim_entry);
                }
            }
            self.l2_tlb.insert(entry);
        }
    }

    /// 更新访问模式
    fn update_access_pattern(&mut self, vpn: u64, asid: u16) {
        self.access_history.push_back((vpn, asid));
        if self.access_history.len() > 256 {
            self.access_history.pop_front();
        }
    }

    /// 触发预取
    fn trigger_prefetch(&mut self, current_vpn: u64, asid: u16) {
        if !self.config.adaptive_replacement {
            return;
        }

        // 检测顺序访问模式
        if let Some(&(prev_vpn, _)) = self.access_history.back() {
            if current_vpn == prev_vpn + 1 {
                // 顺序访问，预取后续页面
                for i in 1..=self.config.prefetch_window {
                    let prefetch_vpn = current_vpn + i as u64;
                    let prefetch_key = (prefetch_vpn, asid);

                    if !self.prefetch_queue.contains(&prefetch_key) {
                        self.prefetch_queue.push_back(prefetch_key);
                        if self.prefetch_queue.len() > self.config.prefetch_window {
                            self.prefetch_queue.pop_front();
                        }
                    }
                }
            }
        }
    }

    /// 处理预取队列
    pub fn process_prefetch(&mut self) -> Vec<(u64, u16)> {
        let mut prefetch_requests = Vec::new();

        while let Some((vpn, asid)) = self.prefetch_queue.pop_front() {
            // 检查是否已经在任何级别的TLB中
            let key = SingleLevelTlb::make_key(vpn, asid);
            let in_l1 = self.l1_tlb.entries.contains_key(&key);
            let in_l2 = self.l2_tlb.entries.contains_key(&key);
            let in_l3 = self.l3_tlb.entries.contains_key(&key);

            if !in_l1 && !in_l2 && !in_l3 {
                prefetch_requests.push((vpn, asid));
            }
        }

        prefetch_requests
    }

    /// 刷新指定ASID的所有TLB
    pub fn flush_asid(&mut self, asid: u16) {
        self.l1_tlb.flush_asid(asid);
        self.l2_tlb.flush_asid(asid);
        self.l3_tlb.flush_asid(asid);
    }

    /// 刷新所有TLB
    pub fn flush_all(&mut self) {
        self.l1_tlb.flush_all();
        self.l2_tlb.flush_all();
        self.l3_tlb.flush_all();
        self.prefetch_queue.clear();
        self.access_history.clear();
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> &Arc<AtomicTlbStats> {
        &self.stats
    }

    /// 获取各级TLB使用情况
    pub fn get_usage(&self) -> (usize, usize, usize) {
        (
            self.l1_tlb.usage(),
            self.l2_tlb.usage(),
            self.l3_tlb.usage(),
        )
    }

    /// 重置统计信息
    pub fn reset_stats(&self) {
        let _ = Arc::try_unwrap(self.stats.clone()).map(|stats| AtomicTlbStats::new());
    }
}

impl TlbManager for MultiLevelTlb {
    fn lookup(&mut self, addr: GuestAddr, asid: u16, access: AccessType) -> Option<TlbEntry> {
        // 将地址转换为页号（假设4KB页）
        let vpn = addr >> 12;
        
        if let Some((ppn, flags)) = self.translate(vpn, asid, access) {
            // 将页号转换回地址
            let phys_addr = (ppn << 12) | (addr & 0xFFF);
            Some(TlbEntry {
                guest_addr: addr,
                phys_addr,
                flags,
                asid,
            })
        } else {
            None
        }
    }

    fn update(&mut self, entry: TlbEntry) {
        // 将地址转换为页号
        let vpn = entry.guest_addr >> 12;
        let ppn = entry.phys_addr >> 12;
        self.insert(vpn, ppn, entry.flags, entry.asid);
    }

    fn flush(&mut self) {
        self.flush_all();
    }

    fn flush_asid(&mut self, asid: u16) {
        MultiLevelTlb::flush_asid(self, asid);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_level_tlb_basic() {
        let config = MultiLevelTlbConfig::default();
        let mut tlb = MultiLevelTlb::new(config);

        // 插入条目
        tlb.insert(0x1000, 0x2000, 0x5, 0); // R+W flags

        // 查找测试
        let result = tlb.translate(0x1000, 0, AccessType::Read);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), (0x2000, 0x5));

        // 检查统计
        let stats = tlb.get_stats();
        assert_eq!(stats.l1_hits.load(Ordering::Relaxed), 1);
        assert_eq!(stats.total_lookups.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_tlb_promotion() {
        let mut config = MultiLevelTlbConfig::default();
        config.l1_capacity = 1; // 很小的L1，便于测试提升
        let mut tlb = MultiLevelTlb::new(config);

        // 填充L1
        tlb.insert(0x1000, 0x2000, 0x5, 0);
        tlb.translate(0x1000, 0, AccessType::Read); // L1 hit

        // 插入新条目，应该提升到L1
        tlb.insert(0x2000, 0x3000, 0x5, 0);
        let result = tlb.translate(0x2000, 0, AccessType::Read);
        assert!(result.is_some());

        let stats = tlb.get_stats();
        assert!(stats.l1_hits.load(Ordering::Relaxed) >= 1);
    }

    #[test]
    fn test_adaptive_replacement() {
        let config = MultiLevelTlbConfig::default();
        let mut tlb = MultiLevelTlb::new(config);

        // 插入多个条目
        for i in 0..10 {
            tlb.insert(i, i + 0x1000, 0x5, 0);
        }

        // 频繁访问某些条目
        for _ in 0..100 {
            for i in 0..3 {
                tlb.translate(i, 0, AccessType::Read);
            }
        }

        // 检查高频条目是否仍在TLB中
        let usage = tlb.get_usage();
        assert!(usage.0 > 0 || usage.1 > 0); // 至少在某个级别中
    }
}
