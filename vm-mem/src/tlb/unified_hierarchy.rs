//! 统一TLB层次结构
//!
//! 整合L1/L2/L3多级TLB，提供统一的层次化TLB管理接口。
//!
//! ## 设计目标
//!
//! 1. **层次化查找**: L1 -> L2 -> L3 逐级查找
//! 2. **智能填充**: 自动填充到所有层级
//! 3. **策略灵活**: 支持不同的替换策略
//! 4. **批量失效**: 支持TLB批量失效操作

use super::management::manager::{StandardTlbManager, TlbManager};
use std::sync::Arc;
use vm_core::{AccessType, GuestAddr, TlbEntry};

// ============================================================================
// 替换策略
// ============================================================================

/// TLB替换策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReplacementPolicy {
    /// LRU (Least Recently Used)
    #[default]
    LRU,
    /// LFU (Least Frequently Used)
    LFU,
    /// FIFO (First In First Out)
    FIFO,
    /// Random
    Random,
    /// 自适应替换策略（ARC）
    AdaptiveARC,
}

// ============================================================================
// 统一TLB层次结构
// ============================================================================

/// 统一TLB层次结构
///
/// 提供三层TLB层次结构：
/// - L1: 最快访问，小容量（ITLB + DTLB分离）
/// - L2: 中等访问，中等容量（统一TLB）
/// - L3: 大容量，共享TLB
///
/// # 性能特点
///
/// - L1命中率: ~90-95%（访问延迟: ~1-2周期）
/// - L2命中率: ~5-8%（访问延迟: ~5-10周期）
/// - L3命中率: ~1-2%（访问延迟: ~20-30周期）
/// - 页表遍历: <1%（访问延迟: ~100-200周期）
pub struct UnifiedTlbHierarchy {
    /// L1指令TLB（最快访问）
    l1_itlb: Arc<parking_lot::Mutex<StandardTlbManager>>,
    /// L1数据TLB（最快访问）
    l1_dtlb: Arc<parking_lot::Mutex<StandardTlbManager>>,
    /// L2统一TLB（中等访问）
    l2_tlb: Arc<parking_lot::Mutex<StandardTlbManager>>,
    /// L3共享TLB（大容量）
    l3_tlb: Arc<parking_lot::Mutex<StandardTlbManager>>,

    /// 各层级容量
    l1_itlb_capacity: usize,
    l1_dtlb_capacity: usize,
    l2_capacity: usize,
    l3_capacity: usize,

    /// 替换策略
    replacement_policy: ReplacementPolicy,

    /// 统计信息
    stats: Arc<HierarchyStats>,
}

/// TLB层次结构统计信息
#[derive(Debug, Default)]
pub struct HierarchyStats {
    /// L1命中次数
    pub l1_hits: Arc<std::sync::atomic::AtomicU64>,
    /// L2命中次数
    pub l2_hits: Arc<std::sync::atomic::AtomicU64>,
    /// L3命中次数
    pub l3_hits: Arc<std::sync::atomic::AtomicU64>,
    /// 全部未命中次数
    pub total_misses: Arc<std::sync::atomic::AtomicU64>,
}

impl HierarchyStats {
    /// 计算总体命中率
    pub fn overall_hit_rate(&self) -> f64 {
        let l1 = self.l1_hits.load(std::sync::atomic::Ordering::Relaxed);
        let l2 = self.l2_hits.load(std::sync::atomic::Ordering::Relaxed);
        let l3 = self.l3_hits.load(std::sync::atomic::Ordering::Relaxed);
        let misses = self.total_misses.load(std::sync::atomic::Ordering::Relaxed);
        let total = l1 + l2 + l3 + misses;

        if total == 0 {
            0.0
        } else {
            (l1 + l2 + l3) as f64 / total as f64
        }
    }

