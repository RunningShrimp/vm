//! # vm-adaptive - 自适应优化机制
//!
//! 基于运行时反馈的动态优化系统，实现智能的性能调优和资源管理。
//!
//! ## 主要功能
//!
//! - **运行时性能分析**: 实时收集和分析系统性能指标
//! - **动态优化策略**: 基于反馈的编译和执行优化
//! - **工作负载感知**: 识别和适应不同的工作负载模式
//! - **硬件感知优化**: 根据硬件特性动态调整优化策略
//! - **自适应阈值调整**: 运行时调整各种性能阈值
//! - **资源管理优化**: 智能的CPU、内存和I/O资源分配

// 条件导入
cfg_if::cfg_if! {
    if #[cfg(feature = "async")] {
        use tokio;
        use num_cpus;
    }
}

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;
// TODO: vm_monitor 模块暂时禁用，需要实现或从 vm-monitor 包导入
// use vm_monitor::PerformanceMonitor;

/// 简化的性能监控器（替代 vm_monitor）
#[derive(Debug, Clone)]
pub struct PerformanceMonitor {
    pub metrics: HashMap<String, f64>,
    pub time_series: HashMap<String, Vec<(std::time::Instant, f64)>>,
}

impl PerformanceMonitor {
    pub fn new() -> Self {
        Self {
            metrics: HashMap::new(),
            time_series: HashMap::new(),
        }
    }

    pub fn record_metric(&mut self, key: String, value: f64) {
        self.metrics.insert(key.clone(), value);
        let entry = self.time_series.entry(key).or_insert_with(Vec::new);
        entry.push((std::time::Instant::now(), value));
    }

    pub fn record_metric_with_timestamp(&mut self, key: String, value: f64, _timestamp: std::time::Instant) {
        self.metrics.insert(key.clone(), value);
        let entry = self.time_series.entry(key).or_insert_with(Vec::new);
        entry.push((_timestamp, value));
    }

    pub fn get_metric(&self, key: &str) -> Option<f64> {
        self.metrics.get(key).copied()
    }

    pub fn get_metric_stats(&self, key: &str, _window_seconds: u64) -> Option<MetricStats> {
        let values = self.time_series.get(key)?;
        if values.is_empty() {
            return None;
        }

        let len = values.len();
        let sum: f64 = values.iter().map(|v| v.1).sum();
        let avg = sum / len as f64;
        let min = values.iter().map(|v| v.1).reduce(f64::min).unwrap_or(0.0);
        let max = values.iter().map(|v| v.1).reduce(f64::max).unwrap_or(0.0);

        Some(MetricStats {
            count: len,
            avg,
            min,
            max,
            current: values.last().map(|v| v.1).unwrap_or(0.0),
        })
    }
}

/// 指标统计信息
#[derive(Debug, Clone)]
pub struct MetricStats {
    pub count: usize,
    pub avg: f64,
    pub min: f64,
    pub max: f64,
    pub current: f64,
}

/// 自适应优化器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveOptimizerConfig {
    /// 分析窗口大小（秒）
    pub analysis_window_seconds: u64,
    /// 优化调整间隔（秒）
    pub adjustment_interval_seconds: u64,
    /// 性能阈值容忍度
    pub performance_tolerance: f64,
    /// 启用工作负载感知
    pub enable_workload_awareness: bool,
    /// 启用硬件感知优化
    pub enable_hardware_awareness: bool,
    /// 最大优化尝试次数
    pub max_optimization_attempts: usize,
    /// 优化冷却时间（秒）
    pub optimization_cooldown_seconds: u64,
}

impl Default for AdaptiveOptimizerConfig {
    fn default() -> Self {
        Self {
            analysis_window_seconds: 300,    // 5分钟
            adjustment_interval_seconds: 60, // 1分钟
            performance_tolerance: 0.05,     // 5%
            enable_workload_awareness: true,
            enable_hardware_awareness: true,
            max_optimization_attempts: 10,
            optimization_cooldown_seconds: 30,
        }
    }
}

/// 工作负载特征
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkloadPattern {
    /// CPU密集型
    CpuIntensive,
    /// 内存密集型
    MemoryIntensive,
    /// I/O密集型
    IoIntensive,
    /// 混合型
    Mixed,
    /// 未知/未分类
    Unknown,
}

