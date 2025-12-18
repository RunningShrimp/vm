//! Monitoring bounded context
//! 
//! This module defines the monitoring domain, including performance metrics,
//! health checks, and alerting for the JIT engine.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use crate::common::{Config, Stats, JITErrorBuilder, JITResult};

/// Unique identifier for monitoring contexts
pub type MonitoringId = u64;

/// Metric type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetricType {
    /// Counter metric
    Counter,
    /// Gauge metric
    Gauge,
    /// Histogram metric
    Histogram,
    /// Summary metric
    Summary,
}

impl Default for MetricType {
    fn default() -> Self {
        MetricType::Counter
    }
}

impl std::fmt::Display for MetricType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MetricType::Counter => write!(f, "Counter"),
            MetricType::Gauge => write!(f, "Gauge"),
            MetricType::Histogram => write!(f, "Histogram"),
            MetricType::Summary => write!(f, "Summary"),
        }
    }
}

/// Alert severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AlertSeverity {
    /// Debug level
    Debug,
    /// Info level
    Info,
    /// Warning level
    Warning,
    /// Error level
    Error,
    /// Critical level
    Critical,
}

impl Default for AlertSeverity {
    fn default() -> Self {
        AlertSeverity::Info
    }
}

impl std::fmt::Display for AlertSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlertSeverity::Debug => write!(f, "Debug"),
            AlertSeverity::Info => write!(f, "Info"),
            AlertSeverity::Warning => write!(f, "Warning"),
            AlertSeverity::Error => write!(f, "Error"),
            AlertSeverity::Critical => write!(f, "Critical"),
        }
    }
}

/// Health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HealthStatus {
    /// Healthy
    Healthy,
    /// Degraded
    Degraded,
    /// Unhealthy
    Unhealthy,
    /// Unknown
    Unknown,
}

impl Default for HealthStatus {
    fn default() -> Self {
        HealthStatus::Unknown
    }
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "Healthy"),
            HealthStatus::Degraded => write!(f, "Degraded"),
            HealthStatus::Unhealthy => write!(f, "Unhealthy"),
            HealthStatus::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Monitoring configuration
#[derive(Debug, Clone)]
pub struct MonitoringConfig {
    /// Enable metrics collection
    pub enable_metrics: bool,
    /// Enable health checks
    pub enable_health_checks: bool,
    /// Enable alerts
    pub enable_alerts: bool,
    /// Metrics collection interval
    pub metrics_interval: Duration,
    /// Health check interval
    pub health_check_interval: Duration,
    /// Alert retention period
    pub alert_retention: Duration,
    /// Maximum number of alerts to retain
    pub max_alerts: usize,
    /// Custom monitoring parameters
    pub custom_params: HashMap<String, String>,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enable_metrics: true,
            enable_health_checks: true,
            enable_alerts: true,
            metrics_interval: Duration::from_secs(10),
            health_check_interval: Duration::from_secs(30),
            alert_retention: Duration::from_secs(3600), // 1 hour
            max_alerts: 1000,
            custom_params: HashMap::new(),
        }
    }
}

impl Config for MonitoringConfig {
    fn validate(&self) -> Result<(), String> {
        if self.metrics_interval.is_zero() {
            return Err("Metrics interval cannot be zero".to_string());
        }
        
        if self.health_check_interval.is_zero() {
            return Err("Health check interval cannot be zero".to_string());
        }
        
        if self.alert_retention.is_zero() {
            return Err("Alert retention period cannot be zero".to_string());
        }
        
        if self.max_alerts == 0 {
            return Err("Maximum alerts cannot be zero".to_string());
        }
        
        Ok(())
    }
    
    fn summary(&self) -> String {
        format!(
            "MonitoringConfig(metrics={}, health_checks={}, alerts={}, metrics_interval={:?}, health_check_interval={:?})",
            self.enable_metrics,
            self.enable_health_checks,
            self.enable_alerts,
            self.metrics_interval,
            self.health_check_interval
        )
    }
    
    fn merge(&mut self, other: &Self) {
        // Merge enable flags
        self.enable_metrics = self.enable_metrics || other.enable_metrics;
        self.enable_health_checks = self.enable_health_checks || other.enable_health_checks;
        self.enable_alerts = self.enable_alerts || other.enable_alerts;
        
        // Use the shorter intervals
        if other.metrics_interval < self.metrics_interval {
            self.metrics_interval = other.metrics_interval;
        }
        
        if other.health_check_interval < self.health_check_interval {
            self.health_check_interval = other.health_check_interval;
        }
        
        // Use the shorter retention period
        if other.alert_retention < self.alert_retention {
            self.alert_retention = other.alert_retention;
        }
        
        // Use the larger max alerts
        if other.max_alerts > self.max_alerts {
            self.max_alerts = other.max_alerts;
        }
        
        // Merge custom parameters
        for (key, value) in &other.custom_params {
            self.custom_params.insert(key.clone(), value.clone());
        }
    }
}

