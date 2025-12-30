//! 代码缓存模块（简化版）
//!
//! 提供基础的代码缓存功能

use std::collections::HashMap;
use std::sync::Arc;

use crate::jit::backend::CompiledCode;

/// 缓存统计信息
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// 命中次数
    pub hits: usize,
    /// 未命中次数
    pub misses: usize,
    /// 插入次数
    pub inserts: usize,
    /// 移除次数
    pub removals: usize,
}

impl CacheStats {
    /// 计算命中率
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            return 0.0;
        }
        self.hits as f64 / total as f64
    }
}

/// 分层缓存统计
#[derive(Debug, Clone, Default)]
pub struct TieredCacheStats {
    /// L1缓存命中
    pub l1_hits: usize,
    /// L2缓存命中
    pub l2_hits: usize,
    /// L3缓存命中
    pub l3_hits: usize,
    /// 总未命中
    pub misses: usize,
    /// 基础统计
    pub base_stats: CacheStats,
    /// L1到L2提升
    pub l1_to_l2_promotions: usize,
    /// L2到L1提升
    pub l2_to_l1_promotions: usize,
    /// L3到L2提升
    pub l3_to_l2_promotions: usize,
    /// L1驱逐
    pub l1_evictions: usize,
    /// L2驱逐
    pub l2_evictions: usize,
    /// L3驱逐
    pub l3_evictions: usize,
}

/// 简单的代码缓存
pub struct CodeCache {
    /// 缓存条目
    entries: HashMap<u64, Arc<CompiledCode>>,
    /// 缓存大小限制
    max_size: usize,
    /// 统计信息
    stats: TieredCacheStats,
}

impl CodeCache {
    /// 创建新的代码缓存
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_size,
            stats: TieredCacheStats::default(),
        }
    }

    /// 查找缓存的代码
    pub fn lookup(&self, key: u64) -> Option<Arc<CompiledCode>> {
        self.entries.get(&key).cloned()
    }

    /// 插入代码到缓存
    pub fn insert(&mut self, key: u64, code: Arc<CompiledCode>) {
        // 如果缓存已满，清空一些条目
        if self.entries.len() >= self.max_size {
            self.entries.clear();
        }
        self.entries.insert(key, code);
    }

    /// 清空缓存
    pub fn clear(&mut self) {
        self.entries.clear();
        self.stats = TieredCacheStats::default();
    }

    /// 获取缓存大小
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 获取统计信息
    pub fn stats(&self) -> &TieredCacheStats {
        &self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_basic() {
        let mut cache = CodeCache::new(10);
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_cache_insert() {
        let mut cache = CodeCache::new(10);
        let code = Arc::new(CompiledCode {
            code: vec![1, 2, 3],
            size: 3,
            exec_addr: 0,
        });
        cache.insert(123, code);
        assert_eq!(cache.len(), 1);
        assert!(cache.lookup(123).is_some());
    }

    #[test]
    fn test_cache_stats() {
        let stats = CacheStats {
            hits: 80,
            misses: 20,
            inserts: 100,
            removals: 0,
        };
        assert!((stats.hit_rate() - 0.8).abs() < 0.01);
    }
}
