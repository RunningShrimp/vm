//! 缓存优化器
//!
//! 提供智能缓存策略以提高跨架构执行性能

use std::collections::{HashMap, VecDeque};

use std::time::{Duration, Instant};
use vm_core::GuestAddr;

/// 缓存条目
#[derive(Debug, Clone)]
struct CacheEntry {
    /// 编译后的代码
    code: Vec<u8>,
    /// 访问次数
    access_count: u64,
    /// 最后访问时间
    last_access: Instant,
}

/// 缓存策略
#[derive(Debug, Clone, Copy)]
pub enum CachePolicy {
    /// LRU（最近最少使用）
    LRU,
    /// LFU（最不经常使用）
    LFU,
    /// FIFO（先进先出）
    FIFO,
    /// 自适应（根据访问模式自动选择）
    Adaptive,
}

/// 缓存优化器配置
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// 最大缓存大小（字节）
    pub max_size: usize,
    /// 缓存策略
    pub policy: CachePolicy,
    /// 启用预取
    pub enable_prefetch: bool,
    /// 预取阈值（访问次数）
    pub prefetch_threshold: u64,
    /// 启用分层缓存
    pub enable_tiered_cache: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_size: 64 * 1024 * 1024, // 64MB
            policy: CachePolicy::Adaptive,
            enable_prefetch: true,
            prefetch_threshold: 10,
            enable_tiered_cache: true,
        }
    }
}

/// 缓存优化器
pub struct CacheOptimizer {
    config: CacheConfig,
    /// 热缓存（频繁访问的代码）
    hot_cache: HashMap<GuestAddr, CacheEntry>,
    /// 冷缓存（偶尔访问的代码）
    cold_cache: HashMap<GuestAddr, CacheEntry>,
    /// LRU队列（用于LRU策略）
    lru_queue: VecDeque<GuestAddr>,
    /// 当前缓存大小
    current_size: usize,
    /// 缓存统计
    stats: CacheStats,
}

/// 缓存统计
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// 缓存命中次数
    pub hits: u64,
    /// 缓存未命中次数
    pub misses: u64,
    /// 缓存驱逐次数
    pub evictions: u64,
    /// 预取次数
    pub prefetches: u64,
}

impl CacheOptimizer {
    /// 创建新的缓存优化器
    pub fn new(config: CacheConfig) -> Self {
        Self {
            config,
            hot_cache: HashMap::new(),
            cold_cache: HashMap::new(),
            lru_queue: VecDeque::new(),
            current_size: 0,
            stats: CacheStats::default(),
        }
    }

    /// 获取缓存的代码
    pub fn get(&mut self, pc: GuestAddr) -> Option<Vec<u8>> {
        // 先检查热缓存
        if let Some(code) = {
            if let Some(entry) = self.hot_cache.get_mut(&pc) {
                entry.access_count += 1;
                entry.last_access = Instant::now();
                self.stats.hits += 1;
                Some(entry.code.clone())
            } else {
                None
            }
        } {
            self.update_lru(pc);
            return Some(code);
        }

        // 再检查冷缓存
        if let Some((code, should_promote, cloned_entry)) = {
            if let Some(entry) = self.cold_cache.get_mut(&pc) {
                entry.access_count += 1;
                entry.last_access = Instant::now();
                self.stats.hits += 1;
                let promote = entry.access_count >= self.config.prefetch_threshold;
                let cloned = if promote { Some(entry.clone()) } else { None };
                Some((entry.code.clone(), promote, cloned))
            } else {
                None
            }
        } {
            if should_promote && let Some(e) = cloned_entry {
                self.promote_to_hot(pc, e);
            }
            return Some(code);
        }

        self.stats.misses += 1;
        None
    }

    /// 插入缓存
    pub fn insert(&mut self, pc: GuestAddr, code: Vec<u8>, _compile_time: Duration) {
        let code_size = code.len();

        // 检查是否需要驱逐
        while self.current_size + code_size > self.config.max_size {
            self.evict();
        }

        let entry = CacheEntry {
            code: code.clone(),
            access_count: 1,
            last_access: Instant::now(),
        };

        // 根据策略选择缓存位置
        match self.config.policy {
            CachePolicy::LRU | CachePolicy::Adaptive => {
                // 新条目先放入冷缓存
                self.cold_cache.insert(pc, entry);
            }
            _ => {
                self.hot_cache.insert(pc, entry);
            }
        }

        self.current_size += code_size;
        self.update_lru(pc);
    }