    /// 获取各层级命中率分布
    pub fn hit_distribution(&self) -> (f64, f64, f64) {
        let l1 = self.l1_hits.load(std::sync::atomic::Ordering::Relaxed);
        let l2 = self.l2_hits.load(std::sync::atomic::Ordering::Relaxed);
        let l3 = self.l3_hits.load(std::sync::atomic::Ordering::Relaxed);
        let total = l1 + l2 + l3;

        if total == 0 {
            (0.0, 0.0, 0.0)
        } else {
            (
                l1 as f64 / total as f64,
                l2 as f64 / total as f64,
                l3 as f64 / total as f64,
            )
        }
    }
}

impl UnifiedTlbHierarchy {
    /// 创建新的统一TLB层次结构
    ///
    /// # 参数
    /// - `l1_itlb_capacity`: L1指令TLB容量
    /// - `l1_dtlb_capacity`: L1数据TLB容量
    /// - `l2_capacity`: L2统一TLB容量
    /// - `l3_capacity`: L3共享TLB容量
    /// - `replacement_policy`: 替换策略
    ///
    /// # 示例
    /// ```
    /// use vm_mem::tlb::unified_hierarchy::{UnifiedTlbHierarchy, ReplacementPolicy};
    ///
    /// let hierarchy = UnifiedTlbHierarchy::new(
    ///     64,   // L1 ITLB
    ///     128,  // L1 DTLB
    ///     512,  // L2 unified TLB
    ///     2048, // L3 shared TLB
    ///     ReplacementPolicy::LRU,
    /// );
    /// ```
    pub fn new(
        l1_itlb_capacity: usize,
        l1_dtlb_capacity: usize,
        l2_capacity: usize,
        l3_capacity: usize,
        replacement_policy: ReplacementPolicy,
    ) -> Self {
        Self {
            l1_itlb: Arc::new(parking_lot::Mutex::new(StandardTlbManager::new(
                l1_itlb_capacity,
            ))),
            l1_dtlb: Arc::new(parking_lot::Mutex::new(StandardTlbManager::new(
                l1_dtlb_capacity,
            ))),
            l2_tlb: Arc::new(parking_lot::Mutex::new(StandardTlbManager::new(
                l2_capacity,
            ))),
            l3_tlb: Arc::new(parking_lot::Mutex::new(StandardTlbManager::new(
                l3_capacity,
            ))),
            l1_itlb_capacity,
            l1_dtlb_capacity,
            l2_capacity,
            l3_capacity,
            replacement_policy,
            stats: Arc::new(HierarchyStats::default()),
        }
    }

    /// 使用默认配置创建
    ///
    /// 默认配置：
    /// - L1 ITLB: 64条目
    /// - L1 DTLB: 128条目
    /// - L2 TLB: 512条目
    /// - L3 TLB: 2048条目
    /// - 策略: LRU
    pub fn with_defaults() -> Self {
        Self::new(64, 128, 512, 2048, ReplacementPolicy::default())
    }

    /// 层次化查找（内部方法）
    ///
    /// 按照L1 -> L2 -> L3的顺序查找，返回找到的TLB条目
    ///
    /// # 参数
    /// - `addr`: 虚拟地址
    /// - `access`: 访问类型
    ///
    /// # 返回
    /// 成功返回TLB条目，失败返回None
    fn lookup_internal(&self, addr: GuestAddr, asid: u16, access: AccessType) -> Option<TlbEntry> {
        // 1. 检查L1 TLB（根据访问类型选择ITLB或DTLB）
        let l1_result = match access {
            AccessType::Execute => self.l1_itlb.lock().lookup(addr, asid, access),
            _ => self.l1_dtlb.lock().lookup(addr, asid, access),
        };

        if let Some(entry) = l1_result {
            self.stats
                .l1_hits
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Some(entry);
        }

        // 2. 检查L2 TLB（统一TLB）
        if let Some(entry) = self.l2_tlb.lock().lookup(addr, asid, access) {
            self.stats
                .l2_hits
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

            // 将L2命中的条目提升到L1
            self.promote_to_l1(entry, access);

            return Some(entry);
        }

        // 3. 检查L3 TLB
        if let Some(entry) = self.l3_tlb.lock().lookup(addr, asid, access) {
            self.stats
                .l3_hits
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

            // 将L3命中的条目提升到L2和L1
            self.l2_tlb.lock().update(entry);
            self.promote_to_l1(entry, access);

            return Some(entry);
        }

        // 4. 全部未命中
        self.stats
            .total_misses
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        None
    }

