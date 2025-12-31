//! 翻译缓存模块
//!
//! 提供高性能的代码翻译缓存，支持LRU驱逐策略和多级缓存

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use vm_ir::IRBlock;

use crate::jit::backend::CompiledCode;

/// 翻译缓存条目
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// 编译后的代码
    pub code: Arc<CompiledCode>,
    /// IR块（可选，用于调试）
    pub ir_block: Option<IRBlock>,
    /// 访问时间戳
    pub last_access: u64,
    /// 访问次数
    pub access_count: u64,
}

/// 翻译缓存配置
#[derive(Debug, Clone)]
pub struct TranslationCacheConfig {
    /// 最大缓存条目数
    pub max_entries: usize,
    /// 是否启用LRU驱逐
    pub enable_lru: bool,
    /// 是否统计访问信息
    pub track_access: bool,
    /// 预分配容量
    pub prealloc_capacity: usize,
}

impl Default for TranslationCacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 1024,
            enable_lru: true,
            track_access: true,
            prealloc_capacity: 128,
        }
    }
}

/// 翻译缓存统计信息
#[derive(Debug, Clone, Default)]
pub struct TranslationCacheStats {
    /// 查找次数
    pub lookups: u64,
    /// 命中次数
    pub hits: u64,
    /// 未命中次数
    pub misses: u64,
    /// 插入次数
    pub inserts: u64,
    /// 驱逐次数
    pub evictions: u64,
    /// 清空次数
    pub clears: u64,
    /// 当前条目数
    pub current_entries: usize,
}

impl TranslationCacheStats {
    /// 计算命中率
    pub fn hit_rate(&self) -> f64 {
        if self.lookups == 0 {
            return 0.0;
        }
        self.hits as f64 / self.lookups as f64
    }
}

/// 翻译缓存
///
/// 提供高性能的代码翻译缓存，支持LRU驱逐策略
pub struct TranslationCache {
    /// 缓存条目映射
    entries: HashMap<u64, CacheEntry>,
    /// LRU访问队列
    lru_queue: VecDeque<u64>,
    /// 配置
    config: TranslationCacheConfig,
    /// 统计信息
    stats: TranslationCacheStats,
    /// 访问计数器
    access_counter: u64,
}

impl TranslationCache {
    /// 创建新的翻译缓存
    pub fn new() -> Self {
        Self::with_config(TranslationCacheConfig::default())
    }

    /// 使用指定配置创建翻译缓存
    pub fn with_config(config: TranslationCacheConfig) -> Self {
        Self {
            entries: HashMap::with_capacity(config.prealloc_capacity),
            lru_queue: VecDeque::with_capacity(config.prealloc_capacity),
            config,
            stats: TranslationCacheStats::default(),
            access_counter: 0,
        }
    }

    /// 查找缓存的代码
    pub fn lookup(&mut self, key: u64) -> Option<Arc<CompiledCode>> {
        self.stats.lookups += 1;

        if let Some(entry) = self.entries.get_mut(&key) {
            // 缓存命中
            self.stats.hits += 1;

            if self.config.track_access {
                entry.access_count += 1;
                entry.last_access = self.access_counter;
                self.access_counter += 1;
            }

            // 更新LRU队列
            if self.config.enable_lru {
                self.lru_queue.retain(|&k| k != key);
                self.lru_queue.push_back(key);
            }

            Some(entry.code.clone())
        } else {
            // 缓存未命中
            self.stats.misses += 1;
            None
        }
    }

    /// 插入代码到缓存
    pub fn insert(&mut self, key: u64, code: Arc<CompiledCode>, ir_block: Option<IRBlock>) {
        // 检查是否需要驱逐
        if self.entries.len() >= self.config.max_entries {
            self.evict_lru();
        }

        let entry = CacheEntry {
            code,
            ir_block,
            last_access: self.access_counter,
            access_count: 1,
        };

        self.entries.insert(key, entry);
        self.stats.inserts += 1;

        if self.config.track_access {
            self.access_counter += 1;
        }

        // 更新LRU队列
        if self.config.enable_lru {
            self.lru_queue.push_back(key);
        }

        // 更新当前条目数
        self.stats.current_entries = self.entries.len();
    }

    /// 插入代码到缓存（简化版本）
    pub fn insert_simple(&mut self, key: u64, code: Arc<CompiledCode>) {
        self.insert(key, code, None);
    }

    /// 驱逐最少使用的条目
    fn evict_lru(&mut self) {
        if let Some(key) = self.lru_queue.pop_front() {
            self.entries.remove(&key);
            self.stats.evictions += 1;
        }
    }

    /// 清空缓存
    pub fn clear(&mut self) {
        self.entries.clear();
        self.lru_queue.clear();
        self.stats.clears += 1;
        self.stats.current_entries = 0;
    }

    /// 获取缓存大小
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 检查缓存是否为空
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 检查是否包含指定key
    pub fn contains(&self, key: u64) -> bool {
        self.entries.contains_key(&key)
    }

    /// 移除指定条目
    pub fn remove(&mut self, key: u64) -> bool {
        let removed = self.entries.remove(&key).is_some();
        if removed {
            self.lru_queue.retain(|&k| k != key);
            self.stats.evictions += 1;
            self.stats.current_entries = self.entries.len();
        }
        removed
    }

    /// 获取统计信息
    pub fn stats(&self) -> &TranslationCacheStats {
        &self.stats
    }

    /// 重置统计信息
    pub fn reset_stats(&mut self) {
        self.stats = TranslationCacheStats {
            current_entries: self.entries.len(),
            ..Default::default()
        };
    }

