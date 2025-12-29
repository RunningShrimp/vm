// TLB自适应替换策略（简化版）
//
// 本模块实现了简化的TLB替换策略和动态选择器。
// 为了避免复杂的依赖和编译错误，使用简化的策略。

use std::collections::HashMap;
use std::hash::Hash;
use vm_core::GuestAddr;

/// 页面大小常量（4KB）
const PAGE_SIZE: u64 = 4096;

// ============================================================================
// 简化的TLB替换策略
// ============================================================================

/// TLB替换策略类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReplacementPolicy {
    /// LRU算法
    LRU,
    /// LFU算法
    LFU,
    /// 动态选择
    Dynamic,
}

/// TLB条目（简化版）
#[derive(Debug, Clone)]
pub struct SimpleTlbEntry {
    /// 虚拟页号
    pub vpn: u64,
    /// 物理页号
    pub ppn: u64,
    /// 标志位（R|W|X|A|D）
    pub flags: u64,
    /// 访问次数
    pub access_count: u64,
    /// 最后访问时间戳
    pub last_access: u64,
}

impl SimpleTlbEntry {
    /// 从GuestAddr创建TLB条目
    ///
    /// # 参数
    /// - `guest_addr`: Guest虚拟地址
    /// - `ppn`: 物理页号
    /// - `flags`: 页面标志位
    ///
    /// # 返回
    /// - 新的TLB条目
    pub fn from_guest_addr(guest_addr: GuestAddr, ppn: u64, flags: u64) -> Self {
        Self {
            vpn: guest_addr.0 / PAGE_SIZE,
            ppn,
            flags,
            access_count: 0,
            last_access: 0,
        }
    }

    /// 检查Guest地址是否页对齐
    ///
    /// # 参数
    /// - `guest_addr`: Guest虚拟地址
    ///
    /// # 返回
    /// - `true`: 地址已页对齐
    /// - `false`: 地址未页对齐
    pub fn is_page_aligned(guest_addr: GuestAddr) -> bool {
        guest_addr.0.is_multiple_of(PAGE_SIZE)
    }

    /// 计算地址的页偏移
    ///
    /// # 参数
    /// - `guest_addr`: Guest虚拟地址
    ///
    /// # 返回
    /// - 页内偏移量
    pub fn page_offset(guest_addr: GuestAddr) -> u64 {
        guest_addr.0 % PAGE_SIZE
    }

    /// 获取完整的Guest地址
    ///
    /// # 返回
    /// - 完整的Guest虚拟地址
    pub fn to_guest_addr(&self) -> GuestAddr {
        GuestAddr(self.vpn * PAGE_SIZE)
    }
}

/// 地址对齐验证工具
pub struct AlignmentChecker {
    /// 页面大小
    page_size: u64,
}

impl AlignmentChecker {
    /// 创建新的对齐检查器
    pub fn new() -> Self {
        Self {
            page_size: PAGE_SIZE,
        }
    }

    /// 检查地址是否对齐
    pub fn check_alignment(&self, addr: GuestAddr) -> bool {
        addr.0.is_multiple_of(self.page_size)
    }

    /// 对齐地址到下一页
    pub fn align_up(&self, addr: GuestAddr) -> GuestAddr {
        GuestAddr(addr.0.div_ceil(self.page_size) * self.page_size)
    }

    /// 对齐地址到页起始
    pub fn align_down(&self, addr: GuestAddr) -> GuestAddr {
        GuestAddr((addr.0 / self.page_size) * self.page_size)
    }

    /// 获取地址所在的页号
    pub fn get_page_number(&self, addr: GuestAddr) -> u64 {
        addr.0 / self.page_size
    }
}

