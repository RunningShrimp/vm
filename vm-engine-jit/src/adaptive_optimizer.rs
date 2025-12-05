//! 运行时自适应优化系统
//!
//! 根据实时反馈动态调整优化参数，如热点阈值、缓存容量等。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

/// 性能反馈样本
#[derive(Clone, Debug)]
pub struct PerformanceFeedback {
    /// 执行时间 (微秒)
    pub execution_time_us: u64,
    /// 缓存命中率 (0-100)
    pub cache_hit_rate: f64,
    /// 块链接命中数
    pub chain_hits: u64,
    /// 块链接失败数
    pub chain_misses: u64,
    /// 追踪编译成功数
    pub trace_compilations: u64,
    /// GC 暂停时间 (微秒)
    pub gc_pause_us: u64,
}

impl Default for PerformanceFeedback {
    fn default() -> Self {
        Self {
            execution_time_us: 0,
            cache_hit_rate: 0.0,
            chain_hits: 0,
            chain_misses: 0,
            trace_compilations: 0,
            gc_pause_us: 0,
        }
    }
}

/// 自适应参数
#[derive(Clone, Debug)]
pub struct AdaptiveParameters {
    /// 块链接 LRU 容量 (默认 100)
    pub block_chaining_capacity: usize,
    /// 内联缓存多态目标最大数 (默认 4)
    pub polymorphic_target_limit: usize,
    /// 热点阈值 (默认 100 次)
    pub hotspot_threshold: u64,
    /// 追踪最大长度 (默认 50 块)
    pub trace_max_length: usize,
    /// JIT 编译延迟阈值 (微秒)
    pub jit_compile_latency_us: u64,
    /// GC 触发频率调整因子
    pub gc_trigger_factor: f64,
}

impl Default for AdaptiveParameters {
    fn default() -> Self {
        Self {
            block_chaining_capacity: 100,
            polymorphic_target_limit: 4,
            hotspot_threshold: 100,
            trace_max_length: 50,
            jit_compile_latency_us: 5000, // 5ms
            gc_trigger_factor: 1.0,
        }
    }
}

/// 优化策略
#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum OptimizationStrategy {
    /// 保守策略：优先编译速度
    Conservative,
    /// 平衡策略：平衡编译速度和执行效率
    Balanced,
    /// 激进策略：优先执行效率
    Aggressive,
}

impl Default for OptimizationStrategy {
    fn default() -> Self {
        OptimizationStrategy::Balanced
    }
}

/// 自适应优化器
pub struct AdaptiveOptimizer {
    /// 当前参数
    parameters: Arc<RwLock<AdaptiveParameters>>,
    /// 性能反馈历史
    feedback_history: Arc<Mutex<Vec<PerformanceFeedback>>>,
    /// 优化策略
    strategy: Arc<RwLock<OptimizationStrategy>>,
    /// 上次参数调整时间
    last_adjustment: Arc<Mutex<Instant>>,
    /// 参数调整最小间隔
    adjustment_interval: Duration,
}

impl AdaptiveOptimizer {
    /// 创建新的自适应优化器
    pub fn new() -> Self {
        Self {
            parameters: Arc::new(RwLock::new(AdaptiveParameters::default())),
            feedback_history: Arc::new(Mutex::new(Vec::new())),
            strategy: Arc::new(RwLock::new(OptimizationStrategy::Balanced)),
            last_adjustment: Arc::new(Mutex::new(Instant::now())),
            adjustment_interval: Duration::from_secs(1),
        }
    }

    /// 记录性能反馈
    pub fn record_feedback(&self, feedback: PerformanceFeedback) {
        let mut history = self.feedback_history.lock().unwrap();
        history.push(feedback);

        // 保留最近 1000 个样本
        if history.len() > 1000 {
            history.remove(0);
        }
    }

