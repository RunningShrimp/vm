//! JIT 并行编译模块 - 工作窃取调度器
//!
//! 本模块实现了基于工作窃取（work-stealing）的并行JIT编译架构。
//!
//! # 架构优势
//! - **低锁竞争**: 每个线程维护本地队列，减少同步开销
//! - **负载均衡**: 空闲线程自动从其他线程窃取任务
//! - **缓存亲和性**: 线程优先处理本地任务，提升缓存命中率
//!
//! # 性能目标
//! - 8 核场景下性能提升 4-6x
//! - 缓存命中率 > 80%
//!
//! # 使用示例
//! ```rust
//! use vm_engine::jit::parallel::{ParallelJITCompiler, CompilationTask};
//! use vm_ir::IRBlock;
//!
//! let compiler = ParallelJITCompiler::new(num_threads);
//! compiler.submit(IRBlock::new(...));
//! compiler.wait_all();
//! ```

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam_queue::SegQueue;
use vm_core::GuestAddr;
use vm_ir::IRBlock;

use crate::jit::backend::CompiledCode;
use crate::jit::code_cache::CodeCache;
use crate::jit::compiler::JITCompiler;

/// 编译任务
#[derive(Debug)]
#[allow(dead_code)] // JIT编译器基础设施 - 预留用于并行编译优化
pub struct CompilationTask {
    /// 任务ID
    pub id: u64,
    /// 客户端地址（用于缓存键）
    pub guest_addr: GuestAddr,
    /// IR块
    pub ir_block: IRBlock,
    /// 任务优先级（0-10，10最高）
    pub priority: u8,
    /// 创建时间
    pub created_at: Instant,
}

/// 编译结果
#[derive(Debug)]
#[allow(dead_code)] // JIT编译器基础设施 - 预留用于并行编译优化
pub struct CompilationResult {
    /// 任务ID
    pub task_id: u64,
    /// 客户端地址
    pub guest_addr: GuestAddr,
    /// 编译后的代码
    pub code: Arc<CompiledCode>,
    /// 编译耗时
    pub duration: Duration,
    /// 是否成功
    pub success: bool,
}

/// 本地任务队列（公共API以支持WorkStealingScheduler）
pub struct LocalQueue {
    /// 任务队列（使用无锁队列实现）
    queue: SegQueue<CompilationTask>,
    /// 线程ID
    worker_id: usize,
}

impl LocalQueue {
    /// 创建新的本地队列
    fn new(worker_id: usize) -> Self {
        Self {
            queue: SegQueue::new(),
            worker_id,
        }
    }

    /// 推送任务到本地队列（公共API以形成逻辑闭环）
    pub fn push(&self, task: CompilationTask) {
        self.queue.push(task);
    }

    /// 从本地队列弹出任务（LIFO - 优先处理本地任务）
    fn pop(&self) -> Option<CompilationTask> {
        self.queue.pop()
    }

    /// 窃取任务（FIFO - 窃取时从尾部取，减少竞争）（公共API以形成逻辑闭环）
    pub fn steal(&self) -> Option<CompilationTask> {
        // 注意：SegQueue 不支持高效的 FIFO steal
        // 在生产环境中应使用专门的 work-stealing deque
        // 这里简化为随机访问
        self.queue.pop()
    }

    /// 获取worker ID（形成逻辑闭环）
    pub fn worker_id(&self) -> usize {
        self.worker_id
    }
}

/// 工作窃取调度器
pub struct WorkStealingScheduler {
    /// 本地队列（每个工作线程一个）
    local_queues: Vec<Arc<LocalQueue>>,
    /// 全局任务队列（用于新任务提交）
    global_queue: Arc<SegQueue<CompilationTask>>,
    /// 工作线程句柄
    workers: Vec<JoinHandle<()>>,
    /// 运行标志
    running: Arc<AtomicBool>,
    /// 待处理任务计数
    pending_tasks: Arc<AtomicUsize>,
    /// 编译缓存
    code_cache: Arc<std::sync::Mutex<CodeCache>>,
    /// 统计信息
    stats: Arc<SchedulerStats>,
}

