//! VirtIO 零拷贝数据传输优化
//!
//! 提供零拷贝 scatter-gather 操作、内存预留和缓冲区池管理，
//! 以优化 VirtIO 设备的数据传输性能。
//!
//! # 主要特性
//! - ScatterGather 列表，支持非连续内存
//! - BufferPool 缓冲区预留，避免重复分配
//! - DirectMemoryAccess 直接内存访问
//! - ZeroCopyChain 零拷贝链路管理
//! - MemoryMapping 内存映射缓存

use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Arc, Mutex, RwLock};

use vm_core::{MemoryError, VmError, VmResult};

/// 内存映射缓存条目
#[derive(Clone, Debug)]
pub struct MappingEntry {
    /// 虚拟地址范围
    pub vaddr_range: Range<u64>,
    /// 物理地址
    pub paddr: u64,
    /// 缓存是否有效
    pub valid: bool,
}

/// 内存映射缓存
///
/// 缓存虚拟地址到物理地址的映射，避免重复 MMU 查询。
#[derive(Clone)]
pub struct MappingCache {
    /// 映射缓存表
    mappings: Arc<RwLock<HashMap<u64, MappingEntry>>>,
    /// 最大缓存大小
    max_entries: usize,
}

impl MappingCache {
    /// 创建新的映射缓存
    pub fn new(max_entries: usize) -> Self {
        Self {
            mappings: Arc::new(RwLock::new(HashMap::new())),
            max_entries,
        }
    }

    /// Helper to acquire mappings write lock with error handling
    fn lock_mappings_write(
        &self,
    ) -> VmResult<std::sync::RwLockWriteGuard<'_, HashMap<u64, MappingEntry>>> {
        self.mappings.write().map_err(|_| {
            VmError::Memory(MemoryError::PageTableError {
                message: "MappingCache mappings lock is poisoned".to_string(),
                level: None,
            })
        })
    }

    /// Helper to acquire mappings read lock with error handling
    fn lock_mappings_read(
        &self,
    ) -> VmResult<std::sync::RwLockReadGuard<'_, HashMap<u64, MappingEntry>>> {
        self.mappings.read().map_err(|_| {
            VmError::Memory(MemoryError::PageTableError {
                message: "MappingCache mappings lock is poisoned".to_string(),
                level: None,
            })
        })
    }

    /// 查询映射
    pub fn lookup(&self, vaddr: u64) -> Option<MappingEntry> {
        let mappings = match self.lock_mappings_read() {
            Ok(guard) => guard,
            Err(_) => return None,
        };
        for (_, entry) in mappings.iter() {
            if entry.vaddr_range.contains(&vaddr) && entry.valid {
                return Some(entry.clone());
            }
        }
        None
    }

    /// 插入映射
    pub fn insert(&self, vaddr: u64, entry: MappingEntry) -> bool {
        let mut mappings = match self.lock_mappings_write() {
            Ok(guard) => guard,
            Err(_) => return false,
        };

        if mappings.len() >= self.max_entries {
            // 简单的 LRU 策略：删除第一个条目
            if let Some(key) = mappings.keys().next().cloned() {
                mappings.remove(&key);
            }
        }

        mappings.insert(vaddr, entry);
        true
    }

    /// 清除缓存
    pub fn clear(&self) {
        if let Ok(mut mappings) = self.lock_mappings_write() {
            mappings.clear();
        }
    }

    /// 获取缓存大小
    pub fn size(&self) -> usize {
        match self.lock_mappings_read() {
            Ok(mappings) => mappings.len(),
            Err(_) => 0,
        }
    }
}

/// Scatter-Gather 片段
#[derive(Clone, Debug)]
pub struct SgSegment {
    /// 物理地址
    pub paddr: u64,
    /// 大小
    pub len: u32,
    /// 标志位（VIRTIO_DESC_F_NEXT 等）
    pub flags: u16,
}

/// Scatter-Gather 列表
///
/// 支持非连续内存的高效数据传输。
#[derive(Clone)]
pub struct ScatterGatherList {
    /// 片段列表
    pub segments: Vec<SgSegment>,
    /// 总大小
    pub total_len: u64,
}

