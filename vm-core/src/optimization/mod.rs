//! 自动优化系统 (Round 36)
//!
//! 智能优化控制器,提供工作负载识别、平台检测和优化策略自动生成

pub mod auto_optimizer;

pub use auto_optimizer::{
    AutoOptimizer, OptimizationStrategy, PerformanceMetrics, PlatformCapabilities,
    WorkloadCharacteristics, WorkloadType,
};
