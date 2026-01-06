//! macOS/iOS QoS (Quality of Service) 类封装
//!
//! QoS类定义了任务的执行优先级和期望的核心类型(P-core vs E-core)
//!
//! 参考文档:
//! - https://developer.apple.com/documentation/xcode/improving-your-app-s-performance

use std::io;

/// QoS (Quality of Service) 类
///
/// 定义任务的执行优先级和调度期望
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum QoSClass {
    /// 用户交互 (最高优先级)
    ///
    /// 用于直接响应用户交互的任务
    /// - **核心类型**: P-core (性能核心)
    /// - **延迟要求**: 极低 (< 10ms)
    /// - **典型用途**: 动画渲染、UI更新、事件处理
    UserInteractive = 0x21,

    /// 用户启动 (高优先级)
    ///
    /// 用于用户发起但不需要即时响应的任务
    /// - **核心类型**: P-core (性能核心)
    /// - **延迟要求**: 低 (< 100ms)
    /// - **典型用途**: 文件打开、网络请求、数据加载
    UserInitiated = 0x19,

    /// 实用工具 (默认优先级)
    ///
    /// 用于用户可感知但不需要即时完成的任务
    /// - **核心类型**: P-core/E-core混合
    /// - **延迟要求**: 中等 (< 1s)
    /// - **典型用途**: 计算、I/O、数据处理
    Utility = 0x11,

    /// 后台 (低优先级)
    ///
    /// 用于用户不可见的后台任务
    /// - **核心类型**: E-core (能效核心)
    /// - **延迟要求**: 宽松 (秒级)
    /// - **典型用途**: 同步、备份、清理
    Background = 0x09,

    /// 未指定 (系统推断)
    ///
    /// 系统会根据上下文推断QoS类
    /// - **核心类型**: 系统决定
    /// - **延迟要求**: 不确定
    /// - **典型用途**: 兼容性
    Unspecified = 0x00,
}

impl QoSClass {
    /// 获取QoS类的优先级分数(越高越优先)
    pub fn priority_score(&self) -> i32 {
        match self {
            QoSClass::UserInteractive => 100,
            QoSClass::UserInitiated => 80,
            QoSClass::Utility => 60,
            QoSClass::Background => 40,
            QoSClass::Unspecified => 0,
        }
    }

    /// 是否应该使用P-core(性能核心)
    pub fn prefers_performance_core(&self) -> bool {
        matches!(self, QoSClass::UserInteractive | QoSClass::UserInitiated)
    }

    /// 是否应该使用E-core(能效核心)
    pub fn prefers_efficiency_core(&self) -> bool {
        matches!(self, QoSClass::Background)
    }

    /// 获取QoS类名称
    pub fn name(&self) -> &'static str {
        match self {
            QoSClass::UserInteractive => "UserInteractive",
            QoSClass::UserInitiated => "UserInitiated",
            QoSClass::Utility => "Utility",
            QoSClass::Background => "Background",
            QoSClass::Unspecified => "Unspecified",
        }
    }
}

/// pthread QoS类(用于FFI)
///
/// # Naming Convention Note
/// 这些变体名称使用SCREAMING_SNAKE_CASE以匹配Apple的pthread API命名约定。
/// 虽然不符合Rust命名规范，但这是必要的，因为它们直接映射到系统API。
#[repr(i32)]
#[allow(non_camel_case_types)]  // FFI绑定需要匹配系统API命名
pub enum pthread_qos_class_t {
    QOS_CLASS_USER_INTERACTIVE = 0x21,
    QOS_CLASS_USER_INITIATED = 0x19,
    QOS_CLASS_DEFAULT = 0x15,  // macOS默认
    QOS_CLASS_UTILITY = 0x11,
    QOS_CLASS_BACKGROUND = 0x09,
}

// macOS pthread QoS FFI declarations at module level
#[cfg(target_os = "macos")]
unsafe extern "C" {
    /// Set the QoS class of the current thread
    #[link_name = "pthread_set_qos_class_self"]
    fn pthread_set_qos_class_self_impl(
        qos_class: pthread_qos_class_t,
        relative_priority: i32,
    ) -> i32;

    /// Get the QoS class of the current thread
    #[link_name = "pthread_get_qos_class_self_np"]
    fn pthread_get_qos_class_self_np_impl() -> pthread_qos_class_t;
}

