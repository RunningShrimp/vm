//! # vm-monitor - 虚拟机性能监控框架
//!
//! 提供全面的性能监控、指标收集、分析和可视化功能。
//!
//! ## 主要功能
//!
//! - **实时性能监控**: CPU、内存、I/O、网络等系统指标
//! - **虚拟机特定指标**: TLB命中率、JIT编译统计、设备性能等
//! - **历史数据存储**: 时间序列数据存储和查询
//! - **告警系统**: 基于阈值的性能告警
//! - **可视化界面**: Web界面展示性能数据
//! - **API接口**: RESTful API用于外部集成

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::broadcast;

pub mod vendor_metrics;
pub use vendor_metrics::{
    PerformanceAnalyzer as VendorPerformanceAnalyzer, VendorMetricsCollector,
};

pub mod performance_analyzer;
pub use performance_analyzer::PerformanceAnalyzer;

pub mod metrics_collector;
pub use metrics_collector::{
    GcMetrics, JitMetrics, MemoryMetrics, MetricsCollector, ParallelMetrics, SystemMetrics,
    SystemOverallMetrics, TlbMetrics,
};

pub mod alerts;
pub mod dashboard;
pub mod export;

pub use alerts::{Alert, AlertLevel, AlertManager, AlertType};

/// 监控错误类型
#[derive(Debug, Error)]
pub enum MonitorError {
    #[error("RwLock poisoned")]
    LockError,
    #[error("Metric '{0}' already exists")]
    MetricExists(String),
    #[error("Alert rule '{0}' already exists")]
    AlertRuleExists(String),
    #[error("System time error")]
    TimeError,
    #[error("No data available")]
    NoData,
}

/// 性能快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSnapshot {
    /// 时间戳
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// 会话ID
    pub session_id: uuid::Uuid,
    /// 运行时间
    pub uptime: std::time::Duration,
    /// 性能指标
    pub metrics: SystemMetrics,
    /// 告警列表
    pub alerts: Vec<Alert>,
}

/// 告警阈值配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertThresholds {
    /// JIT执行速度阈值
    pub jit_execution_rate: f64,
    /// TLB命中率阈值
    pub tlb_hit_rate: f64,
    /// 内存使用率阈值
    pub memory_usage_rate: f64,
    /// GC暂停时间阈值（纳秒）
    pub gc_pause_time_ns: u64,
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            jit_execution_rate: 1000.0,
            tlb_hit_rate: 0.9,
            memory_usage_rate: 0.8,
            gc_pause_time_ns: 1_000_000, // 1ms
        }
    }
}

/// 仪表板配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DashboardConfig {
    /// 服务器配置
    pub server: ServerConfig,
}

/// 服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// 绑定地址
    pub bind_address: String,
    /// 绑定端口
    pub bind_port: u16,
    /// 静态文件目录
    pub static_dir: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0".to_string(),
            bind_port: 8080,
            static_dir: None,
        }
    }
}

/// Prometheus配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrometheusConfig {
    /// 绑定地址
    pub bind_address: String,
    /// 绑定端口
    pub bind_port: u16,
    /// 指标前缀
    pub metrics_prefix: String,
}

/// 监控指标类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MetricType {
    // 系统级指标
    CpuUsage,
    MemoryUsage,
    DiskIo,
    NetworkIo,

    // 虚拟机级指标
    VmCpuUsage,
    VmMemoryUsage,
    VmDiskIo,
    VmNetworkIo,

    // JIT编译指标
    JitCompileTime,
    JitCompileCount,
    JitExecutionCount,

    // TLB指标
    TlbHitRate,
    TlbLookupCount,
    TlbMissCount,

    // 设备指标
    DeviceIoLatency,
    DeviceThroughput,

    // 自定义指标
    Custom,
}

/// 指标数据点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricPoint {
    /// 时间戳（Unix时间戳，毫秒）
    pub timestamp: u64,
    /// 指标值
    pub value: f64,
    /// 标签（用于分组和过滤）
    pub tags: HashMap<String, String>,
}

/// 指标配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricConfig {
    /// 指标名称
    pub name: String,
    /// 指标类型
    pub metric_type: MetricType,
    /// 收集间隔（秒）
    pub collection_interval: u64,
    /// 保留时间（小时）
    pub retention_hours: u64,
    /// 标签
    pub tags: HashMap<String, String>,
}

