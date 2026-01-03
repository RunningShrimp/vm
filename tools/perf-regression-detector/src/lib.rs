//! 性能回归检测系统
//!
//! 本模块提供性能回归检测功能，包括：
//! - 性能指标收集
//! - 历史数据存储
//! - 回归检测算法
//! - 报告生成

pub mod collector;
pub mod config;
pub mod detector;
pub mod reporter;
pub mod storage;

pub use collector::{PerformanceCollector, PerformanceMetrics};
pub use config::ReportFormat;
pub use config::{MetricThreshold, RegressionDetectorConfig};
pub use detector::{RegressionDetector, RegressionResult, RegressionSeverity};
pub use reporter::RegressionReporter;
pub use storage::{PerformanceStorage, SqlitePerformanceStorage};