/// Metric value
#[derive(Debug, Clone)]
pub enum MetricValue {
    /// Counter value
    Counter(u64),
    /// Gauge value
    Gauge(f64),
    /// Histogram values
    Histogram(Vec<f64>),
    /// Summary values
    Summary {
        /// Sum of values
        sum: f64,
        /// Count of values
        count: u64,
        /// Quantiles
        quantiles: Vec<(f64, f64)>, // (quantile, value)
    },
}

impl Default for MetricValue {
    fn default() -> Self {
        MetricValue::Counter(0)
    }
}

/// Metric
#[derive(Debug, Clone)]
pub struct Metric {
    /// Metric name
    pub name: String,
    /// Metric type
    pub metric_type: MetricType,
    /// Metric value
    pub value: MetricValue,
    /// Metric labels
    pub labels: HashMap<String, String>,
    /// Timestamp
    pub timestamp: Instant,
}

impl Metric {
    /// Create a new counter metric
    pub fn counter(name: String, value: u64) -> Self {
        Self {
            name,
            metric_type: MetricType::Counter,
            value: MetricValue::Counter(value),
            labels: HashMap::new(),
            timestamp: Instant::now(),
        }
    }
    
    /// Create a new gauge metric
    pub fn gauge(name: String, value: f64) -> Self {
        Self {
            name,
            metric_type: MetricType::Gauge,
            value: MetricValue::Gauge(value),
            labels: HashMap::new(),
            timestamp: Instant::now(),
        }
    }
    
    /// Create a new histogram metric
    pub fn histogram(name: String, values: Vec<f64>) -> Self {
        Self {
            name,
            metric_type: MetricType::Histogram,
            value: MetricValue::Histogram(values),
            labels: HashMap::new(),
            timestamp: Instant::now(),
        }
    }
    
    /// Create a new summary metric
    pub fn summary(name: String, sum: f64, count: u64, quantiles: Vec<(f64, f64)>) -> Self {
        Self {
            name,
            metric_type: MetricType::Summary,
            value: MetricValue::Summary { sum, count, quantiles },
            labels: HashMap::new(),
            timestamp: Instant::now(),
        }
    }
    
    /// Add a label to the metric
    pub fn with_label(mut self, key: String, value: String) -> Self {
        self.labels.insert(key, value);
        self
    }
    
    /// Add multiple labels to the metric
    pub fn with_labels(mut self, labels: HashMap<String, String>) -> Self {
        for (key, value) in labels {
            self.labels.insert(key, value);
        }
        self
    }
}

/// Alert
#[derive(Debug, Clone)]
pub struct Alert {
    /// Alert ID
    pub alert_id: MonitoringId,
    /// Alert name
    pub name: String,
    /// Alert severity
    pub severity: AlertSeverity,
    /// Alert message
    pub message: String,
    /// Alert timestamp
    pub timestamp: Instant,
    /// Alert labels
    pub labels: HashMap<String, String>,
    /// Alert resolved
    pub resolved: bool,
    /// Resolution timestamp
    pub resolved_at: Option<Instant>,
}

impl Alert {
    /// Create a new alert
    pub fn new(name: String, severity: AlertSeverity, message: String) -> Self {
        Self {
            alert_id: generate_monitoring_id(),
            name,
            severity,
            message,
            timestamp: Instant::now(),
            labels: HashMap::new(),
            resolved: false,
            resolved_at: None,
        }
    }
    
    /// Add a label to the alert
    pub fn with_label(mut self, key: String, value: String) -> Self {
        self.labels.insert(key, value);
        self
    }
    
    /// Add multiple labels to the alert
    pub fn with_labels(mut self, labels: HashMap<String, String>) -> Self {
        for (key, value) in labels {
            self.labels.insert(key, value);
        }
        self
    }
    
    /// Resolve the alert
    pub fn resolve(&mut self) {
        self.resolved = true;
        self.resolved_at = Some(Instant::now());
    }
}

/// Health check result
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    /// Check name
    pub name: String,
    /// Health status
    pub status: HealthStatus,
    /// Check message
    pub message: String,
    /// Check timestamp
    pub timestamp: Instant,
    /// Check duration
    pub duration: Duration,
    /// Additional details
    pub details: HashMap<String, String>,
}

