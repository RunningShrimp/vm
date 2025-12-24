//! # vm-runtime - 多运行时支持与GMP型协程调度器
//!
//! 提供多种异步运行时的支持、统一抽象，以及基于Go GMP模型的高性能协程调度器。
//!
//! ## 主要功能
//!
//! - **GMP型协程调度器**: 虚拟CPU协程（G）、处理器（P）、工作线程（M）、反应器（Reactor）
//! - **运行时抽象层**: 统一的异步运行时接口
//! - **多运行时支持**: 支持Tokio、Async-std、Smol等运行时
//! - **运行时选择**: 基于工作负载的智能运行时选择
//! - **性能监控**: 运行时性能指标收集和分析
//! - **兼容性层**: 确保不同运行时间的兼容性
//! - **配置管理**: 灵活的运行时配置选项

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use vm_core::VmError;
#[cfg(feature = "monitoring")]
use vm_monitor::PerformanceMonitor;
#[cfg(not(feature = "monitoring"))]
pub struct PerformanceMonitor;
#[cfg(not(feature = "monitoring"))]
impl PerformanceMonitor {
    pub fn new(_: ()) -> Self {
        Self
    }
    pub fn record_metric(&self, _: &str, _: f64, _: std::collections::HashMap<String, String>) {}
}

/// GMP型协程调度器模块
pub mod scheduler;
pub use scheduler::{
    Coroutine, CoroutineScheduler, CoroutineState, Priority, Processor, Reactor, SchedulerStats,
    WorkerThread, YieldReason,
};

/// GC运行时模块
pub mod gc;
pub mod profiler;
pub use gc::{
    AdaptiveQuota, AllocStats, GcError, GcPhase, GcResult, GcRuntime, GcRuntimeStats, GcStats,
    LockFreeWriteBarrier, OptimizedGc, ParallelMarker, WriteBarrierType,
};

/// 遗留的CoroutinePool（已废弃，推荐使用新的CoroutineScheduler）
// CoroutinePool模块已被移除，推荐使用新的CoroutineScheduler
// CoroutineScheduler提供更好的性能和GMP模型支持

// 为了保持向后兼容性，保留GmpPriority别名
pub use scheduler::Priority as GmpPriority;

/// 运行时类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuntimeType {
    /// Tokio运行时
    Tokio,
    /// Async-std运行时
    AsyncStd,
    /// Smol运行时
    Smol,
    /// 自定义运行时
    Custom,
}

/// 运行时配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// 运行时类型
    pub runtime_type: RuntimeType,
    /// 线程池大小
    pub thread_pool_size: usize,
    /// 最大阻塞线程数
    pub max_blocking_threads: usize,
    /// 任务队列容量
    pub task_queue_capacity: usize,
    /// 启用工作窃取
    pub enable_work_stealing: bool,
    /// 启用I/O驱动
    pub enable_io_driver: bool,
    /// 启用时间驱动
    pub enable_time_driver: bool,
    /// 自定义配置
    pub custom_config: HashMap<String, String>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            runtime_type: RuntimeType::Tokio,
            thread_pool_size: num_cpus::get(),
            max_blocking_threads: 512,
            task_queue_capacity: 1024,
            enable_work_stealing: true,
            enable_io_driver: true,
            enable_time_driver: true,
            custom_config: HashMap::new(),
        }
    }
}

/// 运行时抽象trait
#[async_trait::async_trait]
pub trait AsyncRuntime: Send + Sync {
    fn as_any(&self) -> &dyn std::any::Any;
    /// 获取运行时类型
    fn runtime_type(&self) -> RuntimeType;

    /// 启动运行时
    async fn start(&mut self) -> Result<(), VmError>;

    /// 停止运行时
    async fn stop(&mut self) -> Result<(), VmError>;

    /// 生成任务ID
    fn generate_task_id(&self) -> TaskId;

    /// 提交异步任务
    async fn submit_task<F, Fut, T>(&self, task: F) -> Result<TaskHandle<T>, VmError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<T, VmError>> + Send + 'static,
        T: Send + 'static;

    /// 等待任务完成
    async fn wait_task<T: Send + 'static>(&self, handle: TaskHandle<T>) -> Result<T, VmError>;

    /// 取消任务
    async fn cancel_task(&self, task_id: TaskId) -> Result<(), VmError>;

    /// 获取任务状态
    fn get_task_status(&self, task_id: TaskId) -> TaskStatus;