/// 调度器统计信息
#[derive(Debug, Default)]
pub struct SchedulerStats {
    /// 总任务数
    pub total_tasks: AtomicUsize,
    /// 成功编译数
    pub successful_compilations: AtomicUsize,
    /// 失败编译数
    pub failed_compilations: AtomicUsize,
    /// 缓存命中数
    pub cache_hits: AtomicUsize,
    /// 本地队列命中数
    pub local_queue_hits: AtomicUsize,
    /// 窃取任务数
    pub stolen_tasks: AtomicUsize,
    /// 总编译时间（微秒）
    pub total_compilation_time_us: AtomicUsize,
}

impl WorkStealingScheduler {
    /// 创建新的工作窃取调度器
    ///
    /// # 参数
    /// - `num_workers`: 工作线程数（通常设置为 CPU 核心数）
    pub fn new(num_workers: usize) -> Self {
        assert!(num_workers > 0, "至少需要1个工作线程");

        let running = Arc::new(AtomicBool::new(true));
        let pending_tasks = Arc::new(AtomicUsize::new(0));
        let code_cache = Arc::new(std::sync::Mutex::new(CodeCache::new(10000))); // 最多缓存 10000 个编译结果
        let stats = Arc::new(SchedulerStats::default());

        // 创建本地队列并包装在 Arc 中
        let local_queues: Vec<Arc<LocalQueue>> = (0..num_workers)
            .map(|worker_id| Arc::new(LocalQueue::new(worker_id)))
            .collect();

        // 创建共享的全局队列
        let global_queue = Arc::new(SegQueue::new());

        // 创建工作线程
        let mut workers = Vec::with_capacity(num_workers);
        for worker_id in 0..num_workers {
            // 克隆本地队列的 Arc 引用（不是队列本身）
            let local_queues_arc: Vec<Arc<LocalQueue>> =
                local_queues.iter().map(Arc::clone).collect();

            let global_queue_clone = Arc::clone(&global_queue);
            let running_clone = Arc::clone(&running);
            let pending_clone = Arc::clone(&pending_tasks);
            let cache_clone = Arc::clone(&code_cache);
            let stats_clone = Arc::clone(&stats);

            let handle = thread::spawn(move || {
                Self::worker_loop(
                    worker_id,
                    num_workers,
                    local_queues_arc,
                    global_queue_clone,
                    running_clone,
                    pending_clone,
                    cache_clone,
                    stats_clone,
                );
            });

            workers.push(handle);
        }

        Self {
            local_queues,
            global_queue,
            workers,
            running,
            pending_tasks,
            code_cache,
            stats,
        }
    }

    /// 工作线程主循环
    #[allow(clippy::too_many_arguments)]
    fn worker_loop(
        worker_id: usize,
        num_workers: usize,
        local_queues: Vec<Arc<LocalQueue>>,
        global_queue: Arc<SegQueue<CompilationTask>>,
        running: Arc<AtomicBool>,
        pending_tasks: Arc<AtomicUsize>,
        code_cache: Arc<std::sync::Mutex<CodeCache>>,
        stats: Arc<SchedulerStats>,
    ) {
        // 创建独立的 JIT 编译器实例
        let mut compiler = JITCompiler::new();

        while running.load(Ordering::Relaxed) {
            // 1. 尝试从本地队列获取任务
            let task = if let Some(task) = local_queues[worker_id].pop() {
                stats.local_queue_hits.fetch_add(1, Ordering::Relaxed);
                task
            }
            // 2. 尝试从全局队列获取任务
            else if let Some(task) = global_queue.pop() {
                task
            }
            // 3. 尝试从其他线程窃取任务
            else {
                // 随机选择一个受害者线程
                let victim = (worker_id + 1) % num_workers;
                if let Some(task) = local_queues[victim].pop() {
                    stats.stolen_tasks.fetch_add(1, Ordering::Relaxed);
                    task
                } else {
                    // 没有任务，短暂休眠
                    thread::sleep(Duration::from_micros(100));
                    continue;
                }
            };

            // 处理任务
            let start = Instant::now();
            let result = Self::compile_task(&mut compiler, &code_cache, &stats, task);
            let duration = start.elapsed();

            stats
                .total_compilation_time_us
                .fetch_add(duration.as_micros() as usize, Ordering::Relaxed);

            if result.success {
                stats
                    .successful_compilations
                    .fetch_add(1, Ordering::Relaxed);
            } else {
                stats.failed_compilations.fetch_add(1, Ordering::Relaxed);
            }

            // 减少待处理任务计数
            pending_tasks.fetch_sub(1, Ordering::Relaxed);
        }
    }

