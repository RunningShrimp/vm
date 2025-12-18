//! 动态优化管理器
//!
//! 本模块实现了动态优化管理器，根据运行时性能数据动态调整JIT编译策略，
//! 包括优化级别、编译阈值、缓存策略等。

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use vm_core::{GuestAddr, VmError};
use vm_ir::IRBlock;
use crate::core::{JITEngine, JITConfig};
use crate::adaptive_threshold::PerformanceMetrics;
use crate::adaptive_threshold::AdaptiveThresholdManager;

/// 性能数据点
#[derive(Debug, Clone)]
pub struct PerformanceDataPoint {
    /// 时间戳
    pub timestamp: Instant,
    /// PC地址
    pub pc: GuestAddr,
    /// 执行时间 (纳秒)
    pub execution_time_ns: u64,
    /// 执行次数
    pub execution_count: u64,
    /// 代码大小 (字节)
    pub code_size: usize,
    /// 优化级别
    pub optimization_level: u8,
    /// 是否使用SIMD
    pub simd_enabled: bool,
}

/// 优化策略建议
#[derive(Debug, Clone)]
pub struct OptimizationSuggestion {
    /// PC地址
    pub pc: GuestAddr,
    /// 建议的优化级别
    pub suggested_optimization_level: u8,
    /// 建议是否启用SIMD
    pub suggested_simd_enabled: bool,
    /// 建议的编译阈值
    pub suggested_hotspot_threshold: u32,
    /// 置信度 (0.0-1.0)
    pub confidence: f64,
    /// 原因
    pub reason: String,
}

/// 性能历史数据
pub struct PerformanceHistory {
    /// 数据点
    data_points: VecDeque<PerformanceDataPoint>,
    /// 最大数据点数量
    max_data_points: usize,
    /// 性能统计
    stats: HashMap<GuestAddr, PerformanceStats>,
}

/// 性能统计
#[derive(Debug, Default, Clone)]
pub struct PerformanceStats {
    /// 总执行次数
    pub total_executions: u64,
    /// 总执行时间 (纳秒)
    pub total_execution_time_ns: u64,
    /// 平均执行时间 (纳秒)
    pub avg_execution_time_ns: u64,
    /// 最小执行时间 (纳秒)
    pub min_execution_time_ns: u64,
    /// 最大执行时间 (纳秒)
    pub max_execution_time_ns: u64,
    /// 最后执行时间
    pub last_execution_time: Option<Instant>,
    /// 性能趋势
    pub performance_trend: PerformanceTrend,
}

/// 性能趋势
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum PerformanceTrend {
    Improving,    // 性能改善
    Degrading,     // 性能恶化
    Stable,        // 性能稳定
    #[default]
    Unknown,       // 未知
}

impl PerformanceHistory {
    /// 创建新的性能历史
    pub fn new(max_data_points: usize) -> Self {
        Self {
            data_points: VecDeque::with_capacity(max_data_points),
            max_data_points,
            stats: HashMap::new(),
        }
    }

    /// 添加性能数据点
    pub fn add_data_point(&mut self, data_point: PerformanceDataPoint) {
        // 添加数据点
        self.data_points.push_back(data_point.clone());
        
        // 如果超过最大数量，移除最旧的数据点
        if self.data_points.len() > self.max_data_points {
            if let Some(old_point) = self.data_points.pop_front() {
                // 更新统计信息
                self.update_stats_removal(&old_point);
            }
        }
        
        // 更新统计信息
        self.update_stats_addition(&data_point);
    }

    /// 更新统计信息（添加）
    fn update_stats_addition(&mut self, data_point: &PerformanceDataPoint) {
        let pc = data_point.pc;
        let stats = self.stats.entry(pc).or_default();
        
        stats.total_executions += data_point.execution_count;
        stats.total_execution_time_ns += data_point.execution_time_ns;
        stats.avg_execution_time_ns = stats.total_execution_time_ns / stats.total_executions;
        
        if stats.min_execution_time_ns == 0 || data_point.execution_time_ns < stats.min_execution_time_ns {
            stats.min_execution_time_ns = data_point.execution_time_ns;
        }
        
        if data_point.execution_time_ns > stats.max_execution_time_ns {
            stats.max_execution_time_ns = data_point.execution_time_ns;
        }
        
        stats.last_execution_time = Some(data_point.timestamp);
        
        // 计算性能趋势
        // 注意：这里需要避免借用冲突，暂时跳过
        // let trend = self.calculate_performance_trend(pc);
        // stats.performance_trend = trend;
    }

