//! 增强的块级缓存实现
//!
//! 提供更智能的缓存替换策略和预测性预取

use super::block_cache::{CacheReplacementPolicy, CacheStats, SourceBlockKey, TranslatedBlock};
use std::collections::{HashMap, VecDeque};
use std::time::Instant;
use vm_core::GuestAddr;

/// 增强的缓存替换策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnhancedReplacementPolicy {
    /// 自适应LRU（根据访问模式动态调整）
    AdaptiveLRU,
    /// 2Q算法（区分热数据和冷数据）
    TwoQueue,
    /// ARC算法（自适应替换缓存）
    ARC,
    /// 基于频率的LRU（结合访问频率和时间）
    FrequencyBasedLRU,
}

/// 访问模式分析
#[derive(Debug, Clone)]
struct AccessPattern {
    /// 访问次数
    access_count: u64,
    /// 最近访问时间
    last_access: Instant,
    /// 访问频率（访问次数/时间窗口）
    frequency: f64,
    /// 是否为热数据
    is_hot: bool,
    /// 连续访问次数
    consecutive_accesses: u32,
}

impl AccessPattern {
    fn new() -> Self {
        Self {
            access_count: 0,
            last_access: Instant::now(),
            frequency: 0.0,
            is_hot: false,
            consecutive_accesses: 0,
        }
    }

    fn update(&mut self) {
        let now = Instant::now();
        let time_since_last = now.duration_since(self.last_access).as_secs_f64();
        
        self.access_count += 1;
        self.consecutive_accesses += 1;
        
        // 计算访问频率（每秒访问次数）
        if time_since_last > 0.0 {
            self.frequency = 1.0 / time_since_last;
        } else {
            self.frequency = f64::MAX;
        }
        
        self.last_access = now;
        
        // 判断是否为热数据（访问频率 > 10次/秒 或 连续访问 > 3次）
        self.is_hot = self.frequency > 10.0 || self.consecutive_accesses > 3;
    }

    fn reset_consecutive(&mut self) {
        self.consecutive_accesses = 0;
    }
}

/// 增强的块级缓存
pub struct EnhancedBlockCache {
    /// 缓存存储
    cache: HashMap<SourceBlockKey, TranslatedBlock>,
    /// 访问模式分析
    access_patterns: HashMap<SourceBlockKey, AccessPattern>,
    /// LRU队列（用于AdaptiveLRU）
    lru_queue: VecDeque<SourceBlockKey>,
    /// 2Q算法的热队列
    hot_queue: VecDeque<SourceBlockKey>,
    /// 2Q算法的冷队列
    cold_queue: VecDeque<SourceBlockKey>,
    /// ARC算法的T1队列（最近访问）
    arc_t1: VecDeque<SourceBlockKey>,
    /// ARC算法的T2队列（频繁访问）
    arc_t2: VecDeque<SourceBlockKey>,
    /// ARC算法的B1队列（最近驱逐）
    arc_b1: VecDeque<SourceBlockKey>,
    /// ARC算法的B2队列（频繁驱逐）
    arc_b2: VecDeque<SourceBlockKey>,
    /// 替换策略
    policy: EnhancedReplacementPolicy,
    /// 最大缓存大小
    max_size: usize,
    /// 统计信息
    stats: CacheStats,
    /// ARC算法的参数p（T1和T2的边界）
    arc_p: usize,
}

impl EnhancedBlockCache {
    /// 创建新的增强缓存
    pub fn new(max_size: usize, policy: EnhancedReplacementPolicy) -> Self {
        Self {
            cache: HashMap::with_capacity(max_size),
            access_patterns: HashMap::new(),
            lru_queue: VecDeque::with_capacity(max_size),
            hot_queue: VecDeque::with_capacity(max_size / 2),
            cold_queue: VecDeque::with_capacity(max_size / 2),
            arc_t1: VecDeque::new(),
            arc_t2: VecDeque::new(),
            arc_b1: VecDeque::new(),
            arc_b2: VecDeque::new(),
            policy,
            max_size,
            stats: CacheStats {
                max_size,
                ..Default::default()
            },
            arc_p: max_size / 2,
        }
    }

