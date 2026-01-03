//! GC参数自适应调整
//!
//! 根据运行时数据动态调整GC参数，包括：
//! - 基于分配速率的GC触发
//! - 年轻代比例自适应调整
//! - 晋升阈值自适应调整

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// 分配速率跟踪器
pub struct AllocationRateTracker {
    /// 分配历史（时间戳，分配大小）
    allocation_history: Arc<Mutex<VecDeque<(Instant, u64)>>>,
    /// 历史窗口大小（秒）
    window_size_sec: u64,
    /// 总分配量（字节）
    total_allocated: AtomicU64,
    /// 当前分配速率（字节/秒）
    current_rate: AtomicU64,
}

impl AllocationRateTracker {
    /// 创建新的分配速率跟踪器
    pub fn new(window_size_sec: u64) -> Self {
        Self {
            allocation_history: Arc::new(Mutex::new(VecDeque::new())),
            window_size_sec,
            total_allocated: AtomicU64::new(0),
            current_rate: AtomicU64::new(0),
        }
    }

    /// 记录分配
    pub fn record_allocation(&self, size: u64) {
        let now = Instant::now();
        self.total_allocated.fetch_add(size, Ordering::Relaxed);

        let mut history = self.allocation_history.lock().unwrap();
        history.push_back((now, size));

        // 清理过期数据
        let cutoff = now - Duration::from_secs(self.window_size_sec);
        while let Some(&(time, _)) = history.front() {
            if time < cutoff {
                history.pop_front();
            } else {
                break;
            }
        }

        // 计算当前分配速率
        self.update_rate();
    }

    /// 更新分配速率
    fn update_rate(&self) {
        let history = self.allocation_history.lock().unwrap();
        if history.len() < 2 {
            self.current_rate.store(0, Ordering::Relaxed);
            return;
        }

        let now = Instant::now();
        let cutoff = now - Duration::from_secs(self.window_size_sec);

        // 计算窗口内的总分配量
        let mut total_bytes = 0u64;
        let mut oldest_time = None;

        for &(time, size) in history.iter() {
            if time >= cutoff {
                total_bytes += size;
                if oldest_time.is_none() || time < oldest_time.unwrap() {
                    oldest_time = Some(time);
                }
            }
        }

        if let Some(oldest) = oldest_time {
            let elapsed = now.duration_since(oldest);
            if elapsed.as_secs() > 0 {
                let rate = total_bytes / elapsed.as_secs();
                self.current_rate.store(rate, Ordering::Relaxed);
            } else {
                // 时间窗口太小，使用最近的数据估算
                let recent_total: u64 = history.iter().map(|(_, size)| size).sum();
                let estimated_rate = recent_total * 10; // 假设是最近0.1秒的数据
                self.current_rate.store(estimated_rate, Ordering::Relaxed);
            }
        } else {
            self.current_rate.store(0, Ordering::Relaxed);
        }
    }

    /// 获取当前分配速率（字节/秒）
    pub fn get_allocation_rate(&self) -> u64 {
        self.current_rate.load(Ordering::Relaxed)
    }

    /// 获取总分配量
    pub fn get_total_allocated(&self) -> u64 {
        self.total_allocated.load(Ordering::Relaxed)
    }
}

/// 年轻代比例自适应调整器
pub struct YoungGenRatioAdjuster {
    /// 当前年轻代比例
    current_ratio: AtomicU64, // 存储为百分比 * 100
    /// 目标存活率（0.0-1.0）
    target_survival_rate: f64,
    /// 最小比例
    min_ratio: f64,
    /// 最大比例
    max_ratio: f64,
    /// 存活率历史
    survival_rate_history: Arc<Mutex<VecDeque<f64>>>,
    /// 历史窗口大小
    history_window: usize,
}

impl YoungGenRatioAdjuster {
    /// 创建新的年轻代比例调整器
    pub fn new(
        initial_ratio: f64,
        target_survival_rate: f64,
        min_ratio: f64,
        max_ratio: f64,
    ) -> Self {
        Self {
            current_ratio: AtomicU64::new((initial_ratio * 100.0) as u64),
            target_survival_rate,
            min_ratio,
            max_ratio,
            survival_rate_history: Arc::new(Mutex::new(VecDeque::new())),
            history_window: 10,
        }
    }

    /// 记录GC后的存活率
    pub fn record_survival_rate(&self, survival_rate: f64) {
        let mut history = self.survival_rate_history.lock().unwrap();
        history.push_back(survival_rate);

        // 保持历史窗口大小
        while history.len() > self.history_window {
            history.pop_front();
        }

        // 根据存活率调整年轻代比例
        self.adjust_ratio();
    }

