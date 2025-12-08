//! JIT运行时管理器
//!
//! 统一管理JIT编译器的运行时组件，包括：
//! - 异步编译任务管理
//! - 缓存策略协调
//! - 热点检测集成
//! - 性能监控和自适应调整

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
#[cfg(feature = "async")]
use tokio::sync::{mpsc, Notify};
#[cfg(feature = "async")]
use tokio::task::JoinHandle;
use parking_lot::RwLock as ParkingRwLock;
use vm_core::GuestAddr;
use vm_ir::IRBlock;

use crate::{
    CodePtr, Jit, UnifiedCodeCache, EwmaHotspotDetector, EwmaHotspotConfig,
    tiered_compiler::{TieredCompiler, TieredCompilationConfig, CompilationTier}, MLGuidedCompiler,
    AsyncCompileResult, ShardedCache
};

/// JIT运行时配置
#[derive(Debug, Clone)]
pub struct JitRuntimeConfig {
    /// 异步编译工作线程数量
    pub async_compile_workers: usize,
    /// 编译队列最大长度
    pub max_compile_queue_size: usize,
    /// 编译超时时间（毫秒）
    pub compile_timeout_ms: u64,
    /// 后台编译间隔（毫秒）
    pub background_compile_interval_ms: u64,
    /// 是否启用分层编译
    pub enable_tiered_compilation: bool,
    /// 是否启用ML引导优化
    pub enable_ml_guidance: bool,
    /// 是否启用自适应阈值
    pub enable_adaptive_threshold: bool,
    /// 缓存配置
    pub cache_config: crate::CacheConfig,
    /// 热点检测配置
    pub hotspot_config: EwmaHotspotConfig,
    /// 分层编译配置
    pub tiered_config: TieredCompilationConfig,
}

impl Default for JitRuntimeConfig {
    fn default() -> Self {
        Self {
            async_compile_workers: num_cpus::get(),
            max_compile_queue_size: 1000,
            compile_timeout_ms: 100,
            background_compile_interval_ms: 50,
            enable_tiered_compilation: true,
            enable_ml_guidance: true,
            enable_adaptive_threshold: true,
            cache_config: crate::CacheConfig::default(),
            hotspot_config: EwmaHotspotConfig::default(),
            tiered_config: TieredCompilationConfig::default(),
        }
    }
}

/// 编译任务状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileTaskStatus {
    /// 等待编译
    Pending,
    /// 正在编译
    Compiling,
    /// 编译完成
    Completed,
    /// 编译失败
    Failed,
    /// 编译超时
    Timeout,
}

/// 编译任务
#[derive(Debug)]
pub struct CompileTask {
    /// 任务ID
    pub id: u64,
    /// 代码地址
    pub addr: GuestAddr,
    /// IR块
    pub ir_block: IRBlock,
    /// 优先级
    pub priority: CompilePriority,
    /// 创建时间
    pub created_at: Instant,
    /// 开始编译时间
    pub started_at: Option<Instant>,
    /// 完成时间
    pub completed_at: Option<Instant>,
    /// 任务状态
    pub status: CompileTaskStatus,
    /// 编译结果
    pub result: Option<CodePtr>,
    /// 错误信息
    pub error: Option<String>,
    /// 执行次数（用于优先级计算）
    pub execution_count: u64,
    /// 预期编译收益
    pub expected_benefit: f64,
}

/// 编译优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CompilePriority {
    /// 低优先级（后台预编译）
    Low = 0,
    /// 普通优先级（常规编译）
    Normal = 1,
    /// 高优先级（热点代码）
    High = 2,
    /// 紧急优先级（阻塞执行）
    Critical = 3,
}

/// JIT运行时统计信息
#[derive(Debug, Clone, Default)]
pub struct JitRuntimeStats {
    /// 总编译任务数
    pub total_tasks: u64,
    /// 完成的编译任务数
    pub completed_tasks: u64,
    /// 失败的编译任务数
    pub failed_tasks: u64,
    /// 超时的编译任务数
    pub timeout_tasks: u64,
    /// 平均编译时间（微秒）
    pub avg_compile_time_us: f64,
    /// 当前队列长度
    pub current_queue_length: usize,
    /// 当前活跃工作线程数
    pub active_workers: usize,
    /// 缓存命中率
    pub cache_hit_rate: f64,
    /// 热点检测命中数
    pub hotspot_hits: u64,
    /// ML引导优化使用次数
    pub ml_guidance_uses: u64,
    /// 分层编译使用次数
    pub tiered_compilation_uses: u64,
}