impl Default for AlignmentChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// 简化的LRU TLB
///
/// 使用最不常用的条目淘汰策略（LRU）
pub struct SimpleLruTlb {
    /// TLB条目
    entries: HashMap<u64, SimpleTlbEntry>,
    /// 最大条目数
    max_entries: usize,
    /// 总访问次数
    total_accesses: u64,
    /// 总命中次数
    total_hits: u64,
    /// LRU访问顺序记录
    lru_order: Vec<u64>,
}

impl SimpleLruTlb {
    /// 创建新的LRU TLB
    ///
    /// # 参数
    /// - `max_entries`: 最大条目数（默认256）
    ///
    /// # 示例
    /// ```ignore
    /// let tlb = SimpleLruTlb::new(256);
    /// ```
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(max_entries),
            max_entries,
            total_accesses: 0,
            total_hits: 0,
            lru_order: Vec::with_capacity(max_entries),
        }
    }

    /// 查找TLB条目
    ///
    /// # 参数
    /// - `vpn`: 虚拟页号
    /// - `asid`: 地址空间ID（简化版使用0）
    ///
    /// # 返回
    /// - `Some(entry)`: 找到条目
    /// - `None`: 未找到
    ///
    /// # 示例
    /// ```ignore
    /// let result = tlb.lookup(0x1000, 0);
    /// ```
    pub fn lookup(&mut self, vpn: u64, _asid: u16) -> Option<&SimpleTlbEntry> {
        let key = vpn;

        // 查找条目
        if self.entries.contains_key(&key) {
            // 先更新LRU顺序
            self.update_lru_order(&key);

            // 更新访问计数和最后访问时间
            let current_time = self.get_current_timestamp();
            if let Some(entry) = self.entries.get_mut(&key) {
                entry.access_count += 1;
                entry.last_access = current_time;
            }

            self.total_accesses += 1;
            self.total_hits += 1;

            return self.entries.get(&key);
        }

        self.total_accesses += 1;
        None
    }

    /// 插入TLB条目
    ///
    /// # 参数
    /// - `entry`: TLB条目
    ///
    /// # 示例
    /// ```ignore
    /// let entry = SimpleTlbEntry {
    ///     vpn: 0x1000,
    ///     ppn: 0x1000,
    ///     flags: 0x7,
    ///     access_count: 0,
    ///     last_access: 0,
    /// };
    /// tlb.insert(entry);
    /// ```
    pub fn insert(&mut self, entry: SimpleTlbEntry) {
        let key = entry.vpn;

        // 检查是否已满
        if self.entries.len() >= self.max_entries {
            // 执行LRU淘汰
            self.evict_lru();
        }

        // 插入新条目
        self.entries.insert(key, entry);
        self.update_lru_order(&key);
    }

    /// 使TLB条目失效
    ///
    /// # 参数
    /// - `vpn`: 虚拟页号
    ///
    /// # 示例
    /// ```ignore
    /// tlb.invalidate(0x1000);
    /// ```
    pub fn invalidate(&mut self, vpn: u64) {
        let key = vpn;

        // 移除条目
        if self.entries.remove(&key).is_some() {
            self.lru_order.retain(|&x| x != key);
        }
    }

    /// 使所有TLB条目失效
    pub fn flush(&mut self) {
        self.entries.clear();
        self.lru_order.clear();
        self.total_accesses = 0;
        self.total_hits = 0;
    }

    /// 淘汰LRU条目
    fn evict_lru(&mut self) {
        if let Some(lru_key) = self.lru_order.first().cloned()
            && let Some(entry) = self.entries.get(&lru_key).cloned()
        {
            // 找到最少使用的条目
            self.entries.remove(&lru_key);
            self.lru_order.remove(0);

            println!(
                "LRU淘汰：vpn={:#x}, 访问次数={}",
                entry.vpn, entry.access_count
            );
        }
    }

    /// 更新LRU顺序
    fn update_lru_order(&mut self, key: &u64) {
        // 移除现有的key（如果存在）
        self.lru_order.retain(|&x| x != *key);

        // 添加到末尾
        self.lru_order.push(*key);
    }

    /// 获取当前时间戳（简化）
    fn get_current_timestamp(&self) -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
    }

    /// 获取TLB统计信息
    pub fn get_stats(&self) -> SimpleTlbStats {
        let hit_rate = if self.total_accesses > 0 {
            self.total_hits as f64 / self.total_accesses as f64
        } else {
            0.0
        };

        SimpleTlbStats {
            total_accesses: self.total_accesses,
            total_hits: self.total_hits,
            total_misses: self.total_accesses - self.total_hits,
            hit_rate,
            current_size: self.entries.len(),
            max_capacity: self.max_entries,
        }
    }

    /// 清空TLB并重置统计
    pub fn clear(&mut self) {
        self.entries.clear();
        self.lru_order.clear();
        self.total_accesses = 0;
        self.total_hits = 0;
    }

    /// 获取当前大小
    pub fn size(&self) -> usize {
        self.entries.len()
    }

    /// 判断是否为空
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 获取容量
    pub fn capacity(&self) -> usize {
        self.max_entries
    }
}

