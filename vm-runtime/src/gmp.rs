use context::stack::ProtectedFixedSizeStack;
use context::{Context as CpuContext, ResumeOntopFn, Transfer};
use core_affinity::CoreId;
use crossbeam_queue::SegQueue;
use futures::task::ArcWake;
#[cfg(target_family = "unix")]
use mio::unix::SourceFd;
use mio::{Events, Interest, Poll as MioPoll, Token, Waker as MioWaker};
use parking_lot::Mutex;
use stacker::maybe_grow;
use std::future::Future;
#[cfg(target_family = "unix")]
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::task::{Context as TaskContext, Poll};
use std::thread;
use std::time::{Duration, Instant};
use vm_core::VmError;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    High,
    Medium,
    Low,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CoroutineState {
    Ready,
    Running,
    Blocked,
    Dead,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum YieldReason {
    TimeSliceExpired,
    AwaitingIO,
}

fn state_to_u8(s: CoroutineState) -> u8 {
    match s {
        CoroutineState::Ready => 0,
        CoroutineState::Running => 1,
        CoroutineState::Blocked => 2,
        CoroutineState::Dead => 3,
    }
}

fn u8_to_state(v: u8) -> CoroutineState {
    match v {
        0 => CoroutineState::Ready,
        1 => CoroutineState::Running,
        2 => CoroutineState::Blocked,
        _ => CoroutineState::Dead,
    }
}

thread_local! { static CURRENT_CO: std::cell::RefCell<Option<Arc<Coroutine>>> = std::cell::RefCell::new(None); }
thread_local! { static CURRENT_SCHED: std::cell::RefCell<Option<SchedulerHandle>> = std::cell::RefCell::new(None); }
thread_local! { static CURRENT_CMDS: std::cell::RefCell<Option<Arc<SegQueue<ReactorCmd>>>> = std::cell::RefCell::new(None); }

pub struct Coroutine {
    id: u64,
    state: AtomicU8,
    priority: AtomicU8,
    fut: Mutex<Option<Pin<Box<dyn Future<Output = ()> + Send>>>>,
    last_yield: AtomicU8,
    stack: Mutex<Option<ProtectedFixedSizeStack>>,
    ctx: Mutex<Option<CpuContext>>,
}

impl Coroutine {
    fn new(
        id: u64,
        priority: Priority,
        fut: Pin<Box<dyn Future<Output = ()> + Send>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            id,
            state: AtomicU8::new(state_to_u8(CoroutineState::Ready)),
            priority: AtomicU8::new(priority_to_u8(priority)),
            fut: Mutex::new(Some(fut)),
            last_yield: AtomicU8::new(0),
            stack: Mutex::new(None),
            ctx: Mutex::new(None),
        })
    }
    fn new_stackful(
        id: u64,
        priority: Priority,
        task: Box<dyn FnOnce() -> Result<(), VmError> + Send>,
    ) -> Arc<Self> {
        extern "C" fn entry(mut t: Transfer) -> ! {
            let ptr = t.data as *mut Option<Box<dyn FnOnce() -> Result<(), VmError> + Send>>;
            unsafe {
                if let Some(task) = (*ptr).take() {
                    let _ = task();
                }
            }
            loop {
                t = unsafe { t.context.resume(0) };
            }
        }
        let carrier: Option<Box<dyn FnOnce() -> Result<(), VmError> + Send>> = Some(task);
        let mut co = Arc::new(Self {
            id,
            state: AtomicU8::new(state_to_u8(CoroutineState::Ready)),
            priority: AtomicU8::new(priority_to_u8(priority)),
            fut: Mutex::new(None),
            last_yield: AtomicU8::new(0),
            stack: Mutex::new(Some(ProtectedFixedSizeStack::default())),
            ctx: Mutex::new(None),
        });
        {
            let mut stack_guard = co.stack.lock();
            if let Some(ref stack) = *stack_guard {
                unsafe {
                    let ctx = CpuContext::new(stack, entry);
                    let mut transfer = Transfer::new(ctx, 0);
                    let mut boxed = Box::new(carrier);
                    let ptr = (&mut *boxed) as *mut _ as usize;
                    let Transfer { context, .. } = transfer.context.resume(ptr);
                    *co.ctx.lock() = Some(context);
                    std::mem::drop(boxed);
                }
            }
        }
        co
    }

    fn set_state(&self, s: CoroutineState) {
        self.state.store(state_to_u8(s), Ordering::SeqCst)
    }
    fn get_state(&self) -> CoroutineState {
        u8_to_state(self.state.load(Ordering::SeqCst))
    }
    fn set_last_yield(&self, r: YieldReason) {
        let v = match r {
            YieldReason::TimeSliceExpired => 0,
            YieldReason::AwaitingIO => 1,
        };
        self.last_yield.store(v, Ordering::SeqCst)
    }
    fn get_priority(&self) -> Priority {
        u8_to_priority(self.priority.load(Ordering::SeqCst))
    }
    fn set_priority(&self, p: Priority) {
        self.priority.store(priority_to_u8(p), Ordering::SeqCst)
    }
}

