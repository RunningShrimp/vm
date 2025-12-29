//! 实时性能监控 - 延迟分析与预测告警
//!
//! 提供：
//! - 尾延迟监控（P99、P99.9）
//! - 抖动检测与分析
//! - 预测性告警
//! - 性能指标采集

use parking_lot::RwLock;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;

/// 性能指标
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub min_latency_us: u64,
    pub max_latency_us: u64,
    pub avg_latency_us: f64,
    pub p50_latency_us: u64,
    pub p99_latency_us: u64,
    pub p999_latency_us: u64,
    pub jitter_us: u64,
    pub throughput_kops: f64,
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            min_latency_us: u64::MAX,
            max_latency_us: 0,
            avg_latency_us: 0.0,
            p50_latency_us: 0,
            p99_latency_us: 0,
            p999_latency_us: 0,
            jitter_us: 0,
            throughput_kops: 0.0,
        }
    }
}

/// 延迟样本
#[derive(Debug, Clone)]
pub struct LatencySample {
    pub timestamp: Instant,
    pub latency_us: u64,
    pub operation_id: u32,
}

/// 延迟分析器
pub struct LatencyAnalyzer {
    samples: Arc<RwLock<VecDeque<LatencySample>>>,
    max_samples: usize,
    metrics: Arc<RwLock<PerformanceMetrics>>,
}

impl LatencyAnalyzer {
    pub fn new(max_samples: usize) -> Self {
        Self {
            samples: Arc::new(RwLock::new(VecDeque::new())),
            max_samples,
            metrics: Arc::new(RwLock::new(PerformanceMetrics::default())),
        }
    }

    /// 记录延迟样本
    pub fn record_sample(&self, latency_us: u64, operation_id: u32) {
        let sample = LatencySample {
            timestamp: Instant::now(),
            latency_us,
            operation_id,
        };

        let mut samples = self.samples.write();
        samples.push_back(sample);

        if samples.len() > self.max_samples {
            samples.pop_front();
        }

        // 实时更新指标
        drop(samples);
        self.update_metrics();
    }

    /// 更新性能指标
    fn update_metrics(&self) {
        let samples = self.samples.read();
        if samples.is_empty() {
            return;
        }

        let mut latencies: Vec<u64> = samples.iter().map(|s| s.latency_us).collect();
        latencies.sort_unstable();

        let min = latencies[0];
        let max = latencies[latencies.len() - 1];
        let avg = latencies.iter().sum::<u64>() as f64 / latencies.len() as f64;

        let p50_idx = (latencies.len() * 50) / 100;
        let p99_idx = (latencies.len() * 99) / 100;
        let p999_idx = (latencies.len() * 999) / 1000;

        let p50 = latencies[p50_idx.min(latencies.len() - 1)];
        let p99 = latencies[p99_idx.min(latencies.len() - 1)];
        let p999 = latencies[p999_idx.min(latencies.len() - 1)];

        let jitter = max - min;

        let mut metrics = self.metrics.write();
        metrics.min_latency_us = min;
        metrics.max_latency_us = max;
        metrics.avg_latency_us = avg;
        metrics.p50_latency_us = p50;
        metrics.p99_latency_us = p99;
        metrics.p999_latency_us = p999;
        metrics.jitter_us = jitter;
        metrics.throughput_kops = 1_000_000.0 / avg / 1000.0;
    }

    /// 获取性能指标
    pub fn get_metrics(&self) -> PerformanceMetrics {
        self.metrics.read().clone()
    }

    /// 获取样本数
    pub fn get_sample_count(&self) -> usize {
        self.samples.read().len()
    }
}

/// 抖动检测器
pub struct JitterDetector {
    baseline_latency_us: u64,
    jitter_threshold_us: u64,
    detection_history: Arc<RwLock<VecDeque<(Instant, u64)>>>,
    anomalies: Arc<RwLock<Vec<(Instant, u64, String)>>>,
}

