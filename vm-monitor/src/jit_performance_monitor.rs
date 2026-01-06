//! JIT性能监控服务（简化版）
//!
//! 独立的JIT性能监控器，不依赖DomainEventBus

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

/// JIT性能指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventBasedJitMetrics {
    /// 总编译次数
    pub total_compilations: u64,
    /// 总编译时间（纳秒）
    pub total_compile_time_ns: u64,
    /// 平均编译时间（纳秒）
    pub avg_compile_time_ns: u64,
    /// 最小编译时间（纳秒）
    pub min_compile_time_ns: u64,
    /// 最大编译时间（纳秒）
    pub max_compile_time_ns: u64,
    /// 热点检测次数
    pub hotspot_detections: u64,
    /// 最后更新时间（Unix时间戳，秒）
    pub last_updated_timestamp: u64,
}

impl Default for EventBasedJitMetrics {
    fn default() -> Self {
        Self {
            total_compilations: 0,
            total_compile_time_ns: 0,
            avg_compile_time_ns: 0,
            min_compile_time_ns: u64::MAX,
            max_compile_time_ns: 0,
            hotspot_detections: 0,
            last_updated_timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

impl EventBasedJitMetrics {
    /// 更新编译时间指标
    pub fn update_compile_time(&mut self, compile_time_ns: u64) {
        self.total_compilations += 1;
        self.total_compile_time_ns += compile_time_ns;
        self.avg_compile_time_ns = self.total_compile_time_ns / self.total_compilations;
        self.min_compile_time_ns = self.min_compile_time_ns.min(compile_time_ns);
        self.max_compile_time_ns = self.max_compile_time_ns.max(compile_time_ns);
        self.last_updated_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }

    /// 增加热点检测计数
    pub fn increment_hotspot(&mut self) {
        self.hotspot_detections += 1;
        self.last_updated_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }
}

/// JIT性能监控器（简化版）
///
/// 独立的监控器，可以手动记录编译事件
pub struct EventBasedJitMonitor {
    /// 按代码块地址分组的指标
    block_metrics: Arc<Mutex<HashMap<u64, EventBasedJitMetrics>>>,
    /// 全局汇总指标
    global_metrics: Arc<Mutex<EventBasedJitMetrics>>,
}

impl EventBasedJitMonitor {
    /// 创建新的JIT性能监控器
    pub fn new() -> Self {
        Self {
            block_metrics: Arc::new(Mutex::new(HashMap::new())),
            global_metrics: Arc::new(Mutex::new(EventBasedJitMetrics::default())),
        }
    }

    /// 记录编译事件
    pub fn record_compilation(&self, block_addr: u64, compile_time_ns: u64) {
        // 更新全局指标
        let mut global = self.global_metrics.lock();
        global.update_compile_time(compile_time_ns);

        // 更新特定代码块指标
        let mut metrics = self.block_metrics.lock();
        let block_metric = metrics.entry(block_addr).or_default();
        block_metric.update_compile_time(compile_time_ns);
    }

    /// 记录热点检测
    pub fn record_hotspot(&self, block_addr: u64) {
        let mut global = self.global_metrics.lock();
        global.increment_hotspot();

        // 更新特定代码块热点计数
        let mut metrics = self.block_metrics.lock();
        let block_metric = metrics.entry(block_addr).or_default();
        block_metric.increment_hotspot();
    }

    /// 获取全局性能指标
    pub fn get_global_metrics(&self) -> EventBasedJitMetrics {
        self.global_metrics.lock().clone()
    }

    /// 获取特定代码块的指标
    pub fn get_block_metrics(&self, block_addr: u64) -> Option<EventBasedJitMetrics> {
        self.block_metrics.lock().get(&block_addr).cloned()
    }

    /// 获取所有代码块指标
    pub fn get_all_block_metrics(&self) -> HashMap<u64, EventBasedJitMetrics> {
        self.block_metrics.lock().clone()
    }

    /// 生成性能报告
    pub fn generate_report(&self) -> EventBasedPerformanceReport {
        let global = self.get_global_metrics();
        let blocks = self.get_all_block_metrics();

        EventBasedPerformanceReport {
            global_metrics: global.clone(),
            blocks_reported: blocks.len(),
            total_blocks: blocks.len(),
            slowest_blocks: Self::find_slowest_blocks(&blocks, 10),
            most_hot_blocks: Self::find_hot_blocks(&blocks, 10),
            generated_at_timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    /// 查找编译最慢的代码块
    fn find_slowest_blocks(
        metrics: &HashMap<u64, EventBasedJitMetrics>,
        count: usize,
    ) -> Vec<(u64, EventBasedJitMetrics)> {
        let mut blocks: Vec<_> = metrics.iter().collect();
        blocks.sort_by(|a, b| b.1.avg_compile_time_ns.cmp(&a.1.avg_compile_time_ns));
        blocks
            .into_iter()
            .take(count)
            .map(|(addr, m)| (*addr, m.clone()))
            .collect()
    }

    /// 查找最热的代码块
    fn find_hot_blocks(
        metrics: &HashMap<u64, EventBasedJitMetrics>,
        count: usize,
    ) -> Vec<(u64, EventBasedJitMetrics)> {
        let mut blocks: Vec<_> = metrics.iter().collect();
        blocks.sort_by(|a, b| b.1.hotspot_detections.cmp(&a.1.hotspot_detections));
        blocks
            .into_iter()
            .take(count)
            .map(|(addr, m)| (*addr, m.clone()))
            .collect()
    }

    /// 重置所有指标
    pub fn reset(&self) {
        self.block_metrics.lock().clear();
        *self.global_metrics.lock() = EventBasedJitMetrics::default();
    }

    /// 打印性能摘要
    pub fn print_summary(&self) {
        let global = self.get_global_metrics();
        println!("=== JIT Performance Summary ===");
        println!("Total compilations: {}", global.total_compilations);
        println!(
            "Total compile time: {:.2}s",
            global.total_compile_time_ns as f64 / 1e9
        );
        println!(
            "Avg compile time: {:.2}μs",
            global.avg_compile_time_ns as f64 / 1e3
        );
        println!(
            "Min compile time: {:.2}μs",
            global.min_compile_time_ns as f64 / 1e3
        );
        println!(
            "Max compile time: {:.2}μs",
            global.max_compile_time_ns as f64 / 1e3
        );
        println!("Hotspot detections: {}", global.hotspot_detections);
        println!("===============================");
    }
}

impl Default for EventBasedJitMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// 性能报告（事件驱动版本）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventBasedPerformanceReport {
    /// 全局指标
    pub global_metrics: EventBasedJitMetrics,
    /// 已报告的代码块数量
    pub blocks_reported: usize,
    /// 代码块总数
    pub total_blocks: usize,
    /// 编译最慢的代码块
    pub slowest_blocks: Vec<(u64, EventBasedJitMetrics)>,
    /// 最热的代码块
    pub most_hot_blocks: Vec<(u64, EventBasedJitMetrics)>,
    /// 报告生成时间（Unix时间戳）
    pub generated_at_timestamp: u64,
}

impl EventBasedPerformanceReport {
    /// 打印报告
    pub fn print(&self) {
        println!("\n=== JIT Performance Report ===");
        println!("Generated at: {}s since epoch", self.generated_at_timestamp);

        println!("\n--- Global Metrics ---");
        println!(
            "Total compilations: {}",
            self.global_metrics.total_compilations
        );
        println!(
            "Average compile time: {:.2}μs",
            self.global_metrics.avg_compile_time_ns as f64 / 1e3
        );

        if !self.slowest_blocks.is_empty() {
            println!(
                "\n--- Slowest Compilation Blocks (Top {}) ---",
                self.slowest_blocks.len()
            );
            for (i, (addr, metrics)) in self.slowest_blocks.iter().enumerate() {
                println!(
                    "{}. Block 0x{:x}: {:.2}μs avg, {} compilations",
                    i + 1,
                    addr,
                    metrics.avg_compile_time_ns as f64 / 1e3,
                    metrics.total_compilations
                );
            }
        }

        if !self.most_hot_blocks.is_empty() {
            println!(
                "\n--- Hottest Blocks (Top {}) ---",
                self.most_hot_blocks.len()
            );
            for (i, (addr, metrics)) in self.most_hot_blocks.iter().enumerate() {
                println!(
                    "{}. Block 0x{:x}: {} hotspot detections, {} compilations",
                    i + 1,
                    addr,
                    metrics.hotspot_detections,
                    metrics.total_compilations
                );
            }
        }

        println!("\n===========================\n");
    }

    /// 导出为JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_update() {
        let mut metrics = EventBasedJitMetrics::default();
        metrics.update_compile_time(1000);
        metrics.update_compile_time(2000);
        metrics.update_compile_time(3000);

        assert_eq!(metrics.total_compilations, 3);
        assert_eq!(metrics.avg_compile_time_ns, 2000);
        assert_eq!(metrics.min_compile_time_ns, 1000);
        assert_eq!(metrics.max_compile_time_ns, 3000);
    }

    #[test]
    fn test_hotspot_tracking() {
        let mut metrics = EventBasedJitMetrics::default();
        metrics.increment_hotspot();
        metrics.increment_hotspot();

        assert_eq!(metrics.hotspot_detections, 2);
    }

    #[test]
    fn test_monitor_record_compilation() {
        let monitor = EventBasedJitMonitor::new();
        monitor.record_compilation(0x1000, 1000);
        monitor.record_compilation(0x1000, 2000);
        monitor.record_compilation(0x2000, 3000);

        let metrics = monitor.get_block_metrics(0x1000).unwrap();
        assert_eq!(metrics.total_compilations, 2);
        assert_eq!(metrics.avg_compile_time_ns, 1500);

        let global = monitor.get_global_metrics();
        assert_eq!(global.total_compilations, 3);
    }

    #[test]
    fn test_report_generation() {
        let monitor = EventBasedJitMonitor::new();
        monitor.record_compilation(0x1000, 1000);
        monitor.record_compilation(0x2000, 2000);
        monitor.record_hotspot(0x1000);

        let report = monitor.generate_report();
        assert_eq!(report.total_blocks, 2);
        assert_eq!(report.global_metrics.total_compilations, 2);
        assert_eq!(report.global_metrics.hotspot_detections, 1);
    }

    #[test]
    fn test_monitor_reset() {
        let monitor = EventBasedJitMonitor::new();
        monitor.record_compilation(0x1000, 1000);
        monitor.record_hotspot(0x1000);

        monitor.reset();

        let global = monitor.get_global_metrics();
        assert_eq!(global.total_compilations, 0);
        assert_eq!(global.hotspot_detections, 0);
        assert_eq!(monitor.get_all_block_metrics().len(), 0);
    }
}
