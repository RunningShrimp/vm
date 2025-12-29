//! 设备服务层
//!
//! 实现统一的设备控制服务，处理所有设备的业务逻辑。
//! 符合DDD贫血模型原则，将设备控制逻辑从设备结构移至服务层。

use parking_lot::Mutex;
use std::sync::{Arc, Mutex as StdMutex};
use vm_core::{GuestAddr, MMU, VmConfig, VmError, VmResult};

use vm_device::block_service::BlockDeviceService;
use vm_device::clint::{Clint, ClintMmio};
use vm_device::gpu_virt::GpuManager;
use vm_device::plic::{Plic, PlicMmio};
use vm_device::virtio_ai::{VirtioAi, VirtioAiMmio};

/// 设备服务
///
/// 负责处理所有设备的业务逻辑，包括：
/// - 设备初始化
/// - 设备I/O处理
/// - 设备状态管理
/// - 设备中断处理
pub struct DeviceService {
    /// MMU引用（用于设备I/O）
    mmu: Option<Arc<StdMutex<Box<dyn MMU>>>>,
    /// GPU管理器
    gpu_manager: Option<GpuManager>,
    /// CLINT设备
    clint: Option<Arc<Mutex<Clint>>>,
    /// PLIC设备
    plic: Option<Arc<Mutex<Plic>>>,
    /// Block设备服务（处理VirtIO Block设备的业务逻辑）
    block_service: Option<BlockDeviceService>,
    /// 设备轮询任务句柄（用于异步轮询）
    poll_task_handle: Option<tokio::task::JoinHandle<()>>,
}

impl DeviceService {
    /// 创建新的设备服务
    pub fn new() -> Self {
        Self {
            mmu: None,
            gpu_manager: None,
            clint: None,
            plic: None,
            block_service: None,
            poll_task_handle: None,
        }
    }

    /// 初始化GPU后端
    pub fn init_gpu(&mut self, gpu_backend: Option<String>) -> VmResult<()> {
        let mut gpu_manager = GpuManager::new();

        if let Some(backend_name) = &gpu_backend {
            gpu_manager
                .select_backend_by_name(backend_name)
                .map_err(|e| {
                    VmError::Device(vm_core::DeviceError::InitFailed {
                        device_type: "GPU".to_string(),
                        message: format!("GPU backend select failed: {}", e),
                    })
                })?;
        } else {
            gpu_manager.auto_select_backend();
        }

        // GPU初始化需要MMU，所以延迟到map_devices时初始化
        self.gpu_manager = Some(gpu_manager);
        Ok(())
    }

    /// 初始化所有设备
    ///
    /// 根据配置初始化CLINT、PLIC、VirtIO等设备
    pub async fn initialize_devices(
        &mut self,
        config: &VmConfig,
        mmu: Arc<StdMutex<Box<dyn MMU>>>,
    ) -> VmResult<()> {
        self.mmu = Some(Arc::clone(&mmu));

        // 初始化CLINT (Clock Interrupt)
        let clint = Arc::new(Mutex::new(Clint::new(config.vcpu_count, 10_000_000))); // 10MHz
        self.clint = Some(Arc::clone(&clint));

        // 初始化PLIC (Platform Level Interrupt Controller)
        let plic = Arc::new(Mutex::new(Plic::new(127, config.vcpu_count * 2)));
        self.plic = Some(Arc::clone(&plic));

        // 初始化GPU后端（如果已设置）
        if let Some(ref mut gpu_manager) = self.gpu_manager {
            let _mmu_guard = mmu.lock().map_err(|e| {
                VmError::Device(vm_core::DeviceError::InitFailed {
                    device_type: "MMU".to_string(),
                    message: format!("MMU lock is poisoned: {:?}", e),
                })
            })?;
            gpu_manager.init_selected_backend().map_err(|e| {
                VmError::Device(vm_core::DeviceError::InitFailed {
                    device_type: "GPU".to_string(),
                    message: format!("GPU init failed: {:?}", e),
                })
            })?;
        }

        // 初始化Block设备服务
        // 使用默认配置：1GB磁盘容量，512字节扇区大小，可读写
        // 由于VmConfig中没有disk_gb字段，我们使用固定值：1GB = 2097152扇区（假设512字节扇区）
        let disk_capacity_sectors = 2097152u64; // 1GB / 512 bytes per sector
        let block_service = BlockDeviceService::new(disk_capacity_sectors, 512, false);
        self.block_service = Some(block_service);

        Ok(())
    }