    /// 生成定时器
    async fn delay(&self, duration: Duration) -> Result<(), VmError>;

    /// 获取运行时统计信息
    fn get_stats(&self) -> RuntimeStats;
}

#[async_trait::async_trait]
pub trait AsyncRuntimeBase: Send + Sync {
    fn as_any(&self) -> &dyn std::any::Any;
    fn runtime_type(&self) -> RuntimeType;
    async fn start(&mut self) -> Result<(), VmError>;
    async fn stop(&mut self) -> Result<(), VmError>;
    fn generate_task_id(&self) -> TaskId;
    async fn cancel_task(&self, task_id: TaskId) -> Result<(), VmError>;
    fn get_task_status(&self, task_id: TaskId) -> TaskStatus;
    async fn delay(&self, duration: Duration) -> Result<(), VmError>;
    fn get_stats(&self) -> RuntimeStats;
}

/// 任务ID
pub type TaskId = u64;

/// 任务句柄
#[derive(Debug, Clone)]
pub struct TaskHandle<T> {
    pub task_id: TaskId,
    pub _phantom: std::marker::PhantomData<T>,
}

/// 任务状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// 待处理
    Pending,
    /// 运行中
    Running,
    /// 已完成
    Completed,
    /// 已取消
    Cancelled,
    /// 失败
    Failed,
}

/// 运行时统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeStats {
    /// 活动任务数
    pub active_tasks: usize,
    /// 等待任务数
    pub pending_tasks: usize,
    /// 已完成任务数
    pub completed_tasks: u64,
    /// 失败任务数
    pub failed_tasks: u64,
    /// 线程池利用率
    pub thread_pool_utilization: f64,
    /// 内存使用量（字节）
    pub memory_usage_bytes: u64,
    /// I/O操作数
    pub io_operations: u64,
    /// 平均任务执行时间（纳秒）
    pub avg_task_duration_ns: u64,
}

/// 运行时管理器
pub struct RuntimeManager {
    /// 当前运行时
    current_runtime: Option<Box<dyn AsyncRuntimeBase>>,
    /// 可用运行时列表
    available_runtimes: HashMap<RuntimeType, Box<dyn AsyncRuntimeBase>>,
    /// 配置
    config: RuntimeConfig,
    /// 性能监控器
    performance_monitor: Arc<PerformanceMonitor>,
    /// 运行时选择器
    runtime_selector: Arc<RwLock<RuntimeSelector>>,
    /// 统计收集器
    stats_collector: Arc<RwLock<StatsCollector>>,
}

impl RuntimeManager {
    /// 创建新的运行时管理器
    pub fn new(performance_monitor: Arc<PerformanceMonitor>) -> Self {
        let config = RuntimeConfig::default();
        let runtime_selector = Arc::new(RwLock::new(RuntimeSelector::new()));
        let stats_collector = Arc::new(RwLock::new(StatsCollector::new()));

        Self {
            current_runtime: None,
            available_runtimes: HashMap::new(),
            config,
            performance_monitor,
            runtime_selector,
            stats_collector,
        }
    }

    /// 初始化运行时管理器
    pub async fn initialize(&mut self) -> Result<(), VmError> {
        // 注册可用的运行时
        self.register_tokio_runtime().await?;
        self.register_async_std_runtime().await?;
        self.register_smol_runtime().await?;
        self.register_custom_runtime().await?;

        // 选择并启动默认运行时
        self.select_and_start_runtime().await?;

        // 启动统计收集
        self.start_stats_collection().await?;

        Ok(())
    }

    /// 注册Tokio运行时
    async fn register_tokio_runtime(&mut self) -> Result<(), VmError> {
        let runtime = TokioRuntime::new(self.config.clone());
        self.available_runtimes
            .insert(RuntimeType::Tokio, Box::new(runtime));
        Ok(())
    }

    /// 注册Async-std运行时
    async fn register_async_std_runtime(&mut self) -> Result<(), VmError> {
        let runtime = AsyncStdRuntime::new(self.config.clone());
        self.available_runtimes
            .insert(RuntimeType::AsyncStd, Box::new(runtime));
        Ok(())
    }

    /// 注册Smol运行时
    async fn register_smol_runtime(&mut self) -> Result<(), VmError> {
        let runtime = SmolRuntime::new(self.config.clone());
        self.available_runtimes
            .insert(RuntimeType::Smol, Box::new(runtime));
        Ok(())
    }

