//! 设备注册表 - DDD实体层
//!
//! 管理所有已注册设备的实体信息

use std::collections::HashMap;
use vm_core::{DeviceError, GuestAddr, VmError};

/// 设备实体
#[derive(Debug, Clone)]
pub struct DeviceEntity {
    /// 设备ID
    pub id: String,
    /// 设备类型
    pub device_type: DeviceType,
    /// 基础地址
    pub base_addr: GuestAddr,
    /// 地址空间大小
    pub size: u64,
    /// 中断号
    pub interrupt: Option<u32>,
    /// 设备状态
    pub status: DeviceStatus,
    /// 配置参数
    pub config: HashMap<String, String>,
}

/// 设备类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceType {
    /// 块设备
    Block,
    /// 网络设备
    Network,
    /// 输入设备
    Input,
    /// 显示设备
    Display,
    /// 音频设备
    Audio,
    /// 其他设备
    Other(String),
}

/// 设备状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceStatus {
    /// 未初始化
    Uninitialized,
    /// 已初始化
    Initialized,
    /// 运行中
    Running,
    /// 已停止
    Stopped,
    /// 错误状态
    Error(String),
}

/// 设备注册表实体
#[derive(Debug)]
pub struct DeviceRegistryEntity {
    /// 已注册的设备
    devices: HashMap<String, DeviceEntity>,
    /// 地址映射（地址 -> 设备ID）
    address_map: HashMap<GuestAddr, String>,
    /// 统计信息
    stats: DeviceRegistryStats,
}

/// 注册表统计信息
#[derive(Debug, Clone, Default)]
pub struct DeviceRegistryStats {
    /// 总设备数
    pub total_devices: usize,
    /// 运行中的设备数
    pub running_devices: usize,
    /// 错误设备数
    pub error_devices: usize,
}

impl Default for DeviceRegistryEntity {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceRegistryEntity {
    /// 创建新的设备注册表
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
            address_map: HashMap::new(),
            stats: DeviceRegistryStats::default(),
        }
    }

    /// 注册设备
    pub fn register_device(&mut self, device: DeviceEntity) -> Result<(), VmError> {
        // 检查地址冲突
        if self.address_map.contains_key(&device.base_addr) {
            return Err(VmError::Device(DeviceError::IoFailed {
                device_type: "generic".to_string(),
                operation: "address_mapping".to_string(),
                message: format!("Address conflict at {:#x}", device.base_addr.0),
            }));
        }

        // 检查ID冲突
        if self.devices.contains_key(&device.id) {
            return Err(VmError::Device(DeviceError::InitFailed {
                device_type: "generic".to_string(),
                message: format!("Device {} already exists", device.id),
            }));
        }

        let device_id = device.id.clone();
        let addr = device.base_addr;

        self.devices.insert(device_id.clone(), device);
        self.address_map.insert(addr, device_id);

        self.update_stats();

        Ok(())
    }

    /// 注销设备
    pub fn unregister_device(&mut self, device_id: &str) -> Result<(), VmError> {
        if let Some(device) = self.devices.remove(device_id) {
            self.address_map.remove(&device.base_addr);
            self.update_stats();
            Ok(())
        } else {
            Err(VmError::Device(DeviceError::NotFound {
                device_type: "generic".to_string(),
                identifier: device_id.to_string(),
            }))
        }
    }

    /// 获取设备
    pub fn get_device(&self, device_id: &str) -> Option<&DeviceEntity> {
        self.devices.get(device_id)
    }

    /// 通过地址查找设备
    pub fn get_device_by_address(&self, addr: GuestAddr) -> Option<&DeviceEntity> {
        self.address_map
            .get(&addr)
            .and_then(|device_id| self.devices.get(device_id))
    }

    /// 获取所有设备
    pub fn get_all_devices(&self) -> Vec<&DeviceEntity> {
        self.devices.values().collect()
    }

    /// 更新设备状态
    pub fn update_device_status(
        &mut self,
        device_id: &str,
        status: DeviceStatus,
    ) -> Result<(), VmError> {
        if let Some(device) = self.devices.get_mut(device_id) {
            device.status = status;
            self.update_stats();
            Ok(())
        } else {
            Err(VmError::Device(DeviceError::NotFound {
                device_type: "generic".to_string(),
                identifier: device_id.to_string(),
            }))
        }
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> &DeviceRegistryStats {
        &self.stats
    }

    /// 更新统计信息
    fn update_stats(&mut self) {
        self.stats.total_devices = self.devices.len();
        self.stats.running_devices = self
            .devices
            .values()
            .filter(|d| matches!(d.status, DeviceStatus::Running))
            .count();
        self.stats.error_devices = self
            .devices
            .values()
            .filter(|d| matches!(d.status, DeviceStatus::Error(_)))
            .count();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_device(id: &str, addr: u64) -> DeviceEntity {
        DeviceEntity {
            id: id.to_string(),
            device_type: DeviceType::Block,
            base_addr: GuestAddr(addr),
            size: 4096,
            interrupt: Some(42),
            status: DeviceStatus::Initialized,
            config: HashMap::new(),
        }
    }

    #[test]
    fn test_device_registry() {
        let mut registry = DeviceRegistryEntity::new();

        // 注册设备
        let device1 = create_test_device("device1", 0x1000);
        let device2 = create_test_device("device2", 0x2000);

        assert!(registry.register_device(device1.clone()).is_ok());
        assert!(registry.register_device(device2.clone()).is_ok());

        // 验证注册
        assert_eq!(registry.get_stats().total_devices, 2);
        assert!(registry.get_device("device1").is_some());
        assert!(registry.get_device_by_address(GuestAddr(0x1000)).is_some());

        // 测试地址冲突
        let conflict_device = create_test_device("device3", 0x1000);
        assert!(registry.register_device(conflict_device).is_err());

        // 测试ID冲突
        let id_conflict = create_test_device("device1", 0x3000);
        assert!(registry.register_device(id_conflict).is_err());

        // 注销设备
        assert!(registry.unregister_device("device1").is_ok());
        assert_eq!(registry.get_stats().total_devices, 1);
        assert!(registry.get_device("device1").is_none());
    }
}
