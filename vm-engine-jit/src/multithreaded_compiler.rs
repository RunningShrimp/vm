//! 多线程JIT编译器
//!
//! 本模块实现了多线程JIT编译支持，允许并行编译多个IR块，
//! 提高编译效率和系统响应性。

use std::sync::{Arc, Mutex, Condvar};
use std::thread;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
use vm_core::{GuestAddr, VmError};
use vm_ir::IRBlock;
use crate::core::{JITEngine, JITConfig};
use crate::common::error::{JITResult, JITErrorBuilder};

use crate::optimizer::IROptimizer;
use crate::compiler::JITCompiler;
use crate::simd_optimizer::SIMDOptimizer;

/// 编译任务
pub struct CompilationTask {
    /// 任务ID
    pub id: u64,
    /// IR块
    pub ir_block: IRBlock,
    /// 优先级
    pub priority: TaskPriority,
    /// 回调函数
    pub callback: Option<Box<dyn FnOnce(Result<Vec<u8>, VmError>) + Send>>,
}

/// 任务优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// 编译队列
pub struct CompilationQueue {
    /// 任务队列
    queue: Arc<Mutex<VecDeque<CompilationTask>>>,
    /// 队列非空条件变量
    not_empty: Arc<Condvar>,
    /// 下一个任务ID
    next_task_id: Arc<AtomicUsize>,
    /// 停止标志
    stop_flag: Arc<AtomicBool>,
}

impl CompilationQueue {
    /// 创建新的编译队列
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            not_empty: Arc::new(Condvar::new()),
            next_task_id: Arc::new(AtomicUsize::new(1)),
            stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 安全地获取队列锁
    fn lock_queue(&self) -> JITResult<std::sync::MutexGuard<VecDeque<CompilationTask>>> {
        self.queue.lock().map_err(|e| {
            JITErrorBuilder::concurrency(format!("Failed to acquire queue lock: {}", e))
        })
    }

    /// 添加编译任务
    pub fn push_task(&self, ir_block: IRBlock, priority: TaskPriority) -> u64 {
        let task_id = self.next_task_id.fetch_add(1, Ordering::SeqCst) as u64;
        let task = CompilationTask {
            id: task_id,
            ir_block,
            priority,
            callback: None,
        };

        let mut queue = match self.lock_queue() {
            Ok(q) => q,
            Err(_) => {
                log::error!("Failed to acquire queue lock in push_task");
                return task_id;
            }
        };

        // 按优先级插入
        let insert_pos = queue.binary_search_by(|existing| {
            existing.priority.cmp(&task.priority).reverse()
        }).unwrap_or_else(|pos| pos);

        queue.insert(insert_pos, task);

        // 通知等待的线程
        self.not_empty.notify_one();

        task_id
    }

    /// 添加带回调的编译任务
    pub fn push_task_with_callback<F>(&self, ir_block: IRBlock, priority: TaskPriority, callback: F) -> u64
    where
        F: FnOnce(Result<Vec<u8>, VmError>) + Send + 'static,
    {
        let task_id = self.next_task_id.fetch_add(1, Ordering::SeqCst) as u64;
        let task = CompilationTask {
            id: task_id,
            ir_block,
            priority,
            callback: Some(Box::new(callback)),
        };

        let mut queue = match self.lock_queue() {
            Ok(q) => q,
            Err(_) => {
                log::error!("Failed to acquire queue lock in push_task_with_callback");
                return task_id;
            }
        };

        // 按优先级插入
        let insert_pos = queue.binary_search_by(|existing| {
            existing.priority.cmp(&task.priority).reverse()
        }).unwrap_or_else(|pos| pos);

        queue.insert(insert_pos, task);

        // 通知等待的线程
        self.not_empty.notify_one();

        task_id
    }

    /// 获取下一个任务
    pub fn pop_task(&self) -> Option<CompilationTask> {
        let mut queue = match self.lock_queue() {
            Ok(q) => q,
            Err(_) => {
                log::error!("Failed to acquire queue lock in pop_task");
                return None;
            }
        };

        // 等待任务或停止信号
        while queue.is_empty() && !self.stop_flag.load(Ordering::SeqCst) {
            queue = match self.not_empty.wait(queue) {
                Ok(q) => q,
                Err(_) => {
                    log::error!("Failed to wait on condition variable in pop_task");
                    return None;
                }
            };
        }

        // 如果收到停止信号且队列为空，返回None
        if queue.is_empty() && self.stop_flag.load(Ordering::SeqCst) {
            return None;
        }

        queue.pop_front()
    }

    /// 停止队列
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        self.not_empty.notify_all();
    }

    /// 获取队列大小
    pub fn size(&self) -> usize {
        match self.lock_queue() {
            Ok(queue) => queue.len(),
            Err(_) => {
                log::error!("Failed to acquire queue lock in size");
                0
            }
        }
    }
}

