use std::sync::Arc;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use thiserror::Error;
use tracing::{info as tinfo, debug as tdebug};
use log::{info, error, debug};

use vm_core::{VirtualMachine, VmConfig, Decoder, ExecutionEngine, ExecStatus, Fault, GuestAddr};
use vm_engine_jit::Jit;
use vm_engine_interpreter::ExecInterruptAction;
use vm_ir::{IRBlock, Terminator};
use vm_mem::SoftMmu;
use vm_frontend_riscv64::RiscvDecoder;
use vm_engine_interpreter::Interpreter;
use vm_device::block::{VirtioBlock, VirtioBlockMmio};
use vm_device::clint::{Clint, ClintMmio};
use vm_device::plic::{Plic, PlicMmio};
use vm_device::virtio_ai::{VirtioAi, VirtioAiMmio};
use vm_device::gpu_virt::GpuManager;

pub struct VmService {
    pub vm: VirtualMachine<IRBlock>,
    // 注意：decoder 现在不再使用，因为具体的 decoder 与执行引擎配对
    // 具体的 decoder 在各个引擎内部实现 (Interpreter, Jit, Hybrid)
    interpreter: Interpreter,
    // We keep the interpreter here for now, matching the original cli structure.
    // In the future, this should be managed via vm.vcpus
    // 具体将来通过 vm.vcpus 管理
    run_flag: Arc<AtomicBool>,
    pause_flag: Arc<AtomicBool>,
    trap_handler: Option<Arc<dyn Fn(&Fault, &mut Interpreter) -> ExecInterruptAction + Send + Sync>>, 
    irq_policy: Option<Arc<dyn Fn(&mut Interpreter) -> ExecInterruptAction + Send + Sync>>, 
    code_pool: Option<Arc<Mutex<HashMap<GuestAddr, vm_engine_jit::CodePtr>>>>,
    adaptive_snapshot: Option<vm_engine_jit::AdaptiveThresholdStats>,
    adaptive_config: Option<vm_engine_jit::AdaptiveThresholdConfig>,
}

impl VmService {
    pub async fn new(config: VmConfig, gpu_backend: Option<String>) -> Result<Self, ServiceError> {
        info!("Initializing VM Service with config: {:?}", config);
        tinfo!(guest_arch=?config.guest_arch, vcpus=?config.vcpu_count, mem=?config.memory_size, exec=?config.exec_mode, "service:new");

        // GPU Initialization
        let mut gpu_manager = GpuManager::new();
        if let Some(backend_name) = &gpu_backend {
             gpu_manager.select_backend_by_name(backend_name)
                 .map_err(|e| ServiceError::GpuInit(e.to_string()))?;
        } else {
             gpu_manager.auto_select_backend();
        }
        
        gpu_manager.init_selected_backend()
             .map_err(|e| ServiceError::GpuInit(e.to_string()))?;

        // JIT adaptive and pool env config
        let mut adaptive_cfg = vm_engine_jit::AdaptiveThresholdConfig::default();
        let mut share_pool = std::env::var("VM_JIT_SHARE_POOL").map(|s| !(s=="0" || s.eq_ignore_ascii_case("false"))).unwrap_or(true);
        if let Ok(cfg_path) = std::env::var("VM_SERVICE_CONFIG") {
            if let Ok(text) = std::fs::read_to_string(&cfg_path) {
                for line in text.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') { continue; }
                    if let Some((k, v)) = line.split_once('=') {
                        let k = k.trim(); let v = v.trim();
                        match k {
                            "VM_JIT_MIN_THRESHOLD" => if let Ok(n) = v.parse::<u64>() { adaptive_cfg.min_threshold = n; },
                            "VM_JIT_MAX_THRESHOLD" => if let Ok(n) = v.parse::<u64>() { adaptive_cfg.max_threshold = n; },
                            "VM_JIT_SAMPLE_WINDOW" => if let Ok(n) = v.parse::<usize>() { adaptive_cfg.sample_window = n; },
                            "VM_JIT_COMPILE_TIME_WEIGHT" => if let Ok(n) = v.parse::<f64>() { adaptive_cfg.compile_time_weight = n; },
                            "VM_JIT_EXEC_BENEFIT_WEIGHT" => if let Ok(n) = v.parse::<f64>() { adaptive_cfg.exec_benefit_weight = n; },
                            "VM_JIT_SHARE_POOL" => { share_pool = !(v=="0" || v.eq_ignore_ascii_case("false")); },
                            _ => {}
                        }
                    }
                }
            }
        }
        if let Ok(v) = std::env::var("VM_JIT_MIN_THRESHOLD") { if let Ok(n) = v.parse::<u64>() { adaptive_cfg.min_threshold = n; } }
        if let Ok(v) = std::env::var("VM_JIT_MAX_THRESHOLD") { if let Ok(n) = v.parse::<u64>() { adaptive_cfg.max_threshold = n; } }
        if let Ok(v) = std::env::var("VM_JIT_SAMPLE_WINDOW") { if let Ok(n) = v.parse::<usize>() { adaptive_cfg.sample_window = n; } }
        if let Ok(v) = std::env::var("VM_JIT_COMPILE_TIME_WEIGHT") { if let Ok(n) = v.parse::<f64>() { adaptive_cfg.compile_time_weight = n; } }
        if let Ok(v) = std::env::var("VM_JIT_EXEC_BENEFIT_WEIGHT") { if let Ok(n) = v.parse::<f64>() { adaptive_cfg.exec_benefit_weight = n; } }
        if let Ok(v) = std::env::var("VM_JIT_SHARE_POOL") { share_pool = !(v=="0" || v.eq_ignore_ascii_case("false")); }

