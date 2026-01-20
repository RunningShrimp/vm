//! GC 性能指标和统计信息

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::{GCPhase, GCType};

/// GC 统计信息
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GCStats {
    /// GC 总次数
    pub total_collections: u64,
    /// 完整 GC 次数
    pub full_collections: u64,
    /// 增量 GC 次数
    pub incremental_collections: u64,
    /// 总回收时间（毫秒）
    pub total_collection_time_ms: u64,
    /// 平均停顿时间（毫秒）
    pub avg_pause_time_ms: f64,
    /// 最大停顿时间（毫秒）
    pub max_pause_time_ms: u64,
    /// 回收的对象数量
    pub objects_reclaimed: u64,
    /// 回收的内存（字节）
    pub bytes_reclaimed: u64,
    /// 当前堆使用量（字节）
    pub current_heap_size: u64,
    /// 峰值堆使用量（字节）
    pub peak_heap_size: u64,
}

impl GCStats {
    /// 计算回收效率（字节数/毫秒）
    pub fn throughput(&self) -> f64 {
        if self.total_collection_time_ms > 0 {
            self.bytes_reclaimed as f64 / self.total_collection_time_ms as f64
        } else {
            0.0
        }
    }

    /// 计算平均每次 GC 回收的对象数
    pub fn avg_objects_per_gc(&self) -> f64 {
        if self.total_collections > 0 {
            self.objects_reclaimed as f64 / self.total_collections as f64
        } else {
            0.0
        }
    }

    /// 计算堆使用率
    pub fn heap_utilization(&self, max_heap_size: u64) -> f64 {
        if max_heap_size > 0 {
            self.current_heap_size as f64 / max_heap_size as f64
        } else {
            0.0
        }
    }
}

/// GC 堆统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GCHeapStats {
    /// 新生代大小（字节）
    pub young_gen_size: u64,
    /// 新生代已用（字节）
    pub young_gen_used: u64,
    /// 老年代大小（字节）
    pub old_gen_size: u64,
    /// 老年代已用（字节）
    pub old_gen_used: u64,
    /// 总对象数量
    pub total_objects: u64,
    /// 存活对象数量
    pub live_objects: u64,
}

impl GCHeapStats {
    /// 创建新的堆统计信息
    pub fn new(young_gen_size: u64, old_gen_size: u64) -> Self {
        Self {
            young_gen_size,
            young_gen_used: 0,
            old_gen_size,
            old_gen_used: 0,
            total_objects: 0,
            live_objects: 0,
        }
    }

    /// 获取新生代使用率
    pub fn young_gen_utilization(&self) -> f64 {
        if self.young_gen_size > 0 {
            self.young_gen_used as f64 / self.young_gen_size as f64
        } else {
            0.0
        }
    }

    /// 获取老年代使用率
    pub fn old_gen_utilization(&self) -> f64 {
        if self.old_gen_size > 0 {
            self.old_gen_used as f64 / self.old_gen_size as f64
        } else {
            0.0
        }
    }

    /// 获取总使用率
    pub fn overall_utilization(&self) -> f64 {
        let total_size = self.young_gen_size + self.old_gen_size;
        let total_used = self.young_gen_used + self.old_gen_used;
        if total_size > 0 {
            total_used as f64 / total_size as f64
        } else {
            0.0
        }
    }

    /// 获取存活率
    pub fn survival_rate(&self) -> f64 {
        if self.total_objects > 0 {
            self.live_objects as f64 / self.total_objects as f64
        } else {
            0.0
        }
    }
}

impl Default for GCHeapStats {
    fn default() -> Self {
        Self::new(16 * 1024 * 1024, 128 * 1024 * 1024)
    }
}

/// GC 指标收集器
pub struct GCMetrics {
    /// GC 类型
    gc_type: GCType,
    /// 统计信息
    stats: GCStats,
    /// 当前阶段
    current_phase: GCPhase,
    /// 阶段开始时间
    phase_start_time: Option<std::time::Instant>,
}

impl GCMetrics {
    /// 创建新的指标收集器
    pub fn new(gc_type: GCType) -> Self {
        Self {
            gc_type,
            stats: GCStats::default(),
            current_phase: GCPhase::Idle,
            phase_start_time: None,
        }
    }

    /// 开始 GC 阶段
    pub fn begin_phase(&mut self, phase: GCPhase) {
        self.current_phase = phase;
        self.phase_start_time = Some(std::time::Instant::now());
    }

    /// 结束 GC 阶段
    pub fn end_phase(&mut self) -> Duration {
        if let Some(start) = self.phase_start_time {
            let duration = start.elapsed();
            self.phase_start_time = None;
            self.current_phase = GCPhase::Idle;
            duration
        } else {
            Duration::ZERO
        }
    }