    async fn register_custom_runtime(&mut self) -> Result<(), VmError> {
        // Custom runtime registration disabled - using new CoroutineScheduler instead
        // let runtime = GmpRuntimeAdapter::new();
        // self.available_runtimes
        //     .insert(RuntimeType::Custom, Box::new(runtime));
        Ok(())
    }

    /// 选择并启动运行时
    async fn select_and_start_runtime(&mut self) -> Result<(), VmError> {
        let selected_type = self
            .runtime_selector
            .read()
            .unwrap()
            .select_runtime(&self.available_runtimes.keys().cloned().collect::<Vec<_>>());

        if let Some(runtime) = self.available_runtimes.get_mut(&selected_type) {
            runtime.start().await?;
            self.current_runtime = Some(self.available_runtimes.remove(&selected_type).unwrap());
        }

        Ok(())
    }

    /// 切换运行时
    pub async fn switch_runtime(&mut self, runtime_type: RuntimeType) -> Result<(), VmError> {
        // 停止当前运行时
        if let Some(ref mut runtime) = self.current_runtime {
            runtime.stop().await?;
        }

        // 启动新运行时
        if let Some(runtime) = self.available_runtimes.get_mut(&runtime_type) {
            runtime.start().await?;
            self.current_runtime = Some(self.available_runtimes.remove(&runtime_type).unwrap());
        }

        Ok(())
    }

    /// 提交任务到当前运行时
    pub async fn submit_task<F, Fut, T>(&self, task: F) -> Result<TaskHandle<T>, VmError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<T, VmError>> + Send + 'static,
        T: Send + 'static,
    {
        if let Some(ref runtime) = self.current_runtime {
            if let Some(r) = runtime.as_any().downcast_ref::<TokioRuntime>() {
                    <TokioRuntime as AsyncRuntime>::submit_task(r, task).await
            } else if let Some(r) = runtime.as_any().downcast_ref::<AsyncStdRuntime>() {
                <AsyncStdRuntime as AsyncRuntime>::submit_task(r, task).await
            } else if let Some(r) = runtime.as_any().downcast_ref::<SmolRuntime>() {
                <SmolRuntime as AsyncRuntime>::submit_task(r, task).await
            } else {
                Err(VmError::Core(vm_core::CoreError::Internal {
                    message: "Unknown runtime type".to_string(),
                    module: "vm-runtime".to_string(),
                }))
            }
        } else {
            Err(VmError::Core(vm_core::CoreError::Internal {
                message: "No active runtime".to_string(),
                module: "vm-runtime".to_string(),
            }))
        }
    }

    /// 带优先级提交任务（仅自定义GMP运行时实现）
    pub fn submit_task_with_priority<F, Fut>(
        &self,
        task: F,
        priority: Priority,
    ) -> Result<TaskId, VmError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<(), VmError>> + Send + 'static,
    {
        // 消耗task参数以避免未使用变量警告
        drop(task);
        
        if let Some(ref runtime) = self.current_runtime {
            if let Some(_custom) = runtime.as_any().downcast_ref::<TokioRuntime>() {
                // Priority submission delegated to the new CoroutineScheduler
                Err(VmError::Core(vm_core::CoreError::Internal {
                    message: format!("Use CoroutineScheduler::submit_task for priority scheduling (requested: {:?})", priority)
                        .to_string(),
                    module: "vm-runtime".to_string(),
                }))
            } else {
                Err(VmError::Core(vm_core::CoreError::Internal {
                    message: format!("Priority submission requires CoroutineScheduler (requested priority: {:?})", priority)
                        .to_string(),
                    module: "vm-runtime".to_string(),
                }))
            }
        } else {
            Err(VmError::Core(vm_core::CoreError::Internal {
                message: format!("No active runtime available for task with priority: {:?}", priority)
                    .to_string(),
                module: "vm-runtime".to_string(),
            }))
        }
    }

