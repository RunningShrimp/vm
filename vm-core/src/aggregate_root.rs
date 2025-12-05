//! 聚合根实现
//!
//! 将VirtualMachine重构为聚合根，负责维护聚合不变式和发布领域事件。

use crate::domain_event_bus::DomainEventBus;
use crate::domain_events::{DomainEvent, DomainEventEnum, VmConfigSnapshot, VmLifecycleEvent};
use crate::{VmConfig, VmError, VmResult, VmState};
use std::sync::Arc;
use std::time::SystemTime;

/// 聚合根trait
///
/// 所有聚合根都应该实现这个trait，提供事件发布能力。
pub trait AggregateRoot: Send + Sync {
    /// 获取聚合ID
    fn aggregate_id(&self) -> &str;

    /// 获取未提交的事件
    fn uncommitted_events(&self) -> Vec<DomainEventEnum>;

    /// 标记事件为已提交
    fn mark_events_as_committed(&mut self);
}

/// 虚拟机聚合根
///
/// 这是虚拟机的聚合根，负责：
/// - 维护聚合不变式
/// - 发布领域事件
/// - 管理聚合状态
#[cfg(not(feature = "no_std"))]
#[derive(Clone)]
pub struct VirtualMachineAggregate {
    /// 虚拟机ID
    vm_id: String,
    /// 配置
    config: VmConfig,
    /// 当前状态
    state: VmState,
    /// 事件总线（可选，如果为None则使用全局总线）
    event_bus: Option<Arc<DomainEventBus>>,
    /// 未提交的事件
    uncommitted_events: Vec<DomainEventEnum>,
    /// 聚合版本（用于乐观锁）
    version: u64,
}

#[cfg(not(feature = "no_std"))]
impl VirtualMachineAggregate {
    /// 创建新的虚拟机聚合
    pub fn new(vm_id: String, config: VmConfig) -> Self {
        let mut aggregate = Self {
            vm_id: vm_id.clone(),
            config: config.clone(),
            state: VmState::Created,
            event_bus: None,
            uncommitted_events: Vec::new(),
            version: 1,
        };

        // 发布创建事件
        aggregate.record_event(DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmCreated {
            vm_id: vm_id.clone(),
            config: VmConfigSnapshot::from(&config),
            occurred_at: SystemTime::now(),
        }));

