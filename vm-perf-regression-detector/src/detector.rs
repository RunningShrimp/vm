//! 性能回归检测算法

use anyhow::Result;
use serde::{Deserialize, Serialize};
use statrs::distribution::{ContinuousCDF, Normal, StudentsT};

use super::collector::PerformanceMetrics;
use super::config::{DetectionAlgorithm, MetricThreshold, RegressionDetectorConfig};

/// 回归检测结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionResult {
    /// 指标名称
    pub metric_name: String,
    /// 当前值
    pub current_value: f64,
    /// 基准值（历史平均值）
    pub baseline_value: f64,
    /// 变化百分比
    pub percentage_change: f64,
    /// 回归严重程度
    pub severity: RegressionSeverity,
    /// 统计显著性p值
    pub p_value: Option<f64>,
    /// 检测算法
    pub algorithm: String,
    /// 检测时间
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// 回归严重程度
#[derive(Debug, Clone, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum RegressionSeverity {
    /// 无回归
    None,
    /// 轻微回归
    Minor,
    /// 中等回归
    Moderate,
    /// 严重回归
    Major,
    /// 关键回归
    Critical,
}

/// 性能回归检测器
pub struct RegressionDetector {
    config: RegressionDetectorConfig,
}

impl RegressionDetector {
    /// 创建新的回归检测器
    pub fn new(config: RegressionDetectorConfig) -> Self {
        Self { config }
    }

    /// 检测性能回归
    pub fn detect_regressions(
        &self,
        current_metrics: &PerformanceMetrics,
        historical_metrics: &[PerformanceMetrics],
    ) -> Result<Vec<RegressionResult>> {
        let mut results = Vec::new();

        // 检测执行时间回归
        if let Some(threshold) = self.config.metric_thresholds.get("execution_time")
            && threshold.enabled
            && let Some(result) = self.detect_execution_time_regression(
                current_metrics,
                historical_metrics,
                threshold,
            )?
        {
            results.push(result);
        }

        // 检测内存使用回归
        if let Some(threshold) = self.config.metric_thresholds.get("memory_usage")
            && threshold.enabled
            && let Some(result) =
                self.detect_memory_usage_regression(current_metrics, historical_metrics, threshold)?
        {
            results.push(result);
        }

        // 检测JIT编译时间回归
        if let Some(threshold) = self.config.metric_thresholds.get("jit_compilation_time")
            && threshold.enabled
            && let Some(result) = self.detect_jit_compilation_time_regression(
                current_metrics,
                historical_metrics,
                threshold,
            )?
        {
            results.push(result);
        }

        // 检测指令吞吐量回归
        if let Some(threshold) = self.config.metric_thresholds.get("instruction_throughput")
            && threshold.enabled
            && let Some(result) = self.detect_instruction_throughput_regression(
                current_metrics,
                historical_metrics,
                threshold,
            )?
        {
            results.push(result);
        }

        // 检测自定义指标回归
        for (metric_name, value) in &current_metrics.custom_metrics {
            if let Some(threshold) = self.config.metric_thresholds.get(metric_name)
                && threshold.enabled
            {
                let historical_values: Vec<f64> = historical_metrics
                    .iter()
                    .filter_map(|m| m.custom_metrics.get(metric_name).copied())
                    .collect();

                if historical_values.len() >= threshold.min_samples
                    && let Some(result) = self.detect_custom_metric_regression(
                        metric_name,
                        *value,
                        &historical_values,
                        threshold,
                    )?
                {
                    results.push(result);
                }
            }
        }

        Ok(results)
    }

    /// 检测执行时间回归
    fn detect_execution_time_regression(
        &self,
        current_metrics: &PerformanceMetrics,
        historical_metrics: &[PerformanceMetrics],
        threshold: &MetricThreshold,
    ) -> Result<Option<RegressionResult>> {
        let current_value = current_metrics.execution_time_us as f64;
        let historical_values: Vec<f64> = historical_metrics
            .iter()
            .map(|m| m.execution_time_us as f64)
            .collect();

        if historical_values.len() < threshold.min_samples {
            return Ok(None);
        }

        self.detect_regression(
            "execution_time",
            current_value,
            &historical_values,
            threshold,
        )
    }

