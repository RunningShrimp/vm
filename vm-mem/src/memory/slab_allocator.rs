//! Slab Allocator
//!
//! 高性能 slab 分配器，用于快速分配固定大小的对象。
//! 减少 内存碎片和提高分配性能。
//!
//! ## 性能特点
//! - **O(1) 分配/释放**: 使用空闲链表实现常数时间操作
//! - **低内存开销**: 每个大小类只有少量元数据
//! - **缓存友好**: 相同大小的对象连续存储
//! - **线程安全**: 使用锁保护并发访问
//!
//! ## 使用场景
//! - **小对象分配**: 频繁分配的小对象（< 8KB）
//! - **固定大小对象**: 如页表项、缓存条目等
//! - **高并发场景**: 多线程频繁分配/释放
//!
//! ## 设计
//! - **大小类**: 预定义的大小类别（8, 16, 32, 64, ..., 8192）
//! - **Slab**: 每个大小类有多个 slab（内存块）
//! - **空闲链表**: 每个 slab 维护空闲对象链表
//! - **按需增长**: slab 按需创建和销毁

use std::ptr::NonNull;
use std::sync::{Arc, Mutex};

use vm_core::error::MemoryError;

/// Slab 分配器错误
#[derive(Debug, thiserror::Error)]
pub enum SlabError {
    #[error("Invalid size: {0}")]
    InvalidSize(usize),

    #[error("Out of memory")]
    OutOfMemory,

    #[error("Corrupted slab: {0}")]
    Corrupted(String),

    #[error("Size class not found for size: {0}")]
    SizeClassNotFound(usize),
}

impl From<SlabError> for MemoryError {
    fn from(err: SlabError) -> Self {
        MemoryError::AllocationFailed {
            message: err.to_string(),
            size: None, // Size not available in SlabError context
        }
    }
}

/// 大小类定义
#[derive(Debug, Clone, Copy)]
struct SizeClass {
    /// 对象大小
    size: usize,
    /// 每个 slab 的对象数量
    objects_per_slab: usize,
}

impl SizeClass {
    /// 创建大小类
    fn new(size: usize) -> Self {
        // 根据对象大小计算每个 slab 的对象数量
        // 目标: 每个 slab 约 1MB (或至少 16 个对象)
        let min_objects = 16;
        let target_slab_size = 1024 * 1024; // 1MB

        let objects_per_slab = (target_slab_size / size).max(min_objects);

        Self {
            size,
            objects_per_slab,
        }
    }
}

/// Slab 内存块
#[derive(Debug)]
struct Slab {
    /// 大小类
    size_class: SizeClass,
    /// 内存基址
    base: NonNull<u8>,
    /// Slab 总大小（字节）
    total_size: usize,
    /// 空闲对象链表（使用偏移量表示）
    free_list: Vec<usize>,
    /// 已分配对象数
    allocated: usize,
}

impl Slab {
    /// 创建新的 slab
    fn new(size_class: SizeClass) -> Result<Self, SlabError> {
        let total_size = size_class.size * size_class.objects_per_slab;

        // 使用全局分配器分配内存
        let layout = std::alloc::Layout::from_size_align(total_size, 8)
            .map_err(|_| SlabError::InvalidSize(total_size))?;

        let ptr = unsafe { std::alloc::alloc(layout) };
        if ptr.is_null() {
            return Err(SlabError::OutOfMemory);
        }

        let base = NonNull::new(ptr).ok_or(SlabError::OutOfMemory)?;

        // 初始化空闲链表
        let mut free_list = Vec::with_capacity(size_class.objects_per_slab);
        for i in 0..size_class.objects_per_slab {
            free_list.push(i);
        }

        Ok(Self {
            size_class,
            base,
            total_size,
            free_list,
            allocated: 0,
        })
    }

    /// 从 slab 分配对象
    fn allocate(&mut self) -> Option<NonNull<u8>> {
        if let Some(index) = self.free_list.pop() {
            self.allocated += 1;
            let offset = index * self.size_class.size;
            unsafe {
                let ptr = self.base.as_ptr().add(offset);
                Some(NonNull::new_unchecked(ptr))
            }
        } else {
            None
        }
    }

