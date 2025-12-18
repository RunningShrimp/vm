//! 优化的代码缓存实现
//!
//! 提供高性能的代码缓存，包括热点检测、预取和分层缓存策略。

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use vm_core::GuestAddr;
use crate::code_cache::CodeCache;

/// 分层缓存配置
#[derive(Debug, Clone)]
pub struct OptimizedCacheConfig {
    /// L1缓存大小（热点代码）
    pub l1_size: usize,
    /// L2缓存大小（常用代码）
    pub l2_size: usize,
    /// L3缓存大小（所有代码）
    pub l3_size: usize,
    /// 热点阈值
    pub hotspot_threshold: u32,
    /// 预取窗口大小
    pub prefetch_window: usize,
    /// 缓存行大小
    pub cache_line_size: usize,
}

impl Default for OptimizedCacheConfig {
    fn default() -> Self {
        Self {
            l1_size: 1024 * 1024,      // 1MB for hot code
            l2_size: 8 * 1024 * 1024,   // 8MB for frequently used code
            l3_size: 64 * 1024 * 1024,  // 64MB for all code
            hotspot_threshold: 100,
            prefetch_window: 16,
            cache_line_size: 64,
        }
    }
}

/// 缓存条目
#[derive(Debug, Clone)]
struct OptimizedCacheEntry {
    /// 编译后的代码
    code: Vec<u8>,
    /// 访问计数
    access_count: AtomicU64,
    /// 最后访问时间（纳秒）
    last_access_ns: AtomicU64,
    /// 代码大小
    size: usize,
    /// 缓存级别（1=L1热点, 2=L2常用, 3=L3其他）
    cache_level: u8,
    /// 预取标记
    prefetch_mark: bool,
}

/// 优化的代码缓存
pub struct OptimizedCodeCache {
    /// 配置
    config: OptimizedCacheConfig,
    /// L1缓存（热点代码）- 使用HashMap实现O(1)访问
    l1_cache: Arc<Mutex<HashMap<GuestAddr, OptimizedCacheEntry>>>,
    /// L2缓存（常用代码）
    l2_cache: Arc<Mutex<HashMap<GuestAddr, OptimizedCacheEntry>>>,
    /// L3缓存（所有代码）
    l3_cache: Arc<Mutex<HashMap<GuestAddr, OptimizedCacheEntry>>>,
    /// L1 LRU顺序（用于淘汰）
    l1_lru: Arc<Mutex<VecDeque<GuestAddr>>>,
    /// L2 LRU顺序
    l2_lru: Arc<Mutex<VecDeque<GuestAddr>>>,
    /// L3 LRU顺序
    l3_lru: Arc<Mutex<VecDeque<GuestAddr>>>,
    /// 当前缓存大小
    l1_current_size: AtomicUsize,
    l2_current_size: AtomicUsize,
    l3_current_size: AtomicUsize,
    /// 统计信息
    stats: Arc<Mutex<OptimizedCacheStats>>,
}

/// 优化的缓存统计
#[derive(Debug, Clone, Default)]
pub struct OptimizedCacheStats {
    /// L1命中次数
    pub l1_hits: AtomicU64,
    /// L2命中次数
    pub l2_hits: AtomicU64,
    /// L3命中次数
    pub l3_hits: AtomicU64,
    /// 总未命中次数
    pub total_misses: AtomicU64,
    /// 预取命中次数
    pub prefetch_hits: AtomicU64,
    /// 热点提升次数
    pub hotspot_promotions: AtomicU64,
    /// 缓存淘汰次数
    pub evictions: AtomicU64,
}

impl OptimizedCodeCache {
    /// 创建新的优化缓存
    pub fn new(config: OptimizedCacheConfig) -> Self {
        Self {
            config,
            l1_cache: Arc::new(Mutex::new(HashMap::new())),
            l2_cache: Arc::new(Mutex::new(HashMap::new())),
            l3_cache: Arc::new(Mutex::new(HashMap::new())),
            l1_lru: Arc::new(Mutex::new(VecDeque::new())),
            l2_lru: Arc::new(Mutex::new(VecDeque::new())),
            l3_lru: Arc::new(Mutex::new(VecDeque::new())),
            l1_current_size: AtomicUsize::new(0),
            l2_current_size: AtomicUsize::new(0),
            l3_current_size: AtomicUsize::new(0),
            stats: Arc::new(Mutex::new(OptimizedCacheStats::default())),
        }
    }

