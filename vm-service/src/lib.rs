// Core modules (always present)
pub mod di_setup;
pub mod execution_orchestrator;
pub mod vm_service;

// Re-export service container for convenience
pub use di_setup::ServiceContainer;

// Conditional features module
#[cfg(feature = "devices")]
pub mod device_service;

// Feature-based re-exports (consolidated)
use std::sync::{Arc, Mutex};

#[cfg(feature = "devices")]
pub use device_service::DeviceService;
use execution_orchestrator::ExecutionOrchestrator;
use log::info;
use tracing::info as tinfo;
use vm_core::vm_state::VirtualMachineState;
use vm_core::{ExecMode, VmConfig, VmError};
use vm_engine::interpreter::Interpreter;
#[cfg(feature = "jit")]
use vm_engine::jit::{JITCompiler, JITConfig};
use vm_ir::IRBlock;
use vm_mem::SoftMmu;
pub use vm_service::{IrqPolicy, TrapHandler, VirtualMachineService as CoreVmService};
#[cfg(feature = "smmu")]
pub use vm_smmu::{SmmuConfig, SmmuStats};

/// VmService - 薄包装层
///
/// 保持向后兼容的API，将所有业务逻辑委托给 VirtualMachineService
pub struct VmService {
    /// 虚拟机状态实例（保留用于向后兼容）
    pub vm_state: Arc<Mutex<VirtualMachineState<IRBlock>>>,
    /// 虚拟机服务（包含所有业务逻辑）
    vm_service: CoreVmService<IRBlock>,
    /// 设备服务（处理所有设备相关业务逻辑）- 需要 devices feature
    #[cfg(feature = "devices")]
    _device_service: DeviceService,
    /// 解释器（保留用于向后兼容，将来通过 vm.vcpus 管理）
    interpreter: Interpreter,
    /// JIT编译器（当exec_mode为JIT时启用）
    #[cfg(feature = "jit")]
    jit_compiler: Option<Arc<Mutex<JITCompiler>>>,
    /// 执行模式
    exec_mode: ExecMode,
    /// 服务容器（管理所有领域服务的依赖注入）
    service_container: ServiceContainer,
}

// 实现Send和Sync特性，使VmService可以在多线程环境中安全共享
unsafe impl Send for VmService {}
unsafe impl Sync for VmService {}

impl VmService {
    pub async fn new(config: VmConfig, gpu_backend: Option<String>) -> Result<Self, VmError> {
        info!("Initializing VM Service with config: {:?}", config);
        tinfo!(guest_arch=?config.guest_arch, vcpus=?config.vcpu_count, mem=?config.memory_size, exec=?config.exec_mode, "service:new");

        // Create MMU - use Arc<SoftMmu> to share with device service
        let mmu = Arc::new(SoftMmu::new(config.memory_size, false));
        let vm_state: VirtualMachineState<IRBlock> =
            VirtualMachineState::new(config.clone(), Box::new((*mmu).clone()));

        // 创建执行编排器，选择最优执行路径
        let host_arch = ExecutionOrchestrator::detect_host_arch();
        let orchestrator =
            ExecutionOrchestrator::new(host_arch, config.guest_arch, config.exec_mode);
        let execution_path = orchestrator.select_execution_path();

        info!(
            "Execution orchestrator: host={:?}, guest={:?}, path={:?}",
            host_arch, config.guest_arch, execution_path
        );

        // Initialize Decoder and Interpreter
        // 多架构decoder factory支持：
        // - RISC-V 64位（完全实现）
        // - ARM64（完全实现）
        // - x86_64（完全实现）
        // - PowerPC64（回退到RISC-V）
        // Decoder基于VmConfig.guest_arch自动选择（不再硬编码RISC-V）
        let mut interpreter = Interpreter::new();
        interpreter.set_reg(0, 0); // x0 = 0

        // 创建 VirtualMachineState 和 VirtualMachineService
        // 直接使用已创建的 vm_state 来初始化 vm_service
        let vm_state_arc = Arc::new(Mutex::new(vm_state.clone()));
        let vm_service = CoreVmService::new(vm_state);

        // 初始化设备服务（仅当 devices feature 启用时）
        #[cfg(feature = "devices")]
        let device_service = Self::init_device_service(gpu_backend, &config, mmu).await?;

        // 初始化JIT编译器（当exec_mode为JIT时）
        #[cfg(feature = "jit")]
        let jit_compiler = if config.exec_mode == ExecMode::JIT {
            let jit_config = JITConfig::default();
            match JITCompiler::with_config(jit_config) {
                Ok(compiler) => {
                    info!("JIT compiler initialized successfully");
                    Some(Arc::new(Mutex::new(compiler)))
                }
                Err(e) => {
                    log::warn!(
                        "Failed to initialize JIT compiler: {:?}, falling back to interpreter",
                        e
                    );
                    None
                }
            }
        } else {
            None
        };

        // 创建服务容器（管理所有领域服务的依赖注入）
        let service_container = ServiceContainer::new();

        info!("Service container initialized with domain services");

        let result = Self {
            vm_state: vm_state_arc,
            vm_service,
            #[cfg(feature = "devices")]
            _device_service: device_service,
            interpreter,
            #[cfg(feature = "jit")]
            jit_compiler,
            exec_mode: config.exec_mode,
            service_container,
        };

        Ok(result)
    }

    #[cfg(feature = "devices")]
    async fn init_device_service(
        gpu_backend: Option<String>,
        config: &VmConfig,
        mmu_arc: Arc<SoftMmu>,
    ) -> Result<DeviceService, VmError> {
        let mut ds = DeviceService::new();
        ds.init_gpu(gpu_backend)?;
        // Convert Arc<SoftMmu> to Arc<Mutex<Box<dyn MMU>>>
        let mmu_mutex: Arc<std::sync::Mutex<Box<dyn vm_core::MMU>>> =
            Arc::new(std::sync::Mutex::new(Box::new((*mmu_arc).clone())));
        ds.initialize_devices(config, mmu_mutex).await?;
        ds.map_devices().await?;
        ds.start_polling()?;
        Ok(ds)
    }

