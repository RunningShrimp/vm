//! 执行循环管理模块
//!
//! 模块架构：
//! - 主模块：基础执行逻辑（无feature gate）
//! - jit_execution: JIT编译支持（通过performance feature gate）
//! - async_execution: 协程调度支持（通过performance feature gate）

use log::{debug, error, info};
use std::collections::HashMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use tracing::{debug as tdebug, info as tinfo};
use vm_core::{Decoder, ExecStatus, ExecutionEngine, GuestAddr, GuestArch, VmError, VmResult};
use vm_engine::interpreter::{ExecInterruptAction, Interpreter};
use vm_ir::Terminator;
use vm_mem::SoftMmu;

use super::decoder_factory;

/// JIT 配置类型别名
///
/// 简化复杂的 JIT 配置类型，提高代码可读性
#[cfg(feature = "performance")]
pub type JitConfigOption = Option<(
    Option<Arc<Mutex<HashMap<GuestAddr, vm_engine::jit::CodePtr>>>>,
    Option<vm_engine::jit::AdaptiveThresholdConfig>,
)>;

/// VCPU 执行参数
///
/// 将多个执行参数封装到结构体中，减少函数参数数量
#[derive(Clone, Debug)]
pub struct VcpuExecuteParams {
    /// 调试标志
    pub debug: bool,
    /// 初始 PC
    pub thread_pc: GuestAddr,
    /// 运行标志
    pub run_flag: Arc<AtomicBool>,
    /// 暂停标志
    pub pause_flag: Arc<AtomicBool>,
    /// 客户机架构
    pub guest_arch: GuestArch,
    /// CPU ID
    pub cpu_id: usize,
    /// 最大步数
    pub max_steps: usize,
    /// 混合执行模式
    pub hybrid: bool,
}

// 条件模块：JIT编译支持
#[cfg(feature = "performance")]
pub mod jit_execution {
    use super::*;
    use vm_engine::jit::{AdaptiveThresholdConfig, AdaptiveThresholdStats, CodePtr, Jit};

    /// JIT执行器状态
    pub struct JitExecutionState {
        pub code_pool: Option<Arc<Mutex<HashMap<GuestAddr, CodePtr>>>>,
        pub adaptive_snapshot: Arc<Mutex<Option<AdaptiveThresholdStats>>>,
        pub adaptive_config: Option<AdaptiveThresholdConfig>,
    }

    impl Default for JitExecutionState {
        fn default() -> Self {
            Self::new()
        }
    }

    impl JitExecutionState {
        pub fn new() -> Self {
            Self {
                code_pool: None,
                adaptive_snapshot: Arc::new(Mutex::new(None)),
                adaptive_config: None,
            }
        }

        pub fn with_config(config: AdaptiveThresholdConfig) -> Self {
            Self {
                code_pool: None,
                adaptive_snapshot: Arc::new(Mutex::new(None)),
                adaptive_config: Some(config),
            }
        }

        pub fn with_code_pool(
            pool: Arc<Mutex<HashMap<GuestAddr, CodePtr>>>,
            config: Option<AdaptiveThresholdConfig>,
        ) -> Self {
            Self {
                code_pool: Some(pool),
                adaptive_snapshot: Arc::new(Mutex::new(None)),
                adaptive_config: config,
            }
        }

        pub fn create_jit(&self, hybrid: bool) -> Option<Jit> {
            if hybrid {
                match (&self.code_pool, &self.adaptive_config) {
                    (Some(_), Some(cfg)) => Some(Jit::with_adaptive_config(cfg.clone())),
                    (Some(_), None) => Some(Jit::new()),
                    (None, Some(cfg)) => Some(Jit::with_adaptive_config(cfg.clone())),
                    (None, None) => Some(Jit::new()),
                }
            } else {
                None
            }
        }

        pub fn update_snapshot(&self, step: usize) {
            if step.is_multiple_of(1000)
                && let Ok(mut snapshot) = self.adaptive_snapshot.lock()
            {
                *snapshot = Some(AdaptiveThresholdStats::default());
            }
        }
    }

    /// JIT执行包装器
    pub fn run_with_jit(
        jit: &mut Jit,
        _interp: &mut Interpreter,
        mmu: &mut SoftMmu,
        block: &vm_ir::Block,
        pc: GuestAddr,
    ) -> vm_core::ExecResult {
        jit.set_pc(pc);
        jit.run(mmu, block)
    }