/// 硬件特性信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareProfile {
    /// CPU核心数
    pub cpu_cores: usize,
    /// CPU频率（MHz）
    pub cpu_frequency_mhz: u64,
    /// L1缓存大小（KB）
    pub l1_cache_kb: usize,
    /// L2缓存大小（KB）
    pub l2_cache_kb: usize,
    /// L3缓存大小（KB）
    pub l3_cache_kb: usize,
    /// 内存大小（GB）
    pub memory_gb: usize,
    /// 是否支持SIMD
    pub has_simd: bool,
    /// 是否支持AVX512
    pub has_avx512: bool,
    /// NUMA节点数
    pub numa_nodes: usize,
}

/// 优化建议
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationRecommendation {
    /// 建议ID
    pub id: String,
    /// 建议类型
    pub recommendation_type: RecommendationType,
    /// 描述
    pub description: String,
    /// 预期性能提升
    pub expected_improvement: f64,
    /// 置信度（0.0-1.0）
    pub confidence: f64,
    /// 优先级
    pub priority: Priority,
    /// 实施成本
    pub implementation_cost: Cost,
}

/// 建议类型
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RecommendationType {
    /// JIT编译优化
    JitOptimization,
    /// 内存管理优化
    MemoryOptimization,
    /// I/O优化
    IoOptimization,
    /// CPU调度优化
    CpuScheduling,
    /// 缓存优化
    CacheOptimization,
    /// SIMD优化
    SimdOptimization,
}

/// 优先级
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

/// 实施成本
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Cost {
    /// 低成本（配置更改）
    Low,
    /// 中等成本（代码路径更改）
    Medium,
    /// 高成本（架构更改）
    High,
}

/// 性能分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAnalysis {
    /// 时间戳
    pub timestamp: u64,
    /// 当前工作负载模式
    pub workload_pattern: WorkloadPattern,
    /// 性能指标
    pub metrics: HashMap<String, f64>,
    /// 瓶颈识别
    pub bottlenecks: Vec<String>,
    /// 优化机会
    pub optimization_opportunities: Vec<String>,
}

/// 自适应优化器
pub struct AdaptiveOptimizer {
    /// 配置
    config: AdaptiveOptimizerConfig,
    /// 性能监控器
    performance_monitor: Arc<PerformanceMonitor>,
    /// 硬件特性
    hardware_profile: HardwareProfile,
    /// 历史性能分析
    performance_history: VecDeque<PerformanceAnalysis>,
    /// 当前优化状态
    optimization_state: OptimizationState,
    /// 最后优化时间
    last_optimization_time: Instant,
    /// 优化尝试计数
    optimization_attempts: usize,
    /// 当前活跃的优化建议
    active_recommendations: HashMap<String, OptimizationRecommendation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationState {
    /// 当前JIT阈值
    jit_threshold: u64,
    /// 当前内存分配策略
    memory_strategy: String,
    /// 当前I/O调度策略
    io_strategy: String,
    /// 当前CPU亲和性设置
    cpu_affinity: Vec<usize>,
    /// SIMD启用状态
    simd_enabled: bool,
}

impl Default for OptimizationState {
    fn default() -> Self {
        Self {
            jit_threshold: 100,
            memory_strategy: "default".to_string(),
            io_strategy: "default".to_string(),
            cpu_affinity: Vec::new(),
            simd_enabled: true,
        }
    }
}

impl AdaptiveOptimizer {
    /// 创建新的自适应优化器
    pub fn new(
        config: AdaptiveOptimizerConfig,
        performance_monitor: Arc<PerformanceMonitor>,
    ) -> Self {
        let hardware_profile = Self::detect_hardware_profile();

        Self {
            config,
            performance_monitor,
            hardware_profile,
            performance_history: VecDeque::with_capacity(100),
            optimization_state: OptimizationState::default(),
            last_optimization_time: Instant::now(),
            optimization_attempts: 0,
            active_recommendations: HashMap::new(),
        }
    }

    /// 运行自适应优化循环
    #[cfg(feature = "async")]
    pub async fn run_optimization_loop(&mut self) {
        let mut interval =
            tokio::time::interval(Duration::from_secs(self.config.adjustment_interval_seconds));

        loop {
            interval.tick().await;
            self.perform_optimization_cycle().await;
        }
    }

