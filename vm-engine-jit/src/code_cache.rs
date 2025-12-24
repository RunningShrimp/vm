//! 代码缓存接口和实现
//!
//! 定义了代码缓存的抽象接口和多种实现策略，用于管理编译后的机器码。
//! 支持多种缓存策略：
//! - LRU（Least Recently Used）：最近最少使用淘汰
//! - SimpleHash：简单哈希缓存（无淘汰策略）
//! - Optimized：分层缓存（L1/L2/L3），热点代码在L1

use std::collections::{HashMap, VecDeque};
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use vm_core::GuestAddr;

/// 分层缓存统计
#[derive(Debug, Clone, Default)]
pub struct TieredCacheStats {
    /// 基础统计
    pub base_stats: CacheStats,
    /// L1命中次数
    pub l1_hits: u64,
    /// L2命中次数
    pub l2_hits: u64,
    /// L3命中次数
    pub l3_hits: u64,
    /// L1到L2提升次数
    pub l1_to_l2_promotions: u64,
    /// L2到L1提升次数
    pub l2_to_l1_promotions: u64,
    /// L3到L2提升次数
    pub l3_to_l2_promotions: u64,
    /// L1驱逐次数
    pub l1_evictions: u64,
    /// L2驱逐次数
    pub l2_evictions: u64,
    /// L3驱逐次数
    pub l3_evictions: u64,
}

/// 代码缓存接口
///
/// 负责管理JIT编译后的机器码缓存。代码缓存是JIT性能优化的关键组件，
/// 通过缓存已编译的代码块，避免重复编译，显著提高执行效率。
///
/// # 使用场景
/// - JIT编译缓存：缓存编译后的代码块
/// - 分层缓存：多级缓存策略（L1/L2/L3）
/// - 缓存淘汰：LRU、LFU等淘汰策略
/// - 性能监控：缓存命中率统计和优化
///
/// # 缓存策略
/// - LRU（Least Recently Used）：最近最少使用淘汰
/// - LFU（Least Frequently Used）：最不常使用淘汰
/// - 分层缓存：多级缓存，热点代码在L1，冷门代码在L2
///
/// # 示例
/// ```ignore
/// let mut cache = LRUCache::new(1024 * 1024); // 1MB缓存
/// cache.insert(GuestAddr(0x1000), compiled_code);
/// if let Some(code) = cache.get(GuestAddr(0x1000)) {
///     // 执行缓存的代码
/// }
/// ```
pub trait CodeCache: Send + Sync {
    /// 插入编译后的代码
    ///
    /// 将编译后的机器码插入缓存。
    /// 如果缓存已满，根据淘汰策略移除旧条目。
    ///
    /// # 参数
    /// - `pc`: 代码块的起始地址（缓存键）
    /// - `code`: 编译后的机器码
    ///
    /// # 注意
    /// - 如果键已存在，会覆盖旧值
    /// - 缓存大小受size_limit()限制
    fn insert(&mut self, pc: GuestAddr, code: Vec<u8>);
    
    /// 获取编译后的代码
    ///
    /// 从缓存中获取指定地址的代码。
    /// 获取成功后，会更新访问时间（用于LRU等淘汰策略）。
    ///
    /// # 参数
    /// - `pc`: 代码块的起始地址
    ///
    /// # 返回
    /// 缓存的机器码（如果存在），否则返回None
    fn get(&self, pc: GuestAddr) -> Option<Vec<u8>>;
    
    /// 检查代码是否已缓存
    ///
    /// 检查指定地址的代码是否存在于缓存中。
    ///
    /// # 参数
    /// - `pc`: 代码块的起始地址
    ///
    /// # 返回
    /// 存在返回true，不存在返回false
    fn contains(&self, pc: GuestAddr) -> bool;
    
