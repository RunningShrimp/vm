//! TLB刷新策略高级优化
//!
//! 实现更智能的TLB刷新策略，包括预测性刷新、选择性刷新和自适应优化

use crate::GuestAddr;
use crate::tlb::per_cpu_tlb::PerCpuTlbManager;
use crate::tlb::tlb_flush::{FlushStrategy, FlushScope, FlushRequest, TlbFlushConfig, AccessPattern, TlbFlushManager, TlbFlushStatsSnapshot};
use vm_core::{AccessType, VmError};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

// 辅助函数：将AccessType转换为可哈希的值
fn access_type_to_hashable(access_type: AccessType) -> u8 {
    match access_type {
        AccessType::Read => 0,
        AccessType::Write => 1,
        AccessType::Execute => 2,
        AccessType::Atomic => 3,
    }
}

// AccessType的包装类型，实现Hash
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct HashableAccessType(u8);

impl From<AccessType> for HashableAccessType {
    fn from(access_type: AccessType) -> Self {
        Self(access_type_to_hashable(access_type))
    }
}

impl From<HashableAccessType> for AccessType {
    fn from(hashable: HashableAccessType) -> Self {
        match hashable.0 {
            0 => AccessType::Read,
            1 => AccessType::Write,
            2 => AccessType::Execute,
            3 => AccessType::Atomic,
            _ => AccessType::Read, // 默认值
        }
    }
}

// 辅助函数：计算两个GuestAddr之间的页数
fn page_count_between(start: GuestAddr, end: GuestAddr) -> u64 {
    (end.0 - start.0) / 4096 + 1
}

/// 预测性刷新配置
#[derive(Debug, Clone)]
pub struct PredictiveFlushConfig {
    /// 是否启用预测性刷新
    pub enabled: bool,
    /// 预测窗口大小（页面数）
    pub prediction_window: usize,
    /// 预测准确率阈值
    pub accuracy_threshold: f64,
    /// 最大预测刷新数量
    pub max_predictive_flushes: usize,
    /// 预测历史大小
    pub history_size: usize,
}

impl Default for PredictiveFlushConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            prediction_window: 8,
            accuracy_threshold: 0.7,
            max_predictive_flushes: 4,
            history_size: 256,
        }
    }
}

/// 选择性刷新配置
#[derive(Debug, Clone)]
pub struct SelectiveFlushConfig {
    /// 是否启用选择性刷新
    pub enabled: bool,
    /// 热点页面保护阈值
    pub hot_page_threshold: u64,
    /// 冷页面淘汰阈值
    pub cold_page_threshold: u64,
    /// 访问频率权重
    pub frequency_weight: f64,
    /// 最近访问权重
    pub recency_weight: f64,
    /// 页面大小权重
    pub size_weight: f64,
}

impl Default for SelectiveFlushConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hot_page_threshold: 50,
            cold_page_threshold: 5,
            frequency_weight: 0.5,
            recency_weight: 0.3,
            size_weight: 0.2,
        }
    }
}

/// 自适应刷新配置
#[derive(Debug, Clone)]
pub struct AdaptiveFlushConfig {
    /// 是否启用自适应刷新
    pub enabled: bool,
    /// 性能监控窗口大小
    pub monitoring_window: usize,
    /// 性能下降阈值
    pub performance_threshold: f64,
    /// 策略切换间隔（秒）
    pub strategy_switch_interval: u64,
    /// 最小样本数量
    pub min_samples: usize,
}

impl Default for AdaptiveFlushConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            monitoring_window: 100,
            performance_threshold: 0.1,
            strategy_switch_interval: 30,
            min_samples: 20,
        }
    }
}

/// 高级TLB刷新配置
#[derive(Debug, Clone)]
pub struct AdvancedTlbFlushConfig {
    /// 基础刷新配置
    pub base_config: TlbFlushConfig,
    /// 预测性刷新配置
    pub predictive_config: PredictiveFlushConfig,
    /// 选择性刷新配置
    pub selective_config: SelectiveFlushConfig,
    /// 自适应刷新配置
    pub adaptive_config: AdaptiveFlushConfig,
}