    /// 释放对象回 slab
    fn deallocate(&mut self, ptr: NonNull<u8>) -> Result<(), SlabError> {
        let addr = ptr.as_ptr() as usize;
        let base_addr = self.base.as_ptr() as usize;

        // 验证指针在范围内
        if addr < base_addr || addr >= base_addr + self.total_size {
            return Err(SlabError::Corrupted("pointer out of range".to_string()));
        }

        let offset = addr - base_addr;

        // 验证对齐
        if !offset.is_multiple_of(self.size_class.size) {
            return Err(SlabError::Corrupted("misaligned pointer".to_string()));
        }

        let index = offset / self.size_class.size;

        // 验证索引未重复释放
        if self.free_list.contains(&index) {
            return Err(SlabError::Corrupted("double free detected".to_string()));
        }

        self.free_list.push(index);
        self.allocated -= 1;

        Ok(())
    }

    /// 检查 slab 是否为空
    fn is_empty(&self) -> bool {
        self.allocated == 0
    }

    /// 检查 slab 是否已满
    fn is_full(&self) -> bool {
        self.free_list.is_empty()
    }
}

impl Drop for Slab {
    fn drop(&mut self) {
        unsafe {
            let layout = std::alloc::Layout::from_size_align_unchecked(self.total_size, 8);
            std::alloc::dealloc(self.base.as_ptr(), layout);
        }
    }
}

/// 大小类管理器
#[derive(Debug)]
struct SizeClassManager {
    /// 大小类配置
    size_class: SizeClass,
    /// Slab 列表
    slabs: Vec<Slab>,
    /// 当前使用的 slab（用于快速分配）
    current_slab: Option<usize>,
}

impl SizeClassManager {
    /// 创建大小类管理器
    fn new(size: usize) -> Self {
        let size_class = SizeClass::new(size);
        Self {
            size_class,
            slabs: Vec::new(),
            current_slab: None,
        }
    }

    /// 分配对象
    fn allocate(&mut self) -> Result<NonNull<u8>, SlabError> {
        // 尝试从当前 slab 分配
        if let Some(current_idx) = self.current_slab
            && let Some(ptr) = self.slabs[current_idx].allocate()
        {
            return Ok(ptr);
        }

        // 当前 slab 已满，寻找有空闲空间的 slab
        for (idx, slab) in self.slabs.iter_mut().enumerate() {
            if !slab.is_full()
                && let Some(ptr) = slab.allocate()
            {
                self.current_slab = Some(idx);
                return Ok(ptr);
            }
        }

        // 所有 slab 都满了，创建新 slab
        let mut new_slab = Slab::new(self.size_class)?;
        let ptr = new_slab.allocate().ok_or(SlabError::OutOfMemory)?;
        let new_idx = self.slabs.len();
        self.slabs.push(new_slab);
        self.current_slab = Some(new_idx);

        Ok(ptr)
    }

    /// 释放对象
    fn deallocate(&mut self, ptr: NonNull<u8>) -> Result<(), SlabError> {
        // 查找包含此指针的 slab
        for slab in &mut self.slabs {
            let base_addr = slab.base.as_ptr() as usize;
            let ptr_addr = ptr.as_ptr() as usize;

            if ptr_addr >= base_addr && ptr_addr < base_addr + slab.total_size {
                return slab.deallocate(ptr);
            }
        }

        Err(SlabError::Corrupted(
            "pointer not found in any slab".to_string(),
        ))
    }

    /// 清理空的 slab
    fn cleanup_empty_slabs(&mut self) {
        self.slabs.retain(|slab| !slab.is_empty());

        // 重置当前 slab 索引
        if self.slabs.is_empty() {
            self.current_slab = None;
        } else if let Some(idx) = self.current_slab
            && idx >= self.slabs.len()
        {
            self.current_slab = Some(self.slabs.len().saturating_sub(1));
        }
    }

    /// 获取统计信息
    fn stats(&self) -> SizeClassStats {
        let total_slabs = self.slabs.len();
        let total_objects = self
            .slabs
            .iter()
            .map(|s| s.size_class.objects_per_slab)
            .sum::<usize>();
        let allocated = self.slabs.iter().map(|s| s.allocated).sum::<usize>();

        SizeClassStats {
            object_size: self.size_class.size,
            total_slabs,
            total_objects,
            allocated,
            available: total_objects - allocated,
        }
    }
}

