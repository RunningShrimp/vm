#[cfg(not(feature = "no_std"))]
pub mod device_event_handler;
pub mod device_service;
#[cfg(not(feature = "no_std"))]
pub mod execution_event_handler;
#[cfg(not(feature = "no_std"))]
pub mod event_handlers;
#[cfg(not(feature = "no_std"))]
pub mod memory_event_handler;
#[cfg(not(feature = "no_std"))]
pub mod snapshot_event_handler;
pub mod vm_service;
#[cfg(not(feature = "no_std"))]
pub mod vm_service_event_driven;

use log::{debug, error, info};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use thiserror::Error;
use tracing::{debug as tdebug, info as tinfo};

use crate::device_service::DeviceService;
use crate::vm_service::VirtualMachineService;
use vm_core::vm_state::VirtualMachineState;
use vm_core::{
    Decoder, ExecStatus, ExecutionEngine, Fault, GuestAddr, VirtualMachine, VmConfig, VmError,
};
use vm_engine_interpreter::ExecInterruptAction;
use vm_engine_interpreter::Interpreter;
use vm_engine_jit::Jit;
use vm_frontend_riscv64::RiscvDecoder;
use vm_ir::{IRBlock, Terminator};
use vm_mem::SoftMmu;

/// VmService - 薄包装层
///
/// 保持向后兼容的API，将所有业务逻辑委托给 VirtualMachineService
pub struct VmService {
    /// 虚拟机实例（保留用于向后兼容）
    pub vm: VirtualMachine<IRBlock>,
    /// 虚拟机服务（包含所有业务逻辑）
    vm_service: VirtualMachineService<IRBlock>,
    /// 设备服务（处理所有设备相关业务逻辑）
    device_service: DeviceService,
    /// 解释器（保留用于向后兼容，将来通过 vm.vcpus 管理）
    interpreter: Interpreter,
}

