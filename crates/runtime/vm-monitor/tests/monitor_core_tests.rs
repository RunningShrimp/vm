//! VM Monitor Core Tests
//!
//! Comprehensive tests for vm-monitor core functionality including:
//! - MonitorError types
//! - PerformanceSnapshot structure
//! - AlertThresholds configuration
//! - DashboardConfig and ServerConfig
//! - MetricType enum
//! - MetricPoint structure
//! - MetricConfig structure
//! - AlertRule and AlertSeverity
//! - AlertEvent structure

use std::collections::HashMap;
use std::time::Duration;
use vm_monitor::{
    AlertEvent, AlertRule, AlertSeverity, AlertThresholds, DashboardConfig, MetricConfig,
    MetricPoint, MetricType, MonitorError, PerformanceSnapshot, ServerConfig,
};

// ============================================================================
// MonitorError Tests
// ============================================================================

#[test]
fn test_monitor_error_lock_error() {
    let err = MonitorError::LockError;
    let error_msg = format!("{}", err);
    assert!(error_msg.contains("RwLock poisoned"));
}

#[test]
fn test_monitor_error_metric_exists() {
    let err = MonitorError::MetricExists("cpu_usage".to_string());
    let error_msg = format!("{}", err);
    assert!(error_msg.contains("Metric 'cpu_usage' already exists"));
}

#[test]
fn test_monitor_error_alert_rule_exists() {
    let err = MonitorError::AlertRuleExists("high_cpu".to_string());
    let error_msg = format!("{}", err);
    assert!(error_msg.contains("Alert rule 'high_cpu' already exists"));
}

#[test]
fn test_monitor_error_time_error() {
    let err = MonitorError::TimeError;
    let error_msg = format!("{}", err);
    assert!(error_msg.contains("System time error"));
}

#[test]
fn test_monitor_error_no_data() {
    let err = MonitorError::NoData;
    let error_msg = format!("{}", err);
    assert!(error_msg.contains("No data available"));
}

#[test]
fn test_monitor_error_debug() {
    let err = MonitorError::LockError;
    let debug_str = format!("{:?}", err);
    assert!(debug_str.contains("LockError"));
}

// ============================================================================
// MetricType Tests
// ============================================================================

#[test]
fn test_metric_type_cpu_usage() {
    let mt = MetricType::CpuUsage;
    assert_eq!(mt, MetricType::CpuUsage);
}

#[test]
fn test_metric_type_memory_usage() {
    let mt = MetricType::MemoryUsage;
    assert_eq!(mt, MetricType::MemoryUsage);
}

#[test]
fn test_metric_type_disk_io() {
    let mt = MetricType::DiskIo;
    assert_eq!(mt, MetricType::DiskIo);
}

#[test]
fn test_metric_type_network_io() {
    let mt = MetricType::NetworkIo;
    assert_eq!(mt, MetricType::NetworkIo);
}

#[test]
fn test_metric_type_vm_cpu_usage() {
    let mt = MetricType::VmCpuUsage;
    assert_eq!(mt, MetricType::VmCpuUsage);
}

#[test]
fn test_metric_type_vm_memory_usage() {
    let mt = MetricType::VmMemoryUsage;
    assert_eq!(mt, MetricType::VmMemoryUsage);
}

#[test]
fn test_metric_type_jit_compile_time() {
    let mt = MetricType::JitCompileTime;
    assert_eq!(mt, MetricType::JitCompileTime);
}

#[test]
fn test_metric_type_jit_compile_count() {
    let mt = MetricType::JitCompileCount;
    assert_eq!(mt, MetricType::JitCompileCount);
}

#[test]
fn test_metric_type_tlb_hit_rate() {
    let mt = MetricType::TlbHitRate;
    assert_eq!(mt, MetricType::TlbHitRate);
}

#[test]
fn test_metric_type_device_io_latency() {
    let mt = MetricType::DeviceIoLatency;
    assert_eq!(mt, MetricType::DeviceIoLatency);
}

#[test]
fn test_metric_type_custom() {
    let mt = MetricType::Custom;
    assert_eq!(mt, MetricType::Custom);
}

#[test]
fn test_metric_type_equality() {
    assert_eq!(MetricType::CpuUsage, MetricType::CpuUsage);
    assert_ne!(MetricType::CpuUsage, MetricType::MemoryUsage);
}

#[test]
fn test_metric_type_clone() {
    let mt1 = MetricType::CpuUsage;
    let mt2 = mt1;
    assert_eq!(mt1, mt2);
}

