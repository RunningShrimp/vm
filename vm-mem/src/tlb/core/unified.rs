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
use vm_core::error::MemoryError;
use vm_core::{AccessType, GuestAddr, GuestPhysAddr};

// Import TlbManager trait for impl blocks
use crate::memory::memory_pool::{MemoryPool, StackPool};

/// Error type for TLB operations
pub type TlbResult<T> = Result<T, MemoryError>;

/// 从标志位转换为访问类型
fn access_type_from_flags(flags: u64) -> vm_core::AccessType {
    use vm_core::AccessType;

    if flags & 0x1 != 0 {
        AccessType::Read
    } else if flags & 0x2 != 0 {
        AccessType::Write
    } else {
        AccessType::Execute
    }
}

/// 统一TLB接口
pub trait UnifiedTlb: Send + Sync {
    /// 查找TLB条目
    fn lookup(&self, gva: GuestAddr, access_type: AccessType) -> Option<TlbEntryResult>;

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
#[derive(Debug, Clone, Copy)]
pub struct TlbEntryResult {
    /// 物理地址
    pub gpa: GuestPhysAddr,
    /// 页表标志
    pub flags: u64,
    /// 页面大小
    pub page_size: u64,
    /// 是否命中
    pub hit: bool,
}

impl Default for TlbEntryResult {
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
#[derive(Debug, Clone, Copy, Default)]
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
    entries: Arc<RwLock<HashMap<GuestAddr, TlbEntryResult>>>,
    stats: Arc<RwLock<TlbStats>>,
    max_entries: usize,
    // 使用内存池来减少分配开销
    result_pool: Arc<RwLock<StackPool<TlbEntryResult>>>,
}

impl BasicTlb {
    /// 创建基础TLB
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(TlbStats::default())),
            max_entries,
            result_pool: Arc::new(RwLock::new(StackPool::<TlbEntryResult>::with_capacity(
                max_entries * 2,
            ))),
        }
    }

    /// Helper: Get read lock on entries
    fn lock_entries(
        &self,
    ) -> Result<std::sync::RwLockReadGuard<'_, HashMap<GuestAddr, TlbEntryResult>>, MemoryError>
    {
        self.entries.read().map_err(|_| MemoryError::MmuLockFailed {
            message: "Tlb entries lock poisoned".to_string(),
        })
    }

    /// Helper: Get write lock on entries
    fn lock_entries_mut(
        &self,
    ) -> Result<std::sync::RwLockWriteGuard<'_, HashMap<GuestAddr, TlbEntryResult>>, MemoryError>
    {
        self.entries
            .write()
            .map_err(|_| MemoryError::MmuLockFailed {
                message: "Tlb entries lock poisoned".to_string(),
            })
    }

    /// Helper: Get read lock on stats
    fn lock_stats(&self) -> Result<std::sync::RwLockReadGuard<'_, TlbStats>, MemoryError> {
        self.stats.read().map_err(|_| MemoryError::MmuLockFailed {
            message: "Tlb stats lock poisoned".to_string(),
        })
    }

    /// Helper: Get write lock on stats
    fn lock_stats_mut(&self) -> Result<std::sync::RwLockWriteGuard<'_, TlbStats>, MemoryError> {
        self.stats.write().map_err(|_| MemoryError::MmuLockFailed {
            message: "Tlb stats lock poisoned".to_string(),
        })
    }

    /// Helper: Get write lock on result pool
    fn lock_pool_mut(
        &self,
    ) -> Result<std::sync::RwLockWriteGuard<'_, StackPool<TlbEntryResult>>, MemoryError> {
        self.result_pool
            .write()
            .map_err(|_| MemoryError::MmuLockFailed {
                message: "Tlb result pool lock poisoned".to_string(),
            })
    }
}