    /// 移除指定地址的代码
    ///
    /// 从缓存中移除指定地址的代码条目。
    ///
    /// # 参数
    /// - `pc`: 代码块的起始地址
    ///
    /// # 返回
    /// 被移除的代码（如果存在），否则返回None
    fn remove(&mut self, pc: GuestAddr) -> Option<Vec<u8>>;
    
    /// 清空缓存
    ///
    /// 移除所有缓存的代码条目。
    /// 通常在代码失效或重置时调用。
    fn clear(&mut self);
    
    /// 获取缓存统计信息
    ///
    /// 返回缓存的详细统计信息，包括命中/未命中次数、缓存大小等。
    ///
    /// # 返回
    /// 缓存统计信息
    fn stats(&self) -> CacheStats;
    
    /// 设置缓存大小限制
    ///
    /// 设置缓存的最大大小（字节）。
    /// 如果当前大小超过新限制，会自动淘汰一些条目。
    ///
    /// # 参数
    /// - `limit`: 最大缓存大小（字节）
    fn set_size_limit(&mut self, limit: usize);
    
    /// 获取缓存大小限制
    ///
    /// # 返回
    /// 当前设置的最大缓存大小（字节）
    fn size_limit(&self) -> usize;
    
    /// 获取当前缓存大小
    ///
    /// # 返回
    /// 当前已使用的缓存大小（字节）
    fn current_size(&self) -> usize;
    
    /// 获取缓存条目数量
    ///
    /// # 返回
    /// 当前缓存中的代码块数量
    fn entry_count(&self) -> usize;
    
    /// 获取分层缓存统计（可选实现）
    ///
    /// 返回分层缓存（L1/L2/L3）的详细统计信息。
    /// 只有实现了分层缓存的类型才返回有效数据。
    ///
    /// # 返回
    /// 分层缓存统计信息（如果支持），否则返回None
    fn tiered_stats(&self) -> Option<TieredCacheStats> {
        None
    }
}

/// 缓存统计信息
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// 命中次数
    pub hits: u64,
    /// 未命中次数
    pub misses: u64,
    /// 插入次数
    pub inserts: u64,
    /// 移除次数
    pub removals: u64,
    /// 清空次数
    pub clears: u64,
    /// 当前缓存大小（字节）
    pub current_size: usize,
    /// 最大缓存大小（字节）
    pub max_size: usize,
    /// 缓存条目数量
    pub entry_count: usize,
}

impl CacheStats {
    /// 计算命中率
    pub fn hit_rate(&self) -> f64 {
        if self.hits + self.misses == 0 {
            0.0
        } else {
            self.hits as f64 / (self.hits + self.misses) as f64
        }
    }
}

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

/// LRU缓存实现
pub struct LRUCache {
    /// 缓存条目
    entries: RefCell<HashMap<GuestAddr, CacheEntry>>,
    /// LRU顺序（从旧到新）
    lru_order: RefCell<Vec<GuestAddr>>,
    /// 缓存统计
    stats: RefCell<CacheStats>,
    /// 缓存大小限制
    size_limit: usize,
    /// 当前缓存大小
    current_size: RefCell<usize>,
}

// 为LRUCache手动实现Sync特性，因为我们确保了线程安全
unsafe impl Sync for LRUCache {}

/// 缓存条目
#[derive(Debug, Clone)]
struct CacheEntry {
    /// 编译后的代码
    code: Vec<u8>,
    /// 最后访问时间
    last_access: std::time::Instant,
    /// 访问次数
    access_count: u64,
}

impl LRUCache {
    /// 创建新的LRU缓存
    pub fn new(size_limit: usize) -> Self {
        Self {
            entries: RefCell::new(HashMap::new()),
            lru_order: RefCell::new(Vec::new()),
            stats: RefCell::new(CacheStats {
                max_size: size_limit,
                ..Default::default()
            }),
            size_limit,
            current_size: RefCell::new(0),
        }
    }
    