impl ScatterGatherList {
    /// 创建空的 SG 列表
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
            total_len: 0,
        }
    }

    /// 添加片段
    pub fn add_segment(&mut self, paddr: u64, len: u32, flags: u16) {
        self.segments.push(SgSegment { paddr, len, flags });
        self.total_len += len as u64;
    }

    /// 获取片段数
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// 检查是否连续
    pub fn is_contiguous(&self) -> bool {
        if self.segments.is_empty() {
            return true;
        }

        for i in 0..self.segments.len() - 1 {
            let current = &self.segments[i];
            let next = &self.segments[i + 1];
            if current.paddr + current.len as u64 != next.paddr {
                return false;
            }
        }

        true
    }

    /// 获取第一段的物理地址（如果连续）
    pub fn first_paddr(&self) -> Option<u64> {
        self.segments.first().map(|s| s.paddr)
    }

    /// 获取诊断信息
    pub fn diagnostic_report(&self) -> String {
        format!(
            "ScatterGatherList: {} segments, {} bytes total, contiguous={}",
            self.segments.len(),
            self.total_len,
            self.is_contiguous()
        )
    }
}

impl Default for ScatterGatherList {
    fn default() -> Self {
        Self::new()
    }
}

/// 缓冲区池条目
#[derive(Clone)]
struct BufferPoolEntry {
    /// 缓冲区数据
    pub data: Arc<Vec<u8>>,
    /// 是否被使用
    pub in_use: bool,
}

/// 缓冲区池
///
/// 预先分配缓冲区，减少运行时分配开销。
pub struct BufferPool {
    /// 缓冲区池
    pool: Arc<Mutex<Vec<BufferPoolEntry>>>,
    /// 缓冲区大小
    buffer_size: usize,
    /// 池大小
    pool_size: usize,
    /// 统计：分配次数
    allocations: Arc<Mutex<u64>>,
    /// 统计：重用次数
    reuses: Arc<Mutex<u64>>,
}

impl BufferPool {
    /// 创建缓冲区池
    pub fn new(buffer_size: usize, pool_size: usize) -> Self {
        let mut pool = Vec::new();
        for _ in 0..pool_size {
            pool.push(BufferPoolEntry {
                data: Arc::new(vec![0u8; buffer_size]),
                in_use: false,
            });
        }

        Self {
            pool: Arc::new(Mutex::new(pool)),
            buffer_size,
            pool_size,
            allocations: Arc::new(Mutex::new(0)),
            reuses: Arc::new(Mutex::new(0)),
        }
    }