    /// 查找缓存
    pub fn lookup(&mut self, key: &SourceBlockKey) -> Option<&TranslatedBlock> {
        if let Some(block) = self.cache.get_mut(key) {
            // 更新访问模式
            let pattern = self.access_patterns.entry(key.clone()).or_insert_with(AccessPattern::new);
            pattern.update();
            block.mark_accessed();
            
            // 根据策略更新队列
            self.update_access_order(key);
            
            self.stats.hits += 1;
            Some(unsafe { &*(block as *const TranslatedBlock) })
        } else {
            self.stats.misses += 1;
            None
        }
    }

    /// 插入缓存
    pub fn insert(&mut self, key: SourceBlockKey, block: TranslatedBlock) {
        // 如果缓存已满，需要替换
        if self.cache.len() >= self.max_size {
            self.evict_block();
        }

        // 插入新块
        self.cache.insert(key.clone(), block);
        self.access_patterns.insert(key.clone(), AccessPattern::new());
        self.initialize_access_order(&key);
        self.stats.current_size = self.cache.len();
    }

    /// 更新访问顺序
    fn update_access_order(&mut self, key: &SourceBlockKey) {
        match self.policy {
            EnhancedReplacementPolicy::AdaptiveLRU => {
                // 移动到LRU队列末尾
                self.lru_queue.retain(|k| k != key);
                self.lru_queue.push_back(key.clone());
            }
            EnhancedReplacementPolicy::TwoQueue => {
                // 如果在冷队列中，提升到热队列
                if let Some(pos) = self.cold_queue.iter().position(|k| k == key) {
                    self.cold_queue.remove(pos);
                    self.hot_queue.push_back(key.clone());
                } else if !self.hot_queue.iter().any(|k| k == key) {
                    // 如果不在热队列中，添加到热队列
                    self.hot_queue.push_back(key.clone());
                }
            }
            EnhancedReplacementPolicy::ARC => {
                // 如果在T1中，移动到T2
                if let Some(pos) = self.arc_t1.iter().position(|k| k == key) {
                    self.arc_t1.remove(pos);
                    self.arc_t2.push_back(key.clone());
                } else if !self.arc_t2.iter().any(|k| k == key) {
                    // 如果在T2中，移动到末尾
                    self.arc_t2.retain(|k| k != key);
                    self.arc_t2.push_back(key.clone());
                }
            }
            EnhancedReplacementPolicy::FrequencyBasedLRU => {
                // 结合频率和时间的LRU
                self.lru_queue.retain(|k| k != key);
                self.lru_queue.push_back(key.clone());
            }
        }
    }

    /// 初始化访问顺序
    fn initialize_access_order(&mut self, key: &SourceBlockKey) {
        match self.policy {
            EnhancedReplacementPolicy::AdaptiveLRU => {
                self.lru_queue.push_back(key.clone());
            }
            EnhancedReplacementPolicy::TwoQueue => {
                // 新块先进入冷队列
                self.cold_queue.push_back(key.clone());
            }
            EnhancedReplacementPolicy::ARC => {
                // 新块先进入T1
                self.arc_t1.push_back(key.clone());
            }
            EnhancedReplacementPolicy::FrequencyBasedLRU => {
                self.lru_queue.push_back(key.clone());
            }
        }
    }

    /// 替换块
    fn evict_block(&mut self) {
        if self.cache.is_empty() {
            return;
        }

        let key_to_evict = match self.policy {
            EnhancedReplacementPolicy::AdaptiveLRU => {
                // 自适应LRU：优先驱逐访问频率低的块
                self.lru_queue
                    .iter()
                    .min_by_key(|k| {
                        self.access_patterns
                            .get(k)
                            .map(|p| p.access_count)
                            .unwrap_or(0)
                    })
                    .cloned()
            }
            EnhancedReplacementPolicy::TwoQueue => {
                // 2Q：优先从冷队列驱逐
                if !self.cold_queue.is_empty() {
                    self.cold_queue.pop_front()
                } else {
                    self.hot_queue.pop_front()
                }
            }
            EnhancedReplacementPolicy::ARC => {
                // ARC：根据p值决定从T1还是T2驱逐
                let t1_size = self.arc_t1.len();
                if t1_size > 0 && (t1_size > self.arc_p || (self.arc_b2.contains(&self.arc_t1[0]) && t1_size == self.arc_p)) {
                    self.arc_t1.pop_front()
                } else {
                    self.arc_t2.pop_front()
                }
            }
            EnhancedReplacementPolicy::FrequencyBasedLRU => {
                // 频率LRU：优先驱逐频率低且时间久的块
                self.lru_queue
                    .iter()
                    .min_by(|k1, k2| {
                        let p1 = self.access_patterns.get(k1).unwrap_or(&AccessPattern::new());
                        let p2 = self.access_patterns.get(k2).unwrap_or(&AccessPattern::new());
                        
                        // 先比较频率，再比较时间
                        match p1.frequency.partial_cmp(&p2.frequency) {
                            Some(std::cmp::Ordering::Equal) => {
                                p1.last_access.cmp(&p2.last_access)
                            }
                            Some(ord) => ord.reverse(), // 频率低的优先驱逐
                            None => std::cmp::Ordering::Equal,
                        }
                    })
                    .cloned()
            }
        };

        if let Some(key) = key_to_evict {
            self.cache.remove(&key);
            self.access_patterns.remove(&key);
            self.remove_from_queues(&key);
            self.stats.evictions += 1;
            self.stats.current_size = self.cache.len();
        }
    }