    /// 映射设备到MMIO地址空间
    ///
    /// 将设备注册到MMU的MMIO映射中
    pub async fn map_devices(&self) -> VmResult<()> {
        let mmu = self
            .mmu
            .as_ref()
            .ok_or_else(|| {
                VmError::Device(vm_core::DeviceError::InitFailed {
                    device_type: "MMU".to_string(),
                    message: "MMU not set".to_string(),
                })
            })?
            .clone();

        let clint = self
            .clint
            .as_ref()
            .ok_or_else(|| {
                VmError::Device(vm_core::DeviceError::InitFailed {
                    device_type: "CLINT".to_string(),
                    message: "CLINT not initialized".to_string(),
                })
            })?
            .clone();

        let plic = self
            .plic
            .as_ref()
            .ok_or_else(|| {
                VmError::Device(vm_core::DeviceError::InitFailed {
                    device_type: "PLIC".to_string(),
                    message: "PLIC not initialized".to_string(),
                })
            })?
            .clone();

        let mmu_guard = mmu.lock().map_err(|e| {
            VmError::Device(vm_core::DeviceError::InitFailed {
                device_type: "MMU".to_string(),
                message: format!("MMU lock is poisoned: {:?}", e),
            })
        })?;

        // CLINT @ 0x0200_0000 (16KB)
        let clint_mmio = ClintMmio::new(Arc::clone(&clint));
        mmu_guard.map_mmio(
            vm_core::GuestAddr(0x0200_0000),
            0x10000,
            Box::new(clint_mmio),
        );

        // PLIC @ 0x0C00_0000 (64MB)
        let plic_mmio = PlicMmio::new(Arc::clone(&plic));
        plic_mmio.set_virtio_queue_source_base(32);
        mmu_guard.map_mmio(
            vm_core::GuestAddr(0x0C00_0000),
            0x4000000,
            Box::new(plic_mmio),
        );

        // VirtIO Block 设备暂不映射到 MMU（需完整实现 MmioDevice）
        // VirtIO AI @ 0x1000_1000 (4KB)
        let virtio_ai = VirtioAiMmio::new(VirtioAi::new());
        mmu_guard.map_mmio(vm_core::GuestAddr(0x1000_1000), 0x1000, Box::new(virtio_ai));

        Ok(())
    }

    /// 启动设备轮询任务
    ///
    /// 创建异步任务定期轮询所有设备
    pub fn start_polling(&mut self) -> VmResult<()> {
        let mmu = self
            .mmu
            .as_ref()
            .ok_or_else(|| {
                VmError::Device(vm_core::DeviceError::InitFailed {
                    device_type: "MMU".to_string(),
                    message: "MMU not set".to_string(),
                })
            })?
            .clone();

        // 如果已有轮询任务，先取消它
        if let Some(handle) = self.poll_task_handle.take() {
            handle.abort();
        }

        // 创建新的轮询任务
        let handle = tokio::spawn(async move {
            use tokio::time::{Duration, sleep};
            loop {
                sleep(Duration::from_millis(10)).await;
                if let Ok(mmu_guard) = mmu.lock() {
                    mmu_guard.poll_devices();
                }
            }
        });

        self.poll_task_handle = Some(handle);
        Ok(())
    }

    /// 停止设备轮询任务
    pub fn stop_polling(&mut self) {
        if let Some(handle) = self.poll_task_handle.take() {
            handle.abort();
        }
    }

    /// 设置MMU引用
    pub fn set_mmu(&mut self, mmu: Arc<StdMutex<Box<dyn MMU>>>) {
        self.mmu = Some(mmu);
    }

    /// 获取CLINT设备引用
    pub fn clint(&self) -> Option<Arc<Mutex<Clint>>> {
        self.clint.as_ref().map(Arc::clone)
    }

    /// 获取PLIC设备引用
    pub fn plic(&self) -> Option<Arc<Mutex<Plic>>> {
        self.plic.as_ref().map(Arc::clone)
    }

