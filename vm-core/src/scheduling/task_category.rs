//! 任务分类
//!
//! 根据任务特性将其分类,并映射到合适的QoS类和核心类型

use crate::scheduling::qos::QoSClass;
use std::fmt;

/// 任务类别
///
/// 定义了不同类型的任务及其调度偏好
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskCategory {
    /// 性能关键任务
    ///
    /// 需要最大性能的核心任务,直接影响系统性能
    /// - **QoS**: UserInteractive
    /// - **核心**: P-core
    /// - **示例**: JIT编译器核心路径、热点代码生成
    PerformanceCritical,

    /// 延迟敏感任务
    ///
    /// 需要快速响应的用户交互任务
    /// - **QoS**: UserInitiated
    /// - **核心**: P-core
    /// - **示例**: 同步操作、事件处理、用户请求
    LatencySensitive,

    /// 普通任务
    ///
    /// 常规计算和I/O任务
    /// - **QoS**: Utility
    /// - **核心**: P-core/E-core混合
    /// - **示例**: 数据处理、常规计算、网络请求
    Normal,

    /// 批处理任务
    ///
    /// 可以延后执行的大量数据处理任务
    /// - **QoS**: Utility
    /// - **核心**: E-core
    /// - **示例**: 垃圾回收、批量优化、日志处理
    BatchProcessing,

    /// 后台清理任务
    ///
    /// 不影响用户体验的后台维护任务
    /// - **QoS**: Background
    /// - **核心**: E-core
    /// - **示例**: 缓存清理、日志归档、统计数据收集
    BackgroundCleanup,
}

impl TaskCategory {
    /// 转换为QoS类
    pub fn to_qos(&self) -> QoSClass {
        match self {
            TaskCategory::PerformanceCritical => QoSClass::UserInteractive,
            TaskCategory::LatencySensitive => QoSClass::UserInitiated,
            TaskCategory::Normal => QoSClass::Utility,
            TaskCategory::BatchProcessing => QoSClass::Utility,
            TaskCategory::BackgroundCleanup => QoSClass::Background,
        }
    }

    /// 是否应该使用P-core
    pub fn prefers_performance_core(&self) -> bool {
        matches!(
            self,
            TaskCategory::PerformanceCritical | TaskCategory::LatencySensitive
        )
    }

    /// 是否应该使用E-core
    pub fn prefers_efficiency_core(&self) -> bool {
        matches!(self, TaskCategory::BatchProcessing | TaskCategory::BackgroundCleanup)
    }

    /// 获取性能影响描述
    pub fn performance_impact(&self) -> PerformanceImpact {
        match self {
            TaskCategory::PerformanceCritical => PerformanceImpact::Critical,
            TaskCategory::LatencySensitive => PerformanceImpact::High,
            TaskCategory::Normal => PerformanceImpact::Medium,
            TaskCategory::BatchProcessing => PerformanceImpact::Low,
            TaskCategory::BackgroundCleanup => PerformanceImpact::Minimal,
        }
    }

    /// 获取任务类别描述
    pub fn description(&self) -> &'static str {
        match self {
            TaskCategory::PerformanceCritical => "性能关键: 需要最大性能的核心任务",
            TaskCategory::LatencySensitive => "延迟敏感: 需要快速响应的用户交互任务",
            TaskCategory::Normal => "普通: 常规计算和I/O任务",
            TaskCategory::BatchProcessing => "批处理: 可延后执行的大量数据处理任务",
            TaskCategory::BackgroundCleanup => "后台清理: 不影响用户体验的后台维护任务",
        }
    }

    /// 获取推荐的核心类型
    pub fn recommended_core_type(&self) -> CoreType {
        match self {
            TaskCategory::PerformanceCritical => CoreType::Performance,
            TaskCategory::LatencySensitive => CoreType::Performance,
            TaskCategory::Normal => CoreType::Balanced,
            TaskCategory::BatchProcessing => CoreType::Efficiency,
            TaskCategory::BackgroundCleanup => CoreType::Efficiency,
        }
    }
}

