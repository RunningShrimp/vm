use crate::{Fault, GuestAddr};

// Re-export commonly used error types
use std::backtrace::Backtrace;
use std::error::Error;
use std::fmt;
use std::sync::Arc;

/// 统一的虚拟机错误类型
///
/// 这是整个虚拟机系统的统一错误类型，所有模块都应该使用这个错误类型
/// 或可以转换为这个类型的错误。它支持错误链、上下文信息和回溯。
#[derive(Debug, Clone)]
pub enum VmError {
    /// 核心/基础架构错误
    Core(CoreError),
    /// 内存管理错误
    Memory(MemoryError),
    /// 执行引擎错误
    Execution(ExecutionError),
    /// 设备模拟错误
    Device(DeviceError),
    /// 平台/加速器错误
    Platform(PlatformError),
    /// IO 错误
    Io(String),
    /// 带上下文的错误包装器
    WithContext {
        /// 原始错误
        error: Box<VmError>,
        /// 上下文信息
        context: String,
        /// 可选的回溯信息
        backtrace: Option<Arc<Backtrace>>,
    },
    /// 多个错误的聚合
    Multiple(Vec<VmError>),
}

/// 核心系统错误
///
/// 包含虚拟机核心系统的基础错误，如配置、状态、内部错误等。
#[derive(Debug, Clone, PartialEq)]
pub enum CoreError {
    /// 配置错误
    Config {
        /// 错误描述
        message: String,
        /// 配置项路径
        path: Option<String>,
    },
    /// 无效配置
    InvalidConfig {
        /// 错误描述
        message: String,
        /// 配置字段
        field: String,
    },
    /// 无效状态
    InvalidState {
        /// 错误描述
        message: String,
        /// 当前状态
        current: String,
        /// 期望状态
        expected: String,
    },
    /// 未支持功能
    NotSupported {
        /// 功能描述
        feature: String,
        /// 模块名称
        module: String,
    },
    /// 解码错误
    DecodeError {
        /// 错误描述
        message: String,
        /// 错误位置（如指令地址）
        position: Option<GuestAddr>,
        /// 模块名称
        module: String,
    },
    /// 内部错误
    Internal {
        /// 错误描述
        message: String,
        /// 模块名称
        module: String,
    },
    /// 未实现功能
    NotImplemented {
        /// 功能描述
        feature: String,
        /// 模块名称
        module: String,
    },
    /// 资源耗尽
    ResourceExhausted {
        /// 资源类型
        resource: String,
        /// 当前使用量
        current: u64,
        /// 限制
        limit: u64,
    },
    /// 并发错误
    Concurrency {
        /// 错误描述
        message: String,
        /// 操作类型
        operation: String,
    },
    /// 无效参数
    InvalidParameter {
        /// 参数名称
        name: String,
        /// 参数值
        value: String,
        /// 错误描述
        message: String,
    },
}

/// 内存管理错误
///
/// 包含所有与内存管理相关的错误，如访问违规、映射失败等。
#[derive(Debug, Clone, PartialEq)]
pub enum MemoryError {
    /// 访问违规
    AccessViolation {
        /// 访问地址
        addr: GuestAddr,
        /// 错误描述
        msg: String,
        /// 访问类型
        access_type: Option<crate::AccessType>,
    },
    /// 映射失败
    MappingFailed {
        /// 错误描述
        message: String,
        /// 源地址
        src: Option<GuestAddr>,
        /// 目标地址
        dst: Option<GuestAddr>,
    },
    /// 分配失败
    AllocationFailed {
        /// 错误描述
        message: String,
        /// 请求大小
        size: Option<usize>,
    },
    /// MMU 锁失败
    MmuLockFailed {
        /// 错误描述
        message: String,
    },
    /// 无效地址
    InvalidAddress(GuestAddr),
    /// 对齐错误
    AlignmentError {
        /// 地址
        addr: GuestAddr,
        /// 要求的对齐
        required: u64,
        /// 实际大小
        size: u8,
    },
    /// 页表错误
    PageTableError {
        /// 错误描述
        message: String,
        /// 页表级别
        level: Option<u8>,
    },
    /// 保护失败
    ProtectionFailed {
        /// 错误描述
        message: String,
        /// 地址
        addr: Option<GuestAddr>,
    },
}

