//! 自适应编译阈值调整
//!
//! 本模块实现智能的自适应编译阈值调整机制，根据运行时性能数据
//! 动态调整JIT编译的阈值，优化编译时间和执行性能的平衡。

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use vm_core::{GuestAddr, VmError};
use serde::{Serialize, Deserialize};
use serde_with::serde_as;

/// 自适应编译配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveCompilationConfig {
    /// 基础编译阈值
    pub base_compilation_threshold: u64,
    /// 最小编译阈值
    pub min_compilation_threshold: u64,
    /// 最大编译阈值
    pub max_compilation_threshold: u64,
    /// 调整窗口大小
    pub adjustment_window: Duration,
    /// 性能目标权重
    pub performance_weight: f64,
    /// 编译时间权重
    pub compilation_time_weight: f64,
    /// 内存使用权重
    pub memory_weight: f64,
    /// 调整步长
    pub adjustment_step: f64,
    /// 启用激进调整
    pub enable_aggressive_adjustment: bool,
    /// 历史数据保留数量
    pub history_size: usize,
}

impl Default for AdaptiveCompilationConfig {
    fn default() -> Self {
        Self {
            base_compilation_threshold: 100,
            min_compilation_threshold: 10,
            max_compilation_threshold: 1000,
            adjustment_window: Duration::from_secs(30),
            performance_weight: 0.5,
            compilation_time_weight: 0.3,
            memory_weight: 0.2,
            adjustment_step: 0.1,
            enable_aggressive_adjustment: false,
            history_size: 100,
        }
    }
}

/// 性能指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// 平均执行时间
    pub average_execution_time: Duration,
    /// 编译时间
    pub compilation_time: Duration,
    /// 内存使用量
    pub memory_usage: u64,
    /// 缓存命中率
    pub cache_hit_rate: f64,
    /// 指令执行速度
    pub execution_speed: f64,
    /// 编译收益
    pub compilation_benefit: f64,
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            average_execution_time: Duration::from_micros(100),
            compilation_time: Duration::from_micros(1000),
            memory_usage: 1024 * 1024, // 1MB
            cache_hit_rate: 0.8,
            execution_speed: 1000.0, // IPS
            compilation_benefit: 1.0,
        }
    }
}

/// 阈值调整历史
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdAdjustmentHistory {
    /// 调整时间（毫秒）
    pub timestamp: Duration,
    /// 旧阈值
    pub old_threshold: u64,
    /// 新阈值
    pub new_threshold: u64,
    /// 调整原因
    pub reason: String,
    /// 性能指标
    pub metrics: PerformanceMetrics,
    /// 调整评分
    pub adjustment_score: f64,
}

/// 自适应编译阈值管理器
pub struct AdaptiveThresholdManager {
    /// 配置
    config: AdaptiveCompilationConfig,
    /// 当前编译阈值
    current_threshold: Arc<Mutex<u64>>,
    /// 性能指标历史
    metrics_history: Arc<Mutex<VecDeque<PerformanceMetrics>>>,
    /// 阈值调整历史
    adjustment_history: Arc<Mutex<VecDeque<ThresholdAdjustmentHistory>>>,
    /// 最后调整时间
    last_adjustment: Arc<Mutex<Instant>>,
    /// 性能基线
    performance_baseline: Arc<Mutex<PerformanceMetrics>>,
    /// 代码块执行统计
    block_stats: Arc<Mutex<HashMap<GuestAddr, BlockExecutionStats>>>,
}

/// 代码块执行统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockExecutionStats {
    /// 执行次数
    pub execution_count: u64,
    /// 总执行时间
    pub total_execution_time: Duration,
    /// 是否已编译
    pub is_compiled: bool,
    /// 编译时间
    pub compilation_time: Duration,
    /// 最后执行时间（毫秒）
    pub last_execution: Duration,
}

impl Default for BlockExecutionStats {
    fn default() -> Self {
        Self {
            execution_count: 0,
            total_execution_time: Duration::ZERO,
            is_compiled: false,
            compilation_time: Duration::ZERO,
            last_execution: Duration::from_millis(Instant::now().elapsed().as_millis() as u64),
        }
    }
}

impl AdaptiveThresholdManager {
    /// 创建新的自适应阈值管理器
    pub fn new(config: AdaptiveCompilationConfig) -> Self {
        Self {
            current_threshold: Arc::new(Mutex::new(config.base_compilation_threshold)),
            metrics_history: Arc::new(Mutex::new(VecDeque::new())),
            adjustment_history: Arc::new(Mutex::new(VecDeque::new())),
            last_adjustment: Arc::new(Mutex::new(Instant::now())),
            performance_baseline: Arc::new(Mutex::new(PerformanceMetrics::default())),
            block_stats: Arc::new(Mutex::new(HashMap::new())),
            config,
        }
    }

