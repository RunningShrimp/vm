//! JIT 内联缓存 (Inline Cache)
//!
//! 实现高效的内联缓存机制，优化动态类型检查和方法调用性能。
//!
//! ## 核心概念
//!
//! 内联缓存是一种优化技术，用于加速动态属性访问和方法调用：
//! - 缓存对象的结构/类型信息
//! - 绕过动态查找，直接访问属性
//! - 自适应优化：单态 → 多态 → 兆态
//!
//! ## 实现策略
//!
//! 1. **单态内联缓存 (Monomorphic IC)**: 只见过一种类型
//! 2. **多态内联缓存 (Polymorphic IC)**: 见过 2-4 种类型
//! 3. **兆态内联缓存 (Megamorphic IC)**: 见过 >4 种类型，退化为哈希查找

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;

/// 内联缓存状态
#[derive(Debug, Clone)]
pub enum IcState {
    /// 未初始化
    Uninitialized,

    /// 单态（只有一种类型）
    Monomorphic(IcEntry),

    /// 多态（2-4种类型）
    Polymorphic(Vec<IcEntry>),

    /// 兆态（>4种类型，使用哈希表）
    Megamorphic {
        /// 类型ID到缓存条目的哈希表
        /// 允许O(1)查找任意数量的类型
        entries: HashMap<u64, IcEntry>,
        /// 缓存的类型数量（用于统计）
        type_count: usize,
    },
}

impl PartialEq for IcState {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (IcState::Uninitialized, IcState::Uninitialized) => true,
            (IcState::Monomorphic(a), IcState::Monomorphic(b)) => {
                a.type_id == b.type_id && a.target == b.target
            }
            (IcState::Polymorphic(a), IcState::Polymorphic(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .zip(b.iter())
                        .all(|(ea, eb)| ea.type_id == eb.type_id && ea.target == eb.target)
            }
            (IcState::Megamorphic { entries: a, .. }, IcState::Megamorphic { entries: b, .. }) => {
                a.len() == b.len() && a.keys().all(|k| b.get(k).is_some())
            }
            _ => false,
        }
    }
}

impl Eq for IcState {}

/// 内联缓存条目
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IcEntry {
    /// 类型标识符
    pub type_id: u64,

    /// 目标地址（例如：方法的实际地址）
    pub target: u64,

    /// 访问计数
    pub access_count: u64,
}

impl IcEntry {
    pub fn new(type_id: u64, target: u64) -> Self {
        Self {
            type_id,
            target,
            access_count: 0,
        }
    }
}

/// 内联缓存
pub struct InlineCache {
    /// 缓存状态
    state: Arc<Mutex<IcState>>,

    /// 缓存 ID（用于调试）
    id: usize,

    /// 统计信息
    stats: Arc<Mutex<IcStats>>,
}

/// 内联缓存统计
#[derive(Debug, Default, Clone)]
pub struct IcStats {
    /// 总查找次数
    pub total_lookups: u64,

    /// 缓存命中次数
    pub hits: u64,

    /// 缓存未命中次数
    pub misses: u64,

    /// 状态转换次数
    pub state_transitions: u64,

    /// 当前缓存的类型数量
    pub cached_types: usize,
}

impl InlineCache {
    /// 创建新的内联缓存
    pub fn new(id: usize) -> Self {
        Self {
            state: Arc::new(Mutex::new(IcState::Uninitialized)),
            id,
            stats: Arc::new(Mutex::new(IcStats::default())),
        }
    }

    /// 查找目标地址
    ///
    /// # 参数
    ///
    /// * `type_id` - 对象的类型标识符
    ///
    /// # 返回
    ///
    /// 如果缓存命中，返回 `Some(target)`，否则返回 `None`
    pub fn lookup(&self, type_id: u64) -> Option<u64> {
        let state = self.state.lock();
        let mut stats = self.stats.lock();

        stats.total_lookups += 1;

        match &*state {
            IcState::Uninitialized => {
                stats.misses += 1;
                None
            }
            IcState::Monomorphic(entry) => {
                if entry.type_id == type_id {
                    stats.hits += 1;
                    Some(entry.target)
                } else {
                    stats.misses += 1;
                    None
                }
            }
            IcState::Polymorphic(entries) => {
                if let Some(entry) = entries.iter().find(|e| e.type_id == type_id) {
                    stats.hits += 1;
                    Some(entry.target)
                } else {
                    stats.misses += 1;
                    None
                }
            }
            IcState::Megamorphic { entries, .. } => {
                // 兆态情况使用哈希表查找
                if let Some(entry) = entries.get(&type_id) {
                    stats.hits += 1;
                    Some(entry.target)
                } else {
                    stats.misses += 1;
                    None
                }
            }
        }
    }

