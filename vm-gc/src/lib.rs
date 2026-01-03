//! # VM Garbage Collection Crate
//!
//! This crate provides garbage collection functionality for the VM project.
//! It serves as an independent crate to break the circular dependency between
//! vm-core and vm-optimizers.
//!
//! ## Architecture
//!
//! ```text
//!     vm-core
//!        ↓
//!     vm-gc (this crate)
//!        ↑
//! vm-optimizers
//! ```
//!
//! ## Features
//!
//! - **generational**: Generational garbage collection
//! - **incremental**: Incremental garbage collection
//! - **adaptive**: Adaptive GC that switches strategies
//! - **stats**: GC statistics and profiling
//! - **benchmarking**: Benchmarking support
//!
//! ## Usage
//!
//! ```ignore
//! use vm_gc::{GcManager, GcConfig};
//!
//! let config = GcConfig::default();
//! let mut gc = GcManager::new(config);
//!
//! // Allocate objects
//! gc.allocate(...);
//!
//! // Trigger collection
//! gc.collect();
//! ```

#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

pub mod adaptive;
pub mod common;
pub mod concurrent;
pub mod error;
pub mod gc;
pub mod generational;
pub mod incremental;
pub mod stats;
pub mod traits;
pub mod write_barrier;

// Re-export common types
pub use error::{GcError, GcResult};
pub use stats::GcStats;
pub use traits::{GcPolicy, GcStrategy};

// Re-export common/shared types
pub use common::{ObjectMetadata as CommonObjectMetadata, ObjectPtr};

// Re-export concurrent GC types
pub use concurrent::{ConcurrentGC, ConcurrentGCStats, GCColor};

// Re-export write barrier types
pub use write_barrier::{
    BarrierStats, BarrierType as WriteBarrierTypeEnum, CardMarkingBarrier, SATBBarrier,
    WriteBarrier,
};

// Re-export main GC types
pub use gc::{
    AdaptiveQuota, AllocStats, GcPhase, LockFreeWriteBarrier, OptimizedGc, OptimizedGcStats,
    ParallelMarker, WriteBarrierType,
};

// Re-export generational GC types
pub use generational::{
    BaseGenerationalGC,
    BaseGenerationalGCStats,
    Card,
    // Enhanced generational GC
    CardTable,
    Generation,
    GenerationalGC as EnhancedGenerationalGC,
    GenerationalGCConfig,
    GenerationalGCStats as EnhancedGenerationalGCStats,
    GenerationalGcResult,
    ObjectMetadata as EnhancedObjectMetadata,
    OldGenerationConfig,
    YoungGCStrategy,
    YoungGenerationConfig,
};

// Re-export incremental GC types
pub use incremental::{
    // Base incremental GC
    BaseIncrementalGc,
    // Enhanced incremental GC
    GCPhase as IncrementalGCPhase,
    IncrementalGC,
    IncrementalGC as EnhancedIncrementalGC,
    IncrementalGCConfig,
    IncrementalGCStats,
    IncrementalPhase,
    IncrementalProgress,
    MarkStack,
};

// Re-export adaptive GC types
pub use adaptive::{
    AdaptiveGCConfig, AdaptiveGCStats, AdaptiveGCTuner, GCPerformanceMetrics, GCProblem,
    PerformanceHistory, PerformanceHistoryEntry, TuningAction,
};

/// GC configuration
#[derive(Debug, Clone)]
pub struct GcConfig {
    /// Enable generational collection
    pub generational: bool,

    /// Enable incremental collection
    pub incremental: bool,

    /// Heap size threshold (in bytes)
    pub heap_threshold: usize,

    /// GC trigger threshold (0.0 - 1.0)
    pub gc_threshold: f64,

    /// Enable statistics collection
    pub enable_stats: bool,
}

impl Default for GcConfig {
    fn default() -> Self {
        Self {
            generational: true,
            incremental: false,
            heap_threshold: 16 * 1024 * 1024, // 16MB
            gc_threshold: 0.8,
            enable_stats: false,
        }
    }
}

/// Main GC manager
///
/// This is the primary interface for garbage collection.
/// Different GC strategies can be plugged in via the `GcStrategy` trait.
pub struct GcManager<S: GcStrategy> {
    /// GC configuration
    config: GcConfig,

    /// GC strategy implementation
    strategy: S,

    /// GC statistics
    stats: GcStats,
}

impl<S: GcStrategy> GcManager<S> {
    /// Create a new GC manager with the given strategy
    pub fn new(config: GcConfig, strategy: S) -> Self {
        Self {
            config,
            strategy,
            stats: GcStats::default(),
        }
    }

    /// Get GC configuration
    pub fn config(&self) -> &GcConfig {
        &self.config
    }

    /// Get GC statistics
    pub fn stats(&self) -> &GcStats {
        &self.stats
    }

    /// Trigger a garbage collection cycle
    ///
    /// # Errors
    ///
    /// Returns an error if the collection fails
    pub fn collect(&mut self) -> GcResult<()> {
        let start = std::time::Instant::now();

        // Delegate to the strategy
        self.strategy.collect()?;

        // Update statistics
        if self.config.enable_stats {
            self.stats.collections += 1;
            self.stats.total_collection_time += start.elapsed();
        }

        Ok(())
    }

    /// Allocate memory for a new object
    ///
    /// This is a placeholder - actual implementation will depend on
    /// the memory management system being used.
    pub fn allocate(&mut self, size: usize) -> GcResult<*mut u8> {
        self.strategy.allocate(size)
    }

    /// Check if a GC cycle should be triggered
    pub fn should_collect(&self) -> bool {
        self.strategy.should_collect(&self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = GcConfig::default();
        assert!(config.generational);
        assert!(!config.incremental);
        assert_eq!(config.heap_threshold, 16 * 1024 * 1024);
    }

    #[test]
    fn test_stats_default() {
        let stats = GcStats::default();
        assert_eq!(stats.collections, 0);
    }
}