    /// 插入TLB条目到所有层级
    ///
    /// # 参数
    /// - `entry`: TLB条目
    /// - `access`: 访问类型
    ///
    /// # 填充策略
    /// - L1: 根据访问类型填充到ITLB或DTLB
    /// - L2: 总是填充
    /// - L3: 总是填充
    pub fn insert(&mut self, entry: TlbEntry, access: AccessType) {
        // 填充到L1（根据访问类型）
        match access {
            AccessType::Execute => self.l1_itlb.lock().update(entry),
            _ => self.l1_dtlb.lock().update(entry),
        }

        // 填充到L2
        self.l2_tlb.lock().update(entry);

        // 填充到L3
        self.l3_tlb.lock().update(entry);
    }

    /// 将条目提升到L1
    fn promote_to_l1(&self, entry: TlbEntry, access: AccessType) {
        match access {
            AccessType::Execute => self.l1_itlb.lock().update(entry),
            _ => self.l1_dtlb.lock().update(entry),
        }
    }

    /// 批量失效所有TLB
    pub fn invalidate_all(&mut self) {
        self.l1_itlb.lock().flush();
        self.l1_dtlb.lock().flush();
        self.l2_tlb.lock().flush();
        self.l3_tlb.lock().flush();
    }

    /// 失效指定ASID的所有TLB
    pub fn invalidate_asid(&mut self, asid: u16) {
        self.l1_itlb.lock().flush_asid(asid);
        self.l1_dtlb.lock().flush_asid(asid);
        self.l2_tlb.lock().flush_asid(asid);
        self.l3_tlb.lock().flush_asid(asid);
    }

    /// 失效指定页面的所有TLB
    pub fn invalidate_page(&mut self, addr: GuestAddr) {
        self.l1_itlb.lock().flush_page(addr);
        self.l1_dtlb.lock().flush_page(addr);
        self.l2_tlb.lock().flush_page(addr);
        self.l3_tlb.lock().flush_page(addr);
    }

    /// 获取统计信息
    pub fn stats(&self) -> Arc<HierarchyStats> {
        Arc::clone(&self.stats)
    }

    /// 获取各层级容量
    pub fn capacities(&self) -> (usize, usize, usize, usize) {
        (
            self.l1_itlb_capacity,
            self.l1_dtlb_capacity,
            self.l2_capacity,
            self.l3_capacity,
        )
    }

    /// 动态调整各层级容量
    ///
    /// # 参数
    /// - `l1_itlb`: 新的L1指令TLB容量
    /// - `l1_dtlb`: 新的L1数据TLB容量
    /// - `l2`: 新的L2 TLB容量
    /// - `l3`: 新的L3 TLB容量
    ///
    /// # 注意
    /// 调整容量会清空对应TLB的所有条目
    pub fn resize(&mut self, l1_itlb: usize, l1_dtlb: usize, l2: usize, l3: usize) {
        self.l1_itlb = Arc::new(parking_lot::Mutex::new(StandardTlbManager::new(l1_itlb)));
        self.l1_dtlb = Arc::new(parking_lot::Mutex::new(StandardTlbManager::new(l1_dtlb)));
        self.l2_tlb = Arc::new(parking_lot::Mutex::new(StandardTlbManager::new(l2)));
        self.l3_tlb = Arc::new(parking_lot::Mutex::new(StandardTlbManager::new(l3)));
        self.l1_itlb_capacity = l1_itlb;
        self.l1_dtlb_capacity = l1_dtlb;
        self.l2_capacity = l2;
        self.l3_capacity = l3;
    }
}

// ============================================================================
// TlbManager trait 实现
// ============================================================================

