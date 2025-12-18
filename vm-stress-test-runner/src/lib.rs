//! VM压力测试运行器
//!
//! 本模块提供全面的压力测试功能，包括：
//! - 跨架构翻译压力测试
//! - JIT编译压力测试
//! - 内存管理压力测试
//! - 资源泄漏检测

pub mod stress_test_runner;

pub use stress_test_runner::*;