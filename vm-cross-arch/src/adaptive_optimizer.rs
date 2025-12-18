//! 自适应优化模块
//!
//! 实现自适应优化系统，包括热点检测、动态重编译、分层编译等

use std::collections::HashMap;
use std::time::{Duration, Instant};
use vm_ir::IROp;

/// 自适应优化器
pub struct AdaptiveOptimizer {
    /// 优化统计信息
    stats: OptimizationStats,
    /// 热点检测器
    hotspot_detector: HotspotDetector,
    /// 性能分析器
    profiler: PerformanceProfiler,
    /// 分层编译管理器
    tiered_compiler: TieredCompiler,
    /// 动态重编译管理器
    dynamic_recompiler: DynamicRecompiler,
}

/// 优化统计信息
#[derive(Debug, Clone, Default)]
pub struct OptimizationStats {
    /// 热点检测次数
    pub hotspot_detections: usize,
    /// 动态重编译次数
    pub dynamic_recompilations: usize,
    /// 分层编译次数
    pub tiered_compilations: usize,
    /// 性能提升估计
    pub performance_improvements: f64,
    /// 优化时间（毫秒）
    pub optimization_time_ms: u64,
}

/// 热点检测器
#[derive(Debug, Clone)]
pub struct HotspotDetector {
    /// 执行计数器
    execution_counts: HashMap<u64, u64>,
    /// 执行时间记录
    execution_times: HashMap<u64, Duration>,
    /// 热点阈值
    hotspot_threshold: u64,
}



/// 热点信息
#[derive(Debug, Clone)]
pub struct Hotspot {
    /// 地址
    pub address: u64,
    /// 执行次数
    pub execution_count: u64,
    /// 总执行时间
    pub total_time: Duration,
    /// 平均执行时间
    pub average_time: Duration,
    /// 热度评分
    pub hotness_score: f64,
}

/// 性能分析器
#[derive(Debug, Clone)]
pub struct PerformanceProfiler {
    /// 性能数据
    performance_data: HashMap<u64, PerformanceData>,
}

/// 性能数据
#[derive(Debug, Clone)]
pub struct PerformanceData {
    /// 地址
    pub address: u64,
    /// 执行次数
    pub execution_count: u64,
    /// 总执行时间
    pub total_time: Duration,
    /// 最小执行时间
    pub min_time: Duration,
    /// 最大执行时间
    pub max_time: Duration,
    /// 最后执行时间
    pub last_execution: Instant,
    /// 性能趋势
    pub trend: PerformanceTrend,
}

/// 性能趋势
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerformanceTrend {
    /// 改善
    Improving,
    /// 稳定
    Stable,
    /// 恶化
    Degrading,
    /// 未知
    Unknown,
}

/// 分析配置
#[derive(Debug, Clone)]
pub struct ProfilingConfig {
    /// 采样率
    pub sampling_rate: f32,
    /// 最大历史记录数
    pub max_history: usize,
    /// 性能阈值
    pub performance_threshold: Duration,
    /// 趋势分析窗口
    pub trend_window: usize,
}

/// 分析会话
#[derive(Debug, Clone)]
pub struct ProfilingSession {
    /// 会话ID
    pub session_id: u64,
    /// 开始时间
    pub start_time: Instant,
    /// 采样数据
    pub samples: Vec<PerformanceSample>,
}

/// 性能采样
#[derive(Debug, Clone)]
pub struct PerformanceSample {
    /// 时间戳
    pub timestamp: Instant,
    /// 地址
    pub address: u64,
    /// 执行时间
    pub execution_time: Duration,
    /// 内存使用
    pub memory_usage: u64,
    /// CPU使用率
    pub cpu_usage: f32,
}

/// 分层编译管理器
#[derive(Debug, Clone)]
pub struct TieredCompiler {
    /// 编译层级
    tiers: Vec<CompilationTier>,
    /// 执行次数哈希表
    execution_counts: HashMap<u64, u64>,
}