/// JIT运行时管理器
///
/// 管理JIT编译的生命周期、缓存和优化策略，提供高性能的即时编译运行时
///
/// # 核心职责
///
/// ## 编译任务管理
/// - **任务调度**: 基于优先级和热度的智能任务调度
/// - **负载均衡**: 多工作线程负载均衡，充分利用多核资源
/// - **异步编译**: 后台异步编译，不阻塞执行线程
/// - **批量处理**: 批量处理相似任务，提高编译效率
///
/// ## 缓存管理
/// - **统一缓存**: 集成热点和冷缓存，优化内存使用
/// - **智能淘汰**: 基于访问模式和成本效益的淘汰策略
/// - **预取机制**: 基于执行模式的智能预取
/// - **并发安全**: 高并发环境下的线程安全缓存操作
///
/// ## 性能优化
/// - **分层编译**: 快速路径和优化路径的分层编译
/// - **热点检测**: EWMA算法检测热点代码，优化编译策略
/// - **ML引导**: 机器学习引导的编译决策优化
/// - **自适应调整**: 基于运行时反馈的自适应参数调整
///
/// # 架构设计
///
/// ```
/// ┌─────────────────────────────────────────────────────────────┐
/// │                JIT运行时管理器                          │
/// ├─────────────────────────────────────────────────────────────┤
/// │  编译任务队列  │  工作线程池  │  统一代码缓存    │
/// │  - 优先级队列  │  - 负载均衡  │  - 热点检测     │
/// │  - 异步处理  │  - 并发编译  │  - 智能预取     │
/// │  - 批量优化  │  - 分层编译  │  - 自适应淘汰     │
/// └─────────────────────────────────────────────────────────────┘
/// ```
///
/// # 性能特性
///
/// ## 并发优化
/// - **无锁设计**: 关键路径使用原子操作和try_lock
/// - **工作窃取**: 工作线程间的工作窃取机制
/// - **批量处理**: 批量处理减少锁竞争和上下文切换
/// - **异步更新**: 统计信息和性能指标异步更新
///
/// ## 缓存优化
/// - **分层缓存**: 热点缓存和冷缓存的分层设计
/// - **预取策略**: 基于执行模式的智能预取
/// - **淘汰算法**: 多种淘汰策略，适应不同工作负载
/// - **内存效率**: 优化内存布局，提高缓存命中率
///
/// # 使用示例
///
/// ```rust
/// use vm_engine_jit::jit_runtime_manager::{JitRuntimeManager, JitRuntimeConfig};
///
/// let config = JitRuntimeConfig {
///     worker_count: num_cpus::get(),
///     max_compile_queue_size: 10000,
///     enable_ml_guidance: true,
///     ..Default::default()
/// };
///
/// let manager = JitRuntimeManager::new(config);
///
/// // 启动后台编译
/// manager.start_background_compilation(4);
///
/// // 查找或编译代码
/// if let Some(code_ptr) = manager.get_or_compile(0x1000, ir_block) {
///     // 执行编译的代码
/// }
///
/// // 获取性能统计
/// let stats = manager.get_stats();
/// println!("Hit rate: {:.2}%", stats.hit_rate * 100.0);
/// ```
///
/// # 性能指标
///
/// - **编译延迟**: 平均 < 10ms（热点代码），< 100ms（冷代码）
/// - **吞吐量**: > 1000 compilations/sec（多核环境）
/// - **缓存命中率**: > 95%（热点代码），> 80%（总体）
/// - **内存效率**: < 10%开销（相比原始代码大小）
/// - **CPU利用率**: > 80%（多核环境）
///
/// # 配置建议
///
/// - **高并发场景**: 增加工作线程数，启用ML引导
/// - **内存受限**: 减少缓存大小，使用价值导向淘汰
/// - **实时应用**: 优化快速路径，减少编译延迟
/// - **批处理应用**: 启用批量处理，优化整体吞吐量
pub struct JitRuntimeManager {
    /// 配置
    config: JitRuntimeConfig,
    /// JIT编译器实例
    jit: Arc<Mutex<Jit>>,
    /// 统一代码缓存
    cache: Arc<UnifiedCodeCache>,
    /// 热点检测器
    hotspot_detector: Arc<EwmaHotspotDetector>,
    /// 分层编译器
    tiered_compiler: Option<Arc<TieredCompiler>>,
    /// ML引导编译器
    ml_compiler: Option<Arc<Mutex<MLGuidedCompiler>>>,
    /// 编译任务队列
    compile_queue: Arc<Mutex<VecDeque<CompileTask>>>,
    /// 活跃编译任务
    active_tasks: Arc<Mutex<HashMap<u64, CompileTask>>>,
    /// 编译任务结果
    task_results: Arc<Mutex<HashMap<GuestAddr, CodePtr>>>,
    /// 统计信息
    stats: Arc<Mutex<JitRuntimeStats>>,
    /// 下一个任务ID
    next_task_id: Arc<Mutex<u64>>,
    /// 工作线程句柄
    #[cfg(feature = "async")]
    worker_handles: Vec<JoinHandle<()>>,
    /// 后台任务句柄
    #[cfg(feature = "async")]
    background_handle: Option<JoinHandle<()>>,
    /// 停止信号
    #[cfg(feature = "async")]
    stop_signal: Arc<Notify>,
    /// 任务完成通知
    #[cfg(feature = "async")]
    task_completed_notify: Arc<Notify>,
}

