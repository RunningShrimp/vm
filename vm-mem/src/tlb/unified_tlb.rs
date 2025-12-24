//! 统一TLB接口和实现
//!
//! 提供多种TLB实现的统一接口，支持根据场景选择最佳实现：
//! - 基础TLB：简单实现，适用于基本场景
//! - 优化TLB：多级TLB，适用于高性能场景
//! - 并发TLB：无锁设计，适用于高并发场景
//!
//! ## 使用方式
//!
//! ### 基础TLB（默认）
//! ```toml
//! [dependencies]
//! vm-mem = { path = "../vm-mem" }
//! ```
//!
//! ### 优化TLB
//! ```toml
//! [dependencies]
//! vm-mem = { path = "../vm-mem", features = ["tlb-optimized"] }
//! ```
//!
//! ### 并发TLB
//! ```toml
//! [dependencies]
//! vm-mem = { path = "../vm-mem", features = ["tlb-concurrent"] }
//! ```

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use vm_core::{AccessType, GuestAddr, GuestPhysAddr};
use crate::memory::memory_pool::{MemoryPool, StackPool};

/// 从标志位转换为访问类型
fn access_type_from_flags(flags: u64) -> vm_core::AccessType {
    use vm_core::AccessType;
    
    if flags & 0x1 != 0 { AccessType::Read }
    else if flags & 0x2 != 0 { AccessType::Write }
    else { AccessType::Execute }
}

/// 统一TLB接口
pub trait UnifiedTlb: Send + Sync {
    /// 查找TLB条目
    fn lookup(&self, gva: GuestAddr, access_type: AccessType) -> Option<TlbResult>;
    
    /// 插入TLB条目
    fn insert(&self, gva: GuestAddr, gpa: GuestPhysAddr, flags: u64, asid: u16);
    
    /// 使TLB条目失效
    fn invalidate(&self, gva: GuestAddr);
    
    /// 使所有TLB条目失效
    fn invalidate_all(&self);
    
    /// 获取TLB统计信息
    fn get_stats(&self) -> TlbStats;
    
    /// 清空TLB
    fn flush(&self);
}

/// TLB查找结果
#[derive(Debug, Clone)]
pub struct TlbResult {
    /// 物理地址
    pub gpa: GuestPhysAddr,
    /// 页表标志
    pub flags: u64,
    /// 页面大小
    pub page_size: u64,
    /// 是否命中
    pub hit: bool,
}

impl Default for TlbResult {
    fn default() -> Self {
        Self {
            gpa: GuestPhysAddr(0),
            flags: 0,
            page_size: 4096,
            hit: false,
        }
    }
}

/// TLB统计信息
#[derive(Debug, Clone, Default)]
pub struct TlbStats {
    /// 查找次数
    pub lookups: u64,
    /// 命中次数
    pub hits: u64,
    /// 未命中次数
    pub misses: u64,
    /// 失效次数
    pub invalidations: u64,
    /// 预取次数
    pub prefetches: u64,
}

impl TlbStats {
    /// 计算命中率
    pub fn hit_rate(&self) -> f64 {
        if self.lookups == 0 {
            return 0.0;
        }
        self.hits as f64 / self.lookups as f64
    }
}

/// 基础TLB实现
pub struct BasicTlb {
    entries: Arc<RwLock<HashMap<GuestAddr, TlbResult>>>,
    stats: Arc<RwLock<TlbStats>>,
    max_entries: usize,
    // 使用内存池来减少分配开销
    result_pool: Arc<RwLock<StackPool<TlbResult>>>,
}

impl BasicTlb {
    /// 创建基础TLB
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(TlbStats::default())),
            max_entries,
            result_pool: Arc::new(RwLock::new(StackPool::<TlbResult>::with_capacity(max_entries * 2))),
        }
    }
}