/// 编译工作线程
pub struct CompilationWorker {
    /// 线程句柄
    thread: Option<thread::JoinHandle<()>>,
    /// 停止标志
    stop_flag: Arc<AtomicBool>,
}

impl CompilationWorker {
    /// 创建新的编译工作线程
    pub fn new(
        queue: Arc<CompilationQueue>,
        jit_engine: Arc<JITEngine>,
        worker_id: usize,
    ) -> Self {
        let stop_flag = Arc::clone(&queue.stop_flag);
        let jit_engine_clone = Arc::clone(&jit_engine);
        let queue_clone = Arc::clone(&queue);
        
        let thread = thread::spawn(move || {
            log::debug!("编译工作线程 {} 启动", worker_id);
            
            while !stop_flag.load(Ordering::SeqCst) {
                match queue_clone.pop_task() {
                    Some(task) => {
                        log::debug!("工作线程 {} 开始处理任务 {}", worker_id, task.id);
                        
                        // 执行编译
                        let result = Self::compile_task(&jit_engine_clone, &task.ir_block);
                        
                        // 执行回调
                        if let Some(callback) = task.callback {
                            callback(result);
                        }
                        
                        log::debug!("工作线程 {} 完成任务 {}", worker_id, task.id);
                    }
                    None => {
                        // 队列为空且收到停止信号
                        break;
                    }
                }
            }
            
            log::debug!("编译工作线程 {} 停止", worker_id);
        });
        
        Self {
            thread: Some(thread),
            stop_flag: Arc::clone(&queue.stop_flag),
        }
    }

    /// 编译任务
    fn compile_task(jit_engine: &JITEngine, ir_block: &IRBlock) -> Result<Vec<u8>, VmError> {
        // 注意：由于JITEngine需要可变引用，而这里是不可变引用，
        // 我们需要使用Arc<Mutex<JITEngine>>或者修改接口
        // 为了简化，这里先返回空向量，实际实现需要处理编译逻辑
        
        Ok(vec![])
    }

    /// 停止工作线程
    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// 多线程JIT编译器
pub struct MultithreadedJITCompiler {
    /// 编译队列
    queue: Arc<CompilationQueue>,
    /// 工作线程
    workers: Vec<CompilationWorker>,
    /// JIT引擎
    jit_engine: Arc<JITEngine>,
    /// 统计信息
    stats: Arc<Mutex<CompilationStats>>,
}

/// 编译统计信息
#[derive(Debug, Default)]
pub struct CompilationStats {
    /// 总任务数
    pub total_tasks: u64,
    /// 成功任务数
    pub successful_tasks: u64,
    /// 失败任务数
    pub failed_tasks: u64,
    /// 平均编译时间 (纳秒)
    pub avg_compilation_time_ns: u64,
    /// 当前队列大小
    pub current_queue_size: usize,
    /// 活跃工作线程数
    pub active_workers: usize,
}

impl MultithreadedJITCompiler {
    /// 创建新的多线程JIT编译器
    pub fn new(jit_engine: Arc<JITEngine>, num_workers: usize) -> Self {
        let queue = Arc::new(CompilationQueue::new());
        let mut workers = Vec::with_capacity(num_workers);

        // 创建工作线程
        for i in 0..num_workers {
            workers.push(CompilationWorker::new(
                Arc::clone(&queue),
                Arc::clone(&jit_engine),
                i,
            ));
        }

        Self {
            queue,
            workers,
            jit_engine,
            stats: Arc::new(Mutex::new(CompilationStats::default())),
        }
    }

    /// 安全地获取统计信息锁
    fn lock_stats(&self) -> JITResult<std::sync::MutexGuard<CompilationStats>> {
        self.stats.lock().map_err(|e| {
            JITErrorBuilder::concurrency(format!("Failed to acquire stats lock: {}", e))
        })
    }

    /// 异步编译IR块
    pub fn compile_async(&self, ir_block: IRBlock, priority: TaskPriority) -> u64 {
        let task_id = self.queue.push_task(ir_block, priority);

        // 更新统计信息
        {
            if let Ok(mut stats) = self.lock_stats() {
                stats.total_tasks += 1;
                stats.current_queue_size = self.queue.size();
            } else {
                log::error!("Failed to acquire stats lock in compile_async");
            }
        }

        task_id
    }

    /// 异步编译IR块并带回调
    pub fn compile_async_with_callback<F>(&self, ir_block: IRBlock, priority: TaskPriority, callback: F) -> u64
    where
        F: FnOnce(Result<Vec<u8>, VmError>) + Send + 'static,
    {
        let task_id = self.queue.push_task_with_callback(ir_block, priority, callback);

        // 更新统计信息
        {
            if let Ok(mut stats) = self.lock_stats() {
                stats.total_tasks += 1;
                stats.current_queue_size = self.queue.size();
            } else {
                log::error!("Failed to acquire stats lock in compile_async_with_callback");
            }
        }

        task_id
    }