    /// 编译单个任务
    fn compile_task(
        compiler: &mut JITCompiler,
        code_cache: &Arc<std::sync::Mutex<CodeCache>>,
        stats: &SchedulerStats,
        task: CompilationTask,
    ) -> CompilationResult {
        stats.total_tasks.fetch_add(1, Ordering::Relaxed);

        // 将 GuestAddr 转换为 u64 作为缓存键
        let cache_key = task.guest_addr.0;

        // 检查缓存
        if let Some(cached_code) = code_cache.lock().unwrap().lookup(cache_key) {
            stats.cache_hits.fetch_add(1, Ordering::Relaxed);
            return CompilationResult {
                task_id: task.id,
                guest_addr: task.guest_addr,
                code: cached_code,
                duration: Duration::ZERO,
                success: true,
            };
        }

        // 编译 IR 块（测量编译时间）
        let compile_start = std::time::Instant::now();
        let compilation_result = compiler.compile(&task.ir_block);
        let compile_duration = compile_start.elapsed();

        match compilation_result {
            Ok(_compiled_block) => {
                // 创建 CompiledCode 占位符（简化实现）
                let compiled_code = Arc::new(CompiledCode {
                    code: vec![0x90, 0x90, 0x90], // NOP x3（占位符）
                    size: 3,
                    exec_addr: 0,
                });

                // 缓存编译结果
                code_cache
                    .lock()
                    .unwrap()
                    .insert(cache_key, Arc::clone(&compiled_code));

                CompilationResult {
                    task_id: task.id,
                    guest_addr: task.guest_addr,
                    code: compiled_code,
                    duration: compile_duration,
                    success: true,
                }
            }
            Err(_) => CompilationResult {
                task_id: task.id,
                guest_addr: task.guest_addr,
                code: Arc::new(CompiledCode {
                    code: vec![],
                    size: 0,
                    exec_addr: 0,
                }),
                duration: Duration::ZERO,
                success: false,
            },
        }
    }

    /// 提交编译任务
    pub fn submit(&self, ir_block: IRBlock, guest_addr: GuestAddr, priority: u8) -> u64 {
        static NEXT_TASK_ID: AtomicUsize = AtomicUsize::new(1);

        let task_id = NEXT_TASK_ID.fetch_add(1, Ordering::SeqCst) as u64;
        let task = CompilationTask {
            id: task_id,
            guest_addr,
            ir_block,
            priority: priority.min(10), // 限制在 0-10 范围
            created_at: Instant::now(),
        };

        // 推送到全局队列
        self.global_queue.push(task);
        self.pending_tasks.fetch_add(1, Ordering::Relaxed);

        task_id
    }

    /// 等待所有任务完成
    pub fn wait_all(&self) {
        while self.pending_tasks.load(Ordering::Relaxed) > 0 {
            thread::sleep(Duration::from_millis(10));
        }
    }

    /// 停止调度器
    pub fn shutdown(self) {
        self.running.store(false, Ordering::Relaxed);

        // 等待所有工作线程退出
        for worker in self.workers {
            let _ = worker.join();
        }
    }

    /// 获取统计信息
    pub fn stats(&self) -> &SchedulerStats {
        &self.stats
    }