impl VmService {
    pub async fn new(config: VmConfig, gpu_backend: Option<String>) -> Result<Self, VmError> {
        info!("Initializing VM Service with config: {:?}", config);
        tinfo!(guest_arch=?config.guest_arch, vcpus=?config.vcpu_count, mem=?config.memory_size, exec=?config.exec_mode, "service:new");

        // JIT adaptive and pool env config
        let mut adaptive_cfg = vm_engine_jit::AdaptiveThresholdConfig::default();
        let mut share_pool = std::env::var("VM_JIT_SHARE_POOL")
            .map(|s| !(s == "0" || s.eq_ignore_ascii_case("false")))
            .unwrap_or(true);
        if let Ok(cfg_path) = std::env::var("VM_SERVICE_CONFIG") {
            if let Ok(text) = std::fs::read_to_string(&cfg_path) {
                for line in text.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    if let Some((k, v)) = line.split_once('=') {
                        let k = k.trim();
                        let v = v.trim();
                        match k {
                            "VM_JIT_MIN_THRESHOLD"
                            | "VM_JIT_MAX_THRESHOLD"
                            | "VM_JIT_SAMPLE_WINDOW"
                            | "VM_JIT_COMPILE_TIME_WEIGHT"
                            | "VM_JIT_EXEC_BENEFIT_WEIGHT" => { /* ignored in stub */ }
                            "VM_JIT_SHARE_POOL" => {
                                share_pool = !(v == "0" || v.eq_ignore_ascii_case("false"));
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        // Ignored fields in stub
        if let Ok(v) = std::env::var("VM_JIT_SHARE_POOL") {
            share_pool = !(v == "0" || v.eq_ignore_ascii_case("false"));
        }

        // Create MMU
        let mmu = SoftMmu::new(config.memory_size, false);
        let vm: VirtualMachine<IRBlock> = VirtualMachine::with_mmu(config.clone(), Box::new(mmu));
        let mmu_arc = vm.mmu();

        // 初始化设备服务
        let mut device_service = DeviceService::new();
        device_service.init_gpu(gpu_backend)?;
        device_service
            .initialize_devices(&config, mmu_arc.clone())
            .await?;
        device_service.map_devices(&config).await?;
        device_service.start_polling()?;

        // Initialize Decoder and Interpreter
        // Currently hardcoded for RISC-V 64
        // Decoder is now integrated within each execution engine
        let mut interpreter = Interpreter::new();
        interpreter.set_reg(0, 0); // x0 = 0

        // 创建 VirtualMachineState 和 VirtualMachineService
        // 从 vm 中提取 MMU（注意：这里我们使用相同的 MMU 引用）
        // 为了保持状态一致性，我们使用 vm 的 MMU
        let mmu_for_state =
            Box::new(SoftMmu::new(config.memory_size, false)) as Box<dyn vm_core::MMU>;

        let vm_state = VirtualMachineState::new(config.clone(), mmu_for_state);
        let mut vm_service = VirtualMachineService::new(vm_state);

        // 配置 JIT 相关设置
        if share_pool {
            vm_service.set_shared_pool(true);
        }
        vm_service.set_hot_config(adaptive_cfg);

        Ok(Self {
            vm,
            vm_service,
            device_service,
            interpreter,
        })
    }

    pub fn load_kernel(&mut self, path: &str, addr: u64) -> Result<(), VmError> {
        info!("Loading kernel from {} to {:#x}", path, addr);
        self.vm_service.load_kernel_file(path, addr)
    }

    pub fn load_test_program(&mut self, code_base: u64) -> Result<(), VmError> {
        use vm_frontend_riscv64::api::*;

        let data_base: u64 = 0x100;

        let code = vec![
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

        self.vm_service.load_test_program(code_base)
    }

    pub fn run(&mut self, start_pc: u64) -> Result<(), VmError> {
        self.vm_service.run(start_pc)
    }

    pub fn configure_tlb_from_env(&mut self) -> Result<(), VmError> {
        self.vm_service.configure_tlb_from_env()
    }

    pub fn set_trap_handler(
        &mut self,
        h: Arc<dyn Fn(&VmError, &mut Interpreter) -> ExecInterruptAction + Send + Sync>,
    ) {
        self.vm_service.set_trap_handler(h);
    }

    pub fn set_irq_policy(
        &mut self,
        p: Arc<dyn Fn(&mut Interpreter) -> ExecInterruptAction + Send + Sync>,
    ) {
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

    pub fn hot_stats(&self) -> Option<vm_engine_jit::AdaptiveThresholdStats> {
        self.vm_service.hot_stats()
    }

    pub fn set_hot_config(&mut self, cfg: vm_engine_jit::AdaptiveThresholdConfig) {
        self.vm_service.set_hot_config(cfg);
    }

    pub fn set_hot_config_vals(
        &mut self,
        min: u64,
        max: u64,
        window: Option<usize>,
        compile_w: Option<f64>,
        benefit_w: Option<f64>,
    ) {
        self.vm_service
            .set_hot_config_vals(min, max, window, compile_w, benefit_w);
    }

    pub fn set_shared_pool(&mut self, enable: bool) {
        self.vm_service.set_shared_pool(enable);
    }

    pub fn hot_snapshot(
        &self,
    ) -> Option<(
        vm_engine_jit::AdaptiveThresholdConfig,
        vm_engine_jit::AdaptiveThresholdStats,
    )> {
        self.vm_service.hot_snapshot()
    }

    pub fn export_hot_snapshot_json(&self) -> Option<String> {
        self.vm_service.export_hot_snapshot_json()
    }

    pub async fn run_async(&mut self, start_pc: u64) -> Result<(), VmError> {
        self.vm_service.run_async(start_pc).await
    }

    pub fn create_snapshot(
        &mut self,
        name: String,
        description: String,
    ) -> Result<String, VmError> {
        self.vm_service.create_snapshot(name, description)
    }

    pub fn restore_snapshot(&mut self, id: &str) -> Result<(), VmError> {
        self.vm_service.restore_snapshot(id)
    }

    pub fn list_snapshots(&self) -> Result<Vec<vm_core::snapshot::Snapshot>, VmError> {
        self.vm_service.list_snapshots()
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

    pub fn list_templates(&self) -> Result<Vec<vm_core::template::VmTemplate>, VmError> {
        self.vm_service.list_templates()
    }

    pub fn serialize_state(&self) -> Result<Vec<u8>, VmError> {
        self.vm_service.serialize_state()
    }

    pub fn deserialize_state(&mut self, data: &[u8]) -> Result<(), VmError> {
        self.vm_service.deserialize_state(data)
    }
}
