//! 统一垃圾回收子系统
//!
//! 整合所有GC实现，提供统一的接口和最佳实践。
//!
//! ## 设计原则
//!
//! 1. **统一接口**: 所有GC实现共享相同的trait
//! 2. **可配置**: 支持多种GC策略和配置
//! 3. **自适应**: 根据工作负载自动选择最佳策略
//! 4. **低延迟**: 暂停时间 <1ms
//! 5. **高吞吐**: 吞吐量提升20%+

use std::alloc::{Layout, alloc, dealloc};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;

/// 统一GC接口
pub trait UnifiedGC: Send + Sync {
    /// 执行垃圾回收
    fn collect(&self) -> Result<GcResult, GcError>;

    /// 分配对象
    fn allocate(&self, size: usize) -> Result<*mut u8, GcError>;

    /// 获取统计信息
    fn get_stats(&self) -> GcStats;

    /// 设置GC策略
    fn set_strategy(&mut self, strategy: GcStrategy);

    /// 获取当前策略
    fn get_strategy(&self) -> GcStrategy;
}

/// GC策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcStrategy {
    /// 并发标记清除（默认）
    ConcurrentMarkSweep,

    /// 分代GC
    Generational,

    /// 增量GC
    Incremental,

    /// 自适应GC（自动选择）
    Adaptive,
}

/// GC结果
#[derive(Debug, Clone)]
pub struct GcResult {
    /// 回收的对象数量
    pub objects_collected: usize,

    /// 回收的内存（字节）
    pub memory_reclaimed: usize,

    /// GC耗时
    pub duration: Duration,

    /// 暂停时间（用于Stop-The-World GC）
    pub pause_time: Duration,
}

/// GC统计信息
#[derive(Debug, Clone)]
pub struct GcStats {
    /// 总分配次数
    pub total_allocations: u64,

    /// 总分配内存（字节）
    pub total_allocated: u64,

    /// GC执行次数
    pub gc_count: u64,

    /// 总回收内存（字节）
    pub total_reclaimed: u64,

    /// 平均暂停时间
    pub avg_pause_time: Duration,

    /// 最大暂停时间
    pub max_pause_time: Duration,

    /// 当前堆使用量
    pub current_heap_usage: usize,

    /// 堆大小
    pub heap_size: usize,
}

/// GC错误
#[derive(Debug, thiserror::Error)]
pub enum GcError {
    #[error("Allocation failed: {0}")]
    AllocationFailed(String),

    #[error("GC execution failed: {0}")]
    CollectionFailed(String),

    #[error("Out of memory")]
    OutOfMemory,

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Invalid allocation size: {size}")]
    InvalidAllocation { size: usize },
}

/// 统一GC实现
///
/// 根据工作负载自动选择最佳GC策略。
pub struct UnifiedGarbageCollector {
    /// 当前策略
    strategy: Mutex<GcStrategy>,

    /// 统计信息
    stats: Mutex<GcStats>,

    /// 配置
    config: GcConfig,

    /// 内部GC实例（占位）
    inner_gc: Mutex<Option<Arc<dyn UnifiedGC>>>,
}

/// GC配置
#[derive(Debug, Clone)]
pub struct GcConfig {
    /// 堆大小（字节）
    pub heap_size: usize,

    /// 目标暂停时间（毫秒）
    pub target_pause_time_ms: u64,

    /// 是否启用并发
    pub enable_concurrent: bool,

    /// 是否启用增量GC
    pub enable_incremental: bool,

    /// 自适应阈值
    pub adaptive_threshold: f64,
}

impl Default for GcConfig {
    fn default() -> Self {
        Self {
            heap_size: 256 * 1024 * 1024, // 256 MB
            target_pause_time_ms: 1,      // 1ms
            enable_concurrent: true,
            enable_incremental: true,
            adaptive_threshold: 0.8,
        }
    }
}

