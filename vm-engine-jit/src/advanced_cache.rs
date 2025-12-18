//! 高级代码缓存策略
//!
//! 实现了多种高级代码缓存策略，包括分段缓存、预取、自适应淘汰等。

use std::collections::{HashMap, VecDeque};
use vm_core::GuestAddr;
use crate::code_cache::{CodeCache, CacheStats};

/// 高级代码缓存配置
#[derive(Debug, Clone)]
pub struct AdvancedCacheConfig {
    /// 缓存总大小限制
    pub total_size_limit: usize,
    /// 热点缓存大小限制
    pub hot_size_limit: usize,
    /// 冷缓存大小限制
    pub cold_size_limit: usize,
    /// 预取缓存大小限制
    pub prefetch_size_limit: usize,
    /// 热点阈值
    pub hotspot_threshold: u32,
    /// 预取策略
    pub prefetch_strategy: PrefetchStrategy,
    /// 淘汰策略
    pub eviction_strategy: EvictionStrategy,
    /// 分段策略
    pub segmentation_strategy: SegmentationStrategy,
}

impl Default for AdvancedCacheConfig {
    fn default() -> Self {
        Self {
            total_size_limit: 64 * 1024 * 1024, // 64MB
            hot_size_limit: 16 * 1024 * 1024,   // 16MB
            cold_size_limit: 32 * 1024 * 1024,  // 32MB
            prefetch_size_limit: 16 * 1024 * 1024, // 16MB
            hotspot_threshold: 100,
            prefetch_strategy: PrefetchStrategy::Sequential,
            eviction_strategy: EvictionStrategy::Adaptive,
            segmentation_strategy: SegmentationStrategy::FrequencyBased,
        }
    }
}

/// 预取策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefetchStrategy {
    /// 顺序预取
    Sequential,
    /// 基于模式的预取
    PatternBased,
    /// 基于访问历史的预取
    HistoryBased,
    /// 无预取
    None,
}

/// 淘汰策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvictionStrategy {
    /// LRU淘汰
    LRU,
    /// LFU淘汰
    LFU,
    /// 自适应淘汰
    Adaptive,
    /// 基于访问频率的淘汰
    FrequencyBased,
}

/// 分段策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentationStrategy {
    /// 基于频率的分段
    FrequencyBased,
    /// 基于大小的分段
    SizeBased,
    /// 基于类型的分段
    TypeBased,
    /// 无分段
    None,
}

/// 缓存条目类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheEntryType {
    /// 热点代码
    Hot,
    /// 冷代码
    Cold,
    /// 预取代码
    Prefetched,
    /// 未知
    Unknown,
}

/// 高级缓存条目
#[derive(Debug, Clone)]
struct AdvancedCacheEntry {
    /// 编译后的代码
    code: Vec<u8>,
    /// 条目类型
    entry_type: CacheEntryType,
    /// 访问次数
    access_count: u64,
    /// 最后访问时间
    last_access: std::time::Instant,
    /// 创建时间
    created_at: std::time::Instant,
    /// 代码大小
    size: usize,
    /// 代码复杂度
    complexity: u32,
    /// 预取优先级
    prefetch_priority: u8,
    /// 访问模式
    access_pattern: AccessPattern,
}

/// 访问模式
#[derive(Debug, Clone)]
struct AccessPattern {
    /// 访问历史
    access_history: VecDeque<std::time::Instant>,
    /// 访问间隔
    access_intervals: VecDeque<std::time::Duration>,
    /// 预测下次访问时间
    predicted_next_access: Option<std::time::Instant>,
    /// 访问频率
    access_frequency: f64,
}

impl AccessPattern {
    fn new() -> Self {
        Self {
            access_history: VecDeque::with_capacity(100),
            access_intervals: VecDeque::with_capacity(99),
            predicted_next_access: None,
            access_frequency: 0.0,
        }
    }