    /// 检查是否需要使用JIT
    pub fn should_use_jit(exec_mode: vm_core::ExecMode) -> bool {
        matches!(
            exec_mode,
            vm_core::ExecMode::HardwareAssisted | vm_core::ExecMode::JIT
        )
    }
}

// 条件模块：协程调度支持
#[cfg(feature = "performance")]
pub mod async_execution {
    use super::*;
    use vm_runtime::CoroutineScheduler;

    /// 协程执行状态
    pub struct CoroutineExecutionState {
        pub coroutine_scheduler: Option<Arc<Mutex<CoroutineScheduler>>>,
    }

    impl Default for CoroutineExecutionState {
        fn default() -> Self {
            Self::new()
        }
    }

    impl CoroutineExecutionState {
        pub fn new() -> Self {
            Self {
                coroutine_scheduler: None,
            }
        }

        pub fn with_scheduler(scheduler: Arc<Mutex<CoroutineScheduler>>) -> Self {
            Self {
                coroutine_scheduler: Some(scheduler),
            }
        }

        pub fn has_scheduler(&self) -> bool {
            self.coroutine_scheduler.is_some()
        }

        pub fn take_scheduler(&mut self) -> Option<Arc<Mutex<CoroutineScheduler>>> {
            self.coroutine_scheduler.take()
        }
    }

    /// 单vCPU异步执行（带JIT支持）
    pub async fn run_single_vcpu(
        ctx: &ExecutionContext,
        start_pc: GuestAddr,
        base_mmu: SoftMmu,
        debug: bool,
        exec_mode: vm_core::ExecMode,
        max_steps: usize,
    ) -> VmResult<()> {
        let mut local_mmu = base_mmu.clone();
        let mut interp = Interpreter::new();
        interp.set_reg(0, 0);
        let mut decoder = decoder_factory::create_decoder(ctx.guest_arch);
        let mut pc = start_pc;

        let hybrid = super::jit_execution::should_use_jit(exec_mode);
        let mut jit = ctx
            .perf_state
            .jit
            .as_ref()
            .and_then(|state| state.create_jit(hybrid));

        for step in 0..max_steps {
            tokio::select! {
                _ = async { if !ctx.run_flag.load(Ordering::Relaxed) { Err(()) } else { Ok(()) } } => {},
                _ = async { if ctx.pause_flag.load(Ordering::Relaxed) { Err(()) } else { Ok(()) } } => {},
                else => {}
            }

            match decoder.decode(&local_mmu, pc) {
                Ok(block) => {
                    let res = if let Some(j) = jit.as_mut() {
                        super::jit_execution::run_with_jit(
                            j,
                            &mut interp,
                            &mut local_mmu,
                            &block,
                            pc,
                        )
                    } else {
                        interp.run(&mut local_mmu, &block)
                    };

                    if debug && step % 1000 == 0 {
                        debug!("[Async Step {}] PC={:#x}", step, pc);
                        tdebug!(step=?step, pc=?pc, "service:run_async_tick");
                    }

                    if hybrid && let Some(jit_state) = &ctx.perf_state.jit {
                        jit_state.update_snapshot(step);
                    }

                    pc = res.next_pc;
                }
                Err(_) => {
                    break;
                }
            }
            tokio::task::yield_now().await;
        }

        info!("=== Async Execution Complete ===");
        tinfo!(pc=?pc, "service:run_async_complete");
        Ok(())
    }