        // Create MMU
        let mmu = SoftMmu::new(config.memory_size, false);
        let vm: VirtualMachine<IRBlock> = VirtualMachine::with_mmu(config.clone(), Box::new(mmu));
        let mmu_arc = vm.mmu();

        // Initialize CLINT (Clock Interrupt)
        let clint = Arc::new(Mutex::new(Clint::new(config.vcpu_count as usize, 10_000_000))); // 10MHz
        let clint_mmio = ClintMmio::new(Arc::clone(&clint));

        // Initialize PLIC (Platform Level Interrupt Controller)
        let plic = Arc::new(Mutex::new(Plic::new(127, config.vcpu_count as usize * 2)));
        let plic_mmio = PlicMmio::new(Arc::clone(&plic));
        plic_mmio.set_virtio_queue_source_base(32);

        // Initialize VirtIO AI
        let virtio_ai = VirtioAiMmio::new(VirtioAi::new());

        // Map Devices to MMIO
        {
            let mut mmu = mmu_arc.lock().map_err(|_| ServiceError::MmuLock)?;
            
            // CLINT @ 0x0200_0000 (16KB)
            mmu.map_mmio(0x0200_0000, 0x10000, Box::new(clint_mmio));
            
            // PLIC @ 0x0C00_0000 (64MB)
            mmu.map_mmio(0x0C00_0000, 0x4000000, Box::new(plic_mmio));

            // VirtIO Block @ 0x1000_0000 (4KB)
            if let Some(disk_path) = &config.virtio.block_image {
                info!("Initializing VirtIO Block device with image: {}", disk_path);
                match VirtioBlock::open(disk_path, false).await {
                    Ok(block_dev) => {
                        let block_mmio = VirtioBlockMmio::new(block_dev);
                        mmu.map_mmio(0x1000_0000, 0x1000, Box::new(block_mmio));
                        {
                            let pm = PlicMmio::new(Arc::clone(&plic));
                            pm.register_source_range("virtio-block", 32, 16);
                            pm.register_source_range("virtio-ai", 48, 16);
                        }
                        mmu.map_mmio(0x1000_1000, 0x1000, Box::new(virtio_ai));
                        info!("VirtIO Block device initialized at 0x1000_0000");
                    }
                    Err(e) => {
                        error!("Failed to open disk image: {}", e);
                        return Err(ServiceError::Device(e.to_string()));
                    }
                }
            }
        }

        // 异步设备轮询任务（Tokio）
        {
            let mmu_poll = mmu_arc.clone();
            tokio::spawn(async move {
                use tokio::time::{sleep, Duration};
                loop {
                    sleep(Duration::from_millis(10)).await;
                    if let Ok(mut mmu) = mmu_poll.lock() {
                        mmu.poll_devices();
                    }
                }
            });
        }

        // Initialize Decoder and Interpreter
        // Currently hardcoded for RISC-V 64
        // Decoder is now integrated within each execution engine
        let mut interpreter = Interpreter::new();
        interpreter.set_reg(0, 0); // x0 = 0