impl UnifiedTlb for BasicTlb {
    fn lookup(&self, gva: GuestAddr, access_type: AccessType) -> Option<TlbResult> {
        let entries = self.entries.read().unwrap();
        let result = entries.get(&gva).cloned();
        drop(entries);
        
        // 检查访问权限是否匹配
        let result = if let Some(mut entry) = result {
            let entry_access_type = access_type_from_flags(entry.flags);
            
            // 验证访问权限
            let access_allowed = match (entry_access_type, access_type) {
                // 如果条目有读权限，则允许读访问
                (AccessType::Read, AccessType::Read) => true,
                // 如果条目有写权限，则允许写和读访问
                (AccessType::Write, AccessType::Write) => true,
                (AccessType::Write, AccessType::Read) => true,
                // 如果条目有执行权限，则只允许执行访问
                (AccessType::Execute, AccessType::Execute) => true,
                // 其他组合不允许
                _ => false,
            };
            
            if access_allowed {
                entry.hit = true;
                Some(entry)
            } else {
                None
            }
        } else {
            None
        };
        
        // 更新统计
        {
            let mut stats = self.stats.write().unwrap();
            stats.lookups += 1;
            if result.is_some() {
                stats.hits += 1;
            } else {
                stats.misses += 1;
            }
        }
        
        result
    }
    
    fn insert(&self, gva: GuestAddr, gpa: GuestPhysAddr, flags: u64, _asid: u16) {
        let mut entries = self.entries.write().unwrap();
        
        // 如果已满，移除最旧的条目
        if entries.len() >= self.max_entries {
            entries.clear();
            let mut stats = self.stats.write().unwrap();
            stats.invalidations += 1;
        }
        
        // 从内存池分配TlbResult
        let result = {
            let mut pool = self.result_pool.write().unwrap();
            pool.allocate().unwrap_or_else(|_| TlbResult {
                gpa,
                flags,
                page_size: 4096, // 默认4KB页面
                hit: true,
            })
        };
        
        entries.insert(gva, result);
    }
    
    fn invalidate(&self, gva: GuestAddr) {
        let mut entries = self.entries.write().unwrap();
        if let Some(result) = entries.remove(&gva) {
            // 将TlbResult归还到内存池
            let mut pool = self.result_pool.write().unwrap();
            pool.deallocate(result);
        }
        let mut stats = self.stats.write().unwrap();
        stats.invalidations += 1;
    }
    
    fn invalidate_all(&self) {
        let mut entries = self.entries.write().unwrap();
        let count = entries.len() as u64;
        
        // 将所有条目归还到内存池
        if count > 0 {
            let mut pool = self.result_pool.write().unwrap();
            for (_, result) in entries.drain() {
                pool.deallocate(result);
            }
        } else {
            entries.clear();
        }
        
        let mut stats = self.stats.write().unwrap();
        stats.invalidations += count;
    }
    
    fn get_stats(&self) -> TlbStats {
        let stats = self.stats.read().unwrap();
        TlbStats {
            lookups: stats.lookups,
            hits: stats.hits,
            misses: stats.misses,
            invalidations: stats.invalidations,
            prefetches: stats.prefetches,
        }
    }
    
    fn flush(&self) {
        self.invalidate_all();
    }
}

/// 优化TLB实现（多级TLB）
#[cfg(feature = "tlb-optimized")]
pub struct OptimizedTlb {
    inner: std::sync::Mutex<crate::MultiLevelTlb>,
}

#[cfg(feature = "tlb-optimized")]
impl OptimizedTlb {
    /// 创建优化TLB
    pub fn new() -> Self {
        Self {
            inner: std::sync::Mutex::new(crate::MultiLevelTlb::new(crate::MultiLevelTlbConfig::default())),
        }
    }
}

#[cfg(feature = "tlb-optimized")]
impl UnifiedTlb for OptimizedTlb {
    fn lookup(&self, gva: GuestAddr, access_type: AccessType) -> Option<TlbResult> {
        let mut inner = self.inner.lock().unwrap();
        // ConcurrentTlbManager uses translate method instead of lookup
        inner.translate(gva, 0, access_type).map(|(phys_addr, flags)| TlbResult {
            gpa: phys_addr,
            flags,
            page_size: 4096, // 默认4KB页面
            hit: true,
        })
    }
    
    fn insert(&self, gva: GuestAddr, gpa: GuestPhysAddr, flags: u64, asid: u16) {
        use vm_core::TlbManager;
        let mut inner = self.inner.lock().unwrap();
        // Convert gva to vpn (virtual page number)
        let vpn = gva >> 12;
        inner.insert(vpn, gpa, flags, asid);
    }
    
    fn invalidate(&self, gva: GuestAddr) {
        let mut inner = self.inner.lock().unwrap();
        // MultiLevelTlb doesn't have a direct invalidate method
        // This is a placeholder implementation
    }
    