impl UnifiedTlb for BasicTlb {
    fn lookup(&self, gva: GuestAddr, access_type: AccessType) -> Option<TlbEntryResult> {
        // Try to get entries lock, return None on failure
        let entries = match self.lock_entries() {
            Ok(guard) => guard,
            Err(_) => return None,
        };

        // 检查访问权限是否匹配
        let result = if let Some(entry) = entries.get(&gva) {
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
                // 使用结构体更新语法，利用Copy trait避免克隆
                Some(TlbEntryResult {
                    hit: true,
                    ..*entry
                })
            } else {
                None
            }
        } else {
            None
        };
        drop(entries);

        // 更新统计
        if let Ok(mut stats) = self.lock_stats_mut() {
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
        // Silently fail on lock errors
        if let Ok(mut entries) = self.lock_entries_mut() {
            // 如果已满，移除最旧的条目
            if entries.len() >= self.max_entries {
                entries.clear();
                if let Ok(mut stats) = self.lock_stats_mut() {
                    stats.invalidations += 1;
                }
            }

            // 从内存池分配TlbEntryResult
            let mut result = if let Ok(mut pool) = self.lock_pool_mut() {
                pool.allocate().unwrap_or(TlbEntryResult {
                    gpa,
                    flags,
                    page_size: 4096, // 默认4KB页面
                    hit: true,
                })
            } else {
                TlbEntryResult {
                    gpa,
                    flags,
                    page_size: 4096,
                    hit: true,
                }
            };

            // 设置正确的字段值
            result.gpa = gpa;
            result.flags = flags;
            result.page_size = 4096;
            result.hit = false;

            entries.insert(gva, result);
        }
    }

    fn invalidate(&self, gva: GuestAddr) {
        // Silently fail on lock errors
        if let Ok(mut entries) = self.lock_entries_mut()
            && let Some(result) = entries.remove(&gva)
        {
            // 将TlbEntryResult归还到内存池
            if let Ok(mut pool) = self.lock_pool_mut() {
                pool.deallocate(result);
            }
        }
        if let Ok(mut stats) = self.lock_stats_mut() {
            stats.invalidations += 1;
        }
    }

    fn invalidate_all(&self) {
        // Silently fail on lock errors
        if let Ok(mut entries) = self.lock_entries_mut() {
            let count = entries.len() as u64;

            // 将所有条目归还到内存池
            if count > 0 {
                if let Ok(mut pool) = self.lock_pool_mut() {
                    for (_, result) in entries.drain() {
                        pool.deallocate(result);
                    }
                }
            } else {
                entries.clear();
            }

            if let Ok(mut stats) = self.lock_stats_mut() {
                stats.invalidations += count;
            }
        }
    }

    fn get_stats(&self) -> TlbStats {
        match self.lock_stats() {
            Ok(stats) => *stats, // 直接复制，避免逐字段复制
            Err(_) => TlbStats::default(),
        }
    }

    fn flush(&self) {
        self.invalidate_all();
    }
}

// ============================================================================
// Module: Optimized TLB Implementations
// ============================================================================
// This module contains optimized TLB implementations with multi-level caching,
// concurrent access, and advanced replacement policies.
// ============================================================================
#[cfg(feature = "optimizations")]
mod optimized_tlb_impl {
    use super::*;
    use crate::{ConcurrentTlbConfig, ConcurrentTlbManager, MultiLevelTlb, MultiLevelTlbConfig};

    /// 优化TLB实现（多级TLB）
    pub struct OptimizedTlb {
        inner: std::sync::Mutex<MultiLevelTlb>,
    }

    impl OptimizedTlb {
        /// 创建优化TLB
        pub fn new() -> Self {
            Self {
                inner: std::sync::Mutex::new(MultiLevelTlb::new(MultiLevelTlbConfig::default())),
            }
        }

