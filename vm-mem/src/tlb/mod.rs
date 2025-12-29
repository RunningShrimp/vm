//! TLB (Translation Lookaside Buffer) 模块
//!
//! 提供多种TLB实现，适用于不同场景：
//! - 基础TLB：简单实现，适用于基本场景
//! - 优化TLB：多级TLB，适用于高性能场景
//! - 并发TLB：无锁设计，适用于高并发场景
//! - 统一TLB：统一接口，支持动态选择最佳实现

pub mod per_cpu_tlb;
pub mod tlb_basic;
pub mod tlb_concurrent;
pub mod tlb_flush;
pub mod tlb_manager;
pub mod tlb_sync;
pub mod unified_tlb;

// 重新导出主要类型
pub use per_cpu_tlb::*;
pub use tlb_basic::*;
pub use tlb_concurrent::*;
pub use tlb_flush::*;
pub use tlb_flush::{
    AccessPredictor, AdaptiveFlushConfig, AdvancedTlbFlushConfig, AdvancedTlbFlushManager,
    PageImportanceEvaluator, PerformanceMonitor, PerformanceTrend, PredictiveFlushConfig,
    PredictiveFlushStatsSnapshot, SelectiveFlushConfig,
};
pub use tlb_manager::*;
pub use tlb_sync::*;
// 使用显式导入避免 TlbStats 冲突
pub use unified_tlb::{
    AdaptiveReplacementPolicy, AtomicTlbStats, MultiLevelTlb, MultiLevelTlbConfig,
    OptimizedTlbEntry, TlbFactory, TlbResult, UnifiedTlb,
};

#[cfg(feature = "optimizations")]
pub use unified_tlb::multilevel_tlb_impl::SingleLevelTlb;
