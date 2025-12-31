//! 设备服务测试
//!
//! 测试设备附加、分离、I/O等设备管理功能。

use vm_core::VmError;
use vm_service::device_service::{DeviceService, DeviceInfo, DeviceType};
use std::sync::Arc;
use tokio::sync::Mutex;

// ============================================================================
// 设备服务测试（30个测试）
// ============================================================================

#[cfg(test)]
mod device_tests {
    use super::*;

    #[tokio::test]
    async fn test_device_attach_block() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "test_block".to_string(),
            device_type: DeviceType::Block,
            irq: None,
        };

        let result = service.attach_device(0, device).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_device_attach_network() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "test_net".to_string(),
            device_type: DeviceType::Network,
            irq: Some(10),
        };

        let result = service.attach_device(0, device).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_device_detach() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "temp_device".to_string(),
            device_type: DeviceType::Block,
            irq: None,
        };

        service.attach_device(0, device.clone()).await.unwrap();
        let result = service.detach_device(0, &device.name).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_device_list() {
        let service = DeviceService::new().await;

        let device1 = DeviceInfo {
            name: "device1".to_string(),
            device_type: DeviceType::Block,
            irq: None,
        };

        let device2 = DeviceInfo {
            name: "device2".to_string(),
            device_type: DeviceType::Network,
            irq: Some(5),
        };

        service.attach_device(0, device1).await.unwrap();
        service.attach_device(0, device2).await.unwrap();

        let devices = service.list_devices(0).await.unwrap();
        assert_eq!(devices.len(), 2);
    }

    #[tokio::test]
    async fn test_device_find() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "findable".to_string(),
            device_type: DeviceType::Block,
            irq: None,
        };

        service.attach_device(0, device.clone()).await.unwrap();
        let found = service.find_device(0, "findable").await.unwrap();

        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "findable");
    }

    #[tokio::test]
    async fn test_device_io_read() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "io_device".to_string(),
            device_type: DeviceType::Block,
            irq: None,
        };

        service.attach_device(0, device).await.unwrap();
        let result = service.device_read(0, "io_device", 0x0, 4).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_device_io_write() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "io_device".to_string(),
            device_type: DeviceType::Block,
            irq: None,
        };

        service.attach_device(0, device).await.unwrap();
        let data = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let result = service.device_write(0, "io_device", 0x0, &data).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_device_get_info() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "info_device".to_string(),
            device_type: DeviceType::Network,
            irq: Some(15),
        };

        service.attach_device(0, device.clone()).await.unwrap();
        let info = service.get_device_info(0, "info_device").await.unwrap();

        assert_eq!(info.name, "info_device");
        assert_eq!(info.irq, Some(15));
    }

    #[tokio::test]
    async fn test_device_hotplug() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "hotplug".to_string(),
            device_type: DeviceType::USB,
            irq: Some(20),
        };

        let result = service.hotplug_device(0, device).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_device_hotunplug() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "hotunplug".to_string(),
            device_type: DeviceType::USB,
            irq: None,
        };

        service.attach_device(0, device.clone()).await.unwrap();
        let result = service.hotunplug_device(0, &device.name).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_device_reset() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "reset_device".to_string(),
            device_type: DeviceType::Block,
            irq: None,
        };

        service.attach_device(0, device.clone()).await.unwrap();
        let result = service.reset_device(0, &device.name).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_device_enable() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "enable_device".to_string(),
            device_type: DeviceType::Network,
            irq: None,
        };

        service.attach_device(0, device.clone()).await.unwrap();
        let result = service.enable_device(0, &device.name).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_device_disable() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "disable_device".to_string(),
            device_type: DeviceType::Network,
            irq: None,
        };

        service.attach_device(0, device.clone()).await.unwrap();
        let result = service.disable_device(0, &device.name).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_device_configure() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "config_device".to_string(),
            device_type: DeviceType::Block,
            irq: None,
        };

        service.attach_device(0, device.clone()).await.unwrap();

        let config = serde_json::json!({"size": 1024});
        let result = service.configure_device(0, &device.name, &config).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_device_get_status() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "status_device".to_string(),
            device_type: DeviceType::Network,
            irq: None,
        };

        service.attach_device(0, device.clone()).await.unwrap();
        let status = service.get_device_status(0, &device.name).await.unwrap();

        assert!(!status.is_empty());
    }

    #[tokio::test]
    async fn test_device_get_statistics() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "stats_device".to_string(),
            device_type: DeviceType::Block,
            irq: None,
        };

        service.attach_device(0, device.clone()).await.unwrap();
        let stats = service.get_device_statistics(0, &device.name).await.unwrap();

        assert!(stats.contains_key("reads") || stats.contains_key("writes"));
    }

    #[tokio::test]
    async fn test_device_set_irq() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "irq_device".to_string(),
            device_type: DeviceType::Network,
            irq: None,
        };

        service.attach_device(0, device.clone()).await.unwrap();
        let result = service.set_device_irq(0, &device.name, Some(10)).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_device_get_irq() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "get_irq".to_string(),
            device_type: DeviceType::Network,
            irq: Some(5),
        };

        service.attach_device(0, device.clone()).await.unwrap();
        let irq = service.get_device_irq(0, &device.name).await.unwrap();

        assert_eq!(irq, Some(5));
    }

    #[tokio::test]
    async fn test_device_dma_read() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "dma_device".to_string(),
            device_type: DeviceType::Block,
            irq: None,
        };

        service.attach_device(0, device.clone()).await.unwrap();
        let result = service.dma_read(0, &device.name, 0x1000, 0x2000, 512).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_device_dma_write() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "dma_write".to_string(),
            device_type: DeviceType::Block,
            irq: None,
        };

        service.attach_device(0, device.clone()).await.unwrap();
        let data = vec![0u8; 512];
        let result = service.dma_write(0, &device.name, 0x1000, 0x2000, &data).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_device_interrupt() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "int_device".to_string(),
            device_type: DeviceType::Network,
            irq: Some(7),
        };

        service.attach_device(0, device.clone()).await.unwrap();
        let result = service.trigger_interrupt(0, &device.name).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_device_map_memory() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "mmio_device".to_string(),
            device_type: DeviceType::Block,
            irq: None,
        };

        service.attach_device(0, device.clone()).await.unwrap();
        let result = service.map_device_memory(0, &device.name, 0xF000, 0x1000).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_device_unmap_memory() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "unmap_device".to_string(),
            device_type: DeviceType::Block,
            irq: None,
        };

        service.attach_device(0, device.clone()).await.unwrap();
        service.map_device_memory(0, &device.name, 0xE000, 0x1000).await.unwrap();

        let result = service.unmap_device_memory(0, &device.name, 0xE000).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_device_batch_attach() {
        let service = DeviceService::new().await;

        let devices = vec![
            DeviceInfo {
                name: "batch1".to_string(),
                device_type: DeviceType::Block,
                irq: None,
            },
            DeviceInfo {
                name: "batch2".to_string(),
                device_type: DeviceType::Network,
                irq: Some(5),
            },
            DeviceInfo {
                name: "batch3".to_string(),
                device_type: DeviceType::USB,
                irq: None,
            },
        ];

        let result = service.attach_device_batch(0, devices).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_device_multiple_vcpus() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "shared_device".to_string(),
            device_type: DeviceType::Network,
            irq: Some(3),
        };

        service.attach_device(0, device.clone()).await.unwrap();
        service.attach_device(1, device.clone()).await.unwrap();

        let vcpu0_devices = service.list_devices(0).await.unwrap();
        let vcpu1_devices = service.list_devices(1).await.unwrap();

        assert_eq!(vcpu0_devices.len(), 1);
        assert_eq!(vcpu1_devices.len(), 1);
    }

    #[tokio::test]
    async fn test_device_console() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "console".to_string(),
            device_type: DeviceType::Console,
            irq: None,
        };

        service.attach_device(0, device.clone()).await.unwrap();
        let result = service.console_write(0, &device.name, b"Hello, World!\n").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_device_serial() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "serial".to_string(),
            device_type: DeviceType::Serial,
            irq: Some(4),
        };

        service.attach_device(0, device.clone()).await.unwrap();
        let result = service.serial_configure(0, &device.name, 115200, 8, 1, 0).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_device_block_read() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "block_read".to_string(),
            device_type: DeviceType::Block,
            irq: None,
        };

        service.attach_device(0, device.clone()).await.unwrap();
        let result = service.block_read(0, &device.name, 0, 512).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_device_block_write() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "block_write".to_string(),
            device_type: DeviceType::Block,
            irq: None,
        };

        service.attach_device(0, device.clone()).await.unwrap();
        let data = vec![0u8; 512];
        let result = service.block_write(0, &device.name, 0, &data).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_device_block_flush() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "block_flush".to_string(),
            device_type: DeviceType::Block,
            irq: None,
        };

        service.attach_device(0, device.clone()).await.unwrap();
        let result = service.block_flush(0, &device.name).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_device_network_send() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "net_send".to_string(),
            device_type: DeviceType::Network,
            irq: Some(10),
        };

        service.attach_device(0, device.clone()).await.unwrap();
        let data = vec![0x45, 0x00, 0x00]; // Ethernet frame
        let result = service.network_send(0, &device.name, &data).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_device_network_receive() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "net_recv".to_string(),
            device_type: DeviceType::Network,
            irq: Some(10),
        };

        service.attach_device(0, device.clone()).await.unwrap();
        let result = service.network_receive(0, &device.name, 1514).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_device_get_mac_address() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "mac_device".to_string(),
            device_type: DeviceType::Network,
            irq: None,
        };

        service.attach_device(0, device.clone()).await.unwrap();
        let mac = service.get_mac_address(0, &device.name).await.unwrap();

        assert_eq!(mac.len(), 6);
    }

    #[tokio::test]
    async fn test_device_set_mac_address() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "set_mac".to_string(),
            device_type: DeviceType::Network,
            irq: None,
        };

        service.attach_device(0, device.clone()).await.unwrap();
        let mac = vec![0x02, 0x00, 0x00, 0x00, 0x00, 0x01];
        let result = service.set_mac_address(0, &device.name, &mac).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_device_get_driver_version() {
        let service = DeviceService::new().await;

        let device = DeviceInfo {
            name: "version_device".to_string(),
            device_type: DeviceType::Block,
            irq: None,
        };

        service.attach_device(0, device.clone()).await.unwrap();
        let version = service.get_driver_version(0, &device.name).await.unwrap();

        assert!(!version.is_empty());
    }
}

