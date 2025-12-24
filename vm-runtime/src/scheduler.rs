//! # GMP型协程任务调度器
//!
//! 模拟Go语言GMP模型的高性能协程调度器，包含：
//! - 协程（G）：虚拟CPU，异步任务执行单元
//! - 处理器（P）：本地任务队列，包含高/中/低三级优先级队列
//! - 工作线程（M）：OS线程，绑定到CPU核心，执行调度循环
//! - 反应器（Reactor）：后台I/O运行时，监听和唤醒阻塞协程

use std::collections::VecDeque;
use std::os::unix::io::RawFd;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_queue::SegQueue;
use parking_lot::Mutex;
use uuid::Uuid;

/// Legacy compatibility types and functions
/// These are provided for backward compatibility with the deprecated gmp module
#[derive(Debug, Clone, Copy)]
pub struct Token(usize);

impl Token {
    /// 获取 Token 的值
    pub fn value(&self) -> usize {
        self.0
    }
}

/// Legacy yield_now implementation for backward compatibility
pub async fn yield_now(_reason: YieldReason) {
    // In the new scheduler, we simulate yield by sleeping for a short duration
    // This is a simplified version for backward compatibility
    std::thread::sleep(Duration::from_micros(10));
}

#[cfg(target_family = "unix")]
pub fn register_readable(_fd: RawFd) -> Token {
    // This is a no-op implementation for backward compatibility
    static NEXT_TOKEN: AtomicUsize = AtomicUsize::new(0);
    let id = NEXT_TOKEN.fetch_add(1, Ordering::SeqCst);
    Token(id)
}

#[cfg(target_family = "unix")]
pub fn register_writable(_fd: RawFd) -> Token {
    // This is a no-op implementation for backward compatibility
    static NEXT_TOKEN: AtomicUsize = AtomicUsize::new(0);
    let id = NEXT_TOKEN.fetch_add(1, Ordering::SeqCst);
    Token(id)
}

#[cfg(target_family = "unix")]
pub fn unregister(_token: Token) {
    // This is a no-op implementation for backward compatibility
}

/// 优先级枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    High = 2,
    Medium = 1,
    Low = 0,
}

impl Priority {
    pub fn as_usize(self) -> usize {
        match self {
            Priority::High => 2,
            Priority::Medium => 1,
            Priority::Low => 0,
        }
    }

    pub fn from_usize(val: usize) -> Self {
        match val {
            2 => Priority::High,
            1 => Priority::Medium,
            _ => Priority::Low,
        }
    }
}

/// 让出原因
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YieldReason {
    /// 时间片已过期
    TimeSliceExpired,
    /// 等待I/O
    AwaitingIO,
    /// 手动让出
    Manual,
    /// 被锁阻塞
    BlockedOnLock,
}

/// 协程状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoroutineState {
    /// 就绪，等待调度
    Ready,
    /// 运行中
    Running,
    /// 被阻塞
    Blocked,
    /// 已死亡
    Dead,
}

impl CoroutineState {
    fn as_usize(self) -> usize {
        match self {
            CoroutineState::Ready => 0,
            CoroutineState::Running => 1,
            CoroutineState::Blocked => 2,
            CoroutineState::Dead => 3,
        }
    }

    fn from_usize(val: usize) -> Self {
        match val {
            0 => CoroutineState::Ready,
            1 => CoroutineState::Running,
            2 => CoroutineState::Blocked,
            3 => CoroutineState::Dead,
            _ => CoroutineState::Dead,
        }
    }
}

/// 虚拟CPU协程
pub struct Coroutine {
    id: String,
    /// 原子操作的协程状态
    state: Arc<AtomicUsize>,
    priority: Priority,
    created_at: Instant,
    /// 上次调度时间戳（微秒）
    last_scheduled: Arc<AtomicU64>,
    /// 执行次数
    execution_count: Arc<AtomicU64>,
    /// 执行的任务（可重入的回调函数）
    task: Arc<Mutex<Option<Box<dyn Fn() + Send + Sync>>>>,
}