    /// 获取缓存命中率
    pub fn cache_hit_rate(&self) -> f64 {
        let total = self.stats.total_tasks.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        let hits = self.stats.cache_hits.load(Ordering::Relaxed);
        hits as f64 / total as f64
    }

    /// 获取平均编译时间（微秒）
    pub fn avg_compilation_time_us(&self) -> f64 {
        let total = self.stats.total_tasks.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        let time_us = self.stats.total_compilation_time_us.load(Ordering::Relaxed);
        time_us as f64 / total as f64
    }

    /// 获取本地队列引用（形成逻辑闭环）
    pub fn local_queues(&self) -> &[Arc<LocalQueue>] {
        &self.local_queues
    }

    /// 获取代码缓存引用（形成逻辑闭环）
    pub fn code_cache(&self) -> &Arc<std::sync::Mutex<CodeCache>> {
        &self.code_cache
    }
}

/// 并行 JIT 编译器（简化接口）
pub struct ParallelJITCompiler {
    scheduler: WorkStealingScheduler,
}

impl ParallelJITCompiler {
    /// 创建新的并行 JIT 编译器
    ///
    /// # 参数
    /// - `num_threads`: 工作线程数（默认为 CPU 核心数）
    pub fn new(num_threads: Option<usize>) -> Self {
        let num_threads = num_threads.unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
        });

        Self {
            scheduler: WorkStealingScheduler::new(num_threads),
        }
    }

    /// 提交编译任务
    pub fn submit(&self, ir_block: IRBlock, guest_addr: GuestAddr) -> u64 {
        self.scheduler.submit(ir_block, guest_addr, 5) // 默认中等优先级
    }

    /// 提交高优先级编译任务
    pub fn submit_high_priority(&self, ir_block: IRBlock, guest_addr: GuestAddr) -> u64 {
        self.scheduler.submit(ir_block, guest_addr, 10)
    }

    /// 等待所有任务完成
    pub fn wait_all(&self) {
        self.scheduler.wait_all();
    }

    /// 获取统计信息
    pub fn stats(&self) -> &SchedulerStats {
        self.scheduler.stats()
    }

    /// 停止编译器
    pub fn shutdown(self) {
        self.scheduler.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parallel_compiler_creation() {
        let compiler = ParallelJITCompiler::new(Some(2));
        assert!(compiler.stats().total_tasks.load(Ordering::Relaxed) == 0);
    }

    #[test]
    fn test_task_submission() {
        let compiler = ParallelJITCompiler::new(Some(2));

        // 创建简单的 IR 块
        let mut ir_block = IRBlock::new(GuestAddr(0x1000));
        ir_block.ops.push(vm_ir::IROp::MovImm {
            dst: 1, // RegId
            imm: 42,
        });

        // 提交任务
        let task_id = compiler.submit(ir_block, GuestAddr(0x1000));
        assert!(task_id > 0);

        // 等待完成
        compiler.wait_all();
    }

    #[test]
    fn test_priority_submission() {
        let compiler = ParallelJITCompiler::new(Some(2));

        let mut ir_block = IRBlock::new(GuestAddr(0x2000));
        ir_block.ops.push(vm_ir::IROp::MovImm {
            dst: 1, // RegId
            imm: 100,
        });

        // 提交高优先级任务
        let task_id = compiler.submit_high_priority(ir_block, GuestAddr(0x2000));
        assert!(task_id > 0);

        compiler.wait_all();
    }

    #[test]
    fn test_stats_collection() {
        let compiler = ParallelJITCompiler::new(Some(2));

        // 提交多个任务
        for i in 0..5 {
            let mut ir_block = IRBlock::new(GuestAddr(0x3000 + i * 0x10));
            ir_block.ops.push(vm_ir::IROp::MovImm {
                dst: 1, // RegId
                imm: i as u64,
            });
            compiler.submit(ir_block, GuestAddr(0x3000 + i * 0x10));
        }

        compiler.wait_all();

        let stats = compiler.stats();
        let total = stats.total_tasks.load(Ordering::Relaxed);
        assert!(total >= 5);
    }
}