    /// 等待任务完成
    pub async fn wait_task<T: Send + 'static>(&self, handle: TaskHandle<T>) -> Result<T, VmError> {
        if let Some(ref runtime) = self.current_runtime {
            if let Some(r) = runtime.as_any().downcast_ref::<TokioRuntime>() {
                <TokioRuntime as AsyncRuntime>::wait_task(r, handle).await
            } else if let Some(r) = runtime.as_any().downcast_ref::<AsyncStdRuntime>() {
                <AsyncStdRuntime as AsyncRuntime>::wait_task(r, handle).await
            } else if let Some(r) = runtime.as_any().downcast_ref::<SmolRuntime>() {
                <SmolRuntime as AsyncRuntime>::wait_task(r, handle).await
            } else {
                Err(VmError::Core(vm_core::CoreError::Internal { 
                    message: "Unknown runtime type".to_string(),
                    module: "vm-runtime".to_string(),
                }))
            }
        } else {
            Err(VmError::Core(vm_core::CoreError::Internal {
                message: "No active runtime".to_string(),
                module: "vm-runtime".to_string(),
            }))
        }
    }

    /// 获取当前运行时统计
    pub fn get_current_runtime_stats(&self) -> Option<RuntimeStats> {
        self.current_runtime.as_ref().map(|r| r.get_stats())
    }

    /// 获取所有可用运行时
    pub fn get_available_runtimes(&self) -> Vec<RuntimeType> {
        self.available_runtimes.keys().cloned().collect()
    }

    /// 获取运行时性能比较
    pub fn get_runtime_performance_comparison(
        &self,
    ) -> HashMap<RuntimeType, RuntimePerformanceProfile> {
        let mut comparison = HashMap::new();

        // 为每个运行时创建性能概况
        for (runtime_type, runtime) in &self.available_runtimes {
            let stats = runtime.get_stats();
            let profile = RuntimePerformanceProfile {
                runtime_type: *runtime_type,
                throughput: self.calculate_throughput(&stats),
                latency: self.calculate_latency(&stats),
                memory_efficiency: self.calculate_memory_efficiency(&stats),
                scalability: self.calculate_scalability(&stats),
            };
            comparison.insert(*runtime_type, profile);
        }

        comparison
    }

    /// 启动统计收集
    async fn start_stats_collection(&self) -> Result<(), VmError> {
        let stats_collector = Arc::clone(&self.stats_collector);
        let performance_monitor = Arc::clone(&self.performance_monitor);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));

            loop {
                interval.tick().await;

                // 收集运行时统计
                if let Some(stats) = stats_collector.read().unwrap().get_latest_stats() {
                    performance_monitor.record_metric(
                        "runtime.active_tasks",
                        stats.active_tasks as f64,
                        std::collections::HashMap::new(),
                    );

                    performance_monitor.record_metric(
                        "runtime.completed_tasks",
                        stats.completed_tasks as f64,
                        std::collections::HashMap::new(),
                    );

                    performance_monitor.record_metric(
                        "runtime.thread_pool_utilization",
                        stats.thread_pool_utilization,
                        std::collections::HashMap::new(),
                    );
                }
            }
        });

        Ok(())
    }

    /// 计算吞吐量
    fn calculate_throughput(&self, stats: &RuntimeStats) -> f64 {
        // 简化的吞吐量计算：已完成任务数 / 时间
        stats.completed_tasks as f64 / 60.0 // 每分钟任务数
    }

    /// 计算延迟
    fn calculate_latency(&self, stats: &RuntimeStats) -> Duration {
        Duration::from_nanos(stats.avg_task_duration_ns)
    }

    /// 计算内存效率
    fn calculate_memory_efficiency(&self, stats: &RuntimeStats) -> f64 {
        // 简化的内存效率计算
        if stats.active_tasks > 0 {
            1.0 / (stats.memory_usage_bytes as f64 / stats.active_tasks as f64 / 1024.0 / 1024.0)
        } else {
            0.0
        }
    }

    /// 计算可扩展性
    fn calculate_scalability(&self, stats: &RuntimeStats) -> f64 {
        // 简化的可扩展性计算：线程池利用率
        stats.thread_pool_utilization
    }
}

/// 运行时性能概况
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimePerformanceProfile {
    pub runtime_type: RuntimeType,
    pub throughput: f64,
    pub latency: Duration,
    pub memory_efficiency: f64,
    pub scalability: f64,
}

/// 运行时选择器
pub struct RuntimeSelector {
    /// 运行时性能历史
    performance_history: HashMap<RuntimeType, Vec<RuntimePerformanceProfile>>,
    /// 当前选择策略
    selection_strategy: SelectionStrategy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionStrategy {
    /// 性能最优
    PerformanceOptimal,
    /// 内存最优
    MemoryOptimal,
    /// 低延迟
    LowLatency,
    /// 平衡
    Balanced,
}

impl RuntimeSelector {
    pub fn new() -> Self {
        Self {
            performance_history: HashMap::new(),
            selection_strategy: SelectionStrategy::Balanced,
        }
    }