impl Coroutine {
    pub fn new<F>(priority: Priority, task: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        Self {
            id: Uuid::new_v4().to_string(),
            state: Arc::new(AtomicUsize::new(CoroutineState::Ready.as_usize())),
            priority,
            created_at: Instant::now(),
            last_scheduled: Arc::new(AtomicU64::new(0)),
            execution_count: Arc::new(AtomicU64::new(0)),
            task: Arc::new(Mutex::new(Some(Box::new(task)))),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn priority(&self) -> Priority {
        self.priority
    }

    pub fn state(&self) -> CoroutineState {
        CoroutineState::from_usize(self.state.load(Ordering::SeqCst))
    }

    /// 原子地设置状态
    pub fn set_state(&self, new_state: CoroutineState) {
        self.state.store(new_state.as_usize(), Ordering::SeqCst);
    }

    /// 记录执行时间和次数
    pub fn record_execution(&self) {
        let elapsed = self.created_at.elapsed().as_micros() as u64;
        self.last_scheduled.store(elapsed, Ordering::Release);
        self.execution_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn execution_count(&self) -> u64 {
        self.execution_count.load(Ordering::Relaxed)
    }

    pub fn last_scheduled(&self) -> u64 {
        self.last_scheduled.load(Ordering::Acquire)
    }

    /// 协程执行时间（毫秒）
    pub fn elapsed_ms(&self) -> u64 {
        self.created_at.elapsed().as_millis() as u64
    }

    /// 执行协程任务
    pub fn execute(&self) {
        if let Some(task) = self.task.lock().as_ref() {
            task();
        }
    }
}

impl Clone for Coroutine {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            state: Arc::clone(&self.state),
            priority: self.priority,
            created_at: self.created_at,
            last_scheduled: Arc::clone(&self.last_scheduled),
            execution_count: Arc::clone(&self.execution_count),
            task: Arc::clone(&self.task),
        }
    }
}

/// 处理器（P）- 本地优先级任务队列
pub struct Processor {
    id: usize,
    /// 高优先级就绪队列
    high_priority_queue: Arc<Mutex<VecDeque<Arc<Coroutine>>>>,
    /// 中优先级就绪队列
    medium_priority_queue: Arc<Mutex<VecDeque<Arc<Coroutine>>>>,
    /// 低优先级就绪队列
    low_priority_queue: Arc<Mutex<VecDeque<Arc<Coroutine>>>>,
}

impl Processor {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            high_priority_queue: Arc::new(Mutex::new(VecDeque::new())),
            medium_priority_queue: Arc::new(Mutex::new(VecDeque::new())),
            low_priority_queue: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn id(&self) -> usize {
        self.id
    }

    /// 将协程加入对应优先级队列
    pub fn enqueue(&self, coroutine: Arc<Coroutine>) {
        match coroutine.priority() {
            Priority::High => {
                self.high_priority_queue.lock().push_back(coroutine);
            }
            Priority::Medium => {
                self.medium_priority_queue.lock().push_back(coroutine);
            }
            Priority::Low => {
                self.low_priority_queue.lock().push_back(coroutine);
            }
        }
    }

    /// 按优先级获取下一个可运行协程（High > Medium > Low）
    pub fn dequeue_next(&self) -> Option<Arc<Coroutine>> {
        // 检查高优先级队列
        {
            let mut high = self.high_priority_queue.lock();
            if let Some(coro) = high.pop_front() {
                return Some(coro);
            }
        }

        // 检查中优先级队列
        {
            let mut medium = self.medium_priority_queue.lock();
            if let Some(coro) = medium.pop_front() {
                return Some(coro);
            }
        }

        // 检查低优先级队列
        {
            let mut low = self.low_priority_queue.lock();
            low.pop_front()
        }
    }

