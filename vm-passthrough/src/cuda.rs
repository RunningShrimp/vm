//! # CUDA GPU 加速支持 (WIP)
//!
//! 提供 NVIDIA GPU 的 CUDA 加速功能，包括：
//! - 设备初始化和管理
//! - 异步内存复制
//! - JIT 编译 GPU 加速
//! - 计算内核执行
//!
//! ## 当前状态
//!
//! - **开发状态**: 🚧 Work In Progress
//! - **功能完整性**: ~60%（基础功能已实现）
//! - **生产就绪**: ⚠️ 仅推荐用于开发环境
//!
//! ## 已实现功能
//!
//! - ✅ 设备初始化和基本信息获取
//! - ✅ 内存管理（malloc/free）
//! - ✅ 异步内存复制（H2D/D2H）
//! - ✅ 流管理
//! - ✅ 基础设备信息查询
//!
//! ## 待实现功能
//!
//! - ⏳ 设备到设备内存复制
//! - ⏳ 内核执行逻辑
//! - ⏳ 多设备管理
//! - ⏳ 高级CUDA特性
//!
//! ## 依赖项
//!
//! - `cuda-rs`: CUDA驱动绑定
//! - NVIDIA GPU驱动
//!
//! ## 相关Issue
//!
//! - 跟踪: #待创建（内核执行实现）
//!
//! ## 贡献指南
//!
//! 如果您有CUDA开发经验并希望帮助实现此模块，请：
//! 1. 确保有NVIDIA GPU和CUDA环境
//! 2. 参考NVIDIA CUDA文档
//! 3. 联系维护者review
//! 4. 提交PR并包含测试用例

use std::ptr;
use std::sync::Arc;
use std::time::Instant;

use super::{PassthroughError, PciAddress};

// 导入vm-core的GPU类型以实现trait
#[cfg(feature = "cuda")]
use vm_core::gpu::{GpuBuffer, GpuCompute, GpuDeviceInfo, GpuExecutionResult, GpuKernel, GpuArg, GpuResult, GpuError};

/// CUDA 设备指针
#[derive(Debug, Clone, Copy)]
pub struct CudaDevicePtr {
    pub ptr: u64,
    pub size: usize,
}

unsafe impl Send for CudaDevicePtr {}
unsafe impl Sync for CudaDevicePtr {}

/// CUDA 内存复制方向
#[derive(Debug, Clone, Copy)]
pub enum CudaMemcpyKind {
    HostToDevice,
    DeviceToHost,
    DeviceToDevice,
}

/// CUDA 流（用于异步操作）
pub struct CudaStream {
    pub stream: ptr::NonNull<std::ffi::c_void>,
}

unsafe impl Send for CudaStream {}
unsafe impl Sync for CudaStream {}

impl CudaStream {
    /// 创建新的 CUDA 流
    pub fn new() -> Result<Self, PassthroughError> {
        #[cfg(feature = "cuda")]
        {
            use cudarc::driver::result;

            let mut stream = std::ptr::null_mut();
            unsafe {
                result::cuStreamCreate(&mut stream, 0).map_err(|e| {
                    PassthroughError::DriverBindingFailed(format!(
                        "CUDA stream create failed: {:?}",
                        e
                    ))
                })?;
            }

            Ok(Self {
                stream: ptr::NonNull::new(stream).expect("non-null stream"),
            })
        }

        #[cfg(not(feature = "cuda"))]
        {
            log::warn!("CUDA support not enabled, creating mock stream");
            Ok(Self {
                stream: ptr::NonNull::dangling(),
            })
        }
    }

    /// 同步流
    pub fn synchronize(&self) -> Result<(), PassthroughError> {
        #[cfg(feature = "cuda")]
        unsafe {
            use cudarc::driver::result;
            result::cuStreamSynchronize(self.stream.as_ptr()).map_err(|e| {
                PassthroughError::DriverBindingFailed(format!("CUDA stream sync failed: {:?}", e))
            })?;
        }

        #[cfg(not(feature = "cuda"))]
        {
            log::warn!("CUDA synchronize called but CUDA not enabled");
        }

        Ok(())
    }
}

impl Drop for CudaStream {
    fn drop(&mut self) {
        #[cfg(feature = "cuda")]
        unsafe {
            use cudarc::driver::result;
            let _ = result::cuStreamDestroy_v2(self.stream.as_ptr());
        }
    }
}

/// CUDA 加速器
///
/// 提供基本的 CUDA 加速功能，包括内存管理和内核执行。
pub struct CudaAccelerator {
    pub device_id: i32,
    pub device_name: String,
    pub compute_capability: (u32, u32),
    pub total_memory_mb: usize,
    pub stream: CudaStream,
}