struct Processor {
    id: usize,
    high: SegQueue<Arc<Coroutine>>,
    medium: SegQueue<Arc<Coroutine>>,
    low: SegQueue<Arc<Coroutine>>,
}

impl Processor {
    fn new(id: usize) -> Arc<Self> {
        Arc::new(Self {
            id,
            high: SegQueue::new(),
            medium: SegQueue::new(),
            low: SegQueue::new(),
        })
    }
    fn push(&self, co: Arc<Coroutine>, p: Priority) {
        match p {
            Priority::High => self.high.push(co),
            Priority::Medium => self.medium.push(co),
            Priority::Low => self.low.push(co),
        }
    }
    fn pop_next(&self) -> Option<Arc<Coroutine>> {
        self.high
            .pop()
            .or_else(|| self.medium.pop())
            .or_else(|| self.low.pop())
    }
}

#[derive(Clone)]
struct SchedulerHandle {
    global: Arc<SegQueue<Arc<Coroutine>>>,
    processors: Arc<Vec<Arc<Processor>>>,
    reactor_cmds: Arc<SegQueue<ReactorCmd>>,
}

impl SchedulerHandle {
    fn enqueue_ready(&self, co: Arc<Coroutine>) {
        let idx = (co.id as usize) % self.processors.len();
        self.processors[idx].push(co, Priority::Medium)
    }
}

struct CoroutineWaker {
    co: Arc<Coroutine>,
    sched: SchedulerHandle,
}

impl ArcWake for CoroutineWaker {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        arc_self.co.set_state(CoroutineState::Ready);
        arc_self.sched.enqueue_ready(arc_self.co.clone())
    }
}

struct WorkerThread {
    proc: Arc<Processor>,
    sched: SchedulerHandle,
    time_slice: Duration,
    shutdown: Arc<AtomicBool>,
}