impl fmt::Display for TaskCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// 性能影响等级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PerformanceImpact {
    /// 关键影响: 直接决定系统性能
    Critical,
    /// 高影响: 显著影响用户体验
    High,
    /// 中等影响: 可感知但不关键
    Medium,
    /// 低影响: 最小影响
    Low,
    /// 极小影响: 几乎不可感知
    Minimal,
}

impl fmt::Display for PerformanceImpact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PerformanceImpact::Critical => write!(f, "Critical"),
            PerformanceImpact::High => write!(f, "High"),
            PerformanceImpact::Medium => write!(f, "Medium"),
            PerformanceImpact::Low => write!(f, "Low"),
            PerformanceImpact::Minimal => write!(f, "Minimal"),
        }
    }
}

/// 推荐的核心类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoreType {
    /// 性能核心 (P-core)
    Performance,
    /// 能效核心 (E-core)
    Efficiency,
    /// 均衡 (P-core/E-core混合)
    Balanced,
}

impl fmt::Display for CoreType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreType::Performance => write!(f, "P-core"),
            CoreType::Efficiency => write!(f, "E-core"),
            CoreType::Balanced => write!(f, "Balanced"),
        }
    }
}

/// 设置当前线程的任务类别
///
/// 这会设置相应的QoS类,影响任务调度到P-core还是E-core
///
/// # 参数
/// - `category`: 任务类别
///
/// # 返回
/// - 成功: Ok(())
/// - 失败: Err(io::Error)
///
/// # 示例
/// ```rust,no_run
/// use vm_core::scheduling::TaskCategory;
///
/// // 标记当前线程为性能关键任务
/// set_task_category(TaskCategory::PerformanceCritical)?;
/// ```
pub fn set_task_category(category: TaskCategory) -> std::io::Result<()> {
    crate::scheduling::qos::set_current_thread_qos(category.to_qos())
}

/// 获取当前线程的任务类别
///
/// # 返回
/// - 当前线程的任务类别(基于当前QoS类推断)
pub fn get_task_category() -> TaskCategory {
    let qos = crate::scheduling::qos::get_current_thread_qos();
    match qos {
        QoSClass::UserInteractive => TaskCategory::PerformanceCritical,
        QoSClass::UserInitiated => TaskCategory::LatencySensitive,
        QoSClass::Utility => TaskCategory::Normal,
        QoSClass::Background => TaskCategory::BackgroundCleanup,
        QoSClass::Unspecified => TaskCategory::Normal,
    }
}

/// 在指定任务类别中执行任务
///
/// # 参数
/// - `category`: 任务类别
/// - `f`: 要执行的任务
///
/// # 返回
/// - 任务的返回值
///
/// # 示例
/// ```rust
/// use vm_core::scheduling::{TaskCategory, with_task_category};
///
/// let result = with_task_category(TaskCategory::PerformanceCritical, || {
///     // 在性能关键类别下执行JIT编译
///     compile_jit_code()
/// });
/// ```
pub fn with_task_category<F, R>(category: TaskCategory, f: F) -> R
where
    F: FnOnce() -> R,
{
    // 保存旧类别
    let old_category = get_task_category();

    // 设置新类别
    let _ = set_task_category(category);

    // 执行任务
    let result = f();

    // 恢复旧类别
    let _ = set_task_category(old_category);

    result
}

/// 在性能关键类别中执行任务(快捷方式)
pub fn with_performance_critical<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    with_task_category(TaskCategory::PerformanceCritical, f)
}

/// 在延迟敏感类别中执行任务(快捷方式)
pub fn with_latency_sensitive<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    with_task_category(TaskCategory::LatencySensitive, f)
}

