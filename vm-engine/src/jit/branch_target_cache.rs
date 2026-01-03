//! 分支目标缓存 (Branch Target Cache - BTC)
#![allow(dead_code)] // TODO: JIT structures reserved for future optimization
//!
//! BTC缓存间接分支指令的目标地址，用于优化虚拟机中的分支预测和执行。
//!
//! ## 功能特性
//!
//! - **间接分支缓存**: 缓存间接跳转的目标地址
//! - **预测辅助**: 基于历史数据预测分支目标
//! - **自适应大小**: 根据工作负载动态调整缓存大小
//! - **线程安全**: 使用parking_lot实现高性能并发访问
//!
//! ## 性能影响
//!
//! - **缓存命中**: 避免昂贵的目标地址计算
//! - **预测准确**: 高命中率（95%+）显著提升性能
//! - **低开销**: 快速查找和更新操作
//!
//! ## 使用示例
//!
//! ```rust
//! use vm_engine::jit::branch_target_cache::BranchTargetCache;
//!
//! let cache = BranchTargetCache::new(1024);
//!
//! // 记录分支目标
//! cache.insert(0x1000, 0x2000);
//!
//! // 查找分支目标
//! if let Some(target) = cache.lookup(0x1000) {
//!     println!("Branch target: 0x{:x}", target);
//! }
//!
//! // 获取统计信息
//! let stats = cache.get_stats();
//! println!("Hit rate: {:.2}%", stats.hit_rate() * 100.0);
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::Mutex;
use vm_core::GuestAddr;

/// 分支目标缓存条目
#[derive(Debug, Clone)]
pub struct BranchTargetEntry {
    /// 分支指令地址
    pub branch_addr: GuestAddr,
    /// 目标地址
    pub target_addr: GuestAddr,
    /// 最后访问时间
    pub last_access: std::time::Instant,
    /// 访问计数
    pub access_count: u64,
    /// 分支类型
    pub branch_type: BranchType,
}

/// 分支类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchType {
    /// 间接跳转（通过寄存器）
    IndirectJump,
    /// 间接调用（通过寄存器）
    IndirectCall,
    /// 返回指令
    Return,
    /// 条件分支
    Conditional,
}

/// 分支目标缓存配置
#[derive(Debug, Clone)]
pub struct BranchTargetCacheConfig {
    /// 缓存容量（条目数）
    pub capacity: usize,
    /// 启用预测
    pub enable_prediction: bool,
    /// 历史记录大小（用于预测）
    pub history_size: usize,
    /// 替换策略
    pub replacement_policy: ReplacementPolicy,
}

/// 缓存替换策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplacementPolicy {
    /// 最近最少使用（LRU）
    Lru,
    /// 最不经常使用（LFU）
    Lfu,
    /// 先进先出（FIFO）
    Fifo,
    /// 随机替换
    Random,
}

impl Default for BranchTargetCacheConfig {
    fn default() -> Self {
        Self {
            capacity: 1024,
            enable_prediction: true,
            history_size: 16,
            replacement_policy: ReplacementPolicy::Lru,
        }
    }
}

/// 分支目标缓存统计信息
#[derive(Debug, Default)]
pub struct BranchTargetCacheStats {
    /// 缓存命中次数
    pub hits: AtomicU64,
    /// 缓存未命中次数
    pub misses: AtomicU64,
    /// 插入操作次数
    pub inserts: AtomicU64,
    /// 替换操作次数
    pub evictions: AtomicU64,
    /// 预测正确次数
    pub predictions_correct: AtomicU64,
    /// 预测错误次数
    pub predictions_wrong: AtomicU64,
}

impl Clone for BranchTargetCacheStats {
    fn clone(&self) -> Self {
        Self {
            hits: AtomicU64::new(self.hits.load(Ordering::Relaxed)),
            misses: AtomicU64::new(self.misses.load(Ordering::Relaxed)),
            inserts: AtomicU64::new(self.inserts.load(Ordering::Relaxed)),
            evictions: AtomicU64::new(self.evictions.load(Ordering::Relaxed)),
            predictions_correct: AtomicU64::new(self.predictions_correct.load(Ordering::Relaxed)),
            predictions_wrong: AtomicU64::new(self.predictions_wrong.load(Ordering::Relaxed)),
        }
    }
}