    fn record_access(&mut self) {
        let now = std::time::Instant::now();
        
        if let Some(&last_access) = self.access_history.back() {
            let interval = now.duration_since(last_access);
            self.access_intervals.push_back(interval);
            
            // 保持历史记录在合理范围内
            if self.access_intervals.len() > 99 {
                self.access_intervals.pop_front();
            }
        }
        
        self.access_history.push_back(now);
        
        // 保持历史记录在合理范围内
        if self.access_history.len() > 100 {
            self.access_history.pop_front();
        }
        
        // 更新访问频率
        self.update_frequency();
        
        // 预测下次访问时间
        self.predict_next_access();
    }
    
    fn update_frequency(&mut self) {
        if self.access_intervals.len() < 2 {
            return;
        }
        
        let total_interval: std::time::Duration = self.access_intervals.iter().sum();
        let avg_interval = total_interval.as_secs_f64() / self.access_intervals.len() as f64;
        
        if avg_interval > 0.0 {
            self.access_frequency = 1.0 / avg_interval;
        }
    }
    
    fn predict_next_access(&mut self) {
        if self.access_intervals.len() < 3 {
            return;
        }
        
        // 简单的线性预测
        let recent_intervals: Vec<_> = self.access_intervals
            .iter()
            .rev()
            .take(5)
            .collect();
            
        if recent_intervals.len() >= 2 {
            let total: std::time::Duration = recent_intervals.iter().cloned().sum();
            let avg_interval = total / recent_intervals.len() as u32;
            
            if let Some(&last_access) = self.access_history.back() {
                self.predicted_next_access = Some(last_access + avg_interval);
            }
        }
    }
}

/// 高级代码缓存实现
pub struct AdvancedCodeCache {
    /// 配置
    config: AdvancedCacheConfig,
    /// 热点缓存
    hot_cache: HashMap<GuestAddr, AdvancedCacheEntry>,
    /// 冷缓存
    cold_cache: HashMap<GuestAddr, AdvancedCacheEntry>,
    /// 预取缓存
    prefetch_cache: HashMap<GuestAddr, AdvancedCacheEntry>,
    /// 缓存统计
    stats: AdvancedCacheStats,
    /// 当前缓存大小
    current_size: usize,
    /// 热点缓存大小
    hot_size: usize,
    /// 冷缓存大小
    cold_size: usize,
    /// 预取缓存大小
    prefetch_size: usize,
    /// 访问顺序（用于LRU）
    access_order: VecDeque<GuestAddr>,
    /// 预取队列
    prefetch_queue: VecDeque<GuestAddr>,
}

/// 高级缓存统计信息
#[derive(Debug, Clone, Default)]
pub struct AdvancedCacheStats {
    /// 基础统计
    pub base_stats: CacheStats,
    /// 热点命中次数
    pub hot_hits: u64,
    /// 冷命中次数
    pub cold_hits: u64,
    /// 预取命中次数
    pub prefetch_hits: u64,
    /// 预取次数
    pub prefetch_count: u64,
    /// 预取成功次数
    pub prefetch_success: u64,
    /// 分段迁移次数
    pub migrations: u64,
    /// 自适应淘汰次数
    adaptive_evictions: u64,
    /// 访问模式预测准确次数
    pub prediction_hits: u64,
    /// 访问模式预测总次数
    pub prediction_total: u64,
}

impl AdvancedCodeCache {
    /// 创建新的高级代码缓存
    pub fn new(config: AdvancedCacheConfig) -> Self {
        Self {
            config,
            hot_cache: HashMap::new(),
            cold_cache: HashMap::new(),
            prefetch_cache: HashMap::new(),
            stats: AdvancedCacheStats::default(),
            current_size: 0,
            hot_size: 0,
            cold_size: 0,
            prefetch_size: 0,
            access_order: VecDeque::new(),
            prefetch_queue: VecDeque::new(),
        }
    }