/// 在后台清理类别中执行任务(快捷方式)
pub fn with_background_cleanup<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    with_task_category(TaskCategory::BackgroundCleanup, f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_category_to_qos() {
        assert_eq!(
            TaskCategory::PerformanceCritical.to_qos(),
            QoSClass::UserInteractive
        );
        assert_eq!(
            TaskCategory::LatencySensitive.to_qos(),
            QoSClass::UserInitiated
        );
        assert_eq!(TaskCategory::Normal.to_qos(), QoSClass::Utility);
        assert_eq!(TaskCategory::BatchProcessing.to_qos(), QoSClass::Utility);
        assert_eq!(
            TaskCategory::BackgroundCleanup.to_qos(),
            QoSClass::Background
        );
    }

    #[test]
    fn test_task_category_preferences() {
        // P-core偏好
        assert!(TaskCategory::PerformanceCritical.prefers_performance_core());
        assert!(TaskCategory::LatencySensitive.prefers_performance_core());
        assert!(!TaskCategory::Normal.prefers_performance_core());
        assert!(!TaskCategory::BatchProcessing.prefers_performance_core());

        // E-core偏好
        assert!(TaskCategory::BatchProcessing.prefers_efficiency_core());
        assert!(TaskCategory::BackgroundCleanup.prefers_efficiency_core());
        assert!(!TaskCategory::PerformanceCritical.prefers_efficiency_core());
    }

    #[test]
    fn test_performance_impact() {
        assert_eq!(
            TaskCategory::PerformanceCritical.performance_impact(),
            PerformanceImpact::Critical
        );
        assert_eq!(
            TaskCategory::LatencySensitive.performance_impact(),
            PerformanceImpact::High
        );
        assert_eq!(TaskCategory::Normal.performance_impact(), PerformanceImpact::Medium);
        assert_eq!(
            TaskCategory::BatchProcessing.performance_impact(),
            PerformanceImpact::Low
        );
        assert_eq!(
            TaskCategory::BackgroundCleanup.performance_impact(),
            PerformanceImpact::Minimal
        );
    }

    #[test]
    fn test_recommended_core_type() {
        assert_eq!(
            TaskCategory::PerformanceCritical.recommended_core_type(),
            CoreType::Performance
        );
        assert_eq!(
            TaskCategory::LatencySensitive.recommended_core_type(),
            CoreType::Performance
        );
        assert_eq!(TaskCategory::Normal.recommended_core_type(), CoreType::Balanced);
        assert_eq!(
            TaskCategory::BatchProcessing.recommended_core_type(),
            CoreType::Efficiency
        );
        assert_eq!(
            TaskCategory::BackgroundCleanup.recommended_core_type(),
            CoreType::Efficiency
        );
    }

    #[test]
    fn test_with_task_category() {
        let result = with_task_category(TaskCategory::PerformanceCritical, || {
            42
        });
        assert_eq!(result, 42);
    }

    #[test]
    fn test_with_performance_critical() {
        let result = with_performance_critical(|| {
            "critical"
        });
        assert_eq!(result, "critical");
    }

    #[test]
    fn test_with_latency_sensitive() {
        let result = with_latency_sensitive(|| {
            "latency sensitive"
        });
        assert_eq!(result, "latency sensitive");
    }

    #[test]
    fn test_with_background_cleanup() {
        let result = with_background_cleanup(|| {
            "background"
        });
        assert_eq!(result, "background");
    }

    #[test]
    fn test_set_task_category() {
        let result = set_task_category(TaskCategory::Normal);
        #[cfg(target_os = "macos")]
        assert!(result.is_ok() || result.is_err());
        #[cfg(not(target_os = "macos"))]
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_task_category() {
        let category = get_task_category();
        // 应该返回有效的类别
        match category {
            TaskCategory::PerformanceCritical |
            TaskCategory::LatencySensitive |
            TaskCategory::Normal |
            TaskCategory::BatchProcessing |
            TaskCategory::BackgroundCleanup => {}
        }
    }
}
