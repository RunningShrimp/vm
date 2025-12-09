//! Web仪表板服务
//!
//! 提供基于Web的实时性能监控界面

use std::sync::Arc;
use std::time::Duration;
use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Json, Response},
    routing::{get, post, Router},
};
use axum::extract::Query;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tower::ServiceBuilder;
use tower::Layer;

use crate::{
    MetricsCollector, PerformanceSnapshot,
    alerts::AlertManager, Alert,
    export::PrometheusExporter, DashboardConfig,
    performance_analyzer::PerformanceAnalyzer,
};

/// Web仪表板服务
pub struct DashboardServer {
    config: DashboardConfig,
    metrics_collector: Arc<MetricsCollector>,
    alert_manager: Arc<AlertManager>,
    prometheus_exporter: Option<Arc<PrometheusExporter>>,
    /// 服务器运行标志
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl DashboardServer {
    /// 创建新的仪表板服务器
    pub fn new(
        config: DashboardConfig,
        metrics_collector: Arc<MetricsCollector>,
        alert_manager: Arc<AlertManager>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let prometheus_exporter = config
            .server
            .static_dir
            .as_ref()
            .and_then(|_| {
                PrometheusExporter::new(crate::PrometheusConfig {
                    bind_address: config.server.bind_address.clone(),
                    bind_port: 9090,
                    metrics_prefix: "fvp_vm".to_string(),
                }).ok().map(Arc::new)
            });

        Ok(Self {
            config,
            metrics_collector,
            alert_manager,
            prometheus_exporter,
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        })
    }

    /// 启动Web服务器
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.running.store(true, std::sync::atomic::Ordering::Release);
        let app = self.create_router().await?;

        let addr = format!("{}:{}", self.config.server.bind_address, self.config.server.bind_port);
        let listener = TcpListener::bind(&addr).await?;

        tracing::info!("Dashboard server listening on: {}", addr);
        tracing::info!("Access dashboard at: http://{}/", addr);

        axum::serve(listener, app).await?;
        Ok(())
    }

    /// 停止Web服务器
    pub async fn stop(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        tracing::info!("Stopping dashboard server");
        self.running.store(false, std::sync::atomic::Ordering::Release);
        Ok(())
    }

    /// 创建路由
    async fn create_router(&self) -> Result<Router, Box<dyn std::error::Error + Send + Sync>> {
        let shared_state = (self.metrics_collector.clone(), self.alert_manager.clone());
        let performance_analyzer = Arc::new(PerformanceAnalyzer::new(self.metrics_collector.clone()));

        let app = Router::new()
            // CORS支持
            .layer(
                ServiceBuilder::new()
                    .layer(CorsLayer::permissive())
                    .into_inner()
            )
            // 路由定义
            .route("/", get(index_handler))
            .route("/api/metrics", get(metrics_handler))
            .route("/api/metrics/snapshot", get(metrics_snapshot_handler))
            .route("/api/metrics/history", get(metrics_history_handler))
            .route("/api/performance/analyze", get(performance_analyze_handler))
            .route("/api/performance/report/json", get(performance_report_json_handler))
            .route("/api/performance/report/text", get(performance_report_text_handler))
            .route("/api/alerts", get(alerts_handler))
            .route("/api/alerts/:id", post(ack_alert_handler))
            .route("/health", get(health_handler))
            .fallback(not_found_handler)
            .with_state((shared_state.0, shared_state.1, performance_analyzer));

        Ok(app)
    }

    /// WebSocket循环
    async fn websocket_loop(&self, metrics_collector: Arc<MetricsCollector>) {
        // 实现WebSocket消息推送逻辑
        while self.is_running() {
            // 定期推送指标更新
            tokio::time::sleep(Duration::from_secs(1)).await;

            let metrics = metrics_collector.get_current_metrics().await;

            // 推送指标到WebSocket客户端
            if let Err(e) = self.push_websocket_metrics(&metrics).await {
                tracing::error!("Failed to push metrics via WebSocket: {}", e);
                break;
            }
        }
    }

    /// 推送指标到WebSocket
    async fn push_websocket_metrics(&self, metrics: &crate::SystemMetrics) -> Result<(), Box<dyn std::error::Error>> {
        // 实现WebSocket推送逻辑
        // 这里需要实际的WebSocket连接管理
        tracing::debug!("Pushing metrics to WebSocket clients");
        Ok(())
    }

    /// 检查运行状态
    fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Acquire)
    }
}

/// 首页处理器
async fn index_handler() -> impl IntoResponse {
    let html = include_str!("../static/index.html");
    Html(html).into_response()
}

/// 获取当前指标
async fn metrics_handler(
    State((metrics_collector, alert_manager, _)): State<(Arc<MetricsCollector>, Arc<AlertManager>, Arc<PerformanceAnalyzer>)>,
) -> impl IntoResponse {
    let metrics = metrics_collector.get_current_metrics().await;
    let alerts = alert_manager.get_active_alerts();

    let response = MetricsResponse {
        timestamp: metrics.timestamp,
        metrics,
        alerts,
    };

    Json(response).into_response()
}

/// 获取性能快照
async fn metrics_snapshot_handler(
    State((metrics_collector, _, _)): State<(Arc<MetricsCollector>, Arc<AlertManager>, Arc<PerformanceAnalyzer>)>,
) -> impl IntoResponse {
    let snapshot = PerformanceSnapshot {
        timestamp: chrono::Utc::now(),
        session_id: uuid::Uuid::new_v4(),
        uptime: metrics_collector.get_uptime().await,
        metrics: metrics_collector.get_current_metrics().await,
        alerts: Vec::new(),
    };

    Json(snapshot).into_response()
}