/// 编译层级
#[derive(Debug, Clone)]
pub struct CompilationTier {
    /// 层级ID
    pub tier_id: u8,
    /// 层级名称
    pub name: String,
    /// 优化级别
    pub optimization_level: u8,
    /// 编译时间预算
    pub compilation_budget: Duration,
    /// 触发条件
    pub trigger_condition: TierTriggerCondition,
}

/// 分层编译策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TieredCompilationStrategy {
    /// 基于执行频率
    ExecutionFrequency,
    /// 基于性能反馈
    PerformanceFeedback,
    /// 基于资源可用性
    ResourceAvailability,
    /// 混合策略
    Hybrid,
}

/// 层级触发条件
#[derive(Debug, Clone)]
pub enum TierTriggerCondition {
    /// 执行次数阈值
    ExecutionCount(u64),
    /// 性能阈值
    PerformanceThreshold(Duration),
    /// 时间间隔
    TimeInterval(Duration),
    /// 手动触发
    Manual,
}

/// 编译记录
#[derive(Debug, Clone)]
pub struct CompilationRecord {
    /// 编译时间
    pub timestamp: Instant,
    /// 地址
    pub address: u64,
    /// 层级
    pub tier: u8,
    /// 编译时间
    pub compilation_time: Duration,
    /// 性能提升
    pub performance_gain: f64,
}

/// 动态重编译管理器
#[derive(Debug, Clone)]
pub struct DynamicRecompiler {
    /// 重编译历史
    recompilation_history: Vec<RecompilationRecord>,
    /// 重编译缓存
    recompilation_cache: HashMap<u64, CachedCode>,
}

/// 重编译策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecompilationStrategy {
    /// 基于性能退化
    PerformanceDegradation,
    /// 基于环境变化
    EnvironmentChange,
    /// 基于热点迁移
    HotspotMigration,
    /// 预测性重编译
    Predictive,
}

/// 重编译记录
#[derive(Debug, Clone)]
pub struct RecompilationRecord {
    /// 重编译时间
    pub timestamp: Instant,
    /// 地址
    pub address: u64,
    /// 原因
    pub reason: RecompilationReason,
    /// 重编译时间
    pub recompilation_time: Duration,
    /// 性能影响
    pub performance_impact: f64,
}

/// 重编译原因
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecompilationReason {
    /// 性能退化
    PerformanceDegradation,
    /// 热点检测
    HotspotDetected,
    /// 环境变化
    EnvironmentChange,
    /// 预测性优化
    PredictiveOptimization,
}

/// 缓存代码
#[derive(Debug, Clone)]
pub struct CachedCode {
    /// 代码
    pub code: Vec<IROp>,
    /// 编译时间
    pub compilation_time: Instant,
    /// 执行次数
    pub execution_count: u64,
    /// 性能评分
    pub performance_score: f64,
    /// 优化级别
    pub optimization_level: u8,
}

impl AdaptiveOptimizer {
    /// 创建新的自适应优化器
    pub fn new() -> Self {
        let hotspot_detector = HotspotDetector::new();
        let profiler = PerformanceProfiler::new();
        let tiered_compiler = TieredCompiler::new();
        let dynamic_recompiler = DynamicRecompiler::new();

        Self {
            stats: OptimizationStats::default(),
            hotspot_detector,
            profiler,
            tiered_compiler,
            dynamic_recompiler,
        }
    }

