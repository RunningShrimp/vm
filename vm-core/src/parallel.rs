//! Multi-vCPU 并行执行支持
//!
//! 提供多虚拟 CPU 的并行执行能力，支持 2-8 个 vCPU 并行执行。
//! 使用分片锁机制减少锁竞争，提高并发性能。

use crate::{CoreError, ExecResult, ExecutionEngine, GuestAddr, MMU, VmError};
use parking_lot::RwLock;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(feature = "async")]
/// 协程调度器trait（避免循环依赖）
pub trait CoroutineScheduler: Send + Sync {
    /// 提交协程任务
    fn spawn<F>(&self, priority: crate::Priority, task: F) -> std::sync::Arc<crate::Coroutine>
    where
        F: Fn() + Send + Sync + 'static;

    /// 启动调度器
    fn start(&self) -> std::io::Result<()>;

    /// 停止调度器
    fn stop(&self);

    /// 等待所有协程完成
    fn join_all(&self);
}

/// 分片MMU管理器
///
/// 使用分片锁机制减少多vCPU访问共享MMU时的锁竞争。
/// 每个分片负责一部分地址空间，减少锁的粒度。
pub struct ShardedMmu {
    /// MMU分片
    shards: Vec<Arc<Mutex<Box<dyn MMU>>>>,
    /// 分片掩码
    shard_mask: u64,
    /// 地址到分片的映射函数
    address_shard_fn: fn(GuestAddr) -> usize,
}

impl ShardedMmu {
    /// 创建分片MMU
    pub fn new(mmu_factory: impl Fn() -> Box<dyn MMU>, shard_count: usize) -> Self {
        let mut shards = Vec::with_capacity(shard_count);
        for _ in 0..shard_count {
            shards.push(Arc::new(Mutex::new(mmu_factory())));
        }

        Self {
            shards,
            shard_mask: (shard_count - 1) as u64,
            address_shard_fn: Self::default_address_shard,
        }
    }

    /// 默认地址分片函数（基于地址的高位）
    #[inline]
    fn default_address_shard(addr: GuestAddr) -> usize {
        (addr >> 20) as usize // 使用地址的20-27位作为分片索引
    }

    /// 获取指定地址对应的分片
    #[inline]
    pub fn get_shard(&self, addr: GuestAddr) -> &Arc<Mutex<Box<dyn MMU>>> {
        let shard_idx = (self.address_shard_fn)(addr) & self.shard_mask as usize;
        &self.shards[shard_idx]
    }

    /// 获取所有分片（用于同步操作）
    pub fn all_shards(&self) -> &[Arc<Mutex<Box<dyn MMU>>>] {
        &self.shards
    }

    /// 刷新所有分片的TLB
    pub fn flush_all_tlbs(&self) {
        for shard in &self.shards {
            let mut mmu = shard.lock().unwrap();
            mmu.flush_tlb();
        }
    }
}

/// Multi-vCPU 执行器
///
/// 负责管理和协调多个 vCPU 的并行执行。
/// 使用分片锁机制减少锁竞争，提高并发性能。
/// 并行执行器配置
#[derive(Debug, Clone)]
pub struct ParallelExecutorConfig {
    /// 启用优化特性（无锁数据结构、细粒度锁等）
    pub enable_optimizations: bool,
    /// 启用MMU缓存
    pub enable_mmu_cache: bool,
    /// MMU缓存容量
    pub mmu_cache_capacity: usize,
    /// 分片数量
    pub shard_count: usize,
    /// 启用负载均衡
    pub enable_load_balancing: bool,
}

impl Default for ParallelExecutorConfig {
    fn default() -> Self {
        Self {
            enable_optimizations: true,
            enable_mmu_cache: true,
            mmu_cache_capacity: 1024,
            shard_count: 16,
            enable_load_balancing: true,
        }
    }
}

pub struct MultiVcpuExecutor<B> {
    /// vCPU 集合
    vcpus: Vec<Arc<Mutex<Box<dyn ExecutionEngine<B>>>>>,
    /// 分片内存管理单元
    sharded_mmu: Arc<ShardedMmu>,
    /// 并发统计
    stats: Arc<RwLock<ConcurrencyStats>>,
    /// 协程调度器（可选，用于管理协程资源）
    #[cfg(feature = "async")]
    coroutine_scheduler: Option<Arc<dyn CoroutineScheduler + Send + Sync>>,
    /// 配置
    config: ParallelExecutorConfig,
}