    /// 更新LRU队列
    fn update_lru(&mut self, pc: GuestAddr) {
        // 移除旧位置
        self.lru_queue.retain(|&x| x != pc);
        // 添加到队尾（最近使用）
        self.lru_queue.push_back(pc);
    }

    /// 驱逐缓存条目
    fn evict(&mut self) {
        match self.config.policy {
            CachePolicy::LRU => {
                // 驱逐LRU队列头部的条目
                if let Some(pc) = self.lru_queue.pop_front() {
                    if let Some(entry) = self.hot_cache.remove(&pc) {
                        self.current_size -= entry.code.len();
                        self.stats.evictions += 1;
                    } else if let Some(entry) = self.cold_cache.remove(&pc) {
                        self.current_size -= entry.code.len();
                        self.stats.evictions += 1;
                    }
                }
            }
            CachePolicy::LFU => {
                // 驱逐访问次数最少的条目
                let mut min_access = u64::MAX;
                let mut evict_pc = None;

                for (&pc, entry) in &self.cold_cache {
                    if entry.access_count < min_access {
                        min_access = entry.access_count;
                        evict_pc = Some(pc);
                    }
                }

                if let Some(pc) = evict_pc
                    && let Some(entry) = self.cold_cache.remove(&pc)
                {
                    self.current_size -= entry.code.len();
                    self.stats.evictions += 1;
                }
            }
            CachePolicy::FIFO => {
                // 驱逐最早插入的条目
                if let Some(pc) = self.lru_queue.pop_front()
                    && let Some(entry) = self.cold_cache.remove(&pc)
                {
                    self.current_size -= entry.code.len();
                    self.stats.evictions += 1;
                }
            }
            CachePolicy::Adaptive => {
                // 自适应策略：优先驱逐冷缓存中的条目
                if let Some(pc) = self.lru_queue.pop_front() {
                    if let Some(entry) = self.cold_cache.remove(&pc) {
                        self.current_size -= entry.code.len();
                        self.stats.evictions += 1;
                    } else if let Some(entry) = self.hot_cache.remove(&pc) {
                        self.current_size -= entry.code.len();
                        self.stats.evictions += 1;
                    }
                }
            }
        }
    }

    /// 提升到热缓存
    fn promote_to_hot(&mut self, pc: GuestAddr, entry: CacheEntry) {
        self.cold_cache.remove(&pc);
        self.hot_cache.insert(pc, entry);
    }

    /// 预取代码（基于访问模式）
    pub fn prefetch(&mut self, _pc: GuestAddr, next_pcs: &[GuestAddr]) {
        if !self.config.enable_prefetch {
            return;
        }

        for &next_pc in next_pcs {
            // 如果未缓存且访问次数达到阈值，预取
            if !self.hot_cache.contains_key(&next_pc) && !self.cold_cache.contains_key(&next_pc) {
                self.stats.prefetches += 1;
                // 实际实现中需要编译代码
            }
        }
    }

    /// 获取缓存统计
    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    /// 获取缓存命中率
    pub fn hit_rate(&self) -> f64 {
        let total = self.stats.hits + self.stats.misses;
        if total == 0 {
            0.0
        } else {
            self.stats.hits as f64 / total as f64
        }
    }

    /// 清空缓存
    pub fn clear(&mut self) {
        self.hot_cache.clear();
        self.cold_cache.clear();
        self.lru_queue.clear();
        self.current_size = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_optimizer() {
        let config = CacheConfig::default();
        let mut optimizer = CacheOptimizer::new(config);

        let code = vec![0x90, 0xC3]; // NOP, RET
        optimizer.insert(
            vm_core::GuestAddr(0x1000),
            code.clone(),
            Duration::from_millis(1),
        );

        assert!(optimizer.get(vm_core::GuestAddr(0x1000)).is_some());
        assert!(optimizer.get(vm_core::GuestAddr(0x2000)).is_none());
    }
}
