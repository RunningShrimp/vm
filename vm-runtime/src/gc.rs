//! Garbage Collector Runtime for VM
//!
//! Provides garbage collection functionality integrated with the VM runtime.
//! This module consolidates GC functionality from vm-boot and gc-optimizer.

use parking_lot::RwLock;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::Instant;

/// Re-export gc-optimizer types
pub use gc_optimizer::{
    AdaptiveQuota, AllocStats, GcError, GcPhase, GcResult, GcStats,
    LockFreeWriteBarrier, OptimizedGc, ParallelMarker, WriteBarrierType,
};

/// GC runtime manager
///
/// Integrates GC optimization with VM lifecycle management
pub struct GcRuntime {
    /// Optimized GC collector
    pub gc: Arc<OptimizedGc>,
    /// Runtime statistics
    pub stats: Arc<RwLock<GcRuntimeStats>>,
    /// GC enabled flag
    pub enabled: Arc<AtomicBool>,
}

/// Runtime-specific GC statistics
#[derive(Debug, Clone, Default)]
pub struct GcRuntimeStats {
    /// Total collections
    pub total_collections: u64,
    /// Last collection time
    pub last_collection_time: Option<Instant>,
    /// Cache entries
    pub total_entries: usize,
    pub hot_entries: usize,
    pub cold_entries: usize,
    /// Hit rate
    pub hit_rate: f64,
}

impl GcRuntime {
    pub fn new(num_workers: usize, target_pause_us: u64, barrier_type: WriteBarrierType) -> Self {
        Self {
            gc: Arc::new(OptimizedGc::new(num_workers, target_pause_us, barrier_type)),
            stats: Arc::new(RwLock::new(GcRuntimeStats::default())),
            enabled: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn check_and_run_gc_step(&self) -> bool {
        if !self.is_enabled() {
            return false;
        }

        let stats = self.gc.get_stats();
        let total_allocs = stats.alloc_stats.total_allocs;
        let bytes_used = stats.alloc_stats.bytes_used;
        
        let trigger_threshold = 10000u64;
        
        if total_allocs >= trigger_threshold {
            if let Ok(_) = self.gc.collect_minor(bytes_used) {
                let mut runtime_stats = self.stats.write();
                runtime_stats.total_collections += 1;
                runtime_stats.last_collection_time = Some(Instant::now());
                return true;
            }
        }
        false
    }

    pub fn update_cache_stats(&self, total_entries: usize, hot_entries: usize, cold_entries: usize, hit_rate: f64) {
        let mut stats = self.stats.write();
        stats.total_entries = total_entries;
        stats.hot_entries = hot_entries;
        stats.cold_entries = cold_entries;
        stats.hit_rate = hit_rate;
    }

    pub fn full_gc_on_stop(&self) {
        if !self.is_enabled() {
            return;
        }
        
        let stats = self.gc.get_stats();
        let _ = self.gc.collect_major(stats.alloc_stats.bytes_used);
    }

    pub fn get_runtime_stats(&self) -> GcRuntimeStats {
        self.stats.read().clone()
    }

    pub fn get_gc_stats(&self) -> GcStats {
        self.gc.get_stats()
    }

    pub fn record_write(&self, addr: u64) {
        self.gc.record_write(addr);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gc_runtime_creation() {
        let gc_runtime = GcRuntime::new(4, 10_000, WriteBarrierType::Atomic);
        let stats = gc_runtime.get_runtime_stats();
        assert_eq!(stats.total_entries, 0);
    }

    #[test]
    fn test_gc_runtime_enabled() {
        let gc_runtime = GcRuntime::new(4, 10_000, WriteBarrierType::Atomic);
        
        assert!(gc_runtime.is_enabled());
        
        gc_runtime.set_enabled(false);
        assert!(!gc_runtime.is_enabled());
        
        gc_runtime.set_enabled(true);
        assert!(gc_runtime.is_enabled());
    }

    #[test]
    fn test_gc_runtime_disabled_no_collection() {
        let gc_runtime = GcRuntime::new(4, 10_000, WriteBarrierType::Atomic);
        
        gc_runtime.set_enabled(false);
        
        let result = gc_runtime.check_and_run_gc_step();
        assert!(!result);
    }

    #[test]
    fn test_cache_stats_update() {
        let gc_runtime = GcRuntime::new(4, 10_000, WriteBarrierType::Atomic);
        
        gc_runtime.update_cache_stats(100, 80, 20, 0.95);
        
        let stats = gc_runtime.get_runtime_stats();
        assert_eq!(stats.total_entries, 100);
        assert_eq!(stats.hot_entries, 80);
        assert_eq!(stats.cold_entries, 20);
        assert_eq!(stats.hit_rate, 0.95);
    }
}
