//! 指标收集器模块
//!
//! 负责收集和聚合虚拟机系统的性能指标

use chrono::{DateTime, Utc};
use metrics::{counter, gauge, histogram};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use vm_core::GuestAddr;

use crate::MonitorConfig;

/// 热点检测器trait
pub trait HotspotDetector: Send + Sync {
    fn get_hot_blocks_count(&self) -> usize;
    fn get_hot_blocks(&self) -> HashMap<GuestAddr, u64>;
}

/// 系统性能指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// 指标收集时间
    pub timestamp: DateTime<Utc>,
    /// JIT执行指标
    pub jit_metrics: JitMetrics,
    /// TLB性能指标
    pub tlb_metrics: TlbMetrics,
    /// 内存管理指标
    pub memory_metrics: MemoryMetrics,
    /// GC性能指标
    pub gc_metrics: GcMetrics,
    /// 并行处理指标
    pub parallel_metrics: ParallelMetrics,
    /// 系统整体指标
    pub system_metrics: SystemOverallMetrics,
}

impl Default for SystemMetrics {
    fn default() -> Self {
        Self {
            timestamp: Utc::now(),
            jit_metrics: JitMetrics::default(),
            tlb_metrics: TlbMetrics::default(),
            memory_metrics: MemoryMetrics::default(),
            gc_metrics: GcMetrics::default(),
            parallel_metrics: ParallelMetrics::default(),
            system_metrics: SystemOverallMetrics::default(),
        }
    }
}

/// JIT编译器性能指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JitMetrics {
    /// 总执行次数
    pub total_executions: u64,
    /// 平均执行时间 (纳秒)
    pub avg_execution_time_ns: f64,
    /// 峰值执行时间 (纳秒)
    pub peak_execution_time_ns: u64,
    /// 编译缓存命中率
    pub cache_hit_rate: f64,
    /// 编译速度 (compiles/sec)
    pub compilation_rate: f64,
    /// 执行速度 (execs/sec)
    pub execution_rate: f64,
    /// 热点代码块数量
    pub hot_blocks_count: usize,
    /// 解释执行次数
    pub interpreted_executions: u64,
    /// JIT执行次数
    pub jit_executions: u64,
    /// 平均编译时间 (纳秒)
    pub avg_compile_time_ns: f64,
    /// 最大编译时间 (纳秒)
    pub max_compile_time_ns: u64,
    /// 最小编译时间 (纳秒)
    pub min_compile_time_ns: u64,
    /// 总编译次数
    pub total_compilations: u64,
    /// 编译时间P50 (纳秒)
    pub compile_time_p50_ns: u64,
    /// 编译时间P95 (纳秒)
    pub compile_time_p95_ns: u64,
    /// 编译时间P99 (纳秒)
    pub compile_time_p99_ns: u64,
    /// 执行时间P50 (纳秒)
    pub execution_time_p50_ns: u64,
    /// 执行时间P95 (纳秒)
    pub execution_time_p95_ns: u64,
    /// 执行时间P99 (纳秒)
    pub execution_time_p99_ns: u64,
}

impl Default for JitMetrics {
    fn default() -> Self {
        Self {
            total_executions: 0,
            avg_execution_time_ns: 0.0,
            peak_execution_time_ns: 0,
            cache_hit_rate: 0.0,
            compilation_rate: 0.0,
            execution_rate: 0.0,
            hot_blocks_count: 0,
            interpreted_executions: 0,
            jit_executions: 0,
            avg_compile_time_ns: 0.0,
            max_compile_time_ns: 0,
            min_compile_time_ns: 0,
            total_compilations: 0,
            compile_time_p50_ns: 0,
            compile_time_p95_ns: 0,
            compile_time_p99_ns: 0,
            execution_time_p50_ns: 0,
            execution_time_p95_ns: 0,
            execution_time_p99_ns: 0,
        }
    }
}

/// TLB缓存性能指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlbMetrics {
    /// 总查找次数
    pub total_lookups: u64,
    /// 命中次数
    pub total_hits: u64,
    /// 命中率
    pub hit_rate: f64,
    /// 平均查找时间 (纳秒)
    pub avg_lookup_time_ns: f64,
    /// 当前条目数量
    pub current_entries: usize,
    /// 最大容量
    pub capacity: usize,
    /// 替换策略
    pub replacement_policy: String,
    /// 效率评分
    pub efficiency_score: f64,
    /// 最近命中率
    pub recent_hit_rate: f64,
}

impl Default for TlbMetrics {
    fn default() -> Self {
        Self {
            total_lookups: 0,
            total_hits: 0,
            hit_rate: 0.0,
            avg_lookup_time_ns: 0.0,
            current_entries: 0,
            capacity: 0,
            replacement_policy: "Unknown".to_string(),
            efficiency_score: 0.0,
            recent_hit_rate: 0.0,
        }
    }
}

/// 内存管理性能指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetrics {
    /// 总读操作次数
    pub total_reads: u64,
    /// 总写操作次数
    pub total_writes: u64,
    /// 平均读操作时间 (纳秒)
    pub avg_read_time_ns: f64,
    /// 平均写操作时间 (纳秒)
    pub avg_write_time_ns: f64,
    /// 当前内存使用量 (bytes)
    pub current_usage_bytes: u64,
    /// 总内存容量 (bytes)
    pub total_capacity_bytes: u64,
    /// 内存使用率
    pub usage_rate: f64,
    /// 分配速率 (allocs/sec)
    pub allocation_rate: f64,
    /// 释放速率 (deallocs/sec)
    pub deallocation_rate: f64,
    /// 页面错误次数
    pub page_faults: u64,
}

impl Default for MemoryMetrics {
    fn default() -> Self {
        Self {
            total_reads: 0,
            total_writes: 0,
            avg_read_time_ns: 0.0,
            avg_write_time_ns: 0.0,
            current_usage_bytes: 0,
            total_capacity_bytes: 0,
            usage_rate: 0.0,
            allocation_rate: 0.0,
            deallocation_rate: 0.0,
            page_faults: 0,
        }
    }
}