    /// 执行优化周期
    #[cfg(feature = "async")]
    async fn perform_optimization_cycle(&mut self) {
        // 检查冷却时间
        if self.last_optimization_time.elapsed()
            < Duration::from_secs(self.config.optimization_cooldown_seconds)
        {
            return;
        }

        // 分析当前性能
        let analysis = self.analyze_performance();

        // 存储分析结果
        self.performance_history.push_back(analysis.clone());
        if self.performance_history.len() > 100 {
            self.performance_history.pop_front();
        }

        // 生成优化建议
        let recommendations = self.generate_recommendations(&analysis);

        // 应用优化建议
        for recommendation in recommendations {
            if self.should_apply_recommendation(&recommendation) {
                self.apply_recommendation(recommendation).await;
                self.last_optimization_time = Instant::now();
                self.optimization_attempts += 1;

                if self.optimization_attempts >= self.config.max_optimization_attempts {
                    break;
                }
            }
        }

        // 清理过期建议
        self.cleanup_expired_recommendations();
    }

    /// 分析当前性能
    fn analyze_performance(&self) -> PerformanceAnalysis {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // 收集关键指标
        let mut metrics = HashMap::new();

        // CPU使用率
        if let Some(stats) = self
            .performance_monitor
            .get_metric_stats("system.cpu.usage", 300)
        {
            metrics.insert("cpu_usage".to_string(), stats.avg);
        }

        // 内存使用率
        if let Some(stats) = self
            .performance_monitor
            .get_metric_stats("system.memory.usage", 300)
        {
            metrics.insert("memory_usage".to_string(), stats.avg);
        }

        // JIT编译时间
        if let Some(stats) = self
            .performance_monitor
            .get_metric_stats("vm.jit.compile_time", 300)
        {
            metrics.insert("jit_compile_time".to_string(), stats.avg);
        }

        // TLB命中率
        if let Some(stats) = self
            .performance_monitor
            .get_metric_stats("vm.tlb.hit_rate", 300)
        {
            metrics.insert("tlb_hit_rate".to_string(), stats.avg);
        }

        // 识别工作负载模式
        let workload_pattern = self.identify_workload_pattern(&metrics);

        // 识别瓶颈
        let bottlenecks = self.identify_bottlenecks(&metrics);

        // 识别优化机会
        let optimization_opportunities =
            self.identify_optimization_opportunities(&metrics, &workload_pattern);

        PerformanceAnalysis {
            timestamp,
            workload_pattern,
            metrics,
            bottlenecks,
            optimization_opportunities,
        }
    }

    /// 识别工作负载模式
    fn identify_workload_pattern(&self, metrics: &HashMap<String, f64>) -> WorkloadPattern {
        let cpu_usage = metrics.get("cpu_usage").copied().unwrap_or(0.0);
        let memory_usage = metrics.get("memory_usage").copied().unwrap_or(0.0);
        let jit_compile_time = metrics.get("jit_compile_time").copied().unwrap_or(0.0);

        // 简单的启发式规则
        if cpu_usage > 80.0 && jit_compile_time > 1000.0 {
            WorkloadPattern::CpuIntensive
        } else if memory_usage > 80.0 {
            WorkloadPattern::MemoryIntensive
        } else if cpu_usage < 30.0 && memory_usage < 50.0 {
            WorkloadPattern::IoIntensive
        } else {
            WorkloadPattern::Mixed
        }
    }

    /// 识别性能瓶颈
    fn identify_bottlenecks(&self, metrics: &HashMap<String, f64>) -> Vec<String> {
        let mut bottlenecks = Vec::new();

        if let Some(cpu_usage) = metrics.get("cpu_usage")
            && *cpu_usage > 90.0
        {
            bottlenecks.push("High CPU usage".to_string());
        }

        if let Some(memory_usage) = metrics.get("memory_usage")
            && *memory_usage > 85.0
        {
            bottlenecks.push("High memory usage".to_string());
        }

        if let Some(tlb_hit_rate) = metrics.get("tlb_hit_rate")
            && *tlb_hit_rate < 0.8
        {
            bottlenecks.push("Low TLB hit rate".to_string());
        }

        if let Some(jit_compile_time) = metrics.get("jit_compile_time")
            && *jit_compile_time > 5000.0
        {
            bottlenecks.push("High JIT compilation time".to_string());
        }

        bottlenecks
    }

