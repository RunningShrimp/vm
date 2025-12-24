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
#[derive(Debug)]
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

impl Clone for OptimizedCacheEntry {
    fn clone(&self) -> Self {
        Self {
            code: self.code.clone(),
            access_count: AtomicU64::new(self.access_count.load(Ordering::Relaxed)),
            last_access_ns: AtomicU64::new(self.last_access_ns.load(Ordering::Relaxed)),
            size: self.size,
            cache_level: self.cache_level,
            prefetch_mark: self.prefetch_mark,
        }
    }
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
#[derive(Debug, Default)]
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
    /// 缓存插入次数
    pub inserts: AtomicU64,
    /// 缓存移除次数
    pub removals: AtomicU64,
}

impl Clone for OptimizedCacheStats {
    fn clone(&self) -> Self {
        Self {
            l1_hits: AtomicU64::new(self.l1_hits.load(Ordering::Relaxed)),
            l2_hits: AtomicU64::new(self.l2_hits.load(Ordering::Relaxed)),
            l3_hits: AtomicU64::new(self.l3_hits.load(Ordering::Relaxed)),
            total_misses: AtomicU64::new(self.total_misses.load(Ordering::Relaxed)),
            prefetch_hits: AtomicU64::new(self.prefetch_hits.load(Ordering::Relaxed)),
            hotspot_promotions: AtomicU64::new(self.hotspot_promotions.load(Ordering::Relaxed)),
            evictions: AtomicU64::new(self.evictions.load(Ordering::Relaxed)),
            inserts: AtomicU64::new(self.inserts.load(Ordering::Relaxed)),
            removals: AtomicU64::new(self.removals.load(Ordering::Relaxed)),
        }
    }
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
            && !self.l1_lru.lock().unwrap().is_empty() {
            let oldest_pc = self.l1_lru.lock().unwrap().front().copied();
            if let Some(pc) = oldest_pc {
                self.evict_from_l1(pc);
            }
        }
    }

    /// 确保L2缓存有足够空间
    fn ensure_l2_space(&mut self, required_size: usize) {
        while self.l2_current_size.load(Ordering::Relaxed) + required_size > self.config.l2_size 
            && !self.l2_lru.lock().unwrap().is_empty() {
            let oldest_pc = self.l2_lru.lock().unwrap().front().copied();
            if let Some(pc) = oldest_pc {
                self.evict_from_l2(pc);
            }
        }
    }

    /// 确保L3缓存有足够空间
    fn ensure_l3_space(&mut self, required_size: usize) {
        while self.l3_current_size.load(Ordering::Relaxed) + required_size > self.config.l3_size 
            && !self.l3_lru.lock().unwrap().is_empty() {
            let oldest_pc = self.l3_lru.lock().unwrap().front().copied();
            if let Some(pc) = oldest_pc {
                self.evict_from_l3(pc);
            }
        }
    }

    /// 从L1缓存淘汰条目
    fn evict_from_l1(&mut self, pc: GuestAddr) {
        let entry = self.l1_cache.lock().unwrap().remove(&pc);
        if let Some(entry) = entry {
            self.l1_current_size.fetch_sub(entry.size, Ordering::Relaxed);
            Self::update_lru(&mut self.l1_lru.lock().unwrap(), pc);

            // 将降级的代码移到L2缓存
            self.insert_to_l2(pc, entry);
            self.stats.lock().unwrap().evictions.fetch_add(1, Ordering::Relaxed);
            self.stats.lock().unwrap().removals.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// 从L2缓存淘汰条目
    fn evict_from_l2(&mut self, pc: GuestAddr) {
        let entry = self.l2_cache.lock().unwrap().remove(&pc);
        if let Some(entry) = entry {
            self.l2_current_size.fetch_sub(entry.size, Ordering::Relaxed);
            Self::update_lru(&mut self.l2_lru.lock().unwrap(), pc);

            // 将降级的代码移到L3缓存
            self.insert_to_l3(pc, entry);
            self.stats.lock().unwrap().evictions.fetch_add(1, Ordering::Relaxed);
            self.stats.lock().unwrap().removals.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// 从L3缓存淘汰条目
    fn evict_from_l3(&mut self, pc: GuestAddr) {
        let entry = self.l3_cache.lock().unwrap().remove(&pc);
        if let Some(entry) = entry {
            self.l3_current_size.fetch_sub(entry.size, Ordering::Relaxed);
            Self::update_lru(&mut self.l3_lru.lock().unwrap(), pc);
            self.stats.lock().unwrap().evictions.fetch_add(1, Ordering::Relaxed);
            self.stats.lock().unwrap().removals.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// 插入到L1缓存
    fn insert_to_l1(&mut self, pc: GuestAddr, mut entry: OptimizedCacheEntry) {
        self.ensure_l1_space(entry.size);
        entry.cache_level = 1;
        entry.last_access_ns.store(Self::current_time_ns(), Ordering::Relaxed);

        self.l1_cache.lock().unwrap().insert(pc, entry.clone());
        Self::update_lru(&mut self.l1_lru.lock().unwrap(), pc);
        self.l1_current_size.fetch_add(entry.size, Ordering::Relaxed);
        self.stats.lock().unwrap().inserts.fetch_add(1, Ordering::Relaxed);
    }

    /// 插入到L2缓存
    fn insert_to_l2(&mut self, pc: GuestAddr, mut entry: OptimizedCacheEntry) {
        self.ensure_l2_space(entry.size);
        entry.cache_level = 2;
        entry.last_access_ns.store(Self::current_time_ns(), Ordering::Relaxed);

        self.l2_cache.lock().unwrap().insert(pc, entry.clone());
        Self::update_lru(&mut self.l2_lru.lock().unwrap(), pc);
        self.l2_current_size.fetch_add(entry.size, Ordering::Relaxed);
        self.stats.lock().unwrap().inserts.fetch_add(1, Ordering::Relaxed);
    }

    /// 插入到L3缓存
    fn insert_to_l3(&mut self, pc: GuestAddr, mut entry: OptimizedCacheEntry) {
        self.ensure_l3_space(entry.size);
        entry.cache_level = 3;
        entry.last_access_ns.store(Self::current_time_ns(), Ordering::Relaxed);

        self.l3_cache.lock().unwrap().insert(pc, entry);
        Self::update_lru(&mut self.l3_lru.lock().unwrap(), pc);
        self.l3_current_size.fetch_add(entry.size, Ordering::Relaxed);
        self.stats.lock().unwrap().inserts.fetch_add(1, Ordering::Relaxed);
    }

    /// 检查是否应该提升到热点缓存
    fn should_promote_to_hotspot(&self, entry: &OptimizedCacheEntry) -> bool {
        entry.access_count.load(Ordering::Relaxed) >= self.config.hotspot_threshold as u64
    }

    /// 提升热点代码到L1缓存
    fn promote_to_hotspot(&mut self, pc: GuestAddr) {
        let entry = self.l2_cache.lock().unwrap().remove(&pc);
        if let Some(entry) = entry {
            self.l2_current_size.fetch_sub(entry.size, Ordering::Relaxed);
            Self::update_lru(&mut self.l2_lru.lock().unwrap(), pc);

            self.insert_to_l1(pc, entry);
            self.stats.lock().unwrap().hotspot_promotions.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// 预取相邻代码块
    fn prefetch_neighbors(&mut self, pc: GuestAddr) {
        for offset in 1..=self.config.prefetch_window {
            let neighbor_pc = pc + (offset * self.config.cache_line_size) as u64;

            // 检查L3缓存中是否有相邻代码
            if let Some(entry) = self.l3_cache.lock().unwrap().get(&neighbor_pc).cloned() {
                if !entry.prefetch_mark {
                    // 预取到L2缓存
                    self.insert_to_l2(neighbor_pc, entry);
                    self.stats.lock().unwrap().prefetch_hits.fetch_add(1, Ordering::Relaxed);
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
        self.l1_cache.lock().unwrap().contains_key(&pc)
            || self.l2_cache.lock().unwrap().contains_key(&pc)
            || self.l3_cache.lock().unwrap().contains_key(&pc)
    }

    fn remove(&mut self, pc: GuestAddr) -> Option<Vec<u8>> {
        if let Some(entry) = self.l1_cache.lock().unwrap().remove(&pc) {
            self.l1_current_size.fetch_sub(entry.size, Ordering::Relaxed);
            self.l1_lru.lock().unwrap().retain(|&x| x != pc);
            return Some(entry.code);
        }

        if let Some(entry) = self.l2_cache.lock().unwrap().remove(&pc) {
            self.l2_current_size.fetch_sub(entry.size, Ordering::Relaxed);
            self.l2_lru.lock().unwrap().retain(|&x| x != pc);
            return Some(entry.code);
        }

        if let Some(entry) = self.l3_cache.lock().unwrap().remove(&pc) {
            self.l3_current_size.fetch_sub(entry.size, Ordering::Relaxed);
            self.l3_lru.lock().unwrap().retain(|&x| x != pc);
            return Some(entry.code);
        }

        None
    }

    fn clear(&mut self) {
        self.l1_cache.lock().unwrap().clear();
        self.l2_cache.lock().unwrap().clear();
        self.l3_cache.lock().unwrap().clear();
        self.l1_lru.lock().unwrap().clear();
        self.l2_lru.lock().unwrap().clear();
        self.l3_lru.lock().unwrap().clear();
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
            inserts: self.stats.inserts.load(Ordering::Relaxed),
            removals: self.stats.removals.load(Ordering::Relaxed),
            max_size: self.config.l3_size,
            current_size: self.l3_current_size.load(Ordering::Relaxed)
                + self.l2_current_size.load(Ordering::Relaxed)
                + self.l1_current_size.load(Ordering::Relaxed),
            entry_count: self.l1_cache.lock().unwrap().len() + self.l2_cache.lock().unwrap().len() + self.l3_cache.lock().unwrap().len(),
            hit_rate: if total_accesses > 0 {
                total_hits as f64 / total_accesses as f64
            } else {
                0.0
            },
        }
    }

    fn set_size_limit(&mut self, limit: usize) {
        self.config.l3_size = limit;
        while self.current_size() > limit {
            if let Some(oldest_pc) = self.l3_lru.lock().unwrap().front().cloned() {
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
        self.l1_cache.lock().unwrap().len() + self.l2_cache.lock().unwrap().len() + self.l3_cache.lock().unwrap().len()
    }
}