    pub fn load_kernel(&mut self, path: &str, addr: u64) -> Result<(), VmError> {
        info!("Loading kernel from {} to {:#x}", path, addr);
        self.vm_service
            .load_kernel_file(path, vm_core::GuestAddr(addr))
    }

    pub fn load_test_program(&mut self, code_base: u64) -> Result<(), VmError> {
        #[cfg(feature = "frontend")]
        Self::encode_test_program();

        self.vm_service
            .load_test_program(vm_core::GuestAddr(code_base))
    }

    #[cfg(feature = "frontend")]
    fn encode_test_program() {
        use vm_frontend::riscv64::api::*;

        let data_base: u64 = 0x100;

        let _code = vec![
            encode_addi(1, 0, 10),                // li x1, 10
            encode_addi(2, 0, 20),                // li x2, 20
            encode_add(3, 1, 2),                  // add x3, x1, x2
            encode_addi(10, 0, data_base as i32), // li x10, 0x100
            encode_sw(10, 3, 0),                  // sw x3, 0(x10)
            encode_lw(4, 10, 0),                  // lw x4, 0(x10)
            encode_beq(3, 4, 8),                  // beq x3, x4, +8
            encode_addi(5, 0, 1),                 // li x5, 1 (skipped)
            encode_addi(6, 0, 2),                 // li x6, 2
            encode_jal(0, 0),                     // j . (halt)
        ];
    }

    pub fn run(&mut self, start_pc: u64) -> Result<(), VmError> {
        self.vm_service.run(vm_core::GuestAddr(start_pc))
    }

    pub fn configure_tlb_from_env(&mut self) -> Result<(), VmError> {
        self.vm_service.configure_tlb_from_env()
    }

    pub fn set_trap_handler(&mut self, h: TrapHandler) {
        self.vm_service.set_trap_handler(h);
    }

    pub fn set_irq_policy(&mut self, p: IrqPolicy) {
        self.vm_service.set_irq_policy(p);
    }

    pub fn request_stop(&self) {
        self.vm_service.request_stop();
    }

    pub fn request_pause(&self) {
        self.vm_service.request_pause();
    }

    pub fn request_resume(&self) {
        self.vm_service.request_resume();
    }

    pub fn get_reg(&self, idx: usize) -> u64 {
        // 向后兼容：从 interpreter 获取
        self.interpreter.get_reg(idx as u32)
    }

    pub async fn run_async(&mut self, start_pc: u64) -> Result<(), VmError> {
        self.vm_service
            .run_async(vm_core::GuestAddr(start_pc))
            .await
    }

    pub fn create_snapshot(
        &mut self,
        name: String,
        _description: String,
    ) -> Result<String, VmError> {
        // Snapshot functionality temporarily disabled
        log::warn!("Snapshot functionality temporarily disabled: {}", name);
        Err(VmError::Core(vm_core::CoreError::NotImplemented {
            feature: "create_snapshot".to_string(),
            module: "vm-service".to_string(),
        }))
    }

    pub fn restore_snapshot(&mut self, id: &str) -> Result<(), VmError> {
        // Snapshot functionality temporarily disabled
        log::warn!("Snapshot restore temporarily disabled: {}", id);
        Err(VmError::Core(vm_core::CoreError::NotImplemented {
            feature: "restore_snapshot".to_string(),
            module: "vm-service".to_string(),
        }))
    }

    pub fn list_snapshots(&self) -> Result<Vec<String>, VmError> {
        // Snapshot functionality temporarily disabled
        Ok(vec![])
    }

    pub fn create_template(
        &mut self,
        name: String,
        description: String,
        base_snapshot_id: String,
    ) -> Result<String, VmError> {
        self.vm_service
            .create_template(name, description, base_snapshot_id)
    }

    pub fn list_templates(&self) -> Result<Vec<String>, VmError> {
        self.vm_service.list_templates()
    }

    pub fn serialize_state(&self) -> Result<Vec<u8>, VmError> {
        self.vm_service.serialize_state()
    }

    pub fn deserialize_state(&mut self, data: &[u8]) -> Result<(), VmError> {
        self.vm_service.deserialize_state(data)
    }

    /// 设置JIT热配置值
    pub fn set_hot_config_vals(
        &mut self,
        _min_threshold: u32,
        _max_threshold: u32,
        _sample_window: Option<u32>,
        _compile_weight: Option<f32>,
        _benefit_weight: Option<f32>,
    ) {
        #[cfg(feature = "jit")]
        {
            if let Some(ref jit) = self.jit_compiler {
                // JIT配置由vm-engine的JITCompiler管理
                info!(
                    "JIT hot configuration updated (thresholds: {} - {})",
                    _min_threshold, _max_threshold
                );
            } else {
                info!("JIT not enabled, configuration ignored");
            }
        }
        #[cfg(not(feature = "jit"))]
        {
            info!("JIT feature not enabled, configuration ignored");
        }
    }

    /// 设置共享池
    pub fn set_shared_pool(&mut self, _enable: bool) {
        #[cfg(feature = "jit")]
        {
            if let Some(ref _jit) = self.jit_compiler {
                info!("Shared pool setting updated: {}", _enable);
            } else {
                info!("JIT not enabled, shared pool setting ignored");
            }
        }
        #[cfg(not(feature = "jit"))]
        {
            info!("JIT feature not enabled, shared pool setting ignored");
        }
    }
}