impl HealthCheckResult {
    /// Create a new health check result
    pub fn new(name: String, status: HealthStatus, message: String, duration: Duration) -> Self {
        Self {
            name,
            status,
            message,
            timestamp: Instant::now(),
            duration,
            details: HashMap::new(),
        }
    }
    
    /// Add a detail to the health check result
    pub fn with_detail(mut self, key: String, value: String) -> Self {
        self.details.insert(key, value);
        self
    }
    
    /// Add multiple details to the health check result
    pub fn with_details(mut self, details: HashMap<String, String>) -> Self {
        for (key, value) in details {
            self.details.insert(key, value);
        }
        self
    }
}

/// Monitoring statistics
#[derive(Debug, Clone, Default)]
pub struct MonitoringStats {
    /// Total number of metrics collected
    pub total_metrics: u64,
    /// Total number of alerts generated
    pub total_alerts: u64,
    /// Total number of health checks performed
    pub total_health_checks: u64,
    /// Number of active alerts
    pub active_alerts: u64,
    /// Number of resolved alerts
    pub resolved_alerts: u64,
    /// Number of healthy checks
    pub healthy_checks: u64,
    /// Number of degraded checks
    pub degraded_checks: u64,
    /// Number of unhealthy checks
    pub unhealthy_checks: u64,
    /// Average metrics collection time in nanoseconds
    pub avg_metrics_collection_time_ns: u64,
    /// Average health check time in nanoseconds
    pub avg_health_check_time_ns: u64,
}

impl Stats for MonitoringStats {
    fn reset(&mut self) {
        *self = Self::default();
    }
    
    fn merge(&mut self, other: &Self) {
        self.total_metrics += other.total_metrics;
        self.total_alerts += other.total_alerts;
        self.total_health_checks += other.total_health_checks;
        self.active_alerts = other.active_alerts;
        self.resolved_alerts += other.resolved_alerts;
        self.healthy_checks += other.healthy_checks;
        self.degraded_checks += other.degraded_checks;
        self.unhealthy_checks += other.unhealthy_checks;
        
        // Recalculate average metrics collection time
        if self.total_metrics > 0 {
            self.avg_metrics_collection_time_ns = 
                (self.avg_metrics_collection_time_ns * (self.total_metrics - other.total_metrics) + 
                 other.avg_metrics_collection_time_ns * other.total_metrics) / self.total_metrics;
        }
        
        // Recalculate average health check time
        if self.total_health_checks > 0 {
            self.avg_health_check_time_ns = 
                (self.avg_health_check_time_ns * (self.total_health_checks - other.total_health_checks) + 
                 other.avg_health_check_time_ns * other.total_health_checks) / self.total_health_checks;
        }
    }
    
    fn summary(&self) -> String {
        format!(
            "MonitoringStats(metrics={}, alerts={}, health_checks={}, active_alerts={}, resolved_alerts={}, healthy={}, degraded={}, unhealthy={})",
            self.total_metrics,
            self.total_alerts,
            self.total_health_checks,
            self.active_alerts,
            self.resolved_alerts,
            self.healthy_checks,
            self.degraded_checks,
            self.unhealthy_checks
        )
    }
}

/// Monitoring context
#[derive(Debug, Clone)]
pub struct MonitoringContext {
    /// Monitoring ID
    pub monitoring_id: MonitoringId,
    /// Monitoring configuration
    pub config: MonitoringConfig,
    /// Collected metrics
    pub metrics: Vec<Metric>,
    /// Generated alerts
    pub alerts: Vec<Alert>,
    /// Health check results
    pub health_checks: Vec<HealthCheckResult>,
    /// Monitoring statistics
    pub stats: MonitoringStats,
}

impl MonitoringContext {
    /// Create a new monitoring context
    pub fn new(config: MonitoringConfig) -> Self {
        Self {
            monitoring_id: generate_monitoring_id(),
            config,
            metrics: Vec::new(),
            alerts: Vec::new(),
            health_checks: Vec::new(),
            stats: MonitoringStats::default(),
        }
    }
    
    /// Add a metric
    pub fn add_metric(&mut self, metric: Metric) {
        self.metrics.push(metric);
        self.stats.total_metrics += 1;
    }
    
    /// Add an alert
    pub fn add_alert(&mut self, alert: Alert) {
        self.alerts.push(alert);
        self.stats.total_alerts += 1;
        self.stats.active_alerts += 1;
    }
    