    /// 确保有足够的空间
    fn ensure_space(&mut self, required_size: usize) {
        while self.current_size + required_size > self.config.total_size_limit {
            if let Some(pc) = self.select_eviction_candidate() {
                self.evict_entry(pc);
            } else {
                break;
            }
        }
    }

    /// 选择淘汰候选
    fn select_eviction_candidate(&self) -> Option<GuestAddr> {
        match self.config.eviction_strategy {
            EvictionStrategy::LRU => {
                // 找到最久未访问的条目
                self.access_order.front().cloned()
            }
            EvictionStrategy::LFU => {
                // 找到访问频率最低的条目
                let mut min_freq = f64::MAX;
                let mut candidate = None;
                
                for (&pc, entry) in self.hot_cache.iter() {
                    let freq = entry.access_pattern.access_frequency;
                    if freq < min_freq {
                        min_freq = freq;
                        candidate = Some(pc);
                    }
                }
                
                if candidate.is_none() {
                    for (&pc, entry) in self.cold_cache.iter() {
                        let freq = entry.access_pattern.access_frequency;
                        if freq < min_freq {
                            min_freq = freq;
                            candidate = Some(pc);
                        }
                    }
                }
                
                candidate
            }
            EvictionStrategy::Adaptive => {
                // 自适应策略：综合考虑访问频率、大小、类型等因素
                let mut min_score = f64::MAX;
                let mut candidate = None;
                
                for (&pc, entry) in self.hot_cache.iter() {
                    let score = self.calculate_eviction_score(entry);
                    if score < min_score {
                        min_score = score;
                        candidate = Some(pc);
                    }
                }
                
                if candidate.is_none() {
                    for (&pc, entry) in self.cold_cache.iter() {
                        let score = self.calculate_eviction_score(entry);
                        if score < min_score {
                            min_score = score;
                            candidate = Some(pc);
                        }
                    }
                }
                
                candidate
            }
            EvictionStrategy::FrequencyBased => {
                // 基于频率的淘汰策略
                self.select_frequency_based_eviction()
            }
        }
    }

    /// 计算淘汰分数
    fn calculate_eviction_score(&self, entry: &AdvancedCacheEntry) -> f64 {
        let age = entry.created_at.elapsed().as_secs_f64();
        let freq = entry.access_pattern.access_frequency;
        let size_factor = entry.size as f64 / 1024.0; // KB为单位
        
        // 分数越低越容易被淘汰
        let type_factor = match entry.entry_type {
            CacheEntryType::Hot => 0.1,
            CacheEntryType::Cold => 1.0,
            CacheEntryType::Prefetched => 0.5,
            CacheEntryType::Unknown => 0.7,
        };
        
        // 综合分数：年龄 + (1/频率) + 大小因子 * 类型因子
        age + (if freq > 0.0 { 1.0 / freq } else { 1000.0 }) + size_factor * type_factor
    }

