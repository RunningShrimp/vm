// 测试辅助工具（Testing Helpers）
//
// 本模块提供VM测试的辅助工具：
// - Mock对象
// - 测试断言
// - 性能测试工具

/// Mock对象：用于测试的模拟对象
pub struct Mock {
    /// Mock名称
    pub name: String,
    /// Mock调用记录
    pub calls: Vec<MockCall>,
}

/// Mock调用记录
#[derive(Debug, Clone)]
pub struct MockCall {
    /// 方法名称
    pub method: String,
    /// 参数
    pub args: Vec<String>,
    /// 时间戳
    pub timestamp: u64,
}

impl Mock {
    /// 创建新的Mock对象
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            calls: Vec::new(),
        }
    }

    /// 记录Mock调用
    pub fn record_call(&mut self, method: &str, args: Vec<String>) {
        let call = MockCall {
            method: method.to_string(),
            args,
            timestamp: self.get_timestamp(),
        };
        self.calls.push(call);
    }

    /// 获取调用次数
    pub fn call_count(&self, method: &str) -> usize {
        self.calls
            .iter()
            .filter(|call| call.method == method)
            .count()
    }

    /// 获取最后一次调用
    pub fn last_call(&self, method: &str) -> Option<&MockCall> {
        self.calls.iter().rev().find(|call| call.method == method)
    }

    /// 清空调用记录
    pub fn clear_calls(&mut self) {
        self.calls.clear();
    }

    /// 获取当前时间戳
    fn get_timestamp(&self) -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as u64
    }
}

/// 测试断言辅助
pub mod assertions {
    use std::time::Duration;

    /// 断言相等（带自定义消息）
    pub fn assert_eq_msg(left: &dyn std::fmt::Display, right: &dyn std::fmt::Display, msg: &str) {
        assert_eq!(left.to_string(), right.to_string(), "{}", msg);
    }

    /// 断言不相等（带自定义消息）
    pub fn assert_ne_msg(left: &dyn std::fmt::Display, right: &dyn std::fmt::Display, msg: &str) {
        assert_ne!(left.to_string(), right.to_string(), "{}", msg);
    }

    /// 断言在范围内
    pub fn assert_in_range(value: f64, min: f64, max: f64) {
        assert!(
            value >= min && value <= max,
            "Value {} is not in range [{}, {}]",
            value,
            min,
            max
        );
    }

    /// 断言近似相等（带容差）
    pub fn assert_approx_eq(left: f64, right: f64, tolerance: f64) {
        let diff = (left - right).abs();
        assert!(
            diff <= tolerance,
            "Values {} and {} are not approximately equal (diff = {}, tolerance = {})",
            left,
            right,
            diff,
            tolerance
        );
    }

    /// 断言超时
    pub fn assert_timeout(elapsed: Duration, timeout: Duration) {
        assert!(
            elapsed <= timeout,
            "Operation took {:?}, which exceeds timeout {:?}",
            elapsed,
            timeout
        );
    }

    /// 断言超时（宽松）
    pub fn assert_timeout_loose(elapsed: Duration, timeout: Duration, margin: Duration) {
        assert!(
            elapsed <= timeout + margin,
            "Operation took {:?}, which exceeds timeout {:?} + margin {:?}",
            elapsed,
            timeout,
            margin
        );
    }
}

/// 性能测试工具
pub mod performance {
    use std::time::{Duration, Instant};

    /// 性能测试结果
    #[derive(Debug, Clone)]
    pub struct BenchmarkResult {
        /// 操作名称
        pub name: String,
        /// 迭代次数
        pub iterations: u64,
        /// 总时间
        pub total_duration: Duration,
        /// 平均时间
        pub avg_duration: Duration,
        /// 最小时间
        pub min_duration: Duration,
        /// 最大时间
        pub max_duration: Duration,
        /// 每秒操作数（ops/s）
        pub ops_per_sec: f64,
    }

    /// 执行性能测试
    pub fn benchmark<F>(name: &str, iterations: u64, mut f: F) -> BenchmarkResult
    where
        F: FnMut(),
    {
        let mut durations = Vec::with_capacity(iterations as usize);

        // 执行迭代
        for _ in 0..iterations {
            let start = Instant::now();
            f();
            durations.push(start.elapsed());
        }

        // 计算统计信息
        let total_duration: Duration = durations.iter().sum();
        let avg_duration = total_duration / iterations as u32;
        let min_duration = *durations
            .iter()
            .min()
            .expect("benchmark: durations should not be empty");
        let max_duration = *durations
            .iter()
            .max()
            .expect("benchmark: durations should not be empty");

        // 计算ops/s
        let total_secs = total_duration.as_secs_f64();
        let ops_per_sec = if total_secs > 0.0 {
            iterations as f64 / total_secs
        } else {
            0.0
        };

        BenchmarkResult {
            name: name.to_string(),
            iterations,
            total_duration,
            avg_duration,
            min_duration,
            max_duration,
            ops_per_sec,
        }
    }