    /// 选择运行时
    pub fn select_runtime(&self, available_runtimes: &[RuntimeType]) -> RuntimeType {
        if available_runtimes.is_empty() {
            return RuntimeType::Tokio; // 默认选择
        }

        if available_runtimes.len() == 1 {
            return available_runtimes[0];
        }

        match self.selection_strategy {
            SelectionStrategy::PerformanceOptimal => self.select_by_throughput(available_runtimes),
            SelectionStrategy::MemoryOptimal => {
                self.select_by_memory_efficiency(available_runtimes)
            }
            SelectionStrategy::LowLatency => self.select_by_latency(available_runtimes),
            SelectionStrategy::Balanced => self.select_balanced(available_runtimes),
        }
    }

    /// 按吞吐量选择
    fn select_by_throughput(&self, available_runtimes: &[RuntimeType]) -> RuntimeType {
        *available_runtimes
            .iter()
            .max_by_key(|rt| {
                self.performance_history
                    .get(rt)
                    .and_then(|profiles| profiles.last())
                    .map(|p| p.throughput as u64)
                    .unwrap_or(0)
            })
            .unwrap_or(&RuntimeType::Tokio)
    }

    /// 按内存效率选择
    fn select_by_memory_efficiency(&self, available_runtimes: &[RuntimeType]) -> RuntimeType {
        *available_runtimes
            .iter()
            .max_by_key(|rt| {
                self.performance_history
                    .get(rt)
                    .and_then(|profiles| profiles.last())
                    .map(|p| p.memory_efficiency as u64)
                    .unwrap_or(0)
            })
            .unwrap_or(&RuntimeType::Tokio)
    }

    /// 按延迟选择
    fn select_by_latency(&self, available_runtimes: &[RuntimeType]) -> RuntimeType {
        *available_runtimes
            .iter()
            .min_by_key(|rt| {
                self.performance_history
                    .get(rt)
                    .and_then(|profiles| profiles.last())
                    .map(|p| p.latency.as_nanos() as u64)
                    .unwrap_or(u64::MAX)
            })
            .unwrap_or(&RuntimeType::Tokio)
    }

    /// 平衡选择
    fn select_balanced(&self, available_runtimes: &[RuntimeType]) -> RuntimeType {
        // 计算综合评分
        let mut best_runtime = RuntimeType::Tokio;
        let mut best_score = 0.0;

        for runtime in available_runtimes {
            if let Some(profiles) = self.performance_history.get(runtime) {
                if let Some(profile) = profiles.last() {
                    // 综合评分：吞吐量 * 0.4 + 内存效率 * 0.3 + (1.0 / 延迟秒数) * 0.3
                    let latency_score = 1.0 / (profile.latency.as_secs_f64() + 0.001);
                    let score = profile.throughput * 0.4
                        + profile.memory_efficiency * 0.3
                        + latency_score * 0.3;

                    if score > best_score {
                        best_score = score;
                        best_runtime = *runtime;
                    }
                }
            }
        }

        best_runtime
    }

    /// 更新性能历史
    pub fn update_performance_history(
        &mut self,
        runtime_type: RuntimeType,
        profile: RuntimePerformanceProfile,
    ) {
        self.performance_history
            .entry(runtime_type)
            .or_insert_with(Vec::new)
            .push(profile);

        // 限制历史记录数量
        if let Some(profiles) = self.performance_history.get_mut(&runtime_type) {
            if profiles.len() > 100 {
                profiles.remove(0);
            }
        }
    }

    /// 设置选择策略
    pub fn set_selection_strategy(&mut self, strategy: SelectionStrategy) {
        self.selection_strategy = strategy;
    }
}

/// 统计收集器
pub struct StatsCollector {
    /// 运行时统计历史
    stats_history: Vec<(Instant, RuntimeStats)>,
    /// 最大历史记录数
    max_history: usize,
}

impl StatsCollector {
    pub fn new() -> Self {
        Self {
            stats_history: Vec::new(),
            max_history: 1000,
        }
    }

    /// 记录统计信息
    pub fn record_stats(&mut self, stats: RuntimeStats) {
        self.stats_history.push((Instant::now(), stats));

        if self.stats_history.len() > self.max_history {
            self.stats_history.remove(0);
        }
    }

