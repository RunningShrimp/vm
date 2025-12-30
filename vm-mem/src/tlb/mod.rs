//! TLB (Translation Lookaside Buffer) 模块
//!
//! 本模块提供TLB的各种实现和优化策略，适用于不同场景：
//!
//! ## 核心实现 (core)
//! - **基础TLB**：简单实现，适用于基本场景
//! - **并发TLB**：无锁设计，适用于高并发场景
//! - **Per-CPU TLB**：每CPU独立TLB，避免锁竞争
//! - **统一TLB**：统一接口，支持动态选择最佳实现
//!
//! ## 优化策略 (optimization)
//! - **访问模式追踪**：追踪和分析地址访问模式
//! - **马尔可夫预测器**：基于马尔可夫链的访问预测
//! - **预热机制**：主动预热常用地址
//!
//! ## 管理功能 (management)
//! - **TLB管理器**：集中管理多个TLB实例
//! - **TLB刷新**：高效刷新TLB条目
//! - **TLB同步**：多核间TLB同步机制
//!
//! ## 测试和示例 (testing)
//! - **使用示例**：展示TLB的各种用法
//! - **基准测试**：性能基准测试

// 核心实现
pub mod core;
pub use core::{basic, concurrent, per_cpu, unified};

// 优化策略
pub mod optimization;
pub use optimization::{access_pattern, adaptive, predictor, prefetch};

// 管理功能
pub mod management;
pub use management::{flush, manager, sync};

// 测试和示例
pub mod testing;
pub use testing::examples;

// 重新导出主要类型（向后兼容）
pub use core::basic::{
    SoftwareTlb, TlbConfig as BasicTlbConfig, TlbEntry, TlbReplacePolicy, TlbStats as BasicTlbStats,
};

pub use core::concurrent::{
    ConcurrentTlbConfig, ConcurrentTlbManager, ConcurrentTlbManagerAdapter, ShardedTlb,
};

pub use core::per_cpu::PerCpuTlbManager;

pub use core::unified::{
    AdaptiveReplacementPolicy, AtomicTlbStats, MultiLevelTlb, MultiLevelTlbConfig,
    OptimizedTlbEntry, TlbFactory, TlbResult, UnifiedTlb,
};

// 管理模块类型
pub use management::flush::{
    AccessPredictor, AdaptiveFlushConfig, AdvancedTlbFlushConfig, AdvancedTlbFlushManager,
    PageImportanceEvaluator, PerformanceMonitor, PerformanceTrend, PredictiveFlushConfig,
    PredictiveFlushStatsSnapshot, SelectiveFlushConfig,
};

pub use management::manager::StandardTlbManager;

pub use management::sync::{SyncStrategy as TlbSyncStrategy, TlbSynchronizer};

// 优化模块类型
pub use optimization::predictor::MarkovPredictor;

pub use optimization::prefetch::{TlbPrefetchExample, TlbPrefetchGuide};

#[cfg(feature = "optimizations")]
pub use core::unified::multilevel_tlb_impl::SingleLevelTlb;