impl Default for AdvancedTlbFlushConfig {
    fn default() -> Self {
        Self {
            base_config: TlbFlushConfig::default(),
            predictive_config: PredictiveFlushConfig::default(),
            selective_config: SelectiveFlushConfig::default(),
            adaptive_config: AdaptiveFlushConfig::default(),
        }
    }
}

/// 访问预测器
#[derive(Debug, Clone)]
pub struct AccessPredictor {
    /// 访问历史
    access_history: VecDeque<(GuestAddr, u16, Instant)>,
    /// 预测历史
    prediction_history: VecDeque<(Vec<GuestAddr>, bool)>,
    /// 最大历史大小
    max_history: usize,
    /// 预测窗口大小
    prediction_window: usize,
    /// 模式映射
    pattern_map: HashMap<Vec<GuestAddr>, Vec<GuestAddr>>,
}

impl AccessPredictor {
    /// 创建新的访问预测器
    pub fn new(max_history: usize, prediction_window: usize) -> Self {
        Self {
            access_history: VecDeque::with_capacity(max_history),
            prediction_history: VecDeque::with_capacity(max_history),
            max_history,
            prediction_window,
            pattern_map: HashMap::new(),
        }
    }

    /// 记录访问
    pub fn record_access(&mut self, gva: GuestAddr, asid: u16) {
        let now = Instant::now();
        let page_base = GuestAddr(gva & !(4096 - 1));
        
        // 添加到访问历史
        self.access_history.push_back((page_base, asid, now));
        
        // 保持历史记录大小
        if self.access_history.len() > self.max_history {
            self.access_history.pop_front();
        }
        
        // 更新模式映射
        self.update_pattern_map();
    }

    /// 预测下一个访问
    pub fn predict_next_accesses(&self, asid: u16) -> Vec<GuestAddr> {
        if self.access_history.len() < self.prediction_window {
            return Vec::new();
        }
        
        // 获取最近的访问模式
        let recent_pattern: Vec<_> = self.access_history
            .iter()
            .rev()
            .take(self.prediction_window)
            .filter(|(_, a, _)| *a == asid)
            .map(|(gva, _, _)| *gva)
            .collect();
        
        if recent_pattern.len() < self.prediction_window / 2 {
            return Vec::new();
        }
        
        // 查找匹配的历史模式
        for (pattern, next_pages) in &self.pattern_map {
            if self.pattern_matches(&recent_pattern, pattern) {
                return next_pages.clone();
            }
        }
        
        Vec::new()
    }

    /// 验证预测
    pub fn validate_prediction(&mut self, predicted: &[GuestAddr], actual: GuestAddr) -> bool {
        let page_base = GuestAddr(actual & !(4096 - 1));
        let is_correct = predicted.contains(&page_base);
        
        // 记录预测结果
        self.prediction_history.push_back((predicted.to_vec(), is_correct));
        
        // 保持历史记录大小
        if self.prediction_history.len() > self.max_history {
            self.prediction_history.pop_front();
        }
        
        is_correct
    }

    /// 获取预测准确率
    pub fn get_accuracy(&self) -> f64 {
        if self.prediction_history.is_empty() {
            return 0.0;
        }
        
        let correct = self.prediction_history.iter().filter(|(_, correct)| *correct).count();
        correct as f64 / self.prediction_history.len() as f64
    }

    /// 更新模式映射
    fn update_pattern_map(&mut self) {
        if self.access_history.len() < self.prediction_window * 2 {
            return;
        }
        
        let accesses: Vec<_> = self.access_history.iter().map(|(gva, asid, _)| (*gva, *asid)).collect();
        
        for i in 0..=accesses.len() - self.prediction_window * 2 {
            let pattern: Vec<_> = accesses[i..i + self.prediction_window]
                .iter()
                .filter(|(_, asid)| *asid == accesses[i].1)
                .map(|(gva, _)| *gva)
                .collect();
            
            let next_pages: Vec<_> = accesses[i + self.prediction_window..i + self.prediction_window * 2]
                .iter()
                .filter(|(_, asid)| *asid == accesses[i].1)
                .map(|(gva, _)| *gva)
                .collect();
            
            if pattern.len() >= self.prediction_window / 2 && !next_pages.is_empty() {
                self.pattern_map.insert(pattern, next_pages);
            }
        }
    }

