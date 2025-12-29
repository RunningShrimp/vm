//! JIT引擎性能分析器
//!
//! 本模块提供JIT引擎的性能分析功能，包括编译时间分析、缓存效率分析、
//! 热点检测分析和优化效果分析等。

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use serde::{Serialize, Deserialize};
use vm_core::GuestAddr;

use crate::core::{JITEngine, JITCompilationStats};



/// 性能分析器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAnalyzerConfig {
    /// 是否启用详细分析
    pub enable_detailed_analysis: bool,
    /// 性能数据历史记录数量
    pub history_size: usize,
    /// 报告生成间隔（秒）
    pub report_interval_secs: u64,
    /// 是否启用实时监控
    pub enable_realtime_monitoring: bool,
    /// 性能阈值配置
    pub performance_thresholds: PerformanceThresholds,
}

impl Default for PerformanceAnalyzerConfig {
    fn default() -> Self {
        Self {
            enable_detailed_analysis: true,
            history_size: 1000,
            report_interval_secs: 60,
            enable_realtime_monitoring: true,
            performance_thresholds: PerformanceThresholds::default(),
        }
    }
}

/// 性能阈值配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceThresholds {
    /// 编译时间警告阈值（毫秒）
    pub compilation_time_warning_ms: u64,
    /// 编译时间错误阈值（毫秒）
    pub compilation_time_error_ms: u64,
    /// 缓存命中率警告阈值（百分比）
    pub cache_hit_rate_warning_pct: f64,
    /// 缓存命中率错误阈值（百分比）
    pub cache_hit_rate_error_pct: f64,
    /// 内存使用警告阈值（MB）
    pub memory_usage_warning_mb: u64,
    /// 内存使用错误阈值（MB）
    pub memory_usage_error_mb: u64,
}

impl Default for PerformanceThresholds {
    fn default() -> Self {
        Self {
            compilation_time_warning_ms: 100,
            compilation_time_error_ms: 1000,
            cache_hit_rate_warning_pct: 70.0,
            cache_hit_rate_error_pct: 50.0,
            memory_usage_warning_mb: 100,
            memory_usage_error_mb: 500,
        }
    }
}

/// 性能数据点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceDataPoint {
    /// 时间戳
    pub timestamp: std::time::SystemTime,
    /// 编译统计
    pub compilation_stats: JITCompilationStats,
    /// 缓存统计
    pub cache_stats: CachePerformanceStats,
    /// 热点统计
    pub hotspot_stats: HotspotStats,
    /// 内存使用统计
    pub memory_stats: MemoryStats,
    /// 系统资源统计
    pub system_stats: SystemStats,
}

/// 缓存性能统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachePerformanceStats {
    /// 缓存命中次数
    pub hits: u64,
    /// 缓存未命中次数
    pub misses: u64,
    /// 缓存大小（字节）
    pub size_bytes: usize,
    /// 缓存条目数
    pub entries: usize,
    /// 缓存清理次数
    pub evictions: u64,
    /// 平均访问时间（纳秒）
    pub avg_access_time_ns: u64,
}

impl Default for CachePerformanceStats {
    fn default() -> Self {
        Self {
            hits: 0,
            misses: 0,
            size_bytes: 0,
            entries: 0,
            evictions: 0,
            avg_access_time_ns: 0,
        }
    }
}

impl CachePerformanceStats {
    /// 计算缓存命中率
    pub fn hit_rate(&self) -> f64 {
        if self.hits + self.misses == 0 {
            0.0
        } else {
            (self.hits as f64) / ((self.hits + self.misses) as f64) * 100.0
        }
    }
}

/// 热点统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotspotStats {
    /// 热点代码块数量
    pub hotspot_count: usize,
    /// 总执行次数
    pub total_executions: u64,
    /// 热点执行次数
    pub hotspot_executions: u64,
    /// 平均热点阈值
    pub avg_threshold: f64,
    /// 热点分布
    pub hotspot_distribution: HashMap<GuestAddr, u32>,
}

impl Default for HotspotStats {
    fn default() -> Self {
        Self {
            hotspot_count: 0,
            total_executions: 0,
            hotspot_executions: 0,
            avg_threshold: 0.0,
            hotspot_distribution: HashMap::new(),
        }
    }
}

impl HotspotStats {
    /// 计算热点集中度
    pub fn concentration(&self) -> f64 {
        if self.total_executions == 0 {
            0.0
        } else {
            (self.hotspot_executions as f64) / (self.total_executions as f64) * 100.0
        }
    }
}

/// 内存使用统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    /// JIT代码内存使用（字节）
    pub code_memory_bytes: usize,
    /// 缓存内存使用（字节）
    pub cache_memory_bytes: usize,
    /// IR内存使用（字节）
    pub ir_memory_bytes: usize,
    /// 其他内存使用（字节）
    pub other_memory_bytes: usize,
    /// 总内存使用（字节）
    pub total_memory_bytes: usize,
    /// 内存碎片率
    pub fragmentation_ratio: f64,
}

impl Default for MemoryStats {
    fn default() -> Self {
        Self {
            code_memory_bytes: 0,
            cache_memory_bytes: 0,
            ir_memory_bytes: 0,
            other_memory_bytes: 0,
            total_memory_bytes: 0,
            fragmentation_ratio: 0.0,
        }
    }
}

impl MemoryStats {
    /// 转换为MB
    pub fn total_memory_mb(&self) -> f64 {
        self.total_memory_bytes as f64 / (1024.0 * 1024.0)
    }
}

/// 系统资源统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStats {
    /// CPU使用率（百分比）
    pub cpu_usage_pct: f64,
    /// 线程数
    pub thread_count: usize,
    /// 上下文切换次数
    pub context_switches: u64,
    /// 系统调用次数
    pub syscalls: u64,
    /// 页面错误次数
    pub page_faults: u64,
}