    /// Helper function to safely acquire a mutex lock
    fn lock_mutex<T>(mutex: &Mutex<T>) -> Result<parking_lot::MutexGuard<T>, VmError> {
        mutex.lock().map_err(|e| VmError::Execution(vm_core::ExecutionError::JitError {
            message: format!("Mutex lock poisoned: {}", e),
            function_addr: None,
        }))
    }

    /// 记录代码块执行
    pub fn record_block_execution(&self, addr: GuestAddr, execution_time: Duration) -> Result<(), VmError> {
        let now = Instant::now();

        {
            let mut stats = Self::lock_mutex(&self.block_stats)?;
            let entry = stats.entry(addr).or_default();

            entry.execution_count += 1;
            entry.total_execution_time += execution_time;
            entry.last_execution = now.elapsed();
        }

        // 检查是否需要调整阈值
        self.check_and_adjust_threshold()?;

        Ok(())
    }

    /// 记录代码块编译
    pub fn record_block_compilation(&self, addr: GuestAddr, compilation_time: Duration) -> Result<(), VmError> {
        {
            let mut stats = Self::lock_mutex(&self.block_stats)?;
            let entry = stats.entry(addr).or_default();

            entry.is_compiled = true;
            entry.compilation_time = compilation_time;
        }

        // 检查是否需要调整阈值
        self.check_and_adjust_threshold()?;

        Ok(())
    }

    /// 检查并调整阈值
    fn check_and_adjust_threshold(&self) -> Result<(), VmError> {
        let now = Instant::now();
        let last_adjustment = *Self::lock_mutex(&self.last_adjustment)?;

        // 如果距离上次调整时间不足调整窗口，跳过调整
        if now.duration_since(last_adjustment) < self.config.adjustment_window {
            return Ok(());
        }

        // 计算当前性能指标
        let current_metrics = self.calculate_current_metrics()?;

        // 保存性能指标到历史
        {
            let mut history = Self::lock_mutex(&self.metrics_history)?;
            history.push_back(current_metrics.clone());

            // 保持历史记录在配置的大小范围内
            while history.len() > self.config.history_size {
                history.pop_front();
            }
        }

        // 计算调整建议
        let adjustment_suggestion = self.calculate_adjustment_suggestion(&current_metrics)?;

        // 应用调整
        if adjustment_suggestion.should_adjust {
            self.apply_threshold_adjustment(adjustment_suggestion, current_metrics)?;
        }

        Ok(())
    }

    /// 计算当前性能指标
    fn calculate_current_metrics(&self) -> Result<PerformanceMetrics, VmError> {
        let stats = Self::lock_mutex(&self.block_stats)?;

        if stats.is_empty() {
            return Ok(PerformanceMetrics::default());
        }

        let mut total_execution_time = Duration::ZERO;
        let mut total_execution_count = 0u64;
        let mut total_compilation_time = Duration::ZERO;
        let mut compiled_blocks = 0u64;
        let total_blocks = stats.len() as u64;

        for (_, block_stat) in stats.iter() {
            total_execution_time += block_stat.total_execution_time;
            total_execution_count += block_stat.execution_count;

            if block_stat.is_compiled {
                total_compilation_time += block_stat.compilation_time;
                compiled_blocks += 1;
            }
        }

        let average_execution_time = if total_execution_count > 0 {
            total_execution_time / total_execution_count as u32
        } else {
            Duration::ZERO
        };

        let cache_hit_rate = if total_blocks > 0 {
            compiled_blocks as f64 / total_blocks as f64
        } else {
            0.0
        };

        let execution_speed = if average_execution_time.as_micros() > 0 {
            1_000_000.0 / average_execution_time.as_micros() as f64
        } else {
            0.0
        };

        // 计算编译收益（简化计算）
        let compilation_benefit = if total_compilation_time.as_micros() > 0 {
            (total_execution_time.as_micros() as f64 / total_compilation_time.as_micros() as f64) * cache_hit_rate
        } else {
            1.0
        };

        Ok(PerformanceMetrics {
            average_execution_time,
            compilation_time: total_compilation_time,
            memory_usage: self.estimate_memory_usage(),
            cache_hit_rate,
            execution_speed,
            compilation_benefit,
        })
    }