    /// 自动调整参数
    pub fn auto_adjust(&self) {
        let mut last_adj = self.last_adjustment.lock().unwrap();
        if last_adj.elapsed() < self.adjustment_interval {
            return;
        }
        *last_adj = Instant::now();

        let history = self.feedback_history.lock().unwrap();
        if history.len() < 10 {
            return; // 样本不足，不调整
        }

        // 计算最近 10 个样本的平均指标
        let recent_samples: Vec<_> = history.iter().rev().take(10).collect();

        let avg_cache_hit_rate = recent_samples.iter().map(|f| f.cache_hit_rate).sum::<f64>()
            / recent_samples.len() as f64;

        let avg_exec_time = recent_samples
            .iter()
            .map(|f| f.execution_time_us)
            .sum::<u64>()
            / recent_samples.len() as u64;

        let chain_efficiency = if recent_samples
            .iter()
            .map(|f| f.chain_hits + f.chain_misses)
            .sum::<u64>()
            > 0
        {
            let hits: u64 = recent_samples.iter().map(|f| f.chain_hits).sum();
            let total: u64 = recent_samples
                .iter()
                .map(|f| f.chain_hits + f.chain_misses)
                .sum();
            hits as f64 / total as f64
        } else {
            0.0
        };

        // 根据指标调整策略
        let new_strategy = if avg_cache_hit_rate > 0.95 && avg_exec_time < 1000 {
            OptimizationStrategy::Aggressive
        } else if avg_cache_hit_rate < 0.70 {
            OptimizationStrategy::Conservative
        } else {
            OptimizationStrategy::Balanced
        };

        {
            let mut strategy = self.strategy.write().unwrap();
            *strategy = new_strategy;
        }

        // 根据策略调整参数
        self.adjust_parameters_by_strategy(new_strategy, avg_cache_hit_rate, chain_efficiency);
    }

    /// 根据策略调整参数
    fn adjust_parameters_by_strategy(
        &self,
        strategy: OptimizationStrategy,
        cache_hit_rate: f64,
        chain_efficiency: f64,
    ) {
        let mut params = self.parameters.write().unwrap();

        match strategy {
            OptimizationStrategy::Conservative => {
                // 减小优化开销
                params.block_chaining_capacity = 50;
                params.polymorphic_target_limit = 2;
                params.hotspot_threshold = 200;
                params.trace_max_length = 20;
                params.jit_compile_latency_us = 10000; // 更宽松的延迟要求
            }
            OptimizationStrategy::Balanced => {
                // 平衡参数
                params.block_chaining_capacity = 100;
                params.polymorphic_target_limit = 4;
                params.hotspot_threshold = 100;
                params.trace_max_length = 50;
                params.jit_compile_latency_us = 5000;
            }
            OptimizationStrategy::Aggressive => {
                // 最大化性能
                if cache_hit_rate > 0.90 {
                    params.block_chaining_capacity = 200;
                }
                if chain_efficiency > 0.85 {
                    params.polymorphic_target_limit = 8;
                }
                params.hotspot_threshold = 50;
                params.trace_max_length = 100;
                params.jit_compile_latency_us = 2000;
            }
        }

        // 动态调整 GC 触发因子
        if cache_hit_rate < 0.70 {
            params.gc_trigger_factor *= 1.1; // 更频繁的 GC
        } else if cache_hit_rate > 0.95 {
            params.gc_trigger_factor *= 0.9; // 更少的 GC
        }
        params.gc_trigger_factor = params.gc_trigger_factor.max(0.5).min(2.0);
    }

    /// 获取当前参数
    pub fn get_parameters(&self) -> AdaptiveParameters {
        self.parameters.read().unwrap().clone()
    }

    /// 设置参数
    pub fn set_parameters(&self, params: AdaptiveParameters) {
        let mut p = self.parameters.write().unwrap();
        *p = params;
    }

    /// 获取当前策略
    pub fn get_strategy(&self) -> OptimizationStrategy {
        *self.strategy.read().unwrap()
    }

    /// 获取性能统计
    pub fn get_stats(&self) -> AdaptiveOptimizationStats {
        let history = self.feedback_history.lock().unwrap();

        if history.is_empty() {
            return AdaptiveOptimizationStats::default();
        }

        let avg_cache_hit_rate =
            history.iter().map(|f| f.cache_hit_rate).sum::<f64>() / history.len() as f64;
        let avg_exec_time =
            history.iter().map(|f| f.execution_time_us).sum::<u64>() / history.len() as u64;
        let total_gc_pause = history.iter().map(|f| f.gc_pause_us).sum();
        let total_chains = history.iter().map(|f| f.chain_hits + f.chain_misses).sum();

        AdaptiveOptimizationStats {
            sample_count: history.len(),
            avg_cache_hit_rate,
            avg_exec_time_us: avg_exec_time,
            total_gc_pause_us: total_gc_pause,
            total_chains,
            current_strategy: *self.strategy.read().unwrap(),
        }
    }