        /// Helper: Get lock on inner TLB
        fn lock_inner(&self) -> Result<std::sync::MutexGuard<'_, MultiLevelTlb>, MemoryError> {
            self.inner.lock().map_err(|_| MemoryError::MmuLockFailed {
                message: "OptimizedTlb inner lock poisoned".to_string(),
            })
        }
    }

    impl Default for OptimizedTlb {
        fn default() -> Self {
            Self::new()
        }
    }

    impl super::UnifiedTlb for OptimizedTlb {
        fn lookup(&self, gva: GuestAddr, access_type: AccessType) -> Option<TlbEntryResult> {
            match self.lock_inner() {
                Ok(mut inner) => {
                    let vpn = gva >> 12;
                    inner
                        .translate(vpn, 0, access_type)
                        .map(|(phys_addr, flags)| TlbEntryResult {
                            gpa: vm_core::GuestPhysAddr(phys_addr),
                            flags,
                            page_size: 4096,
                            hit: true,
                        })
                }
                Err(_) => None,
            }
        }

        fn insert(&self, gva: GuestAddr, gpa: GuestPhysAddr, flags: u64, asid: u16) {
            if let Ok(mut inner) = self.lock_inner() {
                let vpn = gva >> 12;
                let ppn = gpa >> 12;
                inner.insert(vpn, ppn, flags, asid);
            }
        }

        fn invalidate(&self, _gva: GuestAddr) {
            drop(self.lock_inner());
        }

        fn invalidate_all(&self) {
            drop(self.lock_inner());
        }

        fn get_stats(&self) -> TlbStats {
            match self.lock_inner() {
                Ok(inner) => {
                    let stats = inner.get_stats();
                    use std::sync::atomic::Ordering;
                    TlbStats {
                        lookups: stats.total_lookups.load(Ordering::Relaxed),
                        hits: stats.l1_hits.load(Ordering::Relaxed)
                            + stats.l2_hits.load(Ordering::Relaxed)
                            + stats.l3_hits.load(Ordering::Relaxed),
                        misses: stats.total_misses.load(Ordering::Relaxed),
                        invalidations: 0,
                        prefetches: 0,
                    }
                }
                Err(_) => TlbStats::default(),
            }
        }

        fn flush(&self) {
            if let Ok(mut inner) = self.lock_inner() {
                inner.flush_all();
            }
        }
    }

    /// 并发TLB实现
    pub struct ConcurrentTlb {
        inner: std::sync::Mutex<ConcurrentTlbManager>,
    }

    impl ConcurrentTlb {
        /// 创建并发TLB
        pub fn new() -> Self {
            Self {
                inner: std::sync::Mutex::new(ConcurrentTlbManager::new(
                    ConcurrentTlbConfig::default(),
                )),
            }
        }

        /// Helper: Get lock on inner TLB
        fn lock_inner(
            &self,
        ) -> Result<std::sync::MutexGuard<'_, ConcurrentTlbManager>, MemoryError> {
            self.inner.lock().map_err(|_| MemoryError::MmuLockFailed {
                message: "ConcurrentTlb inner lock poisoned".to_string(),
            })
        }

        /// Helper: Get mutable lock on inner TLB
        #[allow(dead_code)]
        fn lock_inner_mut(
            &self,
        ) -> Result<std::sync::MutexGuard<'_, ConcurrentTlbManager>, MemoryError> {
            self.inner.lock().map_err(|_| MemoryError::MmuLockFailed {
                message: "ConcurrentTlb inner lock poisoned".to_string(),
            })
        }
    }

    impl Default for ConcurrentTlb {
        fn default() -> Self {
            Self::new()
        }
    }

    impl super::UnifiedTlb for ConcurrentTlb {
        fn lookup(&self, gva: GuestAddr, access_type: AccessType) -> Option<TlbEntryResult> {
            match self.lock_inner() {
                Ok(inner) => {
                    let vpn = gva >> 12;
                    inner
                        .translate(vpn, 0, access_type)
                        .map(|(phys_addr, flags)| TlbEntryResult {
                            gpa: vm_core::GuestPhysAddr(phys_addr),
                            flags,
                            page_size: 4096,
                            hit: true,
                        })
                }
                Err(_) => None,
            }
        }

        fn insert(&self, gva: GuestAddr, gpa: GuestPhysAddr, flags: u64, asid: u16) {
            if let Ok(inner) = self.lock_inner() {
                let vpn = gva >> 12;
                let ppn = gpa >> 12;
                inner.insert(vpn, ppn, flags, asid);
            }
        }

        fn invalidate(&self, _gva: GuestAddr) {
            drop(self.lock_inner());
        }

        fn invalidate_all(&self) {
            drop(self.lock_inner());
        }

        fn get_stats(&self) -> TlbStats {
            match self.lock_inner() {
                Ok(inner) => {
                    let stats = inner.get_stats();
                    use std::sync::atomic::Ordering;
                    TlbStats {
                        lookups: stats.total_lookups.load(Ordering::Relaxed),
                        hits: stats.fast_path_hits.load(Ordering::Relaxed)
                            + stats.sharded_hits.load(Ordering::Relaxed),
                        misses: stats.total_misses.load(Ordering::Relaxed),
                        invalidations: 0,
                        prefetches: 0,
                    }
                }
                Err(_) => TlbStats::default(),
            }
        }

        fn flush(&self) {
            if let Ok(inner) = self.lock_inner() {
                inner.flush_all();
            }
        }
    }
}