    /// 估算内存使用量
    fn estimate_memory_usage(&self) -> u64 {
        match Self::lock_mutex(&self.block_stats) {
            Ok(stats) => {
                let compiled_blocks = stats.values()
                    .filter(|s| s.is_compiled)
                    .count();

                // 假设每个编译后的代码块占用10KB内存
                (compiled_blocks * 10 * 1024) as u64
            }
            Err(_) => {
                // If lock is poisoned, assume 0 memory usage
                0
            }
        }
    }

    /// 计算调整建议
    fn calculate_adjustment_suggestion(&self, metrics: &PerformanceMetrics) -> Result<ThresholdAdjustmentSuggestion, VmError> {
        let baseline = Self::lock_mutex(&self.performance_baseline)?;
        let current_threshold = *Self::lock_mutex(&self.current_threshold)?;

        // 计算性能评分
        let performance_score = self.calculate_performance_score(metrics, &baseline);

        // 计算调整方向和幅度
        let (adjustment_direction, adjustment_factor) = if performance_score > 1.1 {
            // 性能良好，可以降低阈值以编译更多代码
            (AdjustmentDirection::Decrease, self.config.adjustment_step)
        } else if performance_score < 0.9 {
            // 性能不佳，提高阈值以减少编译开销
            (AdjustmentDirection::Increase, self.config.adjustment_step)
        } else {
            // 性能正常，不调整
            (AdjustmentDirection::None, 0.0)
        };

        // 计算建议的新阈值
        let suggested_threshold = match adjustment_direction {
            AdjustmentDirection::Increase => {
                let new_threshold = (current_threshold as f64 * (1.0 + adjustment_factor)) as u64;
                new_threshold.min(self.config.max_compilation_threshold)
            }
            AdjustmentDirection::Decrease => {
                let new_threshold = (current_threshold as f64 * (1.0 - adjustment_factor)) as u64;
                new_threshold.max(self.config.min_compilation_threshold)
            }
            AdjustmentDirection::None => current_threshold,
        };

        let should_adjust = suggested_threshold != current_threshold;
        let reason = if should_adjust {
            match adjustment_direction {
                AdjustmentDirection::Increase => {
                    format!("性能评分 {:.2} 低于目标，提高编译阈值", performance_score)
                }
                AdjustmentDirection::Decrease => {
                    format!("性能评分 {:.2} 高于目标，降低编译阈值", performance_score)
                }
                AdjustmentDirection::None => "无需调整".to_string(),
            }
        } else {
            "性能正常，无需调整".to_string()
        };

        Ok(ThresholdAdjustmentSuggestion {
            should_adjust,
            suggested_threshold,
            adjustment_direction,
            adjustment_factor,
            reason,
            performance_score,
        })
    }

    /// 计算性能评分
    fn calculate_performance_score(&self, metrics: &PerformanceMetrics, baseline: &PerformanceMetrics) -> f64 {
        // 执行时间评分（时间越短分数越高）
        let execution_time_score = if baseline.average_execution_time.as_micros() > 0 {
            baseline.average_execution_time.as_micros() as f64 / metrics.average_execution_time.as_micros() as f64
        } else {
            1.0
        };
        
        // 编译时间评分（时间越短分数越高）
        let compilation_time_score = if baseline.compilation_time.as_micros() > 0 {
            baseline.compilation_time.as_micros() as f64 / metrics.compilation_time.as_micros() as f64
        } else {
            1.0
        };
        
        // 内存使用评分（使用越少分数越高）
        let memory_score = if baseline.memory_usage > 0 {
            baseline.memory_usage as f64 / metrics.memory_usage as f64
        } else {
            1.0
        };
        
        // 缓存命中率评分（命中率越高分数越高）
        let cache_hit_score = metrics.cache_hit_rate;
        
        // 综合评分
        let overall_score = execution_time_score * self.config.performance_weight
            + compilation_time_score * self.config.compilation_time_weight
            + memory_score * self.config.memory_weight
            + cache_hit_score * 0.2; // 缓存命中率权重固定为0.2
        
        overall_score
    }