impl WorkerThread {
    fn run(self, core: Option<CoreId>) {
        if let Some(c) = core {
            let _ = core_affinity::set_for_current(c);
        }
        while !self.shutdown.load(Ordering::SeqCst) {
            let co_opt = self.proc.pop_next().or_else(|| self.sched.global.pop());
            if let Some(co) = co_opt {
                co.set_state(CoroutineState::Running);
                let start = Instant::now();
                let w = Arc::new(CoroutineWaker {
                    co: co.clone(),
                    sched: SchedulerHandle {
                        global: self.sched.global.clone(),
                        processors: self.sched.processors.clone(),
                        reactor_cmds: self.sched.reactor_cmds.clone(),
                    },
                });
                let waker = futures::task::waker_ref(&w);
                let mut ctx = TaskContext::from_waker(&*waker);
                let mut fut_opt = co.fut.lock();
                if let Some(mut fut) = fut_opt.take() {
                    CURRENT_CO.with(|cell| {
                        *cell.borrow_mut() = Some(co.clone());
                    });
                    CURRENT_SCHED.with(|cell| {
                        *cell.borrow_mut() = Some(SchedulerHandle {
                            global: self.sched.global.clone(),
                            processors: self.sched.processors.clone(),
                            reactor_cmds: self.sched.reactor_cmds.clone(),
                        });
                    });
                    CURRENT_CMDS.with(|cell| {
                        *cell.borrow_mut() = Some(self.sched.reactor_cmds.clone());
                    });
                    const STACK_ALIGN: usize = 4096;
                    const STACK_SIZE: usize = 512 * 1024;
                    unsafe {
                        let layout =
                            std::alloc::Layout::from_size_align(STACK_SIZE, STACK_ALIGN).unwrap();
                        let new_stack = std::alloc::alloc(layout);
                        let _ = psm::on_stack(new_stack, STACK_SIZE, || {
                            let _ = fut.as_mut().poll(&mut ctx);
                        });
                        std::alloc::dealloc(new_stack, layout);
                    }
                    CURRENT_CO.with(|cell| {
                        *cell.borrow_mut() = None;
                    });
                    CURRENT_SCHED.with(|cell| {
                        *cell.borrow_mut() = None;
                    });
                    CURRENT_CMDS.with(|cell| {
                        *cell.borrow_mut() = None;
                    });
                    let elapsed = start.elapsed();
                    if elapsed >= self.time_slice {
                        co.set_last_yield(YieldReason::TimeSliceExpired);
                        let curp = co.get_priority();
                        let newp = match curp {
                            Priority::High => Priority::Medium,
                            Priority::Medium => Priority::Low,
                            Priority::Low => Priority::Low,
                        };
                        co.set_priority(newp);
                    }
                    let done = matches!(co.get_state(), CoroutineState::Dead);
                    if !done {
                        co.set_state(CoroutineState::Ready);
                        *fut_opt = Some(fut);
                        self.proc.push(co.clone(), co.get_priority());
                    }
                } else if co.ctx.lock().is_some() {
                    let mut ctx_guard = co.ctx.lock();
                    if let Some(ctx_val) = ctx_guard.take() {
                        unsafe {
                            let Transfer { context, .. } = ctx_val.resume(0);
                            *ctx_guard = Some(context);
                        }
                    }
                    let elapsed = start.elapsed();
                    if elapsed >= self.time_slice {
                        co.set_last_yield(YieldReason::TimeSliceExpired);
                        let curp = co.get_priority();
                        let newp = match curp {
                            Priority::High => Priority::Medium,
                            Priority::Medium => Priority::Low,
                            Priority::Low => Priority::Low,
                        };
                        co.set_priority(newp);
                        extern "C" fn timeslice_ontop(tr: Transfer) -> Transfer {
                            let ptr = tr.data as *mut Carrier;
                            if !ptr.is_null() {
                                unsafe {
                                    let carrier = &*ptr;
                                    carrier.co.set_last_yield(YieldReason::TimeSliceExpired);
                                    carrier.co.set_state(CoroutineState::Ready);
                                }
                            }
                            tr
                        }
                        struct Carrier {
                            co: Arc<Coroutine>,
                        }
                        let raw = Box::into_raw(Box::new(Carrier { co: co.clone() }));
                        let data = raw as usize;
                        let mut ctx_guard2 = co.ctx.lock();
                        if let Some(ctx2) = ctx_guard2.take() {
                            unsafe {
                                let Transfer { context, .. } =
                                    ctx2.resume_ontop(data, timeslice_ontop as ResumeOntopFn);
                                *ctx_guard2 = Some(context);
                            }
                        }
                        unsafe {
                            let _ = Box::from_raw(raw);
                        }
                    }
                    let done = matches!(co.get_state(), CoroutineState::Dead);
                    if !done {
                        co.set_state(CoroutineState::Ready);
                        self.proc.push(co.clone(), co.get_priority());
                    }
                }
            } else {
                thread::sleep(Duration::from_micros(200));
            }
        }
    }
}

enum ReactorCmd {
    RegisterReadable(RawFd, Token, Arc<Coroutine>),
    RegisterWritable(RawFd, Token, Arc<Coroutine>),
    Unregister(Token),
}