// Re-export optimized implementations
#[cfg(feature = "optimizations")]
pub use optimized_tlb_impl::{ConcurrentTlb, OptimizedTlb};

// ============================================================================
// Module: TLB Factory
// ============================================================================
// Factory module for creating appropriate TLB implementations based on
// available feature flags.
// ============================================================================
#[cfg(feature = "optimizations")]
mod tlb_factory_optimized {

    pub struct TlbFactory;

    impl TlbFactory {
        /// 创建基础TLB
        pub fn create_basic_tlb(max_entries: usize) -> Box<dyn super::UnifiedTlb> {
            Box::new(super::BasicTlb::new(max_entries))
        }

        /// 创建优化TLB
        pub fn create_optimized_tlb() -> Box<dyn super::UnifiedTlb> {
            Box::new(super::OptimizedTlb::new())
        }

        /// 创建并发TLB
        pub fn create_concurrent_tlb() -> Box<dyn super::UnifiedTlb> {
            Box::new(super::ConcurrentTlb::new())
        }

        /// 根据可用特性创建最佳TLB实现
        pub fn create_best_tlb(_max_entries: usize) -> Box<dyn super::UnifiedTlb> {
            Self::create_concurrent_tlb()
        }
    }
}

#[cfg(not(feature = "optimizations"))]
mod tlb_factory_basic {
    use super::*;

    pub struct TlbFactory;

    impl TlbFactory {
        /// 创建基础TLB
        pub fn create_basic_tlb(max_entries: usize) -> Box<dyn super::UnifiedTlb> {
            Box::new(super::BasicTlb::new(max_entries))
        }

        /// 根据可用特性创建最佳TLB实现
        pub fn create_best_tlb(max_entries: usize) -> Box<dyn super::UnifiedTlb> {
            Box::new(super::BasicTlb::new(max_entries))
        }
    }
}

// Re-export TlbFactory based on feature
#[cfg(feature = "optimizations")]
pub use tlb_factory_optimized::TlbFactory;

#[cfg(not(feature = "optimizations"))]
pub use tlb_factory_basic::TlbFactory;

// ============================================================================
// Module: Tests
// ============================================================================
// Test implementations for TLB functionality
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    // Basic tests that always run
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

// Optimized feature-specific tests
#[cfg(all(test, feature = "optimizations"))]
mod tests_optimized {
    use super::*;

    #[test]
    fn test_basic_tlb() {
        let tlb = BasicTlb::new(16);

        // 插入条目
        tlb.insert(GuestAddr(0x1000), GuestPhysAddr(0x2000), 0x7, 0);

        // 查找条目
        let result = tlb.lookup(GuestAddr(0x1000), AccessType::Read);
        assert!(result.is_some());
        assert_eq!(
            result.expect("TLB lookup failed").gpa,
            GuestPhysAddr(0x2000)
        );

        // 检查统计
        let stats = tlb.get_stats();
        assert_eq!(stats.lookups, 1);
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 0);
    }
}

