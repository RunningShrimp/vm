//! JIT性能监控服务
//!
//! 本模块提供JIT编译和执行的实时性能监控功能。
//! 订阅DomainEventBus上的JIT相关事件，收集性能指标，生成报告。
//!
//! ## 功能
//!
//! - 订阅JIT编译事件（CodeBlockCompiled）
//! - 订阅热点检测事件（HotspotDetected）
//! - 收集性能统计信息
//! - 生成性能分析报告
//!
//! ## 使用示例
//!
//! ```rust,no_run
//! use std::sync::Arc;
//! use vm_core::domain_services::{DomainEventBus, ExecutionEvent};
//! use vm_monitor::jit_monitor::JitPerformanceMonitor;
//!
//! let event_bus = Arc::new(DomainEventBus::new());
//! let monitor = JitPerformanceMonitor::new();
//!
//! // 手动处理事件
//! let event = ExecutionEvent::CodeBlockCompiled {
//!     vm_id: "test-vm".to_string(),
//!     pc: 0x1000,
//!     block_size: 256,
//! };
//! monitor.handle_code_block_compiled(&event);
//!
//! // 生成报告
//! let report = monitor.generate_report();
//! println!("{}", report);
//! ```

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Instant;

// 重新导出vm-core的ExecutionEvent
pub use vm_core::domain_services::ExecutionEvent;

/// JIT性能指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JitMetric {
    /// 指标名称
    pub name: String,
    /// 指标值
    pub value: f64,
    /// 单位
    pub unit: String,
    /// 时间戳
    pub timestamp: std::time::SystemTime,
}

/// 代码块编译记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompilationRecord {
    /// VM ID
    pub vm_id: String,
    /// PC地址
    pub pc: u64,
    /// 代码块大小
    pub block_size: usize,
    /// 编译时间（毫秒）
    pub compile_time_ms: u64,
    /// 时间戳
    pub timestamp: std::time::SystemTime,
}

/// 热点检测记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotspotRecord {
    /// VM ID
    pub vm_id: String,
    /// PC地址
    pub pc: u64,
    /// 执行次数
    pub execution_count: u64,
    /// 时间戳
    pub timestamp: std::time::SystemTime,
}

/// JIT性能统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JitStatistics {
    /// 总编译次数
    pub total_compilations: u64,
    /// 总编译代码大小（字节）
    pub total_compiled_bytes: u64,
    /// 平均代码块大小
    pub avg_block_size: f64,
    /// 总热点检测次数
    pub total_hotspots: u64,
    /// 平均执行次数（热点）
    pub avg_execution_count: f64,
}

/// JIT性能报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    /// 报告生成时间
    pub generated_at: std::time::SystemTime,
    /// 监控时长（秒）
    pub monitoring_duration_secs: u64,
    /// JIT统计
    pub statistics: JitStatistics,
    /// 最近编译记录（最多100条）
    pub recent_compilations: Vec<CompilationRecord>,
    /// 最近热点记录（最多100条）
    pub recent_hotspots: Vec<HotspotRecord>,
    /// 性能指标
    pub metrics: Vec<JitMetric>,
}

/// JIT性能监控器
pub struct JitPerformanceMonitor {
    /// 开始时间
    start_time: Instant,
    /// 编译记录
    compilations: Mutex<VecDeque<CompilationRecord>>,
    /// 热点记录
    hotspots: Mutex<VecDeque<HotspotRecord>>,
    /// 编译次数统计
    compilation_count: Mutex<u64>,
    /// 编译字节数统计
    compiled_bytes: Mutex<u64>,
    /// 热点检测次数统计
    hotspot_count: Mutex<u64>,
}

