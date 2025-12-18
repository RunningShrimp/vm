//! 高级性能优化模块
//!
//! 提供各种高级性能优化技术：
//! - 预取优化：智能预取数据
//! - 批处理优化：批量处理操作
//! - 缓存友好：优化数据结构布局
//! - SIMD优化：使用SIMD指令加速

pub mod prefetch;
pub mod batch;
pub mod cache_friendly;
pub mod simd_opt;

// 重新导出主要类型
pub use prefetch::*;
pub use batch::*;
pub use cache_friendly::*;
pub use simd_opt::*;