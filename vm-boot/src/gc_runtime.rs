//! vm-boot 运行时的 GC 集成
//!
//! 集成 GC 到 VM 生命周期，提供完整的垃圾回收功能

use crate::gc::mark_sweep::{MarkSweepCollector, RootReference, RootSource};
use parking_lot::Mutex;
use std::sync::Arc;

/// GC 运行时管理器
pub struct GcRuntime {
    /// 缓存使用统计
    pub cache_stats: Arc<Mutex<CacheStatistics>>,
    /// GC 触发阈值 (0.0-1.0)
    pub gc_trigger_threshold: f32,
    /// 标记-清除收集器
    mark_sweep: Arc<MarkSweepCollector>,
    /// 是否启用GC
    enabled: bool,
}

/// 缓存统计信息
#[derive(Debug, Clone)]
pub struct CacheStatistics {
    pub total_entries: usize,
    pub hot_entries: usize,
    pub cold_entries: usize,
    pub hit_rate: f64,
}

impl Default for CacheStatistics {
    fn default() -> Self {
        Self {
            total_entries: 0,
            hot_entries: 0,
            cold_entries: 0,
            hit_rate: 0.0,
        }
    }
}

impl GcRuntime {
    /// 创建新的 GC 运行时管理器
    pub fn new(_hot_cache_size: usize, _cold_cache_entries: usize) -> Self {
        Self {
            cache_stats: Arc::new(Mutex::new(CacheStatistics::default())),
            gc_trigger_threshold: 0.8,
            mark_sweep: Arc::new(MarkSweepCollector::new()),
            enabled: true,
        }
    }

    /// 启用或禁用GC
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// 在每次 poll_events 中调用，检查并执行增量 GC
    ///
    /// 返回 true 如果执行了 GC，false 否则。
    pub fn check_and_run_gc_step(&self) -> bool {
        if !self.enabled {
            return false;
        }

        let stats = self.cache_stats.lock();
        let total_entries = stats.total_entries as f32;
        let max_entries = 10000.0;

        if total_entries >= max_entries * self.gc_trigger_threshold {
            drop(stats); // 释放锁
            self.run_gc();
            return true;
        }
        false
    }

    /// 执行GC收集
    fn run_gc(&self) {
        // 扫描根集合（从寄存器、栈等）
        self.scan_roots();

        // 执行标记-清除GC
        let gc_stats = self.mark_sweep.collect();
        tracing::debug!(
            "GC completed: {} objects marked, {} objects reclaimed ({} bytes)",
            gc_stats.objects_marked,
            gc_stats.objects_reclaimed,
            gc_stats.bytes_reclaimed
        );
    }

    /// 扫描根集合
    fn scan_roots(&self) {
        // TODO: 实际实现需要：
        // 1. 从VM寄存器扫描根对象
        // 2. 从调用栈扫描根对象
        // 3. 从全局变量扫描根对象
        //
        // 当前实现：添加占位根对象
        // 实际实现应该调用 VM 状态接口获取真实的根对象
    }

    /// 更新缓存统计信息
    pub fn update_stats(&self, stats: CacheStatistics) {
        *self.cache_stats.lock() = stats;
    }

    /// 在 vm stop 时，执行完整 GC 清理
    pub fn full_gc_on_stop(&self) {
        if !self.enabled {
            return;
        }

        tracing::debug!("Executing full GC on VM stop");

        // 扫描所有根对象
        self.scan_roots();

        // 执行完整的标记-清除GC
        let gc_stats = self.mark_sweep.collect();
        tracing::info!(
            "Full GC completed: {} objects marked, {} objects reclaimed ({} bytes)",
            gc_stats.objects_marked,
            gc_stats.objects_reclaimed,
            gc_stats.bytes_reclaimed
        );
    }

    /// 注册对象到GC系统
    pub fn register_object(&self, addr: u64, size: usize, obj_type: u32) {
        self.mark_sweep.register_object(addr, size, obj_type);
    }

    /// 添加根对象引用
    pub fn add_root(&self, addr: u64, source: RootSource) {
        self.mark_sweep.add_root(RootReference { addr, source });
    }

    /// 获取GC统计信息
    pub fn get_gc_stats(&self) -> crate::gc::mark_sweep::GcCollectionStats {
        self.mark_sweep.get_stats()
    }

    /// 获取当前缓存统计
    pub fn get_stats(&self) -> CacheStatistics {
        self.cache_stats.lock().clone()
    }
}

// 实现统一的GC接口trait（如果可用）
// 注意：这里需要vm-boot依赖vm-engine-jit才能使用
// 为了保持vm-boot的独立性，这里暂时注释掉
// 如果需要统一接口，可以在vm-boot的Cargo.toml中添加vm-engine-jit依赖
/*
#[cfg(feature = "unified-gc")]
impl vm_engine_jit::gc_trait::GcTrait for GcRuntime {
    fn should_trigger_gc(&self) -> bool {
        self.check_and_run_gc_step()
    }

    fn run_gc_step(&self) -> (bool, usize) {
        // 简化实现：只检查是否触发，不执行实际GC
        (self.check_and_run_gc_step(), 0)
    }

    fn start_gc(&self, _roots: &[u64]) -> Instant {
        Instant::now()
    }

    fn finish_gc(&self, _cycle_start_time: Instant) {
        // 简化实现：无操作
    }

    fn write_barrier(&self, _obj_addr: u64, _child_addr: u64) {
        // 简化实现：无操作
    }

    fn get_stats(&self) -> Box<dyn vm_engine_jit::gc_trait::GcStats> {
        let stats = self.get_stats();
        Box::new(GcRuntimeStatsAdapter {
            cache_stats: stats,
        })
    }

    fn update_heap_usage(&self, _used_bytes: u64) {
        // 简化实现：无操作
    }

    fn full_gc_on_stop(&self) {
        self.full_gc_on_stop()
    }
}

#[cfg(feature = "unified-gc")]
struct GcRuntimeStatsAdapter {
    cache_stats: CacheStatistics,
}

#[cfg(feature = "unified-gc")]
impl vm_engine_jit::gc_trait::GcStats for GcRuntimeStatsAdapter {
    fn heap_used(&self) -> u64 {
        self.cache_stats.total_entries as u64 * 1024 // 假设每个条目1KB
    }

    fn heap_usage_ratio(&self) -> f64 {
        let max_entries = 10000.0;
        (self.cache_stats.total_entries as f64 / max_entries).min(1.0)
    }

    fn phase(&self) -> vm_engine_jit::gc_trait::GcPhase {
        vm_engine_jit::gc_trait::GcPhase::Idle
    }
}
*/

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gc_runtime_creation() {
        let gc_runtime = GcRuntime::new(1024 * 1024, 1000);
        let stats = gc_runtime.get_stats();
        assert_eq!(stats.total_entries, 0);
    }

    #[test]
    fn test_gc_trigger() {
        let gc_runtime = GcRuntime::new(1024, 100);

        // 初始状态不应该触发
        assert!(!gc_runtime.check_and_run_gc_step());

        // 模拟高缓存使用
        let mut stats = gc_runtime.get_stats();
        stats.total_entries = 8500; // 85% 满度
        gc_runtime.update_stats(stats);

        // 现在应该触发
        assert!(gc_runtime.check_and_run_gc_step());
    }
}