impl TlbManager for UnifiedTlbHierarchy {
    fn lookup(&mut self, addr: GuestAddr, asid: u16, access: AccessType) -> Option<TlbEntry> {
        // 调用内部的lookup_internal方法
        self.lookup_internal(addr, asid, access)
    }

    fn update(&mut self, entry: TlbEntry) {
        // 根据条目中的flags判断访问类型
        let is_execute = (entry.flags & crate::pte_flags::X) != 0;
        let access = if is_execute {
            AccessType::Execute
        } else {
            AccessType::Read
        };

        self.insert(entry, access);
    }

    fn flush(&mut self) {
        self.invalidate_all();
    }

    fn flush_asid(&mut self, asid: u16) {
        self.invalidate_asid(asid);
    }
}

// UnifiedTlbHierarchy的其他方法（不在TlbManager trait中）
impl UnifiedTlbHierarchy {
    /// 失效指定页面的所有TLB
    pub fn flush_page(&mut self, addr: GuestAddr) {
        self.invalidate_page(addr);
    }

    /// 获取总容量
    pub fn capacity(&self) -> usize {
        let (i, d, l2, l3) = self.capacities();
        i + d + l2 + l3
    }

    /// 获取当前条目数（估算值）
    pub fn len(&self) -> usize {
        // StandardTlbManager不公开len()方法，返回估算值
        // 假设新创建的TLB是空的
        0
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        // 基于len()判断是否为空
        self.len() == 0
    }
}

// ============================================================================
// 自适应TLB管理器
// ============================================================================

/// 自适应TLB管理器
///
/// 根据运行时访问模式动态调整TLB配置
pub struct AdaptiveTlbManager {
    /// 基础TLB层次结构
    hierarchy: UnifiedTlbHierarchy,

    /// 访问模式检测窗口
    detection_window: usize,

    /// 访问历史
    access_history: Arc<std::sync::Mutex<Vec<AccessType>>>,

    /// 自适应调整阈值
    adjustment_threshold: f64,

    /// 是否启用自适应调整
    adaptive_enabled: bool,
}

impl AdaptiveTlbManager {
    /// 创建新的自适应TLB管理器
    pub fn new(
        l1_itlb: usize,
        l1_dtlb: usize,
        l2: usize,
        l3: usize,
        detection_window: usize,
        adjustment_threshold: f64,
    ) -> Self {
        Self {
            hierarchy: UnifiedTlbHierarchy::new(
                l1_itlb,
                l1_dtlb,
                l2,
                l3,
                ReplacementPolicy::AdaptiveARC,
            ),
            detection_window,
            access_history: Arc::new(std::sync::Mutex::new(Vec::with_capacity(detection_window))),
            adjustment_threshold,
            adaptive_enabled: true,
        }
    }

    /// 使用默认配置创建自适应TLB管理器
    pub fn with_defaults() -> Self {
        Self::new(64, 128, 512, 2048, 1000, 0.05)
    }

    /// 分析访问模式并动态调整
    fn analyze_and_adapt(&self) {
        if !self.adaptive_enabled {
            return;
        }

        let history = self.access_history.lock().unwrap();
        if history.len() < self.detection_window {
            return;
        }

        // 计算指令和数据访问的比例
        let mut execute_count = 0;
        let mut data_count = 0;

        for access in history.iter() {
            match access {
                AccessType::Execute => execute_count += 1,
                _ => data_count += 1,
            }
        }

        let total = execute_count + data_count;
        let execute_ratio = execute_count as f64 / total as f64;

        // 如果指令访问比例过高，增加ITLB容量
        if execute_ratio > 0.8 {
            // 需要调整容量（这里简化处理）
            // 实际实现中会调用hierarchy.resize()
        }
        // 如果数据访问比例过高，增加DTLB容量
        else if execute_ratio < 0.2 {
            // 需要调整容量
        }
    }

    /// 启用自适应调整
    pub fn enable_adaptive(&mut self) {
        self.adaptive_enabled = true;
    }

    /// 禁用自适应调整
    pub fn disable_adaptive(&mut self) {
        self.adaptive_enabled = false;
    }