        aggregate
    }

    /// 使用指定的事件总线创建
    pub fn with_event_bus(vm_id: String, config: VmConfig, event_bus: Arc<DomainEventBus>) -> Self {
        let mut aggregate = Self {
            vm_id: vm_id.clone(),
            config: config.clone(),
            state: VmState::Created,
            event_bus: Some(event_bus),
            uncommitted_events: Vec::new(),
            version: 1,
        };

        aggregate.record_event(DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmCreated {
            vm_id: vm_id.clone(),
            config: VmConfigSnapshot::from(&config),
            occurred_at: SystemTime::now(),
        }));

        aggregate
    }

    /// 启动虚拟机
    pub fn start(&mut self) -> VmResult<()> {
        // 检查不变式：只能在Created或Paused状态启动
        if self.state != VmState::Created && self.state != VmState::Paused {
            return Err(VmError::Core(crate::CoreError::InvalidState {
                message: format!("Cannot start VM in state {:?}", self.state),
                current: format!("{:?}", self.state),
                expected: "Created or Paused".to_string(),
            }));
        }

        let old_state = self.state;
        self.state = VmState::Running;
        self.version += 1;

        // 发布状态变更事件
        self.record_event(DomainEventEnum::VmLifecycle(
            VmLifecycleEvent::VmStateChanged {
                vm_id: self.vm_id.clone(),
                from: old_state,
                to: self.state,
                occurred_at: SystemTime::now(),
            },
        ));

        // 发布启动事件
        self.record_event(DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmStarted {
            vm_id: self.vm_id.clone(),
            occurred_at: SystemTime::now(),
        }));

        Ok(())
    }

    /// 暂停虚拟机
    pub fn pause(&mut self) -> VmResult<()> {
        if self.state != VmState::Running {
            return Err(VmError::Core(crate::CoreError::InvalidState {
                message: format!("Cannot pause VM in state {:?}", self.state),
                current: format!("{:?}", self.state),
                expected: "Running".to_string(),
            }));
        }

        let old_state = self.state;
        self.state = VmState::Paused;
        self.version += 1;

        self.record_event(DomainEventEnum::VmLifecycle(
            VmLifecycleEvent::VmStateChanged {
                vm_id: self.vm_id.clone(),
                from: old_state,
                to: self.state,
                occurred_at: SystemTime::now(),
            },
        ));

        self.record_event(DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmPaused {
            vm_id: self.vm_id.clone(),
            occurred_at: SystemTime::now(),
        }));

        Ok(())
    }

    /// 恢复虚拟机
    pub fn resume(&mut self) -> VmResult<()> {
        if self.state != VmState::Paused {
            return Err(VmError::Core(crate::CoreError::InvalidState {
                message: format!("Cannot resume VM in state {:?}", self.state),
                current: format!("{:?}", self.state),
                expected: "Paused".to_string(),
            }));
        }

        let old_state = self.state;
        self.state = VmState::Running;
        self.version += 1;

        self.record_event(DomainEventEnum::VmLifecycle(
            VmLifecycleEvent::VmStateChanged {
                vm_id: self.vm_id.clone(),
                from: old_state,
                to: self.state,
                occurred_at: SystemTime::now(),
            },
        ));

        self.record_event(DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmResumed {
            vm_id: self.vm_id.clone(),
            occurred_at: SystemTime::now(),
        }));

        Ok(())
    }

    /// 停止虚拟机
    pub fn stop(&mut self, reason: String) -> VmResult<()> {
        let old_state = self.state;
        self.state = VmState::Stopped;
        self.version += 1;

        self.record_event(DomainEventEnum::VmLifecycle(
            VmLifecycleEvent::VmStateChanged {
                vm_id: self.vm_id.clone(),
                from: old_state,
                to: self.state,
                occurred_at: SystemTime::now(),
            },
        ));

        self.record_event(DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmStopped {
            vm_id: self.vm_id.clone(),
            reason,
            occurred_at: SystemTime::now(),
        }));

        Ok(())
    }

    /// 记录领域事件
    fn record_event(&mut self, event: DomainEventEnum) {
        self.uncommitted_events.push(event);
    }

    /// 提交事件到事件总线
    pub fn commit_events(&mut self) -> VmResult<()> {
        let bus = self
            .event_bus
            .as_ref()
            .map(Arc::clone)
            .unwrap_or_else(|| Arc::new(DomainEventBus::new()));

        for event in &self.uncommitted_events {
            bus.publish(event.clone())?;
        }

        self.mark_events_as_committed();
        Ok(())
    }

    /// 获取虚拟机ID
    pub fn vm_id(&self) -> &str {
        &self.vm_id
    }

    /// 获取当前状态
    pub fn state(&self) -> VmState {
        self.state
    }

    /// 获取配置
    pub fn config(&self) -> &VmConfig {
        &self.config
    }

    /// 获取版本
    pub fn version(&self) -> u64 {
        self.version
    }

    /// 从事件回放重建聚合状态
    ///
    /// 这是事件溯源的核心方法，通过回放历史事件来重建聚合状态。
    /// 这个方法不会发布新事件，只是重建状态。
    ///
    /// # 参数
    /// - `events`: 要回放的事件列表（按序号排序）
    ///
    /// # 返回
    /// 重建后的聚合根
    pub fn from_events(vm_id: String, config: VmConfig, events: Vec<crate::event_store::StoredEvent>) -> Self {
        let mut aggregate = Self {
            vm_id: vm_id.clone(),
            config: config.clone(),
            state: VmState::Created,
            event_bus: None,
            uncommitted_events: Vec::new(),
            version: 0,
        };

        // 回放所有事件
        for stored_event in events {
            aggregate.apply_event(&stored_event.event);
            aggregate.version = stored_event.sequence_number;
        }

        aggregate
    }

    /// 应用事件到聚合状态（内部方法）
    ///
    /// 这个方法用于事件回放，不会记录事件到uncommitted_events。
    fn apply_event(&mut self, event: &DomainEventEnum) {
        match event {
            DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmCreated { config, .. }) => {
                // 从事件中恢复配置（如果需要）
                // 注意：这里假设config已经通过构造函数传入
                self.state = VmState::Created;
            }
            DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmStarted { .. }) => {
                if self.state == VmState::Created || self.state == VmState::Paused {
                    self.state = VmState::Running;
                }
            }
            DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmPaused { .. }) => {
                if self.state == VmState::Running {
                    self.state = VmState::Paused;
                }
            }
            DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmResumed { .. }) => {
                if self.state == VmState::Paused {
                    self.state = VmState::Running;
                }
            }
            DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmStopped { .. }) => {
                self.state = VmState::Stopped;
            }
            DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmStateChanged { to, .. }) => {
                self.state = *to;
            }
            _ => {
                // 其他事件类型不影响聚合状态，可以在这里扩展
            }
        }
    }
}