/// 执行引擎错误
///
/// 包含执行引擎相关的错误，如指令错误、执行失败等。
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionError {
    /// 故障/异常
    Fault(Fault),
    /// 无效指令
    InvalidInstruction {
        /// 操作码
        opcode: u64,
        /// 指令地址
        pc: GuestAddr,
    },
    /// 执行暂停
    Halted {
        /// 暂停原因
        reason: String,
    },
    /// 取指失败
    FetchFailed {
        /// 程序计数器
        pc: GuestAddr,
        /// 错误描述
        message: String,
    },
    /// 超时
    Timeout {
        /// 超时时间（毫秒）
        timeout_ms: u64,
        /// 操作类型
        operation: String,
    },
    /// JIT 编译错误
    JitError {
        /// 错误描述
        message: String,
        /// 函数地址
        function_addr: Option<GuestAddr>,
    },
}

/// 设备模拟错误
///
/// 包含所有设备模拟相关的错误。
#[derive(Debug, Clone, PartialEq)]
pub enum DeviceError {
    /// 设备未找到
    NotFound {
        /// 设备类型
        device_type: String,
        /// 设备标识
        identifier: String,
    },
    /// 初始化失败
    InitFailed {
        /// 设备类型
        device_type: String,
        /// 错误描述
        message: String,
    },
    /// IO 操作失败
    IoFailed {
        /// 设备类型
        device_type: String,
        /// 操作类型
        operation: String,
        /// 错误描述
        message: String,
    },
    /// 配置错误
    ConfigError {
        /// 设备类型
        device_type: String,
        /// 配置项
        config_item: String,
        /// 错误描述
        message: String,
    },
    /// 设备繁忙
    Busy {
        /// 设备类型
        device_type: String,
    },
    /// 不支持的操作
    UnsupportedOperation {
        /// 设备类型
        device_type: String,
        /// 操作名称
        operation: String,
    },
}

/// 平台/加速器错误
///
/// 包含平台特定的错误和硬件虚拟化相关的错误。
#[derive(Debug, Clone, PartialEq)]
pub enum PlatformError {
    /// 加速器不可用
    AcceleratorUnavailable {
        /// 平台名称
        platform: String,
        /// 错误描述
        reason: String,
    },
    /// 虚拟化管理程序错误
    HypervisorError {
        /// 错误描述
        message: String,
        /// 错误代码
        code: Option<i32>,
    },
    /// 不支持的架构
    UnsupportedArch {
        /// 架构名称
        arch: String,
        /// 支持的架构列表
        supported: Vec<String>,
    },
    /// 权限不足
    PermissionDenied {
        /// 操作名称
        operation: String,
        /// 所需权限
        required_permission: String,
    },
    /// 系统调用失败
    SystemCallFailed {
        /// 系统调用名称
        syscall: String,
        /// 错误代码
        errno: Option<i32>,
    },
    /// 初始化失败
    InitializationFailed(String),
    /// 资源分配失败
    ResourceAllocationFailed(String),
    /// 内存映射失败
    MemoryMappingFailed(String),
    /// 执行失败
    ExecutionFailed(String),
    /// 访问被拒绝
    AccessDenied(String),
    /// 无效参数
    InvalidParameter {
        /// 参数名称
        name: String,
        /// 参数值
        value: String,
        /// 错误消息
        message: String,
    },
    /// 不支持的操作
    UnsupportedOperation(String),
    /// 硬件不可用
    HardwareUnavailable(String),
    /// IO 错误
    IoError(String),
    /// IOCTL 错误
    IoctlError {
        /// 错误号
        errno: i32,
        /// 操作名称
        operation: String,
    },
    /// 设备分配失败
    DeviceAssignmentFailed(String),
    /// 内存访问失败
    MemoryAccessFailed(String),
    /// 无效状态
    InvalidState {
        /// 错误消息
        message: String,
        /// 当前状态
        current: String,
        /// 期望状态
        expected: String,
    },
}

// ============================================================================
// Display Implementations
// ============================================================================