    /// 应用阈值调整
    fn apply_threshold_adjustment(&self, suggestion: ThresholdAdjustmentSuggestion, metrics: PerformanceMetrics) -> Result<(), VmError> {
        let old_threshold = *Self::lock_mutex(&self.current_threshold)?;

        {
            let mut current_threshold = Self::lock_mutex(&self.current_threshold)?;
            *current_threshold = suggestion.suggested_threshold;
        }

        {
            let mut last_adjustment = Self::lock_mutex(&self.last_adjustment)?;
            *last_adjustment = Instant::now();
        }

        // 记录调整历史
        let adjustment_history = ThresholdAdjustmentHistory {
            timestamp: Duration::from_millis(Instant::now().elapsed().as_millis() as u64),
            old_threshold,
            new_threshold: suggestion.suggested_threshold,
            reason: suggestion.reason.clone(),
            metrics,
            adjustment_score: suggestion.performance_score,
        };

        {
            let mut history = Self::lock_mutex(&self.adjustment_history)?;
            history.push_back(adjustment_history);

            // 保持历史记录在配置的大小范围内
            while history.len() > self.config.history_size {
                history.pop_front();
            }
        }

        Ok(())
    }

    /// 获取当前编译阈值
    pub fn get_current_threshold(&self) -> u64 {
        Self::lock_mutex(&self.current_threshold)
            .map(|guard| *guard)
            .unwrap_or(100) // Return default threshold if lock fails
    }

    /// 手动设置编译阈值
    pub fn set_threshold(&self, threshold: u64) -> Result<(), VmError> {
        if threshold < self.config.min_compilation_threshold || threshold > self.config.max_compilation_threshold {
            return Err(VmError::Execution(vm_core::ExecutionError::JitError {
                message: format!("阈值 {} 超出范围 [{}, {}]", threshold, self.config.min_compilation_threshold, self.config.max_compilation_threshold),
                function_addr: None,
            }));
        }

        let old_threshold = *Self::lock_mutex(&self.current_threshold)?;

        {
            let mut current_threshold = Self::lock_mutex(&self.current_threshold)?;
            *current_threshold = threshold;
        }

        // 记录手动调整历史
        let adjustment_history = ThresholdAdjustmentHistory {
            timestamp: Duration::from_millis(Instant::now().elapsed().as_millis() as u64),
            old_threshold,
            new_threshold: threshold,
            reason: "手动设置".to_string(),
            metrics: self.calculate_current_metrics()?,
            adjustment_score: 0.0,
        };

        {
            let mut history = Self::lock_mutex(&self.adjustment_history)?;
            history.push_back(adjustment_history);
        }

        Ok(())
    }

    /// 重置为基线阈值
    pub fn reset_to_baseline(&self) -> Result<(), VmError> {
        self.set_threshold(self.config.base_compilation_threshold)
    }

    /// 设置性能基线
    pub fn set_performance_baseline(&self, baseline: PerformanceMetrics) -> Result<(), VmError> {
        let mut current_baseline = Self::lock_mutex(&self.performance_baseline)?;
        *current_baseline = baseline;
        Ok(())
    }

    /// 获取调整历史
    pub fn get_adjustment_history(&self) -> Vec<ThresholdAdjustmentHistory> {
        Self::lock_mutex(&self.adjustment_history)
            .map(|history| history.iter().cloned().collect())
            .unwrap_or_default() // Return empty vec if lock fails
    }

    /// 获取性能指标历史
    pub fn get_metrics_history(&self) -> Vec<PerformanceMetrics> {
        Self::lock_mutex(&self.metrics_history)
            .map(|history| history.iter().cloned().collect())
            .unwrap_or_default() // Return empty vec if lock fails
    }

