// Core modules (always present)
pub mod vm_service;

// Conditional features module
#[cfg(feature = "devices")]
pub mod device_service;

// Feature-based re-exports (consolidated)
pub use vm_service::{IrqPolicy, TrapHandler, VirtualMachineService as CoreVmService};

#[cfg(feature = "devices")]
pub use device_service::DeviceService;

#[cfg(feature = "smmu")]
pub use vm_smmu::{SmmuConfig, SmmuStats};

use log::info;
use std::sync::{Arc, Mutex};
use tracing::info as tinfo;

use vm_core::vm_state::VirtualMachineState;
use vm_core::{VmConfig, VmError};
use vm_engine_interpreter::Interpreter;
use vm_ir::IRBlock;
use vm_mem::SoftMmu;

/// VmService - 薄包装层
///
/// 保持向后兼容的API，将所有业务逻辑委托给 VirtualMachineService
pub struct VmService {
    /// 虚拟机状态实例（保留用于向后兼容）
    pub vm_state: Arc<Mutex<VirtualMachineState<IRBlock>>>,
    /// 虚拟机服务（包含所有业务逻辑）
    vm_service: VirtualMachineService<IRBlock>,
    /// 设备服务（处理所有设备相关业务逻辑）- 需要 devices feature
    #[cfg(feature = "devices")]
    device_service: DeviceService,
    /// 解释器（保留用于向后兼容，将来通过 vm.vcpus 管理）
    interpreter: Interpreter,
}

// 实现Send和Sync特性，使VmService可以在多线程环境中安全共享
unsafe impl Send for VmService {}
unsafe impl Sync for VmService {}

impl VmService {
    pub async fn new(config: VmConfig, gpu_backend: Option<String>) -> Result<Self, VmError> {
        info!("Initializing VM Service with config: {:?}", config);
        tinfo!(guest_arch=?config.guest_arch, vcpus=?config.vcpu_count, mem=?config.memory_size, exec=?config.exec_mode, "service:new");

        // JIT support has been removed
        let _share_pool = false;

        // Create MMU
        let mmu = SoftMmu::new(config.memory_size, false);
        let vm_state: VirtualMachineState<IRBlock> =
            VirtualMachineState::new(config.clone(), Box::new(mmu));
        let mmu_arc = vm_state.mmu.clone();

        // Initialize Decoder and Interpreter
        // Currently hardcoded for RISC-V 64
        // Decoder is now integrated within each execution engine
        let mut interpreter = Interpreter::new();
        interpreter.set_reg(0, 0); // x0 = 0

        // 创建 VirtualMachineState 和 VirtualMachineService
        // 直接使用已创建的 vm_state 来初始化 vm_service
        let vm_state_arc = Arc::new(Mutex::new(vm_state.clone()));
        let vm_service = VirtualMachineService::new(vm_state);

        // 初始化设备服务（仅当 devices feature 启用时）
        #[cfg(feature = "devices")]
        let device_service = Self::init_device_service(gpu_backend, &config, mmu_arc).await?;

        // JIT support has been removed

        let result = Self {
            vm_state: vm_state_arc,
            vm_service,
            #[cfg(feature = "devices")]
            device_service,
            interpreter,
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
        ds.initialize_devices(config, mmu_arc.clone()).await?;
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
        // JIT support has been removed, so this is a no-op
        info!("JIT support has been removed, ignoring set_hot_config_vals call");
    }

    /// 设置共享池
    pub fn set_shared_pool(&mut self, _enable: bool) {
        // JIT support has been removed, so this is a no-op
        info!("JIT support has been removed, ignoring set_shared_pool call");
    }
}
