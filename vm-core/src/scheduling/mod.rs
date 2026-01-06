//! macOS大小核调度支持
//!
//! Round 38: Apple Silicon大小核调度优化
//!
//! 提供QoS类封装和任务分类,用于在macOS上优化P-core/E-core任务分配

pub mod qos;
pub mod task_category;

pub use qos::{QoSClass, get_current_thread_qos, set_current_thread_qos};
pub use task_category::{
    PerformanceImpact, TaskCategory, get_task_category, set_task_category, with_task_category,
};

/// macOS调度器
///
/// 用于自动分配任务到合适的核心类型
#[derive(Debug, Clone)]
pub struct BigLittleScheduler {
    policy: SchedulingPolicy,
}

/// 调度策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulingPolicy {
    /// 自动调度(根据任务类别自动选择QoS)
    Automatic,
    /// 强制P-core(性能优先)
    PerformanceFirst,
    /// 强制E-core(能效优先)
    EfficiencyFirst,
}

impl Default for BigLittleScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl BigLittleScheduler {
    /// 创建新的调度器
    pub fn new() -> Self {
        Self {
            policy: SchedulingPolicy::Automatic,
        }
    }

    /// 使用指定策略创建调度器
    pub fn with_policy(policy: SchedulingPolicy) -> Self {
        Self { policy }
    }

    /// 设置调度策略
    pub fn set_policy(&mut self, policy: SchedulingPolicy) {
        self.policy = policy;
    }

    /// 获取当前策略
    pub fn policy(&self) -> SchedulingPolicy {
        self.policy
    }

    /// 执行任务(自动调度)
    ///
    /// 根据任务类别自动选择合适的QoS类
    pub fn schedule_task<F, R>(&self, category: TaskCategory, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        with_task_category(category, f)
    }

    /// 异步执行任务(自动调度)
    #[cfg(feature = "async")]
    pub async fn schedule_async_task<F, R>(&self, category: TaskCategory, f: F) -> R
    where
        F: std::future::Future<Output = R>,
    {
        // 在任务开始时设置QoS
        set_task_category(category);
        f.await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_creation() {
        let scheduler = BigLittleScheduler::new();
        assert_eq!(scheduler.policy(), SchedulingPolicy::Automatic);
    }

    #[test]
    fn test_scheduler_with_policy() {
        let scheduler = BigLittleScheduler::with_policy(SchedulingPolicy::PerformanceFirst);
        assert_eq!(scheduler.policy(), SchedulingPolicy::PerformanceFirst);
    }

    #[test]
    fn test_set_policy() {
        let mut scheduler = BigLittleScheduler::new();
        scheduler.set_policy(SchedulingPolicy::EfficiencyFirst);
        assert_eq!(scheduler.policy(), SchedulingPolicy::EfficiencyFirst);
    }

    #[test]
    fn test_schedule_task() {
        let scheduler = BigLittleScheduler::new();
        let result = scheduler.schedule_task(TaskCategory::LatencySensitive, || 42);
        assert_eq!(result, 42);
    }

    #[test]
    fn test_task_category_qos_mapping() {
        // 验证任务类别到QoS的映射
        use qos::QoSClass;

        let mappings = [
            (TaskCategory::PerformanceCritical, QoSClass::UserInteractive),
            (TaskCategory::LatencySensitive, QoSClass::UserInitiated),
            (TaskCategory::Normal, QoSClass::Utility),
            (TaskCategory::BatchProcessing, QoSClass::Utility),
            (TaskCategory::BackgroundCleanup, QoSClass::Background),
        ];

        for (category, expected_qos) in mappings.iter() {
            let qos = category.to_qos();
            assert_eq!(
                qos, *expected_qos,
                "Category {:?} should map to {:?}",
                category, expected_qos
            );
        }
    }
}