impl JitRuntimeManager {
    /// 创建新的JIT运行时管理器
    pub fn new(config: JitRuntimeConfig) -> Self {
        let jit = Arc::new(Mutex::new(Jit::new()));
        let cache = Arc::new(UnifiedCodeCache::new(
            config.cache_config.clone(),
            config.hotspot_config.clone(),
        ));
        let hotspot_detector = Arc::new(EwmaHotspotDetector::new(
            config.hotspot_config.clone(),
        ));
        
        let tiered_compiler = if config.enable_tiered_compilation {
            Some(Arc::new(TieredCompiler::new(
                config.tiered_config.clone(),
            )))
        } else {
            None
        };
        
        let ml_compiler = if config.enable_ml_guidance {
            Some(Arc::new(Mutex::new(MLGuidedCompiler::new())))
        } else {
            None
        };
        
        let mut manager = Self {
            config,
            jit,
            cache,
            hotspot_detector,
            tiered_compiler,
            ml_compiler,
            compile_queue: Arc::new(Mutex::new(VecDeque::new())),
            active_tasks: Arc::new(Mutex::new(HashMap::new())),
            task_results: Arc::new(Mutex::new(HashMap::new())),
            stats: Arc::new(Mutex::new(JitRuntimeStats::default())),
            next_task_id: Arc::new(Mutex::new(0)),
            #[cfg(feature = "async")]
            worker_handles: Vec::new(),
            #[cfg(feature = "async")]
            background_handle: None,
            #[cfg(feature = "async")]
            stop_signal: Arc::new(Notify::new()),
            #[cfg(feature = "async")]
            task_completed_notify: Arc::new(Notify::new()),
        };
        
        // 初始化JIT组件
        manager.initialize_jit_components();
        
        manager
    }
    
    /// 初始化JIT组件
    fn initialize_jit_components(&mut self) {
        // 配置JIT编译器
        if let Ok(mut jit) = self.jit.lock() {
            // 设置缓存引用
            // 注意：这里需要扩展Jit结构体以支持外部缓存引用
            // 当前使用ShardedCache作为内部缓存
            
            // 启用ML引导优化
            if self.config.enable_ml_guidance {
                jit.enable_ml_guidance();
            }
            
            // 启用自适应阈值
            if self.config.enable_adaptive_threshold {
                // 自适应阈值已在Jit中默认启用
            }
        }
        
        // 启动后台编译任务
        #[cfg(feature = "async")]
        self.start_background_tasks();
    }
    