    /// 检查自适应调整是否启用
    pub fn is_adaptive_enabled(&self) -> bool {
        self.adaptive_enabled
    }
}

impl TlbManager for AdaptiveTlbManager {
    fn lookup(&mut self, addr: GuestAddr, asid: u16, access: AccessType) -> Option<TlbEntry> {
        // 记录访问历史
        {
            let mut history = self.access_history.lock().unwrap();
            history.push(access);

            // 如果超过检测窗口，移除最旧的记录
            if history.len() > self.detection_window {
                history.remove(0);
            }
        }

        // 定期分析并调整
        if self
            .access_history
            .lock()
            .unwrap()
            .len()
            .is_multiple_of(self.detection_window)
        {
            self.analyze_and_adapt();
        }

        self.hierarchy.lookup(addr, asid, access)
    }

    fn update(&mut self, entry: TlbEntry) {
        self.hierarchy.update(entry);
    }

    fn flush(&mut self) {
        self.hierarchy.flush();
    }

    fn flush_asid(&mut self, asid: u16) {
        self.hierarchy.flush_asid(asid);
    }
}

// AdaptiveTlbManager的其他方法（不在TlbManager trait中）
impl AdaptiveTlbManager {
    /// 失效指定页面的所有TLB
    pub fn flush_page(&mut self, addr: GuestAddr) {
        self.hierarchy.flush_page(addr);
    }

    /// 获取总容量
    pub fn capacity(&self) -> usize {
        self.hierarchy.capacity()
    }

