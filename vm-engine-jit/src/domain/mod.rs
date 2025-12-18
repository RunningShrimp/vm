//! JIT引擎领域模型
//!
//! 本模块定义了JIT引擎的领域模型和限界上下文，遵循DDD设计原则。

pub mod compilation;
pub mod optimization;
pub mod execution;
pub mod caching;
pub mod monitoring;
pub mod hardware_acceleration;
pub mod service;

// 重新导出主要类型
pub use compilation::*;
pub use optimization::*;
pub use execution::*;
pub use caching::*;
pub use monitoring::*;
pub use hardware_acceleration::*;
pub use service::*;