pub mod advanced;
pub mod unified;

// 导出 cache_friendly 模块的主要类型 (Round 25: 缓存优化)
pub use advanced::cache_friendly::{
    AlignmentInfo, CopyStrategy, FastMemoryCopier, MemoryCopyConfig, MemoryCopyStats,
    MemoryCopyStatsSnapshot,
};

pub use unified::{
    MemoryManagerFactory, MemoryPool, MemoryStats, PhysicalMemoryManager, UnifiedMemoryManager,
};
