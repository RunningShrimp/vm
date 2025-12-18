//! 代码缓存接口和实现
//!
//! 定义了代码缓存的抽象接口和多种实现策略，用于管理编译后的机器码。

use std::collections::HashMap;
use std::cell::RefCell;
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
pub trait CodeCache: Send + Sync {
    /// 插入编译后的代码
    fn insert(&mut self, pc: GuestAddr, code: Vec<u8>);
    
    /// 获取编译后的代码
    fn get(&self, pc: GuestAddr) -> Option<Vec<u8>>;
    
    /// 检查代码是否已缓存
    fn contains(&self, pc: GuestAddr) -> bool;
    
    /// 移除指定地址的代码
    fn remove(&mut self, pc: GuestAddr) -> Option<Vec<u8>>;
    
    /// 清空缓存
    fn clear(&mut self);
    
    /// 获取缓存统计信息
    fn stats(&self) -> CacheStats;
    
    /// 设置缓存大小限制
    fn set_size_limit(&mut self, limit: usize);
    
    /// 获取缓存大小限制
    fn size_limit(&self) -> usize;
    
    /// 获取当前缓存大小
    fn current_size(&self) -> usize;
    
    /// 获取缓存条目数量
    fn entry_count(&self) -> usize;
    
    /// 获取分层缓存统计（可选实现）
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