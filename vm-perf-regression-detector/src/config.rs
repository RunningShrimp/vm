//! 性能回归检测配置

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 性能回归检测器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionDetectorConfig {
    /// 数据库路径
    pub database_path: String,
    /// 指标阈值配置
    pub metric_thresholds: HashMap<String, MetricThreshold>,
    /// 回归检测算法配置
    pub detection_config: DetectionConfig,
    /// 报告配置
    pub report_config: ReportConfig,
}

/// 指标阈值配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricThreshold {
    /// 警告阈值（百分比）
    pub warning_threshold: f64,
    /// 错误阈值（百分比）
    pub error_threshold: f64,
    /// 最小样本数
    pub min_samples: usize,
    /// 是否启用此指标
    pub enabled: bool,
}

/// 回归检测算法配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionConfig {
    /// 检测算法类型
    pub algorithm: DetectionAlgorithm,
    /// 统计显著性水平
    pub significance_level: f64,
    /// 时间窗口（天）
    pub time_window_days: u32,
    /// 最小历史数据点数
    pub min_history_points: usize,
}

/// 报告配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportConfig {
    /// 报告格式
    pub format: ReportFormat,
    /// 输出路径
    pub output_path: String,
    /// 是否生成图表
    pub generate_charts: bool,
    /// 图表输出路径
    pub charts_path: String,
}

/// 检测算法类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DetectionAlgorithm {
    /// Z-score检测
    ZScore,
    /// T-test检测
    TTest,
    /// Mann-Whitney U检测
    MannWhitneyU,
    /// 移动平均检测
    MovingAverage { window_size: usize },
    /// 线性回归检测
    LinearRegression,
}

/// 报告格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReportFormat {
    /// 文本格式
    Text,
    /// JSON格式
    Json,
    /// HTML格式
    Html,
    /// Markdown格式
    Markdown,
}

impl Default for RegressionDetectorConfig {
    fn default() -> Self {
        let mut metric_thresholds = HashMap::new();
        
        // 默认指标阈值
        metric_thresholds.insert("execution_time".to_string(), MetricThreshold {
            warning_threshold: 10.0,  // 10%警告
            error_threshold: 25.0,    // 25%错误
            min_samples: 5,
            enabled: true,
        });
        
        metric_thresholds.insert("memory_usage".to_string(), MetricThreshold {
            warning_threshold: 15.0,  // 15%警告
            error_threshold: 30.0,    // 30%错误
            min_samples: 5,
            enabled: true,
        });
        
        metric_thresholds.insert("jit_compilation_time".to_string(), MetricThreshold {
            warning_threshold: 20.0,  // 20%警告
            error_threshold: 40.0,    // 40%错误
            min_samples: 3,
            enabled: true,
        });
        
        metric_thresholds.insert("instruction_throughput".to_string(), MetricThreshold {
            warning_threshold: -10.0, // -10%警告（下降）
            error_threshold: -20.0,   // -20%错误（下降）
            min_samples: 5,
            enabled: true,
        });
        
        Self {
            database_path: "performance_data.db".to_string(),
            metric_thresholds,
            detection_config: DetectionConfig {
                algorithm: DetectionAlgorithm::ZScore,
                significance_level: 0.05,
                time_window_days: 30,
                min_history_points: 10,
            },
            report_config: ReportConfig {
                format: ReportFormat::Text,
                output_path: "regression_report.txt".to_string(),
                generate_charts: false,
                charts_path: "charts".to_string(),
            },
        }
    }
}

impl Default for DetectionAlgorithm {
    fn default() -> Self {
        Self::ZScore
    }
}

impl Default for ReportFormat {
    fn default() -> Self {
        Self::Text
    }
}

impl RegressionDetectorConfig {
    /// 从文件加载配置
    pub fn from_file(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = serde_json::from_str(&content)?;
        Ok(config)
    }
}