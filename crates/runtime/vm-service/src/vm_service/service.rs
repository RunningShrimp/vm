use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use vm_core::vm_state::VirtualMachineState;
use vm_core::{GuestAddr, MemoryError, VmConfig, VmError, VmLifecycleState, VmResult};
use vm_mem::SoftMmu;

// Re-export type aliases for public use
pub use super::execution::IrqPolicy;
pub use super::execution::TrapHandler;
use super::execution::{ExecutionContext, run_async, run_sync};
use super::lifecycle::{pause, request_pause, request_resume, request_stop, reset, start, stop};
#[cfg(feature = "performance")]
use super::performance::{PerformanceConfig, PerformanceContext, PerformanceStats};
#[cfg(feature = "smmu")]
use super::smmu::SmmuContext;

/// 虚拟机服务
///
/// 负责处理虚拟机的业务逻辑，符合DDD领域服务模式。
///
/// # 核心职责
/// - **内核加载**: 从文件或内存加载Guest内核
/// - **生命周期管理**: 启动、暂停、停止、重置虚拟机
/// - **状态管理**: 序列化和反序列化VM状态
/// - **执行控制**: 同步/异步执行，陷阱处理
///
/// # 使用场景
/// - 虚拟机创建：初始化VM实例
/// - 内核引导：加载和启动Guest OS
/// - 运行时控制：暂停、恢复、停止VM
/// - 快照操作：保存和恢复VM状态
///
/// # 特性
/// - `performance`: 启用性能优化（JIT编译、异步I/O、前端生成）
/// - `smmu`: 启用SMMU（IOMMU）支持
///
/// # 示例
/// ```ignore
/// use vm_service::VirtualMachineService;
/// use vm_core::VmConfig;
///
/// let config = VmConfig::default();
/// let service = VirtualMachineService::from_config(config, mmu);
///
/// // 加载内核
/// service.load_kernel_file("kernel.bin", GuestAddr(0x80200000))?;
///
/// // 启动VM
/// service.run(GuestAddr(0x80200000))?;
/// ```
pub struct VirtualMachineService<B> {
    /// 虚拟机状态
    state: Arc<Mutex<VirtualMachineState<B>>>,
    /// 运行标志
    run_flag: Arc<AtomicBool>,
    /// 暂停标志
    pause_flag: Arc<AtomicBool>,
    /// 陷阱处理器
    trap_handler: Option<TrapHandler>,
    /// 中断策略
    irq_policy: Option<IrqPolicy>,
    /// 性能优化上下文 (需要 performance feature)
    #[cfg(feature = "performance")]
    performance: PerformanceContext,
    /// SMMU 上下文 (需要 smmu feature)
    #[cfg(feature = "smmu")]
    smmu: SmmuContext,
}

