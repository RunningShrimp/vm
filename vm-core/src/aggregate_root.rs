//! 聚合根实现
//!
//! 将VirtualMachine重构为聚合根,负责维护聚合不变式和发布领域事件。

#![cfg(feature = "std")]

use crate::jit::domain_event_bus::DomainEventBus;
use crate::jit::domain_events::{DomainEventEnum, VmConfigSnapshot, VmLifecycleEvent};
use crate::{VmLifecycleState, VmConfig, VmError, VmResult, VmState};
use std::sync::Arc;
use std::time::SystemTime;

/// 聚合根trait
///
/// 所有聚合根都应该实现这个trait，提供事件发布能力。
/// 聚合根是领域驱动设计(DDD)中的核心概念，代表一个业务实体的一致性边界。
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
#[derive(Clone)]
pub struct VirtualMachineAggregate {
    /// 虚拟机ID
    vm_id: String,
    /// 配置
    config: VmConfig,
    /// 当前状态
    state: VmLifecycleState,
    /// 事件总线(可选，如果为None则使用全局总线)
    event_bus: Option<Arc<DomainEventBus>>,
    /// 未提交的事件
    uncommitted_events: Vec<DomainEventEnum>,
    /// 聚合版本(用于乐观锁)
    version: u64,
}

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
    pub(crate) fn set_state(&mut self, state: VmLifecycleState) {
        self.state = state;
        self.version += 1;
    }

    /// Record an event (internal method)
    pub(crate) fn record_event(&mut self, event: DomainEventEnum) {
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
    pub fn from_events(
        vm_id: String,
        config: VmConfig,
        events: Vec<crate::event_store::StoredEvent>,
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

    /// 应用事件到聚合状态(内部方法)
    fn apply_event(&mut self, event: &DomainEventEnum) {
        match event {
            DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmCreated { .. }) => {
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
            _ => {}
        }
    }
}

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
mod tests {
    use super::*;
    use crate::jit::domain_events::{DomainEventEnum, VmLifecycleEvent};
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
        let events = aggregate.uncommitted_events();
        assert_eq!(events.len(), 1);
    }
}

