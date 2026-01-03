//! 核心TLB实现
//!
//! 本模块包含TLB的核心实现：
//! - 基础TLB：简单实现，适用于基本场景
//! - 并发TLB：无锁设计，适用于高并发场景
//! - Per-CPU TLB：每CPU独立TLB，避免锁竞争
//! - 统一TLB：统一接口，支持动态选择最佳实现
//! - 优化哈希TLB：针对大规模配置优化的哈希TLB

pub mod basic;
pub mod concurrent;
pub mod lockfree;
pub mod optimized_hash;
pub mod per_cpu;
pub mod unified;

// 重新导出主要类型
pub use basic::*;
pub use concurrent::{ConcurrentTlbConfig, ShardedTlb};
pub use lockfree::{LockFreeTlb, TlbEntry as LockFreeTlbEntry};
pub use optimized_hash::{ConcurrentOptimizedHashTlb, HashTlbStats, OptimizedHashTlb, PackedTlbEntry};
pub use per_cpu::*;
// 从unified模块导入，但重命名TlbStats以避免冲突
pub use unified::TlbStats as UnifiedTlbStats;
pub use unified::{MultiLevelTlb, MultiLevelTlbConfig};