/// 并发性能统计
#[derive(Debug, Default, Clone)]
pub struct ConcurrencyStats {
    /// 总执行次数
    pub total_executions: u64,
    /// 锁竞争次数
    pub lock_contentions: u64,
    /// 平均等待时间（纳秒）
    pub avg_wait_time_ns: u64,
    /// 最大等待时间（纳秒）
    pub max_wait_time_ns: u64,
}

impl<B: 'static + Send + Sync + Clone> MultiVcpuExecutor<B> {
    /// 创建新的 multi-vCPU 执行器（使用分片锁优化）
    pub fn new(
        vcpu_count: u32,
        mmu_factory: impl Fn() -> Box<dyn MMU>,
        engine_factory: impl Fn() -> Box<dyn ExecutionEngine<B>>,
        shard_count: usize,
    ) -> Self {
        Self::with_config(
            vcpu_count,
            mmu_factory,
            engine_factory,
            ParallelExecutorConfig {
                shard_count,
                ..Default::default()
            },
        )
    }

    /// 使用配置创建 multi-vCPU 执行器
    pub fn with_config(
        vcpu_count: u32,
        mmu_factory: impl Fn() -> Box<dyn MMU>,
        engine_factory: impl Fn() -> Box<dyn ExecutionEngine<B>>,
        config: ParallelExecutorConfig,
    ) -> Self {
        let mut vcpus = Vec::new();
        for _ in 0..vcpu_count {
            let engine = engine_factory();
            vcpus.push(Arc::new(Mutex::new(engine)));
        }

        let sharded_mmu = Arc::new(ShardedMmu::new(mmu_factory, config.shard_count));

        Self {
            vcpus,
            sharded_mmu,
            stats: Arc::new(RwLock::new(ConcurrencyStats::default())),
            #[cfg(feature = "async")]
            coroutine_scheduler: None,
            config,
        }
    }

    /// 创建传统的 multi-vCPU 执行器（向后兼容）
    pub fn new_legacy(
        vcpu_count: u32,
        mmu: Arc<Mutex<Box<dyn MMU>>>,
        engine_factory: impl Fn() -> Box<dyn ExecutionEngine<B>>,
    ) -> Self {
        let mut vcpus = Vec::new();
        for _ in 0..vcpu_count {
            let engine = engine_factory();
            vcpus.push(Arc::new(Mutex::new(engine)));
        }

        // 将传统MMU包装成分片MMU（只有一个分片）
        let sharded_mmu = Arc::new(ShardedMmu {
            shards: vec![mmu],
            shard_mask: 0,
            address_shard_fn: |_| 0,
        });

        Self {
            vcpus,
            sharded_mmu,
            stats: Arc::new(RwLock::new(ConcurrencyStats::default())),
            #[cfg(feature = "async")]
            coroutine_scheduler: None,
            config: ParallelExecutorConfig::default(),
        }
    }

    /// 添加 vCPU
    pub fn add_vcpu(&mut self, vcpu: Arc<Mutex<Box<dyn ExecutionEngine<B>>>>) {
        self.vcpus.push(vcpu);
    }

    /// 并行运行所有 vCPU（使用分片锁优化）
    ///
    /// # 已弃用
    ///
    /// 此方法已弃用，推荐使用 `run_parallel_async` 以获得更好的性能和异步支持。
    ///
    /// # 迁移指南
    ///
    /// ```rust,ignore
    /// // 旧代码
    /// executor.run_parallel(&blocks)?;
    ///
    /// // 新代码
    /// executor.run_parallel_async(&blocks).await?;
    /// ```
    #[deprecated(note = "Use run_parallel_async instead for better performance and async support")]
    #[cfg(not(feature = "async"))]
    pub fn run_parallel(&mut self, blocks: &[B]) -> Result<Vec<ExecResult>, VmError> {
        // 同步版本：使用线程（向后兼容）
        if blocks.len() != self.vcpus.len() {
            return Err(VmError::Core(CoreError::InvalidState {
                message: "Block count must match vCPU count".to_string(),
                current: format!("{} blocks", blocks.len()),
                expected: format!("{} vCPUs", self.vcpus.len()),
            }));
        }

        use std::thread;
        let results = Arc::new(Mutex::new(Vec::with_capacity(self.vcpus.len())));
        let mut handles: Vec<thread::JoinHandle<()>> = vec![];

        for (vcpu, block) in self.vcpus.iter().zip(blocks.iter()) {
            let vcpu_clone: Arc<Mutex<Box<dyn ExecutionEngine<B>>>> = Arc::clone(vcpu);
            let sharded_mmu_clone: Arc<ShardedMmu> = Arc::clone(&self.sharded_mmu);
            let results_clone: Arc<Mutex<Vec<ExecResult>>> = Arc::clone(&results);
            let stats_clone: Arc<RwLock<ConcurrencyStats>> = Arc::clone(&self.stats);
            let block_clone = block.clone();

            let handle = thread::spawn(move || {
                let start_time = std::time::Instant::now();
                let mut vcpu_guard = match vcpu_clone.lock() {
                    Ok(guard) => guard,
                    Err(_) => return,
                };
                let mut mmu_adapter = ShardedMmuAdapter {
                    sharded_mmu: sharded_mmu_clone,
                    stats: stats_clone.clone(),
                };
                let result = vcpu_guard.run(&mut mmu_adapter, &block_clone);
                let elapsed = start_time.elapsed();
                {
                    let mut stats = stats_clone.write();
                    stats.total_executions += 1;
                    stats.avg_wait_time_ns =
                        (stats.avg_wait_time_ns + elapsed.as_nanos() as u64) / 2;
                    if elapsed.as_nanos() as u64 > stats.max_wait_time_ns {
                        stats.max_wait_time_ns = elapsed.as_nanos() as u64;
                    }
                }
                if let Ok(mut results_guard) = results_clone.lock() {
                    results_guard.push(result);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().map_err(|_| {
                VmError::Core(CoreError::Concurrency {
                    message: "Thread join failed".to_string(),
                    operation: "run_parallel".to_string(),
                })
            })?;
        }

        let results_guard = Arc::try_unwrap(results).map_err(|_| {
            VmError::Core(CoreError::Internal {
                message: "Failed to unwrap results".to_string(),
                module: "MultiVcpuExecutor".to_string(),
            })
        })?;
        let results = results_guard.into_inner().map_err(|_| {
            VmError::Core(CoreError::Internal {
                message: "Failed to get results".to_string(),
                module: "MultiVcpuExecutor".to_string(),
            })
        })?;
        Ok(results)
    }

    /// 并行运行所有 vCPU（使用协程，需要async feature）
    #[cfg(feature = "async")]
    pub fn run_parallel(&mut self, blocks: &[B]) -> Result<Vec<ExecResult>, VmError> {
        if blocks.len() != self.vcpus.len() {
            return Err(VmError::Core(CoreError::InvalidState {
                message: "Block count must match vCPU count".to_string(),
                current: format!("{} blocks", blocks.len()),
                expected: format!("{} vCPUs", self.vcpus.len()),
            }));
        }
        let rt = tokio::runtime::Runtime::new().map_err(|e| {
            VmError::Core(CoreError::Internal {
                message: format!("Failed to create tokio runtime: {}", e),
                module: "MultiVcpuExecutor".to_string(),
            })
        })?;
        rt.block_on(self.run_parallel_async(blocks))
    }

    /// 获取 vCPU 数量
    pub fn vcpu_count(&self) -> usize {
        self.vcpus.len()
    }

    /// 获取并发性能统计
    pub fn get_concurrency_stats(&self) -> ConcurrencyStats {
        self.stats.read().clone()
    }

    /// 设置协程调度器
    ///
    /// 设置协程调度器后，run_parallel_async将优先使用协程调度器执行任务
    #[cfg(feature = "async")]
    pub fn set_coroutine_scheduler<S: CoroutineScheduler + 'static>(&mut self, scheduler: Arc<S>) {
        self.coroutine_scheduler = Some(scheduler as Arc<dyn CoroutineScheduler + Send + Sync>);
    }

    /// 获取协程调度器（如果已设置）
    #[cfg(feature = "async")]
    pub fn get_coroutine_scheduler(&self) -> Option<&Arc<dyn CoroutineScheduler + Send + Sync>> {
        self.coroutine_scheduler.as_ref()
    }

    /// 创建默认协程池
    ///
    /// 注意：此方法需要外部协程池实现，暂时移除
    #[cfg(feature = "async")]
    pub fn create_default_pool(&mut self) {
        // 需要外部提供协程池实现
        // let pool = Arc::new(vm_runtime::CoroutinePool::new(self.vcpus.len() * 2));
        // self.coroutine_pool = Some(pool);
    }

    /// 异步并行运行所有 vCPU（使用协程）
    ///
    /// 使用async/await协程替代线程，减少上下文切换开销
    /// 优先使用协程调度器管理协程资源，提高资源利用率
    /// 如果没有设置协程调度器，回退到tokio::spawn
    #[cfg(feature = "async")]
    pub async fn run_parallel_async(&self, blocks: &[B]) -> Result<Vec<ExecResult>, VmError> {
        // 如果设置了协程调度器，优先使用协程调度器
        if let Some(scheduler) = &self.coroutine_scheduler {
            return self.run_parallel_with_scheduler(blocks, scheduler.as_ref());
        }

        // 否则使用tokio::spawn（向后兼容）
        use tokio::sync::Mutex as AsyncMutex;

        if blocks.len() != self.vcpus.len() {
            return Err(VmError::Core(CoreError::InvalidState {
                message: "Block count must match vCPU count".to_string(),
                current: format!("{} blocks", blocks.len()),
                expected: format!("{} vCPUs", self.vcpus.len()),
            }));
        }

        let results = Arc::new(AsyncMutex::new(Vec::with_capacity(self.vcpus.len())));

        // 使用tokio::join!并行执行所有vCPU协程
        let mut tasks = Vec::new();

        for (_vcpu_id, (vcpu, block)) in self.vcpus.iter().zip(blocks.iter()).enumerate() {
            let vcpu_clone: Arc<Mutex<Box<dyn ExecutionEngine<B>>>> = Arc::clone(vcpu);
            let sharded_mmu_clone: Arc<ShardedMmu> = Arc::clone(&self.sharded_mmu);
            let results_clone: Arc<AsyncMutex<Vec<ExecResult>>> = Arc::clone(&results);
            let stats_clone: Arc<RwLock<ConcurrencyStats>> = Arc::clone(&self.stats);
            let block_clone = block.clone();

            // 使用tokio::spawn创建协程任务
            #[cfg(feature = "async")]
            {
                let task = tokio::spawn(async move {
                    let start_time = std::time::Instant::now();

                    // 执行 vCPU（在异步上下文中）
                    // 注意：这里仍然使用同步锁，因为ExecutionEngine接口是同步的
                    // 在实际应用中，应该使用tokio::sync::Mutex或异步MMU接口
                    let mut vcpu_guard = match vcpu_clone.lock() {
                        Ok(guard) => guard,
                        Err(_) => return, // 如果锁被污染，协程退出
                    };

                    // 创建分片MMU适配器
                    let mut mmu_adapter = ShardedMmuAdapter {
                        sharded_mmu: sharded_mmu_clone,
                        stats: stats_clone.clone(),
                    };

                    // 执行vCPU
                    // 优化：如果MMU支持异步接口，使用异步版本；否则使用spawn_blocking
                    // 这样可以减少线程池压力，提高并发性能
                    #[cfg(feature = "async")]
                    let result = {
                        // 尝试使用异步MMU（如果可用）
                        // 目前ExecutionEngine接口是同步的，所以仍需要spawn_blocking
                        // 但我们可以优化：只在真正需要阻塞时才使用spawn_blocking
                        // 对于快速路径（如TLB命中），可以直接执行
                        tokio::task::spawn_blocking(move || {
                            vcpu_guard.run(&mut mmu_adapter, &block_clone)
                        })
                        .await
                        .unwrap_or_else(|_| ExecResult {
                            status: crate::ExecStatus::Ok,
                            stats: crate::ExecStats::default(),
                            next_pc: 0,
                        })
                    };

                    let elapsed = start_time.elapsed();
                    {
                        let mut stats = stats_clone.write();
                        stats.total_executions += 1;
                        stats.avg_wait_time_ns =
                            (stats.avg_wait_time_ns + elapsed.as_nanos() as u64) / 2;
                        if elapsed.as_nanos() as u64 > stats.max_wait_time_ns {
                            stats.max_wait_time_ns = elapsed.as_nanos() as u64;
                        }
                    }

                    let mut results_guard = results_clone.lock().await;
                    results_guard.push(result);
                });

                tasks.push(task);
            }
        }

        // 等待所有协程完成并收集结果
        let mut final_results = Vec::new();
        for task in tasks {
            if let Ok(_) = task.await {
                // 任务已完成，结果已写入共享的 results
            }
        }

        // 获取结果
        let results_guard = results.lock().await;
        // 手动克隆每个 ExecResult
        let mut final_results = Vec::new();
        for result in results_guard.iter() {
            final_results.push(crate::ExecResult {
                status: match &result.status {
                    crate::ExecStatus::Continue => crate::ExecStatus::Continue,
                    crate::ExecStatus::Ok => crate::ExecStatus::Ok,
                    crate::ExecStatus::Fault(err) => crate::ExecStatus::Fault(err.clone()),
                    crate::ExecStatus::IoRequest => crate::ExecStatus::IoRequest,
                    crate::ExecStatus::InterruptPending => crate::ExecStatus::InterruptPending,
                },
                stats: result.stats.clone(),
                next_pc: result.next_pc,
            });
        }
        Ok(final_results)
    }

    /// 使用协程调度器并行运行所有 vCPU
    ///
    /// 使用协程调度器管理协程资源，提高资源利用率
    ///
    /// 注意：此方法需要外部提供协程调度器，避免循环依赖
    #[cfg(feature = "async")]
    pub fn run_parallel_with_scheduler<S>(
        &self,
        blocks: &[B],
        scheduler: &S,
    ) -> Result<Vec<crate::ExecResult>, VmError>
    where
        S: CoroutineScheduler + Send + Sync,
    {
        #[cfg(feature = "async")]
        use tokio::sync::Mutex as AsyncMutex;

        if blocks.len() != self.vcpus.len() {
            return Err(VmError::Core(CoreError::InvalidState {
                message: "Block count must match vCPU count".to_string(),
                current: format!("{} blocks", blocks.len()),
                expected: format!("{} vCPUs", self.vcpus.len()),
            }));
        }

        let results = Arc::new(parking_lot::Mutex::new(Vec::with_capacity(self.vcpus.len())));
        let task_handles = Arc::new(parking_lot::Mutex::new(Vec::new()));

        // 使用协程调度器提交任务
        for (_vcpu_id, (vcpu, block)) in self.vcpus.iter().zip(blocks.iter()).enumerate() {
            let vcpu_clone: Arc<Mutex<Box<dyn ExecutionEngine<B>>>> = Arc::clone(vcpu);
            let sharded_mmu_clone: Arc<ShardedMmu> = Arc::clone(&self.sharded_mmu);
            let results_clone: Arc<parking_lot::Mutex<Vec<ExecResult>>> = Arc::clone(&results);
            let stats_clone: Arc<RwLock<ConcurrencyStats>> = Arc::clone(&self.stats);
            let block_clone = block.clone();

            let task = move || {
                let start_time = std::time::Instant::now();

                // 执行 vCPU
                let mut vcpu_guard = match vcpu_clone.lock() {
                    Ok(guard) => guard,
                    Err(_) => return,
                };

                let mut mmu_adapter = ShardedMmuAdapter {
                    sharded_mmu: sharded_mmu_clone,
                    stats: stats_clone.clone(),
                };

                // 使用spawn_blocking执行阻塞操作
                #[cfg(feature = "async")]
                let result = {
                    let handle = std::thread::spawn(move || {
                        vcpu_guard.run(&mut mmu_adapter, &block_clone)
                    });
                    
                    // 等待任务完成
                    handle.join().unwrap_or_else(|_| ExecResult {
                        status: crate::ExecStatus::Ok,
                        stats: crate::ExecStats::default(),
                        next_pc: 0,
                    })
                };

                let elapsed = start_time.elapsed();
                {
                    let mut stats = stats_clone.write();
                    stats.total_executions += 1;
                    stats.avg_wait_time_ns =
                        (stats.avg_wait_time_ns + elapsed.as_nanos() as u64) / 2;
                    if elapsed.as_nanos() as u64 > stats.max_wait_time_ns {
                        stats.max_wait_time_ns = elapsed.as_nanos() as u64;
                    }
                }

                results_clone.lock().push(result);
            };

            // 使用协程调度器提交任务
            let handle = scheduler.spawn(crate::Priority::Medium, task);
            task_handles.lock().push(handle);
        }

        // 启动调度器
        if let Err(e) = scheduler.start() {
            return Err(VmError::Core(CoreError::Internal {
                message: format!("Failed to start scheduler: {}", e),
                module: "MultiVcpuExecutor".to_string(),
            }));
        }

        // 等待所有协程完成
        scheduler.join_all();

        // 停止调度器
        scheduler.stop();

        // 获取结果
        let results_guard = results.lock();
        // 手动克隆每个 ExecResult
        let mut final_results = Vec::new();
        for result in results_guard.iter() {
            final_results.push(crate::ExecResult {
                status: match &result.status {
                    crate::ExecStatus::Continue => crate::ExecStatus::Continue,
                    crate::ExecStatus::Ok => crate::ExecStatus::Ok,
                    crate::ExecStatus::Fault(err) => crate::ExecStatus::Fault(err.clone()),
                    crate::ExecStatus::IoRequest => crate::ExecStatus::IoRequest,
                    crate::ExecStatus::InterruptPending => crate::ExecStatus::InterruptPending,
                },
                stats: result.stats.clone(),
                next_pc: result.next_pc,
            });
        }
        Ok(final_results)
    }
}

/// vCPU 负载均衡器
///
/// 用于在多个 vCPU 之间均衡工作负载。
pub struct VcpuLoadBalancer {
    /// 各 vCPU 的负载
    vcpu_loads: Vec<u64>,
    /// 负载均衡策略
    policy: LoadBalancePolicy,
    /// 当前轮询索引
    round_robin_index: std::sync::atomic::AtomicUsize,
}

/// 负载均衡策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadBalancePolicy {
    /// 轮询
    RoundRobin,
    /// 最少负载
    LeastLoaded,
    /// 加权轮询
    WeightedRoundRobin,
}

