//! 聚合根实现
//!
//! 将VirtualMachine重构为聚合根，负责维护聚合不变式和发布领域事件。

use crate::domain_event_bus::DomainEventBus;
use crate::domain_events::{DomainEventEnum, VmConfigSnapshot, VmLifecycleEvent};
use crate::{VmLifecycleState, VmConfig, VmError, VmResult, VmState};
use std::sync::Arc;
use std::time::SystemTime;

/// 虚拟机应用服务
///
/// 负责协调领域服务和基础设施层，处理事件发布等横切关注点。
pub struct VmApplicationService {
    event_bus: Option<Arc<DomainEventBus>>,
}

impl VmApplicationService {
    /// 创建新的应用服务
    pub fn new() -> Self {
        Self {
            event_bus: None,
        }
    }

    /// 使用事件总线创建应用服务
    pub fn with_event_bus(event_bus: Arc<DomainEventBus>) -> Self {
        Self {
            event_bus: Some(event_bus),
        }
    }

    /// 提交聚合根的事件到事件总线
    ///
    /// 这个方法负责将聚合根中未提交的事件发布到事件总线，
    /// 并标记这些事件为已提交。
    pub fn commit_aggregate_events(&self, aggregate: &mut VirtualMachineAggregate) -> VmResult<()> {
        let events = aggregate.take_uncommitted_events();

        if let Some(event_bus) = &self.event_bus {
            for event in events {
                event_bus.publish(event)?;
            }
        }

        Ok(())
    }
}

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
    
    /// 记录事件（内部方法）
    /// 
    /// 这个方法被领域服务用于在验证后记录事件
    fn record_event(&mut self, event: DomainEventEnum);
}

/// 虚拟机聚合根
///
/// 这是虚拟机的聚合根，遵循 DDD 贫血模型原则：
/// - **状态管理**: 维护聚合状态（vm_id, config, state, version）
/// - **事件记录**: 记录未提交的领域事件（uncommitted_events）
/// - **事件回放**: 通过 `from_events` 和 `apply_event` 支持事件溯源
///
/// **注意**: 聚合根不直接发布事件。事件发布由应用服务（`VmApplicationService`）
/// 或领域服务（如 `VmLifecycleDomainService`）负责。
///
/// 聚合根只提供内部方法（`set_state`, `record_event`）供领域服务调用，
/// 确保业务逻辑在领域服务中，而不是在聚合根中。
#[cfg(not(feature = "no_std"))]
#[derive(Clone)]
pub struct VirtualMachineAggregate {
    /// 虚拟机ID
    vm_id: String,
    /// 配置
    config: VmConfig,
    /// 当前状态
    state: VmLifecycleState,
    /// 事件总线（可选，用于测试和向后兼容）
    /// 
    /// **注意**: 根据 DDD 贫血模型，聚合根不应直接发布事件。
    /// 事件发布应由应用服务或领域服务处理。
    /// 此字段保留仅用于向后兼容和测试目的。
    // 注意：此字段在测试中被使用，但不在生产代码中使用
    #[cfg_attr(not(test), allow(dead_code))]
    event_bus: Option<Arc<DomainEventBus>>,
    /// 未提交的事件
    /// 
    /// 这些事件由领域服务通过 `record_event` 方法记录，
    /// 然后由应用服务通过 `commit_aggregate_events` 发布。
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
            state: VmLifecycleState::Created,
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
            state: VmLifecycleState::Created,
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

    /// Set VM state directly (internal method)
    /// 
    /// This method is used by domain services to set the state
    /// after validation has been performed.
    pub(crate) fn set_state(&mut self, state: VmLifecycleState) {
        self.state = state;
        self.version += 1;
    }
    
    /// Record an event (internal method)
    /// 
    /// This method is used by domain services to record events
    /// after validation has been performed.
    pub(crate) fn record_event(&mut self, event: DomainEventEnum) {
        self.uncommitted_events.push(event);
    }

