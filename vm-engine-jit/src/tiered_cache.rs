//! 分层代码缓存实现
//!
//! 实现了多级缓存策略，根据代码访问频率和热度自动调整缓存层级。

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use vm_core::GuestAddr;
use crate::code_cache::{CodeCache, CacheStats, TieredCacheStats};

/// 分层缓存配置
#[derive(Debug, Clone)]
pub struct TieredCacheConfig {
    /// L1缓存大小（字节）- 存储热点代码
    pub l1_size: usize,
    /// L2缓存大小（字节）- 存储常用代码
    pub l2_size: usize,
    /// L3缓存大小（字节）- 存储所有代码
    pub l3_size: usize,
    /// 热点阈值 - 访问次数超过此值的代码进入L1
    pub hotspot_threshold: u32,
    /// 常用阈值 - 访问次数超过此值的代码进入L2
    pub frequent_threshold: u32,
    /// L1缓存最大条目数
    pub l1_max_entries: usize,
    /// L2缓存最大条目数
    pub l2_max_entries: usize,
}

impl Default for TieredCacheConfig {
    fn default() -> Self {
        Self {
            l1_size: 256 * 1024,      // 256KB
            l2_size: 2 * 1024 * 1024,  // 2MB
            l3_size: 64 * 1024 * 1024, // 64MB
            hotspot_threshold: 1000,
            frequent_threshold: 100,
            l1_max_entries: 1000,
            l2_max_entries: 10000,
        }
    }
}

/// 缓存条目
#[derive(Debug, Clone)]
struct CacheEntry {
    /// 代码数据
    code: Vec<u8>,
    /// 访问次数
    access_count: u32,
    /// 最后访问时间
    last_access: std::time::Instant,
    /// 代码大小
    size: usize,
    /// 创建时间
    created_at: std::time::Instant,
}

/// 分层代码缓存
pub struct TieredCodeCache {
    /// 缓存配置
    config: TieredCacheConfig,
    /// L1缓存 - 热点代码
    l1_cache: Arc<Mutex<HashMap<GuestAddr, CacheEntry>>>,
    /// L2缓存 - 常用代码
    l2_cache: Arc<Mutex<HashMap<GuestAddr, CacheEntry>>>,
    /// L3缓存 - 所有代码
    l3_cache: Arc<Mutex<HashMap<GuestAddr, CacheEntry>>>,
    /// L1访问顺序（用于LRU）
    l1_access_order: Arc<Mutex<VecDeque<GuestAddr>>>,
    /// L2访问顺序（用于LRU）
    l2_access_order: Arc<Mutex<VecDeque<GuestAddr>>>,
    /// 缓存统计
    stats: Arc<Mutex<TieredCacheStats>>,
    /// 当前各层大小
    current_sizes: Arc<Mutex<TieredCacheSizes>>,
}



/// 分层缓存大小
#[derive(Debug, Default)]
struct TieredCacheSizes {
    l1_size: usize,
    l2_size: usize,
    l3_size: usize,
}

impl TieredCodeCache {
    /// 创建新的分层缓存
    pub fn new(config: TieredCacheConfig) -> Self {
        Self {
            config,
            l1_cache: Arc::new(Mutex::new(HashMap::new())),
            l2_cache: Arc::new(Mutex::new(HashMap::new())),
            l3_cache: Arc::new(Mutex::new(HashMap::new())),
            l1_access_order: Arc::new(Mutex::new(VecDeque::new())),
            l2_access_order: Arc::new(Mutex::new(VecDeque::new())),
            stats: Arc::new(Mutex::new(TieredCacheStats::default())),
            current_sizes: Arc::new(Mutex::new(TieredCacheSizes::default())),
        }
    }
    