impl fmt::Display for VmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VmError::Core(e) => write!(f, "Core error: {}", e),
            VmError::Memory(e) => write!(f, "Memory error: {}", e),
            VmError::Execution(e) => write!(f, "Execution error: {}", e),
            VmError::Device(e) => write!(f, "Device error: {}", e),
            VmError::Platform(e) => write!(f, "Platform error: {}", e),
            VmError::Io(e) => write!(f, "IO error: {}", e),
            VmError::WithContext { error, context, .. } => write!(f, "{}: {}", context, error),
            VmError::Multiple(errors) => {
                write!(f, "Multiple errors occurred:")?;
                for (i, e) in errors.iter().enumerate() {
                    writeln!(f, "  {}. {}", i + 1, e)?;
                }
                Ok(())
            }
        }
    }
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::Config { message, path } => match path {
                Some(p) => write!(f, "Configuration error at '{}': {}", p, message),
                None => write!(f, "Configuration error: {}", message),
            },
            CoreError::InvalidConfig { message, field } => {
                write!(f, "Invalid configuration: {} (field: {})", message, field)
            }
            CoreError::InvalidState {
                message,
                current,
                expected,
            } => {
                write!(
                    f,
                    "Invalid state: {} (current: {}, expected: {})",
                    message, current, expected
                )
            }
            CoreError::NotSupported { feature, module } => {
                write!(f, "Feature '{}' not supported in {}", feature, module)
            }
            CoreError::DecodeError {
                message,
                position,
                module,
            } => match position {
                Some(pos) => write!(f, "Decode error in {} at {:#x}: {}", module, pos, message),
                None => write!(f, "Decode error in {}: {}", module, message),
            },
            CoreError::Internal { message, module } => {
                write!(f, "Internal error in {}: {}", module, message)
            }
            CoreError::NotImplemented { feature, module } => {
                write!(f, "Feature '{}' not implemented in {}", feature, module)
            }
            CoreError::ResourceExhausted {
                resource,
                current,
                limit,
            } => {
                write!(
                    f,
                    "Resource '{}' exhausted: {}/{}",
                    resource, current, limit
                )
            }
            CoreError::Concurrency { message, operation } => {
                write!(f, "Concurrency error during '{}': {}", operation, message)
            }
            CoreError::InvalidParameter {
                name,
                value,
                message,
            } => {
                write!(f, "Invalid parameter '{}='{}': {}", name, value, message)
            }
        }
    }
}

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryError::AccessViolation {
                addr,
                msg,
                access_type,
            } => match access_type {
                Some(at) => write!(f, "Access violation at {:#x} ({:?}): {}", addr, at, msg),
                None => write!(f, "Access violation at {:#x}: {}", addr, msg),
            },
            MemoryError::MappingFailed { message, src, dst } => match (src, dst) {
                (Some(s), Some(d)) => write!(f, "Mapping failed {:#x} -> {:#x}: {}", s, d, message),
                (Some(s), None) => write!(f, "Mapping failed from {:#x}: {}", s, message),
                (None, Some(d)) => write!(f, "Mapping failed to {:#x}: {}", d, message),
                (None, None) => write!(f, "Mapping failed: {}", message),
            },
            MemoryError::AllocationFailed { message, size } => match size {
                Some(s) => write!(f, "Allocation failed ({} bytes): {}", s, message),
                None => write!(f, "Allocation failed: {}", message),
            },
            MemoryError::MmuLockFailed { message } => {
                write!(f, "Failed to acquire MMU lock: {}", message)
            }
            MemoryError::InvalidAddress(addr) => write!(f, "Invalid address: {:#x}", addr),
            MemoryError::AlignmentError {
                addr,
                required,
                size,
            } => {
                write!(
                    f,
                    "Alignment error: address {:#x} not aligned to {} bytes (size: {})",
                    addr, required, size
                )
            }
            MemoryError::PageTableError { message, level } => match level {
                Some(l) => write!(f, "Page table error at level {}: {}", l, message),
                None => write!(f, "Page table error: {}", message),
            },
            MemoryError::ProtectionFailed { message, addr } => match addr {
                Some(a) => write!(f, "Protection failed at {:#x}: {}", a, message),
                None => write!(f, "Protection failed: {}", message),
            },
        }
    }
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecutionError::Fault(fault) => write!(f, "Fault: {:?}", fault),
            ExecutionError::InvalidInstruction { opcode, pc } => {
                write!(f, "Invalid instruction {:#x} at {:#x}", opcode, pc)
            }
            ExecutionError::Halted { reason } => write!(f, "CPU halted: {}", reason),
            ExecutionError::FetchFailed { pc, message } => {
                write!(f, "Instruction fetch failed at {:#x}: {}", pc, message)
            }
            ExecutionError::Timeout {
                timeout_ms,
                operation,
            } => {
                write!(
                    f,
                    "Operation '{}' timed out after {}ms",
                    operation, timeout_ms
                )
            }
            ExecutionError::JitError {
                message,
                function_addr,
            } => match function_addr {
                Some(addr) => write!(f, "JIT error at {:#x}: {}", addr, message),
                None => write!(f, "JIT error: {}", message),
            },
        }
    }
}