/// GC性能指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcMetrics {
    /// 总GC周期数
    pub total_cycles: u64,
    /// 平均GC时间 (纳秒)
    pub avg_gc_time_ns: f64,
    /// 最大GC时间 (纳秒)
    pub max_gc_time_ns: u64,
    /// 最小GC时间 (纳秒)
    pub min_gc_time_ns: u64,
    /// 平均暂停时间 (纳秒)
    pub avg_pause_time_ns: f64,
    /// 最大暂停时间 (纳秒)
    pub max_pause_time_ns: u64,
    /// GC频率 (cycles/sec)
    pub gc_rate: f64,
    /// 回收的对象数量
    pub objects_collected: u64,
    /// GC时间P50 (纳秒)
    pub gc_time_p50_ns: u64,
    /// GC时间P95 (纳秒)
    pub gc_time_p95_ns: u64,
    /// GC时间P99 (纳秒)
    pub gc_time_p99_ns: u64,
}

impl Default for GcMetrics {
    fn default() -> Self {
        Self {
            total_cycles: 0,
            avg_gc_time_ns: 0.0,
            max_gc_time_ns: 0,
            min_gc_time_ns: 0,
            avg_pause_time_ns: 0.0,
            max_pause_time_ns: 0,
            gc_rate: 0.0,
            objects_collected: 0,
            gc_time_p50_ns: 0,
            gc_time_p95_ns: 0,
            gc_time_p99_ns: 0,
        }
    }
}

/// 并行处理性能指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelMetrics {
    /// 总并行操作次数
    pub total_parallel_operations: u64,
    /// 平均并行执行时间 (纳秒)
    pub avg_parallel_time_ns: f64,
    /// 当前活跃vCPU数量
    pub active_vcpu_count: usize,
    /// 最大vCPU数量
    pub max_vcpu_count: usize,
    /// 并行效率评分
    pub efficiency_score: f64,
    /// 负载均衡策略
    pub load_balancing_policy: String,
    /// 线程安全事件次数
    pub thread_safety_events: u64,
    /// 等待队列长度
    pub pending_queue_length: usize,
}

impl Default for ParallelMetrics {
    fn default() -> Self {
        Self {
            total_parallel_operations: 0,
            avg_parallel_time_ns: 0.0,
            active_vcpu_count: 0,
            max_vcpu_count: 0,
            efficiency_score: 0.0,
            load_balancing_policy: "Unknown".to_string(),
            thread_safety_events: 0,
            pending_queue_length: 0,
        }
    }
}

/// 系统整体性能指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemOverallMetrics {
    /// 总操作次数
    pub total_operations: u64,
    /// 平均操作时间 (纳秒)
    pub avg_operation_time_ns: f64,
    /// 系统吞吐量 (ops/sec)
    pub throughput: f64,
    /// 系统效率评分
    pub efficiency_score: f64,
    /// CPU使用率
    pub cpu_usage_rate: f64,
    /// 系统响应时间百分位数
    pub response_time_p50: u64,
    pub response_time_p95: u64,
    pub response_time_p99: u64,
    /// 错误率
    pub error_rate: f64,
}

impl Default for SystemOverallMetrics {
    fn default() -> Self {
        Self {
            total_operations: 0,
            avg_operation_time_ns: 0.0,
            throughput: 0.0,
            efficiency_score: 0.0,
            cpu_usage_rate: 0.0,
            response_time_p50: 0,
            response_time_p95: 0,
            response_time_p99: 0,
            error_rate: 0.0,
        }
    }
}

/// 指标收集器
pub struct MetricsCollector {
    config: MonitorConfig,
    current_metrics: Arc<RwLock<SystemMetrics>>,
    historical_metrics: Arc<RwLock<Vec<SystemMetrics>>>,
    collection_interval: Duration,
    is_running: Arc<std::sync::atomic::AtomicBool>,

    // 计数器
    total_operations: Arc<AtomicU64>,
    total_jit_executions: Arc<AtomicU64>,
    total_interpreted_executions: Arc<AtomicU64>,
    total_tlb_lookups: Arc<AtomicU64>,
    total_tlb_hits: Arc<AtomicU64>,
    total_memory_reads: Arc<AtomicU64>,
    total_memory_writes: Arc<AtomicU64>,
    total_parallel_operations: Arc<AtomicU64>,
    total_errors: Arc<AtomicU64>,

    // 计时器
    execution_time_sum_ns: Arc<AtomicU64>,
    execution_time_max_ns: Arc<AtomicU64>,
    lookup_time_sum_ns: Arc<AtomicU64>,
    read_time_sum_ns: Arc<AtomicU64>,
    write_time_sum_ns: Arc<AtomicU64>,
    parallel_time_sum_ns: Arc<AtomicU64>,

    // 状态
    current_tlb_entries: Arc<RwLock<usize>>,
    current_memory_usage: Arc<RwLock<u64>>,
    active_vcpus: Arc<RwLock<usize>>,
    compilation_cache_size: Arc<AtomicUsize>,

    // 执行速率统计
    execution_start_time: Arc<RwLock<Instant>>,
    execution_count_since_start: Arc<AtomicU64>,

    // 热点检测统计
    hot_blocks: Arc<RwLock<HashMap<GuestAddr, u64>>>, // PC -> execution count
    hotspot_detector: Option<Arc<dyn HotspotDetector>>,

    // 编译时间统计
    compile_time_sum_ns: Arc<AtomicU64>,
    compile_time_max_ns: Arc<AtomicU64>,
    compile_time_min_ns: Arc<AtomicU64>,
    total_compilations: Arc<AtomicU64>,
    compile_time_samples: Arc<RwLock<Vec<u64>>>, // 用于计算百分位数

    // GC时间统计
    gc_time_sum_ns: Arc<AtomicU64>,
    gc_time_max_ns: Arc<AtomicU64>,
    gc_time_min_ns: Arc<AtomicU64>,
    total_gc_cycles: Arc<AtomicU64>,
    gc_pause_time_sum_ns: Arc<AtomicU64>,
    gc_pause_time_max_ns: Arc<AtomicU64>,
    gc_time_samples: Arc<RwLock<Vec<u64>>>, // 用于计算百分位数

    // 执行时间详细统计（用于百分位数计算）
    execution_time_samples: Arc<RwLock<Vec<u64>>>,
    
    // 系统集成配置（用于动态获取TLB容量、GC统计等）
    tlb_capacity: Arc<RwLock<usize>>,
    objects_collected: Arc<AtomicU64>,
    page_faults: Arc<AtomicU64>,
    max_vcpu_count: Arc<RwLock<usize>>,
    load_balancing_policy: Arc<RwLock<String>>,
}