impl Default for JitPerformanceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl JitPerformanceMonitor {
    /// 创建新的JIT性能监控器
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            compilations: Mutex::new(VecDeque::with_capacity(100)),
            hotspots: Mutex::new(VecDeque::with_capacity(100)),
            compilation_count: Mutex::new(0),
            compiled_bytes: Mutex::new(0),
            hotspot_count: Mutex::new(0),
        }
    }

    /// 处理代码块编译事件
    pub fn handle_code_block_compiled(&self, event: &ExecutionEvent) {
        if let ExecutionEvent::CodeBlockCompiled {
            vm_id,
            pc,
            block_size,
        } = event
        {
            let record = CompilationRecord {
                vm_id: vm_id.clone(),
                pc: *pc,
                block_size: *block_size,
                compile_time_ms: 0, // 实际应该从事件中获取
                timestamp: std::time::SystemTime::now(),
            };

            let mut comps = self.compilations.lock().unwrap();
            comps.push_back(record);
            if comps.len() > 100 {
                comps.pop_front();
            }

            *self.compilation_count.lock().unwrap() += 1;
            *self.compiled_bytes.lock().unwrap() += *block_size as u64;
        }
    }

    /// 处理热点检测事件
    pub fn handle_hotspot_detected(&self, event: &ExecutionEvent) {
        if let ExecutionEvent::HotspotDetected {
            vm_id,
            pc,
            execution_count,
        } = event
        {
            let record = HotspotRecord {
                vm_id: vm_id.clone(),
                pc: *pc,
                execution_count: *execution_count,
                timestamp: std::time::SystemTime::now(),
            };

            let mut spots = self.hotspots.lock().unwrap();
            spots.push_back(record);
            if spots.len() > 100 {
                spots.pop_front();
            }

            *self.hotspot_count.lock().unwrap() += 1;
        }
    }

    /// 生成性能报告
    pub fn generate_report(&self) -> PerformanceReport {
        let compilations = self.compilations.lock().unwrap();
        let hotspots = self.hotspots.lock().unwrap();
        let compilation_count = *self.compilation_count.lock().unwrap();
        let compiled_bytes = *self.compiled_bytes.lock().unwrap();
        let hotspot_count = *self.hotspot_count.lock().unwrap();

        let avg_block_size = if compilation_count > 0 {
            compiled_bytes as f64 / compilation_count as f64
        } else {
            0.0
        };

        let total_execution_count: u64 = hotspots.iter().map(|h| h.execution_count).sum();
        let avg_execution_count = if hotspot_count > 0 {
            total_execution_count as f64 / hotspot_count as f64
        } else {
            0.0
        };

        let statistics = JitStatistics {
            total_compilations: compilation_count,
            total_compiled_bytes: compiled_bytes,
            avg_block_size,
            total_hotspots: hotspot_count,
            avg_execution_count,
        };

        let recent_compilations: Vec<CompilationRecord> = compilations.iter().cloned().collect();
        let recent_hotspots: Vec<HotspotRecord> = hotspots.iter().cloned().collect();

        // 生成性能指标
        let metrics = vec![
            JitMetric {
                name: "compilation_rate".to_string(),
                value: compilation_count as f64 / self.start_time.elapsed().as_secs_f64(),
                unit: "compilations/sec".to_string(),
                timestamp: std::time::SystemTime::now(),
            },
            JitMetric {
                name: "throughput_mb".to_string(),
                value: (compiled_bytes as f64) / (1024.0 * 1024.0),
                unit: "MB".to_string(),
                timestamp: std::time::SystemTime::now(),
            },
            JitMetric {
                name: "hotspot_rate".to_string(),
                value: hotspot_count as f64 / self.start_time.elapsed().as_secs_f64(),
                unit: "hotspots/sec".to_string(),
                timestamp: std::time::SystemTime::now(),
            },
        ];

        PerformanceReport {
            generated_at: std::time::SystemTime::now(),
            monitoring_duration_secs: self.start_time.elapsed().as_secs(),
            statistics,
            recent_compilations,
            recent_hotspots,
            metrics,
        }
    }

    /// 获取当前统计信息（快照）
    pub fn get_statistics(&self) -> JitStatistics {
        let compilation_count = *self.compilation_count.lock().unwrap();
        let compiled_bytes = *self.compiled_bytes.lock().unwrap();
        let hotspot_count = *self.hotspot_count.lock().unwrap();

        let avg_block_size = if compilation_count > 0 {
            compiled_bytes as f64 / compilation_count as f64
        } else {
            0.0
        };

        let hotspots = self.hotspots.lock().unwrap();
        let total_execution_count: u64 = hotspots.iter().map(|h| h.execution_count).sum();
        let avg_execution_count = if hotspot_count > 0 {
            total_execution_count as f64 / hotspot_count as f64
        } else {
            0.0
        };

        JitStatistics {
            total_compilations: compilation_count,
            total_compiled_bytes: compiled_bytes,
            avg_block_size,
            total_hotspots: hotspot_count,
            avg_execution_count,
        }
    }

    /// 重置统计信息
    pub fn reset(&self) {
        self.compilations.lock().unwrap().clear();
        self.hotspots.lock().unwrap().clear();
        *self.compilation_count.lock().unwrap() = 0;
        *self.compiled_bytes.lock().unwrap() = 0;
        *self.hotspot_count.lock().unwrap() = 0;
    }
}