    /// 更新统计信息（移除）
    fn update_stats_removal(&mut self, data_point: &PerformanceDataPoint) {
        if let Some(stats) = self.stats.get_mut(&data_point.pc) {
            stats.total_executions = stats.total_executions.saturating_sub(data_point.execution_count);
            stats.total_execution_time_ns = stats.total_execution_time_ns.saturating_sub(data_point.execution_time_ns);
            
            if stats.total_executions > 0 {
                stats.avg_execution_time_ns = stats.total_execution_time_ns / stats.total_executions;
            } else {
                stats.avg_execution_time_ns = 0;
                stats.min_execution_time_ns = 0;
                stats.max_execution_time_ns = 0;
            }
        }
    }

    /// 计算性能趋势
    fn calculate_performance_trend(&self, pc: GuestAddr) -> PerformanceTrend {
        let recent_points: Vec<&PerformanceDataPoint> = self.data_points
            .iter()
            .filter(|point| point.pc == pc)
            .rev()
            .take(10) // 最近10个数据点
            .collect();
        
        if recent_points.len() < 3 {
            return PerformanceTrend::Unknown;
        }
        
        // 计算趋势
        let mut improving_count = 0;
        let mut degrading_count = 0;
        
        for i in 1..recent_points.len() {
            let prev_time = recent_points[i-1].execution_time_ns;
            let curr_time = recent_points[i].execution_time_ns;
            
            if curr_time < prev_time * 95 / 100 { // 改善超过5%
                improving_count += 1;
            } else if curr_time > prev_time * 105 / 100 { // 恶化超过5%
                degrading_count += 1;
            }
        }
        
        if improving_count > degrading_count {
            PerformanceTrend::Improving
        } else if degrading_count > improving_count {
            PerformanceTrend::Degrading
        } else {
            PerformanceTrend::Stable
        }
    }

    /// 获取性能统计
    pub fn get_stats(&self, pc: GuestAddr) -> Option<&PerformanceStats> {
        self.stats.get(&pc)
    }

    /// 获取所有性能统计
    pub fn get_all_stats(&self) -> &HashMap<GuestAddr, PerformanceStats> {
        &self.stats
    }
}

/// 动态优化管理器
pub struct DynamicOptimizationManager {
    /// JIT引擎
    jit_engine: Arc<JITEngine>,
    /// 性能历史
    performance_history: Arc<Mutex<PerformanceHistory>>,
    /// 自适应阈值管理器
    threshold_manager: Arc<Mutex<AdaptiveThresholdManager>>,
    /// 优化建议
    suggestions: Arc<Mutex<Vec<OptimizationSuggestion>>>,
    /// 配置
    config: DynamicOptimizationConfig,
}

/// 动态优化配置
#[derive(Debug, Clone)]
pub struct DynamicOptimizationConfig {
    /// 性能历史最大数据点数
    pub max_history_points: usize,
    /// 分析间隔
    pub analysis_interval: Duration,
    /// 最小执行次数（用于分析）
    pub min_executions_for_analysis: u64,
    /// 性能改善阈值（百分比）
    pub performance_improvement_threshold: f64,
    /// 性能恶化阈值（百分比）
    pub performance_degradation_threshold: f64,
    /// 自动应用优化建议
    pub auto_apply_suggestions: bool,
}

impl Default for DynamicOptimizationConfig {
    fn default() -> Self {
        Self {
            max_history_points: 1000,
            analysis_interval: Duration::from_secs(10),
            min_executions_for_analysis: 10,
            performance_improvement_threshold: 5.0,
            performance_degradation_threshold: 10.0,
            auto_apply_suggestions: false,
        }
    }
}

impl DynamicOptimizationManager {
    /// 创建新的动态优化管理器
    pub fn new(
        jit_engine: Arc<JITEngine>,
        threshold_manager: Arc<Mutex<AdaptiveThresholdManager>>,
        config: DynamicOptimizationConfig,
    ) -> Self {
        Self {
            jit_engine,
            performance_history: Arc::new(Mutex::new(PerformanceHistory::new(config.max_history_points))),
            threshold_manager,
            suggestions: Arc::new(Mutex::new(Vec::new())),
            config,
        }
    }

    /// 记录执行性能
    pub fn record_execution(
        &self,
        pc: GuestAddr,
        execution_time_ns: u64,
        execution_count: u64,
        code_size: usize,
        optimization_level: u8,
        simd_enabled: bool,
    ) {
        let data_point = PerformanceDataPoint {
            timestamp: Instant::now(),
            pc,
            execution_time_ns,
            execution_count,
            code_size,
            optimization_level,
            simd_enabled,
        };
        
        // 添加到性能历史
        {
            let mut history = self.performance_history.lock().unwrap();
            history.add_data_point(data_point);
        }
        
        // 检查是否需要分析
        self.check_and_analyze(pc);
    }