    /// 检查模式是否匹配
    fn pattern_matches(&self, pattern1: &[GuestAddr], pattern2: &[GuestAddr]) -> bool {
        if pattern1.len() != pattern2.len() {
            return false;
        }
        
        let mut matches = 0;
        for (p1, p2) in pattern1.iter().zip(pattern2.iter()) {
            if p1 == p2 {
                matches += 1;
            }
        }
        
        matches as f64 / pattern1.len() as f64 >= 0.8
    }
}

/// 页面重要性评估器
#[derive(Debug)]
pub struct PageImportanceEvaluator {
    /// 页面访问统计
    page_stats: Arc<Mutex<HashMap<(GuestAddr, u16), PageStats>>>,
    /// 评估配置
    config: SelectiveFlushConfig,
    /// 最后清理时间
    last_cleanup: Arc<Mutex<Instant>>,
}

/// 页面统计信息
#[derive(Debug, Clone)]
struct PageStats {
    /// 访问次数
    access_count: u64,
    /// 最后访问时间
    last_access: Instant,
    /// 页面大小
    page_size: u64,
    /// 访问类型分布
    access_types: HashMap<HashableAccessType, u64>,
    /// 访问时间戳列表
    access_timestamps: Vec<Instant>,
    /// 访问模式
    access_pattern: Option<AccessPattern>,
}

impl PageStats {
    fn new() -> Self {
        Self {
            access_count: 0,
            last_access: Instant::now(),
            page_size: 4096,
            access_types: HashMap::new(),
            access_timestamps: Vec::new(),
            access_pattern: None,
        }
    }
}