        Ok(Self {
            vm,
            interpreter,
            run_flag: Arc::new(AtomicBool::new(false)),
            pause_flag: Arc::new(AtomicBool::new(false)),
            trap_handler: None,
            irq_policy: None,
            code_pool: if share_pool { Some(Arc::new(Mutex::new(HashMap::new()))) } else { None },
            adaptive_snapshot: None,
            adaptive_config: Some(adaptive_cfg),
        })
    }

    pub fn load_kernel(&mut self, path: &str, addr: u64) -> Result<(), ServiceError> {
        info!("Loading kernel from {} to {:#x}", path, addr);
        self.vm.load_kernel_file(path, addr).map_err(|e| ServiceError::Device(e.to_string()))
    }

    pub fn load_test_program(&mut self, code_base: u64) -> Result<(), ServiceError> {
        use vm_frontend_riscv64::api::*;
        
        let data_base: u64 = 0x100;
        
        let code = vec![
            encode_addi(1, 0, 10),          // li x1, 10
            encode_addi(2, 0, 20),          // li x2, 20
            encode_add(3, 1, 2),            // add x3, x1, x2
            encode_addi(10, 0, data_base as i32), // li x10, 0x100
            encode_sw(10, 3, 0),            // sw x3, 0(x10)
            encode_lw(4, 10, 0),            // lw x4, 0(x10)
            encode_beq(3, 4, 8),            // beq x3, x4, +8
            encode_addi(5, 0, 1),           // li x5, 1 (skipped)
            encode_addi(6, 0, 2),           // li x6, 2
            encode_jal(0, 0),               // j . (halt)
        ];

        let mmu_arc = self.vm.mmu();
        let mut mmu = mmu_arc.lock().map_err(|_| ServiceError::MmuLock)?;
        for (i, insn) in code.iter().enumerate() {
            mmu.write(code_base + (i as u64 * 4), *insn as u64, 4).map_err(|f| ServiceError::Device(format!("MMU write fault: {:?}", f)))?;
        }
        
        info!("Test program loaded at {:#x}", code_base);
        Ok(())
    }

    pub fn run(&mut self, start_pc: u64) -> Result<(), ServiceError> {
        self.vm.start().map_err(|e| ServiceError::Device(e.to_string()))?;
        self.run_flag.store(true, Ordering::Relaxed);
        self.pause_flag.store(false, Ordering::Relaxed);
        
        let max_steps = 1_000_000;
        let mut pc = start_pc;
        
        info!("Starting execution from PC={:#x}", pc);

        let mmu_arc = self.vm.mmu();
        let debug = self.vm.config().debug_trace;
        let vcpu_count = self.vm.config().vcpu_count as usize;

        // 基准 MMU 克隆，避免重复锁
        let base_mmu: SoftMmu = {
            let mmu_guard = mmu_arc.lock().map_err(|_| ServiceError::MmuLock)?;
            let any_ref = mmu_guard.as_any();
            let smmu = any_ref.downcast_ref::<SoftMmu>().ok_or_else(|| ServiceError::MmuType)?;
            smmu.clone()
        };

        if vcpu_count <= 1 {
            let mut local_mmu = base_mmu.clone();
            let mut local_interpreter = Interpreter::new();
            local_interpreter.set_reg(0, 0);
            let mut local_decoder = RiscvDecoder;

            for step in 0..max_steps {
                if !self.run_flag.load(Ordering::Relaxed) { break; }
                if self.pause_flag.load(Ordering::Relaxed) { break; }
                match local_decoder.decode(&local_mmu, pc) {
                    Ok(block) => {
                        let _res = local_interpreter.run(&mut local_mmu, &block);
                        if matches!(_res.status, vm_core::ExecStatus::InterruptPending) {
                            if let Some(p) = &self.irq_policy { let _ = p(&mut local_interpreter); }
                            pc = _res.next_pc;
                            continue;
                        }
                        if let ExecStatus::Fault(ref f) = _res.status {
                            if let Some(h) = &self.trap_handler {
                                match h(f, &mut local_interpreter) {
                                    ExecInterruptAction::Continue | ExecInterruptAction::InjectState => {
                                        local_interpreter.resume_from_trap();
                                        pc = local_interpreter.get_pc();
                                        continue;
                                    }
                                    ExecInterruptAction::Retry => { continue; }
                                    ExecInterruptAction::Mask => { break; }
                                    ExecInterruptAction::Deliver | ExecInterruptAction::Abort => { break; }
                                }
                            }
                            else {
                                // 默认 Trap 向量策略：按照 mtvec/stvec 设置 PC
                                let mode = local_interpreter.get_priv_mode();
                                let (vec, cause) = if mode == 3 { (local_interpreter.read_csr(0x305), local_interpreter.read_csr(0x342)) } else { (local_interpreter.read_csr(0x105), local_interpreter.read_csr(0x142)) };
                                let base = vec & !0x3;
                                let is_interrupt = (cause >> 63) != 0;
                                let mtvec_mode = vec & 0x3;
                                let target = if is_interrupt && mtvec_mode == 1 { base.wrapping_add(4 * (cause & 0xfff)) } else { base };
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
                            Terminator::CondJmp { cond, target_true, target_false } => {
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
            let mut handles = Vec::with_capacity(vcpu_count);
            for _i in 0..vcpu_count {
                let mut local_mmu = base_mmu.clone();
                let debug_local = debug;
                let mut thread_pc = pc;
                let run_flag = Arc::clone(&self.run_flag);
                let pause_flag = Arc::clone(&self.pause_flag);
                handles.push(thread::spawn(move || {
                    let mut interp = Interpreter::new();
                    interp.set_reg(0, 0);
                    let mut decoder = RiscvDecoder;
                    for step in 0..max_steps {
                        if !run_flag.load(Ordering::Relaxed) { break; }
                        if pause_flag.load(Ordering::Relaxed) { break; }
                        match decoder.decode(&local_mmu, thread_pc) {
                            Ok(block) => {
                                let _res = interp.run(&mut local_mmu, &block);
                                if matches!(_res.status, vm_core::ExecStatus::InterruptPending) {
                                    thread_pc = _res.next_pc;
                                    continue;
                                }
                                if let ExecStatus::Fault(ref _f) = _res.status { }
                                if debug_local && step % 1000 == 0 {
                                    debug!("[CPU {} Step {}] PC={:#x}", _i, step, thread_pc);
                                }
                                match &block.term {
                                    Terminator::Jmp { target } => {
                                        if *target == thread_pc { break; }
                                        thread_pc = *target;
                                    }
                                    Terminator::CondJmp { cond, target_true, target_false } => {
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
                                    Terminator::Ret | Terminator::Fault { .. } => { break; }
                                    _ => thread_pc += 4,
                                }
                            }
                            Err(_) => { break; }
                        }
                    }
                }));
            }
            for h in handles { let _ = h.join(); }
        }

        info!("=== Execution Complete ===");
        tinfo!(pc=?pc, steps=?max_steps, "service:run_complete");
        Ok(())
    }

    pub fn configure_tlb_from_env(&mut self) {
        if let Ok(itlb_str) = std::env::var("VM_ITLB") {
            if let Ok(itlb) = itlb_str.parse::<usize>() {
                let dtlb = std::env::var("VM_DTLB").ok().and_then(|s| s.parse().ok()).unwrap_or(128usize);
                if let Ok(mut mmu) = self.vm.mmu().lock() {
                    if let Some(smmu) = mmu.as_any_mut().downcast_mut::<vm_mem::SoftMmu>() {
                        smmu.resize_tlbs(itlb, dtlb);
                        let (ci, cd) = smmu.tlb_capacity();
                        log::info!("TLB resized: itlb={}, dtlb={}", ci, cd);
                    }
                }
            }
        }
    }

    pub fn set_trap_handler(&mut self, h: Arc<dyn Fn(&Fault, &mut Interpreter) -> ExecInterruptAction + Send + Sync>) { self.trap_handler = Some(h); }
    pub fn set_irq_policy(&mut self, p: Arc<dyn Fn(&mut Interpreter) -> ExecInterruptAction + Send + Sync>) { self.irq_policy = Some(p); }

    pub fn request_stop(&self) { self.run_flag.store(false, Ordering::Relaxed); }
    pub fn request_pause(&self) { self.pause_flag.store(true, Ordering::Relaxed); }
    pub fn request_resume(&self) { self.pause_flag.store(false, Ordering::Relaxed); }

    pub fn get_reg(&self, idx: usize) -> u64 {
        self.interpreter.get_reg(idx as u32)
    }

    pub fn hot_stats(&self) -> Option<vm_engine_jit::AdaptiveThresholdStats> {
        self.adaptive_snapshot.clone()
    }

    pub fn set_hot_config(&mut self, cfg: vm_engine_jit::AdaptiveThresholdConfig) {
        self.adaptive_config = Some(cfg);
    }

    pub fn set_hot_config_vals(&mut self, min: u64, max: u64, window: Option<usize>, compile_w: Option<f64>, benefit_w: Option<f64>) {
        let mut cfg = vm_engine_jit::AdaptiveThresholdConfig::default();
        cfg.min_threshold = min;
        cfg.max_threshold = max;
        if let Some(w) = window { cfg.sample_window = w; }
        if let Some(w) = compile_w { cfg.compile_time_weight = w; }
        if let Some(w) = benefit_w { cfg.exec_benefit_weight = w; }
        self.adaptive_config = Some(cfg);
    }

    pub fn set_shared_pool(&mut self, enable: bool) {
        if enable {
            if self.code_pool.is_none() {
                self.code_pool = Some(Arc::new(Mutex::new(HashMap::new())));
            }
        } else {
            self.code_pool = None;
        }
    }

    pub fn hot_snapshot(&self) -> Option<(vm_engine_jit::AdaptiveThresholdConfig, vm_engine_jit::AdaptiveThresholdStats)> {
        match (self.adaptive_config.clone(), self.adaptive_snapshot.clone()) {
            (Some(cfg), Some(stats)) => Some((cfg, stats)),
            _ => None,
        }
    }

    pub fn export_hot_snapshot_json(&self) -> Option<String> {
        self.hot_snapshot().map(|(cfg, stats)| {
            format!(
                "{{\"min_threshold\":{},\"max_threshold\":{},\"sample_window\":{},\"compile_time_weight\":{},\"exec_benefit_weight\":{},\"compiled_hits\":{},\"interpreted_runs\":{},\"total_compiles\":{} }}",
                cfg.min_threshold,
                cfg.max_threshold,
                cfg.sample_window,
                cfg.compile_time_weight,
                cfg.exec_benefit_weight,
                stats.compiled_hits,
                stats.interpreted_runs,
                stats.total_compiles,
            )
        })
    }

    pub async fn run_async(&mut self, start_pc: u64) -> Result<(), ServiceError> {
        self.vm.start().map_err(|e| ServiceError::Device(e.to_string()))?;
        self.run_flag.store(true, Ordering::Relaxed);
        self.pause_flag.store(false, Ordering::Relaxed);

        let max_steps = 1_000_000;
        let mut pc = start_pc;
        info!("Starting async execution from PC={:#x}", pc);
        tinfo!(pc=?pc, "service:run_async_start");
        let mmu_arc = self.vm.mmu();
        let debug = self.vm.config().debug_trace;
        let vcpu_count = self.vm.config().vcpu_count as usize;

        if vcpu_count <= 1 {
            let mut local_mmu = {
                let mmu_guard = mmu_arc.lock().map_err(|_| ServiceError::MmuLock)?;
                let any_ref = mmu_guard.as_any();
                let smmu = any_ref.downcast_ref::<SoftMmu>().ok_or_else(|| ServiceError::MmuType)?;
                smmu.clone()
            };
            let hybrid = matches!(self.vm.config().exec_mode, vm_core::ExecMode::Hybrid | vm_core::ExecMode::Jit);
            let mut interp = Interpreter::new();
            interp.set_reg(0, 0);
            let mut jit = if hybrid {
                let pool = self.code_pool.as_ref().cloned();
                Some(match (pool, self.adaptive_config.clone()) {
                    (Some(p), Some(cfg)) => Jit::with_adaptive_config(cfg).with_pool_cache(p),
                    (Some(p), None) => Jit::new().with_pool_cache(p),
                    (None, Some(cfg)) => Jit::with_adaptive_config(cfg),
                    (None, None) => Jit::new(),
                })
            } else { None };
            let mut decoder = RiscvDecoder;
            for step in 0..max_steps {
                tokio::select! {
                    _ = async { if !self.run_flag.load(Ordering::Relaxed) { Err(()) } else { Ok(()) } } => {},
                    _ = async { if self.pause_flag.load(Ordering::Relaxed) { Err(()) } else { Ok(()) } } => {},
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
                        if debug && step % 1000 == 0 { debug!("[Async Step {}] PC={:#x}", step, pc); tdebug!(step=?step, pc=?pc, "service:run_async_tick"); }
                        if hybrid && step % 1000 == 0 {
                            if let Some(j) = jit.as_ref() { self.adaptive_snapshot = Some(j.adaptive_stats()); }
                        }
                        pc = res.next_pc;
                    }
                    Err(_) => { break; }
                }
                tokio::task::yield_now().await;
            }
        } else {
            let mut handles = Vec::with_capacity(vcpu_count);
            let pool_main = self.code_pool.as_ref().cloned();
            let run_flag_main = Arc::clone(&self.run_flag);
            let pause_flag_main = Arc::clone(&self.pause_flag);
            let hybrid_main = matches!(self.vm.config().exec_mode, vm_core::ExecMode::Hybrid | vm_core::ExecMode::Jit);
            for i in 0..vcpu_count {
                let mmu_base = {
                let mmu_guard = mmu_arc.lock().map_err(|_| ServiceError::MmuLock)?;
                let any_ref = mmu_guard.as_any();
                let smmu = any_ref.downcast_ref::<SoftMmu>().ok_or_else(|| ServiceError::MmuType)?;
                    smmu.clone()
                };
                let debug_local = debug;
                let mut thread_pc = pc;
                let run_flag = Arc::clone(&run_flag_main);
                let pause_flag = Arc::clone(&pause_flag_main);
                let pool_iter = pool_main.as_ref().map(|p| p.clone());
                let cfg_iter = self.adaptive_config.clone();
                handles.push(tokio::spawn(async move {
                    let mut interp = Interpreter::new();
                    interp.set_reg(0, 0);
                    let hybrid = hybrid_main;
                    let mut jit = if hybrid {
                        Some(match (pool_iter, cfg_iter) {
                            (Some(p), Some(cfg)) => Jit::with_adaptive_config(cfg).with_pool_cache(p),
                            (Some(p), None) => Jit::new().with_pool_cache(p),
                            (None, Some(cfg)) => Jit::with_adaptive_config(cfg),
                            (None, None) => Jit::new(),
                        })
                    } else { None };
                let mut decoder = RiscvDecoder;
                let mut local_mmu = mmu_base;
                for step in 0..max_steps {
                        if !run_flag.load(Ordering::Relaxed) { break; }
                        if pause_flag.load(Ordering::Relaxed) { break; }
                        match decoder.decode(&local_mmu, thread_pc) {
                            Ok(block) => {
                                let res = if let Some(j) = jit.as_mut() {
                                    j.set_pc(thread_pc);
                                    j.run(&mut local_mmu, &block)
                                } else {
                                    interp.run(&mut local_mmu, &block)
                                };
                                if debug_local && step % 1000 == 0 { debug!("[CPU {} Async Step {}] PC={:#x}", i, step, thread_pc); tdebug!(cpu=?i, step=?step, pc=?thread_pc, "service:run_async_tick_cpu"); }
                                thread_pc = res.next_pc;
                            }
                            Err(_) => { break; }
                        }
                        tokio::task::yield_now().await;
                    }
                }));
            }
            for h in handles { let _ = h.await; }
        }
        info!("=== Async Execution Complete ===");
        tinfo!(pc=?pc, "service:run_async_complete");
        Ok(())
    }

    pub fn create_snapshot(&mut self, name: String, description: String) -> Result<String, ServiceError> {
        self.vm.create_snapshot(name, description).map_err(|e| ServiceError::Device(e.to_string()))
    }

    pub fn restore_snapshot(&mut self, id: &str) -> Result<(), ServiceError> {
        self.vm.restore_snapshot(id).map_err(|e| ServiceError::Device(e.to_string()))
    }

    pub fn list_snapshots(&self) -> Result<Vec<vm_core::snapshot::Snapshot>, ServiceError> {
        self.vm.list_snapshots().map_err(|e| ServiceError::Device(e.to_string()))
    }

    pub fn create_template(&mut self, name: String, description: String, base_snapshot_id: String) -> Result<String, ServiceError> {
        self.vm.create_template(name, description, base_snapshot_id).map_err(|e| ServiceError::Device(e.to_string()))
    }

    pub fn list_templates(&self) -> Result<Vec<vm_core::template::VmTemplate>, ServiceError> {
        self.vm.list_templates().map_err(|e| ServiceError::Device(e.to_string()))
    }

    pub fn serialize_state(&self) -> Result<Vec<u8>, ServiceError> {
        self.vm.serialize_state().map_err(|e| ServiceError::Device(e.to_string()))
    }

    pub fn deserialize_state(&mut self, data: &[u8]) -> Result<(), ServiceError> {
        self.vm.deserialize_state(data).map_err(|e| ServiceError::Device(e.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("MMU lock failure")] MmuLock,
    #[error("MMU type mismatch")] MmuType,
    #[error("GPU init error: {0}")] GpuInit(String),
    #[error("Device error: {0}")] Device(String),
}