    /// 获取所有队列的总大小
    pub fn queue_size(&self) -> usize {
        let high = self.high_priority_queue.lock().len();
        let medium = self.medium_priority_queue.lock().len();
        let low = self.low_priority_queue.lock().len();
        high + medium + low
    }

    /// 获取各优先级队列的大小
    pub fn queue_sizes(&self) -> (usize, usize, usize) {
        (
            self.high_priority_queue.lock().len(),
            self.medium_priority_queue.lock().len(),
            self.low_priority_queue.lock().len(),
        )
    }

    /// 尝试从其他processor窃取任务（work-stealing）
    pub fn try_steal_from(&self, other: &Processor) -> Option<Arc<Coroutine>> {
        // 优先从高优先级窃取
        if let Some(coro) = other.high_priority_queue.lock().pop_front() {
            return Some(coro);
        }
        if let Some(coro) = other.medium_priority_queue.lock().pop_front() {
            return Some(coro);
        }
        other.low_priority_queue.lock().pop_front()
    }
}

impl Clone for Processor {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            high_priority_queue: Arc::clone(&self.high_priority_queue),
            medium_priority_queue: Arc::clone(&self.medium_priority_queue),
            low_priority_queue: Arc::clone(&self.low_priority_queue),
        }
    }
}

/// 工作线程（M）
pub struct WorkerThread {
    id: usize,
    processor: Processor,
    running: Arc<AtomicBool>,
    thread_handle: Arc<Mutex<Option<thread::JoinHandle<()>>>>,
}

impl WorkerThread {
    pub fn new(id: usize, processor: Processor) -> Self {
        Self {
            id,
            processor,
            running: Arc::new(AtomicBool::new(false)),
            thread_handle: Arc::new(Mutex::new(None)),
        }
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// 启动工作线程并绑定到CPU核心
    pub fn start(&self) {
        let processor = self.processor.clone();
        let running = Arc::clone(&self.running);
        let worker_id = self.id;

        running.store(true, Ordering::Release);

        let handle = thread::spawn(move || {
            // 绑定到CPU核心
            if let Some(core_ids) = core_affinity::get_core_ids() {
                if worker_id < core_ids.len() {
                    let core_id = core_ids[worker_id];
                    if !core_affinity::set_for_current(core_id) {
                        eprintln!("[Worker {}] Failed to bind to CPU core", worker_id);
                    } else {
                        tracing::info!("[Worker {}] Bound to CPU core {:?}", worker_id, core_id);
                    }
                }
            }

            Self::scheduling_loop(&processor, &running, worker_id);
        });

        *self.thread_handle.lock() = Some(handle);
    }

    /// 调度循环
    fn scheduling_loop(processor: &Processor, running: &Arc<AtomicBool>, worker_id: usize) {
        const TIME_SLICE_MS: u64 = 2;

        while running.load(Ordering::Acquire) {
            if let Some(coroutine) = processor.dequeue_next() {
                if coroutine.state() != CoroutineState::Dead {
                    coroutine.set_state(CoroutineState::Running);
                    coroutine.record_execution();

                    // 执行时间片
                    let start = Instant::now();
                    let time_slice = Duration::from_millis(TIME_SLICE_MS);

                    // 模拟协程执行（实际应通过Future poll实现）
                    while start.elapsed() < time_slice {
                        thread::sleep(Duration::from_micros(10));
                    }

                    // 时间片过期，将协程重新放入就绪队列
                    if start.elapsed() >= time_slice {
                        coroutine.set_state(CoroutineState::Ready);
                        processor.enqueue(coroutine.clone());
                        tracing::debug!(
                            "[Worker {}] Time slice expired for coroutine {}",
                            worker_id,
                            coroutine.id()
                        );
                    }
                }
            } else {
                // 没有就绪任务，尝试work-stealing
                thread::sleep(Duration::from_micros(100));
            }
        }

        tracing::info!("[Worker {}] Shutdown", worker_id);
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Release);
    }