impl<B: 'static> VirtualMachineService<B> {
    /// 创建新的虚拟机服务
    pub fn new(state: VirtualMachineState<B>) -> Self {
        Self {
            state: Arc::new(Mutex::new(state)),
            run_flag: Arc::new(AtomicBool::new(false)),
            pause_flag: Arc::new(AtomicBool::new(false)),
            trap_handler: None,
            irq_policy: None,
            #[cfg(feature = "performance")]
            performance: PerformanceContext::new(),
            #[cfg(feature = "smmu")]
            smmu: SmmuContext::new(),
        }
    }

    /// 初始化 SMMU 支持
    ///
    /// 创建并初始化 SMMU 管理器和设备管理器。
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`，失败返回错误。
    #[cfg(feature = "smmu")]
    pub fn init_smmu(&mut self) -> VmResult<()> {
        self.smmu.init()
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
        super::kernel_loader::load_kernel(mmu, data, load_addr)
    }

    /// Load bzImage kernel with proper setup/protected mode separation
    ///
    /// This method properly loads bzImage kernels by:
    /// 1. Splitting the kernel into setup code and protected mode code
    /// 2. Loading setup code at 0x10000
    /// 3. Loading protected mode code at 0x100000
    ///
    /// # Arguments
    /// * `data` - Kernel file data
    /// * `setup_load_addr` - Where to load setup code (typically 0x10000)
    /// * `pm_load_addr` - Where to load protected mode code (typically 0x100000)
    ///
    /// # Returns
    /// Entry point for the kernel (setup code address)
    pub fn load_bzimage_kernel(
        &mut self,
        data: &[u8],
        setup_load_addr: GuestAddr,
        pm_load_addr: GuestAddr,
    ) -> VmResult<GuestAddr> {
        // 业务逻辑：验证内核数据
        if data.is_empty() {
            return Err(VmError::Core(vm_core::CoreError::Config {
                message: "Kernel data cannot be empty".to_string(),
                path: Some("kernel_data".to_string()),
            }));
        }

        // 业务逻辑：验证加载地址
        if setup_load_addr == vm_core::GuestAddr(0) || pm_load_addr == vm_core::GuestAddr(0) {
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
        super::kernel_loader::load_bzimage_kernel_properly(mmu, data, setup_load_addr, pm_load_addr)
    }

    /// 从文件加载内核
    pub fn load_kernel_file(&self, path: &str, load_addr: GuestAddr) -> VmResult<()> {
        let data = super::kernel_loader::load_kernel_file(path, load_addr)?;

        // Get MMU from state
        let state = self.state.lock().map_err(|_| {
            VmError::Memory(MemoryError::MmuLockFailed {
                message: "Failed to acquire state lock".to_string(),
            })
        })?;

        let mmu = state.mmu();
        drop(state);

        // Load kernel data
        super::kernel_loader::load_kernel(mmu, &data, load_addr)
    }

    /// Load bzImage kernel and return entry point
    ///
    /// This method is specifically for Linux bzImage kernels. It:
    /// 1. Loads the kernel at the specified address
    /// 2. Parses the boot protocol header
    /// 3. Returns the correct entry point (bypassing real-mode setup)
    pub fn load_bzimage_kernel_file(
        &self,
        path: &str,
        load_addr: GuestAddr,
    ) -> VmResult<GuestAddr> {
        let data = super::kernel_loader::load_kernel_file(path, load_addr)?;

        // Get MMU from state
        let state = self.state.lock().map_err(|_| {
            VmError::Memory(MemoryError::MmuLockFailed {
                message: "Failed to acquire state lock".to_string(),
            })
        })?;

        let mmu = state.mmu();
        drop(state);

        // Load bzImage and get entry point
        let entry_point = super::kernel_loader::load_bzimage_kernel(mmu, &data, load_addr)?;

        log::info!("bzImage loaded, entry point: 0x{:x}", entry_point.0);

        Ok(entry_point)
    }

    /// 使用异步I/O从文件加载内核（需要 performance feature）
    #[cfg(feature = "performance")]
    pub fn load_kernel_file_async(&self, path: &str, load_addr: GuestAddr) -> VmResult<()> {
        // 验证状态
        let state = self.state.lock().map_err(|_| {
            VmError::Memory(MemoryError::MmuLockFailed {
                message: "Failed to acquire state lock".to_string(),
            })
        })?;

        if state.state() == VmLifecycleState::Running {
            drop(state);
            return Err(VmError::Core(vm_core::CoreError::InvalidState {
                message: "Cannot load kernel while VM is running".to_string(),
                current: "running".to_string(),
                expected: "stopped".to_string(),
            }));
        }

        if load_addr == vm_core::GuestAddr(0) {
            drop(state);
            return Err(VmError::Core(vm_core::CoreError::Config {
                message: "Load address cannot be zero".to_string(),
                path: Some("load_addr".to_string()),
            }));
        }

        drop(state);

        // 创建异步MMU包装器
        use vm_mem::SoftMmu;
        let soft_mmu =
            Arc::new(tokio::sync::Mutex::new(
                Box::new(SoftMmu::new(1024 * 1024 * 1024, false)) as Box<dyn vm_core::MMU + Send>,
            ));

        // 调用异步加载包装器
        super::kernel_loader::load_kernel_file_async_sync(soft_mmu, path, load_addr)
    }

    /// 使用异步I/O从内存数据加载内核（需要 performance feature）
    #[cfg(feature = "performance")]
    pub fn load_kernel_async(&self, data: &[u8], load_addr: GuestAddr) -> VmResult<()> {
        // 验证状态
        let state = self.state.lock().map_err(|_| {
            VmError::Memory(MemoryError::MmuLockFailed {
                message: "Failed to acquire state lock".to_string(),
            })
        })?;

        if state.state() == VmLifecycleState::Running {
            drop(state);
            return Err(VmError::Core(vm_core::CoreError::InvalidState {
                message: "Cannot load kernel while VM is running".to_string(),
                current: "running".to_string(),
                expected: "stopped".to_string(),
            }));
        }

        if load_addr == vm_core::GuestAddr(0) {
            drop(state);
            return Err(VmError::Core(vm_core::CoreError::Config {
                message: "Load address cannot be zero".to_string(),
                path: Some("load_addr".to_string()),
            }));
        }

        if data.is_empty() {
            drop(state);
            return Err(VmError::Core(vm_core::CoreError::Config {
                message: "Kernel data cannot be empty".to_string(),
                path: Some("kernel_data".to_string()),
            }));
        }

        drop(state);

        // 创建异步MMU包装器并使用async API加载
        use vm_mem::SoftMmu;
        let soft_mmu =
            Arc::new(tokio::sync::Mutex::new(
                Box::new(SoftMmu::new(1024 * 1024 * 1024, false)) as Box<dyn vm_core::MMU + Send>,
            ));

        // 调用异步加载包装器
        super::kernel_loader::load_kernel_async_sync(soft_mmu, data, load_addr)
    }

    /// 启动 VM
    pub fn start(&self) -> VmResult<()> {
        start(Arc::clone(&self.state))
    }

    /// 暂停 VM
    pub fn pause(&self) -> VmResult<()> {
        pause(Arc::clone(&self.state))
    }

    /// 停止 VM
    pub fn stop(&self) -> VmResult<()> {
        stop(Arc::clone(&self.state))
    }

    /// 重置 VM
    pub fn reset(&self) -> VmResult<()> {
        reset(Arc::clone(&self.state))
    }

    /// 创建快照（调用模块函数）
    pub fn create_snapshot(&self, name: String, description: String) -> VmResult<String> {
        super::snapshot_manager::create_snapshot(Arc::clone(&self.state), name, description)
    }

    /// 异步创建快照（需要 performance feature）
    #[cfg(feature = "performance")]
    pub async fn create_snapshot_async(
        &self,
        name: String,
        description: String,
    ) -> VmResult<String> {
        // Note: VirtualMachineService uses std::sync::Mutex for optimal performance
        // in synchronous operations (start, pause, stop, etc.). Converting to
        // tokio::sync::Mutex would require ALL methods to become async, including
        // simple lifecycle operations, which is not desirable.
        //
        // Note: snapshot_manager functions are synchronous, but we're in an async context
        // We can call them directly since they don't block significantly
        super::snapshot_manager::create_snapshot(Arc::clone(&self.state), name, description)
    }

    /// 恢复快照（调用模块函数）
    pub fn restore_snapshot(&self, id: &str) -> VmResult<()> {
        super::snapshot_manager::restore_snapshot(Arc::clone(&self.state), id)
    }

    /// 异步恢复快照（需要 performance feature）
    #[cfg(feature = "performance")]
    pub async fn restore_snapshot_async(&self, id: &str) -> VmResult<()> {
        // Note: VirtualMachineService uses std::sync::Mutex for optimal performance
        // in synchronous operations (start, pause, stop, etc.). Converting to
        // tokio::sync::Mutex would require ALL methods to become async, including
        // simple lifecycle operations, which is not desirable.
        //
        // Note: snapshot_manager functions are synchronous, but we're in an async context
        // We can call them directly since they don't block significantly
        super::snapshot_manager::restore_snapshot(Arc::clone(&self.state), id)
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

    /// 生成测试程序代码（需要 performance feature）
    #[cfg(feature = "performance")]
    fn generate_test_program(&self) -> VmResult<Vec<u32>> {
        use vm_frontend::riscv64::{
            encode_add, encode_addi, encode_beq, encode_jal, encode_lw, encode_sw,
        };

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

    /// 生成测试程序代码（未启用性能功能时）
    #[cfg(not(feature = "performance"))]
    fn generate_test_program(&self) -> VmResult<Vec<u32>> {
        Err(VmError::Core(vm_core::CoreError::NotImplemented {
            feature: "Test program generation".to_string(),
            module: "vm-service".to_string(),
        }))
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
    pub fn set_trap_handler(&mut self, h: TrapHandler) {
        self.trap_handler = Some(h);
    }

    /// 设置中断策略
    pub fn set_irq_policy(&mut self, p: IrqPolicy) {
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

    /// 获取MMU的Arc引用（用于x86启动等特殊操作）
    ///
    /// # Safety
    /// 此方法提供对底层MMU Arc的访问，仅应用于高级操作如x86启动器
    pub fn mmu_arc(&self) -> VmResult<Arc<std::sync::Mutex<Box<dyn vm_core::MMU>>>> {
        let state = self.state.lock().map_err(|_| {
            VmError::Core(vm_core::CoreError::Internal {
                message: "Failed to lock state".to_string(),
                module: "VirtualMachineService".to_string(),
            })
        })?;

        Ok(state.mmu())
    }

    /// 启动x86内核（使用real-mode启动器和正确的boot protocol）
    ///
    /// 此方法专门用于x86_64内核的启动流程：
    /// 1. 设置Linux boot protocol参数
    /// 2. 计算正确的入口点
    /// 3. 使用X86BootExecutor执行real-mode启动代码
    /// 4. 处理模式转换（Real → Protected → Long）
    /// 5. 返回64位内核入口点或启动结果
    pub fn boot_x86_kernel(&mut self) -> VmResult<super::x86_boot_exec::X86BootResult> {
        use super::x86_boot_exec::X86BootExecutor;
        use super::x86_boot_setup::BootConfig;

        log::info!("=== Starting x86 Boot Sequence with Boot Protocol ===");

        // Get MMU access
        let mmu_arc = self.mmu_arc()?;
        let mut mmu_guard = mmu_arc.lock().map_err(|_| {
            VmError::Memory(vm_core::MemoryError::MmuLockFailed {
                message: "Failed to lock MMU".to_string(),
            })
        })?;

        // Install ACPI tables for hardware discovery
        use super::acpi::AcpiManager;
        let mut acpi_manager = AcpiManager::new();
        acpi_manager.install_tables(&mut **mmu_guard)?;
        log::info!("✓ ACPI tables installed for hardware discovery");

        // Create boot executor
        let mut executor = X86BootExecutor::new();

        // Initialize EFI runtime for framebuffer support
        log::info!("=== Initializing EFI Runtime ===");
        let mut efi_runtime = super::efi::EfiRuntime::new();
        efi_runtime.initialize(&mut **mmu_guard)?;

        // Get framebuffer info
        let (fb_addr, fb_width, fb_height, fb_bpp) = efi_runtime.get_framebuffer_info();
        let fb_stride = fb_width * (fb_bpp / 8);

        log::info!(
            "EFI Framebuffer: {:#010X} - {}x{}x{} (stride: {})",
            fb_addr,
            fb_width,
            fb_height,
            fb_bpp,
            fb_stride
        );

        // Configure boot protocol parameters with EFI framebuffer
        // This tells the kernel to use the EFI framebuffer for display
        let config = BootConfig {
            vid_mode: 0xFFFF, // Normal VGA text mode (80x25)
            root_dev: 0,      // Use default
            // Ubuntu installer boot parameters with EFI framebuffer
            cmdline: "boot=casper debug ignore_loglevel --".to_string(),
            initrd_addr: None,
            initrd_size: None,
            // EFI framebuffer configuration
            efifb_addr: Some(fb_addr),
            efifb_width: Some(fb_width),
            efifb_height: Some(fb_height),
            efifb_stride: Some(fb_stride),
        };

        // Kernel is loaded at 0x10000 by install-debian command
        let kernel_load_addr = 0x10000;
        log::info!("Kernel load address: {:#010X}", kernel_load_addr);

        // Execute boot sequence with proper boot protocol setup
        let result = executor.boot_with_protocol(&mut **mmu_guard, kernel_load_addr, &config)?;

        log::info!("=== Boot Sequence Complete ===");
        Ok(result)
    }

    /// 列出所有快照
    pub fn list_snapshots(&self) -> VmResult<Vec<String>> {
        super::snapshot_manager::list_snapshots(Arc::clone(&self.state))
    }

    /// 列出所有模板
    pub fn list_templates(&self) -> VmResult<Vec<String>> {
        super::snapshot_manager::list_templates(Arc::clone(&self.state))
    }

    /// 创建模板
    pub fn create_template(
        &self,
        name: String,
        description: String,
        base_snapshot_id: String,
    ) -> VmResult<String> {
        super::snapshot_manager::create_template(
            Arc::clone(&self.state),
            name,
            description,
            base_snapshot_id,
        )
    }

    /// 序列化虚拟机状态以进行迁移
    pub fn serialize_state(&self) -> VmResult<Vec<u8>> {
        super::snapshot_manager::serialize_state(Arc::clone(&self.state))
    }

    /// 从序列化数据中反序列化并恢复虚拟机状态
    pub fn deserialize_state(&self, data: &[u8]) -> VmResult<()> {
        super::snapshot_manager::deserialize_state(Arc::clone(&self.state), data)
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

    // ============================================================
    // 性能优化功能 (需要 performance feature)
    // ============================================================
    #[cfg(feature = "performance")]
    pub fn get_performance_stats(&self) -> Option<PerformanceStats> {
        self.performance.get_stats()
    }

    #[cfg(feature = "performance")]
    pub fn set_performance_config(&mut self, config: PerformanceConfig) {
        self.performance.set_config(config);
    }

    #[cfg(not(feature = "performance"))]
    pub fn get_performance_stats(&self) -> Option<()> {
        None
    }

    #[cfg(not(feature = "performance"))]
    pub fn set_performance_config(&mut self, _config: ()) {
        // No-op when performance features are disabled
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

        // 使用性能上下文创建执行上下文
        let ctx = self.create_execution_context(guest_arch);

        run_sync(&ctx, start_pc, base_mmu, debug, vcpu_count)
    }

    /// 异步执行循环（需要 performance feature）
    #[cfg(feature = "performance")]
    pub async fn run_async(&self, start_pc: GuestAddr) -> VmResult<()> {
        self.start()?;

        // Extract all needed data from state and drop the lock before await
        let (mmu_arc, debug, vcpu_count, exec_mode, guest_arch) = {
            let state = self.state.lock().map_err(|_| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: "Failed to lock state".to_string(),
                    module: "VirtualMachineService".to_string(),
                })
            })?;

            let mmu = state.mmu();
            let debug = false; // VmConfig中没有debug_trace字段，使用默认值
            let vcpu_count = state.config().vcpu_count;
            let exec_mode = state.config().exec_mode;
            let guest_arch = state.config().guest_arch;
            (mmu, debug, vcpu_count, exec_mode, guest_arch)
        };

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
        let coroutine_scheduler: Option<Arc<Mutex<vm_core::runtime::CoroutineScheduler>>> =
            if vcpu_count > 1 {
                // 为多vCPU场景创建协程调度器
                Some(Arc::new(Mutex::new(
                    vm_core::runtime::CoroutineScheduler::new().map_err(|e| {
                        VmError::Core(vm_core::CoreError::Internal {
                            message: format!("Failed to create coroutine scheduler: {}", e),
                            module: "VirtualMachineService".to_string(),
                        })
                    })?,
                )))
            } else {
                None
            };

        // 使用性能上下文创建异步执行上下文
        let ctx = self.create_async_execution_context(guest_arch, coroutine_scheduler);

        run_async(&ctx, start_pc, base_mmu, debug, vcpu_count, exec_mode).await
    }

    /// 创建执行上下文（辅助方法）
    #[cfg(feature = "performance")]
    fn create_execution_context(&self, guest_arch: vm_core::GuestArch) -> ExecutionContext {
        self.performance.create_execution_context(
            Arc::clone(&self.run_flag),
            Arc::clone(&self.pause_flag),
            guest_arch,
            self.trap_handler.clone(),
            self.irq_policy.clone(),
        )
    }

    #[cfg(not(feature = "performance"))]
    fn create_execution_context(&self, guest_arch: vm_core::GuestArch) -> ExecutionContext {
        ExecutionContext {
            run_flag: Arc::clone(&self.run_flag),
            pause_flag: Arc::clone(&self.pause_flag),
            guest_arch,
            trap_handler: self.trap_handler.clone(),
            irq_policy: self.irq_policy.clone(),
        }
    }

    /// 创建异步执行上下文（辅助方法）
    #[cfg(feature = "performance")]
    fn create_async_execution_context(
        &self,
        guest_arch: vm_core::GuestArch,
        coroutine_scheduler: Option<Arc<Mutex<vm_core::runtime::CoroutineScheduler>>>,
    ) -> ExecutionContext {
        self.performance.create_async_execution_context(
            Arc::clone(&self.run_flag),
            Arc::clone(&self.pause_flag),
            guest_arch,
            self.trap_handler.clone(),
            self.irq_policy.clone(),
            coroutine_scheduler,
        )
    }
}

// ============================================================
// SMMU 设备管理功能 (需要 smmu feature)
// ============================================================
#[cfg(feature = "smmu")]
impl<B> VirtualMachineService<B> {
    /// 附加设备到 SMMU
    ///
    /// 为 PCIe 设备分配 SMMU Stream ID 并配置 DMA 地址空间。
    ///
    /// # 参数
    ///
    /// * `bdf` - PCIe BDF 标识符 (格式: "BBBB:DD:F.F")
    /// * `dma_start` - DMA 地址空间起始地址
    /// * `dma_size` - DMA 地址空间大小
    ///
    /// # 返回值
    ///
    /// 成功返回分配的 Stream ID，失败返回错误。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let stream_id = service.attach_device_to_smmu("0000:01:00.0", 0x1000, 0x10000)?;
    /// ```
    pub fn attach_device_to_smmu(&self, bdf: &str, dma_start: u64, dma_size: u64) -> VmResult<u16> {
        self.smmu.attach_device(bdf, dma_start, dma_size)
    }

    /// 从 SMMU 移除设备
    ///
    /// # 参数
    ///
    /// * `bdf` - PCIe BDF 标识符
    pub fn detach_device_from_smmu(&self, bdf: &str) -> VmResult<()> {
        self.smmu.detach_device(bdf)
    }

    /// 转换设备的 DMA 地址
    ///
    /// # 参数
    ///
    /// * `bdf` - PCIe BDF 标识符
    /// * `guest_addr` - 客户机物理地址
    /// * `size` - 访问大小
    ///
    /// # 返回值
    ///
    /// 成功返回转换后的物理地址，失败返回错误。
    pub fn translate_device_dma(
        &self,
        bdf: &str,
        guest_addr: GuestAddr,
        size: u64,
    ) -> VmResult<u64> {
        self.smmu.translate_dma(bdf, guest_addr, size)
    }

    /// 列出所有附加到 SMMU 的设备
    ///
    /// # 返回值
    ///
    /// 返回设备 BDF 列表。
    pub fn list_smmu_devices(&self) -> VmResult<Vec<String>> {
        self.smmu.list_devices()
    }

    /// 获取 SMMU 统计信息
    ///
    /// # 返回值
    ///
    /// 返回 SMMU 统计信息，包括转换次数、缓存命中率等。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let stats = service.get_smmu_stats()?;
    /// println!("Total translations: {}", stats.total_translations);
    /// ```
    pub fn get_smmu_stats(&self) -> VmResult<vm_smmu::SmmuStats> {
        self.smmu.get_stats()
    }
}
