//! 分代垃圾回收
//!
//! 实现年轻代和老年代分离的分代GC策略

pub mod base;
pub mod enhanced;

// Re-export commonly used types from base implementation
pub use base::{
    Generation, GenerationalGC as BaseGenerationalGC,
    GenerationalGCStats as BaseGenerationalGCStats, GenerationalGcResult, OldGenerationConfig,
    YoungGCStrategy, YoungGenerationConfig,
};

// Re-export enhanced implementation types
pub use enhanced::{
    Card, CardTable, GenerationalGC, GenerationalGCConfig, GenerationalGCStats, ObjectMetadata,
};
