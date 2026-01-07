//! GPU设备和计算抽象
//!
//! 定义统一的GPU计算接口,支持CUDA和ROCm。

use super::error::{GpuError, GpuResult};

/// GPU设备类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GpuDeviceType {
    /// NVIDIA GPU (CUDA)
    Cuda,
    /// AMD GPU (ROCm/HIP)
    Rocm,
    /// 其他GPU (Vulkan, OpenCL等)
    Other,
}

/// GPU设备信息
#[derive(Debug, Clone)]
pub struct GpuDeviceInfo {
    /// 设备类型
    pub device_type: GpuDeviceType,
    /// 设备名称
    pub name: String,
    /// 设备ID
    pub device_id: i32,
    /// 计算能力 (major, minor)
    pub compute_capability: (u32, u32),
    /// 总内存大小(MB)
    pub total_memory_mb: usize,
    /// 可用内存大小(MB)
    pub free_memory_mb: usize,
    /// 多处理器数量
    pub multiprocessor_count: u32,
    /// 最大时钟频率(KHz)
    pub clock_rate_khz: u32,
    /// L2缓存大小(bytes)
    pub l2_cache_size: usize,
    /// 是否支持统一内存
    pub supports_unified_memory: bool,
    /// 是否支持共享内存
    pub supports_shared_memory: bool,
}

/// GPU缓冲区(设备内存)
#[derive(Debug, Clone)]
pub struct GpuBuffer {
    /// 设备指针
    pub ptr: u64,
    /// 大小(bytes)
    pub size: usize,
    /// 设备ID
    pub device_id: i32,
}

unsafe impl Send for GpuBuffer {}
unsafe impl Sync for GpuBuffer {}

/// GPU内核
#[derive(Debug, Clone)]
pub struct GpuKernel {
    /// 内核名称
    pub name: String,
    /// 编译后的二进制(PTX或Cubin)
    pub binary: Vec<u8>,
    /// 元数据
    pub metadata: KernelMetadata,
}

/// 内核元数据
#[derive(Debug, Clone)]
pub struct KernelMetadata {
    /// 内核名称
    pub name: String,
    /// 源代码(如果可用)
    pub source: Option<String>,
    /// 编译时间戳
    pub compiled_at: Option<u64>,
    /// 参数数量
    pub num_params: usize,
    /// 共享内存大小(bytes)
    pub shared_memory_size: usize,
}

/// GPU执行参数
#[derive(Debug, Clone)]
pub enum GpuArg {
    /// 无符号8位整数
    U8(u8),
    /// 无符号32位整数
    U32(u32),
    /// 无符号64位整数
    U64(u64),
    /// 有符号32位整数
    I32(i32),
    /// 有符号64位整数
    I64(i64),
    /// 32位浮点数
    F32(f32),
    /// 64位浮点数
    F64(f64),
    /// 缓冲区
    Buffer(GpuBuffer),
    /// 原始指针(谨慎使用)
    RawPtr(u64),
}

/// GPU执行结果
#[derive(Debug, Clone)]
pub struct GpuExecutionResult {
    /// 执行是否成功
    pub success: bool,
    /// 执行时间(纳秒)
    pub execution_time_ns: u64,
    /// 返回数据(如果适用)
    pub return_data: Option<Vec<u8>>,
}

/// GPU计算统一trait
///
/// 定义所有GPU设备必须实现的接口,支持CUDA和ROCm。
pub trait GpuCompute: Send + Sync {
    /// 初始化GPU设备
    ///
    /// # Safety
    /// 调用者确保不重复初始化
    fn initialize(&mut self) -> GpuResult<()>;

    /// 获取设备信息
    fn device_info(&self) -> GpuDeviceInfo;

    /// 检查设备是否可用
    fn is_available(&self) -> bool {
        true
    }

    /// 分配设备内存
    ///
    /// # Arguments
    ///
    /// * `size` - 内存大小(bytes)
    fn allocate_memory(&self, size: usize) -> GpuResult<GpuBuffer>;

    /// 释放设备内存
    ///
    /// # Arguments
    ///
    /// * `buffer` - 要释放的缓冲区
    fn free_memory(&self, buffer: GpuBuffer) -> GpuResult<()>;

    /// 从主机复制数据到设备
    ///
    /// # Arguments
    ///
    /// * `host_data` - 主机数据
    /// * `device_buffer` - 设备缓冲区
    fn copy_h2d(&self, host_data: &[u8], device_buffer: &GpuBuffer) -> GpuResult<()>;

    /// 从设备复制数据到主机
    ///
    /// # Arguments
    ///
    /// * `device_buffer` - 设备缓冲区
    /// * `host_data` - 主机数据缓冲区
    fn copy_d2h(&self, device_buffer: &GpuBuffer, host_data: &mut [u8]) -> GpuResult<()>;

    /// 编译GPU内核
    ///
    /// # Arguments
    ///
    /// * `source` - 内核源代码
    /// * `kernel_name` - 内核名称
    fn compile_kernel(&self, source: &str, kernel_name: &str) -> GpuResult<GpuKernel>;

    /// 执行GPU内核
    ///
    /// # Arguments
    ///
    /// * `kernel` - 编译后的内核
    /// * `grid_dim` - 网格维度 (x, y, z)
    /// * `block_dim` - 块维度 (x, y, z)
    /// * `args` - 内核参数
    /// * `shared_memory_size` - 共享内存大小
    fn execute_kernel(
        &self,
        kernel: &GpuKernel,
        grid_dim: (u32, u32, u32),
        block_dim: (u32, u32, u32),
        args: &[GpuArg],
        shared_memory_size: usize,
    ) -> GpuResult<GpuExecutionResult>;