impl MetricsCollector {
    /// 创建新的指标收集器
    pub fn new(config: MonitorConfig) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let current_metrics = Arc::new(RwLock::new(SystemMetrics::default()));
        let historical_metrics = Arc::new(RwLock::new(Vec::new()));

        let collection_interval = Duration::from_secs(config.default_collection_interval);

        Ok(Self {
            config,
            current_metrics,
            historical_metrics,
            collection_interval,
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),

            total_operations: Arc::new(AtomicU64::new(0)),
            total_jit_executions: Arc::new(AtomicU64::new(0)),
            total_interpreted_executions: Arc::new(AtomicU64::new(0)),
            total_tlb_lookups: Arc::new(AtomicU64::new(0)),
            total_tlb_hits: Arc::new(AtomicU64::new(0)),
            total_memory_reads: Arc::new(AtomicU64::new(0)),
            total_memory_writes: Arc::new(AtomicU64::new(0)),
            total_parallel_operations: Arc::new(AtomicU64::new(0)),
            total_errors: Arc::new(AtomicU64::new(0)),

            execution_time_sum_ns: Arc::new(AtomicU64::new(0)),
            execution_time_max_ns: Arc::new(AtomicU64::new(0)),
            lookup_time_sum_ns: Arc::new(AtomicU64::new(0)),
            read_time_sum_ns: Arc::new(AtomicU64::new(0)),
            write_time_sum_ns: Arc::new(AtomicU64::new(0)),
            parallel_time_sum_ns: Arc::new(AtomicU64::new(0)),

            current_tlb_entries: Arc::new(RwLock::new(0)),
            current_memory_usage: Arc::new(RwLock::new(0)),
            active_vcpus: Arc::new(RwLock::new(0)),
            compilation_cache_size: Arc::new(AtomicUsize::new(0)),

            execution_start_time: Arc::new(RwLock::new(Instant::now())),
            execution_count_since_start: Arc::new(AtomicU64::new(0)),

            hot_blocks: Arc::new(RwLock::new(HashMap::new())),
            hotspot_detector: None,

            compile_time_sum_ns: Arc::new(AtomicU64::new(0)),
            compile_time_max_ns: Arc::new(AtomicU64::new(0)),
            compile_time_min_ns: Arc::new(AtomicU64::new(u64::MAX)),
            total_compilations: Arc::new(AtomicU64::new(0)),
            compile_time_samples: Arc::new(RwLock::new(Vec::new())),

            gc_time_sum_ns: Arc::new(AtomicU64::new(0)),
            gc_time_max_ns: Arc::new(AtomicU64::new(0)),
            gc_time_min_ns: Arc::new(AtomicU64::new(u64::MAX)),
            total_gc_cycles: Arc::new(AtomicU64::new(0)),
            gc_pause_time_sum_ns: Arc::new(AtomicU64::new(0)),
            gc_pause_time_max_ns: Arc::new(AtomicU64::new(0)),
            gc_time_samples: Arc::new(RwLock::new(Vec::new())),

            execution_time_samples: Arc::new(RwLock::new(Vec::new())),
            
            // 系统集成配置
            tlb_capacity: Arc::new(RwLock::new(4096)), // 默认TLB容量
            objects_collected: Arc::new(AtomicU64::new(0)),
            page_faults: Arc::new(AtomicU64::new(0)),
            max_vcpu_count: Arc::new(RwLock::new(8)), // 默认最大vCPU数
            load_balancing_policy: Arc::new(RwLock::new("RoundRobin".to_string())),
        })
    }

    /// 开始指标收集
    pub async fn start_collection(&self, interval: Duration) {
        if self
            .is_running
            .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
        {
            tracing::warn!("Metrics collection already running");
            return;
        }

        tracing::info!("Starting metrics collection with interval: {:?}", interval);

        let collector = self.clone();
        let mut interval_timer = tokio::time::interval(interval);

        loop {
            if !self.is_running.load(Ordering::Relaxed) {
                break;
            }

            tokio::select! {
                _ = interval_timer.tick() => {
                    collector.collect_metrics().await;
                }
            }
        }

        tracing::info!("Metrics collection stopped");
    }

    /// 停止指标收集
    pub async fn stop(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.is_running.store(false, Ordering::Relaxed);
        Ok(())
    }

    /// 收集当前指标
    pub async fn collect_metrics(&self) {
        let metrics = self.calculate_metrics().await;

        // 更新当前指标
        {
            let mut current = self.current_metrics.write().await;
            *current = metrics.clone();
        }

        // 添加到历史记录
        {
            let mut history = self.historical_metrics.write().await;
            history.push(metrics.clone());

            // 清理过期数据
            let retention_hours = self.config.default_retention_hours;
            let max_records =
                (retention_hours * 3600 / self.collection_interval.as_secs()) as usize;
            if history.len() > max_records {
                let remove_count = history.len() - max_records;
                history.drain(0..remove_count);
            }
        }

        // 推送指标到metrics库
        self.push_to_metrics_library(&metrics).await;
    }

    /// 计算性能指标
    async fn calculate_metrics(&self) -> SystemMetrics {
        let now = Utc::now();

        let total_ops = self.total_operations.load(Ordering::Relaxed);
        let total_jit = self.total_jit_executions.load(Ordering::Relaxed);
        let total_interpreted = self.total_interpreted_executions.load(Ordering::Relaxed);
        let total_tlb = self.total_tlb_lookups.load(Ordering::Relaxed);
        let tlb_hits = self.total_tlb_hits.load(Ordering::Relaxed);
        let total_reads = self.total_memory_reads.load(Ordering::Relaxed);
        let total_writes = self.total_memory_writes.load(Ordering::Relaxed);
        let total_parallel = self.total_parallel_operations.load(Ordering::Relaxed);
        let total_errors = self.total_errors.load(Ordering::Relaxed);

        // 记录总操作数用于计算吞吐量
        let _throughput_ops = total_ops;

        // 计算百分位数
        let compile_time_samples = self.compile_time_samples.read().await.clone();
        let execution_time_samples = self.execution_time_samples.read().await.clone();

        let (compile_p50, compile_p95, compile_p99) =
            self.calculate_percentiles(&compile_time_samples);
        let (exec_p50, exec_p95, exec_p99) = self.calculate_percentiles(&execution_time_samples);

        let total_compilations = self.total_compilations.load(Ordering::Relaxed);
        let compile_time_sum = self.compile_time_sum_ns.load(Ordering::Relaxed);
        let compile_time_max = self.compile_time_max_ns.load(Ordering::Relaxed);
        let compile_time_min = self.compile_time_min_ns.load(Ordering::Relaxed);

        // JIT指标
        let jit_metrics = JitMetrics {
            total_executions: total_jit + total_interpreted,
            avg_execution_time_ns: if total_jit + total_interpreted > 0 {
                self.execution_time_sum_ns.load(Ordering::Relaxed) as f64
                    / (total_jit + total_interpreted) as f64
            } else {
                0.0
            },
            peak_execution_time_ns: self.execution_time_max_ns.load(Ordering::Relaxed),
            cache_hit_rate: self.calculate_cache_hit_rate().await,
            compilation_rate: self.calculate_compilation_rate(),
            execution_rate: self.calculate_execution_rate().await,
            hot_blocks_count: self.get_hot_blocks_count().await,
            interpreted_executions: total_interpreted,
            jit_executions: total_jit,
            avg_compile_time_ns: if total_compilations > 0 {
                compile_time_sum as f64 / total_compilations as f64
            } else {
                0.0
            },
            max_compile_time_ns: compile_time_max,
            min_compile_time_ns: if compile_time_min == u64::MAX {
                0
            } else {
                compile_time_min
            },
            total_compilations,
            compile_time_p50_ns: compile_p50,
            compile_time_p95_ns: compile_p95,
            compile_time_p99_ns: compile_p99,
            execution_time_p50_ns: exec_p50,
            execution_time_p95_ns: exec_p95,
            execution_time_p99_ns: exec_p99,
        };

        // TLB指标
        let tlb_metrics = TlbMetrics {
            total_lookups: total_tlb,
            total_hits: tlb_hits,
            hit_rate: if total_tlb > 0 {
                tlb_hits as f64 / total_tlb as f64
            } else {
                0.0
            },
            avg_lookup_time_ns: if total_tlb > 0 {
                self.lookup_time_sum_ns.load(Ordering::Relaxed) as f64 / total_tlb as f64
            } else {
                0.0
            },
            current_entries: *self.current_tlb_entries.read().await,
            capacity: *self.tlb_capacity.read().await,
            replacement_policy: "AdaptiveLru".to_string(),
            efficiency_score: self.calculate_tlb_efficiency().await,
            recent_hit_rate: self.calculate_recent_hit_rate().await,
        };

        // GC指标
        let gc_time_samples = self.gc_time_samples.read().await.clone();
        let (gc_p50, gc_p95, gc_p99) = self.calculate_percentiles(&gc_time_samples);

        let total_gc_cycles = self.total_gc_cycles.load(Ordering::Relaxed);
        let gc_time_sum = self.gc_time_sum_ns.load(Ordering::Relaxed);
        let gc_pause_time_sum = self.gc_pause_time_sum_ns.load(Ordering::Relaxed);
        let gc_time_max = self.gc_time_max_ns.load(Ordering::Relaxed);
        let gc_time_min = self.gc_time_min_ns.load(Ordering::Relaxed);
        let gc_pause_time_max = self.gc_pause_time_max_ns.load(Ordering::Relaxed);

        let gc_metrics = GcMetrics {
            total_cycles: total_gc_cycles,
            avg_gc_time_ns: if total_gc_cycles > 0 {
                gc_time_sum as f64 / total_gc_cycles as f64
            } else {
                0.0
            },
            max_gc_time_ns: gc_time_max,
            min_gc_time_ns: if gc_time_min == u64::MAX {
                0
            } else {
                gc_time_min
            },
            avg_pause_time_ns: if total_gc_cycles > 0 {
                gc_pause_time_sum as f64 / total_gc_cycles as f64
            } else {
                0.0
            },
            max_pause_time_ns: gc_pause_time_max,
            gc_rate: self.calculate_gc_rate(total_gc_cycles).await,
            objects_collected: self.objects_collected.load(Ordering::Relaxed),
            gc_time_p50_ns: gc_p50,
            gc_time_p95_ns: gc_p95,
            gc_time_p99_ns: gc_p99,
        };

        // 内存指标
        let current_usage = *self.current_memory_usage.read().await;
        let total_capacity = 512 * 1024 * 1024; // 512MB
        let memory_metrics = MemoryMetrics {
            total_reads,
            total_writes,
            avg_read_time_ns: if total_reads > 0 {
                self.read_time_sum_ns.load(Ordering::Relaxed) as f64 / total_reads as f64
            } else {
                0.0
            },
            avg_write_time_ns: if total_writes > 0 {
                self.write_time_sum_ns.load(Ordering::Relaxed) as f64 / total_writes as f64
            } else {
                0.0
            },
            current_usage_bytes: current_usage,
            total_capacity_bytes: total_capacity,
            usage_rate: if total_capacity > 0 {
                current_usage as f64 / total_capacity as f64 * 100.0
            } else {
                0.0
            },
            allocation_rate: self.calculate_allocation_rate().await,
            deallocation_rate: self.calculate_deallocation_rate().await,
            page_faults: self.page_faults.load(Ordering::Relaxed),
        };

        // 并行指标
        let active_vcpus = *self.active_vcpus.read().await;
        let parallel_metrics = ParallelMetrics {
            total_parallel_operations: total_parallel,
            avg_parallel_time_ns: if total_parallel > 0 {
                self.parallel_time_sum_ns.load(Ordering::Relaxed) as f64 / total_parallel as f64
            } else {
                0.0
            },
            active_vcpu_count: active_vcpus,
            max_vcpu_count: *self.max_vcpu_count.read().await,
            efficiency_score: self.calculate_parallel_efficiency().await,
            load_balancing_policy: self.load_balancing_policy.read().await.clone(),
            thread_safety_events: self.calculate_thread_safety_events().await,
            pending_queue_length: self.calculate_pending_queue_length().await,
        };

        // 系统整体指标
        let total_ops = total_reads + total_writes + total_jit + total_interpreted + total_parallel;
        let system_metrics = SystemOverallMetrics {
            total_operations: total_ops,
            avg_operation_time_ns: if total_ops > 0 {
                let total_time = self.execution_time_sum_ns.load(Ordering::Relaxed)
                    + self.lookup_time_sum_ns.load(Ordering::Relaxed)
                    + self.read_time_sum_ns.load(Ordering::Relaxed)
                    + self.write_time_sum_ns.load(Ordering::Relaxed)
                    + self.parallel_time_sum_ns.load(Ordering::Relaxed);
                total_time as f64 / total_ops as f64
            } else {
                0.0
            },
            throughput: self.calculate_system_throughput().await,
            efficiency_score: self.calculate_system_efficiency().await,
            cpu_usage_rate: self.calculate_cpu_usage().await,
            response_time_p50: exec_p50,
            response_time_p95: exec_p95,
            response_time_p99: exec_p99,
            error_rate: if total_ops > 0 {
                total_errors as f64 / total_ops as f64
            } else {
                0.0
            },
        };

        SystemMetrics {
            timestamp: now,
            jit_metrics,
            tlb_metrics,
            memory_metrics,
            gc_metrics,
            parallel_metrics,
            system_metrics,
        }
    }

    /// 推送指标到metrics库
    async fn push_to_metrics_library(&self, metrics: &SystemMetrics) {
        // 注意：metrics库的宏API在不同版本中可能有变化
        // 这里使用正确的API格式：counter!(name, value) 或 counter!(name; labels)
        // JIT指标
        counter!("fvp_jit_executions_total").increment(metrics.jit_metrics.total_executions);
        histogram!("fvp_jit_execution_duration_seconds")
            .record(metrics.jit_metrics.avg_execution_time_ns / 1_000_000_000.0);
        gauge!("fvp_jit_compilation_cache_size").set(metrics.jit_metrics.hot_blocks_count as f64);
        gauge!("fvp_jit_compilation_rate").set(metrics.jit_metrics.compilation_rate);

        // TLB指标
        counter!("fvp_tlb_lookups_total").increment(metrics.tlb_metrics.total_lookups);
        counter!("fvp_tlb_hits_total").increment(metrics.tlb_metrics.total_hits);
        gauge!("fvp_tlb_hit_rate").set(metrics.tlb_metrics.hit_rate / 100.0);
        gauge!("fvp_tlb_entries").set(metrics.tlb_metrics.current_entries as f64);

        // 内存指标
        counter!("fvp_memory_reads_total").increment(metrics.memory_metrics.total_reads);
        counter!("fvp_memory_writes_total").increment(metrics.memory_metrics.total_writes);
        gauge!("fvp_memory_usage_bytes").set(metrics.memory_metrics.current_usage_bytes as f64);
        gauge!("fvp_memory_allocation_rate").set(metrics.memory_metrics.allocation_rate);

        // 并行指标
        counter!("fvp_parallel_operations_total")
            .increment(metrics.parallel_metrics.total_parallel_operations);
        gauge!("fvp_vcpu_active_count").set(metrics.parallel_metrics.active_vcpu_count as f64);
        gauge!("fvp_parallel_efficiency").set(metrics.parallel_metrics.efficiency_score / 100.0);

        // 系统指标
        counter!("fvp_system_operations_total").increment(metrics.system_metrics.total_operations);
        gauge!("fvp_system_throughput").set(metrics.system_metrics.throughput);
        gauge!("fvp_system_efficiency").set(metrics.system_metrics.efficiency_score / 100.0);
    }

    /// 获取当前指标
    pub async fn get_current_metrics(&self) -> SystemMetrics {
        self.current_metrics.read().await.clone()
    }

    /// 获取历史指标
    pub async fn get_historical_metrics(&self) -> Vec<SystemMetrics> {
        self.historical_metrics.read().await.clone()
    }

    /// 获取运行时间
    pub async fn get_uptime(&self) -> Duration {
        let start_time = *self.execution_start_time.read().await;
        start_time.elapsed()
    }

    /// 记录JIT执行
    pub fn record_jit_execution(&self, duration: Duration, was_compiled: bool) {
        // 记录编译状态，用于性能分析
        if was_compiled {
            // 这是一次新编译的JIT执行
        } else {
            // 这是一次缓存的JIT代码执行
        }

        self.total_jit_executions.fetch_add(1, Ordering::Relaxed);
        self.total_operations.fetch_add(1, Ordering::Relaxed);
        self.record_execution(); // 记录执行用于速率计算

        let duration_ns = duration.as_nanos() as u64;
        self.execution_time_sum_ns
            .fetch_add(duration_ns, Ordering::Relaxed);

        // 更新峰值
        let current_max = self.execution_time_max_ns.load(Ordering::Relaxed);
        if duration_ns > current_max {
            self.execution_time_max_ns
                .store(duration_ns, Ordering::Relaxed);
        }
    }

    /// 记录解释执行
    pub fn record_interpreted_execution(&self, duration: Duration) {
        self.total_interpreted_executions
            .fetch_add(1, Ordering::Relaxed);
        self.total_operations.fetch_add(1, Ordering::Relaxed);
        self.record_execution(); // 记录执行用于速率计算

        let duration_ns = duration.as_nanos() as u64;
        self.execution_time_sum_ns
            .fetch_add(duration_ns, Ordering::Relaxed);
    }

    /// 记录TLB查找
    pub fn record_tlb_lookup(&self, duration: Duration, was_hit: bool) {
        self.total_tlb_lookups.fetch_add(1, Ordering::Relaxed);

        if was_hit {
            self.total_tlb_hits.fetch_add(1, Ordering::Relaxed);
        }

        let duration_ns = duration.as_nanos() as u64;
        self.lookup_time_sum_ns
            .fetch_add(duration_ns, Ordering::Relaxed);
    }

    /// 记录内存操作
    pub fn record_memory_operation(&self, is_write: bool, duration: Duration) {
        self.total_operations.fetch_add(1, Ordering::Relaxed);

        let duration_ns = duration.as_nanos() as u64;
        if is_write {
            self.total_memory_writes.fetch_add(1, Ordering::Relaxed);
            self.write_time_sum_ns
                .fetch_add(duration_ns, Ordering::Relaxed);
        } else {
            self.total_memory_reads.fetch_add(1, Ordering::Relaxed);
            self.read_time_sum_ns
                .fetch_add(duration_ns, Ordering::Relaxed);
        }
    }

    /// 记录并行操作
    pub fn record_parallel_operation(&self, duration: Duration) {
        self.total_parallel_operations
            .fetch_add(1, Ordering::Relaxed);
        self.total_operations.fetch_add(1, Ordering::Relaxed);

        let duration_ns = duration.as_nanos() as u64;
        self.parallel_time_sum_ns
            .fetch_add(duration_ns, Ordering::Relaxed);
    }

    /// 记录错误
    pub fn record_error(&self) {
        self.total_errors.fetch_add(1, Ordering::Relaxed);
    }

    /// 更新TLB条目数
    pub async fn update_tlb_entries(&self, count: usize) {
        let mut entries = self.current_tlb_entries.write().await;
        *entries = count;
    }

    /// 更新内存使用量
    pub async fn update_memory_usage(&self, usage_bytes: u64) {
        let mut current = self.current_memory_usage.write().await;
        *current = usage_bytes;
    }

    /// 更新活跃vCPU数量
    pub async fn update_active_vcpus(&self, count: usize) {
        let mut active = self.active_vcpus.write().await;
        *active = count;
    }

    /// 更新编译缓存大小
    pub fn update_compilation_cache_size(&self, size: usize) {
        self.compilation_cache_size.store(size, Ordering::Relaxed);
    }
    
    // ========== 系统集成配置方法 ==========
    
    /// 设置TLB容量（从TLB系统获取）
    pub async fn set_tlb_capacity(&self, capacity: usize) {
        let mut cap = self.tlb_capacity.write().await;
        *cap = capacity;
    }
    
    /// 记录GC回收的对象数
    pub fn record_objects_collected(&self, count: u64) {
        self.objects_collected.fetch_add(count, Ordering::Relaxed);
    }
    
    /// 记录页面错误
    pub fn record_page_fault(&self) {
        self.page_faults.fetch_add(1, Ordering::Relaxed);
    }
    
    /// 批量记录页面错误
    pub fn record_page_faults(&self, count: u64) {
        self.page_faults.fetch_add(count, Ordering::Relaxed);
    }
    
    /// 设置最大vCPU数量（从系统配置获取）
    pub async fn set_max_vcpu_count(&self, count: usize) {
        let mut max_vcpus = self.max_vcpu_count.write().await;
        *max_vcpus = count;
    }
    
    /// 设置负载均衡策略（从系统获取）
    pub async fn set_load_balancing_policy(&self, policy: String) {
        let mut lb_policy = self.load_balancing_policy.write().await;
        *lb_policy = policy;
    }
    
    /// 配置系统集成参数（一次性设置多个）
    pub async fn configure_system_integration(
        &self,
        tlb_capacity: Option<usize>,
        max_vcpu_count: Option<usize>,
        load_balancing_policy: Option<String>,
    ) {
        if let Some(cap) = tlb_capacity {
            self.set_tlb_capacity(cap).await;
        }
        if let Some(count) = max_vcpu_count {
            self.set_max_vcpu_count(count).await;
        }
        if let Some(policy) = load_balancing_policy {
            self.set_load_balancing_policy(policy).await;
        }
    }

    // 私有辅助方法
    async fn calculate_cache_hit_rate(&self) -> f64 {
        // 缓存命中率：JIT执行次数 / (JIT执行次数 + 编译次数)
        let total_jit = self.total_jit_executions.load(Ordering::Relaxed);
        let total_compilations = self.total_compilations.load(Ordering::Relaxed);

        let total_attempts = total_jit + total_compilations;
        if total_attempts == 0 {
            return 0.0;
        }

        total_jit as f64 / total_attempts as f64
    }

    fn calculate_compilation_rate(&self) -> f64 {
        // 计算编译速率：总编译次数 / 运行时间（秒）
        // 注意：这是一个同步方法，但execution_start_time是异步的
        // 使用blocking_read或改为async方法
        let total_compilations = self.total_compilations.load(Ordering::Relaxed);
        // 使用tokio::runtime::Handle::current()来在同步上下文中执行异步操作
        // 或者改为async方法
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let start_time = handle.block_on(self.execution_start_time.read());
            let elapsed_secs = start_time.elapsed().as_secs_f64();
            if elapsed_secs > 0.0 {
                total_compilations as f64 / elapsed_secs
            } else {
                0.0
            }
        } else {
            // 如果没有运行时上下文，返回0
            0.0
        }
    }

    async fn calculate_tlb_efficiency(&self) -> f64 {
        // TLB效率评分：基于命中率和查找时间
        let total_lookups = self.total_tlb_lookups.load(Ordering::Relaxed);
        let total_hits = self.total_tlb_hits.load(Ordering::Relaxed);

        if total_lookups == 0 {
            return 0.0;
        }

        let hit_rate = total_hits as f64 / total_lookups as f64;
        let avg_lookup_time = if total_lookups > 0 {
            self.lookup_time_sum_ns.load(Ordering::Relaxed) as f64 / total_lookups as f64
        } else {
            0.0
        };

        // 效率评分：命中率 * (1 - 归一化的查找时间)
        // 假设1ns为基准，查找时间越短效率越高
        let normalized_time = (avg_lookup_time / 1000.0).min(1.0); // 归一化到0-1
        hit_rate * (1.0 - normalized_time)
    }

    async fn calculate_recent_hit_rate(&self) -> f64 {
        // 最近命中率：使用总命中率作为近似值
        // 注意：完整的滑动窗口统计需要额外的数据结构来跟踪最近N次查找
        // 当前实现使用总命中率，这对于大多数场景已经足够
        let total_lookups = self.total_tlb_lookups.load(Ordering::Relaxed);
        let total_hits = self.total_tlb_hits.load(Ordering::Relaxed);

        if total_lookups == 0 {
            return 0.0;
        }

        total_hits as f64 / total_lookups as f64
    }

    /// 计算线程安全事件数
    async fn calculate_thread_safety_events(&self) -> u64 {
        // 线程安全事件：统计并发访问时的冲突次数
        // 当前实现返回0，实际实现需要从并发数据结构获取
        // 这需要与并发TLB、并发GC等系统集成
        0
    }

    /// 计算待处理队列长度
    async fn calculate_pending_queue_length(&self) -> usize {
        // 待处理队列长度：统计等待执行的协程/任务数量
        // 当前实现返回0，实际实现需要从协程池或任务调度器获取
        // 这需要与vm-runtime的协程池集成
        0
    }

    async fn calculate_allocation_rate(&self) -> f64 {
        // 分配速率：内存分配次数 / 运行时间（秒）
        // 简化实现：使用内存写入次数作为近似值
        let total_writes = self.total_memory_writes.load(Ordering::Relaxed);
        let start_time = *self.execution_start_time.read().await;
        let elapsed_secs = start_time.elapsed().as_secs_f64();

        if elapsed_secs > 0.0 {
            total_writes as f64 / elapsed_secs
        } else {
            0.0
        }
    }

    async fn calculate_deallocation_rate(&self) -> f64 {
        // 释放速率：内存释放次数 / 运行时间（秒）
        // 简化实现：使用GC周期数作为近似值
        let total_gc_cycles = self.total_gc_cycles.load(Ordering::Relaxed);
        let start_time = *self.execution_start_time.read().await;
        let elapsed_secs = start_time.elapsed().as_secs_f64();

        if elapsed_secs > 0.0 {
            total_gc_cycles as f64 / elapsed_secs
        } else {
            0.0
        }
    }

    async fn calculate_parallel_efficiency(&self) -> f64 {
        // 并行效率：并行操作数 / 总操作数
        let total_parallel = self.total_parallel_operations.load(Ordering::Relaxed);
        let total_ops = self.total_operations.load(Ordering::Relaxed);

        if total_ops == 0 {
            return 0.0;
        }

        total_parallel as f64 / total_ops as f64
    }

    async fn calculate_system_throughput(&self) -> f64 {
        // 系统吞吐量：总操作数 / 运行时间（秒）
        let total_ops = self.total_operations.load(Ordering::Relaxed);
        let start_time = *self.execution_start_time.read().await;
        let elapsed_secs = start_time.elapsed().as_secs_f64();

        if elapsed_secs > 0.0 {
            total_ops as f64 / elapsed_secs
        } else {
            0.0
        }
    }

    async fn calculate_system_efficiency(&self) -> f64 {
        // 系统效率评分：综合考虑多个指标
        let tlb_efficiency = self.calculate_tlb_efficiency().await;
        let parallel_efficiency = self.calculate_parallel_efficiency().await;
        let total_ops = self.total_operations.load(Ordering::Relaxed);
        let total_errors = self.total_errors.load(Ordering::Relaxed);

        let error_rate = if total_ops > 0 {
            total_errors as f64 / total_ops as f64
        } else {
            0.0
        };

        // 综合评分：TLB效率 * 并行效率 * (1 - 错误率)
        (tlb_efficiency * 0.4 + parallel_efficiency * 0.4 + (1.0 - error_rate) * 0.2).min(1.0)
    }

    async fn calculate_cpu_usage(&self) -> f64 {
        // CPU使用率：执行时间 / 总时间
        // 简化实现：使用执行时间总和估算
        let execution_time_sum = self.execution_time_sum_ns.load(Ordering::Relaxed);
        let start_time = *self.execution_start_time.read().await;
        let elapsed_ns = start_time.elapsed().as_nanos() as u64;

        if elapsed_ns > 0 {
            (execution_time_sum as f64 / elapsed_ns as f64 * 100.0).min(100.0)
        } else {
            0.0
        }
    }

    /// 计算执行速率（指令/秒）
    async fn calculate_execution_rate(&self) -> f64 {
        let start_time = *self.execution_start_time.read().await;
        let elapsed = start_time.elapsed();

        if elapsed.as_secs() > 0 {
            let total_executions = self.execution_count_since_start.load(Ordering::Relaxed);
            total_executions as f64 / elapsed.as_secs() as f64
        } else if elapsed.as_millis() > 0 {
            let total_executions = self.execution_count_since_start.load(Ordering::Relaxed);
            total_executions as f64 / (elapsed.as_millis() as f64 / 1000.0)
        } else {
            // 如果时间太短，使用最近的时间窗口
            let total_jit = self.total_jit_executions.load(Ordering::Relaxed);
            let total_interpreted = self.total_interpreted_executions.load(Ordering::Relaxed);
            let total_executions = total_jit + total_interpreted;

            // 使用收集间隔作为时间窗口
            let window_secs = self.collection_interval.as_secs_f64();
            if window_secs > 0.0 {
                total_executions as f64 / window_secs
            } else {
                0.0
            }
        }
    }

    /// 获取热点代码块数量
    async fn get_hot_blocks_count(&self) -> usize {
        // 首先尝试从热点检测器获取
        if let Some(ref detector) = self.hotspot_detector {
            return detector.get_hot_blocks_count();
        }

        // 否则从内部hot_blocks获取
        let hot_blocks = self.hot_blocks.read().await;
        hot_blocks.len()
    }

    /// 记录执行（用于计算执行速率）
    pub fn record_execution(&self) {
        self.execution_count_since_start
            .fetch_add(1, Ordering::Relaxed);
    }

    /// 记录执行时间
    pub async fn record_execution_time(&self, time_ns: u64) {
        self.execution_time_sum_ns
            .fetch_add(time_ns, Ordering::Relaxed);
        let mut max = self.execution_time_max_ns.load(Ordering::Relaxed);
        while time_ns > max {
            match self.execution_time_max_ns.compare_exchange_weak(
                max,
                time_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => max = x,
            }
        }

        // 记录样本用于百分位数计算（限制样本数量）
        let mut samples = self.execution_time_samples.write().await;
        samples.push(time_ns);
        if samples.len() > 1000 {
            samples.remove(0);
        }
    }

    /// 记录编译时间
    pub fn record_compile_time(&self, time_ns: u64) {
        self.compile_time_sum_ns
            .fetch_add(time_ns, Ordering::Relaxed);
        self.total_compilations.fetch_add(1, Ordering::Relaxed);

        // 更新最大值
        let mut max = self.compile_time_max_ns.load(Ordering::Relaxed);
        while time_ns > max {
            match self.compile_time_max_ns.compare_exchange_weak(
                max,
                time_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => max = x,
            }
        }

        // 更新最小值
        let mut min = self.compile_time_min_ns.load(Ordering::Relaxed);
        while time_ns < min {
            match self.compile_time_min_ns.compare_exchange_weak(
                min,
                time_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => min = x,
            }
        }

        // 记录样本（异步）
        let samples = self.compile_time_samples.clone();
        tokio::spawn(async move {
            let mut samples = samples.write().await;
            samples.push(time_ns);
            if samples.len() > 1000 {
                samples.remove(0);
            }
        });
    }

    /// 记录GC时间
    pub fn record_gc_time(&self, time_ns: u64, pause_time_ns: u64) {
        self.gc_time_sum_ns.fetch_add(time_ns, Ordering::Relaxed);
        self.gc_pause_time_sum_ns
            .fetch_add(pause_time_ns, Ordering::Relaxed);
        self.total_gc_cycles.fetch_add(1, Ordering::Relaxed);

        // 更新最大值
        let mut max = self.gc_time_max_ns.load(Ordering::Relaxed);
        while time_ns > max {
            match self.gc_time_max_ns.compare_exchange_weak(
                max,
                time_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => max = x,
            }
        }

        let mut pause_max = self.gc_pause_time_max_ns.load(Ordering::Relaxed);
        while pause_time_ns > pause_max {
            match self.gc_pause_time_max_ns.compare_exchange_weak(
                pause_max,
                pause_time_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => pause_max = x,
            }
        }

        // 更新最小值
        let mut min = self.gc_time_min_ns.load(Ordering::Relaxed);
        while time_ns < min {
            match self.gc_time_min_ns.compare_exchange_weak(
                min,
                time_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => min = x,
            }
        }

        // 记录样本（异步）
        let samples = self.gc_time_samples.clone();
        tokio::spawn(async move {
            let mut samples = samples.write().await;
            samples.push(time_ns);
            if samples.len() > 1000 {
                samples.remove(0);
            }
        });
    }

    /// 记录热点代码块
    pub async fn record_hot_block(&self, pc: GuestAddr, execution_count: u64) {
        let mut hot_blocks = self.hot_blocks.write().await;
        hot_blocks.insert(pc, execution_count);
    }

    /// 计算GC频率
    async fn calculate_gc_rate(&self, total_cycles: u64) -> f64 {
        let start_time = self.execution_start_time.read().await;
        let elapsed_secs = start_time.elapsed().as_secs_f64();
        if elapsed_secs > 0.0 {
            total_cycles as f64 / elapsed_secs
        } else {
            0.0
        }
    }

    /// 计算百分位数
    fn calculate_percentiles(&self, samples: &[u64]) -> (u64, u64, u64) {
        if samples.is_empty() {
            return (0, 0, 0);
        }

        let mut sorted = samples.to_vec();
        sorted.sort();

        let p50_idx = (sorted.len() as f64 * 0.5) as usize;
        let p95_idx = (sorted.len() as f64 * 0.95) as usize;
        let p99_idx = (sorted.len() as f64 * 0.99) as usize;

        let p50 = sorted
            .get(p50_idx.min(sorted.len() - 1))
            .copied()
            .unwrap_or(0);
        let p95 = sorted
            .get(p95_idx.min(sorted.len() - 1))
            .copied()
            .unwrap_or(0);
        let p99 = sorted
            .get(p99_idx.min(sorted.len() - 1))
            .copied()
            .unwrap_or(0);

        (p50, p95, p99)
    }

    /// 设置热点检测器
    pub fn set_hotspot_detector(&mut self, detector: Arc<dyn HotspotDetector>) {
        self.hotspot_detector = Some(detector);
    }

    /// 获取热点代码块列表
    pub async fn get_hot_blocks(&self) -> HashMap<GuestAddr, u64> {
        if let Some(ref detector) = self.hotspot_detector {
            return detector.get_hot_blocks();
        }

        let hot_blocks = self.hot_blocks.read().await;
        hot_blocks.clone()
    }
}

