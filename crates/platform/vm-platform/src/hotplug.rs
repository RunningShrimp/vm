//! 设备热插拔管理
//!
//! 提供虚拟机设备的动态添加和移除功能
//! 从 vm-boot/hotplug.rs 迁移而来

use vm_core::VmError;

/// 设备类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    /// 网络设备
    Network,
    /// 块设备
    Block,
    /// 字符设备
    Char,
    /// USB 设备
    Usb,
    /// PCI 设备
    Pci,
}

/// 设备信息
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    /// 设备名称
    pub name: String,
    /// 设备类型
    pub device_type: DeviceType,
    /// 设备地址
    pub address: Option<String>,
    /// 设备描述
    pub description: String,
}

/// 热插拔事件
#[derive(Debug, Clone)]
pub enum HotplugEvent {
    /// 设备已添加
    DeviceAdded(String),
    /// 设备已移除
    DeviceRemoved(String),
    /// 设备状态变更
    DeviceStateChanged(String, DeviceState),
    /// 错误发生
    HotplugError(String),
}

/// 设备状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceState {
    Unknown,
    Detached,
    Attached,
    InUse,
}

/// 热插拔管理器特征
pub trait HotplugManager: Send + Sync {
    /// 添加设备
    fn add_device(&mut self, device: DeviceInfo) -> Result<(), VmError>;

    /// 移除设备
    fn remove_device(&mut self, device_name: &str) -> Result<(), VmError>;

    /// 列出所有设备
    fn list_devices(&self) -> Result<Vec<DeviceInfo>, VmError>;

    /// 获取设备状态
    fn get_device_state(&self, device_name: &str) -> Result<DeviceState, VmError>;
}

/// 简化的热插拔管理器实现
pub struct SimpleHotplugManager {
    devices: std::collections::HashMap<String, DeviceInfo>,
}

impl SimpleHotplugManager {
    /// 创建新的热插拔管理器
    pub fn new() -> Self {
        Self {
            devices: std::collections::HashMap::new(),
        }
    }

    /// 添加设备
    pub fn add_device(&mut self, device: DeviceInfo) -> Result<(), VmError> {
        log::info!("Adding device: {}", device.name);
        self.devices.insert(device.name.clone(), device);
        Ok(())
    }

    /// 移除设备
    pub fn remove_device(&mut self, device_name: &str) -> Result<(), VmError> {
        log::info!("Removing device: {}", device_name);

        if self.devices.remove(device_name).is_none() {
            return Err(VmError::Io(format!("Device '{}' not found", device_name)));
        }

        Ok(())
    }

    /// 列出所有设备
    pub fn list_devices(&self) -> Result<Vec<DeviceInfo>, VmError> {
        Ok(self.devices.values().cloned().collect())
    }

    /// 获取设备状态
    pub fn get_device_state(&self, device_name: &str) -> Result<DeviceState, VmError> {
        if let Some(_device) = self.devices.get(device_name) {
            Ok(DeviceState::Attached)
        } else {
            Ok(DeviceState::Unknown)
        }
    }
}

impl Default for SimpleHotplugManager {
    fn default() -> Self {
        Self::new()
    }
}
