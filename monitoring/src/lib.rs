// 性能监控与调优系统
// 支持Prometheus指标暴露和实时性能监控

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

// Prometheus客户端
use prometheus::{
    Counter, Histogram, IntCounter, IntGauge, Opts, Registry,
};
use hyper::{Body, Request, Response, Server};
use hyper::service::{make_service_fn, service_fn};
use futures::executor::block_on;

/// 性能指标类型
pub enum MetricType {
    /// 计数器 (递增)
    Counter,
    /// 仪表盘 (可增减)
    Gauge,
    /// 直方图 (分布统计)
    Histogram,
}

/// 性能指标配置
pub struct MetricConfig {
    pub name: String,
    pub help: String,
    pub metric_type: MetricType,
    pub labels: Vec<(String, String)>,
}

/// 性能监控器
pub struct PerformanceMonitor {
    // Prometheus注册表
    registry: Arc<Registry>,
    // 指标映射
    counters: Arc<RwLock<HashMap<String, IntCounter>>>,
    gauges: Arc<RwLock<HashMap<String, IntGauge>>>,
    histograms: Arc<RwLock<HashMap<String, Histogram>>>,
    // 服务器运行状态
    server_running: Arc<AtomicU64>,
    // 服务器线程
    server_thread: Option<thread::JoinHandle<()>>,
}

impl PerformanceMonitor {
    /// 创建新的性能监控器
    pub fn new() -> Self {
        Self {
            registry: Arc::new(Registry::new()),
            counters: Arc::new(RwLock::new(HashMap::new())),
            gauges: Arc::new(RwLock::new(HashMap::new())),
            histograms: Arc::new(RwLock::new(HashMap::new())),
            server_running: Arc::new(AtomicU64::new(0)),
            server_thread: None,
        }
    }

    /// 注册指标
    pub fn register_metric(&self, config: MetricConfig) {
        match config.metric_type {
            MetricType::Counter => {
                let counter = IntCounter::with_opts(Opts::new(config.name.clone(), config.help))
                    .expect("Failed to create counter");
                
                // 注册到Prometheus注册表
                self.registry
                    .register(Box::new(counter.clone()))
                    .expect("Failed to register counter");
                
                // 保存到映射
                let mut counters = self.counters.write();
                counters.insert(config.name, counter);
            },
            MetricType::Gauge => {
                let gauge = IntGauge::with_opts(Opts::new(config.name.clone(), config.help))
                    .expect("Failed to create gauge");
                
                self.registry
                    .register(Box::new(gauge.clone()))
                    .expect("Failed to register gauge");
                
                let mut gauges = self.gauges.write();
                gauges.insert(config.name, gauge);
            },
            MetricType::Histogram => {
                // 默认直方图分位
                let histogram = Histogram::with_opts(Opts::new(config.name.clone(), config.help))
                    .expect("Failed to create histogram");
                
                self.registry
                    .register(Box::new(histogram.clone()))
                    .expect("Failed to register histogram");
                
                let mut histograms = self.histograms.write();
                histograms.insert(config.name, histogram);
            },
        }
    }

    /// 递增计数器
    pub fn increment_counter(&self, name: &str, value: u64) {
        if let Some(counter) = self.counters.read().get(name) {
            counter.inc_by(value);
        }
    }

    /// 设置仪表盘值
    pub fn set_gauge(&self, name: &str, value: i64) {
        if let Some(gauge) = self.gauges.read().get(name) {
            gauge.set(value);
        }
    }

    /// 观察直方图
    pub fn observe_histogram(&self, name: &str, value: f64) {
        if let Some(histogram) = self.histograms.read().get(name) {
            histogram.observe(value);
        }
    }

    /// 启动Prometheus指标服务器
    pub fn start_server(&mut self, address: &str) {
        if self.server_running.load(Ordering::Acquire) == 1 {
            return; // 服务器已在运行
        }

        let registry = Arc::clone(&self.registry);
        let server_addr = address.parse().expect("Invalid address");
        
        // 创建HTTP服务
        let make_svc = make_service_fn(move |_conn| {
            let registry = Arc::clone(&registry);
            async move {
                Ok::<_, hyper::Error>(service_fn(move |req: Request<Body>| {
                    let registry = Arc::clone(&registry);
                    async move {
                        match req.uri().path() {
                            "/metrics" => {
                                let metrics = prometheus::TextEncoder::new()
                                    .encode_to_string(&registry.gather())
                                    .unwrap();
                                Ok(Response::new(Body::from(metrics)))
                            },
                            _ => Ok(Response::builder()
                                .status(404)
                                .body(Body::from("Not Found"))
                                .unwrap()),
                        }
                    }
                }))
            }
        });

        // 启动服务器线程
        let server_running = Arc::clone(&self.server_running);
        let server_thread = thread::spawn(move || {
            server_running.store(1, Ordering::Release);
            
            let server = Server::bind(&server_addr).serve(make_svc);
            println!("Prometheus metrics server listening on: http://{}", server_addr);
            
            // 运行服务器
            block_on(server).expect("Failed to run metrics server");
            
            server_running.store(0, Ordering::Release);
        });

        self.server_thread = Some(server_thread);
    }

    /// 停止Prometheus指标服务器
    pub fn stop_server(&mut self) {
        // 注意：Hyper服务器当前没有优雅停止的简单方法
        // 这里只更新状态
        self.server_running.store(0, Ordering::Release);
    }

    /// 检查服务器是否在运行
    pub fn is_server_running(&self) -> bool {
        self.server_running.load(Ordering::Acquire) == 1
    }
}

impl Default for PerformanceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    
    #[test]
    fn test_counter() {
        let monitor = PerformanceMonitor::new();
        monitor.register_metric(MetricConfig {
            name: "vm_syscall_total".to_string(),
            help: "Total number of system calls".to_string(),
            metric_type: MetricType::Counter,
            labels: Vec::new(),
        });
        
        monitor.increment_counter("vm_syscall_total", 5);
        // 注意：无法直接测试Prometheus导出，但结构应该正确
    }

    #[test]
    fn test_gauge() {
        let monitor = PerformanceMonitor::new();
        monitor.register_metric(MetricConfig {
            name: "vm_memory_usage_bytes".to_string(),
            help: "Current memory usage".to_string(),
            metric_type: MetricType::Gauge,
            labels: Vec::new(),
        });
        
        monitor.set_gauge("vm_memory_usage_bytes", 1024 * 1024);
    }

    #[test]
    fn test_histogram() {
        let monitor = PerformanceMonitor::new();
        monitor.register_metric(MetricConfig {
            name: "vm_syscall_latency_seconds".to_string(),
            help: "System call latency".to_string(),
            metric_type: MetricType::Histogram,
            labels: Vec::new(),
        });
        
        monitor.observe_histogram("vm_syscall_latency_seconds", 0.001);
    }
}