    /// Add a health check result
    pub fn add_health_check(&mut self, health_check: HealthCheckResult) {
        match health_check.status {
            HealthStatus::Healthy => self.stats.healthy_checks += 1,
            HealthStatus::Degraded => self.stats.degraded_checks += 1,
            HealthStatus::Unhealthy => self.stats.unhealthy_checks += 1,
            HealthStatus::Unknown => {}
        }
        
        self.health_checks.push(health_check);
        self.stats.total_health_checks += 1;
    }
    
    /// Resolve an alert
    pub fn resolve_alert(&mut self, alert_id: MonitoringId) -> bool {
        if let Some(alert) = self.alerts.iter_mut().find(|a| a.alert_id == alert_id) {
            if !alert.resolved {
                alert.resolve();
                self.stats.active_alerts -= 1;
                self.stats.resolved_alerts += 1;
                return true;
            }
        }
        false
    }
    
    /// Clean up old alerts based on retention policy
    pub fn cleanup_old_alerts(&mut self) {
        let now = Instant::now();
        let retention = self.config.alert_retention;
        
        self.alerts.retain(|alert| {
            if alert.resolved {
                if let Some(resolved_at) = alert.resolved_at {
                    now.duration_since(resolved_at) < retention
                } else {
                    false
                }
            } else {
                true
            }
        });
        
        // Ensure we don't exceed the maximum number of alerts
        if self.alerts.len() > self.config.max_alerts {
            // Sort by timestamp (oldest first) and keep the newest ones
            self.alerts.sort_by_key(|a| a.timestamp);
            self.alerts.drain(0..self.alerts.len() - self.config.max_alerts);
        }
    }
    
    /// Get active alerts
    pub fn get_active_alerts(&self) -> Vec<&Alert> {
        self.alerts.iter().filter(|a| !a.resolved).collect()
    }
    
    /// Get alerts by severity
    pub fn get_alerts_by_severity(&self, severity: AlertSeverity) -> Vec<&Alert> {
        self.alerts.iter().filter(|a| a.severity == severity).collect()
    }
    
    /// Get latest health status
    pub fn get_latest_health_status(&self) -> HealthStatus {
        if self.health_checks.is_empty() {
            return HealthStatus::Unknown;
        }
        
        // Get the most recent health check
        let latest_check = self.health_checks.iter()
            .max_by_key(|h| h.timestamp)
            .unwrap();
        
        latest_check.status
    }
}

/// Monitoring service
pub struct MonitoringService {
    /// Monitoring contexts
    contexts: HashMap<MonitoringId, Arc<RwLock<MonitoringContext>>>,
    /// Global monitoring statistics
    global_stats: MonitoringStats,
}

impl MonitoringService {
    /// Create a new monitoring service
    pub fn new() -> Self {
        Self {
            contexts: HashMap::new(),
            global_stats: MonitoringStats::default(),
        }
    }
    
    /// Create a new monitoring context
    pub fn create_context(&mut self, config: MonitoringConfig) -> MonitoringId {
        let context = MonitoringContext::new(config);
        let monitoring_id = context.monitoring_id;
        self.contexts.insert(monitoring_id, Arc::new(RwLock::new(context)));
        monitoring_id
    }
    
    /// Get a monitoring context
    pub fn get_context(&self, monitoring_id: MonitoringId) -> Option<Arc<RwLock<MonitoringContext>>> {
        self.contexts.get(&monitoring_id).cloned()
    }
    
    /// Remove a monitoring context
    pub fn remove_context(&mut self, monitoring_id: MonitoringId) -> bool {
        self.contexts.remove(&monitoring_id).is_some()
    }
    
    /// Record a metric
    pub fn record_metric(&self, monitoring_id: MonitoringId, metric: Metric) -> JITResult<()> {
        let context = self.get_context(monitoring_id)
            .ok_or_else(|| JITErrorBuilder::unknown(format!("Monitoring context {} not found", monitoring_id)))?;
        
        let mut ctx = context.write().map_err(|e| {
            JITErrorBuilder::unknown(format!("Failed to acquire write lock: {}", e))
        })?;
        
        if ctx.config.enable_metrics {
            ctx.add_metric(metric);
        }
        
        Ok(())
    }
    