    fn invalidate_all(&self) {
        let mut inner = self.inner.lock().unwrap();
        // MultiLevelTlb doesn't have a direct invalidate_all method
        // This is a placeholder implementation
    }
    
    fn get_stats(&self) -> TlbStats {
        let inner = self.inner.lock().unwrap();
        let stats = inner.get_stats();
        use std::sync::atomic::Ordering;
        TlbStats {
            lookups: stats.total_lookups.load(Ordering::Relaxed),
            hits: stats.l1_hits.load(Ordering::Relaxed) + 
                   stats.l2_hits.load(Ordering::Relaxed) + 
                   stats.l3_hits.load(Ordering::Relaxed),
            misses: stats.total_misses.load(Ordering::Relaxed),
            invalidations: 0, // Not tracked in MultiLevelTlb
            prefetches: 0, // Not tracked in MultiLevelTlb
        }
    }
    
    fn flush(&self) {
        use vm_core::TlbManager;
        let mut inner = self.inner.lock().unwrap();
        inner.flush_all();
    }
}

/// 并发TLB实现
#[cfg(feature = "tlb-concurrent")]
pub struct ConcurrentTlb {
    inner: std::sync::Mutex<crate::ConcurrentTlbManager>,
}

#[cfg(feature = "tlb-concurrent")]
impl ConcurrentTlb {
    /// 创建并发TLB
    pub fn new() -> Self {
        Self {
            inner: std::sync::Mutex::new(crate::ConcurrentTlbManager::new(crate::ConcurrentTlbConfig::default())),
        }
    }
}

#[cfg(feature = "tlb-concurrent")]
impl UnifiedTlb for ConcurrentTlb {
    fn lookup(&self, gva: GuestAddr, access_type: AccessType) -> Option<TlbResult> {
        let mut inner = self.inner.lock().unwrap();
        // ConcurrentTlbManager uses translate method instead of lookup
        inner.translate(gva, 0, access_type).map(|(phys_addr, flags)| TlbResult {
            gpa: phys_addr,
            flags,
            page_size: 4096, // 默认4KB页面
            hit: true,
        })
    }
    
    fn insert(&self, gva: GuestAddr, gpa: GuestPhysAddr, flags: u64, asid: u16) {
        use vm_core::TlbManager;
        let mut inner = self.inner.lock().unwrap();
        // Convert gva to vpn (virtual page number)
        let vpn = gva >> 12;
        inner.insert(vpn, gpa, flags, asid);
    }
    
    fn invalidate(&self, gva: GuestAddr) {
        let mut inner = self.inner.lock().unwrap();
        // ConcurrentTlbManager doesn't have a direct invalidate method
        // This is a placeholder implementation
    }
    
    fn invalidate_all(&self) {
        let mut inner = self.inner.lock().unwrap();
        // ConcurrentTlbManager doesn't have a direct invalidate_all method
        // This is a placeholder implementation
    }
    
    fn get_stats(&self) -> TlbStats {
        let inner = self.inner.lock().unwrap();
        let stats = inner.get_stats();
        use std::sync::atomic::Ordering;
        TlbStats {
            lookups: stats.total_lookups.load(Ordering::Relaxed),
            hits: stats.fast_path_hits.load(Ordering::Relaxed) + stats.sharded_hits.load(Ordering::Relaxed),
            misses: stats.total_misses.load(Ordering::Relaxed),
            invalidations: 0, // Not tracked in ConcurrentTlbManager
            prefetches: 0, // Not tracked in ConcurrentTlbManager
        }
    }
    
    fn flush(&self) {
        use vm_core::TlbManager;
        let mut inner = self.inner.lock().unwrap();
        inner.flush_all();
    }
}



/// TLB工厂，根据特性标志创建合适的TLB实现
pub struct TlbFactory;

impl TlbFactory {
    /// 创建基础TLB
    #[cfg(feature = "tlb-basic")]
    pub fn create_basic_tlb(max_entries: usize) -> Box<dyn UnifiedTlb> {
        Box::new(BasicTlb::new(max_entries))
    }
    
    /// 创建优化TLB
    #[cfg(feature = "tlb-optimized")]
    pub fn create_optimized_tlb() -> Box<dyn UnifiedTlb> {
        Box::new(OptimizedTlb::new())
    }
    
