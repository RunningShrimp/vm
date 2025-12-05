//! 性能基准测试框架
//!
//! 用于测量协程迁移前后的性能对比。
//! 包括：
//! - 上下文切换开销
//! - 吞吐量对比
//! - 内存使用量对比
//! - I/O性能对比

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use std::collections::HashMap;

/// 性能基准测试结果
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    /// 测试名称
    pub test_name: String,
    /// 执行次数
    pub iterations: u64,
    /// 总运行时间（毫秒）
    pub total_time_ms: u64,
    /// 平均每次运行时间（微秒）
    pub avg_time_us: f64,
    /// 最小运行时间（微秒）
    pub min_time_us: u64,
    /// 最大运行时间（微秒）
    pub max_time_us: u64,
    /// 标准差
    pub stddev_us: f64,
    /// 吞吐量（操作/秒）
    pub throughput_ops_per_sec: f64,
}

/// 性能基准测试套件
pub struct BenchmarkSuite {
    /// 测试名称
    name: String,
    /// 所有结果
    results: HashMap<String, BenchmarkResult>,
    /// 运行次数统计
    run_counts: HashMap<String, u64>,
}

impl BenchmarkSuite {
    /// 创建新的基准测试套件
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            results: HashMap::new(),
            run_counts: HashMap::new(),
        }
    }

    /// 运行一个基准测试
    ///
    /// # 参数
    /// - `test_name`: 测试名称
    /// - `iterations`: 迭代次数
    /// - `f`: 测试函数
    pub fn bench<F>(&mut self, test_name: &str, iterations: u64, mut f: F) -> BenchmarkResult
    where
        F: FnMut(),
    {
        let mut times = Vec::with_capacity(iterations as usize);
        let start = Instant::now();

        for _ in 0..iterations {
            let iter_start = Instant::now();
            f();
            times.push(iter_start.elapsed().as_micros() as u64);
        }

        let total_time = start.elapsed();
        let total_time_ms = total_time.as_millis() as u64;

        // 计算统计数据
        let min_time = *times.iter().min().unwrap_or(&0);
        let max_time = *times.iter().max().unwrap_or(&0);
        let avg_time = times.iter().sum::<u64>() as f64 / iterations as f64;
        
        // 计算标准差
        let variance = times.iter()
            .map(|&t| {
                let diff = t as f64 - avg_time;
                diff * diff
            })
            .sum::<f64>() / iterations as f64;
        let stddev = variance.sqrt();

        // 计算吞吐量
        let throughput = if total_time.as_secs_f64() > 0.0 {
            iterations as f64 / total_time.as_secs_f64()
        } else {
            0.0
        };

        let result = BenchmarkResult {
            test_name: test_name.to_string(),
            iterations,
            total_time_ms,
            avg_time_us: avg_time,
            min_time_us: min_time,
            max_time_us: max_time,
            stddev_us: stddev,
            throughput_ops_per_sec: throughput,
        };

        self.results.insert(test_name.to_string(), result.clone());
        self.run_counts.insert(test_name.to_string(), iterations);

        result
    }

    /// 打印所有结果
    pub fn print_results(&self) {
        println!("\n==== Benchmark Suite: {} ====", self.name);
        println!("{:<30} {:<15} {:<15} {:<15} {:<15}", 
                 "Test Name", "Avg (us)", "Min (us)", "Max (us)", "Throughput");
        println!("{}", "=".repeat(90));

        for (name, result) in &self.results {
            println!("{:<30} {:<15.2} {:<15} {:<15} {:<15.0}",
                     name,
                     result.avg_time_us,
                     result.min_time_us,
                     result.max_time_us,
                     result.throughput_ops_per_sec);
        }
    }

    /// 生成对比报告
    pub fn compare(&self, baseline: &BenchmarkSuite) -> ComparisonReport {
        let mut improvements = Vec::new();
        let mut regressions = Vec::new();

        for (test_name, current_result) in &self.results {
            if let Some(baseline_result) = baseline.results.get(test_name) {
                let improvement_percent = 
                    (baseline_result.avg_time_us - current_result.avg_time_us) 
                    / baseline_result.avg_time_us * 100.0;

                if improvement_percent > 0.1 {
                    improvements.push((test_name.clone(), improvement_percent));
                } else if improvement_percent < -0.1 {
                    regressions.push((test_name.clone(), -improvement_percent));
                }
            }
        }

        ComparisonReport {
            baseline_name: baseline.name.clone(),
            current_name: self.name.clone(),
            improvements,
            regressions,
        }
    }

    /// 导出为JSON格式
    pub fn to_json(&self) -> String {
        let mut json = format!(r#"{{"suite_name": "{}", "tests": [{{"#, self.name);
        
        let mut first = true;
        for (name, result) in &self.results {
            if !first {
                json.push_str(", ");
            }
            json.push_str(&format!(
                r#"{{"name": "{}", "iterations": {}, "avg_us": {:.2}, "min_us": {}, "max_us": {}, "stddev_us": {:.2}, "throughput": {:.0}}}"#,
                name, result.iterations, result.avg_time_us, result.min_time_us, 
                result.max_time_us, result.stddev_us, result.throughput_ops_per_sec
            ));
            first = false;
        }
        
        json.push_str("}]}");
        json
    }
}

/// 对比报告
#[derive(Debug)]
pub struct ComparisonReport {
    /// 基线套件名称
    pub baseline_name: String,
    /// 当前套件名称
    pub current_name: String,
    /// 性能改进（百分比）
    pub improvements: Vec<(String, f64)>,
    /// 性能回归（百分比）
    pub regressions: Vec<(String, f64)>,
}

