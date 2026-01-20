//! JIT引擎通用模块
//!
//! 本模块包含JIT引擎的通用基础设施，用于减少重复代码并提高一致性。

pub mod config;
pub mod error;
pub mod stats;

// 重新导出主要类型
pub use config::*;
pub use error::*;
pub use stats::*;
