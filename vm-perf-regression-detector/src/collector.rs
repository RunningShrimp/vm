//! 性能指标收集器

use std::collections::HashMap;
use std::time::Instant;
use serde::{Deserialize, Serialize};
use vm_core::GuestArch;
use vm_cross_arch::UnifiedExecutor;
use chrono::Utc;
use anyhow;

/// 性能指标收集器
pub struct PerformanceCollector {
    /// 收集的指标
    metrics: HashMap<String, Vec<f64>>,
    /// 当前测试上下文
    context: TestContext,
    /// 开始时间
    start_time: Option<Instant>,
}

/// 测试上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestContext {
    /// 源架构
    pub src_arch: GuestArch,
    /// 目标架构
    pub dst_arch: GuestArch,
    /// 测试名称
    pub test_name: String,
    /// 测试版本
    pub version: String,
    /// 测试环境信息
    pub environment: EnvironmentInfo,
}

/// 环境信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentInfo {
    /// CPU核心数
    pub cpu_cores: usize,
    /// 内存大小（MB）
    pub memory_mb: usize,
    /// 操作系统
    pub os: String,
    /// 编译器版本
    pub rustc_version: String,
    /// 优化级别
    pub opt_level: String,
}

/// 性能指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// 测试上下文
    pub context: TestContext,
    /// 执行时间（微秒）
    pub execution_time_us: u64,
    /// JIT编译时间（微秒）
    pub jit_compilation_time_us: u64,
    /// 内存使用量（字节）
    pub memory_usage_bytes: u64,
    /// 翻译的指令数
    pub instructions_translated: usize,
    /// 指令吞吐量（指令/秒）
    pub instruction_throughput: f64,
    /// 缓存命中率
    pub cache_hit_rate: f64,
    /// 其他自定义指标
    pub custom_metrics: HashMap<String, f64>,
    /// 收集时间戳
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl PerformanceCollector {
    /// 创建新的性能收集器
    pub fn new(context: TestContext) -> Self {
        Self {
            metrics: HashMap::new(),
            context,
            start_time: None,
        }
    }

    /// 获取当前测试上下文
    pub fn get_context(&self) -> TestContext {
        self.context.clone()
    }

    /// 开始收集性能指标
    pub fn start_collection(&mut self) {
        self.start_time = Some(Instant::now());
    }

    /// 收集性能指标（不依赖执行器）
    pub fn collect_metrics(&mut self) -> anyhow::Result<PerformanceMetrics> {
        let start_time = self.start_time.ok_or_else(|| anyhow::anyhow!("Collection not started"))?;
        let elapsed = start_time.elapsed();

        // 将metrics的Vec<f64>转换为f64（取平均值）
        let mut custom_metrics: HashMap<String, f64> = HashMap::new();
        for (name, values) in &self.metrics {
            if !values.is_empty() {
                let avg = values.iter().sum::<f64>() / values.len() as f64;
                custom_metrics.insert(name.clone(), avg);
            }
        }

        let metrics = PerformanceMetrics {
            context: self.context.clone(),
            execution_time_us: elapsed.as_micros() as u64,
            jit_compilation_time_us: 0, // 模拟值
            memory_usage_bytes: 0, // 模拟值
            instructions_translated: 0, // 模拟值
            instruction_throughput: 0.0, // 模拟值
            cache_hit_rate: 0.0, // 模拟值
            custom_metrics,
            timestamp: Utc::now(),
        };

        Ok(metrics)
    }

    /// 记录指标值
    pub fn record_metric(&mut self, name: &str, value: f64) {
        let metrics = self.metrics.entry(name.to_string()).or_insert_with(Vec::new);
        metrics.push(value);
    }

    /// 从执行器收集性能指标
    pub fn collect_from_executor(&mut self, executor: &UnifiedExecutor) -> anyhow::Result<PerformanceMetrics> {
        let start_time = self.start_time.ok_or_else(|| anyhow::anyhow!("Collection not started"))?;
        let elapsed = start_time.elapsed();

        // 获取执行器统计信息
        let executor_stats = executor.stats();

        // 计算指令吞吐量
        let instruction_throughput = if elapsed.as_micros() > 0 {
            executor_stats.total_executions as f64 / (elapsed.as_micros() as f64 / 1_000_000.0)
        } else {
            0.0
        };

        // 将metrics的Vec<f64>转换为f64（取平均值）
        let mut custom_metrics: HashMap<String, f64> = HashMap::new();
        for (name, values) in &self.metrics {
            if !values.is_empty() {
                let avg = values.iter().sum::<f64>() / values.len() as f64;
                custom_metrics.insert(name.clone(), avg);
            }
        }

        // 获取内存使用量（从MMU获取物理内存大小）
        let memory_usage_bytes = executor.memory_size() as u64;

        let metrics = PerformanceMetrics {
            context: self.context.clone(),
            execution_time_us: elapsed.as_micros() as u64,
            jit_compilation_time_us: executor_stats.jit_compilation_time_us,
            memory_usage_bytes,
            instructions_translated: executor_stats.instructions_translated,
            instruction_throughput,
            cache_hit_rate: executor_stats.jit_hit_rate, // 使用JIT命中率作为缓存命中率
            custom_metrics,
            timestamp: Utc::now(),
        };

        Ok(metrics)
    }

    /// 收集系统环境信息
    pub fn collect_environment_info() -> EnvironmentInfo {
        EnvironmentInfo {
            cpu_cores: num_cpus::get(),
            memory_mb: get_memory_size_mb(),
            os: get_os_info(),
            rustc_version: get_rustc_version(),
            opt_level: get_opt_level(),
        }
    }
}

/// 获取内存大小（MB）
fn get_memory_size_mb() -> usize {
    // 简化实现，实际应该查询系统内存信息
    8192 // 假设8GB内存
}

/// 获取操作系统信息
fn get_os_info() -> String {
    std::env::consts::OS.to_string()
}

/// 获取Rust编译器版本
fn get_rustc_version() -> String {
    // 简化实现，实际应该调用rustc --version
    "1.70.0".to_string()
}

/// 获取优化级别
fn get_opt_level() -> String {
    if cfg!(debug_assertions) {
        "debug".to_string()
    } else {
        "release".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_core::GuestArch;

    #[test]
    fn test_performance_collector() {
        let context = TestContext {
            src_arch: GuestArch::X86_64,
            dst_arch: GuestArch::ARM64,
            test_name: "test_translation".to_string(),
            version: "1.0.0".to_string(),
            environment: PerformanceCollector::collect_environment_info(),
        };

        let mut collector = PerformanceCollector::new(context);
        collector.start_collection();
        collector.record_metric("custom_metric", 42.0);

        assert_eq!(collector.metrics.get("custom_metric").unwrap(), &vec![42.0]);
        assert!(collector.start_time.is_some());
    }

    #[test]
    fn test_environment_info() {
        let env = PerformanceCollector::collect_environment_info();
        
        assert!(env.cpu_cores > 0);
        assert!(env.memory_mb > 0);
        assert!(!env.os.is_empty());
        assert!(!env.rustc_version.is_empty());
        assert!(!env.opt_level.is_empty());
    }
}