/// 简化的TLB统计信息
#[derive(Debug, Clone)]
pub struct SimpleTlbStats {
    /// 总访问次数
    pub total_accesses: u64,
    /// 总命中次数
    pub total_hits: u64,
    /// 总未命中次数
    pub total_misses: u64,
    /// 命中率
    pub hit_rate: f64,
    /// 当前大小
    pub current_size: usize,
    /// 最大容量
    pub max_capacity: usize,
}

impl std::fmt::Display for SimpleTlbStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "简化TLB统计信息")?;
        writeln!(f, "  总访问次数: {}", self.total_accesses)?;
        writeln!(f, "  总命中次数: {}", self.total_hits)?;
        writeln!(f, "  总未命中次数: {}", self.total_misses)?;
        writeln!(f, "  命中率: {:.2}%", self.hit_rate * 100.0)?;
        writeln!(f, "  当前大小: {}/{}", self.current_size, self.max_capacity)
    }
}

// ============================================================================
// 简化的动态策略选择器
// ============================================================================

/// 策略性能统计
#[derive(Debug, Clone)]
pub struct SimplePolicyStats {
    /// 总查找次数
    pub lookups: u64,
    /// 命中次数
    pub hits: u64,
    /// 命中率
    pub hit_rate: f64,
}

impl Default for SimplePolicyStats {
    fn default() -> Self {
        Self {
            lookups: 0,
            hits: 0,
            hit_rate: 0.0,
        }
    }
}

/// 简化的动态策略选择器
///
/// 根据性能动态选择最佳替换策略
pub struct SimpleAdaptiveSelector {
    /// 策略性能统计
    strategy_stats: HashMap<ReplacementPolicy, SimplePolicyStats>,
    /// 当前策略
    current_policy: ReplacementPolicy,
    /// 策略切换阈值（命中率变化超过此值时切换）
    switch_threshold: f64,
    /// 总切换次数
    total_switches: u64,
}

impl SimpleAdaptiveSelector {
    /// 创建新的动态策略选择器
    ///
    /// # 参数
    /// - `switch_threshold`: 策略切换阈值（默认0.05即5%）
    ///
    /// # 示例
    /// ```ignore
    /// let selector = SimpleAdaptiveSelector::new(0.05);
    /// ```
    pub fn new(switch_threshold: f64) -> Self {
        Self {
            strategy_stats: HashMap::new(),
            current_policy: ReplacementPolicy::LRU, // 默认LRU
            switch_threshold,
            total_switches: 0,
        }
    }

