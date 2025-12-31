//! 自适应GC调优
//!
//! 根据运行时行为动态调整GC参数，优化性能。

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Instant, SystemTime};

use super::gc_generational_enhanced::{GenerationalGCConfig, GenerationalGCStats};

// ============================================================================
// 性能指标
// ============================================================================

/// GC性能指标
#[derive(Debug, Clone)]
pub struct GCPerformanceMetrics {
    /// 堆大小（字节）
    pub heap_size: usize,
    /// 已使用内存（字节）
    pub used_memory: usize,
    /// 内存碎片率（0.0-1.0）
    pub fragmentation_rate: f64,
    /// 平均GC暂停时间（纳秒）
    pub avg_pause_time_ns: u64,
    /// 99th percentile暂停时间（纳秒）
    pub p99_pause_time_ns: u64,
    /// GC吞吐量（0.0-1.0，非GC时间占比）
    pub throughput: f64,
    /// Minor GC次数
    pub minor_gc_count: u64,
    /// Major GC次数
    pub major_gc_count: u64,
    /// 晋升对象数
    pub promoted_objects: u64,
    /// 回收对象数
    pub collected_objects: u64,
}

impl GCPerformanceMetrics {
    /// 计算碎片率
    pub fn compute_fragmentation(&self) -> f64 {
        if self.heap_size == 0 {
            return 0.0;
        }
        // 简化计算：未使用内存的比例
        let unused = self.heap_size.saturating_sub(self.used_memory);
        unused as f64 / self.heap_size as f64
    }

    /// 是否内存压力过高
    pub fn is_high_memory_pressure(&self) -> bool {
        self.used_memory as f64 / self.heap_size as f64 > 0.9
    }

    /// 是否碎片率过高
    pub fn is_high_fragmentation(&self) -> bool {
        self.fragmentation_rate > 0.3
    }

    /// 是否暂停时间过长
    pub fn is_long_pause(&self, threshold_ms: u64) -> bool {
        self.p99_pause_time_ns > threshold_ms * 1_000_000
    }

    /// 是否吞吐量过低
    pub fn is_low_throughput(&self, threshold: f64) -> bool {
        self.throughput < threshold
    }
}

// ============================================================================
// 诊断问题
// ============================================================================

/// 诊断出的问题
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GCProblem {
    /// 无问题
    None,
    /// 高碎片率
    HighFragmentation,
    /// 长暂停时间
    LongPauseTime,
    /// 低吞吐量
    LowThroughput,
    /// 高内存压力
    HighMemoryPressure,
    /// 频繁晋升
    FrequentPromotion,
    /// OOM风险
    OOMRisk,
}

impl std::fmt::Display for GCProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GCProblem::None => write!(f, "None"),
            GCProblem::HighFragmentation => write!(f, "HighFragmentation"),
            GCProblem::LongPauseTime => write!(f, "LongPauseTime"),
            GCProblem::LowThroughput => write!(f, "LowThroughput"),
            GCProblem::HighMemoryPressure => write!(f, "HighMemoryPressure"),
            GCProblem::FrequentPromotion => write!(f, "FrequentPromotion"),
            GCProblem::OOMRisk => write!(f, "OOMRisk"),
        }
    }
}

// ============================================================================
// 调优动作
// ============================================================================

/// 调优动作
#[derive(Debug, Clone)]
pub struct TuningAction {
    /// 时间戳
    pub timestamp: SystemTime,
    /// 检测到的问题
    pub problem: GCProblem,
    /// 旧配置
    pub old_config: AdaptiveGCConfig,
    /// 新配置
    pub new_config: AdaptiveGCConfig,
    /// 调优原因
    pub reason: String,
}

// ============================================================================
// 自适应GC配置
// ============================================================================

