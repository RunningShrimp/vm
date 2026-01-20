//! VM Optimizers - 垃圾回收和优化器模块
//!
//! 提供分代GC、并发GC、写屏障、内存优化、机器学习优化等功能

pub mod adaptive;
pub mod memory;
pub mod ml;
pub mod pgo;

// Re-export OptimizedGC types
// Re-export Adaptive Optimizer types
pub use adaptive::{
    AdaptiveOptimizer, AdaptiveOptimizerConfig, Cost, HardwareProfile, OptimizationRecommendation,
    OptimizationReport, PerformanceAnalysis, PerformanceMonitor, Priority, RecommendationType,
    WorkloadPattern,
};

// Re-export GC types from vm-gc crate
pub use vm_gc::{
    AdaptiveGCConfig,
    AdaptiveGCStats,
    AdaptiveGCTuner,
    AdaptiveQuota,

    AllocStats,
    BarrierStats,
    BaseGenerationalGC,
    BaseGenerationalGCStats,
    BaseIncrementalGc,

    Card,
    CardMarkingBarrier,

    // Generational GC (Enhanced)
    CardTable,
    // Concurrent GC
    ConcurrentGC,
    ConcurrentGCStats,

    EnhancedGenerationalGC,
    EnhancedGenerationalGCStats,
    EnhancedIncrementalGC,
    EnhancedObjectMetadata,

    GCColor,
    GCPerformanceMetrics,
    // Adaptive GC
    GCProblem,
    // Core GC types
    GcError,
    GcPhase,
    GcResult,
    GcStats,

    // Generational GC (Base)
    Generation,
    GenerationalGCConfig,
    GenerationalGcResult,

    IncrementalGCConfig,
    // Incremental GC (Enhanced)
    IncrementalGCPhase,
    IncrementalGCStats,
    // Incremental GC (Base)
    IncrementalPhase,
    IncrementalProgress,
    LockFreeWriteBarrier,
    MarkStack,

    OldGenerationConfig,
    // Optimized GC
    OptimizedGc,
    OptimizedGcStats,
    ParallelMarker,
    PerformanceHistory,
    PerformanceHistoryEntry,
    SATBBarrier,
    TuningAction,
    // Write Barriers
    WriteBarrier,
    WriteBarrierType,
    WriteBarrierTypeEnum,
    YoungGCStrategy,
    YoungGenerationConfig,
};

// Re-export Memory Optimizer types
pub use memory::{
    AccessPattern, ConcurrencyConfig, MemoryError, MemoryOptimizer, NumaConfig, TlbEntry, TlbStats,
};
// Re-export ML Compiler types
pub use ml::{
    ABTestFramework, ABTestMetrics, ABTestResults, BlockFeatures, CompilationDecision,
    MLCompilerStats, MLGuidedCompiler, SimpleLinearModel,
};
// Re-export PGO types
pub use pgo::{
    AotOptimizationDriver, AotOptimizationHint, BlockProfile, CallProfile, PgoIterationResult,
    PgoManager, PgoOptimizationStats, ProfileCollector, ProfileStats,
};