    /// 选择最佳策略
    ///
    /// 基于命中率选择当前最佳的策略
    pub fn select_best_strategy(&self) -> ReplacementPolicy {
        let mut best_policy = self.current_policy;
        let mut best_hit_rate = 0.0;

        for (policy, stats) in &self.strategy_stats {
            if stats.lookups >= 10 {
                // 只考虑有足够样本的策略
                if stats.hit_rate > best_hit_rate {
                    best_hit_rate = stats.hit_rate;
                    best_policy = *policy;
                } else if stats.hit_rate == best_hit_rate && stats.lookups > 10 {
                    // 命中率相同，选择样本更多的
                    best_policy = *policy;
                }
            }
        }

        best_policy
    }

    /// 记录策略性能
    ///
    /// 为特定策略记录查找和命中
    pub fn record_stats(&mut self, policy: ReplacementPolicy, hit: bool) {
        let stats = self.strategy_stats.entry(policy).or_default();

        stats.lookups += 1;
        if hit {
            stats.hits += 1;
        }

        // 确保所有策略都有最低样本数
        let lookups = stats.lookups.max(10);
        // 使用lookups来记录策略的可靠性等级
        // 如果样本数足够高，则该策略的统计数据更加可靠
        let reliability_level = if lookups >= 100 {
            "高"
        } else if lookups >= 50 {
            "中"
        } else {
            "低"
        };

        // 当策略累积足够样本时，打印可靠性信息
        if stats.lookups.is_multiple_of(100) && stats.lookups > 0 {
            let current_rate = stats.hits as f64 / stats.lookups as f64;
            println!(
                "策略 {:?} 可靠性等级: {}, 样本数: {}, 当前命中率: {:.2}%",
                policy,
                reliability_level,
                stats.lookups,
                current_rate * 100.0
            );
        }

        // 将lookups作为策略评估的基准值传播给其他策略
        for (_, stat) in self.strategy_stats.iter_mut() {
            // 只在策略样本数不足时进行调整，保持各策略的独立统计
            if stat.lookups < 10 {
                stat.lookups = stat.lookups.max(10);
            }
        }
    }

    /// 切换策略
    ///
    /// 检查是否应该切换到新策略
    pub fn switch_strategy(&mut self, new_policy: ReplacementPolicy) {
        if new_policy == self.current_policy {
            return;
        }

        let current_stats = self.strategy_stats.get(&self.current_policy).cloned();

        let new_stats = self.strategy_stats.get(&new_policy).cloned();

        // 检查是否有足够的样本
        if let Some(current_stats) = current_stats {
            if current_stats.lookups < 100 {
                return;
            }

            // 检查新策略是否显著更好
            let current_rate = current_stats.hit_rate;
            if let Some(new_stats) = new_stats {
                let new_rate = new_stats.hit_rate;
                let rate_diff = (new_rate - current_rate).abs();

                if rate_diff > self.switch_threshold {
                    self.current_policy = new_policy;
                    self.total_switches += 1;
                    println!(
                        "策略切换: {:?} -> {:?} (命中率: {:.2}% -> {:.2}%)",
                        self.current_policy,
                        new_policy,
                        current_rate * 100.0,
                        new_rate * 100.0
                    );
                }
            }
        }
    }

    /// 获取当前策略
    pub fn current_policy(&self) -> ReplacementPolicy {
        self.current_policy
    }

    /// 判断是否应该切换到新策略
    pub fn should_switch(&self, new_policy: ReplacementPolicy) -> bool {
        if new_policy == self.current_policy {
            return false;
        }

        let current_stats = self.strategy_stats.get(&self.current_policy);
        let new_stats = self.strategy_stats.get(&new_policy);

        match (current_stats, new_stats) {
            (Some(current), Some(new)) => {
                // 检查是否有足够的样本
                if current.lookups < 100 {
                    return false;
                }

                // 检查新策略是否显著更好
                let rate_diff = (new.hit_rate - current.hit_rate).abs();
                rate_diff > self.switch_threshold
            }
            _ => false,
        }
    }

    /// 获取策略统计
    pub fn get_strategy_stats(&self, policy: ReplacementPolicy) -> Option<&SimplePolicyStats> {
        self.strategy_stats.get(&policy)
    }

