//! 最简化版统一JIT编译缓存实现
//!
//! 专注于基本的缓存功能，不依赖外部模块和异步

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use std::time::Instant;

/// 缓存条目
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// 代码指针
    pub code_ptr: *const u8,
    /// 代码大小
    pub code_size: usize,
    /// 访问次数
    pub access_count: u64,
    /// 编译成本
    pub compilation_cost: u64,
    /// 创建时间戳
    pub created_timestamp: u64,
    /// 最后访问时间戳
    pub last_access_timestamp: u64,
    /// 热度评分
    pub hotness_score: f32,
    /// 执行收益
    pub execution_benefit: f32,
}

impl CacheEntry {
    pub fn new(code_ptr: *const u8, code_size: usize) -> Self {
        let now = Self::current_timestamp();
        Self {
            code_ptr,
            code_size,
            access_count: 0,
            compilation_cost: 0,
            created_timestamp: now,
            last_access_timestamp: now,
            hotness_score: 0.0,
            execution_benefit: 0.0,
        }
    }

    /// 获取当前时间戳（纳秒）
    fn current_timestamp() -> u64 {
        use std::time::SystemTime;
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    }

    /// 更新访问信息
    pub fn update_access(&mut self) {
        self.last_access_timestamp = Self::current_timestamp();
        self.access_count += 1;
        self.update_hotness_score();
    }

    /// 更新热度评分
    fn update_hotness_score(&mut self) {
        let now = Self::current_timestamp();
        let age_ns = now.saturating_sub(self.created_timestamp);
        let recency_ns = now.saturating_sub(self.last_access_timestamp);
        
        // 转换为秒
        let age = age_ns as f32 / 1_000_000_000.0;
        let recency = recency_ns as f32 / 1_000_000_000.0;
        
        let access_count = self.access_count as f32;

        // 简化的热度计算
        let frequency = access_count / (age + 1.0);
        let recency_score = 1.0 / (recency + 1.0);
        let age_factor = 1.0 / (age / 60.0 + 1.0);

        self.hotness_score = frequency * 0.4 + recency_score * 0.3 + age_factor * 0.3;
    }

    /// 计算价值评分
    pub fn calculate_value_score(&self) -> f32 {
        let benefit_cost_ratio = if self.compilation_cost > 0 {
            self.execution_benefit / self.compilation_cost as f32
        } else {
            0.0
        };

        // 综合热度、成本效益和大小
        let size_efficiency = (1000.0 / self.code_size as f32).min(1.0);

        self.hotness_score * 0.4 + benefit_cost_ratio * 0.4 + size_efficiency * 0.2
    }
}

/// 缓存统计信息
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// 总条目数
    pub total_entries: usize,
    /// 总大小（字节）
    pub total_size_bytes: usize,
    /// 命中次数
    pub hits: u64,
    /// 失误次数
    pub misses: u64,
    /// 淘汰次数
    pub evictions: u64,
    /// 命中率
    pub hit_rate: f64,
    /// 平均查找时间（纳秒）
    pub avg_lookup_time_ns: u64,
    /// 内存使用率
    pub memory_usage_ratio: f64,
}

/// 缓存配置
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// 最大条目数
    pub max_entries: usize,
    /// 最大内存大小（字节）
    pub max_memory_bytes: usize,
    /// 清理间隔（秒）
    pub cleanup_interval_secs: u64,
    /// 热度衰减因子
    pub hotness_decay_factor: f64,
    /// 预热大小
    pub warmup_size: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 10000,
            max_memory_bytes: 100 * 1024 * 1024,      // 100MB
            cleanup_interval_secs: 60,                // 1分钟
            hotness_decay_factor: 0.99,
            warmup_size: 1000,
        }
    }
}

/// 简化版统一代码缓存
pub struct UnifiedCodeCache {
    /// 主缓存
    main_cache: Arc<RwLock<HashMap<u64, CacheEntry>>>,
    /// LRU索引
    lru_index: Arc<RwLock<VecDeque<u64>>>,
    /// 配置
    config: CacheConfig,
    /// 统计信息
    stats: Arc<RwLock<CacheStats>>,
}

impl UnifiedCodeCache {
    /// 创建新的统一代码缓存
    pub fn new(config: CacheConfig) -> Self {
        Self {
            main_cache: Arc::new(RwLock::new(HashMap::new())),
            lru_index: Arc::new(RwLock::new(VecDeque::new())),
            config: config.clone(),
            stats: Arc::new(RwLock::new(CacheStats::default())),
        }
    }