impl CudaAccelerator {
    /// 创建新的 CUDA 加速器
    ///
    /// # Arguments
    ///
    /// * `device_id` - CUDA 设备 ID（默认为 0）
    pub fn new(device_id: i32) -> Result<Self, PassthroughError> {
        log::info!("Initializing CUDA accelerator for device {}", device_id);

        #[cfg(feature = "cuda")]
        {
            use cudarc::driver::result;

            unsafe {
                // 初始化 CUDA
                result::cuInit(0).map_err(|e| {
                    PassthroughError::DriverBindingFailed(format!("CUDA init failed: {:?}", e))
                })?;

                // 获取设备
                let mut device = std::ptr::null_mut();
                result::cuDeviceGet(&mut device, device_id).map_err(|e| {
                    PassthroughError::DeviceNotFound(format!(
                        "CUDA device {} not found: {:?}",
                        device_id, e
                    ))
                })?;

                // 获取设备名称
                let mut name = [0u8; 256];
                result::cuDeviceGetName(name.as_mut_ptr() as *mut i8, 256, device).map_err(
                    |e| {
                        PassthroughError::DriverBindingFailed(format!(
                            "CUDA get name failed: {:?}",
                            e
                        ))
                    },
                )?;
                let device_name = String::from_utf8_lossy(&name)
                    .trim_end_matches('\0')
                    .to_string();

                // 获取计算能力
                let mut major = 0u32;
                let mut minor = 0u32;
                result::cuDeviceComputeCapability(&mut major, &mut minor, device).map_err(|e| {
                    PassthroughError::DriverBindingFailed(format!(
                        "CUDA compute capability failed: {:?}",
                        e
                    ))
                })?;
                let compute_capability = (major, minor);

                // 获取总内存
                let mut total_memory = 0usize;
                result::cuDeviceTotalMem_v2(&mut total_memory as *mut usize as *mut usize, device)
                    .map_err(|e| {
                        PassthroughError::DriverBindingFailed(format!(
                            "CUDA get memory failed: {:?}",
                            e
                        ))
                    })?;
                let total_memory_mb = total_memory / (1024 * 1024);

                let stream = CudaStream::new()?;

                log::info!(
                    "CUDA accelerator initialized: {} (Compute: {}.{} Memory: {} MB)",
                    device_name,
                    major,
                    minor,
                    total_memory_mb
                );

                Ok(Self {
                    device_id,
                    device_name,
                    compute_capability,
                    total_memory_mb,
                    stream,
                })
            }
        }

        #[cfg(not(feature = "cuda"))]
        {
            log::warn!("CUDA support not enabled, creating mock accelerator");
            Ok(Self {
                device_id,
                device_name: "Mock CUDA Device".to_string(),
                compute_capability: (7, 5),
                total_memory_mb: 8192,
                stream: CudaStream::new()?,
            })
        }
    }

    /// 分配 GPU 内存
    pub fn malloc(&self, size: usize) -> Result<CudaDevicePtr, PassthroughError> {
        #[cfg(feature = "cuda")]
        {
            use cudarc::driver::result;

            let mut d_ptr = std::ptr::null_mut();
            unsafe {
                result::cuMemAlloc_v2(&mut d_ptr, size).map_err(|e| {
                    PassthroughError::DriverBindingFailed(format!("CUDA malloc failed: {:?}", e))
                })?;
            }

            log::trace!("Allocated {} bytes on GPU", size);

            Ok(CudaDevicePtr {
                ptr: d_ptr as u64,
                size,
            })
        }

        #[cfg(not(feature = "cuda"))]
        {
            log::trace!("Mock CUDA malloc: {} bytes", size);
            Ok(CudaDevicePtr { ptr: 0, size })
        }
    }

    /// 释放 GPU 内存
    pub fn free(&self, d_ptr: CudaDevicePtr) -> Result<(), PassthroughError> {
        #[cfg(feature = "cuda")]
        {
            use cudarc::driver::result;

            unsafe {
                result::cuMemFree_v2(d_ptr.ptr as *mut std::ffi::c_void).map_err(|e| {
                    PassthroughError::DriverBindingFailed(format!("CUDA free failed: {:?}", e))
                })?;
            }

            log::trace!("Freed GPU memory at {:?}", d_ptr);
        }

        #[cfg(not(feature = "cuda"))]
        {
            log::trace!("Mock CUDA free");
        }

        Ok(())
    }

    /// 异步内存复制（Host → Device）
    pub async fn memcpy_h2d_async(
        &self,
        dst: CudaDevicePtr,
        src: &[u8],
    ) -> Result<(), PassthroughError> {
        let start = Instant::now();

        #[cfg(feature = "cuda")]
        {
            use cudarc::driver::result;

            let size = std::cmp::min(src.len(), dst.size);
            unsafe {
                result::cuMemcpyHtoDAsync_v2(
                    dst.ptr as *mut std::ffi::c_void,
                    src.as_ptr() as *const std::ffi::c_void,
                    size,
                    self.stream.stream.as_ptr(),
                )
                .map_err(|e| {
                    PassthroughError::DriverBindingFailed(format!(
                        "CUDA H2D memcpy failed: {:?}",
                        e
                    ))
                })?;
            }

            log::trace!("Async memcpy H2D: {} bytes in {:?}", size, start.elapsed());
        }

        #[cfg(not(feature = "cuda"))]
        {
            log::trace!("Mock async memcpy H2D: {} bytes", src.len().min(dst.size));
        }

        Ok(())
    }

