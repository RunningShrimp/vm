//! GPU错误类型定义

use std::fmt;

/// GPU错误类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GpuError {
    /// 没有可用的GPU设备
    NoDeviceAvailable,

    /// 设备初始化失败
    DeviceInitializationFailed { device_type: String, reason: String },

    /// 内存分配失败
    MemoryAllocationFailed {
        requested_size: usize,
        reason: String,
    },

    /// 内存复制失败
    MemoryCopyFailed { direction: String, reason: String },

    /// 内核编译失败
    KernelCompilationFailed {
        kernel_name: String,
        source: String,
        reason: String,
    },

    /// 内核加载失败
    KernelLoadingFailed { kernel_name: String, reason: String },

    /// 内核执行失败
    KernelExecutionFailed { kernel_name: String, reason: String },

    /// 特性不支持
    FeatureNotSupported { feature: String, device: String },

    /// 驱动绑定失败
    DriverBindingFailed { driver_type: String, reason: String },

    /// IO错误
    Io(String),

    /// 其他错误
    Other(String),
}

impl fmt::Display for GpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GpuError::NoDeviceAvailable => {
                write!(f, "No GPU device available")
            }
            GpuError::DeviceInitializationFailed {
                device_type,
                reason,
            } => {
                write!(f, "Failed to initialize {} device: {}", device_type, reason)
            }
            GpuError::MemoryAllocationFailed {
                requested_size,
                reason,
            } => {
                write!(
                    f,
                    "Failed to allocate GPU memory ({} bytes): {}",
                    requested_size, reason
                )
            }
            GpuError::MemoryCopyFailed { direction, reason } => {
                write!(f, "Failed to copy memory ({}): {}", direction, reason)
            }
            GpuError::KernelCompilationFailed {
                kernel_name,
                source,
                reason,
            } => {
                write!(
                    f,
                    "Failed to compile kernel '{}': {} - Source: {}",
                    kernel_name, reason, source
                )
            }
            GpuError::KernelLoadingFailed {
                kernel_name,
                reason,
            } => {
                write!(f, "Failed to load kernel '{}': {}", kernel_name, reason)
            }
            GpuError::KernelExecutionFailed {
                kernel_name,
                reason,
            } => {
                write!(f, "Failed to execute kernel '{}': {}", kernel_name, reason)
            }
            GpuError::FeatureNotSupported { feature, device } => {
                write!(
                    f,
                    "Feature '{}' not supported on device {}",
                    feature, device
                )
            }
            GpuError::DriverBindingFailed {
                driver_type,
                reason,
            } => {
                write!(f, "Failed to bind {} driver: {}", driver_type, reason)
            }
            GpuError::Io(msg) => {
                write!(f, "IO error: {}", msg)
            }
            GpuError::Other(msg) => {
                write!(f, "GPU error: {}", msg)
            }
        }
    }
}

impl std::error::Error for GpuError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        // 由于使用了String而不是std::io::Error，这里返回None
        None
    }
}

/// GPU结果类型
pub type GpuResult<T> = Result<T, GpuError>;

impl From<std::io::Error> for GpuError {
    fn from(err: std::io::Error) -> Self {
        GpuError::Io(err.to_string())
    }
}
