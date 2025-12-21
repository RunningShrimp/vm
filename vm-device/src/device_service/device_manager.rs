//! 设备管理器服务 - DDD服务层
//!
//! 提供设备生命周期管理和协调的服务接口

use super::device_registry::{DeviceEntity, DeviceRegistryEntity, DeviceStatus, DeviceType};
use std::sync::{Arc, Mutex};
use vm_core::{DeviceError, GuestAddr, MMU, VmError};

/// 设备管理器服务
pub struct DeviceManagerService {
    /// 设备注册表实体
    registry: Arc<Mutex<DeviceRegistryEntity>>,
}

impl Default for DeviceManagerService {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceManagerService {
    /// 创建新的设备管理器服务
    pub fn new() -> Self {
        Self {
            registry: Arc::new(Mutex::new(DeviceRegistryEntity::new())),
        }
    }

    /// 注册新设备
    pub fn register_device(&self, device: DeviceEntity) -> Result<(), VmError> {
        let mut registry = self.registry.lock().unwrap();
        registry.register_device(device)
    }

    /// 注销设备
    pub fn unregister_device(&self, device_id: &str) -> Result<(), VmError> {
        let mut registry = self.registry.lock().unwrap();
        registry.unregister_device(device_id)
    }

    /// 初始化设备
    pub fn initialize_device(&self, device_id: &str, mmu: &mut dyn MMU) -> Result<(), VmError> {
        let mut registry = self.registry.lock().unwrap();

        // 获取设备
        let device = registry.get_device(device_id).ok_or_else(|| {
            VmError::Device(DeviceError::NotFound {
                device_type: "generic".to_string(),
                identifier: device_id.to_string(),
            })
        })?;

        // 验证设备地址空间
        self.validate_device_address(device, mmu)?;

        // 更新设备状态
        registry.update_device_status(device_id, DeviceStatus::Initialized)?;

        Ok(())
    }

    /// 启动设备
    pub fn start_device(&self, device_id: &str) -> Result<(), VmError> {
        let mut registry = self.registry.lock().unwrap();
        registry.update_device_status(device_id, DeviceStatus::Running)
    }

    /// 停止设备
    pub fn stop_device(&self, device_id: &str) -> Result<(), VmError> {
        let mut registry = self.registry.lock().unwrap();
        registry.update_device_status(device_id, DeviceStatus::Stopped)
    }

    /// 获取设备状态
    pub fn get_device_status(&self, device_id: &str) -> Option<DeviceStatus> {
        let registry = self.registry.lock().unwrap();
        registry.get_device(device_id).map(|d| d.status.clone())
    }

    /// 获取所有设备状态
    pub fn get_all_device_status(&self) -> Vec<(String, DeviceStatus)> {
        let registry = self.registry.lock().unwrap();
        registry
            .get_all_devices()
            .into_iter()
            .map(|d| (d.id.clone(), d.status.clone()))
            .collect()
    }

    /// 处理设备I/O操作
    pub fn handle_device_io(
        &self,
        addr: GuestAddr,
        data: u64,
        is_write: bool,
        mmu: &mut dyn MMU,
    ) -> Result<Option<u64>, VmError> {
        let registry = self.registry.lock().unwrap();

        // 查找地址对应的设备
        if let Some(device) = registry.get_device_by_address(addr) {
            if !matches!(device.status, DeviceStatus::Running) {
                return Err(VmError::Device(DeviceError::NotFound {
                    device_type: "generic".to_string(),
                    identifier: device.id.clone(),
                }));
            }

            // 根据设备类型处理I/O
            match device.device_type {
                DeviceType::Block => self.handle_block_io(device, addr, data, is_write, mmu),
                DeviceType::Network => self.handle_network_io(device, addr, data, is_write, mmu),
                _ => Err(VmError::Device(DeviceError::UnsupportedOperation {
                    device_type: "generic".to_string(),
                    operation: format!("{:?}", device.device_type),
                })),
            }
        } else {
            Err(VmError::Device(DeviceError::NotFound {
                device_type: "mmio".to_string(),
                identifier: format!("address_{:#x}", addr.0),
            }))
        }
    }

    /// 处理块设备I/O
    fn handle_block_io(
        &self,
        _device: &DeviceEntity,
        _addr: GuestAddr,
        _data: u64,
        _is_write: bool,
        _mmu: &mut dyn MMU,
    ) -> Result<Option<u64>, VmError> {
        // 在实际实现中，这里会调用具体的块设备处理逻辑
        // 例如：读取/写入块数据，处理命令队列等
        Ok(Some(0)) // 占位符返回值
    }

