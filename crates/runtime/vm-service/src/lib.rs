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
    /// AHCI SATA控制器（用于磁盘I/O）
    ahci_controller: Option<vm_device::ahci::AhciController>,
    /// ATAPI CD-ROM设备（用于ISO访问）
    atapi_cdrom: Option<vm_device::atapi::AtapiCdRom>,
}

// 实现Send和Sync特性，使VmService可以在多线程环境中安全共享
unsafe impl Send for VmService {}
unsafe impl Sync for VmService {}

impl VmService {
    pub async fn new(config: VmConfig, gpu_backend: Option<String>) -> Result<Self, VmError> {
        info!("Initializing VM Service with config: {:?}", config);
        tinfo!(guest_arch=?config.guest_arch, vcpus=?config.vcpu_count, mem=?config.memory_size, exec=?config.exec_mode, "service:new");

        // Create MMU - use Arc<SoftMmu> to share with device service
        // For x86_64, increase physical memory to accommodate high load addresses (0x80000000+)
        // This allows Bare mode (identity mapping) to work for kernel loading
        let mmu_memory_size = match config.guest_arch {
            vm_core::GuestArch::X86_64 => std::cmp::max(config.memory_size, 3 * 1024 * 1024 * 1024), // Min 3GB for x86_64
            _ => config.memory_size,
        };

        let mut mmu = SoftMmu::new(mmu_memory_size, false);

        // Set paging mode based on guest architecture
        use vm_mem::PagingMode;
        let paging_mode = match config.guest_arch {
            vm_core::GuestArch::Riscv64 => PagingMode::Sv39,
            vm_core::GuestArch::Arm64 => PagingMode::Arm64,
            vm_core::GuestArch::X86_64 => {
                // x86_64 MMU is now implemented - use X86_64 paging mode
                PagingMode::X86_64
            }
            _ => PagingMode::Bare,
        };
        mmu.set_paging_mode(paging_mode);
        info!(
            "MMU paging mode set to {:?} for guest architecture {:?} (physical memory: {} MB)",
            paging_mode,
            config.guest_arch,
            mmu_memory_size / (1024 * 1024)
        );

        let mmu = Arc::new(mmu);
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
            ahci_controller: None,
            atapi_cdrom: None,
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
        setup_load_addr: u64,
        pm_load_addr: u64,
    ) -> Result<u64, VmError> {
        info!(
            "Loading bzImage kernel: setup={:#x}, pm={:#x}",
            setup_load_addr, pm_load_addr
        );
        let entry_point = self.vm_service.load_bzimage_kernel(
            data,
            vm_core::GuestAddr(setup_load_addr),
            vm_core::GuestAddr(pm_load_addr),
        )?;
        Ok(entry_point.0)
    }