impl VcpuLoadBalancer {
    /// 创建新的负载均衡器
    pub fn new(vcpu_count: usize, policy: LoadBalancePolicy) -> Self {
        Self {
            vcpu_loads: vec![0; vcpu_count],
            policy,
            round_robin_index: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// 选择下一个应该执行的 vCPU
    pub fn select_vcpu(&self) -> usize {
        match self.policy {
            LoadBalancePolicy::RoundRobin => {
                // 原子性轮询
                let current = self
                    .round_robin_index
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                current % self.vcpu_loads.len()
            }
            LoadBalancePolicy::LeastLoaded => {
                // 选择负载最少的 vCPU
                self.vcpu_loads
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, load)| *load)
                    .map(|(idx, _)| idx)
                    .unwrap_or(0)
            }
            LoadBalancePolicy::WeightedRoundRobin => {
                // 简单的加权轮询实现（基于负载的倒数作为权重）
                let total_weight: f64 = self
                    .vcpu_loads
                    .iter()
                    .map(|&load| if load == 0 { 100.0 } else { 1.0 / load as f64 })
                    .sum();

                if total_weight == 0.0 {
                    return 0;
                }

                // 使用系统时间作为简单的随机数种子
                let random_weight = {
                    let seed = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos() as u64;
                    // 简单的线性同余生成器
                    let mut rng_state = seed;
                    rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
                    let rng_float = (rng_state % 1000000) as f64 / 1000000.0;
                    rng_float * total_weight
                };
                let mut accumulated_weight = 0.0;

                for (idx, &load) in self.vcpu_loads.iter().enumerate() {
                    let weight = if load == 0 { 100.0 } else { 1.0 / load as f64 };
                    accumulated_weight += weight;
                    if random_weight <= accumulated_weight {
                        return idx;
                    }
                }

                self.vcpu_loads.len() - 1
            }
        }
    }

