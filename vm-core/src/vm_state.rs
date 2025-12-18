//! 虚拟机状态数据结构
//!
//! 定义虚拟机的纯数据结构，符合DDD贫血模型原则。
//! 所有业务逻辑应位于服务层（VirtualMachineService）。

use crate::snapshot;
use crate::template;
use crate::{ExecStats, ExecutionEngine, MMU, VmConfig, VmState};
use std::sync::{Arc, Mutex};

/// 虚拟机状态容器
///
/// 这是一个纯数据结构，仅包含状态数据，不包含业务逻辑。
/// 所有业务操作应通过 VirtualMachineService 进行。
#[cfg(not(feature = "no_std"))]
pub struct VirtualMachineState<B> {
    /// 配置
    pub config: VmConfig,
    /// 状态
    pub state: VmState,
    /// MMU（共享访问）
    pub mmu: Arc<Mutex<Box<dyn MMU>>>,
    /// vCPU 列表
    pub vcpus: Vec<Arc<Mutex<dyn ExecutionEngine<B>>>>,
    /// 执行统计
    pub stats: ExecStats,
    /// 快照管理器
    pub snapshot_manager: Arc<Mutex<snapshot::SnapshotMetadataManager>>,
    /// 模板管理器
    pub template_manager: Arc<Mutex<template::TemplateManager>>,
}

#[cfg(not(feature = "no_std"))]
impl<B: 'static> VirtualMachineState<B> {
    /// 创建新的虚拟机状态
    pub fn new(config: VmConfig, mmu: Box<dyn MMU>) -> Self {
        Self {
            config,
            state: VmState::Created,
            mmu: Arc::new(Mutex::new(mmu)),
            vcpus: Vec::new(),
            stats: ExecStats::default(),
            snapshot_manager: Arc::new(Mutex::new(snapshot::SnapshotMetadataManager::new())),
            template_manager: Arc::new(Mutex::new(template::TemplateManager::new())),
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

    /// 获取 VM 状态
    pub fn state(&self) -> VmState {
        self.state
    }

    /// 设置 VM 状态
    pub fn set_state(&mut self, state: VmState) {
        self.state = state;
    }

    /// 获取执行统计
    pub fn stats(&self) -> &ExecStats {
        &self.stats
    }

    /// 获取快照管理器
    pub fn snapshot_manager(&self) -> Arc<Mutex<snapshot::SnapshotMetadataManager>> {
        Arc::clone(&self.snapshot_manager)
    }

    /// 获取模板管理器
    pub fn template_manager(&self) -> Arc<Mutex<template::TemplateManager>> {
        Arc::clone(&self.template_manager)
    }
}