struct Reactor {
    poll: MioPoll,
    events: Events,
    waker: MioWaker,
    mapping: Mutex<std::collections::HashMap<usize, Arc<Coroutine>>>,
    sched: SchedulerHandle,
    shutdown: Arc<AtomicBool>,
    cmds: Arc<SegQueue<ReactorCmd>>,
}

impl Reactor {
    fn new(sched: SchedulerHandle, shutdown: Arc<AtomicBool>) -> Self {
        let poll = MioPoll::new().unwrap();
        let waker = MioWaker::new(poll.registry(), Token(usize::MAX)).unwrap();
        let cmds = sched.reactor_cmds.clone();
        Self {
            poll,
            events: Events::with_capacity(1024),
            waker,
            mapping: Mutex::new(std::collections::HashMap::new()),
            sched,
            shutdown,
            cmds,
        }
    }
    fn run(mut self, core: Option<CoreId>) {
        if let Some(c) = core {
            let _ = core_affinity::set_for_current(c);
        }
        while !self.shutdown.load(Ordering::SeqCst) {
            while let Some(cmd) = self.cmds.pop() {
                match cmd {
                    ReactorCmd::RegisterReadable(fd, tok, co) => {
                        #[cfg(target_family = "unix")]
                        {
                            let mut src = SourceFd(&fd);
                            let _ =
                                self.poll
                                    .registry()
                                    .register(&mut src, tok, Interest::READABLE);
                        }
                        self.mapping.lock().insert(tok.0, co);
                    }
                    ReactorCmd::RegisterWritable(fd, tok, co) => {
                        #[cfg(target_family = "unix")]
                        {
                            let mut src = SourceFd(&fd);
                            let _ =
                                self.poll
                                    .registry()
                                    .register(&mut src, tok, Interest::WRITABLE);
                        }
                        self.mapping.lock().insert(tok.0, co);
                    }
                    ReactorCmd::Unregister(tok) => {
                        let id = tok.0;
                        self.mapping.lock().remove(&id);
                    }
                }
            }
            let _ = self
                .poll
                .poll(&mut self.events, Some(Duration::from_millis(10)));
            for ev in &self.events {
                let tok = ev.token();
                if let Some(co) = self.mapping.lock().get(&tok.0).cloned() {
                    co.set_state(CoroutineState::Ready);
                    self.sched.enqueue_ready(co);
                }
            }
        }
    }
}

pub struct GmpRuntime {
    task_counter: AtomicU64,
    global: Arc<SegQueue<Arc<Coroutine>>>,
    processors: Arc<Vec<Arc<Processor>>>,
    worker_handles: Mutex<Vec<thread::JoinHandle<()>>>,
    reactor_handle: Mutex<Option<thread::JoinHandle<()>>>,
    shutdown: Arc<AtomicBool>,
    time_slice: Duration,
    reactor_cmds: Arc<SegQueue<ReactorCmd>>,
}

impl GmpRuntime {
    pub fn new() -> Self {
        let cores = num_cpus::get();
        let p_count = cores.saturating_sub(1).max(1);
        let mut ps = Vec::with_capacity(p_count);
        for i in 0..p_count {
            ps.push(Processor::new(i))
        }
        Self {
            task_counter: AtomicU64::new(1),
            global: Arc::new(SegQueue::new()),
            processors: Arc::new(ps),
            worker_handles: Mutex::new(Vec::new()),
            reactor_handle: Mutex::new(None),
            shutdown: Arc::new(AtomicBool::new(false)),
            time_slice: Duration::from_millis(2),
            reactor_cmds: Arc::new(SegQueue::new()),
        }
    }

    pub fn start(&self) -> Result<(), VmError> {
        let cores = core_affinity::get_core_ids().unwrap_or_default();
        let mut handles = Vec::new();
        for (i, proc) in self.processors.iter().enumerate() {
            let sched = SchedulerHandle {
                global: self.global.clone(),
                processors: self.processors.clone(),
                reactor_cmds: self.reactor_cmds.clone(),
            };
            let worker = WorkerThread {
                proc: proc.clone(),
                sched,
                time_slice: self.time_slice,
                shutdown: self.shutdown.clone(),
            };
            let core = cores.get(i).cloned();
            let h = thread::spawn(move || worker.run(core));
            handles.push(h);
        }
        *self.worker_handles.lock() = handles;
        let sched = SchedulerHandle {
            global: self.global.clone(),
            processors: self.processors.clone(),
            reactor_cmds: self.reactor_cmds.clone(),
        };
        let reactor = Reactor::new(sched, self.shutdown.clone());
        let core = cores.get(self.processors.len()).cloned();
        let h = thread::spawn(move || reactor.run(core));
        *self.reactor_handle.lock() = Some(h);
        Ok(())
    }