#[test]
fn test_metric_type_copy() {
    let mt1 = MetricType::CpuUsage;
    let mt2 = mt1;
    assert_eq!(mt1, mt2);
}

// ============================================================================
// MetricPoint Tests
// ============================================================================

#[test]
fn test_metric_point_creation() {
    let mut tags = HashMap::new();
    tags.insert("host".to_string(), "server1".to_string());

    let point = MetricPoint {
        timestamp: 1609459200000,
        value: 75.5,
        tags,
    };

    assert_eq!(point.timestamp, 1609459200000);
    assert_eq!(point.value, 75.5);
    assert_eq!(point.tags.get("host"), Some(&"server1".to_string()));
}

#[test]
fn test_metric_point_empty_tags() {
    let point = MetricPoint {
        timestamp: 1609459200000,
        value: 50.0,
        tags: HashMap::new(),
    };

    assert!(point.tags.is_empty());
    assert_eq!(point.value, 50.0);
}

#[test]
fn test_metric_point_clone() {
    let mut tags = HashMap::new();
    tags.insert("region".to_string(), "us-west".to_string());

    let point1 = MetricPoint {
        timestamp: 1609459200000,
        value: 100.0,
        tags: tags.clone(),
    };

    let point2 = point1.clone();
    assert_eq!(point1.timestamp, point2.timestamp);
    assert_eq!(point1.value, point2.value);
    assert_eq!(point1.tags, point2.tags);
}

// ============================================================================
// MetricConfig Tests
// ============================================================================

#[test]
fn test_metric_config_creation() {
    let mut tags = HashMap::new();
    tags.insert("service".to_string(), "vm".to_string());

    let config = MetricConfig {
        name: "cpu_usage".to_string(),
        metric_type: MetricType::CpuUsage,
        collection_interval: 60,
        retention_hours: 24,
        tags,
    };

    assert_eq!(config.name, "cpu_usage");
    assert_eq!(config.collection_interval, 60);
    assert_eq!(config.retention_hours, 24);
}

#[test]
fn test_metric_config_clone() {
    let config1 = MetricConfig {
        name: "memory_usage".to_string(),
        metric_type: MetricType::MemoryUsage,
        collection_interval: 30,
        retention_hours: 48,
        tags: HashMap::new(),
    };

    let config2 = config1.clone();
    assert_eq!(config1.name, config2.name);
    assert_eq!(config1.metric_type, config2.metric_type);
}

// ============================================================================
// AlertSeverity Tests
// ============================================================================

#[test]
fn test_alert_severity_info() {
    let severity = AlertSeverity::Info;
    assert_eq!(severity, AlertSeverity::Info);
}

#[test]
fn test_alert_severity_warning() {
    let severity = AlertSeverity::Warning;
    assert_eq!(severity, AlertSeverity::Warning);
}

#[test]
fn test_alert_severity_error() {
    let severity = AlertSeverity::Error;
    assert_eq!(severity, AlertSeverity::Error);
}

#[test]
fn test_alert_severity_critical() {
    let severity = AlertSeverity::Critical;
    assert_eq!(severity, AlertSeverity::Critical);
}

#[test]
fn test_alert_severity_equality() {
    assert_eq!(AlertSeverity::Warning, AlertSeverity::Warning);
    assert_ne!(AlertSeverity::Info, AlertSeverity::Critical);
}

#[test]
fn test_alert_severity_clone() {
    let severity1 = AlertSeverity::Error;
    let severity2 = severity1;
    assert_eq!(severity1, severity2);
}

#[test]
fn test_alert_severity_copy() {
    let severity1 = AlertSeverity::Critical;
    let severity2 = severity1;
    assert_eq!(severity1, severity2);
}

// ============================================================================
// AlertRule Tests
// ============================================================================

#[test]
fn test_alert_rule_creation() {
    let rule = AlertRule {
        id: "rule1".to_string(),
        name: "High CPU Usage".to_string(),
        metric_name: "cpu_usage".to_string(),
        condition: "value > 80".to_string(),
        severity: AlertSeverity::Warning,
        description: "CPU usage exceeds 80%".to_string(),
        enabled: true,
    };

    assert_eq!(rule.id, "rule1");
    assert_eq!(rule.metric_name, "cpu_usage");
    assert_eq!(rule.enabled, true);
}