    /// 提升代码到更高层级的缓存
    fn promote_to_l1(&self, pc: GuestAddr, entry: CacheEntry) {
        let mut l1_cache = self.l1_cache.lock().unwrap();
        let mut l2_cache = self.l2_cache.lock().unwrap();
        let mut l1_order = self.l1_access_order.lock().unwrap();
        let mut l2_order = self.l2_access_order.lock().unwrap();
        let mut sizes = self.current_sizes.lock().unwrap();
        let mut stats = self.stats.lock().unwrap();
        
        // 检查L1空间
        if sizes.l1_size + entry.size > self.config.l1_size || 
           l1_cache.len() >= self.config.l1_max_entries {
            // 需要驱逐L1中的条目
            self.evict_from_l1(&mut l1_cache, &mut l1_order, &mut sizes, &mut stats);
        }
        
        // 从L2中移除（如果存在）
        if l2_cache.contains_key(&pc) {
            l2_cache.remove(&pc);
            l2_order.retain(|&addr| addr != pc);
            sizes.l2_size -= entry.size;
            stats.l2_to_l1_promotions += 1;
        }
        
        // 添加到L1
        l1_cache.insert(pc, entry.clone());
        l1_order.push_back(pc);
        sizes.l1_size += entry.size;
        stats.l1_hits += 1;
    }
    
    /// 提升代码到L2缓存
    fn promote_to_l2(&self, pc: GuestAddr, entry: CacheEntry) {
        let mut l2_cache = self.l2_cache.lock().unwrap();
        let mut l3_cache = self.l3_cache.lock().unwrap();
        let mut l2_order = self.l2_access_order.lock().unwrap();
        let mut sizes = self.current_sizes.lock().unwrap();
        let mut stats = self.stats.lock().unwrap();
        
        // 检查L2空间
        if sizes.l2_size + entry.size > self.config.l2_size || 
           l2_cache.len() >= self.config.l2_max_entries {
            // 需要驱逐L2中的条目
            self.evict_from_l2(&mut l2_cache, &mut l2_order, &mut sizes, &mut stats);
        }
        
        // 从L3中移除（如果存在）
        if l3_cache.contains_key(&pc) {
            l3_cache.remove(&pc);
            sizes.l3_size -= entry.size;
            stats.l3_to_l2_promotions += 1;
        }
        
        // 添加到L2
        l2_cache.insert(pc, entry.clone());
        l2_order.push_back(pc);
        sizes.l2_size += entry.size;
        stats.l2_hits += 1;
    }
    
    /// 从L1缓存驱逐条目
    fn evict_from_l1(&self, 
                     l1_cache: &mut HashMap<GuestAddr, CacheEntry>,
                     l1_order: &mut VecDeque<GuestAddr>,
                     sizes: &mut TieredCacheSizes,
                     stats: &mut TieredCacheStats) {
        // 使用智能驱逐策略，考虑访问频率和年龄
        let evict_pc = {
            let mut min_pc = None;
            let mut min_score = std::f64::INFINITY;
            let now = std::time::Instant::now();
            for (&pc, entry) in l1_cache.iter() {
                // 计算综合评分：考虑年龄和访问频率
                let age_secs = now.duration_since(entry.created_at).as_secs_f64();
                let access_freq = entry.access_count as f64 / age_secs.max(1.0);
                
                // 综合评分：较老且访问频率低的条目优先被驱逐
                let score = age_secs / (access_freq + 1.0);
                
                if score < min_score {
                    min_score = score;
                    min_pc = Some(pc);
                }
            }
            min_pc
        };
        
        if let Some(pc) = evict_pc {
            // 从访问顺序中移除
            l1_order.retain(|&addr| addr != pc);
            
            if let Some(entry) = l1_cache.remove(&pc) {
                sizes.l1_size -= entry.size;
                stats.l1_evictions += 1;
                
                // 将被驱逐的条目降级到L2
                self.demote_to_l2(pc, entry);
            }
        } else if let Some(pc) = l1_order.pop_front() {
            // 回退到FIFO策略
            if let Some(entry) = l1_cache.remove(&pc) {
                sizes.l1_size -= entry.size;
                stats.l1_evictions += 1;
                
                // 将被驱逐的条目降级到L2
                self.demote_to_l2(pc, entry);
            }
        }
    }
    
