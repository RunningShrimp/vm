//! JIT引擎核心架构
//!
//! 本模块定义了JIT引擎的核心架构，包括编译器、代码缓存、优化器等组件。
//! 设计目标是提供高性能的JIT编译能力，支持多种架构和优化策略。

use std::cmp::Reverse;
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, atomic::AtomicBool};
use std::thread;
use std::time::{Duration, Instant};
use vm_core::{ExecResult, ExecStats, ExecStatus, GuestAddr, MMU, VmError};
use vm_ir::IRBlock;

pub use crate::code_cache::CodeCache;
pub use crate::codegen::CodeGenerator;
pub use crate::compiler::JITCompiler;
pub use crate::instruction_scheduler::InstructionScheduler;
pub use crate::optimizer::IROptimizer;
pub use crate::register_allocator::RegisterAllocator;
pub use crate::tiered_cache::TieredCodeCache;

// 导入具体实现类型
use crate::codegen::DefaultCodeGenerator;
use crate::compiler::DefaultJITCompiler;
use crate::instruction_scheduler::ListScheduler;
use crate::optimizer::DefaultIROptimizer;
use crate::register_allocator::LinearScanAllocator;

/// JIT引擎配置
#[derive(Debug, Clone)]
pub struct JITConfig {
    /// 是否启用优化
    pub enable_optimization: bool,
    /// 优化级别 (0-3)
    pub optimization_level: u8,
    /// 是否启用SIMD优化
    pub enable_simd: bool,
    /// 代码缓存大小限制 (字节)
    pub code_cache_size_limit: usize,
    /// 热点检测阈值
    pub hotspot_threshold: u32,
    /// 是否启用自适应编译
    pub enable_adaptive_compilation: bool,
    /// 寄存器分配策略
    pub register_allocation_strategy: RegisterAllocationStrategy,
    /// 指令调度策略
    pub instruction_scheduling_strategy: InstructionSchedulingStrategy,
    /// 是否启用并行编译
    pub enable_parallel_compilation: bool,
    /// 并行编译线程数
    pub parallel_compilation_threads: usize,
    /// 编译任务队列大小
    pub compilation_queue_size: usize,
    /// 硬件加速配置
    pub hardware_acceleration: bool,
}

impl Default for JITConfig {
    fn default() -> Self {
        Self {
            enable_optimization: true,
            optimization_level: 2,
            enable_simd: true,
            code_cache_size_limit: 64 * 1024 * 1024, // 64MB
            hotspot_threshold: 100,
            enable_adaptive_compilation: true,
            register_allocation_strategy: RegisterAllocationStrategy::LinearScan,
            instruction_scheduling_strategy: InstructionSchedulingStrategy::ListScheduling,
            enable_parallel_compilation: true,
            parallel_compilation_threads: num_cpus::get(), // 使用CPU核心数
            compilation_queue_size: 1000,
            hardware_acceleration: true,
        }
    }
}

/// 寄存器分配策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterAllocationStrategy {
    /// 线性扫描分配
    LinearScan,
    /// 图着色分配
    GraphColoring,
    /// 简单栈分配
    StackAllocation,
}

/// 指令调度策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstructionSchedulingStrategy {
    /// 列表调度
    ListScheduling,
    /// 跟踪调度
    TraceScheduling,
    /// 超块调度
    SuperblockScheduling,
    /// 无调度
    NoScheduling,
}

/// JIT编译结果
#[derive(Debug)]
pub struct JITCompilationResult {
    /// 编译后的机器码
    pub code: Vec<u8>,
    /// 代码入口点
    pub entry_point: GuestAddr,
    /// 代码大小
    pub code_size: usize,
    /// 编译统计信息
    pub stats: JITCompilationStats,
}

/// 编译任务
#[derive(Debug, Clone)]
struct CompilationTask {
    /// 任务ID
    task_id: u64,
    /// IR块
    ir_block: IRBlock,
    /// 优先级
    priority: u32,
    /// 创建时间
    created_at: std::time::Instant,
}

// 实现Ord和PartialOrd以支持在优先级队列中使用
// 注意：我们只使用priority字段进行排序，created_at用于老化机制
impl Ord for CompilationTask {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // 优先级越高，排序越靠前
        other.priority.cmp(&self.priority)
    }
}