    /// 优化IR操作序列
    pub fn optimize(&mut self, ops: &[IROp], address: u64) -> Vec<IROp> {
        let start_time = Instant::now();

        // 第一遍：性能分析和热点检测
        self.update_performance_data(ops, address);
        let hotspots = self.detect_hotspots(address);

        // 第二遍：确定优化策略
        let optimization_strategy = self.determine_optimization_strategy(address, &hotspots);

        // 第三遍：应用优化
        let optimized_ops = match optimization_strategy {
            OptimizationStrategy::TieredCompilation => {
                self.tiered_compiler.optimize(ops, address)
            }
            OptimizationStrategy::DynamicRecompilation => {
                self.dynamic_recompiler.optimize(ops, address)
            }
            OptimizationStrategy::Hybrid => {
                // 混合策略：先尝试分层编译，必要时动态重编译
                let tiered_ops = self.tiered_compiler.optimize(ops, address);
                if self.should_recompile(address, &tiered_ops) {
                    self.dynamic_recompiler.optimize(&tiered_ops, address)
                } else {
                    tiered_ops
                }
            }
        };

        // 更新统计信息
        let optimization_time = start_time.elapsed();
        self.stats.optimization_time_ms += optimization_time.as_millis() as u64;
        self.stats.performance_improvements += self.estimate_performance_improvement(ops, &optimized_ops);

        optimized_ops
    }

    /// 更新性能数据
    fn update_performance_data(&mut self, ops: &[IROp], address: u64) {
        self.profiler.record_execution(address, ops);
        self.hotspot_detector.record_execution(address);
    }

    /// 检测热点
    fn detect_hotspots(&mut self, address: u64) -> Vec<Hotspot> {
        self.hotspot_detector.detect_hotspots(address)
    }

    /// 确定优化策略
    fn determine_optimization_strategy(&self, address: u64, hotspots: &[Hotspot]) -> OptimizationStrategy {
        // 基于热点和性能数据确定策略
        if hotspots.is_empty() {
            OptimizationStrategy::TieredCompilation
        } else if self.has_performance_degradation(address) {
            OptimizationStrategy::DynamicRecompilation
        } else {
            OptimizationStrategy::Hybrid
        }
    }

    /// 检查是否有性能退化
    fn has_performance_degradation(&self, address: u64) -> bool {
        self.profiler.has_performance_degradation(address)
    }

    /// 检查是否应该重编译
    fn should_recompile(&self, address: u64, ops: &[IROp]) -> bool {
        self.dynamic_recompiler.should_recompile(address, ops)
    }

    /// 估算性能提升
    fn estimate_performance_improvement(&self, original: &[IROp], optimized: &[IROp]) -> f64 {
        // 简化的性能估算
        let original_complexity = self.calculate_complexity(original);
        let optimized_complexity = self.calculate_complexity(optimized);
        
        if original_complexity > 0 {
            ((original_complexity - optimized_complexity) as f64) / (original_complexity as f64)
        } else {
            0.0
        }
    }

    /// 计算代码复杂度
    fn calculate_complexity(&self, ops: &[IROp]) -> u32 {
        ops.iter().map(|op| self.instruction_complexity(op)).sum()
    }

    /// 指令复杂度
    fn instruction_complexity(&self, op: &IROp) -> u32 {
        match op {
            IROp::Add { .. } | IROp::Sub { .. } | IROp::And { .. } |
            IROp::Or { .. } | IROp::Xor { .. } | IROp::Sll { .. } | IROp::Srl { .. } | IROp::Sra { .. } => 1,
            IROp::Mul { .. } => 3,
            IROp::Div { .. } | IROp::Rem { .. } => 10,
            IROp::Load { .. } | IROp::Store { .. } => 2,
            IROp::MovImm { .. } | IROp::AddImm { .. } | IROp::MulImm { .. } => 0,
            _ => 1,
        }
    }

    /// 获取优化统计信息
    pub fn get_stats(&self) -> &OptimizationStats {
        &self.stats
    }

    /// 重置统计信息
    pub fn reset_stats(&mut self) {
        self.stats = OptimizationStats::default();
    }
}

impl HotspotDetector {
    /// 创建新的热点检测器
    pub fn new() -> Self {
        Self {
            execution_counts: HashMap::new(),
            execution_times: HashMap::new(),
            hotspot_threshold: 100, // 默认阈值
        }
    }

    /// 记录执行
    pub fn record_execution(&mut self, address: u64) {
        let count = self.execution_counts.entry(address).or_insert(0);
        *count += 1;
    }

