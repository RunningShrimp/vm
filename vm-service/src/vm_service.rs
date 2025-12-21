//! 虚拟机服务层
//!
//! 实现VirtualMachineService，处理虚拟机的业务逻辑。
//! 符合DDD贫血模型原则，将业务逻辑从实体类移至服务层。

pub mod decoder_factory;
mod execution;
mod kernel_loader;
mod lifecycle;
mod snapshot_manager;

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use vm_core::vm_state::VirtualMachineState;
use vm_core::{GuestAddr, MemoryError, VmConfig, VmError, VmLifecycleState, VmResult}; // 恢复VmError导入，因为它在代码中被使用
use vm_engine_interpreter::{ExecInterruptAction, Interpreter};
use vm_engine_jit::{AdaptiveThresholdConfig, AdaptiveThresholdStats, CodePtr};
use vm_mem::SoftMmu;

use self::execution::{ExecutionContext, run_async, run_sync};
use self::lifecycle::{request_pause, request_resume, request_stop};
use self::snapshot_manager::{
    create_template, deserialize_state, list_snapshots, list_templates, serialize_state,
};
/// 虚拟机服务
///
/// 负责处理虚拟机的业务逻辑，包括：
/// - 内核加载
/// - 快照管理
/// - 状态序列化/反序列化
/// - VM生命周期管理
/// - 执行循环管理
#[cfg(not(feature = "no_std"))]
pub struct VirtualMachineService<B> {
    /// 虚拟机状态
    state: Arc<Mutex<VirtualMachineState<B>>>,
    /// 运行标志
    run_flag: Arc<AtomicBool>,
    /// 暂停标志
    pause_flag: Arc<AtomicBool>,
    /// 陷阱处理器
    trap_handler:
        Option<Arc<dyn Fn(&VmError, &mut Interpreter) -> ExecInterruptAction + Send + Sync>>,
    /// 中断策略
    irq_policy: Option<Arc<dyn Fn(&mut Interpreter) -> ExecInterruptAction + Send + Sync>>,
    /// JIT代码池
    code_pool: Option<Arc<Mutex<HashMap<GuestAddr, CodePtr>>>>,
    /// 自适应快照（使用 Arc<Mutex> 支持内部可变性）
    adaptive_snapshot: Arc<Mutex<Option<AdaptiveThresholdStats>>>,
    /// 自适应配置
    adaptive_config: Option<AdaptiveThresholdConfig>,
}