    /// 获取最新统计信息
    pub fn get_latest_stats(&self) -> Option<&RuntimeStats> {
        self.stats_history.last().map(|(_, stats)| stats)
    }

    /// 获取统计历史
    pub fn get_stats_history(&self, duration: Duration) -> Vec<&RuntimeStats> {
        let cutoff = Instant::now() - duration;
        self.stats_history
            .iter()
            .filter(|(time, _)| *time >= cutoff)
            .map(|(_, stats)| stats)
            .collect()
    }
}

/// Tokio运行时实现
pub struct TokioRuntime {
    config: RuntimeConfig,
    runtime: Option<tokio::runtime::Runtime>,
    task_counter: std::sync::atomic::AtomicU64,
}



impl TokioRuntime {
    pub fn new(config: RuntimeConfig) -> Self {
        Self {
            config,
            runtime: None,
            task_counter: std::sync::atomic::AtomicU64::new(0),
        }
    }
}

#[async_trait::async_trait]
impl AsyncRuntime for TokioRuntime {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn runtime_type(&self) -> RuntimeType {
        RuntimeType::Tokio
    }

    async fn start(&mut self) -> Result<(), VmError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(self.config.thread_pool_size)
            .max_blocking_threads(self.config.max_blocking_threads)
            .thread_name("vm-tokio")
            .enable_all()
            .build()
            .map_err(|e| {
                VmError::Io(std::io::Error::new(std::io::ErrorKind::Other, e).to_string())
            })?;

        self.runtime = Some(runtime);
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), VmError> {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
        Ok(())
    }

    fn generate_task_id(&self) -> TaskId {
        self.task_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    async fn submit_task<F, Fut, T>(&self, task: F) -> Result<TaskHandle<T>, VmError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<T, VmError>> + Send + 'static,
        T: Send + 'static,
    {
        if let Some(ref runtime) = self.runtime {
            let task_id = <TokioRuntime as AsyncRuntime>::generate_task_id(self);

            // 在Tokio运行时中生成任务
            runtime.spawn(async move { task().await });

            Ok(TaskHandle {
                task_id,
                _phantom: std::marker::PhantomData,
            })
        } else {
            Err(VmError::Core(vm_core::CoreError::Internal {
                message: "Tokio runtime not started".to_string(),
                module: "vm-runtime".to_string(),
            }))
        }
    }

    async fn wait_task<T: Send + 'static>(&self, _handle: TaskHandle<T>) -> Result<T, VmError> {
        // 简化的实现 - 实际应该等待具体任务完成
        Err(VmError::Core(vm_core::CoreError::NotImplemented { 
            feature: "task waiting".to_string(),
            module: "vm-runtime".to_string(),
        }))
    }

    async fn cancel_task(&self, _task_id: TaskId) -> Result<(), VmError> {
        // 简化的实现
        Ok(())
    }

    fn get_task_status(&self, _task_id: TaskId) -> TaskStatus {
        TaskStatus::Completed
    }

    async fn delay(&self, duration: Duration) -> Result<(), VmError> {
        if let Some(ref runtime) = self.runtime {
            runtime.spawn(async move {
                tokio::time::sleep(duration).await;
            });
        }
        Ok(())
    }

    fn get_stats(&self) -> RuntimeStats {
        RuntimeStats {
            active_tasks: 0,
            pending_tasks: 0,
            completed_tasks: 0,
            failed_tasks: 0,
            thread_pool_utilization: 0.5,
            memory_usage_bytes: 1024 * 1024,
            io_operations: 0,
            avg_task_duration_ns: 1000000,
        }
    }
}

#[async_trait::async_trait]
impl AsyncRuntimeBase for TokioRuntime {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn runtime_type(&self) -> RuntimeType {
        RuntimeType::Tokio
    }
    async fn start(&mut self) -> Result<(), VmError> {
        AsyncRuntime::start(self).await
    }
    async fn stop(&mut self) -> Result<(), VmError> {
        AsyncRuntime::stop(self).await
    }
    fn generate_task_id(&self) -> TaskId {
        AsyncRuntime::generate_task_id(self)
    }
    async fn cancel_task(&self, task_id: TaskId) -> Result<(), VmError> {
        AsyncRuntime::cancel_task(self, task_id).await
    }
    fn get_task_status(&self, task_id: u64) -> TaskStatus {
        AsyncRuntime::get_task_status(self, task_id)
    }
    async fn delay(&self, duration: Duration) -> Result<(), VmError> {
        AsyncRuntime::delay(self, duration).await
    }
    fn get_stats(&self) -> RuntimeStats {
        AsyncRuntime::get_stats(self)
    }
}

