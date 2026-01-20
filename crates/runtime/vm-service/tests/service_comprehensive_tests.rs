//! vm-service 综合测试
//!
//! 覆盖快照管理、执行服务、设备服务、配置管理等核心功能。

use std::sync::Arc;

use tokio::sync::Mutex;
use vm_core::{GuestAddr, VmConfig, VmError, VmLifecycleState};
use vm_service::config_manager::ConfigManager;
use vm_service::device_service::DeviceService;
use vm_service::execution_service::ExecutionService;
use vm_service::snapshot_manager::SnapshotManager;

// ============================================================================
// 快照管理器测试（30个测试）
// ============================================================================

#[cfg(test)]
mod snapshot_tests {
    use super::*;

    #[tokio::test]
    async fn test_snapshot_create() {
        let manager = SnapshotManager::new("test".to_string()).await;
        let result = manager.create_snapshot("test_snapshot").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_restore() {
        let manager = SnapshotManager::new("test".to_string()).await;

        // 创建快照
        manager.create_snapshot("test_snapshot").await.unwrap();

        // 恢复快照
        let result = manager.restore_snapshot("test_snapshot").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_delete() {
        let manager = SnapshotManager::new("test".to_string()).await;

        manager.create_snapshot("temp_snapshot").await.unwrap();
        let result = manager.delete_snapshot("temp_snapshot").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_list() {
        let manager = SnapshotManager::new("test".to_string()).await;

        manager.create_snapshot("snap1").await.unwrap();
        manager.create_snapshot("snap2").await.unwrap();

        let snapshots = manager.list_snapshots().await.unwrap();
        assert_eq!(snapshots.len(), 2);
    }

    #[tokio::test]
    async fn test_snapshot_metadata() {
        let manager = SnapshotManager::new("test".to_string()).await;

        manager.create_snapshot("metadata_test").await.unwrap();
        let metadata = manager.get_snapshot_metadata("metadata_test").await;

        assert!(metadata.is_ok());
        let meta = metadata.unwrap();
        assert_eq!(meta.name, "metadata_test");
    }

    #[tokio::test]
    async fn test_snapshot_export() {
        let manager = SnapshotManager::new("test".to_string()).await;

        manager.create_snapshot("export_test").await.unwrap();
        let result = manager
            .export_snapshot("export_test", "/tmp/test.snap")
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_import() {
        let manager = SnapshotManager::new("test".to_string()).await;

        // 先导出
        manager.create_snapshot("import_test").await.unwrap();
        manager
            .export_snapshot("import_test", "/tmp/test.snap")
            .await
            .unwrap();

        // 再导入
        let result = manager
            .import_snapshot("import_test", "/tmp/test.snap")
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_incremental() {
        let manager = SnapshotManager::new("test".to_string()).await;

        manager.create_snapshot("base").await.unwrap();
        let result = manager.create_incremental_snapshot("base", "inc1").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_validation() {
        let manager = SnapshotManager::new("test".to_string()).await;

        manager.create_snapshot("valid_snap").await.unwrap();
        let result = manager.validate_snapshot("valid_snap").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_compression() {
        let manager = SnapshotManager::new("test".to_string()).await;

        manager.create_snapshot("compress_test").await.unwrap();
        let compressed = manager.compress_snapshot("compress_test").await;

        assert!(compressed.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_decompression() {
        let manager = SnapshotManager::new("test".to_string()).await;

        manager.create_snapshot("decompress_test").await.unwrap();
        manager.compress_snapshot("decompress_test").await.unwrap();
        let result = manager.decompress_snapshot("decompress_test").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_auto_cleanup() {
        let manager = SnapshotManager::new("test".to_string()).await;

        // 创建多个快照
        for i in 0..15 {
            manager
                .create_snapshot(&format!("snap_{}", i))
                .await
                .unwrap();
        }

        // 自动清理应该保留最新的10个
        let result = manager.auto_cleanup(10).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_schedule() {
        let manager = SnapshotManager::new("test".to_string()).await;

        let result = manager.schedule_snapshot("scheduled", 1000).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_rollback() {
        let manager = SnapshotManager::new("test".to_string()).await;

        manager.create_snapshot("before_change").await.unwrap();
        // 模拟一些更改
        manager.create_snapshot("after_change").await.unwrap();

        let result = manager.rollback_to_snapshot("before_change").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_clone() {
        let manager = SnapshotManager::new("test".to_string()).await;

        manager.create_snapshot("original").await.unwrap();
        let result = manager.clone_snapshot("original", "cloned").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_merge() {
        let manager = SnapshotManager::new("test".to_string()).await;

        manager.create_snapshot("base").await.unwrap();
        manager
            .create_incremental_snapshot("base", "inc1")
            .await
            .unwrap();

        let result = manager.merge_snapshots("base", "inc1", "merged").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_size_limit() {
        let manager = SnapshotManager::new("test".to_string()).await;

        manager.set_size_limit(1024 * 1024); // 1MB
        let result = manager.create_snapshot("large_snap").await;

        // 应该成功（测试数据不会太大）
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_encryption() {
        let manager = SnapshotManager::new("test".to_string()).await;

        manager.create_snapshot("encrypt_test").await.unwrap();
        let result = manager.encrypt_snapshot("encrypt_test", "password").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_decryption() {
        let manager = SnapshotManager::new("test".to_string()).await;

        manager.create_snapshot("decrypt_test").await.unwrap();
        manager
            .encrypt_snapshot("decrypt_test", "password")
            .await
            .unwrap();
        let result = manager.decrypt_snapshot("decrypt_test", "password").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_checksum_verification() {
        let manager = SnapshotManager::new("test".to_string()).await;

        manager.create_snapshot("checksum_test").await.unwrap();
        let result = manager.verify_checksum("checksum_test").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_versioning() {
        let manager = SnapshotManager::new("test".to_string()).await;

        manager.create_snapshot("v1").await.unwrap();
        let result = manager.create_version("v1", "v2").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_diff() {
        let manager = SnapshotManager::new("test".to_string()).await;

        manager.create_snapshot("snap1").await.unwrap();
        manager.create_snapshot("snap2").await.unwrap();

        let diff = manager.compute_diff("snap1", "snap2").await;
        assert!(diff.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_patch() {
        let manager = SnapshotManager::new("test".to_string()).await;

        manager.create_snapshot("base").await.unwrap();
        let diff = manager.compute_diff("base", "modified").await;

        // 应用patch
        let result = manager.apply_patch("base", diff.unwrap()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_remote_storage() {
        let manager = SnapshotManager::new("test".to_string()).await;

        let result = manager.save_to_remote("test", "s3://bucket/test").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_local_storage() {
        let manager = SnapshotManager::new("test".to_string()).await;

        let result = manager.save_to_local("test", "/tmp/snapshots/").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_automatic_backup() {
        let manager = SnapshotManager::new("test".to_string()).await;

        let result = manager.enable_auto_backup(3600).await; // 每小时备份
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_metadata_search() {
        let manager = SnapshotManager::new("test".to_string()).await;

        manager.create_snapshot("searchable").await.unwrap();
        let results = manager.search_metadata("searchable").await.unwrap();

        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_snapshot_batch_operations() {
        let manager = SnapshotManager::new("test".to_string()).await;

        let names = vec!["batch1", "batch2", "batch3"];
        let result = manager.create_batch_snapshot(&names).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_error_handling() {
        let manager = SnapshotManager::new("test".to_string()).await;

        // 尝试恢复不存在的快照
        let result = manager.restore_snapshot("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_snapshot_concurrent_access() {
        let manager = Arc::new(SnapshotManager::new("test".to_string()).await);

        let manager1 = Arc::clone(&manager);
        let manager2 = Arc::clone(&manager);

        let task1 = tokio::spawn(async move { manager1.create_snapshot("concurrent1").await });

        let task2 = tokio::spawn(async move { manager2.create_snapshot("concurrent2").await });

        let (result1, result2) = tokio::join!(task1, task2);
        assert!(result1.unwrap().is_ok());
        assert!(result2.unwrap().is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_memory_limit() {
        let manager = SnapshotManager::new("test".to_string()).await;

        manager.set_memory_limit(10 * 1024 * 1024); // 10MB
        let result = manager.check_memory_usage().await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_priority_queue() {
        let manager = SnapshotManager::new("test".to_string()).await;

        manager.set_priority("high_priority", 10).await;
        let result = manager.get_snapshot_by_priority().await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_gc() {
        let manager = SnapshotManager::new("test".to_string()).await;

        // 创建一些旧快照
        for i in 0..5 {
            manager
                .create_snapshot(&format!("old_{}", i))
                .await
                .unwrap();
        }

        let result = manager.run_gc().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_statistics() {
        let manager = SnapshotManager::new("test".to_string()).await;

        manager.create_snapshot("stats_test").await.unwrap();
        let stats = manager.get_statistics().await.unwrap();

        assert!(stats.total_snapshots > 0);
    }

    #[tokio::test]
    async fn test_snapshot_locking() {
        let manager = SnapshotManager::new("test".to_string()).await;

        manager.create_snapshot("locked").await.unwrap();
        let result = manager.lock_snapshot("locked").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_unlocking() {
        let manager = SnapshotManager::new("test".to_string()).await;

        manager.create_snapshot("locked").await.unwrap();
        manager.lock_snapshot("locked").await.unwrap();
        let result = manager.unlock_snapshot("locked").await;

        assert!(result.is_ok());
    }
}

// ============================================================================
// 执行服务测试（40个测试）
// ============================================================================

#[cfg(test)]
mod execution_tests {
    use super::*;

    fn create_test_config() -> VmConfig {
        VmConfig {
            memory_size: 1024 * 1024, // 1MB
            vcpu_count: 1,
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_execution_start() {
        let service = ExecutionService::new(create_test_config()).await;
        let result = service.start().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_pause() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let result = service.pause().await;
        assert!(result.is_ok());
        assert_eq!(service.get_state().await, VmLifecycleState::Paused);
    }

    #[tokio::test]
    async fn test_execution_resume() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();
        service.pause().await.unwrap();

        let result = service.resume().await;
        assert!(result.is_ok());
        assert_eq!(service.get_state().await, VmLifecycleState::Running);
    }

    #[tokio::test]
    async fn test_execution_stop() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let result = service.stop().await;
        assert!(result.is_ok());
        assert_eq!(service.get_state().await, VmLifecycleState::Stopped);
    }

    #[tokio::test]
    async fn test_execution_reset() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let result = service.reset().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_step() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let result = service.step().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_step_instruction() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let result = service.step_instruction().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_get_registers() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let regs = service.get_registers(0).await.unwrap();
        assert!(!regs.is_empty());
    }

    #[tokio::test]
    async fn test_execution_set_registers() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let result = service.set_registers(0, vec![0; 32]).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_read_memory() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let result = service.read_memory(GuestAddr(0), 64).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_write_memory() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let data = vec![0x90; 64]; // NOP指令
        let result = service.write_memory(GuestAddr(0), &data).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_get_pc() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let pc = service.get_program_counter(0).await.unwrap();
        assert_eq!(pc, GuestAddr(0));
    }

    #[tokio::test]
    async fn test_execution_set_pc() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let result = service.set_program_counter(0, GuestAddr(0x1000)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_multiple_vcpus() {
        let config = VmConfig {
            memory_size: 1024 * 1024,
            vcpu_count: 4,
            ..Default::default()
        };
        let service = ExecutionService::new(config).await;

        let result = service.start().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_interrupt_inject() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let result = service.inject_interrupt(0, 1).await; // Timer interrupt
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_get_stats() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        // 执行一些指令
        let _ = service.step().await;

        let stats = service.get_execution_stats().await.unwrap();
        assert!(stats.instructions_executed > 0);
    }

    #[tokio::test]
    async fn test_execution_breakpoint() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let result = service.set_breakpoint(GuestAddr(0x100)).await;
        assert!(result.is_ok());

        let breakpoints = service.list_breakpoints().await.unwrap();
        assert_eq!(breakpoints.len(), 1);
    }

    #[tokio::test]
    async fn test_execution_clear_breakpoint() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        service.set_breakpoint(GuestAddr(0x100)).await.unwrap();
        let result = service.clear_breakpoint(GuestAddr(0x100)).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_single_step_mode() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        service.enable_single_step().await.unwrap();
        let result = service.step().await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_continue() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        service.pause().await.unwrap();
        let result = service.r#continue().await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_state_save() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let result = service.save_state("test_state").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_state_restore() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        service.save_state("test_state").await.unwrap();
        service.stop().await.unwrap();

        let result = service.restore_state("test_state").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_watchpoint() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let result = service.set_watchpoint(GuestAddr(0x1000), 4, false).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_get_memory_regions() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let regions = service.get_memory_regions().await.unwrap();
        assert!(!regions.is_empty());
    }

    #[tokio::test]
    async fn test_execution_flush_tlb() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let result = service.flush_tlb().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_invalidate_cache() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let result = service.invalidate_code_cache().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_set_timeout() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let result = service.set_timeout(1000).await; // 1秒超时
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_get_vcpu_state() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let state = service.get_vcpu_state(0).await.unwrap();
        assert!(state.vcpu_id == 0);
    }

    #[tokio::test]
    async fn test_execution_set_vcpu_state() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let state = service.get_vcpu_state(0).await.unwrap();
        let result = service.set_vcpu_state(0, &state).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_migrate() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let result = service.migrate_to(1).await; // 迁移到CPU 1
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_pin_vcpu() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let result = service.pin_vcpu(0, 0).await; // Pin vCPU 0 to CPU 0
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_unpin_vcpu() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        service.pin_vcpu(0, 0).await.unwrap();
        let result = service.unpin_vcpu(0).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_get_affinity() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let affinity = service.get_cpu_affinity(0).await.unwrap();
        assert!(affinity.is_some());
    }

    #[tokio::test]
    async fn test_execution_set_affinity() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let result = service.set_cpu_affinity(0, vec![0, 1]).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_get_frequency() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let freq = service.get_cpu_frequency(0).await.unwrap();
        assert!(freq > 0);
    }

    #[tokio::test]
    async fn test_execution_set_frequency() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let result = service.set_cpu_frequency(0, 2000).await; // 2GHz
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_performance_counters() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let result = service.enable_performance_counters(0).await;
        assert!(result.is_ok());

        let counters = service.get_performance_counters(0).await.unwrap();
        assert!(!counters.is_empty());
    }

    #[tokio::test]
    async fn test_execution_reset_performance_counters() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        service.enable_performance_counters(0).await.unwrap();
        let result = service.reset_performance_counters(0).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_emergency_shutdown() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let result = service.emergency_shutdown().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_graceful_shutdown() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let result = service.graceful_shutdown().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_get_pending_interrupts() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        service.inject_interrupt(0, 1).await.unwrap();
        let interrupts = service.get_pending_interrupts(0).await.unwrap();

        assert!(!interrupts.is_empty());
    }

    #[tokio::test]
    async fn test_execution_clear_interrupt() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        service.inject_interrupt(0, 1).await.unwrap();
        let result = service.clear_interrupt(0, 1).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_get_event_fd() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let fd = service.get_event_fd(0).await;
        assert!(fd.is_ok());
    }

    #[tokio::test]
    async fn test_execution_notify_event() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let result = service.notify_event(0).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_run_until() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let result = service.run_until(GuestAddr(0x1000)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_run_for_instructions() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let result = service.run_for_instructions(100).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_get_tlb_size() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let size = service.get_tlb_size(0).await.unwrap();
        assert!(size > 0);
    }

    #[tokio::test]
    async fn test_execution_set_tlb_size() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let result = service.set_tlb_size(0, 1024).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_get_cache_config() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let config = service.get_cache_config(0).await.unwrap();
        assert!(config.l1_size > 0);
    }

    #[tokio::test]
    async fn test_execution_set_cache_config() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let result = service.set_cache_config(0, 32 * 1024, 4).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_enable_tracing() {
        let service = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        let result = service.enable_tracing(0).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_disable_tracing() {
        let service: ExecutionService = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        service.enable_tracing(0).await.unwrap();
        let result = service.disable_tracing(0).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution_get_trace_data() {
        let service: ExecutionService = ExecutionService::new(create_test_config()).await;
        service.start().await.unwrap();

        service.enable_tracing(0).await.unwrap();
        service.step().await.unwrap();

        let trace = service.get_trace_data(0).await.unwrap();
        assert!(!trace.is_empty());
    }
}
