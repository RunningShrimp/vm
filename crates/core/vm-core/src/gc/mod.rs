//! 垃圾回收 (Garbage Collection) 模块
//!
//! 提供多种 GC 策略实现，包括分代 GC、标记清除、复制算法等。
//!
//! ## 功能特性
//!
//! - **分代 GC**: 新生代和老年代分离，优化不同年龄对象的回收策略
//! - **并发标记**: 支持并发标记减少停顿时间
//! - **增量回收**: 将 GC 工作分散到多个时间片
//! - **写屏障**: 实现卡片标记和 Brooks 指针等写屏障技术
//! - **性能监控**: 提供 GC 统计信息和性能指标
//!
//! ## 使用示例
//!
//! ```rust,ignore
//! use vm_core::gc::{GCStrategy, GenerationalGC, GCConfig};
//!
//! let config = GCConfig::default();
//! let mut gc = GenerationalGC::new(config);
//!
//! // 分配对象
//! let obj = gc.allocate(1024, 8)?;
//!
//! // 手动触发 GC
//! gc.collect(false)?;
//! ```

pub mod card_table;
pub mod concurrent;
pub mod error;
pub mod generational;
pub mod metrics;
pub mod object;
pub mod parallel_sweep;
pub mod roots;
pub mod safepoint;
pub mod unified;

// Re-export commonly used type aliases
pub use GCType as GCTypeEnum;
pub use card_table::CardTable;
pub use concurrent::{ConcurrentGC, ConcurrentGCConfig};
pub use error::{GCError, GCResult};
pub use generational::{GCCollectionStats, GenerationalGC};
pub use metrics::{GCHeapStats, GCMetrics, GCStats};
pub use object::{GCObject, GCObjectPtr, GCObjectRef, ObjectHeader, ObjectType};
pub use parallel_sweep::{ParallelSweepConfig, ParallelSweeper, parallel_sweep_objects};
pub use roots::{RootScanner, RootSet};
pub use safepoint::{Safepoint, SafepointManager, SafepointState, SafepointStats};

/// GC 策略 trait
pub trait GCStrategy: Send + Sync {
    /// 分配内存
    fn allocate(&mut self, size: usize, align: usize) -> GCResult<GCObjectPtr>;

    /// 触发垃圾回收
    fn collect(&mut self, force_full: bool) -> GCResult<GCCollectionStats>;

    /// 写屏障 - 字段更新时调用
    fn write_barrier(&mut self, obj: GCObjectPtr, field_offset: usize, new_val: GCObjectPtr);

    /// 获取堆统计信息
    fn get_heap_stats(&self) -> GCHeapStats;

    /// 获取 GC 统计信息
    fn get_gc_stats(&self) -> GCStats;

    /// 重置统计信息
    fn reset_stats(&mut self);
}

/// GC 实现类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GCType {
    /// 无 GC (手动管理)
    None,
    /// Boehm GC (保守式 GC)
    Boehm,
    /// 分代 GC
    #[default]
    Generational,
    /// 并发标记清除
    ConcurrentMarkSweep,
    /// 增量式 GC
    Incremental,
}

/// GC 阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GCPhase {
    /// 空闲
    Idle,
    /// 标记阶段
    Marking,
    /// 重定位阶段
    Relocating,
    /// 清除阶段
    Sweeping,
    /// 压缩阶段
    Compacting,
}

/// GC 触发原因
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GCTrigger {
    /// 堆内存不足
    HeapExhausted,
    /// 分配速率过高
    AllocationRate,
    /// 手动触发
    Manual,
    /// 定时触发
    Timer,
    /// 压力触发
    Pressure,
}

/// GC 配置
#[derive(Debug, Clone)]
pub struct GCConfig {
    /// GC 类型
    pub gc_type: GCType,
    /// 新生代大小（字节）
    pub young_gen_size: usize,
    /// 老年代大小（字节）
    pub old_gen_size: usize,
    /// 是否启用并发标记
    pub enable_concurrent: bool,
    /// 是否启用增量回收
    pub enable_incremental: bool,
    /// 目标停顿时间（毫秒）
    pub target_pause_time_ms: u64,
    /// 触发 GC 的堆使用率阈值 (0.0-1.0)
    pub heap_threshold: f64,
    /// 写屏障类型
    pub write_barrier: WriteBarrierType,
}

/// 写屏障类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WriteBarrierType {
    /// 无写屏障
    None,
    /// 卡片标记
    #[default]
    CardMarking,
    /// Brooks 指针
    BrooksPointer,
    /// SATB (Snapshot-At-The-Beginning)
    SATB,
}

impl Default for GCConfig {
    fn default() -> Self {
        Self {
            gc_type: GCType::Generational,
            young_gen_size: 16 * 1024 * 1024, // 16 MB
            old_gen_size: 128 * 1024 * 1024,  // 128 MB
            enable_concurrent: true,
            enable_incremental: true,
            target_pause_time_ms: 10,
            heap_threshold: 0.8,
            write_barrier: WriteBarrierType::CardMarking,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gc_config_default() {
        let config = GCConfig::default();
        assert_eq!(config.gc_type, GCType::Generational);
        assert_eq!(config.young_gen_size, 16 * 1024 * 1024);
        assert_eq!(config.old_gen_size, 128 * 1024 * 1024);
    }

    #[test]
    fn test_gc_type_default() {
        let gc_type = GCType::default();
        assert_eq!(gc_type, GCType::Generational);
    }
}
