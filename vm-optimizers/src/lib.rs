//! Unified optimization framework for VM
//!
//! This crate provides a comprehensive optimization framework including:
//! - Garbage collection optimizers (generational and incremental)
//! - Memory allocation optimization
//! - Profile-guided optimization (PGO)
//! - ML-guided compilation optimization
//! - Adaptive compilation optimization
//!
//! # Modules
//!
//! - [`gc`]: Garbage collection optimization with lock-free write barriers,
//!   parallel marking, and adaptive quota management
//! - [`memory`]: Memory access optimization with TLB prefetching,
//!   parallel page tables, and NUMA-aware allocation
//! - [`pgo`]: Profile-guided optimization with runtime profiling,
//!   hot path detection, and AOT optimization hints
//! - [`ml`]: ML-guided compilation with tier prediction,
//!   feature extraction, and A/B testing framework
//! - [`adaptive`]: Adaptive compilation with performance monitoring,
//!   strategy adjustment, and runtime decision making

pub mod adaptive;
pub mod gc;
pub mod memory;
pub mod ml;
pub mod pgo;

// Re-exports for GC module
pub use gc::{
    AdaptiveQuota, GcError, GcPhase, GcResult, GcStats, LockFreeWriteBarrier, OptimizedGc,
    ParallelMarker, WriteBarrierType,
};

// Re-exports for Memory module
pub use memory::{
    AccessPattern, AsyncPrefetchingTlb, MemoryError, MemoryOptimizer, MemoryResult, NumaAllocator,
    NumaConfig, PageTableEntry, ParallelPageTable, TlbEntry, TlbStats,
};

// Re-exports for PGO module
pub use pgo::{
    AotOptimizationDriver, AotOptimizationHint, BlockProfile, CallProfile, PgoIterationResult,
    PgoManager, PgoOptimizationStats, ProfileCollector, ProfileStats,
};

// Re-exports for ML module
pub use ml::{
    ABTestFramework, ABTestMetrics, ABTestResults, BlockFeatures, CompilationDecision,
    MLCompilerStats, MLGuidedCompiler, SimpleLinearModel,
};