    /// 查找代码
    pub fn lookup(&self, addr: u64) -> Option<*const u8> {
        let start_time = Instant::now();

        // 快速路径：主缓存查找
        if let Ok(mut cache) = self.main_cache.try_write() {
            if let Some(entry) = cache.get_mut(&addr) {
                // 更新访问信息
                entry.update_access();
                
                // 更新LRU索引
                self.update_lru_index(addr);
                
                // 更新统计信息
                self.update_hit_stats();
                
                let lookup_time = start_time.elapsed().as_nanos() as u64;
                self.update_lookup_time(lookup_time);
                
                return Some(entry.code_ptr);
            }
        }

        // 未命中
        let lookup_time = start_time.elapsed().as_nanos() as u64;
        self.update_lookup_time(lookup_time);
        
        // 更新未命中统计
        self.update_miss_stats();
        
        None
    }

    /// 插入代码
    pub fn insert(&self, addr: u64, code_ptr: *const u8, code_size: usize, compile_time_ns: u64) {
        let mut entry = CacheEntry::new(code_ptr, code_size);
        entry.compilation_cost = compile_time_ns;

        // 插入到主缓存
        if let Ok(mut cache) = self.main_cache.try_write() {
            // 检查是否需要淘汰（在写锁保护下）
            if cache.len() >= self.config.max_entries {
                drop(cache); // 释放写锁
                self.evict_approximate();
                cache = self.main_cache.try_write().unwrap(); // 重新获取写锁
            }
            
            // 再次检查容量，然后插入
            if cache.len() < self.config.max_entries {
                cache.insert(addr, entry);
            }
        }

        // 更新LRU索引
        self.update_lru_index(addr);

        // 更新统计信息
        self.update_insert_stats(code_size);
    }

    /// 更新LRU索引
    fn update_lru_index(&self, addr: u64) {
        if let Ok(mut lru) = self.lru_index.try_write() {
            // 简化的LRU更新：移除旧位置，添加到尾部
            if let Some(pos) = lru.iter().position(|&a| a == addr) {
                lru.remove(pos);
            }
            lru.push_back(addr);
        }
    }

    /// 更新命中统计
    fn update_hit_stats(&self) {
        if let Ok(mut stats) = self.stats.try_write() {
            stats.hits += 1;
            let total = stats.hits + stats.misses;
            stats.hit_rate = stats.hits as f64 / total as f64;
        }
    }

    /// 更新未命中统计
    fn update_miss_stats(&self) {
        if let Ok(mut stats) = self.stats.try_write() {
            stats.misses += 1;
            let total = stats.hits + stats.misses;
            stats.hit_rate = stats.hits as f64 / total as f64;
        }
    }

    /// 更新查找时间统计
    fn update_lookup_time(&self, time_ns: u64) {
        if let Ok(mut stats) = self.stats.try_write() {
            // 简化：使用指数移动平均
            stats.avg_lookup_time_ns = (stats.avg_lookup_time_ns * 7 + time_ns) / 8;
        }
    }

    /// 更新插入统计
    fn update_insert_stats(&self, code_size: usize) {
        if let Ok(mut stats) = self.stats.try_write() {
            if let Ok(cache) = self.main_cache.try_read() {
                stats.total_entries = cache.len();
                stats.total_size_bytes += code_size;
            }
        }
    }

    /// 简化的近似淘汰策略
    fn evict_approximate(&self) {
        // 采样淘汰：只检查LRU索引的前N个条目
        let lru_candidates = {
            if let Ok(lru) = self.lru_index.try_read() {
                lru.iter().take(10).copied().collect::<Vec<_>>()
            } else {
                return;
            }
        };

        // 简单评分：基于访问次数和年龄
        let mut candidates = Vec::new();
        for addr in &lru_candidates {
            if let Ok(cache) = self.main_cache.try_read() {
                if let Some(entry) = cache.get(&addr) {
                    // 简化的评分：访问次数越少，分数越高（越应该淘汰）
                    let score = 1.0 / (entry.access_count as f32 + 1.0);
                    candidates.push((*addr, score));
                }
            }
        }

        // 按评分排序，淘汰得分最高的20%
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let evict_count = (candidates.len() / 5).max(1);

        for (addr, _) in candidates.iter().take(evict_count) {
            if let Ok(mut cache) = self.main_cache.try_write() {
                cache.remove(addr);
            }
            
            // 从LRU索引中移除
            if let Ok(mut lru) = self.lru_index.try_write() {
                if let Some(pos) = lru.iter().position(|&a| a == *addr) {
                    lru.remove(pos);
                }
            }
        }

        // 更新淘汰统计
        if let Ok(mut stats) = self.stats.try_write() {
            stats.evictions += evict_count as u64;
        }
    }

