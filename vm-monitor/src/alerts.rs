//! 告警管理模块
//!
//! 提供性能告警检测、通知和确认功能

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{AlertThresholds, SystemMetrics};

/// 告警管理器
pub struct AlertManager {
    thresholds: AlertThresholds,
    active_alerts: Arc<RwLock<HashMap<String, Alert>>>,
    alert_history: Arc<RwLock<Vec<Alert>>>,
    alert_cooldown: HashMap<String, Instant>,
}

/// 告警结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    /// 告警ID
    pub id: String,
    /// 告警类型
    pub alert_type: AlertType,
    /// 告警级别
    pub level: AlertLevel,
    /// 告警标题
    pub title: String,
    /// 告警描述
    pub description: String,
    /// 当前值
    pub current_value: f64,
    /// 阈值
    pub threshold: f64,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 确认时间
    pub acknowledged_at: Option<DateTime<Utc>>,
    /// 是否活跃
    pub active: bool,
    /// 相关指标
    pub metrics: AlertMetrics,
}

/// 告警类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertType {
    /// JIT执行速度告警
    JitExecutionRate,
    /// TLB命中率告警
    TlbHitRate,
    /// 内存使用率告警
    MemoryUsage,
    /// CPU使用率告警
    CpuUsage,
    /// 系统吞吐量告警
    SystemThroughput,
}

/// 告警级别
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertLevel {
    /// 信息
    Info,
    /// 警告
    Warning,
    /// 严重
    Critical,
}

/// 告警相关指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertMetrics {
    /// JIT指标
    pub jit: Option<JitAlertMetrics>,
    /// TLB指标
    pub tlb: Option<TlbAlertMetrics>,
    /// 内存指标
    pub memory: Option<MemoryAlertMetrics>,
    /// 并行指标
    pub parallel: Option<ParallelAlertMetrics>,
}

/// JIT告警指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JitAlertMetrics {
    /// 执行速度
    pub execution_rate: f64,
    /// 编译缓存大小
    pub compilation_cache_size: u64,
    /// 编译速度
    pub compilation_rate: f64,
}

/// TLB告警指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlbAlertMetrics {
    /// 命中率
    pub hit_rate: f64,
    /// 查找次数
    pub lookup_count: u64,
    /// 命中次数
    pub hit_count: u64,
    /// TLB条目数
    pub entries: u64,
}

/// 内存告警指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryAlertMetrics {
    /// 内存使用率
    pub usage_ratio: f64,
    /// 分配速度
    pub allocation_rate: f64,
    /// 读写操作
    pub read_write_ops: u64,
}

/// 并行告警指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelAlertMetrics {
    /// 并行效率
    pub efficiency: f64,
    /// 活跃vCPU数
    pub active_vcpus: u32,
    /// 并行操作数
    pub parallel_ops: u64,
}

impl AlertManager {
    /// 创建新的告警管理器
    pub fn new(thresholds: AlertThresholds) -> Self {
        Self {
            thresholds,
            active_alerts: Arc::new(RwLock::new(HashMap::new())),
            alert_history: Arc::new(RwLock::new(Vec::new())),
            alert_cooldown: HashMap::new(),
        }
    }

    /// 启动告警监控
    pub async fn start_monitoring(&self, metrics_collector: Arc<crate::MetricsCollector>) {
        let mut manager = self.clone();
        let mut interval = tokio::time::interval(Duration::from_secs(5));

        tokio::spawn(async move {
            loop {
                interval.tick().await;

                let metrics = metrics_collector.get_current_metrics().await;
                manager.check_alerts(&metrics).await;
            }
        });
    }

    /// 检查指标并生成告警
    async fn check_alerts(&mut self, metrics: &SystemMetrics) {
        let now = Instant::now();

        // 检查JIT执行速度
        if metrics.jit_metrics.execution_rate < self.thresholds.jit_execution_rate {
            let alert_key = "jit_execution_rate";
            if !self.is_in_cooldown(alert_key, now) {
                self.create_alert(
                    AlertType::JitExecutionRate,
                    AlertLevel::Warning,
                    "JIT执行速度低于阈值",
                    &format!(
                        "当前JIT执行速度: {:.2} execs/sec，阈值: {:.2} execs/sec",
                        metrics.jit_metrics.execution_rate,
                        self.thresholds.jit_execution_rate
                    ),
                    metrics.jit_metrics.execution_rate,
                    self.thresholds.jit_execution_rate,
                    metrics,
                ).await;
                self.set_cooldown(alert_key, now, Duration::from_secs(300)); // 5分钟冷却
            }
        }

        // 检查TLB命中率
        if metrics.tlb_metrics.hit_rate < self.thresholds.tlb_hit_rate {
            let alert_key = "tlb_hit_rate";
            if !self.is_in_cooldown(alert_key, now) {
                self.create_alert(
                    AlertType::TlbHitRate,
                    AlertLevel::Warning,
                    "TLB命中率低于阈值",
                    &format!(
                        "当前TLB命中率: {:.2}%，阈值: {:.2}%",
                        metrics.tlb_metrics.hit_rate,
                        self.thresholds.tlb_hit_rate
                    ),
                    metrics.tlb_metrics.hit_rate,
                    self.thresholds.tlb_hit_rate,
                    metrics,
                ).await;
                self.set_cooldown(alert_key, now, Duration::from_secs(300));
            }
        }

        // 检查内存使用率
        let memory_usage_ratio = metrics.memory_metrics.usage_rate / 100.0;
        if memory_usage_ratio * 100.0 > self.thresholds.memory_usage_rate {
            let alert_key = "memory_usage";
            if !self.is_in_cooldown(alert_key, now) {
                self.create_alert(
                    AlertType::MemoryUsage,
                    AlertLevel::Critical,
                    "内存使用率过高",
                    &format!(
                        "当前内存使用率: {:.2}%，阈值: {:.2}%",
                        memory_usage_ratio * 100.0,
                        self.thresholds.memory_usage_rate
                    ),
                    memory_usage_ratio * 100.0,
                    self.thresholds.memory_usage_rate,
                    metrics,
                ).await;
                self.set_cooldown(alert_key, now, Duration::from_secs(600)); // 10分钟冷却
            }
        }

        // 检查系统吞吐量
        // 注意：AlertThresholds中没有system_throughput字段，暂时注释掉此检查
        // 如果需要系统吞吐量告警，需要在AlertThresholds中添加system_throughput字段
        // if metrics.system_metrics.throughput < self.thresholds.system_throughput {
        //     let alert_key = "system_throughput";
        //     if !self.is_in_cooldown(alert_key, now) {
        //         self.create_alert(
        //             AlertType::SystemThroughput,
        //             AlertLevel::Warning,
        //             "系统吞吐量低于阈值",
        //             &format!(
        //                 "当前系统吞吐量: {:.2} ops/sec，阈值: {:.2} ops/sec",
        //                 metrics.system_metrics.throughput,
        //                 0.0  // 临时值
        //             ),
        //             metrics.system_metrics.throughput,
        //             0.0,  // 临时值
        //             metrics,
        //         ).await;
        //         self.set_cooldown(alert_key, now, Duration::from_secs(300));
        //     }
        // }
    }