    /// 执行单个vCPU异步任务（带JIT支持）
    ///
    /// # 参数
    ///
    /// * `local_mmu` - 本地 MMU 实例
    /// * `params` - VCPU 执行参数
    /// * `jit_config` - JIT 配置
    pub async fn execute_vcpu_with_jit(
        mut local_mmu: vm_mem::SoftMmu,
        params: VcpuExecuteParams,
        jit_config: JitConfigOption,
    ) {
        let mut interp = Interpreter::new();
        interp.set_reg(0, 0);

        let mut jit = jit_config.and_then(|(pool, cfg)| {
            if params.hybrid {
                Some(match (pool, cfg) {
                    (Some(_), Some(cfg)) => vm_engine::jit::Jit::with_adaptive_config(cfg),
                    (Some(_), None) => vm_engine::jit::Jit::new(),
                    (None, Some(cfg)) => vm_engine::jit::Jit::with_adaptive_config(cfg),
                    (None, None) => vm_engine::jit::Jit::new(),
                })
            } else {
                None
            }
        });

        let mut decoder = decoder_factory::create_decoder(params.guest_arch);
        let mut thread_pc = params.thread_pc;

        for step in 0..params.max_steps {
            if !params.run_flag.load(Ordering::Relaxed) {
                break;
            }
            if params.pause_flag.load(Ordering::Relaxed) {
                break;
            }

            match decoder.decode(&local_mmu, thread_pc) {
                Ok(block) => {
                    let res = if let Some(j) = jit.as_mut() {
                        super::jit_execution::run_with_jit(
                            j,
                            &mut interp,
                            &mut local_mmu,
                            &block,
                            thread_pc,
                        )
                    } else {
                        interp.run(&mut local_mmu, &block)
                    };

                    if params.debug && step % 1000 == 0 {
                        debug!(
                            "[CPU {} Async Step {}] PC={:#x}",
                            params.cpu_id, step, thread_pc
                        );
                        tdebug!(cpu=?params.cpu_id, step=?step, pc=?thread_pc, "service:run_async_tick_cpu");
                    }
                    thread_pc = res.next_pc;
                }
                Err(_) => {
                    break;
                }
            }
            tokio::task::yield_now().await;
        }
    }
}

// 基础异步执行模块（无性能扩展）
#[cfg(not(feature = "performance"))]
pub mod async_execution {
    use super::*;

    pub struct CoroutineExecutionState; // Dummy type

    pub async fn run_single_vcpu(
        ctx: &ExecutionContext,
        start_pc: GuestAddr,
        base_mmu: SoftMmu,
        debug: bool,
        _exec_mode: vm_core::ExecMode,
        max_steps: usize,
    ) -> VmResult<()> {
        let mut local_mmu = base_mmu.clone();
        let mut interp = Interpreter::new();
        interp.set_reg(0, 0);
        let mut decoder = decoder_factory::create_decoder(ctx.guest_arch);
        let mut pc = start_pc;

        for step in 0..max_steps {
            tokio::select! {
                _ = async { if !ctx.run_flag.load(Ordering::Relaxed) { Err(()) } else { Ok(()) } } => {},
                _ = async { if ctx.pause_flag.load(Ordering::Relaxed) { Err(()) } else { Ok(()) } } => {},
                else => {}
            }

            match decoder.decode(&local_mmu, pc) {
                Ok(block) => {
                    let res = interp.run(&mut local_mmu, &block);

                    if debug && step % 1000 == 0 {
                        debug!("[Async Step {}] PC={:#x}", step, pc);
                        tdebug!(step=?step, pc=?pc, "service:run_async_tick");
                    }

                    pc = res.next_pc;
                }
                Err(_) => {
                    break;
                }
            }
            tokio::task::yield_now().await;
        }

        info!("=== Async Execution Complete ===");
        tinfo!(pc=?pc, "service:run_async_complete");
        Ok(())
    }

    pub async fn execute_vcpu_with_jit(
        mut local_mmu: vm_mem::SoftMmu,
        debug: bool,
        mut thread_pc: GuestAddr,
        run_flag: Arc<AtomicBool>,
        pause_flag: Arc<AtomicBool>,
        guest_arch: GuestArch,
        cpu_id: usize,
        max_steps: usize,
        _hybrid: bool,
        _jit_config: Option<((), ())>,
    ) {
        let mut interp = Interpreter::new();
        interp.set_reg(0, 0);
        let mut decoder = decoder_factory::create_decoder(guest_arch);

        for step in 0..max_steps {
            if !run_flag.load(Ordering::Relaxed) {
                break;
            }
            if pause_flag.load(Ordering::Relaxed) {
                break;
            }

            match decoder.decode(&local_mmu, thread_pc) {
                Ok(block) => {
                    let res = interp.run(&mut local_mmu, &block);
                    if debug && step % 1000 == 0 {
                        debug!("[CPU {} Async Step {}] PC={:#x}", cpu_id, step, thread_pc);
                        tdebug!(cpu=?cpu_id, step=?step, pc=?thread_pc, "service:run_async_tick_cpu");
                    }
                    thread_pc = res.next_pc;
                }
                Err(_) => {
                    break;
                }
            }
            tokio::task::yield_now().await;
        }
    }
}

/// Type alias for trap handler to reduce type complexity
pub type TrapHandler = Arc<dyn Fn(&VmError, &mut Interpreter) -> ExecInterruptAction + Send + Sync>;