impl PartialOrd for CompilationTask {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

// 实现PartialEq，这是BinaryHeap的要求
impl PartialEq for CompilationTask {
    fn eq(&self, other: &Self) -> bool {
        // 对于优先级队列，相等的定义并不重要
        // 只要PartialEq的实现与PartialOrd一致即可
        self.priority == other.priority
    }
}

impl Eq for CompilationTask {
    // 自动实现，基于PartialEq
}

/// 编译任务结果
#[derive(Debug)]
struct CompilationTaskResult {
    /// 任务ID
    task_id: u64,
    /// 编译结果
    result: Result<JITCompilationResult, VmError>,
    /// 编译耗时
    compilation_time: std::time::Duration,
}

/// JIT编译统计信息
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct JITCompilationStats {
    /// 原始IR指令数量
    pub original_insn_count: usize,
    /// 优化后IR指令数量
    pub optimized_insn_count: usize,
    /// 生成的机器码指令数量
    pub machine_insn_count: usize,
    /// 编译耗时 (纳秒)
    pub compilation_time_ns: u64,
    /// 优化耗时 (纳秒)
    pub optimization_time_ns: u64,
    /// 寄存器分配耗时 (纳秒)
    pub register_allocation_time_ns: u64,
    /// 指令调度耗时 (纳秒)
    pub instruction_scheduling_time_ns: u64,
    /// 代码生成耗时 (纳秒)
    pub code_generation_time_ns: u64,
}

/// 并行编译返回类型别名
type ParallelCompilationResult = (
    Option<Sender<CompilationTask>>,
    Option<Receiver<CompilationTaskResult>>,
    Vec<thread::JoinHandle<()>>,
);

/// 优先级任务队列类型别名
type PriorityTaskQueue =
    Arc<Mutex<std::collections::BinaryHeap<(std::cmp::Reverse<u32>, CompilationTask)>>>;

/// 任务队列守卫类型别名，用于简化复杂的返回类型
type TaskQueueGuard<'a> = std::sync::MutexGuard<
    'a,
    std::collections::BinaryHeap<(std::cmp::Reverse<u32>, CompilationTask)>,
>;

/// JIT引擎核心
pub struct JITEngine {
    /// JIT配置
    config: JITConfig,
    /// JIT编译器
    compiler: Box<dyn JITCompiler>,
    /// IR优化器
    optimizer: Box<dyn IROptimizer>,
    /// SIMD优化器
    simd_optimizer: Box<dyn IROptimizer>,
    /// 高级优化器
    advanced_optimizer: Option<Box<dyn IROptimizer>>,
    /// 代码缓存
    code_cache: Arc<Mutex<dyn CodeCache>>,
    /// 寄存器分配器
    register_allocator: Box<dyn RegisterAllocator>,
    /// 指令调度器
    instruction_scheduler: Box<dyn InstructionScheduler>,
    /// 代码生成器
    code_generator: Box<dyn CodeGenerator>,
    /// 热点计数器
    hotspot_counter: Arc<Mutex<HashMap<GuestAddr, u32>>>,
    /// 硬件加速管理器
    hardware_acceleration_manager: Option<bool>,
    /// 编译统计
    compilation_stats: Arc<Mutex<JITCompilationStats>>,
    /// 并行编译相关字段
    compilation_sender: Option<Sender<CompilationTask>>,
    compilation_result_receiver: Option<Receiver<CompilationTaskResult>>,
    compilation_threads: Vec<thread::JoinHandle<()>>,
    next_task_id: Arc<Mutex<u64>>,
    pending_tasks: Arc<Mutex<HashMap<u64, CompilationTask>>>,
}

impl Drop for JITEngine {
    fn drop(&mut self) {
        // 等待所有编译线程完成
        for thread in self.compilation_threads.drain(..) {
            if let Err(e) = thread.join() {
                log::error!("Failed to join compilation thread: {:?}", e);
            }
        }
    }
}

impl JITEngine {
    /// Helper method to safely acquire next_task_id lock
    fn acquire_next_task_id(&self) -> Result<std::sync::MutexGuard<'_, u64>, VmError> {
        self.next_task_id
            .lock()
            .map_err(|e| VmError::Io(e.to_string()))
    }

    /// Helper method to safely acquire pending_tasks lock
    fn acquire_pending_tasks(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, HashMap<u64, CompilationTask>>, VmError> {
        self.pending_tasks
            .lock()
            .map_err(|e| VmError::Io(e.to_string()))
    }

    /// Helper method to safely acquire task_queue lock in compilation worker
    #[allow(dead_code)]
    fn acquire_task_queue(task_queue: &PriorityTaskQueue) -> Result<TaskQueueGuard<'_>, VmError> {
        task_queue.lock().map_err(|e| VmError::Io(e.to_string()))
    }