#[cfg(not(feature = "no_std"))]
impl AggregateRoot for VirtualMachineAggregate {
    fn aggregate_id(&self) -> &str {
        &self.vm_id
    }

    fn uncommitted_events(&self) -> Vec<DomainEventEnum> {
        self.uncommitted_events.clone()
    }

    fn mark_events_as_committed(&mut self) {
        self.uncommitted_events.clear();
    }
}

#[cfg(test)]
#[cfg(not(feature = "no_std"))]
mod tests {
    use super::*;
    use crate::domain_events::{DomainEventEnum, VmLifecycleEvent};
    use crate::{GuestArch, VmConfig};

    #[test]
    fn test_virtual_machine_aggregate_creation() {
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 64 * 1024 * 1024,
            vcpu_count: 1,
            ..Default::default()
        };

        let aggregate = VirtualMachineAggregate::new("test-vm".to_string(), config);
        
        assert_eq!(aggregate.aggregate_id(), "test-vm");
        assert_eq!(aggregate.state, VmState::Created);
        assert_eq!(aggregate.version, 1);
        
        // 应该有一个创建事件
        let events = aggregate.uncommitted_events();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmCreated { .. })));
    }

    #[test]
    fn test_virtual_machine_aggregate_with_event_bus() {
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 64 * 1024 * 1024,
            vcpu_count: 1,
            ..Default::default()
        };

        let event_bus = Arc::new(DomainEventBus::new());
        let aggregate = VirtualMachineAggregate::with_event_bus(
            "test-vm".to_string(),
            config,
            event_bus.clone(),
        );
        
        assert_eq!(aggregate.aggregate_id(), "test-vm");
        assert!(aggregate.event_bus.is_some());
    }

    #[test]
    fn test_virtual_machine_start() {
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 64 * 1024 * 1024,
            vcpu_count: 1,
            ..Default::default()
        };

        let mut aggregate = VirtualMachineAggregate::new("test-vm".to_string(), config);
        
        // 清除创建事件
        aggregate.mark_events_as_committed();
        
        // 启动虚拟机
        assert!(aggregate.start().is_ok());
        assert_eq!(aggregate.state, VmState::Running);
        
        // 应该有一个启动事件
        let events = aggregate.uncommitted_events();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmStarted { .. })));
    }

    #[test]
    fn test_virtual_machine_start_invalid_state() {
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 64 * 1024 * 1024,
            vcpu_count: 1,
            ..Default::default()
        };

        let mut aggregate = VirtualMachineAggregate::new("test-vm".to_string(), config);
        aggregate.state = VmState::Running;
        
        // 已经在运行状态，不能再次启动
        assert!(aggregate.start().is_err());
    }

    #[test]
    fn test_virtual_machine_pause() {
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 64 * 1024 * 1024,
            vcpu_count: 1,
            ..Default::default()
        };

        let mut aggregate = VirtualMachineAggregate::new("test-vm".to_string(), config);
        aggregate.state = VmState::Running;
        aggregate.mark_events_as_committed();
        
        // 暂停虚拟机
        assert!(aggregate.pause().is_ok());
        assert_eq!(aggregate.state, VmState::Paused);
        
        // 应该有一个暂停事件
        let events = aggregate.uncommitted_events();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmPaused { .. })));
    }

    #[test]
    fn test_virtual_machine_resume() {
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 64 * 1024 * 1024,
            vcpu_count: 1,
            ..Default::default()
        };

        let mut aggregate = VirtualMachineAggregate::new("test-vm".to_string(), config);
        aggregate.state = VmState::Paused;
        aggregate.mark_events_as_committed();
        
        // 恢复虚拟机
        assert!(aggregate.resume().is_ok());
        assert_eq!(aggregate.state, VmState::Running);
        
        // 应该有一个恢复事件
        let events = aggregate.uncommitted_events();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmResumed { .. })));
    }

    #[test]
    fn test_virtual_machine_stop() {
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 64 * 1024 * 1024,
            vcpu_count: 1,
            ..Default::default()
        };

        let mut aggregate = VirtualMachineAggregate::new("test-vm".to_string(), config);
        aggregate.state = VmState::Running;
        aggregate.mark_events_as_committed();
        
        // 停止虚拟机
        assert!(aggregate.stop("Test reason".to_string()).is_ok());
        assert_eq!(aggregate.state, VmState::Stopped);
        
        // 应该有一个停止事件
        let events = aggregate.uncommitted_events();
        assert_eq!(events.len(), 1);
        if let DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmStopped { reason, .. }) = &events[0] {
            assert_eq!(reason, "Test reason");
        } else {
            panic!("Expected VmStopped event");
        }
    }

    #[test]
    fn test_aggregate_root_trait() {
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 64 * 1024 * 1024,
            vcpu_count: 1,
            ..Default::default()
        };

        let mut aggregate = VirtualMachineAggregate::new("test-vm".to_string(), config);
        
        // 测试 aggregate_id
        assert_eq!(aggregate.aggregate_id(), "test-vm");
        
        // 测试 uncommitted_events
        let events = aggregate.uncommitted_events();
        assert!(!events.is_empty());
        
        // 测试 mark_events_as_committed
        aggregate.mark_events_as_committed();
        assert!(aggregate.uncommitted_events().is_empty());
    }

    #[test]
    fn test_version_increment() {
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 64 * 1024 * 1024,
            vcpu_count: 1,
            ..Default::default()
        };

        let mut aggregate = VirtualMachineAggregate::new("test-vm".to_string(), config);
        let initial_version = aggregate.version;
        
        aggregate.start().unwrap();
        aggregate.mark_events_as_committed();
        
        // 版本应该在操作后递增
        assert!(aggregate.version > initial_version);
    }

    #[test]
    fn test_pause_invalid_state() {
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 64 * 1024 * 1024,
            vcpu_count: 1,
            ..Default::default()
        };

        let mut aggregate = VirtualMachineAggregate::new("test-vm".to_string(), config);
        // 在Created状态不能暂停
        assert!(aggregate.pause().is_err());
        
        aggregate.state = VmState::Stopped;
        // 在Stopped状态不能暂停
        assert!(aggregate.pause().is_err());
    }

    #[test]
    fn test_resume_invalid_state() {
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 64 * 1024 * 1024,
            vcpu_count: 1,
            ..Default::default()
        };

        let mut aggregate = VirtualMachineAggregate::new("test-vm".to_string(), config);
        // 在Created状态不能恢复
        assert!(aggregate.resume().is_err());
        
        aggregate.state = VmState::Running;
        // 在Running状态不能恢复
        assert!(aggregate.resume().is_err());
        
        aggregate.state = VmState::Stopped;
        // 在Stopped状态不能恢复
        assert!(aggregate.resume().is_err());
    }

    #[test]
    fn test_stop_from_paused_state() {
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 64 * 1024 * 1024,
            vcpu_count: 1,
            ..Default::default()
        };

        let mut aggregate = VirtualMachineAggregate::new("test-vm".to_string(), config);
        aggregate.state = VmState::Paused;
        aggregate.mark_events_as_committed();
        
        // 可以从Paused状态停止
        assert!(aggregate.stop("Stopped from paused".to_string()).is_ok());
        assert_eq!(aggregate.state, VmState::Stopped);
    }

    #[test]
    fn test_commit_events() {
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 64 * 1024 * 1024,
            vcpu_count: 1,
            ..Default::default()
        };

        let mut aggregate = VirtualMachineAggregate::new("test-vm".to_string(), config);
        
        // 应该有未提交的事件
        assert!(!aggregate.uncommitted_events().is_empty());
        
        // 提交事件到事件总线（使用内部事件总线或创建新的）
        assert!(aggregate.commit_events().is_ok());
        assert!(aggregate.uncommitted_events().is_empty());
    }

    #[test]
    fn test_commit_events_with_event_bus() {
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 64 * 1024 * 1024,
            vcpu_count: 1,
            ..Default::default()
        };

        let event_bus = Arc::new(DomainEventBus::new());
        let mut aggregate = VirtualMachineAggregate::with_event_bus(
            "test-vm".to_string(),
            config,
            event_bus.clone(),
        );
        
        // 提交事件
        assert!(aggregate.commit_events().is_ok());
        assert!(aggregate.uncommitted_events().is_empty());
    }

    #[test]
    fn test_state_transitions() {
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 64 * 1024 * 1024,
            vcpu_count: 1,
            ..Default::default()
        };

        let mut aggregate = VirtualMachineAggregate::new("test-vm".to_string(), config);
        aggregate.mark_events_as_committed();
        
        // Created -> Running
        assert!(aggregate.start().is_ok());
        assert_eq!(aggregate.state, VmState::Running);
        aggregate.mark_events_as_committed();
        
        // Running -> Paused
        assert!(aggregate.pause().is_ok());
        assert_eq!(aggregate.state, VmState::Paused);
        aggregate.mark_events_as_committed();
        
        // Paused -> Running
        assert!(aggregate.resume().is_ok());
        assert_eq!(aggregate.state, VmState::Running);
        aggregate.mark_events_as_committed();
        
        // Running -> Stopped
        assert!(aggregate.stop("Test".to_string()).is_ok());
        assert_eq!(aggregate.state, VmState::Stopped);
    }

    #[test]
    fn test_get_state() {
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 64 * 1024 * 1024,
            vcpu_count: 1,
            ..Default::default()
        };

        let aggregate = VirtualMachineAggregate::new("test-vm".to_string(), config);
        assert_eq!(aggregate.state(), VmState::Created);
    }

    #[test]
    fn test_get_config() {
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 64 * 1024 * 1024,
            vcpu_count: 1,
            ..Default::default()
        };

        let aggregate = VirtualMachineAggregate::new("test-vm".to_string(), config.clone());
        assert_eq!(aggregate.config().guest_arch, config.guest_arch);
    }

    #[test]
    fn test_get_version() {
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 64 * 1024 * 1024,
            vcpu_count: 1,
            ..Default::default()
        };

        let mut aggregate = VirtualMachineAggregate::new("test-vm".to_string(), config);
        let initial_version = aggregate.version();
        
        aggregate.start().unwrap();
        assert!(aggregate.version() > initial_version);
    }

    #[test]
    fn test_vm_id_getter() {
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 64 * 1024 * 1024,
            vcpu_count: 1,
            ..Default::default()
        };

        let aggregate = VirtualMachineAggregate::new("test-vm-123".to_string(), config);
        assert_eq!(aggregate.vm_id(), "test-vm-123");
    }
}