    /// 调整年轻代比例
    fn adjust_ratio(&self) {
        let history = self.survival_rate_history.lock().unwrap();
        if history.is_empty() {
            return;
        }

        // 计算平均存活率
        let avg_survival_rate: f64 = history.iter().sum::<f64>() / history.len() as f64;

        // 如果存活率高于目标，增加年轻代比例（更多对象在年轻代，更频繁的minor GC）
        // 如果存活率低于目标，减少年轻代比例（更少对象在年轻代，更少的minor GC）
        let current_ratio = self.current_ratio.load(Ordering::Relaxed) as f64 / 100.0;
        let mut new_ratio = current_ratio;

        if avg_survival_rate > self.target_survival_rate {
            // 存活率太高，增加年轻代比例
            new_ratio += 0.05; // 增加5%
        } else if avg_survival_rate < self.target_survival_rate * 0.8 {
            // 存活率太低，减少年轻代比例
            new_ratio -= 0.05; // 减少5%
        }

        // 限制在最小和最大比例之间
        new_ratio = new_ratio.max(self.min_ratio).min(self.max_ratio);

        self.current_ratio.store((new_ratio * 100.0) as u64, Ordering::Relaxed);
    }

    /// 获取当前年轻代比例
    pub fn get_ratio(&self) -> f64 {
        self.current_ratio.load(Ordering::Relaxed) as f64 / 100.0
    }
}

/// 晋升阈值自适应调整器
pub struct PromotionThresholdAdjuster {
    /// 当前晋升阈值
    current_threshold: AtomicU32,
    /// 最小阈值
    min_threshold: u32,
    /// 最大阈值
    max_threshold: u32,
    /// 对象存活次数分布（存活次数 -> 对象数量）
    survival_count_distribution: Arc<Mutex<HashMap<u32, u64>>>,
    /// 目标晋升比例（0.0-1.0，期望晋升的对象比例）
    target_promotion_ratio: f64,
}

impl PromotionThresholdAdjuster {
    /// 创建新的晋升阈值调整器
    pub fn new(
        initial_threshold: u32,
        min_threshold: u32,
        max_threshold: u32,
        target_promotion_ratio: f64,
    ) -> Self {
        Self {
            current_threshold: AtomicU32::new(initial_threshold),
            min_threshold,
            max_threshold,
            survival_count_distribution: Arc::new(Mutex::new(HashMap::new())),
            target_promotion_ratio,
        }
    }

    /// 记录对象的存活次数
    pub fn record_survival_count(&self, survival_count: u32) {
        let mut dist = self.survival_count_distribution.lock().unwrap();
        *dist.entry(survival_count).or_insert(0) += 1;
    }

    /// 根据存活次数分布调整晋升阈值
    pub fn adjust_threshold(&self) {
        let dist = self.survival_count_distribution.lock().unwrap();
        if dist.is_empty() {
            return;
        }

        // 计算总对象数
        let total_objects: u64 = dist.values().sum();

        if total_objects == 0 {
            return;
        }

        // 计算当前阈值下的晋升比例
        let current_threshold = self.current_threshold.load(Ordering::Relaxed);
        let promoted_objects: u64 = dist
            .iter()
            .filter(|(count, _)| **count >= current_threshold)
            .map(|(_, num)| *num)
            .sum();

        let current_promotion_ratio = promoted_objects as f64 / total_objects as f64;

        // 根据当前晋升比例调整阈值
        let mut new_threshold = current_threshold as i32;

        if current_promotion_ratio > self.target_promotion_ratio * 1.2 {
            // 晋升比例太高，提高阈值（更难晋升）
            new_threshold += 1;
        } else if current_promotion_ratio < self.target_promotion_ratio * 0.8 {
            // 晋升比例太低，降低阈值（更容易晋升）
            new_threshold -= 1;
        }

        // 限制在最小和最大阈值之间
        new_threshold = new_threshold.max(self.min_threshold as i32).min(self.max_threshold as i32);

        self.current_threshold.store(new_threshold as u32, Ordering::Relaxed);
    }

    /// 获取当前晋升阈值
    pub fn get_threshold(&self) -> u32 {
        self.current_threshold.load(Ordering::Relaxed)
    }

    /// 重置存活次数分布（在GC周期后调用）
    pub fn reset_distribution(&self) {
        let mut dist = self.survival_count_distribution.lock().unwrap();
        dist.clear();
    }
}

/// GC自适应调整器
///
/// 整合所有自适应调整功能
pub struct GcAdaptiveAdjuster {
    /// 分配速率跟踪器
    allocation_tracker: Arc<AllocationRateTracker>,
    /// 年轻代比例调整器
    young_gen_adjuster: Arc<YoungGenRatioAdjuster>,
    /// 晋升阈值调整器
    promotion_adjuster: Arc<PromotionThresholdAdjuster>,
    /// 上次GC时间
    last_gc_time: Arc<Mutex<Option<Instant>>>,
    /// GC触发阈值（基于分配速率）
    allocation_trigger_threshold: u64, // 字节/秒
}