/// 自适应GC配置
#[derive(Debug, Clone)]
pub struct AdaptiveGCConfig {
    /// 堆大小（字节）
    pub heap_size: usize,
    /// 新生代比例（0.0-1.0）
    pub nursery_ratio: f64,
    /// GC触发阈值（堆使用率，0.0-1.0）
    pub gc_threshold: f64,
    /// 增量GC工作配额
    pub work_quota: usize,
    /// 最小工作配额
    pub min_work_quota: usize,
    /// 最大工作配额
    pub max_work_quota: usize,
    /// 晋升年龄阈值
    pub promotion_age: u8,
    /// 晋升比例阈值
    pub promotion_ratio: f64,
    /// 目标暂停时间（毫秒）
    pub target_pause_time_ms: u64,
    /// 目标吞吐量（0.0-1.0）
    pub target_throughput: f64,
    /// 是否启用压缩
    pub enable_compaction: bool,
    /// 压缩触发阈值（碎片率）
    pub compaction_threshold: f64,
}

impl Default for AdaptiveGCConfig {
    fn default() -> Self {
        Self {
            heap_size: 256 * 1024 * 1024, // 256MB
            nursery_ratio: 0.1,            // 10%
            gc_threshold: 0.8,
            work_quota: 100,
            min_work_quota: 10,
            max_work_quota: 1000,
            promotion_age: 3,
            promotion_ratio: 0.8,
            target_pause_time_ms: 5,
            target_throughput: 0.9,
            enable_compaction: true,
            compaction_threshold: 0.3,
        }
    }
}

// ============================================================================
// 自适应GC统计
// ============================================================================

/// 自适应GC统计
#[derive(Debug, Default)]
pub struct AdaptiveGCStats {
    /// 调优次数
    pub tuning_count: AtomicU64,
    /// 问题检测次数
    pub problem_detection_count: AtomicU64,
    /// 配置变更次数
    pub config_change_count: AtomicU64,
    /// 最后调优时间
    pub last_tuning_time: AtomicU64,
}

impl AdaptiveGCStats {
    /// 调优频率（每秒调优次数）
    pub fn tuning_frequency(&self) -> f64 {
        let count = self.tuning_count.load(Ordering::Relaxed);
        let last_time = self.last_tuning_time.load(Ordering::Relaxed);

        if last_time == 0 {
            return 0.0;
        }

        let elapsed = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() - last_time;

        if elapsed == 0 {
            return 0.0;
        }

        count as f64 / elapsed as f64
    }
}

// ============================================================================
// 历史数据
// ============================================================================

/// 性能历史记录
#[derive(Debug, Clone)]
pub struct PerformanceHistoryEntry {
    /// 时间戳
    pub timestamp: SystemTime,
    /// 性能指标
    pub metrics: GCPerformanceMetrics,
    /// 检测到的问题
    pub problem: GCProblem,
}

/// 性能历史
pub struct PerformanceHistory {
    /// 历史记录
    entries: VecDeque<PerformanceHistoryEntry>,
    /// 最大保留条目数
    max_entries: usize,
}

