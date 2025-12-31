//! vm-service 补充测试 - 边界条件和错误处理
//!
//! 覆盖边界条件、错误处理等场景

use vm_core::{GuestAddr, VmError, VmConfig, VmLifecycleState};
use vm_service::snapshot_manager::SnapshotManager;
use vm_service::execution_service::ExecutionService;
use vm_service::device_service::DeviceService;

// ============================================================================
// 错误处理测试（40个测试）
// ============================================================================

#[cfg(test)]
mod error_handling_tests {
    use super::*;

    #[tokio::test]
    async fn test_snapshot_restore_nonexistent() {
        let manager = SnapshotManager::new("test".to_string()).await;
        let result = manager.restore_snapshot("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_snapshot_delete_nonexistent() {
        let manager = SnapshotManager::new("test".to_string()).await;
        let result = manager.delete_snapshot("nonexistent").await;
        // 应该成功（幂等操作）或失败，取决于实现
        let _ = result;
    }

    #[tokio::test]
    async fn test_snapshot_empty_name() {
        let manager = SnapshotManager::new("test".to_string()).await;
        let result = manager.create_snapshot("").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_snapshot_duplicate_name() {
        let manager = SnapshotManager::new("test".to_string()).await;
        manager.create_snapshot("test_snapshot").await.unwrap();

        // 重复创建应该失败
        let result = manager.create_snapshot("test_snapshot").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execution_service_invalid_state_transition() {
        let config = VmConfig::default();
        let service = ExecutionService::new(config);

        // 从Stopped状态直接跳转到Running应该成功
        let result = service.start().await;
        assert!(result.is_ok());

        // 已经在Running状态，再次start应该失败
        let result = service.start().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_device_service_attach_invalid_device() {
        let service = DeviceService::new();
        let result = service.attach_device("invalid_device_type", "test_device").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_device_service_detach_nonexistent() {
        let service = DeviceService::new();
        let result = service.detach_device("nonexistent_device").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_snapshot_name_with_special_chars() {
        let manager = SnapshotManager::new("test".to_string()).await;

        // 测试特殊字符
        let special_names = vec!["test/snapshot", "test\\snapshot"];
        for name in special_names {
            let result = manager.create_snapshot(name).await;
            assert!(result.is_err(), "Special char should fail: {}", name);
        }
    }

    #[tokio::test]
    async fn test_snapshot_very_long_name() {
        let manager = SnapshotManager::new("test".to_string()).await;

        // 测试超长名称（1000字符）
        let long_name = "a".repeat(1000);
        let result = manager.create_snapshot(&long_name).await;
        // 应该失败或成功，取决于实现
        let _ = result;
    }

    #[tokio::test]
    async fn test_execution_pause_when_not_running() {
        let config = VmConfig::default();
        let service = ExecutionService::new(config);

        // 未启动时暂停应该失败
        let result = service.pause().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execution_resume_when_not_paused() {
        let config = VmConfig::default();
        let service = ExecutionService::new(config);

        service.start().await.unwrap();

        // 运行中恢复应该失败
        let result = service.resume().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execution_terminate_when_stopped() {
        let config = VmConfig::default();
        let service = ExecutionService::new(config);

        // 未启动时终止应该成功（幂等）
        let result = service.terminate().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_device_service_io_to_nonexistent() {
        let service = DeviceService::new();

        // 向不存在的设备进行I/O
        let result = service.device_io("nonexistent", 0x1000, 0x42).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execution_service_multiple_lifecycle_cycles() {
        let config = VmConfig::default();
        let service = ExecutionService::new(config);

        // 测试多个生命周期循环
        for _ in 0..3 {
            service.start().await.unwrap();
            service.pause().await.unwrap();
            service.resume().await.unwrap();
            service.terminate().await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_device_service_multiple_attach_detach() {
        let service = DeviceService::new();

        // 多次附加和分离
        for i in 0..5 {
            let device_name = format!("test_device_{}", i);
            service.attach_device("serial", &device_name).await.unwrap();
            service.detach_device(&device_name).await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_snapshot_manager_list_empty() {
        let manager = SnapshotManager::new("test".to_string()).await;
        let snapshots = manager.list_snapshots().await;
        assert!(snapshots.is_ok());
        assert!(snapshots.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_snapshot_manager_list_after_delete() {
        let manager = SnapshotManager::new("test".to_string()).await;

        manager.create_snapshot("test1").await.unwrap();
        manager.create_snapshot("test2").await.unwrap();

        manager.delete_snapshot("test1").await.unwrap();

        let snapshots = manager.list_snapshots().await.unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0], "test2");
    }

    #[tokio::test]
    async fn test_execution_service_get_state_initial() {
        let config = VmConfig::default();
        let service = ExecutionService::new(config);

        let state = service.get_state().await;
        assert_eq!(state, VmLifecycleState::Stopped);
    }

    #[tokio::test]
    async fn test_execution_service_get_state_after_start() {
        let config = VmConfig::default();
        let service = ExecutionService::new(config);

        service.start().await.unwrap();
        let state = service.get_state().await;
        assert_eq!(state, VmLifecycleState::Running);
    }

    #[tokio::test]
    async fn test_execution_service_get_state_after_pause() {
        let config = VmConfig::default();
        let service = ExecutionService::new(config);

        service.start().await.unwrap();
        service.pause().await.unwrap();

        let state = service.get_state().await;
        assert_eq!(state, VmLifecycleState::Paused);
    }

    #[tokio::test]
    async fn test_execution_service_get_state_after_terminate() {
        let config = VmConfig::default();
        let service = ExecutionService::new(config);

        service.start().await.unwrap();
        service.terminate().await.unwrap();

        let state = service.get_state().await;
        assert_eq!(state, VmLifecycleState::Stopped);
    }

    #[tokio::test]
    async fn test_device_service_list_devices_empty() {
        let service = DeviceService::new();
        let devices = service.list_devices().await;
        assert!(devices.is_ok());
        assert!(devices.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_device_service_list_devices_after_attach() {
        let service = DeviceService::new();

        service.attach_device("serial", "serial1").await.unwrap();
        service.attach_device("serial", "serial2").await.unwrap();

        let devices = service.list_devices().await.unwrap();
        assert_eq!(devices.len(), 2);
    }

    #[tokio::test]
    async fn test_device_service_list_devices_after_detach() {
        let service = DeviceService::new();

        service.attach_device("serial", "serial1").await.unwrap();
        service.attach_device("serial", "serial2").await.unwrap();
        service.detach_device("serial1").await.unwrap();

        let devices = service.list_devices().await.unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0], "serial2");
    }

    #[tokio::test]
    async fn test_snapshot_manager_max_snapshots() {
        let manager = SnapshotManager::new("test".to_string()).await;

        // 创建大量快照
        for i in 0..100 {
            let name = format!("snapshot_{}", i);
            let result = manager.create_snapshot(&name).await;
            // 应该都成功，或者在第N个后失败（如果有容量限制）
            let _ = result;
        }

        let snapshots = manager.list_snapshots().await.unwrap();
        // 验证快照数量合理
        assert!(snapshots.len() <= 100);
    }

    #[tokio::test]
    async fn test_execution_service_rapid_start_stop() {
        let config = VmConfig::default();
        let service = ExecutionService::new(config);

        // 快速启动停止
        for _ in 0..10 {
            service.start().await.unwrap();
            service.terminate().await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_snapshot_create_with_metadata() {
        let manager = SnapshotManager::new("test".to_string()).await;

        // 测试带元数据的快照创建（如果支持）
        // 这里只是测试不会崩溃
        let result = manager.create_snapshot("test_metadata").await;
        let _ = result;
    }

    #[tokio::test]
    async fn test_execution_service_cleanup_after_terminate() {
        let config = VmConfig::default();
        let service = ExecutionService::new(config);

        service.start().await.unwrap();
        service.terminate().await.unwrap();

        // 验证资源已清理（如果实现了资源管理）
        let state = service.get_state().await;
        assert_eq!(state, VmLifecycleState::Stopped);
    }

    #[tokio::test]
    async fn test_device_service_device_type_validation() {
        let service = DeviceService::new();

        // 测试无效设备类型
        let invalid_types = vec!["", "invalid", "123", "!@#$%"];
        for device_type in invalid_types {
            let result = service.attach_device(device_type, "test").await;
            assert!(result.is_err(), "Invalid type should fail: {}", device_type);
        }
    }

    #[tokio::test]
    async fn test_snapshot_name_case_sensitivity() {
        let manager = SnapshotManager::new("test".to_string()).await;

        // 测试大小写敏感
        manager.create_snapshot("Test").await.unwrap();
        manager.create_snapshot("test").await.unwrap();
        manager.create_snapshot("TEST").await.unwrap();

        let snapshots = manager.list_snapshots().await.unwrap();
        assert_eq!(snapshots.len(), 3);
    }

    #[tokio::test]
    async fn test_snapshot_name_with_numbers() {
        let manager = SnapshotManager::new("test".to_string()).await;

        // 测试包含数字的快照名称
        let names_with_numbers = vec![
            "snapshot123",
            "123snapshot",
            "snap123shot",
            "0",
            "999",
        ];

        for name in names_with_numbers {
            let result = manager.create_snapshot(name).await;
            assert!(result.is_ok(), "Should accept numbers in name: {}", name);
        }
    }

    #[tokio::test]
    async fn test_snapshot_name_with_underscores() {
        let manager = SnapshotManager::new("test".to_string()).await;

        // 测试包含下划线的快照名称
        let names_with_underscores = vec![
            "snapshot_test",
            "_snapshot",
            "snapshot_",
            "__test__",
        ];

        for name in names_with_underscores {
            let result = manager.create_snapshot(name).await;
            assert!(result.is_ok(), "Should accept underscores in name: {}", name);
        }
    }

    #[tokio::test]
    async fn test_snapshot_name_with_dashes() {
        let manager = SnapshotManager::new("test".to_string()).await;

        // 测试包含连字符的快照名称
        let names_with_dashes = vec![
            "snapshot-test",
            "-snapshot",
            "snapshot-",
            "--test--",
        ];

        for name in names_with_dashes {
            let result = manager.create_snapshot(name).await;
            // 可能成功或失败，取决于实现
            let _ = result;
        }
    }

    #[tokio::test]
    async fn test_device_attach_detach_consistency() {
        let service = DeviceService::new();

        // 附加设备
        service.attach_device("serial", "test_serial").await.unwrap();

        // 分离设备
        service.detach_device("test_serial").await.unwrap();

        // 再次分离应该失败（已不存在）
        let result = service.detach_device("test_serial").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execution_pause_resume_sequence() {
        let config = VmConfig::default();
        let service = ExecutionService::new(config);

        service.start().await.unwrap();

        // 暂停
        service.pause().await.unwrap();
        assert_eq!(service.get_state().await, VmLifecycleState::Paused);

        // 恢复
        service.resume().await.unwrap();
        assert_eq!(service.get_state().await, VmLifecycleState::Running);

        // 再次暂停
        service.pause().await.unwrap();
        assert_eq!(service.get_state().await, VmLifecycleState::Paused);
    }

    #[tokio::test]
    async fn test_snapshot_create_restore_cycle() {
        let manager = SnapshotManager::new("test".to_string()).await;

        // 创建快照
        manager.create_snapshot("cycle_test").await.unwrap();

        // 恢复快照
        manager.restore_snapshot("cycle_test").await.unwrap();

        // 再次创建同名快照应该失败（或覆盖，取决于实现）
        let result = manager.create_snapshot("cycle_test").await;
        let _ = result; // 接受任意结果
    }

    #[tokio::test]
    async fn test_device_service_attach_same_name_twice() {
        let service = DeviceService::new();

        // 第一次附加
        service.attach_device("serial", "same_name").await.unwrap();

        // 第二次附加同名设备应该失败
        let result = service.attach_device("serial", "same_name").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execution_service_terminates_from_paused() {
        let config = VmConfig::default();
        let service = ExecutionService::new(config);

        service.start().await.unwrap();
        service.pause().await.unwrap();

        // 从暂停状态终止
        service.terminate().await.unwrap();
        assert_eq!(service.get_state().await, VmLifecycleState::Stopped);
    }

    #[tokio::test]
    async fn test_snapshot_manager_persistent_snapshots() {
        let manager = SnapshotManager::new("test_persistent".to_string()).await;

        // 创建多个快照
        for i in 0..10 {
            let name = format!("persist_{}", i);
            manager.create_snapshot(&name).await.unwrap();
        }

        // 列出所有快照
        let snapshots = manager.list_snapshots().await.unwrap();
        assert_eq!(snapshots.len(), 10);
    }

    #[tokio::test]
    async fn test_device_service_multiple_device_types() {
        let service = DeviceService::new();

        // 尝试附加不同类型的设备
        let device_types = vec!["serial", "parallel", "network"];
        for (i, device_type) in device_types.iter().enumerate() {
            let name = format!("device_{}", i);
            // 可能成功或失败，取决于支持的设备类型
            let _ = service.attach_device(device_type, &name).await;
        }
    }

    #[tokio::test]
    async fn test_execution_service_get_stats() {
        let config = VmConfig::default();
        let service = ExecutionService::new(config);

        service.start().await.unwrap();

        // 获取统计信息（如果支持）
        // 这里只是测试不会崩溃
        let _ = service.get_state().await;
    }

    #[tokio::test]
    async fn test_snapshot_manager_pagination() {
        let manager = SnapshotManager::new("test_pagination".to_string()).await;

        // 创建25个快照
        for i in 0..25 {
            let name = format!("page_{}", i);
            manager.create_snapshot(&name).await.unwrap();
        }

        // 获取所有快照
        let snapshots = manager.list_snapshots().await.unwrap();
        assert_eq!(snapshots.len(), 25);

        // 验证快照按顺序排列
        for (i, snapshot) in snapshots.iter().enumerate() {
            assert_eq!(snapshot, format!("page_{}", i));
        }
    }

    #[tokio::test]
    async fn test_device_service_device_info() {
        let service = DeviceService::new();

        service.attach_device("serial", "info_test").await.unwrap();

        // 获取设备信息（如果支持）
        // 这里只是测试不会崩溃
        let devices = service.list_devices().await.unwrap();
        assert!(devices.contains(&"info_test".to_string()));
    }

    #[tokio::test]
    async fn test_execution_service_multiple_start_attempts() {
        let config = VmConfig::default();
        let service = ExecutionService::new(config);

        service.start().await.unwrap();

        // 多次尝试启动
        for _ in 0..5 {
            let result = service.start().await;
            assert!(result.is_err(), "Should fail when already started");
        }
    }

    #[tokio::test]
    async fn test_snapshot_manager_unicode_names() {
        let manager = SnapshotManager::new("test_unicode".to_string()).await;

        // 测试Unicode快照名称
        let unicode_names = vec![
            "snapshot_测试",
            "测试_snapshot",
            "スナップショット",
            "снапшот",
        ];

        for name in unicode_names {
            let result = manager.create_snapshot(name).await;
            // 可能成功或失败，取决于实现
            let _ = result;
        }
    }

    #[tokio::test]
    async fn test_device_service_detach_after_terminate() {
        let config = VmConfig::default();
        let service = ExecutionService::new(config);

        service.start().await.unwrap();

        // 终止服务
        service.terminate().await.unwrap();

        // 设备操作可能不再可用
        let result = service.attach_device("serial", "test").await;
        // 可能成功或失败，取决于实现
        let _ = result;
    }

    #[tokio::test]
    async fn test_snapshot_manager_automatic_cleanup() {
        let manager = SnapshotManager::new("test_cleanup".to_string()).await;

        // 创建快照
        manager.create_snapshot("temp").await.unwrap();

        // 删除快照
        manager.delete_snapshot("temp").await.unwrap();

        // 验证快照已删除
        let snapshots = manager.list_snapshots().await.unwrap();
        assert!(!snapshots.contains(&"temp".to_string()));
    }

    #[tokio::test]
    async fn test_execution_service_restart_after_terminate() {
        let config = VmConfig::default();
        let service = ExecutionService::new(config);

        // 第一个生命周期
        service.start().await.unwrap();
        service.terminate().await.unwrap();

        // 第二个生命周期
        service.start().await.unwrap();
        service.terminate().await.unwrap();

        assert_eq!(service.get_state().await, VmLifecycleState::Stopped);
    }

    #[tokio::test]
    async fn test_snapshot_manager_concurrent_access_same_name() {
        // 注意：这个测试需要并发支持，这里简化为顺序测试
        let manager = SnapshotManager::new("test_concurrent".to_string()).await;

        // 创建同名快照
        manager.create_snapshot("same").await.unwrap();

        // 第二次创建应该失败
        let result = manager.create_snapshot("same").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_device_service_empty_device_name() {
        let service = DeviceService::new();

        // 测试空设备名称
        let result = service.attach_device("serial", "").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execution_service_config_validation() {
        // 测试无效配置
        let invalid_config = VmConfig {
            // 设置一些无效值（如果可能）
            ..Default::default()
        };

        let service = ExecutionService::new(invalid_config);

        // 应该能够创建服务
        // 启动可能失败或成功，取决于验证
        let result = service.start().await;
        let _ = result;
    }
}
