//! 实时性能监控系统
//!
//! Round 37: 生产级优化系统
//!
//! 提供持续的性能监控、分析和自动优化能力:
//! - 实时性能指标收集
//! - 性能趋势分析
//! - 异常检测
//! - 自动优化触发

use std::collections::VecDeque;
use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

/// 实时性能指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealTimeMetrics {
    /// 时间戳 (纳秒)
    pub timestamp_ns: u64,
    /// 操作类型
    pub operation_type: String,
    /// 操作耗时 (纳秒)
    pub latency_ns: u64,
    /// 内存使用 (字节)
    pub memory_bytes: u64,
    /// CPU使用率 (0-100)
    pub cpu_percent: f64,
    /// 吞吐量 (操作/秒)
    pub throughput_ops_per_sec: f64,
}

/// 性能统计窗口
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceWindow {
    /// 窗口开始时间
    pub start_ns: u64,
    /// 窗口结束时间
    pub end_ns: u64,
    /// 样本数量
    pub sample_count: usize,
    /// 平均延迟 (纳秒)
    pub avg_latency_ns: f64,
    /// P50延迟 (纳秒)
    pub p50_latency_ns: u64,
    /// P95延迟 (纳秒)
    pub p95_latency_ns: u64,
    /// P99延迟 (纳秒)
    pub p99_latency_ns: u64,
    /// 最小延迟 (纳秒)
    pub min_latency_ns: u64,
    /// 最大延迟 (纳秒)
    pub max_latency_ns: u64,
    /// 标准差
    pub std_dev_ns: f64,
    /// 总吞吐量
    pub total_throughput: f64,
}

/// 性能异常检测
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAnomaly {
    /// 异常类型
    pub anomaly_type: AnomalyType,
    /// 检测时间
    pub detected_at_ns: u64,
    /// 严重程度 (0-1)
    pub severity: f64,
    /// 描述
    pub description: String,
    /// 建议操作
    pub suggested_action: String,
}

/// 异常类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnomalyType {
    /// 延迟突增
    LatencySpike,
    /// 内存泄漏
    MemoryLeak,
    /// CPU过载
    CPUOverload,
    /// 吞吐量下降
    ThroughputDrop,
    /// 性能回归
    PerformanceRegression,
}

/// 实时性能监控器
pub struct RealTimeMonitor {
    /// 指标历史 (最近10000条)
    metrics_history: Arc<Mutex<VecDeque<RealTimeMetrics>>>,
    /// 当前窗口统计
    current_window: Arc<Mutex<Option<PerformanceWindow>>>,
    /// 检测到的异常
    anomalies: Arc<Mutex<Vec<PerformanceAnomaly>>>,
    /// 性能基线
    baseline: Arc<Mutex<Option<PerformanceWindow>>>,
}

impl RealTimeMonitor {
    /// 创建新的实时监控器
    pub fn new() -> Self {
        Self {
            metrics_history: Arc::new(Mutex::new(VecDeque::with_capacity(10000))),
            current_window: Arc::new(Mutex::new(None)),
            anomalies: Arc::new(Mutex::new(Vec::new())),
            baseline: Arc::new(Mutex::new(None)),
        }
    }

    /// 记录实时指标
    pub fn record_metric(&self, metric: RealTimeMetrics) {
        let mut history = self.metrics_history.lock();
        history.push_back(metric);

        // 保持最近10000条记录
        if history.len() > 10000 {
            history.pop_front();
        }

        // 每100条样本更新一次统计窗口
        if history.len().is_multiple_of(100) {
            self.update_window();
            self.detect_anomalies();
        }
    }

    /// 更新统计窗口
    fn update_window(&self) {
        let history = self.metrics_history.lock();
        if history.len() < 10 {
            return;
        }

        let latencies: Vec<u64> = history.iter().map(|m| m.latency_ns).collect();

        let sorted = {
            let mut sorted = latencies.clone();
            sorted.sort();
            sorted
        };

        let count = latencies.len();
        let sum: u64 = latencies.iter().sum();
        let avg = sum as f64 / count as f64;

        let variance = latencies
            .iter()
            .map(|&x| {
                let diff = x as f64 - avg;
                diff * diff
            })
            .sum::<f64>()
            / count as f64;
        let std_dev = variance.sqrt();

        // 计算百分位数
        let p50 = sorted[count * 50 / 100];
        let p95 = sorted[count * 95 / 100];
        let p99 = sorted[count * 99 / 100];

        let min = sorted[0];
        let max = sorted[count - 1];

        // 计算吞吐量
        let duration_ns =
            history.back().unwrap().timestamp_ns - history.front().unwrap().timestamp_ns;
        let throughput = if duration_ns > 0 {
            count as f64 * 1_000_000_000.0 / duration_ns as f64
        } else {
            0.0
        };

        let window = PerformanceWindow {
            start_ns: history.front().unwrap().timestamp_ns,
            end_ns: history.back().unwrap().timestamp_ns,
            sample_count: count,
            avg_latency_ns: avg,
            p50_latency_ns: p50,
            p95_latency_ns: p95,
            p99_latency_ns: p99,
            min_latency_ns: min,
            max_latency_ns: max,
            std_dev_ns: std_dev,
            total_throughput: throughput,
        };

        *self.current_window.lock() = Some(window.clone());

        // 如果还没有基线,设置当前窗口为基线
        if self.baseline.lock().is_none() {
            *self.baseline.lock() = Some(window);
        }
    }