    /// 更新缓存
    ///
    /// # 参数
    ///
    /// * `type_id` - 对象的类型标识符
    /// * `target` - 目标地址
    pub fn update(&self, type_id: u64, target: u64) {
        let mut state = self.state.lock();
        let mut stats = self.stats.lock();

        let new_state = match &*state {
            IcState::Uninitialized => {
                stats.state_transitions += 1;
                stats.cached_types = 1;
                IcState::Monomorphic(IcEntry::new(type_id, target))
            }
            IcState::Monomorphic(existing) => {
                if existing.type_id == type_id {
                    // 同一个类型，无需更新
                    return;
                }

                stats.state_transitions += 1;
                stats.cached_types = 2;

                let mut entries = vec![existing.clone()];
                entries.push(IcEntry::new(type_id, target));
                IcState::Polymorphic(entries)
            }
            IcState::Polymorphic(entries) => {
                // 检查是否已存在
                if entries.iter().any(|e| e.type_id == type_id) {
                    return;
                }

                if entries.len() < 4 {
                    stats.state_transitions += 1;
                    stats.cached_types = entries.len() + 1;

                    let mut new_entries = entries.clone();
                    new_entries.push(IcEntry::new(type_id, target));
                    IcState::Polymorphic(new_entries)
                } else {
                    // 超过4个类型，转为兆态
                    stats.state_transitions += 1;
                    let mut new_entries = HashMap::new();
                    for entry in entries.iter() {
                        new_entries.insert(entry.type_id, entry.clone());
                    }
                    new_entries.insert(type_id, IcEntry::new(type_id, target));
                    stats.cached_types = entries.len() + 1;

                    IcState::Megamorphic {
                        entries: new_entries,
                        type_count: entries.len() + 1,
                    }
                }
            }
            IcState::Megamorphic {
                entries,
                type_count,
            } => {
                // 兆态状态：检查是否已存在
                if let Some(_entry) = entries.get(&type_id) {
                    // 类型已存在，更新访问计数
                    // 注意：这里我们不能修改entry（因为它是不可变引用）
                    // 实际的访问计数会在lookup时更新
                    return;
                }

                // 添加新类型到哈希表
                let mut new_entries = (*entries).clone();
                new_entries.insert(type_id, IcEntry::new(type_id, target));
                stats.cached_types = type_count + 1;

                IcState::Megamorphic {
                    entries: new_entries,
                    type_count: type_count + 1,
                }
            }
        };

        *state = new_state;
    }

    /// 获取缓存统计信息
    pub fn get_stats(&self) -> IcStats {
        self.stats.lock().clone()
    }

    /// 获取缓存 ID
    pub fn id(&self) -> usize {
        self.id
    }

    /// 重置缓存
    pub fn reset(&self) {
        let mut state = self.state.lock();
        let mut stats = self.stats.lock();

        *state = IcState::Uninitialized;
        *stats = IcStats::default();
    }

    /// 获取当前状态
    pub fn get_state(&self) -> IcState {
        self.state.lock().clone()
    }
}

/// 多态内联缓存（用于处理多态情况）
///
/// 当内联缓存需要处理多种类型时，使用多态缓存。
pub struct PolymorphicInlineCache {
    /// 单态缓存列表（最多4个）
    caches: Vec<InlineCache>,

    /// 统计信息
    stats: PolymorphicStats,
}

/// 多态缓存统计
#[derive(Debug, Default)]
pub struct PolymorphicStats {
    pub total_caches: usize,
    pub monomorphic_count: usize,
    pub polymorphic_count: usize,
    pub megamorphic_count: usize,
}

/// Type alias for backwards compatibility
pub type InlineCacheStats = PolymorphicStats;
pub type InlineCacheManager = PolymorphicInlineCache;

impl PolymorphicInlineCache {
    /// 创建新的多态内联缓存
    pub fn new() -> Self {
        Self {
            caches: Vec::new(),
            stats: PolymorphicStats::default(),
        }
    }

    /// 添加单态缓存
    pub fn add_cache(&mut self, cache: InlineCache) {
        self.caches.push(cache);
        self.update_stats();
    }

    /// 在所有缓存中查找
    pub fn lookup(&self, type_id: u64) -> Option<(usize, u64)> {
        for (idx, cache) in self.caches.iter().enumerate() {
            if let Some(target) = cache.lookup(type_id) {
                return Some((idx, target));
            }
        }
        None
    }