    pub fn join(&self) {
        if let Some(handle) = self.thread_handle.lock().take() {
            let _ = handle.join();
        }
    }
}

impl Clone for WorkerThread {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            processor: self.processor.clone(),
            running: Arc::clone(&self.running),
            thread_handle: Arc::clone(&self.thread_handle),
        }
    }
}

/// 后台I/O反应器（Reactor）
pub struct Reactor {
    id: usize,
    running: Arc<AtomicBool>,
    thread_handle: Arc<Mutex<Option<thread::JoinHandle<()>>>>,
}

impl Reactor {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            running: Arc::new(AtomicBool::new(false)),
            thread_handle: Arc::new(Mutex::new(None)),
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// 启动I/O监控线程
    pub fn start(&self) {
        let running = Arc::clone(&self.running);
        let reactor_id = self.id;

        running.store(true, Ordering::Release);

        let handle = thread::spawn(move || {
            // 绑定到最后一个CPU核心
            if let Some(core_ids) = core_affinity::get_core_ids() {
                if let Some(core_id) = core_ids.last() {
                    if !core_affinity::set_for_current(*core_id) {
                        eprintln!("[Reactor {}] Failed to bind to CPU core", reactor_id);
                    } else {
                        tracing::info!("[Reactor {}] Bound to CPU core {:?}", reactor_id, core_id);
                    }
                }
            }

            Self::event_loop(&running, reactor_id);
        });

        *self.thread_handle.lock() = Some(handle);
    }

    /// I/O事件循环
    fn event_loop(running: &Arc<AtomicBool>, reactor_id: usize) {
        tracing::info!("[Reactor {}] Started I/O event loop", reactor_id);

        while running.load(Ordering::Acquire) {
            // 模拟I/O监听
            // 实际应使用mio::Poll或epoll/kqueue
            thread::sleep(Duration::from_millis(10));
            tracing::debug!("[Reactor {}] Poll check", reactor_id);
        }

        tracing::info!("[Reactor {}] Shutdown", reactor_id);
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Release);
    }

    pub fn join(&self) {
        if let Some(handle) = self.thread_handle.lock().take() {
            let _ = handle.join();
        }
    }
}

impl Clone for Reactor {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            running: Arc::clone(&self.running),
            thread_handle: Arc::clone(&self.thread_handle),
        }
    }
}

/// 全局协程调度器
pub struct CoroutineScheduler {
    /// 全局任务队列（无锁）
    global_queue: Arc<SegQueue<Arc<Coroutine>>>,
    /// 处理器列表
    processors: Arc<Mutex<Vec<Processor>>>,
    /// 工作线程列表
    workers: Arc<Mutex<Vec<WorkerThread>>>,
    /// 反应器
    reactor: Arc<Mutex<Option<Reactor>>>,
    /// 工作线程数
    num_workers: usize,
    /// 调度器是否运行中
    running: Arc<AtomicBool>,
}

impl CoroutineScheduler {
    /// 创建新的调度器
    pub fn new() -> std::io::Result<Self> {
        let num_cpus = num_cpus::get();
        let num_workers = (num_cpus.saturating_sub(1)).max(1);

        let mut processors = Vec::new();
        let mut workers = Vec::new();

        // 创建处理器和工作线程
        for i in 0..num_workers {
            let processor = Processor::new(i);
            processors.push(processor.clone());

            let worker = WorkerThread::new(i, processor);
            workers.push(worker);
        }

        tracing::info!(
            "[Scheduler] Initialized with {} worker threads (CPU cores: {})",
            num_workers,
            num_cpus
        );

        Ok(Self {
            global_queue: Arc::new(SegQueue::new()),
            processors: Arc::new(Mutex::new(processors)),
            workers: Arc::new(Mutex::new(workers)),
            reactor: Arc::new(Mutex::new(Some(Reactor::new(num_cpus - 1)))),
            num_workers,
            running: Arc::new(AtomicBool::new(false)),
        })
    }

