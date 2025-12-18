//! VM通用库
//!
//! 提供虚拟机实现中使用的通用数据结构和工具

pub mod lockfree;

// 重新导出主要类型
pub use lockfree::*;