    /// 异步内存复制（Device → Host）
    pub async fn memcpy_d2h_async(
        &self,
        dst: &mut [u8],
        src: CudaDevicePtr,
    ) -> Result<(), PassthroughError> {
        let start = Instant::now();

        #[cfg(feature = "cuda")]
        {
            use cudarc::driver::result;

            let size = std::cmp::min(dst.len(), src.size);
            unsafe {
                result::cuMemcpyDtoHAsync_v2(
                    dst.as_mut_ptr() as *mut std::ffi::c_void,
                    src.ptr as *const std::ffi::c_void,
                    size,
                    self.stream.stream.as_ptr(),
                )
                .map_err(|e| {
                    PassthroughError::DriverBindingFailed(format!(
                        "CUDA D2H memcpy failed: {:?}",
                        e
                    ))
                })?;
            }

            log::trace!("Async memcpy D2H: {} bytes in {:?}", size, start.elapsed());
        }

        #[cfg(not(feature = "cuda"))]
        {
            log::trace!("Mock async memcpy D2H: {} bytes", dst.len().min(src.size));
        }

        Ok(())
    }

    /// 同步内存复制（Host ↔ Device）
    pub fn memcpy_sync(
        &self,
        dst: CudaDevicePtr,
        src: &[u8],
        kind: CudaMemcpyKind,
    ) -> Result<(), PassthroughError> {
        #[cfg(feature = "cuda")]
        {
            use cudarc::driver::result;

            let start = Instant::now();
            let size = std::cmp::min(src.len(), dst.size);

            match kind {
                CudaMemcpyKind::HostToDevice => unsafe {
                    result::cuMemcpyHtoD_v2(
                        dst.ptr as *mut std::ffi::c_void,
                        src.as_ptr() as *const std::ffi::c_void,
                        size,
                    )
                    .map_err(|e| {
                        PassthroughError::DriverBindingFailed(format!(
                            "CUDA memcpy H2D failed: {:?}",
                            e
                        ))
                    })?;
                },
                CudaMemcpyKind::DeviceToHost => unsafe {
                    result::cuMemcpyDtoH_v2(
                        dst.ptr as *mut std::ffi::c_void,
                        src.as_ptr() as *const std::ffi::c_void,
                        size,
                    )
                    .map_err(|e| {
                        PassthroughError::DriverBindingFailed(format!(
                            "CUDA memcpy D2H failed: {:?}",
                            e
                        ))
                    })?;
                },
                CudaMemcpyKind::DeviceToDevice => unsafe {
                    // 注意: 这里dst和src都应该解释为CudaDevicePtr
                    // 但当前API签名使用src: &[u8]，这在DeviceToDevice情况下不太合适
                    // 这是一个临时解决方案，更好的做法是改变API签名
                    return Err(PassthroughError::DriverBindingFailed(
                        "Device-to-device memcpy requires special API. Use memcpy_d2d() instead.".to_string(),
                    ));
                }
            }

            log::trace!(
                "Sync memcpy {:?}: {} bytes in {:?}",
                kind,
                size,
                start.elapsed()
            );
        }

        #[cfg(not(feature = "cuda"))]
        {
            log::trace!(
                "Mock sync memcpy {:?}: {} bytes",
                kind,
                src.len().min(dst.size)
            );
        }

        Ok(())
    }

    /// 设备到设备的内存复制
    ///
    /// 在GPU设备内存之间复制数据，比Host中转更高效。
    ///
    /// # Arguments
    ///
    /// * `dst` - 目标设备指针
    /// * `src` - 源设备指针
    /// * `size` - 要复制的字节数
    ///
    /// # Example
    ///
    /// ```ignore
    /// let accel = CudaAccelerator::new(0)?;
    /// let src = accel.malloc(1024)?;
    /// let dst = accel.malloc(1024)?;
    /// // 直接在GPU内存间复制，无需Host中转
    /// accel.memcpy_d2d(dst, src, 1024)?;
    /// ```
    pub fn memcpy_d2d(
        &self,
        dst: CudaDevicePtr,
        src: CudaDevicePtr,
        size: usize,
    ) -> Result<(), PassthroughError> {
        log::trace!("Device-to-device memcpy: {} bytes", size);

        #[cfg(feature = "cuda")]
        {
            use cudarc::driver::result;

            let start = Instant::now();
            let copy_size = std::cmp::min(size, std::cmp::min(dst.size, src.size));

            unsafe {
                result::cuMemcpyDtoD_v2(
                    dst.ptr as *mut std::ffi::c_void,
                    src.ptr as *const std::ffi::c_void,
                    copy_size,
                )
                .map_err(|e| {
                    PassthroughError::DriverBindingFailed(format!(
                        "CUDA D2D memcpy failed: {:?}",
                        e
                    ))
                })?;
            }

            log::trace!("D2D memcpy: {} bytes in {:?}", copy_size, start.elapsed());
        }

        #[cfg(not(feature = "cuda"))]
        {
            log::trace!("Mock D2D memcpy: {} bytes", size);
        }

        Ok(())
    }