    /// Helper to acquire pool lock with error handling
    fn lock_pool(&self) -> VmResult<std::sync::MutexGuard<'_, Vec<BufferPoolEntry>>> {
        self.pool.lock().map_err(|_| {
            VmError::Memory(MemoryError::PageTableError {
                message: "BufferPool pool lock is poisoned".to_string(),
                level: None,
            })
        })
    }

    /// Helper to acquire allocations lock with error handling
    fn lock_allocations(&self) -> VmResult<std::sync::MutexGuard<'_, u64>> {
        self.allocations.lock().map_err(|_| {
            VmError::Memory(MemoryError::PageTableError {
                message: "BufferPool allocations lock is poisoned".to_string(),
                level: None,
            })
        })
    }

    /// Helper to acquire reuses lock with error handling
    fn lock_reuses(&self) -> VmResult<std::sync::MutexGuard<'_, u64>> {
        self.reuses.lock().map_err(|_| {
            VmError::Memory(MemoryError::PageTableError {
                message: "BufferPool reuses lock is poisoned".to_string(),
                level: None,
            })
        })
    }

    /// 从池中分配缓冲区
    pub fn allocate(&self) -> Arc<Vec<u8>> {
        let mut pool = match self.lock_pool() {
            Ok(guard) => guard,
            Err(_) => return Arc::new(vec![0u8; self.buffer_size]),
        };

        // 查找未被使用的缓冲区
        for entry in pool.iter_mut() {
            if !entry.in_use {
                entry.in_use = true;
                if let Ok(mut reuses) = self.lock_reuses() {
                    *reuses += 1;
                }
                return Arc::clone(&entry.data);
            }
        }

        // 没有可用的缓冲区，创建新的
        if let Ok(mut allocs) = self.lock_allocations() {
            *allocs += 1;
        }
        Arc::new(vec![0u8; self.buffer_size])
    }

    /// 释放缓冲区到池中
    pub fn release(&self, _buffer: Arc<Vec<u8>>) {
        if let Ok(mut pool) = self.lock_pool() {
            for entry in pool.iter_mut() {
                if entry.in_use {
                    entry.in_use = false;
                    break;
                }
            }
        }
    }

    /// 获取统计信息
    pub fn stats(&self) -> (u64, u64) {
        let allocs = match self.lock_allocations() {
            Ok(guard) => *guard,
            Err(_) => 0,
        };
        let reuses = match self.lock_reuses() {
            Ok(guard) => *guard,
            Err(_) => 0,
        };
        (allocs, reuses)
    }

    /// 获取池中可用缓冲区数
    pub fn available_count(&self) -> usize {
        match self.lock_pool() {
            Ok(pool) => pool.iter().filter(|e| !e.in_use).count(),
            Err(_) => 0,
        }
    }

    /// 诊断报告
    pub fn diagnostic_report(&self) -> String {
        let (allocs, reuses) = self.stats();
        format!(
            "BufferPool: size={}, buffer_size={}, available={}, allocations={}, reuses={}",
            self.pool_size,
            self.buffer_size,
            self.available_count(),
            allocs,
            reuses
        )
    }
}

/// 直接内存访问管理
///
/// 提供优化的内存读写操作，支持零拷贝 DMA。
pub struct DirectMemoryAccess {
    /// 虚拟地址到物理地址的映射缓存
    mapping_cache: MappingCache,
    /// 统计：缓存命中
    cache_hits: Arc<Mutex<u64>>,
    /// 统计：缓存未中
    cache_misses: Arc<Mutex<u64>>,
}

impl DirectMemoryAccess {
    /// 创建 DMA 管理器
    pub fn new(cache_size: usize) -> Self {
        Self {
            mapping_cache: MappingCache::new(cache_size),
            cache_hits: Arc::new(Mutex::new(0)),
            cache_misses: Arc::new(Mutex::new(0)),
        }
    }

    /// Helper to acquire cache_hits lock with error handling
    fn lock_cache_hits(&self) -> VmResult<std::sync::MutexGuard<'_, u64>> {
        self.cache_hits.lock().map_err(|_| {
            VmError::Memory(MemoryError::PageTableError {
                message: "DirectMemoryAccess cache_hits lock is poisoned".to_string(),
                level: None,
            })
        })
    }

    /// Helper to acquire cache_misses lock with error handling
    fn lock_cache_misses(&self) -> VmResult<std::sync::MutexGuard<'_, u64>> {
        self.cache_misses.lock().map_err(|_| {
            VmError::Memory(MemoryError::PageTableError {
                message: "DirectMemoryAccess cache_misses lock is poisoned".to_string(),
                level: None,
            })
        })
    }

    /// 查询内存映射（带缓存）
    pub fn lookup_mapping(&self, vaddr: u64) -> Option<MappingEntry> {
        if let Some(entry) = self.mapping_cache.lookup(vaddr) {
            if let Ok(mut hits) = self.lock_cache_hits() {
                *hits += 1;
            }
            return Some(entry);
        }

        if let Ok(mut misses) = self.lock_cache_misses() {
            *misses += 1;
        }
        None
    }

    /// 缓存映射
    pub fn cache_mapping(&self, vaddr: u64, entry: MappingEntry) {
        self.mapping_cache.insert(vaddr, entry);
    }

    /// 获取缓存命中率
    pub fn cache_hit_rate(&self) -> f64 {
        let hits = match self.lock_cache_hits() {
            Ok(guard) => *guard,
            Err(_) => 0,
        };
        let misses = match self.lock_cache_misses() {
            Ok(guard) => *guard,
            Err(_) => 0,
        };
        let total = hits + misses;

        if total == 0 {
            return 0.0;
        }

        hits as f64 / total as f64
    }

    /// 清除映射缓存
    pub fn clear_cache(&self) {
        self.mapping_cache.clear();
        if let Ok(mut hits) = self.lock_cache_hits() {
            *hits = 0;
        }
        if let Ok(mut misses) = self.lock_cache_misses() {
            *misses = 0;
        }
    }

    /// 诊断报告
    pub fn diagnostic_report(&self) -> String {
        let hits = match self.lock_cache_hits() {
            Ok(guard) => *guard,
            Err(_) => 0,
        };
        let misses = match self.lock_cache_misses() {
            Ok(guard) => *guard,
            Err(_) => 0,
        };
        format!(
            "DirectMemoryAccess: hits={}, misses={}, hit_rate={:.2}%",
            hits,
            misses,
            self.cache_hit_rate() * 100.0
        )
    }
}