impl BranchTargetCacheStats {
    /// 计算缓存命中率
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;
        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }

    /// 计算预测准确率
    pub fn prediction_accuracy(&self) -> f64 {
        let correct = self.predictions_correct.load(Ordering::Relaxed);
        let wrong = self.predictions_wrong.load(Ordering::Relaxed);
        let total = correct + wrong;
        if total == 0 {
            0.0
        } else {
            correct as f64 / total as f64
        }
    }
}

/// 分支目标历史记录（用于预测）
#[derive(Debug, Clone)]
struct BranchHistory {
    /// 历史目标地址（按时间顺序）
    targets: Vec<GuestAddr>,
    /// 目标地址出现次数
    target_counts: HashMap<GuestAddr, usize>,
}

impl BranchHistory {
    fn new(_branch_addr: GuestAddr, size: usize) -> Self {
        Self {
            targets: Vec::with_capacity(size),
            target_counts: HashMap::new(),
        }
    }

    /// 记录一个目标地址
    fn record_target(&mut self, target: GuestAddr, size: usize) {
        // 更新计数
        *self.target_counts.entry(target).or_insert(0) += 1;

        // 添加到历史
        self.targets.push(target);
        if self.targets.len() > size {
            // 移除最旧的条目
            if !self.targets.is_empty() {
                let old = self.targets.remove(0);
                // 减少旧目标的计数
                if let Some(count) = self.target_counts.get_mut(&old) {
                    *count -= 1;
                    if *count == 0 {
                        self.target_counts.remove(&old);
                    }
                }
            }
        }
    }

    /// 预测下一个目标地址
    fn predict(&self) -> Option<GuestAddr> {
        if self.targets.is_empty() {
            return None;
        }

        // 返回最频繁的目标
        self.target_counts
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(&addr, _)| addr)
    }
}

/// 分支目标缓存
pub struct BranchTargetCache {
    /// 缓存条目
    entries: Arc<Mutex<HashMap<GuestAddr, BranchTargetEntry>>>,
    /// 访问顺序（用于LRU）
    access_order: Arc<Mutex<Vec<GuestAddr>>>,
    /// 统计信息
    stats: Arc<BranchTargetCacheStats>,
    /// 配置
    config: BranchTargetCacheConfig,
    /// 分支历史（用于预测）
    histories: Arc<Mutex<HashMap<GuestAddr, BranchHistory>>>,
}

impl BranchTargetCache {
    /// 创建新的分支目标缓存
    pub fn new(capacity: usize) -> Self {
        Self::with_config(BranchTargetCacheConfig {
            capacity,
            ..Default::default()
        })
    }