// ============================================================================
// 配置管理测试（20个测试）
// ============================================================================

#[cfg(test)]
mod config_tests {
    use super::*;

    #[tokio::test]
    async fn test_config_create() {
        let manager = ConfigManager::new().await;
        let config = manager.get_default_config().await;

        assert!(config.is_ok());
    }

    #[tokio::test]
    async fn test_config_load() {
        let manager = ConfigManager::new().await;

        let result = manager.load_config("test_config").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_config_save() {
        let manager = ConfigManager::new().await;

        let config = VmConfig::default();
        let result = manager.save_config("test_save", &config).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_config_validate() {
        let manager = ConfigManager::new().await;

        let config = VmConfig::default();
        let result = manager.validate_config(&config).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_config_merge() {
        let manager = ConfigManager::new().await;

        let base = VmConfig::default();
        let override_config = VmConfig {
            vcpu_count: 4,
            ..Default::default()
        };

        let result = manager.merge_configs(&base, &override_config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_config_clone() {
        let manager = ConfigManager::new().await;

        let config = VmConfig::default();
        let cloned = manager.clone_config(&config).await;

        assert!(cloned.is_ok());
    }

    #[tokio::test]
    async fn test_config_diff() {
        let manager = ConfigManager::new().await;

        let config1 = VmConfig::default();
        let config2 = VmConfig {
            vcpu_count: 2,
            ..Default::default()
        };

        let diff = manager.config_diff(&config1, &config2).await.unwrap();
        assert!(!diff.is_empty());
    }

    #[tokio::test]
    async fn test_config_patch() {
        let manager = ConfigManager::new().await;

        let config = VmConfig::default();
        let patch = serde_json::json!({"vcpu_count": 4});

        let result = manager.patch_config(&config, &patch).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_config_rollback() {
        let manager = ConfigManager::new().await;

        let config = VmConfig::default();
        manager.save_config("rollback_test", &config).await.unwrap();

        let result = manager.rollback_config("rollback_test").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_config_list_templates() {
        let manager = ConfigManager::new().await;

        let templates = manager.list_templates().await.unwrap();
        assert!(!templates.is_empty());
    }

    #[tokio::test]
    async fn test_config_apply_template() {
        let manager = ConfigManager::new().await;

        let result = manager.apply_template("default").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_config_export() {
        let manager = ConfigManager::new().await;

        let config = VmConfig::default();
        let result = manager.export_config(&config, "/tmp/test_config.json").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_config_import() {
        let manager = ConfigManager::new().await;

        let result = manager.import_config("/tmp/test_config.json").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_config_set_memory_size() {
        let manager = ConfigManager::new().await;

        let result = manager.set_memory_size(512 * 1024 * 1024).await; // 512MB
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_config_set_vcpu_count() {
        let manager = ConfigManager::new().await;

        let result = manager.set_vcpu_count(8).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_config_enable_feature() {
        let manager = ConfigManager::new().await;

        let result = manager.enable_feature("nested_virtualization").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_config_disable_feature() {
        let manager = ConfigManager::new().await;

        let result = manager.disable_feature("nested_virtualization").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_config_get_features() {
        let manager = ConfigManager::new().await;

        let features = manager.get_enabled_features().await.unwrap();
        assert!(!features.is_empty());
    }

    #[tokio::test]
    async fn test_config_reset_to_default() {
        let manager = ConfigManager::new().await;

        let result = manager.reset_to_default().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_config_get_schema() {
        let manager = ConfigManager::new().await;

        let schema = manager.get_config_schema().await.unwrap();
        assert!(!schema.is_empty());
    }

    #[tokio::test]
    async fn test_config_validate_schema() {
        let manager = ConfigManager::new().await;

        let config = VmConfig::default();
        let result = manager.validate_against_schema(&config).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_config_get_presets() {
        let manager = ConfigManager::new().await;

        let presets = manager.list_presets().await.unwrap();
        assert!(!presets.is_empty());
    }

    #[tokio::test]
    async fn test_config_apply_preset() {
        let manager = ConfigManager::new().await;

        let result = manager.apply_preset("high_performance").await;
        assert!(result.is_ok());
    }
}