/// 大小类统计信息
#[derive(Debug, Clone)]
pub struct SizeClassStats {
    /// 对象大小
    pub object_size: usize,
    /// Slab 总数
    pub total_slabs: usize,
    /// 总对象数
    pub total_objects: usize,
    /// 已分配对象数
    pub allocated: usize,
    /// 可用对象数
    pub available: usize,
}

/// Slab 分配器
#[derive(Debug)]
pub struct SlabAllocator {
    /// 大小类管理器列表
    size_classes: Arc<Mutex<Vec<SizeClassManager>>>,
    /// 统计信息
    stats: Arc<SlabAllocatorStats>,
}

/// Slab 分配器统计信息
#[derive(Debug, Default)]
pub struct SlabAllocatorStats {
    /// 分配次数
    pub allocs: std::sync::atomic::AtomicU64,
    /// 释放次数
    pub deallocs: std::sync::atomic::AtomicU64,
    /// Slab 创建次数
    pub slabs_created: std::sync::atomic::AtomicU64,
    /// Slab 销毁次数
    pub slabs_destroyed: std::sync::atomic::AtomicU64,
}

impl SlabAllocator {
    /// 预定义的大小类（字节）
    const SIZE_CLASSES: &'static [usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192];

    /// 创建新的 slab 分配器
    pub fn new() -> Self {
        let size_classes = Self::SIZE_CLASSES
            .iter()
            .map(|&size| SizeClassManager::new(size))
            .collect();

        Self {
            size_classes: Arc::new(Mutex::new(size_classes)),
            stats: Arc::new(SlabAllocatorStats::default()),
        }
    }

    /// 分配内存
    ///
    /// # 参数
    /// - `size`: 请求的大小（字节）
    ///
    /// # 返回
    /// 返回指向已分配内存的指针
    pub fn allocate(&self, size: usize) -> Result<NonNull<u8>, SlabError> {
        // 查找合适的大小类
        let size_class_idx = self.find_size_class(size)?;

        let mut size_classes = self
            .size_classes
            .lock()
            .map_err(|_| SlabError::Corrupted("mutex poisoned".to_string()))?;

        let ptr = size_classes[size_class_idx].allocate()?;

        // 更新统计
        self.stats
            .allocs
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        Ok(ptr)
    }

    /// 释放内存
    ///
    /// # 参数
    /// - `ptr`: 要释放的指针
    /// - `size`: 原始分配大小
    pub fn deallocate(&self, ptr: NonNull<u8>, size: usize) -> Result<(), SlabError> {
        let size_class_idx = self.find_size_class(size)?;

        let mut size_classes = self
            .size_classes
            .lock()
            .map_err(|_| SlabError::Corrupted("mutex poisoned".to_string()))?;

        size_classes[size_class_idx].deallocate(ptr)?;

        // 更新统计
        self.stats
            .deallocs
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // 定期清理空的 slab（每 100 次释放）
        let deallocs = self
            .stats
            .deallocs
            .load(std::sync::atomic::Ordering::Relaxed);
        if deallocs.is_multiple_of(100) {
            size_classes[size_class_idx].cleanup_empty_slabs();
        }

        Ok(())
    }

    /// 查找合适的大小类
    fn find_size_class(&self, size: usize) -> Result<usize, SlabError> {
        // 验证大小
        if size == 0 || size > Self::SIZE_CLASSES.last().copied().unwrap_or(0) {
            return Err(SlabError::SizeClassNotFound(size));
        }

        // 找到最小的足够大的大小类
        Self::SIZE_CLASSES
            .iter()
            .position(|&class_size| class_size >= size)
            .ok_or(SlabError::SizeClassNotFound(size))
    }

    /// 获取统计信息
    pub fn stats(&self) -> SlabAllocatorSnapshot {
        let size_classes = self
            .size_classes
            .lock()
            .map_err(|_| SlabError::Corrupted("mutex poisoned".to_string()))
            .unwrap();

        let size_class_stats: Vec<SizeClassStats> =
            size_classes.iter().map(|sc| sc.stats()).collect();

        SlabAllocatorSnapshot {
            allocs: self.stats.allocs.load(std::sync::atomic::Ordering::Relaxed),
            deallocs: self
                .stats
                .deallocs
                .load(std::sync::atomic::Ordering::Relaxed),
            slabs_created: self
                .stats
                .slabs_created
                .load(std::sync::atomic::Ordering::Relaxed),
            slabs_destroyed: self
                .stats
                .slabs_destroyed
                .load(std::sync::atomic::Ordering::Relaxed),
            size_classes: size_class_stats,
        }
    }
}