/// 告警规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    /// 规则ID
    pub id: String,
    /// 规则名称
    pub name: String,
    /// 指标名称
    pub metric_name: String,
    /// 条件（例如: "value > 80"）
    pub condition: String,
    /// 告警级别
    pub severity: AlertSeverity,
    /// 描述
    pub description: String,
    /// 是否启用
    pub enabled: bool,
}

/// 告警级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// 告警事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertEvent {
    /// 事件ID
    pub id: String,
    /// 规则ID
    pub rule_id: String,
    /// 时间戳
    pub timestamp: u64,
    /// 告警级别
    pub severity: AlertSeverity,
    /// 消息
    pub message: String,
    /// 指标值
    pub value: f64,
    /// 阈值
    pub threshold: f64,
}

/// 性能监控器
pub struct PerformanceMonitor {
    /// 指标存储
    metrics: Arc<RwLock<HashMap<String, VecDeque<MetricPoint>>>>,
    /// 指标配置
    metric_configs: Arc<RwLock<HashMap<String, MetricConfig>>>,
    /// 告警规则
    alert_rules: Arc<RwLock<HashMap<String, AlertRule>>>,
    /// 活跃告警
    active_alerts: Arc<RwLock<HashMap<String, AlertEvent>>>,
    /// 告警广播通道
    alert_sender: broadcast::Sender<AlertEvent>,
    /// 监控任务句柄
    monitor_handles: Arc<RwLock<Vec<tokio::task::JoinHandle<()>>>>,
    /// 配置
    config: MonitorConfig,
}

/// 监控器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorConfig {
    /// 最大指标保留点数
    pub max_points_per_metric: usize,
    /// 默认收集间隔（秒）
    pub default_collection_interval: u64,
    /// 默认保留时间（小时）
    pub default_retention_hours: u64,
    /// 告警检查间隔（秒）
    pub alert_check_interval: u64,
    /// 启用Web界面
    pub enable_web_interface: bool,
    /// Web界面端口
    pub web_port: u16,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            max_points_per_metric: 10000,
            default_collection_interval: 10,
            default_retention_hours: 24,
            alert_check_interval: 30,
            enable_web_interface: true,
            web_port: 8080,
        }
    }
}