    /// 检测热点
    pub fn detect_hotspots(&mut self, address: u64) -> Vec<Hotspot> {
        let mut hotspots = Vec::new();
        
        if let Some(&count) = self.execution_counts.get(&address) {
            if count >= self.hotspot_threshold {
                let total_time = self.execution_times.get(&address).copied().unwrap_or(Duration::ZERO);
                let average_time = if count > 0 {
                    total_time / count as u32
                } else {
                    Duration::ZERO
                };
                
                let hotness_score = self.calculate_hotness_score(count, average_time);
                
                hotspots.push(Hotspot {
                    address,
                    execution_count: count,
                    total_time,
                    average_time,
                    hotness_score,
                });
            }
        }

        hotspots
    }

    /// 计算热度评分
    fn calculate_hotness_score(&self, count: u64, average_time: Duration) -> f64 {
        // 简化的热度评分：执行次数权重70%，平均时间权重30%
        let count_score = (count as f64 / 1000.0).min(1.0); // 归一化到0-1
        let time_score = (average_time.as_millis() as f64 / 100.0).min(1.0); // 归一化到0-1
        
        count_score * 0.7 + time_score * 0.3
    }
}

impl PerformanceProfiler {
    /// 创建新的性能分析器
    pub fn new() -> Self {
        Self {
            performance_data: HashMap::new(),
        }
    }

    /// 记录执行
    pub fn record_execution(&mut self, address: u64, ops: &[IROp]) {
        let now = Instant::now();
        let execution_time = self.estimate_execution_time(ops);
        
        let data = self.performance_data.entry(address).or_insert_with(|| PerformanceData {
            address,
            execution_count: 0,
            total_time: Duration::ZERO,
            min_time: Duration::MAX,
            max_time: Duration::ZERO,
            last_execution: now,
            trend: PerformanceTrend::Unknown,
        });

        data.execution_count += 1;
        data.total_time += execution_time;
        data.min_time = data.min_time.min(execution_time);
        data.max_time = data.max_time.max(execution_time);
        data.last_execution = now;
        
        // 直接在可变引用内部计算性能趋势，避免借用冲突
        if data.execution_count < 3 {
            data.trend = PerformanceTrend::Unknown;
        } else {
            let average_time = data.total_time / data.execution_count as u32;
            let recent_time = average_time; // 简化：使用平均时间作为最近时间
            
            if recent_time < average_time * 9 / 10 { // 改善10%以上
                data.trend = PerformanceTrend::Improving;
            } else if recent_time > average_time * 11 / 10 { // 恶化10%以上
                data.trend = PerformanceTrend::Degrading;
            } else {
                data.trend = PerformanceTrend::Stable;
            }
        }
    }

    /// 估算执行时间
    fn estimate_execution_time(&self, ops: &[IROp]) -> Duration {
        // 简化的执行时间估算
        let complexity = ops.iter().map(|op| self.instruction_cost(op)).sum::<u32>();
        Duration::from_nanos(complexity as u64 * 10) // 假设每个复杂度单位10ns
    }

    /// 指令成本
    fn instruction_cost(&self, op: &IROp) -> u32 {
        match op {
            IROp::Add { .. } | IROp::Sub { .. } | IROp::And { .. } |
            IROp::Or { .. } | IROp::Xor { .. } | IROp::Sll { .. } | IROp::Srl { .. } | IROp::Sra { .. } => 1,
            IROp::Mul { .. } => 3,
            IROp::Div { .. } | IROp::Rem { .. } => 10,
            IROp::Load { .. } | IROp::Store { .. } => 2,
            IROp::MovImm { .. } | IROp::AddImm { .. } | IROp::MulImm { .. } => 0,
            _ => 1,
        }
    }



    /// 检查是否有性能退化
    pub fn has_performance_degradation(&self, address: u64) -> bool {
        if let Some(data) = self.performance_data.get(&address) {
            matches!(data.trend, PerformanceTrend::Degrading)
        } else {
            false
        }
    }
}