    /// 快速性能测试（执行1次）
    pub fn quick_benchmark<F>(name: &str, f: F) -> BenchmarkResult
    where
        F: FnOnce(),
    {
        let start = Instant::now();
        f();
        let duration = start.elapsed();

        BenchmarkResult {
            name: name.to_string(),
            iterations: 1,
            total_duration: duration,
            avg_duration: duration,
            min_duration: duration,
            max_duration: duration,
            ops_per_sec: if duration.as_secs_f64() > 0.0 {
                1.0 / duration.as_secs_f64()
            } else {
                0.0
            },
        }
    }

    impl std::fmt::Display for BenchmarkResult {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            writeln!(f, "Benchmark: {}", self.name)?;
            writeln!(f, "  Iterations: {}", self.iterations)?;
            writeln!(f, "  Total Time: {:?}", self.total_duration)?;
            writeln!(f, "  Avg Time: {:?}", self.avg_duration)?;
            writeln!(f, "  Min Time: {:?}", self.min_duration)?;
            writeln!(f, "  Max Time: {:?}", self.max_duration)?;
            writeln!(f, "  Ops/sec: {:.2}", self.ops_per_sec)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_creation() {
        let mock = Mock::new("test_mock");
        assert_eq!(mock.name, "test_mock");
        assert!(mock.calls.is_empty());
    }

    #[test]
    fn test_mock_record_call() {
        let mut mock = Mock::new("test_mock");

        mock.record_call("test_method", vec!["arg1".to_string(), "arg2".to_string()]);

        assert_eq!(mock.call_count("test_method"), 1);
        assert_eq!(mock.calls.len(), 1);
    }

    #[test]
    fn test_mock_last_call() {
        let mut mock = Mock::new("test_mock");

        mock.record_call("method1", vec![]);
        mock.record_call("method2", vec![]);

        let last_call = mock
            .last_call("method2")
            .expect("test_mock_last_call: should have last call for method2");
        assert_eq!(last_call.method, "method2");
    }

    #[test]
    fn test_mock_clear() {
        let mut mock = Mock::new("test_mock");

        mock.record_call("method", vec![]);
        mock.record_call("method", vec![]);

        assert_eq!(mock.call_count("method"), 2);

        mock.clear_calls();

        assert_eq!(mock.call_count("method"), 0);
    }

    #[test]
    fn test_assert_eq_msg() {
        assertions::assert_eq_msg(&42, &42, "Values should be equal");
    }

    #[test]
    fn test_assert_ne_msg() {
        assertions::assert_ne_msg(&42, &43, "Values should not be equal");
    }

    #[test]
    fn test_assert_in_range() {
        assertions::assert_in_range(50.0, 10.0, 100.0);
    }

    #[test]
    fn test_assert_approx_eq() {
        assertions::assert_approx_eq(10.0, 10.1, 0.2);
    }

    #[test]
    fn test_assert_timeout() {
        use std::time::Duration;
        let elapsed = Duration::from_millis(50);
        let timeout = Duration::from_millis(100);

        assertions::assert_timeout(elapsed, timeout);
    }

    #[test]
    fn test_assert_timeout_loose() {
        use std::time::Duration;
        let elapsed = Duration::from_millis(50);
        let timeout = Duration::from_millis(100);
        let margin = Duration::from_millis(10);

        assertions::assert_timeout_loose(elapsed, timeout, margin);
    }

    #[test]
    fn test_benchmark() {
        let counter = std::sync::atomic::AtomicU64::new(0);
        let iterations = 1000;

        let result = performance::benchmark("test", iterations, || {
            counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        });

        assert_eq!(result.iterations, iterations);
        assert!(result.ops_per_sec > 0.0);
    }

    #[test]
    fn test_quick_benchmark() {
        let result = performance::quick_benchmark("test", || {
            std::thread::sleep(std::time::Duration::from_millis(10));
        });

        assert_eq!(result.iterations, 1);
        assert!(result.total_duration.as_millis() >= 10);
    }

    #[test]
    fn test_benchmark_display() {
        let counter = std::sync::atomic::AtomicU64::new(0);

        let result = performance::benchmark("test", 100, || {
            counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        });

        let display = format!("{}", result);
        assert!(display.contains("Benchmark: test"));
        assert!(display.contains("Iterations:"));
    }
}