/// 零拷贝链路
///
/// 管理一个完整的零拷贝数据传输操作。
#[derive(Clone)]
pub struct ZeroCopyChain {
    /// 链路 ID
    pub id: u32,
    /// Scatter-Gather 列表
    pub sg_list: ScatterGatherList,
    /// 缓冲区引用
    pub buffer: Option<Arc<Vec<u8>>>,
    /// 传输大小
    pub transferred_len: u32,
    /// 是否完成
    pub completed: bool,
}

impl ZeroCopyChain {
    /// 创建零拷贝链路
    pub fn new(id: u32) -> Self {
        Self {
            id,
            sg_list: ScatterGatherList::new(),
            buffer: None,
            transferred_len: 0,
            completed: false,
        }
    }

    /// 添加 SG 片段
    pub fn add_segment(&mut self, paddr: u64, len: u32, flags: u16) {
        self.sg_list.add_segment(paddr, len, flags);
    }

    /// 设置缓冲区
    pub fn set_buffer(&mut self, buffer: Arc<Vec<u8>>) {
        self.buffer = Some(buffer);
    }

    /// 标记为完成
    pub fn mark_complete(&mut self) {
        self.completed = true;
    }

    /// 诊断报告
    pub fn diagnostic_report(&self) -> String {
        format!(
            "ZeroCopyChain {}: {} segments, {} bytes, completed={}",
            self.id,
            self.sg_list.segment_count(),
            self.sg_list.total_len,
            self.completed
        )
    }
}

/// VirtIO 零拷贝管理器
///
/// 协调零拷贝缓冲区、映射和链路。
pub struct VirtioZeroCopyManager {
    /// 缓冲区池
    buffer_pool: Arc<BufferPool>,
    /// 直接内存访问管理
    dma: Arc<DirectMemoryAccess>,
    /// 活跃的零拷贝链路
    chains: Arc<Mutex<HashMap<u32, ZeroCopyChain>>>,
    /// 下一个链路 ID
    next_chain_id: Arc<Mutex<u32>>,
    /// 统计：完成的链路
    completed_chains: Arc<Mutex<u64>>,
}

impl VirtioZeroCopyManager {
    /// 创建零拷贝管理器
    pub fn new(buffer_pool: Arc<BufferPool>, cache_size: usize) -> Self {
        Self {
            buffer_pool,
            dma: Arc::new(DirectMemoryAccess::new(cache_size)),
            chains: Arc::new(Mutex::new(HashMap::new())),
            next_chain_id: Arc::new(Mutex::new(0)),
            completed_chains: Arc::new(Mutex::new(0)),
        }
    }

