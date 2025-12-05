//! EWMA（指数加权移动平均）热点检测算法
//!
//! 使用EWMA算法优化热点检测，提供更平滑和响应更快的热点识别
//! 整合多维度评分系统，考虑代码复杂度、执行时间等多维度因素

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use vm_core::GuestAddr;

/// EWMA热点检测器
///
/// 使用指数加权移动平均算法，对执行频率和执行时间进行平滑处理
/// 整合多维度评分系统，考虑代码复杂度、执行时间等多维度因素
pub struct EwmaHotspotDetector {
    /// EWMA执行频率（每个地址）
    frequency_ewma: Arc<RwLock<HashMap<GuestAddr, f64>>>,
    /// EWMA执行时间（每个地址）
    execution_time_ewma: Arc<RwLock<HashMap<GuestAddr, f64>>>,
    /// 综合热点评分（每个地址）
    hotspot_scores: Arc<RwLock<HashMap<GuestAddr, f64>>>,
    /// 每个地址的执行记录（用于多维度评分）
    execution_records: Arc<RwLock<HashMap<GuestAddr, VecDeque<ExecutionRecord>>>>,
    /// 热点阈值缓存（用于自适应阈值）
    threshold_cache: Arc<RwLock<HashMap<GuestAddr, u64>>>,
    /// 配置
    config: EwmaHotspotConfig,
    /// 统计信息
    stats: Arc<RwLock<EwmaHotspotStats>>,
    /// 上次清理时间
    last_cleanup: Arc<Mutex<Instant>>,
}


/// EWMA热点检测配置
#[derive(Debug, Clone)]
pub struct EwmaHotspotConfig {
    /// 频率EWMA的alpha参数（0-1），越大越重视最新数据
    pub frequency_alpha: f64,
    /// 执行时间EWMA的alpha参数
    pub execution_time_alpha: f64,
    /// 热点阈值（综合评分）
    pub hotspot_threshold: f64,
    /// 最小执行次数（低于此值不考虑为热点）
    pub min_execution_count: u64,
    /// 最小执行时间阈值（微秒）
    pub min_execution_time_us: u64,
    /// 频率权重
    pub frequency_weight: f64,
    /// 执行时间权重
    pub execution_time_weight: f64,
    /// 复杂度权重（多维度评分）
    pub complexity_weight: f64,
    /// 基础执行次数阈值（用于自适应阈值计算）
    pub base_threshold: u64,
    /// 时间窗口大小（毫秒，用于多维度评分）
    pub time_window_ms: u64,
    /// 热点衰减因子（0-1）
    pub decay_factor: f64,
}


/// 执行记录（用于多维度评分）
#[derive(Debug, Clone)]
struct ExecutionRecord {
    /// 执行时间戳
    timestamp: Instant,
    /// 执行持续时间（微秒）
    duration_us: u64,
    /// 代码复杂度评分
    complexity_score: f64,
}

impl Default for EwmaHotspotConfig {
    fn default() -> Self {
        Self {
            frequency_alpha: 0.3,      // 30%权重给最新数据
            execution_time_alpha: 0.2, // 20%权重给最新数据
            hotspot_threshold: 1.0,
            min_execution_count: 10,
            min_execution_time_us: 5,
            frequency_weight: 0.4,
            execution_time_weight: 0.4,
            complexity_weight: 0.2, // 新增复杂度权重
            base_threshold: 100,
            time_window_ms: 1000,
            decay_factor: 0.95,
        }
    }
}

/// EWMA热点检测统计信息
#[derive(Debug, Clone, Default)]
pub struct EwmaHotspotStats {
    /// 总检测次数
    pub total_detections: u64,
    /// 热点识别次数
    pub hotspot_identifications: u64,
    /// 平均EWMA频率
    pub avg_frequency_ewma: f64,
    /// 平均EWMA执行时间
    pub avg_execution_time_ewma: f64,
    /// 当前热点数量
    pub current_hotspots: usize,
    /// 平均执行时间（微秒）
    pub avg_execution_time_us: f64,
    /// 最大执行时间（微秒）
    pub max_execution_time_us: u64,
    /// 清理次数
    pub cleanup_count: u64,
}

/// 热点检测统计信息（向后兼容别名）
pub type HotspotStats = EwmaHotspotStats;