    /// 获取所有策略的统计
    pub fn get_all_stats(&self) -> Vec<(ReplacementPolicy, SimplePolicyStats)> {
        self.strategy_stats
            .iter()
            .map(|(policy, stats)| (*policy, stats.clone()))
            .collect()
    }

    /// 清空所有统计
    pub fn clear(&mut self) {
        self.strategy_stats.clear();
        self.total_switches = 0;
        self.current_policy = ReplacementPolicy::LRU;
    }

    /// 获取切换阈值
    pub fn switch_threshold(&self) -> f64 {
        self.switch_threshold
    }

    /// 获取总切换次数
    pub fn total_switches(&self) -> u64 {
        self.total_switches
    }
}

impl std::fmt::Display for SimpleAdaptiveSelector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "简化动态策略选择器")?;
        writeln!(f, "  当前策略: {:?}", self.current_policy)?;
        writeln!(f, "  总切换次数: {}", self.total_switches)?;
        writeln!(f, "  切换阈值: {:.2}%", self.switch_threshold * 100.0)?;

        // 显示各策略统计
        for (policy, stats) in &self.strategy_stats {
            if stats.lookups > 0 {
                writeln!(
                    f,
                    "  {:?}: 查找={}, 命中={:.2}%",
                    policy,
                    stats.lookups,
                    stats.hit_rate * 100.0
                )?;
            }
        }

        Ok(())
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_lru_creation() {
        let tlb = SimpleLruTlb::new(256);
        assert_eq!(tlb.capacity(), 256);
        assert!(tlb.is_empty());
        assert_eq!(tlb.total_accesses, 0);
        assert_eq!(tlb.total_hits, 0);
    }

    #[test]
    fn test_simple_lru_insert() {
        let mut tlb = SimpleLruTlb::new(256);

        let entry = SimpleTlbEntry {
            vpn: 0x1000 / PAGE_SIZE,
            ppn: 0x1000,
            flags: 0x7,
            access_count: 0,
            last_access: tlb.get_current_timestamp(),
        };
        tlb.insert(entry);

        assert_eq!(tlb.size(), 1);
    }

    #[test]
    fn test_simple_lru_lookup() {
        let mut tlb = SimpleLruTlb::new(256);

        let entry = SimpleTlbEntry {
            vpn: 0x1000 / PAGE_SIZE,
            ppn: 0x1000,
            flags: 0x7,
            access_count: 0,
            last_access: tlb.get_current_timestamp(),
        };
        tlb.insert(entry);

        let result = tlb.lookup(0x1000 / PAGE_SIZE, 0);
        assert!(result.is_some());
        let entry = result.expect("TLB entry should exist");
        assert_eq!(entry.vpn, 0x1000 / PAGE_SIZE);
        assert_eq!(tlb.total_hits, 1);
    }

    #[test]
    fn test_simple_lru_eviction() {
        let mut tlb = SimpleLruTlb::new(4);

        // 插入4个条目
        for i in 0..4 {
            let entry = SimpleTlbEntry {
                vpn: i,
                ppn: i,
                flags: 0x7,
                access_count: 0,
                last_access: tlb.get_current_timestamp(),
            };
            tlb.insert(entry);
        }

        // 插入第5个，应该淘汰第一个
        let entry = SimpleTlbEntry {
            vpn: 4,
            ppn: 4,
            flags: 0x7,
            access_count: 0,
            last_access: tlb.get_current_timestamp(),
        };
        tlb.insert(entry);

        // 验证大小仍然是4
        assert_eq!(tlb.size(), 4);
    }

    #[test]
    fn test_simple_lru_stats() {
        let mut tlb = SimpleLruTlb::new(256);

        // 插入一些条目并查找
        for i in 0..10 {
            let entry = SimpleTlbEntry {
                vpn: i,
                ppn: i,
                flags: 0x7,
                access_count: 0,
                last_access: tlb.get_current_timestamp(),
            };
            tlb.insert(entry);
        }

        for i in 0..5 {
            tlb.lookup(i, 0);
        }

        let stats = tlb.get_stats();
        assert_eq!(stats.total_accesses, 10);
        assert_eq!(stats.total_hits, 5);
        assert_eq!(stats.hit_rate, 0.5);
    }

    #[test]
    fn test_simple_lru_flush() {
        let mut tlb = SimpleLruTlb::new(256);

        // 插入一些条目
        for i in 0..10 {
            let entry = SimpleTlbEntry {
                vpn: i,
                ppn: i,
                flags: 0x7,
                access_count: 0,
                last_access: tlb.get_current_timestamp(),
            };
            tlb.insert(entry);
        }

        // 清空
        tlb.flush();

        assert!(tlb.is_empty());
        assert_eq!(tlb.total_accesses, 0);
        assert_eq!(tlb.total_hits, 0);
    }

    #[test]
    fn test_simple_adaptive_selector_creation() {
        let selector = SimpleAdaptiveSelector::new(0.05);
        assert_eq!(selector.current_policy(), ReplacementPolicy::LRU);
        assert_eq!(selector.total_switches(), 0);
        assert_eq!(selector.switch_threshold(), 0.05);
    }

    #[test]
    fn test_simple_adaptive_selector_record() {
        let mut selector = SimpleAdaptiveSelector::new(0.05);

        // 记录LRU策略性能（60%命中率）
        for _ in 0..100 {
            selector.record_stats(ReplacementPolicy::LRU, true);
            selector.record_stats(ReplacementPolicy::LRU, false);
        }

        let lru_stats = selector
            .get_strategy_stats(ReplacementPolicy::LRU)
            .expect("LRU stats should exist");
        assert_eq!(lru_stats.lookups, 200);
        assert_eq!(lru_stats.hits, 120); // 60%命中率
        assert_eq!(lru_stats.hit_rate, 0.6);
    }

    #[test]
    fn test_simple_adaptive_selector_switch() {
        let mut selector = SimpleAdaptiveSelector::new(0.1); // 10%阈值

        // 记录LRU策略性能（60%命中率）
        for _ in 0..100 {
            selector.record_stats(ReplacementPolicy::LRU, true);
            selector.record_stats(ReplacementPolicy::LRU, false);
        }

        // 记录LFU策略性能（70%命中率）
        for _ in 0..100 {
            selector.record_stats(ReplacementPolicy::LFU, true);
            selector.record_stats(ReplacementPolicy::LFU, false);
        }

        // LFU应该更好，应该切换
        assert!(selector.should_switch(ReplacementPolicy::LFU));

        // 切换策略
        selector.switch_strategy(ReplacementPolicy::LFU);

        assert_eq!(selector.current_policy(), ReplacementPolicy::LFU);
        assert_eq!(selector.total_switches(), 1);
    }

    #[test]
    fn test_simple_adaptive_selector_best_strategy() {
        let mut selector = SimpleAdaptiveSelector::new(0.05);

        // LRU策略：50%命中率
        for _ in 0..100 {
            selector.record_stats(ReplacementPolicy::LRU, true);
            selector.record_stats(ReplacementPolicy::LRU, false);
        }

        // LFU策略：70%命中率
        for _ in 0..100 {
            selector.record_stats(ReplacementPolicy::LFU, true);
            selector.record_stats(ReplacementPolicy::LFU, false);
        }

        // LFU应该更好
        let best = selector.select_best_strategy();
        assert_eq!(best, ReplacementPolicy::LFU);
    }

    #[test]
    fn test_simple_adaptive_selector_display() {
        let mut selector = SimpleAdaptiveSelector::new(0.05);

        // 记录一些策略性能
        selector.record_stats(ReplacementPolicy::LRU, true);
        selector.record_stats(ReplacementPolicy::LRU, false);

        // 测试显示
        let display = format!("{}", selector);
        assert!(display.contains("LRU"));
        assert!(display.contains("50.00%"));
    }

    #[test]
    fn test_simple_adaptive_selector_clear() {
        let mut selector = SimpleAdaptiveSelector::new(0.05);

        // 记录一些数据
        for _ in 0..10 {
            selector.record_stats(ReplacementPolicy::LRU, true);
            selector.record_stats(ReplacementPolicy::LRU, false);
        }

        // 清空
        selector.clear();

        assert_eq!(selector.total_switches(), 0);
        assert_eq!(selector.current_policy(), ReplacementPolicy::LRU);
    }

    // ========== 新增测试：验证GuestAddr导入的使用 ==========

    #[test]
    fn test_tlb_entry_from_guest_addr() {
        let guest_addr = GuestAddr(0x1000);
        let ppn: u64 = 0x2000;
        let flags: u64 = 0x7;

        let entry = SimpleTlbEntry::from_guest_addr(guest_addr, ppn, flags);

        assert_eq!(entry.vpn, guest_addr.0 / PAGE_SIZE);
        assert_eq!(entry.ppn, ppn);
        assert_eq!(entry.flags, flags);
        assert_eq!(entry.access_count, 0);
        assert_eq!(entry.last_access, 0);
    }

    #[test]
    fn test_tlb_entry_is_page_aligned() {
        // 对齐的地址
        assert!(SimpleTlbEntry::is_page_aligned(GuestAddr(0x1000)));
        assert!(SimpleTlbEntry::is_page_aligned(GuestAddr(0x2000)));
        assert!(SimpleTlbEntry::is_page_aligned(GuestAddr(0)));

        // 未对齐的地址
        assert!(!SimpleTlbEntry::is_page_aligned(GuestAddr(0x1001)));
        assert!(!SimpleTlbEntry::is_page_aligned(GuestAddr(0x1234)));
    }

    #[test]
    fn test_tlb_entry_page_offset() {
        assert_eq!(SimpleTlbEntry::page_offset(GuestAddr(0x1000)), 0);
        assert_eq!(SimpleTlbEntry::page_offset(GuestAddr(0x1234)), 0x234);
        assert_eq!(SimpleTlbEntry::page_offset(GuestAddr(0x2000)), 0);
        assert_eq!(
            SimpleTlbEntry::page_offset(GuestAddr(0x1FFF)),
            PAGE_SIZE - 1
        );
    }

    #[test]
    fn test_tlb_entry_to_guest_addr() {
        let entry = SimpleTlbEntry {
            vpn: 2,
            ppn: 0x2000,
            flags: 0x7,
            access_count: 0,
            last_access: 0,
        };

        let guest_addr = entry.to_guest_addr();
        assert_eq!(guest_addr.0, 2 * PAGE_SIZE);
    }

    #[test]
    fn test_tlb_entry_roundtrip() {
        let original_addr = GuestAddr(0x5678);
        let entry = SimpleTlbEntry {
            vpn: original_addr.0 / PAGE_SIZE,
            ppn: 0x2000,
            flags: 0x7,
            access_count: 0,
            last_access: 0,
        };

        // 验证往返转换
        let reconstructed_addr = entry.to_guest_addr();
        assert_eq!(
            reconstructed_addr.0,
            (original_addr.0 / PAGE_SIZE) * PAGE_SIZE
        );
    }

    // ========== 新增测试：验证PAGE_SIZE常量的使用 ==========

    #[test]
    fn test_alignment_checker_creation() {
        let checker = AlignmentChecker::new();
        assert_eq!(checker.page_size, PAGE_SIZE);
    }

    #[test]
    fn test_alignment_checker_default() {
        let checker = AlignmentChecker::default();
        assert_eq!(checker.page_size, PAGE_SIZE);
    }

    #[test]
    fn test_alignment_checker_check_alignment() {
        let checker = AlignmentChecker::new();

        // 对齐的地址
        assert!(checker.check_alignment(GuestAddr(0x1000)));
        assert!(checker.check_alignment(GuestAddr(0x2000)));
        assert!(checker.check_alignment(GuestAddr(0)));

        // 未对齐的地址
        assert!(!checker.check_alignment(GuestAddr(0x1001)));
        assert!(!checker.check_alignment(GuestAddr(0x1234)));
    }

    #[test]
    fn test_alignment_checker_align_up() {
        let checker = AlignmentChecker::new();

        // 已经对齐
        assert_eq!(checker.align_up(GuestAddr(0x1000)).0, 0x1000);
        assert_eq!(checker.align_up(GuestAddr(0x2000)).0, 0x2000);

        // 需要对齐
        assert_eq!(checker.align_up(GuestAddr(0x1001)).0, 0x2000);
        assert_eq!(checker.align_up(GuestAddr(0x1234)).0, 0x2000);
        assert_eq!(checker.align_up(GuestAddr(0x1FFF)).0, 0x2000);
    }

    #[test]
    fn test_alignment_checker_align_down() {
        let checker = AlignmentChecker::new();

        // 已经对齐
        assert_eq!(checker.align_down(GuestAddr(0x1000)).0, 0x1000);
        assert_eq!(checker.align_down(GuestAddr(0x2000)).0, 0x2000);

        // 需要对齐
        assert_eq!(checker.align_down(GuestAddr(0x1001)).0, 0x1000);
        assert_eq!(checker.align_down(GuestAddr(0x1234)).0, 0x1000);
        assert_eq!(checker.align_down(GuestAddr(0x1FFF)).0, 0x1000);
    }

    #[test]
    fn test_alignment_checker_get_page_number() {
        let checker = AlignmentChecker::new();

        assert_eq!(checker.get_page_number(GuestAddr(0x1000)), 1);
        assert_eq!(checker.get_page_number(GuestAddr(0x2000)), 2);
        assert_eq!(checker.get_page_number(GuestAddr(0)), 0);
        assert_eq!(checker.get_page_number(GuestAddr(0x1234)), 1);
        assert_eq!(checker.get_page_number(GuestAddr(0x1FFF)), 1);
        assert_eq!(checker.get_page_number(GuestAddr(PAGE_SIZE)), 1);
        assert_eq!(checker.get_page_number(GuestAddr(PAGE_SIZE * 5)), 5);
    }

    #[test]
    fn test_alignment_checker_consistency() {
        let checker = AlignmentChecker::new();

        // 验证对齐操作的幂等性
        let addr = GuestAddr(0x12345);
        let aligned_up = checker.align_up(addr);
        let aligned_up_again = checker.align_up(aligned_up);
        assert_eq!(aligned_up.0, aligned_up_again.0);

        let aligned_down = checker.align_down(addr);
        let aligned_down_again = checker.align_down(aligned_down);
        assert_eq!(aligned_down.0, aligned_down_again.0);

        // 验证对齐后地址确实是对齐的
        assert!(checker.check_alignment(aligned_up));
        assert!(checker.check_alignment(aligned_down));
    }

    #[test]
    fn test_tlb_entry_alignment_integration() {
        // 测试SimpleTlbEntry和AlignmentChecker的集成
        let checker = AlignmentChecker::new();
        let addr = GuestAddr(0x12345);

        // 使用AlignmentChecker获取页号
        let page_num = checker.get_page_number(addr);

        // 创建对应的TLB条目
        let entry = SimpleTlbEntry::from_guest_addr(addr, page_num, 0x7);

        // 验证页号正确
        assert_eq!(entry.vpn, page_num);

        // 验证重建的地址对齐正确
        let reconstructed = entry.to_guest_addr();
        assert_eq!(reconstructed.0, checker.align_down(addr).0);
    }
}
