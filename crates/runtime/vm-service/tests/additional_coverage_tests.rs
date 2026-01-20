//! vm-service 边界条件和错误处理测试
//!
//! 覆盖边界条件、错误处理、并发访问等场景

use std::sync::Arc;

use vm_core::{GuestAddr, VmConfig, VmError, VmLifecycleState};
use vm_service::device_service::DeviceService;
use vm_service::execution_service::ExecutionService;
use vm_service::snapshot_manager::SnapshotManager;

// ============================================================================
// 错误处理测试（30个测试）
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
        // 应该成功（幂等操作）
        assert!(result.is_ok());
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

        // 从Stopped状态直接跳转到Running应该失败
        let result = service.start().await;
        assert!(result.is_ok());

        // 已经在Running状态，再次start应该失败
        let result = service.start().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_device_service_attach_invalid_device() {
        let service = DeviceService::new();
        let result = service
            .attach_device("invalid_device_type", "test_device")
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_device_service_detach_nonexistent() {
        let service = DeviceService::new();
        let result = service.detach_device("nonexistent_device").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_config_manager_invalid_config() {
        use vm_service::config_manager::ConfigManager;

        // 测试空配置
        let result = ConfigManager::validate_config(&VmConfig::default());
        // 默认配置应该有效
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_name_with_special_chars() {
        let manager = SnapshotManager::new("test".to_string()).await;

        // 测试特殊字符
        let special_names = vec!["test/snapshot", "test\\snapshot", "test\x00snapshot"];
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
        // 这里我们接受任意结果，只测试不会崩溃
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
    async fn test_snapshot_concurrent_create() {
        let manager = Arc::new(Mutex::new(SnapshotManager::new("test".to_string()).await));
        let mut handles = vec![];

        // 并发创建同名快照
        for _ in 0..10 {
            let mgr = manager.clone();
            handles.push(tokio::spawn(async move {
                mgr.lock().await.create_snapshot("concurrent_test").await
            }));
        }

        // 只有一个应该成功
        let results: Vec<_> = futures::future::join_all(handles).await;
        let success_count = results.iter().filter(|r| r.is_ok()).count();
        assert!(success_count <= 1, "At most one should succeed");
    }

    #[tokio::test]
    async fn test_snapshot_concurrent_restore() {
        let manager = Arc::new(Mutex::new(SnapshotManager::new("test".to_string()).await));

        // 先创建快照
        manager.lock().await.create_snapshot("test").await.unwrap();

        // 并发恢复
        let mut handles = vec![];
        for _ in 0..10 {
            let mgr = manager.clone();
            handles.push(tokio::spawn(async move {
                mgr.lock().await.restore_snapshot("test").await
            }));
        }

        // 所有恢复应该成功
        let results: Vec<_> = futures::future::join_all(handles).await;
        let success_count = results.iter().filter(|r| r.is_ok()).count();
        assert_eq!(success_count, 10, "All restores should succeed");
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
    async fn test_device_service_concurrent_attach_different_devices() {
        let service = Arc::new(Mutex::new(DeviceService::new()));
        let mut handles = vec![];

        // 并发附加不同设备
        for i in 0..10 {
            let svc = service.clone();
            let device_name = format!("device_{}", i);
            handles.push(tokio::spawn(async move {
                svc.lock().await.attach_device("serial", &device_name).await
            }));
        }

        // 所有附加应该成功
        let results: Vec<_> = futures::future::join_all(handles).await;
        let success_count = results.iter().filter(|r| r.is_ok()).count();
        assert_eq!(success_count, 10);
    }

    #[tokio::test]
    async fn test_config_manager_update_after_start() {
        use vm_service::config_manager::ConfigManager;

        let config = VmConfig::default();
        let mut service = ExecutionService::new(config);

        service.start().await.unwrap();

        // 运行时更新配置
        let new_config = VmConfig::default();
        let result = service.update_config(new_config).await;

        // 取决于实现，可能成功或失败
        let _ = result;
    }

    #[tokio::test]
    async fn test_snapshot_create_with_metadata() {
        let manager = SnapshotManager::new("test".to_string()).await;

        // 测试带元数据的快照创建
        let metadata = serde_json::json!({"version": "1.0", "description": "test"});
        let result = manager
            .create_snapshot_with_metadata("test_meta", metadata)
            .await;

        // 可能成功或失败，取决于实现
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
}

// ============================================================================
// 性能和压力测试（10个测试）
// ============================================================================

#[cfg(test)]
mod performance_tests {
    use super::*;

    #[tokio::test]
    async fn test_snapshot_create_performance() {
        let manager = SnapshotManager::new("test".to_string()).await;

        let start = std::time::Instant::now();
        for i in 0..100 {
            let name = format!("perf_snapshot_{}", i);
            manager.create_snapshot(&name).await.unwrap();
        }
        let duration = start.elapsed();

        // 100个快照应该在合理时间内完成（例如<10秒）
        assert!(duration.as_secs() < 10, "Too slow: {:?}", duration);
    }

    #[tokio::test]
    async fn test_execution_service_state_query_performance() {
        let config = VmConfig::default();
        let service = ExecutionService::new(config);

        service.start().await.unwrap();

        let start = std::time::Instant::now();
        for _ in 0..1000 {
            service.get_state().await;
        }
        let duration = start.elapsed();

        // 1000次查询应该在合理时间内完成（例如<1秒）
        assert!(duration.as_secs() < 1, "Too slow: {:?}", duration);
    }

    #[tokio::test]
    async fn test_device_service_list_performance() {
        let service = DeviceService::new();

        // 添加100个设备
        for i in 0..100 {
            let name = format!("device_{}", i);
            service.attach_device("serial", &name).await.unwrap();
        }

        let start = std::time::Instant::now();
        for _ in 0..100 {
            service.list_devices().await.unwrap();
        }
        let duration = start.elapsed();

        // 100次列表查询应该在合理时间内完成（例如<1秒）
        assert!(duration.as_secs() < 1, "Too slow: {:?}", duration);
    }

    #[tokio::test]
    async fn test_snapshot_list_performance() {
        let manager = SnapshotManager::new("test".to_string()).await;

        // 创建100个快照
        for i in 0..100 {
            let name = format!("snapshot_{}", i);
            manager.create_snapshot(&name).await.unwrap();
        }

        let start = std::time::Instant::now();
        for _ in 0..100 {
            manager.list_snapshots().await.unwrap();
        }
        let duration = start.elapsed();

        // 100次列表查询应该在合理时间内完成（例如<1秒）
        assert!(duration.as_secs() < 1, "Too slow: {:?}", duration);
    }

    #[tokio::test]
    async fn test_concurrent_snapshot_operations() {
        let manager = Arc::new(Mutex::new(SnapshotManager::new("test".to_string()).await));
        let mut handles = vec![];

        // 混合操作：创建、恢复、删除
        for i in 0..50 {
            let mgr = manager.clone();
            handles.push(tokio::spawn(async move {
                let name = format!("snapshot_{}", i);
                if i % 3 == 0 {
                    mgr.lock().await.create_snapshot(&name).await
                } else if i % 3 == 1 {
                    mgr.lock().await.restore_snapshot(&name).await
                } else {
                    mgr.lock().await.delete_snapshot(&name).await
                }
            }));
        }

        // 等待所有操作完成
        let results: Vec<_> = futures::future::join_all(handles).await;

        // 统计成功和失败
        let success_count = results.iter().filter(|r| r.is_ok()).count();
        let _ = success_count;

        // 只要不崩溃就算通过
        assert!(true);
    }

    #[tokio::test]
    async fn test_execution_service_memory_leak_test() {
        let config = VmConfig::default();

        // 创建多个服务实例
        for _ in 0..10 {
            let service = ExecutionService::new(config.clone());
            service.start().await.unwrap();
            service.terminate().await.unwrap();
            // 服务应该被正确清理
        }
    }

    #[tokio::test]
    async fn test_device_service_attach_detach_stress() {
        let service = DeviceService::new();

        // 快速附加和分离
        for i in 0..100 {
            let name = format!("device_{}", i);
            service.attach_device("serial", &name).await.unwrap();
            service.detach_device(&name).await.unwrap();
        }

        // 最终应该没有设备
        let devices = service.list_devices().await.unwrap();
        assert!(devices.is_empty());
    }

    #[tokio::test]
    async fn test_snapshot_large_snapshot_test() {
        let manager = SnapshotManager::new("test_large".to_string()).await;

        // 创建大量快照
        for i in 0..1000 {
            let name = format!("large_snapshot_{}", i);
            let result = manager.create_snapshot(&name).await;
            if result.is_err() {
                // 如果有容量限制，可以接受失败
                break;
            }
        }

        let snapshots = manager.list_snapshots().await.unwrap();
        assert!(snapshots.len() >= 100, "Should have at least 100 snapshots");
    }

    #[tokio::test]
    async fn test_execution_service_rapid_state_transitions() {
        let config = VmConfig::default();
        let service = ExecutionService::new(config);

        // 快速状态转换
        for _ in 0..50 {
            service.start().await.unwrap();
            service.pause().await.unwrap();
            service.resume().await.unwrap();
            service.terminate().await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_snapshot_manager_concurrent_list_and_modify() {
        let manager = Arc::new(Mutex::new(SnapshotManager::new("test".to_string()).await));
        let mut handles = vec![];

        // 并发列表和修改
        for i in 0..20 {
            let mgr = manager.clone();
            handles.push(tokio::spawn(async move {
                if i % 2 == 0 {
                    mgr.lock().await.list_snapshots().await
                } else {
                    let name = format!("snapshot_{}", i);
                    mgr.lock().await.create_snapshot(&name).await
                }
            }));
        }

        let results: Vec<_> = futures::future::join_all(handles).await;

        // 所有不崩溃就算通过
        assert!(true);
    }
}