    /// 检测内存使用回归
    fn detect_memory_usage_regression(
        &self,
        current_metrics: &PerformanceMetrics,
        historical_metrics: &[PerformanceMetrics],
        threshold: &MetricThreshold,
    ) -> Result<Option<RegressionResult>> {
        let current_value = current_metrics.memory_usage_bytes as f64;
        let historical_values: Vec<f64> = historical_metrics
            .iter()
            .map(|m| m.memory_usage_bytes as f64)
            .collect();

        if historical_values.len() < threshold.min_samples {
            return Ok(None);
        }

        self.detect_regression("memory_usage", current_value, &historical_values, threshold)
    }

    /// 检测JIT编译时间回归
    fn detect_jit_compilation_time_regression(
        &self,
        current_metrics: &PerformanceMetrics,
        historical_metrics: &[PerformanceMetrics],
        threshold: &MetricThreshold,
    ) -> Result<Option<RegressionResult>> {
        let current_value = current_metrics.jit_compilation_time_us as f64;
        let historical_values: Vec<f64> = historical_metrics
            .iter()
            .map(|m| m.jit_compilation_time_us as f64)
            .collect();

        if historical_values.len() < threshold.min_samples {
            return Ok(None);
        }

        self.detect_regression(
            "jit_compilation_time",
            current_value,
            &historical_values,
            threshold,
        )
    }

    /// 检测指令吞吐量回归
    fn detect_instruction_throughput_regression(
        &self,
        current_metrics: &PerformanceMetrics,
        historical_metrics: &[PerformanceMetrics],
        threshold: &MetricThreshold,
    ) -> Result<Option<RegressionResult>> {
        let current_value = current_metrics.instruction_throughput;
        let historical_values: Vec<f64> = historical_metrics
            .iter()
            .map(|m| m.instruction_throughput)
            .collect();

        if historical_values.len() < threshold.min_samples {
            return Ok(None);
        }

        self.detect_regression(
            "instruction_throughput",
            current_value,
            &historical_values,
            threshold,
        )
    }

    /// 检测自定义指标回归
    fn detect_custom_metric_regression(
        &self,
        metric_name: &str,
        current_value: f64,
        historical_values: &[f64],
        threshold: &MetricThreshold,
    ) -> Result<Option<RegressionResult>> {
        if historical_values.len() < threshold.min_samples {
            return Ok(None);
        }

        self.detect_regression(metric_name, current_value, historical_values, threshold)
    }