    /// 预热缓存（插入多个条目）
    pub fn warmup(&mut self, entries: Vec<(u64, Arc<CompiledCode>)>) {
        for (key, code) in entries {
            if self.entries.len() < self.config.max_entries {
                self.insert_simple(key, code);
            }
        }
    }

    /// 获取缓存配置
    pub fn config(&self) -> &TranslationCacheConfig {
        &self.config
    }

    /// 更新配置
    pub fn set_config(&mut self, config: TranslationCacheConfig) {
        // 如果新配置的max_entries更小，需要驱逐多余条目
        while self.entries.len() > config.max_entries {
            self.evict_lru();
        }
        self.config = config;
    }

    /// 获取缓存中最常访问的条目
    pub fn get_hot_entries(&self, limit: usize) -> Vec<(u64, &CacheEntry)> {
        let mut entries: Vec<_> = self.entries.iter().collect();
        entries.sort_by(|a, b| b.1.access_count.cmp(&a.1.access_count));
        entries
            .into_iter()
            .take(limit)
            .map(|(k, v)| (*k, v))
            .collect()
    }

    /// 获取缓存大小（字节）
    pub fn size_in_bytes(&self) -> usize {
        self.entries
            .values()
            .map(|entry| {
                let code_size = entry.code.code.len() + std::mem::size_of::<CompiledCode>();
                let entry_size = std::mem::size_of::<CacheEntry>();
                code_size + entry_size
            })
            .sum()
    }
}

impl Default for TranslationCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_creation() {
        let cache = TranslationCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_cache_insert() {
        let mut cache = TranslationCache::new();
        let code = Arc::new(CompiledCode {
            code: vec![1, 2, 3],
            size: 3,
            exec_addr: 0,
        });

        cache.insert_simple(123, code);
        assert_eq!(cache.len(), 1);
        assert!(cache.contains(123));
    }

    #[test]
    fn test_cache_lookup() {
        let mut cache = TranslationCache::new();
        let code = Arc::new(CompiledCode {
            code: vec![1, 2, 3],
            size: 3,
            exec_addr: 0,
        });

        cache.insert_simple(123, code.clone());
        let found = cache.lookup(123);
        assert!(found.is_some());
        assert_eq!(found.unwrap().code, vec![1, 2, 3]);
    }

    #[test]
    fn test_cache_miss() {
        let mut cache = TranslationCache::new();
        let found = cache.lookup(999);
        assert!(found.is_none());
    }

    #[test]
    fn test_cache_stats() {
        let mut cache = TranslationCache::new();
        let code = Arc::new(CompiledCode {
            code: vec![1, 2, 3],
            size: 3,
            exec_addr: 0,
        });

        cache.insert_simple(123, code);
        cache.lookup(123); // hit
        cache.lookup(999); // miss

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert!((stats.hit_rate() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_cache_clear() {
        let mut cache = TranslationCache::new();
        let code = Arc::new(CompiledCode {
            code: vec![1, 2, 3],
            size: 3,
            exec_addr: 0,
        });

        cache.insert_simple(123, code);
        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_remove() {
        let mut cache = TranslationCache::new();
        let code = Arc::new(CompiledCode {
            code: vec![1, 2, 3],
            size: 3,
            exec_addr: 0,
        });

        cache.insert_simple(123, code);
        assert!(cache.remove(123));
        assert!(!cache.contains(123));
        assert!(!cache.remove(999));
    }

    #[test]
    fn test_lru_eviction() {
        let config = TranslationCacheConfig {
            max_entries: 3,
            ..Default::default()
        };
        let mut cache = TranslationCache::with_config(config);

        // 插入3个条目
        for i in 1..=3 {
            let code = Arc::new(CompiledCode {
                code: vec![i as u8],
                size: 1,
                exec_addr: 0,
            });
            cache.insert_simple(i, code);
        }

        // 访问第1个条目，使其成为最近使用
        cache.lookup(1);

        // 插入第4个条目，应该驱逐第2个（最少使用）
        let code = Arc::new(CompiledCode {
            code: vec![4],
            size: 1,
            exec_addr: 0,
        });
        cache.insert_simple(4, code);

        assert!(cache.contains(1)); // 最近访问，应该保留
        assert!(!cache.contains(2)); // 被驱逐
        assert!(cache.contains(3));
        assert!(cache.contains(4));
    }

    #[test]
    fn test_hot_entries() {
        let mut cache = TranslationCache::new();

        // 插入多个条目
        for i in 1..=5 {
            let code = Arc::new(CompiledCode {
                code: vec![i as u8],
                size: 1,
                exec_addr: 0,
            });
            cache.insert_simple(i, code);
        }

        // 多次访问某些条目
        for _ in 0..10 {
            cache.lookup(1);
            cache.lookup(2);
        }
        for _ in 0..5 {
            cache.lookup(3);
        }

        let hot_entries = cache.get_hot_entries(2);
        assert_eq!(hot_entries.len(), 2);
        assert_eq!(hot_entries[0].0, 1); // 访问最多
        assert_eq!(hot_entries[1].0, 2);
    }

    #[test]
    fn test_warmup() {
        let mut cache = TranslationCache::new();

        let entries = vec![
            (
                1,
                Arc::new(CompiledCode {
                    code: vec![1],
                    size: 1,
                    exec_addr: 0,
                }),
            ),
            (
                2,
                Arc::new(CompiledCode {
                    code: vec![2],
                    size: 1,
                    exec_addr: 0,
                }),
            ),
            (
                3,
                Arc::new(CompiledCode {
                    code: vec![3],
                    size: 1,
                    exec_addr: 0,
                }),
            ),
        ];

        cache.warmup(entries);
        assert_eq!(cache.len(), 3);
    }
}