    /// 识别优化机会
    fn identify_optimization_opportunities(
        &self,
        metrics: &HashMap<String, f64>,
        workload_pattern: &WorkloadPattern,
    ) -> Vec<String> {
        let mut opportunities = Vec::new();

        match workload_pattern {
            WorkloadPattern::CpuIntensive => {
                opportunities.push(
                    "Consider increasing JIT threshold for CPU-intensive workloads".to_string(),
                );
                if self.hardware_profile.has_simd {
                    opportunities.push("Enable SIMD optimizations".to_string());
                }
            }
            WorkloadPattern::MemoryIntensive => {
                opportunities.push("Optimize memory allocation strategy".to_string());
                opportunities.push("Consider NUMA-aware memory placement".to_string());
            }
            WorkloadPattern::IoIntensive => {
                opportunities.push("Optimize I/O scheduling".to_string());
                opportunities.push("Consider asynchronous I/O".to_string());
            }
            WorkloadPattern::Mixed => {
                opportunities
                    .push("Balance resource allocation across CPU, memory, and I/O".to_string());
            }
            WorkloadPattern::Unknown => {
                opportunities.push("Gather more performance data for better analysis".to_string());
            }
        }

        // 基于具体指标的建议
        if let Some(tlb_hit_rate) = metrics.get("tlb_hit_rate")
            && *tlb_hit_rate < 0.85
        {
            opportunities
                .push("Improve TLB performance through better memory access patterns".to_string());
        }

        opportunities
    }

    /// 生成优化建议
    fn generate_recommendations(
        &self,
        analysis: &PerformanceAnalysis,
    ) -> Vec<OptimizationRecommendation> {
        let mut recommendations = Vec::new();

        // 基于工作负载模式的建议
        match analysis.workload_pattern {
            WorkloadPattern::CpuIntensive => {
                recommendations.push(OptimizationRecommendation {
                    id: "jit_threshold_increase".to_string(),
                    recommendation_type: RecommendationType::JitOptimization,
                    description: "Increase JIT compilation threshold for CPU-intensive workloads"
                        .to_string(),
                    expected_improvement: 0.15,
                    confidence: 0.8,
                    priority: Priority::High,
                    implementation_cost: Cost::Low,
                });

                if self.hardware_profile.has_simd && !self.optimization_state.simd_enabled {
                    recommendations.push(OptimizationRecommendation {
                        id: "enable_simd".to_string(),
                        recommendation_type: RecommendationType::SimdOptimization,
                        description: "Enable SIMD optimizations for better vector processing"
                            .to_string(),
                        expected_improvement: 0.25,
                        confidence: 0.9,
                        priority: Priority::High,
                        implementation_cost: Cost::Low,
                    });
                }
            }
            WorkloadPattern::MemoryIntensive => {
                recommendations.push(OptimizationRecommendation {
                    id: "memory_strategy_optimize".to_string(),
                    recommendation_type: RecommendationType::MemoryOptimization,
                    description: "Switch to optimized memory allocation strategy".to_string(),
                    expected_improvement: 0.1,
                    confidence: 0.7,
                    priority: Priority::Medium,
                    implementation_cost: Cost::Medium,
                });
            }
            WorkloadPattern::IoIntensive => {
                recommendations.push(OptimizationRecommendation {
                    id: "io_strategy_optimize".to_string(),
                    recommendation_type: RecommendationType::IoOptimization,
                    description: "Optimize I/O scheduling for I/O-intensive workloads".to_string(),
                    expected_improvement: 0.2,
                    confidence: 0.8,
                    priority: Priority::High,
                    implementation_cost: Cost::Medium,
                });
            }
            _ => {}
        }

        // 基于瓶颈的建议
        for bottleneck in &analysis.bottlenecks {
            if bottleneck.contains("TLB") {
                recommendations.push(OptimizationRecommendation {
                    id: "tlb_optimization".to_string(),
                    recommendation_type: RecommendationType::CacheOptimization,
                    description: "Optimize memory access patterns to improve TLB performance"
                        .to_string(),
                    expected_improvement: 0.12,
                    confidence: 0.75,
                    priority: Priority::Medium,
                    implementation_cost: Cost::Medium,
                });
            }
        }

        recommendations
    }