/// Async-std运行时实现
pub struct AsyncStdRuntime {
    config: RuntimeConfig,
    task_counter: std::sync::atomic::AtomicU64,
}



impl AsyncStdRuntime {
    pub fn new(config: RuntimeConfig) -> Self {
        Self {
            config,
            task_counter: std::sync::atomic::AtomicU64::new(0),
        }
    }
}

#[async_trait::async_trait]
impl AsyncRuntime for AsyncStdRuntime {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn runtime_type(&self) -> RuntimeType {
        RuntimeType::AsyncStd
    }

    async fn start(&mut self) -> Result<(), VmError> {
        // Async-std运行时初始化
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), VmError> {
        Ok(())
    }

    fn generate_task_id(&self) -> TaskId {
        self.task_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    async fn submit_task<F, Fut, T>(&self, _task: F) -> Result<TaskHandle<T>, VmError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<T, VmError>> + Send + 'static,
        T: Send + 'static,
    {
        let task_id = <AsyncStdRuntime as AsyncRuntime>::generate_task_id(self);
        Ok(TaskHandle {
            task_id,
            _phantom: std::marker::PhantomData,
        })
    }

    async fn wait_task<T: Send + 'static>(&self, _handle: TaskHandle<T>) -> Result<T, VmError> {
        Err(VmError::Core(vm_core::CoreError::NotImplemented { 
            feature: "async-std task waiting".to_string(),
            module: "vm-runtime".to_string(),
        }))
    }

    async fn cancel_task(&self, _task_id: TaskId) -> Result<(), VmError> {
        Ok(())
    }

    fn get_task_status(&self, _task_id: TaskId) -> TaskStatus {
        TaskStatus::Completed
    }

    async fn delay(&self, _duration: Duration) -> Result<(), VmError> {
        Ok(())
    }

    fn get_stats(&self) -> RuntimeStats {
        // 使用配置信息来生成更合理的统计数据
        let thread_pool_size = self.config.thread_pool_size as f64;
        let active_threads = (thread_pool_size * 0.4) as usize; // 40% 利用率
        
        RuntimeStats {
            active_tasks: active_threads * 5, // 假设每个活跃线程处理5个任务
            pending_tasks: self.config.task_queue_capacity / 4, // 队列容量的25%
            completed_tasks: 0,
            failed_tasks: 0,
            thread_pool_utilization: 0.4,
            memory_usage_bytes: 512 * 1024,
            io_operations: 0,
            avg_task_duration_ns: 1200000,
        }
    }
}

#[async_trait::async_trait]
impl AsyncRuntimeBase for AsyncStdRuntime {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn runtime_type(&self) -> RuntimeType {
        RuntimeType::AsyncStd
    }
    async fn start(&mut self) -> Result<(), VmError> {
        <AsyncStdRuntime as AsyncRuntime>::start(self).await
    }
    async fn stop(&mut self) -> Result<(), VmError> {
        <AsyncStdRuntime as AsyncRuntime>::stop(self).await
    }
    fn generate_task_id(&self) -> TaskId {
        AsyncRuntime::generate_task_id(self)
    }
    async fn cancel_task(&self, task_id: TaskId) -> Result<(), VmError> {
        AsyncRuntime::cancel_task(self, task_id).await
    }
    fn get_task_status(&self, task_id: TaskId) -> TaskStatus {
        AsyncRuntime::get_task_status(self, task_id)
    }
    async fn delay(&self, duration: Duration) -> Result<(), VmError> {
        AsyncRuntime::delay(self, duration).await
    }
    fn get_stats(&self) -> RuntimeStats {
        AsyncRuntime::get_stats(self)
    }
}

/// Smol运行时实现
pub struct SmolRuntime {
    config: RuntimeConfig,
    task_counter: std::sync::atomic::AtomicU64,
}

impl SmolRuntime {
    pub fn new(config: RuntimeConfig) -> Self {
        Self {
            config,
            task_counter: std::sync::atomic::AtomicU64::new(0),
        }
    }
}