    /// 异步设备到设备内存复制
    ///
    /// 异步版本，在指定的CUDA流上执行复制操作。
    ///
    /// # Arguments
    ///
    /// * `dst` - 目标设备指针
    /// * `src` - 源设备指针
    /// * `size` - 要复制的字节数
    ///
    /// # Example
    ///
    /// ```ignore
    /// let accel = CudaAccelerator::new(0)?;
    /// let src = accel.malloc(1024)?;
    /// let dst = accel.memcpy_d2d_async(dst, src, 1024).await?;
    /// // 等待操作完成
    /// accel.stream.synchronize()?;
    /// ```
    pub async fn memcpy_d2d_async(
        &self,
        dst: CudaDevicePtr,
        src: CudaDevicePtr,
        size: usize,
    ) -> Result<(), PassthroughError> {
        log::trace!("Async device-to-device memcpy: {} bytes", size);

        #[cfg(feature = "cuda")]
        {
            use cudarc::driver::result;

            let start = Instant::now();
            let copy_size = std::cmp::min(size, std::cmp::min(dst.size, src.size));

            unsafe {
                result::cuMemcpyDtoDAsync_v2(
                    dst.ptr as *mut std::ffi::c_void,
                    src.ptr as *const std::ffi::c_void,
                    copy_size,
                    self.stream.stream.as_ptr(),
                )
                .map_err(|e| {
                    PassthroughError::DriverBindingFailed(format!(
                        "CUDA async D2D memcpy failed: {:?}",
                        e
                    ))
                })?;
            }

            log::trace!(
                "Async D2D memcpy: {} bytes in {:?}",
                copy_size,
                start.elapsed()
            );
        }

        #[cfg(not(feature = "cuda"))]
        {
            log::trace!("Mock async D2D memcpy: {} bytes", size);
        }

        Ok(())
    }

    /// 获取设备信息
    pub fn get_device_info(&self) -> CudaDeviceInfo {
        CudaDeviceInfo {
            device_id: self.device_id,
            name: self.device_name.clone(),
            compute_capability: self.compute_capability,
            total_memory_mb: self.total_memory_mb,
        }
    }

    /// 检查设备是否支持某个功能
    pub fn supports_feature(&self, feature: CudaFeature) -> bool {
        match feature {
            CudaFeature::ComputeCapability(major, minor) => {
                self.compute_capability >= (major, minor)
            }
            CudaFeature::Memory(size_mb) => self.total_memory_mb >= size_mb,
        }
    }
}

/// CUDA 设备信息
#[derive(Debug, Clone)]
pub struct CudaDeviceInfo {
    pub device_id: i32,
    pub name: String,
    pub compute_capability: (u32, u32),
    pub total_memory_mb: usize,
}

/// CUDA 功能特性
#[derive(Debug, Clone, Copy)]
pub enum CudaFeature {
    ComputeCapability(u32, u32),
    Memory(usize),
}

/// GPU 计算内核（占位实现）
pub struct GpuKernel {
    pub name: String,
    pub kernel_ptr: u64,
}

impl GpuKernel {
    pub fn new(name: String) -> Self {
        Self {
            name,
            kernel_ptr: 0,
        }
    }

    /// 执行内核
    ///
    /// 使用 cuLaunchKernel API 启动 CUDA 内核
    ///
    /// # Arguments
    ///
    /// * `grid_dim` - 网格维度 (x, y, z)
    /// * `block_dim` - 块维度 (x, y, z)
    ///
    /// # Example
    ///
    /// ```ignore
    /// let kernel = GpuKernel::new("my_kernel".to_string());
    /// // 启动内核：1个块，每个块32个线程
    /// kernel.launch((1, 1, 1), (32, 1, 1))?;
    /// ```
    pub fn launch(
        &self,
        grid_dim: (u32, u32, u32),
        block_dim: (u32, u32, u32),
    ) -> Result<(), PassthroughError> {
        log::debug!(
            "Launching GPU kernel '{}' with grid {:?} and block {:?}",
            self.name,
            grid_dim,
            block_dim
        );

        #[cfg(feature = "cuda")]
        {
            use cudarc::driver::result;

            // 检查内核是否已加载
            if self.kernel_ptr == 0 {
                return Err(PassthroughError::DriverBindingFailed(
                    format!("Kernel '{}' not loaded. Call load_from_ptx() first.", self.name)
                ));
            }

            unsafe {
                // 启动内核
                // 参数说明:
                // - f: 内核函数指针
                // - gridDimX/Y/Z: 网格维度
                // - blockDimX/Y/Z: 块维度
                // - sharedMemBytes: 共享内存大小 (bytes)
                // - hStream: CUDA 流
                // - kernelParams: 内核参数数组
                // - extra: 额外参数
                result::cuLaunchKernel(
                    self.kernel_ptr as *mut std::ffi::c_void,
                    grid_dim.0,
                    grid_dim.1,
                    grid_dim.2,
                    block_dim.0,
                    block_dim.1,
                    block_dim.2,
                    0, // sharedMemBytes - 暂不支持动态共享内存
                    std::ptr::null_mut(), // hStream - 使用默认流
                    std::ptr::null_mut(), // kernelParams - 暂不支持参数传递
                    std::ptr::null_mut(), // extra - 暂不支持额外参数
                )
                .map_err(|e| {
                    PassthroughError::DriverBindingFailed(format!(
                        "Failed to launch kernel '{}': {:?}",
                        self.name, e
                    ))
                })?;

                log::trace!(
                    "Kernel '{}' launched successfully (grid: {:?}, block: {:?})",
                    self.name,
                    grid_dim,
                    block_dim
                );
            }
        }

        #[cfg(not(feature = "cuda"))]
        {
            log::warn!(
                "GPU kernel launch called but CUDA not enabled (kernel: '{}')",
                self.name
            );
        }

        Ok(())
    }