    /// 获取未提交的事件用于应用服务提交
    pub fn take_uncommitted_events(&mut self) -> Vec<DomainEventEnum> {
        let events = self.uncommitted_events.clone();
        self.mark_events_as_committed();
        events
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
    pub fn from_events(
        vm_id: String,
        config: VmConfig,
        events: Vec<crate::event_store_legacy::StoredEvent>,
    ) -> Self {
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
            DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmCreated { .. }) => {
                // 从事件中恢复配置（如果需要）
                // 注意：这里假设config已经通过构造函数传入
                self.state = VmLifecycleState::Created;
            }
            DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmStarted { .. }) => {
                if self.state == VmLifecycleState::Created || self.state == VmLifecycleState::Paused {
                    self.state = VmLifecycleState::Running;
                }
            }
            DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmPaused { .. }) => {
                if self.state == VmLifecycleState::Running {
                    self.state = VmLifecycleState::Paused;
                }
            }
            DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmResumed { .. }) => {
                if self.state == VmLifecycleState::Paused {
                    self.state = VmLifecycleState::Running;
                }
            }
            DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmStopped { .. }) => {
                self.state = VmLifecycleState::Stopped;
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
        assert_eq!(aggregate.state, VmLifecycleState::Created);
        assert_eq!(aggregate.version, 1);

        // 应该有一个创建事件
        let events = aggregate.uncommitted_events();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmCreated { .. })
        ));
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
    fn test_virtual_machine_set_state() {
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 64 * 1024 * 1024,
            vcpu_count: 1,
            ..Default::default()
        };

        let mut aggregate = VirtualMachineAggregate::new("test-vm".to_string(), config);
        let initial_version = aggregate.version();

        // 设置状态
        aggregate.set_state(VmLifecycleState::Running);
        assert_eq!(aggregate.state(), VmLifecycleState::Running);
        assert_eq!(aggregate.version(), initial_version + 1);

        // 再次设置状态
        aggregate.set_state(VmLifecycleState::Paused);
        assert_eq!(aggregate.state(), VmLifecycleState::Paused);
        assert_eq!(aggregate.version(), initial_version + 2);
    }

    #[test]
    fn test_virtual_machine_record_event() {
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 64 * 1024 * 1024,
            vcpu_count: 1,
            ..Default::default()
        };

        let mut aggregate = VirtualMachineAggregate::new("test-vm".to_string(), config);
        
        // 清除创建事件
        aggregate.mark_events_as_committed();
        
        // 记录一个自定义事件
        let custom_event = DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmStarted {
            vm_id: "test-vm".to_string(),
            occurred_at: SystemTime::now(),
        });
        
        aggregate.record_event(custom_event);
        
        // 应该有一个事件
        let events = aggregate.uncommitted_events();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmStarted { .. })
        ));
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
        let initial_version = aggregate.version();

        // Set state multiple times
        aggregate.set_state(VmLifecycleState::Running);
        aggregate.set_state(VmLifecycleState::Paused);
        aggregate.set_state(VmLifecycleState::Stopped);

        // Version should have been incremented for each state change
        assert_eq!(aggregate.version(), initial_version + 3);
    }

    #[test]
    fn test_commit_events() {
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

        // 应该有未提交的事件
        assert!(!aggregate.uncommitted_events().is_empty());

        // 使用应用服务提交事件
        let app_service = VmApplicationService::with_event_bus(event_bus);
        assert!(app_service.commit_aggregate_events(&mut aggregate).is_ok());
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

        // 使用应用服务提交事件
        let app_service = VmApplicationService::with_event_bus(event_bus);
        assert!(app_service.commit_aggregate_events(&mut aggregate).is_ok());
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
        let initial_version = aggregate.version();

        // Created -> Running
        aggregate.set_state(VmLifecycleState::Running);
        assert_eq!(aggregate.state(), VmLifecycleState::Running);
        assert_eq!(aggregate.version(), initial_version + 1);

        // Running -> Paused
        aggregate.set_state(VmLifecycleState::Paused);
        assert_eq!(aggregate.state(), VmLifecycleState::Paused);
        assert_eq!(aggregate.version(), initial_version + 2);

        // Paused -> Running
        aggregate.set_state(VmLifecycleState::Running);
        assert_eq!(aggregate.state(), VmLifecycleState::Running);
        assert_eq!(aggregate.version(), initial_version + 3);

        // Running -> Stopped
        aggregate.set_state(VmLifecycleState::Stopped);
        assert_eq!(aggregate.state(), VmLifecycleState::Stopped);
        assert_eq!(aggregate.version(), initial_version + 4);
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
        assert_eq!(aggregate.state(), VmLifecycleState::Created);
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

        // 使用领域服务启动VM，这会增加版本号
        use crate::domain_services::vm_lifecycle_service::VmLifecycleDomainService;
        let service = VmLifecycleDomainService::new();
        service.start_vm(&mut aggregate).unwrap();

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