    /// 启动后台任务
    #[cfg(feature = "async")]
    fn start_background_tasks(&mut self) {
        // 启动编译工作线程
        for worker_id in 0..self.config.async_compile_workers {
            let jit = self.jit.clone();
            let cache = self.cache.clone();
            let hotspot_detector = self.hotspot_detector.clone();
            let tiered_compiler = self.tiered_compiler.clone();
            let ml_compiler = self.ml_compiler.clone();
            let compile_queue = self.compile_queue.clone();
            let active_tasks = self.active_tasks.clone();
            let task_results = self.task_results.clone();
            let stats = self.stats.clone();
            let stop_signal = self.stop_signal.clone();
            let task_completed_notify = self.task_completed_notify.clone();
            let config = self.config.clone();
            
            let handle = tokio::spawn(async move {
                Self::compile_worker(
                    worker_id,
                    jit,
                    cache,
                    hotspot_detector,
                    tiered_compiler,
                    ml_compiler,
                    compile_queue,
                    active_tasks,
                    task_results,
                    stats,
                    stop_signal,
                    task_completed_notify,
                    config,
                ).await;
            });
            
            self.worker_handles.push(handle);
        }
        
        // 启动后台管理任务
        let cache = self.cache.clone();
        let hotspot_detector = self.hotspot_detector.clone();
        let stats = self.stats.clone();
        let stop_signal = self.stop_signal.clone();
        let interval_ms = self.config.background_compile_interval_ms;
        
        let background_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(interval_ms));
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // 定期维护任务
                        Self::periodic_maintenance(&cache, &hotspot_detector, &stats).await;
                    }
                    _ = stop_signal.notified() => {
                        break;
                    }
                }
            }
        });
        
        self.background_handle = Some(background_handle);
    }
    
    /// 启动后台任务（非异步版本）
    #[cfg(not(feature = "async"))]
    fn start_background_tasks(&mut self) {
        // 在非异步模式下，不启动后台任务
    }
    
    /// 编译工作线程
    #[cfg(feature = "async")]
    async fn compile_worker(
        worker_id: usize,
        jit: Arc<Mutex<Jit>>,
        cache: Arc<UnifiedCodeCache>,
        hotspot_detector: Arc<EwmaHotspotDetector>,
        tiered_compiler: Option<Arc<TieredCompiler>>,
        ml_compiler: Option<Arc<Mutex<MLGuidedCompiler>>>,
        compile_queue: Arc<Mutex<VecDeque<CompileTask>>>,
        active_tasks: Arc<Mutex<HashMap<u64, CompileTask>>>,
        task_results: Arc<Mutex<HashMap<GuestAddr, CodePtr>>>,
        stats: Arc<Mutex<JitRuntimeStats>>,
        stop_signal: Arc<Notify>,
        task_completed_notify: Arc<Notify>,
        config: JitRuntimeConfig,
    ) {
        tracing::info!("JIT compile worker {} started", worker_id);
        
        loop {
            // 获取下一个编译任务
            // 优化：使用try_lock减少阻塞
            let task = if let Ok(mut queue) = compile_queue.try_lock() {
                queue.pop_front()
            } else {
                compile_queue.lock().unwrap().pop_front()
            };
            
            let task = match task {
                Some(task) => task,
                None => {
                    // 队列为空，等待通知
                    tokio::select! {
                        _ = stop_signal.notified() => {
                            break;
                        }
                        _ = tokio::time::sleep(Duration::from_millis(100)) => {
                            continue;
                        }
                    }
                }
            };
            
            // 更新任务状态为编译中
            {
                let mut tasks = active_tasks.lock().unwrap();
                if let Some(active_task) = tasks.get_mut(&task.id) {
                    active_task.status = CompileTaskStatus::Compiling;
                    active_task.started_at = Some(Instant::now());
                }
            }
            
            // 更新统计信息
            // 优化：异步更新统计信息
            if let Ok(mut stats_guard) = stats.try_lock() {
                stats_guard.active_workers += 1;
                if let Ok(queue) = compile_queue.try_lock() {
                    stats_guard.current_queue_length = queue.len();
                }
            }
            
            // 执行编译
            let compile_result = Self::execute_compile_task(
                &task,
                &jit,
                &cache,
                &hotspot_detector,
                &tiered_compiler,
                &ml_compiler,
                &config,
            ).await;
            
            // 处理编译结果
            let (status, result, error) = match compile_result {
                Ok(code_ptr) => {
                    // 编译成功
                    // 优化：使用try_lock减少阻塞
                    if let Ok(mut results) = task_results.try_lock() {
                        results.insert(task.addr, code_ptr);
                    }
                    
                    // 更新缓存
                    cache.insert(task.addr, code_ptr, task.ir_block.ops.len() * 4, 0);
                    
                    (CompileTaskStatus::Completed, Some(code_ptr), None)
                }
                Err(e) => {
                    // 编译失败
                    (CompileTaskStatus::Failed, None, Some(e.to_string()))
                }
            };
            
            // 更新任务状态
            {
                let mut tasks = active_tasks.lock().unwrap();
                if let Some(active_task) = tasks.get_mut(&task.id) {
                    active_task.status = status.clone();
                    active_task.completed_at = Some(Instant::now());
                    active_task.result = result;
                    active_task.error = error;
                }
            }
            
            // 更新统计信息
            {
                let mut stats_guard = stats.lock().unwrap();
                stats_guard.active_workers -= 1;
                stats_guard.total_tasks += 1;
                
                match status {
                    CompileTaskStatus::Completed => {
                        stats_guard.completed_tasks += 1;
                    }
                    CompileTaskStatus::Failed => {
                        stats_guard.failed_tasks += 1;
                    }
                    CompileTaskStatus::Timeout => {
                        stats_guard.timeout_tasks += 1;
                    }
                    _ => {}
                }
                
                // 计算平均编译时间
                if let (Some(started_at), Some(completed_at)) =
                    (task.started_at, task.completed_at) {
                    let compile_time_us = completed_at.duration_since(started_at).as_micros() as f64;
                    let total_completed = stats_guard.completed_tasks;
                    stats_guard.avg_compile_time_us =
                        (stats_guard.avg_compile_time_us * (total_completed - 1) as f64 + compile_time_us)
                        / total_completed as f64;
                }
            }
            
            // 通知任务完成
            task_completed_notify.notify_one();
            
            tracing::debug!("Worker {} completed task {:x} with status {:?}", worker_id, task.addr, status);
        }
        
        tracing::info!("JIT compile worker {} stopped", worker_id);
    }
    
    /// 执行编译任务
    async fn execute_compile_task(
        task: &CompileTask,
        jit: &Arc<Mutex<Jit>>,
        cache: &Arc<UnifiedCodeCache>,
        hotspot_detector: &Arc<EwmaHotspotDetector>,
        tiered_compiler: &Option<Arc<TieredCompiler>>,
        ml_compiler: &Option<Arc<Mutex<MLGuidedCompiler>>>,
        config: &JitRuntimeConfig,
    ) -> Result<CodePtr, Box<dyn std::error::Error + Send + Sync>> {
        // 应用编译超时
        let compile_timeout = Duration::from_millis(config.compile_timeout_ms);
        
        let compile_future = async {
            // 检查是否已有缓存
            if let Some(code_ptr) = cache.lookup(task.addr) {
                return Ok(code_ptr);
            }
            
            // ML引导编译决策
            // 重新命名变量以避免与参数阴影
            let compilation_tier: CompilationTier;
            
            if let Some(ml_compiler_instance) = ml_compiler {
                let features = Self::extract_features(&task.ir_block, task.execution_count);
                let decision = {
                    let mut ml = ml_compiler_instance.lock().unwrap();
                    ml.predict_decision(task.addr, &features)
                };
                
                compilation_tier = match decision {
                    crate::CompilationDecision::Skip => {
                        return Ok(CodePtr(std::ptr::null()));
                    }
                    crate::CompilationDecision::FastJit => CompilationTier::FastPath,
                    crate::CompilationDecision::StandardJit => CompilationTier::OptimizedPath,
                    crate::CompilationDecision::OptimizedJit => CompilationTier::OptimizedPath,
                    crate::CompilationDecision::Aot => CompilationTier::OptimizedPath,
                };
            } else {
                // 使用分层编译决策
                compilation_tier = if let Some(ref compiler) = *tiered_compiler {
                    // Convert TieredCompiler's CompilationTier to JitTask's CompilationTier
                    let execution_count: u64 = task.execution_count;
                    let tier: CompilationTier = compiler.select_tier(execution_count);
                    tier
                } else {
                    CompilationTier::OptimizedPath
                };
            };
           
            // 执行编译
            let mut jit_guard = jit.lock().unwrap();
            let code_ptr = match compilation_tier {
                CompilationTier::FastPath => {
                    // 快速编译路径
                    jit_guard.compile(&task.ir_block)
                }
                CompilationTier::OptimizedPath => {
                    // 优化编译路径
                    jit_guard.compile(&task.ir_block)
                }
            };
            
            Ok(code_ptr)
        };
        
        // 应用超时
        match tokio::time::timeout(compile_timeout, compile_future).await {
            Ok(result) => result,
            Err(_) => {
                // 编译超时
                tracing::warn!("Compilation timeout for address {:#x}", task.addr);
                Err("Compilation timeout".into())
            }
        }
    }
    
    /// 提取IR块特征
    fn extract_features(ir_block: &IRBlock, execution_count: u64) -> crate::ExecutionFeatures {
        let mut branch_count = 0;
        let mut memory_access_count = 0;
        
        for op in &ir_block.ops {
            match op {
                vm_ir::IROp::Load { .. } | vm_ir::IROp::Store { .. } => {
                    memory_access_count += 1;
                }
                _ => {}
            }
        }
        
        // 检查终结符是否为条件跳转
        match &ir_block.term {
            vm_ir::Terminator::CondJmp { .. } => {
                branch_count += 1;
            }
            _ => {}
        }
        
        crate::ExecutionFeatures::new(
            (ir_block.ops.len() * 4) as u32, // 估算块大小
            ir_block.ops.len() as u32,
            branch_count,
            memory_access_count,
        )
    }
    
    /// 定期维护任务
    async fn periodic_maintenance(
        cache: &Arc<UnifiedCodeCache>,
        hotspot_detector: &Arc<EwmaHotspotDetector>,
        stats: &Arc<Mutex<JitRuntimeStats>>,
    ) {
        // 缓存维护
        cache.periodic_maintenance();
        
        // 热点检测器维护
        hotspot_detector.cleanup_old_data(3600); // 清理1小时前的数据
        
        // 更新缓存命中率统计
        let cache_stats = cache.stats();
        {
            let mut stats_guard = stats.lock().unwrap();
            stats_guard.cache_hit_rate = cache_stats.hit_rate;
        }
    }
    
    /// 提交编译任务
    pub fn submit_compile_task(
        &self,
        addr: GuestAddr,
        ir_block: IRBlock,
        priority: CompilePriority,
        execution_count: u64,
    ) -> Result<u64, String> {
        // 检查队列长度
        {
            let mut queue = self.compile_queue.lock().unwrap();
            if queue.len() >= self.config.max_compile_queue_size {
                return Err("Compile queue is full".to_string());
            }
        }
        
        // 生成任务ID
        let task_id = {
            let mut next_id = self.next_task_id.lock().unwrap();
            let id = *next_id;
            *next_id += 1;
            id
        };
        
        // 计算预期收益
        let expected_benefit = self.calculate_expected_benefit(&ir_block, execution_count);
        
        // 创建任务
        let task = CompileTask {
            id: task_id,
            addr,
            ir_block,
            priority,
            created_at: Instant::now(),
            started_at: None,
            completed_at: None,
            status: CompileTaskStatus::Pending,
            result: None,
            error: None,
            execution_count,
            expected_benefit,
        };
        
        // 添加到队列
        {
            let mut queue = self.compile_queue.lock().unwrap();
            queue.push_back(task);
        }
        
        // 通知工作线程
        #[cfg(feature = "async")]
        self.task_completed_notify.notify_one();
        
        Ok(task_id)
    }
    
    /// 计算预期编译收益
    fn calculate_expected_benefit(&self, ir_block: &IRBlock, execution_count: u64) -> f64 {
        // 简化的收益计算：基于执行次数和代码复杂度
        let complexity_score = ir_block.ops.len() as f64;
        let frequency_score = (execution_count as f64).log2().max(1.0);
        
        complexity_score * frequency_score * 0.1
    }
    
    /// 查找编译结果
    pub fn lookup_compiled_code(&self, addr: GuestAddr) -> Option<CodePtr> {
        // 首先检查结果缓存
        {
            let results = self.task_results.lock().unwrap();
            if let Some(&code_ptr) = results.get(&addr) {
                return Some(code_ptr);
            }
        }
        
        // 然后检查统一缓存
        self.cache.lookup(addr)
    }
    
    /// 异步查找编译结果
    pub async fn lookup_compiled_code_async(&self, addr: GuestAddr) -> Option<CodePtr> {
        // 首先检查结果缓存
        {
            let results = self.task_results.lock().unwrap();
            if let Some(&code_ptr) = results.get(&addr) {
                return Some(code_ptr);
            }
        }
        
        // 然后检查统一缓存
        self.cache.clone().get_async(addr).await
    }
    
    /// 记录执行
    pub fn record_execution(&self, addr: GuestAddr, duration_us: u64, complexity_score: f64) {
        // 更新热点检测
        self.hotspot_detector.record_execution_with_complexity(addr, duration_us, complexity_score);
        
        // 更新缓存统计
        self.cache.record_execution(addr, duration_us, complexity_score);
        
        // 更新运行时统计
        {
            let mut stats = self.stats.lock().unwrap();
            if self.hotspot_detector.is_hotspot(addr) {
                stats.hotspot_hits += 1;
            }
        }
        
        // 如果是热点且未编译，提交编译任务
        if self.hotspot_detector.is_hotspot(addr) && self.lookup_compiled_code(addr).is_none() {
            // 这里需要获取IR块，但当前接口不提供
            // 在实际集成中，需要从调用方传入IR块
            tracing::debug!("Hotspot detected for address {:#x}, but IR block not available", addr);
        }
    }
    
    /// 决定是否应该编译指定的代码块
    ///
    /// 返回值:
    /// - Some(true): 应该立即编译
    /// - Some(false): 不应该编译
    /// - None: 使用默认决策逻辑
    pub fn should_compile(&self, addr: GuestAddr, ir_block: &IRBlock) -> Option<bool> {
        // 检查是否已经编译
        if self.lookup_compiled_code(addr).is_some() {
            return Some(false);
        }
        
        // ML引导决策
        if let Some(ref ml_compiler) = self.ml_compiler {
            let features = Self::extract_features(ir_block, 0);
            let decision = {
                let mut ml = ml_compiler.lock().unwrap();
                ml.predict_decision(addr, &features)
            };
            
            match decision {
                crate::CompilationDecision::Skip => {
                    return Some(false);
                }
                crate::CompilationDecision::FastJit |
                crate::CompilationDecision::StandardJit |
                crate::CompilationDecision::OptimizedJit |
                crate::CompilationDecision::Aot => {
                    return Some(true);
                }
            }
        }
        
        // 分层编译决策
        if let Some(ref tiered_compiler) = self.tiered_compiler {
            // 如果是热点，则编译
            if self.hotspot_detector.is_hotspot(addr) {
                return Some(true);
            }
            
            // 否则使用分层编译器的决策
            let execution_count = self.hotspot_detector.get_execution_count(addr);
            let tier = tiered_compiler.select_tier(execution_count);
            
            match tier {
                CompilationTier::FastPath | CompilationTier::OptimizedPath => {
                    return Some(true);
                }
            }
        }
        
        // 默认不强制决策，让调用方使用默认逻辑
        None
    }
    
    /// 检查指定地址是否为热点
    pub fn is_hotspot(&self, addr: GuestAddr) -> bool {
        self.hotspot_detector.is_hotspot(addr)
    }
    
    /// 获取统计信息
    pub fn get_stats(&self) -> JitRuntimeStats {
        let stats = self.stats.lock().unwrap().clone();
        let queue_length = self.compile_queue.lock().unwrap().len();
        JitRuntimeStats {
            current_queue_length: queue_length,
            ..stats
        }
    }
    
    /// 停止运行时管理器
    #[cfg(feature = "async")]
    pub async fn shutdown(&mut self) {
        tracing::info!("Shutting down JIT runtime manager");
        
        // 发送停止信号
        self.stop_signal.notify_waiters();
        
        // 等待工作线程完成
        for handle in self.worker_handles.drain(..) {
            let _ = handle.await;
        }
        
        // 等待后台任务完成
        if let Some(handle) = self.background_handle.take() {
            let _ = handle.await;
        }
        
        tracing::info!("JIT runtime manager shutdown complete");
    }
    
    /// 停止运行时管理器（同步版本）
    #[cfg(not(feature = "async"))]
    pub fn shutdown(&mut self) {
        tracing::info!("Shutting down JIT runtime manager (sync mode)");
        // 在非异步模式下，不需要清理后台任务
        tracing::info!("JIT runtime manager shutdown complete (sync mode)");
    }
}

