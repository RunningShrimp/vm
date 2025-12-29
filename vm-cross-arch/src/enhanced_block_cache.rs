//! 块级缓存优化
//!
//! 增强CrossArchBlockCache，添加统计和性能监控功能

use super::block_cache::{CrossArchBlockCache, SourceBlockKey, TranslatedBlock};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::{Mutex, RwLock};


/// 增强的块级缓存，带详细统计
pub struct EnhancedBlockCache {
    /// 基础缓存
    inner: Arc<Mutex<CrossArchBlockCache>>,
    /// 缓存统计
    stats: Arc<RwLock<EnhancedCacheStats>>,
    /// 热块追踪
    hot_blocks: Arc<RwLock<HashMap<u64, HotBlockInfo>>>,
    /// 热块阈值（访问次数）
    hot_threshold: u64,
}

/// 增强的缓存统计
#[derive(Debug, Clone, Default)]
pub struct EnhancedCacheStats {
    /// 总查询次数
    pub total_queries: u64,
    /// 缓存命中次数
    pub hits: u64,
    /// 缓存未命中次数
    pub misses: u64,
    /// 命中率
    pub hit_rate: f64,
    /// 平均访问时间（微秒）
    pub avg_access_time_us: f64,
    /// 热块命中次数
    pub hot_block_hits: u64,
    /// 冷块命中次数
    pub cold_block_hits: u64,
}

/// 热块信息
#[derive(Debug, Clone)]
struct HotBlockInfo {
    /// 块地址
    address: u64,
    /// 访问次数
    access_count: u64,
    /// 最后访问时间
    last_access: std::time::Instant,
    /// 总访问时间（微秒）
    total_access_time_us: u64,
}

impl EnhancedBlockCache {
    /// 创建新的增强缓存
    pub fn new(max_size: usize, hot_threshold: u64) -> Self {
        use super::block_cache::CacheReplacementPolicy;
        Self {
            inner: Arc::new(Mutex::new(CrossArchBlockCache::new(max_size, CacheReplacementPolicy::Lru))),
            stats: Arc::new(RwLock::new(EnhancedCacheStats::default())),
            hot_blocks: Arc::new(RwLock::new(HashMap::new())),
            hot_threshold,
        }
    }

    /// 使用默认配置创建
    pub fn with_default_config() -> Self {
        Self::new(1024, 100) // 1K条目，100次访问为热块
    }

    /// 查找翻译块（带统计）
    pub fn lookup(&self, key: &SourceBlockKey) -> Option<TranslatedBlock> {
        let start = std::time::Instant::now();

        // 查找缓存并立即克隆结果
        let result = {
            let mut cache = self.inner.lock().unwrap();
            cache.lookup(key).cloned()
        };

        // 更新统计
        let mut stats = self.stats.write().unwrap();
        stats.total_queries += 1;

        if result.is_some() {
            stats.hits += 1;

            // 更热块统计
            if let Some(ref _block) = result {
                self.update_hot_block_stats(key.start_pc.0, start.elapsed());
            }
        } else {
            stats.misses += 1;
        }

        stats.hit_rate = stats.hits as f64 / stats.total_queries as f64;

        // 更新平均访问时间
        let elapsed_us = start.elapsed().as_micros() as f64;
        stats.avg_access_time_us =
            (stats.avg_access_time_us * (stats.total_queries - 1) as f64 + elapsed_us)
                / stats.total_queries as f64;

        result
    }

    /// 插入翻译块
    pub fn insert(&self, key: SourceBlockKey, block: TranslatedBlock) {
        self.inner.lock().unwrap().insert(key, block);
    }

    /// 获取热块列表
    pub fn get_hot_blocks(&self) -> Vec<u64> {
        let hot_blocks = self.hot_blocks.read().unwrap();
        hot_blocks
            .values()
            .filter(|info| info.access_count >= self.hot_threshold)
            .map(|info| info.address)
            .collect()
    }

    /// 更热块统计
    fn update_hot_block_stats(&self, address: u64, access_time: std::time::Duration) {
        let mut hot_blocks = self.hot_blocks.write().unwrap();
        let info = hot_blocks.entry(address).or_insert_with(|| HotBlockInfo {
            address,
            access_count: 0,
            last_access: std::time::Instant::now(),
            total_access_time_us: 0,
        });

        info.access_count += 1;
        info.last_access = std::time::Instant::now();
        info.total_access_time_us += access_time.as_micros() as u64;
    }