    /// 记录 GC 完成信息
    pub fn record_collection(
        &mut self,
        is_full: bool,
        pause_time: Duration,
        objects_reclaimed: u64,
        bytes_reclaimed: u64,
    ) {
        self.stats.total_collections += 1;
        if is_full {
            self.stats.full_collections += 1;
        } else {
            self.stats.incremental_collections += 1;
        }

        let pause_ms = pause_time.as_millis() as u64;
        self.stats.total_collection_time_ms += pause_ms;
        self.stats.max_pause_time_ms = self.stats.max_pause_time_ms.max(pause_ms);
        self.stats.avg_pause_time_ms =
            self.stats.total_collection_time_ms as f64 / self.stats.total_collections as f64;

        self.stats.objects_reclaimed += objects_reclaimed;
        self.stats.bytes_reclaimed += bytes_reclaimed;
    }

    /// 更新堆大小
    pub fn update_heap_size(&mut self, current_size: u64) {
        self.stats.current_heap_size = current_size;
        self.stats.peak_heap_size = self.stats.peak_heap_size.max(current_size);
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> &GCStats {
        &self.stats
    }

    /// 重置统计信息
    pub fn reset(&mut self) {
        self.stats = GCStats::default();
        self.current_phase = GCPhase::Idle;
        self.phase_start_time = None;
    }

    /// 获取当前阶段
    pub fn current_phase(&self) -> GCPhase {
        self.current_phase
    }

    /// 获取 GC 类型
    pub fn gc_type(&self) -> GCType {
        self.gc_type
    }
}

/// 实时 GC 计数器
pub struct AtomicGCMetrics {
    /// 总分配次数
    pub total_allocations: AtomicU64,
    /// 总分配字节数
    pub total_allocated_bytes: AtomicU64,
    /// GC 触发次数
    pub gc_count: AtomicU64,
    /// 当前停顿时间（纳秒）
    pub current_pause_ns: AtomicU64,
}

impl AtomicGCMetrics {
    /// 创建新的原子指标
    pub fn new() -> Self {
        Self {
            total_allocations: AtomicU64::new(0),
            total_allocated_bytes: AtomicU64::new(0),
            gc_count: AtomicU64::new(0),
            current_pause_ns: AtomicU64::new(0),
        }
    }

    /// 记录分配
    pub fn record_allocation(&self, size: usize) {
        self.total_allocations.fetch_add(1, Ordering::Relaxed);
        self.total_allocated_bytes
            .fetch_add(size as u64, Ordering::Relaxed);
    }

    /// 记录 GC 开始
    pub fn begin_gc(&self) {
        self.gc_count.fetch_add(1, Ordering::Relaxed);
    }

    /// 获取分配速率（字节/秒）
    pub fn allocation_rate(&self, elapsed_sec: f64) -> f64 {
        if elapsed_sec > 0.0 {
            self.total_allocated_bytes.load(Ordering::Relaxed) as f64 / elapsed_sec
        } else {
            0.0
        }
    }
}

impl Default for AtomicGCMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gc_stats_throughput() {
        let stats = GCStats {
            total_collection_time_ms: 100,
            bytes_reclaimed: 1000000,
            ..Default::default()
        };

        assert!((stats.throughput() - 10000.0).abs() < 0.01);
    }

    #[test]
    fn test_heap_stats_utilization() {
        let stats = GCHeapStats::new(1024, 2048);
        assert_eq!(stats.young_gen_size, 1024);
        assert_eq!(stats.old_gen_size, 2048);
        assert_eq!(stats.young_gen_utilization(), 0.0);
    }

    #[test]
    fn test_metrics_phases() {
        let mut metrics = GCMetrics::new(GCType::Generational);
        assert_eq!(metrics.current_phase(), GCPhase::Idle);

        metrics.begin_phase(GCPhase::Marking);
        assert_eq!(metrics.current_phase(), GCPhase::Marking);

        std::thread::sleep(std::time::Duration::from_millis(10));
        let duration = metrics.end_phase();
        assert!(duration.as_millis() >= 10);
        assert_eq!(metrics.current_phase(), GCPhase::Idle);
    }

    #[test]
    fn test_atomic_metrics() {
        let metrics = AtomicGCMetrics::new();
        metrics.record_allocation(1024);
        metrics.record_allocation(2048);

        assert_eq!(metrics.total_allocations.load(Ordering::Relaxed), 2);
        assert_eq!(metrics.total_allocated_bytes.load(Ordering::Relaxed), 3072);
    }
}