impl PerformanceMonitor {
    /// 创建新的性能监控器
    pub fn new(config: MonitorConfig) -> Self {
        let (alert_sender, _) = broadcast::channel(100);

        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
            metric_configs: Arc::new(RwLock::new(HashMap::new())),
            alert_rules: Arc::new(RwLock::new(HashMap::new())),
            active_alerts: Arc::new(RwLock::new(HashMap::new())),
            alert_sender,
            monitor_handles: Arc::new(RwLock::new(Vec::new())),
            config,
        }
    }

    // Helper methods for lock operations
    fn lock_metric_configs(
        &self,
    ) -> Result<std::sync::RwLockWriteGuard<'_, HashMap<String, MetricConfig>>, MonitorError> {
        self.metric_configs
            .write()
            .map_err(|_| MonitorError::LockError)
    }

    fn lock_metrics_write(
        &self,
    ) -> Result<std::sync::RwLockWriteGuard<'_, HashMap<String, VecDeque<MetricPoint>>>, MonitorError>
    {
        self.metrics.write().map_err(|_| MonitorError::LockError)
    }

    fn lock_metrics_read(
        &self,
    ) -> Result<std::sync::RwLockReadGuard<'_, HashMap<String, VecDeque<MetricPoint>>>, MonitorError>
    {
        self.metrics.read().map_err(|_| MonitorError::LockError)
    }

    fn lock_alert_rules_write(
        &self,
    ) -> Result<std::sync::RwLockWriteGuard<'_, HashMap<String, AlertRule>>, MonitorError> {
        self.alert_rules
            .write()
            .map_err(|_| MonitorError::LockError)
    }

    fn lock_alert_rules_read(
        &self,
    ) -> Result<std::sync::RwLockReadGuard<'_, HashMap<String, AlertRule>>, MonitorError> {
        self.alert_rules.read().map_err(|_| MonitorError::LockError)
    }

    fn lock_active_alerts_read(
        &self,
    ) -> Result<std::sync::RwLockReadGuard<'_, HashMap<String, AlertEvent>>, MonitorError> {
        self.active_alerts
            .read()
            .map_err(|_| MonitorError::LockError)
    }

    fn lock_active_alerts_write(
        &self,
    ) -> Result<std::sync::RwLockWriteGuard<'_, HashMap<String, AlertEvent>>, MonitorError> {
        self.active_alerts
            .write()
            .map_err(|_| MonitorError::LockError)
    }

    fn lock_monitor_handles_write(
        &self,
    ) -> Result<std::sync::RwLockWriteGuard<'_, Vec<tokio::task::JoinHandle<()>>>, MonitorError>
    {
        self.monitor_handles
            .write()
            .map_err(|_| MonitorError::LockError)
    }

    fn get_timestamp_ms() -> Result<u64, MonitorError> {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .map_err(|_| MonitorError::TimeError)
    }

    /// 注册指标
    pub fn register_metric(&self, config: MetricConfig) -> Result<(), String> {
        let mut configs = self.lock_metric_configs().map_err(|e| e.to_string())?;
        if configs.contains_key(&config.name) {
            return Err(format!("Metric '{}' already exists", config.name));
        }

        configs.insert(config.name.clone(), config);
        Ok(())
    }

    /// 记录指标值
    pub fn record_metric(&self, name: &str, value: f64, tags: HashMap<String, String>) {
        let timestamp = match Self::get_timestamp_ms() {
            Ok(ts) => ts,
            Err(_) => return, // Silently fail on time error
        };

        let point = MetricPoint {
            timestamp,
            value,
            tags,
        };

        let mut metrics = match self.lock_metrics_write() {
            Ok(m) => m,
            Err(_) => return, // Silently fail on lock error
        };
        let points = metrics.entry(name.to_string()).or_default();

        points.push_back(point);

        // 限制点数
        while points.len() > self.config.max_points_per_metric {
            points.pop_front();
        }
    }

    /// 获取指标数据
    pub fn get_metric_data(
        &self,
        name: &str,
        start_time: Option<u64>,
        end_time: Option<u64>,
    ) -> Vec<MetricPoint> {
        let metrics = match self.lock_metrics_read() {
            Ok(m) => m,
            Err(_) => return Vec::new(), // Return empty on lock error
        };
        if let Some(points) = metrics.get(name) {
            let filtered: Vec<_> = points
                .iter()
                .filter(|p| {
                    if let Some(start) = start_time
                        && p.timestamp < start
                    {
                        return false;
                    }
                    if let Some(end) = end_time
                        && p.timestamp > end
                    {
                        return false;
                    }
                    true
                })
                .cloned()
                .collect();
            filtered
        } else {
            Vec::new()
        }
    }

    /// 计算指标统计信息
    pub fn get_metric_stats(&self, name: &str, window_seconds: u64) -> Option<MetricStats> {
        let end_time = match Self::get_timestamp_ms() {
            Ok(t) => t,
            Err(_) => return None,
        };
        let start_time = end_time.saturating_sub(window_seconds * 1000);

        let data = self.get_metric_data(name, Some(start_time), Some(end_time));
        if data.is_empty() {
            return None;
        }

        let values: Vec<f64> = data.iter().map(|p| p.value).collect();
        let min = values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max = values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let sum: f64 = values.iter().sum();
        let avg = sum / values.len() as f64;

        // 计算标准差
        let variance = values.iter().map(|v| (v - avg).powi(2)).sum::<f64>() / values.len() as f64;
        let std_dev = variance.sqrt();

        let latest = *values.last().unwrap_or(&0.0);

        Some(MetricStats {
            count: values.len(),
            min,
            max,
            avg,
            std_dev,
            latest,
        })
    }

    /// 添加告警规则
    pub fn add_alert_rule(&self, rule: AlertRule) -> Result<(), String> {
        let mut rules = self.lock_alert_rules_write().map_err(|e| e.to_string())?;
        if rules.contains_key(&rule.id) {
            return Err(format!("Alert rule '{}' already exists", rule.id));
        }

        rules.insert(rule.id.clone(), rule);
        Ok(())
    }

    /// 删除告警规则
    pub fn remove_alert_rule(&self, rule_id: &str) -> bool {
        match self.lock_alert_rules_write() {
            Ok(mut rules) => rules.remove(rule_id).is_some(),
            Err(_) => false,
        }
    }

    /// 获取活跃告警
    pub fn get_active_alerts(&self) -> Vec<AlertEvent> {
        match self.lock_active_alerts_read() {
            Ok(alerts) => alerts.values().cloned().collect(),
            Err(_) => Vec::new(),
        }
    }

    /// 订阅告警事件
    pub fn subscribe_alerts(&self) -> broadcast::Receiver<AlertEvent> {
        self.alert_sender.subscribe()
    }

    /// 启动监控任务
    pub async fn start_monitoring(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 系统指标收集任务
        let monitor = Arc::new(self.clone());
        let handle = tokio::spawn(async move {
            monitor.system_metrics_collector().await;
        });
        if let Ok(mut handles) = self.lock_monitor_handles_write() {
            handles.push(handle);
        }

        // 告警检查任务
        let monitor = Arc::new(self.clone());
        let handle = tokio::spawn(async move {
            monitor.alert_checker().await;
        });
        if let Ok(mut handles) = self.lock_monitor_handles_write() {
            handles.push(handle);
        }

        // 数据清理任务
        let monitor = Arc::new(self.clone());
        let handle = tokio::spawn(async move {
            monitor.data_cleanup_task().await;
        });
        if let Ok(mut handles) = self.lock_monitor_handles_write() {
            handles.push(handle);
        }

        Ok(())
    }

    /// 停止监控
    pub async fn stop_monitoring(&self) {
        let handles = match self.lock_monitor_handles_write() {
            Ok(mut h) => std::mem::take(&mut *h),
            Err(_) => return, // Silently fail on lock error
        };
        for handle in handles {
            handle.abort();
        }
    }

    /// 系统指标收集器
    async fn system_metrics_collector(&self) {
        let mut interval =
            tokio::time::interval(Duration::from_secs(self.config.default_collection_interval));

        loop {
            interval.tick().await;

            // 收集CPU使用率
            if let Ok(cpu_usage) = self.collect_cpu_usage() {
                self.record_metric("system.cpu.usage", cpu_usage, HashMap::new());
            }

            // 收集内存使用率
            if let Ok(memory_usage) = self.collect_memory_usage() {
                self.record_metric("system.memory.usage", memory_usage, HashMap::new());
            }

            // 收集磁盘I/O
            if let Ok(disk_io) = self.collect_disk_io() {
                self.record_metric("system.disk.io", disk_io, HashMap::new());
            }

            // 收集网络I/O
            if let Ok(network_io) = self.collect_network_io() {
                self.record_metric("system.network.io", network_io, HashMap::new());
            }
        }
    }

    /// 告警检查器
    async fn alert_checker(&self) {
        let mut interval =
            tokio::time::interval(Duration::from_secs(self.config.alert_check_interval));

        loop {
            interval.tick().await;

            let rules = match self.lock_alert_rules_read() {
                Ok(r) => r.clone(),
                Err(_) => continue, // Skip on lock error
            };
            for rule in rules.values() {
                if !rule.enabled {
                    continue;
                }

                if let Some(stats) = self.get_metric_stats(&rule.metric_name, 300) {
                    // 5分钟窗口
                    if self.check_alert_condition(&rule.condition, stats.latest) {
                        self.trigger_alert(rule, stats.latest).await;
                    }
                }
            }
        }
    }

    /// 数据清理任务
    async fn data_cleanup_task(&self) {
        let mut interval = tokio::time::interval(Duration::from_secs(3600)); // 每小时清理一次

        loop {
            interval.tick().await;

            let configs = match self.metric_configs.read() {
                Ok(c) => c.clone(),
                Err(_) => continue, // Skip on lock error
            };
            let mut metrics = match self.metrics.write() {
                Ok(m) => m,
                Err(_) => continue, // Skip on lock error
            };

            for (name, config) in configs.iter() {
                if let Some(points) = metrics.get_mut(name) {
                    let cutoff_time = match Self::get_timestamp_ms() {
                        Ok(t) => t.saturating_sub(config.retention_hours * 3600 * 1000),
                        Err(_) => continue, // Skip on time error
                    };

                    // 移除过期数据点
                    while let Some(point) = points.front() {
                        if point.timestamp < cutoff_time {
                            points.pop_front();
                        } else {
                            break;
                        }
                    }
                }
            }
        }
    }

    /// 检查告警条件
    fn check_alert_condition(&self, condition: &str, value: f64) -> bool {
        // 简单的条件解析器，支持: value > 80, value < 10 等
        if let Some(gt_pos) = condition.find(">")
            && let Ok(threshold) = condition[gt_pos + 1..].trim().parse::<f64>()
        {
            return value > threshold;
        }
        if let Some(lt_pos) = condition.find("<")
            && let Ok(threshold) = condition[lt_pos + 1..].trim().parse::<f64>()
        {
            return value < threshold;
        }
        false
    }

    /// 触发告警
    async fn trigger_alert(&self, rule: &AlertRule, value: f64) {
        let now_ms = match Self::get_timestamp_ms() {
            Ok(t) => t,
            Err(_) => return, // Silently fail on time error
        };

        let event = AlertEvent {
            id: format!("alert_{}_{}", rule.id, now_ms),
            rule_id: rule.id.clone(),
            timestamp: now_ms,
            severity: rule.severity,
            message: format!("{}: {} (value: {:.2})", rule.name, rule.description, value),
            value,
            threshold: self.parse_threshold(&rule.condition),
        };

        // 存储活跃告警
        let mut alerts = match self.lock_active_alerts_write() {
            Ok(a) => a,
            Err(_) => return, // Silently fail on lock error
        };
        alerts.insert(event.id.clone(), event.clone());

        // 广播告警事件
        let _ = self.alert_sender.send(event);
    }

    /// 解析阈值
    fn parse_threshold(&self, condition: &str) -> f64 {
        if let Some(gt_pos) = condition.find(">")
            && let Ok(threshold) = condition[gt_pos + 1..].trim().parse::<f64>()
        {
            return threshold;
        }
        if let Some(lt_pos) = condition.find("<")
            && let Ok(threshold) = condition[lt_pos + 1..].trim().parse::<f64>()
        {
            return threshold;
        }
        0.0
    }

    /// 收集CPU使用率
    fn collect_cpu_usage(&self) -> Result<f64, Box<dyn std::error::Error + Send + Sync>> {
        // 平台特定的CPU使用率收集
        #[cfg(target_os = "linux")]
        {
            use std::fs;
            let stat = fs::read_to_string("/proc/stat")?;
            let lines: Vec<&str> = stat.lines().collect();
            if let Some(cpu_line) = lines.first() {
                if cpu_line.starts_with("cpu ") {
                    let fields: Vec<&str> = cpu_line.split_whitespace().collect();
                    if fields.len() >= 8 {
                        let user: u64 = fields[1].parse()?;
                        let nice: u64 = fields[2].parse()?;
                        let system: u64 = fields[3].parse()?;
                        let idle: u64 = fields[4].parse()?;
                        let total = user + nice + system + idle;
                        if total > 0 {
                            return Ok((total - idle) as f64 / total as f64 * 100.0);
                        }
                    }
                }
            }
        }

        // 默认实现
        Ok(0.0)
    }

    /// 收集内存使用率
    fn collect_memory_usage(&self) -> Result<f64, Box<dyn std::error::Error + Send + Sync>> {
        #[cfg(target_os = "linux")]
        {
            use std::fs;
            let meminfo = fs::read_to_string("/proc/meminfo")?;
            let lines: Vec<&str> = meminfo.lines().collect();

            let mut total = 0u64;
            let mut available = 0u64;

            for line in lines {
                if line.starts_with("MemTotal:") {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        total = kb_str.parse()?;
                    }
                } else if line.starts_with("MemAvailable:") {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        available = kb_str.parse()?;
                    }
                }
            }

            if total > 0 {
                return Ok((total - available) as f64 / total as f64 * 100.0);
            }
        }

        Ok(0.0)
    }

    /// 收集磁盘I/O
    fn collect_disk_io(&self) -> Result<f64, Box<dyn std::error::Error + Send + Sync>> {
        // 简化的磁盘I/O收集
        Ok(0.0)
    }

    /// 收集网络I/O
    fn collect_network_io(&self) -> Result<f64, Box<dyn std::error::Error + Send + Sync>> {
        // 简化的网络I/O收集
        Ok(0.0)
    }

    /// 生成性能报告
    pub fn generate_report(&self) -> PerformanceReport {
        let timestamp = Self::get_timestamp_ms().unwrap_or(0);

        let mut report = PerformanceReport {
            timestamp,
            system_metrics: HashMap::new(),
            vm_metrics: HashMap::new(),
            alerts: self.get_active_alerts(),
        };

        // 收集系统指标统计
        let system_metric_names = vec![
            "system.cpu.usage",
            "system.memory.usage",
            "system.disk.io",
            "system.network.io",
        ];

        for name in system_metric_names {
            if let Some(stats) = self.get_metric_stats(name, 3600) {
                // 1小时窗口
                report.system_metrics.insert(name.to_string(), stats);
            }
        }

        // 收集VM指标统计
        let vm_metric_names = vec![
            "vm.cpu.usage",
            "vm.memory.usage",
            "vm.jit.compile_time",
            "vm.tlb.hit_rate",
        ];

        for name in vm_metric_names {
            if let Some(stats) = self.get_metric_stats(name, 3600) {
                report.vm_metrics.insert(name.to_string(), stats);
            }
        }

        report
    }
}