#[cfg(not(feature = "no_std"))]
impl<B: 'static> VirtualMachineService<B> {
    /// 创建新的虚拟机服务
    pub fn new(state: VirtualMachineState<B>) -> Self {
        Self {
            state: Arc::new(Mutex::new(state)),
            run_flag: Arc::new(AtomicBool::new(false)),
            pause_flag: Arc::new(AtomicBool::new(false)),
            trap_handler: None,
            irq_policy: None,
            code_pool: None,
            adaptive_snapshot: Arc::new(Mutex::new(None)),
            adaptive_config: None,
        }
    }

    /// 从配置创建虚拟机服务
    pub fn from_config(config: VmConfig, mmu: Box<dyn vm_core::MMU>) -> Self {
        let state = VirtualMachineState::new(config, mmu);
        Self::new(state)
    }

    /// 加载内核镜像到内存（领域服务方法）
    ///
    /// 封装内核加载的业务逻辑，包括验证、地址检查等
    pub fn load_kernel(&self, data: &[u8], load_addr: GuestAddr) -> VmResult<()> {
        // 业务逻辑：验证内核数据
        if data.is_empty() {
            return Err(VmError::Core(vm_core::CoreError::Config {
                message: "Kernel data cannot be empty".to_string(),
                path: Some("kernel_data".to_string()),
            }));
        }

        // 业务逻辑：验证加载地址
        if load_addr == vm_core::GuestAddr(0) {
            return Err(VmError::Core(vm_core::CoreError::Config {
                message: "Load address cannot be zero".to_string(),
                path: Some("load_addr".to_string()),
            }));
        }

        // 业务逻辑：检查虚拟机状态是否允许加载内核
        let state = self.state.lock().map_err(|_| {
            VmError::Memory(MemoryError::MmuLockFailed {
                message: "Failed to acquire state lock".to_string(),
            })
        })?;

        if state.state() == VmLifecycleState::Running {
            return Err(VmError::Core(vm_core::CoreError::InvalidState {
                message: "Cannot load kernel while VM is running".to_string(),
                current: "running".to_string(),
                expected: "stopped".to_string(),
            }));
        }

        let mmu = state.mmu();
        drop(state);

        // 调用基础设施层进行实际加载
        kernel_loader::load_kernel(mmu, data, load_addr)
    }

    /// 从文件加载内核
    #[cfg(not(feature = "no_std"))]
    pub fn load_kernel_file(&self, path: &str, load_addr: GuestAddr) -> VmResult<()> {
        kernel_loader::load_kernel_file(path, load_addr)
    }

    /// 启动 VM
    pub fn start(&self) -> VmResult<()> {
        lifecycle::start(Arc::clone(&self.state))
    }

    /// 暂停 VM
    pub fn pause(&self) -> VmResult<()> {
        lifecycle::pause(Arc::clone(&self.state))
    }

    /// 停止 VM
    pub fn stop(&self) -> VmResult<()> {
        lifecycle::stop(Arc::clone(&self.state))
    }

    /// 重置 VM
    pub fn reset(&self) -> VmResult<()> {
        lifecycle::reset(Arc::clone(&self.state))
    }

    /// 创建快照（调用模块函数）
    pub fn create_snapshot(&self, name: String, description: String) -> VmResult<String> {
        snapshot_manager::create_snapshot(Arc::clone(&self.state), name, description)
    }

    /// 异步创建快照
    #[cfg(feature = "async")]
    pub async fn create_snapshot_async(
        &self,
        name: String,
        description: String,
    ) -> VmResult<String> {
        use crate::snapshot_manager::create_snapshot_async;
        // 注意：需要将同步Mutex转换为异步Mutex
        // 为了简化，这里使用spawn_blocking包装
        let state_clone = Arc::clone(&self.state);
        tokio::task::spawn_blocking(move || create_snapshot(state_clone, name, description))
            .await
            .map_err(|e| VmError::Io(format!("Failed to create snapshot: {}", e)))?
    }

    /// 恢复快照（调用模块函数）
    pub fn restore_snapshot(&self, id: &str) -> VmResult<()> {
        snapshot_manager::restore_snapshot(Arc::clone(&self.state), id)
    }

    /// 异步恢复快照
    #[cfg(feature = "async")]
    pub async fn restore_snapshot_async(&self, id: &str) -> VmResult<()> {
        use crate::snapshot_manager::restore_snapshot_async;
        // 注意：需要将同步Mutex转换为异步Mutex
        // 为了简化，这里使用spawn_blocking包装
        let state_clone = Arc::clone(&self.state);
        let id_str = id.to_string();
        tokio::task::spawn_blocking(move || restore_snapshot(state_clone, &id_str))
            .await
            .map_err(|e| VmError::Io(format!("Failed to restore snapshot: {}", e)))?
    }

    /// 获取状态引用（用于只读访问）
    pub fn state(&self) -> Arc<Mutex<VirtualMachineState<B>>> {
        Arc::clone(&self.state)
    }

    /// 加载测试程序到内存（领域服务方法）
    ///
    /// 加载一个简单的RISC-V测试程序，用于验证VM功能
    pub fn load_test_program(&self, code_base: GuestAddr) -> VmResult<()> {
        // 业务逻辑：验证地址
        if code_base == vm_core::GuestAddr(0) {
            return Err(VmError::Core(vm_core::CoreError::Config {
                message: "Code base address cannot be zero".to_string(),
                path: Some("code_base".to_string()),
            }));
        }

        // 业务逻辑：生成测试程序代码
        let test_program = self.generate_test_program()?;
        let data_base = 0x100; // 数据段基地址

        // 调用基础设施层加载程序
        self.load_program_infrastructure(code_base, &test_program, data_base)
    }

    /// 生成测试程序代码（业务逻辑）
    fn generate_test_program(&self) -> VmResult<Vec<u32>> {
        use vm_frontend_riscv64::api::*;

        let data_base: u64 = 0x100;

        // 业务逻辑：定义测试程序的功能
        // 这个程序执行：10 + 20 = 30，然后存储到内存，读取回来比较
        let code = vec![
            encode_addi(1, 0, 10),                // li x1, 10
            encode_addi(2, 0, 20),                // li x2, 20
            encode_add(3, 1, 2),                  // add x3, x1, x2  (x3 = 30)
            encode_addi(10, 0, data_base as i32), // li x10, 0x100
            encode_sw(10, 3, 0),                  // sw x3, 0(x10)  (store 30)
            encode_lw(4, 10, 0),                  // lw x4, 0(x10)  (load 30)
            encode_beq(3, 4, 8),                  // beq x3, x4, +8 (if equal, skip)
            encode_addi(5, 0, 1),                 // li x5, 1 (error flag)
            encode_addi(6, 0, 2),                 // li x6, 2
            encode_jal(0, 0),                     // j . (halt)
        ];

        Ok(code)
    }

    /// 基础设施层：实际的程序加载实现
    fn load_program_infrastructure(
        &self,
        code_base: GuestAddr,
        program: &[u32],
        data_base: u64,
    ) -> VmResult<()> {
        let state = self.state.lock().map_err(|_| {
            VmError::Memory(MemoryError::MmuLockFailed {
                message: "Failed to acquire state lock".to_string(),
            })
        })?;

        let mmu = state.mmu();
        let mut mmu_guard = mmu.lock().map_err(|_| {
            VmError::Memory(MemoryError::MmuLockFailed {
                message: "Failed to acquire MMU lock".to_string(),
            })
        })?;

        // 基础设施：写入程序代码
        for (i, &insn) in program.iter().enumerate() {
            mmu_guard.write(code_base + (i as u64 * 4), insn as u64, 4)?;
        }

        // 基础设施：初始化数据段（如果需要）
        if data_base != 0 {
            // 初始化数据内存为0
            mmu_guard.write(vm_core::GuestAddr(data_base), 0, 8)?
        }

        Ok(())
    }

    /// 从环境变量配置TLB大小
    pub fn configure_tlb_from_env(&self) -> VmResult<()> {
        if let Ok(itlb_str) = std::env::var("VM_ITLB")
            && let Ok(itlb) = itlb_str.parse::<usize>()
        {
            let dtlb = std::env::var("VM_DTLB")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(128usize);

            let state = self.state.lock().map_err(|_| {
                VmError::Memory(MemoryError::MmuLockFailed {
                    message: "Failed to acquire state lock".to_string(),
                })
            })?;

            let mmu = state.mmu();
            let mut mmu_guard = mmu.lock().map_err(|_| {
                VmError::Memory(MemoryError::MmuLockFailed {
                    message: "Failed to acquire MMU lock".to_string(),
                })
            })?;

            if let Some(smmu) = mmu_guard.as_any_mut().downcast_mut::<SoftMmu>() {
                smmu.resize_tlbs(itlb, dtlb);
                let (ci, cd) = smmu.tlb_capacity();
                log::info!("TLB resized: itlb={}, dtlb={}", ci, cd);
            }
        }
        Ok(())
    }

    /// 设置陷阱处理器
    pub fn set_trap_handler(
        &mut self,
        h: Arc<dyn Fn(&VmError, &mut Interpreter) -> ExecInterruptAction + Send + Sync>,
    ) {
        self.trap_handler = Some(h);
    }

    /// 设置中断策略
    pub fn set_irq_policy(
        &mut self,
        p: Arc<dyn Fn(&mut Interpreter) -> ExecInterruptAction + Send + Sync>,
    ) {
        self.irq_policy = Some(p);
    }

    /// 请求停止执行
    pub fn request_stop(&self) {
        request_stop(&self.run_flag);
    }

    /// 请求暂停执行
    pub fn request_pause(&self) {
        request_pause(&self.pause_flag);
    }

    /// 请求恢复执行
    pub fn request_resume(&self) {
        request_resume(&self.pause_flag);
    }

    /// 列出所有快照
    pub fn list_snapshots(&self) -> VmResult<Vec<vm_core::snapshot::Snapshot>> {
        snapshot_manager::list_snapshots(Arc::clone(&self.state))
    }

    /// 列出所有模板
    pub fn list_templates(&self) -> VmResult<Vec<vm_core::template::VmTemplate>> {
        snapshot_manager::list_templates(Arc::clone(&self.state))
    }

    /// 创建模板
    pub fn create_template(
        &self,
        name: String,
        description: String,
        base_snapshot_id: String,
    ) -> VmResult<String> {
        snapshot_manager::create_template(
            Arc::clone(&self.state),
            name,
            description,
            base_snapshot_id,
        )
    }

    /// 序列化虚拟机状态以进行迁移
    pub fn serialize_state(&self) -> VmResult<Vec<u8>> {
        snapshot_manager::serialize_state(Arc::clone(&self.state))
    }

    /// 从序列化数据中反序列化并恢复虚拟机状态
    pub fn deserialize_state(&self, data: &[u8]) -> VmResult<()> {
        snapshot_manager::deserialize_state(Arc::clone(&self.state), data)
    }

    /// 获取寄存器值
    pub fn get_reg(&self, idx: usize) -> VmResult<u64> {
        let state = self.state.lock().map_err(|_| {
            VmError::Core(vm_core::CoreError::Internal {
                message: "Failed to lock state".to_string(),
                module: "VirtualMachineService".to_string(),
            })
        })?;

        if let Some(vcpu) = state.vcpus.first() {
            let vcpu_guard = vcpu.lock().map_err(|_| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: "Failed to lock vCPU".to_string(),
                    module: "VirtualMachineService".to_string(),
                })
            })?;
            Ok(vcpu_guard.get_reg(idx))
        } else {
            Err(VmError::Core(vm_core::CoreError::Config {
                message: "No vCPU available".to_string(),
                path: None,
            }))
        }
    }

    /// 获取JIT热点统计
    pub fn hot_stats(&self) -> Option<AdaptiveThresholdStats> {
        self.adaptive_snapshot
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
    }

    /// 设置JIT热点配置
    pub fn set_hot_config(&mut self, cfg: AdaptiveThresholdConfig) {
        self.adaptive_config = Some(cfg);
    }

    /// 设置JIT热点配置值
    pub fn set_hot_config_vals(
        &mut self,
        min: u64,
        max: u64,
        _window: Option<usize>,
        _compile_w: Option<f64>,
        _benefit_w: Option<f64>,
    ) {
        let mut cfg = AdaptiveThresholdConfig::default();
        cfg.cold_threshold = min;
        cfg.hot_threshold = max;
        // Note: window, compile_w, benefit_w are ignored in current AdaptiveThresholdConfig
        self.adaptive_config = Some(cfg);
    }

    /// 设置共享代码池
    pub fn set_shared_pool(&mut self, enable: bool) {
        if enable {
            if self.code_pool.is_none() {
                self.code_pool = Some(Arc::new(Mutex::new(HashMap::new())));
            }
        } else {
            self.code_pool = None;
        }
    }

    /// 获取JIT热点快照
    pub fn hot_snapshot(&self) -> Option<(AdaptiveThresholdConfig, AdaptiveThresholdStats)> {
        let snapshot = self
            .adaptive_snapshot
            .lock()
            .ok()
            .and_then(|guard| guard.clone());
        match (self.adaptive_config.clone(), snapshot) {
            (Some(cfg), Some(stats)) => Some((cfg, stats)),
            _ => None,
        }
    }

    /// 导出JIT热点快照为JSON
    pub fn export_hot_snapshot_json(&self) -> Option<String> {
        self.hot_snapshot().map(|(cfg, stats)| {
            format!(
                "{{\"cold_threshold\":{},\"hot_threshold\":{},\"enable_adaptive\":{},\"execution_count\":{}}}",
                cfg.cold_threshold, cfg.hot_threshold, cfg.enable_adaptive, stats.execution_count
            )
        })
    }

    /// 同步执行循环
    pub fn run(&self, start_pc: GuestAddr) -> VmResult<()> {
        self.start()?;

        let state = self.state.lock().map_err(|_| {
            VmError::Core(vm_core::CoreError::Internal {
                message: "Failed to lock state".to_string(),
                module: "VirtualMachineService".to_string(),
            })
        })?;

        let mmu_arc = state.mmu();
        let debug = false; // VmConfig中没有debug_trace字段，使用默认值
        let vcpu_count = state.config().vcpu_count;
        let guest_arch = state.config().guest_arch;
        drop(state);

        // 基准 MMU 克隆，避免重复锁
        let base_mmu: SoftMmu = {
            let mmu_guard = mmu_arc.lock().map_err(|_| {
                VmError::Memory(MemoryError::MmuLockFailed {
                    message: "Failed to acquire MMU lock".to_string(),
                })
            })?;
            let any_ref = mmu_guard.as_any();
            any_ref
                .downcast_ref::<SoftMmu>()
                .ok_or_else(|| VmError::Memory(MemoryError::InvalidAddress(vm_core::GuestAddr(0))))?
                .clone()
        };

        #[cfg(feature = "async")]
        let coroutine_scheduler = None; // 同步执行不使用协程调度器

        let ctx = ExecutionContext {
            run_flag: Arc::clone(&self.run_flag),
            pause_flag: Arc::clone(&self.pause_flag),
            guest_arch,
            trap_handler: self.trap_handler.clone(),
            irq_policy: self.irq_policy.clone(),
            code_pool: self.code_pool.as_ref().cloned(),
            adaptive_snapshot: Arc::clone(&self.adaptive_snapshot),
            adaptive_config: self.adaptive_config.clone(),
            #[cfg(feature = "async")]
            coroutine_scheduler,
        };

        run_sync(&ctx, start_pc, base_mmu, debug, vcpu_count)
    }

    /// 异步执行循环
    pub async fn run_async(&self, start_pc: GuestAddr) -> VmResult<()> {
        self.start()?;

        let state = self.state.lock().map_err(|_| {
            VmError::Core(vm_core::CoreError::Internal {
                message: "Failed to lock state".to_string(),
                module: "VirtualMachineService".to_string(),
            })
        })?;

        let mmu_arc = state.mmu();
        let debug = false; // VmConfig中没有debug_trace字段，使用默认值
        let vcpu_count = state.config().vcpu_count;
        let exec_mode = state.config().exec_mode;
        let guest_arch = state.config().guest_arch;
        drop(state);

        // 基准 MMU 克隆，避免重复锁
        let base_mmu: SoftMmu = {
            let mmu_guard = mmu_arc.lock().map_err(|_| {
                VmError::Memory(MemoryError::MmuLockFailed {
                    message: "Failed to acquire MMU lock".to_string(),
                })
            })?;
            let any_ref = mmu_guard.as_any();
            any_ref
                .downcast_ref::<SoftMmu>()
                .ok_or_else(|| VmError::Memory(MemoryError::InvalidAddress(vm_core::GuestAddr(0))))?
                .clone()
        };

        // 创建或获取协程调度器（用于优化多vCPU执行）
        #[cfg(feature = "async")]
        let coroutine_scheduler = if vcpu_count > 1 {
            // 为多vCPU场景创建协程调度器
            Some(Arc::new(vm_runtime::CoroutineScheduler::new().map_err(
                |e| {
                    VmError::Core(vm_core::CoreError::Internal {
                        message: format!("Failed to create coroutine scheduler: {}", e),
                        module: "VirtualMachineService".to_string(),
                    })
                },
            )?))
        } else {
            None
        };

        let ctx = ExecutionContext {
            run_flag: Arc::clone(&self.run_flag),
            pause_flag: Arc::clone(&self.pause_flag),
            guest_arch,
            trap_handler: self.trap_handler.clone(),
            irq_policy: self.irq_policy.clone(),
            code_pool: self.code_pool.as_ref().cloned(),
            adaptive_snapshot: Arc::clone(&self.adaptive_snapshot),
            adaptive_config: self.adaptive_config.clone(),
            #[cfg(feature = "async")]
            coroutine_scheduler,
        };

        run_async(&ctx, start_pc, base_mmu, debug, vcpu_count, exec_mode).await
    }
}