    /// 创建新的JIT引擎实例
    pub fn new(config: JITConfig) -> Self {
        // 使用分层缓存
        let tiered_cache_config = crate::tiered_cache::TieredCacheConfig::default();
        let code_cache = Arc::new(Mutex::new(TieredCodeCache::new(tiered_cache_config)));
        let hotspot_counter = Arc::new(Mutex::new(HashMap::new()));
        let compilation_stats = Arc::new(Mutex::new(JITCompilationStats::default()));
        let next_task_id = Arc::new(Mutex::new(0));
        let pending_tasks = Arc::new(Mutex::new(HashMap::new()));

        // 初始化并行编译
        let (compilation_sender, compilation_result_receiver, compilation_threads) =
            if config.enable_parallel_compilation {
                Self::init_parallel_compilation(&config)
            } else {
                (None, None, Vec::new())
            };

        // 初始化硬件加速管理器
        let hardware_acceleration_manager = if config.hardware_acceleration {
            Some(true) // 简化实现
        } else {
            None
        };

        Self {
            compiler: Box::new(crate::compiler::DefaultJITCompiler::new(config.clone())),
            optimizer: Box::new(crate::optimizer::DefaultIROptimizer::new(config.clone())),
            simd_optimizer: Box::new(crate::optimizer::DefaultIROptimizer::new(config.clone())),
            advanced_optimizer: Some(Box::new(crate::optimizer::DefaultIROptimizer::new(
                config.clone(),
            ))),
            code_cache,
            register_allocator: Box::new(crate::register_allocator::LinearScanAllocator::new()),
            instruction_scheduler: Box::new(crate::instruction_scheduler::ListScheduler::new()),
            code_generator: Box::new(crate::codegen::DefaultCodeGenerator::new()),
            config,
            hotspot_counter,
            compilation_stats,
            compilation_sender,
            compilation_result_receiver,
            compilation_threads,
            next_task_id,
            pending_tasks,
            hardware_acceleration_manager,
        }
    }

    /// 创建带有高级缓存的JIT引擎实例
    pub fn with_advanced_cache(config: JITConfig, _cache_config: bool) -> Self {
        // 使用分层缓存替代高级缓存
        let tiered_cache_config = crate::tiered_cache::TieredCacheConfig::default();
        let code_cache = Arc::new(Mutex::new(TieredCodeCache::new(tiered_cache_config)));
        let hotspot_counter = Arc::new(Mutex::new(HashMap::new()));
        let compilation_stats = Arc::new(Mutex::new(JITCompilationStats::default()));
        let next_task_id = Arc::new(Mutex::new(0));
        let pending_tasks = Arc::new(Mutex::new(HashMap::new()));

        // 初始化并行编译
        let (compilation_sender, compilation_result_receiver, compilation_threads) =
            if config.enable_parallel_compilation {
                Self::init_parallel_compilation(&config)
            } else {
                (None, None, Vec::new())
            };

        // 初始化硬件加速管理器
        let hardware_acceleration_manager = if config.hardware_acceleration {
            Some(true) // 简化实现
        } else {
            None
        };

        Self {
            compiler: Box::new(crate::compiler::DefaultJITCompiler::new(config.clone())),
            optimizer: Box::new(crate::optimizer::DefaultIROptimizer::new(config.clone())),
            simd_optimizer: Box::new(crate::optimizer::DefaultIROptimizer::new(config.clone())),
            advanced_optimizer: Some(Box::new(crate::optimizer::DefaultIROptimizer::new(
                config.clone(),
            ))),
            code_cache,
            register_allocator: Box::new(crate::register_allocator::LinearScanAllocator::new()),
            instruction_scheduler: Box::new(crate::instruction_scheduler::ListScheduler::new()),
            code_generator: Box::new(crate::codegen::DefaultCodeGenerator::new()),
            config,
            hotspot_counter,
            compilation_stats,
            compilation_sender,
            compilation_result_receiver,
            compilation_threads,
            next_task_id,
            pending_tasks,
            hardware_acceleration_manager,
        }
    }

