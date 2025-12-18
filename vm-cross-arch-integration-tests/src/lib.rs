//! 跨架构集成测试库
//!
//! 本库提供跨架构集成测试的框架和工具，用于验证VM项目的跨架构兼容性

pub mod cross_arch_integration_tests;
pub mod cross_arch_integration_tests_part2;
pub mod cross_arch_integration_tests_part3;

pub use cross_arch_integration_tests::{
    CrossArchIntegrationTestFramework, 
    CrossArchTestConfig, 
    CrossArchTestResult,
    CrossArchPerformanceMetrics
};