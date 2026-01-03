// SMMU错误类型定义
//
// This module provides standardized error handling for the SMMU component,
// with conversions to the unified VmError framework.

use vm_core::{CoreError, DeviceError, MemoryError, VmError};

/// SMMU错误类型
///
/// SMMU-specific errors that can be converted to the unified VmError type.
#[derive(Debug, Clone, thiserror::Error)]
pub enum SmmuError {
    /// 配置错误
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// 权限错误
    #[error("Permission denied: {0}")]
    PermissionError(String),

    /// 地址转换错误
    #[error("Translation failed for address {address:#x}")]
    TranslationError { address: u64 },

    /// 页表遍历错误
    #[error("Page table walk error: {0}")]
    PageTableWalkError(String),

    /// 中断错误
    #[error("Interrupt error: {0}")]
    InterruptError(String),

    /// MSI错误
    #[error("MSI error: {0}")]
    MsiError(String),

    /// 命令错误
    #[error("Command error: {0}")]
    CommandError(String),

    /// 未实现功能
    #[error("Feature not implemented: {0}")]
    NotImplementedError(String),

    /// 资源耗尽错误
    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),

    /// 无效参数错误
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    /// 内部错误
    #[error("Internal error: {0}")]
    Internal(String),

    /// 未初始化
    #[error("SMMU not initialized")]
    NotInitialized,

    /// 设备未找到
    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    /// 设备未附加
    #[error("Device not attached: {0}")]
    DeviceNotAttached(String),

    /// 无效地址
    #[error("Invalid address {addr:#x}: {reason}")]
    InvalidAddress { addr: u64, reason: String },
}

/// SMMU结果类型
pub type SmmuResult<T> = Result<T, SmmuError>;

/// Conversion from SmmuError to unified VmError
impl From<SmmuError> for VmError {
    fn from(err: SmmuError) -> Self {
        match err {
            SmmuError::ConfigError(msg) => VmError::Core(CoreError::Config {
                message: msg,
                path: Some("smmu".to_string()),
            }),
            SmmuError::PermissionError(msg) => VmError::Memory(MemoryError::ProtectionFailed {
                message: msg,
                addr: None,
            }),
            SmmuError::TranslationError { address } => {
                VmError::Memory(MemoryError::PageTableError {
                    message: format!("Translation failed for address {:#x}", address),
                    level: None,
                })
            }
            SmmuError::PageTableWalkError(msg) => VmError::Memory(MemoryError::PageTableError {
                message: msg,
                level: None,
            }),
            SmmuError::InterruptError(msg) => VmError::Device(DeviceError::IoFailed {
                device_type: "smmu".to_string(),
                operation: "interrupt".to_string(),
                message: msg,
            }),
            SmmuError::MsiError(msg) => VmError::Device(DeviceError::IoFailed {
                device_type: "smmu".to_string(),
                operation: "msi".to_string(),
                message: msg,
            }),
            SmmuError::CommandError(msg) => VmError::Device(DeviceError::IoFailed {
                device_type: "smmu".to_string(),
                operation: "command".to_string(),
                message: msg,
            }),
            SmmuError::NotImplementedError(feature) => VmError::Core(CoreError::NotImplemented {
                feature,
                module: "smmu".to_string(),
            }),
            SmmuError::ResourceExhausted(resource) => VmError::Core(CoreError::ResourceExhausted {
                resource,
                current: 0,
                limit: 0,
            }),
            SmmuError::InvalidParameter(msg) => VmError::Core(CoreError::InvalidParameter {
                name: "unknown".to_string(),
                value: "".to_string(),
                message: msg,
            }),
            SmmuError::Internal(msg) => VmError::Core(CoreError::Internal {
                message: msg,
                module: "smmu".to_string(),
            }),
            SmmuError::NotInitialized => VmError::Core(CoreError::InvalidState {
                message: "SMMU not initialized".to_string(),
                current: "uninitialized".to_string(),
                expected: "initialized".to_string(),
            }),
            SmmuError::DeviceNotFound(id) => VmError::Device(DeviceError::NotFound {
                device_type: "smmu".to_string(),
                identifier: id,
            }),
            SmmuError::DeviceNotAttached(id) => VmError::Device(DeviceError::ConfigError {
                device_type: "smmu".to_string(),
                config_item: "attachment".to_string(),
                message: format!("Device not attached: {}", id),
            }),
            SmmuError::InvalidAddress { addr, reason: _ } => {
                VmError::Memory(MemoryError::InvalidAddress(vm_core::GuestAddr(addr)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = SmmuError::ConfigError("test".to_string());
        assert!(err.to_string().contains("test"));
    }

    #[test]
    fn test_translation_error() {
        let err = SmmuError::TranslationError { address: 0x1000 };
        assert!(err.to_string().contains("0x1000"));
    }

    #[test]
    fn test_conversion_to_vm_error() {
        let smmu_err = SmmuError::ConfigError("test".to_string());
        let vm_err: VmError = smmu_err.into();
        assert!(matches!(vm_err, VmError::Core(..)));

        let smmu_err = SmmuError::TranslationError { address: 0x2000 };
        let vm_err: VmError = smmu_err.into();
        assert!(matches!(vm_err, VmError::Memory(..)));
    }
}