    /// 从L2缓存驱逐条目
    fn evict_from_l2(&self, 
                     l2_cache: &mut HashMap<GuestAddr, CacheEntry>,
                     l2_order: &mut VecDeque<GuestAddr>,
                     sizes: &mut TieredCacheSizes,
                     stats: &mut TieredCacheStats) {
        // 使用智能驱逐策略，考虑访问频率和年龄
        let evict_pc = {
            let mut min_pc = None;
            let mut min_score = std::f64::INFINITY;
            let now = std::time::Instant::now();
            for (&pc, entry) in l2_cache.iter() {
                // 计算综合评分：考虑年龄和访问频率
                let age_secs = now.duration_since(entry.created_at).as_secs_f64();
                let access_freq = entry.access_count as f64 / age_secs.max(1.0);
                
                // 综合评分：较老且访问频率低的条目优先被驱逐
                let score = age_secs / (access_freq + 1.0);
                
                if score < min_score {
                    min_score = score;
                    min_pc = Some(pc);
                }
            }
            min_pc
        };
        
        if let Some(pc) = evict_pc {
            // 从访问顺序中移除
            l2_order.retain(|&addr| addr != pc);
            
            if let Some(entry) = l2_cache.remove(&pc) {
                sizes.l2_size -= entry.size;
                stats.l2_evictions += 1;
                
                // 将被驱逐的条目降级到L3
                self.demote_to_l3(pc, entry);
            }
        } else if let Some(pc) = l2_order.pop_front() {
            // 回退到FIFO策略
            if let Some(entry) = l2_cache.remove(&pc) {
                sizes.l2_size -= entry.size;
                stats.l2_evictions += 1;
                
                // 将被驱逐的条目降级到L3
                self.demote_to_l3(pc, entry);
            }
        }
    }
    
    /// 降级条目到L2
    fn demote_to_l2(&self, pc: GuestAddr, entry: CacheEntry) {
        let mut l2_cache = self.l2_cache.lock().unwrap();
        let mut l2_order = self.l2_access_order.lock().unwrap();
        let mut sizes = self.current_sizes.lock().unwrap();
        
        // 检查L2空间
        if sizes.l2_size + entry.size <= self.config.l2_size && 
           l2_cache.len() < self.config.l2_max_entries {
            l2_cache.insert(pc, entry.clone());
            l2_order.push_back(pc);
            sizes.l2_size += entry.size;
        }
    }
    
    /// 降级条目到L3
    fn demote_to_l3(&self, pc: GuestAddr, entry: CacheEntry) {
        let mut l3_cache = self.l3_cache.lock().unwrap();
        let mut sizes = self.current_sizes.lock().unwrap();
        
        // 检查L3空间
        if sizes.l3_size + entry.size <= self.config.l3_size {
            l3_cache.insert(pc, entry.clone());
            sizes.l3_size += entry.size;
        }
    }
    
    /// 更新访问统计
    fn update_access_stats(&self, pc: GuestAddr) {
        let now = std::time::Instant::now();
        
        // 更新L1访问顺序
        {
            let mut l1_order = self.l1_access_order.lock().unwrap();
            l1_order.retain(|&addr| addr != pc);
            l1_order.push_back(pc);
        }
        
        // 更新L2访问顺序
        {
            let mut l2_order = self.l2_access_order.lock().unwrap();
            l2_order.retain(|&addr| addr != pc);
            l2_order.push_back(pc);
        }
        
        // 更新访问计数和时间
        for cache in [&self.l1_cache, &self.l2_cache, &self.l3_cache] {
            let mut cache = cache.lock().unwrap();
            if let Some(entry) = cache.get_mut(&pc) {
                entry.access_count += 1;
                entry.last_access = now;
            }
        }
    }
}