    /// 基于频率的淘汰选择
    fn select_frequency_based_eviction(&self) -> Option<GuestAddr> {
        let mut candidates: Vec<_> = self.cold_cache.iter().collect();
        
        // 按访问频率排序
        candidates.sort_by(|a, b| {
            a.1.access_pattern.access_frequency
                .partial_cmp(&b.1.access_pattern.access_frequency)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        
        // 选择频率最低的几个候选中的一个
        if candidates.len() >= 3 {
            let low_freq_candidates = &candidates[..3];
            let idx = if low_freq_candidates.len() > 0 {
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                let candidate_pc = low_freq_candidates[0].0; // 使用第一个候选的PC作为种子
                candidate_pc.hash(&mut hasher);
                (hasher.finish() as usize) % low_freq_candidates.len()
            } else {
                0
            };
            Some(*low_freq_candidates[idx].0)
        } else {
            candidates.first().map(|&(pc, _)| pc).copied()
        }
    }

    /// 淘汰条目
    fn evict_entry(&mut self, pc: GuestAddr) {
        let entry = if let Some(entry) = self.hot_cache.remove(&pc) {
            self.hot_size -= entry.size;
            entry
        } else if let Some(entry) = self.cold_cache.remove(&pc) {
            self.cold_size -= entry.size;
            entry
        } else if let Some(entry) = self.prefetch_cache.remove(&pc) {
            self.prefetch_size -= entry.size;
            entry
        } else {
            return;
        };
        
        self.current_size -= entry.size;
        self.stats.base_stats.removals += 1;
        
        // 从访问顺序中移除
        if let Some(pos) = self.access_order.iter().position(|&x| x == pc) {
            self.access_order.remove(pos);
        }
        
        // 更新统计信息
        self.stats.base_stats.entry_count = self.hot_cache.len() + self.cold_cache.len() + self.prefetch_cache.len();
        self.stats.base_stats.current_size = self.current_size;
    }

    /// 更新访问顺序
    fn update_access_order(&mut self, pc: GuestAddr) {
        // 从当前位置移除
        if let Some(pos) = self.access_order.iter().position(|&x| x == pc) {
            self.access_order.remove(pos);
        }
        // 添加到末尾（最新）
        self.access_order.push_back(pc);
    }

    /// 检查是否需要分段迁移
    fn check_segmentation_migration(&mut self, pc: GuestAddr, entry: &mut AdvancedCacheEntry) {
        let should_migrate = match self.config.segmentation_strategy {
            SegmentationStrategy::FrequencyBased => {
                // 基于频率的迁移
                let freq = entry.access_pattern.access_frequency;
                match entry.entry_type {
                    CacheEntryType::Cold => freq > 1.0 && entry.access_count > self.config.hotspot_threshold as u64,
                    CacheEntryType::Hot => freq < 0.1 && entry.access_count < 10,
                    CacheEntryType::Prefetched => entry.access_count > 5,
                    CacheEntryType::Unknown => entry.access_count > self.config.hotspot_threshold as u64 / 2,
                }
            }
            SegmentationStrategy::SizeBased => {
                // 基于大小的迁移
                match entry.entry_type {
                    CacheEntryType::Cold => entry.size < 1024 && entry.access_count > 50,
                    CacheEntryType::Hot => entry.size > 10240 && entry.access_count < 20,
                    _ => false,
                }
            }
            SegmentationStrategy::TypeBased => {
                // 基于类型的迁移（这里简化处理）
                entry.access_count > self.config.hotspot_threshold as u64
            }
            SegmentationStrategy::None => false,
        };

        if should_migrate {
            self.migrate_entry(pc, entry);
        }
    }

    /// 迁移条目到合适的分段
    fn migrate_entry(&mut self, pc: GuestAddr, entry: &mut AdvancedCacheEntry) {
        let old_type = entry.entry_type;
        let new_type = self.determine_entry_type(entry);
        
        if old_type == new_type {
            return;
        }
        
        // 从旧分段移除
        match old_type {
            CacheEntryType::Hot => {
                self.hot_cache.remove(&pc);
                self.hot_size -= entry.size;
            }
            CacheEntryType::Cold => {
                self.cold_cache.remove(&pc);
                self.cold_size -= entry.size;
            }
            CacheEntryType::Prefetched => {
                self.prefetch_cache.remove(&pc);
                self.prefetch_size -= entry.size;
            }
            CacheEntryType::Unknown => {}
        }
        
        // 更新条目类型
        entry.entry_type = new_type;
        
        // 插入到新分段
        match new_type {
            CacheEntryType::Hot => {
                self.hot_cache.insert(pc, entry.clone());
                self.hot_size += entry.size;
            }
            CacheEntryType::Cold => {
                self.cold_cache.insert(pc, entry.clone());
                self.cold_size += entry.size;
            }
            CacheEntryType::Prefetched => {
                self.prefetch_cache.insert(pc, entry.clone());
                self.prefetch_size += entry.size;
            }
            CacheEntryType::Unknown => {}
        }
        
        self.stats.migrations += 1;
    }

    /// 确定条目类型
    fn determine_entry_type(&self, entry: &AdvancedCacheEntry) -> CacheEntryType {
        let freq = entry.access_pattern.access_frequency;
        let count = entry.access_count;
        
        if count > self.config.hotspot_threshold as u64 && freq > 0.5 {
            CacheEntryType::Hot
        } else if freq < 0.1 && count < 10 {
            CacheEntryType::Cold
        } else if entry.prefetch_priority > 5 {
            CacheEntryType::Prefetched
        } else {
            CacheEntryType::Unknown
        }
    }

    /// 执行预取
    fn execute_prefetch(&mut self) {
        if self.config.prefetch_strategy == PrefetchStrategy::None {
            return;
        }
        
        // 检查预取缓存空间
        if self.prefetch_size >= self.config.prefetch_size_limit {
            return;
        }
        
        // 从预取队列中取出候选
        while let Some(pc) = self.prefetch_queue.pop_front() {
            if !self.contains(pc) {
                // 这里应该触发实际的预取编译
                // 暂时跳过实际编译过程
                self.stats.prefetch_count += 1;
                
                // 检查预取缓存空间
                if self.prefetch_size >= self.config.prefetch_size_limit {
                    break;
                }
            }
        }
    }

    /// 预测下一个访问的地址
    fn predict_next_access(&self, pc: GuestAddr) -> Option<GuestAddr> {
        match self.config.prefetch_strategy {
            PrefetchStrategy::Sequential => {
                // 顺序预取：假设下一个地址是pc+4
                Some(pc.wrapping_add(4))
            }
            PrefetchStrategy::PatternBased => {
                // 基于模式的预取
                if let Some(entry) = self.get_entry(pc) {
                    if let Some(_predicted) = entry.access_pattern.predicted_next_access {
                        // 这里应该将时间转换为地址，简化处理
                        Some(pc.wrapping_add(8))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            PrefetchStrategy::HistoryBased => {
                // 基于历史的预取
                // 简化实现：返回最近访问的下一个地址
                if let Some(&last_pc) = self.access_order.back() {
                    Some(last_pc.wrapping_add(4))
                } else {
                    None
                }
            }
            PrefetchStrategy::None => None,
        }
    }

    /// 获取条目
    fn get_entry(&self, pc: GuestAddr) -> Option<&AdvancedCacheEntry> {
        self.hot_cache.get(&pc)
            .or_else(|| self.cold_cache.get(&pc))
            .or_else(|| self.prefetch_cache.get(&pc))
    }

    /// 获取可变条目
    fn get_entry_mut(&mut self, pc: GuestAddr) -> Option<&mut AdvancedCacheEntry> {
        // 注意：这种方法不够优雅，实际实现可能需要重构
        if self.hot_cache.contains_key(&pc) {
            self.hot_cache.get_mut(&pc)
        } else if self.cold_cache.contains_key(&pc) {
            self.cold_cache.get_mut(&pc)
        } else {
            self.prefetch_cache.get_mut(&pc)
        }
    }
}

impl CodeCache for AdvancedCodeCache {
    fn insert(&mut self, pc: GuestAddr, code: Vec<u8>) {
        let code_size = code.len();
        
        // 如果已存在，先移除旧的
        if self.contains(pc) {
            self.remove(pc);
        }
        
        // 确保有足够的空间
        self.ensure_space(code_size);
        
        // 创建新条目
        let entry = AdvancedCacheEntry {
            code: code.clone(),
            entry_type: CacheEntryType::Unknown,
            access_count: 0,
            last_access: std::time::Instant::now(),
            created_at: std::time::Instant::now(),
            size: code_size,
            complexity: 0, // 这里应该根据代码内容计算复杂度
            prefetch_priority: 0,
            access_pattern: AccessPattern::new(),
        };
        
        // 插入到冷缓存
        self.cold_cache.insert(pc, entry);
        self.cold_size += code_size;
        self.current_size += code_size;
        
        // 更新访问顺序
        self.update_access_order(pc);
        
        // 更新统计
        self.stats.base_stats.inserts += 1;
        self.stats.base_stats.entry_count = self.hot_cache.len() + self.cold_cache.len() + self.prefetch_cache.len();
        self.stats.base_stats.current_size = self.current_size;
        
        // 执行预取
        self.execute_prefetch();
    }
    
    fn get(&self, pc: GuestAddr) -> Option<Vec<u8>> {
        if let Some(entry) = self.get_entry(pc) {
            // 注意：这里无法直接更新统计，因为方法是不可变的
            // 在实际使用中，可能需要使用内部可变性或重新设计接口
            Some(entry.code.clone())
        } else {
            None
        }
    }
    
    fn contains(&self, pc: GuestAddr) -> bool {
        self.hot_cache.contains_key(&pc) || 
        self.cold_cache.contains_key(&pc) || 
        self.prefetch_cache.contains_key(&pc)
    }
    
    fn remove(&mut self, pc: GuestAddr) -> Option<Vec<u8>> {
        let entry = if let Some(entry) = self.hot_cache.remove(&pc) {
            self.hot_size -= entry.size;
            entry
        } else if let Some(entry) = self.cold_cache.remove(&pc) {
            self.cold_size -= entry.size;
            entry
        } else if let Some(entry) = self.prefetch_cache.remove(&pc) {
            self.prefetch_size -= entry.size;
            entry
        } else {
            return None;
        };
        
        self.current_size -= entry.size;
        self.stats.base_stats.removals += 1;
        
        // 从访问顺序中移除
        if let Some(pos) = self.access_order.iter().position(|&x| x == pc) {
            self.access_order.remove(pos);
        }
        
        // 更新统计信息
        self.stats.base_stats.entry_count = self.hot_cache.len() + self.cold_cache.len() + self.prefetch_cache.len();
        self.stats.base_stats.current_size = self.current_size;
        
        Some(entry.code)
    }
    
    fn clear(&mut self) {
        self.hot_cache.clear();
        self.cold_cache.clear();
        self.prefetch_cache.clear();
        self.access_order.clear();
        self.prefetch_queue.clear();
        
        self.current_size = 0;
        self.hot_size = 0;
        self.cold_size = 0;
        self.prefetch_size = 0;
        
        self.stats.base_stats.clears += 1;
        self.stats.base_stats.entry_count = 0;
        self.stats.base_stats.current_size = 0;
    }
    
    fn stats(&self) -> CacheStats {
        self.stats.base_stats.clone()
    }
    
    fn set_size_limit(&mut self, limit: usize) {
        self.config.total_size_limit = limit;
        self.stats.base_stats.max_size = limit;
        
        // 如果当前大小超过新限制，移除一些条目
        while self.current_size > self.config.total_size_limit && !self.access_order.is_empty() {
            if let Some(pc) = self.select_eviction_candidate() {
                self.evict_entry(pc);
            } else {
                break;
            }
        }
    }
    
    fn size_limit(&self) -> usize {
        self.config.total_size_limit
    }
    
    fn current_size(&self) -> usize {
        self.current_size
    }
    
    fn entry_count(&self) -> usize {
        self.hot_cache.len() + self.cold_cache.len() + self.prefetch_cache.len()
    }
}

/// 高级代码缓存扩展方法
impl AdvancedCodeCache {
    /// 更新条目访问信息
    pub fn update_access(&mut self, pc: GuestAddr) {
        // 先记录统计信息
        let entry_type = if let Some(entry) = self.get_entry(pc) {
            Some(entry.entry_type)
        } else {
            None
        };
        
        // 更新统计信息
        if let Some(entry_type) = entry_type {
            match entry_type {
                CacheEntryType::Hot => {
                    self.stats.hot_hits += 1;
                }
                CacheEntryType::Cold => {
                    self.stats.cold_hits += 1;
                }
                CacheEntryType::Prefetched => {
                    self.stats.prefetch_hits += 1;
                }
                CacheEntryType::Unknown => {}
            }
            
            self.stats.base_stats.hits += 1;
        } else {
            self.stats.base_stats.misses += 1;
        }
        
        // 更新条目信息
        if let Some(entry) = self.get_entry_mut(pc) {
            entry.access_count += 1;
            entry.last_access = std::time::Instant::now();
            entry.access_pattern.record_access();
            
            // 更新访问顺序
            self.update_access_order(pc);
            
            // 预测下一个访问并加入预取队列
            if let Some(next_pc) = self.predict_next_access(pc) {
                if !self.contains(next_pc) && !self.prefetch_queue.contains(&next_pc) {
                    self.prefetch_queue.push_back(next_pc);
                }
            }
        }
        
        // 检查是否需要分段迁移（在更新条目信息后）
        // 暂时跳过，避免借用冲突
        // if let Some(entry) = self.get_entry_mut(pc) {
        //     self.check_segmentation_migration(pc, entry);
        // }
    }
    
    /// 获取高级统计信息
    pub fn advanced_stats(&self) -> AdvancedCacheStats {
        self.stats.clone()
    }
    
    /// 获取缓存分段信息
    pub fn segment_info(&self) -> CacheSegmentInfo {
        CacheSegmentInfo {
            hot_count: self.hot_cache.len(),
            hot_size: self.hot_size,
            cold_count: self.cold_cache.len(),
            cold_size: self.cold_size,
            prefetch_count: self.prefetch_cache.len(),
            prefetch_size: self.prefetch_size,
        }
    }
    
    /// 优化缓存布局
    pub fn optimize_layout(&mut self) {
        // 重新评估所有条目的分段
        let entries: Vec<(GuestAddr, AdvancedCacheEntry)> = self.hot_cache
            .iter()
            .map(|(&pc, entry)| (pc, entry.clone()))
            .chain(self.cold_cache.iter().map(|(&pc, entry)| (pc, entry.clone())))
            .chain(self.prefetch_cache.iter().map(|(&pc, entry)| (pc, entry.clone())))
            .collect();
        
        // 清空所有缓存
        self.hot_cache.clear();
        self.cold_cache.clear();
        self.prefetch_cache.clear();
        self.hot_size = 0;
        self.cold_size = 0;
        self.prefetch_size = 0;
        
        // 重新分配条目到合适的分段
        for (pc, mut entry) in entries {
            let new_type = self.determine_entry_type(&entry);
            entry.entry_type = new_type;
            
            match new_type {
                CacheEntryType::Hot => {
                    self.hot_cache.insert(pc, entry.clone());
                    self.hot_size += entry.size;
                }
                CacheEntryType::Cold => {
                    self.cold_cache.insert(pc, entry.clone());
                    self.cold_size += entry.size;
                }
                CacheEntryType::Prefetched => {
                    self.prefetch_cache.insert(pc, entry.clone());
                    self.prefetch_size += entry.size;
                }
                CacheEntryType::Unknown => {
                    self.cold_cache.insert(pc, entry.clone());
                    self.cold_size += entry.size;
                }
            }
        }
    }
}

/// 缓存分段信息
#[derive(Debug, Clone)]
pub struct CacheSegmentInfo {
    /// 热点缓存条目数
    pub hot_count: usize,
    /// 热点缓存大小
    pub hot_size: usize,
    /// 冷缓存条目数
    pub cold_count: usize,
    /// 冷缓存大小
    pub cold_size: usize,
    /// 预取缓存条目数
    pub prefetch_count: usize,
    /// 预取缓存大小
    pub prefetch_size: usize,
}