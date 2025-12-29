// VM日志系统（Logging System）
//
// 提供高性能的日志记录功能，支持多种日志级别和输出目标。

use std::sync::atomic::{AtomicU64, Ordering};
use std::io::{self, Write};
use std::fs::{File, OpenOptions};

/// 日志级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub enum LogLevel {
    /// 调试级别（最详细）
    Debug,
    /// 信息级别
    Info,
    /// 警告级别
    Warn,
    /// 错误级别
    Error,
}

impl LogLevel {
    /// 获取日志级别名称
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        }
    }

    /// 获取日志级别优先级（用于排序）
    pub fn priority(&self) -> u8 {
        match self {
            LogLevel::Debug => 0,
            LogLevel::Info => 1,
            LogLevel::Warn => 2,
            LogLevel::Error => 3,
        }
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 日志记录器
pub struct VmLogger {
    /// 最小日志级别
    min_level: LogLevel,
    /// 日志输出文件
    log_file: Option<File>,
    /// 控制台输出
    console_output: bool,
    /// 日志统计
    stats: VmLoggerStats,
}

/// 日志统计
#[derive(Debug, Clone)]
pub struct VmLoggerStats {
    /// 总日志消息数
    pub total_messages: AtomicU64,
    /// 调试消息数
    pub debug_messages: AtomicU64,
    /// 信息消息数
    pub info_messages: AtomicU64,
    /// 警告消息数
    pub warn_messages: AtomicU64,
    /// 错误消息数
    pub error_messages: AtomicU64,
}

impl VmLogger {
    /// 创建新的日志记录器
    ///
    /// # 参数
    /// - `min_level`: 最小日志级别（默认Info）
    /// - `console_output`: 是否输出到控制台（默认true）
    ///
    /// # 示例
    /// ```ignore
    /// let logger = VmLogger::new(LogLevel::Info, true);
    /// ```
    pub fn new(min_level: LogLevel, console_output: bool) -> Self {
        Self {
            min_level,
            log_file: None,
            console_output,
            stats: VmLoggerStats {
                total_messages: AtomicU64::new(0),
                debug_messages: AtomicU64::new(0),
                info_messages: AtomicU64::new(0),
                warn_messages: AtomicU64::new(0),
                error_messages: AtomicU64::new(0),
            },
        }
    }

    /// 记录日志消息
    ///
    /// # 参数
    /// - `level`: 日志级别
    /// - `message`: 日志消息
    /// - `module`: 模块名称（可选）
    ///
    /// # 示例
    /// ```ignore
    /// logger.log(LogLevel::Info, "VM initialized", Some("vm-core"));
    /// ```
    pub fn log(&self, level: LogLevel, message: &str, module: Option<&str>) {
        // 检查日志级别
        if level.priority() < self.min_level.priority() {
            return;
        }

        // 更新统计
        self.stats.total_messages.fetch_add(1, Ordering::Relaxed);
        match level {
            LogLevel::Debug => self.stats.debug_messages.fetch_add(1, Ordering::Relaxed),
            LogLevel::Info => self.stats.info_messages.fetch_add(1, Ordering::Relaxed),
            LogLevel::Warn => self.stats.warn_messages.fetch_add(1, Ordering::Relaxed),
            LogLevel::Error => self.stats.error_messages.fetch_add(1, Ordering::Relaxed),
        }

        // 格式化日志消息
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .as_millis();
        let module_str = module.map(|m| format!("[{}]", m)).unwrap_or(String::new());
        let formatted = format!("[{:?}] {} {}: {}", timestamp, module_str, level.as_str(), message);

        // 输出到控制台
        if self.console_output {
            println!("{}", formatted);
        }

        // 写入日志文件
        if let Some(file) = &self.log_file {
            let _ = writeln!(file, "{}", formatted);
        }
    }

    /// 调试日志
    pub fn debug(&self, message: &str, module: Option<&str>) {
        self.log(LogLevel::Debug, message, module);
    }

    /// 信息日志
    pub fn info(&self, message: &str, module: Option<&str>) {
        self.log(LogLevel::Info, message, module);
    }

    /// 警告日志
    pub fn warn(&self, message: &str, module: Option<&str>) {
        self.log(LogLevel::Warn, message, module);
    }

    /// 错误日志
    pub fn error(&self, message: &str, module: Option<&str>) {
        self.log(LogLevel::Error, message, module);
    }

    /// 设置日志文件
    pub fn set_log_file(&mut self, path: &str) -> std::io::Result<()> {
        let file = File::create(path)?;
        self.log_file = Some(file);
        Ok(())
    }

    /// 获取日志统计
    pub fn get_stats(&self) -> VmLoggerStats {
        VmLoggerStats {
            total_messages: self.stats.total_messages.load(Ordering::Relaxed),
            debug_messages: self.stats.debug_messages.load(Ordering::Relaxed),
            info_messages: self.stats.info_messages.load(Ordering::Relaxed),
            warn_messages: self.stats.warn_messages.load(Ordering::Relaxed),
            error_messages: self.stats.error_messages.load(Ordering::Relaxed),
        }
    }

    /// 清空日志统计
    pub fn clear_stats(&self) {
        self.stats.total_messages.store(0, Ordering::Relaxed);
        self.stats.debug_messages.store(0, Ordering::Relaxed);
        self.stats.info_messages.store(0, Ordering::Relaxed);
        self.stats.warn_messages.store(0, Ordering::Relaxed);
        self.stats.error_messages.store(0, Ordering::Relaxed);
    }

    /// 切换控制台输出
    pub fn set_console_output(&mut self, enabled: bool) {
        self.console_output = enabled;
    }
}

/// 全局日志记录器
lazy_static! {
    pub static ref LOGGER: VmLogger = VmLogger::new(LogLevel::Info, true);
}

/// 便捷的日志宏
#[macro_export]
macro_rules! vm_debug {
    ($($($arg:tt)*)*) => {
        $crate::vm_common::logging::LOGGER.debug(&format!($($($arg)*)*));
    }
}

#[macro_export]
macro_rules! vm_info {
    ($($($arg:tt)*)*) => {
        $crate::vm_common::logging::LOGGER.info(&format!($($($arg)*)*));
    }
}

#[macro_export]
macro_rules! vm_warn {
    ($($($arg:tt)*)*) => {
        $crate::vm_common::logging::LOGGER.warn(&format!($($($arg)*)*));
    }
}

#[macro_export]
macro_rules! vm_error {
    ($($($arg:tt)*)*) => {
        $crate::vm_common::logging::LOGGER.error(&format!($($($arg)*)*));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logger_creation() {
        let logger = VmLogger::new(LogLevel::Info, true);
        assert_eq!(logger.min_level, LogLevel::Info);
        assert!(logger.console_output);
    }

    #[test]
    fn test_log_levels() {
        assert!(LogLevel::Debug.priority() < LogLevel::Info.priority());
        assert_eq!(LogLevel::Info.as_str(), "INFO");
    }

    #[test]
    fn test_log_stats() {
        let stats = VmLoggerStats::new();
        assert_eq!(stats.total_messages.load(Ordering::Relaxed), 0);
        assert_eq!(stats.debug_messages.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_logging_macros() {
        vm_debug!("Debug message");
        vm_info!("Info message");
        vm_warn!("Warning message");
        vm_error!("Error message");
    }
}
