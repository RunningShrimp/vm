//! 设备热插拔功能
//!
//! 支持在虚拟机运行时动态添加和移除设备

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use vm_core::{GuestAddr, MmioDevice};

/// 热插拔错误
#[derive(Debug, thiserror::Error)]
pub enum HotplugError {
    #[error("Device already exists: {0}")]
    DeviceExists(String),
    #[error("Device not found: {0}")]
    DeviceNotFound(String),
    #[error("Address conflict at 0x{0:x}")]
    AddressConflict(GuestAddr),
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
}

/// 设备类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    /// 块设备
    Block,
    /// 网络设备
    Network,
    /// 串口
    Serial,
    /// GPU
    Gpu,
    /// 其他
    Other,
}

impl DeviceType {
    pub fn name(&self) -> &'static str {
        match self {
            DeviceType::Block => "block",
            DeviceType::Network => "network",
            DeviceType::Serial => "serial",
            DeviceType::Gpu => "gpu",
            DeviceType::Other => "other",
        }
    }
}

/// 设备信息
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    /// 设备 ID
    pub id: String,
    /// 设备类型
    pub device_type: DeviceType,
    /// MMIO 基地址
    pub base_addr: GuestAddr,
    /// MMIO 大小
    pub size: u64,
    /// 是否可热插拔
    pub hotpluggable: bool,
    /// 设备描述
    pub description: Option<String>,
}

impl DeviceInfo {
    /// 创建新的设备信息
    pub fn new(
        id: impl Into<String>,
        device_type: DeviceType,
        base_addr: GuestAddr,
        size: u64,
    ) -> Self {
        Self {
            id: id.into(),
            device_type,
            base_addr,
            size,
            hotpluggable: true,
            description: None,
        }
    }

    /// 设置是否可热插拔
    pub fn with_hotpluggable(mut self, hotpluggable: bool) -> Self {
        self.hotpluggable = hotpluggable;
        self
    }

    /// 设置描述
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

/// 热插拔设备
pub struct HotplugDevice {
    /// 设备信息
    pub info: DeviceInfo,
    /// MMIO 设备实现
    pub device: Box<dyn MmioDevice>,
}

/// 热插拔管理器
pub struct HotplugManager {
    /// 已注册的设备
    devices: Arc<Mutex<HashMap<String, HotplugDevice>>>,
    /// 地址分配器
    next_addr: Arc<Mutex<GuestAddr>>,
    /// 基地址
    base_addr: GuestAddr,
    /// 地址空间大小
    addr_space_size: u64,
}

impl HotplugManager {
    /// 创建新的热插拔管理器
    pub fn new(base_addr: GuestAddr, addr_space_size: u64) -> Self {
        Self {
            devices: Arc::new(Mutex::new(HashMap::new())),
            next_addr: Arc::new(Mutex::new(base_addr)),
            base_addr,
            addr_space_size,
        }
    }

    /// 分配地址
    fn allocate_addr(&self, size: u64) -> Result<GuestAddr, HotplugError> {
        let mut next_addr = self
            .next_addr
            .lock()
            .map_err(|_| HotplugError::InvalidOperation("Failed to lock next_addr".into()))?;

        let addr = *next_addr;
        let new_next = addr + size;

        if new_next > self.base_addr + self.addr_space_size {
            return Err(HotplugError::InvalidOperation(
                "Address space exhausted".to_string(),
            ));
        }

        // 4KB 对齐
        *next_addr = GuestAddr((new_next + 0xFFF) & !0xFFF);

        Ok(addr)
    }

    /// 检查地址冲突
    fn check_address_conflict(&self, base_addr: GuestAddr, size: u64) -> Result<(), HotplugError> {
        let devices = self
            .devices
            .lock()
            .map_err(|_| HotplugError::InvalidOperation("Failed to lock devices".into()))?;

        for device in devices.values() {
            let dev_start = device.info.base_addr;
            let dev_end = dev_start + device.info.size;
            let new_start = base_addr;
            let new_end = base_addr + size;

            // 检查是否有重叠
            if new_start < dev_end && new_end > dev_start {
                return Err(HotplugError::AddressConflict(base_addr));
            }
        }

        Ok(())
    }

    /// 添加设备
    pub fn add_device(
        &self,
        mut info: DeviceInfo,
        device: Box<dyn MmioDevice>,
    ) -> Result<(), HotplugError> {
        let mut devices = self
            .devices
            .lock()
            .map_err(|_| HotplugError::InvalidOperation("Failed to lock devices".into()))?;

        // 检查设备是否已存在
        if devices.contains_key(&info.id) {
            return Err(HotplugError::DeviceExists(info.id.clone()));
        }

        // 如果没有指定地址，自动分配
        if info.base_addr == GuestAddr(0) {
            info.base_addr = self.allocate_addr(info.size)?;
        } else {
            // 使用现有的地址冲突检查方法
            self.check_address_conflict(info.base_addr, info.size)?;
        }

        log::info!(
            "Adding {} device '{}' at 0x{:x} (size: 0x{:x})",
            info.device_type.name(),
            info.id,
            info.base_addr,
            info.size
        );

        devices.insert(info.id.clone(), HotplugDevice { info, device });

        Ok(())
    }

    /// 移除设备
    pub fn remove_device(&self, id: &str) -> Result<DeviceInfo, HotplugError> {
        let mut devices = self
            .devices
            .lock()
            .map_err(|_| HotplugError::InvalidOperation("Failed to lock devices".into()))?;

        let device = devices
            .remove(id)
            .ok_or_else(|| HotplugError::DeviceNotFound(id.to_string()))?;

        if !device.info.hotpluggable {
            // 如果设备不可热插拔，放回去
            devices.insert(id.to_string(), device);
            return Err(HotplugError::InvalidOperation(format!(
                "Device '{}' is not hotpluggable",
                id
            )));
        }

        log::info!(
            "Removing {} device '{}' from 0x{:x}",
            device.info.device_type.name(),
            device.info.id,
            device.info.base_addr
        );

        Ok(device.info)
    }

