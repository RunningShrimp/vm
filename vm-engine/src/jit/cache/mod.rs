//! 缓存管理模块（基础设施层）
//!
//! 提供通用的缓存管理实现，支持多种替换策略。
//! 这是基础设施层的实现，具体的技术细节应在此模块中。

pub mod manager;

pub use manager::{CacheEntry, CacheReplacementPolicy, GenericCacheManager};