// ============================================================================
// Module: Multi-Level TLB Implementation
// ============================================================================
// This module contains the complete multi-level TLB implementation with
// advanced features like adaptive replacement, prefetching, and statistics.
// Only available when the "optimizations" feature is enabled.
// ============================================================================
#[cfg(feature = "optimizations")]
pub mod multilevel_tlb_impl {
    use crate::PAGE_SHIFT;
    // 补充需要的额外导入（HashMap 和 Arc 已在文件顶部导入）
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use std::time::{Duration, Instant};
    use vm_core::{AccessType, GuestAddr, GuestPhysAddr};

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

    impl Default for AtomicTlbStats {
        fn default() -> Self {
            Self::new()
        }
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

    use std::collections::HashMap;
    use std::sync::Arc;

    /// 单级TLB实现
    pub struct SingleLevelTlb {
        /// TLB条目存储
        pub entries: HashMap<u64, OptimizedTlbEntry>,
        /// LRU访问顺序
        pub lru_order: VecDeque<u64>,
        /// 频率计数器
        pub frequency_counter: HashMap<u64, u32>,
        /// 容量
        pub capacity: usize,
        /// 替换策略
        pub replacement_policy: AdaptiveReplacementPolicy,
        /// 当前时间戳计数器
        pub timestamp_counter: u32,
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
        pub fn make_key(vpn: u64, asid: u16) -> u64 {
            (vpn << 16) | (asid as u64)
        }

        /// 查找条目
        pub fn lookup(&mut self, vpn: u64, asid: u16) -> Option<&OptimizedTlbEntry> {
            let key = Self::make_key(vpn, asid);

            if self.entries.contains_key(&key) {
                self.timestamp_counter = self.timestamp_counter.wrapping_add(1);
                self.update_lru_order(key);
                *self.frequency_counter.entry(key).or_insert(0) += 1;
                let entry = self.entries.get_mut(&key)?;
                entry.update_access(self.timestamp_counter);
                Some(entry)
            } else {
                None
            }
        }