impl fmt::Display for DeviceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeviceError::NotFound {
                device_type,
                identifier,
            } => {
                write!(f, "Device '{}' not found: {}", device_type, identifier)
            }
            DeviceError::InitFailed {
                device_type,
                message,
            } => {
                write!(
                    f,
                    "Device '{}' initialization failed: {}",
                    device_type, message
                )
            }
            DeviceError::IoFailed {
                device_type,
                operation,
                message,
            } => {
                write!(
                    f,
                    "Device '{}' IO operation '{}' failed: {}",
                    device_type, operation, message
                )
            }
            DeviceError::ConfigError {
                device_type,
                config_item,
                message,
            } => {
                write!(
                    f,
                    "Device '{}' configuration '{}' error: {}",
                    device_type, config_item, message
                )
            }
            DeviceError::Busy { device_type } => {
                write!(f, "Device '{}' is busy", device_type)
            }
            DeviceError::UnsupportedOperation {
                device_type,
                operation,
            } => {
                write!(
                    f,
                    "Operation '{}' not supported by device '{}'",
                    operation, device_type
                )
            }
        }
    }
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlatformError::AcceleratorUnavailable { platform, reason } => {
                write!(f, "Accelerator unavailable on {}: {}", platform, reason)
            }
            PlatformError::HypervisorError { message, code } => match code {
                Some(c) => write!(f, "Hypervisor error (code {}): {}", c, message),
                None => write!(f, "Hypervisor error: {}", message),
            },
            PlatformError::UnsupportedArch { arch, supported } => {
                write!(
                    f,
                    "Unsupported architecture '{}'. Supported: {}",
                    arch,
                    supported.join(", ")
                )
            }
            PlatformError::PermissionDenied {
                operation,
                required_permission,
            } => {
                write!(
                    f,
                    "Permission denied for operation '{}'. Requires: {}",
                    operation, required_permission
                )
            }
            PlatformError::SystemCallFailed { syscall, errno } => match errno {
                Some(e) => write!(f, "System call '{}' failed with errno {}", syscall, e),
                None => write!(f, "System call '{}' failed", syscall),
            },
            PlatformError::InitializationFailed(msg) => write!(f, "Initialization failed: {}", msg),
            PlatformError::ResourceAllocationFailed(msg) => {
                write!(f, "Resource allocation failed: {}", msg)
            }
            PlatformError::MemoryMappingFailed(msg) => write!(f, "Memory mapping failed: {}", msg),
            PlatformError::ExecutionFailed(msg) => write!(f, "Execution failed: {}", msg),
            PlatformError::AccessDenied(msg) => write!(f, "Access denied: {}", msg),
            PlatformError::InvalidParameter {
                name,
                value,
                message,
            } => {
                write!(
                    f,
                    "Invalid parameter '{}': {} (value: {})",
                    name, message, value
                )
            }
            PlatformError::UnsupportedOperation(msg) => write!(f, "Unsupported operation: {}", msg),
            PlatformError::HardwareUnavailable(msg) => write!(f, "Hardware unavailable: {}", msg),
            PlatformError::IoError(e) => write!(f, "IO error: {}", e),
            PlatformError::IoctlError { errno, operation } => {
                write!(f, "IOCTL '{}' failed with errno {}", operation, errno)
            }
            PlatformError::DeviceAssignmentFailed(msg) => {
                write!(f, "Device assignment failed: {}", msg)
            }
            PlatformError::MemoryAccessFailed(msg) => {
                write!(f, "Memory access failed: {}", msg)
            }
            PlatformError::InvalidState {
                message,
                current,
                expected,
            } => {
                write!(
                    f,
                    "Invalid state: {} (current: {}, expected: {})",
                    message, current, expected
                )
            }
        }
    }
}