    /// 获取当前时间戳（纳秒）
    fn current_time_ns() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    }

    /// 更新LRU顺序
    fn update_lru(lru: &mut VecDeque<GuestAddr>, pc: GuestAddr) {
        // 从当前位置移除
        if let Some(pos) = lru.iter().position(|&x| x == pc) {
            lru.remove(pos);
        }
        // 添加到末尾（最新）
        lru.push_back(pc);
    }

    /// 确保L1缓存有足够空间
    fn ensure_l1_space(&mut self, required_size: usize) {
        while self.l1_current_size.load(Ordering::Relaxed) + required_size > self.config.l1_size 
            && !self.l1_lru.is_empty() {
            if let Some(oldest_pc) = self.l1_lru.front().cloned() {
                self.evict_from_l1(oldest_pc);
            }
        }
    }

    /// 确保L2缓存有足够空间
    fn ensure_l2_space(&mut self, required_size: usize) {
        while self.l2_current_size.load(Ordering::Relaxed) + required_size > self.config.l2_size 
            && !self.l2_lru.is_empty() {
            if let Some(oldest_pc) = self.l2_lru.front().cloned() {
                self.evict_from_l2(oldest_pc);
            }
        }
    }

    /// 确保L3缓存有足够空间
    fn ensure_l3_space(&mut self, required_size: usize) {
        while self.l3_current_size.load(Ordering::Relaxed) + required_size > self.config.l3_size 
            && !self.l3_lru.is_empty() {
            if let Some(oldest_pc) = self.l3_lru.front().cloned() {
                self.evict_from_l3(oldest_pc);
            }
        }
    }

    /// 从L1缓存淘汰条目
    fn evict_from_l1(&mut self, pc: GuestAddr) {
        if let Some(entry) = self.l1_cache.remove(&pc) {
            self.l1_current_size.fetch_sub(entry.size, Ordering::Relaxed);
            self.l1_lru.remove(0);
            
            // 将降级的代码移到L2缓存
            self.insert_to_l2(pc, entry);
            self.stats.evictions.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// 从L2缓存淘汰条目
    fn evict_from_l2(&mut self, pc: GuestAddr) {
        if let Some(entry) = self.l2_cache.remove(&pc) {
            self.l2_current_size.fetch_sub(entry.size, Ordering::Relaxed);
            self.l2_lru.remove(0);
            
            // 将降级的代码移到L3缓存
            self.insert_to_l3(pc, entry);
            self.stats.evictions.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// 从L3缓存淘汰条目
    fn evict_from_l3(&mut self, pc: GuestAddr) {
        if let Some(entry) = self.l3_cache.remove(&pc) {
            self.l3_current_size.fetch_sub(entry.size, Ordering::Relaxed);
            self.l3_lru.remove(0);
            self.stats.evictions.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// 插入到L1缓存
    fn insert_to_l1(&mut self, pc: GuestAddr, mut entry: OptimizedCacheEntry) {
        self.ensure_l1_space(entry.size);
        entry.cache_level = 1;
        entry.last_access_ns.store(Self::current_time_ns(), Ordering::Relaxed);
        
        self.l1_cache.insert(pc, entry.clone());
        Self::update_lru(&mut self.l1_lru, pc);
        self.l1_current_size.fetch_add(entry.size, Ordering::Relaxed);
    }

    /// 插入到L2缓存
    fn insert_to_l2(&mut self, pc: GuestAddr, mut entry: OptimizedCacheEntry) {
        self.ensure_l2_space(entry.size);
        entry.cache_level = 2;
        entry.last_access_ns.store(Self::current_time_ns(), Ordering::Relaxed);
        
        self.l2_cache.insert(pc, entry.clone());
        Self::update_lru(&mut self.l2_lru, pc);
        self.l2_current_size.fetch_add(entry.size, Ordering::Relaxed);
    }

    /// 插入到L3缓存
    fn insert_to_l3(&mut self, pc: GuestAddr, mut entry: OptimizedCacheEntry) {
        self.ensure_l3_space(entry.size);
        entry.cache_level = 3;
        entry.last_access_ns.store(Self::current_time_ns(), Ordering::Relaxed);
        
        self.l3_cache.insert(pc, entry);
        Self::update_lru(&mut self.l3_lru, pc);
        self.l3_current_size.fetch_add(entry.size, Ordering::Relaxed);
    }

    /// 检查是否应该提升到热点缓存
    fn should_promote_to_hotspot(&self, entry: &OptimizedCacheEntry) -> bool {
        entry.access_count.load(Ordering::Relaxed) >= self.config.hotspot_threshold as u64
    }

    /// 提升热点代码到L1缓存
    fn promote_to_hotspot(&mut self, pc: GuestAddr) {
        if let Some(entry) = self.l2_cache.remove(&pc) {
            self.l2_current_size.fetch_sub(entry.size, Ordering::Relaxed);
            self.l2_lru.remove(|&x| x == pc);
            
            self.insert_to_l1(pc, entry);
            self.stats.hotspot_promotions.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// 预取相邻代码块
    fn prefetch_neighbors(&mut self, pc: GuestAddr) {
        for offset in 1..=self.config.prefetch_window {
            let neighbor_pc = pc + (offset * self.config.cache_line_size) as u64;
            
            // 检查L3缓存中是否有相邻代码
            if let Some(entry) = self.l3_cache.get(&neighbor_pc) {
                if !entry.prefetch_mark {
                    // 预取到L2缓存
                    self.insert_to_l2(neighbor_pc, entry.clone());
                    self.stats.prefetch_hits.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    }
}

impl CodeCache for OptimizedCodeCache {
    fn insert(&mut self, pc: GuestAddr, code: Vec<u8>) {
        let entry = OptimizedCacheEntry {
            size: code.len(),
            code,
            access_count: AtomicU64::new(1),
            last_access_ns: AtomicU64::new(Self::current_time_ns()),
            cache_level: 3, // 初始插入L3
            prefetch_mark: false,
        };

        // 新代码直接插入L3缓存
        self.insert_to_l3(pc, entry);
    }

    fn get(&self, pc: GuestAddr) -> Option<Vec<u8>> {
        let current_time = Self::current_time_ns();
        
        // 首先检查L1缓存（热点代码）
        if let Ok(mut l1_cache) = self.l1_cache.lock() {
            if let Some(entry) = l1_cache.get_mut(&pc) {
                entry.access_count.fetch_add(1, Ordering::Relaxed);
                entry.last_access_ns.store(current_time, Ordering::Relaxed);
                
                if let Ok(mut l1_lru) = self.l1_lru.lock() {
                    Self::update_lru(&mut l1_lru, pc);
                }
                
                if let Ok(mut stats) = self.stats.lock() {
                    stats.l1_hits.fetch_add(1, Ordering::Relaxed);
                }
                
                // 预取相邻代码块
                self.prefetch_neighbors(pc);
                
                return Some(entry.code.clone());
            }
        }

        // 然后检查L2缓存（常用代码）
        if let Ok(mut l2_cache) = self.l2_cache.lock() {
            if let Some(entry) = l2_cache.get_mut(&pc) {
                entry.access_count.fetch_add(1, Ordering::Relaxed);
                entry.last_access_ns.store(current_time, Ordering::Relaxed);
                
                if let Ok(mut l2_lru) = self.l2_lru.lock() {
                    Self::update_lru(&mut l2_lru, pc);
                }
                
                if let Ok(mut stats) = self.stats.lock() {
                    stats.l2_hits.fetch_add(1, Ordering::Relaxed);
                }
                
                // 检查是否应该提升到热点缓存
                if self.should_promote_to_hotspot(entry) {
                    self.promote_to_hotspot(pc);
                }
                
                return Some(entry.code.clone());
            }
        }

        // 最后检查L3缓存（所有代码）
        if let Ok(mut l3_cache) = self.l3_cache.lock() {
            if let Some(entry) = l3_cache.get_mut(&pc) {
                entry.access_count.fetch_add(1, Ordering::Relaxed);
                entry.last_access_ns.store(current_time, Ordering::Relaxed);
                
                if let Ok(mut l3_lru) = self.l3_lru.lock() {
                    Self::update_lru(&mut l3_lru, pc);
                }
                
                if let Ok(mut stats) = self.stats.lock() {
                    stats.l3_hits.fetch_add(1, Ordering::Relaxed);
                }
                
                // 提升到L2缓存
                let entry_clone = entry.clone();
                self.insert_to_l2(pc, entry_clone);
                
                return Some(entry.code.clone());
            }
        }

        // 缓存未命中
        if let Ok(mut stats) = self.stats.lock() {
            stats.total_misses.fetch_add(1, Ordering::Relaxed);
        }
        None
    }

    fn contains(&self, pc: GuestAddr) -> bool {
        self.l1_cache.contains_key(&pc) 
            || self.l2_cache.contains_key(&pc) 
            || self.l3_cache.contains_key(&pc)
    }

    fn remove(&mut self, pc: GuestAddr) -> Option<Vec<u8>> {
        // 按优先级顺序检查并移除
        if let Some(entry) = self.l1_cache.remove(&pc) {
            self.l1_current_size.fetch_sub(entry.size, Ordering::Relaxed);
            self.l1_lru.remove(|&x| x == pc);
            return Some(entry.code);
        }

        if let Some(entry) = self.l2_cache.remove(&pc) {
            self.l2_current_size.fetch_sub(entry.size, Ordering::Relaxed);
            self.l2_lru.remove(|&x| x == pc);
            return Some(entry.code);
        }

        if let Some(entry) = self.l3_cache.remove(&pc) {
            self.l3_current_size.fetch_sub(entry.size, Ordering::Relaxed);
            self.l3_lru.remove(|&x| x == pc);
            return Some(entry.code);
        }

        None
    }

    fn clear(&mut self) {
        self.l1_cache.clear();
        self.l2_cache.clear();
        self.l3_cache.clear();
        self.l1_lru.clear();
        self.l2_lru.clear();
        self.l3_lru.clear();
        self.l1_current_size.store(0, Ordering::Relaxed);
        self.l2_current_size.store(0, Ordering::Relaxed);
        self.l3_current_size.store(0, Ordering::Relaxed);
    }

    fn stats(&self) -> crate::code_cache::CacheStats {
        let l1_hits = self.stats.l1_hits.load(Ordering::Relaxed);
        let l2_hits = self.stats.l2_hits.load(Ordering::Relaxed);
        let l3_hits = self.stats.l3_hits.load(Ordering::Relaxed);
        let total_misses = self.stats.total_misses.load(Ordering::Relaxed);
        let total_hits = l1_hits + l2_hits + l3_hits;
        let total_accesses = total_hits + total_misses;
        
        crate::code_cache::CacheStats {
            hits: total_hits,
            misses: total_misses,
            inserts: 0, // TODO: track inserts
            removals: 0, // TODO: track removals
            max_size: self.config.l3_size,
            current_size: self.l3_current_size.load(Ordering::Relaxed) 
                + self.l2_current_size.load(Ordering::Relaxed)
                + self.l1_current_size.load(Ordering::Relaxed),
            entry_count: self.l1_cache.len() + self.l2_cache.len() + self.l3_cache.len(),
            hit_rate: if total_accesses > 0 {
                total_hits as f64 / total_accesses as f64
            } else {
                0.0
            },
        }
    }

    fn set_size_limit(&mut self, limit: usize) {
        self.config.l3_size = limit;
        // 确保当前大小不超过新限制
        while self.current_size() > limit {
            if let Some(oldest_pc) = self.l3_lru.front().cloned() {
                self.evict_from_l3(oldest_pc);
            }
        }
    }

    fn size_limit(&self) -> usize {
        self.config.l3_size
    }

    fn current_size(&self) -> usize {
        self.l1_current_size.load(Ordering::Relaxed) 
            + self.l2_current_size.load(Ordering::Relaxed)
            + self.l3_current_size.load(Ordering::Relaxed)
    }

    fn entry_count(&self) -> usize {
        self.l1_cache.len() + self.l2_cache.len() + self.l3_cache.len()
    }
}