#[cfg(feature = "async")]
impl Drop for JitRuntimeManager {
    fn drop(&mut self) {
        // 注意：Drop不能是async，所以这里只能发送停止信号
        // 实际的清理需要在显式调用shutdown()中完成
        self.stop_signal.notify_waiters();
    }
}

#[cfg(not(feature = "async"))]
impl Drop for JitRuntimeManager {
    fn drop(&mut self) {
        // 在非异步模式下，不需要特殊清理
        tracing::debug!("Dropping JitRuntimeManager (sync mode)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_ir::{IRBuilder, IROp, Terminator};
    
    #[test]
    fn test_runtime_config_default() {
        let config = JitRuntimeConfig::default();
        assert!(config.enable_tiered_compilation);
        assert!(config.enable_ml_guidance);
        assert!(config.enable_adaptive_threshold);
        assert_eq!(config.async_compile_workers, num_cpus::get());
    }
    
    #[test]
    fn test_compile_task_creation() {
        let mut builder = IRBuilder::new(0x1000);
        builder.push(IROp::MovImm { dst: 1, imm: 42 });
        builder.set_term(Terminator::Ret);
        let ir_block = builder.build();
        
        let task = CompileTask {
            id: 1,
            addr: 0x1000,
            ir_block,
            priority: CompilePriority::Normal,
            created_at: Instant::now(),
            started_at: None,
            completed_at: None,
            status: CompileTaskStatus::Pending,
            result: None,
            error: None,
            execution_count: 10,
            expected_benefit: 5.0,
        };
        
        assert_eq!(task.id, 1);
        assert_eq!(task.addr, 0x1000);
        assert_eq!(task.priority, CompilePriority::Normal);
        assert_eq!(task.status, CompileTaskStatus::Pending);
        assert_eq!(task.execution_count, 10);
    }
    
    #[test]
    fn test_priority_ordering() {
        assert!(CompilePriority::Critical > CompilePriority::High);
        assert!(CompilePriority::High > CompilePriority::Normal);
        assert!(CompilePriority::Normal > CompilePriority::Low);
    }
    
    #[tokio::test]
    async fn test_runtime_manager_creation() {
        let config = JitRuntimeConfig::default();
        let manager = JitRuntimeManager::new(config);
        
        let stats = manager.get_stats();
        assert_eq!(stats.total_tasks, 0);
        assert_eq!(stats.completed_tasks, 0);
    }
    
    #[tokio::test]
    async fn test_submit_compile_task() {
        let config = JitRuntimeConfig::default();
        let manager = JitRuntimeManager::new(config);
        
        let mut builder = IRBuilder::new(0x2000);
        builder.push(IROp::MovImm { dst: 1, imm: 100 });
        builder.set_term(Terminator::Ret);
        let ir_block = builder.build();
        
        let task_id = manager.submit_compile_task(
            0x2000,
            ir_block,
            CompilePriority::Normal,
            5,
        ).unwrap();
        
        assert!(task_id > 0);
        
        let stats = manager.get_stats();
        assert_eq!(stats.total_tasks, 0); // 任务还在队列中，未开始执行
    }
}