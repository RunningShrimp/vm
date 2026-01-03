//! 指标导出模块
//!
//! 提供Prometheus、JSON文件等格式的指标导出功能

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

use async_trait::async_trait;

use crate::{MetricsCollector, PrometheusConfig, SystemMetrics};

/// 指标导出器trait
#[async_trait]
pub trait MetricsExporter: Send + Sync {
    /// 开始导出
    async fn start_export(&self, metrics_collector: Arc<MetricsCollector>) -> Result<()>;

    /// 停止导出
    async fn stop(&self) -> Result<()>;
}

/// Prometheus导出器
pub struct PrometheusExporter {
    config: PrometheusConfig,
    // 注意：prometheus crate未在依赖中，暂时移除registry字段
    // registry: prometheus::Registry,
    // 这里可以添加其他Prometheus相关字段
}

impl PrometheusExporter {
    /// 创建新的Prometheus导出器
    pub fn new(_config: PrometheusConfig) -> Result<Self> {
        // 注意：prometheus crate未在依赖中，简化实现
        // 实际使用时需要添加prometheus依赖到Cargo.toml
        // let registry = prometheus::Registry::new();
        // Self::register_metrics(&registry)?;

        Ok(Self {
            config: _config,
            // registry,
        })
    }

    /// 注册Prometheus指标 (简化版本)
    #[allow(dead_code)]
    fn register_metrics(_registry: &()) -> Result<()> {
        // 简化版本，暂时不注册具体指标
        // 实际实现中应该在这里注册所有的Prometheus指标
        tracing::debug!("Prometheus metrics registration completed");
        Ok(())
    }

    /// 更新Prometheus指标 (简化版本)
    async fn update_metrics(&self, metrics: &SystemMetrics) -> Result<()> {
        // 这里是简化版本，实际实现需要存储metric句柄
        // 当前版本只是记录日志
        tracing::debug!(
            "Updating Prometheus metrics: JIT={:.0}, TLB={:.1}%, Memory={:.1}%",
            metrics.jit_metrics.execution_rate,
            metrics.tlb_metrics.hit_rate,
            metrics.memory_metrics.usage_rate
        );
        Ok(())
    }

    /// 获取Prometheus格式的指标 (简化版本)
    pub async fn gather_metrics(&self) -> Result<String> {
        // 简化版本，返回基本的指标格式
        let metrics = format!(
            "# HELP fvp_jit_execution_rate JIT execution rate per second\n# TYPE \
             fvp_jit_execution_rate gauge\nfvp_jit_execution_rate {}\n# HELP fvp_tlb_hit_rate TLB \
             hit rate percentage\n# TYPE fvp_tlb_hit_rate gauge\nfvp_tlb_hit_rate {}\n",
            0.0, // 实际应该从当前指标获取
            0.0
        );
        Ok(metrics)
    }
}

#[async_trait::async_trait]
impl MetricsExporter for PrometheusExporter {
    async fn start_export(&self, metrics_collector: Arc<MetricsCollector>) -> Result<()> {
        let exporter = self.clone();
        let mut interval = tokio::time::interval(Duration::from_secs(5));

        tokio::spawn(async move {
            loop {
                interval.tick().await;

                let metrics = metrics_collector.get_current_metrics().await;
                if let Err(e) = exporter.update_metrics(&metrics).await {
                    tracing::error!("Failed to update Prometheus metrics: {}", e);
                }
            }
        });

        tracing::info!(
            "Prometheus exporter started on {}:{}",
            self.config.bind_address,
            self.config.bind_port
        );
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        tracing::info!("Prometheus exporter stopped");
        Ok(())
    }
}

// Prometheus导出器不实现Clone，而是使用Arc包装
impl Clone for PrometheusExporter {
    fn clone(&self) -> Self {
        // 简化实现，重新创建一个新的实例
        PrometheusExporter::new(self.config.clone())
            .unwrap_or_else(|_| panic!("Failed to clone PrometheusExporter"))
    }
}

/// JSON文件导出器
pub struct JsonFileExporter {
    output_path: String,
    export_interval: Duration,
    is_running: Arc<RwLock<bool>>,
}

impl JsonFileExporter {
    /// 创建新的JSON文件导出器
    pub fn new(output_path: String, export_interval: Duration) -> Self {
        Self {
            output_path,
            export_interval,
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// 导出指标到JSON文件
    async fn export_to_file(&self, metrics: &SystemMetrics) -> Result<()> {
        let json_data = serde_json::to_string_pretty(metrics)?;

        // 确保输出目录存在
        if let Some(parent) = std::path::Path::new(&self.output_path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(&self.output_path, json_data).await?;

        tracing::debug!("Metrics exported to JSON file: {}", self.output_path);
        Ok(())
    }
}

#[async_trait::async_trait]
impl MetricsExporter for JsonFileExporter {
    async fn start_export(&self, metrics_collector: Arc<MetricsCollector>) -> Result<()> {
        *self.is_running.write().await = true;

        let exporter = self.clone();
        let is_running = Arc::clone(&self.is_running);
        let mut interval = tokio::time::interval(self.export_interval);

        tokio::spawn(async move {
            while *is_running.read().await {
                interval.tick().await;

                let metrics = metrics_collector.get_current_metrics().await;
                if let Err(e) = exporter.export_to_file(&metrics).await {
                    tracing::error!("Failed to export JSON metrics: {}", e);
                }
            }
        });

        tracing::info!("JSON file exporter started, output: {}", self.output_path);
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        *self.is_running.write().await = false;
        tracing::info!("JSON file exporter stopped");
        Ok(())
    }
}

impl Clone for JsonFileExporter {
    fn clone(&self) -> Self {
        Self {
            output_path: self.output_path.clone(),
            export_interval: self.export_interval,
            is_running: Arc::clone(&self.is_running),
        }
    }
}

/// 导出配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportConfig {
    /// Prometheus导出配置
    pub prometheus: Option<PrometheusConfig>,
    /// JSON文件导出配置
    pub json_file: Option<JsonExportConfig>,
}

/// JSON文件导出配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonExportConfig {
    /// 输出文件路径
    pub output_path: String,
    /// 导出间隔
    pub export_interval: Duration,
}