    pub fn stop(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(h) = self.reactor_handle.lock().take() {
            let _ = h.join();
        }
        for h in self.worker_handles.lock().drain(..) {
            let _ = h.join();
        }
    }

    pub fn submit_task_with_priority<F, Fut>(&self, task: F, priority: Priority) -> u64
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), VmError>> + Send + 'static,
    {
        let id = self.task_counter.fetch_add(1, Ordering::Relaxed);
        let fut = Box::pin(async move {
            let _ = task().await;
        });
        let co = Coroutine::new(id, priority, fut);
        self.global.push(co);
        id
    }

    pub fn submit_stackful_with_priority<F>(&self, task: F, priority: Priority) -> u64
    where
        F: FnOnce() -> Result<(), VmError> + Send + 'static,
    {
        let id = self.task_counter.fetch_add(1, Ordering::Relaxed);
        let co = Coroutine::new_stackful(id, priority, Box::new(task));
        self.global.push(co);
        id
    }
}

impl Drop for GmpRuntime {
    fn drop(&mut self) {
        self.stop()
    }
}

pub struct GmpRuntimeAdapter {
    inner: Arc<GmpRuntime>,
}

impl GmpRuntimeAdapter {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(GmpRuntime::new()),
        }
    }
    pub fn submit_task_with_priority<F, Fut>(&self, task: F, priority: Priority) -> u64
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<(), VmError>> + Send + 'static,
    {
        self.inner.submit_task_with_priority(task, priority)
    }
}

#[async_trait::async_trait]
impl super::AsyncRuntime for GmpRuntimeAdapter {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn runtime_type(&self) -> super::RuntimeType {
        super::RuntimeType::Custom
    }
    async fn start(&mut self) -> Result<(), VmError> {
        self.inner.start()
    }
    async fn stop(&mut self) -> Result<(), VmError> {
        self.inner.stop();
        Ok(())
    }
    fn generate_task_id(&self) -> super::TaskId {
        self.inner.task_counter.fetch_add(1, Ordering::Relaxed)
    }
    async fn submit_task<F, Fut, T>(&self, task: F) -> Result<super::TaskHandle<T>, VmError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<T, VmError>> + Send + 'static,
        T: Send + 'static,
    {
        let id = self.generate_task_id();
        let fut = Box::pin(async move {
            let _ = task().await;
        });
        let co = Coroutine::new(id, Priority::Medium, fut);
        self.inner.global.push(co);
        Ok(super::TaskHandle {
            task_id: id,
            _phantom: std::marker::PhantomData,
        })
    }
    async fn wait_task<T: Send + 'static>(
        &self,
        _handle: super::TaskHandle<T>,
    ) -> Result<T, VmError> {
        Err(VmError::Core(vm_core::CoreError::NotImplemented {
            feature: "wait_task".to_string(),
            module: "vm-runtime-custom".to_string(),
        }))
    }
    async fn cancel_task(&self, _task_id: super::TaskId) -> Result<(), VmError> {
        Ok(())
    }
    fn get_task_status(&self, _task_id: super::TaskId) -> super::TaskStatus {
        super::TaskStatus::Completed
    }
    async fn delay(&self, duration: Duration) -> Result<(), VmError> {
        thread::sleep(duration);
        Ok(())
    }
    fn get_stats(&self) -> super::RuntimeStats {
        super::RuntimeStats {
            active_tasks: 0,
            pending_tasks: 0,
            completed_tasks: 0,
            failed_tasks: 0,
            thread_pool_utilization: 0.6,
            memory_usage_bytes: 1024 * 1024,
            io_operations: 0,
            avg_task_duration_ns: 1_000_000,
        }
    }
}