    /// 更新 vCPU 负载
    pub fn update_load(&mut self, vcpu_id: usize, load: u64) {
        if vcpu_id < self.vcpu_loads.len() {
            self.vcpu_loads[vcpu_id] = load;
        }
    }

    /// 获取负载统计
    pub fn get_stats(&self) -> (u64, u64, f64) {
        if self.vcpu_loads.is_empty() {
            return (0, 0, 0.0);
        }

        let min = *self.vcpu_loads.iter().min().unwrap_or(&0);
        let max = *self.vcpu_loads.iter().max().unwrap_or(&0);
        let avg = self.vcpu_loads.iter().sum::<u64>() as f64 / self.vcpu_loads.len() as f64;

        (min, max, avg)
    }
}

/// 分片MMU适配器
///
/// 为ExecutionEngine提供统一的MMU接口，同时使用分片锁减少竞争。
struct ShardedMmuAdapter {
    sharded_mmu: Arc<ShardedMmu>,
    stats: Arc<RwLock<ConcurrencyStats>>,
}

impl MMU for ShardedMmuAdapter {
    fn translate(
        &mut self,
        va: GuestAddr,
        access: crate::AccessType,
    ) -> Result<GuestAddr, crate::VmError> {
        // 根据地址选择分片
        let shard = self.sharded_mmu.get_shard(va);
        let start_time = std::time::Instant::now();

        // 获取锁并执行翻译
        let result = {
            let mut mmu_guard = shard.lock().unwrap();
            mmu_guard.translate(va, access)
        };

        let elapsed = start_time.elapsed();

        // 记录锁竞争统计
        if elapsed.as_nanos() > 1000 {
            // 如果等待超过1微秒，认为有竞争
            let mut stats = self.stats.write();
            stats.lock_contentions += 1;
        }

        result
    }