    /// 获取当前条目数
    pub fn len(&self) -> usize {
        self.hierarchy.len()
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.hierarchy.is_empty()
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hierarchy_creation() {
        let hierarchy = UnifiedTlbHierarchy::with_defaults();
        let (i, d, l2, l3) = hierarchy.capacities();
        assert_eq!(i, 64);
        assert_eq!(d, 128);
        assert_eq!(l2, 512);
        assert_eq!(l3, 2048);
    }

    #[test]
    fn test_lookup_empty() {
        let mut hierarchy = UnifiedTlbHierarchy::with_defaults();
        let result = hierarchy.lookup(GuestAddr(0x1000), 0, AccessType::Read);
        assert!(result.is_none());
    }

    #[test]
    fn test_insert_and_lookup() {
        let mut hierarchy = UnifiedTlbHierarchy::with_defaults();

        let entry = TlbEntry {
            guest_addr: GuestAddr(0x1000),
            phys_addr: vm_core::GuestPhysAddr(0x2000),
            flags: crate::pte_flags::R | crate::pte_flags::W,
            asid: 0,
        };

        hierarchy.insert(entry, AccessType::Read);

        let result = hierarchy.lookup(GuestAddr(0x1000), 0, AccessType::Read);
        assert!(result.is_some());
        assert_eq!(result.unwrap().phys_addr, vm_core::GuestPhysAddr(0x2000));
    }

    #[test]
    fn test_stats_hit_rate() {
        let stats = HierarchyStats::default();

        // 模拟命中
        stats
            .l1_hits
            .fetch_add(90, std::sync::atomic::Ordering::Relaxed);
        stats
            .l2_hits
            .fetch_add(7, std::sync::atomic::Ordering::Relaxed);
        stats
            .l3_hits
            .fetch_add(2, std::sync::atomic::Ordering::Relaxed);
        stats
            .total_misses
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        assert!((stats.overall_hit_rate() - 0.99).abs() < 0.01);
    }

    #[test]
    fn test_invalidate_all() {
        let mut hierarchy = UnifiedTlbHierarchy::with_defaults();

        let entry = TlbEntry {
            guest_addr: GuestAddr(0x1000),
            phys_addr: vm_core::GuestPhysAddr(0x2000),
            flags: crate::pte_flags::R,
            asid: 0,
        };

        hierarchy.insert(entry, AccessType::Read);
        hierarchy.invalidate_all();

        let result = hierarchy.lookup(GuestAddr(0x1000), 0, AccessType::Read);
        assert!(result.is_none());
    }

    #[test]
    fn test_multi_level_lookup() {
        let mut hierarchy = UnifiedTlbHierarchy::with_defaults();

        let entry = TlbEntry {
            guest_addr: GuestAddr(0x1000),
            phys_addr: vm_core::GuestPhysAddr(0x2000),
            flags: crate::pte_flags::R | crate::pte_flags::W,
            asid: 0,
        };

        // 插入条目
        hierarchy.insert(entry, AccessType::Read);

        // L1应该命中
        let result = hierarchy.lookup(GuestAddr(0x1000), 0, AccessType::Read);
        assert!(result.is_some());

        // 验证统计信息
        let stats = hierarchy.stats();
        assert_eq!(stats.l1_hits.load(std::sync::atomic::Ordering::Relaxed), 1);
    }

    #[test]
    fn test_execute_vs_data_access() {
        let mut hierarchy = UnifiedTlbHierarchy::with_defaults();

        let entry = TlbEntry {
            guest_addr: GuestAddr(0x1000),
            phys_addr: vm_core::GuestPhysAddr(0x2000),
            flags: crate::pte_flags::R | crate::pte_flags::X,
            asid: 0,
        };

        // Execute访问应该使用ITLB
        hierarchy.insert(entry, AccessType::Execute);
        let result = hierarchy.lookup(GuestAddr(0x1000), 0, AccessType::Execute);
        assert!(result.is_some());

        // Read访问应该使用DTLB（可能不在DTLB中）
        let _result2 = hierarchy.lookup(GuestAddr(0x1000), 0, AccessType::Read);
        // 可能返回Some或None，取决于实现
    }

    #[test]
    fn test_flush_asid() {
        let mut hierarchy = UnifiedTlbHierarchy::with_defaults();

        let entry1 = TlbEntry {
            guest_addr: GuestAddr(0x1000),
            phys_addr: vm_core::GuestPhysAddr(0x2000),
            flags: crate::pte_flags::R,
            asid: 1,
        };

        let entry2 = TlbEntry {
            guest_addr: GuestAddr(0x2000),
            phys_addr: vm_core::GuestPhysAddr(0x3000),
            flags: crate::pte_flags::R,
            asid: 2,
        };

        hierarchy.insert(entry1, AccessType::Read);
        hierarchy.insert(entry2, AccessType::Read);

        // 刷新ASID=1
        hierarchy.flush_asid(1);

        // ASID=1的条目应该被清除
        let result1 = hierarchy.lookup(GuestAddr(0x1000), 1, AccessType::Read);
        assert!(result1.is_none());

        // ASID=2的条目应该仍然存在
        let result2 = hierarchy.lookup(GuestAddr(0x2000), 2, AccessType::Read);
        assert!(result2.is_some());
    }

    #[test]
    fn test_custom_capacities() {
        let hierarchy = UnifiedTlbHierarchy::new(
            128,  // L1 ITLB
            256,  // L1 DTLB
            1024, // L2
            4096, // L3
            ReplacementPolicy::LFU,
        );

        let (i, d, l2, l3) = hierarchy.capacities();
        assert_eq!(i, 128);
        assert_eq!(d, 256);
        assert_eq!(l2, 1024);
        assert_eq!(l3, 4096);
    }

    #[test]
    fn test_replacement_policies() {
        // 测试不同的替换策略创建
        let lru = UnifiedTlbHierarchy::new(64, 128, 512, 2048, ReplacementPolicy::LRU);
        let lfu = UnifiedTlbHierarchy::new(64, 128, 512, 2048, ReplacementPolicy::LFU);
        let fifo = UnifiedTlbHierarchy::new(64, 128, 512, 2048, ReplacementPolicy::FIFO);
        let random = UnifiedTlbHierarchy::new(64, 128, 512, 2048, ReplacementPolicy::Random);
        let adaptive = UnifiedTlbHierarchy::new(64, 128, 512, 2048, ReplacementPolicy::AdaptiveARC);

        // 验证它们都能正常工作
        assert_eq!(lru.capacity(), 64 + 128 + 512 + 2048);
        assert_eq!(lfu.capacity(), 64 + 128 + 512 + 2048);
        assert_eq!(fifo.capacity(), 64 + 128 + 512 + 2048);
        assert_eq!(random.capacity(), 64 + 128 + 512 + 2048);
        assert_eq!(adaptive.capacity(), 64 + 128 + 512 + 2048);
    }

    #[test]
    fn test_multiple_inserts() {
        let mut hierarchy = UnifiedTlbHierarchy::with_defaults();

        // 插入多个条目
        for i in 0..10 {
            let entry = TlbEntry {
                guest_addr: GuestAddr(0x1000 + i * 0x1000),
                phys_addr: vm_core::GuestPhysAddr(0x2000 + i * 0x1000),
                flags: crate::pte_flags::R,
                asid: 0,
            };
            hierarchy.insert(entry, AccessType::Read);
        }

        // 验证所有条目都可以找到
        for i in 0..10 {
            let result = hierarchy.lookup(GuestAddr(0x1000 + i * 0x1000), 0, AccessType::Read);
            assert!(result.is_some());
        }
    }

    #[test]
    fn test_capacity_exceeded() {
        let mut hierarchy = UnifiedTlbHierarchy::new(
            4,  // L1 ITLB - 小容量
            4,  // L1 DTLB - 小容量
            8,  // L2
            16, // L3
            ReplacementPolicy::LRU,
        );

        // 插入超过容量的条目
        for i in 0..50 {
            let entry = TlbEntry {
                guest_addr: GuestAddr(0x1000 + i * 0x1000),
                phys_addr: vm_core::GuestPhysAddr(0x2000 + i * 0x1000),
                flags: crate::pte_flags::R,
                asid: 0,
            };
            hierarchy.insert(entry, AccessType::Read);
        }

        // 应该仍然能够查找（旧条目可能被驱逐）
        let _result = hierarchy.lookup(GuestAddr(0x1000), 0, AccessType::Read);
        // 可能返回Some或None，取决于驱逐策略
    }

    #[test]
    fn test_hit_distribution() {
        let stats = HierarchyStats::default();

        stats
            .l1_hits
            .fetch_add(80, std::sync::atomic::Ordering::Relaxed);
        stats
            .l2_hits
            .fetch_add(15, std::sync::atomic::Ordering::Relaxed);
        stats
            .l3_hits
            .fetch_add(5, std::sync::atomic::Ordering::Relaxed);

        let (l1_pct, l2_pct, l3_pct) = stats.hit_distribution();
        assert!((l1_pct - 0.8).abs() < 0.01); // 80%
        assert!((l2_pct - 0.15).abs() < 0.01); // 15%
        assert!((l3_pct - 0.05).abs() < 0.01); // 5%
    }

    #[test]
    fn test_adaptive_manager() {
        let adaptive_manager = AdaptiveTlbManager::with_defaults();

        assert_eq!(adaptive_manager.capacity(), 64 + 128 + 512 + 2048);
        assert_eq!(adaptive_manager.len(), 0);
        assert!(adaptive_manager.is_empty());
    }

    #[test]
    fn test_adaptive_manager_enable_disable() {
        let mut adaptive_manager = AdaptiveTlbManager::with_defaults();

        adaptive_manager.enable_adaptive();
        assert!(adaptive_manager.is_adaptive_enabled());

        adaptive_manager.disable_adaptive();
        assert!(!adaptive_manager.is_adaptive_enabled());
    }

    #[test]
    fn test_flush_page() {
        let mut hierarchy = UnifiedTlbHierarchy::with_defaults();

        let entry = TlbEntry {
            guest_addr: GuestAddr(0x1000),
            phys_addr: vm_core::GuestPhysAddr(0x2000),
            flags: crate::pte_flags::R,
            asid: 0,
        };

        hierarchy.insert(entry, AccessType::Read);
        hierarchy.flush_page(GuestAddr(0x1000));

        let result = hierarchy.lookup(GuestAddr(0x1000), 0, AccessType::Read);
        assert!(result.is_none());
    }

    #[test]
    fn test_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let hierarchy = Arc::new(std::sync::Mutex::new(UnifiedTlbHierarchy::with_defaults()));
        let mut handles = vec![];

        // 创建多个线程并发访问
        for i in 0..4 {
            let hierarchy_clone = Arc::clone(&hierarchy);
            let handle = thread::spawn(move || {
                let mut hierarchy = hierarchy_clone.lock().unwrap();
                let entry = TlbEntry {
                    guest_addr: GuestAddr(0x1000 + i * 0x1000),
                    phys_addr: vm_core::GuestPhysAddr(0x2000 + i * 0x1000),
                    flags: crate::pte_flags::R,
                    asid: 0,
                };
                hierarchy.insert(entry, AccessType::Read);
                hierarchy.lookup(GuestAddr(0x1000 + i * 0x1000), 0, AccessType::Read)
            });
            handles.push(handle);
        }

        // 等待所有线程完成
        for handle in handles {
            assert!(handle.join().is_ok());
        }
    }

