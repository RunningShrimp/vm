//! 执行循环管理模块

use log::{debug, error, info};
use std::collections::HashMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
// 移除std::thread，改用tokio::spawn
use tracing::{debug as tdebug, info as tinfo};
use vm_core::{
    ExecStatus, GuestAddr, MemoryError, VmError, VmResult,
};
use vm_engine_interpreter::{ExecInterruptAction, Interpreter};
use vm_engine_jit::{AdaptiveThresholdConfig, AdaptiveThresholdStats, CodePtr, Jit};
use vm_frontend_riscv64::RiscvDecoder;
use vm_ir::Terminator;
use vm_mem::SoftMmu;

#[cfg(feature = "async")]
use vm_runtime::CoroutinePool;

/// 执行上下文，包含执行循环所需的所有状态
pub struct ExecutionContext {
    /// 运行标志
    pub run_flag: Arc<AtomicBool>,
    /// 暂停标志
    pub pause_flag: Arc<AtomicBool>,
    /// 陷阱处理器
    pub trap_handler: Option<Arc<dyn Fn(&VmError, &mut Interpreter) -> ExecInterruptAction + Send + Sync>>,
    /// 中断策略
    pub irq_policy: Option<Arc<dyn Fn(&mut Interpreter) -> ExecInterruptAction + Send + Sync>>,
    /// JIT代码池
    pub code_pool: Option<Arc<Mutex<HashMap<GuestAddr, CodePtr>>>>,
    /// 自适应快照
    pub adaptive_snapshot: Arc<Mutex<Option<AdaptiveThresholdStats>>>,
    /// 自适应配置
    pub adaptive_config: Option<AdaptiveThresholdConfig>,
    /// 协程池（可选，用于优化多vCPU执行）
    #[cfg(feature = "async")]
    pub coroutine_pool: Option<Arc<CoroutinePool>>,
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
    let mut pc = start_pc;

    info!("Starting execution from PC={:#x}", pc);