#[test]
fn test_alert_rule_disabled() {
    let rule = AlertRule {
        id: "rule2".to_string(),
        name: "Test Rule".to_string(),
        metric_name: "test_metric".to_string(),
        condition: "value > 100".to_string(),
        severity: AlertSeverity::Info,
        description: "Test".to_string(),
        enabled: false,
    };

    assert_eq!(rule.enabled, false);
}

#[test]
fn test_alert_rule_clone() {
    let rule1 = AlertRule {
        id: "rule1".to_string(),
        name: "Test".to_string(),
        metric_name: "metric".to_string(),
        condition: "value > 50".to_string(),
        severity: AlertSeverity::Error,
        description: "Test alert".to_string(),
        enabled: true,
    };

    let rule2 = rule1.clone();
    assert_eq!(rule1.id, rule2.id);
    assert_eq!(rule1.severity, rule2.severity);
}

// ============================================================================
// AlertEvent Tests
// ============================================================================

#[test]
fn test_alert_event_creation() {
    let event = AlertEvent {
        id: "event1".to_string(),
        rule_id: "rule1".to_string(),
        timestamp: 1609459200000,
        severity: AlertSeverity::Warning,
        message: "High CPU detected".to_string(),
        value: 85.5,
        threshold: 80.0,
    };

    assert_eq!(event.id, "event1");
    assert_eq!(event.value, 85.5);
    assert_eq!(event.threshold, 80.0);
}

#[test]
fn test_alert_event_clone() {
    let event1 = AlertEvent {
        id: "event1".to_string(),
        rule_id: "rule1".to_string(),
        timestamp: 1609459200000,
        severity: AlertSeverity::Critical,
        message: "Critical alert".to_string(),
        value: 99.9,
        threshold: 95.0,
    };

    let event2 = event1.clone();
    assert_eq!(event1.id, event2.id);
    assert_eq!(event1.severity, event2.severity);
}

// ============================================================================
// AlertThresholds Tests
// ============================================================================

#[test]
fn test_alert_thresholds_default() {
    let thresholds = AlertThresholds::default();
    assert_eq!(thresholds.jit_execution_rate, 1000.0);
    assert_eq!(thresholds.tlb_hit_rate, 0.9);
    assert_eq!(thresholds.memory_usage_rate, 0.8);
    assert_eq!(thresholds.gc_pause_time_ns, 1_000_000);
}

#[test]
fn test_alert_thresholds_custom() {
    let thresholds = AlertThresholds {
        jit_execution_rate: 2000.0,
        tlb_hit_rate: 0.95,
        memory_usage_rate: 0.85,
        gc_pause_time_ns: 500_000,
    };

    assert_eq!(thresholds.jit_execution_rate, 2000.0);
    assert_eq!(thresholds.tlb_hit_rate, 0.95);
    assert_eq!(thresholds.memory_usage_rate, 0.85);
    assert_eq!(thresholds.gc_pause_time_ns, 500_000);
}

#[test]
fn test_alert_thresholds_clone() {
    let thresholds1 = AlertThresholds::default();
    let thresholds2 = thresholds1.clone();
    assert_eq!(
        thresholds1.jit_execution_rate,
        thresholds2.jit_execution_rate
    );
}

// ============================================================================
// ServerConfig Tests
// ============================================================================

#[test]
fn test_server_config_default() {
    let config = ServerConfig::default();
    assert_eq!(config.bind_address, "0.0.0.0");
    assert_eq!(config.bind_port, 8080);
    assert_eq!(config.static_dir, None);
}

#[test]
fn test_server_config_custom() {
    let config = ServerConfig {
        bind_address: "127.0.0.1".to_string(),
        bind_port: 9090,
        static_dir: Some("/var/www".to_string()),
    };

    assert_eq!(config.bind_address, "127.0.0.1");
    assert_eq!(config.bind_port, 9090);
    assert_eq!(config.static_dir, Some("/var/www".to_string()));
}

#[test]
fn test_server_config_clone() {
    let config1 = ServerConfig::default();
    let config2 = config1.clone();
    assert_eq!(config1.bind_address, config2.bind_address);
    assert_eq!(config1.bind_port, config2.bind_port);
}

// ============================================================================
// DashboardConfig Tests
// ============================================================================

#[test]
fn test_dashboard_config_default() {
    let config = DashboardConfig::default();
    assert_eq!(config.server.bind_address, "0.0.0.0");
    assert_eq!(config.server.bind_port, 8080);
}

#[test]
fn test_dashboard_config_custom() {
    let server_config = ServerConfig {
        bind_address: "localhost".to_string(),
        bind_port: 3000,
        static_dir: None,
    };

    let config = DashboardConfig {
        server: server_config,
    };

    assert_eq!(config.server.bind_address, "localhost");
    assert_eq!(config.server.bind_port, 3000);
}