impl EwmaHotspotDetector {
    /// 创建新的EWMA热点检测器
    pub fn new(config: EwmaHotspotConfig) -> Self {
        Self {
            frequency_ewma: Arc::new(RwLock::new(HashMap::new())),
            execution_time_ewma: Arc::new(RwLock::new(HashMap::new())),
            hotspot_scores: Arc::new(RwLock::new(HashMap::new())),
            execution_records: Arc::new(RwLock::new(HashMap::new())),
            threshold_cache: Arc::new(RwLock::new(HashMap::new())),
            config,
            stats: Arc::new(RwLock::new(EwmaHotspotStats::default())),
            last_cleanup: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// 记录执行
    ///
    /// 使用EWMA算法更新执行频率和执行时间的估计值
    /// 支持多维度评分（包括复杂度）
    pub fn record_execution(&self, addr: GuestAddr, duration_us: u64) {
        self.record_execution_with_complexity(addr, duration_us, 1.0)
    }

    /// 记录执行（带复杂度评分）
    ///
    /// 使用EWMA算法更新执行频率和执行时间的估计值
    /// 支持多维度评分（包括复杂度）
    pub fn record_execution_with_complexity(
        &self,
        addr: GuestAddr,
        duration_us: u64,
        complexity_score: f64,
    ) {
        // 添加执行记录（用于多维度评分）
        {
            let record = ExecutionRecord {
                timestamp: Instant::now(),
                duration_us,
                complexity_score,
            };
            let mut records = self.execution_records.write().unwrap();
            let addr_records = records.entry(addr).or_insert_with(VecDeque::new);
            addr_records.push_back(record);

            // 限制记录数量
            while addr_records.len() > 1000 {
                addr_records.pop_front();
            }
        }

        // 更新频率EWMA
        {
            let mut freq_map = self.frequency_ewma.write().unwrap();
            let current_freq = freq_map.get(&addr).copied().unwrap_or(0.0);
            // 每次执行增加1，使用EWMA平滑
            let new_freq = self.config.frequency_alpha * 1.0
                + (1.0 - self.config.frequency_alpha) * current_freq;
            freq_map.insert(addr, new_freq);
        }

        // 更新执行时间EWMA
        {
            let mut time_map = self.execution_time_ewma.write().unwrap();
            let current_time = time_map.get(&addr).copied().unwrap_or(0.0);
            let duration_f64 = duration_us as f64;
            let new_time = self.config.execution_time_alpha * duration_f64
                + (1.0 - self.config.execution_time_alpha) * current_time;
            time_map.insert(addr, new_time);
        }

        // 更新综合热点评分（使用多维度评分）
        self.update_hotspot_score_multidimensional(addr);

        // 定期清理旧记录
        self.cleanup_old_records();

        // 更新统计
        {
            let mut stats = self.stats.write().unwrap();
            stats.total_detections += 1;
            stats.avg_execution_time_us = (stats.avg_execution_time_us
                * (stats.total_detections - 1) as f64
                + duration_us as f64)
                / stats.total_detections as f64;
            stats.max_execution_time_us = stats.max_execution_time_us.max(duration_us);
        }
    }

    /// 更新热点评分（EWMA方法）
    fn update_hotspot_score(&self, addr: GuestAddr) {
        self.update_hotspot_score_multidimensional(addr);
    }

    /// 更新热点评分（多维度方法）
    fn update_hotspot_score_multidimensional(&self, addr: GuestAddr) {
        // 首先使用EWMA方法计算基础评分
        let freq_ewma = {
            let freq_map = self.frequency_ewma.read().unwrap();
            freq_map.get(&addr).copied().unwrap_or(0.0)
        };

        let time_ewma = {
            let time_map = self.execution_time_ewma.read().unwrap();
            time_map.get(&addr).copied().unwrap_or(0.0)
        };

        // 归一化频率评分（基于阈值）
        let normalized_freq = (freq_ewma / self.config.min_execution_count as f64).min(2.0);

        // 归一化执行时间评分
        let normalized_time = if time_ewma >= self.config.min_execution_time_us as f64 {
            (time_ewma / self.config.min_execution_time_us as f64).min(2.0)
        } else {
            0.0
        };

        // 计算复杂度评分（从执行记录中获取）
        let complexity_score = {
            let records_map = self.execution_records.read().unwrap();
            if let Some(records) = records_map.get(&addr) {
                if !records.is_empty() {
                    let now = Instant::now();
                    let window_duration = Duration::from_millis(self.config.time_window_ms);
                    let recent_records: Vec<_> = records
                        .iter()
                        .filter(|r| now.duration_since(r.timestamp) <= window_duration)
                        .collect();

                    if !recent_records.is_empty() {
                        recent_records
                            .iter()
                            .map(|r| r.complexity_score)
                            .sum::<f64>()
                            / recent_records.len() as f64
                    } else {
                        1.0
                    }
                } else {
                    1.0
                }
            } else {
                1.0
            }
        };

        // 加权组合（包括复杂度）
        let base_score = self.config.frequency_weight * normalized_freq
            + self.config.execution_time_weight * normalized_time
            + self.config.complexity_weight * complexity_score;

        // 应用衰减因子
        let records_map = self.execution_records.read().unwrap();
        let record_count = records_map.get(&addr).map(|r| r.len()).unwrap_or(0);
        let decayed_score = base_score * self.config.decay_factor.powf(record_count as f64 / 100.0);

        // 更新评分
        {
            let mut scores = self.hotspot_scores.write().unwrap();
            scores.insert(addr, decayed_score);

            // 更新统计
            let mut stats = self.stats.write().unwrap();
            if decayed_score >= self.config.hotspot_threshold {
                stats.hotspot_identifications += 1;
            }
            stats.current_hotspots = scores
                .values()
                .filter(|&&s| s >= self.config.hotspot_threshold)
                .count();
        }
    }

    /// 检查是否为热点
    pub fn is_hotspot(&self, addr: GuestAddr) -> bool {
        let scores = self.hotspot_scores.read().unwrap();
        scores
            .get(&addr)
            .map(|&score| score >= self.config.hotspot_threshold)
            .unwrap_or(false)
    }

    /// 获取热点评分
    pub fn get_hotspot_score(&self, addr: GuestAddr) -> f64 {
        let scores = self.hotspot_scores.read().unwrap();
        scores.get(&addr).copied().unwrap_or(0.0)
    }

    /// 获取EWMA频率
    pub fn get_frequency_ewma(&self, addr: GuestAddr) -> f64 {
        let freq_map = self.frequency_ewma.read().unwrap();
        freq_map.get(&addr).copied().unwrap_or(0.0)
    }

    /// 获取EWMA执行时间
    pub fn get_execution_time_ewma(&self, addr: GuestAddr) -> f64 {
        let time_map = self.execution_time_ewma.read().unwrap();
        time_map.get(&addr).copied().unwrap_or(0.0)
    }

    /// 获取统计信息
    pub fn stats(&self) -> EwmaHotspotStats {
        let stats = self.stats.read().unwrap();

        // 计算平均EWMA值
        let freq_map = self.frequency_ewma.read().unwrap();
        let time_map = self.execution_time_ewma.read().unwrap();

        let avg_freq = if !freq_map.is_empty() {
            freq_map.values().sum::<f64>() / freq_map.len() as f64
        } else {
            0.0
        };

        let avg_time = if !time_map.is_empty() {
            time_map.values().sum::<f64>() / time_map.len() as f64
        } else {
            0.0
        };

        EwmaHotspotStats {
            total_detections: stats.total_detections,
            hotspot_identifications: stats.hotspot_identifications,
            avg_frequency_ewma: avg_freq,
            avg_execution_time_ewma: avg_time,
            current_hotspots: stats.current_hotspots,
            avg_execution_time_us: stats.avg_execution_time_us,
            max_execution_time_us: stats.max_execution_time_us,
            cleanup_count: stats.cleanup_count,
        }
    }

    /// 获取统计信息（向后兼容方法）
    pub fn get_stats(&self) -> EwmaHotspotStats {
        self.stats()
    }

    /// 获取自适应阈值
    pub fn get_adaptive_threshold(&self, addr: GuestAddr) -> u64 {
        // 检查缓存
        {
            let thresholds = self.threshold_cache.read().unwrap();
            if let Some(&threshold) = thresholds.get(&addr) {
                return threshold;
            }
        }

        // 计算自适应阈值
        let threshold = self.calculate_adaptive_threshold(addr);

        // 更新缓存
        {
            let mut thresholds = self.threshold_cache.write().unwrap();
            thresholds.insert(addr, threshold);
        }

        threshold
    }

    /// 计算自适应阈值
    fn calculate_adaptive_threshold(&self, addr: GuestAddr) -> u64 {
        let records = {
            let records_map = self.execution_records.read().unwrap();
            records_map.get(&addr).cloned().unwrap_or_default()
        };

        if records.is_empty() {
            return self.config.base_threshold;
        }

        // 基于执行时间调整阈值
        let avg_execution_time =
            records.iter().map(|r| r.duration_us).sum::<u64>() / records.len() as u64;

        let time_factor = if avg_execution_time > self.config.min_execution_time_us {
            // 执行时间长的块，降低阈值
            0.8
        } else {
            // 执行时间短的块，提高阈值
            1.2
        };

        // 基于复杂度调整阈值
        let avg_complexity =
            records.iter().map(|r| r.complexity_score).sum::<f64>() / records.len() as f64;
        let complexity_factor = if avg_complexity > 1.0 {
            // 复杂代码，降低阈值
            0.9
        } else {
            // 简单代码，提高阈值
            1.1
        };

        // 计算最终阈值
        let adjusted_threshold =
            (self.config.base_threshold as f64 * time_factor * complexity_factor) as u64;

        // 限制在合理范围内
        adjusted_threshold.max(10).min(1000)
    }

    /// 清理旧数据
    ///
    /// 移除长时间未访问的地址数据
    pub fn cleanup_old_data(&self, max_age_seconds: u64) {
        self.cleanup_old_records();
    }

    /// 清理旧记录
    fn cleanup_old_records(&self) {
        let mut last_cleanup = self.last_cleanup.lock().unwrap();
        let now = Instant::now();

        // 每秒清理一次
        if now.duration_since(*last_cleanup) < Duration::from_secs(1) {
            return;
        }

        *last_cleanup = now;

        let cutoff_time = now - Duration::from_millis(self.config.time_window_ms * 10); // 保留10个时间窗口

        let mut total_removed = 0;
        {
            let mut records = self.execution_records.write().unwrap();
            for (_, addr_records) in records.iter_mut() {
                let original_len = addr_records.len();
                addr_records.retain(|r| r.timestamp > cutoff_time);
                total_removed += original_len - addr_records.len();
            }
        }

        // 清理缓存中的无效条目
        {
            let mut scores = self.hotspot_scores.write().unwrap();
            let mut thresholds = self.threshold_cache.write().unwrap();
            let mut freq_map = self.frequency_ewma.write().unwrap();
            let mut time_map = self.execution_time_ewma.write().unwrap();

            let threshold = self.config.hotspot_threshold * 0.1; // 保留10%阈值以上的数据

            scores.retain(|&addr, score| {
                if *score < threshold {
                    freq_map.remove(&addr);
                    time_map.remove(&addr);
                    thresholds.remove(&addr);
                    false
                } else {
                    // 应用衰减
                    *score *= self.config.decay_factor;
                    *score > 0.01 // 移除过低的评分
                }
            });

            // 清理没有执行记录的阈值缓存
            let records_map = self.execution_records.read().unwrap();
            thresholds.retain(|addr, _| records_map.contains_key(addr));
        }

        // 更新统计信息
        {
            let mut stats = self.stats.write().unwrap();
            stats.cleanup_count += 1;
            let scores = self.hotspot_scores.read().unwrap();
            stats.current_hotspots = scores
                .values()
                .filter(|&&s| s >= self.config.hotspot_threshold)
                .count();
        }

        if total_removed > 0 {
            tracing::debug!("Hotspot detector cleaned up {} old records", total_removed);
        }
    }

    /// 获取热点列表
    pub fn get_hotspots(&self) -> Vec<(GuestAddr, f64)> {
        let scores = self.hotspot_scores.read().unwrap();
        let mut hotspots: Vec<_> = scores
            .iter()
            .filter(|&(_, &score)| score >= self.config.hotspot_threshold)
            .map(|(&addr, &score)| (addr, score))
            .collect();

        // 按评分降序排序
        hotspots.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        hotspots
    }

    /// 重置统计信息
    pub fn reset_stats(&self) {
        let mut stats = self.stats.write().unwrap();
        *stats = EwmaHotspotStats::default();
    }

    /// 生成诊断报告
    pub fn diagnostic_report(&self) -> String {
        let stats = self.stats();
        let hotspots = self.get_hotspots();

        format!(
            r#"=== EWMA Hotspot Detection Report ===

Configuration:
  Frequency Alpha: {:.2}
  Execution Time Alpha: {:.2}
  Hotspot Threshold: {:.2}
  Min Execution Count: {}
  Min Execution Time: {}μs
  Frequency Weight: {:.2}
  Execution Time Weight: {:.2}
  Complexity Weight: {:.2}
  Base Threshold: {}
  Time Window: {}ms
  Decay Factor: {:.2}

Statistics:
  Total Detections: {}
  Hotspot Identifications: {}
  Average Execution Time: {:.2}μs
  Maximum Execution Time: {}μs
  Cleanup Count: {}
  Current Hotspots: {}

Top 10 Hotspots:
{}
"#,
            self.config.frequency_alpha,
            self.config.execution_time_alpha,
            self.config.hotspot_threshold,
            self.config.min_execution_count,
            self.config.min_execution_time_us,
            self.config.frequency_weight,
            self.config.execution_time_weight,
            self.config.complexity_weight,
            self.config.base_threshold,
            self.config.time_window_ms,
            self.config.decay_factor,
            stats.total_detections,
            stats.hotspot_identifications,
            stats.avg_execution_time_us,
            stats.max_execution_time_us,
            stats.cleanup_count,
            stats.current_hotspots,
            hotspots
                .iter()
                .take(10)
                .map(|(addr, score)| format!("  0x{:x}: {:.2}", addr, score))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

impl Default for EwmaHotspotDetector {
    fn default() -> Self {
        Self::new(EwmaHotspotConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ewma_hotspot_detection() {
        let detector = EwmaHotspotDetector::new(EwmaHotspotConfig::default());

        // 记录多次执行
        for _ in 0..20 {
            detector.record_execution(0x1000, 10);
        }

        // 应该识别为热点
        assert!(detector.is_hotspot(0x1000));
        assert!(detector.get_hotspot_score(0x1000) >= 1.0);
    }

    #[test]
    fn test_ewma_smoothing() {
        let detector = EwmaHotspotDetector::new(EwmaHotspotConfig::default());

        // 记录执行
        detector.record_execution(0x2000, 10);
        let freq1 = detector.get_frequency_ewma(0x2000);

        detector.record_execution(0x2000, 10);
        let freq2 = detector.get_frequency_ewma(0x2000);

        // EWMA应该平滑增长
        assert!(freq2 > freq1);
    }

    #[test]
    fn test_multidimensional_scoring() {
        let detector = EwmaHotspotDetector::new(EwmaHotspotConfig::default());

        // 记录多次执行（带复杂度）
        for _ in 0..100 {
            detector.record_execution_with_complexity(0x1000, 50, 1.0);
        }

        // 应该被识别为热点
        assert!(detector.is_hotspot(0x1000));

        // 获取热点列表
        let hotspots = detector.get_hotspots();
        assert!(!hotspots.is_empty());
        assert_eq!(hotspots[0].0, 0x1000);
    }

    #[test]
    fn test_adaptive_threshold() {
        let detector = EwmaHotspotDetector::new(EwmaHotspotConfig::default());

        // 记录长时间执行
        for _ in 0..50 {
            detector.record_execution_with_complexity(0x2000, 1000, 2.0); // 长时间，高复杂度
        }

        let threshold = detector.get_adaptive_threshold(0x2000);

        // 由于执行时间长且复杂度高，阈值应该降低
        assert!(threshold < 100); // 基础阈值是100
    }

    #[test]
    fn test_cleanup_old_records() {
        let config = EwmaHotspotConfig {
            time_window_ms: 10, // 10ms窗口
            ..Default::default()
        };
        let detector = EwmaHotspotDetector::new(config);

        // 记录执行
        detector.record_execution(0x3000, 50);

        // 等待超过时间窗口
        std::thread::sleep(std::time::Duration::from_millis(20));

        // 再次记录以触发清理
        detector.record_execution(0x4000, 50);

        // 检查统计信息
        let stats = detector.get_stats();
        assert!(stats.cleanup_count > 0);
    }

    #[test]
    fn test_diagnostic_report() {
        let detector = EwmaHotspotDetector::new(EwmaHotspotConfig::default());

        // 记录一些执行
        for _ in 0..50 {
            detector.record_execution_with_complexity(0x1000, 100, 1.5);
        }

        let report = detector.diagnostic_report();
        assert!(report.contains("EWMA Hotspot Detection Report"));
        assert!(report.contains("Statistics:"));
    }
}