    /// 启动调度器
    pub fn start(&self) -> std::io::Result<()> {
        self.running.store(true, Ordering::Release);

        // 启动所有工作线程
        for worker in self.workers.lock().iter() {
            worker.start();
        }

        // 启动反应器
        if let Some(reactor) = self.reactor.lock().as_ref() {
            reactor.start();
        }

        tracing::info!("[Scheduler] Started with all workers and reactor");
        Ok(())
    }

    /// 停止调度器
    pub fn stop(&self) {
        self.running.store(false, Ordering::Release);

        // 停止所有工作线程
        for worker in self.workers.lock().iter() {
            worker.stop();
        }

        // 停止反应器
        if let Some(reactor) = self.reactor.lock().as_ref() {
            reactor.stop();
        }

        tracing::info!("[Scheduler] Stopped");
    }

    /// 等待所有线程完成
    pub fn join_all(&self) {
        for worker in self.workers.lock().iter() {
            worker.join();
        }

        if let Some(reactor) = self.reactor.lock().as_mut() {
            reactor.join();
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// 提交任务到全局队列
    pub fn submit_task<F>(&self, priority: Priority, task: F) -> Arc<Coroutine>
    where
        F: Fn() + Send + Sync + 'static,
    {
        let coroutine = Arc::new(Coroutine::new(priority, task));
        self.global_queue.push(coroutine.clone());
        tracing::debug!(
            "[Scheduler] Task submitted: {} (priority: {:?})",
            coroutine.id(),
            priority
        );
        coroutine
    }

    /// 从全局队列分发任务到处理器
    pub fn distribute_tasks(&self) {
        let processors = self.processors.lock();
        let mut processor_idx = 0;

        let mut distributed_count = 0;
        while let Some(coro) = self.global_queue.pop() {
            processors[processor_idx % self.num_workers].enqueue(coro);
            processor_idx += 1;
            distributed_count += 1;
        }

        if distributed_count > 0 {
            tracing::debug!("[Scheduler] Distributed {} tasks", distributed_count);
        }
    }

    /// 获取调度器统计信息
    pub fn get_stats(&self) -> SchedulerStats {
        let processors = self.processors.lock();
        let mut queue_sizes = Vec::new();
        let mut total_tasks = 0;

        for processor in processors.iter() {
            let size = processor.queue_size();
            queue_sizes.push(size);
            total_tasks += size;
        }

        let global_queue_size = self.global_queue.len();

        SchedulerStats {
            num_workers: self.num_workers,
            total_tasks,
            global_queue_size,
            processor_queue_sizes: queue_sizes,
            running: self.is_running(),
        }
    }

    pub fn num_workers(&self) -> usize {
        self.num_workers
    }

    pub fn global_queue_size(&self) -> usize {
        self.global_queue.len()
    }
}

impl Clone for CoroutineScheduler {
    fn clone(&self) -> Self {
        Self {
            global_queue: Arc::clone(&self.global_queue),
            processors: Arc::clone(&self.processors),
            workers: Arc::clone(&self.workers),
            reactor: Arc::clone(&self.reactor),
            num_workers: self.num_workers,
            running: Arc::clone(&self.running),
        }
    }
}

impl Default for CoroutineScheduler {
    fn default() -> Self {
        Self::new().expect("Failed to create scheduler")
    }
}

/// 调度器统计信息
#[derive(Debug, Clone)]
pub struct SchedulerStats {
    pub num_workers: usize,
    pub total_tasks: usize,
    pub global_queue_size: usize,
    pub processor_queue_sizes: Vec<usize>,
    pub running: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::High > Priority::Medium);
        assert!(Priority::Medium > Priority::Low);
        assert_eq!(Priority::High.as_usize(), 2);
        assert_eq!(Priority::Medium.as_usize(), 1);
        assert_eq!(Priority::Low.as_usize(), 0);
    }

