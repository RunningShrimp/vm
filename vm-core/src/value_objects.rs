//! 值对象定义
//!
//! 将基础类型封装为值对象，提供类型安全性和验证逻辑。

use crate::VmError;
use serde::{Deserialize, Serialize};
use std::fmt;

/// 虚拟机ID值对象
///
/// 封装虚拟机ID，提供验证逻辑。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VmId(String);

impl VmId {
    /// 创建新的VM ID
    ///
    /// # 验证规则
    /// - 长度必须在1-64字符之间
    /// - 只能包含字母、数字、连字符和下划线
    pub fn new(id: String) -> Result<Self, VmError> {
        if id.is_empty() || id.len() > 64 {
            return Err(VmError::Core(crate::CoreError::Config {
                message: "VM ID must be between 1 and 64 characters".to_string(),
                path: Some("vm_id".to_string()),
            }));
        }

        if !id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(VmError::Core(crate::CoreError::Config {
                message: "VM ID can only contain alphanumeric characters, hyphens, and underscores"
                    .to_string(),
                path: Some("vm_id".to_string()),
            }));
        }

        Ok(Self(id))
    }

    /// 从字符串创建（不验证，用于内部使用）
    pub fn from_string_unchecked(id: String) -> Self {
        Self(id)
    }

    /// 获取ID字符串
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for VmId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for VmId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// 内存大小值对象
///
/// 封装内存大小，提供验证和单位转换。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MemorySize {
    bytes: u64,
}

impl MemorySize {
    /// 最小内存大小（1MB）
    pub const MIN: Self = Self { bytes: 1024 * 1024 };
    /// 最大内存大小（1TB）
    pub const MAX: Self = Self {
        bytes: 1024 * 1024 * 1024 * 1024,
    };

    /// 从字节创建
    pub fn from_bytes(bytes: u64) -> Result<Self, VmError> {
        if bytes < Self::MIN.bytes {
            return Err(VmError::Core(crate::CoreError::Config {
                message: format!(
                    "Memory size too small: {} bytes (minimum: {} bytes)",
                    bytes,
                    Self::MIN.bytes
                ),
                path: Some("memory_size".to_string()),
            }));
        }

        if bytes > Self::MAX.bytes {
            return Err(VmError::Core(crate::CoreError::Config {
                message: format!(
                    "Memory size too large: {} bytes (maximum: {} bytes)",
                    bytes,
                    Self::MAX.bytes
                ),
                path: Some("memory_size".to_string()),
            }));
        }

        Ok(Self { bytes })
    }

    /// 从MB创建
    pub fn from_mb(mb: u64) -> Result<Self, VmError> {
        Self::from_bytes(mb * 1024 * 1024)
    }

    /// 从GB创建
    pub fn from_gb(gb: u64) -> Result<Self, VmError> {
        Self::from_bytes(gb * 1024 * 1024 * 1024)
    }

    /// 获取字节数
    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// 获取MB数
    pub fn as_mb(&self) -> u64 {
        self.bytes / (1024 * 1024)
    }

    /// 获取GB数
    pub fn as_gb(&self) -> u64 {
        self.bytes / (1024 * 1024 * 1024)
    }

    /// 检查是否为页对齐
    pub fn is_page_aligned(&self) -> bool {
        self.bytes % 4096 == 0
    }
}

impl From<MemorySize> for u64 {
    fn from(size: MemorySize) -> Self {
        size.bytes
    }
}

impl From<MemorySize> for usize {
    fn from(size: MemorySize) -> Self {
        size.bytes as usize
    }
}

/// vCPU数量值对象
///
/// 封装vCPU数量，提供验证逻辑。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct VcpuCount {
    count: u32,
}

impl VcpuCount {
    /// 最小vCPU数量
    pub const MIN: u32 = 1;
    /// 最大vCPU数量
    pub const MAX: u32 = 256;

    /// 创建新的vCPU数量
    pub fn new(count: u32) -> Result<Self, VmError> {
        if count < Self::MIN {
            return Err(VmError::Core(crate::CoreError::Config {
                message: format!("vCPU count too small: {} (minimum: {})", count, Self::MIN),
                path: Some("vcpu_count".to_string()),
            }));
        }

        if count > Self::MAX {
            return Err(VmError::Core(crate::CoreError::Config {
                message: format!("vCPU count too large: {} (maximum: {})", count, Self::MAX),
                path: Some("vcpu_count".to_string()),
            }));
        }

        Ok(Self { count })
    }

    /// 获取数量
    pub fn count(&self) -> u32 {
        self.count
    }
}

impl From<VcpuCount> for u32 {
    fn from(count: VcpuCount) -> Self {
        count.count
    }
}

/// 端口号值对象
///
/// 封装端口号，提供验证逻辑。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PortNumber {
    port: u16,
}

impl PortNumber {
    /// 创建新的端口号
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    /// 获取端口号
    pub fn port(&self) -> u16 {
        self.port
    }

    /// 检查是否为特权端口（< 1024）
    pub fn is_privileged(&self) -> bool {
        self.port < 1024
    }
}

impl From<PortNumber> for u16 {
    fn from(port: PortNumber) -> Self {
        port.port
    }
}

/// 设备ID值对象
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceId(String);

