//! 虚拟机状态数据结构
//!
//! 定义虚拟机的纯数据结构，符合DDD贫血模型原则。
//! 所有业务逻辑应位于服务层（VirtualMachineService）。

use crate::snapshot::SnapshotMetadataManager;
use crate::template::TemplateManager;
use crate::{ExecStats, ExecutionEngine, MMU, VmConfig, VmLifecycleState};
use std::sync::{Arc, Mutex};

/// 虚拟机状态容器
///
/// 这是一个纯数据结构，仅包含状态数据，不包含业务逻辑。
/// 所有业务操作应通过 VirtualMachineService 进行。
pub struct VirtualMachineState<B> {
    /// 配置
    pub config: VmConfig,
    /// 生命周期状态
    pub state: VmLifecycleState,
    /// MMU（共享访问）
    pub mmu: Arc<Mutex<Box<dyn MMU>>>,
    /// vCPU 列表
    pub vcpus: Vec<Arc<Mutex<dyn ExecutionEngine<B>>>>,
    /// 执行统计
    pub stats: ExecStats,
    /// 快照管理器
    pub snapshot_manager: Arc<Mutex<SnapshotMetadataManager>>,
    /// 模板管理器
    pub template_manager: Arc<Mutex<TemplateManager>>,
}

impl<B: 'static> VirtualMachineState<B> {
    /// 创建新的虚拟机状态
    pub fn new(config: VmConfig, mmu: Box<dyn MMU>) -> Self {
        Self {
            config,
            state: VmLifecycleState::Created,
            mmu: Arc::new(Mutex::new(mmu)),
            vcpus: Vec::new(),
            stats: ExecStats::default(),
            snapshot_manager: Arc::new(Mutex::new(SnapshotMetadataManager::new())),
            template_manager: Arc::new(Mutex::new(TemplateManager::new())),
        }
    }

    /// 添加 vCPU
    pub fn add_vcpu(&mut self, vcpu: Arc<Mutex<dyn ExecutionEngine<B>>>) {
        self.vcpus.push(vcpu);
    }

    /// 获取 MMU 引用
    pub fn mmu(&self) -> Arc<Mutex<Box<dyn MMU>>> {
        Arc::clone(&self.mmu)
    }

    /// 获取配置
    pub fn config(&self) -> &VmConfig {
        &self.config
    }

    /// 获取 VM 生命周期状态
    pub fn state(&self) -> VmLifecycleState {
        self.state.clone()
    }

    /// 设置 VM 生命周期状态
    pub fn set_state(&mut self, state: VmLifecycleState) {
        self.state = state;
    }

    /// 获取执行统计
    pub fn stats(&self) -> &ExecStats {
        &self.stats
    }

    /// 获取快照管理器
    pub fn snapshot_manager(&self) -> Arc<Mutex<SnapshotMetadataManager>> {
        Arc::clone(&self.snapshot_manager)
    }

    // /// 获取模板管理器
    // pub fn template_manager(&self) -> Arc<Mutex<template::TemplateManager>> {
    //     Arc::clone(&self.template_manager)
    // }
}

impl<B: 'static> Clone for VirtualMachineState<B> {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            state: self.state.clone(),
            mmu: Arc::clone(&self.mmu),
            vcpus: self.vcpus.clone(),
            stats: self.stats.clone(),
            snapshot_manager: Arc::clone(&self.snapshot_manager),
            template_manager: Arc::clone(&self.template_manager),
        }
    }
}

/// ============================================================================
/// 测试模块
/// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_lifecycle_state_default() {
        // Test that VmLifecycleState has the expected variants
        let _created = VmLifecycleState::Created;
        let _running = VmLifecycleState::Running;
        let _paused = VmLifecycleState::Paused;
        let _stopped = VmLifecycleState::Stopped;
    }

    #[test]
    fn test_vm_lifecycle_state_clone() {
        let state = VmLifecycleState::Running;
        let cloned = state.clone();
        assert_eq!(state, cloned);
    }

    #[test]
    fn test_vm_lifecycle_state_partial_eq() {
        assert_eq!(VmLifecycleState::Created, VmLifecycleState::Created);
        assert_eq!(VmLifecycleState::Running, VmLifecycleState::Running);
        assert_ne!(VmLifecycleState::Created, VmLifecycleState::Running);
    }

    #[test]
    fn test_exec_stats_default() {
        let stats = ExecStats::default();
        assert_eq!(stats.executed_ops, 0);
        assert_eq!(stats.executed_insns, 0);
        assert_eq!(stats.mem_accesses, 0);
        assert_eq!(stats.exec_time_ns, 0);
        assert_eq!(stats.tlb_hits, 0);
        assert_eq!(stats.tlb_misses, 0);
    }

    #[test]
    fn test_exec_stats_clone() {
        let mut stats = ExecStats::default();
        stats.executed_insns = 100;
        stats.mem_accesses = 50;

        let cloned = stats.clone();
        assert_eq!(cloned.executed_insns, 100);
        assert_eq!(cloned.mem_accesses, 50);
    }

    #[test]
    fn test_snapshot_metadata_manager_in_state() {
        // Test that snapshot manager is correctly initialized
        let manager = SnapshotMetadataManager::new();
        assert_eq!(manager.snapshots.len(), 0);
        assert!(manager.current_snapshot.is_none());
    }

    #[test]
    fn test_template_manager_in_state() {
        // Test that template manager is correctly initialized
        let manager = TemplateManager::new();
        assert_eq!(manager.templates.len(), 0);
    }

    #[test]
    fn test_managers_default() {
        // Test Default trait for managers
        let snapshot_mgr = SnapshotMetadataManager::default();
        assert_eq!(snapshot_mgr.snapshots.len(), 0);

        let template_mgr = TemplateManager::default();
        assert_eq!(template_mgr.templates.len(), 0);
    }

    #[test]
    fn test_managers_clone() {
        let mut snapshot_mgr = SnapshotMetadataManager::new();
        snapshot_mgr.snapshots.insert(
            "test".to_string(),
            crate::snapshot::Snapshot {
                id: "test".to_string(),
                parent_id: None,
                name: "Test".to_string(),
                description: "Test snapshot".to_string(),
                memory_dump_path: "/test".to_string(),
            },
        );

        let cloned = snapshot_mgr.clone();
        assert_eq!(cloned.snapshots.len(), 1);

        let mut template_mgr = TemplateManager::new();
        let id = template_mgr.create_template(
            "Test".to_string(),
            "Description".to_string(),
            "snapshot-1".to_string(),
        );

        let cloned = template_mgr.clone();
        assert!(cloned.get_template(&id).is_some());
    }
}