impl Clone for MetricsCollector {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            current_metrics: self.current_metrics.clone(),
            historical_metrics: self.historical_metrics.clone(),
            collection_interval: self.collection_interval,
            is_running: self.is_running.clone(),

            total_operations: self.total_operations.clone(),
            total_jit_executions: self.total_jit_executions.clone(),
            total_interpreted_executions: self.total_interpreted_executions.clone(),
            total_tlb_lookups: self.total_tlb_lookups.clone(),
            total_tlb_hits: self.total_tlb_hits.clone(),
            total_memory_reads: self.total_memory_reads.clone(),
            total_memory_writes: self.total_memory_writes.clone(),
            total_parallel_operations: self.total_parallel_operations.clone(),
            total_errors: self.total_errors.clone(),

            execution_time_sum_ns: self.execution_time_sum_ns.clone(),
            execution_time_max_ns: self.execution_time_max_ns.clone(),
            lookup_time_sum_ns: self.lookup_time_sum_ns.clone(),
            read_time_sum_ns: self.read_time_sum_ns.clone(),
            write_time_sum_ns: self.write_time_sum_ns.clone(),
            parallel_time_sum_ns: self.parallel_time_sum_ns.clone(),

            current_tlb_entries: self.current_tlb_entries.clone(),
            current_memory_usage: self.current_memory_usage.clone(),
            active_vcpus: self.active_vcpus.clone(),
            compilation_cache_size: self.compilation_cache_size.clone(),