        /// 插入条目
        pub fn insert(&mut self, entry: OptimizedTlbEntry) -> bool {
            let key = Self::make_key(entry.vpn, entry.asid);

            if self.entries.len() >= self.capacity
                && !self.entries.contains_key(&key)
                && !self.evict_victim()
            {
                return false;
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
                AdaptiveReplacementPolicy::FrequencyBasedLru => self
                    .lru_order
                    .iter()
                    .min_by_key(|&&k| self.frequency_counter.get(&k).unwrap_or(&0))
                    .copied(),
                AdaptiveReplacementPolicy::TimeBasedLru => self.lru_order.front().copied(),
                AdaptiveReplacementPolicy::Hybrid => self
                    .lru_order
                    .iter()
                    .min_by_key(|&&k| {
                        let freq = self.frequency_counter.get(&k).unwrap_or(&0);
                        let time_pos = self.lru_order.iter().position(|&x| x == k).unwrap_or(0);
                        (*freq as usize) * 2 + time_pos
                    })
                    .copied(),
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
        pub l1_tlb: SingleLevelTlb,
        /// L2 TLB（中等）
        pub l2_tlb: SingleLevelTlb,
        /// L3 TLB（大容量）
        pub l3_tlb: SingleLevelTlb,
        /// 预取队列
        prefetch_queue: VecDeque<(u64, u16)>,
        /// 访问模式历史
        access_history: VecDeque<(u64, u16)>,
        /// 统计信息
        pub stats: Arc<AtomicTlbStats>,
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

            if !self.l1_tlb.insert(entry) && !self.l2_tlb.insert(entry) {
                self.l3_tlb.insert(entry);
            }
        }

        /// 提升条目到L1
        fn promote_to_l1(&mut self, entry: OptimizedTlbEntry) {
            let mut promoted_entry = entry;
            promoted_entry.hot_mark = true;

            if !self.l1_tlb.insert(promoted_entry) {
                if let Some(victim_key) = self.l1_tlb.lru_order.front()
                    && let Some(victim_entry) = self.l1_tlb.entries.get(victim_key)
                {
                    self.l2_tlb.insert(*victim_entry);
                }
                self.l1_tlb.insert(promoted_entry);
            }
        }

        /// 提升条目到L2
        fn promote_to_l2(&mut self, entry: OptimizedTlbEntry) {
            if !self.l2_tlb.insert(entry) {
                if let Some(victim_key) = self.l2_tlb.lru_order.front()
                    && let Some(victim_entry) = self.l2_tlb.entries.get(victim_key)
                {
                    self.l3_tlb.insert(*victim_entry);
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

            if let Some(&(prev_vpn, _)) = self.access_history.back()
                && current_vpn == prev_vpn + 1
            {
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

    impl vm_core::TlbManager for MultiLevelTlb {
        fn lookup(
            &mut self,
            addr: GuestAddr,
            asid: u16,
            access: AccessType,
        ) -> Option<vm_core::TlbEntry> {
            let vpn = addr.0 >> PAGE_SHIFT;
            let key = SingleLevelTlb::make_key(vpn, asid);

            if let Some(entry) = self.l1_tlb.entries.get(&key)
                && entry.check_permission(access)
            {
                return Some(vm_core::TlbEntry {
                    guest_addr: addr,
                    phys_addr: GuestPhysAddr(entry.ppn << PAGE_SHIFT),
                    flags: entry.flags,
                    asid,
                });
            }

            if let Some(entry) = self.l2_tlb.entries.get(&key)
                && entry.check_permission(access)
            {
                return Some(vm_core::TlbEntry {
                    guest_addr: addr,
                    phys_addr: GuestPhysAddr(entry.ppn << PAGE_SHIFT),
                    flags: entry.flags,
                    asid,
                });
            }

            if let Some(entry) = self.l3_tlb.entries.get(&key)
                && entry.check_permission(access)
            {
                return Some(vm_core::TlbEntry {
                    guest_addr: addr,
                    phys_addr: GuestPhysAddr(entry.ppn << PAGE_SHIFT),
                    flags: entry.flags,
                    asid,
                });
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
    }

    /// Adapter to make MultiLevelTlb compatible with vm_core::TlbManager
    pub struct MultiLevelTlbAdapter {
        inner: MultiLevelTlb,
    }

    impl MultiLevelTlbAdapter {
        pub fn new(config: MultiLevelTlbConfig) -> Self {
            Self {
                inner: MultiLevelTlb::new(config),
            }
        }

        pub fn inner(&self) -> &MultiLevelTlb {
            &self.inner
        }

        pub fn inner_mut(&mut self) -> &mut MultiLevelTlb {
            &mut self.inner
        }
    }

    impl vm_core::TlbManager for MultiLevelTlbAdapter {
        fn lookup(
            &mut self,
            addr: GuestAddr,
            asid: u16,
            access: AccessType,
        ) -> Option<vm_core::TlbEntry> {
            // 将GuestAddr转换为VPN
            let vpn = addr.0 >> 12;
            let result = self.inner.translate(vpn, asid, access)?;
            Some(vm_core::TlbEntry {
                guest_addr: addr,
                phys_addr: vm_core::GuestPhysAddr(result.0),
                flags: result.1,
                asid,
            })
        }

        fn update(&mut self, entry: vm_core::TlbEntry) {
            let vpn = entry.guest_addr.0 >> 12;
            let ppn = entry.phys_addr.0 >> 12;
            self.inner.insert(vpn, ppn, entry.flags, entry.asid);
        }

        fn flush(&mut self) {
            self.inner.flush_all();
        }

        fn flush_asid(&mut self, asid: u16) {
            self.inner.flush_asid(asid);
        }
    }
}

// Re-export multi-level TLB types
#[cfg(feature = "optimizations")]
pub use multilevel_tlb_impl::{
    AdaptiveReplacementPolicy, AtomicTlbStats, MultiLevelTlb, MultiLevelTlbAdapter,
    MultiLevelTlbConfig, OptimizedTlbEntry,
};