    fn fetch_insn(&self, pc: GuestAddr) -> Result<u64, crate::VmError> {
        let shard = self.sharded_mmu.get_shard(pc);
        let mmu_guard = shard.lock().unwrap();
        mmu_guard.fetch_insn(pc)
    }

    fn read(&self, pa: GuestAddr, size: u8) -> Result<u64, crate::VmError> {
        let shard = self.sharded_mmu.get_shard(pa);
        let mmu_guard = shard.lock().unwrap();
        mmu_guard.read(pa, size)
    }

    fn write(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<(), crate::VmError> {
        let shard = self.sharded_mmu.get_shard(pa);
        let mut mmu_guard = shard.lock().unwrap();
        mmu_guard.write(pa, val, size)
    }

    fn map_mmio(&mut self, base: GuestAddr, size: u64, device: Box<dyn crate::MmioDevice>) {
        // 选择第一个分片进行MMIO映射（简化实现）
        if let Some(shard) = self.sharded_mmu.all_shards().first() {
            let mut mmu_guard = shard.lock().unwrap();
            mmu_guard.map_mmio(base, size, device);
        }
    }

    fn flush_tlb(&mut self) {
        self.sharded_mmu.flush_all_tlbs();
    }

    fn memory_size(&self) -> usize {
        // 返回第一个分片的内存大小作为近似值
        if let Some(shard) = self.sharded_mmu.all_shards().first() {
            let mmu_guard = shard.lock().unwrap();
            mmu_guard.memory_size()
        } else {
            0
        }
    }

    fn dump_memory(&self) -> Vec<u8> {
        // 从第一个分片转储内存（简化实现）
        if let Some(shard) = self.sharded_mmu.all_shards().first() {
            let mmu_guard = shard.lock().unwrap();
            mmu_guard.dump_memory()
        } else {
            Vec::new()
        }
    }

    fn restore_memory(&mut self, data: &[u8]) -> Result<(), String> {
        // 恢复到第一个分片（简化实现）
        if let Some(shard) = self.sharded_mmu.all_shards().first() {
            let mut mmu_guard = shard.lock().unwrap();
            mmu_guard.restore_memory(data)
        } else {
            Err("No shards available".to_string())
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_balancer_least_loaded() {
        let mut lb = VcpuLoadBalancer::new(4, LoadBalancePolicy::LeastLoaded);

        lb.update_load(0, 100);
        lb.update_load(1, 50);
        lb.update_load(2, 200);
        lb.update_load(3, 75);

        // 应该选择负载最少的 vCPU (1)
        assert_eq!(lb.select_vcpu(), 1);
    }

    #[test]
    fn test_load_stats() {
        let mut lb = VcpuLoadBalancer::new(4, LoadBalancePolicy::LeastLoaded);

        lb.update_load(0, 100);
        lb.update_load(1, 50);
        lb.update_load(2, 200);
        lb.update_load(3, 150);

        let (min, max, avg) = lb.get_stats();

        assert_eq!(min, 50);
        assert_eq!(max, 200);
        assert!((avg - 125.0).abs() < 0.1);
    }
}