/// 指标统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricStats {
    pub count: usize,
    pub min: f64,
    pub max: f64,
    pub avg: f64,
    pub std_dev: f64,
    pub latest: f64,
}

/// 性能报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    pub timestamp: u64,
    pub system_metrics: HashMap<String, MetricStats>,
    pub vm_metrics: HashMap<String, MetricStats>,
    pub alerts: Vec<AlertEvent>,
}

impl Clone for PerformanceMonitor {
    fn clone(&self) -> Self {
        Self {
            metrics: Arc::clone(&self.metrics),
            metric_configs: Arc::clone(&self.metric_configs),
            alert_rules: Arc::clone(&self.alert_rules),
            active_alerts: Arc::clone(&self.active_alerts),
            alert_sender: self.alert_sender.clone(),
            monitor_handles: Arc::clone(&self.monitor_handles),
            config: self.config.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_performance_monitor() {
        let config = MonitorConfig::default();
        let monitor = PerformanceMonitor::new(config);

        // 注册指标
        let metric_config = MetricConfig {
            name: "test.cpu".to_string(),
            metric_type: MetricType::CpuUsage,
            collection_interval: 10,
            retention_hours: 24,
            tags: HashMap::new(),
        };
        assert!(monitor.register_metric(metric_config).is_ok());

        // 记录指标值
        monitor.record_metric("test.cpu", 75.5, HashMap::new());

        // 获取指标数据
        let data = monitor.get_metric_data("test.cpu", None, None);
        assert_eq!(data.len(), 1);
        assert_eq!(data[0].value, 75.5);

        // 获取统计信息
        if let Some(stats) = monitor.get_metric_stats("test.cpu", 3600) {
            assert_eq!(stats.count, 1);
            assert_eq!(stats.min, 75.5);
            assert_eq!(stats.max, 75.5);
            assert_eq!(stats.avg, 75.5);
            assert_eq!(stats.latest, 75.5);
        } else {
            panic!("Expected stats");
        }
    }

    #[tokio::test]
    async fn test_alert_system() {
        let config = MonitorConfig::default();
        let monitor = PerformanceMonitor::new(config);

        // 注册指标
        let metric_config = MetricConfig {
            name: "test.alert".to_string(),
            metric_type: MetricType::CpuUsage,
            collection_interval: 10,
            retention_hours: 24,
            tags: HashMap::new(),
        };
        monitor.register_metric(metric_config).unwrap();

        // 添加告警规则
        let rule = AlertRule {
            id: "high_cpu".to_string(),
            name: "High CPU Usage".to_string(),
            metric_name: "test.alert".to_string(),
            condition: "value > 80".to_string(),
            severity: AlertSeverity::Warning,
            description: "CPU usage is too high".to_string(),
            enabled: true,
        };
        monitor.add_alert_rule(rule).unwrap();

        // 记录高值
        monitor.record_metric("test.alert", 85.0, HashMap::new());

        // 手动触发告警检查（在实际使用中由后台任务处理）
        // 这里简化测试
    }
}