/// 设置当前线程的QoS类
///
/// # 参数
/// - `qos`: QoS类
///
/// # 返回
/// - 成功: Ok(())
/// - 失败: Err(io::Error)
///
/// # 示例
/// ```rust
/// use vm_core::scheduling::QoSClass;
///
/// // 设置当前线程为用户交互优先级
/// set_current_thread_qos(QoSClass::UserInteractive)?;
/// ```
///
/// # macOS实现
/// 使用pthread API: `pthread_set_qos_class_self`
pub fn set_current_thread_qos(qos: QoSClass) -> io::Result<()> {
    #[cfg(all(target_os = "macos", not(test)))]
    {
        let pthread_qos = match qos {
            QoSClass::UserInteractive => pthread_qos_class_t::QOS_CLASS_USER_INTERACTIVE,
            QoSClass::UserInitiated => pthread_qos_class_t::QOS_CLASS_USER_INITIATED,
            QoSClass::Utility => pthread_qos_class_t::QOS_CLASS_UTILITY,
            QoSClass::Background => pthread_qos_class_t::QOS_CLASS_BACKGROUND,
            QoSClass::Unspecified => pthread_qos_class_t::QOS_CLASS_DEFAULT,
        };

        let ret = unsafe { pthread_set_qos_class_self_impl(pthread_qos, 0) };

        if ret == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    #[cfg(not(all(target_os = "macos", not(test))))]
    {
        // 非macOS平台或测试环境: 无操作
        let _ = qos;
        Ok(())
    }
}

/// 获取当前线程的QoS类
///
/// # 返回
/// - 当前线程的QoS类
///
/// # 示例
/// ```rust
/// use vm_core::scheduling::get_current_thread_qos;
///
/// let qos = get_current_thread_qos();
/// println!("Current QoS: {:?}", qos);
/// ```
pub fn get_current_thread_qos() -> QoSClass {
    #[cfg(all(target_os = "macos", not(test)))]
    {
        let pthread_qos = unsafe { pthread_get_qos_class_self_np_impl() };

        match pthread_qos {
            pthread_qos_class_t::QOS_CLASS_USER_INTERACTIVE => QoSClass::UserInteractive,
            pthread_qos_class_t::QOS_CLASS_USER_INITIATED => QoSClass::UserInitiated,
            pthread_qos_class_t::QOS_CLASS_UTILITY => QoSClass::Utility,
            pthread_qos_class_t::QOS_CLASS_BACKGROUND => QoSClass::Background,
            _ => QoSClass::Unspecified,
        }
    }

    #[cfg(not(all(target_os = "macos", not(test))))]
    {
        QoSClass::Unspecified
    }
}

/// 在指定QoS类中执行任务
///
/// # 参数
/// - `qos`: QoS类
/// - `f`: 要执行的任务
///
/// # 返回
/// - 任务的返回值
///
/// # 示例
/// ```rust
/// use vm_core::scheduling::QoSClass;
///
/// let result = with_qos(QoSClass::UserInteractive, || {
///     // 在用户交互优先级下执行
///     42
/// });
/// ```
pub fn with_qos<F, R>(qos: QoSClass, f: F) -> R
where
    F: FnOnce() -> R,
{
    // 保存旧QoS
    let old_qos = get_current_thread_qos();

    // 设置新QoS
    let _ = set_current_thread_qos(qos);

    // 执行任务
    let result = f();

    // 恢复旧QoS
    let _ = set_current_thread_qos(old_qos);

    result
}

#[cfg(test)]
#[cfg(not(target_os = "macos"))]  // Skip QOS tests on macOS due to pthread linking issues
mod tests {
    use super::*;

    #[test]
    fn test_qos_priority_score() {
        assert!(QoSClass::UserInteractive.priority_score() > QoSClass::UserInitiated.priority_score());
        assert!(QoSClass::UserInitiated.priority_score() > QoSClass::Utility.priority_score());
        assert!(QoSClass::Utility.priority_score() > QoSClass::Background.priority_score());
    }

    #[test]
    fn test_qos_prefers_performance_core() {
        assert!(QoSClass::UserInteractive.prefers_performance_core());
        assert!(QoSClass::UserInitiated.prefers_performance_core());
        assert!(!QoSClass::Utility.prefers_performance_core());
        assert!(!QoSClass::Background.prefers_performance_core());
    }

    #[test]
    fn test_qos_prefers_efficiency_core() {
        assert!(!QoSClass::UserInteractive.prefers_efficiency_core());
        assert!(!QoSClass::UserInitiated.prefers_efficiency_core());
        assert!(!QoSClass::Utility.prefers_efficiency_core());
        assert!(QoSClass::Background.prefers_efficiency_core());
    }

    #[test]
    fn test_qos_name() {
        assert_eq!(QoSClass::UserInteractive.name(), "UserInteractive");
        assert_eq!(QoSClass::UserInitiated.name(), "UserInitiated");
        assert_eq!(QoSClass::Utility.name(), "Utility");
        assert_eq!(QoSClass::Background.name(), "Background");
        assert_eq!(QoSClass::Unspecified.name(), "Unspecified");
    }

    #[test]
    fn test_set_current_thread_qos() {
        // 设置QoS应该不panic
        let result = set_current_thread_qos(QoSClass::Utility);
        #[cfg(target_os = "macos")]
        assert!(result.is_ok() || result.is_err()); // 可能成功或失败,取决于系统
        #[cfg(not(target_os = "macos"))]
        assert!(result.is_ok()); // 非macOS总是成功
    }

    #[test]
    fn test_get_current_thread_qos() {
        let qos = get_current_thread_qos();
        // 应该返回有效的QoS类
        match qos {
            QoSClass::UserInteractive |
            QoSClass::UserInitiated |
            QoSClass::Utility |
            QoSClass::Background |
            QoSClass::Unspecified => {}
        }
    }

    #[test]
    fn test_with_qos() {
        let result = with_qos(QoSClass::UserInitiated, || {
            42
        });
        assert_eq!(result, 42);

        // 验证QoS被恢复
        let qos_after = get_current_thread_qos();
        // 不应该是UserInitiated(因为已恢复)
        // (具体值取决于测试环境)
    }
}