impl Error for VmError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            VmError::Core(e) => Some(e),
            VmError::Memory(e) => Some(e),
            VmError::Execution(e) => Some(e),
            VmError::Device(e) => Some(e),
            VmError::Platform(e) => Some(e),
            VmError::Io(_) => None,
            VmError::WithContext { error, .. } => Some(error.as_ref()),
            VmError::Multiple(_) => None,
        }
    }
}

impl Error for CoreError {}
impl Error for MemoryError {}
impl Error for ExecutionError {}
impl Error for DeviceError {}
impl Error for PlatformError {}

// ============================================================================
// Conversions
// ============================================================================

impl From<CoreError> for VmError {
    fn from(e: CoreError) -> Self {
        VmError::Core(e)
    }
}

impl From<MemoryError> for VmError {
    fn from(e: MemoryError) -> Self {
        VmError::Memory(e)
    }
}

impl From<ExecutionError> for VmError {
    fn from(e: ExecutionError) -> Self {
        VmError::Execution(e)
    }
}

impl From<DeviceError> for VmError {
    fn from(e: DeviceError) -> Self {
        VmError::Device(e)
    }
}

impl From<PlatformError> for VmError {
    fn from(e: PlatformError) -> Self {
        VmError::Platform(e)
    }
}

impl From<std::io::Error> for VmError {
    fn from(e: std::io::Error) -> Self {
        VmError::Io(e.to_string())
    }
}

impl From<Fault> for VmError {
    fn from(f: Fault) -> Self {
        VmError::Execution(ExecutionError::Fault(f))
    }
}

impl From<String> for VmError {
    fn from(s: String) -> Self {
        VmError::Core(CoreError::Internal {
            message: s,
            module: "unknown".to_string(),
        })
    }
}

impl PartialEq for VmError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (VmError::Core(a), VmError::Core(b)) => a == b,
            (VmError::Memory(a), VmError::Memory(b)) => a == b,
            (VmError::Execution(a), VmError::Execution(b)) => a == b,
            (VmError::Device(a), VmError::Device(b)) => a == b,
            (VmError::Platform(a), VmError::Platform(b)) => a == b,
            (VmError::Io(a), VmError::Io(b)) => a == b,
            (
                VmError::WithContext {
                    error: a,
                    context: ca,
                    ..
                },
                VmError::WithContext {
                    error: b,
                    context: cb,
                    ..
                },
            ) => a == b && ca == cb,
            (VmError::Multiple(a), VmError::Multiple(b)) => a == b,
            _ => false,
        }
    }
}

impl From<&str> for VmError {
    fn from(s: &str) -> Self {
        VmError::Core(CoreError::Internal {
            message: s.to_string(),
            module: "unknown".to_string(),
        })
    }
}

// ============================================================================
// Error Context Trait
// ============================================================================

/// 错误上下文扩展 trait
///
/// 提供类似 anyhow 的错误上下文功能，支持错误链和上下文信息。
pub trait ErrorContext<T> {
    /// 添加静态上下文字符串
    fn context(self, ctx: &str) -> Result<T, VmError>;

    /// 使用闭包动态生成上下文
    fn with_context<F, S>(self, f: F) -> Result<T, VmError>
    where
        F: FnOnce() -> S,
        S: Into<String>;
}

impl<T, E> ErrorContext<T> for Result<T, E>
where
    E: Into<VmError>,
{
    fn context(self, ctx: &str) -> Result<T, VmError> {
        self.map_err(|e| {
            let vm_err = e.into();
            VmError::WithContext {
                error: Box::new(vm_err),
                context: ctx.to_string(),
                backtrace: Some(Arc::new(Backtrace::capture())),
            }
        })
    }

    fn with_context<F, S>(self, f: F) -> Result<T, VmError>
    where
        F: FnOnce() -> S,
        S: Into<String>,
    {
        self.map_err(|e| {
            let vm_err = e.into();
            VmError::WithContext {
                error: Box::new(vm_err),
                context: f().into(),
                backtrace: Some(Arc::new(Backtrace::capture())),
            }
        })
    }
}

