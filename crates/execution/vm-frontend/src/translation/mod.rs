//! 跨架构翻译模块
//!
//! 提供高效的指令翻译和缓存功能。

pub mod batch;
pub mod cache;

pub use batch::{BatchTranslationConfig, BatchTranslationResult, BatchTranslator};
pub use cache::{CacheEntry, CacheStatsSnapshot, TranslationCache};