    /// 获取设备信息
    pub fn get_device_info(&self, id: &str) -> Result<DeviceInfo, HotplugError> {
        let devices = self
            .devices
            .lock()
            .map_err(|_| HotplugError::InvalidOperation("Failed to lock devices".into()))?;

        devices
            .get(id)
            .map(|d| d.info.clone())
            .ok_or_else(|| HotplugError::DeviceNotFound(id.to_string()))
    }

    /// 列出所有设备
    pub fn list_devices(&self) -> Result<Vec<DeviceInfo>, HotplugError> {
        let devices = self
            .devices
            .lock()
            .map_err(|_| HotplugError::InvalidOperation("Failed to lock devices".into()))?;
        Ok(devices.values().map(|d| d.info.clone()).collect())
    }

    /// 检查设备是否存在
    pub fn device_exists(&self, id: &str) -> bool {
        if let Ok(devices) = self.devices.lock() {
            devices.contains_key(id)
        } else {
            false
        }
    }

    /// 获取设备数量
    pub fn device_count(&self) -> usize {
        self.devices
            .lock()
            .map(|devices| devices.len())
            .unwrap_or(0)
    }

    /// 按类型列出设备
    pub fn list_devices_by_type(
        &self,
        device_type: DeviceType,
    ) -> Result<Vec<DeviceInfo>, HotplugError> {
        let devices = self
            .devices
            .lock()
            .map_err(|_| HotplugError::InvalidOperation("Failed to lock devices".into()))?;
        Ok(devices
            .values()
            .filter(|d| d.info.device_type == device_type)
            .map(|d| d.info.clone())
            .collect())
    }
}

/// 热插拔事件
#[derive(Debug, Clone)]
pub enum HotplugEvent {
    /// 设备已添加
    DeviceAdded(DeviceInfo),
    /// 设备已移除
    DeviceRemoved(DeviceInfo),
    /// 设备错误
    DeviceError { id: String, error: String },
}

/// 热插拔事件监听器
pub trait HotplugEventListener: Send {
    /// 处理事件
    fn on_event(&mut self, event: HotplugEvent);
}

/// 简单的日志事件监听器
pub struct LogHotplugListener;

impl HotplugEventListener for LogHotplugListener {
    fn on_event(&mut self, event: HotplugEvent) {
        match event {
            HotplugEvent::DeviceAdded(info) => {
                log::info!("Device added: {} ({})", info.id, info.device_type.name());
            }
            HotplugEvent::DeviceRemoved(info) => {
                log::info!("Device removed: {} ({})", info.id, info.device_type.name());
            }
            HotplugEvent::DeviceError { id, error } => {
                log::error!("Device error: {}: {}", id, error);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 模拟 MMIO 设备
    struct DummyDevice;
    impl MmioDevice for DummyDevice {
        fn read(&self, _offset: u64, _size: u8) -> vm_core::VmResult<u64> {
            Ok(0)
        }
        fn write(&mut self, _offset: u64, _val: u64, _size: u8) -> vm_core::VmResult<()> {
            Ok(())
        }
    }

    #[test]
    fn test_hotplug_manager() {
        let manager = HotplugManager::new(vm_core::GuestAddr(0x10000000), 0x10000000);

        let info = DeviceInfo::new("test0", DeviceType::Block, vm_core::GuestAddr(0), 0x1000);
        let device = Box::new(DummyDevice);

        manager
            .add_device(info, device)
            .expect("Failed to add device");
        assert!(manager.device_exists("test0"));
        assert_eq!(manager.device_count(), 1);

        let info = manager
            .get_device_info("test0")
            .expect("Failed to get device info");
        assert_ne!(info.base_addr, vm_core::GuestAddr(0)); // 应该已分配地址

        manager
            .remove_device("test0")
            .expect("Failed to remove device");
        assert!(!manager.device_exists("test0"));
    }

    #[test]
    fn test_address_allocation() {
        let manager = HotplugManager::new(vm_core::GuestAddr(0x10000000), 0x10000000);

        let info1 = DeviceInfo::new("dev1", DeviceType::Block, vm_core::GuestAddr(0), 0x1000);
        let info2 = DeviceInfo::new("dev2", DeviceType::Network, vm_core::GuestAddr(0), 0x2000);

        manager
            .add_device(info1, Box::new(DummyDevice))
            .expect("Failed to add dev1");
        manager
            .add_device(info2, Box::new(DummyDevice))
            .expect("Failed to add dev2");

        let dev1_info = manager
            .get_device_info("dev1")
            .expect("Failed to get dev1 info");
        let dev2_info = manager
            .get_device_info("dev2")
            .expect("Failed to get dev2 info");

        // 地址不应该重叠
        assert!(dev1_info.base_addr.0 + dev1_info.size <= dev2_info.base_addr.0);
    }

    #[test]
    fn test_address_conflict() {
        let manager = HotplugManager::new(vm_core::GuestAddr(0x10000000), 0x10000000);

        let info1 = DeviceInfo::new(
            "dev1",
            DeviceType::Block,
            vm_core::GuestAddr(0x10000000),
            0x1000,
        );
        let info2 = DeviceInfo::new(
            "dev2",
            DeviceType::Network,
            vm_core::GuestAddr(0x10000000),
            0x1000,
        );

        manager
            .add_device(info1, Box::new(DummyDevice))
            .expect("Failed to add dev1");
        let result = manager.add_device(info2, Box::new(DummyDevice));

        assert!(result.is_err());
    }
}