#[test]
fn test_dashboard_config_clone() {
    let config1 = DashboardConfig::default();
    let config2 = config1.clone();
    assert_eq!(config1.server.bind_port, config2.server.bind_port);
}

// ============================================================================
// PerformanceSnapshot Tests
// ============================================================================

#[test]
fn test_performance_snapshot_creation() {
    use chrono::Utc;
    use uuid::Uuid;
    use vm_monitor::SystemMetrics;

    let snapshot = PerformanceSnapshot {
        timestamp: Utc::now(),
        session_id: Uuid::new_v4(),
        uptime: Duration::from_secs(3600),
        metrics: SystemMetrics::default(),
        alerts: vec![],
    };

    assert_eq!(snapshot.uptime, Duration::from_secs(3600));
    assert!(snapshot.alerts.is_empty());
}

#[test]
fn test_performance_snapshot_clone() {
    use chrono::Utc;
    use uuid::Uuid;
    use vm_monitor::SystemMetrics;

    let snapshot1 = PerformanceSnapshot {
        timestamp: Utc::now(),
        session_id: Uuid::new_v4(),
        uptime: Duration::from_secs(7200),
        metrics: SystemMetrics::default(),
        alerts: vec![],
    };

    let snapshot2 = snapshot1.clone();
    assert_eq!(snapshot1.uptime, snapshot2.uptime);
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_all_metric_types() {
    let types = vec![
        MetricType::CpuUsage,
        MetricType::MemoryUsage,
        MetricType::DiskIo,
        MetricType::NetworkIo,
        MetricType::VmCpuUsage,
        MetricType::VmMemoryUsage,
        MetricType::VmDiskIo,
        MetricType::VmNetworkIo,
        MetricType::JitCompileTime,
        MetricType::JitCompileCount,
        MetricType::JitExecutionCount,
        MetricType::TlbHitRate,
        MetricType::TlbLookupCount,
        MetricType::TlbMissCount,
        MetricType::DeviceIoLatency,
        MetricType::DeviceThroughput,
        MetricType::Custom,
    ];

    for mt in types {
        let _ = mt;
        let _clone = mt;
    }
}

#[test]
fn test_all_alert_severities() {
    let severities = vec![
        AlertSeverity::Info,
        AlertSeverity::Warning,
        AlertSeverity::Error,
        AlertSeverity::Critical,
    ];

    for severity in severities {
        let _ = severity;
        let _clone = severity;
    }
}

#[test]
fn test_all_monitor_errors() {
    let errors = vec![
        MonitorError::LockError,
        MonitorError::MetricExists("test".to_string()),
        MonitorError::AlertRuleExists("rule".to_string()),
        MonitorError::TimeError,
        MonitorError::NoData,
    ];

    for err in errors {
        let _ = format!("{}", err);
        let _ = format!("{:?}", err);
    }
}

#[test]
fn test_metric_point_with_multiple_tags() {
    let mut tags = HashMap::new();
    tags.insert("host".to_string(), "server1".to_string());
    tags.insert("region".to_string(), "us-west".to_string());
    tags.insert("service".to_string(), "vm".to_string());

    let point = MetricPoint {
        timestamp: 1609459200000,
        value: 75.0,
        tags,
    };

    assert_eq!(point.tags.len(), 3);
    assert_eq!(point.tags.get("service"), Some(&"vm".to_string()));
}

#[test]
fn test_alert_rule_all_severities() {
    let severities = vec![
        AlertSeverity::Info,
        AlertSeverity::Warning,
        AlertSeverity::Error,
        AlertSeverity::Critical,
    ];

    for severity in severities {
        let rule = AlertRule {
            id: "rule1".to_string(),
            name: "Test".to_string(),
            metric_name: "metric".to_string(),
            condition: "value > 0".to_string(),
            severity,
            description: "Test".to_string(),
            enabled: true,
        };

        let _ = rule;
    }
}

#[test]
fn test_metric_config_all_types() {
    let types = vec![
        MetricType::CpuUsage,
        MetricType::MemoryUsage,
        MetricType::TlbHitRate,
        MetricType::JitCompileTime,
    ];

    for metric_type in types {
        let config = MetricConfig {
            name: "test_metric".to_string(),
            metric_type,
            collection_interval: 60,
            retention_hours: 24,
            tags: HashMap::new(),
        };

        let _ = config;
    }
}