    #[test]
    fn test_coroutine_state_transitions() {
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter_clone = counter.clone();
        let coro = Coroutine::new(Priority::Medium, move || {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });
        assert_eq!(coro.state(), CoroutineState::Ready);

        coro.set_state(CoroutineState::Running);
        assert_eq!(coro.state(), CoroutineState::Running);

        coro.set_state(CoroutineState::Blocked);
        assert_eq!(coro.state(), CoroutineState::Blocked);

        coro.set_state(CoroutineState::Dead);
        assert_eq!(coro.state(), CoroutineState::Dead);
    }

    #[test]
    fn test_processor_priority_queue() {
        let processor = Processor::new(0);

        // 创建不同优先级的协程
        let high_coro = Arc::new(Coroutine::new(Priority::High, || {}));
        let med_coro = Arc::new(Coroutine::new(Priority::Medium, || {}));
        let low_coro = Arc::new(Coroutine::new(Priority::Low, || {}));

        // 以相反顺序插入
        processor.enqueue(low_coro.clone());
        processor.enqueue(high_coro.clone());
        processor.enqueue(med_coro.clone());

        // 验证优先级顺序
        assert_eq!(processor.dequeue_next().unwrap().id(), high_coro.id());
        assert_eq!(processor.dequeue_next().unwrap().id(), med_coro.id());
        assert_eq!(processor.dequeue_next().unwrap().id(), low_coro.id());
        assert!(processor.dequeue_next().is_none());
    }

    #[test]
    fn test_scheduler_creation() {
        let scheduler = CoroutineScheduler::new().expect("Failed to create scheduler");
        assert_eq!(
            scheduler.num_workers(),
            num_cpus::get().saturating_sub(1).max(1)
        );
        assert!(!scheduler.is_running());
    }

    #[test]
    fn test_task_submission() {
        let scheduler = CoroutineScheduler::new().expect("Failed to create scheduler");

        let task1 = scheduler.submit_task(Priority::High, || {});
        let task2 = scheduler.submit_task(Priority::Medium, || {});
        let task3 = scheduler.submit_task(Priority::Low, || {});

        assert!(!task1.id().is_empty());
        assert!(!task2.id().is_empty());
        assert!(!task3.id().is_empty());
        assert_ne!(task1.id(), task2.id());

        let stats = scheduler.get_stats();
        assert_eq!(stats.global_queue_size, 3);
    }

    #[test]
    fn test_priority_from_usize() {
        assert_eq!(Priority::from_usize(2), Priority::High);
        assert_eq!(Priority::from_usize(1), Priority::Medium);
        assert_eq!(Priority::from_usize(0), Priority::Low);
        assert_eq!(Priority::from_usize(3), Priority::Low);
    }

    #[test]
    fn test_coroutine_state_conversions() {
        assert_eq!(CoroutineState::Ready.as_usize(), 0);
        assert_eq!(CoroutineState::Running.as_usize(), 1);
        assert_eq!(CoroutineState::Blocked.as_usize(), 2);
        assert_eq!(CoroutineState::Dead.as_usize(), 3);

        assert_eq!(CoroutineState::from_usize(0), CoroutineState::Ready);
        assert_eq!(CoroutineState::from_usize(1), CoroutineState::Running);
        assert_eq!(CoroutineState::from_usize(2), CoroutineState::Blocked);
        assert_eq!(CoroutineState::from_usize(3), CoroutineState::Dead);
        assert_eq!(CoroutineState::from_usize(99), CoroutineState::Dead);
    }

    #[test]
    fn test_token_value() {
        let token = Token(42);
        assert_eq!(token.value(), 42);
    }

    #[test]
    fn test_scheduler_stats() {
        let scheduler = CoroutineScheduler::new().expect("Failed to create scheduler");
        let stats = scheduler.get_stats();

        assert_eq!(stats.total_tasks, 0);
        assert_eq!(stats.global_queue_size, 0);
        assert!(!stats.running);
    }
}