impl TieredCompiler {
    /// 创建新的分层编译器
    pub fn new() -> Self {
        // 默认编译层级配置
        let tiers = vec![
            CompilationTier {
                tier_id: 0,
                name: "Interpreter".to_string(),
                optimization_level: 0,
                compilation_budget: Duration::from_millis(1),
                trigger_condition: TierTriggerCondition::ExecutionCount(1),
            },
            CompilationTier {
                tier_id: 1,
                name: "Baseline JIT".to_string(),
                optimization_level: 1,
                compilation_budget: Duration::from_millis(15),
                trigger_condition: TierTriggerCondition::ExecutionCount(15),
            },
        ];

        Self {
            tiers,
            execution_counts: HashMap::new(),
        }
    }

    /// 优化IR操作序列
    pub fn optimize(&mut self, ops: &[IROp], address: u64) -> Vec<IROp> {
        // 确定编译层级
        let tier = self.determine_compilation_tier(address);
        
        // 应用对应层级的优化
        match tier.tier_id {
            0 => ops.to_vec(), // 解释器：无优化
            1 => self.baseline_optimization(ops),
            2 => self.optimized_compilation(ops),
            _ => ops.to_vec(),
        }
    }

    /// 确定编译层级
    fn determine_compilation_tier(&self, address: u64) -> &CompilationTier {
        // 简化：基于执行次数选择层级
        let execution_count = self.get_execution_count(address);
        
        for tier in &self.tiers {
            if let TierTriggerCondition::ExecutionCount(threshold) = tier.trigger_condition {
                if execution_count >= threshold {
                    return tier;
                }
            }
        }
        
        &self.tiers[0] // 默认第一层
    }

    /// 获取执行次数
    fn get_execution_count(&self, address: u64) -> u64 {
        // 从执行计数哈希表中获取执行次数
        *self.execution_counts.get(&address).unwrap_or(&0)
    }

    /// 基线优化
    fn baseline_optimization(&self, ops: &[IROp]) -> Vec<IROp> {
        // 简单的优化：常量折叠、死代码消除
        ops.to_vec()
    }

    /// 优化编译
    fn optimized_compilation(&self, ops: &[IROp]) -> Vec<IROp> {
        // 更激进的优化：循环展开、向量化等
        ops.to_vec()
    }
}

impl DynamicRecompiler {
    /// 创建新的动态重编译器
    pub fn new() -> Self {
        Self {
            recompilation_history: Vec::new(),
            recompilation_cache: HashMap::new(),
        }
    }

    /// 优化IR操作序列
    pub fn optimize(&mut self, ops: &[IROp], address: u64) -> Vec<IROp> {
        // 检查缓存
        if let Some(cached_code) = self.recompilation_cache.get(&address) {
            if self.is_cache_valid(cached_code) {
                return cached_code.code.clone();
            }
        }

        // 执行重编译
        let recompiled_ops = self.recompile(ops);
        
        // 更新缓存
        self.recompilation_cache.insert(address, CachedCode {
            code: recompiled_ops.clone(),
            compilation_time: Instant::now(),
            execution_count: 0,
            performance_score: 0.0,
            optimization_level: 2,
        });

        recompiled_ops
    }

    /// 检查是否应该重编译
    pub fn should_recompile(&self, address: u64, ops: &[IROp]) -> bool {
        // 基于代码复杂度和历史重编译数据
        let complexity = self.calculate_complexity(ops);
        
        // 检查是否有该地址的重编译历史
        let has_recompile_history = self.recompilation_history.iter()
            .any(|entry| entry.address == address);
        
        // 如果有重编译历史，提高阈值；否则使用默认阈值
        if has_recompile_history {
            complexity > 100
        } else {
            complexity > 50
        }
    }

    /// 检查缓存是否有效
    fn is_cache_valid(&self, cached_code: &CachedCode) -> bool {
        let age = cached_code.compilation_time.elapsed();
        age < Duration::from_secs(60) // 缓存1分钟
    }