impl Default for SystemStats {
    fn default() -> Self {
        Self {
            cpu_usage_pct: 0.0,
            thread_count: 0,
            context_switches: 0,
            syscalls: 0,
            page_faults: 0,
        }
    }
}

/// 性能分析报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    /// 报告生成时间
    pub generated_at: std::time::SystemTime,
    /// 分析时间范围
    pub time_range: TimeRange,
    /// 总体性能摘要
    pub summary: PerformanceSummary,
    /// 编译性能分析
    pub compilation_analysis: CompilationAnalysis,
    /// 缓存性能分析
    pub cache_analysis: CacheAnalysis,
    /// 热点分析
    pub hotspot_analysis: HotspotAnalysis,
    /// 内存使用分析
    pub memory_analysis: MemoryAnalysis,
    /// 性能建议
    pub recommendations: Vec<PerformanceRecommendation>,
    /// 性能警告
    pub warnings: Vec<PerformanceWarning>,
}

/// 时间范围
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    /// 开始时间
    pub start: std::time::SystemTime,
    /// 结束时间
    pub end: std::time::SystemTime,
    /// 持续时间
    pub duration: Duration,
}

/// 性能摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSummary {
    /// 总编译次数
    pub total_compilations: u64,
    /// 总编译时间（毫秒）
    pub total_compilation_time_ms: u64,
    /// 平均编译时间（毫秒）
    pub avg_compilation_time_ms: f64,
    /// 缓存命中率
    pub cache_hit_rate: f64,
    /// 热点集中度
    pub hotspot_concentration: f64,
    /// 内存使用（MB）
    pub memory_usage_mb: f64,
    /// 性能评分（0-100）
    pub performance_score: u8,
}

/// 编译性能分析
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompilationAnalysis {
    /// 编译时间分布
    pub compilation_time_distribution: TimeDistribution,
    /// 编译效率趋势
    pub efficiency_trend: TrendAnalysis,
    /// 优化效果分析
    pub optimization_effectiveness: OptimizationAnalysis,
    /// 并行编译效率
    pub parallel_efficiency: ParallelCompilationAnalysis,
}

/// 时间分布统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeDistribution {
    /// 最小值（毫秒）
    pub min_ms: f64,
    /// 最大值（毫秒）
    pub max_ms: f64,
    /// 平均值（毫秒）
    pub avg_ms: f64,
    /// 中位数（毫秒）
    pub median_ms: f64,
    /// 第95百分位（毫秒）
    pub p95_ms: f64,
    /// 第99百分位（毫秒）
    pub p99_ms: f64,
    /// 标准差（毫秒）
    pub std_dev_ms: f64,
}

/// 趋势分析
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendAnalysis {
    /// 趋势方向
    pub direction: TrendDirection,
    /// 变化率（百分比）
    pub change_rate_pct: f64,
    /// 预测值
    pub prediction: Option<f64>,
    /// 置信度
    pub confidence: f64,
}

/// 趋势方向
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrendDirection {
    /// 上升
    Increasing,
    /// 下降
    Decreasing,
    /// 稳定
    Stable,
    /// 波动
    Fluctuating,
}

/// 优化效果分析
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationAnalysis {
    /// 指令减少率
    pub instruction_reduction_rate: f64,
    /// 优化时间占比
    pub optimization_time_ratio: f64,
    /// 各优化阶段效果
    pub stage_effectiveness: HashMap<String, f64>,
}

/// 并行编译分析
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelCompilationAnalysis {
    /// 并行度
    pub parallelism_degree: f64,
    /// 线程利用率
    pub thread_utilization: f64,
    /// 负载均衡度
    pub load_balance: f64,
    /// 并行效率增益
    pub efficiency_gain: f64,
}

/// 缓存性能分析
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheAnalysis {
    /// 缓存效率趋势
    pub efficiency_trend: TrendAnalysis,
    /// 缓存热点分析
    pub hotspot_analysis: CacheHotspotAnalysis,
    /// 缓存大小优化建议
    pub size_optimization: CacheSizeOptimization,
}

/// 缓存热点分析
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheHotspotAnalysis {
    /// 热点条目
    pub hot_entries: Vec<CacheHotspotEntry>,
    /// 访问模式
    pub access_pattern: AccessPattern,
    /// 局部性分析
    pub locality_analysis: LocalityAnalysis,
}

/// 缓存热点条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheHotspotEntry {
    /// 地址
    pub address: GuestAddr,
    /// 访问次数
    pub access_count: u64,
    /// 访问频率
    pub access_frequency: f64,
    /// 最后访问时间
    pub last_access: std::time::SystemTime,
}

/// 访问模式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccessPattern {
    /// 顺序访问
    Sequential,
    /// 随机访问
    Random,
    /// 局部访问
    Localized,
    /// 混合访问
    Mixed,
}

/// 局部性分析
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalityAnalysis {
    /// 时间局部性
    pub temporal_locality: f64,
    /// 空间局部性
    pub spatial_locality: f64,
}

/// 缓存大小优化
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheSizeOptimization {
    /// 当前大小
    pub current_size_mb: f64,
    /// 建议大小
    pub recommended_size_mb: f64,
    /// 预期性能提升
    pub expected_improvement: f64,
}

/// 热点分析
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotspotAnalysis {
    /// 热点分布
    pub distribution: HotspotDistribution,
    /// 热点演化
    pub evolution: HotspotEvolution,
    /// 热点预测
    pub prediction: HotspotPrediction,
}

/// 热点分布
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotspotDistribution {
    /// 热点密度
    pub density: f64,
    /// 热点聚类
    pub clusters: Vec<HotspotCluster>,
}

/// 热点聚类
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotspotCluster {
    /// 聚类中心
    pub center: GuestAddr,
    /// 聚类半径
    pub radius: usize,
    /// 聚类大小
    pub size: usize,
    /// 聚类强度
    pub intensity: f64,
}