    /// Boot x86 kernel using real-mode boot executor
    ///
    /// This method uses the X86BootExecutor to handle the complete x86 boot sequence:
    /// 1. Real-mode execution
    /// 2. Mode transitions (Real → Protected → Long)
    /// 3. Returns 64-bit kernel entry point
    pub fn boot_x86_kernel(&mut self) -> Result<vm_service::x86_boot_exec::X86BootResult, VmError> {
        info!("Booting x86 kernel with real-mode executor");
        self.vm_service.boot_x86_kernel()
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

    /// 获取执行模式（形成逻辑闭环）
    pub fn exec_mode(&self) -> ExecMode {
        self.exec_mode
    }

    /// 获取服务容器（形成逻辑闭环）
    pub fn service_container(&self) -> &ServiceContainer {
        &self.service_container
    }

    // ============================================================
    // 磁盘和ISO管理功能
    // ============================================================

    /// 创建虚拟磁盘镜像
    ///
    /// # 参数
    /// - `path`: 磁盘镜像文件路径
    /// - `size_gb`: 磁盘大小(GB)
    ///
    /// # 返回
    /// 成功返回磁盘信息,失败返回错误
    pub fn create_disk(
        &self,
        path: &str,
        size_gb: u64,
    ) -> Result<vm_device::disk_image::DiskInfo, String> {
        use vm_device::disk_image::DiskImageCreator;

        log::info!("Creating {}GB disk image at: {}", size_gb, path);

        let creator = DiskImageCreator::new(path, size_gb)
            .format(vm_device::disk_image::DiskFormat::Raw)
            .sector_size(512)
            .preallocate(false);

        match creator.create() {
            Ok(info) => {
                log::info!(
                    "Disk created successfully: {} sectors, {} MB",
                    info.sector_count,
                    info.size_mb()
                );
                Ok(info)
            }
            Err(e) => {
                log::error!("Failed to create disk: {}", e);
                Err(e)
            }
        }
    }

    /// 快速创建20GB磁盘镜像
    ///
    /// # 参数
    /// - `path`: 磁盘镜像文件路径
    pub fn create_disk_20gb(&self, path: &str) -> Result<vm_device::disk_image::DiskInfo, String> {
        self.create_disk(path, 20)
    }

    /// 检查磁盘镜像是否存在并获取信息
    ///
    /// # 参数
    /// - `path`: 磁盘镜像文件路径
    pub fn get_disk_info(&self, path: &str) -> Result<vm_device::disk_image::DiskInfo, String> {
        use vm_device::disk_image::DiskImageCreator;

        let creator = DiskImageCreator::new(path, 1); // size doesn't matter for info check
        creator.info()
    }

    /// 加载ISO镜像到虚拟机
    ///
    /// # 参数
    /// - `iso_path`: ISO镜像文件路径
    ///
    /// # 返回
    /// 成功返回ISO大小和挂载信息,失败返回错误
    pub fn attach_iso(&mut self, iso_path: &str) -> Result<IsoInfo, String> {
        use std::path::Path;

        log::info!("Attaching ISO: {}", iso_path);

        let path = Path::new(iso_path);
        if !path.exists() {
            return Err(format!("ISO file not found: {}", iso_path));
        }

        let metadata =
            std::fs::metadata(path).map_err(|e| format!("Failed to get ISO metadata: {}", e))?;

        let size_bytes = metadata.len();
        let size_mb = size_bytes as f64 / (1024.0 * 1024.0);

        log::info!("ISO attached successfully: {} MB", size_mb);

        Ok(IsoInfo {
            path: iso_path.to_string(),
            size_bytes,
            size_mb: size_mb as u64,
        })
    }

    /// 检查ISO镜像是否存在并获取信息
    ///
    /// # 参数
    /// - `iso_path`: ISO镜像文件路径
    pub fn get_iso_info(&self, iso_path: &str) -> Result<IsoInfo, String> {
        use std::path::Path;

        let path = Path::new(iso_path);
        if !path.exists() {
            return Err(format!("ISO file not found: {}", iso_path));
        }

        let metadata =
            std::fs::metadata(path).map_err(|e| format!("Failed to get ISO metadata: {}", e))?;

        let size_bytes = metadata.len();
        let size_mb = (size_bytes as f64 / (1024.0 * 1024.0)) as u64;

        Ok(IsoInfo {
            path: iso_path.to_string(),
            size_bytes,
            size_mb,
        })
    }

    /// 获取VGA显示内容
    ///
    /// 返回格式化的VGA文本显示(80x25)
    pub fn get_vga_display(&self) -> Result<String, String> {
        use std::sync::Arc;
        use vm_service::vga;

        // 从vm_state获取MMU
        let state = self
            .vm_state
            .lock()
            .map_err(|_| "Failed to acquire state lock".to_string())?;

        let mmu_arc: Arc<std::sync::Mutex<Box<dyn vm_core::MMU>>> = state.mmu();
        let mmu_mutex = mmu_arc
            .lock()
            .map_err(|_| "Failed to acquire MMU lock".to_string())?;
        let mmu = mmu_mutex.as_ref();

        let vga_output = vga::vga_format_border(mmu);

        // 同时保存到文件
        let _ = vga::vga_save_to_file(mmu, "/tmp/debian_vga_output.txt");

        Ok(vga_output)
    }

    /// 保存VGA显示到文件
    ///
    /// # 参数
    /// - `path`: 输出文件路径
    pub fn save_vga_display(&self, path: &str) -> Result<(), String> {
        use std::sync::Arc;
        use vm_service::vga;

        let state = self
            .vm_state
            .lock()
            .map_err(|_| "Failed to acquire state lock".to_string())?;

        let mmu_arc: Arc<std::sync::Mutex<Box<dyn vm_core::MMU>>> = state.mmu();
        let mmu_mutex = mmu_arc
            .lock()
            .map_err(|_| "Failed to acquire MMU lock".to_string())?;
        let mmu = mmu_mutex.as_ref();

        vga::vga_save_to_file(mmu, path)
    }

    /// 获取VESA framebuffer内容 (图形模式)
    ///
    /// 返回VESA线性framebuffer的原始数据
    /// 用于捕获Ubuntu graphical installer的显示内容
    ///
    /// # 返回
    /// 成功返回(framebuffer地址, 宽度, 高度, bits_per_pixel, 数据)
    /// 失败返回错误信息
    pub fn get_vesa_framebuffer(&self) -> Result<(u64, u16, u16, u8, Vec<u8>), String> {
        use std::sync::Arc;

        // 从vm_state获取MMU
        let state = self
            .vm_state
            .lock()
            .map_err(|_| "Failed to acquire state lock".to_string())?;

        let mmu_arc: Arc<std::sync::Mutex<Box<dyn vm_core::MMU>>> = state.mmu();
        let mmu_mutex = mmu_arc
            .lock()
            .map_err(|_| "Failed to acquire MMU lock".to_string())?;
        let mmu = mmu_mutex.as_ref();

        // VESA framebuffer地址 (标准LFB地址: 0xE0000000)
        let fb_addr = 0xE0000000u64;

        // 尝试不同的分辨率和格式
        // Ubuntu installer typically uses 1024x768x24 or 1024x768x32
        let resolutions = vec![
            (1024, 768, 32), // Most common for Ubuntu
            (1024, 768, 24),
            (800, 600, 32),
            (800, 600, 24),
            (1920, 1080, 32),
            (1920, 1080, 24),
        ];

        for (width, height, bpp) in resolutions {
            let bytes_per_pixel = bpp / 8;
            let fb_size = width as usize * height as usize * bytes_per_pixel;

            log::debug!(
                "Trying VESA mode: {}x{}x{}bpp @ {:#010X}, size: {} bytes",
                width,
                height,
                bpp,
                fb_addr,
                fb_size
            );

            let mut framebuffer_data = Vec::with_capacity(fb_size);

            // 读取framebuffer数据
            let mut valid_data = true;
            for i in 0..fb_size {
                let addr = vm_core::GuestAddr(fb_addr + i as u64);
                match mmu.read(addr, 1) {
                    Ok(byte) => framebuffer_data.push(byte as u8),
                    Err(_) => {
                        valid_data = false;
                        break;
                    }
                }
            }

            if valid_data && framebuffer_data.len() == fb_size {
                // 检查是否全黑 (未初始化)
                let is_all_black = framebuffer_data.iter().all(|&b| b == 0);

                if !is_all_black {
                    log::info!(
                        "✓ Valid VESA framebuffer found: {}x{}x{}bpp",
                        width,
                        height,
                        bpp
                    );
                    return Ok((fb_addr, width, height, bpp as u8, framebuffer_data));
                }
            }
        }

        // 如果所有分辨率都失败,返回第一个全黑的framebuffer
        let (width, height, bpp) = (1024, 768, 32);
        let bytes_per_pixel = bpp / 8;
        let fb_size = width as usize * height as usize * bytes_per_pixel;

        let mut framebuffer_data = vec![0u8; fb_size];
        for i in 0..fb_size {
            let addr = vm_core::GuestAddr(fb_addr + i as u64);
            if let Ok(byte) = mmu.read(addr, 1) {
                framebuffer_data[i] = byte as u8;
            }
        }

        log::info!(
            "VESA framebuffer (may be uninitialized): {}x{}x{}bpp",
            width,
            height,
            bpp
        );
        Ok((fb_addr, width, height, bpp as u8, framebuffer_data))
    }

    /// 保存VESA framebuffer到PPM图像文件
    ///
    /// PPM (Portable Pixel Map) is a simple image format that's easy to generate
    ///
    /// # 参数
    /// - `path`: 输出文件路径 (不带扩展名)
    ///
    /// # 返回
    /// 成功返回保存的文件路径,失败返回错误信息
    pub fn save_vesa_framebuffer(&self, path: &str) -> Result<String, String> {
        let (fb_addr, width, height, bpp, data) = self.get_vesa_framebuffer()?;

        let bytes_per_pixel = (bpp / 8) as usize;
        let output_path = if path.ends_with(".ppm") {
            path.to_string()
        } else {
            format!("{}.ppm", path)
        };

        log::info!("Saving VESA framebuffer to: {}", output_path);
        log::info!(
            "  Framebuffer: {:#010X}, {}x{}x{}bpp, {} bytes",
            fb_addr,
            width,
            height,
            bpp,
            data.len()
        );

        // 生成PPM文件 (P6格式 = binary)
        let mut ppm_content = format!("P6\n{} {}\n255\n", width, height);

        match bpp {
            32 => {
                // RGBA/BGRX format - extract RGB
                for i in 0..(width as usize * height as usize) {
                    let offset = i * 4;
                    if offset + 3 <= data.len() {
                        // BGRX format (little-endian) -> RGB
                        let r = data[offset + 2] as char;
                        let g = data[offset + 1] as char;
                        let b = data[offset] as char;
                        ppm_content.push(r);
                        ppm_content.push(g);
                        ppm_content.push(b);
                    }
                }
            }
            24 => {
                // BGR format -> RGB
                for i in 0..(width as usize * height as usize) {
                    let offset = i * 3;
                    if offset + 3 <= data.len() {
                        let r = data[offset + 2] as char;
                        let g = data[offset + 1] as char;
                        let b = data[offset] as char;
                        ppm_content.push(r);
                        ppm_content.push(g);
                        ppm_content.push(b);
                    }
                }
            }
            16 => {
                // RGB565 format
                for i in 0..(width as usize * height as usize) {
                    let offset = i * 2;
                    if offset + 2 <= data.len() {
                        let pixel = u16::from_le_bytes([data[offset], data[offset + 1]]);
                        let r = ((pixel >> 11) & 0x1F) * 255 / 31;
                        let g = ((pixel >> 5) & 0x3F) * 255 / 63;
                        let b = (pixel & 0x1F) * 255 / 31;
                        ppm_content.push(r as u8 as char);
                        ppm_content.push(g as u8 as char);
                        ppm_content.push(b as u8 as char);
                    }
                }
            }
            _ => {
                return Err(format!("Unsupported bpp: {}", bpp));
            }
        }

        std::fs::write(&output_path, ppm_content)
            .map_err(|e| format!("Failed to write PPM file: {}", e))?;

        log::info!("✓ VESA framebuffer saved to: {}", output_path);
        Ok(output_path)
    }

    // ============================================================
    // Keyboard Input Support
    // ============================================================

    /// Send keyboard input to VM (for installer interaction)
    ///
    /// This allows sending keystrokes to the Ubuntu installer
    ///
    /// # 参数
    /// - `key`: Character to send
    ///
    /// # 返回
    /// 成功返回(),失败返回错误信息
    pub fn send_key(&self, key: char) -> Result<(), String> {
        log::info!("Sending keyboard input: '{}' ({:04X})", key, key as u32);

        // For now, we'll save keystrokes to a file that the boot executor can read
        // This is a simplified approach - full implementation requires direct BIOS access
        let keyboard_file = "/tmp/vm_keyboard_input.txt";

        // Append keystroke to keyboard input file
        if let Ok(mut existing) = std::fs::read_to_string(keyboard_file) {
            existing.push(key);
            std::fs::write(keyboard_file, existing)
                .map_err(|e| format!("Failed to write keyboard input: {}", e))?;
        } else {
            std::fs::write(keyboard_file, key.to_string())
                .map_err(|e| format!("Failed to write keyboard input: {}", e))?;
        }

        log::info!("  Keystroke queued to: {}", keyboard_file);

        Ok(())
    }

    /// Send string as keyboard input
    ///
    /// # 参数
    /// - `text`: String to send
    ///
    /// # 返回
    /// 成功返回(),失败返回错误信息
    pub fn send_string(&self, text: &str) -> Result<(), String> {
        log::info!("Sending keyboard string: '{}'", text);
        for ch in text.chars() {
            self.send_key(ch)?;
        }
        Ok(())
    }

    /// Send special key (Enter, Escape, etc.)
    ///
    /// # 参数
    /// - `key`: Special key name
    ///
    /// # 返回
    /// 成功返回(),失败返回错误信息
    pub fn send_special_key(&self, key: &str) -> Result<(), String> {
        let ch = match key.to_uppercase().as_str() {
            "ENTER" | "RETURN" => '\n',
            "ESC" | "ESCAPE" => '\x1B',
            "TAB" => '\t',
            "SPACE" => ' ',
            "UP" | "DOWN" | "LEFT" | "RIGHT" => {
                // Arrow keys - use escape sequences
                return self.send_string(&match key {
                    "UP" => "\x1B[A",
                    "DOWN" => "\x1B[B",
                    "LEFT" => "\x1B[D",
                    "RIGHT" => "\x1B[C",
                    _ => return Err(format!("Unknown key: {}", key)),
                });
            }
            "F1" => '\x1B',
            _ => {
                // Check for F2-F12
                if key.starts_with('F') {
                    return Err(format!("Function key {} not implemented", key));
                }
                return Err(format!("Unknown special key: {}", key));
            }
        };

        self.send_key(ch)
    }

    /// Convert character to keyboard scancode
    fn char_to_scancode(ch: char) -> u8 {
        match ch {
            'a'..='z' => (ch as u8) - b'a' + 0x1E,
            'A'..='Z' => (ch as u8) - b'A' + 0x1E,
            '0'..='9' => (ch as u8) - b'0' + 0x02,
            '\n' | '\r' => 0x1C, // Enter
            '\t' => 0x0F,        // Tab
            ' ' => 0x39,         // Space
            '\x1B' => 0x01,      // Escape
            _ => 0x00,           // Unknown
        }
    }

    // ============================================================
    // AHCI SATA控制器管理
    // ============================================================

    /// 初始化AHCI SATA控制器
    ///
    /// # 返回
    /// 成功返回(),失败返回错误信息
    pub fn init_ahci_controller(&mut self) -> Result<(), String> {
        log::info!("Initializing AHCI SATA controller");

        let mut ahci = vm_device::ahci::AhciController::new();
        ahci.init()
            .map_err(|e| format!("Failed to initialize AHCI controller: {:?}", e))?;

        self.ahci_controller = Some(ahci);

        log::info!("AHCI controller initialized successfully");
        Ok(())
    }

    /// 附加磁盘到AHCI端口
    ///
    /// # 参数
    /// - `port_num`: 端口号(0-31)
    /// - `disk_path`: 磁盘镜像文件路径
    ///
    /// # 返回
    /// 成功返回(),失败返回错误信息
    pub fn attach_disk_to_ahci(&mut self, port_num: usize, disk_path: &str) -> Result<(), String> {
        use std::path::Path;

        log::info!("Attaching disk {} to AHCI port {}", disk_path, port_num);

        let ahci = self
            .ahci_controller
            .as_mut()
            .ok_or_else(|| "AHCI controller not initialized".to_string())?;

        // 检查磁盘文件是否存在
        let path = Path::new(disk_path);
        if !path.exists() {
            return Err(format!("Disk file not found: {}", disk_path));
        }

        // 创建块设备
        let disk = vm_device::block_device::RawBlockDevice::new(disk_path)
            .map_err(|e| format!("Failed to create block device: {:?}", e))?;

        // 附加到AHCI端口
        ahci.attach_disk(port_num, Box::new(disk))
            .map_err(|e| format!("Failed to attach disk: {:?}", e))?;

        log::info!("Disk attached successfully to port {}", port_num);
        Ok(())
    }

    /// 获取AHCI控制器引用
    ///
    /// # 返回
    /// 成功返回AHCI控制器引用,失败返回错误信息
    pub fn get_ahci_controller(&self) -> Result<&vm_device::ahci::AhciController, String> {
        self.ahci_controller
            .as_ref()
            .ok_or_else(|| "AHCI controller not initialized".to_string())
    }

    /// 获取AHCI控制器可变引用
    ///
    /// # 返回
    /// 成功返回AHCI控制器可变引用,失败返回错误信息
    pub fn get_ahci_controller_mut(
        &mut self,
    ) -> Result<&mut vm_device::ahci::AhciController, String> {
        self.ahci_controller
            .as_mut()
            .ok_or_else(|| "AHCI controller not initialized".to_string())
    }

    // ============================================================
    // ATAPI CD-ROM设备管理
    // ============================================================

    /// 初始化ATAPI CD-ROM设备
    ///
    /// # 参数
    /// - `iso_path`: ISO镜像文件路径
    ///
    /// # 返回
    /// 成功返回(),失败返回错误信息
    pub fn init_atapi_cdrom(&mut self, iso_path: &str) -> Result<(), String> {
        log::info!("Initializing ATAPI CD-ROM with ISO: {}", iso_path);

        let mut cdrom = vm_device::atapi::AtapiCdRom::new(iso_path)
            .map_err(|e| format!("Failed to create ATAPI CD-ROM: {:?}", e))?;

        cdrom
            .init()
            .map_err(|e| format!("Failed to initialize ATAPI CD-ROM: {:?}", e))?;

        self.atapi_cdrom = Some(cdrom);

        log::info!("ATAPI CD-ROM initialized successfully");
        Ok(())
    }

    /// 获取ATAPI CD-ROM引用
    ///
    /// # 返回
    /// 成功返回ATAPI CD-ROM引用,失败返回错误信息
    pub fn get_atapi_cdrom(&self) -> Result<&vm_device::atapi::AtapiCdRom, String> {
        self.atapi_cdrom
            .as_ref()
            .ok_or_else(|| "ATAPI CD-ROM not initialized".to_string())
    }

    /// 获取ATAPI CD-ROM可变引用
    ///
    /// # 返回
    /// 成功返回ATAPI CD-ROM可变引用,失败返回错误信息
    pub fn get_atapi_cdrom_mut(&mut self) -> Result<&mut vm_device::atapi::AtapiCdRom, String> {
        self.atapi_cdrom
            .as_mut()
            .ok_or_else(|| "ATAPI CD-ROM not initialized".to_string())
    }

    // ============================================================
    // 设备状态信息
    // ============================================================

    /// 获取存储设备状态信息
    ///
    /// # 返回
    /// 返回AHCI和ATAPI设备的状态信息
    pub fn get_storage_devices_info(&self) -> StorageDevicesInfo {
        let ahci_info = self.ahci_controller.as_ref().map(|ahci| {
            format!(
                "AHCI Controller: {} ports implemented",
                ahci.ports_implemented()
            )
        });

        let atapi_info = self.atapi_cdrom.as_ref().map(|cdrom| {
            format!(
                "ATAPI CD-ROM: {} sectors ({} MB)",
                cdrom.capacity(),
                cdrom.size() / (1024 * 1024)
            )
        });

        StorageDevicesInfo {
            ahci_controller: ahci_info.unwrap_or_else(|| "Not initialized".to_string()),
            atapi_cdrom: atapi_info.unwrap_or_else(|| "Not initialized".to_string()),
        }
    }
}

/// 存储设备信息
#[derive(Debug, Clone)]
pub struct StorageDevicesInfo {
    /// AHCI控制器信息
    pub ahci_controller: String,
    /// ATAPI CD-ROM信息
    pub atapi_cdrom: String,
}

/// ISO镜像信息
#[derive(Debug, Clone)]
pub struct IsoInfo {
    /// ISO文件路径
    pub path: String,
    /// ISO文件大小(字节)
    pub size_bytes: u64,
    /// ISO文件大小(MB)
    pub size_mb: u64,
}