    /// 生成自适应调整报告
    pub fn generate_report(&self) -> Result<String, VmError> {
        let current_threshold = self.get_current_threshold();
        let adjustment_history = self.get_adjustment_history();
        let metrics_history = self.get_metrics_history();
        let current_metrics = self.calculate_current_metrics()?;
        
        let mut report = String::new();
        report.push_str("# 自适应编译阈值调整报告\n\n");
        
        report.push_str("## 配置信息\n");
        report.push_str(&format!("- 基础编译阈值: {}\n", self.config.base_compilation_threshold));
        report.push_str(&format!("- 最小编译阈值: {}\n", self.config.min_compilation_threshold));
        report.push_str(&format!("- 最大编译阈值: {}\n", self.config.max_compilation_threshold));
        report.push_str(&format!("- 调整窗口: {:?}\n", self.config.adjustment_window));
        report.push_str(&format!("- 性能权重: {:.2}\n", self.config.performance_weight));
        report.push_str(&format!("- 编译时间权重: {:.2}\n", self.config.compilation_time_weight));
        report.push_str(&format!("- 内存权重: {:.2}\n", self.config.memory_weight));
        report.push_str(&format!("- 调整步长: {:.2}\n", self.config.adjustment_step));
        report.push_str(&format!("- 启用激进调整: {}\n\n", self.config.enable_aggressive_adjustment));
        
        report.push_str("## 当前状态\n");
        report.push_str(&format!("- 当前编译阈值: {}\n", current_threshold));
        report.push_str(&format!("- 平均执行时间: {:?}\n", current_metrics.average_execution_time));
        report.push_str(&format!("- 编译时间: {:?}\n", current_metrics.compilation_time));
        report.push_str(&format!("- 内存使用: {} bytes\n", current_metrics.memory_usage));
        report.push_str(&format!("- 缓存命中率: {:.2}%\n", current_metrics.cache_hit_rate * 100.0));
        report.push_str(&format!("- 执行速度: {:.2} IPS\n", current_metrics.execution_speed));
        report.push_str(&format!("- 编译收益: {:.2}\n\n", current_metrics.compilation_benefit));
        
        if !adjustment_history.is_empty() {
            report.push_str("## 调整历史\n");
            for (i, adj) in adjustment_history.iter().rev().take(10).enumerate() {
                report.push_str(&format!(
                    "{}. 时间: {:?}, {} -> {}, 原因: {}, 评分: {:.2}\n",
                    i + 1,
                    adj.timestamp,
                    adj.old_threshold,
                    adj.new_threshold,
                    adj.reason,
                    adj.adjustment_score
                ));
            }
            report.push_str("\n");
        }
        
        if !metrics_history.is_empty() {
            report.push_str("## 性能指标趋势\n");
            let recent_metrics = &metrics_history[metrics_history.len().saturating_sub(10)..];
            for (i, metrics) in recent_metrics.iter().enumerate() {
                report.push_str(&format!(
                    "{}. 执行时间: {:?}, 编译时间: {:?}, 内存: {}, 缓存命中率: {:.2}%\n",
                    i + 1,
                    metrics.average_execution_time,
                    metrics.compilation_time,
                    metrics.memory_usage,
                    metrics.cache_hit_rate * 100.0
                ));
            }
        }
        
        Ok(report)
    }

    /// 导出配置和数据
    pub fn export_data(&self) -> Result<String, VmError> {
        let data = AdaptiveThresholdData {
            config: self.config.clone(),
            current_threshold: self.get_current_threshold(),
            adjustment_history: self.get_adjustment_history(),
            metrics_history: self.get_metrics_history(),
        };
        
        serde_json::to_string_pretty(&data)
            .map_err(|e| VmError::Execution(vm_core::ExecutionError::JitError { 
                message: e.to_string(),
                function_addr: None,
            }))
    }

    /// 导入配置和数据
    pub fn import_data(&self, json: &str) -> Result<(), VmError> {
        let data: AdaptiveThresholdData = serde_json::from_str(json)
            .map_err(|e| VmError::Execution(vm_core::ExecutionError::JitError { 
                message: e.to_string(),
                function_addr: None,
            }))?;
        
        // 更新配置
        // 注意：这里不能直接更新self.config，因为它是不可变的
        // 在实际实现中，可能需要重新创建管理器实例
        
        // 更新当前阈值
        self.set_threshold(data.current_threshold)?;
        
        // 更新历史数据
        {
            let mut adjustment_history = Self::lock_mutex(&self.adjustment_history)?;
            *adjustment_history = VecDeque::from(data.adjustment_history);
        }

        {
            let mut metrics_history = Self::lock_mutex(&self.metrics_history)?;
            *metrics_history = VecDeque::from(data.metrics_history);
        }

        Ok(())
    }
}

/// 阈值调整建议
#[derive(Debug)]
struct ThresholdAdjustmentSuggestion {
    /// 是否应该调整
    should_adjust: bool,
    /// 建议的新阈值
    suggested_threshold: u64,
    /// 调整方向
    adjustment_direction: AdjustmentDirection,
    /// 调整因子
    adjustment_factor: f64,
    /// 调整原因
    reason: String,
    /// 性能评分
    performance_score: f64,
}

/// 调整方向
#[derive(Debug, Clone, Copy)]
enum AdjustmentDirection {
    Increase,
    Decrease,
    None,
}

/// 自适应阈值数据（用于导入导出）
#[derive(Debug, Serialize, Deserialize)]
struct AdaptiveThresholdData {
    config: AdaptiveCompilationConfig,
    current_threshold: u64,
    adjustment_history: Vec<ThresholdAdjustmentHistory>,
    metrics_history: Vec<PerformanceMetrics>,
}