    /// Generate an alert
    pub fn generate_alert(&self, monitoring_id: MonitoringId, alert: Alert) -> JITResult<()> {
        let context = self.get_context(monitoring_id)
            .ok_or_else(|| JITErrorBuilder::unknown(format!("Monitoring context {} not found", monitoring_id)))?;
        
        let mut ctx = context.write().map_err(|e| {
            JITErrorBuilder::unknown(format!("Failed to acquire write lock: {}", e))
        })?;
        
        if ctx.config.enable_alerts {
            ctx.add_alert(alert);
        }
        
        Ok(())
    }
    
    /// Record a health check result
    pub fn record_health_check(&self, monitoring_id: MonitoringId, health_check: HealthCheckResult) -> JITResult<()> {
        let context = self.get_context(monitoring_id)
            .ok_or_else(|| JITErrorBuilder::unknown(format!("Monitoring context {} not found", monitoring_id)))?;
        
        let mut ctx = context.write().map_err(|e| {
            JITErrorBuilder::unknown(format!("Failed to acquire write lock: {}", e))
        })?;
        
        if ctx.config.enable_health_checks {
            ctx.add_health_check(health_check);
        }
        
        Ok(())
    }
    
    /// Resolve an alert
    pub fn resolve_alert(&self, monitoring_id: MonitoringId, alert_id: MonitoringId) -> JITResult<bool> {
        let context = self.get_context(monitoring_id)
            .ok_or_else(|| JITErrorBuilder::unknown(format!("Monitoring context {} not found", monitoring_id)))?;
        
        let mut ctx = context.write().map_err(|e| {
            JITErrorBuilder::unknown(format!("Failed to acquire write lock: {}", e))
        })?;
        
        Ok(ctx.resolve_alert(alert_id))
    }
    
    /// Clean up old alerts
    pub fn cleanup_old_alerts(&self, monitoring_id: MonitoringId) -> JITResult<()> {
        let context = self.get_context(monitoring_id)
            .ok_or_else(|| JITErrorBuilder::unknown(format!("Monitoring context {} not found", monitoring_id)))?;
        
        let mut ctx = context.write().map_err(|e| {
            JITErrorBuilder::unknown(format!("Failed to acquire write lock: {}", e))
        })?;
        
        ctx.cleanup_old_alerts();
        
        Ok(())
    }
    
    /// Get monitoring statistics
    pub fn get_stats(&self, monitoring_id: MonitoringId) -> JITResult<MonitoringStats> {
        let context = self.get_context(monitoring_id)
            .ok_or_else(|| JITErrorBuilder::unknown(format!("Monitoring context {} not found", monitoring_id)))?;
        
        let ctx = context.read().map_err(|e| {
            JITErrorBuilder::unknown(format!("Failed to acquire read lock: {}", e))
        })?;
        
        Ok(ctx.stats.clone())
    }
    
    /// Get global monitoring statistics
    pub fn get_global_stats(&self) -> &MonitoringStats {
        &self.global_stats
    }
    
    /// Get active alerts
    pub fn get_active_alerts(&self, monitoring_id: MonitoringId) -> JITResult<Vec<Alert>> {
        let context = self.get_context(monitoring_id)
            .ok_or_else(|| JITErrorBuilder::unknown(format!("Monitoring context {} not found", monitoring_id)))?;
        
        let ctx = context.read().map_err(|e| {
            JITErrorBuilder::unknown(format!("Failed to acquire read lock: {}", e))
        })?;
        
        Ok(ctx.get_active_alerts().into_iter().cloned().collect())
    }
    
    /// Get alerts by severity
    pub fn get_alerts_by_severity(&self, monitoring_id: MonitoringId, severity: AlertSeverity) -> JITResult<Vec<Alert>> {
        let context = self.get_context(monitoring_id)
            .ok_or_else(|| JITErrorBuilder::unknown(format!("Monitoring context {} not found", monitoring_id)))?;
        
        let ctx = context.read().map_err(|e| {
            JITErrorBuilder::unknown(format!("Failed to acquire read lock: {}", e))
        })?;
        
        Ok(ctx.get_alerts_by_severity(severity).into_iter().cloned().collect())
    }
    
    /// Get latest health status
    pub fn get_latest_health_status(&self, monitoring_id: MonitoringId) -> JITResult<HealthStatus> {
        let context = self.get_context(monitoring_id)
            .ok_or_else(|| JITErrorBuilder::unknown(format!("Monitoring context {} not found", monitoring_id)))?;
        
        let ctx = context.read().map_err(|e| {
            JITErrorBuilder::unknown(format!("Failed to acquire read lock: {}", e))
        })?;
        
        Ok(ctx.get_latest_health_status())
    }
    
    /// Clear all monitoring contexts
    pub fn clear_all(&mut self) {
        self.contexts.clear();
        self.global_stats.reset();
    }
}

