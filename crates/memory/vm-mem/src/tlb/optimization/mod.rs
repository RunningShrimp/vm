//! TLB优化策略
//!
//! 本模块包含TLB的各种优化策略：
//! - **Const泛型优化**: 使用const泛型实现零开销TLB
//! - 自适应替换策略：根据访问模式动态调整替换算法
//! - 访问模式追踪：追踪和分析地址访问模式
//! - 预测器：基于马尔可夫链的访问预测
//! - 预热机制：主动预热常用地址

pub mod access_pattern;
pub mod adaptive;
pub mod const_generic;
pub mod predictor;
pub mod prefetch;

// 重新导出主要类型
pub use access_pattern::*;
pub use adaptive::*;
pub use const_generic::*;
pub use predictor::*;
pub use prefetch::*;