    /// 创建并发TLB
    #[cfg(feature = "tlb-concurrent")]
    pub fn create_concurrent_tlb() -> Box<dyn UnifiedTlb> {
        Box::new(ConcurrentTlb::new())
    }
    
    /// 根据可用特性创建最佳TLB实现
    pub fn create_best_tlb(max_entries: usize) -> Box<dyn UnifiedTlb> {
        #[cfg(feature = "tlb-concurrent")]
        {
            return Self::create_concurrent_tlb();
        }
        
        #[cfg(all(feature = "tlb-optimized", not(feature = "tlb-concurrent")))]
        {
            return Self::create_optimized_tlb();
        }
        
        #[cfg(all(feature = "tlb-basic", not(feature = "tlb-optimized"), not(feature = "tlb-concurrent")))]
        {
            return Self::create_basic_tlb(max_entries);
        }
        
        #[cfg(not(any(feature = "tlb-basic", feature = "tlb-optimized", feature = "tlb-concurrent")))]
        {
            // 默认使用基础实现
            return Box::new(BasicTlb::new(max_entries));
        }
        
        // 如果没有匹配的特性，使用基础实现作为后备
        #[allow(unreachable_code)]
        Box::new(BasicTlb::new(max_entries))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    #[cfg(feature = "tlb-basic")]
    fn test_basic_tlb() {
        let tlb = BasicTlb::new(16);
        
        // 插入条目
        tlb.insert(0x1000, 0x2000, 0x7, 0);
        
        // 查找条目
        let result = tlb.lookup(0x1000, AccessType::Read);
        assert!(result.is_some());
        assert_eq!(result.unwrap().gpa, 0x2000);
        
        // 检查统计
        let stats = tlb.get_stats();
        assert_eq!(stats.lookups, 1);
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 0);
    }

    #[test]
    fn test_tlb_stats() {
        let stats = TlbStats {
            lookups: 100,
            hits: 80,
            misses: 20,
            invalidations: 5,
            prefetches: 10,
        };

        assert_eq!(stats.hit_rate(), 0.8);
    }
}