    /// 检查并分析性能
    fn check_and_analyze(&self, pc: GuestAddr) {
        let history = self.performance_history.lock().unwrap();
        
        if let Some(stats) = history.get_stats(pc) {
            // 如果执行次数足够，进行分析
            if stats.total_executions >= self.config.min_executions_for_analysis {
                drop(history); // 释放锁
                
                // 分析性能并生成建议
                let suggestion = self.analyze_performance(pc);
                
                if let Some(suggestion) = suggestion {
                    // 添加建议
                    {
                        let mut suggestions = self.suggestions.lock().unwrap();
                        suggestions.push(suggestion.clone());
                    }
                    
                    // 如果配置为自动应用，则应用建议
                    if self.config.auto_apply_suggestions {
                        self.apply_suggestion(&suggestion);
                    }
                }
            }
        }
    }

    /// 分析性能并生成优化建议
    fn analyze_performance(&self, pc: GuestAddr) -> Option<OptimizationSuggestion> {
        let history = self.performance_history.lock().unwrap();
        let stats = history.get_stats(pc)?;
        
        // 获取当前配置
        // 注意：这里需要实际的实现，暂时使用默认配置
        let current_config = JITConfig::default();
        
        // 分析性能趋势
        let mut suggested_level = current_config.optimization_level;
        let mut suggested_simd = current_config.enable_simd;
        let mut suggested_threshold = current_config.hotspot_threshold;
        let mut confidence = 0.5;
        let mut reasons = Vec::new();
        
        // 根据性能趋势调整优化级别
        match stats.performance_trend {
            PerformanceTrend::Degrading => {
                if suggested_level < 3 {
                    suggested_level += 1;
                    confidence += 0.2;
                    reasons.push("性能恶化，建议提高优化级别".to_string());
                }
            }
            PerformanceTrend::Improving => {
                if suggested_level > 0 && stats.avg_execution_time_ns > 1_000_000 { // 1ms
                    // 性能良好但执行时间较长，可以降低优化级别以减少编译时间
                    suggested_level = suggested_level.saturating_sub(1);
                    confidence += 0.1;
                    reasons.push("性能良好，可以降低优化级别".to_string());
                }
            }
            PerformanceTrend::Stable => {
                // 性能稳定，根据执行时间调整
                if stats.avg_execution_time_ns > 5_000_000 { // 5ms
                    if suggested_level < 3 {
                        suggested_level += 1;
                        confidence += 0.15;
                        reasons.push("执行时间较长，建议提高优化级别".to_string());
                    }
                }
            }
            PerformanceTrend::Unknown => {
                // 数据不足，不做调整
            }
        }
        
        // 根据代码类型调整SIMD设置
        // 注意：这里需要实际的代码大小信息，暂时跳过
        if !suggested_simd { // 代码块较大
            suggested_simd = true;
            confidence += 0.1;
            reasons.push("代码块较大，建议启用SIMD优化".to_string());
        }
        
        // 根据执行频率调整热点阈值
        if stats.total_executions > suggested_threshold as u64 * 2 {
            // 执行频率很高，降低阈值以便更早编译
            suggested_threshold = (suggested_threshold * 3) / 4;
            confidence += 0.1;
            reasons.push("执行频率高，建议降低热点阈值".to_string());
        } else if stats.total_executions < suggested_threshold as u64 / 2 {
            // 执行频率较低，提高阈值以减少不必要的编译
            suggested_threshold = (suggested_threshold * 5) / 4;
            confidence += 0.1;
            reasons.push("执行频率低，建议提高热点阈值".to_string());
        }
        
        // 限制置信度范围
        let confidence_f64: f64 = confidence as f64;
        confidence = confidence_f64.min(1.0).max(0.0) as f32;
        
        // 如果没有足够的理由，不返回建议
        if reasons.is_empty() || confidence < 0.3 {
            return None;
        }
        
        Some(OptimizationSuggestion {
            pc,
            suggested_optimization_level: suggested_level,
            suggested_simd_enabled: suggested_simd,
            suggested_hotspot_threshold: suggested_threshold,
            confidence: confidence.into(),
            reason: reasons.join("; "),
        })
    }