impl PageImportanceEvaluator {
    /// 创建新的页面重要性评估器
    pub fn new(config: SelectiveFlushConfig) -> Self {
        Self {
            page_stats: Arc::new(Mutex::new(HashMap::new())),
            config,
            last_cleanup: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// 记录页面访问
    pub fn record_access(&self, gva: GuestAddr, asid: u16, access_type: AccessType) {
        let page_base = GuestAddr(gva & !(4096 - 1));
        let key = (page_base, asid);
        let now = Instant::now();
        
        {
            let mut stats = self.page_stats.lock().unwrap();
            let page_stats = stats.entry(key).or_insert_with(PageStats::new);
            
            page_stats.access_count += 1;
            page_stats.last_access = now;
            *page_stats.access_types.entry(access_type.into()).or_insert(0) += 1;
            
            // 更新访问时间戳
            page_stats.access_timestamps.push(now);
            // 保持时间戳列表合理大小
            if page_stats.access_timestamps.len() > 100 {
                page_stats.access_timestamps.remove(0);
            }
            
            // 简单的访问模式检测
            if page_stats.access_timestamps.len() > 10 {
                // 检查是否有规律的访问间隔
                let mut intervals = Vec::new();
                for i in 1..page_stats.access_timestamps.len() {
                    let interval = page_stats.access_timestamps[i]
                        .duration_since(page_stats.access_timestamps[i-1])
                        .as_millis();
                    intervals.push(interval);
                }
                
                // 如果大多数间隔相似，认为是顺序或局部访问模式
                if intervals.len() > 0 {
                    let interval_count = intervals.len() as u128;
                    let avg_interval = intervals.iter().sum::<u128>() / interval_count;
                    let variance = intervals.iter()
                        .map(|&x| (x as i128 - avg_interval as i128).pow(2))
                        .sum::<i128>() / interval_count as i128;
                    
                    if variance < (avg_interval as i128 / 5) {
                        // 定期访问可以归类为顺序访问
                        page_stats.access_pattern = Some(AccessPattern::Sequential);
                    } else {
                        page_stats.access_pattern = Some(AccessPattern::Random);
                    }
                }
            }
        }
        
        // 定期清理过期统计
        {
            let mut last_cleanup = self.last_cleanup.lock().unwrap();
            if now.duration_since(*last_cleanup) > Duration::from_secs(60) {
                self.cleanup_expired_stats();
                *last_cleanup = now;
            }
        }
    }

    /// 评估页面重要性
    pub fn evaluate_importance(&self, gva: GuestAddr, asid: u16) -> f64 {
        let page_base = GuestAddr(gva & !(4096 - 1));
        let key = (page_base, asid);
        
        let stats = self.page_stats.lock().unwrap();
        if let Some(page_stats) = stats.get(&key) {
            let frequency_score = (page_stats.access_count as f64).log10();
            let recency_score = 1.0 / (1.0 + page_stats.last_access.elapsed().as_secs_f64());
            let size_score = (page_stats.page_size as f64).log10();
            
            // 访问模式评分：定期访问的页面通常更重要
            let pattern_score = match &page_stats.access_pattern {
                Some(AccessPattern::Sequential) => 1.5,
                Some(AccessPattern::Random) => 0.8,
                Some(AccessPattern::Strided { .. }) => 1.2,
                Some(AccessPattern::Localized) => 1.3,
                Some(AccessPattern::Unknown) => 1.0,
                None => 1.0,
            };
            
            // 基于时间戳的访问频率变化评分
            let trend_score = if page_stats.access_timestamps.len() > 5 {
                // 计算最近访问频率
                let recent_window = page_stats.access_timestamps.iter()
                    .rev()
                    .take(5)
                    .collect::<Vec<_>>();
                
                if recent_window.len() > 1 {
                    let recent_interval = recent_window[0].duration_since(*recent_window[4]).as_secs_f64();
                    let recent_frequency = 4.0 / recent_interval; // 4个间隔，5次访问
                    
                    // 计算平均访问频率
                    let total_interval = page_stats.access_timestamps.last().unwrap()
                        .duration_since(*page_stats.access_timestamps.first().unwrap())
                        .as_secs_f64();
                    let avg_frequency = (page_stats.access_timestamps.len() - 1) as f64 / total_interval;
                    
                    // 如果最近频率高于平均频率，说明访问增加
                    if recent_frequency > avg_frequency {
                        1.2
                    } else if recent_frequency < avg_frequency * 0.5 {
                        0.7
                    } else {
                        1.0
                    }
                } else {
                    1.0
                }
            } else {
                1.0
            };
            
            self.config.frequency_weight * frequency_score +
            self.config.recency_weight * recency_score +
            self.config.size_weight * size_score +
            0.5 * pattern_score + // 较小权重
            0.3 * trend_score     // 较小权重
        } else {
            0.0
        }
    }

    /// 获取热点页面
    pub fn get_hot_pages(&self, asid: u16) -> Vec<(GuestAddr, f64)> {
        let stats = self.page_stats.lock().unwrap();
        let mut hot_pages: Vec<_> = stats
            .iter()
            .filter(|((_, a), page_stats)| *a == asid && page_stats.access_count >= self.config.hot_page_threshold)
            .map(|((gva, _), _)| (*gva, self.evaluate_importance(*gva, asid)))
            .collect();
        
        // 按重要性排序
        hot_pages.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        hot_pages
    }

    /// 获取冷页面
    pub fn get_cold_pages(&self, asid: u16) -> Vec<(GuestAddr, f64)> {
        let stats = self.page_stats.lock().unwrap();
        let mut cold_pages: Vec<_> = stats
            .iter()
            .filter(|((_, a), page_stats)| *a == asid && page_stats.access_count <= self.config.cold_page_threshold)
            .map(|((gva, _), _)| (*gva, self.evaluate_importance(*gva, asid)))
            .collect();
        
        // 按重要性排序（升序）
        cold_pages.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        cold_pages
    }

    /// 清理过期统计
    fn cleanup_expired_stats(&self) {
        let cutoff = Instant::now() - Duration::from_secs(300); // 5分钟
        let mut stats = self.page_stats.lock().unwrap();
        stats.retain(|_, page_stats| page_stats.last_access > cutoff);
    }
}

/// 性能监控器
#[derive(Debug)]
pub struct PerformanceMonitor {
    /// 性能历史
    performance_history: VecDeque<PerformanceSnapshot>,
    /// 监控窗口大小
    window_size: usize,
    /// 最后策略切换时间
    last_strategy_switch: Arc<Mutex<Instant>>,
}

/// 性能快照
#[derive(Debug, Clone)]
pub struct PerformanceSnapshot {
    /// 时间戳
    timestamp: Instant,
    /// TLB命中率
    hit_rate: f64,
    /// 平均访问延迟
    avg_latency: Duration,
    /// 刷新次数
    flush_count: u64,
    /// 使用的策略
    strategy: FlushStrategy,
}



impl PerformanceMonitor {
    /// 创建新的性能监控器
    pub fn new(window_size: usize) -> Self {
        Self {
            performance_history: VecDeque::with_capacity(window_size),
            window_size,
            last_strategy_switch: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// 记录性能快照
    pub fn record_snapshot(&mut self, snapshot: PerformanceSnapshot) {
        self.performance_history.push_back(snapshot);
        
        // 保持窗口大小
        if self.performance_history.len() > self.window_size {
            self.performance_history.pop_front();
        }
    }

    /// 分析性能趋势
    pub fn analyze_trend(&self) -> PerformanceTrend {
        if self.performance_history.len() < 2 {
            return PerformanceTrend::Stable;
        }
        
        let recent: Vec<_> = self.performance_history
            .iter()
            .rev()
            .take(self.window_size / 2)
            .collect();
        
        let older: Vec<_> = self.performance_history
            .iter()
            .take(self.window_size / 2)
            .collect();
        
        let recent_avg_hit_rate: f64 = recent.iter().map(|s| s.hit_rate).sum::<f64>() / recent.len() as f64;
        let older_avg_hit_rate: f64 = older.iter().map(|s| s.hit_rate).sum::<f64>() / older.len() as f64;
        
        let recent_avg_latency: Duration = recent.iter().map(|s| s.avg_latency).sum::<Duration>() / recent.len() as u32;
        let older_avg_latency: Duration = older.iter().map(|s| s.avg_latency).sum::<Duration>() / older.len() as u32;
        
        // 使用flush_count进行分析
        let recent_total_flush: u64 = recent.iter().map(|s| s.flush_count).sum();
        let older_total_flush: u64 = older.iter().map(|s| s.flush_count).sum();
        let flush_rate_change = (recent_total_flush as f64 / recent.len() as f64) - (older_total_flush as f64 / older.len() as f64);
        
        // 使用timestamp分析时间分布
        let recent_avg_duration = if recent.len() > 1 {
            let first_ts = recent.first().unwrap().timestamp;
            let last_ts = recent.last().unwrap().timestamp;
            last_ts.duration_since(first_ts).as_secs_f64() / (recent.len() - 1) as f64
        } else {
            0.0
        };
        
        let older_avg_duration = if older.len() > 1 {
            let first_ts = older.first().unwrap().timestamp;
            let last_ts = older.last().unwrap().timestamp;
            last_ts.duration_since(first_ts).as_secs_f64() / (older.len() - 1) as f64
        } else {
            0.0
        };
        
        let hit_rate_change = recent_avg_hit_rate - older_avg_hit_rate;
        let latency_change = recent_avg_latency.as_nanos() as f64 - older_avg_latency.as_nanos() as f64;
        
        // 分析采样频率变化
        let sampling_rate_change = (1.0 / recent_avg_duration.max(0.0001)) - (1.0 / older_avg_duration.max(0.0001));
        
        // 综合分析多个指标，包括采样频率变化
        let is_improving = hit_rate_change > 0.05 && latency_change < 0.0 && flush_rate_change < 0.0;
        let is_degrading = hit_rate_change < -0.05 || latency_change > 0.1 || flush_rate_change > 0.1 || sampling_rate_change > 10.0; // 采样频率突然增加10倍以上可能表示负载增加
        
        if is_improving {
            PerformanceTrend::Improving
        } else if is_degrading {
            PerformanceTrend::Degrading
        } else {
            PerformanceTrend::Stable
        }
    }

    /// 推荐最佳策略
    pub fn recommend_strategy(&self) -> FlushStrategy {
        let trend = self.analyze_trend();
        
        match trend {
            PerformanceTrend::Improving => {
                // 保持当前策略
                self.performance_history.back().map(|s| s.strategy).unwrap_or(FlushStrategy::Adaptive)
            }
            PerformanceTrend::Degrading => {
                // 尝试更激进的策略
                match self.performance_history.back().map(|s| s.strategy).unwrap_or(FlushStrategy::Adaptive) {
                    FlushStrategy::Immediate => FlushStrategy::Batched,
                    FlushStrategy::Delayed => FlushStrategy::Intelligent,
                    FlushStrategy::Batched => FlushStrategy::Intelligent,
                    FlushStrategy::Intelligent => FlushStrategy::Adaptive,
                    FlushStrategy::Adaptive => FlushStrategy::Immediate,
                }
            }
            PerformanceTrend::Stable => {
                // 使用自适应策略
                FlushStrategy::Adaptive
            }
        }
    }

    /// 检查是否应该切换策略
    pub fn should_switch_strategy(&self, config: &AdaptiveFlushConfig) -> bool {
        let last_switch = self.last_strategy_switch.lock().unwrap();
        last_switch.elapsed() >= Duration::from_secs(config.strategy_switch_interval) &&
        self.performance_history.len() >= config.min_samples
    }
}

/// 性能趋势
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerformanceTrend {
    Improving,
    Degrading,
    Stable,
}

/// 高级TLB刷新管理器
pub struct AdvancedTlbFlushManager {
    /// 基础TLB刷新管理器
    base_manager: TlbFlushManager,
    /// 高级配置
    config: AdvancedTlbFlushConfig,
    /// 访问预测器
    predictor: Arc<Mutex<AccessPredictor>>,
    /// 页面重要性评估器
    evaluator: Arc<Mutex<PageImportanceEvaluator>>,
    /// 性能监控器
    monitor: Arc<Mutex<PerformanceMonitor>>,
    /// 当前策略
    current_strategy: Arc<Mutex<FlushStrategy>>,
    /// 预测性刷新统计
    predictive_stats: Arc<PredictiveFlushStats>,
}

/// 预测性刷新统计
#[derive(Debug, Default)]
struct PredictiveFlushStats {
    /// 预测刷新次数
    predictive_flushes: AtomicU64,
    /// 成功预测次数
    successful_predictions: AtomicU64,
    /// 失败预测次数
    failed_predictions: AtomicU64,
    /// 选择性刷新次数
    selective_flushes: AtomicU64,
    /// 跳过的热点页面数
    skipped_hot_pages: AtomicU64,
}

impl AdvancedTlbFlushManager {
    /// 创建新的高级TLB刷新管理器
    pub fn new(
        config: AdvancedTlbFlushConfig,
        tlb_manager: Arc<PerCpuTlbManager>,
    ) -> Self {
        let base_manager = TlbFlushManager::new(
            config.base_config.clone(),
            tlb_manager.clone(),
        );
        
        let predictor = AccessPredictor::new(
            config.predictive_config.history_size,
            config.predictive_config.prediction_window,
        );
        
        let evaluator = PageImportanceEvaluator::new(config.selective_config.clone());
        
        let monitor = PerformanceMonitor::new(config.adaptive_config.monitoring_window);
        
        Self {
            base_manager,
            config,
            predictor: Arc::new(Mutex::new(predictor)),
            evaluator: Arc::new(Mutex::new(evaluator)),
            monitor: Arc::new(Mutex::new(monitor)),
            current_strategy: Arc::new(Mutex::new(FlushStrategy::Adaptive)),
            predictive_stats: Arc::new(PredictiveFlushStats::default()),
        }
    }

    /// 使用默认配置创建高级TLB刷新管理器
    pub fn with_default_config(tlb_manager: Arc<PerCpuTlbManager>) -> Self {
        Self::new(AdvancedTlbFlushConfig::default(), tlb_manager)
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> TlbFlushStatsSnapshot {
        self.base_manager.get_stats()
    }

    /// 重置统计信息
    pub fn reset_stats(&self) {
        self.base_manager.reset_stats();
    }

    /// 记录TLB访问
    pub fn record_access(&self, gva: GuestAddr, asid: u16, access_type: AccessType) {
        // 记录到基础管理器
        self.base_manager.record_access(gva, asid);
        
        // 记录到预测器
        if self.config.predictive_config.enabled {
            let mut predictor = self.predictor.lock().unwrap();
            predictor.record_access(gva, asid);
        }
        
        // 记录到评估器
        if self.config.selective_config.enabled {
            let evaluator = self.evaluator.lock().unwrap();
            evaluator.record_access(gva, asid, access_type);
        }
    }

    /// 请求TLB刷新
    pub fn request_flush(&self, mut request: FlushRequest) -> Result<(), VmError> {
        // 自适应策略选择
        if self.config.adaptive_config.enabled {
            self.adapt_strategy();
        }
        
        // 预测性刷新
        if self.config.predictive_config.enabled {
            self.perform_predictive_flush(&request)?;
        }
        
        // 选择性刷新优化
        if self.config.selective_config.enabled {
            request = self.optimize_flush_selectively(request)?;
        }
        
        // 执行刷新
        self.base_manager.request_flush(request)
    }

    /// 执行预测性刷新
    fn perform_predictive_flush(&self, request: &FlushRequest) -> Result<(), VmError> {
        let predictor = self.predictor.lock().unwrap();
        
        // 检查预测准确率
        if predictor.get_accuracy() < self.config.predictive_config.accuracy_threshold {
            return Ok(());
        }
        
        // 预测下一个访问
        let predicted_pages = predictor.predict_next_accesses(request.asid);
        
        if predicted_pages.is_empty() || 
           predicted_pages.len() > self.config.predictive_config.max_predictive_flushes {
            return Ok(());
        }
        
        // 执行预测性刷新
        for &page in &predicted_pages {
            let predictive_request = FlushRequest::new(
                FlushScope::SinglePage,
                page,
                page,
                request.asid,
                request.priority - 1, // 低优先级
                request.source_cpu,
            );
            
            self.base_manager.request_flush(predictive_request)?;
            self.predictive_stats.predictive_flushes.fetch_add(1, Ordering::Relaxed);
        }
        
        Ok(())
    }

    /// 优化刷新选择
    fn optimize_flush_selectively(&self, request: FlushRequest) -> Result<FlushRequest, VmError> {
        let evaluator = self.evaluator.lock().unwrap();
        
        match request.scope {
            FlushScope::PageRange => {
                // 获取范围内的热点页面
                let hot_pages = evaluator.get_hot_pages(request.asid);
                let cold_pages = evaluator.get_cold_pages(request.asid);
                
                // 如果热点页面比例高，缩小刷新范围
                let hot_in_range = hot_pages.iter()
                    .filter(|(gva, _)| gva.0 >= request.gva.0 && gva.0 <= request.end_gva.0)
                    .count();
                
                let total_in_range = page_count_between(request.gva, request.end_gva);
                
                if hot_in_range as f64 / total_in_range as f64 > 0.7 {
                    // 只刷新冷页面
                    let mut optimized_requests = Vec::new();
                    
                    for (cold_page, _) in cold_pages {
                        if cold_page.0 >= request.gva.0 && cold_page.0 <= request.end_gva.0 {
                            let cold_request = FlushRequest::new(
                                FlushScope::SinglePage,
                                cold_page,
                                cold_page,
                                request.asid,
                                request.priority,
                                request.source_cpu,
                            );
                            optimized_requests.push(cold_request);
                        }
                    }
                    
                    // 执行选择性刷新
                    for cold_request in optimized_requests {
                        self.base_manager.request_flush(cold_request)?;
                        self.predictive_stats.selective_flushes.fetch_add(1, Ordering::Relaxed);
                    }
                    
                    // 跳过原始请求
                    self.predictive_stats.skipped_hot_pages.fetch_add(hot_in_range as u64, Ordering::Relaxed);
                    
                    // 返回一个空请求
                    return Ok(FlushRequest::new(
                        FlushScope::SinglePage,
                        GuestAddr(0),
                        GuestAddr(0),
                        request.asid,
                        0,
                        request.source_cpu,
                    ));
                }
            }
            _ => {}
        }
        
        Ok(request)
    }

    /// 自适应策略调整
    fn adapt_strategy(&self) {
        let monitor = self.monitor.lock().unwrap();
        let mut current_strategy = self.current_strategy.lock().unwrap();
        
        if !monitor.should_switch_strategy(&self.config.adaptive_config) {
            return;
        }
        
        let recommended_strategy = monitor.recommend_strategy();
        
        if *current_strategy != recommended_strategy {
            *current_strategy = recommended_strategy;
            // 更新最后策略切换时间
            let mut last_switch = monitor.last_strategy_switch.lock().unwrap();
            *last_switch = Instant::now();
        }
    }

    /// 获取预测性刷新统计
    pub fn get_predictive_stats(&self) -> PredictiveFlushStatsSnapshot {
        PredictiveFlushStatsSnapshot {
            predictive_flushes: self.predictive_stats.predictive_flushes.load(Ordering::Relaxed),
            successful_predictions: self.predictive_stats.successful_predictions.load(Ordering::Relaxed),
            failed_predictions: self.predictive_stats.failed_predictions.load(Ordering::Relaxed),
            selective_flushes: self.predictive_stats.selective_flushes.load(Ordering::Relaxed),
            skipped_hot_pages: self.predictive_stats.skipped_hot_pages.load(Ordering::Relaxed),
            prediction_accuracy: {
                let total = self.predictive_stats.successful_predictions.load(Ordering::Relaxed) +
                           self.predictive_stats.failed_predictions.load(Ordering::Relaxed);
                if total > 0 {
                    self.predictive_stats.successful_predictions.load(Ordering::Relaxed) as f64 / total as f64
                } else {
                    0.0
                }
            },
        }
    }
}

/// 预测性刷新统计快照
#[derive(Debug, Clone)]
pub struct PredictiveFlushStatsSnapshot {
    pub predictive_flushes: u64,
    pub successful_predictions: u64,
    pub failed_predictions: u64,
    pub selective_flushes: u64,
    pub skipped_hot_pages: u64,
    pub prediction_accuracy: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tlb::per_cpu_tlb::PerCpuTlbManager;

    #[test]
    fn test_access_predictor() {
        let mut predictor = AccessPredictor::new(100, 4);
        
        // 记录顺序访问模式
        for i in 0..10 {
            predictor.record_access(i * 4096, 0);
        }
        
        // 预测下一个访问
        let predicted = predictor.predict_next_accesses(0);
        assert!(!predicted.is_empty());
        
        // 验证预测
        let is_correct = predictor.validate_prediction(&predicted, 10 * 4096);
        assert!(is_correct);
        
        // 检查准确率
        let accuracy = predictor.get_accuracy();
        assert!(accuracy > 0.0);
    }

    #[test]
    fn test_page_importance_evaluator() {
        let mut evaluator = PageImportanceEvaluator::new(SelectiveFlushConfig::default());
        
        // 记录多次访问
        for _ in 0..10 {
            evaluator.record_access(0x1000, 0, AccessType::Read);
        }
        
        // 评估重要性
        let importance = evaluator.evaluate_importance(0x1000, 0);
        assert!(importance > 0.0);
        
        // 获取热点页面
        let hot_pages = evaluator.get_hot_pages(0);
        assert!(!hot_pages.is_empty());
        assert_eq!(hot_pages[0].0, 0x1000);
    }

    #[test]
    fn test_advanced_flush_manager() {
        let tlb_manager = Arc::new(PerCpuTlbManager::with_default_config());
        let flush_manager = AdvancedTlbFlushManager::with_default_config(tlb_manager);
        
        // 记录访问
        flush_manager.record_access(0x1000, 0, AccessType::Read);
        
        // 请求刷新
        let request = FlushRequest::new(
            FlushScope::SinglePage,
            0x1000,
            0x1000,
            0,
            10,
            0,
        );
        
        let result = flush_manager.request_flush(request);
        assert!(result.is_ok());
        
        // 获取统计信息
        let stats = flush_manager.get_stats();
        assert_eq!(stats.total_requests, 1);
        
        let predictive_stats = flush_manager.get_predictive_stats();
        assert_eq!(predictive_stats.predictive_flushes, 0); // 预测性刷新可能不会触发
    }
}