    /// 获取增强统计信息
    pub fn get_enhanced_stats(&self) -> EnhancedCacheStats {
        self.stats.read().unwrap().clone()
    }

    /// 清空缓存和统计
    pub fn clear(&self) {
        self.inner.lock().unwrap().clear();
        let mut stats = self.stats.write().unwrap();
        *stats = EnhancedCacheStats::default();
        self.hot_blocks.write().unwrap().clear();
    }

    /// 预热块（提前插入热块）
    pub fn warm_up(&self, blocks: Vec<(SourceBlockKey, TranslatedBlock)>) {
        for (key, block) in blocks {
            self.insert(key, block);
        }
    }

    /// 获取缓存大小
    pub fn size(&self) -> usize {
        // Get cache size from the base cache stats
        self.inner.lock().unwrap().stats().current_size
    }

    /// 设置最大缓存大小
    pub fn set_max_size(&self, size: usize) {
        self.inner.lock().unwrap().set_max_size(size);
    }

    /// 获取基础统计
    pub fn get_base_stats(&self) -> super::block_cache::CacheStats {
        self.inner.lock().unwrap().stats().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enhanced_cache_creation() {
        let cache = EnhancedBlockCache::with_default_config();
        let stats = cache.get_enhanced_stats();

        assert_eq!(stats.total_queries, 0);
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
    }

    #[test]
    fn test_enhanced_cache_lookup_miss() {
        let cache = EnhancedBlockCache::with_default_config();
        let key = SourceBlockKey::new(
            crate::SourceArch::X86_64,
            crate::TargetArch::ARM64,
            0x1000,
            &crate::vm_ir::IRBlock {
                ops: vec![],
                term: crate::vm_ir::Terminator::Ret,
            },
        );

        let result = cache.lookup(&key);
        assert!(result.is_none());

        let stats = cache.get_enhanced_stats();
        assert_eq!(stats.total_queries, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hit_rate, 0.0);
    }

    #[test]
    fn test_enhanced_cache_hit() {
        let cache = EnhancedBlockCache::with_default_config();
        let key = SourceBlockKey::new(
            crate::SourceArch::X86_64,
            crate::TargetArch::ARM64,
            0x1000,
            &crate::vm_ir::IRBlock {
                ops: vec![],
                term: crate::vm_ir::Terminator::Ret,
            },
        );

        let block = TranslatedBlock {
            instructions: vec![],
            stats: super::super::translation_impl::TranslationStats::default(),
        };

        cache.insert(key.clone(), block);

        let result = cache.lookup(&key);
        assert!(result.is_some());

        let stats = cache.get_enhanced_stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.hit_rate, 0.5); // 1次命中，1次未命中
    }

    #[test]
    fn test_hot_block_detection() {
        let cache = EnhancedBlockCache::with_default_config(); // hot_threshold=100

        let key = SourceBlockKey::new(
            crate::SourceArch::X86_64,
            crate::TargetArch::ARM64,
            0x1000,
            &crate::vm_ir::IRBlock {
                ops: vec![],
                term: crate::vm_ir::Terminator::Ret,
            },
        );

        let block = TranslatedBlock {
            instructions: vec![],
            stats: super::super::translation_impl::TranslationStats::default(),
        };

        cache.insert(key.clone(), block);

        // 访问150次（超过热块阈值100）
        for _ in 0..150 {
            cache.lookup(&key);
        }

        let hot_blocks = cache.get_hot_blocks();
        assert!(hot_blocks.contains(&0x1000));
    }

    #[test]
    fn test_clear() {
        let cache = EnhancedBlockCache::with_default_config();

        let key = SourceBlockKey::new(
            crate::SourceArch::X86_64,
            crate::TargetArch::ARM64,
            0x1000,
            &crate::vm_ir::IRBlock {
                ops: vec![],
                term: crate::vm_ir::Terminator::Ret,
            },
        );

        let block = TranslatedBlock {
            instructions: vec![],
            stats: super::super::translation_impl::TranslationStats::default(),
        };

        cache.insert(key.clone(), block);
        cache.lookup(&key);

        assert_eq!(cache.size(), 1);
        assert_eq!(cache.get_enhanced_stats().total_queries, 1);

        cache.clear();

        assert_eq!(cache.size(), 0);
        assert_eq!(cache.get_enhanced_stats().total_queries, 0);
    }
}
