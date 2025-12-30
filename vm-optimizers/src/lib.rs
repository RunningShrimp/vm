//! VM Optimizers - 垃圾回收和优化器模块
//!
//! 提供分代GC、并发GC、写屏障、内存优化、机器学习优化等功能

pub mod gc;
pub mod gc_concurrent;
pub mod gc_write_barrier;
pub mod gc_generational;
pub mod memory;
pub mod adaptive;
pub mod ml;
pub mod pgo;

// Re-export OptimizedGC types
pub use gc::{
    AdaptiveQuota, AllocStats, GcError, GcPhase, GcResult, GcStats, LockFreeWriteBarrier,
    OptimizedGc, ParallelMarker, WriteBarrierType,
};

// 重新导出并发GC的公共类型
pub use gc_concurrent::{
    GCColor,
    GCStats,
    ConcurrentGC,
};

// 重新导出写屏障的公共类型
pub use gc_write_barrier::{
    BarrierType,
    SATBBarrier,
    CardMarkingBarrier,
    WriteBarrier,
    BarrierStats,
};

// 重新导出分代GC的公共类型
pub use gc_generational::{
    Generation,
    YoungGCStrategy,
    GenerationalGC,
    GenerationalGCStats,
    YoungGenerationConfig,
    OldGenerationConfig,
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
