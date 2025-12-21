//! 测试超时工具
//!
//! 为性能和并发测试提供超时支持，防止测试因并发问题而无限等待
//!
//! ## 使用示例
//!
//! ### 同步测试
//! ```rust,ignore
//! use test_timeout_utils::test_with_timeout;
//!
//! test_with_timeout!(60, test_my_function, {
//!     // 测试代码
//! });
//! ```
//!
//! ### 异步测试
//! ```rust,ignore
//! use test_timeout_utils::tokio_test_with_timeout;
//!
//! tokio_test_with_timeout!(90, test_async_function, {
//!     // 异步测试代码
//! });
//! ```

use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

/// 测试超时配置
pub struct TestTimeout {
    /// 超时时间（秒）
    timeout_seconds: u64,
    /// 是否已超时
    timed_out: Arc<AtomicBool>,
}

impl TestTimeout {
    /// 创建新的超时配置
    pub fn new(timeout_seconds: u64) -> Self {
        Self {
            timeout_seconds,
            timed_out: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 创建默认超时（60秒）
    pub fn default() -> Self {
        Self::new(60)
    }

    /// 创建长时间超时（300秒，5分钟）
    pub fn long() -> Self {
        Self::new(300)
    }

    /// 创建短超时（30秒）
    pub fn short() -> Self {
        Self::new(30)
    }

    /// 执行带超时的测试
    pub fn run_with_timeout<F, R>(&self, test_fn: F) -> Result<R, String>
    where
        F: FnOnce() -> R + Send,
        R: Send,
    {
        let timed_out = Arc::clone(&self.timed_out);
        let timeout_duration = Duration::from_secs(self.timeout_seconds);
        let start = Instant::now();

        // 启动超时监控线程
        let timeout_handle = {
            let timed_out = Arc::clone(&timed_out);
            thread::spawn(move || {
                thread::sleep(timeout_duration);
                timed_out.store(true, Ordering::Release);
            })
        };

        // 执行测试函数
        let result = test_fn();

        // 检查是否超时
        if timed_out.load(Ordering::Acquire) {
            let elapsed = start.elapsed();
            return Err(format!(
                "测试超时：超过 {} 秒（实际耗时：{:.2} 秒）",
                self.timeout_seconds,
                elapsed.as_secs_f64()
            ));
        }

        // 取消超时监控（如果测试提前完成）
        drop(timeout_handle);

        Ok(result)
    }

    /// 检查是否已超时
    pub fn is_timed_out(&self) -> bool {
        self.timed_out.load(Ordering::Acquire)
    }
}

impl Default for TestTimeout {
    fn default() -> Self {
        Self::default()
    }
}

/// 带超时的异步测试宏
///
/// 用法：`tokio_test_with_timeout!(超时秒数, 测试函数名, { 异步测试代码 })`
#[macro_export]
macro_rules! tokio_test_with_timeout {
    ($timeout_secs:expr, $test_name:ident, $test_body:block) => {
        #[tokio::test]
        async fn $test_name() {
            use std::time::{Duration, Instant};
            use tokio::time::timeout;

            let start = Instant::now();
            let timeout_duration = Duration::from_secs($timeout_secs);

            match timeout(timeout_duration, async $test_body).await {
                Ok(result) => result,
                Err(_) => {
                    let elapsed = start.elapsed();
                    panic!(
                        "测试超时：超过 {} 秒（实际耗时：{:.2} 秒）。\
                        这可能是由于死锁、无限循环或资源竞争导致的。",
                        $timeout_secs,
                        elapsed.as_secs_f64()
                    );
                }
            }
        }
    };
}

/// 带超时的同步测试宏
///
/// 用法：`test_with_timeout!(超时秒数, 测试函数名, { 测试代码 })`
#[macro_export]
macro_rules! test_with_timeout {
    ($timeout_secs:expr, $test_name:ident, $test_body:block) => {
        #[test]
        fn $test_name() {
            use std::time::{Duration, Instant};
            use std::thread;
            use std::sync::{Arc, mpsc};
            use std::sync::atomic::{AtomicBool, Ordering};

            let timeout_duration = Duration::from_secs($timeout_secs);
            let start = Instant::now();
            let (tx, rx) = mpsc::channel();
            let timed_out = Arc::new(AtomicBool::new(false));

            // 启动超时监控线程
            let timed_out_clone = Arc::clone(&timed_out);
            let timeout_handle = thread::spawn(move || {
                thread::sleep(timeout_duration);
                timed_out_clone.store(true, Ordering::Release);
                let _ = tx.send(());
            });

            // 执行测试
            let test_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                $test_body
            }));

            // 检查超时
            if timed_out.load(Ordering::Acquire) {
                let elapsed = start.elapsed();
                // 取消超时线程（如果测试提前完成）
                drop(timeout_handle);
                panic!(
                    "测试超时：超过 {} 秒（实际耗时：{:.2} 秒）。\
                    这可能是由于死锁、无限循环或资源竞争导致的。",
                    $timeout_secs,
                    elapsed.as_secs_f64()
                );
            }

            // 取消超时监控（如果测试提前完成）
            drop(timeout_handle);

            // 处理测试结果
            match test_result {
                Ok(result) => result,
                Err(e) => std::panic::resume_unwind(e),
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeout_creation() {
        let timeout = TestTimeout::new(10);
        assert_eq!(timeout.timeout_seconds, 10);
    }

    #[test]
    fn test_timeout_short() {
        let timeout = TestTimeout::short();
        assert_eq!(timeout.timeout_seconds, 30);
    }

    #[test]
    fn test_timeout_long() {
        let timeout = TestTimeout::long();
        assert_eq!(timeout.timeout_seconds, 300);
    }
}