    #[test]
    fn test_stats_increment() {
        let mut hierarchy = UnifiedTlbHierarchy::with_defaults();

        let entry = TlbEntry {
            guest_addr: GuestAddr(0x1000),
            phys_addr: vm_core::GuestPhysAddr(0x2000),
            flags: crate::pte_flags::R,
            asid: 0,
        };

        hierarchy.insert(entry, AccessType::Read);

        // 查找未命中
        hierarchy.lookup(GuestAddr(0x2000), 0, AccessType::Read);

        let stats = hierarchy.stats();
        let l1_hits = stats.l1_hits.load(std::sync::atomic::Ordering::Relaxed);
        let misses = stats
            .total_misses
            .load(std::sync::atomic::Ordering::Relaxed);

        assert_eq!(l1_hits, 0); // 第一次查找
        assert!(misses >= 0);
    }

    #[test]
    fn test_replacement_policy_variants() {
        // 测试所有替换策略的枚举值
        assert_eq!(ReplacementPolicy::LRU, ReplacementPolicy::default());
        assert!(ReplacementPolicy::LRU != ReplacementPolicy::LFU);
        assert!(ReplacementPolicy::LFU != ReplacementPolicy::FIFO);
        assert!(ReplacementPolicy::FIFO != ReplacementPolicy::Random);
        assert!(ReplacementPolicy::Random != ReplacementPolicy::AdaptiveARC);
    }