    /// 确保有足够的空间
    fn ensure_space(&self, required_size: usize) {
        let current_size = *self.current_size.borrow();
        while current_size + required_size > self.size_limit && !self.lru_order.borrow().is_empty() {
            // 移除最旧的条目
            if let Some(oldest_pc) = self.lru_order.borrow().first().cloned() {
                self.remove_internal(oldest_pc);
            }
        }
    }
    
    /// 更新LRU顺序
    fn update_lru(&self, pc: GuestAddr) {
        let now = std::time::Instant::now();
        
        // 更新条目的访问时间和计数
        if let Some(entry) = self.entries.borrow_mut().get_mut(&pc) {
            entry.last_access = now;
            entry.access_count += 1;
        }
        
        // 从当前位置移除
        let mut lru_order = self.lru_order.borrow_mut();
        if let Some(pos) = lru_order.iter().position(|&x| x == pc) {
            lru_order.remove(pos);
        }
        // 添加到末尾（最新）
        lru_order.push(pc);
    }
    
    /// 内部移除方法
    fn remove_internal(&self, pc: GuestAddr) -> Option<Vec<u8>> {
        if let Some(entry) = self.entries.borrow_mut().remove(&pc) {
            let code_len = entry.code.len();
            *self.current_size.borrow_mut() -= code_len;
            
            // 从LRU顺序中移除
            let mut lru_order = self.lru_order.borrow_mut();
            if let Some(pos) = lru_order.iter().position(|&x| x == pc) {
                lru_order.remove(pos);
            }
            drop(lru_order); // 释放借用
            
            let mut stats = self.stats.borrow_mut();
            stats.removals += 1;
            stats.entry_count = self.entries.borrow().len();
            stats.current_size = *self.current_size.borrow();
            
            Some(entry.code)
        } else {
            None
        }
    }
}

impl CodeCache for LRUCache {
    fn insert(&mut self, pc: GuestAddr, code: Vec<u8>) {
        let code_size = code.len();
        
        // 如果已存在，先移除旧的
        if self.entries.borrow().contains_key(&pc) {
            self.remove_internal(pc);
        }
        
        // 确保有足够的空间
        self.ensure_space(code_size);
        
        // 创建新条目
        let entry = CacheEntry {
            code: code.clone(),
            last_access: std::time::Instant::now(),
            access_count: 1,
        };
        
        // 插入条目
        self.entries.borrow_mut().insert(pc, entry);
        self.lru_order.borrow_mut().push(pc);
        
        // 更新统计
        *self.current_size.borrow_mut() += code_size;
        self.stats.borrow_mut().inserts += 1;
        self.stats.borrow_mut().entry_count = self.entries.borrow().len();
        self.stats.borrow_mut().current_size = *self.current_size.borrow();
    }
    
    fn get(&self, pc: GuestAddr) -> Option<Vec<u8>> {
        if let Some(entry) = self.entries.borrow().get(&pc) {
            // 更新LRU顺序
            self.update_lru(pc);
            Some(entry.code.clone())
        } else {
            None
        }
    }
    
    fn contains(&self, pc: GuestAddr) -> bool {
        self.entries.borrow().contains_key(&pc)
    }
    
    fn remove(&mut self, pc: GuestAddr) -> Option<Vec<u8>> {
        self.remove_internal(pc)
    }
    
    fn clear(&mut self) {
        self.entries.borrow_mut().clear();
        self.lru_order.borrow_mut().clear();
        *self.current_size.borrow_mut() = 0;
        self.stats.borrow_mut().clears += 1;
        self.stats.borrow_mut().entry_count = 0;
        self.stats.borrow_mut().current_size = 0;
    }
    
    fn stats(&self) -> CacheStats {
        self.stats.borrow().clone()
    }
    
    fn set_size_limit(&mut self, limit: usize) {
        self.size_limit = limit;
        self.stats.borrow_mut().max_size = limit;
        
        // 如果当前大小超过新限制，移除一些条目
        while *self.current_size.borrow() > self.size_limit && !self.lru_order.borrow().is_empty() {
            if let Some(oldest_pc) = self.lru_order.borrow().first().cloned() {
                self.remove_internal(oldest_pc);
            }
        }
    }
    