    /// 判断是否应该应用建议
    fn should_apply_recommendation(&self, recommendation: &OptimizationRecommendation) -> bool {
        // 检查置信度和优先级
        if recommendation.confidence < 0.6 || matches!(recommendation.priority, Priority::Low) {
            return false;
        }

        // 检查是否已经在活跃建议中
        if self.active_recommendations.contains_key(&recommendation.id) {
            return false;
        }

        // 检查实施成本
        if matches!(recommendation.implementation_cost, Cost::High)
            && self.optimization_attempts >= self.config.max_optimization_attempts / 2
        {
            return false;
        }

        true
    }

    /// 应用优化建议
    async fn apply_recommendation(&mut self, recommendation: OptimizationRecommendation) {
        println!(
            "Applying optimization recommendation: {}",
            recommendation.description
        );

        match recommendation.recommendation_type {
            RecommendationType::JitOptimization => {
                if recommendation.id == "jit_threshold_increase" {
                    self.optimization_state.jit_threshold =
                        (self.optimization_state.jit_threshold as f64 * 1.5) as u64;
                    // 这里应该通知JIT编译器更新阈值
                }
            }
            RecommendationType::SimdOptimization => {
                if recommendation.id == "enable_simd" {
                    self.optimization_state.simd_enabled = true;
                    // 这里应该启用SIMD优化
                }
            }
            RecommendationType::MemoryOptimization => {
                if recommendation.id == "memory_strategy_optimize" {
                    self.optimization_state.memory_strategy = "optimized".to_string();
                    // 这里应该切换内存分配策略
                }
            }
            RecommendationType::IoOptimization => {
                if recommendation.id == "io_strategy_optimize" {
                    self.optimization_state.io_strategy = "optimized".to_string();
                    // 这里应该优化I/O调度
                }
            }
            _ => {}
        }

        // 记录活跃建议
        self.active_recommendations
            .insert(recommendation.id.clone(), recommendation);
    }

    /// 清理过期建议
    fn cleanup_expired_recommendations(&mut self) {
        // 简单的清理策略：保留最近的建议
        if self.active_recommendations.len() > 10 {
            let keys_to_remove: Vec<String> = self
                .active_recommendations
                .keys()
                .take(self.active_recommendations.len() - 10)
                .cloned()
                .collect();

            for key in keys_to_remove {
                self.active_recommendations.remove(&key);
            }
        }
    }

    /// 检测硬件特性
    fn detect_hardware_profile() -> HardwareProfile {
        // 简化的硬件检测
        HardwareProfile {
            cpu_cores: {
                #[cfg(all(feature = "async", target_os = "linux"))]
                { num_cpus::get() }

                #[cfg(not(all(feature = "async", target_os = "linux")))]
                { 4 } // 默认 4 核心
            },
            cpu_frequency_mhz: 3000, // 默认值
            l1_cache_kb: 32,
            l2_cache_kb: 256,
            l3_cache_kb: 8192,
            memory_gb: 16,
            has_simd: cfg!(target_feature = "sse2") || cfg!(target_feature = "neon"),
            has_avx512: cfg!(target_feature = "avx512f"),
            numa_nodes: 1,
        }
    }

    /// 获取当前优化状态
    pub fn get_optimization_state(&self) -> &OptimizationState {
        &self.optimization_state
    }

    /// 获取活跃的优化建议
    pub fn get_active_recommendations(&self) -> Vec<&OptimizationRecommendation> {
        self.active_recommendations.values().collect()
    }

    /// 获取性能历史
    pub fn get_performance_history(&self) -> &VecDeque<PerformanceAnalysis> {
        &self.performance_history
    }

    /// 生成优化报告
    pub fn generate_optimization_report(&self) -> OptimizationReport {
        OptimizationReport {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            hardware_profile: self.hardware_profile.clone(),
            current_state: self.optimization_state.clone(),
            active_recommendations: self.active_recommendations.values().cloned().collect(),
            performance_history_summary: self.summarize_performance_history(),
            optimization_attempts: self.optimization_attempts,
        }
    }