/// Type alias for IRQ policy to reduce type complexity
pub type IrqPolicy = Arc<dyn Fn(&mut Interpreter) -> ExecInterruptAction + Send + Sync>;

/// 执行上下文，包含执行循环所需的所有状态
///
/// 通过使用条件模块，避免在结构体字段级别使用feature gate
pub struct ExecutionContext {
    /// 运行标志
    pub run_flag: Arc<AtomicBool>,
    /// 暂停标志
    pub pause_flag: Arc<AtomicBool>,
    /// 目标架构
    pub guest_arch: GuestArch,
    /// 陷阱处理器
    pub trap_handler: Option<TrapHandler>,
    /// 中断策略
    pub irq_policy: Option<IrqPolicy>,
    /// 性能扩展状态（通过performance feature启用）
    #[cfg(feature = "performance")]
    pub perf_state: PerfExtState,
}

/// 性能扩展状态（仅在performance feature启用时可用）
#[cfg(feature = "performance")]
pub struct PerfExtState {
    /// JIT执行状态
    pub jit: Option<jit_execution::JitExecutionState>,
    /// 协程执行状态
    pub coroutine: Option<async_execution::CoroutineExecutionState>,
}

/// 性能扩展状态占位符（无性能扩展时）
#[cfg(not(feature = "performance"))]
pub struct PerfExtState;

#[cfg(feature = "performance")]
impl Default for PerfExtState {
    fn default() -> Self {
        Self::new()
    }
}

impl PerfExtState {
    pub fn new() -> Self {
        Self {
            jit: None,
            coroutine: None,
        }
    }

    pub fn with_jit_config(mut self, config: vm_engine::jit::AdaptiveThresholdConfig) -> Self {
        self.jit = Some(jit_execution::JitExecutionState::with_config(config));
        self
    }

    pub fn with_coroutine_scheduler(
        mut self,
        scheduler: Arc<Mutex<vm_runtime::CoroutineScheduler>>,
    ) -> Self {
        self.coroutine = Some(async_execution::CoroutineExecutionState::with_scheduler(
            scheduler,
        ));
        self
    }
}

impl ExecutionContext {
    #[cfg(feature = "performance")]
    pub fn new(guest_arch: GuestArch) -> Self {
        Self {
            run_flag: Arc::new(AtomicBool::new(false)),
            pause_flag: Arc::new(AtomicBool::new(false)),
            guest_arch,
            trap_handler: None,
            irq_policy: None,
            perf_state: PerfExtState::new(),
        }
    }

    #[cfg(not(feature = "performance"))]
    pub fn new(guest_arch: GuestArch) -> Self {
        Self {
            run_flag: Arc::new(AtomicBool::new(false)),
            pause_flag: Arc::new(AtomicBool::new(false)),
            guest_arch,
            trap_handler: None,
            irq_policy: None,
        }
    }

    #[cfg(feature = "performance")]
    pub fn with_jit_config(mut self, config: vm_engine::jit::AdaptiveThresholdConfig) -> Self {
        self.perf_state = self.perf_state.with_jit_config(config);
        self
    }

    #[cfg(feature = "performance")]
    pub fn with_coroutine_scheduler(
        mut self,
        scheduler: Arc<Mutex<vm_runtime::CoroutineScheduler>>,
    ) -> Self {
        self.perf_state = self.perf_state.with_coroutine_scheduler(scheduler);
        self
    }
}

/// 同步执行循环
pub fn run_sync(
    ctx: &ExecutionContext,
    start_pc: GuestAddr,
    base_mmu: SoftMmu,
    debug: bool,
    vcpu_count: usize,
) -> VmResult<()> {
    ctx.run_flag.store(true, Ordering::Relaxed);
    ctx.pause_flag.store(false, Ordering::Relaxed);

    let max_steps = 1_000_000;
    let pc = start_pc;

    info!("Starting execution from PC={:#x}", pc);

    if vcpu_count <= 1 {
        run_single_vcpu_sync(ctx, start_pc, base_mmu, debug, max_steps)
    } else {
        run_multi_vcpu_sync(ctx, pc, base_mmu, debug, vcpu_count, max_steps)
    }
}