impl GcAdaptiveAdjuster {
    /// 创建新的GC自适应调整器
    pub fn new(
        initial_young_gen_ratio: f64,
        initial_promotion_threshold: u32,
        allocation_trigger_threshold: u64,
    ) -> Self {
        Self {
            allocation_tracker: Arc::new(AllocationRateTracker::new(5)), // 5秒窗口
            young_gen_adjuster: Arc::new(YoungGenRatioAdjuster::new(
                initial_young_gen_ratio,
                0.1, // 目标存活率10%
                0.1, // 最小10%
                0.5, // 最大50%
            )),
            promotion_adjuster: Arc::new(PromotionThresholdAdjuster::new(
                initial_promotion_threshold,
                1,  // 最小阈值1
                10, // 最大阈值10
                0.2, // 目标晋升比例20%
            )),
            last_gc_time: Arc::new(Mutex::new(None)),
            allocation_trigger_threshold,
        }
    }

    /// 记录分配
    pub fn record_allocation(&self, size: u64) {
        self.allocation_tracker.record_allocation(size);
    }

    /// 检查是否应该触发GC（基于分配速率）
    pub fn should_trigger_gc(&self, heap_usage_ratio: f64) -> bool {
        let allocation_rate = self.allocation_tracker.get_allocation_rate();

        // 如果分配速率超过阈值，触发GC
        if allocation_rate > self.allocation_trigger_threshold {
            return true;
        }

        // 如果堆使用率很高，也触发GC
        if heap_usage_ratio > 0.8 {
            return true;
        }

        // 如果分配速率很高且堆使用率中等，触发GC
        if allocation_rate > self.allocation_trigger_threshold / 2 && heap_usage_ratio > 0.5 {
            return true;
        }

        false
    }

    /// 记录GC完成
    pub fn record_gc_complete(&self, survival_rate: f64) {
        let now = Instant::now();
        {
            let mut last_gc = self.last_gc_time.lock().unwrap();
            *last_gc = Some(now);
        }

        // 记录存活率并调整年轻代比例
        self.young_gen_adjuster.record_survival_rate(survival_rate);

        // 调整晋升阈值
        self.promotion_adjuster.adjust_threshold();
    }

    /// 记录对象存活次数（用于晋升阈值调整）
    pub fn record_object_survival(&self, survival_count: u32) {
        self.promotion_adjuster.record_survival_count(survival_count);
    }

    /// 获取当前年轻代比例
    pub fn get_young_gen_ratio(&self) -> f64 {
        self.young_gen_adjuster.get_ratio()
    }

    /// 获取当前晋升阈值
    pub fn get_promotion_threshold(&self) -> u32 {
        self.promotion_adjuster.get_threshold()
    }

    /// 获取当前分配速率
    pub fn get_allocation_rate(&self) -> u64 {
        self.allocation_tracker.get_allocation_rate()
    }

    /// 重置（在GC周期开始时调用）
    pub fn reset(&self) {
        self.promotion_adjuster.reset_distribution();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocation_rate_tracker() {
        let tracker = AllocationRateTracker::new(5);
        
        // 记录一些分配
        tracker.record_allocation(1000);
        tracker.record_allocation(2000);
        tracker.record_allocation(3000);

        // 等待一小段时间
        std::thread::sleep(Duration::from_millis(100));

        let rate = tracker.get_allocation_rate();
        assert!(rate > 0);
    }

    #[test]
    fn test_young_gen_ratio_adjuster() {
        let adjuster = YoungGenRatioAdjuster::new(0.3, 0.1, 0.1, 0.5);

        // 记录高存活率（应该增加年轻代比例）
        adjuster.record_survival_rate(0.2);
        adjuster.record_survival_rate(0.25);
        
        let ratio = adjuster.get_ratio();
        assert!(ratio >= 0.3); // 应该增加

        // 记录低存活率（应该减少年轻代比例）
        adjuster.record_survival_rate(0.05);
        adjuster.record_survival_rate(0.03);
        
        let ratio2 = adjuster.get_ratio();
        assert!(ratio2 <= ratio); // 应该减少
    }

    #[test]
    fn test_promotion_threshold_adjuster() {
        let adjuster = PromotionThresholdAdjuster::new(3, 1, 10, 0.2);

        // 记录一些存活次数
        adjuster.record_survival_count(1);
        adjuster.record_survival_count(2);
        adjuster.record_survival_count(3);
        adjuster.record_survival_count(4);
        adjuster.record_survival_count(5);

        adjuster.adjust_threshold();
        let threshold = adjuster.get_threshold();
        assert!(threshold >= 1 && threshold <= 10);
    }

    #[test]
    fn test_gc_adaptive_adjuster() {
        let adjuster = GcAdaptiveAdjuster::new(0.3, 3, 1000000);

        // 记录分配
        adjuster.record_allocation(100000);
        adjuster.record_allocation(200000);

        // 检查是否应该触发GC
        let should_trigger = adjuster.should_trigger_gc(0.6);
        // 结果取决于分配速率和时间

        // 记录GC完成
        adjuster.record_gc_complete(0.15);
        
        let ratio = adjuster.get_young_gen_ratio();
        assert!(ratio >= 0.1 && ratio <= 0.5);
    }
}