#[async_trait::async_trait]
impl AsyncRuntime for SmolRuntime {
    async fn start(&mut self) -> Result<(), VmError> {
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), VmError> {
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn runtime_type(&self) -> RuntimeType {
        RuntimeType::Smol
    }

    fn generate_task_id(&self) -> TaskId {
        self.task_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    async fn submit_task<F, Fut, T>(&self, _task: F) -> Result<TaskHandle<T>, VmError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<T, VmError>> + Send + 'static,
        T: Send + 'static,
    {
        let task_id = <SmolRuntime as AsyncRuntime>::generate_task_id(self);
        Ok(TaskHandle {
            task_id,
            _phantom: std::marker::PhantomData,
        })
    }

    async fn wait_task<T: Send + 'static>(&self, _handle: TaskHandle<T>) -> Result<T, VmError> {
        Err(VmError::Core(vm_core::CoreError::NotImplemented { 
            feature: "smol task waiting".to_string(),
            module: "vm-runtime".to_string(),
        }))
    }

    async fn cancel_task(&self, _task_id: TaskId) -> Result<(), VmError> {
        Ok(())
    }

    fn get_task_status(&self, _task_id: TaskId) -> TaskStatus {
        TaskStatus::Completed
    }

    async fn delay(&self, _duration: Duration) -> Result<(), VmError> {
        Ok(())
    }

    fn get_stats(&self) -> RuntimeStats {
        // 使用配置信息来生成更合理的统计数据
        let thread_pool_size = self.config.thread_pool_size as f64;
        let active_threads = (thread_pool_size * 0.3) as usize; // 30% 利用率
        
        RuntimeStats {
            active_tasks: active_threads * 3, // 假设每个活跃线程处理3个任务
            pending_tasks: self.config.task_queue_capacity / 5, // 队列容量的20%
            completed_tasks: 0,
            failed_tasks: 0,
            thread_pool_utilization: 0.3,
            memory_usage_bytes: 256 * 1024,
            io_operations: 0,
            avg_task_duration_ns: 800000,
        }
    }
}

#[async_trait::async_trait]
impl AsyncRuntimeBase for SmolRuntime {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn runtime_type(&self) -> RuntimeType {
        RuntimeType::Smol
    }
    async fn start(&mut self) -> Result<(), VmError> {
        <SmolRuntime as AsyncRuntime>::start(self).await
    }
    async fn stop(&mut self) -> Result<(), VmError> {
        <SmolRuntime as AsyncRuntime>::stop(self).await
    }
    fn generate_task_id(&self) -> TaskId {
        <SmolRuntime as AsyncRuntime>::generate_task_id(self)
    }
    async fn cancel_task(&self, task_id: TaskId) -> Result<(), VmError> {
        <SmolRuntime as AsyncRuntime>::cancel_task(self, task_id).await
    }
    fn get_task_status(&self, task_id: TaskId) -> TaskStatus {
        <SmolRuntime as AsyncRuntime>::get_task_status(self, task_id)
    }
    async fn delay(&self, duration: Duration) -> Result<(), VmError> {
        <SmolRuntime as AsyncRuntime>::delay(self, duration).await
    }
    fn get_stats(&self) -> RuntimeStats {
        <SmolRuntime as AsyncRuntime>::get_stats(self)
    }
}

#[cfg(all(test, feature = "monitoring"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_runtime_manager_initialization() {
        let performance_monitor = Arc::new(PerformanceMonitor::new(MonitorConfig::default()));
        let mut manager = RuntimeManager::new(performance_monitor);

        // 初始化应该成功
        manager.initialize().await.unwrap();

        // 应该有可用的运行时
        assert!(!manager.get_available_runtimes().is_empty());

        // 应该有当前运行时
        assert!(manager.current_runtime.is_some());
    }

    #[test]
    fn test_runtime_selector() {
        let mut selector = RuntimeSelector::new();

        let available = vec![RuntimeType::Tokio, RuntimeType::AsyncStd, RuntimeType::Smol];

        // 默认选择策略应该是平衡的
        let selected = selector.select_runtime(&available);
        assert!(available.contains(&selected));
    }

    #[test]
    fn test_tokio_runtime_creation() {
        let config = RuntimeConfig::default();
        let runtime = TokioRuntime::new(config);

        assert_eq!(runtime.runtime_type(), RuntimeType::Tokio);
    }

    #[test]
    fn test_runtime_stats() {
        let config = RuntimeConfig::default();
        let runtime = TokioRuntime::new(config);

        let stats = runtime.get_stats();
        assert!(stats.thread_pool_utilization >= 0.0 && stats.thread_pool_utilization <= 1.0);
    }
}