impl DeviceId {
    /// 创建新的设备ID
    pub fn new(id: String) -> Result<Self, VmError> {
        if id.is_empty() || id.len() > 128 {
            return Err(VmError::Core(crate::CoreError::Config {
                message: "Device ID must be between 1 and 128 characters".to_string(),
                path: Some("device_id".to_string()),
            }));
        }

        Ok(Self(id))
    }

    /// 获取ID字符串
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for DeviceId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_id_validation() {
        // 有效ID
        assert!(VmId::new("vm-001".to_string()).is_ok());
        assert!(VmId::new("vm_test_123".to_string()).is_ok());
        assert!(VmId::new("a".to_string()).is_ok()); // 最小长度
        assert!(VmId::new("a".repeat(64)).is_ok()); // 最大长度

        // 无效ID
        assert!(VmId::new("".to_string()).is_err());
        assert!(VmId::new("a".repeat(65)).is_err()); // 超过最大长度
        assert!(VmId::new("vm@123".to_string()).is_err()); // 包含非法字符
        assert!(VmId::new("vm 123".to_string()).is_err()); // 包含空格
    }

    #[test]
    fn test_vm_id_display() {
        let id = VmId::new("test-vm".to_string()).unwrap();
        assert_eq!(format!("{}", id), "test-vm");
        assert_eq!(id.as_str(), "test-vm");
        assert_eq!(id.as_ref(), "test-vm");
    }

    #[test]
    fn test_vm_id_from_string_unchecked() {
        let id = VmId::from_string_unchecked("invalid@id".to_string());
        assert_eq!(id.as_str(), "invalid@id");
    }

    #[test]
    fn test_memory_size_validation() {
        // 有效大小
        assert!(MemorySize::from_bytes(MemorySize::MIN.bytes).is_ok());
        assert!(MemorySize::from_bytes(MemorySize::MAX.bytes).is_ok());
        assert!(MemorySize::from_mb(128).is_ok());
        assert!(MemorySize::from_gb(4).is_ok());

        // 无效大小
        assert!(MemorySize::from_bytes(MemorySize::MIN.bytes - 1).is_err());
        assert!(MemorySize::from_bytes(MemorySize::MAX.bytes + 1).is_err());
    }

    #[test]
    fn test_memory_size_conversions() {
        let size = MemorySize::from_mb(128).unwrap();
        assert_eq!(size.as_mb(), 128);
        assert_eq!(size.as_gb(), 0);
        assert_eq!(size.bytes(), 128 * 1024 * 1024);

        let size_gb = MemorySize::from_gb(4).unwrap();
        assert_eq!(size_gb.as_gb(), 4);
        assert_eq!(size_gb.as_mb(), 4096);
        assert_eq!(size_gb.bytes(), 4 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_memory_size_page_alignment() {
        let aligned = MemorySize::from_bytes(4096).unwrap();
        assert!(aligned.is_page_aligned());

        let not_aligned = MemorySize::from_bytes(4097).unwrap();
        assert!(!not_aligned.is_page_aligned());
    }

    #[test]
    fn test_memory_size_from_traits() {
        let size = MemorySize::from_mb(100).unwrap();
        let bytes: u64 = size.into();
        assert_eq!(bytes, 100 * 1024 * 1024);

        let size = MemorySize::from_mb(100).unwrap();
        let bytes: usize = size.into();
        assert_eq!(bytes, 100 * 1024 * 1024);
    }

    #[test]
    fn test_vcpu_count_validation() {
        // 有效数量
        assert!(VcpuCount::new(VcpuCount::MIN).is_ok());
        assert!(VcpuCount::new(VcpuCount::MAX).is_ok());
        assert!(VcpuCount::new(8).is_ok());

        // 无效数量
        assert!(VcpuCount::new(0).is_err());
        assert!(VcpuCount::new(VcpuCount::MAX + 1).is_err());
    }

    #[test]
    fn test_vcpu_count_from_trait() {
        let count = VcpuCount::new(4).unwrap();
        let num: u32 = count.into();
        assert_eq!(num, 4);
        assert_eq!(count.count(), 4);
    }

    #[test]
    fn test_port_number() {
        let port = PortNumber::new(8080);
        assert_eq!(port.port(), 8080);
        assert!(!port.is_privileged());

        let priv_port = PortNumber::new(80);
        assert!(priv_port.is_privileged());
    }

    #[test]
    fn test_port_number_from_trait() {
        let port = PortNumber::new(8080);
        let num: u16 = port.into();
        assert_eq!(num, 8080);
    }

    #[test]
    fn test_device_id_validation() {
        // 有效ID
        assert!(DeviceId::new("device-001".to_string()).is_ok());
        assert!(DeviceId::new("a".to_string()).is_ok()); // 最小长度
        assert!(DeviceId::new("a".repeat(128)).is_ok()); // 最大长度

        // 无效ID
        assert!(DeviceId::new("".to_string()).is_err());
        assert!(DeviceId::new("a".repeat(129)).is_err()); // 超过最大长度
    }

    #[test]
    fn test_device_id_display() {
        let id = DeviceId::new("test-device".to_string()).unwrap();
        assert_eq!(format!("{}", id), "test-device");
        assert_eq!(id.as_str(), "test-device");
        assert_eq!(id.as_ref(), "test-device");
    }
}