/// 单vCPU同步执行
fn run_single_vcpu_sync(
    ctx: &ExecutionContext,
    start_pc: GuestAddr,
    base_mmu: SoftMmu,
    debug: bool,
    max_steps: usize,
) -> VmResult<()> {
    let mut local_mmu = base_mmu.clone();
    let mut local_interpreter = Interpreter::new();
    local_interpreter.set_reg(0, 0);
    let mut local_decoder = decoder_factory::create_decoder(ctx.guest_arch);
    let mut pc = start_pc;

    for step in 0..max_steps {
        if !ctx.run_flag.load(Ordering::Relaxed) {
            break;
        }
        if ctx.pause_flag.load(Ordering::Relaxed) {
            break;
        }
        match local_decoder.decode(&local_mmu, pc) {
            Ok(block) => {
                let _res = local_interpreter.run(&mut local_mmu, &block);
                if matches!(_res.status, ExecStatus::InterruptPending) {
                    if let Some(p) = &ctx.irq_policy {
                        let _ = p(&mut local_interpreter);
                    }
                    pc = _res.next_pc;
                    continue;
                }
                if let ExecStatus::Fault(ref f) = _res.status {
                    if let Some(h) = &ctx.trap_handler {
                        match h(&VmError::Execution(f.clone()), &mut local_interpreter) {
                            ExecInterruptAction::Continue | ExecInterruptAction::InjectState => {
                                local_interpreter.resume_from_trap();
                                pc = local_interpreter.get_pc();
                                continue;
                            }
                            ExecInterruptAction::Retry => {
                                continue;
                            }
                            ExecInterruptAction::Mask => {
                                break;
                            }
                            ExecInterruptAction::Deliver | ExecInterruptAction::Abort => {
                                break;
                            }
                        }
                    } else {
                        // 默认 Trap 向量策略：按照 mtvec/stvec 设置 PC
                        let mode = local_interpreter.get_priv_mode();
                        let (vec, cause) = if mode == 3 {
                            (
                                local_interpreter.read_csr(0x305),
                                local_interpreter.read_csr(0x342),
                            )
                        } else {
                            (
                                local_interpreter.read_csr(0x105),
                                local_interpreter.read_csr(0x142),
                            )
                        };
                        let base = vec & !0x3;
                        let is_interrupt = (cause >> 63) != 0;
                        let mtvec_mode = vec & 0x3;
                        let target = if is_interrupt && mtvec_mode == 1 {
                            base.wrapping_add(4 * (cause & 0xfff))
                        } else {
                            base
                        };
                        local_interpreter.set_pc(vm_core::GuestAddr(target));
                        pc = vm_core::GuestAddr(target);
                        continue;
                    }
                }

                if debug && step % 1000 == 0 {
                    debug!("[Step {}] PC={:#x}", step, pc);
                }

                match &block.term {
                    Terminator::Jmp { target } => {
                        if *target == pc {
                            info!("\n[Step {}] PC={:#x}: HALT (infinite loop)", step, pc);
                            break;
                        }
                        pc = *target;
                    }
                    Terminator::CondJmp {
                        cond,
                        target_true,
                        target_false,
                    } => {
                        if local_interpreter.get_reg(*cond) != 0 {
                            pc = *target_true;
                        } else {
                            pc = *target_false;
                        }
                    }
                    Terminator::JmpReg { base, offset } => {
                        let base_val = local_interpreter.get_reg(*base);
                        pc = vm_core::GuestAddr((base_val as i64 + offset) as u64);
                    }
                    Terminator::Ret => {
                        info!("\n[Step {}] PC={:#x}: RET", step, pc);
                        break;
                    }
                    Terminator::Fault { cause } => {
                        error!("\n[Step {}] PC={:#x}: FAULT (cause={})", step, pc, cause);
                        break;
                    }
                    _ => pc += 4,
                }
            }
            Err(e) => {
                error!("Decode error at {:#x}: {:?}", pc, e);
                break;
            }
        }
    }

    info!("=== Execution Complete ===");
    tinfo!(pc=?pc, steps=?max_steps, "service:run_complete");
    Ok(())
}

/// 多vCPU同步执行
#[cfg(not(feature = "performance"))]
fn run_multi_vcpu_sync(
    ctx: &ExecutionContext,
    pc: GuestAddr,
    base_mmu: SoftMmu,
    debug: bool,
    vcpu_count: usize,
    max_steps: usize,
) -> VmResult<()> {
    // 使用tokio::spawn替代std::thread::spawn
    let rt_result = tokio::runtime::Handle::try_current();
    if let Ok(rt_handle) = rt_result {
        run_with_tokio_handle(rt_handle, ctx, pc, base_mmu, debug, vcpu_count, max_steps)
    } else {
        run_with_new_runtime(ctx, pc, base_mmu, debug, vcpu_count, max_steps)
    }
}