    /// 处理网络设备I/O
    fn handle_network_io(
        &self,
        _device: &DeviceEntity,
        _addr: GuestAddr,
        _data: u64,
        _is_write: bool,
        _mmu: &mut dyn MMU,
    ) -> Result<Option<u64>, VmError> {
        // 在实际实现中，这里会调用具体的网络设备处理逻辑
        Ok(Some(0)) // 占位符返回值
    }

    /// 验证设备地址空间
    fn validate_device_address(
        &self,
        device: &DeviceEntity,
        mmu: &mut dyn MMU,
    ) -> Result<(), VmError> {
        // 检查地址空间是否可用
        let end_addr = GuestAddr(device.base_addr.0 + device.size - 1);

        // 尝试翻译地址以验证内存映射
        mmu.translate(device.base_addr, vm_core::AccessType::Read)?;
        mmu.translate(end_addr, vm_core::AccessType::Read)?;

        Ok(())
    }

    /// 获取设备统计信息
    pub fn get_stats(&self) -> crate::device_service::device_registry::DeviceRegistryStats {
        let registry = self.registry.lock().unwrap();
        registry.get_stats().clone()
    }

    /// 清理错误状态的设备
    pub fn cleanup_failed_devices(&self) -> Result<usize, VmError> {
        let mut registry = self.registry.lock().unwrap();
        let failed_devices: Vec<String> = registry
            .get_all_devices()
            .into_iter()
            .filter(|d| matches!(d.status, DeviceStatus::Error(_)))
            .map(|d| d.id.clone())
            .collect();

        let count = failed_devices.len();
        for device_id in failed_devices {
            registry.unregister_device(&device_id)?;
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device_service::device_registry::DeviceType;
    use std::collections::HashMap;
    use vm_core::{
        AccessType, AddressTranslator, GuestPhysAddr, MemoryAccess, MmioManager, MmuAsAny,
    };

    struct MockMMU;
    impl AddressTranslator for MockMMU {
        fn translate(
            &mut self,
            va: GuestAddr,
            _access: AccessType,
        ) -> Result<GuestPhysAddr, VmError> {
            Ok(GuestPhysAddr(va.0))
        }
        fn flush_tlb(&mut self) {}
    }

    impl MemoryAccess for MockMMU {
        fn read(&self, _addr: GuestAddr, _size: u8) -> Result<u64, VmError> {
            Ok(0)
        }
        fn write(&mut self, _addr: GuestAddr, _val: u64, _size: u8) -> Result<(), VmError> {
            Ok(())
        }
        fn fetch_insn(&self, _pc: GuestAddr) -> Result<u64, VmError> {
            Ok(0)
        }
        fn memory_size(&self) -> usize {
            0
        }
        fn dump_memory(&self) -> Vec<u8> {
            Vec::new()
        }
        fn restore_memory(&mut self, _data: &[u8]) -> Result<(), String> {
            Ok(())
        }
    }

    impl MmioManager for MockMMU {
        fn map_mmio(&self, _base: GuestAddr, _size: u64, _device: Box<dyn vm_core::MmioDevice>) {}
    }

    impl MmuAsAny for MockMMU {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    fn create_test_device(id: &str, addr: u64) -> DeviceEntity {
        DeviceEntity {
            id: id.to_string(),
            device_type: DeviceType::Block,
            base_addr: GuestAddr(addr),
            size: 4096,
            interrupt: Some(42),
            status: DeviceStatus::Uninitialized,
            config: HashMap::new(),
        }
    }

    #[test]
    fn test_device_manager_service() {
        let service = DeviceManagerService::new();
        let mut mmu = MockMMU;

        // 注册设备
        let device = create_test_device("test_device", 0x1000);
        assert!(service.register_device(device).is_ok());

        // 初始化设备
        assert!(service.initialize_device("test_device", &mut mmu).is_ok());
        assert_eq!(
            service.get_device_status("test_device"),
            Some(DeviceStatus::Initialized)
        );

        // 启动设备
        assert!(service.start_device("test_device").is_ok());
        assert_eq!(
            service.get_device_status("test_device"),
            Some(DeviceStatus::Running)
        );

        // 处理I/O操作
        let result = service.handle_device_io(GuestAddr(0x1000), 42, false, &mut mmu);
        assert!(result.is_ok());

        // 停止设备
        assert!(service.stop_device("test_device").is_ok());
        assert_eq!(
            service.get_device_status("test_device"),
            Some(DeviceStatus::Stopped)
        );
    }
}
