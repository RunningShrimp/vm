//! 缓存管理器实现（基础设施层）
//!
//! 提供通用的缓存管理实现，支持多种替换策略和多级缓存。
//! 这是基础设施层的实现，具体的技术细节应在此模块中。

use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::time::Instant;

use vm_core::VmResult;
use vm_core::domain::{CacheManager, CacheStats};

/// 缓存替换策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheReplacementPolicy {
    /// Least Recently Used
    LRU,
    /// Least Frequently Used
    LFU,
    /// First In First Out
    FIFO,
    /// Random replacement
    Random,
}

/// 缓存条目（带元数据）
#[derive(Debug, Clone)]
pub struct CacheEntry<V> {
    /// 缓存值
    pub value: V,
    /// 访问计数
    pub access_count: u64,
    /// 最后访问时间戳
    pub last_access: Instant,
    /// 创建时间戳
    pub created_at: Instant,
    /// 条目优先级
    pub priority: u32,
}

impl<V> CacheEntry<V> {
    pub fn new(value: V) -> Self {
        let now = Instant::now();
        Self {
            value,
            access_count: 0,
            last_access: now,
            created_at: now,
            priority: 0,
        }
    }

    pub fn with_priority(value: V, priority: u32) -> Self {
        let mut entry = Self::new(value);
        entry.priority = priority;
        entry
    }
}

/// 通用缓存管理器（基础设施层实现）
///
/// 实现 `CacheManager` trait，提供通用的缓存管理功能。
/// 支持多种替换策略（LRU、LFU、FIFO、Random）。
pub struct GenericCacheManager<K, V>
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    /// 缓存条目
    entries: HashMap<K, CacheEntry<V>>,
    /// 替换策略
    policy: CacheReplacementPolicy,
    /// LRU 队列（用于 LRU 和 FIFO）
    lru_queue: VecDeque<K>,
    /// 访问频率映射（用于 LFU）
    access_frequency: HashMap<K, u64>,
    /// 最大容量
    capacity: usize,
    /// 当前大小
    size: usize,
    /// 统计信息
    stats: CacheStatistics,
}

/// 缓存统计信息
#[derive(Debug, Clone, Default)]
struct CacheStatistics {
    hits: u64,
    #[allow(dead_code)] // JIT基础设施 - 预留用于性能统计
    misses: u64,
    evictions: u64,
}

impl<K, V> GenericCacheManager<K, V>
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    /// 创建新的缓存管理器
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(capacity),
            policy: CacheReplacementPolicy::LRU,
            lru_queue: VecDeque::with_capacity(capacity),
            access_frequency: HashMap::with_capacity(capacity),
            capacity,
            size: 0,
            stats: CacheStatistics::default(),
        }
    }

    /// 创建带替换策略的缓存管理器
    pub fn with_policy(capacity: usize, policy: CacheReplacementPolicy) -> Self {
        Self {
            entries: HashMap::with_capacity(capacity),
            policy,
            lru_queue: VecDeque::with_capacity(capacity),
            access_frequency: HashMap::with_capacity(capacity),
            capacity,
            size: 0,
            stats: CacheStatistics::default(),
        }
    }

    /// 设置替换策略
    pub fn set_policy(&mut self, policy: CacheReplacementPolicy) {
        self.policy = policy;
    }

    /// 获取统计信息
    pub fn get_statistics(&self) -> &CacheStatistics {
        &self.stats
    }

    /// 内部方法：执行淘汰
    fn evict_one(&mut self) -> VmResult<()> {
        if self.size == 0 {
            return Ok(());
        }

        match self.policy {
            CacheReplacementPolicy::LRU => self.evict_lru(),
            CacheReplacementPolicy::LFU => self.evict_lfu(),
            CacheReplacementPolicy::FIFO => self.evict_fifo(),
            CacheReplacementPolicy::Random => self.evict_random(),
        }
    }

    fn evict_lru(&mut self) -> VmResult<()> {
        if let Some(key) = self.lru_queue.pop_front() {
            self.entries.remove(&key);
            self.access_frequency.remove(&key);
            self.size -= 1;
            self.stats.evictions += 1;
        }
        Ok(())
    }

    fn evict_lfu(&mut self) -> VmResult<()> {
        if let Some((key, _)) = self
            .access_frequency
            .iter()
            .min_by_key(|&(_, &count)| count)
        {
            let key = key.clone();
            self.entries.remove(&key);
            self.access_frequency.remove(&key);
            self.lru_queue.retain(|k| k != &key);
            self.size -= 1;
            self.stats.evictions += 1;
        }
        Ok(())
    }

    fn evict_fifo(&mut self) -> VmResult<()> {
        self.evict_lru()
    }

    fn evict_random(&mut self) -> VmResult<()> {
        if let Some(key) = self.entries.keys().next() {
            let key = key.clone();
            self.entries.remove(&key);
            self.access_frequency.remove(&key);
            self.lru_queue.retain(|k| k != &key);
            self.size -= 1;
            self.stats.evictions += 1;
        }
        Ok(())
    }

    /// 更新 LRU 队列
    fn update_lru(&mut self, key: &K) {
        self.lru_queue.retain(|k| k != key);
        self.lru_queue.push_back(key.clone());
    }
}