/// 多vCPU同步执行（性能模式）
#[cfg(feature = "performance")]
fn run_multi_vcpu_sync(
    ctx: &ExecutionContext,
    pc: GuestAddr,
    base_mmu: SoftMmu,
    debug: bool,
    vcpu_count: usize,
    max_steps: usize,
) -> VmResult<()> {
    // 尝试使用协程调度器
    if let Some(coroutine_state) = &ctx.perf_state.coroutine
        && coroutine_state.has_scheduler()
    {
        return run_with_coroutine_scheduler(ctx, pc, base_mmu, debug, vcpu_count, max_steps);
    }

    // 回退到tokio::spawn
    let rt_result = tokio::runtime::Handle::try_current();
    if let Ok(rt_handle) = rt_result {
        run_with_tokio_handle(rt_handle, ctx, pc, base_mmu, debug, vcpu_count, max_steps)
    } else {
        run_with_new_runtime(ctx, pc, base_mmu, debug, vcpu_count, max_steps)
    }
}

/// 使用协程调度器执行多vCPU任务
#[cfg(feature = "performance")]
fn run_with_coroutine_scheduler(
    ctx: &ExecutionContext,
    pc: GuestAddr,
    base_mmu: SoftMmu,
    debug: bool,
    vcpu_count: usize,
    max_steps: usize,
) -> VmResult<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!(
                    "Failed to create tokio runtime for coroutine scheduler: {}",
                    e
                ),
                module: "vm_service::execution".to_string(),
            })
        })?;

    rt.block_on(async {
        let mut handles = Vec::with_capacity(vcpu_count);
        for i in 0..vcpu_count {
            let local_mmu = base_mmu.clone();
            let debug_local = debug;
            let thread_pc = pc;
            let run_flag = Arc::clone(&ctx.run_flag);
            let pause_flag = Arc::clone(&ctx.pause_flag);
            let guest_arch = ctx.guest_arch;

            handles.push(tokio::task::spawn(async move {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to create tokio runtime in execution task");

                rt.block_on(async move {
                    execute_vcpu_task(
                        local_mmu,
                        VcpuExecuteParams {
                            debug: debug_local,
                            thread_pc,
                            run_flag,
                            pause_flag,
                            guest_arch,
                            cpu_id: i,
                            max_steps,
                            hybrid: false,
                        },
                    )
                    .await
                });
            }));
        }

        for handle in handles {
            let _ = handle.await;
        }
        Ok(())
    })
}

/// 使用现有的tokio句柄执行
fn run_with_tokio_handle(
    rt_handle: tokio::runtime::Handle,
    ctx: &ExecutionContext,
    pc: GuestAddr,
    base_mmu: SoftMmu,
    debug: bool,
    vcpu_count: usize,
    max_steps: usize,
) -> VmResult<()> {
    let mut handles = Vec::with_capacity(vcpu_count);
    for i in 0..vcpu_count {
        let local_mmu = base_mmu.clone();
        let debug_local = debug;
        let thread_pc = pc;
        let run_flag = Arc::clone(&ctx.run_flag);
        let pause_flag = Arc::clone(&ctx.pause_flag);
        let guest_arch = ctx.guest_arch;

        handles.push(rt_handle.spawn(async move {
            execute_vcpu_task(
                local_mmu,
                VcpuExecuteParams {
                    debug: debug_local,
                    thread_pc,
                    run_flag,
                    pause_flag,
                    guest_arch,
                    cpu_id: i,
                    max_steps,
                    hybrid: false,
                },
            )
            .await
        }));
    }

    for task_handle in handles {
        let _ = rt_handle.block_on(task_handle);
    }

    info!("=== Execution Complete ===");
    tinfo!(pc=?pc, steps=?max_steps, "service:run_complete");
    Ok(())
}