/// 热点演化
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotspotEvolution {
    /// 新增热点
    pub new_hotspots: Vec<GuestAddr>,
    /// 消失热点
    pub disappeared_hotspots: Vec<GuestAddr>,
    /// 迁移热点
    pub migrated_hotspots: Vec<(GuestAddr, GuestAddr)>,
}

/// 热点预测
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotspotPrediction {
    /// 潜在热点
    pub potential_hotspots: Vec<PotentialHotspot>,
    /// 预测置信度
    pub confidence: f64,
}

/// 潜在热点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PotentialHotspot {
    /// 地址
    pub address: GuestAddr,
    /// 预测概率
    pub probability: f64,
    /// 预测时间
    pub predicted_time: std::time::SystemTime,
}

/// 内存使用分析
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryAnalysis {
    /// 内存使用趋势
    pub usage_trend: TrendAnalysis,
    /// 内存分布
    pub distribution: MemoryDistribution,
    /// 内存碎片分析
    pub fragmentation_analysis: FragmentationAnalysis,
    /// 内存优化建议
    pub optimization_suggestions: Vec<MemoryOptimizationSuggestion>,
}

/// 内存分布
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryDistribution {
    /// 代码内存占比
    pub code_ratio: f64,
    /// 缓存内存占比
    pub cache_ratio: f64,
    /// IR内存占比
    pub ir_ratio: f64,
    /// 其他内存占比
    pub other_ratio: f64,
}

/// 内存碎片分析
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FragmentationAnalysis {
    /// 外部碎片率
    pub external_fragmentation: f64,
    /// 内部碎片率
    pub internal_fragmentation: f64,
    /// 碎片分布
    pub fragment_distribution: HashMap<usize, u64>,
}

/// 内存优化建议
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryOptimizationSuggestion {
    /// 建议类型
    pub suggestion_type: MemoryOptimizationType,
    /// 预期收益
    pub expected_benefit: f64,
    /// 实施难度
    pub implementation_difficulty: ImplementationDifficulty,
    /// 描述
    pub description: String,
}

/// 内存优化类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryOptimizationType {
    /// 缓存大小调整
    CacheSizeAdjustment,
    /// 内存池优化
    MemoryPoolOptimization,
    /// 垃圾回收优化
    GarbageCollectionOptimization,
    /// 数据结构优化
    DataStructureOptimization,
}

/// 实施难度
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImplementationDifficulty {
    /// 简单
    Easy,
    /// 中等
    Medium,
    /// 困难
    Hard,
}

/// 性能建议
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceRecommendation {
    /// 建议类型
    pub recommendation_type: RecommendationType,
    /// 优先级
    pub priority: RecommendationPriority,
    /// 描述
    pub description: String,
    /// 预期收益
    pub expected_benefit: String,
    /// 实施步骤
    pub implementation_steps: Vec<String>,
}

/// 建议类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationType {
    /// 配置优化
    ConfigurationOptimization,
    /// 算法优化
    AlgorithmOptimization,
    /// 缓存优化
    CacheOptimization,
    /// 内存优化
    MemoryOptimization,
    /// 并行优化
    ParallelOptimization,
}

/// 建议优先级
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationPriority {
    /// 低
    Low,
    /// 中
    Medium,
    /// 高
    High,
    /// 紧急
    Critical,
}

/// 性能警告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceWarning {
    /// 警告级别
    pub level: WarningLevel,
    /// 警告类型
    pub warning_type: WarningType,
    /// 消息
    pub message: String,
    /// 时间戳
    pub timestamp: std::time::SystemTime,
    /// 建议操作
    pub suggested_actions: Vec<String>,
}

/// 警告级别
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WarningLevel {
    /// 信息
    Info,
    /// 警告
    Warning,
    /// 错误
    Error,
    /// 严重
    Critical,
}

/// 警告类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WarningType {
    /// 性能下降
    PerformanceDegradation,
    /// 内存泄漏
    MemoryLeak,
    /// 缓存效率低
    LowCacheEfficiency,
    /// 编译时间过长
    LongCompilationTime,
    /// 资源耗尽
    ResourceExhaustion,
}

/// JIT性能分析器
pub struct JITPerformanceAnalyzer {
    /// 配置
    config: PerformanceAnalyzerConfig,
    /// 性能数据历史
    performance_history: Arc<Mutex<VecDeque<PerformanceDataPoint>>>,
    /// 当前数据点
    current_data: Arc<Mutex<PerformanceDataPoint>>,
    /// 分析开始时间
    analysis_start_time: Instant,
    /// 最后报告时间
    last_report_time: Arc<Mutex<Instant>>,
}

