//! 性能优化模块
//!
//! 提供各种性能优化实现：
//! - 无锁优化：减少锁竞争的优化技术
//! - 汇编优化：使用汇编指令优化关键路径
//! - 高级优化：预取、批处理、缓存友好和SIMD优化

pub mod advanced;
pub mod asm_opt;
pub mod lockless_optimizations;

// 重新导出主要类型
pub use advanced::*;
pub use asm_opt::*;
pub use lockless_optimizations::*;