/// 错误结果类型扩展
pub trait VmResultExt<T> {
    /// 将错误转换为 VmError 并添加上下文
    fn vm_context(self, ctx: &str) -> Result<T, VmError>;

    /// 将错误转换为 VmError 并使用闭包添加上下文
    fn vm_with_context<F, S>(self, f: F) -> Result<T, VmError>
    where
        F: FnOnce() -> S,
        S: Into<String>;
}

impl<T> VmResultExt<T> for Result<T, std::io::Error> {
    fn vm_context(self, ctx: &str) -> Result<T, VmError> {
        self.context(ctx)
    }

    fn vm_with_context<F, S>(self, f: F) -> Result<T, VmError>
    where
        F: FnOnce() -> S,
        S: Into<String>,
    {
        self.with_context(f)
    }
}

// ============================================================================
// 错误恢复和重试机制
// ============================================================================

/// 错误恢复策略
#[derive(Debug, Clone)]
pub enum ErrorRecoveryStrategy {
    /// 不重试
    None,
    /// 固定间隔重试
    Fixed { max_attempts: u32, delay_ms: u64 },
    /// 指数退避重试
    ExponentialBackoff {
        max_attempts: u32,
        initial_delay_ms: u64,
        max_delay_ms: u64,
        multiplier: f64,
    },
    /// 立即重试
    Immediate { max_attempts: u32 },
}

/// 错误恢复 trait
pub trait ErrorRecovery {
    /// 检查错误是否可重试
    fn is_retryable(&self) -> bool;

    /// 获取建议的延迟时间（毫秒）
    fn suggested_delay(&self, attempt: u32) -> u64;
}

impl ErrorRecovery for VmError {
    fn is_retryable(&self) -> bool {
        match self {
            VmError::Core(CoreError::Concurrency { .. }) => true,
            VmError::Memory(MemoryError::MmuLockFailed { .. }) => true,
            VmError::Device(DeviceError::Busy { .. }) => true,
            VmError::Platform(PlatformError::AcceleratorUnavailable { .. }) => true,
            VmError::Io(_) => false, // IO errors stored as strings can't be checked for retryability
            VmError::WithContext { error, .. } => error.is_retryable(),
            _ => false,
        }
    }

    fn suggested_delay(&self, attempt: u32) -> u64 {
        match self {
            VmError::Device(DeviceError::Busy { .. }) => 100 * (1 << attempt.min(6)),
            VmError::Memory(MemoryError::MmuLockFailed { .. }) => 10,
            VmError::Io(_) => 1000, // Default delay for IO errors stored as strings
            _ => 1000,
        }
    }
}

/// 带重试的操作执行器
pub fn retry_with_strategy<T, F>(
    mut operation: F,
    strategy: ErrorRecoveryStrategy,
) -> Result<T, VmError>
where
    F: FnMut() -> Result<T, VmError>,
{
    let mut attempts = 0;
    let max_attempts = match &strategy {
        ErrorRecoveryStrategy::None => return operation(),
        ErrorRecoveryStrategy::Fixed { max_attempts, .. } => *max_attempts,
        ErrorRecoveryStrategy::ExponentialBackoff { max_attempts, .. } => *max_attempts,
        ErrorRecoveryStrategy::Immediate { max_attempts } => *max_attempts,
    };

    loop {
        attempts += 1;
        match operation() {
            Ok(result) => return Ok(result),
            Err(error) if attempts >= max_attempts => {
                return Err(VmError::WithContext {
                    error: Box::new(error),
                    context: format!("Failed after {} attempts", attempts),
                    backtrace: Some(Arc::new(Backtrace::capture())),
                });
            }
            Err(error) if !error.is_retryable() => return Err(error),
            Err(_error) => {
                let delay = match &strategy {
                    ErrorRecoveryStrategy::Fixed { delay_ms, .. } => *delay_ms,
                    ErrorRecoveryStrategy::ExponentialBackoff {
                        initial_delay_ms,
                        max_delay_ms,
                        multiplier,
                        ..
                    } => {
                        let base_delay =
                            *initial_delay_ms as f64 * multiplier.powi(attempts as i32 - 1);
                        base_delay.min(*max_delay_ms as f64) as u64
                    }
                    ErrorRecoveryStrategy::Immediate { .. } => 0,
                    ErrorRecoveryStrategy::None => unreachable!(),
                };

                if delay > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(delay));
                }
            }
        }
    }
}