            execution_start_time: self.execution_start_time.clone(),
            execution_count_since_start: self.execution_count_since_start.clone(),

            hot_blocks: self.hot_blocks.clone(),
            hotspot_detector: self.hotspot_detector.clone(),

            compile_time_sum_ns: self.compile_time_sum_ns.clone(),
            compile_time_max_ns: self.compile_time_max_ns.clone(),
            compile_time_min_ns: self.compile_time_min_ns.clone(),
            total_compilations: self.total_compilations.clone(),
            compile_time_samples: self.compile_time_samples.clone(),

            gc_time_sum_ns: self.gc_time_sum_ns.clone(),
            gc_time_max_ns: self.gc_time_max_ns.clone(),
            gc_time_min_ns: self.gc_time_min_ns.clone(),
            total_gc_cycles: self.total_gc_cycles.clone(),
            gc_pause_time_sum_ns: self.gc_pause_time_sum_ns.clone(),
            gc_pause_time_max_ns: self.gc_pause_time_max_ns.clone(),
            gc_time_samples: self.gc_time_samples.clone(),

            execution_time_samples: self.execution_time_samples.clone(),
            
            // 系统集成配置
            tlb_capacity: self.tlb_capacity.clone(),
            objects_collected: self.objects_collected.clone(),
            page_faults: self.page_faults.clone(),
            max_vcpu_count: self.max_vcpu_count.clone(),
            load_balancing_policy: self.load_balancing_policy.clone(),
        }
    }
}