    /// 处理设备I/O请求
    ///
    /// 根据设备类型和地址，将请求路由到相应的设备处理逻辑
    pub fn process_io(&self, addr: GuestAddr) -> VmResult<u64> {
        // 根据MMIO地址范围路由到相应设备
        // 实际I/O处理由MMU中的MMIO设备处理
        // 这里主要用于路由和日志记录

        if addr >= vm_core::GuestAddr(0x1000_0000) && addr < vm_core::GuestAddr(0x1000_2000) {
            // VirtIO Block设备I/O（由MMU中的设备处理）
            Ok(0)
        } else if addr >= vm_core::GuestAddr(0x0200_0000) && addr < vm_core::GuestAddr(0x0200_1000)
        {
            // CLINT设备I/O（由MMU中的设备处理）
            Ok(0)
        } else if addr >= vm_core::GuestAddr(0x0C00_0000) && addr < vm_core::GuestAddr(0x1000_0000)
        {
            // PLIC设备I/O（由MMU中的设备处理）
            Ok(0)
        } else {
            Err(VmError::Memory(vm_core::MemoryError::InvalidAddress(addr)))
        }
    }

    /// 处理设备中断
    ///
    /// 根据中断源，路由到相应的设备中断处理逻辑
    pub fn handle_interrupt(&self, irq: u32) -> VmResult<()> {
        // 根据IRQ号路由到相应设备
        // 实际中断处理由PLIC和相应设备处理
        // 这里主要用于日志记录和统计

        if (32..48).contains(&irq) {
            // VirtIO Block设备中断
            Ok(())
        } else if (48..64).contains(&irq) {
            // VirtIO AI设备中断
            Ok(())
        } else {
            // 其他中断
            Ok(())
        }
    }

    /// 轮询所有设备
    ///
    /// 定期调用以处理设备的异步操作
    pub fn poll_devices(&self) -> VmResult<()> {
        if let Some(mmu) = &self.mmu {
            let mmu_guard = mmu.lock().map_err(|e| {
                VmError::Device(vm_core::DeviceError::InitFailed {
                    device_type: "MMU".to_string(),
                    message: format!("MMU lock is poisoned: {:?}", e),
                })
            })?;
            mmu_guard.poll_devices();
        }

        // 轮询Block设备
        if self.block_service.is_some() {
            // 在这里可以添加Block设备的轮询逻辑
            // 目前只是占位符实现
        }

        Ok(())
    }

    /// 获取Block设备服务引用
    pub fn block_service(&self) -> Option<&BlockDeviceService> {
        self.block_service.as_ref()
    }

    /// 配置Block设备
    ///
    /// 根据指定的路径和只读标志重新配置块设备。
    /// 注意：这个方法会创建一个新的BlockDeviceService实例并替换当前的实例。
    ///
    /// # 参数
    /// - `_path`: 块设备文件的路径（当前未使用，保留用于未来实现）
    /// - `_readonly`: 是否以只读模式打开设备（当前未使用，保留用于未来实现）
    ///
    /// # 返回值
    /// - `VmResult<()>`: 成功时返回Ok(())，失败时返回错误
    ///
    /// # 错误处理
    /// - 如果设备文件不存在或无法访问，返回VmError
    /// - 如果设备已经在使用中，返回错误
    ///
    /// # 示例
    /// ```ignore
    /// device_service.configure_block_device("/path/to/disk.img", false).await?;
    /// ```
    ///
    /// # 注意
    /// 此方法当前已废弃。建议使用BlockDeviceService::open()直接在初始化时创建设备服务。
    pub async fn configure_block_device(&self, _path: &str, _readonly: bool) -> VmResult<()> {
        // 由于当前方法需要修改self.block_service，但self是不可变引用
        // 这是一个API设计问题。我们提供以下解决方案：
        //
        // 1. 返回一个新的BlockDeviceService，由调用者设置
        // 2. 使用内部可变性（Arc<Mutex<Option<BlockDeviceService>>>）
        // 3. 将这个方法标记为已废弃，建议使用initialize_devices
        //
        // 这里我们选择返回一个描述性的错误，引导用户使用正确的API

        Err(VmError::Device(vm_core::DeviceError::InitFailed {
            device_type: "BlockDevice".to_string(),
            message: "configure_block_device is deprecated. Please use BlockDeviceService::open() directly during initialization.".to_string(),
        }))
    }
}

impl Default for DeviceService {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for DeviceService {
    fn drop(&mut self) {
        self.stop_polling();
    }
}