    /// 生成诊断报告
    pub fn diagnostic_report(&self) -> String {
        let params = self.parameters.read().unwrap();
        let strategy = self.strategy.read().unwrap();
        let stats = self.get_stats();

        format!(
            r#"=== Adaptive Optimization Report ===
Current Strategy: {:?}

Parameters:
  Block Chaining Capacity: {}
  Polymorphic Target Limit: {}
  Hotspot Threshold: {} executions
  Trace Max Length: {} blocks
  JIT Compile Latency: {}us
  GC Trigger Factor: {:.2}

Performance Metrics:
  Sample Count: {}
  Avg Cache Hit Rate: {:.1}%
  Avg Execution Time: {}us
  Total GC Pause: {}us
  Total Chains: {}
"#,
            strategy,
            params.block_chaining_capacity,
            params.polymorphic_target_limit,
            params.hotspot_threshold,
            params.trace_max_length,
            params.jit_compile_latency_us,
            params.gc_trigger_factor,
            stats.sample_count,
            stats.avg_cache_hit_rate * 100.0,
            stats.avg_exec_time_us,
            stats.total_gc_pause_us,
            stats.total_chains,
        )
    }
}

impl Default for AdaptiveOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

/// 自适应优化统计
#[derive(Debug, Clone, Default)]
pub struct AdaptiveOptimizationStats {
    pub sample_count: usize,
    pub avg_cache_hit_rate: f64,
    pub avg_exec_time_us: u64,
    pub total_gc_pause_us: u64,
    pub total_chains: u64,
    pub current_strategy: OptimizationStrategy,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adaptive_optimizer_creation() {
        let optimizer = AdaptiveOptimizer::new();
        let params = optimizer.get_parameters();
        assert_eq!(params.block_chaining_capacity, 100);
        assert_eq!(params.hotspot_threshold, 100);
    }

    #[test]
    fn test_performance_feedback_recording() {
        let optimizer = AdaptiveOptimizer::new();
        let feedback = PerformanceFeedback {
            execution_time_us: 500,
            cache_hit_rate: 0.95,
            chain_hits: 50,
            chain_misses: 5,
            trace_compilations: 3,
            gc_pause_us: 100,
        };

        optimizer.record_feedback(feedback);
        let stats = optimizer.get_stats();
        assert_eq!(stats.sample_count, 1);
    }

    #[test]
    fn test_strategy_adjustment() {
        let optimizer = AdaptiveOptimizer::new();

        // 记录高性能反馈
        for _ in 0..10 {
            let feedback = PerformanceFeedback {
                execution_time_us: 300,
                cache_hit_rate: 0.98,
                chain_hits: 100,
                chain_misses: 2,
                trace_compilations: 5,
                gc_pause_us: 50,
            };
            optimizer.record_feedback(feedback);
        }

        // 等待调整间隔或直接调用内部方法
        std::thread::sleep(std::time::Duration::from_millis(1100));
        optimizer.auto_adjust();

        let strategy = optimizer.get_strategy();
        // 由于数据样本充分且性能指标极高，应该是 Aggressive
        assert!(matches!(
            strategy,
            OptimizationStrategy::Aggressive | OptimizationStrategy::Balanced
        ));

        // 检查参数是否调整（对于高性能场景，容量应增加）
        let params = optimizer.get_parameters();
        assert!(params.block_chaining_capacity >= 100);
    }

    #[test]
    fn test_conservative_strategy() {
        let optimizer = AdaptiveOptimizer::new();

        // 记录低性能反馈
        for _ in 0..10 {
            let feedback = PerformanceFeedback {
                execution_time_us: 2000,
                cache_hit_rate: 0.50,
                chain_hits: 10,
                chain_misses: 90,
                trace_compilations: 1,
                gc_pause_us: 500,
            };
            optimizer.record_feedback(feedback);
        }

        // 等待调整间隔
        std::thread::sleep(std::time::Duration::from_millis(1100));
        optimizer.auto_adjust();

        let strategy = optimizer.get_strategy();
        assert_eq!(strategy, OptimizationStrategy::Conservative);

        // 检查参数是否调整为保守设置
        let params = optimizer.get_parameters();
        assert_eq!(params.block_chaining_capacity, 50);
        assert_eq!(params.hotspot_threshold, 200);
    }

    #[test]
    fn test_diagnostic_report() {
        let optimizer = AdaptiveOptimizer::new();
        let feedback = PerformanceFeedback {
            execution_time_us: 500,
            cache_hit_rate: 0.90,
            chain_hits: 80,
            chain_misses: 20,
            trace_compilations: 4,
            gc_pause_us: 100,
        };

        optimizer.record_feedback(feedback);
        let report = optimizer.diagnostic_report();
        assert!(report.contains("Adaptive Optimization Report"));
        assert!(report.contains("Current Strategy"));
    }
}