impl std::fmt::Display for PerformanceReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "=== JIT性能报告 ===")?;
        writeln!(f, "生成时间: {:?}", self.generated_at)?;
        writeln!(f, "监控时长: {} 秒", self.monitoring_duration_secs)?;
        writeln!(f)?;
        writeln!(f, "--- 统计信息 ---")?;
        writeln!(f, "总编译次数: {}", self.statistics.total_compilations)?;
        writeln!(
            f,
            "总编译字节: {} bytes",
            self.statistics.total_compiled_bytes
        )?;
        writeln!(
            f,
            "平均代码块大小: {:.2} bytes",
            self.statistics.avg_block_size
        )?;
        writeln!(f, "总热点检测: {}", self.statistics.total_hotspots)?;
        writeln!(
            f,
            "平均执行次数: {:.2}",
            self.statistics.avg_execution_count
        )?;
        writeln!(f)?;
        writeln!(f, "--- 性能指标 ---")?;
        for metric in &self.metrics {
            writeln!(f, "{}: {:.2} {}", metric.name, metric.value, metric.unit)?;
        }
        writeln!(f)?;
        writeln!(
            f,
            "--- 最近编译记录 ({}条) ---",
            self.recent_compilations.len()
        )?;
        for (i, record) in self.recent_compilations.iter().take(10).enumerate() {
            writeln!(
                f,
                "{}. PC=0x{:x}, size={} bytes",
                i + 1,
                record.pc,
                record.block_size
            )?;
        }
        writeln!(f)?;
        writeln!(f, "--- 最近热点记录 ({}条) ---", self.recent_hotspots.len())?;
        for (i, record) in self.recent_hotspots.iter().take(10).enumerate() {
            writeln!(
                f,
                "{}. PC=0x{:x}, exec_count={}",
                i + 1,
                record.pc,
                record.execution_count
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor_creation() {
        let monitor = JitPerformanceMonitor::new();

        let stats = monitor.get_statistics();
        assert_eq!(stats.total_compilations, 0);
        assert_eq!(stats.total_hotspots, 0);
    }

    #[test]
    fn test_report_generation() {
        let monitor = JitPerformanceMonitor::new();

        let report = monitor.generate_report();
        assert_eq!(report.statistics.total_compilations, 0);
        assert_eq!(report.recent_compilations.len(), 0);
        assert_eq!(report.recent_hotspots.len(), 0);
    }

    #[test]
    fn test_reset() {
        let monitor = JitPerformanceMonitor::new();

        // 模拟一些数据
        {
            let mut comps = monitor.compilations.lock().unwrap();
            comps.push_back(CompilationRecord {
                vm_id: "test".to_string(),
                pc: 0x1000,
                block_size: 256,
                compile_time_ms: 10,
                timestamp: std::time::SystemTime::now(),
            });
        }
        *monitor.compilation_count.lock().unwrap() = 1;

        // 重置
        monitor.reset();

        // 验证
        let stats = monitor.get_statistics();
        assert_eq!(stats.total_compilations, 0);
        assert_eq!(monitor.compilations.lock().unwrap().len(), 0);
    }
}