impl JitterDetector {
    pub fn new(baseline_latency_us: u64, jitter_threshold_us: u64) -> Self {
        Self {
            baseline_latency_us,
            jitter_threshold_us,
            detection_history: Arc::new(RwLock::new(VecDeque::new())),
            anomalies: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 检测延迟抖动
    pub fn detect(&self, latency_us: u64) {
        let now = Instant::now();
        let mut history = self.detection_history.write();
        history.push_back((now, latency_us));

        if history.len() > 100 {
            history.pop_front();
        }

        // 计算方差
        if history.len() >= 10 {
            let avg: f64 =
                history.iter().map(|(_, l)| *l as f64).sum::<f64>() / history.len() as f64;
            let variance = history
                .iter()
                .map(|(_, l)| {
                    let diff = *l as f64 - avg;
                    diff * diff
                })
                .sum::<f64>()
                / history.len() as f64;
            let stddev = variance.sqrt() as u64;

            if stddev > self.jitter_threshold_us {
                let deviation =
                    (latency_us as i64 - self.baseline_latency_us as i64).unsigned_abs();
                if deviation > self.jitter_threshold_us {
                    let mut anomalies = self.anomalies.write();
                    anomalies.push((
                        now,
                        latency_us,
                        format!("Jitter detected: {} us (stddev: {} us)", deviation, stddev),
                    ));

                    if anomalies.len() > 1000 {
                        anomalies.remove(0);
                    }
                }
            }
        }
    }

    /// 获取异常列表
    pub fn get_anomalies(&self) -> Vec<(Instant, u64, String)> {
        self.anomalies.read().clone()
    }

    /// 获取异常数
    pub fn get_anomaly_count(&self) -> usize {
        self.anomalies.read().len()
    }
}

/// 预测性告警
pub struct PredictiveAlert {
    analyzer: Arc<LatencyAnalyzer>,
    jitter_detector: Arc<JitterDetector>,
    p99_threshold_us: u64,
    p999_threshold_us: u64,
    anomaly_threshold: usize,
}

impl PredictiveAlert {
    pub fn new(
        analyzer: Arc<LatencyAnalyzer>,
        jitter_detector: Arc<JitterDetector>,
        p99_threshold_us: u64,
        p999_threshold_us: u64,
    ) -> Self {
        Self {
            analyzer,
            jitter_detector,
            p99_threshold_us,
            p999_threshold_us,
            anomaly_threshold: 5,
        }
    }

    /// 检查告警条件
    pub fn check_alerts(&self) -> Vec<String> {
        let mut alerts = Vec::new();
        let metrics = self.analyzer.get_metrics();

        if metrics.p99_latency_us > self.p99_threshold_us {
            alerts.push(format!(
                "P99 latency {} us exceeds threshold {} us",
                metrics.p99_latency_us, self.p99_threshold_us
            ));
        }

        if metrics.p999_latency_us > self.p999_threshold_us {
            alerts.push(format!(
                "P99.9 latency {} us exceeds threshold {} us",
                metrics.p999_latency_us, self.p999_threshold_us
            ));
        }

        if metrics.jitter_us > 10000 {
            alerts.push(format!("High jitter detected: {} us", metrics.jitter_us));
        }

        if self.jitter_detector.get_anomaly_count() > self.anomaly_threshold {
            alerts.push(format!(
                "Anomalies detected: {}",
                self.jitter_detector.get_anomaly_count()
            ));
        }

        alerts
    }

    /// 生成性能报告
    pub fn generate_report(&self) -> String {
        let metrics = self.analyzer.get_metrics();
        let anomaly_count = self.jitter_detector.get_anomaly_count();

        format!(
            "Realtime Performance Report:\n\
             =================================\n\
             Min Latency:     {} us\n\
             Avg Latency:     {:.2} us\n\
             P50 Latency:     {} us\n\
             P99 Latency:     {} us\n\
             P99.9 Latency:   {} us\n\
             Max Latency:     {} us\n\
             Jitter:          {} us\n\
             Throughput:      {:.2} Kops/s\n\
             Anomalies:       {}\n\
             Samples:         {}",
            metrics.min_latency_us,
            metrics.avg_latency_us,
            metrics.p50_latency_us,
            metrics.p99_latency_us,
            metrics.p999_latency_us,
            metrics.max_latency_us,
            metrics.jitter_us,
            metrics.throughput_kops,
            anomaly_count,
            self.analyzer.get_sample_count()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_analyzer() {
        let analyzer = LatencyAnalyzer::new(1000);

        for i in 0..100 {
            let latency = 100 + (i % 50) as u64;
            analyzer.record_sample(latency, i as u32);
        }

        let metrics = analyzer.get_metrics();
        assert!(metrics.min_latency_us > 0);
        assert!(metrics.max_latency_us > metrics.min_latency_us);
        assert!(metrics.avg_latency_us > 0.0);
        assert_eq!(analyzer.get_sample_count(), 100);
    }

    #[test]
    fn test_percentile_calculation() {
        let analyzer = LatencyAnalyzer::new(1000);

        // 模拟延迟分布：大部分在 100-150us，少数在 1000+us
        for i in 0..95 {
            analyzer.record_sample(100 + (i % 50) as u64, i as u32);
        }
        for i in 95..100 {
            analyzer.record_sample(1000 + (i * 100) as u64, i as u32);
        }

        let metrics = analyzer.get_metrics();
        assert!(metrics.p99_latency_us > metrics.p50_latency_us);
        assert!(metrics.p999_latency_us >= metrics.p99_latency_us);
    }

    #[test]
    fn test_jitter_detection() {
        let detector = JitterDetector::new(100, 50);

        // 添加足够的基线样本
        for _ in 0..20 {
            detector.detect(100);
        }

        // 添加多个高延迟样本来增加方差
        for _ in 0..10 {
            detector.detect(500); // 5倍基线延迟
        }

        // 验证至少检测到一些异常
        let count = detector.get_anomaly_count();
        assert!(count > 0, "Expected at least 1 anomaly, got {}", count);
    }

    #[test]
    fn test_predictive_alert() {
        let analyzer = Arc::new(LatencyAnalyzer::new(1000));
        let detector = Arc::new(JitterDetector::new(100, 50));
        let alert = PredictiveAlert::new(analyzer.clone(), detector.clone(), 200, 500);

        for i in 0..100 {
            analyzer.record_sample(250 + i, i as u32);
        }

        let alerts = alert.check_alerts();
        assert!(!alerts.is_empty());
    }

    #[test]
    fn test_performance_report() {
        let analyzer = Arc::new(LatencyAnalyzer::new(1000));
        let detector = Arc::new(JitterDetector::new(100, 50));
        let alert = PredictiveAlert::new(analyzer.clone(), detector.clone(), 200, 500);

        for i in 0..100 {
            analyzer.record_sample(100 + (i % 50) as u64, i as u32);
        }

        let report = alert.generate_report();
        assert!(report.contains("Min Latency"));
        assert!(report.contains("P99 Latency"));
        assert!(report.contains("Throughput"));
    }

    #[test]
    fn test_anomaly_tracking() {
        let detector = JitterDetector::new(100, 20);

        // 生成大量异常
        for i in 0..20 {
            detector.detect(100 + i * 100);
        }

        let anomalies = detector.get_anomalies();
        assert!(!anomalies.is_empty());
    }

    #[test]
    fn test_throughput_calculation() {
        let analyzer = LatencyAnalyzer::new(1000);

        // 生成 1000 个样本，延迟 100us
        for i in 0..1000 {
            analyzer.record_sample(100, i as u32);
        }

        let metrics = analyzer.get_metrics();
        // 100us 延迟 -> 10k ops/s = 10 Kops/s
        assert!(metrics.throughput_kops > 5.0 && metrics.throughput_kops < 15.0);
    }
}