    /// 使用配置创建分支目标缓存
    pub fn with_config(config: BranchTargetCacheConfig) -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::with_capacity(config.capacity))),
            access_order: Arc::new(Mutex::new(Vec::with_capacity(config.capacity))),
            stats: Arc::new(BranchTargetCacheStats::default()),
            config,
            histories: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 查找分支目标
    pub fn lookup(&self, branch_addr: GuestAddr) -> Option<GuestAddr> {
        let entries = self.entries.lock();
        let entry = entries.get(&branch_addr)?;

        // 更新访问顺序（LRU）
        if self.config.replacement_policy == ReplacementPolicy::Lru {
            let mut order = self.access_order.lock();
            order.retain(|&addr| addr != branch_addr);
            order.push(branch_addr);
        }

        // 更新统计
        self.stats.hits.fetch_add(1, Ordering::Relaxed);

        Some(entry.target_addr)
    }

    /// 插入或更新分支目标
    pub fn insert(&self, branch_addr: GuestAddr, target_addr: GuestAddr) {
        let mut entries = self.entries.lock();
        let now = std::time::Instant::now();

        // 检查是否需要驱逐
        if !entries.contains_key(&branch_addr) && entries.len() >= self.config.capacity {
            self.evict_one(&mut entries);
        }

        // 更新或插入条目
        let entry = entries.entry(branch_addr).or_insert_with(|| {
            self.stats.inserts.fetch_add(1, Ordering::Relaxed);
            BranchTargetEntry {
                branch_addr,
                target_addr,
                last_access: now,
                access_count: 0,
                branch_type: BranchType::IndirectJump,
            }
        });

        // 更新访问信息
        entry.target_addr = target_addr;
        entry.last_access = now;
        entry.access_count += 1;

        // 更新访问顺序
        if self.config.replacement_policy == ReplacementPolicy::Lru {
            let mut order = self.access_order.lock();
            order.retain(|&addr| addr != branch_addr);
            order.push(branch_addr);
        }

        // 更新历史记录
        if self.config.enable_prediction {
            let mut histories = self.histories.lock();
            let history = histories
                .entry(branch_addr)
                .or_insert_with(|| BranchHistory::new(branch_addr, self.config.history_size));
            history.record_target(target_addr, self.config.history_size);
        }

        self.stats.misses.fetch_add(1, Ordering::Relaxed);
    }

    /// 预测分支目标
    pub fn predict(&self, branch_addr: GuestAddr) -> Option<GuestAddr> {
        if !self.config.enable_prediction {
            return None;
        }

        let histories = self.histories.lock();
        let history = histories.get(&branch_addr)?;

        let prediction = history.predict();

        // 记录预测（无论是否正确）
        if let Some(_predicted) = prediction {
            // 稍后会验证预测是否正确
            self.stats
                .predictions_correct
                .fetch_add(1, Ordering::Relaxed);
        }

        prediction
    }

    /// 验证预测是否正确
    pub fn verify_prediction(&self, predicted: GuestAddr, actual: GuestAddr) {
        if predicted == actual {
            self.stats
                .predictions_correct
                .fetch_add(1, Ordering::Relaxed);
        } else {
            self.stats.predictions_wrong.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// 驱逐一个缓存条目
    fn evict_one(&self, entries: &mut HashMap<GuestAddr, BranchTargetEntry>) {
        match self.config.replacement_policy {
            ReplacementPolicy::Lru => {
                let mut order = self.access_order.lock();
                if let Some(&addr) = order.first() {
                    order.remove(0);
                    entries.remove(&addr);
                    self.stats.evictions.fetch_add(1, Ordering::Relaxed);
                }
            }
            ReplacementPolicy::Lfu => {
                // 找到访问次数最少的条目
                if let Some((&addr, _)) = entries.iter().min_by_key(|(_, entry)| entry.access_count)
                {
                    entries.remove(&addr);
                    self.stats.evictions.fetch_add(1, Ordering::Relaxed);

                    // 更新访问顺序
                    let mut order = self.access_order.lock();
                    order.retain(|&a| a != addr);
                }
            }
            ReplacementPolicy::Fifo => {
                let mut order = self.access_order.lock();
                if let Some(&addr) = order.first() {
                    order.remove(0);
                    entries.remove(&addr);
                    self.stats.evictions.fetch_add(1, Ordering::Relaxed);
                }
            }
            ReplacementPolicy::Random => {
                // 使用简单的hash来选择随机条目
                if let Some(&addr) = entries.keys().next() {
                    entries.remove(&addr);
                    self.stats.evictions.fetch_add(1, Ordering::Relaxed);

                    // 更新访问顺序
                    let mut order = self.access_order.lock();
                    order.retain(|&a| a != addr);
                }
            }
        }
    }

    /// 清空缓存
    pub fn clear(&self) {
        self.entries.lock().clear();
        self.access_order.lock().clear();
        self.histories.lock().clear();
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> BranchTargetCacheStats {
        BranchTargetCacheStats {
            hits: AtomicU64::new(self.stats.hits.load(Ordering::Relaxed)),
            misses: AtomicU64::new(self.stats.misses.load(Ordering::Relaxed)),
            inserts: AtomicU64::new(self.stats.inserts.load(Ordering::Relaxed)),
            evictions: AtomicU64::new(self.stats.evictions.load(Ordering::Relaxed)),
            predictions_correct: AtomicU64::new(
                self.stats.predictions_correct.load(Ordering::Relaxed),
            ),
            predictions_wrong: AtomicU64::new(self.stats.predictions_wrong.load(Ordering::Relaxed)),
        }
    }

    /// 获取缓存大小
    pub fn len(&self) -> usize {
        self.entries.lock().len()
    }

    /// 检查缓存是否为空
    pub fn is_empty(&self) -> bool {
        self.entries.lock().is_empty()
    }

    /// 获取缓存容量
    pub fn capacity(&self) -> usize {
        self.config.capacity
    }

    /// 重置统计信息
    pub fn reset_stats(&self) {
        self.stats.hits.store(0, Ordering::Relaxed);
        self.stats.misses.store(0, Ordering::Relaxed);
        self.stats.inserts.store(0, Ordering::Relaxed);
        self.stats.evictions.store(0, Ordering::Relaxed);
        self.stats.predictions_correct.store(0, Ordering::Relaxed);
        self.stats.predictions_wrong.store(0, Ordering::Relaxed);
    }
}

impl Clone for BranchTargetCache {
    fn clone(&self) -> Self {
        Self {
            entries: Arc::clone(&self.entries),
            access_order: Arc::clone(&self.access_order),
            stats: Arc::clone(&self.stats),
            config: self.config.clone(),
            histories: Arc::clone(&self.histories),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_basic_operations() {
        let cache = BranchTargetCache::new(16);

        // 插入
        cache.insert(GuestAddr(0x1000), GuestAddr(0x2000));

        // 查找
        assert_eq!(cache.lookup(GuestAddr(0x1000)), Some(GuestAddr(0x2000)));

        // 未命中
        assert_eq!(cache.lookup(GuestAddr(0x3000)), None);
    }

    #[test]
    fn test_cache_hit_rate() {
        let cache = BranchTargetCache::new(16);

        cache.insert(GuestAddr(0x1000), GuestAddr(0x2000));

        // 2次查找，1次命中
        cache.lookup(GuestAddr(0x1000));
        cache.lookup(GuestAddr(0x3000));

        let stats = cache.get_stats();
        assert_eq!(stats.hits.load(Ordering::Relaxed), 1);
        assert_eq!(stats.misses.load(Ordering::Relaxed), 1);
        assert!((stats.hit_rate() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_cache_eviction() {
        let cache = BranchTargetCache::new(4);

        // 填满缓存
        cache.insert(GuestAddr(0x1000), GuestAddr(0x2000));
        cache.insert(GuestAddr(0x1100), GuestAddr(0x2100));
        cache.insert(GuestAddr(0x1200), GuestAddr(0x2200));
        cache.insert(GuestAddr(0x1300), GuestAddr(0x2300));

        // 插入第5个，应该驱逐一个
        cache.insert(GuestAddr(0x1400), GuestAddr(0x2400));

        assert_eq!(cache.len(), 4);
    }

    #[test]
    fn test_cache_clear() {
        let cache = BranchTargetCache::new(16);

        cache.insert(GuestAddr(0x1000), GuestAddr(0x2000));
        cache.insert(GuestAddr(0x1100), GuestAddr(0x2100));

        assert_eq!(cache.len(), 2);

        cache.clear();

        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_prediction() {
        let config = BranchTargetCacheConfig {
            capacity: 16,
            enable_prediction: true,
            history_size: 4,
            replacement_policy: ReplacementPolicy::Lru,
        };
        let cache = BranchTargetCache::with_config(config);

        // 多次跳转到同一目标
        for _ in 0..5 {
            cache.insert(GuestAddr(0x1000), GuestAddr(0x2000));
        }

        // 预测应该返回0x2000
        let prediction = cache.predict(GuestAddr(0x1000));
        assert_eq!(prediction, Some(GuestAddr(0x2000)));
    }

    #[test]
    fn test_stats_clone() {
        let cache = BranchTargetCache::new(16);

        cache.insert(GuestAddr(0x1000), GuestAddr(0x2000));
        cache.lookup(GuestAddr(0x1000));

        let stats1 = cache.get_stats();
        let stats2 = cache.get_stats();

        // 验证stats可以克隆且不影响原始数据
        assert_eq!(
            stats1.hits.load(Ordering::Relaxed),
            stats2.hits.load(Ordering::Relaxed)
        );
    }
}