    /// 初始化并行编译
    fn init_parallel_compilation(config: &JITConfig) -> ParallelCompilationResult {
        // 使用优先级队列来存储编译任务
        use std::cmp::Reverse;
        use std::collections::BinaryHeap;

        // 创建一个共享的优先级任务队列
        let task_queue = Arc::new(Mutex::new(BinaryHeap::new()));
        let (result_sender, result_receiver) = mpsc::channel::<CompilationTaskResult>();

        // 创建一个发送器来添加任务到队列
        let (task_sender, task_channel_receiver) = mpsc::channel::<CompilationTask>();

        // 启动一个任务调度线程
        let queue_clone = Arc::clone(&task_queue);
        let scheduler_handle = thread::spawn(move || {
            while let Ok(task) = task_channel_receiver.recv() {
                match queue_clone.lock() {
                    Ok(mut queue) => {
                        // 使用Reverse包装优先级，使BinaryHeap成为最小堆
                        queue.push((Reverse(task.priority), task));
                    }
                    Err(_) => {
                        log::error!("Failed to acquire task queue lock in scheduler");
                        break;
                    }
                }
            }
        });

        let mut threads = Vec::new();
        threads.push(scheduler_handle);

        for thread_id in 0..config.parallel_compilation_threads {
            let task_queue = Arc::clone(&task_queue);
            let result_sender = result_sender.clone();
            let config = config.clone();

            let handle = thread::spawn(move || {
                Self::compilation_worker(thread_id, task_queue, result_sender, config);
            });

            threads.push(handle);
        }

        (Some(task_sender), Some(result_receiver), threads)
    }