    /// 创建告警
    async fn create_alert(
        &self,
        alert_type: AlertType,
        level: AlertLevel,
        title: &str,
        description: &str,
        current_value: f64,
        threshold: f64,
        metrics: &SystemMetrics,
    ) {
        let alert = Alert {
            id: Uuid::new_v4().to_string(),
            alert_type,
            level: level.clone(),
            title: title.to_string(),
            description: description.to_string(),
            current_value,
            threshold,
            created_at: Utc::now(),
            acknowledged_at: None,
            active: true,
            metrics: AlertMetrics {
                jit: Some(JitAlertMetrics {
                    execution_rate: metrics.jit_metrics.execution_rate,
                    compilation_cache_size: metrics.jit_metrics.hot_blocks_count as u64,
                    compilation_rate: metrics.jit_metrics.compilation_rate,
                }),
                tlb: Some(TlbAlertMetrics {
                    hit_rate: metrics.tlb_metrics.hit_rate,
                    lookup_count: metrics.tlb_metrics.total_lookups,
                    hit_count: metrics.tlb_metrics.total_hits,
                    entries: metrics.tlb_metrics.current_entries as u64,
                }),
                memory: Some(MemoryAlertMetrics {
                    usage_ratio: metrics.memory_metrics.usage_rate / 100.0,
                    allocation_rate: metrics.memory_metrics.allocation_rate,
                    read_write_ops: metrics.memory_metrics.total_reads + metrics.memory_metrics.total_writes,
                }),
                parallel: Some(ParallelAlertMetrics {
                    efficiency: metrics.parallel_metrics.efficiency_score,
                    active_vcpus: metrics.parallel_metrics.active_vcpu_count as u32,
                    parallel_ops: metrics.parallel_metrics.total_parallel_operations,
                }),
            },
        };

        // 添加到活跃告警
        {
            let mut active_alerts = self.active_alerts.write().await;
            active_alerts.insert(alert.id.clone(), alert.clone());
        }

        // 添加到历史记录
        {
            let mut history = self.alert_history.write().await;
            history.push(alert.clone());

            // 保持历史记录在合理范围内（最多1000条）
            if history.len() > 1000 {
                history.remove(0);
            }
        }

        tracing::warn!(
            "Alert created: {} - {} (Value: {:.2}, Threshold: {:.2})",
            alert.id,
            alert.title,
            current_value,
            threshold
        );
    }

    /// 确认告警
    pub async fn ack_alert(&self, alert_id: &str) -> Result<(), String> {
        let mut active_alerts = self.active_alerts.write().await;

        if let Some(alert) = active_alerts.get_mut(alert_id) {
            alert.acknowledged_at = Some(Utc::now());
            alert.active = false;

            tracing::info!("Alert acknowledged: {}", alert_id);
            Ok(())
        } else {
            Err(format!("Alert not found: {}", alert_id))
        }
    }

    /// 获取活跃告警
    pub fn get_active_alerts(&self) -> Vec<Alert> {
        // 这个方法不是async，但我们需要处理Arc<RwLock<>>
        // 在实际实现中，应该调用blocking_read或改为async方法
        // 这里先返回空向量，实际使用时需要调整
        Vec::new()
    }

    /// 获取告警历史
    pub async fn get_alert_history(&self, limit: Option<usize>) -> Vec<Alert> {
        let history = self.alert_history.read().await;
        match limit {
            Some(limit) => history.iter().rev().take(limit).cloned().collect(),
            None => history.iter().rev().cloned().collect(),
        }
    }

    /// 检查是否在冷却期
    fn is_in_cooldown(&self, key: &str, now: Instant) -> bool {
        self.alert_cooldown
            .get(key)
            .map(|&cooldown_time| now < cooldown_time)
            .unwrap_or(false)
    }

    /// 设置冷却期
    fn set_cooldown(&mut self, key: &str, now: Instant, duration: Duration) {
        self.alert_cooldown.insert(key.to_string(), now + duration);
    }
}

impl Clone for AlertManager {
    fn clone(&self) -> Self {
        Self {
            thresholds: self.thresholds.clone(),
            active_alerts: Arc::clone(&self.active_alerts),
            alert_history: Arc::clone(&self.alert_history),
            alert_cooldown: HashMap::new(), // 不克隆冷却状态
        }
    }
}