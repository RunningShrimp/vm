//! JIT统计信息占位实现
//!
//! ## 占位符说明
//!
//! `JITStats` 是为未来JIT统计功能预留的结构体。
//! 当前实际的统计功能分散在各个模块中（如 `OptimizingJITStats`）。
//! 此占位符保留以避免API breaking changes，等待统计系统的统一重构。

/// JIT编译统计信息（占位符）
///
/// 预留用于未来统一JIT统计系统。当前请使用各个子模块的专用统计结构：
/// - `OptimizingJITStats`: 优化编译器统计
/// - `InstructionSchedulingStats`: 指令调度统计
/// - `TieredCacheStats`: 分层缓存统计
pub struct JITStats;