impl<K, V> CacheManager<K, V> for GenericCacheManager<K, V>
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    fn get(&self, key: &K) -> Option<V> {
        // 注意：trait 定义中 get 是不可变的，但我们需要更新统计
        // 这里只返回值，统计更新在 get_with_stats 中处理
        self.entries.get(key).map(|entry| entry.value.clone())
    }

    fn put(&mut self, key: K, value: V) {
        // 如果已满且是新键，执行淘汰
        if self.size >= self.capacity && !self.entries.contains_key(&key) {
            let _ = self.evict_one();
        }

        // 插入或更新条目
        let entry = CacheEntry::new(value);
        let is_new = !self.entries.contains_key(&key);
        self.entries.insert(key.clone(), entry);

        if is_new {
            self.size += 1;
            self.update_lru(&key);
            self.access_frequency.insert(key.clone(), 0);
        } else {
            self.update_lru(&key);
        }
    }

    fn evict(&mut self, key: &K) {
        if self.entries.remove(key).is_some() {
            self.access_frequency.remove(key);
            self.lru_queue.retain(|k| k != key);
            self.size -= 1;
        }
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.lru_queue.clear();
        self.access_frequency.clear();
        self.size = 0;
        self.stats = CacheStatistics::default();
    }

    fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.stats.hits,
            misses: self.stats.misses,
            size: self.size,
            capacity: self.capacity,
        }
    }
}

// 扩展方法：更新访问统计
impl<K, V> GenericCacheManager<K, V>
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    /// 获取并更新访问统计（内部使用）
    pub fn get_with_stats(&mut self, key: &K) -> Option<V> {
        // 先更新访问频率，避免借用冲突
        self.access_frequency
            .entry(key.clone())
            .and_modify(|count| *count += 1)
            .or_insert(1);

        if let Some(entry) = self.entries.get_mut(key) {
            entry.access_count += 1;
            entry.last_access = Instant::now();
            // LRU更新在最后进行
            drop(entry);
            self.update_lru(key);
            self.stats.hits += 1;
            self.entries.get(key).map(|e| e.value.clone())
        } else {
            self.stats.misses += 1;
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_creation() {
        let cache = GenericCacheManager::<u64, String>::new(100);
        assert_eq!(cache.capacity, 100);
        assert_eq!(cache.size, 0);
    }

    #[test]
    fn test_cache_put_and_get() {
        let mut cache = GenericCacheManager::<u64, String>::new(10);
        cache.put(1, "value1".to_string());
        assert_eq!(cache.get(&1), Some("value1".to_string()));
    }

    #[test]
    fn test_cache_eviction() {
        let mut cache =
            GenericCacheManager::<u64, String>::with_policy(2, CacheReplacementPolicy::LRU);
        cache.put(1, "value1".to_string());
        cache.put(2, "value2".to_string());
        cache.put(3, "value3".to_string()); // 应该淘汰 1

        assert_eq!(cache.get(&1), None);
        assert_eq!(cache.get(&2), Some("value2".to_string()));
        assert_eq!(cache.get(&3), Some("value3".to_string()));
    }

    #[test]
    fn test_cache_clear() {
        let mut cache = GenericCacheManager::<u64, String>::new(10);
        cache.put(1, "value1".to_string());
        cache.put(2, "value2".to_string());
        cache.clear();
        assert_eq!(cache.size, 0);
        assert_eq!(cache.get(&1), None);
    }

    #[test]
    fn test_cache_stats() {
        let mut cache = GenericCacheManager::<u64, String>::new(10);
        cache.put(1, "value1".to_string());
        let _ = cache.get_with_stats(&1);
        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.size, 1);
    }
}