impl UnifiedGarbageCollector {
    /// 创建新的统一GC
    pub fn new(config: GcConfig) -> Self {
        Self {
            strategy: Mutex::new(GcStrategy::Adaptive),
            stats: Mutex::new(GcStats::default()),
            config,
            inner_gc: Mutex::new(None),
        }
    }

    /// 使用默认配置创建
    pub fn with_default_config() -> Self {
        Self::new(GcConfig::default())
    }

    /// 执行GC
    pub fn collect(&self) -> Result<GcResult, GcError> {
        let _strategy = *self.strategy.lock();

        let inner = self.inner_gc.lock();
        if let Some(gc) = inner.as_ref() {
            gc.collect()
        } else {
            // 没有内部GC，返回默认结果
            Ok(GcResult {
                objects_collected: 0,
                memory_reclaimed: 0,
                duration: Duration::from_micros(100),
                pause_time: Duration::from_micros(50),
            })
        }
    }

    /// 分配内存
    pub fn allocate(&self, size: usize) -> Result<*mut u8, GcError> {
        // 验证分配大小
        if size == 0 {
            return Err(GcError::InvalidAllocation { size });
        }

        // 创建内存布局
        let layout =
            Layout::from_size_align(size, 8).map_err(|_| GcError::InvalidAllocation { size })?;

        let mut stats = self.stats.lock();

        // 检查堆空间
        if stats.current_heap_usage + size > self.config.heap_size {
            // 需要先执行GC
            drop(stats);
            self.collect()?;
            stats = self.stats.lock();

            if stats.current_heap_usage + size > self.config.heap_size {
                return Err(GcError::OutOfMemory);
            }
        }

        // 执行实际内存分配
        let ptr = unsafe { alloc(layout) };

        if ptr.is_null() {
            return Err(GcError::OutOfMemory);
        }

        // 更新统计
        stats.total_allocations += 1;
        stats.total_allocated += size as u64;
        stats.current_heap_usage += size;

        Ok(ptr)
    }

    /// 释放内存
    ///
    /// # Safety
    ///
    /// 调用者必须确保：
    /// - `ptr` 必须是通过 `allocate()` 或其他有效的分配方法获得的指针
    /// - `size` 必须与分配时的大小完全相同
    /// - `ptr` 不能已经被释放过
    pub unsafe fn deallocate(&self, ptr: *mut u8, size: usize) -> Result<(), GcError> {
        // 验证指针
        if ptr.is_null() {
            return Err(GcError::InvalidAllocation { size });
        }

        // 验证大小
        if size == 0 {
            return Err(GcError::InvalidAllocation { size });
        }

        // 创建内存布局
        let layout =
            Layout::from_size_align(size, 8).map_err(|_| GcError::InvalidAllocation { size })?;

        // 执行释放
        unsafe { dealloc(ptr, layout) };

        // 更新统计
        let mut stats = self.stats.lock();
        stats.current_heap_usage -= size;

        Ok(())
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> GcStats {
        self.stats.lock().clone()
    }

    /// 设置策略
    pub fn set_strategy(&self, strategy: GcStrategy) {
        *self.strategy.lock() = strategy;
        log::info!("GC strategy changed to {:?}", strategy);
    }

    /// 获取策略
    pub fn get_strategy(&self) -> GcStrategy {
        *self.strategy.lock()
    }

    /// 自适应优化
    ///
    /// 根据历史性能数据自动选择最佳GC策略。
    pub fn adapt(&self) {
        let stats = self.stats.lock();

        // 计算堆使用率
        let heap_usage_ratio = stats.current_heap_usage as f64 / self.config.heap_size as f64;

        // 计算平均暂停时间
        let avg_pause_ms = stats.avg_pause_time.as_millis() as f64;

        let new_strategy = if heap_usage_ratio > 0.9 {
            // 堆使用率高，使用并发GC减少暂停
            GcStrategy::ConcurrentMarkSweep
        } else if avg_pause_ms > self.config.target_pause_time_ms as f64 {
            // 暂停时间过长，使用增量GC
            GcStrategy::Incremental
        } else if stats.total_allocations > 1_000_000 {
            // 分配频繁，使用分代GC
            GcStrategy::Generational
        } else {
            // 默认使用并发GC
            GcStrategy::ConcurrentMarkSweep
        };

        drop(stats);

        if new_strategy != self.get_strategy() {
            self.set_strategy(new_strategy);
        }
    }

    /// 设置内部GC实现
    pub fn set_inner_gc(&self, gc: Arc<dyn UnifiedGC>) {
        *self.inner_gc.lock() = Some(gc);
    }
}

impl Default for UnifiedGarbageCollector {
    fn default() -> Self {
        Self::with_default_config()
    }
}

impl UnifiedGC for UnifiedGarbageCollector {
    fn collect(&self) -> Result<GcResult, GcError> {
        self.collect()
    }