impl PerformanceHistory {
    /// 创建新的历史记录
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(max_entries),
            max_entries,
        }
    }

    /// 添加记录
    pub fn push(&mut self, entry: PerformanceHistoryEntry) {
        if self.entries.len() >= self.max_entries {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    /// 获取最近的N条记录
    pub fn recent(&self, n: usize) -> Vec<&PerformanceHistoryEntry> {
        self.entries.iter().rev().take(n).collect()
    }

    /// 计算平均暂停时间
    pub fn avg_pause_time_ns(&self) -> f64 {
        if self.entries.is_empty() {
            return 0.0;
        }

        let sum: u64 = self
            .entries
            .iter()
            .map(|e| e.metrics.avg_pause_time_ns)
            .sum();
        sum as f64 / self.entries.len() as f64
    }

    /// 计算平均碎片率
    pub fn avg_fragmentation(&self) -> f64 {
        if self.entries.is_empty() {
            return 0.0;
        }

        let sum: f64 = self.entries.iter().map(|e| e.metrics.fragmentation_rate).sum();
        sum / self.entries.len() as f64
    }

    /// 问题频率（某个问题出现的频率）
    pub fn problem_frequency(&self, problem: GCProblem) -> f64 {
        if self.entries.is_empty() {
            return 0.0;
        }

        let count = self.entries.iter().filter(|e| e.problem == problem).count();
        count as f64 / self.entries.len() as f64
    }
}

// ============================================================================
// 自适应GC调优器
// ============================================================================

/// 自适应GC调优器
///
/// 根据运行时行为动态调整GC参数
pub struct AdaptiveGCTuner {
    /// 当前配置
    config: AdaptiveGCConfig,
    /// 性能历史
    history: PerformanceHistory,
    /// 调优历史
    tuning_history: VecDeque<TuningAction>,
    /// 统计信息
    stats: Arc<AdaptiveGCStats>,
    /// 是否启用自动调优
    auto_tuning_enabled: Arc<AtomicBool>,
    /// 调优间隔（最小秒数）
    tuning_interval_sec: u64,
    /// 最后调优时间
    last_tuning: Option<Instant>,
}

impl AdaptiveGCTuner {
    /// 创建新的自适应调优器
    pub fn new(config: AdaptiveGCConfig) -> Self {
        Self {
            config: config.clone(),
            history: PerformanceHistory::new(100),
            tuning_history: VecDeque::with_capacity(50),
            stats: Arc::new(AdaptiveGCStats::default()),
            auto_tuning_enabled: Arc::new(AtomicBool::new(true)),
            tuning_interval_sec: 10, // 默认10秒
            last_tuning: None,
        }
    }

    /// 启用/禁用自动调优
    pub fn set_auto_tuning(&self, enabled: bool) {
        self.auto_tuning_enabled.store(enabled, Ordering::Relaxed);
    }

    /// 设置调优间隔
    pub fn set_tuning_interval(&mut self, interval_sec: u64) {
        self.tuning_interval_sec = interval_sec;
    }

    /// 记录性能指标
    pub fn record_metrics(&mut self, metrics: GCPerformanceMetrics) {
        let problem = self.diagnose(&metrics);

        let entry = PerformanceHistoryEntry {
            timestamp: SystemTime::now(),
            metrics: metrics.clone(),
            problem,
        };

        self.history.push(entry);

        // 如果检测到问题，增加计数
        if problem != GCProblem::None {
            self.stats
                .problem_detection_count
                .fetch_add(1, Ordering::Relaxed);
        }
    }

    /// 诊断问题
    pub fn diagnose(&self, metrics: &GCPerformanceMetrics) -> GCProblem {
        // 优先级顺序检测

        // 1. OOM风险（最严重）
        if metrics.is_high_memory_pressure() {
            return GCProblem::OOMRisk;
        }

        // 2. 高碎片率
        if metrics.fragmentation_rate > self.config.compaction_threshold {
            return GCProblem::HighFragmentation;
        }

        // 3. 长暂停时间
        if metrics.p99_pause_time_ns > self.config.target_pause_time_ms * 1_000_000 * 2 {
            return GCProblem::LongPauseTime;
        }

        // 4. 低吞吐量
        if metrics.throughput < self.config.target_throughput * 0.8 {
            return GCProblem::LowThroughput;
        }

        // 5. 频繁晋升（晋升率过高）
        if metrics.minor_gc_count > 0 {
            let promotion_rate =
                metrics.promoted_objects as f64 / metrics.collected_objects as f64;
            if promotion_rate > 0.5 {
                return GCProblem::FrequentPromotion;
            }
        }

        GCProblem::None
    }

    /// 执行调优
    pub fn tune(&mut self) -> Option<TuningAction> {
        // 检查是否可以调优
        if !self.should_tune() {
            return None;
        }

        // 获取当前指标
        let current_metrics = self.current_metrics()?;
        let problem = self.diagnose(&current_metrics);

        if problem == GCProblem::None {
            return None;
        }

        // 根据问题调整配置
        let old_config = self.config.clone();
        let new_config = self.adjust_config(problem, &current_metrics)?;
        let reason = self.generate_reason(problem, &old_config, &new_config);

        // 应用新配置
        self.config = new_config.clone();

        // 记录调优动作
        let action = TuningAction {
            timestamp: SystemTime::now(),
            problem,
            old_config,
            new_config,
            reason,
        };

        self.tuning_history.push_back(action.clone());
        self.last_tuning = Some(Instant::now());

        // 更新统计
        self.stats.tuning_count.fetch_add(1, Ordering::Relaxed);
        self.stats.config_change_count.fetch_add(1, Ordering::Relaxed);
        self.stats.last_tuning_time.store(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            Ordering::Relaxed,
        );

        Some(action)
    }

    /// 检查是否应该调优
    fn should_tune(&self) -> bool {
        // 检查自动调优是否启用
        if !self.auto_tuning_enabled.load(Ordering::Relaxed) {
            return false;
        }

        // 检查时间间隔
        if let Some(last) = self.last_tuning {
            let elapsed = last.elapsed().as_secs();
            if elapsed < self.tuning_interval_sec {
                return false;
            }
        }

        true
    }

    /// 调整配置
    fn adjust_config(
        &mut self,
        problem: GCProblem,
        _metrics: &GCPerformanceMetrics,
    ) -> Option<AdaptiveGCConfig> {
        let mut config = self.config.clone();

        match problem {
            GCProblem::OOMRisk => {
                // OOM风险：增加堆大小，提前触发GC
                config.heap_size = (config.heap_size as f64 * 1.5) as usize;
                config.gc_threshold *= 0.8;
            }

            GCProblem::HighFragmentation => {
                // 高碎片率：缩小新生代，启用压缩
                config.nursery_ratio *= 0.9;
                config.enable_compaction = true;
                config.compaction_threshold *= 0.9;
            }

            GCProblem::LongPauseTime => {
                // 长暂停时间：减少工作配额，提前触发GC
                config.work_quota = (config.work_quota * 3 / 4).max(config.min_work_quota);
                config.gc_threshold *= 0.8;
                config.nursery_ratio *= 0.9; // 缩小新生代
            }

            GCProblem::LowThroughput => {
                // 低吞吐量：增加工作配额，延迟触发GC
                config.work_quota = (config.work_quota * 5 / 4).min(config.max_work_quota);
                config.gc_threshold *= 1.2;
                config.nursery_ratio *= 1.1; // 扩大新生代
            }

            GCProblem::FrequentPromotion => {
                // 频繁晋升：提高晋升阈值，扩大新生代
                config.promotion_age = config.promotion_age.saturating_add(1);
                config.promotion_ratio += 0.1;
                config.nursery_ratio *= 1.2;
            }

            _ => return None,
        }

        // 限制配置范围
        config.nursery_ratio = config.nursery_ratio.clamp(0.05, 0.3);
        config.gc_threshold = config.gc_threshold.clamp(0.5, 0.95);
        config.promotion_ratio = config.promotion_ratio.clamp(0.5, 0.95);

        Some(config)
    }

    /// 生成调优原因
    fn generate_reason(
        &self,
        problem: GCProblem,
        old: &AdaptiveGCConfig,
        new: &AdaptiveGCConfig,
    ) -> String {
        format!(
            "Detected: {}. Adjusted config: gc_threshold {:.2}->{:.2}, work_quota {}->{}, nursery_ratio {:.2}->{:.2}",
            problem,
            old.gc_threshold,
            new.gc_threshold,
            old.work_quota,
            new.work_quota,
            old.nursery_ratio,
            new.nursery_ratio
        )
    }

    /// 获取当前指标（最新的一条）
    fn current_metrics(&self) -> Option<GCPerformanceMetrics> {
        self.history.entries.back().map(|e| e.metrics.clone())
    }

    /// 获取当前配置
    pub fn config(&self) -> &AdaptiveGCConfig {
        &self.config
    }

    /// 获取统计信息
    pub fn stats(&self) -> &AdaptiveGCStats {
        &self.stats
    }

    /// 获取调优历史
    pub fn tuning_history(&self) -> Vec<&TuningAction> {
        self.tuning_history.iter().collect()
    }

    /// 获取性能历史
    pub fn history(&self) -> &PerformanceHistory {
        &self.history
    }

    /// 导出为GenerationalGCConfig
    pub fn to_generational_config(&self) -> GenerationalGCConfig {
        GenerationalGCConfig {
            nursery_size: (self.config.heap_size as f64 * self.config.nursery_ratio) as usize,
            promotion_age: self.config.promotion_age,
            promotion_ratio: self.config.promotion_ratio,
            use_card_table: true,
            card_size: 512,
        }
    }

    /// 从GenerationalGCStats计算性能指标
    pub fn compute_metrics_from_stats(
        &self,
        heap_size: usize,
        used_memory: usize,
        stats: &GenerationalGCStats,
        _incremental_stats: &super::gc_incremental_enhanced::IncrementalGCStats,
    ) -> GCPerformanceMetrics {
        let minor_count = stats.minor_gc_count.load(Ordering::Relaxed);
        let major_count = stats.major_gc_count.load(Ordering::Relaxed);
        let total_gc = minor_count + major_count;

        let total_pause_time = stats.minor_gc_time_ns.load(Ordering::Relaxed)
            + stats.major_gc_time_ns.load(Ordering::Relaxed);

        let avg_pause_time_ns = if total_gc > 0 {
            total_pause_time / total_gc
        } else {
            0
        };

        let promoted = stats.promoted_objects.load(Ordering::Relaxed);
        let collected = stats.collected_objects.load(Ordering::Relaxed);

        // 计算吞吐量（简化：假设总运行时间 = GC时间 + 非GC时间）
        let gc_ratio = if total_pause_time > 0 {
            let total_time = total_pause_time * 10; // 假设GC占10%
            total_pause_time as f64 / total_time as f64
        } else {
            0.0
        };
        let throughput = 1.0 - gc_ratio;

        GCPerformanceMetrics {
            heap_size,
            used_memory,
            fragmentation_rate: 0.0, // 需要单独计算
            avg_pause_time_ns,
            p99_pause_time_ns: avg_pause_time_ns * 2, // 简化
            throughput,
            minor_gc_count: minor_count,
            major_gc_count: major_count,
            promoted_objects: promoted,
            collected_objects: collected,
        }
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_metrics() -> GCPerformanceMetrics {
        GCPerformanceMetrics {
            heap_size: 1024 * 1024 * 1024, // 1GB
            used_memory: 800 * 1024 * 1024, // 800MB
            fragmentation_rate: 0.1,
            avg_pause_time_ns: 1_000_000, // 1ms
            p99_pause_time_ns: 5_000_000,  // 5ms
            throughput: 0.95,
            minor_gc_count: 100,
            major_gc_count: 10,
            promoted_objects: 500,
            collected_objects: 5000,
        }
    }

    #[test]
    fn test_adaptive_tuner_creation() {
        let config = AdaptiveGCConfig::default();
        let mut tuner = AdaptiveGCTuner::new(config);

        assert_eq!(tuner.stats().tuning_count.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_record_metrics() {
        let config = AdaptiveGCConfig::default();
        let mut tuner = AdaptiveGCTuner::new(config);

        let metrics = create_test_metrics();
        tuner.record_metrics(metrics.clone());

        assert_eq!(tuner.history().entries.len(), 1);
    }

    #[test]
    fn test_diagnose_oom_risk() {
        let config = AdaptiveGCConfig::default();
        let mut tuner = AdaptiveGCTuner::new(config);

        let mut metrics = create_test_metrics();
        metrics.used_memory = 950 * 1024 * 1024; // 95%

        let problem = tuner.diagnose(&metrics);
        assert_eq!(problem, GCProblem::OOMRisk);
    }

    #[test]
    fn test_diagnose_high_fragmentation() {
        let config = AdaptiveGCConfig::default();
        let mut tuner = AdaptiveGCTuner::new(config);

        let mut metrics = create_test_metrics();
        metrics.fragmentation_rate = 0.4;

        let problem = tuner.diagnose(&metrics);
        assert_eq!(problem, GCProblem::HighFragmentation);
    }

    #[test]
    fn test_diagnose_long_pause() {
        let config = AdaptiveGCConfig::default();
        let mut tuner = AdaptiveGCTuner::new(config);

        let mut metrics = create_test_metrics();
        metrics.p99_pause_time_ns = 20_000_000; // 20ms

        let problem = tuner.diagnose(&metrics);
        assert_eq!(problem, GCProblem::LongPauseTime);
    }

    #[test]
    fn test_diagnose_low_throughput() {
        let config = AdaptiveGCConfig::default();
        let mut tuner = AdaptiveGCTuner::new(config);

        let mut metrics = create_test_metrics();
        metrics.throughput = 0.6; // 60%

        let problem = tuner.diagnose(&metrics);
        assert_eq!(problem, GCProblem::LowThroughput);
    }

    #[test]
    fn test_tune_high_fragmentation() {
        let mut config = AdaptiveGCConfig::default();
        config.compaction_threshold = 0.3;
        let mut tuner = AdaptiveGCTuner::new(config);

        let mut metrics = create_test_metrics();
        metrics.fragmentation_rate = 0.35;
        tuner.record_metrics(metrics);
        tuner.last_tuning = None; // 允许立即调优

        let action = tuner.tune();

        assert!(action.is_some());
        let action = action.unwrap();
        assert_eq!(action.problem, GCProblem::HighFragmentation);
        assert!(action.new_config.nursery_ratio < action.old_config.nursery_ratio);
    }

    #[test]
    fn test_tune_long_pause() {
        let config = AdaptiveGCConfig::default();
        let mut tuner = AdaptiveGCTuner::new(config);

        let mut metrics = create_test_metrics();
        metrics.p99_pause_time_ns = 20_000_000; // 20ms
        tuner.record_metrics(metrics);
        tuner.last_tuning = None; // 允许立即调优

        let action = tuner.tune();

        assert!(action.is_some());
        let action = action.unwrap();
        assert_eq!(action.problem, GCProblem::LongPauseTime);
        assert!(action.new_config.work_quota < action.old_config.work_quota);
    }

    #[test]
    fn test_tuning_interval() {
        let mut config = AdaptiveGCConfig::default();
        config.compaction_threshold = 0.3; // Set explicit threshold
        let mut tuner = AdaptiveGCTuner::new(config);
        tuner.set_tuning_interval(60); // 60秒间隔

        // Use metrics that will trigger a problem (high fragmentation)
        let mut metrics = create_test_metrics();
        metrics.fragmentation_rate = 0.35; // Above compaction_threshold of 0.3
        tuner.record_metrics(metrics);

        // 第一次调优应该成功
        tuner.last_tuning = None;
        assert!(tuner.tune().is_some());

        // 立即再次调优应该失败（间隔不足）
        assert!(tuner.tune().is_none());
    }

    #[test]
    fn test_auto_tuning_disable() {
        let config = AdaptiveGCConfig::default();
        let mut tuner = AdaptiveGCTuner::new(config);

        // 禁用自动调优
        tuner.set_auto_tuning(false);

        let metrics = create_test_metrics();
        tuner.record_metrics(metrics);

        assert!(tuner.tune().is_none());
    }

    #[test]
    fn test_history_average() {
        let config = AdaptiveGCConfig::default();
        let mut tuner = AdaptiveGCTuner::new(config);

        for i in 1..=10 {
            let mut metrics = create_test_metrics();
            metrics.avg_pause_time_ns = i * 1_000_000;
            tuner.record_metrics(metrics);
        }

        let avg = tuner.history.avg_pause_time_ns();
        assert_eq!(avg, 5.5_f64 * 1_000_000_f64); // (1+10)/2 * 1ms
    }

    #[test]
    fn test_to_generational_config() {
        let config = AdaptiveGCConfig {
            heap_size: 100 * 1024 * 1024, // 100MB
            nursery_ratio: 0.2,
            promotion_age: 5,
            promotion_ratio: 0.7,
            ..Default::default()
        };
        let mut tuner = AdaptiveGCTuner::new(config);

        let gen_config = tuner.to_generational_config();
        assert_eq!(gen_config.nursery_size, 20 * 1024 * 1024); // 20MB
        assert_eq!(gen_config.promotion_age, 5);
        assert_eq!(gen_config.promotion_ratio, 0.7);
    }

    #[test]
    fn test_problem_display() {
        assert_eq!(format!("{}", GCProblem::None), "None");
        assert_eq!(format!("{}", GCProblem::HighFragmentation), "HighFragmentation");
        assert_eq!(format!("{}", GCProblem::LongPauseTime), "LongPauseTime");
    }
}
