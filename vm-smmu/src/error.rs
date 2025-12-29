// SMMU错误类型定义

use std::fmt;

/// SMMU错误类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmmuError {
    /// 配置错误
    ConfigError(String),
    /// 权限错误
    PermissionError(String),
    /// 地址转换错误
    TranslationError(u64),
    /// 页表遍历错误
    PageTableWalkError(String),
    /// 中断错误
    InterruptError(String),
    /// MSI错误
    MsiError(String),
    /// 命令错误
    CommandError(String),
    /// 未实现功能
    NotImplementedError(String),
    /// 资源耗尽错误
    ResourceExhausted(String),
    /// 无效参数错误
    InvalidParameter(String),
    /// 内部错误
    Internal(String),
    /// 未初始化
    NotInitialized,
    /// 设备未找到
    DeviceNotFound(String),
    /// 设备未附加
    DeviceNotAttached(String),
    /// 无效地址
    InvalidAddress { addr: u64, reason: String },
}

impl fmt::Display for SmmuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SmmuError::ConfigError(msg) => write!(f, "Config Error: {}", msg),
            SmmuError::PermissionError(msg) => write!(f, "Permission Error: {}", msg),
            SmmuError::TranslationError(addr) => write!(f, "Translation Error: addr={:#x}", addr),
            SmmuError::PageTableWalkError(msg) => write!(f, "Page Table Walk Error: {}", msg),
            SmmuError::InterruptError(msg) => write!(f, "Interrupt Error: {}", msg),
            SmmuError::MsiError(msg) => write!(f, "MSI Error: {}", msg),
            SmmuError::CommandError(msg) => write!(f, "Command Error: {}", msg),
            SmmuError::NotImplementedError(feature) => write!(f, "Not Implemented: {}", feature),
            SmmuError::ResourceExhausted(msg) => write!(f, "Resource Exhausted: {}", msg),
            SmmuError::InvalidParameter(msg) => write!(f, "Invalid Parameter: {}", msg),
            SmmuError::Internal(msg) => write!(f, "Internal Error: {}", msg),
            SmmuError::NotInitialized => write!(f, "SMMU Not Initialized"),
            SmmuError::DeviceNotFound(id) => write!(f, "Device Not Found: {}", id),
            SmmuError::DeviceNotAttached(id) => write!(f, "Device Not Attached: {}", id),
            SmmuError::InvalidAddress { addr, reason } => {
                write!(f, "Invalid Address 0x{:x}: {}", addr, reason)
            }
        }
    }
}

impl std::error::Error for SmmuError {}

/// SMMU结果类型
pub type SmmuResult<T> = Result<T, SmmuError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = SmmuError::ConfigError("test".to_string());
        assert_eq!(err.to_string(), "Config Error: test");
    }

    #[test]
    fn test_translation_error() {
        let err = SmmuError::TranslationError(0x1000);
        assert!(err.to_string().contains("0x1000"));
    }
}