    /// 通用回归检测方法
    fn detect_regression(
        &self,
        metric_name: &str,
        current_value: f64,
        historical_values: &[f64],
        threshold: &MetricThreshold,
    ) -> Result<Option<RegressionResult>> {
        // 计算基准值（历史平均值）
        let baseline_value = historical_values.iter().sum::<f64>() / historical_values.len() as f64;

        // 计算变化百分比
        let percentage_change = ((current_value - baseline_value) / baseline_value) * 100.0;

        // 根据配置的算法进行检测
        let (p_value, algorithm_name) = match &self.config.detection_config.algorithm {
            DetectionAlgorithm::ZScore => {
                let p_value = self.z_score_test(current_value, historical_values)?;
                (Some(p_value), "Z-Score".to_string())
            }
            DetectionAlgorithm::TTest => {
                let p_value = self.t_test(current_value, historical_values)?;
                (Some(p_value), "T-Test".to_string())
            }
            DetectionAlgorithm::MannWhitneyU => {
                let p_value = self.mann_whitney_u_test(current_value, historical_values)?;
                (Some(p_value), "Mann-Whitney U".to_string())
            }
            DetectionAlgorithm::MovingAverage { window_size } => {
                let p_value =
                    self.moving_average_test(current_value, historical_values, *window_size)?;
                (
                    Some(p_value),
                    format!("Moving Average (window={})", window_size),
                )
            }
            DetectionAlgorithm::LinearRegression => {
                let p_value = self.linear_regression_test(current_value, historical_values)?;
                (Some(p_value), "Linear Regression".to_string())
            }
        };

        // 确定回归严重程度
        let severity = if percentage_change >= threshold.error_threshold {
            RegressionSeverity::Critical
        } else if percentage_change >= threshold.error_threshold * 0.75 {
            RegressionSeverity::Major
        } else if percentage_change >= threshold.warning_threshold {
            RegressionSeverity::Moderate
        } else if percentage_change >= threshold.warning_threshold * 0.5 {
            RegressionSeverity::Minor
        } else {
            RegressionSeverity::None
        };

        // 如果没有回归，返回None
        if severity == RegressionSeverity::None {
            return Ok(None);
        }

        Ok(Some(RegressionResult {
            metric_name: metric_name.to_string(),
            current_value,
            baseline_value,
            percentage_change,
            severity,
            p_value,
            algorithm: algorithm_name,
            timestamp: chrono::Utc::now(),
        }))
    }

    /// Z-score检测
    fn z_score_test(&self, current_value: f64, historical_values: &[f64]) -> Result<f64> {
        let mean = historical_values.iter().sum::<f64>() / historical_values.len() as f64;
        let variance = historical_values
            .iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>()
            / (historical_values.len() - 1) as f64;
        let std_dev = variance.sqrt();

        if std_dev == 0.0 {
            return Ok(1.0); // 无变化，p值为1
        }

        let z_score = (current_value - mean) / std_dev;
        let normal = Normal::new(0.0, 1.0)?;
        let p_value = 1.0 - normal.cdf(z_score.abs());

        Ok(p_value)
    }

    /// T-test检测
    fn t_test(&self, current_value: f64, historical_values: &[f64]) -> Result<f64> {
        let n = historical_values.len();
        if n < 2 {
            return Ok(1.0);
        }

        let mean = historical_values.iter().sum::<f64>() / n as f64;
        let variance = historical_values
            .iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>()
            / (n - 1) as f64;
        let std_dev = variance.sqrt();

        if std_dev == 0.0 {
            return Ok(1.0);
        }

        let t_statistic = (current_value - mean) / (std_dev / (n as f64).sqrt());
        let t_dist = StudentsT::new(0.0, 1.0, (n - 1) as f64)?;
        let p_value: f64 = 2.0 * (1.0 - t_dist.cdf(t_statistic.abs()));

        Ok(p_value.min(1.0))
    }

    /// Mann-Whitney U检测
    fn mann_whitney_u_test(&self, current_value: f64, historical_values: &[f64]) -> Result<f64> {
        // 简化实现，实际应该实现完整的Mann-Whitney U检验
        // 这里使用Z-score作为近似
        self.z_score_test(current_value, historical_values)
    }

    /// 移动平均检测
    fn moving_average_test(
        &self,
        current_value: f64,
        historical_values: &[f64],
        window_size: usize,
    ) -> Result<f64> {
        if historical_values.len() < window_size {
            return self.z_score_test(current_value, historical_values);
        }

        let recent_values = &historical_values[historical_values.len() - window_size..];
        let moving_avg = recent_values.iter().sum::<f64>() / window_size as f64;

        let variance = recent_values
            .iter()
            .map(|x| (x - moving_avg).powi(2))
            .sum::<f64>()
            / (window_size - 1) as f64;
        let std_dev = variance.sqrt();

        if std_dev == 0.0 {
            return Ok(1.0);
        }

        let z_score = (current_value - moving_avg) / std_dev;
        let normal = Normal::new(0.0, 1.0)?;
        let p_value = 1.0 - normal.cdf(z_score.abs());

        Ok(p_value)
    }

