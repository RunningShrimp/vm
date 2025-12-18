//! VirtIO 设备性能统计
//!
//! 提供统一的性能监控和统计功能

use std::sync::{Arc, Mutex};
use std::time::Instant;

/// 设备性能统计
#[derive(Debug, Clone)]
pub struct DevicePerformanceStats {
    /// 总请求数
    pub total_requests: u64,
    /// 成功请求数
    pub successful_requests: u64,
    /// 失败请求数
    pub failed_requests: u64,
    /// 总处理时间（微秒）
    pub total_processing_time_us: u64,
    /// 平均延迟（微秒）
    pub average_latency_us: u64,
    /// 最大延迟（微秒）
    pub max_latency_us: u64,
    /// 最小延迟（微秒）
    pub min_latency_us: u64,
    /// 总传输字节数
    pub total_bytes_transferred: u64,
    /// 平均吞吐量（字节/秒）
    pub average_throughput_bps: u64,
    /// 最后更新时间
    pub last_update: Instant,
}

impl DevicePerformanceStats {
    /// 创建新的统计对象
    pub fn new() -> Self {
        Self {
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            total_processing_time_us: 0,
            average_latency_us: 0,
            max_latency_us: 0,
            min_latency_us: u64::MAX,
            total_bytes_transferred: 0,
            average_throughput_bps: 0,
            last_update: Instant::now(),
        }
    }

    /// 记录请求
    pub fn record_request(&mut self, success: bool, latency_us: u64, bytes: u64) {
        self.total_requests += 1;
        if success {
            self.successful_requests += 1;
        } else {
            self.failed_requests += 1;
        }

        self.total_processing_time_us += latency_us;
        self.total_bytes_transferred += bytes;

        // 更新延迟统计
        if latency_us > self.max_latency_us {
            self.max_latency_us = latency_us;
        }
        if latency_us < self.min_latency_us {
            self.min_latency_us = latency_us;
        }

        // 计算平均值
        if self.total_requests > 0 {
            self.average_latency_us = self.total_processing_time_us / self.total_requests;
        }

        // 计算吞吐量（基于总时间和总字节数）
        let elapsed_secs = self.last_update.elapsed().as_secs().max(1);
        self.average_throughput_bps = self.total_bytes_transferred / elapsed_secs;

        self.last_update = Instant::now();
    }

    /// 重置统计信息
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// 获取成功率（百分比）
    pub fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            return 0.0;
        }
        (self.successful_requests as f64 / self.total_requests as f64) * 100.0
    }
}

/// 性能监控器
pub struct PerformanceMonitor {
    /// 统计信息
    stats: Arc<Mutex<DevicePerformanceStats>>,
    /// 是否启用监控
    enabled: bool,
}

impl PerformanceMonitor {
    /// 创建新的性能监控器
    pub fn new(enabled: bool) -> Self {
        Self {
            stats: Arc::new(Mutex::new(DevicePerformanceStats::new())),
            enabled,
        }
    }

    /// 启用监控
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// 禁用监控
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// 是否启用
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// 记录请求（带时间测量）
    pub fn record_request_with_timing<F>(&self, f: F) -> Result<u64, VmError>
    where
        F: FnOnce() -> Result<u64, VmError>,
    {
        if !self.enabled {
            return f();
        }

        let start = Instant::now();
        let result = f();
        let latency = start.elapsed().as_micros() as u64;

        let (success, bytes) = match &result {
            Ok(b) => (true, *b),
            Err(_) => (false, 0),
        };

        if let Ok(mut stats) = self.stats.lock() {
            stats.record_request(success, latency, bytes);
        }

        result
    }

    /// 记录请求（手动）
    pub fn record_request(&self, success: bool, latency_us: u64, bytes: u64) {
        if !self.enabled {
            return;
        }

        if let Ok(mut stats) = self.stats.lock() {
            stats.record_request(success, latency_us, bytes);
        }
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> DevicePerformanceStats {
        if let Ok(stats) = self.stats.lock() {
            stats.clone()
        } else {
            DevicePerformanceStats::new()
        }
    }

    /// 重置统计信息
    pub fn reset_stats(&self) {
        if let Ok(mut stats) = self.stats.lock() {
            stats.reset();
        }
    }
}

impl Default for PerformanceMonitor {
    fn default() -> Self {
        Self::new(true)
    }
}

use vm_core::VmError;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_stats() {
        let mut stats = DevicePerformanceStats::new();

        stats.record_request(true, 100, 1024);
        stats.record_request(true, 200, 2048);
        stats.record_request(false, 50, 0);

        assert_eq!(stats.total_requests, 3);
        assert_eq!(stats.successful_requests, 2);
        assert_eq!(stats.failed_requests, 1);
        assert_eq!(stats.total_bytes_transferred, 3072);
        assert_eq!(stats.max_latency_us, 200);
        assert_eq!(stats.min_latency_us, 50);
    }

    #[test]
    fn test_performance_monitor() {
        let monitor = PerformanceMonitor::new(true);

        let result = monitor.record_request_with_timing(|| {
            std::thread::sleep(Duration::from_millis(10));
            Ok(1024)
        });

        assert!(result.is_ok());
        let stats = monitor.get_stats();
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.successful_requests, 1);
        assert!(stats.average_latency_us > 0);
    }
}