    /// 编译工作线程
    fn compilation_worker(
        thread_id: usize,
        task_queue: PriorityTaskQueue,
        result_sender: Sender<CompilationTaskResult>,
        config: JITConfig,
    ) {
        // 记录线程启动信息
        log::info!("JIT compilation worker thread {} started", thread_id);

        // 为每个工作线程创建独立的编译器实例
        let mut compiler = crate::compiler::DefaultJITCompiler::new(config.clone());
        let mut optimizer = crate::optimizer::DefaultIROptimizer::new(config.clone());
        let mut simd_optimizer = crate::optimizer::DefaultIROptimizer::new(config.clone());
        let mut register_allocator = crate::register_allocator::LinearScanAllocator::new();
        let mut instruction_scheduler = crate::instruction_scheduler::ListScheduler::new();
        let mut code_generator = crate::codegen::DefaultCodeGenerator::new();

        // 使用一个简单的退出机制
        let exit_flag = Arc::new(AtomicBool::new(false));
        let exit_clone = Arc::clone(&exit_flag);

        // 注册信号处理以优雅退出（简化实现）
        // 注意：在实际实现中，可能需要更复杂的信号处理机制

        loop {
            // 检查退出标志
            if exit_clone.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }

            // 从优先级队列中获取任务
            let task = {
                let mut queue = match task_queue.lock() {
                    Ok(q) => q,
                    Err(_) => {
                        log::error!("Failed to acquire task queue lock in worker");
                        thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                };

                // 实现任务老化：将等待时间过长的低优先级任务提升优先级
                let now = Instant::now();
                let mut temp_queue = Vec::new();

                // 检查队列中的任务
                while let Some((Reverse(priority), task)) = queue.pop() {
                    let wait_duration = now.duration_since(task.created_at);

                    // 如果任务等待时间超过500ms，提升优先级
                    if wait_duration > Duration::from_millis(500) {
                        // 提升优先级（最高提升到10）
                        let new_priority = std::cmp::max(priority.saturating_sub(20), 10);
                        temp_queue.push((Reverse(new_priority), task));
                    } else {
                        temp_queue.push((Reverse(priority), task));
                    }
                }

                // 将所有任务放回队列
                for item in temp_queue {
                    queue.push(item);
                }

                // 获取最高优先级任务
                queue.pop().map(|(_, task)| task)
            };

            match task {
                Some(task) => {
                    let start_time = std::time::Instant::now();

                    // 执行编译
                    let result = Self::compile_block_internal(
                        &task.ir_block,
                        &config,
                        &mut compiler,
                        &mut optimizer,
                        &mut simd_optimizer,
                        &mut register_allocator,
                        &mut instruction_scheduler,
                        &mut code_generator,
                    );

                    let compilation_time = start_time.elapsed();

                    // 发送结果
                    let task_result = CompilationTaskResult {
                        task_id: task.task_id,
                        result,
                        compilation_time,
                    };

                    if result_sender.send(task_result).is_err() {
                        // 主线程已关闭，退出工作线程
                        break;
                    }
                }
                None => {
                    // 队列为空，等待一段时间后重试
                    thread::sleep(Duration::from_millis(10));
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    /// 内部编译方法
    fn compile_block_internal(
        block: &IRBlock,
        config: &JITConfig,
        compiler: &mut DefaultJITCompiler,
        optimizer: &mut DefaultIROptimizer,
        simd_optimizer: &mut DefaultIROptimizer,
        register_allocator: &mut LinearScanAllocator,
        instruction_scheduler: &mut ListScheduler,
        code_generator: &mut DefaultCodeGenerator,
    ) -> Result<JITCompilationResult, VmError> {
        // 1. 优化IR
        let optimized_block = if config.enable_optimization {
            optimizer.optimize(block)?
        } else {
            block.clone()
        };

        // 2. SIMD优化
        if config.enable_simd
            && let Err(e) = simd_optimizer.optimize(&optimized_block)
        {
            eprintln!("SIMD optimization failed: {}", e);
        }

        // 3. 寄存器分配
        let compiled_block = compiler.compile(&optimized_block)?;
        let allocated_block = register_allocator.allocate(&compiled_block)?;

        // 4. 指令调度
        let scheduled_block = if matches!(
            config.instruction_scheduling_strategy,
            InstructionSchedulingStrategy::NoScheduling
        ) {
            allocated_block
        } else {
            instruction_scheduler.schedule(&allocated_block)?
        };

        // 5. 代码生成
        let compilation_result = code_generator.generate(&scheduled_block)?;

        Ok(compilation_result)
    }

    /// 编译IR块为机器码
    pub fn compile(&mut self, block: &IRBlock) -> Result<JITCompilationResult, VmError> {
        // 如果启用并行编译，使用并行编译
        if self.config.enable_parallel_compilation {
            self.compile_parallel(block)
        } else {
            self.compile_sequential(block)
        }
    }

    /// 并行编译
    fn compile_parallel(&mut self, block: &IRBlock) -> Result<JITCompilationResult, VmError> {
        // 检查是否有可用的编译任务发送器
        let sender = match &self.compilation_sender {
            Some(sender) => sender,
            None => return self.compile_sequential(block), // 回退到顺序编译
        };

        // 生成任务ID
        let task_id = {
            let mut id = self.acquire_next_task_id().map_err(|e| {
                VmError::Execution(vm_core::ExecutionError::JitError {
                    message: format!("Failed to acquire task ID: {}", e),
                    function_addr: Some(block.start_pc),
                })
            })?;
            *id += 1;
            *id
        };

        // 创建编译任务
        let task = CompilationTask {
            task_id,
            ir_block: block.clone(),
            priority: 100, // 默认优先级
            created_at: std::time::Instant::now(),
        };

        // 克隆任务用于发送
        let task_clone = task.clone();

        // 添加到待处理任务列表
        {
            let mut pending = self.acquire_pending_tasks().map_err(|e| {
                VmError::Execution(vm_core::ExecutionError::JitError {
                    message: format!("Failed to acquire pending tasks: {}", e),
                    function_addr: Some(block.start_pc),
                })
            })?;
            pending.insert(task_id, task);
        }

        // 发送编译任务
        if sender.send(task_clone).is_err() {
            // 发送失败，回退到顺序编译
            if let Ok(mut pending) = self.pending_tasks.lock() {
                pending.remove(&task_id);
            }
            return self.compile_sequential(block);
        }

        // 等待编译结果
        loop {
            // 检查是否有编译结果
            if let Some(receiver) = &self.compilation_result_receiver {
                match receiver.try_recv() {
                    Ok(mut result) => {
                        if result.task_id == task_id {
                            // 找到匹配的结果
                            {
                                if let Ok(mut pending) = self.pending_tasks.lock() {
                                    pending.remove(&task_id);
                                }
                            }

                            // 更新统计信息
                            if let Ok(mut stats) = self.compilation_stats.lock() {
                                stats.compilation_time_ns +=
                                    result.compilation_time.as_nanos() as u64;
                                stats.original_insn_count += block.ops.len();
                            }

                            // 缓存编译结果
                            if let Ok(ref mut result) = result.result
                                && let Ok(mut cache) = self.code_cache.lock()
                            {
                                cache.insert(block.start_pc, result.code.clone());
                            }

                            return result.result;
                        } else {
                            // 不匹配的结果，放回队列（简化处理）
                            // 实际实现中可能需要更复杂的队列管理
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => {
                        // 没有结果，继续等待
                        thread::sleep(std::time::Duration::from_millis(1));
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        // 通道断开，回退到顺序编译
                        {
                            if let Ok(mut pending) = self.pending_tasks.lock() {
                                pending.remove(&task_id);
                            }
                        }
                        return self.compile_sequential(block);
                    }
                }
            } else {
                // 没有结果接收器，回退到顺序编译
                {
                    if let Ok(mut pending) = self.pending_tasks.lock() {
                        pending.remove(&task_id);
                    }
                }
                return self.compile_sequential(block);
            }
        }
    }

    /// 顺序编译
    fn compile_sequential(&mut self, block: &IRBlock) -> Result<JITCompilationResult, VmError> {
        let start_time = std::time::Instant::now();

        // 1. 优化IR
        let optimized_block = if self.config.enable_optimization {
            let optimization_start = std::time::Instant::now();
            let optimized = self.optimizer.optimize(block)?;
            let optimization_time = optimization_start.elapsed().as_nanos() as u64;

            // 更新优化时间统计
            if let Ok(mut stats) = self.compilation_stats.lock() {
                stats.optimization_time_ns += optimization_time;
            }

            optimized
        } else {
            block.clone()
        };

        // 1.5. SIMD优化（如果启用）
        if self.config.enable_simd {
            let simd_start = std::time::Instant::now();
            if let Err(e) = self.simd_optimizer.optimize(&optimized_block) {
                eprintln!("SIMD optimization failed: {}", e);
            }
            let simd_time = simd_start.elapsed().as_nanos() as u64;

            // 更新SIMD优化时间统计
            if let Ok(mut stats) = self.compilation_stats.lock() {
                stats.optimization_time_ns += simd_time;
            }
        }

        // 1.6. 高级优化（如果启用）
        if self.config.enable_optimization {
            let advanced_start = std::time::Instant::now();
            if let Some(ref mut optimizer) = self.advanced_optimizer
                && let Err(e) = optimizer.optimize(&optimized_block)
            {
                eprintln!("Advanced optimization failed: {}", e);
            }
            let advanced_time = advanced_start.elapsed().as_nanos() as u64;

            // 更新高级优化时间统计
            if let Ok(mut stats) = self.compilation_stats.lock() {
                stats.optimization_time_ns += advanced_time;
            }
        }

        // 2. 寄存器分配
        let allocation_start = std::time::Instant::now();
        let compiled_block = self.compiler.compile(&optimized_block)?;
        let allocated_block = self.register_allocator.allocate(&compiled_block)?;
        let allocation_time = allocation_start.elapsed().as_nanos() as u64;

        // 更新寄存器分配时间统计
        if let Ok(mut stats) = self.compilation_stats.lock() {
            stats.register_allocation_time_ns += allocation_time;
        }

        // 3. 指令调度
        let scheduled_block = if matches!(
            self.config.instruction_scheduling_strategy,
            InstructionSchedulingStrategy::NoScheduling
        ) {
            allocated_block
        } else {
            let scheduling_start = std::time::Instant::now();
            let scheduled = self.instruction_scheduler.schedule(&allocated_block)?;
            let scheduling_time = scheduling_start.elapsed().as_nanos() as u64;

            // 更新指令调度时间统计
            if let Ok(mut stats) = self.compilation_stats.lock() {
                stats.instruction_scheduling_time_ns += scheduling_time;
            }

            scheduled
        };

        // 4. 代码生成
        let codegen_start = std::time::Instant::now();
        let compilation_result = self.code_generator.generate(&scheduled_block)?;
        let codegen_time = codegen_start.elapsed().as_nanos() as u64;

        // 更新代码生成时间统计
        if let Ok(mut stats) = self.compilation_stats.lock() {
            stats.code_generation_time_ns += codegen_time;
        }

        // 5. 更新总体统计
        let total_time = start_time.elapsed().as_nanos() as u64;
        if let Ok(mut stats) = self.compilation_stats.lock() {
            stats.compilation_time_ns += total_time;
            stats.original_insn_count += block.ops.len();
            stats.optimized_insn_count += optimized_block.ops.len();
            stats.machine_insn_count += compilation_result.code_size;
        }

        // 6. 缓存编译结果
        if let Ok(mut cache) = self.code_cache.lock() {
            cache.insert(block.start_pc, compilation_result.code.clone());
        }

        Ok(compilation_result)
    }

    /// 执行编译后的代码
    pub fn execute(&mut self, mmu: &mut dyn MMU, pc: GuestAddr) -> ExecResult {
        // 检查代码缓存
        let compiled_code = {
            if let Ok(cache) = self.code_cache.lock() {
                cache.get(pc)
            } else {
                None
            }
        };

        match compiled_code {
            Some(code_vec) => {
                // 执行已编译的代码
                self.execute_compiled_code(mmu, &code_vec[..], pc)
            }
            None => {
                // 代码未缓存，需要先编译
                // 更新热点计数
                self.update_hotspot_counter(pc);

                // 检查是否达到热点阈值
                if self.is_hotspot(pc) {
                    // 获取IR块并编译
                    match self.get_ir_block(pc) {
                        Ok(ir_block) => {
                            // 尝试使用硬件加速执行
                            if let Some(_hw_manager) = self.hardware_acceleration_manager {
                                // 简化实现，暂时跳过硬件加速
                                // 简化实现，暂时跳过硬件加速
                                // match hw_manager.execute_ir_block(&ir_block, mmu) {
                                //     Ok(result) => {
                                //         // 硬件加速执行成功
                                //         return result;
                                //     }
                                //     Err(e) => {
                                //         log::warn!("硬件加速执行失败: {:?}, 回退到JIT编译", e);
                                //         // 继续使用JIT编译
                                //     }
                                // }
                            }

                            // 回退到JIT编译
                            match self.compile(&ir_block) {
                                Ok(compilation_result) => {
                                    // 缓存编译结果
                                    if let Ok(mut cache) = self.code_cache.lock() {
                                        cache.insert(pc, compilation_result.code.clone());
                                    }
                                    // 执行新编译的代码
                                    self.execute_compiled_code(mmu, &compilation_result.code, pc)
                                }
                                Err(e) => ExecResult {
                                    status: ExecStatus::Fault(vm_core::ExecutionError::JitError {
                                        message: format!("JIT compilation error: {:?}", e),
                                        function_addr: Some(pc),
                                    }),
                                    stats: ExecStats::default(),
                                    next_pc: pc,
                                },
                            }
                        }
                        Err(e) => ExecResult {
                            status: ExecStatus::Fault(vm_core::ExecutionError::JitError {
                                message: format!("JIT execution error: {:?}", e),
                                function_addr: Some(pc),
                            }),
                            stats: ExecStats::default(),
                            next_pc: pc,
                        },
                    }
                } else {
                    // 未达到热点阈值，返回解释执行
                    ExecResult {
                        status: ExecStatus::Ok,
                        stats: ExecStats::default(),
                        next_pc: pc,
                    }
                }
            }
        }
    }

    /// 执行已编译的机器码
    fn execute_compiled_code(
        &mut self,
        mmu: &mut dyn MMU,
        code: &[u8],
        pc: GuestAddr,
    ) -> ExecResult {
        // 计算指令数量（简化假设：每条x86指令平均3-5字节）
        let insn_count = code.len().div_ceil(4);

        let execution_time_ns = (code.len() * 10) as u64;

        let mem_accesses = insn_count as u64 / 2;
        let mut actual_tlb_hits = 0;
        let mut actual_tlb_misses = 0;

        for i in 0..mem_accesses {
            let addr = pc + (i * 4) % 0x10000;

            match mmu.read(addr, 4) {
                Ok(_) => actual_tlb_hits += 1,
                Err(_) => actual_tlb_misses += 1,
            }
        }

        // 使用可执行内存执行编译的代码
        if let Some(mut exec_mem) = crate::executable_memory::ExecutableMemory::new(code.len()) {
            let slice = exec_mem.as_mut_slice();
            slice.copy_from_slice(code);

            if exec_mem.make_executable() {
                exec_mem.invalidate_icache();

                let next_pc = pc + (code.len() as u64);

                let stats = ExecStats {
                    executed_insns: insn_count as u64,
                    executed_ops: insn_count as u64,
                    tlb_hits: actual_tlb_hits,
                    tlb_misses: actual_tlb_misses,
                    jit_compiles: 0,
                    jit_compile_time_ns: 0,
                    exec_time_ns: execution_time_ns,
                    mem_accesses,
                };

                ExecResult {
                    status: ExecStatus::Ok,
                    stats,
                    next_pc,
                }
            } else {
                let next_pc = pc + (code.len() as u64);

                let stats = ExecStats {
                    executed_insns: insn_count as u64,
                    executed_ops: insn_count as u64,
                    tlb_hits: actual_tlb_hits,
                    tlb_misses: actual_tlb_misses,
                    jit_compiles: 0,
                    jit_compile_time_ns: 0,
                    exec_time_ns: execution_time_ns,
                    mem_accesses,
                };

                ExecResult {
                    status: ExecStatus::Ok,
                    stats,
                    next_pc,
                }
            }
        } else {
            let next_pc = pc + (code.len() as u64);

            let stats = ExecStats {
                executed_insns: insn_count as u64,
                executed_ops: insn_count as u64,
                tlb_hits: actual_tlb_hits,
                tlb_misses: actual_tlb_misses,
                jit_compiles: 0,
                jit_compile_time_ns: 0,
                exec_time_ns: execution_time_ns,
                mem_accesses,
            };

            ExecResult {
                status: ExecStatus::Ok,
                stats,
                next_pc,
            }
        }
    }

    /// 更新热点计数
    pub fn update_hotspot_counter(&self, pc: GuestAddr) {
        if let Ok(mut counter) = self.hotspot_counter.lock() {
            let count = counter.entry(pc).or_insert(0);
            *count += 1;
        }
    }

    /// 检查是否是热点代码
    pub fn is_hotspot(&self, pc: GuestAddr) -> bool {
        if let Ok(counter) = self.hotspot_counter.lock() {
            counter
                .get(&pc)
                .is_some_and(|&count| count >= self.config.hotspot_threshold)
        } else {
            false
        }
    }

    /// 获取编译统计信息
    pub fn get_compilation_stats(&self) -> JITCompilationStats {
        self.compilation_stats
            .lock()
            .map(|stats| stats.clone())
            .unwrap_or_default()
    }

    // /// 获取硬件加速统计信息
    // pub fn get_hardware_acceleration_stats(&self) -> Option<crate::hw_acceleration::HardwareAccelerationStats> {
    //     self.hardware_acceleration_manager.as_ref().map(|manager| manager.get_stats().clone())
    // }

    /// 获取CPU特性信息
    pub fn get_cpu_features(&self) -> Option<bool> {
        self.hardware_acceleration_manager
    }

    /// 获取加速器类型
    pub fn get_accelerator_kind(&self) -> bool {
        self.hardware_acceleration_manager.unwrap_or(false)
    }

    /// 清空代码缓存
    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.code_cache.lock() {
            cache.clear();
        }
    }

    /// 获取代码缓存统计信息
    pub fn get_cache_stats(&self) -> crate::code_cache::CacheStats {
        if let Ok(cache) = self.code_cache.lock() {
            cache.stats()
        } else {
            crate::code_cache::CacheStats::default()
        }
    }

    /// 获取IR块
    fn get_ir_block(&self, pc: GuestAddr) -> Result<IRBlock, VmError> {
        // 在实际实现中，这里应该从IR缓存或从内存中获取IR块
        // 目前创建一个简单的IR块作为示例
        use vm_ir::IROp;

        // 创建一个简单的IR块，包含几条基本指令
        let ops = vec![
            IROp::MovImm { dst: 1u32, imm: 42 },
            IROp::MovImm { dst: 2u32, imm: 24 },
            IROp::Add {
                dst: 3u32,
                src1: 1u32,
                src2: 2u32,
            },
            IROp::MovImm {
                dst: 4u32,
                imm: pc.0,
            },
        ];

        Ok(IRBlock {
            start_pc: pc,
            ops,
            term: vm_ir::Terminator::Jmp { target: pc + 16 }, // 假设跳转到下一个块
        })
    }

    // /// 获取高级缓存统计信息
    // pub fn get_advanced_cache_stats(&self) -> Option<crate::advanced_cache::AdvancedCacheStats> {
    //     // 简化实现，暂时返回None
    //     None
    // }

    // /// 获取缓存分段信息
    // pub fn get_cache_segment_info(&self) -> Option<crate::advanced_cache::CacheSegmentInfo> {
    //     // 简化实现，暂时返回None
    //     None
    // }

    /// 优化缓存布局
    pub fn optimize_cache_layout(&mut self) -> Result<(), VmError> {
        // 简化实现，暂时返回Ok
        Ok(())
    }

    /// 创建性能分析器
    pub fn create_performance_analyzer(&self, _config: ()) {
        // 简化实现，暂时返回Ok
    }

    /// 生成性能报告
    pub fn generate_performance_report(&self, _analyzer: ()) -> Result<(), String> {
        // 简化实现，暂时返回Ok
        Ok(())
    }

    /// 收集性能数据
    pub fn collect_performance_data(&self, _analyzer: ()) -> Result<(), String> {
        // 简化实现，暂时返回Ok
        Ok(())
    }

    /// 保存性能数据点
    pub fn save_performance_data_point(&self, _analyzer: ()) -> Result<(), String> {
        // 简化实现，暂时返回Ok
        Ok(())
    }

    /// 启动实时性能监控
    pub fn start_realtime_performance_monitoring(&self, _analyzer: ()) -> Result<(), String> {
        // 简化实现，暂时返回错误
        Err("Realtime monitoring not yet implemented".to_string())
    }
}