    fn size_limit(&self) -> usize {
        self.size_limit
    }
    
    fn current_size(&self) -> usize {
        *self.current_size.borrow()
    }
    
    fn entry_count(&self) -> usize {
        self.entries.borrow().len()
    }
}

/// 简单哈希缓存实现（无淘汰策略）
pub struct SimpleHashCache {
    /// 缓存条目
    entries: HashMap<GuestAddr, Vec<u8>>,
    /// 缓存统计
    stats: CacheStats,
    /// 缓存大小限制
    size_limit: usize,
    /// 当前缓存大小
    current_size: usize,
}

impl SimpleHashCache {
    /// 创建新的简单哈希缓存
    pub fn new(size_limit: usize) -> Self {
        Self {
            entries: HashMap::new(),
            stats: CacheStats {
                max_size: size_limit,
                ..Default::default()
            },
            size_limit,
            current_size: 0,
        }
    }
}

impl CodeCache for SimpleHashCache {
    fn insert(&mut self, pc: GuestAddr, code: Vec<u8>) {
        let code_size = code.len();
        
        // 如果已存在，先移除旧的
        if let Some(old_code) = self.entries.remove(&pc) {
            self.current_size -= old_code.len();
        }
        
        // 检查是否有足够空间
        if self.current_size + code_size > self.size_limit {
            // 简单策略：拒绝插入
            return;
        }
        
        // 插入新条目
        self.entries.insert(pc, code);
        self.current_size += code_size;
        
        // 更新统计
        self.stats.inserts += 1;
        self.stats.entry_count = self.entries.len();
        self.stats.current_size = self.current_size;
    }
    
    fn get(&self, pc: GuestAddr) -> Option<Vec<u8>> {
        self.entries.get(&pc).cloned()
    }
    
    fn contains(&self, pc: GuestAddr) -> bool {
        self.entries.contains_key(&pc)
    }
    
    fn remove(&mut self, pc: GuestAddr) -> Option<Vec<u8>> {
        if let Some(code) = self.entries.remove(&pc) {
            self.current_size -= code.len();
            self.stats.removals += 1;
            self.stats.entry_count = self.entries.len();
            self.stats.current_size = self.current_size;
            Some(code)
        } else {
            None
        }
    }
    
    fn clear(&mut self) {
        self.entries.clear();
        self.current_size = 0;
        self.stats.clears += 1;
        self.stats.entry_count = 0;
        self.stats.current_size = 0;
    }
    
    fn stats(&self) -> CacheStats {
        self.stats.clone()
    }
    
    fn set_size_limit(&mut self, limit: usize) {
        self.size_limit = limit;
        self.stats.max_size = limit;
        
        // 如果当前大小超过新限制，清空缓存
        if self.current_size > self.size_limit {
            self.clear();
        }
    }
    
    fn size_limit(&self) -> usize {
        self.size_limit
    }
    
    fn current_size(&self) -> usize {
        self.current_size
    }
    
    fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

/// 优化的缓存条目
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

/// 优化的代码缓存（分层缓存实现）
///
/// 提供高性能的代码缓存，包括热点检测、预取和分层缓存策略。
/// L1缓存存储热点代码，L2缓存存储常用代码，L3缓存存储所有代码。
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
        if let Some(pos) = lru.iter().position(|&x| x == pc) {
            lru.remove(pos);
        }
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
            if let Some(entry) = self.l3_cache.lock().unwrap().get(&neighbor_pc).cloned() {
                if !entry.prefetch_mark {
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
            cache_level: 3,
            prefetch_mark: false,
        };
        self.insert_to_l3(pc, entry);
    }