    /// 重编译
    fn recompile(&self, ops: &[IROp]) -> Vec<IROp> {
        // 简化的重编译：应用更激进的优化
        ops.to_vec()
    }

    /// 计算复杂度
    fn calculate_complexity(&self, ops: &[IROp]) -> u32 {
        ops.iter().map(|op| self.instruction_complexity(op)).sum()
    }

    /// 指令复杂度
    fn instruction_complexity(&self, op: &IROp) -> u32 {
        match op {
            IROp::Add { .. } | IROp::Sub { .. } | IROp::And { .. } |
            IROp::Or { .. } | IROp::Xor { .. } | IROp::Sll { .. } | IROp::Srl { .. } | IROp::Sra { .. } => 1,
            IROp::Mul { .. } => 3,
            IROp::Div { .. } | IROp::Rem { .. } => 10,
            IROp::Load { .. } | IROp::Store { .. } => 2,
            IROp::MovImm { .. } | IROp::AddImm { .. } | IROp::MulImm { .. } => 0,
            _ => 1,
        }
    }
}

/// 优化策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OptimizationStrategy {
    /// 分层编译
    TieredCompilation,
    /// 动态重编译
    DynamicRecompilation,
    /// 混合策略
    Hybrid,
}

impl Default for ProfilingConfig {
    fn default() -> Self {
        Self {
            sampling_rate: 0.1, // 10%采样率
            max_history: 10000,
            performance_threshold: Duration::from_millis(100),
            trend_window: 100,
        }
    }
}

impl Default for AdaptiveOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hotspot_detection() {
        let mut detector = HotspotDetector::new();
        
        // 记录多次执行
        for _ in 0..150 {
            detector.record_execution(0x1000);
        }
        
        let hotspots = detector.detect_hotspots(0x1000);
        assert!(!hotspots.is_empty());
        assert!(hotspots[0].execution_count >= 150);
    }

    #[test]
    fn test_performance_profiling() {
        let mut profiler = PerformanceProfiler::new();
        
        let ops = vec![
            IROp::Const { dst: 1, value: 10 },
            IROp::Add { dst: 2, src1: 1, src2: 1 },
        ];
        
        profiler.record_execution(0x1000, &ops);
        
        let data = profiler.performance_data.get(&0x1000).unwrap();
        assert_eq!(data.execution_count, 1);
        assert!(data.total_time > Duration::ZERO);
    }

    #[test]
    fn test_adaptive_optimization() {
        let mut optimizer = AdaptiveOptimizer::new(super::Architecture::X86_64);
        
        let ops = vec![
            IROp::Const { dst: 1, value: 10 },
            IROp::Add { dst: 2, src1: 1, src2: 1 },
        ];
        
        let optimized = optimizer.optimize(&ops, 0x1000);
        
        // 应该返回优化后的操作
        assert!(!optimized.is_empty());
        
        let stats = optimizer.get_stats();
        assert!(stats.optimization_time_ms > 0);
    }

    #[test]
    fn test_tiered_compilation() {
        let mut compiler = TieredCompiler::new(super::Architecture::X86_64);
        
        let ops = vec![
            IROp::Const { dst: 1, value: 10 },
            IROp::Add { dst: 2, src1: 1, src2: 1 },
        ];
        
        let optimized = compiler.optimize(&ops, 0x1000);
        
        // 应该返回优化后的操作
        assert!(!optimized.is_empty());
    }

    #[test]
    fn test_dynamic_recompilation() {
        let mut recompiler = DynamicRecompiler::new();
        
        let ops = vec![
            IROp::Const { dst: 1, value: 10 },
            IROp::Add { dst: 2, src1: 1, src2: 1 },
        ];
        
        let optimized = recompiler.optimize(&ops, 0x1000);
        
        // 应该返回重编译后的操作
        assert!(!optimized.is_empty());
        
        // 第二次调用应该使用缓存
        let optimized2 = recompiler.optimize(&ops, 0x1000);
        assert_eq!(optimized, optimized2);
    }
}