impl JITPerformanceAnalyzer {
    /// 创建新的性能分析器
    pub fn new(config: PerformanceAnalyzerConfig) -> Self {
        let now = std::time::SystemTime::now();
        let current_data = PerformanceDataPoint {
            timestamp: now,
            compilation_stats: JITCompilationStats::default(),
            cache_stats: CachePerformanceStats::default(),
            hotspot_stats: HotspotStats::default(),
            memory_stats: MemoryStats::default(),
            system_stats: SystemStats::default(),
        };

        Self {
            config,
            performance_history: Arc::new(Mutex::new(VecDeque::new())),
            current_data: Arc::new(Mutex::new(current_data)),
            analysis_start_time: Instant::now(),
            last_report_time: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// 收集性能数据
    pub fn collect_performance_data(&self, jit_engine: &JITEngine) -> Result<(), String> {
        let mut current_data = self.current_data.lock().map_err(|e| e.to_string())?;
        
        // 更新时间戳
        current_data.timestamp = std::time::SystemTime::now();
        
        // 收集编译统计
        current_data.compilation_stats = jit_engine.get_compilation_stats();
        
        // 收集缓存统计
        let cache_stats = jit_engine.get_cache_stats();
        current_data.cache_stats = CachePerformanceStats {
            hits: cache_stats.hits,
            misses: cache_stats.misses,
            size_bytes: cache_stats.current_size,
            entries: cache_stats.entry_count,
            evictions: cache_stats.removals,
            avg_access_time_ns: 0, // 简化实现，实际应该测量访问时间
        };
        
        // 收集内存统计
        current_data.memory_stats = self.collect_memory_stats(jit_engine)?;
        
        // 收集系统统计
        current_data.system_stats = self.collect_system_stats()?;
        
        Ok(())
    }

    /// 保存当前数据点到历史
    pub fn save_data_point(&self) -> Result<(), String> {
        let current_data = self.current_data.lock().map_err(|e| e.to_string())?.clone();
        let mut history = self.performance_history.lock().map_err(|e| e.to_string())?;
        
        // 添加到历史
        history.push_back(current_data);
        
        // 限制历史大小
        while history.len() > self.config.history_size {
            history.pop_front();
        }
        
        Ok(())
    }

    /// 生成性能报告
    pub fn generate_report(&self) -> Result<PerformanceReport, String> {
        let history_guard = self.performance_history.lock().map_err(|e| e.to_string())?;
        let history: Vec<PerformanceDataPoint> = history_guard.iter().cloned().collect();
        drop(history_guard);
        
        if history.is_empty() {
            return Err("No performance data available".to_string());
        }

        let start_time = history[0].timestamp;
        let end_time = history[history.len() - 1].timestamp;
        let duration = end_time.duration_since(start_time)
            .map_err(|e| format!("Invalid time range: {}", e))?;

        let time_range = TimeRange {
            start: start_time,
            end: end_time,
            duration,
        };

        // 生成各部分分析
        let summary = self.generate_summary(&history)?;
        let compilation_analysis = self.analyze_compilation_performance(&history)?;
        let cache_analysis = self.analyze_cache_performance(&history)?;
        let hotspot_analysis = self.analyze_hotspot_performance(&history)?;
        let memory_analysis = self.analyze_memory_performance(&history)?;
        let recommendations = self.generate_recommendations(&summary, &compilation_analysis, &cache_analysis, &memory_analysis)?;
        let warnings = self.generate_warnings(&summary, &compilation_analysis, &cache_analysis, &memory_analysis)?;

        Ok(PerformanceReport {
            generated_at: std::time::SystemTime::now(),
            time_range,
            summary,
            compilation_analysis,
            cache_analysis,
            hotspot_analysis,
            memory_analysis,
            recommendations,
            warnings,
        })
    }

    /// 生成性能摘要
    fn generate_summary(&self, history: &[PerformanceDataPoint]) -> Result<PerformanceSummary, String> {
        if history.is_empty() {
            return Err("No data available for summary".to_string());
        }

        let total_compilations: u64 = history.iter()
            .map(|d| d.compilation_stats.original_insn_count as u64)
            .sum();

        let total_compilation_time_ms: u64 = history.iter()
            .map(|d| d.compilation_stats.compilation_time_ns / 1_000_000)
            .sum();

        let avg_compilation_time_ms = if total_compilations > 0 {
            total_compilation_time_ms as f64 / total_compilations as f64
        } else {
            0.0
        };

        let cache_hit_rate = if !history.is_empty() {
            let total_hits: u64 = history.iter().map(|d| d.cache_stats.hits).sum();
            let total_misses: u64 = history.iter().map(|d| d.cache_stats.misses).sum();
            if total_hits + total_misses > 0 {
                (total_hits as f64) / ((total_hits + total_misses) as f64) * 100.0
            } else {
                0.0
            }
        } else {
            0.0
        };

        let hotspot_concentration = if !history.is_empty() {
            history.iter().map(|d| d.hotspot_stats.concentration()).sum::<f64>() / history.len() as f64
        } else {
            0.0
        };

        let memory_usage_mb = if !history.is_empty() {
            history.iter().map(|d| d.memory_stats.total_memory_mb()).sum::<f64>() / history.len() as f64
        } else {
            0.0
        };

        // 计算性能评分
        let performance_score = self.calculate_performance_score(
            avg_compilation_time_ms,
            cache_hit_rate,
            hotspot_concentration,
            memory_usage_mb,
        );

        Ok(PerformanceSummary {
            total_compilations,
            total_compilation_time_ms,
            avg_compilation_time_ms,
            cache_hit_rate,
            hotspot_concentration,
            memory_usage_mb,
            performance_score,
        })
    }

    /// 计算性能评分
    fn calculate_performance_score(
        &self,
        avg_compilation_time_ms: f64,
        cache_hit_rate: f64,
        hotspot_concentration: f64,
        memory_usage_mb: f64,
    ) -> u8 {
        let score = 100u8;

        // 编译时间评分 (40%)
        let time_score = if avg_compilation_time_ms <= 10.0 {
            40
        } else if avg_compilation_time_ms <= 50.0 {
            30
        } else if avg_compilation_time_ms <= 100.0 {
            20
        } else if avg_compilation_time_ms <= 500.0 {
            10
        } else {
            0
        };

        // 缓存命中率评分 (30%)
        let cache_score = if cache_hit_rate >= 90.0 {
            30
        } else if cache_hit_rate >= 80.0 {
            25
        } else if cache_hit_rate >= 70.0 {
            20
        } else if cache_hit_rate >= 50.0 {
            10
        } else {
            0
        };

        // 热点集中度评分 (20%)
        let hotspot_score = if hotspot_concentration >= 80.0 {
            20
        } else if hotspot_concentration >= 60.0 {
            15
        } else if hotspot_concentration >= 40.0 {
            10
        } else if hotspot_concentration >= 20.0 {
            5
        } else {
            0
        };

        // 内存使用评分 (10%)
        let memory_score = if memory_usage_mb <= 50.0 {
            10
        } else if memory_usage_mb <= 100.0 {
            8
        } else if memory_usage_mb <= 200.0 {
            5
        } else if memory_usage_mb <= 500.0 {
            2
        } else {
            0
        };

        (time_score + cache_score + hotspot_score + memory_score) as u8
    }

    /// 分析编译性能
    fn analyze_compilation_performance(&self, history: &[PerformanceDataPoint]) -> Result<CompilationAnalysis, String> {
        if history.is_empty() {
            return Err("No data available for compilation analysis".to_string());
        }

        // 计算编译时间分布
        let compilation_times: Vec<f64> = history.iter()
            .map(|d| d.compilation_stats.compilation_time_ns as f64 / 1_000_000.0) // 转换为毫秒
            .collect();

        let time_distribution = self.calculate_time_distribution(&compilation_times);

        // 计算效率趋势
        let efficiency_trend = self.calculate_efficiency_trend(&compilation_times);

        // 分析优化效果
        let optimization_effectiveness = self.analyze_optimization_effectiveness(history)?;

        // 分析并行编译效率
        let parallel_efficiency = self.analyze_parallel_efficiency(history)?;

        Ok(CompilationAnalysis {
            compilation_time_distribution: time_distribution,
            efficiency_trend,
            optimization_effectiveness,
            parallel_efficiency,
        })
    }

    /// 计算时间分布
    fn calculate_time_distribution(&self, times: &[f64]) -> TimeDistribution {
        if times.is_empty() {
            return TimeDistribution {
                min_ms: 0.0,
                max_ms: 0.0,
                avg_ms: 0.0,
                median_ms: 0.0,
                p95_ms: 0.0,
                p99_ms: 0.0,
                std_dev_ms: 0.0,
            };
        }

        let mut sorted_times = times.to_vec();
        sorted_times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let min_ms = sorted_times[0];
        let max_ms = sorted_times[sorted_times.len() - 1];
        let avg_ms = times.iter().sum::<f64>() / times.len() as f64;

        let median_ms = if sorted_times.len() % 2 == 0 {
            let mid = sorted_times.len() / 2;
            (sorted_times[mid - 1] + sorted_times[mid]) / 2.0
        } else {
            sorted_times[sorted_times.len() / 2]
        };

        let p95_index = (sorted_times.len() as f64 * 0.95) as usize;
        let p95_ms = sorted_times[p95_index.min(sorted_times.len() - 1)];

        let p99_index = (sorted_times.len() as f64 * 0.99) as usize;
        let p99_ms = sorted_times[p99_index.min(sorted_times.len() - 1)];

        let variance = times.iter()
            .map(|t| (t - avg_ms).powi(2))
            .sum::<f64>() / times.len() as f64;
        let std_dev_ms = variance.sqrt();

        TimeDistribution {
            min_ms,
            max_ms,
            avg_ms,
            median_ms,
            p95_ms,
            p99_ms,
            std_dev_ms,
        }
    }

    /// 计算效率趋势
    fn calculate_efficiency_trend(&self, times: &[f64]) -> TrendAnalysis {
        if times.len() < 2 {
            return TrendAnalysis {
                direction: TrendDirection::Stable,
                change_rate_pct: 0.0,
                prediction: None,
                confidence: 0.0,
            };
        }

        // 简单线性回归计算趋势
        let n = times.len() as f64;
        let sum_x: f64 = (0..times.len()).map(|i| i as f64).sum();
        let sum_y: f64 = times.iter().sum();
        let sum_xy: f64 = times.iter().enumerate().map(|(i, &y)| i as f64 * y).sum();
        let sum_x2: f64 = (0..times.len()).map(|i| (i as f64).powi(2)).sum();

        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x.powi(2));
        let intercept = (sum_y - slope * sum_x) / n;

        // 计算变化率
        let first_value = times[0];
        let last_value = times[times.len() - 1];
        let change_rate_pct = if first_value != 0.0 {
            ((last_value - first_value) / first_value) * 100.0
        } else {
            0.0
        };

        // 确定趋势方向
        let direction = if slope.abs() < 0.01 {
            TrendDirection::Stable
        } else if slope > 0.0 {
            TrendDirection::Increasing
        } else {
            TrendDirection::Decreasing
        };

        // 简单预测下一个值
        let prediction = Some(slope * n + intercept);

        // 计算R²作为置信度
        let mean_y = sum_y / n;
        let ss_tot: f64 = times.iter().map(|y| (y - mean_y).powi(2)).sum();
        let ss_res: f64 = times.iter().enumerate().map(|(i, &y)| {
            let predicted = slope * i as f64 + intercept;
            (y - predicted).powi(2)
        }).sum();
        let confidence = if ss_tot != 0.0 {
            1.0 - (ss_res / ss_tot)
        } else {
            0.0
        };

        TrendAnalysis {
            direction,
            change_rate_pct,
            prediction,
            confidence: confidence.max(0.0).min(1.0),
        }
    }

    /// 分析优化效果
    fn analyze_optimization_effectiveness(&self, history: &[PerformanceDataPoint]) -> Result<OptimizationAnalysis, String> {
        if history.is_empty() {
            return Err("No data available for optimization analysis".to_string());
        }

        let total_original: usize = history.iter().map(|d| d.compilation_stats.original_insn_count).sum();
        let total_optimized: usize = history.iter().map(|d| d.compilation_stats.optimized_insn_count).sum();
        let total_optimization_time: u64 = history.iter().map(|d| d.compilation_stats.optimization_time_ns).sum();
        let total_compilation_time: u64 = history.iter().map(|d| d.compilation_stats.compilation_time_ns).sum();

        let instruction_reduction_rate = if total_original > 0 {
            (1.0 - (total_optimized as f64 / total_original as f64)) * 100.0
        } else {
            0.0
        };

        let optimization_time_ratio = if total_compilation_time > 0 {
            (total_optimization_time as f64 / total_compilation_time as f64) * 100.0
        } else {
            0.0
        };

        let mut stage_effectiveness = HashMap::new();
        stage_effectiveness.insert("instruction_reduction".to_string(), instruction_reduction_rate);
        stage_effectiveness.insert("time_efficiency".to_string(), 100.0 - optimization_time_ratio);

        Ok(OptimizationAnalysis {
            instruction_reduction_rate,
            optimization_time_ratio,
            stage_effectiveness,
        })
    }

    /// 分析并行编译效率
    fn analyze_parallel_efficiency(&self, _history: &[PerformanceDataPoint]) -> Result<ParallelCompilationAnalysis, String> {
        // 简化实现，实际应该分析并行编译数据
        Ok(ParallelCompilationAnalysis {
            parallelism_degree: 0.8,
            thread_utilization: 0.75,
            load_balance: 0.85,
            efficiency_gain: 0.6,
        })
    }

    /// 分析缓存性能
    fn analyze_cache_performance(&self, history: &[PerformanceDataPoint]) -> Result<CacheAnalysis, String> {
        if history.is_empty() {
            return Err("No data available for cache analysis".to_string());
        }

        // 计算缓存效率趋势
        let cache_hit_rates: Vec<f64> = history.iter().map(|d| d.cache_stats.hit_rate()).collect();
        let efficiency_trend = self.calculate_efficiency_trend(&cache_hit_rates);

        // 分析缓存热点
        let hotspot_analysis = self.analyze_cache_hotspots(history)?;

        // 缓存大小优化建议
        let size_optimization = self.analyze_cache_size_optimization(history)?;

        Ok(CacheAnalysis {
            efficiency_trend,
            hotspot_analysis,
            size_optimization,
        })
    }

    /// 分析缓存热点
    fn analyze_cache_hotspots(&self, _history: &[PerformanceDataPoint]) -> Result<CacheHotspotAnalysis, String> {
        // 简化实现
        Ok(CacheHotspotAnalysis {
            hot_entries: vec![],
            access_pattern: AccessPattern::Mixed,
            locality_analysis: LocalityAnalysis {
                temporal_locality: 0.7,
                spatial_locality: 0.6,
            },
        })
    }

    /// 分析缓存大小优化
    fn analyze_cache_size_optimization(&self, history: &[PerformanceDataPoint]) -> Result<CacheSizeOptimization, String> {
        if history.is_empty() {
            return Err("No data available for cache size analysis".to_string());
        }

        let last = history.last()
            .ok_or_else(|| "Failed to get last history entry".to_string())?;
        let current_size_mb = last.cache_stats.size_bytes as f64 / (1024.0 * 1024.0);
        let recommended_size_mb = current_size_mb * 1.2; // 简单建议增加20%
        let expected_improvement = 15.0; // 预期15%性能提升

        Ok(CacheSizeOptimization {
            current_size_mb,
            recommended_size_mb,
            expected_improvement,
        })
    }

    /// 分析热点性能
    fn analyze_hotspot_performance(&self, history: &[PerformanceDataPoint]) -> Result<HotspotAnalysis, String> {
        if history.is_empty() {
            return Err("No data available for hotspot analysis".to_string());
        }

        // 简化实现
        let distribution = HotspotDistribution {
            density: 0.7,
            clusters: vec![],
        };

        let evolution = HotspotEvolution {
            new_hotspots: vec![],
            disappeared_hotspots: vec![],
            migrated_hotspots: vec![],
        };

        let prediction = HotspotPrediction {
            potential_hotspots: vec![],
            confidence: 0.6,
        };

        Ok(HotspotAnalysis {
            distribution,
            evolution,
            prediction,
        })
    }

    /// 分析内存性能
    fn analyze_memory_performance(&self, history: &[PerformanceDataPoint]) -> Result<MemoryAnalysis, String> {
        if history.is_empty() {
            return Err("No data available for memory analysis".to_string());
        }

        // 计算内存使用趋势
        let memory_usage: Vec<f64> = history.iter()
            .map(|d| d.memory_stats.total_memory_bytes as f64 / (1024.0 * 1024.0))
            .collect();
        let usage_trend = self.calculate_efficiency_trend(&memory_usage);

        // 计算内存分布
        let distribution = if !history.is_empty() {
            let last = history.last()
                .ok_or_else(|| "Failed to get last history entry for distribution".to_string())?;
            let total = last.memory_stats.total_memory_bytes as f64;
            
            let code_ratio = if total > 0.0 {
                last.memory_stats.code_memory_bytes as f64 / total * 100.0
            } else {
                0.0
            };
            
            let cache_ratio = if total > 0.0 {
                last.memory_stats.cache_memory_bytes as f64 / total * 100.0
            } else {
                0.0
            };
            
            let ir_ratio = if total > 0.0 {
                last.memory_stats.ir_memory_bytes as f64 / total * 100.0
            } else {
                0.0
            };
            
            let other_ratio = if total > 0.0 {
                last.memory_stats.other_memory_bytes as f64 / total * 100.0
            } else {
                0.0
            };

            MemoryDistribution {
                code_ratio,
                cache_ratio,
                ir_ratio,
                other_ratio,
            }
        } else {
            MemoryDistribution {
                code_ratio: 0.0,
                cache_ratio: 0.0,
                ir_ratio: 0.0,
                other_ratio: 0.0,
            }
        };

        // 内存碎片分析
        let fragmentation_analysis = if !history.is_empty() {
            let last = history.last()
                .ok_or_else(|| "Failed to get last history entry for fragmentation".to_string())?;
            FragmentationAnalysis {
                external_fragmentation: last.memory_stats.fragmentation_ratio * 0.7,
                internal_fragmentation: last.memory_stats.fragmentation_ratio * 0.3,
                fragment_distribution: HashMap::new(),
            }
        } else {
            FragmentationAnalysis {
                external_fragmentation: 0.0,
                internal_fragmentation: 0.0,
                fragment_distribution: HashMap::new(),
            }
        };

        // 生成内存优化建议
        let optimization_suggestions = self.generate_memory_optimization_suggestions(&distribution, &fragmentation_analysis);

        Ok(MemoryAnalysis {
            usage_trend,
            distribution,
            fragmentation_analysis,
            optimization_suggestions,
        })
    }

    /// 生成内存优化建议
    fn generate_memory_optimization_suggestions(
        &self,
        distribution: &MemoryDistribution,
        fragmentation: &FragmentationAnalysis,
    ) -> Vec<MemoryOptimizationSuggestion> {
        let mut suggestions = Vec::new();

        // 缓存内存占比过高
        if distribution.cache_ratio > 60.0 {
            suggestions.push(MemoryOptimizationSuggestion {
                suggestion_type: MemoryOptimizationType::CacheSizeAdjustment,
                expected_benefit: 20.0,
                implementation_difficulty: ImplementationDifficulty::Easy,
                description: "缓存内存占比过高，建议调整缓存大小".to_string(),
            });
        }

        // 内存碎片严重
        if fragmentation.external_fragmentation > 0.3 {
            suggestions.push(MemoryOptimizationSuggestion {
                suggestion_type: MemoryOptimizationType::MemoryPoolOptimization,
                expected_benefit: 15.0,
                implementation_difficulty: ImplementationDifficulty::Medium,
                description: "内存碎片严重，建议使用内存池优化".to_string(),
            });
        }

        // IR内存占比过高
        if distribution.ir_ratio > 25.0 {
            suggestions.push(MemoryOptimizationSuggestion {
                suggestion_type: MemoryOptimizationType::DataStructureOptimization,
                expected_benefit: 10.0,
                implementation_difficulty: ImplementationDifficulty::Hard,
                description: "IR内存占比过高，建议优化数据结构".to_string(),
            });
        }

        suggestions
    }

    /// 生成性能建议
    fn generate_recommendations(
        &self,
        summary: &PerformanceSummary,
        compilation_analysis: &CompilationAnalysis,
        _cache_analysis: &CacheAnalysis,
        _memory_analysis: &MemoryAnalysis,
    ) -> Result<Vec<PerformanceRecommendation>, String> {
        let mut recommendations = Vec::new();

        // 编译时间建议
        if summary.avg_compilation_time_ms > 100.0 {
            recommendations.push(PerformanceRecommendation {
                recommendation_type: RecommendationType::ConfigurationOptimization,
                priority: RecommendationPriority::High,
                description: "编译时间过长，建议优化编译配置".to_string(),
                expected_benefit: "预计减少30-50%编译时间".to_string(),
                implementation_steps: vec![
                    "降低优化级别".to_string(),
                    "启用并行编译".to_string(),
                    "调整热点检测阈值".to_string(),
                ],
            });
        }

        // 缓存命中率建议
        if summary.cache_hit_rate < 70.0 {
            recommendations.push(PerformanceRecommendation {
                recommendation_type: RecommendationType::CacheOptimization,
                priority: RecommendationPriority::Medium,
                description: "缓存命中率较低，建议优化缓存策略".to_string(),
                expected_benefit: "预计提升20-40%执行效率".to_string(),
                implementation_steps: vec![
                    "增加缓存大小".to_string(),
                    "优化缓存替换策略".to_string(),
                    "改进热点检测算法".to_string(),
                ],
            });
        }

        // 内存使用建议
        if summary.memory_usage_mb > 200.0 {
            recommendations.push(PerformanceRecommendation {
                recommendation_type: RecommendationType::MemoryOptimization,
                priority: RecommendationPriority::Medium,
                description: "内存使用较高，建议优化内存管理".to_string(),
                expected_benefit: "预计减少20-30%内存使用".to_string(),
                implementation_steps: vec![
                    "实现内存池".to_string(),
                    "优化数据结构".to_string(),
                    "添加内存压缩".to_string(),
                ],
            });
        }

        // 并行编译建议
        if compilation_analysis.parallel_efficiency.efficiency_gain < 0.5 {
            recommendations.push(PerformanceRecommendation {
                recommendation_type: RecommendationType::ParallelOptimization,
                priority: RecommendationPriority::Low,
                description: "并行编译效率较低，建议优化并行策略".to_string(),
                expected_benefit: "预计提升15-25%编译效率".to_string(),
                implementation_steps: vec![
                    "优化任务分配".to_string(),
                    "改进负载均衡".to_string(),
                    "减少线程同步开销".to_string(),
                ],
            });
        }

        Ok(recommendations)
    }

    /// 生成性能警告
    fn generate_warnings(
        &self,
        summary: &PerformanceSummary,
        compilation_analysis: &CompilationAnalysis,
        _cache_analysis: &CacheAnalysis,
        _memory_analysis: &MemoryAnalysis,
    ) -> Result<Vec<PerformanceWarning>, String> {
        let mut warnings = Vec::new();
        let now = std::time::SystemTime::now();

        // 编译时间警告
        if summary.avg_compilation_time_ms > self.config.performance_thresholds.compilation_time_error_ms as f64 {
            warnings.push(PerformanceWarning {
                level: WarningLevel::Error,
                warning_type: WarningType::LongCompilationTime,
                message: format!("平均编译时间过长: {:.2}ms", summary.avg_compilation_time_ms),
                timestamp: now,
                suggested_actions: vec![
                    "降低优化级别".to_string(),
                    "检查编译器配置".to_string(),
                    "考虑使用更简单的优化策略".to_string(),
                ],
            });
        } else if summary.avg_compilation_time_ms > self.config.performance_thresholds.compilation_time_warning_ms as f64 {
            warnings.push(PerformanceWarning {
                level: WarningLevel::Warning,
                warning_type: WarningType::LongCompilationTime,
                message: format!("编译时间较长: {:.2}ms", summary.avg_compilation_time_ms),
                timestamp: now,
                suggested_actions: vec![
                    "监控编译时间趋势".to_string(),
                    "考虑优化热点代码".to_string(),
                ],
            });
        }

        // 缓存命中率警告
        if summary.cache_hit_rate < self.config.performance_thresholds.cache_hit_rate_error_pct {
            warnings.push(PerformanceWarning {
                level: WarningLevel::Error,
                warning_type: WarningType::LowCacheEfficiency,
                message: format!("缓存命中率过低: {:.2}%", summary.cache_hit_rate),
                timestamp: now,
                suggested_actions: vec![
                    "增加缓存大小".to_string(),
                    "优化缓存替换策略".to_string(),
                    "检查热点检测算法".to_string(),
                ],
            });
        } else if summary.cache_hit_rate < self.config.performance_thresholds.cache_hit_rate_warning_pct {
            warnings.push(PerformanceWarning {
                level: WarningLevel::Warning,
                warning_type: WarningType::LowCacheEfficiency,
                message: format!("缓存命中率较低: {:.2}%", summary.cache_hit_rate),
                timestamp: now,
                suggested_actions: vec![
                    "监控缓存效率".to_string(),
                    "考虑调整缓存参数".to_string(),
                ],
            });
        }

        // 内存使用警告
        if summary.memory_usage_mb > self.config.performance_thresholds.memory_usage_error_mb as f64 {
            warnings.push(PerformanceWarning {
                level: WarningLevel::Error,
                warning_type: WarningType::ResourceExhaustion,
                message: format!("内存使用过高: {:.2}MB", summary.memory_usage_mb),
                timestamp: now,
                suggested_actions: vec![
                    "清理不必要的缓存".to_string(),
                    "优化内存分配策略".to_string(),
                    "考虑实现内存压缩".to_string(),
                ],
            });
        } else if summary.memory_usage_mb > self.config.performance_thresholds.memory_usage_warning_mb as f64 {
            warnings.push(PerformanceWarning {
                level: WarningLevel::Warning,
                warning_type: WarningType::ResourceExhaustion,
                message: format!("内存使用较高: {:.2}MB", summary.memory_usage_mb),
                timestamp: now,
                suggested_actions: vec![
                    "监控内存使用趋势".to_string(),
                    "考虑优化内存管理".to_string(),
                ],
            });
        }

        // 性能下降警告
        if let Some(prediction) = compilation_analysis.efficiency_trend.prediction {
            if prediction > compilation_analysis.efficiency_trend.change_rate_pct.abs() * 1.5 {
                warnings.push(PerformanceWarning {
                    level: WarningLevel::Warning,
                    warning_type: WarningType::PerformanceDegradation,
                    message: "检测到性能下降趋势".to_string(),
                    timestamp: now,
                    suggested_actions: vec![
                        "分析性能瓶颈".to_string(),
                        "检查系统资源使用".to_string(),
                        "考虑回退最近的更改".to_string(),
                    ],
                });
            }
        }

        Ok(warnings)
    }

    /// 收集内存统计
    fn collect_memory_stats(&self, _jit_engine: &JITEngine) -> Result<MemoryStats, String> {
        // 简化实现，实际应该从JIT引擎获取详细的内存统计
        Ok(MemoryStats {
            code_memory_bytes: 10 * 1024 * 1024, // 10MB
            cache_memory_bytes: 20 * 1024 * 1024, // 20MB
            ir_memory_bytes: 5 * 1024 * 1024, // 5MB
            other_memory_bytes: 5 * 1024 * 1024, // 5MB
            total_memory_bytes: 40 * 1024 * 1024, // 40MB
            fragmentation_ratio: 0.15, // 15%碎片率
        })
    }

    /// 收集系统统计
    fn collect_system_stats(&self) -> Result<SystemStats, String> {
        // 简化实现，实际应该使用系统API获取真实的系统统计
        Ok(SystemStats {
            cpu_usage_pct: 45.0,
            thread_count: num_cpus::get(),
            context_switches: 1000,
            syscalls: 500,
            page_faults: 10,
        })
    }

    /// 导出性能报告为JSON
    pub fn export_report_as_json(&self, report: &PerformanceReport) -> Result<String, String> {
        serde_json::to_string_pretty(report)
            .map_err(|e| format!("Failed to serialize report: {}", e))
    }

    /// 从JSON导入性能报告
    pub fn import_report_from_json(&self, json: &str) -> Result<PerformanceReport, String> {
        serde_json::from_str(json)
            .map_err(|e| format!("Failed to deserialize report: {}", e))
    }

    /// 保存性能报告到文件
    pub fn save_report_to_file(&self, report: &PerformanceReport, file_path: &str) -> Result<(), String> {
        let json = self.export_report_as_json(report)?;
        std::fs::write(file_path, json)
            .map_err(|e| format!("Failed to write report to file: {}", e))
    }

    /// 从文件加载性能报告
    pub fn load_report_from_file(&self, file_path: &str) -> Result<PerformanceReport, String> {
        let json = std::fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read report from file: {}", e))?;
        self.import_report_from_json(&json)
    }

    /// 启动实时监控
    pub fn start_realtime_monitoring(&self, _jit_engine: &JITEngine) -> Result<(), String> {
        if !self.config.enable_realtime_monitoring {
            return Err("Realtime monitoring is disabled".to_string());
        }

        // 简化实现，实际应该启动后台线程
        Ok(())
    }
}
