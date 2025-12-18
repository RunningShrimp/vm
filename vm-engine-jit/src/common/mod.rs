//! JIT引擎通用模块
//!
//! 本模块包含JIT引擎的通用基础设施，用于减少重复代码并提高一致性。

pub mod stats;
pub mod error;
pub mod config;

// 重新导出主要类型
pub use stats::*;
pub use error::*;
pub use config::*;