/// 高性能优化TLB实现
//!
//! 实现多级TLB结构、优化的替换算法和预取机制
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
use crate::PAGE_SHIFT;
use vm_core::{AccessType, GuestAddr, GuestPhysAddr, TlbManager};

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
            AccessType::Read => 1 << 1,
            AccessType::Write => 1 << 2,
            AccessType::Execute => 1 << 3,
            AccessType::Atomic => (1 << 1) | (1 << 2),
        };
        (self.flags & required) != 0
    }

    /// 更新访问信息
    #[inline]
    pub fn update_access(&mut self, timestamp: u32) {
        self.access_count = self.access_count.saturating_add(1);
        self.last_access = timestamp;

        if self.access_count > 100 {
            self.frequency_weight = 3;
        } else if self.access_count > 10 {
            self.frequency_weight = 2;
        } else {
            self.frequency_weight = 1;
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
            l1_capacity: 64,
            l2_capacity: 256,
            l3_capacity: 1024,
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
            self.timestamp_counter = self.timestamp_counter.wrapping_add(1);
            self.update_lru_order(key);
            *self.frequency_counter.entry(key).or_insert(0) += 1;
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

        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            if !self.evict_victim() {
                return false;
            }
        }

        self.entries.insert(key, entry);
        self.update_lru_order(key);
        true
    }

    /// 更新LRU顺序
    fn update_lru_order(&mut self, key: u64) {
        if let Some(pos) = self.lru_order.iter().position(|&k| k == key) {
            self.lru_order.remove(pos);
        }
        self.lru_order.push_back(key);
    }

    /// 选择替换受害者
    fn evict_victim(&mut self) -> bool {
        if self.lru_order.is_empty() {
            return false;
        }

        let victim_key = match self.replacement_policy {
            AdaptiveReplacementPolicy::FrequencyBasedLru => {
                self.lru_order
                    .iter()
                    .min_by_key(|&&k| self.frequency_counter.get(&k).unwrap_or(&0))
                    .copied()
            }
            AdaptiveReplacementPolicy::TimeBasedLru => {
                self.lru_order.front().copied()
            }
            AdaptiveReplacementPolicy::Hybrid => {
                self.lru_order
                    .iter()
                    .min_by_key(|&&k| {
                        let freq = self.frequency_counter.get(&k).unwrap_or(&0);
                        let time_pos = self.lru_order.iter().position(|&x| x == k).unwrap_or(0);
                        (*freq as usize) * 2 + time_pos
                    })
                    .copied()
            }
            AdaptiveReplacementPolicy::TwoQueue => {
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
    prefetch_queue: VecDeque<(u64, u16)>,
    /// 访问模式历史
    access_history: VecDeque<(u64, u16)>,
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

        let _timestamp = self.global_timestamp.fetch_add(1, Ordering::Relaxed) as u32;

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

        self.stats.record_miss();
        self.stats.record_lookup(start_time.elapsed());
        self.update_access_pattern(vpn, asid);
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

        if !self.l1_tlb.insert(entry) {
            if !self.l2_tlb.insert(entry) {
                self.l3_tlb.insert(entry);
            }
        }
    }

    /// 提升条目到L1
    fn promote_to_l1(&mut self, entry: OptimizedTlbEntry) {
        let mut promoted_entry = entry;
        promoted_entry.hot_mark = true;

        if !self.l1_tlb.insert(promoted_entry) {
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

        if let Some(&(prev_vpn, _)) = self.access_history.back() {
            if current_vpn == prev_vpn + 1 {
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
        let _ = Arc::try_unwrap(self.stats.clone()).map(|_stats| AtomicTlbStats::new());
    }
}

impl TlbManager for MultiLevelTlb {
    fn lookup(&mut self, addr: GuestAddr, asid: u16, access: AccessType) -> Option<vm_core::TlbEntry> {
        let vpn = addr.0 >> PAGE_SHIFT;
        let key = SingleLevelTlb::make_key(vpn, asid);

        if let Some(entry) = self.l1_tlb.entries.get(&key) {
            if entry.check_permission(access) {
                return Some(vm_core::TlbEntry {
                    guest_addr: addr,
                    phys_addr: GuestPhysAddr(entry.ppn << PAGE_SHIFT),
                    flags: entry.flags,
                    asid,
                });
            }
        }

        if let Some(entry) = self.l2_tlb.entries.get(&key) {
            if entry.check_permission(access) {
                return Some(vm_core::TlbEntry {
                    guest_addr: addr,
                    phys_addr: GuestPhysAddr(entry.ppn << PAGE_SHIFT),
                    flags: entry.flags,
                    asid,
                });
            }
        }

        if let Some(entry) = self.l3_tlb.entries.get(&key) {
            if entry.check_permission(access) {
                return Some(vm_core::TlbEntry {
                    guest_addr: addr,
                    phys_addr: GuestPhysAddr(entry.ppn << PAGE_SHIFT),
                    flags: entry.flags,
                    asid,
                });
            }
        }

        None
    }

    fn update(&mut self, entry: vm_core::TlbEntry) {
        let vpn = entry.guest_addr.0 >> PAGE_SHIFT;
        let ppn = entry.phys_addr.0 >> PAGE_SHIFT;
        self.insert(vpn, ppn, entry.flags, entry.asid);
    }

    fn flush(&mut self) {
        self.flush_all();
    }

    fn flush_asid(&mut self, asid: u16) {
        self.l1_tlb.flush_asid(asid);
        self.l2_tlb.flush_asid(asid);
        self.l3_tlb.flush_asid(asid);
    }

    fn get_stats(&self) -> Option<vm_core::TlbStats> {
        let total_lookups = self.stats.total_lookups.load(Ordering::Relaxed);
        let hits = self.stats.l1_hits.load(Ordering::Relaxed) + self.stats.l2_hits.load(Ordering::Relaxed) + self.stats.l3_hits.load(Ordering::Relaxed);
        let misses = self.stats.total_misses.load(Ordering::Relaxed);
        let hit_rate = if total_lookups > 0 {
            hits as f64 / total_lookups as f64
        } else {
            0.0
        };

        Some(vm_core::TlbStats {
            total_lookups,
            hits,
            misses,
            hit_rate,
            current_entries: (self.l1_tlb.usage() + self.l2_tlb.usage() + self.l3_tlb.usage()) as usize,
            capacity: (self.config.l1_capacity + self.config.l2_capacity + self.config.l3_capacity) as usize,
        })
    }
}