/// 获取历史指标
async fn metrics_history_handler(
    State((metrics_collector, _, _)): State<(Arc<MetricsCollector>, Arc<AlertManager>, Arc<PerformanceAnalyzer>)>,
    params: Query<HistoryParams>,
) -> impl IntoResponse {
    let history = metrics_collector.get_historical_metrics().await;

    let start_index = params.start.unwrap_or(0);
    let count = params.count.unwrap_or(100).min(history.len() - start_index);

    let slice = if start_index < history.len() {
        &history[start_index..start_index + count]
    } else {
        &[]
    };

    Json(HistoryResponse {
        data: slice.to_vec(),
        total_count: history.len(),
    }).into_response()
}

/// 执行性能分析
async fn performance_analyze_handler(
    State((_, _, analyzer)): State<(Arc<MetricsCollector>, Arc<AlertManager>, Arc<PerformanceAnalyzer>)>,
) -> impl IntoResponse {
    let analysis = analyzer.analyze().await;
    Json(analysis).into_response()
}

/// 获取性能报告（JSON格式）
async fn performance_report_json_handler(
    State((_, _, analyzer)): State<(Arc<MetricsCollector>, Arc<AlertManager>, Arc<PerformanceAnalyzer>)>,
) -> impl IntoResponse {
    match analyzer.generate_report_json().await {
        Ok(json) => {
            let body: axum::body::Body = json.into();
            Response::builder()
                .header(header::CONTENT_TYPE, "application/json")
                .body(body)
                .unwrap()
                .into_response()
        }
        Err(e) => {
            let error = ErrorResponse {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                message: format!("Failed to generate report: {}", e),
                timestamp: chrono::Utc::now(),
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(error)).into_response()
        }
    }
}

/// 获取性能报告（文本格式）
async fn performance_report_text_handler(
    State((_, _, analyzer)): State<(Arc<MetricsCollector>, Arc<AlertManager>, Arc<PerformanceAnalyzer>)>,
) -> impl IntoResponse {
    let report = analyzer.generate_report_text().await;
    let body: axum::body::Body = report.into();
    Response::builder()
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(body)
        .unwrap()
        .into_response()
}

/// 获取告警列表
async fn alerts_handler(
    State((_, alert_manager, _)): State<(Arc<MetricsCollector>, Arc<AlertManager>, Arc<PerformanceAnalyzer>)>,
) -> impl IntoResponse {
    let alerts = alert_manager.get_active_alerts();
    Json(alerts).into_response()
}

/// 确认告警
async fn ack_alert_handler(
    State((_, alert_manager, _)): State<(Arc<MetricsCollector>, Arc<AlertManager>, Arc<PerformanceAnalyzer>)>,
    Path(alert_id): Path<String>,
) -> impl IntoResponse {
    match alert_manager.ack_alert(&alert_id).await {
        Ok(()) => {
            Json(AckResponse { success: true }).into_response()
        }
        Err(_) => {
            let response = AckResponse { success: false };
            (StatusCode::NOT_FOUND, Json(response)).into_response()
        }
    }
}

/// 健康检查
async fn health_handler(
    State((metrics_collector, _, _)): State<(Arc<MetricsCollector>, Arc<AlertManager>, Arc<PerformanceAnalyzer>)>,
) -> impl IntoResponse {
    let uptime = metrics_collector.get_uptime().await;
    let health = HealthStatus {
        status: "healthy".to_string(),
        timestamp: chrono::Utc::now(),
        uptime,
        version: env!("CARGO_PKG_VERSION", "0.1.0").to_string(),
    };

    Json(health).into_response()
}

/// 404错误处理器
async fn not_found_handler() -> impl IntoResponse {
    let error = ErrorResponse {
        code: StatusCode::NOT_FOUND.as_u16(),
        message: "Resource not found".to_string(),
        timestamp: chrono::Utc::now(),
    };

    (StatusCode::NOT_FOUND, Json(error)).into_response()
}

/// 指标响应
#[derive(Debug, Serialize)]
struct MetricsResponse {
    timestamp: chrono::DateTime<chrono::Utc>,
    metrics: crate::SystemMetrics,
    alerts: Vec<Alert>,
}

/// 历史数据查询参数
#[derive(Debug, Deserialize)]
struct HistoryParams {
    start: Option<usize>,
    count: Option<usize>,
}

/// 历史数据响应
#[derive(Debug, Serialize)]
struct HistoryResponse {
    data: Vec<crate::SystemMetrics>,
    total_count: usize,
}

/// 告警确认响应
#[derive(Debug, Serialize)]
struct AckResponse {
    success: bool,
}

/// 健康状态响应
#[derive(Debug, Serialize)]
struct HealthStatus {
    status: String,
    timestamp: chrono::DateTime<chrono::Utc>,
    uptime: std::time::Duration,
    version: String,
}

/// 错误响应
#[derive(Debug, Serialize)]
struct ErrorResponse {
    code: u16,
    message: String,
    timestamp: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dashboard_server_creation() {
        let config = DashboardConfig::default();
        let metrics_collector = Arc::new(MetricsCollector::new(crate::MonitorConfig::default()).unwrap());
        let alert_manager = Arc::new(crate::AlertManager::new(crate::AlertThresholds::default()));

        let server = DashboardServer::new(config, metrics_collector, alert_manager);
        assert!(server.is_ok());
    }
}