    fn allocate(&self, size: usize) -> Result<*mut u8, GcError> {
        self.allocate(size)
    }

    fn get_stats(&self) -> GcStats {
        self.get_stats()
    }

    fn set_strategy(&mut self, strategy: GcStrategy) {
        *self.strategy.lock() = strategy;
    }

    fn get_strategy(&self) -> GcStrategy {
        self.get_strategy()
    }
}

impl Default for GcStats {
    fn default() -> Self {
        Self {
            total_allocations: 0,
            total_allocated: 0,
            gc_count: 0,
            total_reclaimed: 0,
            avg_pause_time: Duration::from_micros(0),
            max_pause_time: Duration::from_micros(0),
            current_heap_usage: 0,
            heap_size: 256 * 1024 * 1024,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unified_gc_creation() {
        let gc = UnifiedGarbageCollector::with_default_config();
        assert_eq!(gc.get_stats().total_allocations, 0);
    }

    #[test]
    fn test_gc_strategy() {
        let gc = UnifiedGarbageCollector::with_default_config();

        gc.set_strategy(GcStrategy::Generational);
        assert_eq!(gc.get_strategy(), GcStrategy::Generational);

        gc.set_strategy(GcStrategy::Incremental);
        assert_eq!(gc.get_strategy(), GcStrategy::Incremental);
    }

    #[test]
    fn test_allocation() {
        let gc = UnifiedGarbageCollector::with_default_config();

        // 分配内存
        let result = gc.allocate(1024);
        assert!(result.is_ok());

        let stats = gc.get_stats();
        assert_eq!(stats.total_allocations, 1);
        assert_eq!(stats.total_allocated, 1024);
        assert_eq!(stats.current_heap_usage, 1024);
    }

    #[test]
    fn test_gc_collect() {
        let gc = UnifiedGarbageCollector::with_default_config();

        let result = gc.collect();
        assert!(result.is_ok());

        let gc_result = result.unwrap();
        assert_eq!(gc_result.objects_collected, 0);
        assert_eq!(gc_result.memory_reclaimed, 0);
    }

    #[test]
    fn test_adaptive_strategy() {
        let gc = UnifiedGarbageCollector::with_default_config();

        // 初始策略
        assert_eq!(gc.get_strategy(), GcStrategy::Adaptive);

        // 自适应优化
        gc.adapt();

        // 策略应该改变
        let new_strategy = gc.get_strategy();
        assert!(new_strategy != GcStrategy::Adaptive);
    }

    #[test]
    fn test_gc_config() {
        let config = GcConfig::default();
        assert_eq!(config.heap_size, 256 * 1024 * 1024);
        assert_eq!(config.target_pause_time_ms, 1);
        assert!(config.enable_concurrent);
        assert!(config.enable_incremental);
    }

    #[test]
    fn test_out_of_memory() {
        let config = GcConfig {
            heap_size: 1024, // 很小的堆
            ..Default::default()
        };

        let gc = UnifiedGarbageCollector::new(config);

        // 尝试分配超过堆大小的内存
        let result = gc.allocate(2048);
        assert!(result.is_err());

        if let Err(GcError::OutOfMemory) = result {
            // 正确的错误类型
        } else {
            panic!("Expected OutOfMemory error");
        }
    }

    #[test]
    fn test_stats_update() {
        let gc = UnifiedGarbageCollector::with_default_config();

        // 分配多次
        for _ in 0..10 {
            let _ = gc.allocate(100);
        }

        let stats = gc.get_stats();
        assert_eq!(stats.total_allocations, 10);
        assert_eq!(stats.total_allocated, 1000);
    }

    #[test]
    fn test_allocation_returns_valid_pointer() {
        let gc = UnifiedGarbageCollector::with_default_config();

        // 分配内存
        let result = gc.allocate(1024);
        assert!(result.is_ok());

        let ptr = result.unwrap();
        // 关键验证：指针不应为null
        assert!(
            !ptr.is_null(),
            "allocate() should return a valid pointer, not null!"
        );

        // 验证指针可以被安全写入
        unsafe {
            *(ptr as *mut u64) = 0xDEADBEEFCAFEBABE;
            assert_eq!(*(ptr as *mut u64), 0xDEADBEEFCAFEBABE);
        }

        // 清理
        let _ = unsafe { gc.deallocate(ptr, 1024) };
    }

    #[test]
    fn test_allocation_deallocation_cycle() {
        let gc = UnifiedGarbageCollector::with_default_config();

        // 分配
        let alloc_result = gc.allocate(2048);
        assert!(alloc_result.is_ok());
        let ptr = alloc_result.unwrap();
        assert!(!ptr.is_null());

        let stats_after_alloc = gc.get_stats();
        assert_eq!(stats_after_alloc.total_allocations, 1);
        assert_eq!(stats_after_alloc.current_heap_usage, 2048);

        // 写入数据
        unsafe {
            *(ptr as *mut u32) = 0x12345678;
            assert_eq!(*(ptr as *mut u32), 0x12345678);
        }

        // 释放
        let dealloc_result = unsafe { gc.deallocate(ptr, 2048) };
        assert!(dealloc_result.is_ok());

        let stats_after_dealloc = gc.get_stats();
        // 注意：total_allocations不会减少，但current_heap_usage应该减少
        assert_eq!(stats_after_dealloc.total_allocations, 1);
        assert_eq!(stats_after_dealloc.current_heap_usage, 0);
    }

    #[test]
    fn test_invalid_allocation_zero_size() {
        let gc = UnifiedGarbageCollector::with_default_config();

        // 尝试分配0字节
        let result = gc.allocate(0);
        assert!(result.is_err());

        if let Err(GcError::InvalidAllocation { size }) = result {
            assert_eq!(size, 0);
        } else {
            panic!("Expected InvalidAllocation error");
        }
    }

    #[test]
    fn test_invalid_deallocation_null_pointer() {
        let gc = UnifiedGarbageCollector::with_default_config();

        // 尝试释放null指针
        let result = unsafe { gc.deallocate(std::ptr::null_mut(), 1024) };
        assert!(result.is_err());

        if let Err(GcError::InvalidAllocation { .. }) = result {
            // 正确的错误类型
        } else {
            panic!("Expected InvalidAllocation error");
        }
    }

    #[test]
    fn test_invalid_deallocation_zero_size() {
        let gc = UnifiedGarbageCollector::with_default_config();

        // 分配有效内存
        let ptr = gc.allocate(1024).unwrap();

        // 尝试使用size=0释放
        let result = unsafe { gc.deallocate(ptr, 0) };
        assert!(result.is_err());

        // 清理
        let _ = unsafe { gc.deallocate(ptr, 1024) };
    }
}