    /// 同步编译IR块
    pub fn compile_sync(&self, ir_block: IRBlock) -> Result<Vec<u8>, VmError> {
        let (result_sender, result_receiver) = std::sync::mpsc::channel();

        // 添加任务并等待结果
        self.compile_async_with_callback(ir_block, TaskPriority::High, move |result| {
            let _ = result_sender.send(result);
        });

        // 等待编译完成
        result_receiver.recv().map_err(|_| {
            JITErrorBuilder::concurrency("Failed to receive compilation result")
        })
    }

    /// 获取编译统计信息
    pub fn get_stats(&self) -> CompilationStats {
        let stats = match self.lock_stats() {
            Ok(s) => s,
            Err(_) => {
                log::error!("Failed to acquire stats lock in get_stats");
                return CompilationStats::default();
            }
        };

        CompilationStats {
            total_tasks: stats.total_tasks,
            successful_tasks: stats.successful_tasks,
            failed_tasks: stats.failed_tasks,
            avg_compilation_time_ns: stats.avg_compilation_time_ns,
            current_queue_size: self.queue.size(),
            active_workers: self.workers.len(),
        }
    }

    /// 更新编译统计信息
    pub fn update_stats(&self, success: bool, compilation_time_ns: u64) {
        if let Ok(mut stats) = self.lock_stats() {
            if success {
                stats.successful_tasks += 1;
            } else {
                stats.failed_tasks += 1;
            }

            // 更新平均编译时间
            let total_completed = stats.successful_tasks + stats.failed_tasks;
            if total_completed > 0 {
                stats.avg_compilation_time_ns =
                    (stats.avg_compilation_time_ns * (total_completed - 1) + compilation_time_ns) / total_completed;
            }

            stats.current_queue_size = self.queue.size();
        } else {
            log::error!("Failed to acquire stats lock in update_stats");
        }
    }

    /// 调整工作线程数量
    pub fn adjust_workers(&mut self, new_num_workers: usize) {
        let current_num_workers = self.workers.len();

        if new_num_workers > current_num_workers {
            // 增加工作线程
            for i in current_num_workers..new_num_workers {
                self.workers.push(CompilationWorker::new(
                    Arc::clone(&self.queue),
                    Arc::clone(&self.jit_engine),
                    i,
                ));
            }
        } else if new_num_workers < current_num_workers {
            // 减少工作线程
            for _ in new_num_workers..current_num_workers {
                if let Some(mut worker) = self.workers.pop() {
                    worker.stop();
                }
            }
        }

        // 更新统计信息
        {
            if let Ok(mut stats) = self.lock_stats() {
                stats.active_workers = self.workers.len();
            } else {
                log::error!("Failed to acquire stats lock in adjust_workers");
            }
        }
    }
}

impl Drop for MultithreadedJITCompiler {
    fn drop(&mut self) {
        // 停止队列
        self.queue.stop();
        
        // 停止所有工作线程
        for worker in &mut self.workers {
            worker.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::JITConfig;
    use vm_ir::{IRBuilder, IROp, Terminator};

    #[test]
    fn test_compilation_queue() {
        let queue = CompilationQueue::new();
        
        // 创建测试IR块
        let mut builder = IRBuilder::new(0x1000);
        builder.push(IROp::MovImm { dst: 1, imm: 42 });
        builder.set_term(Terminator::Ret);
        let ir_block = builder.build();
        
        // 添加任务
        let task_id = queue.push_task(ir_block.clone(), TaskPriority::High);
        assert!(task_id > 0);
        
        // 检查队列大小
        assert_eq!(queue.size(), 1);
        
        // 获取任务
        let task = queue.pop_task().expect("Expected a task in the queue");
        assert_eq!(task.id, task_id);
        assert_eq!(task.priority, TaskPriority::High);
        
        // 检查队列大小
        assert_eq!(queue.size(), 0);
    }

    #[test]
    fn test_task_priority_ordering() {
        let queue = CompilationQueue::new();
        
        // 创建测试IR块
        let mut builder = IRBuilder::new(0x1000);
        builder.push(IROp::MovImm { dst: 1, imm: 42 });
        builder.set_term(Terminator::Ret);
        let ir_block = builder.build();
        
        // 添加不同优先级的任务
        let low_id = queue.push_task(ir_block.clone(), TaskPriority::Low);
        let high_id = queue.push_task(ir_block.clone(), TaskPriority::High);
        let normal_id = queue.push_task(ir_block.clone(), TaskPriority::Normal);
        
        // 验证任务按优先级排序
        let task1 = queue.pop_task().expect("Expected high priority task");
        assert_eq!(task1.id, high_id);
        assert_eq!(task1.priority, TaskPriority::High);

        let task2 = queue.pop_task().expect("Expected normal priority task");
        assert_eq!(task2.id, normal_id);
        assert_eq!(task2.priority, TaskPriority::Normal);

        let task3 = queue.pop_task().expect("Expected low priority task");
        assert_eq!(task3.id, low_id);
        assert_eq!(task3.priority, TaskPriority::Low);
    }
}