// ============================================================================
// 错误聚合和日志
// ============================================================================

/// 错误收集器
#[derive(Debug, Default)]
pub struct ErrorCollector {
    errors: Vec<VmError>,
}

impl ErrorCollector {
    /// 创建新的错误收集器
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加错误
    pub fn push(&mut self, error: VmError) {
        self.errors.push(error);
    }

    /// 添加结果，如果是错误的话
    pub fn push_result<T>(&mut self, result: Result<T, VmError>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(error) => {
                self.push(error);
                None
            }
        }
    }

    /// 检查是否有错误
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// 获取错误数量
    pub fn len(&self) -> usize {
        self.errors.len()
    }

    /// 检查是否有错误
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    /// 获取错误列表
    pub fn errors(&self) -> &[VmError] {
        &self.errors
    }

    /// 将错误转换为 VmError::Multiple
    pub fn into_vm_error(self) -> Option<VmError> {
        if self.errors.is_empty() {
            None
        } else if self.errors.len() == 1 {
            // Safe: we just checked there's exactly one error
            self.errors.into_iter().next()
        } else {
            Some(VmError::Multiple(self.errors))
        }
    }

    /// 清空错误
    pub fn clear(&mut self) {
        self.errors.clear();
    }
}

/// 错误日志记录器
pub struct ErrorLogger;

impl ErrorLogger {
    /// 记录错误到日志
    pub fn log_error(error: &VmError) {
        log::error!("VM Error: {}", error);

        // 如果有错误链，记录源错误
        let mut source = error.source();
        let mut depth = 1;
        while let Some(err) = source {
            log::error!("  Caused by ({}): {}", depth, err);
            source = err.source();
            depth += 1;
            if depth > 10 {
                break;
            } // 防止无限循环
        }
    }

    /// 记录警告
    pub fn log_warning(error: &VmError) {
        log::warn!("VM Warning: {}", error);
    }

    /// 记录调试信息
    pub fn log_debug(error: &VmError) {
        log::debug!("VM Debug: {}", error);
    }
}

/// ============================================================================
/// 统一错误转换 Trait
/// ============================================================================

/// 统一错误转换 Trait
///
/// 为所有错误类型提供转换为 `VmError` 的标准方法。
pub trait IntoVmError {
    /// 将错误转换为 `VmError`
    fn into_vm_error(self) -> VmError;

    /// 将错误转换为 `VmError` 并附加上下文信息
    fn into_vm_error_with_context(self, context: impl Into<String>) -> VmError;
}

/// 为 `VmError` 实现恒等转换
impl IntoVmError for VmError {
    fn into_vm_error(self) -> VmError {
        self
    }

    fn into_vm_error_with_context(self, _context: impl Into<String>) -> VmError {
        self
    }
}

/// 为字符串错误提供简单转换
impl IntoVmError for String {
    fn into_vm_error(self) -> VmError {
        VmError::Io(self)
    }

    fn into_vm_error_with_context(self, context: impl Into<String>) -> VmError {
        VmError::Io(format!("{}: {}", context.into(), self))
    }
}

/// 为 `&str` 错误提供简单转换
impl IntoVmError for &str {
    fn into_vm_error(self) -> VmError {
        VmError::Io(self.to_string())
    }

    fn into_vm_error_with_context(self, context: impl Into<String>) -> VmError {
        VmError::Io(format!("{}: {}", context.into(), self))
    }
}

/// 为 Box<dyn Error> 提供转换
impl IntoVmError for Box<dyn std::error::Error + Send + Sync> {
    fn into_vm_error(self) -> VmError {
        VmError::Io(self.to_string())
    }

    fn into_vm_error_with_context(self, context: impl Into<String>) -> VmError {
        VmError::Io(format!("{}: {}", context.into(), self))
    }
}

/// 为标准库 IO 错误提供转换
impl IntoVmError for std::io::Error {
    fn into_vm_error(self) -> VmError {
        VmError::Io(self.to_string())
    }

    fn into_vm_error_with_context(self, context: impl Into<String>) -> VmError {
        VmError::Io(format!("{}: {}", context.into(), self))
    }
}