    /// 检测性能异常
    fn detect_anomalies(&self) {
        let current = self.current_window.lock();
        let baseline = self.baseline.lock();

        let (current, baseline) = match (current.as_ref(), baseline.as_ref()) {
            (Some(c), Some(b)) => (c, b),
            _ => return,
        };

        let mut anomalies = self.anomalies.lock();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        // 检测延迟突增 (当前平均 > 基线平均 * 2)
        if current.avg_latency_ns > baseline.avg_latency_ns * 2.0 {
            anomalies.push(PerformanceAnomaly {
                anomaly_type: AnomalyType::LatencySpike,
                detected_at_ns: now,
                severity: (current.avg_latency_ns / baseline.avg_latency_ns - 1.0).min(1.0),
                description: format!(
                    "延迟突增: {:.1}x (基线: {:.0}ns → 当前: {:.0}ns)",
                    current.avg_latency_ns / baseline.avg_latency_ns,
                    baseline.avg_latency_ns,
                    current.avg_latency_ns
                ),
                suggested_action: "检查系统负载,考虑启用更多优化".to_string(),
            });
        }

        // 检测吞吐量下降 (当前吞吐量 < 基线 * 0.8)
        if current.total_throughput < baseline.total_throughput * 0.8 {
            anomalies.push(PerformanceAnomaly {
                anomaly_type: AnomalyType::ThroughputDrop,
                detected_at_ns: now,
                severity: (1.0 - current.total_throughput / baseline.total_throughput).min(1.0),
                description: format!(
                    "吞吐量下降: {:.1}% (基线: {:.0} ops/s → 当前: {:.0} ops/s)",
                    (1.0 - current.total_throughput / baseline.total_throughput) * 100.0,
                    baseline.total_throughput,
                    current.total_throughput
                ),
                suggested_action: "检查瓶颈,重新评估优化策略".to_string(),
            });
        }

        // 检测P99延迟恶化
        if current.p99_latency_ns > (baseline.p99_latency_ns as f64 * 1.5) as u64 {
            let severity_ratio =
                current.p99_latency_ns as f64 / baseline.p99_latency_ns as f64 - 1.0;
            anomalies.push(PerformanceAnomaly {
                anomaly_type: AnomalyType::PerformanceRegression,
                detected_at_ns: now,
                severity: severity_ratio.min(1.0),
                description: format!(
                    "P99延迟恶化: {:.1}x (基线: {:.0}ns → 当前: {:.0}ns)",
                    current.p99_latency_ns as f64 / baseline.p99_latency_ns as f64,
                    baseline.p99_latency_ns,
                    current.p99_latency_ns
                ),
                suggested_action: "启用热点检测和优化".to_string(),
            });
        }

        // 保持最近100个异常
        let len = anomalies.len();
        if len > 100 {
            anomalies.drain(0..len - 100);
        }
    }

    /// 获取当前统计窗口
    pub fn current_window(&self) -> Option<PerformanceWindow> {
        self.current_window.lock().clone()
    }

    /// 获取性能基线
    pub fn baseline(&self) -> Option<PerformanceWindow> {
        self.baseline.lock().clone()
    }

    /// 获取最近的异常
    pub fn recent_anomalies(&self, count: usize) -> Vec<PerformanceAnomaly> {
        let anomalies = self.anomalies.lock();
        anomalies.iter().rev().take(count).cloned().collect()
    }

    /// 设置性能基线
    pub fn set_baseline(&self, window: PerformanceWindow) {
        *self.baseline.lock() = Some(window);
    }

    /// 重置监控
    pub fn reset(&self) {
        self.metrics_history.lock().clear();
        *self.current_window.lock() = None;
        self.anomalies.lock().clear();
        *self.baseline.lock() = None;
    }
}

impl Default for RealTimeMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_real_time_monitor() {
        let monitor = RealTimeMonitor::new();

        // 模拟性能指标
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        for i in 0..150 {
            let metric = RealTimeMetrics {
                timestamp_ns: now + i * 1_000_000,
                operation_type: "test".to_string(),
                latency_ns: 1000 + (i % 100) * 10,
                memory_bytes: 1024,
                cpu_percent: 50.0,
                throughput_ops_per_sec: 1000.0,
            };
            monitor.record_metric(metric);
        }

        let window = monitor.current_window();
        assert!(window.is_some());
        let window = window.unwrap();
        assert_eq!(window.sample_count, 100); // 最后100条
    }

    #[test]
    fn test_anomaly_detection() {
        let monitor = RealTimeMonitor::new();

        // 设置基线
        let baseline = PerformanceWindow {
            start_ns: 0,
            end_ns: 1_000_000_000,
            sample_count: 100,
            avg_latency_ns: 1000.0,
            p50_latency_ns: 900,
            p95_latency_ns: 1500,
            p99_latency_ns: 2000,
            min_latency_ns: 500,
            max_latency_ns: 3000,
            std_dev_ns: 200.0,
            total_throughput: 100.0,
        };
        monitor.set_baseline(baseline);

        // 模拟性能下降
        let degraded = PerformanceWindow {
            start_ns: 0,
            end_ns: 1_000_000_000,
            sample_count: 100,
            avg_latency_ns: 3000.0, // 3x恶化
            p50_latency_ns: 2500,
            p95_latency_ns: 4000,
            p99_latency_ns: 5000,
            min_latency_ns: 1000,
            max_latency_ns: 8000,
            std_dev_ns: 500.0,
            total_throughput: 50.0, // 50%下降
        };
        monitor.set_baseline(degraded);

        let anomalies = monitor.recent_anomalies(10);
        // 应该检测到延迟突增和吞吐量下降
        assert!(!anomalies.is_empty());
    }
}