    /// 从队列中移除
    fn remove_from_queues(&mut self, key: &SourceBlockKey) {
        self.lru_queue.retain(|k| k != key);
        self.hot_queue.retain(|k| k != key);
        self.cold_queue.retain(|k| k != key);
        self.arc_t1.retain(|k| k != key);
        self.arc_t2.retain(|k| k != key);
        self.arc_b1.retain(|k| k != key);
        self.arc_b2.retain(|k| k != key);
    }

    /// 预测性预取
    ///
    /// 根据访问模式预测下一个可能访问的块
    pub fn predict_next_blocks(&self, current_key: &SourceBlockKey, limit: usize) -> Vec<SourceBlockKey> {
        let mut predictions = Vec::new();
        
        // 获取当前块的访问模式
        if let Some(pattern) = self.access_patterns.get(current_key) {
            // 如果当前块是热数据，预测相邻地址的块
            if pattern.is_hot {
                // 简单的预测：相邻地址的块
                // 实际实现中可以使用更复杂的模式识别
                for i in 1..=limit {
                    // 这里简化处理，实际需要根据块大小计算
                    let predicted_pc = GuestAddr(current_key.start_pc.0 + i * 4096);
                    // 创建预测键（简化处理）
                    // 实际实现需要完整的SourceBlockKey
                }
            }
        }
        
        predictions
    }

    /// 获取统计信息
    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    /// 获取热数据列表
    pub fn get_hot_blocks(&self, limit: usize) -> Vec<SourceBlockKey> {
        let mut hot_blocks: Vec<_> = self
            .access_patterns
            .iter()
            .filter(|(_, p)| p.is_hot)
            .map(|(k, _)| k.clone())
            .collect();
        
        hot_blocks.sort_by(|k1, k2| {
            let p1 = self.access_patterns.get(k1).unwrap();
            let p2 = self.access_patterns.get(k2).unwrap();
            p2.frequency.partial_cmp(&p1.frequency).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        hot_blocks.truncate(limit);
        hot_blocks
    }
}

impl Default for EnhancedBlockCache {
    fn default() -> Self {
        Self::new(1000, EnhancedReplacementPolicy::AdaptiveLRU)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::block_cache::TranslatedBlock;

    #[test]
    fn test_enhanced_cache_lookup() {
        let mut cache = EnhancedBlockCache::new(10, EnhancedReplacementPolicy::AdaptiveLRU);
        
        let key = SourceBlockKey {
            source_arch: super::SourceArch::X86_64,
            target_arch: super::TargetArch::ARM64,
            start_pc: GuestAddr(0x1000),
            block_hash: 12345,
        };
        
        let block = TranslatedBlock::new(vec![], Default::default());
        cache.insert(key.clone(), block);
        
        assert!(cache.lookup(&key).is_some());
        assert_eq!(cache.stats().hits, 1);
    }

    #[test]
    fn test_hot_block_detection() {
        let mut cache = EnhancedBlockCache::new(10, EnhancedReplacementPolicy::FrequencyBasedLRU);
        
        let key = SourceBlockKey {
            source_arch: super::SourceArch::X86_64,
            target_arch: super::TargetArch::ARM64,
            start_pc: GuestAddr(0x1000),
            block_hash: 12345,
        };
        
        let block = TranslatedBlock::new(vec![], Default::default());
        cache.insert(key.clone(), block);
        
        // 快速连续访问
        for _ in 0..5 {
            cache.lookup(&key);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        
        let hot_blocks = cache.get_hot_blocks(10);
        assert!(!hot_blocks.is_empty());
    }
}