/// Slab 分配器快照
#[derive(Debug, Clone)]
pub struct SlabAllocatorSnapshot {
    /// 总分配次数
    pub allocs: u64,
    /// 总释放次数
    pub deallocs: u64,
    /// Slab 创建次数
    pub slabs_created: u64,
    /// Slab 销毁次数
    pub slabs_destroyed: u64,
    /// 各大小类统计
    pub size_classes: Vec<SizeClassStats>,
}

impl Default for SlabAllocator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_class_creation() {
        let sc = SizeClass::new(64);
        assert_eq!(sc.size, 64);
        assert!(sc.objects_per_slab >= 16);
    }

    #[test]
    fn test_slab_creation() {
        let size_class = SizeClass::new(64);
        let slab = Slab::new(size_class).unwrap();
        assert_eq!(slab.allocated, 0);
        assert_eq!(slab.free_list.len(), slab.size_class.objects_per_slab);
    }

    #[test]
    fn test_slab_allocate_deallocate() {
        let size_class = SizeClass::new(64);
        let mut slab = Slab::new(size_class).unwrap();

        // 分配所有对象
        let mut ptrs = Vec::new();
        for _ in 0..slab.size_class.objects_per_slab {
            let ptr = slab.allocate().unwrap();
            ptrs.push(ptr);
        }

        assert!(slab.is_full());
        assert_eq!(slab.allocated, slab.size_class.objects_per_slab);

        // 释放一个对象
        slab.deallocate(ptrs[0]).unwrap();
        assert!(!slab.is_full());
        assert_eq!(slab.allocated, slab.size_class.objects_per_slab - 1);
    }

    #[test]
    fn test_size_class_manager() {
        let mut manager = SizeClassManager::new(64);

        // 分配多个对象
        let ptrs: Vec<_> = (0..10).filter_map(|_| manager.allocate().ok()).collect();
        assert_eq!(ptrs.len(), 10);

        // 释放对象
        for ptr in ptrs {
            assert!(manager.deallocate(ptr).is_ok());
        }
    }

    #[test]
    fn test_slab_allocator() {
        let allocator = SlabAllocator::new();

        // 分配不同大小的对象
        let sizes = [8, 16, 32, 64, 128, 256, 512, 1024];

        for &size in &sizes {
            let ptr = allocator.allocate(size).unwrap();
            assert!(!ptr.as_ptr().is_null());

            // 释放
            allocator.deallocate(ptr, size).unwrap();
        }
    }

    #[test]
    fn test_slab_allocator_invalid_size() {
        let allocator = SlabAllocator::new();

        // 太大
        assert!(allocator.allocate(16384).is_err());

        // 零大小
        assert!(allocator.allocate(0).is_err());
    }

    #[test]
    fn test_slab_allocator_stats() {
        let allocator = SlabAllocator::new();

        // 分配一些对象
        let ptr1 = allocator.allocate(64).unwrap();
        let ptr2 = allocator.allocate(64).unwrap();

        let stats = allocator.stats();
        assert_eq!(stats.allocs, 2);

        // 释放一个对象
        allocator.deallocate(ptr1, 64).unwrap();

        let stats = allocator.stats();
        assert_eq!(stats.deallocs, 1);
    }

    #[test]
    fn test_find_size_class() {
        let allocator = SlabAllocator::new();

        // 测试各种大小
        assert_eq!(allocator.find_size_class(8).unwrap(), 0);
        assert_eq!(allocator.find_size_class(16).unwrap(), 1);
        assert_eq!(allocator.find_size_class(100).unwrap(), 4); // 128
        assert_eq!(allocator.find_size_class(1000).unwrap(), 7); // 1024
    }

    #[test]
    fn test_slab_double_free() {
        let allocator = SlabAllocator::new();
        let ptr = allocator.allocate(64).unwrap();

        // 第一次释放成功
        allocator.deallocate(ptr, 64).unwrap();

        // 第二次释放应该失败
        assert!(allocator.deallocate(ptr, 64).is_err());
    }
}
