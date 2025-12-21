//! 设备服务模块 - DDD贫血模型
//!
//! 将设备管理逻辑拆分为服务层和实体层

pub mod device_manager;
pub mod device_registry;

// 重新导出主要类型
pub use device_manager::DeviceManagerService;
pub use device_registry::{
    DeviceEntity, DeviceRegistryEntity, DeviceRegistryStats, DeviceStatus, DeviceType,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use vm_core::{
        AccessType, AddressTranslator, GuestAddr, GuestPhysAddr, MemoryAccess, MmioManager,
        MmuAsAny, VmError,
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

    #[test]
    fn test_device_service_integration() {
        let service = DeviceManagerService::new();
        let mut mmu = MockMMU;

        // 创建和注册设备
        let device = DeviceEntity {
            id: "test_block_device".to_string(),
            device_type: DeviceType::Block,
            base_addr: GuestAddr(0x1000),
            size: 4096,
            interrupt: Some(42),
            status: DeviceStatus::Uninitialized,
            config: HashMap::new(),
        };

        // 注册设备
        assert!(service.register_device(device).is_ok());

        // 初始化并启动设备
        assert!(
            service
                .initialize_device("test_block_device", &mut mmu)
                .is_ok()
        );
        assert!(service.start_device("test_block_device").is_ok());

        // 验证设备状态
        assert_eq!(
            service.get_device_status("test_block_device"),
            Some(DeviceStatus::Running)
        );

        // 检查统计信息
        let stats = service.get_stats();
        assert_eq!(stats.total_devices, 1);
        assert_eq!(stats.running_devices, 1);

        // 停止并注销设备
        assert!(service.stop_device("test_block_device").is_ok());
        assert!(service.unregister_device("test_block_device").is_ok());

        let stats_after = service.get_stats();
        assert_eq!(stats_after.total_devices, 0);
    }
}