/// 使用新的tokio运行时执行
fn run_with_new_runtime(
    ctx: &ExecutionContext,
    pc: GuestAddr,
    base_mmu: SoftMmu,
    debug: bool,
    vcpu_count: usize,
    max_steps: usize,
) -> VmResult<()> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| {
        VmError::Core(vm_core::CoreError::Internal {
            message: format!("Failed to create tokio runtime: {}", e),
            module: "vm_service".to_string(),
        })
    })?;

    rt.block_on(async {
        let mut handles = Vec::with_capacity(vcpu_count);
        for i in 0..vcpu_count {
            let local_mmu = base_mmu.clone();
            let debug_local = debug;
            let thread_pc = pc;
            let run_flag = Arc::clone(&ctx.run_flag);
            let pause_flag = Arc::clone(&ctx.pause_flag);
            let guest_arch = ctx.guest_arch;

            handles.push(tokio::spawn(async move {
                execute_vcpu_task(
                    local_mmu,
                    VcpuExecuteParams {
                        debug: debug_local,
                        thread_pc,
                        run_flag,
                        pause_flag,
                        guest_arch,
                        cpu_id: i,
                        max_steps,
                        hybrid: false, // Not used in non-JIT execution
                    },
                )
                .await
            }));
        }

        for handle in handles {
            let _ = handle.await;
        }
        Ok(())
    })
}

/// 单个vCPU的异步执行任务
async fn execute_vcpu_task(mut local_mmu: vm_mem::SoftMmu, params: VcpuExecuteParams) {
    let mut interp = Interpreter::new();
    interp.set_reg(0, 0);
    let mut decoder = decoder_factory::create_decoder(params.guest_arch);
    let mut thread_pc = params.thread_pc;

    for step in 0..params.max_steps {
        if !params.run_flag.load(Ordering::Relaxed) {
            break;
        }
        if params.pause_flag.load(Ordering::Relaxed) {
            break;
        }

        match decoder.decode(&local_mmu, thread_pc) {
            Ok(block) => {
                let _res = interp.run(&mut local_mmu, &block);
                if matches!(_res.status, ExecStatus::InterruptPending) {
                    thread_pc = _res.next_pc;
                    continue;
                }
                if let ExecStatus::Fault(ref _f) = _res.status {}
                if params.debug && step % 1000 == 0 {
                    debug!("[CPU {} Step {}] PC={:#x}", params.cpu_id, step, thread_pc);
                }
                match &block.term {
                    Terminator::Jmp { target } => {
                        if *target == thread_pc {
                            break;
                        }
                        thread_pc = *target;
                    }
                    Terminator::CondJmp {
                        cond,
                        target_true,
                        target_false,
                    } => {
                        if interp.get_reg(*cond) != 0 {
                            thread_pc = *target_true;
                        } else {
                            thread_pc = *target_false;
                        }
                    }
                    Terminator::JmpReg { base, offset } => {
                        let base_val = interp.get_reg(*base);
                        thread_pc = vm_core::GuestAddr((base_val as i64 + offset) as u64);
                    }
                    Terminator::Ret | Terminator::Fault { .. } => {
                        break;
                    }
                    _ => thread_pc += 4,
                }
            }
            Err(_) => {
                break;
            }
        }
        tokio::task::yield_now().await;
    }
}

/// 异步执行循环
pub async fn run_async(
    ctx: &ExecutionContext,
    start_pc: GuestAddr,
    base_mmu: SoftMmu,
    debug: bool,
    vcpu_count: usize,
    exec_mode: vm_core::ExecMode,
) -> VmResult<()> {
    ctx.run_flag.store(true, Ordering::Relaxed);
    ctx.pause_flag.store(false, Ordering::Relaxed);

    let max_steps = 1_000_000;
    let pc = start_pc;
    info!("Starting async execution from PC={:#x}", pc);
    tinfo!(pc=?pc, "service:run_async_start");

    if vcpu_count <= 1 {
        run_single_vcpu_async(ctx, start_pc, base_mmu, debug, exec_mode, max_steps).await
    } else {
        run_multi_vcpu_async(ctx, pc, base_mmu, debug, vcpu_count, exec_mode, max_steps).await
    }
}

/// 单vCPU异步执行
async fn run_single_vcpu_async(
    ctx: &ExecutionContext,
    start_pc: GuestAddr,
    base_mmu: SoftMmu,
    debug: bool,
    exec_mode: vm_core::ExecMode,
    max_steps: usize,
) -> VmResult<()> {
    async_execution::run_single_vcpu(ctx, start_pc, base_mmu, debug, exec_mode, max_steps).await
}