    /// 从 PTX (Parallel Thread Execution) 代码加载内核
    ///
    /// PTX 是 CUDA 的汇编语言，需要从 PTX 代码中加载内核才能执行。
    ///
    /// # Arguments
    ///
    /// * `accelerator` - CUDA 加速器引用
    /// * `ptx_code` - PTX 代码字符串
    /// * `kernel_name` - 要加载的内核名称
    ///
    /// # Example
    ///
    /// ```ignore
    /// let accelerator = CudaAccelerator::new(0)?;
    /// let mut kernel = GpuKernel::new("my_kernel".to_string());
    /// let ptx = r#"
    ///     .version 7.5
    ///     .target sm_50
    ///     .address_size 64
    ///
    ///     .visible .entry my_kernel(
    ///         .param .u64 .ptr .global .align 8 input
    ///     )
    ///     {
    ///         ret;
    ///     }
    /// "#;
    /// kernel.load_from_ptx(&accelerator, ptx, "my_kernel")?;
    /// ```
    pub fn load_from_ptx(
        &mut self,
        accelerator: &CudaAccelerator,
        ptx_code: &str,
        kernel_name: &str,
    ) -> Result<(), PassthroughError> {
        log::info!("Loading CUDA kernel '{}' from PTX", kernel_name);

        #[cfg(feature = "cuda")]
        {
            use cudarc::driver::result;

            unsafe {
                // 加载 PTX 模块
                let mut module = std::ptr::null_mut();
                result::cuModuleLoadData(
                    &mut module,
                    ptx_code.as_ptr() as *const std::ffi::c_void,
                )
                .map_err(|e| {
                    PassthroughError::DriverBindingFailed(format!(
                        "Failed to load PTX module for kernel '{}': {:?}",
                        kernel_name, e
                    ))
                })?;

                // 获取内核函数指针
                let mut kernel_ptr = 0u64;
                result::cuModuleGetFunction(
                    &mut kernel_ptr as *mut u64 as *mut *mut std::ffi::c_void,
                    module,
                    std::ffi::CString::new(kernel_name)
                        .map_err(|e| {
                            PassthroughError::DriverBindingFailed(format!(
                                "Invalid kernel name '{}': {}",
                                kernel_name, e
                            ))
                        })?
                        .as_ptr(),
                )
                .map_err(|e| {
                    PassthroughError::DriverBindingFailed(format!(
                        "Failed to get kernel '{}' from module: {:?}",
                        kernel_name, e
                    ))
                })?;

                self.kernel_ptr = kernel_ptr;
                self.name = kernel_name.to_string();

                log::info!(
                    "Kernel '{}' loaded successfully (ptr: 0x{:x})",
                    kernel_name,
                    kernel_ptr
                );

                // 注意: 这里不立即卸载模块，因为内核需要它
                // 在实际生产代码中，应该在 GpuKernel 的 Drop 中处理模块卸载
            }
        }

        #[cfg(not(feature = "cuda"))]
        {
            log::warn!(
                "load_from_ptx called but CUDA not enabled (kernel: '{}')",
                kernel_name
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cuda_accelerator_creation() {
        let accelerator = CudaAccelerator::new(0);
        assert!(accelerator.is_ok());

        let accel = accelerator.unwrap();
        assert_eq!(accel.device_id, 0);
        assert!(!accel.device_name.is_empty());
        assert!(accel.total_memory_mb > 0);
    }

    #[test]
    fn test_cuda_device_ptr() {
        let ptr = CudaDevicePtr {
            ptr: 0x1000,
            size: 1024,
        };
        assert_eq!(ptr.ptr, 0x1000);
        assert_eq!(ptr.size, 1024);
    }

    #[test]
    fn test_cuda_stream() {
        let stream = CudaStream::new();
        assert!(stream.is_ok());

        let stream = stream.unwrap();
        assert!(stream.synchronize().is_ok());
    }

    #[test]
    fn test_cuda_malloc_free() {
        let accelerator = CudaAccelerator::new(0).unwrap();
        let d_ptr = accelerator.malloc(4096);
        assert!(d_ptr.is_ok());

        let d_ptr = d_ptr.unwrap();
        assert_eq!(d_ptr.size, 4096);

        let result = accelerator.free(d_ptr);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cuda_memcpy() {
        let accelerator = CudaAccelerator::new(0).unwrap();
        let d_ptr = accelerator.malloc(1024).unwrap();

        let src_data = vec![42u8; 1024];
        let result = accelerator.memcpy_sync(d_ptr, &src_data, CudaMemcpyKind::HostToDevice);
        assert!(result.is_ok());

        // 清理
        let _ = accelerator.free(d_ptr);
    }

    #[test]
    fn test_cuda_feature_check() {
        let accelerator = CudaAccelerator::new(0).unwrap();

        // 测试计算能力检查
        assert!(accelerator.supports_feature(CudaFeature::ComputeCapability(5, 0)));
        assert!(!accelerator.supports_feature(CudaFeature::ComputeCapability(10, 0)));

        // 测试内存检查
        assert!(accelerator.supports_feature(CudaFeature::Memory(100)));
        assert!(!accelerator.supports_feature(CudaFeature::Memory(100000)));
    }

    #[test]
    fn test_gpu_kernel() {
        let kernel = GpuKernel::new("test_kernel".to_string());
        assert_eq!(kernel.name, "test_kernel");

        // 测试内核启动（在未加载时应该失败）
        let result = kernel.launch((1, 1, 1), (32, 1, 1));
        #[cfg(feature = "cuda")]
        assert!(result.is_err()); // 内核未加载，应该失败
        #[cfg(not(feature = "cuda"))]
        assert!(result.is_ok()); // Mock模式总是成功
    }

    #[test]
    fn test_memcpy_d2d() {
        let accelerator = CudaAccelerator::new(0).unwrap();

        // 分配两个设备内存区域
        let src = accelerator.malloc(1024).unwrap();
        let dst = accelerator.malloc(1024).unwrap();

        // 测试设备到设备复制
        let result = accelerator.memcpy_d2d(dst, src, 1024);
        assert!(result.is_ok());

        // 清理
        let _ = accelerator.free(src);
        let _ = accelerator.free(dst);
    }

    #[test]
    fn test_cuda_device_info() {
        let accelerator = CudaAccelerator::new(0).unwrap();
        let info = accelerator.get_device_info();

        assert_eq!(info.device_id, 0);
        assert!(!info.name.is_empty());
        assert!(info.total_memory_mb > 0);

        // 验证计算能力格式合理
        assert!(info.compute_capability.0 >= 5); // 至少是5.x
        assert!(info.compute_capability.0 <= 9); // 不超过9.x (当前最新)
        assert!(info.compute_capability.1 <= 9);
    }
}

// ============================================================================
// GpuCompute trait 实现
// ============================================================================

#[cfg(feature = "cuda")]
impl GpuCompute for CudaAccelerator {
    fn initialize(&mut self) -> GpuResult<()> {
        // CudaAccelerator在创建时已经初始化，这里只需确认
        Ok(())
    }

    fn device_info(&self) -> GpuDeviceInfo {
        #[cfg(feature = "cuda")]
        {
            use cudarc::driver::result;
            use cudarc::driver::sys;

            // Query actual free memory
            let free_memory_mb = unsafe {
                match result::cuMemGetInfo_v2() {
                    Ok((free, _total)) => free / (1024 * 1024),
                    Err(_) => self.total_memory_mb, // Fallback to total if query fails
                }
            };

            // Query multiprocessor count
            let multiprocessor_count = unsafe {
                result::cuDeviceGetAttribute(
                    self.device_id,
                    sys::CUdevice_attribute::CU_DEVICE_ATTRIBUTE_MULTIPROCESSOR_COUNT
                ).unwrap_or(0) as u32
            };

            // Query clock rate
            let clock_rate_khz = unsafe {
                result::cuDeviceGetAttribute(
                    self.device_id,
                    sys::CUdevice_attribute::CU_DEVICE_ATTRIBUTE_CLOCK_RATE
                ).unwrap_or(0) as u32
            };

            // Query L2 cache size
            let l2_cache_size = unsafe {
                result::cuDeviceGetAttribute(
                    self.device_id,
                    sys::CUdevice_attribute::CU_DEVICE_ATTRIBUTE_L2_CACHE_SIZE
                ).unwrap_or(0) as usize
            };

            // Detect unified memory support (CUDA 6.0+)
            let supports_unified_memory = self.compute_capability >= (5, 0);

            GpuDeviceInfo {
                device_id: self.device_id as u32,
                device_name: self.device_name.clone(),
                vendor: "NVIDIA".to_string(),
                total_memory_mb: self.total_memory_mb,
                free_memory_mb,
                multiprocessor_count,
                clock_rate_khz,
                l2_cache_size,
                supports_unified_memory,
                compute_capability: format!("{}.{}", self.compute_capability.0, self.compute_capability.1),
            }
        }

        #[cfg(not(feature = "cuda"))]
        {
            GpuDeviceInfo {
                device_id: self.device_id as u32,
                device_name: self.device_name.clone(),
                vendor: "NVIDIA".to_string(),
                total_memory_mb: self.total_memory_mb,
                free_memory_mb: self.total_memory_mb,
                multiprocessor_count: 0,
                clock_rate_khz: 0,
                l2_cache_size: 0,
                supports_unified_memory: false,
                compute_capability: format!("{}.{}", self.compute_capability.0, self.compute_capability.1),
            }
        }
    }

    fn allocate_memory(&self, size: usize) -> GpuResult<GpuBuffer> {
        let ptr = self.malloc(size)?;
        Ok(GpuBuffer {
            ptr: ptr.ptr,
            size: ptr.size,
        })
    }

    fn free_memory(&self, buffer: GpuBuffer) -> GpuResult<()> {
        let device_ptr = CudaDevicePtr {
            ptr: buffer.ptr,
            size: buffer.size,
        };
        self.free(device_ptr)?;
        Ok(())
    }

    fn copy_h2d(&self, host_data: &[u8], device_buffer: &GpuBuffer) -> GpuResult<()> {
        let device_ptr = CudaDevicePtr {
            ptr: device_buffer.ptr,
            size: device_buffer.size,
        };
        self.memcpy_h2d(device_ptr, host_data)?;
        Ok(())
    }

    fn copy_d2h(&self, device_buffer: &GpuBuffer, host_data: &mut [u8]) -> GpuResult<()> {
        let device_ptr = CudaDevicePtr {
            ptr: device_buffer.ptr,
            size: device_buffer.size,
        };
        self.memcpy_d2h(host_data, device_ptr)?;
        Ok(())
    }

    fn compile_kernel(&self, source: &str, kernel_name: &str) -> GpuResult<GpuKernel> {
        #[cfg(feature = "cuda")]
        {
            use cudarc::nvrtc::result;

            // Create NVRTC program
            let program = unsafe {
                result::nvrtcCreateProgram(source.as_ptr() as *const i8, ptr::null(), 0, ptr::null(), ptr::null())
                    .map_err(|e| GpuError::CompilationFailed {
                        kernel: kernel_name.to_string(),
                        message: format!("Failed to create NVRTC program: {:?}", e),
                    })?
            };

            // Get compute capability for compilation options
            let compute_capability = format!("-arch=sm_{}", self.compute_capability.0 * 10 + self.compute_capability.1);

            // Compile the program
            let compilation_options = [compute_capability.as_str()];
            unsafe {
                result::nvrtcCompileProgram(program, compilation_options.len() as i32,
                    compilation_options.as_ptr() as *const *const i8)
                    .map_err(|e| {
                        // Get compilation log if available
                        let log_size = result::nvrtcGetProgramLogSize(program).unwrap_or(0);
                        if log_size > 0 {
                            let mut log = vec![0u8; log_size];
                            result::nvrtcGetProgramLog(program, log.as_mut_ptr()).ok();
                            let log_str = String::from_utf8_lossy(&log);
                            GpuError::CompilationFailed {
                                kernel: kernel_name.to_string(),
                                message: format!("NVRTC compilation failed: {:?}\nLog:\n{}", e, log_str),
                            }
                        } else {
                            GpuError::CompilationFailed {
                                kernel: kernel_name.to_string(),
                                message: format!("NVRTC compilation failed: {:?}", e),
                            }
                        }
                    })?;
            }

            // Get PTX size
            let ptx_size = unsafe {
                result::nvrtcGetPTXSize(program).map_err(|e| GpuError::CompilationFailed {
                    kernel: kernel_name.to_string(),
                    message: format!("Failed to get PTX size: {:?}", e),
                })?
            };

            // Get PTX code
            let mut ptx = vec![0u8; ptx_size];
            unsafe {
                result::nvrtcGetPTX(program, ptx.as_mut_ptr()).map_err(|e| GpuError::CompilationFailed {
                    kernel: kernel_name.to_string(),
                    message: format!("Failed to get PTX: {:?}", e),
                })?;
            }

            // Destroy the program
            unsafe {
                result::nvrtcDestroyProgram(&program).map_err(|e| {
                    log::warn!("Failed to destroy NVRTC program: {:?}", e);
                    // Non-fatal error, continue
                }).ok();
            }

            // Create kernel metadata
            let metadata = vm_core::gpu::KernelMetadata {
                name: kernel_name.to_string(),
                source: Some(source.to_string()),
                compiled_at: Some(std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()),
                num_params: 0, // TODO: Parse from source
                shared_memory_size: 0, // TODO: Parse from source
            };

            Ok(GpuKernel {
                name: kernel_name.to_string(),
                binary: ptx,
                metadata,
            })
        }

        #[cfg(not(feature = "cuda"))]
        {
            Err(GpuError::CompilationFailed {
                kernel: kernel_name.to_string(),
                message: "CUDA feature not enabled".to_string(),
            })
        }
    }

    fn execute_kernel(
        &self,
        kernel: &GpuKernel,
        grid_dim: (u32, u32, u32),
        block_dim: (u32, u32, u32),
        args: &[GpuArg],
        shared_memory_size: usize,
    ) -> GpuResult<GpuExecutionResult> {
        #[cfg(feature = "cuda")]
        {
            use cudarc::driver::result;
            use std::ffi::CString;

            let start = std::time::Instant::now();

            // Load PTX module
            let ptx_cstring = CString::new(kernel.binary.clone()).map_err(|e| GpuError::ExecutionFailed {
                kernel: kernel.name.clone(),
                message: format!("Failed to create PTX CString: {}", e),
            })?;

            let mut module = std::ptr::null_mut();
            unsafe {
                result::cuModuleLoadData(&mut module, ptx_cstring.as_ptr() as *const _).map_err(|e| GpuError::ExecutionFailed {
                    kernel: kernel.name.clone(),
                    message: format!("Failed to load PTX module: {:?}", e),
                })?;
            }

            // Get kernel function
            let kernel_name_cstring = CString::new(kernel.name.as_str()).map_err(|e| GpuError::ExecutionFailed {
                kernel: kernel.name.clone(),
                message: format!("Failed to create kernel name CString: {}", e),
            })?;

            let mut kernel_func = std::ptr::null_mut();
            unsafe {
                result::cuModuleGetFunction(&mut kernel_func, module, kernel_name_cstring.as_ptr()).map_err(|e| {
                    // Cleanup module on error
                    result::cuModuleUnload(module).ok();
                    GpuError::ExecutionFailed {
                        kernel: kernel.name.clone(),
                        message: format!("Failed to get kernel function: {:?}", e),
                    }
                })?;
            }

            // Prepare kernel arguments
            // Convert GpuArg enum to raw pointers
            let mut kernel_args: Vec<Vec<u8>> = Vec::new();
            let mut kernel_arg_ptrs: Vec<*const std::ffi::c_void> = Vec::new();

            for arg in args {
                let arg_bytes = match arg {
                    GpuArg::U8(v) => v.to_le_bytes().to_vec(),
                    GpuArg::U32(v) => v.to_le_bytes().to_vec(),
                    GpuArg::U64(v) => v.to_le_bytes().to_vec(),
                    GpuArg::I32(v) => v.to_le_bytes().to_vec(),
                    GpuArg::I64(v) => v.to_le_bytes().to_vec(),
                    GpuArg::F32(v) => v.to_le_bytes().to_vec(),
                    GpuArg::F64(v) => v.to_le_bytes().to_vec(),
                    GpuArg::Buffer(buf) => buf.ptr.to_le_bytes().to_vec(),
                };
                kernel_args.push(arg_bytes);
                kernel_arg_ptrs.push(kernel_args.last().unwrap().as_ptr() as *const std::ffi::c_void);
            }

            // Launch kernel
            unsafe {
                result::cuLaunchKernel(
                    kernel_func,
                    grid_dim.0, grid_dim.1, grid_dim.2,  // grid dim
                    block_dim.0, block_dim.1, block_dim.2,  // block dim
                    shared_memory_size as u32,  // shared memory bytes
                    self.stream.stream,  // stream
                    kernel_arg_ptrs.as_mut_ptr() as *mut *mut _,  // kernel arguments
                    std::ptr::null_mut(),  // extra (optional)
                ).map_err(|e| {
                    // Cleanup on error
                    result::cuModuleUnload(module).ok();
                    GpuError::ExecutionFailed {
                        kernel: kernel.name.clone(),
                        message: format!("Failed to launch kernel: {:?}", e),
                    }
                })?;
            }

            // Cleanup module (kernel can still be used)
            unsafe {
                result::cuModuleUnload(module).map_err(|e| {
                    log::warn!("Failed to unload module: {:?}", e);
                }).ok();
            }

            let elapsed = start.elapsed();

            Ok(GpuExecutionResult {
                kernel_name: kernel.name.clone(),
                execution_time_us: elapsed.as_micros() as u64,
                bytes_transferred: 0, // TODO: Track actual memory transfers
            })
        }

        #[cfg(not(feature = "cuda"))]
        {
            Err(GpuError::ExecutionFailed {
                kernel: kernel.name.clone(),
                message: "CUDA feature not enabled".to_string(),
            })
        }
    }

    fn synchronize(&self) -> GpuResult<()> {
        Ok(())
    }
}
