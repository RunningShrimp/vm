//! 增量式垃圾回收
//!
//! 提供基于时间预算的增量式垃圾收集，减少GC暂停时间

pub mod base;
pub mod enhanced;

// Re-export commonly used types from base implementation
pub use base::{IncrementalGc as BaseIncrementalGc, IncrementalPhase, IncrementalProgress};

// Re-export enhanced implementation types
pub use enhanced::{
    GCPhase, IncrementalGC, IncrementalGC as EnhancedIncrementalGC, IncrementalGCConfig,
    IncrementalGCStats, MarkStack,
};