    /// Helper to acquire chains lock with error handling
    fn lock_chains(&self) -> VmResult<std::sync::MutexGuard<'_, HashMap<u32, ZeroCopyChain>>> {
        self.chains.lock().map_err(|_| {
            VmError::Memory(MemoryError::PageTableError {
                message: "VirtioZeroCopyManager chains lock is poisoned".to_string(),
                level: None,
            })
        })
    }

    /// Helper to acquire next_chain_id lock with error handling
    fn lock_next_chain_id(&self) -> VmResult<std::sync::MutexGuard<'_, u32>> {
        self.next_chain_id.lock().map_err(|_| {
            VmError::Memory(MemoryError::PageTableError {
                message: "VirtioZeroCopyManager next_chain_id lock is poisoned".to_string(),
                level: None,
            })
        })
    }

    /// Helper to acquire completed_chains lock with error handling
    fn lock_completed_chains(&self) -> VmResult<std::sync::MutexGuard<'_, u64>> {
        self.completed_chains.lock().map_err(|_| {
            VmError::Memory(MemoryError::PageTableError {
                message: "VirtioZeroCopyManager completed_chains lock is poisoned".to_string(),
                level: None,
            })
        })
    }

    /// 创建新的零拷贝链路
    pub fn create_chain(&self) -> ZeroCopyChain {
        let mut next_id = match self.lock_next_chain_id() {
            Ok(guard) => guard,
            Err(_) => return ZeroCopyChain::new(0),
        };
        let id = *next_id;
        *next_id = next_id.wrapping_add(1);

        ZeroCopyChain::new(id)
    }

    /// 注册链路
    pub fn register_chain(&self, chain: ZeroCopyChain) {
        if let Ok(mut chains) = self.lock_chains() {
            chains.insert(chain.id, chain);
        }
    }

    /// 获取链路
    pub fn get_chain(&self, id: u32) -> Option<ZeroCopyChain> {
        match self.lock_chains() {
            Ok(chains) => chains.get(&id).cloned(),
            Err(_) => None,
        }
    }

    /// 完成链路
    pub fn complete_chain(&self, id: u32) -> bool {
        let mut chains = match self.lock_chains() {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        if let Some(chain) = chains.get_mut(&id) {
            chain.mark_complete();
            if let Ok(mut completed) = self.lock_completed_chains() {
                *completed += 1;
            }
            return true;
        }
        false
    }

    /// 删除链路
    pub fn remove_chain(&self, id: u32) -> Option<ZeroCopyChain> {
        match self.lock_chains() {
            Ok(mut chains) => chains.remove(&id),
            Err(_) => None,
        }
    }

    /// 分配缓冲区
    pub fn allocate_buffer(&self) -> Arc<Vec<u8>> {
        self.buffer_pool.allocate()
    }

    /// 释放缓冲区
    pub fn release_buffer(&self, buffer: Arc<Vec<u8>>) {
        self.buffer_pool.release(buffer);
    }

    /// 查询映射
    pub fn lookup_mapping(&self, vaddr: u64) -> Option<MappingEntry> {
        self.dma.lookup_mapping(vaddr)
    }

    /// 缓存映射
    pub fn cache_mapping(&self, vaddr: u64, entry: MappingEntry) {
        self.dma.cache_mapping(vaddr, entry);
    }

    /// 获取活跃链路数
    pub fn active_chains(&self) -> usize {
        match self.lock_chains() {
            Ok(chains) => chains.len(),
            Err(_) => 0,
        }
    }

    /// 获取统计信息
    pub fn stats(&self) -> (u64, u64, f64) {
        let completed = match self.lock_completed_chains() {
            Ok(guard) => *guard,
            Err(_) => 0,
        };
        (
            completed,
            self.active_chains() as u64,
            self.dma.cache_hit_rate(),
        )
    }

    /// 诊断报告
    pub fn diagnostic_report(&self) -> String {
        let (completed, active, hit_rate) = self.stats();
        format!(
            "VirtioZeroCopyManager: completed={}, active={}, cache_hit_rate={:.2}%\n  {}\n  {}",
            completed,
            active,
            hit_rate * 100.0,
            self.buffer_pool.diagnostic_report(),
            self.dma.diagnostic_report()
        )
    }
}