impl CodeCache for TieredCodeCache {
    fn insert(&mut self, pc: GuestAddr, code: Vec<u8>) {
        let entry = CacheEntry {
            size: code.len(),
            code,
            access_count: 1,
            last_access: std::time::Instant::now(),
            created_at: std::time::Instant::now(),
        };
        
        // 根据访问频率决定插入到哪一层
        if entry.access_count >= self.config.hotspot_threshold {
            self.promote_to_l1(pc, entry);
        } else if entry.access_count >= self.config.frequent_threshold {
            self.promote_to_l2(pc, entry);
        } else {
            // 插入到L3
            let mut l3_cache = self.l3_cache.lock().unwrap();
            let mut sizes = self.current_sizes.lock().unwrap();
            let mut stats = self.stats.lock().unwrap();
            
            // 检查L3空间
            if sizes.l3_size + entry.size > self.config.l3_size {
                // 改进的驱逐策略，同时考虑访问时间和创建时间
                let evict_pc = {
                    let mut min_pc = None;
                    let mut min_score = std::f64::INFINITY;
                    let now = std::time::Instant::now();
                    for (&pc, entry) in l3_cache.iter() {
                        // 计算综合评分：考虑年龄和访问频率
                        // 年龄得分：条目存在的时长（秒）
                        let age_secs = now.duration_since(entry.created_at).as_secs_f64();
                        // 访问频率得分：每秒平均访问次数
                        let access_freq = entry.access_count as f64 / age_secs.max(1.0);
                        
                        // 综合评分：较老且访问频率低的条目优先被驱逐
                        // 这里我们使用一个简单的公式：age_secs / (access_freq + 1.0)
                        // 加1是为了避免除零错误
                        let score = age_secs / (access_freq + 1.0);
                        
                        if score < min_score {
                            min_score = score;
                            min_pc = Some(pc);
                        }
                    }
                    min_pc
                };
                
                if let Some(evict_pc) = evict_pc {
                    if let Some(evict_entry) = l3_cache.remove(&evict_pc) {
                        sizes.l3_size -= evict_entry.size;
                        stats.l3_evictions += 1;
                    }
                }
            }
            
            l3_cache.insert(pc, entry.clone());
            sizes.l3_size += entry.size;
            stats.base_stats.inserts += 1;
        }
    }
    
    fn get(&self, pc: GuestAddr) -> Option<Vec<u8>> {
        // 按L1 -> L2 -> L3的顺序查找
        let mut stats = self.stats.lock().unwrap();
        
        // 检查L1
        {
            let l1_cache = self.l1_cache.lock().unwrap();
            if let Some(entry) = l1_cache.get(&pc) {
                stats.base_stats.hits += 1;
                stats.l1_hits += 1;
                self.update_access_stats(pc);
                return Some(entry.code.clone());
            }
        }
        
        // 检查L2
        {
            {
                let l2_cache = self.l2_cache.lock().unwrap();
                if let Some(entry) = l2_cache.get(&pc) {
                    stats.base_stats.hits += 1;
                    stats.l2_hits += 1;
                    
                    // 检查是否需要提升到L1
                    if entry.access_count + 1 >= self.config.hotspot_threshold {
                        let entry_clone = entry.clone();
                        let code = entry.code.clone();
                        drop(l2_cache);
                        drop(stats);
                        self.promote_to_l1(pc, entry_clone);
                        return Some(code);
                    }
                    
                    let code = entry.code.clone();
                    drop(l2_cache);
                    self.update_access_stats(pc);
                    return Some(code);
                }
            }
        }
        
        // 检查L3
        {
            let l3_cache = self.l3_cache.lock().unwrap();
            if let Some(entry) = l3_cache.get(&pc) {
                stats.base_stats.hits += 1;
                stats.l3_hits += 1;
                
                // 检查是否需要提升到L2
                if entry.access_count + 1 >= self.config.frequent_threshold {
                    let entry_clone = entry.clone();
                    let code = entry.code.clone();
                    drop(l3_cache);
                    drop(stats);
                    self.promote_to_l2(pc, entry_clone);
                    return Some(code);
                }
                
                let code = entry.code.clone();
                drop(l3_cache);
                self.update_access_stats(pc);
                return Some(code);
            }
        }
        
        // 未命中
        stats.base_stats.misses += 1;
        None
    }
    