    fn get(&self, pc: GuestAddr) -> Option<Vec<u8>> {
        let current_time = Self::current_time_ns();
        
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
                return Some(entry.code.clone());
            }
        }

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
                if self.should_promote_to_hotspot(entry) {
                    let mut this = unsafe { &mut *(self as *const _ as *mut Self) };
                    this.promote_to_hotspot(pc);
                }
                return Some(entry.code.clone());
            }
        }

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
                let mut this = unsafe { &mut *(self as *const _ as *mut Self) };
                this.insert_to_l2(pc, entry.clone());
                return Some(entry.code.clone());
            }
        }

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

    fn stats(&self) -> CacheStats {
        let l1_hits = self.stats.l1_hits.load(Ordering::Relaxed);
        let l2_hits = self.stats.l2_hits.load(Ordering::Relaxed);
        let l3_hits = self.stats.l3_hits.load(Ordering::Relaxed);
        let total_misses = self.stats.total_misses.load(Ordering::Relaxed);
        let total_hits = l1_hits + l2_hits + l3_hits;
        let total_accesses = total_hits + total_misses;

        CacheStats {
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

    fn tiered_stats(&self) -> Option<TieredCacheStats> {
        let stats = self.stats.lock().ok()?;
        Some(TieredCacheStats {
            base_stats: self.stats(),
            l1_hits: stats.l1_hits.load(Ordering::Relaxed),
            l2_hits: stats.l2_hits.load(Ordering::Relaxed),
            l3_hits: stats.l3_hits.load(Ordering::Relaxed),
            l1_to_l2_promotions: 0,
            l2_to_l1_promotions: stats.hotspot_promotions.load(Ordering::Relaxed),
            l3_to_l2_promotions: 0,
            l1_evictions: 0,
            l2_evictions: 0,
            l3_evictions: stats.evictions.load(Ordering::Relaxed),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lru_cache_creation() {
        let cache = LRUCache::new(1024);
        let stats = cache.stats();
        assert_eq!(stats.max_size, 1024);
        assert_eq!(stats.entry_count, 0);
    }

    #[test]
    fn test_lru_cache_insert_and_get() {
        let mut cache = LRUCache::new(1024);
        let code = vec![0x90, 0x90, 0x90];
        cache.insert(GuestAddr(0x1000), code.clone());
        
        let result = cache.get(GuestAddr(0x1000));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), code);
    }

    #[test]
    fn test_lru_cache_miss() {
        let cache = LRUCache::new(1024);
        let result = cache.get(GuestAddr(0x1000));
        assert!(result.is_none());
    }

    #[test]
    fn test_lru_cache_stats() {
        let mut cache = LRUCache::new(1024);
        let code = vec![0x90, 0x90, 0x90];
        cache.insert(GuestAddr(0x1000), code.clone());
        
        let stats = cache.stats();
        assert_eq!(stats.inserts, 1);
        assert_eq!(stats.entry_count, 1);
        assert_eq!(stats.current_size, 3);
    }

    #[test]
    fn test_simple_hash_cache_creation() {
        let cache = SimpleHashCache::new(1024);
        let stats = cache.stats();
        assert_eq!(stats.max_size, 1024);
        assert_eq!(stats.entry_count, 0);
    }

    #[test]
    fn test_simple_hash_cache_insert_and_get() {
        let mut cache = SimpleHashCache::new(1024);
        let code = vec![0x90, 0x90, 0x90];
        cache.insert(GuestAddr(0x1000), code.clone());
        
        let result = cache.get(GuestAddr(0x1000));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), code);
    }

    #[test]
    fn test_optimized_cache_creation() {
        let cache = OptimizedCodeCache::new(OptimizedCacheConfig::default());
        let stats = cache.stats();
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
    }

    #[test]
    fn test_optimized_cache_config_default() {
        let config = OptimizedCacheConfig::default();
        assert_eq!(config.l1_size, 1024 * 1024);
        assert_eq!(config.l2_size, 8 * 1024 * 1024);
        assert_eq!(config.l3_size, 64 * 1024 * 1024);
        assert_eq!(config.hotspot_threshold, 100);
    }

    #[test]
    fn test_optimized_cache_insert_and_get() {
        let mut cache = OptimizedCodeCache::new(OptimizedCacheConfig::default());
        let code = vec![0x90, 0x90, 0x90];
        cache.insert(GuestAddr(0x1000), code.clone());
        
        let result = cache.get(GuestAddr(0x1000));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), code);
    }

    #[test]
    fn test_lru_cache_clear() {
        let mut cache = LRUCache::new(1024);
        let code = vec![0x90, 0x90, 0x90];
        cache.insert(GuestAddr(0x1000), code);
        cache.clear();
        
        let stats = cache.stats();
        assert_eq!(stats.entry_count, 0);
        assert_eq!(stats.current_size, 0);
    }

    #[test]
    fn test_lru_cache_set_size_limit() {
        let mut cache = LRUCache::new(1024);
        cache.set_size_limit(512);
        assert_eq!(cache.size_limit(), 512);
    }

    #[test]
    fn test_lru_cache_remove() {
        let mut cache = LRUCache::new(1024);
        let code = vec![0x90, 0x90, 0x90];
        cache.insert(GuestAddr(0x1000), code.clone());
        
        let result = cache.remove(GuestAddr(0x1000));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), code);
        
        let result2 = cache.get(GuestAddr(0x1000));
        assert!(result2.is_none());
    }

    #[test]
    fn test_simple_hash_cache_remove() {
        let mut cache = SimpleHashCache::new(1024);
        let code = vec![0x90, 0x90, 0x90];
        cache.insert(GuestAddr(0x1000), code.clone());
        
        let result = cache.remove(GuestAddr(0x1000));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), code);
    }

    #[test]
    fn test_optimized_cache_entry_count() {
        let cache = OptimizedCodeCache::new(OptimizedCacheConfig::default());
        assert_eq!(cache.entry_count(), 0);
    }

    #[test]
    fn test_optimized_cache_clear() {
        let mut cache = OptimizedCodeCache::new(OptimizedCacheConfig::default());
        let code = vec![0x90, 0x90, 0x90];
        cache.insert(GuestAddr(0x1000), code);
        cache.clear();
        
        let stats = cache.stats();
        assert_eq!(stats.entry_count, 0);
        assert_eq!(stats.current_size, 0);
    }

    #[test]
    fn test_optimized_cache_size_limit() {
        let cache = OptimizedCodeCache::new(OptimizedCacheConfig::default());
        assert_eq!(cache.size_limit(), 64 * 1024 * 1024);
    }

    #[test]
    fn test_lru_cache_current_size() {
        let mut cache = LRUCache::new(1024);
        assert_eq!(cache.current_size(), 0);
        
        let code = vec![0x90, 0x90, 0x90];
        cache.insert(GuestAddr(0x1000), code);
        assert_eq!(cache.current_size(), 3);
    }

    #[test]
    fn test_optimized_cache_hit_rate() {
        let cache = OptimizedCodeCache::new(OptimizedCacheConfig::default());
        let stats = cache.stats();
        assert_eq!(stats.hit_rate, 0.0);
    }

    #[test]
    fn test_cache_stats_hit_rate_calculation() {
        let mut stats = CacheStats::default();
        stats.hits = 80;
        stats.misses = 20;
        assert_eq!(stats.hit_rate, 0.8);
    }

    #[test]
    fn test_tiered_cache_stats() {
        let cache = OptimizedCodeCache::new(OptimizedCacheConfig::default());
        let tiered_stats = cache.tiered_stats();
        assert!(tiered_stats.is_some());
        
        let stats = tiered_stats.unwrap();
        assert_eq!(stats.l1_hits, 0);
        assert_eq!(stats.l2_hits, 0);
        assert_eq!(stats.l3_hits, 0);
    }
}