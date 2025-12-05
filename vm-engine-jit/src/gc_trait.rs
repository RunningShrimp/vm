//! GC统一接口trait
//!
//! 定义统一的GC接口，支持不同的GC实现

use std::time::Instant;

/// GC统计信息trait
pub trait GcStats {
    /// 获取堆使用量（字节）
    fn heap_used(&self) -> u64;
    
    /// 获取堆使用率（0.0-1.0）
    fn heap_usage_ratio(&self) -> f64;
    
    /// 获取GC阶段
    fn phase(&self) -> GcPhase;
}

/// GC阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcPhase {
    /// 空闲状态
    Idle,
    /// 准备标记
    MarkPrepare,
    /// 标记中
    Marking,
    /// 标记完成
    MarkTerminate,
    /// 清扫中
    Sweeping,
    /// 完成
    Complete,
}

/// GC统一接口trait
///
/// 所有GC实现都应该实现这个trait，以提供统一的接口
pub trait GcTrait: Send + Sync {
    /// 检查是否应该触发GC
    fn should_trigger_gc(&self) -> bool;
    
    /// 执行增量GC步骤
    ///
    /// 返回 (是否完成, 处理的元素数量)
    fn run_gc_step(&self) -> (bool, usize);
    
    /// 启动完整GC周期
    ///
    /// `roots`: GC根对象地址列表
    /// 返回GC周期开始时间
    fn start_gc(&self, roots: &[u64]) -> Instant;
    
    /// 完成GC周期
    ///
    /// `cycle_start_time`: GC周期开始时间
    fn finish_gc(&self, cycle_start_time: Instant);
    
    /// 写屏障
    ///
    /// 当对象引用被修改时调用
    /// `obj_addr`: 对象地址
    /// `child_addr`: 子对象地址
    fn write_barrier(&self, obj_addr: u64, child_addr: u64);
    
    /// 获取GC统计信息
    fn get_stats(&self) -> Box<dyn GcStats>;
    
    /// 更新堆使用量
    fn update_heap_usage(&self, used_bytes: u64);
    
    /// 在VM停止时执行完整GC清理
    fn full_gc_on_stop(&self);
}

use std::sync::Arc;

/// GC适配器：将UnifiedGC适配到GcTrait
pub struct UnifiedGcAdapter {
    gc: Arc<super::unified_gc::UnifiedGC>,
}

impl UnifiedGcAdapter {
    pub fn new(gc: Arc<super::unified_gc::UnifiedGC>) -> Self {
        Self { gc }
    }
}

impl GcTrait for UnifiedGcAdapter {
    fn should_trigger_gc(&self) -> bool {
        self.gc.should_trigger_gc()
    }
    
    fn run_gc_step(&self) -> (bool, usize) {
        let phase = self.gc.phase();
        match phase {
            super::unified_gc::GCPhase::Idle => (true, 0),
            super::unified_gc::GCPhase::Marking => {
                self.gc.incremental_mark()
            }
            super::unified_gc::GCPhase::Sweeping => {
                self.gc.incremental_sweep()
            }
            _ => (true, 0),
        }
    }
    
    fn start_gc(&self, roots: &[u64]) -> Instant {
        self.gc.start_gc(roots)
    }
    
    fn finish_gc(&self, cycle_start_time: Instant) {
        self.gc.finish_gc(cycle_start_time)
    }
    
    fn write_barrier(&self, obj_addr: u64, child_addr: u64) {
        self.gc.write_barrier(obj_addr, child_addr)
    }
    
    fn get_stats(&self) -> Box<dyn GcStats> {
        Box::new(UnifiedGcStatsAdapter {
            stats: self.gc.stats(),
            heap_used: self.gc.get_heap_used(),
            heap_usage_ratio: self.gc.get_heap_usage_ratio(),
            phase: self.gc.phase(),
        })
    }
    
    fn update_heap_usage(&self, used_bytes: u64) {
        self.gc.update_heap_usage(used_bytes)
    }
    
    fn full_gc_on_stop(&self) {
        // UnifiedGC在finish_gc中已经完成清理
        // 这里可以添加额外的清理逻辑
    }
}

/// UnifiedGC统计信息适配器
struct UnifiedGcStatsAdapter {
    stats: Arc<super::unified_gc::UnifiedGcStats>,
    heap_used: u64,
    heap_usage_ratio: f64,
    phase: super::unified_gc::GCPhase,
}

impl GcStats for UnifiedGcStatsAdapter {
    fn heap_used(&self) -> u64 {
        self.heap_used
    }
    
    fn heap_usage_ratio(&self) -> f64 {
        self.heap_usage_ratio
    }
    
    fn phase(&self) -> GcPhase {
        match self.phase {
            super::unified_gc::GCPhase::Idle => GcPhase::Idle,
            super::unified_gc::GCPhase::MarkPrepare => GcPhase::MarkPrepare,
            super::unified_gc::GCPhase::Marking => GcPhase::Marking,
            super::unified_gc::GCPhase::MarkTerminate => GcPhase::MarkTerminate,
            super::unified_gc::GCPhase::Sweeping => GcPhase::Sweeping,
            super::unified_gc::GCPhase::Complete => GcPhase::Complete,
        }
    }
}

