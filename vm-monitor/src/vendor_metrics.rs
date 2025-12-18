//! 厂商扩展性能监控
//!
//! 监控厂商扩展指令的使用率和性能指标

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// 厂商扩展使用统计
#[derive(Debug, Clone)]
pub struct VendorExtensionMetrics {
    /// 扩展名称
    pub extension_name: String,
    /// 指令执行次数
    pub instruction_count: u64,
    /// 总执行时间
    pub total_time: Duration,
    /// 平均执行时间
    pub avg_time: Duration,
    /// 最大执行时间
    pub max_time: Duration,
    /// 最小执行时间
    pub min_time: Duration,
}

impl VendorExtensionMetrics {
    pub fn new(extension_name: String) -> Self {
        Self {
            extension_name,
            instruction_count: 0,
            total_time: Duration::ZERO,
            avg_time: Duration::ZERO,
            max_time: Duration::ZERO,
            min_time: Duration::MAX,
        }
    }

    pub fn record_execution(&mut self, duration: Duration) {
        self.instruction_count += 1;
        self.total_time += duration;
        self.avg_time = self.total_time / self.instruction_count as u32;
        if duration > self.max_time {
            self.max_time = duration;
        }
        if duration < self.min_time {
            self.min_time = duration;
        }
    }

    pub fn usage_rate(&self, total_instructions: u64) -> f64 {
        if total_instructions == 0 {
            0.0
        } else {
            (self.instruction_count as f64 / total_instructions as f64) * 100.0
        }
    }
}

/// 性能监控器
pub struct VendorMetricsCollector {
    metrics: Arc<RwLock<HashMap<String, VendorExtensionMetrics>>>,
    total_instructions: Arc<RwLock<u64>>,
}

impl VendorMetricsCollector {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
            total_instructions: Arc::new(RwLock::new(0)),
        }
    }

    /// 记录指令执行
    pub fn record_instruction(&self, extension_name: &str, duration: Duration) {
        let mut metrics = self.metrics.write().unwrap();
        let entry = metrics
            .entry(extension_name.to_string())
            .or_insert_with(|| VendorExtensionMetrics::new(extension_name.to_string()));
        entry.record_execution(duration);

        *self.total_instructions.write().unwrap() += 1;
    }

    /// 获取扩展指标
    pub fn get_metrics(&self, extension_name: &str) -> Option<VendorExtensionMetrics> {
        let metrics = self.metrics.read().unwrap();
        metrics.get(extension_name).cloned()
    }

    /// 获取所有指标
    pub fn get_all_metrics(&self) -> HashMap<String, VendorExtensionMetrics> {
        let metrics = self.metrics.read().unwrap();
        metrics.clone()
    }

    /// 获取总指令数
    pub fn total_instructions(&self) -> u64 {
        *self.total_instructions.read().unwrap()
    }

    /// 重置统计
    pub fn reset(&self) {
        let mut metrics = self.metrics.write().unwrap();
        metrics.clear();
        *self.total_instructions.write().unwrap() = 0;
    }

    /// 生成性能报告
    pub fn generate_report(&self) -> String {
        let metrics = self.metrics.read().unwrap();
        let total = *self.total_instructions.read().unwrap();

        let mut report = String::from("=== 厂商扩展性能报告 ===\n");
        report.push_str(&format!("总指令数: {}\n\n", total));

        for (name, metric) in metrics.iter() {
            report.push_str(&format!("扩展: {}\n", name));
            report.push_str(&format!("  执行次数: {}\n", metric.instruction_count));
            report.push_str(&format!("  使用率: {:.2}%\n", metric.usage_rate(total)));
            report.push_str(&format!("  平均时间: {:?}\n", metric.avg_time));
            report.push_str(&format!("  最大时间: {:?}\n", metric.max_time));
            report.push_str(&format!("  最小时间: {:?}\n", metric.min_time));
            report.push('\n');
        }

        report
    }
}

impl Default for VendorMetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// 性能分析工具
pub struct PerformanceAnalyzer {
    collector: Arc<VendorMetricsCollector>,
}

impl PerformanceAnalyzer {
    pub fn new(collector: Arc<VendorMetricsCollector>) -> Self {
        Self { collector }
    }

    /// 分析扩展使用情况
    pub fn analyze_usage(&self) -> Vec<(String, f64)> {
        let metrics = self.collector.get_all_metrics();
        let total = self.collector.total_instructions();

        let mut usage: Vec<(String, f64)> = metrics
            .iter()
            .map(|(name, metric)| (name.clone(), metric.usage_rate(total)))
            .collect();

        usage.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        usage
    }

    /// 识别性能瓶颈
    pub fn identify_bottlenecks(&self) -> Vec<String> {
        let metrics = self.collector.get_all_metrics();
        let mut bottlenecks = Vec::new();

        for (name, metric) in metrics.iter() {
            // 如果平均执行时间超过1ms，认为是瓶颈
            if metric.avg_time > Duration::from_millis(1) {
                bottlenecks.push(format!("{}: 平均执行时间 {:?}", name, metric.avg_time));
            }
        }

        bottlenecks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_collector() {
        let collector = VendorMetricsCollector::new();

        collector.record_instruction("Apple AMX", Duration::from_micros(100));
        collector.record_instruction("Apple AMX", Duration::from_micros(150));

        let metrics = collector.get_metrics("Apple AMX").unwrap();
        assert_eq!(metrics.instruction_count, 2);
        assert_eq!(metrics.avg_time, Duration::from_micros(125));
    }
}