impl ComparisonReport {
    /// 打印对比报告
    pub fn print(&self) {
        println!("\n==== Performance Comparison Report ====");
        println!("Baseline: {}", self.baseline_name);
        println!("Current: {}", self.current_name);
        
        if !self.improvements.is_empty() {
            println!("\n✓ Improvements:");
            for (test, percent) in &self.improvements {
                println!("  - {}: +{:.1}%", test, percent);
            }
        }

        if !self.regressions.is_empty() {
            println!("\n✗ Regressions:");
            for (test, percent) in &self.regressions {
                println!("  - {}: -{:.1}%", test, percent);
            }
        }

        if self.improvements.is_empty() && self.regressions.is_empty() {
            println!("\nNo significant changes detected.");
        }
    }

    /// 获取总体改进百分比
    pub fn overall_improvement_percent(&self) -> f64 {
        if self.improvements.is_empty() && self.regressions.is_empty() {
            0.0
        } else {
            let total_improvement: f64 = self.improvements.iter().map(|(_, p)| p).sum();
            let total_regression: f64 = self.regressions.iter().map(|(_, p)| p).sum();
            total_improvement - total_regression
        }
    }
}

/// 协程性能计数器
pub struct CoroutinePerformanceCounter {
    /// 上下文切换次数
    pub context_switches: Arc<AtomicU64>,
    /// 执行的指令总数
    pub instructions_executed: Arc<AtomicU64>,
    /// 阻塞次数
    pub blocks_count: Arc<AtomicU64>,
    /// 总运行时间（纳秒）
    pub total_runtime_ns: Arc<AtomicU64>,
}

impl CoroutinePerformanceCounter {
    /// 创建新的性能计数器
    pub fn new() -> Self {
        Self {
            context_switches: Arc::new(AtomicU64::new(0)),
            instructions_executed: Arc::new(AtomicU64::new(0)),
            blocks_count: Arc::new(AtomicU64::new(0)),
            total_runtime_ns: Arc::new(AtomicU64::new(0)),
        }
    }

    /// 增加上下文切换计数
    pub fn inc_context_switches(&self) {
        self.context_switches.fetch_add(1, Ordering::Relaxed);
    }

    /// 增加执行指令计数
    pub fn add_instructions(&self, count: u64) {
        self.instructions_executed.fetch_add(count, Ordering::Relaxed);
    }

    /// 增加阻塞计数
    pub fn inc_blocks(&self) {
        self.blocks_count.fetch_add(1, Ordering::Relaxed);
    }

    /// 添加运行时间
    pub fn add_runtime_ns(&self, ns: u64) {
        self.total_runtime_ns.fetch_add(ns, Ordering::Relaxed);
    }

    /// 获取统计摘要
    pub fn get_summary(&self) -> PerformanceSummary {
        let context_switches = self.context_switches.load(Ordering::Relaxed);
        let instructions = self.instructions_executed.load(Ordering::Relaxed);
        let blocks = self.blocks_count.load(Ordering::Relaxed);
        let runtime_ns = self.total_runtime_ns.load(Ordering::Relaxed);

        PerformanceSummary {
            context_switches,
            instructions_executed: instructions,
            blocks_count: blocks,
            total_runtime_ms: runtime_ns / 1_000_000,
            avg_instructions_per_block: if blocks > 0 { instructions / blocks } else { 0 },
            instructions_per_ms: if runtime_ns > 0 { (instructions as f64 * 1_000_000.0) / runtime_ns as f64 } else { 0.0 },
        }
    }

    /// 重置所有计数
    pub fn reset(&self) {
        self.context_switches.store(0, Ordering::Relaxed);
        self.instructions_executed.store(0, Ordering::Relaxed);
        self.blocks_count.store(0, Ordering::Relaxed);
        self.total_runtime_ns.store(0, Ordering::Relaxed);
    }
}

/// 性能摘要
#[derive(Debug, Clone)]
pub struct PerformanceSummary {
    /// 上下文切换次数
    pub context_switches: u64,
    /// 执行的指令总数
    pub instructions_executed: u64,
    /// 块数量
    pub blocks_count: u64,
    /// 总运行时间（毫秒）
    pub total_runtime_ms: u64,
    /// 平均每块指令数
    pub avg_instructions_per_block: u64,
    /// 每毫秒指令数
    pub instructions_per_ms: f64,
}

impl PerformanceSummary {
    /// 打印摘要
    pub fn print(&self) {
        println!("\n==== Performance Summary ====");
        println!("Context Switches: {}", self.context_switches);
        println!("Instructions Executed: {}", self.instructions_executed);
        println!("Blocks Count: {}", self.blocks_count);
        println!("Total Runtime: {} ms", self.total_runtime_ms);
        println!("Avg Instructions/Block: {}", self.avg_instructions_per_block);
        println!("Instructions/ms: {:.2}", self.instructions_per_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_benchmark_result() {
        let mut suite = BenchmarkSuite::new("Test Suite");
        
        let result = suite.bench("dummy_test", 100, || {
            // 简单的测试操作
            let _ = 1 + 1;
        });

        assert_eq!(result.iterations, 100);
        assert!(result.avg_time_us > 0.0);
    }

    #[test]
    fn test_performance_counter() {
        let counter = CoroutinePerformanceCounter::new();
        counter.inc_context_switches();
        counter.add_instructions(1000);
        counter.inc_blocks();

        let summary = counter.get_summary();
        assert_eq!(summary.context_switches, 1);
        assert_eq!(summary.instructions_executed, 1000);
        assert_eq!(summary.blocks_count, 1);
    }
}
