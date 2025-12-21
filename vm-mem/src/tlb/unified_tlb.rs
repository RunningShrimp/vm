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

use crate::memory::memory_pool::{MemoryPool, StackPool};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use vm_core::{AccessType, GuestAddr, GuestPhysAddr};

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
            result_pool: Arc::new(RwLock::new(StackPool::<TlbResult>::with_capacity(
                max_entries * 2,
            ))),
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
            pool.allocate().unwrap_or(TlbResult {
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
            inner: std::sync::Mutex::new(crate::MultiLevelTlb::new(
                crate::MultiLevelTlbConfig::default(),
            )),
        }
    }
}

#[cfg(feature = "tlb-optimized")]
impl UnifiedTlb for OptimizedTlb {
    fn lookup(&self, gva: GuestAddr, access_type: AccessType) -> Option<TlbResult> {
        let mut inner = self.inner.lock().unwrap();
        // ConcurrentTlbManager uses translate method instead of lookup
        inner
            .translate(gva.0, 0, access_type)
            .map(|(phys_addr, flags)| TlbResult {
                gpa: GuestPhysAddr(phys_addr),
                flags,
                page_size: 4096, // 默认4KB页面
                hit: true,
            })
    }

    fn insert(&self, gva: GuestAddr, gpa: GuestPhysAddr, flags: u64, asid: u16) {
        let mut inner = self.inner.lock().unwrap();
        // Convert gva to vpn (virtual page number)
        let vpn = gva >> 12;
        inner.insert(vpn, gpa.0, flags, asid);
    }

    fn invalidate(&self, gva: GuestAddr) {
        let mut inner = self.inner.lock().unwrap();
        // MultiLevelTlb doesn't have a direct invalidate method,
        // so we clear the entire TLB as a simple implementation
        // In a production system, we would implement selective invalidation
        // based on the virtual page number derived from gva
        let _vpn = gva >> 12; // Extract VPN from the address
        inner.flush_all(); // Flush all entries as a simple invalidation strategy
    }

    fn invalidate_all(&self) {
        let mut inner = self.inner.lock().unwrap();
        // Clear all entries in the TLB
        inner.flush_all();
    }

    fn get_stats(&self) -> TlbStats {
        let mut inner = self.inner.lock().unwrap();
        let stats = inner.get_stats();
        use std::sync::atomic::Ordering;
        TlbStats {
            lookups: stats.total_lookups.load(Ordering::Relaxed),
            hits: stats.l1_hits.load(Ordering::Relaxed)
                + stats.l2_hits.load(Ordering::Relaxed)
                + stats.l3_hits.load(Ordering::Relaxed),
            misses: stats.total_misses.load(Ordering::Relaxed),
            invalidations: 0, // Not tracked in MultiLevelTlb
            prefetches: 0,    // Not tracked in MultiLevelTlb
        }
    }

    fn flush(&self) {
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
            inner: std::sync::Mutex::new(crate::ConcurrentTlbManager::new(
                crate::ConcurrentTlbConfig::default(),
            )),
        }
    }
}

#[cfg(feature = "tlb-concurrent")]
impl UnifiedTlb for ConcurrentTlb {
    fn lookup(&self, gva: GuestAddr, access_type: AccessType) -> Option<TlbResult> {
        let mut inner = self.inner.lock().unwrap();
        // ConcurrentTlbManager uses translate method instead of lookup
        inner
            .translate(gva.0, 0, access_type)
            .map(|(phys_addr, flags)| TlbResult {
                gpa: GuestPhysAddr(phys_addr),
                flags,
                page_size: 4096, // 默认4KB页面
                hit: true,
            })
    }

    fn insert(&self, gva: GuestAddr, gpa: GuestPhysAddr, flags: u64, asid: u16) {
        let mut inner = self.inner.lock().unwrap();
        // Convert gva to vpn (virtual page number)
        let vpn = gva >> 12;
        inner.insert(vpn, gpa.0, flags, asid);
    }

    fn invalidate(&self, gva: GuestAddr) {
        let mut inner = self.inner.lock().unwrap();
        // ConcurrentTlbManager doesn't have a direct invalidate method,
        // so we clear all entries as a simple implementation
        // In a production system, we would implement selective invalidation
        // based on the virtual page number derived from gva
        let _vpn = gva >> 12; // Extract VPN from the address
        inner.flush_all(); // Flush all entries as a simple invalidation strategy
    }

    fn invalidate_all(&self) {
        let mut inner = self.inner.lock().unwrap();
        // Clear all entries in the ConcurrentTlbManager
        inner.flush_all();
    }

    fn get_stats(&self) -> TlbStats {
        let mut inner = self.inner.lock().unwrap();
        let stats = inner.get_stats();
        use std::sync::atomic::Ordering;
        TlbStats {
            lookups: stats.total_lookups.load(Ordering::Relaxed),
            hits: stats.fast_path_hits.load(Ordering::Relaxed)
                + stats.sharded_hits.load(Ordering::Relaxed),
            misses: stats.total_misses.load(Ordering::Relaxed),
            invalidations: 0, // Not tracked in ConcurrentTlbManager
            prefetches: 0,    // Not tracked in ConcurrentTlbManager
        }
    }

    fn flush(&self) {
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
        // Log requested capacity for debugging and future optimization
        log::debug!("Creating TLB with capacity: {} entries", max_entries);

        #[cfg(feature = "tlb-concurrent")]
        {
            // ConcurrentTlb could potentially be configured with capacity in future versions
            log::debug!(
                "Using ConcurrentTlb implementation (capacity parameter noted for future optimization)"
            );
            return Self::create_concurrent_tlb();
        }

        #[cfg(all(feature = "tlb-optimized", not(feature = "tlb-concurrent")))]
        {
            // OptimizedTlb could potentially be configured with capacity in future versions
            log::debug!(
                "Using OptimizedTlb implementation (capacity parameter noted for future optimization)"
            );
            return Self::create_optimized_tlb();
        }

        #[cfg(all(
            feature = "tlb-basic",
            not(feature = "tlb-optimized"),
            not(feature = "tlb-concurrent")
        ))]
        {
            log::debug!("Using BasicTlb with custom capacity: {}", max_entries);
            return Self::create_basic_tlb(max_entries);
        }

        #[cfg(not(any(
            feature = "tlb-basic",
            feature = "tlb-optimized",
            feature = "tlb-concurrent"
        )))]
        {
            // 默认使用基础实现，使用传入的容量参数
            log::debug!("Using default BasicTlb with capacity: {}", max_entries);
            return Box::new(BasicTlb::new(max_entries));
        }

        // 如果没有匹配的特性，使用基础实现作为后备
        #[allow(unreachable_code)]
        {
            log::debug!("Fallback: Using BasicTlb with capacity: {}", max_entries);
            Box::new(BasicTlb::new(max_entries))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_core::{GuestAddr, GuestPhysAddr};

    #[test]
    #[cfg(feature = "tlb-basic")]
    fn test_basic_tlb() {
        let tlb = BasicTlb::new(16);

        // 插入条目
        tlb.insert(GuestAddr(0x1000), GuestPhysAddr(0x2000), 0x7, 0);

        // 查找条目
        let result = tlb.lookup(GuestAddr(0x1000), AccessType::Read);
        assert!(result.is_some());
        assert_eq!(result.unwrap().gpa, GuestPhysAddr(0x2000));

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