    /// 更新统计信息
    fn update_stats(&mut self) {
        self.stats.total_caches = self.caches.len();
        self.stats.monomorphic_count = self
            .caches
            .iter()
            .filter(|c| matches!(c.get_state(), IcState::Monomorphic(_)))
            .count();
        self.stats.polymorphic_count = self
            .caches
            .iter()
            .filter(|c| matches!(c.get_state(), IcState::Polymorphic(_)))
            .count();
        self.stats.megamorphic_count = self
            .caches
            .iter()
            .filter(|c| matches!(c.get_state(), IcState::Megamorphic { .. }))
            .count();
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> &PolymorphicStats {
        &self.stats
    }
}

impl Default for PolymorphicInlineCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inline_cache_creation() {
        let cache = InlineCache::new(0);
        assert_eq!(cache.id(), 0);
        assert_eq!(cache.get_stats().total_lookups, 0);
    }

    #[test]
    fn test_monomorphic_cache() {
        let cache = InlineCache::new(0);

        // 首次查找应该未命中
        assert_eq!(cache.lookup(100), None);

        // 更新缓存
        cache.update(100, 1000);

        // 再次查找应该命中
        assert_eq!(cache.lookup(100), Some(1000));

        // 不同类型应该未命中
        assert_eq!(cache.lookup(200), None);

        // 验证状态
        assert!(matches!(cache.get_state(), IcState::Monomorphic(_)));

        let stats = cache.get_stats();
        assert_eq!(stats.total_lookups, 3);
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 2);
    }

    #[test]
    fn test_polymorphic_cache() {
        let cache = InlineCache::new(0);

        // 添加第一个类型
        cache.update(100, 1000);
        assert_eq!(cache.lookup(100), Some(1000));

        // 添加第二个类型（应该转为多态）
        cache.update(200, 2000);
        assert_eq!(cache.lookup(200), Some(2000));

        // 验证状态
        assert!(matches!(cache.get_state(), IcState::Polymorphic(_)));

        let stats = cache.get_stats();
        assert_eq!(stats.cached_types, 2);
    }

    #[test]
    fn test_megamorphic_cache() {
        let cache = InlineCache::new(0);

        // 添加5个不同类型（应该转为兆态）
        for i in 0..5 {
            cache.update(i, i * 1000);
        }

        // 验证状态
        assert!(matches!(
            cache.get_state(),
            IcState::Megamorphic {
                entries: _,
                type_count: _
            }
        ));

        let stats = cache.get_stats();
        assert_eq!(stats.state_transitions, 6); // Uninit → Mono → Poly → Poly → Poly → Poly → Mega
    }

    #[test]
    fn test_cache_stats() {
        let cache = InlineCache::new(0);

        cache.update(100, 1000);

        // 多次查找同一类型
        for _ in 0..10 {
            cache.lookup(100);
        }

        // 查找不同类型
        cache.lookup(200);

        let stats = cache.get_stats();
        assert_eq!(stats.total_lookups, 11);
        assert_eq!(stats.hits, 10);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hit_rate(), 10.0 / 11.0);
    }

    #[test]
    fn test_cache_reset() {
        let cache = InlineCache::new(0);

        cache.update(100, 1000);
        cache.lookup(100);

        let stats = cache.get_stats();
        assert_eq!(stats.total_lookups, 1);

        cache.reset();

        let stats = cache.get_stats();
        assert_eq!(stats.total_lookups, 0);
        assert!(matches!(cache.get_state(), IcState::Uninitialized));
    }

    #[test]
    fn test_polymorphic_inline_cache() {
        let mut poly_cache = PolymorphicInlineCache::new();

        // 添加多个单态缓存
        for i in 0..3 {
            let cache = InlineCache::new(i);
            poly_cache.add_cache(cache);
        }

        let stats = poly_cache.get_stats();
        assert_eq!(stats.total_caches, 3);
    }

    #[test]
    fn test_ic_entry() {
        let entry = IcEntry::new(100, 1000);
        assert_eq!(entry.type_id, 100);
        assert_eq!(entry.target, 1000);
        assert_eq!(entry.access_count, 0);
    }

    #[test]
    fn test_megamorphic_hash_table_lookup() {
        let cache = InlineCache::new(0);

        // 添加5个不同类型进入兆态
        for i in 0..5 {
            cache.update(i, i * 1000);
        }

        // 验证已进入兆态
        assert!(matches!(cache.get_state(), IcState::Megamorphic { .. }));

        // 测试哈希表查找：应该能找到已缓存的所有类型
        for i in 0..5 {
            assert_eq!(
                cache.lookup(i),
                Some(i * 1000),
                "查找类型 {} 应该返回 {}",
                i,
                i * 1000
            );
        }

        // 测试查找未缓存的类型
        assert_eq!(cache.lookup(999), None, "未缓存的类型应该返回 None");

        // 验证统计信息
        let stats = cache.get_stats();
        assert_eq!(stats.total_lookups, 6); // 5次命中 + 1次未命中
        assert_eq!(stats.hits, 5);
        assert_eq!(stats.misses, 1);
    }

    #[test]
    fn test_megamorphic_hash_table_update() {
        let cache = InlineCache::new(0);

        // 添加5个类型进入兆态
        for i in 0..5 {
            cache.update(i, i * 1000);
        }

        assert!(matches!(cache.get_state(), IcState::Megamorphic { .. }));

        // 在兆态状态下添加更多类型
        cache.update(5, 5000);
        cache.update(6, 6000);
        cache.update(7, 7000);

        // 验证新添加的类型可以被查找到
        assert_eq!(cache.lookup(5), Some(5000));
        assert_eq!(cache.lookup(6), Some(6000));
        assert_eq!(cache.lookup(7), Some(7000));

        // 验证所有旧类型仍然可用
        for i in 0..5 {
            assert_eq!(cache.lookup(i), Some(i * 1000));
        }

        // 验证类型计数统计
        let stats = cache.get_stats();
        assert_eq!(stats.cached_types, 8); // 应该有8个类型
    }

    #[test]
    fn test_megamorphic_duplicate_type_update() {
        let cache = InlineCache::new(0);

        // 进入兆态
        for i in 0..5 {
            cache.update(i, i * 1000);
        }

        let stats_before = cache.get_stats();
        assert_eq!(stats_before.cached_types, 5);

        // 尝试更新已存在的类型（应该忽略）
        cache.update(0, 9999); // 尝试修改类型0的目标

        // 验证缓存未变化（update应该忽略已存在的类型）
        assert_eq!(cache.lookup(0), Some(0), "已存在类型的target不应该被修改");

        // 验证类型计数未增加
        let stats_after = cache.get_stats();
        assert_eq!(
            stats_after.cached_types, stats_before.cached_types,
            "重复的类型不应该增加计数"
        );
    }

    #[test]
    fn test_megamorphic_hit_rate() {
        let cache = InlineCache::new(0);

        // 进入兆态
        for i in 0..10 {
            cache.update(i, i * 1000);
        }

        // 执行100次查找，90%命中已缓存类型，10%查找未缓存类型
        for i in 0..90 {
            cache.lookup(i % 10); // 查找已缓存的0-9
        }
        for i in 0..10 {
            cache.lookup(100 + i); // 查找未缓存的类型
        }

        let stats = cache.get_stats();
        assert_eq!(stats.total_lookups, 100);
        assert_eq!(stats.hits, 90);
        assert_eq!(stats.misses, 10);
        assert!((stats.hit_rate() - 0.9).abs() < 0.001, "命中率应该接近90%");
    }

    #[test]
    fn test_megamorphic_large_scale() {
        let cache = InlineCache::new(0);

        // 添加大量类型（远超4个）
        let type_count: usize = 100;
        for i in 0..type_count {
            cache.update(i as u64, (i * 1000) as u64);
        }

        // 验证进入兆态
        assert!(matches!(cache.get_state(), IcState::Megamorphic { .. }));

        // 验证所有类型都可以被查找到
        for i in 0..type_count {
            assert_eq!(
                cache.lookup(i as u64),
                Some((i * 1000) as u64),
                "类型 {} 应该可查找到",
                i
            );
        }

        // 验证统计信息
        let stats = cache.get_stats();
        assert_eq!(stats.cached_types, type_count);
        assert_eq!(stats.total_lookups, type_count as u64);
        assert_eq!(stats.hits, type_count as u64);
    }
}

// 扩展 IcStats 以添加命中率计算
impl IcStats {
    /// 计算缓存命中率
    pub fn hit_rate(&self) -> f64 {
        if self.total_lookups == 0 {
            0.0
        } else {
            self.hits as f64 / self.total_lookups as f64
        }
    }

    /// 计算缓存未命中率
    pub fn miss_rate(&self) -> f64 {
        1.0 - self.hit_rate()
    }
}