impl Default for MonitoringService {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate a unique monitoring ID
fn generate_monitoring_id() -> MonitoringId {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    COUNTER.fetch_add(1, Ordering::SeqCst)
}

/// Monitoring analysis tools
pub mod analysis {
    use super::*;
    
    /// Analyze monitoring data
    pub fn analyze_monitoring_data(context: &MonitoringContext) -> MonitoringAnalysisReport {
        let mut report = MonitoringAnalysisReport::default();
        
        // Analyze metrics
        for metric in &context.metrics {
            match metric.metric_type {
                MetricType::Counter => {
                    if let MetricValue::Counter(value) = metric.value {
                        report.total_counter_value += value;
                    }
                }
                MetricType::Gauge => {
                    if let MetricValue::Gauge(value) = metric.value {
                        report.gauge_values.push(value);
                    }
                }
                MetricType::Histogram => {
                    if let MetricValue::Histogram(values) = metric.value {
                        report.histogram_values.extend(values);
                    }
                }
                MetricType::Summary => {
                    if let MetricValue::Summary { sum, count, .. } = metric.value {
                        report.total_summary_sum += sum;
                        report.total_summary_count += count;
                    }
                }
            }
        }
        
        // Analyze alerts
        for alert in &context.alerts {
            match alert.severity {
                AlertSeverity::Debug => report.debug_alerts += 1,
                AlertSeverity::Info => report.info_alerts += 1,
                AlertSeverity::Warning => report.warning_alerts += 1,
                AlertSeverity::Error => report.error_alerts += 1,
                AlertSeverity::Critical => report.critical_alerts += 1,
            }
            
            if !alert.resolved {
                report.active_alerts += 1;
            }
        }
        
        // Analyze health checks
        for health_check in &context.health_checks {
            match health_check.status {
                HealthStatus::Healthy => report.healthy_checks += 1,
                HealthStatus::Degraded => report.degraded_checks += 1,
                HealthStatus::Unhealthy => report.unhealthy_checks += 1,
                HealthStatus::Unknown => report.unknown_checks += 1,
            }
        }
        
        // Calculate derived metrics
        if !report.gauge_values.is_empty() {
            report.avg_gauge_value = report.gauge_values.iter().sum::<f64>() / report.gauge_values.len() as f64;
            report.min_gauge_value = report.gauge_values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
            report.max_gauge_value = report.gauge_values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        }
        
        if !report.histogram_values.is_empty() {
            report.avg_histogram_value = report.histogram_values.iter().sum::<f64>() / report.histogram_values.len() as f64;
            report.min_histogram_value = report.histogram_values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
            report.max_histogram_value = report.histogram_values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        }
        
        let total_checks = report.healthy_checks + report.degraded_checks + report.unhealthy_checks + report.unknown_checks;
        if total_checks > 0 {
            report.health_score = (report.healthy_checks as f64 / total_checks as f64) * 100.0;
        }
        
        report
    }
    
    /// Monitoring analysis report
    #[derive(Debug, Clone, Default)]
    pub struct MonitoringAnalysisReport {
        /// Total counter value
        pub total_counter_value: u64,
        /// Gauge values
        pub gauge_values: Vec<f64>,
        /// Average gauge value
        pub avg_gauge_value: f64,
        /// Minimum gauge value
        pub min_gauge_value: f64,
        /// Maximum gauge value
        pub max_gauge_value: f64,
        /// Histogram values
        pub histogram_values: Vec<f64>,
        /// Average histogram value
        pub avg_histogram_value: f64,
        /// Minimum histogram value
        pub min_histogram_value: f64,
        /// Maximum histogram value
        pub max_histogram_value: f64,
        /// Total summary sum
        pub total_summary_sum: f64,
        /// Total summary count
        pub total_summary_count: u64,
        /// Debug alerts
        pub debug_alerts: u64,
        /// Info alerts
        pub info_alerts: u64,
        /// Warning alerts
        pub warning_alerts: u64,
        /// Error alerts
        pub error_alerts: u64,
        /// Critical alerts
        pub critical_alerts: u64,
        /// Active alerts
        pub active_alerts: u64,
        /// Healthy checks
        pub healthy_checks: u64,
        /// Degraded checks
        pub degraded_checks: u64,
        /// Unhealthy checks
        pub unhealthy_checks: u64,
        /// Unknown checks
        pub unknown_checks: u64,
        /// Health score (0-100)
        pub health_score: f64,
    }
    
