//! VM Optimizers - 垃圾回收和优化器模块
//!
//! 提供分代GC、并发GC、写屏障、内存优化、机器学习优化等功能

pub mod adaptive;
pub mod gc;
pub mod gc_concurrent;
pub mod gc_generational;
pub mod gc_incremental_enhanced;
pub mod gc_generational_enhanced;
pub mod gc_adaptive;
pub mod gc_write_barrier;
pub mod memory;
pub mod ml;
pub mod pgo;

// Re-export OptimizedGC types
pub use gc::{
    AdaptiveQuota, AllocStats, GcError, GcPhase, GcResult, GcStats, LockFreeWriteBarrier,
    OptimizedGc, ParallelMarker, WriteBarrierType,
};

// 重新导出并发GC的公共类型
pub use gc_concurrent::{ConcurrentGC, GCColor, GCStats};

// 重新导出写屏障的公共类型
pub use gc_write_barrier::{
    BarrierStats, BarrierType, CardMarkingBarrier, SATBBarrier, WriteBarrier,
};

// 重新导出分代GC的公共类型
pub use gc_generational::{
    Generation, GenerationalGC, GenerationalGCStats, OldGenerationConfig, YoungGCStrategy,
    YoungGenerationConfig,
};

// Re-export Enhanced Incremental GC types
pub use gc_incremental_enhanced::{
    IncrementalGC, IncrementalGCConfig, IncrementalGCStats, MarkStack,
};

// Re-export Enhanced Generational GC types
pub use gc_generational_enhanced::{
    CardTable as EnhancedCardTable, GenerationalGC as GenerationalGCEnhanced,
    GenerationalGCConfig as EnhancedGenerationalGCConfig,
    GenerationalGCStats as EnhancedGenerationalGCStats, ObjectMetadata,
};

// Re-export Adaptive GC Tuner types
pub use gc_adaptive::{
    AdaptiveGCTuner, AdaptiveGCConfig, AdaptiveGCStats, GCPerformanceMetrics, GCProblem,
    PerformanceHistory, TuningAction,
};

// Re-export Memory Optimizer types
pub use memory::{
    AccessPattern, ConcurrencyConfig, MemoryError, MemoryOptimizer, NumaConfig, TlbEntry, TlbStats,
};

// Re-export Adaptive Optimizer types
pub use adaptive::{
    AdaptiveOptimizer, AdaptiveOptimizerConfig, Cost, HardwareProfile, OptimizationRecommendation,
    OptimizationReport, PerformanceAnalysis, PerformanceMonitor, Priority, RecommendationType,
    WorkloadPattern,
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