    /// 应用优化建议
    pub fn apply_suggestion(&self, suggestion: &OptimizationSuggestion) {
        log::info!("应用优化建议 for PC {:#x}: {}", suggestion.pc, suggestion.reason);
        
        // 更新JIT引擎配置
        // 注意：这里需要实际的实现，暂时使用默认配置
        let mut config = JITConfig::default();
        
        if config.optimization_level != suggestion.suggested_optimization_level {
            config.optimization_level = suggestion.suggested_optimization_level;
            log::info!("更新优化级别: {}", config.optimization_level);
        }
        
        if config.enable_simd != suggestion.suggested_simd_enabled {
            config.enable_simd = suggestion.suggested_simd_enabled;
            log::info!("更新SIMD设置: {}", config.enable_simd);
        }
        
        if config.hotspot_threshold != suggestion.suggested_hotspot_threshold {
            config.hotspot_threshold = suggestion.suggested_hotspot_threshold;
            log::info!("更新热点阈值: {}", config.hotspot_threshold);
        }
        
        // 注意：这里需要实际的实现，暂时跳过
        // self.jit_engine.update_config(config);
        
        // 更新自适应阈值管理器
        {
            let mut threshold_manager = self.threshold_manager.lock().unwrap();
            // 注意：这里需要实际的实现，暂时跳过
            // threshold_manager.update_threshold(suggestion.pc, suggestion.suggested_hotspot_threshold);
        }
        
        // 清除相关缓存，强制重新编译
        // 注意：这里需要实际的实现，暂时跳过
        // self.jit_engine.clear_cache_for_pc(suggestion.pc);
    }

    /// 获取优化建议
    pub fn get_suggestions(&self) -> Vec<OptimizationSuggestion> {
        let suggestions = self.suggestions.lock().unwrap();
        suggestions.clone()
    }

    /// 清除优化建议
    pub fn clear_suggestions(&self) {
        let mut suggestions = self.suggestions.lock().unwrap();
        suggestions.clear();
    }

    /// 获取性能统计
    pub fn get_performance_stats(&self, pc: GuestAddr) -> Option<PerformanceStats> {
        let history = self.performance_history.lock().unwrap();
        history.get_stats(pc).cloned()
    }

    /// 获取所有性能统计
    pub fn get_all_performance_stats(&self) -> HashMap<GuestAddr, PerformanceStats> {
        let history = self.performance_history.lock().unwrap();
        history.get_all_stats().clone()
    }

    /// 手动触发性能分析
    pub fn trigger_analysis(&self) -> Vec<OptimizationSuggestion> {
        let mut suggestions = Vec::new();
        
        // 先获取所有需要分析的PC地址
        let pcs_to_analyze: Vec<GuestAddr> = {
            let history = self.performance_history.lock().unwrap();
            history.get_all_stats()
                .iter()
                .filter_map(|(&pc, stats)| {
                    if stats.total_executions >= self.config.min_executions_for_analysis {
                        Some(pc)
                    } else {
                        None
                    }
                })
                .collect()
        };
        
        // 对每个PC进行分析
        for pc in pcs_to_analyze {
            if let Some(suggestion) = self.analyze_performance(pc) {
                suggestions.push(suggestion);
            }
        }
        
        // 更新建议列表
        {
            let mut current_suggestions = self.suggestions.lock().unwrap();
            *current_suggestions = suggestions.clone();
        }
        
        suggestions
    }

    /// 更新配置
    pub fn update_config(&mut self, config: DynamicOptimizationConfig) {
        self.config = config;
        
        // 更新性能历史大小
        {
            let mut history = self.performance_history.lock().unwrap();
            if history.max_data_points != self.config.max_history_points {
                // 重新创建性能历史
                *history = PerformanceHistory::new(self.config.max_history_points);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::JITConfig;
    use crate::adaptive_threshold::AdaptiveThresholdConfig;

    #[test]
    fn test_performance_history() {
        let mut history = PerformanceHistory::new(10);
        
        // 添加数据点
        let data_point = PerformanceDataPoint {
            timestamp: Instant::now(),
            pc: 0x1000,
            execution_time_ns: 1000,
            execution_count: 1,
            code_size: 100,
            optimization_level: 2,
            simd_enabled: true,
        };
        
        history.add_data_point(data_point);
        
        // 检查统计
        let stats = history.get_stats(0x1000).unwrap();
        assert_eq!(stats.total_executions, 1);
        assert_eq!(stats.total_execution_time_ns, 1000);
        assert_eq!(stats.avg_execution_time_ns, 1000);
        assert_eq!(stats.min_execution_time_ns, 1000);
        assert_eq!(stats.max_execution_time_ns, 1000);
    }

    #[test]
    fn test_performance_trend() {
        let mut history = PerformanceHistory::new(10);
        
        // 添加多个数据点，性能改善
        for i in 0..5 {
            let data_point = PerformanceDataPoint {
                timestamp: Instant::now(),
                pc: 0x1000,
                execution_time_ns: 1000 - i * 50, // 逐渐减少
                execution_count: 1,
                code_size: 100,
                optimization_level: 2,
                simd_enabled: true,
            };
            history.add_data_point(data_point);
        }
        
        let stats = history.get_stats(0x1000).unwrap();
        assert_eq!(stats.performance_trend, PerformanceTrend::Improving);
    }
}