/// 多vCPU异步执行
async fn run_multi_vcpu_async(
    ctx: &ExecutionContext,
    pc: GuestAddr,
    base_mmu: SoftMmu,
    debug: bool,
    vcpu_count: usize,
    exec_mode: vm_core::ExecMode,
    max_steps: usize,
) -> VmResult<()> {
    #[cfg(feature = "performance")]
    let use_coroutine = ctx
        .perf_state
        .coroutine
        .as_ref()
        .map(|s| s.has_scheduler())
        .unwrap_or(false);

    #[cfg(not(feature = "performance"))]
    let use_coroutine = false;

    if use_coroutine {
        #[cfg(feature = "performance")]
        return run_multi_vcpu_with_coroutines(
            ctx, pc, base_mmu, debug, vcpu_count, exec_mode, max_steps,
        )
        .await;

        #[cfg(not(feature = "performance"))]
        unreachable!()
    }

    run_multi_vcpu_with_tokio(ctx, pc, base_mmu, debug, vcpu_count, exec_mode, max_steps).await
}

/// 使用协程调度器执行多vCPU异步任务
#[cfg(feature = "performance")]
async fn run_multi_vcpu_with_coroutines(
    ctx: &ExecutionContext,
    pc: GuestAddr,
    base_mmu: SoftMmu,
    debug: bool,
    vcpu_count: usize,
    exec_mode: vm_core::ExecMode,
    max_steps: usize,
) -> VmResult<()> {
    let hybrid = jit_execution::should_use_jit(exec_mode);
    let mut handles = Vec::with_capacity(vcpu_count);

    for i in 0..vcpu_count {
        let mmu_base = base_mmu.clone();
        let debug_local = debug;
        let thread_pc = pc;
        let run_flag = Arc::clone(&ctx.run_flag);
        let pause_flag = Arc::clone(&ctx.pause_flag);
        let guest_arch = ctx.guest_arch;

        let jit_config = ctx.perf_state.jit.as_ref().and_then(|state| {
            if hybrid {
                Some((state.code_pool.clone(), state.adaptive_config.clone()))
            } else {
                None
            }
        });

        handles.push(tokio::task::spawn(async move {
            async_execution::execute_vcpu_with_jit(
                mmu_base,
                VcpuExecuteParams {
                    debug: debug_local,
                    thread_pc,
                    run_flag,
                    pause_flag,
                    guest_arch,
                    cpu_id: i,
                    max_steps,
                    hybrid,
                },
                jit_config,
            )
            .await
        }));
    }

    for handle in handles {
        let _ = handle.await;
    }

    info!("=== Async Execution Complete ===");
    tinfo!(pc=?pc, "service:run_async_complete");
    Ok(())
}

/// 使用tokio::spawn执行多vCPU异步任务
async fn run_multi_vcpu_with_tokio(
    ctx: &ExecutionContext,
    pc: GuestAddr,
    base_mmu: SoftMmu,
    debug: bool,
    vcpu_count: usize,
    exec_mode: vm_core::ExecMode,
    max_steps: usize,
) -> VmResult<()> {
    #[cfg(feature = "performance")]
    let (hybrid, jit_config_clone) = {
        let hybrid = jit_execution::should_use_jit(exec_mode);
        let jit_config = ctx.perf_state.jit.as_ref().and_then(|state| {
            if hybrid {
                Some((state.code_pool.clone(), state.adaptive_config.clone()))
            } else {
                None
            }
        });
        (hybrid, jit_config)
    };

    #[cfg(not(feature = "performance"))]
    let (hybrid, jit_config_clone) = (false, None);

    let mut handles = Vec::with_capacity(vcpu_count);

    for i in 0..vcpu_count {
        let mmu_base = base_mmu.clone();
        let debug_local = debug;
        let thread_pc = pc;
        let run_flag = Arc::clone(&ctx.run_flag);
        let pause_flag = Arc::clone(&ctx.pause_flag);
        let guest_arch = ctx.guest_arch;
        let jit_config = jit_config_clone.clone();

        handles.push(tokio::spawn(async move {
            async_execution::execute_vcpu_with_jit(
                mmu_base,
                VcpuExecuteParams {
                    debug: debug_local,
                    thread_pc,
                    run_flag,
                    pause_flag,
                    guest_arch,
                    cpu_id: i,
                    max_steps,
                    hybrid,
                },
                jit_config,
            )
            .await
        }));
    }

    for h in handles {
        let _ = h.await;
    }

    info!("=== Async Execution Complete ===");
    tinfo!(pc=?pc, "service:run_async_complete");
    Ok(())
}