    /// 线性回归检测
    fn linear_regression_test(&self, current_value: f64, historical_values: &[f64]) -> Result<f64> {
        if historical_values.len() < 3 {
            return self.z_score_test(current_value, historical_values);
        }

        // 简化实现，计算趋势并预测当前值
        let n = historical_values.len();
        let x_sum: f64 = (0..n).map(|i| i as f64).sum();
        let y_sum: f64 = historical_values.iter().sum();
        let xy_sum: f64 = historical_values
            .iter()
            .enumerate()
            .map(|(i, y)| i as f64 * y)
            .sum();
        let x2_sum: f64 = (0..n).map(|i| (i as f64).powi(2)).sum();

        let slope = (n as f64 * xy_sum - x_sum * y_sum) / (n as f64 * x2_sum - x_sum.powi(2));
        let intercept = (y_sum - slope * x_sum) / n as f64;

        // 预测当前值
        let predicted_value = slope * (n as f64) + intercept;

        // 计算残差
        let residuals: Vec<f64> = historical_values
            .iter()
            .enumerate()
            .map(|(i, y)| y - (slope * i as f64 + intercept))
            .collect();

        let residual_variance = residuals.iter().map(|r| r.powi(2)).sum::<f64>() / (n - 2) as f64;
        let residual_std = residual_variance.sqrt();

        if residual_std == 0.0 {
            return Ok(1.0);
        }

        let z_score = (current_value - predicted_value) / residual_std;
        let normal = Normal::new(0.0, 1.0)?;
        let p_value = 1.0 - normal.cdf(z_score.abs());

        Ok(p_value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DetectionAlgorithm, MetricThreshold};
    use std::collections::HashMap;

    #[test]
    fn test_regression_detection() -> Result<()> {
        let config = RegressionDetectorConfig::default();
        let detector = RegressionDetector::new(config);

        // 创建历史数据
        let historical_values = vec![100.0, 105.0, 98.0, 102.0, 99.0];

        // 创建当前指标（增加25%，应该触发严重回归）
        let current_metrics = PerformanceMetrics {
            context: crate::collector::TestContext {
                src_arch: vm_core::GuestArch::X86_64,
                dst_arch: vm_core::GuestArch::Arm64,
                test_name: "test".to_string(),
                version: "1.0.0".to_string(),
                environment: crate::collector::PerformanceCollector::collect_environment_info(),
            },
            execution_time_us: 125, // 增加25%
            jit_compilation_time_us: 500,
            memory_usage_bytes: 1024 * 1024,
            instructions_translated: 1000,
            instruction_throughput: 1000000.0,
            cache_hit_rate: 0.95,
            custom_metrics: HashMap::new(),
            timestamp: chrono::Utc::now(),
        };

        // 创建历史指标
        let historical_metrics: Vec<PerformanceMetrics> = historical_values
            .iter()
            .enumerate()
            .map(|(i, &exec_time)| PerformanceMetrics {
                context: current_metrics.context.clone(),
                execution_time_us: exec_time as u64,
                jit_compilation_time_us: 500,
                memory_usage_bytes: 1024 * 1024,
                instructions_translated: 1000,
                instruction_throughput: 1000000.0,
                cache_hit_rate: 0.95,
                custom_metrics: HashMap::new(),
                timestamp: chrono::Utc::now()
                    - chrono::Duration::days((historical_values.len() - i) as i64),
            })
            .collect();

        // 检测回归
        let results = detector.detect_regressions(&current_metrics, &historical_metrics)?;

        // 应该检测到执行时间回归
        assert!(!results.is_empty());
        let exec_time_regression = results
            .iter()
            .find(|r| r.metric_name == "execution_time")
            .ok_or_else(|| anyhow::anyhow!("Execution time regression not found in results"))?;

        assert_eq!(exec_time_regression.severity, RegressionSeverity::Critical);
        assert!(exec_time_regression.percentage_change >= 25.0);

        Ok(())
    }
}