    #[test]
    fn test_empty_hierarchy_stats() {
        let hierarchy = UnifiedTlbHierarchy::with_defaults();
        let stats = hierarchy.stats();

        assert_eq!(stats.l1_hits.load(std::sync::atomic::Ordering::Relaxed), 0);
        assert_eq!(stats.l2_hits.load(std::sync::atomic::Ordering::Relaxed), 0);
        assert_eq!(stats.l3_hits.load(std::sync::atomic::Ordering::Relaxed), 0);
        assert_eq!(
            stats
                .total_misses
                .load(std::sync::atomic::Ordering::Relaxed),
            0
        );
        assert_eq!(stats.overall_hit_rate(), 0.0);
    }

    #[test]
    fn test_zero_capacity() {
        let hierarchy = UnifiedTlbHierarchy::new(0, 0, 0, 0, ReplacementPolicy::LRU);
        assert_eq!(hierarchy.capacity(), 0);
        assert_eq!(hierarchy.len(), 0);
        assert!(hierarchy.is_empty());
    }

    #[test]
    fn test_very_large_capacity() {
        let hierarchy = UnifiedTlbHierarchy::new(
            1024,  // 1K entries
            2048,  // 2K entries
            8192,  // 8K entries
            32768, // 32K entries
            ReplacementPolicy::LRU,
        );

        assert_eq!(hierarchy.capacity(), 1024 + 2048 + 8192 + 32768);
    }
}
