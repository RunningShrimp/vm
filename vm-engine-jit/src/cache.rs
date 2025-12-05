//! 代码缓存模块（已废弃）
//!
//! 此模块中的类型已迁移到 `unified_cache` 模块。
//! 保留此文件仅用于向后兼容，新代码应使用 `unified_cache` 模块。
//!
//! # 迁移指南
//!
//! - `TbColor` 和 `TbMetadata` 已移动到 `unified_cache` 模块
//! - `CodeCache` trait、`HotCache`、`ColdCache`、`LayeredCodeCache` 已废弃
//! - 请使用 `UnifiedCodeCache` 替代

// 重新导出 unified_cache 中的类型以保持向后兼容
pub use super::unified_cache::{TbColor, TbMetadata};