    if vcpu_count <= 1 {
        let mut local_mmu = base_mmu.clone();
        let mut local_interpreter = Interpreter::new();
        local_interpreter.set_reg(0, 0);
        let mut local_decoder = RiscvDecoder;

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
                            match h(f, &mut local_interpreter) {
                                ExecInterruptAction::Continue
                                | ExecInterruptAction::InjectState => {
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
                            local_interpreter.set_pc(target);
                            pc = target;
                            continue;
                        }
                        break;
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
                            pc = (base_val as i64 + offset) as u64;
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
    } else {
        // 多vCPU执行：优先使用协程池（如果可用），否则使用线程
        #[cfg(feature = "async")]
        if let Some(pool) = ctx.coroutine_pool.as_ref() {
            // 使用协程池执行多vCPU任务
            let mut handles = Vec::with_capacity(vcpu_count);
            for _i in 0..vcpu_count {
                let mut local_mmu = base_mmu.clone();
                let debug_local = debug;
                let mut thread_pc = pc;
                let run_flag = Arc::clone(&ctx.run_flag);
                let pause_flag = Arc::clone(&ctx.pause_flag);
                
                let task = async move {
                    let mut interp = Interpreter::new();
                    interp.set_reg(0, 0);
                    let mut decoder = RiscvDecoder;
                    for step in 0..max_steps {
                        if !run_flag.load(Ordering::Relaxed) {
                            break;
                        }
                        if pause_flag.load(Ordering::Relaxed) {
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
                                if debug_local && step % 1000 == 0 {
                                    debug!("[CPU {} Step {}] PC={:#x}", _i, step, thread_pc);
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
                                        thread_pc = (base_val as i64 + offset) as u64;
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
                        // 让出控制权，允许其他协程执行
                        tokio::task::yield_now().await;
                    }
                };
                
                // 使用协程池提交任务
                if let Ok(handle) = pool.spawn(task).await {
                    handles.push(handle);
                }
            }
            // 等待所有协程完成
            for handle in handles {
                let _ = handle.await;
            }
            return Ok(());
        }
        
        // 使用tokio::spawn替代std::thread::spawn
        // 创建tokio运行时（如果当前不在运行时中）
        let rt = tokio::runtime::Handle::try_current();
        if let Ok(handle) = rt {
            // 在tokio运行时中，直接使用spawn
            let mut handles = Vec::with_capacity(vcpu_count);
            for _i in 0..vcpu_count {
                let mut local_mmu = base_mmu.clone();
                let debug_local = debug;
                let mut thread_pc = pc;
                let run_flag = Arc::clone(&ctx.run_flag);
                let pause_flag = Arc::clone(&ctx.pause_flag);
                handles.push(handle.spawn(async move {
                    let mut interp = Interpreter::new();
                    interp.set_reg(0, 0);
                    let mut decoder = RiscvDecoder;
                    for step in 0..max_steps {
                        if !run_flag.load(Ordering::Relaxed) {
                            break;
                        }
                        if pause_flag.load(Ordering::Relaxed) {
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
                                if debug_local && step % 1000 == 0 {
                                    debug!("[CPU {} Step {}] PC={:#x}", _i, step, thread_pc);
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
                                        thread_pc = (base_val as i64 + offset) as u64;
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
                        // 让出控制权，允许其他协程执行
                        tokio::task::yield_now().await;
                    }
                }));
            }
            // 等待所有协程完成
            for handle in handles {
                let _ = handle.await;
            }
        } else {
            // 不在tokio运行时中，创建临时运行时
            let rt = tokio::runtime::Runtime::new().map_err(|e| {
                VmError::InternalError(format!("Failed to create tokio runtime: {}", e))
            })?;
            rt.block_on(async {
                let mut handles = Vec::with_capacity(vcpu_count);
                for _i in 0..vcpu_count {
                    let mut local_mmu = base_mmu.clone();
                    let debug_local = debug;
                    let mut thread_pc = pc;
                    let run_flag = Arc::clone(&ctx.run_flag);
                    let pause_flag = Arc::clone(&ctx.pause_flag);
                    handles.push(tokio::spawn(async move {
                        let mut interp = Interpreter::new();
                        interp.set_reg(0, 0);
                        let mut decoder = RiscvDecoder;
                        for step in 0..max_steps {
                            if !run_flag.load(Ordering::Relaxed) {
                                break;
                            }
                            if pause_flag.load(Ordering::Relaxed) {
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
                                    if debug_local && step % 1000 == 0 {
                                        debug!("[CPU {} Step {}] PC={:#x}", _i, step, thread_pc);
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
                                            thread_pc = (base_val as i64 + offset) as u64;
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
                            // 让出控制权，允许其他协程执行
                            tokio::task::yield_now().await;
                        }
                    }));
                }
                // 等待所有协程完成
                for handle in handles {
                    let _ = handle.await;
                }
            });
        }
    }

    info!("=== Execution Complete ===");
    tinfo!(pc=?pc, steps=?max_steps, "service:run_complete");
    Ok(())
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
    let mut pc = start_pc;
    info!("Starting async execution from PC={:#x}", pc);
    tinfo!(pc=?pc, "service:run_async_start");

    if vcpu_count <= 1 {
        let mut local_mmu = base_mmu.clone();
        let hybrid = matches!(
            exec_mode,
            vm_core::ExecMode::Hybrid | vm_core::ExecMode::Jit
        );
        let mut interp = Interpreter::new();
        interp.set_reg(0, 0);
        let mut jit = if hybrid {
            let pool = ctx.code_pool.as_ref().cloned();
            Some(match (pool, ctx.adaptive_config.clone()) {
                (Some(_p), Some(cfg)) => Jit::with_adaptive_config(cfg),
                (Some(_p), None) => Jit::new(),
                (None, Some(cfg)) => Jit::with_adaptive_config(cfg),
                (None, None) => Jit::new(),
            })
        } else {
            None
        };
        let mut decoder = RiscvDecoder;
        for step in 0..max_steps {
            tokio::select! {
                _ = async { if !ctx.run_flag.load(Ordering::Relaxed) { Err(()) } else { Ok(()) } } => {},
                _ = async { if ctx.pause_flag.load(Ordering::Relaxed) { Err(()) } else { Ok(()) } } => {},
                else => {}
            }
            match decoder.decode(&local_mmu, pc) {
                Ok(block) => {
                    let res = if let Some(j) = jit.as_mut() {
                        j.set_pc(pc);
                        j.run(&mut local_mmu, &block)
                    } else {
                        interp.run(&mut local_mmu, &block)
                    };
                    if debug && step % 1000 == 0 {
                        debug!("[Async Step {}] PC={:#x}", step, pc);
                        tdebug!(step=?step, pc=?pc, "service:run_async_tick");
                    }
                    if hybrid && step % 1000 == 0 {
                        if let Some(_j) = jit.as_ref() {
                            if let Ok(mut snapshot) = ctx.adaptive_snapshot.lock() {
                                *snapshot = Some(AdaptiveThresholdStats::default());
                            }
                        }
                    }
                    pc = res.next_pc;
                }
                Err(_) => {
                    break;
                }
            }
            tokio::task::yield_now().await;
        }
    } else {
        // 多vCPU异步执行：优先使用协程池（如果可用）
        #[cfg(feature = "async")]
        if let Some(pool) = ctx.coroutine_pool.as_ref() {
            // 使用协程池执行多vCPU任务
            let mut handles = Vec::with_capacity(vcpu_count);
            let pool_main = ctx.code_pool.as_ref().cloned();
            let run_flag_main = Arc::clone(&ctx.run_flag);
            let pause_flag_main = Arc::clone(&ctx.pause_flag);
            let hybrid_main = matches!(
                exec_mode,
                vm_core::ExecMode::Hybrid | vm_core::ExecMode::Jit
            );
            
            for i in 0..vcpu_count {
                let mmu_base = base_mmu.clone();
                let debug_local = debug;
                let mut thread_pc = pc;
                let run_flag = Arc::clone(&run_flag_main);
                let pause_flag = Arc::clone(&pause_flag_main);
                let pool_iter = pool_main.as_ref().map(|p| p.clone());
                let cfg_iter = ctx.adaptive_config.clone();
                
                let task = async move {
                    let mut interp = Interpreter::new();
                    interp.set_reg(0, 0);
                    let hybrid = hybrid_main;
                    let mut jit = if hybrid {
                        Some(match (pool_iter, cfg_iter) {
                            (Some(_p), Some(cfg)) => Jit::with_adaptive_config(cfg),
                            (Some(_p), None) => Jit::new(),
                            (None, Some(cfg)) => Jit::with_adaptive_config(cfg),
                            (None, None) => Jit::new(),
                        })
                    } else {
                        None
                    };
                    let mut decoder = RiscvDecoder;
                    let mut local_mmu = mmu_base;
                    for step in 0..max_steps {
                        if !run_flag.load(Ordering::Relaxed) {
                            break;
                        }
                        if pause_flag.load(Ordering::Relaxed) {
                            break;
                        }
                        match decoder.decode(&local_mmu, thread_pc) {
                            Ok(block) => {
                                let res = if let Some(j) = jit.as_mut() {
                                    j.set_pc(thread_pc);
                                    j.run(&mut local_mmu, &block)
                                } else {
                                    interp.run(&mut local_mmu, &block)
                                };
                                if debug_local && step % 1000 == 0 {
                                    debug!("[CPU {} Async Step {}] PC={:#x}", i, step, thread_pc);
                                    tdebug!(cpu=?i, step=?step, pc=?thread_pc, "service:run_async_tick_cpu");
                                }
                                thread_pc = res.next_pc;
                            }
                            Err(_) => {
                                break;
                            }
                        }
                        tokio::task::yield_now().await;
                    }
                };
                
                // 使用协程池提交任务（高优先级，因为是关键路径）
                // 注意：CoroutinePool的spawn方法返回Result，需要处理错误
                match pool.spawn(task).await {
                    Ok(handle) => handles.push(handle),
                    Err(e) => {
                        tracing::warn!("Failed to spawn coroutine task: {}, falling back to tokio::spawn", e);
                        // 回退到tokio::spawn
                        handles.push(tokio::spawn(task));
                    }
                }
            }
            
            // 等待所有协程完成
            for handle in handles {
                let _ = handle.await;
            }
            return Ok(());
        }
        
        // 后备方案：直接使用tokio::spawn（保留用于向后兼容）
        let mut handles = Vec::with_capacity(vcpu_count);
        let pool_main = ctx.code_pool.as_ref().cloned();
        let run_flag_main = Arc::clone(&ctx.run_flag);
        let pause_flag_main = Arc::clone(&ctx.pause_flag);
        let hybrid_main = matches!(
            exec_mode,
            vm_core::ExecMode::Hybrid | vm_core::ExecMode::Jit
        );
        for i in 0..vcpu_count {
            let mmu_base = base_mmu.clone();
            let debug_local = debug;
            let mut thread_pc = pc;
            let run_flag = Arc::clone(&run_flag_main);
            let pause_flag = Arc::clone(&pause_flag_main);
            let pool_iter = pool_main.as_ref().map(|p| p.clone());
            let cfg_iter = ctx.adaptive_config.clone();
            handles.push(tokio::spawn(async move {
                let mut interp = Interpreter::new();
                interp.set_reg(0, 0);
                let hybrid = hybrid_main;
                let mut jit = if hybrid {
                    Some(match (pool_iter, cfg_iter) {
                        (Some(_p), Some(cfg)) => Jit::with_adaptive_config(cfg),
                        (Some(_p), None) => Jit::new(),
                        (None, Some(cfg)) => Jit::with_adaptive_config(cfg),
                        (None, None) => Jit::new(),
                    })
                } else {
                    None
                };
                let mut decoder = RiscvDecoder;
                let mut local_mmu = mmu_base;
                for step in 0..max_steps {
                    if !run_flag.load(Ordering::Relaxed) {
                        break;
                    }
                    if pause_flag.load(Ordering::Relaxed) {
                        break;
                    }
                    match decoder.decode(&local_mmu, thread_pc) {
                        Ok(block) => {
                            let res = if let Some(j) = jit.as_mut() {
                                j.set_pc(thread_pc);
                                j.run(&mut local_mmu, &block)
                            } else {
                                interp.run(&mut local_mmu, &block)
                            };
                            if debug_local && step % 1000 == 0 {
                                debug!("[CPU {} Async Step {}] PC={:#x}", i, step, thread_pc);
                                tdebug!(cpu=?i, step=?step, pc=?thread_pc, "service:run_async_tick_cpu");
                            }
                            thread_pc = res.next_pc;
                        }
                        Err(_) => {
                            break;
                        }
                    }
                    tokio::task::yield_now().await;
                }
            }));
        }
        for h in handles {
            let _ = h.await;
        }
    }
    info!("=== Async Execution Complete ===");
    tinfo!(pc=?pc, "service:run_async_complete");
    Ok(())
}