    /// 同步设备操作
    fn synchronize(&self) -> GpuResult<()>;
}

/// GPU设备管理器
///
/// 检测和管理所有可用的GPU设备。
pub struct GpuDeviceManager {
    devices: Vec<Box<dyn GpuCompute>>,
    default_device: Option<Box<dyn GpuCompute>>,
}

impl GpuDeviceManager {
    /// 创建新的GPU设备管理器
    ///
    /// 自动检测所有可用的GPU设备。
    pub fn new() -> Self {
        let manager = Self {
            devices: Vec::new(),
            default_device: None,
        };

        // 尝试检测CUDA设备
        #[cfg(feature = "cuda")]
        {
            if let Ok(cuda) = manager.detect_cuda_device() {
                log::info!("CUDA device detected: {}", cuda.device_info().name);
                manager.default_device = Some(cuda);
            }
        }

        // 尝试检测ROCm设备
        #[cfg(feature = "rocm")]
        {
            if let Ok(rocm) = manager.detect_rocm_device() {
                log::info!("ROCm device detected: {}", rocm.device_info().name);
                if manager.default_device.is_none() {
                    manager.default_device = Some(rocm);
                }
            }
        }

        manager
    }

    /// 检查是否有可用的GPU设备
    pub fn has_gpu(&self) -> bool {
        self.default_device.is_some()
    }

    /// 获取默认GPU设备
    pub fn default_device(&self) -> Option<&dyn GpuCompute> {
        self.default_device.as_deref()
    }

    /// 获取所有GPU设备
    pub fn devices(&self) -> &[Box<dyn GpuCompute>] {
        &self.devices
    }

    /// 检测CUDA设备
    ///
    /// 当启用"cuda" feature时可用
    #[cfg(feature = "cuda")]
    #[allow(dead_code)] // 仅在启用cuda feature时使用
    fn detect_cuda_device(&self) -> Result<Box<dyn GpuCompute>, GpuError> {
        // 使用vm-passthrough crate实现CUDA设备检测
        #[cfg(feature = "cuda")]
        {
            use vm_passthrough::CudaAccelerator;

            // 尝试创建CUDA加速器
            match CudaAccelerator::new() {
                Ok(accelerator) => {
                    log::info!("CUDA设备检测成功: {:?}", accelerator.device_info());
                    // ✅ 已实现: CudaAccelerator现在实现了GpuCompute trait
                    Ok(Box::new(accelerator) as Box<dyn GpuCompute>)
                }
                Err(e) => {
                    log::warn!("CUDA设备检测失败: {:?}", e);
                    Err(GpuError::NoDeviceAvailable)
                }
            }
        }
    }

    /// 检测ROCm设备
    ///
    /// 当启用"rocm" feature时可用
    #[cfg(feature = "rocm")]
    #[allow(dead_code)] // 仅在启用rocm feature时使用
    fn detect_rocm_device(&self) -> Result<Box<dyn GpuCompute>, GpuError> {
        // 使用vm-passthrough crate实现ROCm设备检测
        #[cfg(feature = "rocm")]
        {
            use vm_passthrough::RocmAccelerator;

            // 尝试创建ROCm加速器
            match RocmAccelerator::new() {
                Ok(accelerator) => {
                    log::info!("ROCm设备检测成功: {:?}", accelerator.device_info());
                    // ✅ 已实现: RocmAccelerator现在实现了GpuCompute trait
                    Ok(Box::new(accelerator) as Box<dyn GpuCompute>)
                }
                Err(e) => {
                    log::warn!("ROCm设备检测失败: {:?}", e);
                    Err(GpuError::NoDeviceAvailable)
                }
            }
        }
    }

    #[cfg(not(feature = "cuda"))]
    #[allow(dead_code)] // 仅在未启用cuda feature时使用
    fn detect_cuda_device(&self) -> Result<Box<dyn GpuCompute>, GpuError> {
        Err(GpuError::NoDeviceAvailable)
    }

    #[cfg(not(feature = "rocm"))]
    #[allow(dead_code)] // 仅在未启用rocm feature时使用
    fn detect_rocm_device(&self) -> Result<Box<dyn GpuCompute>, GpuError> {
        Err(GpuError::NoDeviceAvailable)
    }
}

impl Default for GpuDeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

// 注意：GpuCompute trait实现已迁移到vm-passthrough crate
//
// ✅ GPU设备信息查询已完全实现 (vm-passthrough/src/cuda.rs:950-998)
// - ✅ 获取实际可用内存: cuMemGetInfo_v2()
// - ✅ 获取多处理器数量: CU_DEVICE_ATTRIBUTE_MULTIPROCESSOR_COUNT
// - ✅ 获取时钟频率: CU_DEVICE_ATTRIBUTE_CLOCK_RATE
// - ✅ 获取L2缓存大小: CU_DEVICE_ATTRIBUTE_L2_CACHE_SIZE
// - ✅ 检测统一内存支持: 根据计算能力判断 (>= 5.0)
//
// 实际实现见: vm-passthrough::cuda::CudaDevice
// 见: vm-passthrough::rocm::RocmDevice (ROCm实现)