    fn contains(&self, pc: GuestAddr) -> bool {
        let l1_cache = self.l1_cache.lock().unwrap();
        if l1_cache.contains_key(&pc) {
            return true;
        }
        
        let l2_cache = self.l2_cache.lock().unwrap();
        if l2_cache.contains_key(&pc) {
            return true;
        }
        
        let l3_cache = self.l3_cache.lock().unwrap();
        l3_cache.contains_key(&pc)
    }
    
    fn remove(&mut self, pc: GuestAddr) -> Option<Vec<u8>> {
        let mut stats = self.stats.lock().unwrap();
        let mut sizes = self.current_sizes.lock().unwrap();
        
        // 从L1移除
        {
            let mut l1_cache = self.l1_cache.lock().unwrap();
            let mut l1_order = self.l1_access_order.lock().unwrap();
            if let Some(entry) = l1_cache.remove(&pc) {
                l1_order.retain(|&addr| addr != pc);
                sizes.l1_size -= entry.size;
                stats.base_stats.removals += 1;
                return Some(entry.code);
            }
        }
        
        // 从L2移除
        {
            let mut l2_cache = self.l2_cache.lock().unwrap();
            let mut l2_order = self.l2_access_order.lock().unwrap();
            if let Some(entry) = l2_cache.remove(&pc) {
                l2_order.retain(|&addr| addr != pc);
                sizes.l2_size -= entry.size;
                stats.base_stats.removals += 1;
                return Some(entry.code);
            }
        }
        
        // 从L3移除
        {
            let mut l3_cache = self.l3_cache.lock().unwrap();
            if let Some(entry) = l3_cache.remove(&pc) {
                sizes.l3_size -= entry.size;
                stats.base_stats.removals += 1;
                return Some(entry.code);
            }
        }
        
        None
    }
    
    fn clear(&mut self) {
        let mut l1_cache = self.l1_cache.lock().unwrap();
        let mut l2_cache = self.l2_cache.lock().unwrap();
        let mut l3_cache = self.l3_cache.lock().unwrap();
        let mut l1_order = self.l1_access_order.lock().unwrap();
        let mut l2_order = self.l2_access_order.lock().unwrap();
        let mut sizes = self.current_sizes.lock().unwrap();
        let mut stats = self.stats.lock().unwrap();
        
        l1_cache.clear();
        l2_cache.clear();
        l3_cache.clear();
        l1_order.clear();
        l2_order.clear();
        
        sizes.l1_size = 0;
        sizes.l2_size = 0;
        sizes.l3_size = 0;
        
        stats.base_stats.hits = 0;
        stats.base_stats.misses = 0;
        stats.base_stats.inserts = 0;
        stats.base_stats.removals = 0;
        stats.l1_hits = 0;
        stats.l2_hits = 0;
        stats.l3_hits = 0;
        stats.l1_to_l2_promotions = 0;
        stats.l2_to_l1_promotions = 0;
        stats.l3_to_l2_promotions = 0;
        stats.l1_evictions = 0;
        stats.l2_evictions = 0;
        stats.l3_evictions = 0;
    }
    
    fn stats(&self) -> CacheStats {
        let tiered_stats = self.stats.lock().unwrap();
        tiered_stats.base_stats.clone()
    }
    
    fn set_size_limit(&mut self, limit: usize) {
        // 更新L3大小限制（总大小）
        self.config.l3_size = limit;
    }
    
    fn size_limit(&self) -> usize {
        self.config.l3_size
    }
    
    fn current_size(&self) -> usize {
        let sizes = self.current_sizes.lock().unwrap();
        sizes.l1_size + sizes.l2_size + sizes.l3_size
    }
    
    fn entry_count(&self) -> usize {
        let l1_cache = self.l1_cache.lock().unwrap();
        let l2_cache = self.l2_cache.lock().unwrap();
        let l3_cache = self.l3_cache.lock().unwrap();
        l1_cache.len() + l2_cache.len() + l3_cache.len()
    }
    
    /// 获取分层缓存统计
    fn tiered_stats(&self) -> Option<TieredCacheStats> {
        Some(self.stats.lock().unwrap().clone())
    }
}