#[async_trait::async_trait]
impl super::AsyncRuntimeBase for GmpRuntimeAdapter {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn runtime_type(&self) -> super::RuntimeType {
        super::RuntimeType::Custom
    }
    async fn start(&mut self) -> Result<(), VmError> {
        <GmpRuntimeAdapter as super::AsyncRuntime>::start(self).await
    }
    async fn stop(&mut self) -> Result<(), VmError> {
        <GmpRuntimeAdapter as super::AsyncRuntime>::stop(self).await
    }
    fn generate_task_id(&self) -> super::TaskId {
        <GmpRuntimeAdapter as super::AsyncRuntime>::generate_task_id(self)
    }
    async fn cancel_task(&self, task_id: super::TaskId) -> Result<(), VmError> {
        <GmpRuntimeAdapter as super::AsyncRuntime>::cancel_task(self, task_id).await
    }
    fn get_task_status(&self, task_id: super::TaskId) -> super::TaskStatus {
        <GmpRuntimeAdapter as super::AsyncRuntime>::get_task_status(self, task_id)
    }
    async fn delay(&self, duration: Duration) -> Result<(), VmError> {
        <GmpRuntimeAdapter as super::AsyncRuntime>::delay(self, duration).await
    }
    fn get_stats(&self) -> super::RuntimeStats {
        <GmpRuntimeAdapter as super::AsyncRuntime>::get_stats(self)
    }
}

pub struct YieldNow {
    reason: YieldReason,
    done: bool,
}

impl Future for YieldNow {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, _cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        if self.done {
            return Poll::Ready(());
        }
        let mut scheduled = false;
        CURRENT_CO.with(|cell| {
            if let Some(co) = cell.borrow().as_ref() {
                co.set_last_yield(self.reason);
                co.set_state(CoroutineState::Ready);
                CURRENT_SCHED.with(|s| {
                    if let Some(h) = s.borrow().as_ref() {
                        h.enqueue_ready(co.clone());
                        scheduled = true
                    }
                });
            }
        });
        self.done = true;
        if scheduled {
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

pub async fn yield_now(reason: YieldReason) {
    YieldNow {
        reason,
        done: false,
    }
    .await
}

#[cfg(target_family = "unix")]
pub fn register_readable(fd: RawFd) -> Token {
    static NEXT_TOKEN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let id = NEXT_TOKEN.fetch_add(1, Ordering::SeqCst);
    let tok = Token(id);
    CURRENT_CO.with(|cco| {
        if let Some(co) = cco.borrow().as_ref() {
            CURRENT_CMDS.with(|cc| {
                if let Some(q) = cc.borrow().as_ref() {
                    q.push(ReactorCmd::RegisterReadable(fd, tok, co.clone()))
                }
            });
        }
    });
    tok
}

#[cfg(target_family = "unix")]
pub fn register_writable(fd: RawFd) -> Token {
    static NEXT_TOKEN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let id = NEXT_TOKEN.fetch_add(1, Ordering::SeqCst);
    let tok = Token(id);
    CURRENT_CO.with(|cco| {
        if let Some(co) = cco.borrow().as_ref() {
            CURRENT_CMDS.with(|cc| {
                if let Some(q) = cc.borrow().as_ref() {
                    q.push(ReactorCmd::RegisterWritable(fd, tok, co.clone()))
                }
            });
        }
    });
    tok
}

#[cfg(target_family = "unix")]
pub fn unregister(token: Token) {
    CURRENT_CMDS.with(|cc| {
        if let Some(q) = cc.borrow().as_ref() {
            q.push(ReactorCmd::Unregister(token))
        }
    });
}
fn priority_to_u8(p: Priority) -> u8 {
    match p {
        Priority::High => 0,
        Priority::Medium => 1,
        Priority::Low => 2,
    }
}
fn u8_to_priority(v: u8) -> Priority {
    match v {
        0 => Priority::High,
        1 => Priority::Medium,
        _ => Priority::Low,
    }
}