    /// 获取统计信息
    pub fn stats(&self) -> CacheStats {
        self.stats.read().unwrap().clone()
    }

    /// 生成诊断报告
    pub fn diagnostic_report(&self) -> String {
        let stats = self.stats();
        let hot_entries = self.get_hot_entries(10);

        format!(
            r#"=== Unified Code Cache Report ===

Configuration:
  Max Entries: {}
  Max Memory: {}MB
  Cleanup Interval: {}s
  Hotness Decay Factor: {:.3}
  Warmup Size: {}

Statistics:
  Total Entries: {}
  Total Size: {}MB
  Hits: {}
  Misses: {}
  Hit Rate: {:.2}%
  Evictions: {}
  Avg Lookup Time: {}ns
  Memory Usage: {:.1}%

Top 10 Hot Entries:
{}
"#,
            self.config.max_entries,
            self.config.max_memory_bytes / (1024 * 1024),
            self.config.cleanup_interval_secs,
            self.config.hotness_decay_factor,
            self.config.warmup_size,
            stats.total_entries,
            stats.total_size_bytes / (1024 * 1024),
            stats.hits,
            stats.misses,
            stats.hit_rate * 100.0,
            stats.evictions,
            stats.avg_lookup_time_ns,
            stats.memory_usage_ratio * 100.0,
            hot_entries
                .iter()
                .map(|(addr, score)| format!("  0x{:x}: {:.2}", addr, score))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }

    /// 获取热点条目列表
    pub fn get_hot_entries(&self, limit: usize) -> Vec<(u64, f32)> {
        if let Ok(cache) = self.main_cache.try_read() {
            let mut hot_entries: Vec<(u64, f32)> = cache
                .iter()
                .map(|(&addr, entry)| (addr, entry.hotness_score))
                .collect();
            
            // 按热度评分降序排序
            hot_entries.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            hot_entries.into_iter().take(limit).collect()
        } else {
            Vec::new()
        }
    }
}

impl Default for UnifiedCodeCache {
    fn default() -> Self {
        Self::new(CacheConfig::default())
    }
}

impl Clone for UnifiedCodeCache {
    fn clone(&self) -> Self {
        Self {
            main_cache: self.main_cache.clone(),
            lru_index: self.lru_index.clone(),
            config: self.config.clone(),
            stats: self.stats.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_creation() {
        let config = CacheConfig::default();
        let cache = UnifiedCodeCache::new(config);
        
        let stats = cache.stats();
        assert_eq!(stats.total_entries, 0);
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
    }

    #[test]
    fn test_cache_insert_and_lookup() {
        let config = CacheConfig::default();
        let cache = UnifiedCodeCache::new(config);
        
        let code_ptr = vec![1u8, 2u8, 3u8].as_ptr();
        cache.insert(0x1000, code_ptr, 3, 1000);
        
        let found_ptr = cache.lookup(0x1000);
        assert_eq!(found_ptr, Some(code_ptr));
        
        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 0);
        assert_eq!(stats.total_entries, 1);
    }

    #[test]
    fn test_cache_miss() {
        let config = CacheConfig::default();
        let cache = UnifiedCodeCache::new(config);
        
        let found_ptr = cache.lookup(0x2000);
        assert_eq!(found_ptr, None);
        
        let stats = cache.stats();
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 1);
    }

    #[test]
    fn test_eviction() {
        let config = CacheConfig {
            max_entries: 2, // 小缓存，触发淘汰
            ..Default::default()
        };
        let cache = UnifiedCodeCache::new(config);
        
        // 插入超过容量的条目，触发淘汰
        let code_ptr1 = vec![1u8].as_ptr();
        let code_ptr2 = vec![2u8].as_ptr();
        let code_ptr3 = vec![3u8].as_ptr();
        
        cache.insert(0x1000, code_ptr1, 1, 1000);
        cache.insert(0x2000, code_ptr2, 1, 1000);
        cache.insert(0x3000, code_ptr3, 1, 1000);
        
        let stats = cache.stats();
        assert!(stats.total_entries <= 2);
        assert!(stats.evictions > 0);
    }

    #[test]
    fn test_hot_entries() {
        let config = CacheConfig::default();
        let cache = UnifiedCodeCache::new(config);
        
        // 插入一些条目
        for i in 0..10 {
            let code_ptr = vec![i as u8].as_ptr();
            cache.insert(i, code_ptr, 1, 1000);
        }
        
        let hot_entries = cache.get_hot_entries(5);
        assert_eq!(hot_entries.len(), 5);
    }
}