    /// 汇总性能历史
    fn summarize_performance_history(&self) -> PerformanceHistorySummary {
        if self.performance_history.is_empty() {
            return PerformanceHistorySummary::default();
        }

        let mut cpu_usage_sum = 0.0;
        let mut memory_usage_sum = 0.0;
        let mut count = 0;

        for analysis in &self.performance_history {
            if let Some(cpu) = analysis.metrics.get("cpu_usage") {
                cpu_usage_sum += cpu;
            }
            if let Some(mem) = analysis.metrics.get("memory_usage") {
                memory_usage_sum += mem;
            }
            count += 1;
        }

        PerformanceHistorySummary {
            average_cpu_usage: if count > 0 {
                cpu_usage_sum / count as f64
            } else {
                0.0
            },
            average_memory_usage: if count > 0 {
                memory_usage_sum / count as f64
            } else {
                0.0
            },
            total_analyses: count,
            workload_pattern_distribution: self.analyze_workload_distribution(),
        }
    }

    /// 分析工作负载分布
    fn analyze_workload_distribution(&self) -> HashMap<WorkloadPattern, usize> {
        let mut distribution = HashMap::new();

        for analysis in &self.performance_history {
            *distribution
                .entry(analysis.workload_pattern.clone())
                .or_insert(0) += 1;
        }

        distribution
    }
}

/// 优化报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationReport {
    pub timestamp: u64,
    pub hardware_profile: HardwareProfile,
    pub current_state: OptimizationState,
    pub active_recommendations: Vec<OptimizationRecommendation>,
    pub performance_history_summary: PerformanceHistorySummary,
    pub optimization_attempts: usize,
}

/// 性能历史汇总
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceHistorySummary {
    pub average_cpu_usage: f64,
    pub average_memory_usage: f64,
    pub total_analyses: usize,
    pub workload_pattern_distribution: HashMap<WorkloadPattern, usize>,
}

impl Default for PerformanceHistorySummary {
    fn default() -> Self {
        Self {
            average_cpu_usage: 0.0,
            average_memory_usage: 0.0,
            total_analyses: 0,
            workload_pattern_distribution: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use vm_monitor::MonitorConfig;

    #[tokio::test]
    async fn test_adaptive_optimizer_creation() {
        let config = AdaptiveOptimizerConfig::default();
        let performance_monitor = Arc::new(PerformanceMonitor::new(MonitorConfig::default()));
        let optimizer = AdaptiveOptimizer::new(config, performance_monitor);

        assert_eq!(optimizer.optimization_attempts, 0);
        assert!(optimizer.active_recommendations.is_empty());
    }

    #[test]
    fn test_workload_pattern_identification() {
        let config = AdaptiveOptimizerConfig::default();
        let performance_monitor = Arc::new(PerformanceMonitor::new(MonitorConfig::default()));
        let optimizer = AdaptiveOptimizer::new(config, performance_monitor);

        // CPU密集型
        let mut metrics = HashMap::new();
        metrics.insert("cpu_usage".to_string(), 85.0);
        metrics.insert("jit_compile_time".to_string(), 2000.0);
        let pattern = optimizer.identify_workload_pattern(&metrics);
        assert_eq!(pattern, WorkloadPattern::CpuIntensive);

        // 内存密集型
        let mut metrics = HashMap::new();
        metrics.insert("memory_usage".to_string(), 90.0);
        let pattern = optimizer.identify_workload_pattern(&metrics);
        assert_eq!(pattern, WorkloadPattern::MemoryIntensive);
    }

    #[test]
    fn test_bottleneck_identification() {
        let config = AdaptiveOptimizerConfig::default();
        let performance_monitor = Arc::new(PerformanceMonitor::new(MonitorConfig::default()));
        let optimizer = AdaptiveOptimizer::new(config, performance_monitor);

        let mut metrics = HashMap::new();
        metrics.insert("cpu_usage".to_string(), 95.0);
        metrics.insert("tlb_hit_rate".to_string(), 0.7);

        let bottlenecks = optimizer.identify_bottlenecks(&metrics);
        assert!(bottlenecks.contains(&"High CPU usage".to_string()));
        assert!(bottlenecks.contains(&"Low TLB hit rate".to_string()));
    }

    #[test]
    fn test_hardware_profile_detection() {
        let profile = AdaptiveOptimizer::detect_hardware_profile();
        assert!(profile.cpu_cores > 0);
        assert!(profile.memory_gb > 0);
    }
}