    impl std::fmt::Display for MonitoringAnalysisReport {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "MonitoringAnalysis(counter={}, gauge_avg={:.2}, histogram_avg={:.2}, alerts={}, active_alerts={}, health_score={:.2}%)",
                self.total_counter_value,
                self.avg_gauge_value,
                self.avg_histogram_value,
                self.debug_alerts + self.info_alerts + self.warning_alerts + self.error_alerts + self.critical_alerts,
                self.active_alerts,
                self.health_score
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_monitoring_config_validation() {
        let mut config = MonitoringConfig::default();
        
        // Valid config
        assert!(config.validate().is_ok());
        
        // Invalid metrics interval
        config.metrics_interval = Duration::ZERO;
        assert!(config.validate().is_err());
        
        // Invalid health check interval
        config.metrics_interval = Duration::from_secs(10);
        config.health_check_interval = Duration::ZERO;
        assert!(config.validate().is_err());
        
        // Invalid alert retention
        config.health_check_interval = Duration::from_secs(30);
        config.alert_retention = Duration::ZERO;
        assert!(config.validate().is_err());
        
        // Invalid max alerts
        config.alert_retention = Duration::from_secs(3600);
        config.max_alerts = 0;
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_metric_creation() {
        let counter = Metric::counter("test_counter".to_string(), 42);
        assert_eq!(counter.name, "test_counter");
        assert_eq!(counter.metric_type, MetricType::Counter);
        assert_eq!(counter.value, MetricValue::Counter(42));
        
        let gauge = Metric::gauge("test_gauge".to_string(), 3.14);
        assert_eq!(gauge.name, "test_gauge");
        assert_eq!(gauge.metric_type, MetricType::Gauge);
        assert_eq!(gauge.value, MetricValue::Gauge(3.14));
        
        let histogram = Metric::histogram("test_histogram".to_string(), vec![1.0, 2.0, 3.0]);
        assert_eq!(histogram.name, "test_histogram");
        assert_eq!(histogram.metric_type, MetricType::Histogram);
        assert_eq!(histogram.value, MetricValue::Histogram(vec![1.0, 2.0, 3.0]));
        
        let summary = Metric::summary("test_summary".to_string(), 6.0, 3, vec![(0.5, 2.0)]);
        assert_eq!(summary.name, "test_summary");
        assert_eq!(summary.metric_type, MetricType::Summary);
        if let MetricValue::Summary { sum, count, quantiles } = summary.value {
            assert_eq!(sum, 6.0);
            assert_eq!(count, 3);
            assert_eq!(quantiles, vec![(0.5, 2.0)]);
        } else {
            panic!("Expected Summary value");
        }
    }
    
    #[test]
    fn test_alert_creation() {
        let mut alert = Alert::new("test_alert".to_string(), AlertSeverity::Warning, "Test message".to_string());
        assert_eq!(alert.name, "test_alert");
        assert_eq!(alert.severity, AlertSeverity::Warning);
        assert_eq!(alert.message, "Test message");
        assert!(!alert.resolved);
        assert!(alert.resolved_at.is_none());
        
        alert.resolve();
        assert!(alert.resolved);
        assert!(alert.resolved_at.is_some());
    }
    
    #[test]
    fn test_health_check_result() {
        let result = HealthCheckResult::new(
            "test_check".to_string(),
            HealthStatus::Healthy,
            "All good".to_string(),
            Duration::from_millis(100)
        );
        assert_eq!(result.name, "test_check");
        assert_eq!(result.status, HealthStatus::Healthy);
        assert_eq!(result.message, "All good");
        assert_eq!(result.duration, Duration::from_millis(100));
    }
    
    #[test]
    fn test_monitoring_context() {
        let config = MonitoringConfig::default();
        let mut context = MonitoringContext::new(config);
        
        assert_eq!(context.metrics.len(), 0);
        assert_eq!(context.alerts.len(), 0);
        assert_eq!(context.health_checks.len(), 0);
        assert_eq!(context.stats.total_metrics, 0);
        assert_eq!(context.stats.total_alerts, 0);
        assert_eq!(context.stats.total_health_checks, 0);
        
        // Add a metric
        let metric = Metric::counter("test_counter".to_string(), 42);
        context.add_metric(metric);
        assert_eq!(context.metrics.len(), 1);
        assert_eq!(context.stats.total_metrics, 1);
        
        // Add an alert
        let alert = Alert::new("test_alert".to_string(), AlertSeverity::Warning, "Test message".to_string());
        context.add_alert(alert);
        assert_eq!(context.alerts.len(), 1);
        assert_eq!(context.stats.total_alerts, 1);
        assert_eq!(context.stats.active_alerts, 1);
        
        // Add a health check
        let health_check = HealthCheckResult::new(
            "test_check".to_string(),
            HealthStatus::Healthy,
            "All good".to_string(),
            Duration::from_millis(100)
        );
        context.add_health_check(health_check);
        assert_eq!(context.health_checks.len(), 1);
        assert_eq!(context.stats.total_health_checks, 1);
        assert_eq!(context.stats.healthy_checks, 1);
        
        // Resolve an alert
        let alert_id = context.alerts[0].alert_id;
        let resolved = context.resolve_alert(alert_id);
        assert!(resolved);
        assert_eq!(context.stats.active_alerts, 0);
        assert_eq!(context.stats.resolved_alerts, 1);
    }
    
    #[test]
    fn test_monitoring_service() {
        let mut service = MonitoringService::new();
        let config = MonitoringConfig::default();
        
        // Create a monitoring context
        let monitoring_id = service.create_context(config);
        assert!(service.get_context(monitoring_id).is_some());
        
        // Record a metric
        let metric = Metric::counter("test_counter".to_string(), 42);
        service.record_metric(monitoring_id, metric).unwrap();
        
        // Generate an alert
        let alert = Alert::new("test_alert".to_string(), AlertSeverity::Warning, "Test message".to_string());
        service.generate_alert(monitoring_id, alert).unwrap();
        
        // Record a health check
        let health_check = HealthCheckResult::new(
            "test_check".to_string(),
            HealthStatus::Healthy,
            "All good".to_string(),
            Duration::from_millis(100)
        );
        service.record_health_check(monitoring_id, health_check).unwrap();
        
        // Get stats
        let stats = service.get_stats(monitoring_id).unwrap();
        assert_eq!(stats.total_metrics, 1);
        assert_eq!(stats.total_alerts, 1);
        assert_eq!(stats.total_health_checks, 1);
        
        // Get active alerts
        let active_alerts = service.get_active_alerts(monitoring_id).unwrap();
        assert_eq!(active_alerts.len(), 1);
        
        // Get latest health status
        let health_status = service.get_latest_health_status(monitoring_id).unwrap();
        assert_eq!(health_status, HealthStatus::Healthy);
        
        // Remove monitoring context
        assert!(service.remove_context(monitoring_id));
        assert!(service.get_context(monitoring_id).is_none());
    }
    
    #[test]
    fn test_monitoring_analysis() {
        let config = MonitoringConfig::default();
        let mut context = MonitoringContext::new(config);
        
        // Add metrics
        context.add_metric(Metric::counter("test_counter".to_string(), 42));
        context.add_metric(Metric::gauge("test_gauge".to_string(), 3.14));
        context.add_metric(Metric::histogram("test_histogram".to_string(), vec![1.0, 2.0, 3.0]));
        context.add_metric(Metric::summary("test_summary".to_string(), 6.0, 3, vec![(0.5, 2.0)]));
        
        // Add alerts
        context.add_alert(Alert::new("test_alert1".to_string(), AlertSeverity::Warning, "Warning message".to_string()));
        context.add_alert(Alert::new("test_alert2".to_string(), AlertSeverity::Error, "Error message".to_string()));
        
        // Add health checks
        context.add_health_check(HealthCheckResult::new(
            "test_check1".to_string(),
            HealthStatus::Healthy,
            "All good".to_string(),
            Duration::from_millis(100)
        ));
        context.add_health_check(HealthCheckResult::new(
            "test_check2".to_string(),
            HealthStatus::Degraded,
            "Some issues".to_string(),
            Duration::from_millis(200)
        ));
        
        // Analyze monitoring data
        let report = analysis::analyze_monitoring_data(&context);
        
        assert_eq!(report.total_counter_value, 42);
        assert_eq!(report.gauge_values.len(), 1);
        assert_eq!(report.avg_gauge_value, 3.14);
        assert_eq!(report.histogram_values.len(), 3);
        assert_eq!(report.avg_histogram_value, 2.0);
        assert_eq!(report.total_summary_sum, 6.0);
        assert_eq!(report.total_summary_count, 3);
        assert_eq!(report.warning_alerts, 1);
        assert_eq!(report.error_alerts, 1);
        assert_eq!(report.active_alerts, 2);
        assert_eq!(report.healthy_checks, 1);
        assert_eq!(report.degraded_checks, 1);
        assert_eq!(report.health_score, 50.0);
    }
}