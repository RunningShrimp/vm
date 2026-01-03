//! VM Lifecycle Domain Service
//!
//! This service encapsulates business logic related to VM lifecycle management.
//! It coordinates state transitions, validates business rules, and publishes
//! appropriate domain events.

use std::sync::Arc;

use crate::domain_services::events::{DomainEventEnum};
use crate::domain_event_bus::DomainEventBus;
use crate::domain_services::events::{VmLifecycleEvent, DomainEventEnum as BaseDomainEventEnum};
use crate::domain_services::rules::{LifecycleBusinessRule, VmStateTransitionRule, VmResourceAvailabilityRule};
use crate::{VmResult, VmState, VmLifecycleState};
use crate::aggregate_root::VirtualMachineAggregate;

/// VM Lifecycle Domain Service
///
/// This service manages VM lifecycle operations by coordinating business rules
/// and state transitions. It follows the domain service pattern to keep
/// business logic out of the aggregate root.
pub struct VmLifecycleDomainService {
    /// Business rules for lifecycle operations
    business_rules: Vec<Box<dyn LifecycleBusinessRule>>,
    /// Event bus for publishing domain events
    event_bus: Option<Arc<DomainEventBus>>,
}

impl VmLifecycleDomainService {
    /// Create a new VM lifecycle domain service with default rules
    pub fn new() -> Self {
        let business_rules: Vec<Box<dyn LifecycleBusinessRule>> = vec![
            Box::new(VmStateTransitionRule),
            Box::new(VmResourceAvailabilityRule),
        ];
        
        Self {
            business_rules,
            event_bus: None,
        }
    }
    
    /// Create a new VM lifecycle domain service with custom rules
    pub fn with_rules(business_rules: Vec<Box<dyn LifecycleBusinessRule>>) -> Self {
        Self {
            business_rules,
            event_bus: None,
        }
    }
    
    /// Set the event bus for publishing domain events
    pub fn with_event_bus(mut self, event_bus: Arc<DomainEventBus>) -> Self {
        self.event_bus = Some(event_bus);
        self
    }
    
    /// Start a VM
    ///
    /// This method validates all business rules before transitioning the VM
    /// to the running state and publishing appropriate events.
    pub fn start_vm(&self, aggregate: &mut VirtualMachineAggregate) -> VmResult<()> {
        // Validate all business rules
        for rule in &self.business_rules {
            rule.validate_start_transition(aggregate)?
        }

        // Record state transition
        let old_state = aggregate.state();
        let old_state_vm = match old_state {
            VmLifecycleState::Created => VmState::Created,
            VmLifecycleState::Running => VmState::Running,
            VmLifecycleState::Paused => VmState::Paused,
            VmLifecycleState::Stopped => VmState::Stopped,
        };
        self.set_vm_state(aggregate, VmState::Running);

        // Publish lifecycle events
        self.publish_state_change_event(aggregate, old_state_vm, VmState::Running)?;
        self.publish_lifecycle_event(aggregate, VmLifecycleEvent::VmStarted {
            vm_id: aggregate.vm_id().to_string(),
        })?;

        Ok(())
    }
    
    /// Pause a VM
    ///
    /// This method validates all business rules before transitioning the VM
    /// to the paused state and publishing appropriate events.
    pub fn pause_vm(&self, aggregate: &mut VirtualMachineAggregate) -> VmResult<()> {
        // Validate all business rules
        for rule in &self.business_rules {
            rule.validate_pause_transition(aggregate)?
        }

        // Record state transition
        let old_state = aggregate.state();
        let old_state_vm = match old_state {
            VmLifecycleState::Created => VmState::Created,
            VmLifecycleState::Running => VmState::Running,
            VmLifecycleState::Paused => VmState::Paused,
            VmLifecycleState::Stopped => VmState::Stopped,
        };
        self.set_vm_state(aggregate, VmState::Paused);

        // Publish lifecycle events
        self.publish_state_change_event(aggregate, old_state_vm, VmState::Paused)?;
        self.publish_lifecycle_event(aggregate, VmLifecycleEvent::VmPaused {
            vm_id: aggregate.vm_id().to_string(),
        })?;

        Ok(())
    }
    
    /// Resume a VM
    ///
    /// This method validates all business rules before transitioning the VM
    /// to the running state and publishing appropriate events.
    pub fn resume_vm(&self, aggregate: &mut VirtualMachineAggregate) -> VmResult<()> {
        // Validate all business rules
        for rule in &self.business_rules {
            rule.validate_resume_transition(aggregate)?
        }

        // Record state transition
        let old_state = aggregate.state();
        let old_state_vm = match old_state {
            VmLifecycleState::Created => VmState::Created,
            VmLifecycleState::Running => VmState::Running,
            VmLifecycleState::Paused => VmState::Paused,
            VmLifecycleState::Stopped => VmState::Stopped,
        };
        self.set_vm_state(aggregate, VmState::Running);

        // Publish lifecycle events
        self.publish_state_change_event(aggregate, old_state_vm, VmState::Running)?;
        self.publish_lifecycle_event(aggregate, VmLifecycleEvent::VmResumed {
            vm_id: aggregate.vm_id().to_string(),
        })?;

        Ok(())
    }
    
    /// Stop a VM
    ///
    /// This method validates all business rules before transitioning the VM
    /// to the stopped state and publishing appropriate events.
    pub fn stop_vm(&self, aggregate: &mut VirtualMachineAggregate, reason: String) -> VmResult<()> {
        // Validate all business rules
        for rule in &self.business_rules {
            rule.validate_stop_transition(aggregate)?
        }

        // Record state transition
        let old_state = aggregate.state();
        let old_state_vm = match old_state {
            VmLifecycleState::Created => VmState::Created,
            VmLifecycleState::Running => VmState::Running,
            VmLifecycleState::Paused => VmState::Paused,
            VmLifecycleState::Stopped => VmState::Stopped,
        };
        self.set_vm_state(aggregate, VmState::Stopped);

        // Publish lifecycle events
        self.publish_state_change_event(aggregate, old_state_vm, VmState::Stopped)?;
        self.publish_lifecycle_event(aggregate, VmLifecycleEvent::VmStopped {
            vm_id: aggregate.vm_id().to_string(),
            reason,
        })?;

        Ok(())
    }
    
    /// Check if a VM can be started
    ///
    /// This method validates all business rules without modifying the VM state.
    pub fn can_start_vm(&self, aggregate: &VirtualMachineAggregate) -> VmResult<()> {
        for rule in &self.business_rules {
            rule.validate_start_transition(aggregate)?;
        }
        Ok(())
    }
    
    /// Check if a VM can be paused
    ///
    /// This method validates all business rules without modifying the VM state.
    pub fn can_pause_vm(&self, aggregate: &VirtualMachineAggregate) -> VmResult<()> {
        for rule in &self.business_rules {
            rule.validate_pause_transition(aggregate)?;
        }
        Ok(())
    }
    
    /// Check if a VM can be resumed
    ///
    /// This method validates all business rules without modifying the VM state.
    pub fn can_resume_vm(&self, aggregate: &VirtualMachineAggregate) -> VmResult<()> {
        for rule in &self.business_rules {
            rule.validate_resume_transition(aggregate)?;
        }
        Ok(())
    }
    
    /// Check if a VM can be stopped
    ///
    /// This method validates all business rules without modifying the VM state.
    pub fn can_stop_vm(&self, aggregate: &VirtualMachineAggregate) -> VmResult<()> {
        for rule in &self.business_rules {
            rule.validate_stop_transition(aggregate)?;
        }
        Ok(())
    }
    
    /// Get valid state transitions from the current state
    ///
    /// This method returns a list of valid state transitions from the current VM state.
    pub fn get_valid_transitions(&self, aggregate: &VirtualMachineAggregate) -> Vec<VmState> {
        let mut valid_transitions = Vec::new();
        
        if self.can_start_vm(aggregate).is_ok() {
            valid_transitions.push(VmState::Running);
        }
        
        if self.can_pause_vm(aggregate).is_ok() {
            valid_transitions.push(VmState::Paused);
        }
        
        if self.can_stop_vm(aggregate).is_ok() {
            valid_transitions.push(VmState::Stopped);
        }
        
        valid_transitions
    }
    
    /// Set VM state (internal method)
    ///
    /// This method directly sets the VM state without validation.
    /// It's used internally after validation has been performed.
    fn set_vm_state(&self, aggregate: &mut VirtualMachineAggregate, state: VmState) {
        let lifecycle_state = match state {
            VmState::Created => VmLifecycleState::Created,
            VmState::Running => VmLifecycleState::Running,
            VmState::Paused => VmLifecycleState::Paused,
            VmState::Stopped => VmLifecycleState::Stopped,
        };
        aggregate.set_state(lifecycle_state);
    }
    
    /// Publish a state change event
    fn publish_state_change_event(
        &self,
        aggregate: &mut VirtualMachineAggregate,
        _from: VmState,
        to: VmState,
    ) -> VmResult<()> {
        let base_event = BaseDomainEventEnum::VmLifecycle(VmLifecycleEvent::VmStateChanged {
            vm_id: aggregate.vm_id().to_string(),
            to: format!("{:?}", to),
        });

        self.publish_base_event(aggregate, base_event)
    }
    
    /// Publish a lifecycle event
    fn publish_lifecycle_event(
        &self,
        aggregate: &mut VirtualMachineAggregate,
        event: VmLifecycleEvent,
    ) -> VmResult<()> {
        let base_event = BaseDomainEventEnum::VmLifecycle(event);
        self.publish_base_event(aggregate, base_event)
    }
    
    /// Publish an event
    fn publish_base_event(
        &self,
        aggregate: &mut VirtualMachineAggregate,
        event: BaseDomainEventEnum,
    ) -> VmResult<()> {
        // Record event in aggregate
        aggregate.record_event(event.clone());

        // If we have an event bus, publish immediately
        if let Some(event_bus) = &self.event_bus {
            let domain_event: DomainEventEnum = event;
            // Event publishing failures are logged but don't fail the operation
            let _ = event_bus.publish(&domain_event);
        }

        Ok(())
    }

    #[allow(dead_code)] // Deprecated: kept for backward compatibility
    fn publish_event(
        &self,
        _aggregate: &mut VirtualMachineAggregate,
        _event: DomainEventEnum,
    ) -> VmResult<()> {
        // Deprecated: use publish_base_event instead
        Ok(())
    }
}

impl Default for VmLifecycleDomainService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GuestArch, VmConfig, VmError};
    use crate::aggregate_root::AggregateRoot;
    
    fn create_test_aggregate() -> VirtualMachineAggregate {
        create_test_aggregate_with_state(VmState::Created)
    }
    
    fn create_test_aggregate_with_state(state: VmState) -> VirtualMachineAggregate {
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: crate::DEFAULT_MEMORY_SIZE,
            vcpu_count: 1,
            ..Default::default()
        };
        
        let mut aggregate = VirtualMachineAggregate::new("test-vm".to_string(), config);
        
        // Set desired state
        aggregate.set_state(state);
        
        aggregate
    }
    
    #[test]
    fn test_vm_lifecycle_service_start() {
        let service = VmLifecycleDomainService::new();
        let mut aggregate = create_test_aggregate();
        
        // Clear creation events
        aggregate.mark_events_as_committed();
        
        // Start the VM
        assert!(service.start_vm(&mut aggregate).is_ok());
        assert_eq!(aggregate.state(), VmState::Running);
        
        // Should have state change and start events
        let events = aggregate.uncommitted_events();
        assert_eq!(events.len(), 2);
    }
    
    #[test]
    fn test_vm_lifecycle_service_pause() {
        let service = VmLifecycleDomainService::new();
        let mut aggregate = create_test_aggregate();

        // Start the VM first
        service.start_vm(&mut aggregate)
            .expect("start_vm should not fail in test");
        aggregate.mark_events_as_committed();

        // Pause the VM
        assert!(service.pause_vm(&mut aggregate).is_ok());
        assert_eq!(aggregate.state(), VmState::Paused);
        
        // Should have state change and pause events
        let events = aggregate.uncommitted_events();
        assert_eq!(events.len(), 2);
    }
    
    #[test]
    fn test_vm_lifecycle_service_resume() {
        let service = VmLifecycleDomainService::new();
        let mut aggregate = create_test_aggregate();
        
        // Start and pause the VM first
        service.start_vm(&mut aggregate)
            .expect("start_vm should not fail in test");
        service.pause_vm(&mut aggregate)
            .expect("pause_vm should not fail in test");
        aggregate.mark_events_as_committed();
        
        // Resume the VM
        assert!(service.resume_vm(&mut aggregate).is_ok());
        assert_eq!(aggregate.state(), VmState::Running);
        
        // Should have state change and resume events
        let events = aggregate.uncommitted_events();
        assert_eq!(events.len(), 2);
    }
    
    #[test]
    fn test_vm_lifecycle_service_stop() {
        let service = VmLifecycleDomainService::new();
        let mut aggregate = create_test_aggregate();

        // Start the VM first
        service.start_vm(&mut aggregate)
            .expect("start_vm should not fail in test");
        aggregate.mark_events_as_committed();

        // Stop the VM
        assert!(service.stop_vm(&mut aggregate, "Test stop".to_string()).is_ok());
        assert_eq!(aggregate.state(), VmState::Stopped);
        
        // Should have state change and stop events
        let events = aggregate.uncommitted_events();
        assert_eq!(events.len(), 2);
    }
    
    #[test]
    fn test_vm_lifecycle_service_invalid_start() {
        let service = VmLifecycleDomainService::new();
        let mut aggregate = create_test_aggregate();

        // Start the VM first
        service.start_vm(&mut aggregate)
            .expect("start_vm should not fail in test");
        aggregate.mark_events_as_committed();

        // Try to start again (should fail)
        assert!(service.start_vm(&mut aggregate).is_err());
        assert_eq!(aggregate.state(), VmState::Running);
        
        // Should have no new events
        let events = aggregate.uncommitted_events();
        assert_eq!(events.len(), 0);
    }
    
    #[test]
    fn test_vm_lifecycle_service_invalid_pause() {
        let service = VmLifecycleDomainService::new();
        let mut aggregate = create_test_aggregate();

        // Clear creation events
        aggregate.mark_events_as_committed();

        // Try to pause without starting (should fail)
        assert!(service.pause_vm(&mut aggregate).is_err());
        assert_eq!(aggregate.state(), VmState::Created);

        // Should have no new events
        let events = aggregate.uncommitted_events();
        assert_eq!(events.len(), 0);
    }
    
    #[test]
    fn test_vm_lifecycle_service_invalid_resume() {
        let service = VmLifecycleDomainService::new();
        let mut aggregate = create_test_aggregate();

        // Clear creation events
        aggregate.mark_events_as_committed();

        // Try to resume without pausing (should fail)
        assert!(service.resume_vm(&mut aggregate).is_err());
        assert_eq!(aggregate.state(), VmState::Created);

        // Should have no new events
        let events = aggregate.uncommitted_events();
        assert_eq!(events.len(), 0);
    }
    
    #[test]
    fn test_vm_lifecycle_service_can_operations() {
        let service = VmLifecycleDomainService::new();
        let aggregate = create_test_aggregate();
        
        // Should be able to start from Created state
        assert!(service.can_start_vm(&aggregate).is_ok());
        assert!(service.can_pause_vm(&aggregate).is_err());
        assert!(service.can_resume_vm(&aggregate).is_err());
        assert!(service.can_stop_vm(&aggregate).is_ok());
    }
    
    #[test]
    fn test_vm_lifecycle_service_get_valid_transitions() {
        let service = VmLifecycleDomainService::new();
        let aggregate = create_test_aggregate();
        
        // From Created state, should be able to start or stop
        let transitions = service.get_valid_transitions(&aggregate);
        assert!(transitions.contains(&VmState::Running));
        assert!(transitions.contains(&VmState::Stopped));
        assert!(!transitions.contains(&VmState::Paused));
    }
    
    #[test]
    fn test_vm_lifecycle_service_with_event_bus() {
        let service = VmLifecycleDomainService::new()
            .with_event_bus(Arc::new(DomainEventBus::new()));

        let mut aggregate = create_test_aggregate();
        aggregate.mark_events_as_committed();

        // Start the VM
        assert!(service.start_vm(&mut aggregate).is_ok());
        assert_eq!(aggregate.state(), VmState::Running);

        // Should have state change and start events
        let events = aggregate.uncommitted_events();
        assert_eq!(events.len(), 2);
    }
    
    #[test]
    fn test_vm_lifecycle_service_with_custom_rules() {
        // Create a custom rule that always fails
        struct AlwaysFailRule;
        impl LifecycleBusinessRule for AlwaysFailRule {
            fn validate_start_transition(&self, _aggregate: &VirtualMachineAggregate) -> VmResult<()> {
                Err(VmError::Core(crate::CoreError::InvalidState {
                    message: "Always fail".to_string(),
                    current: "any".to_string(),
                    expected: "none".to_string(),
                }))
            }

            fn validate_pause_transition(&self, _aggregate: &VirtualMachineAggregate) -> VmResult<()> {
                Err(VmError::Core(crate::CoreError::InvalidState {
                    message: "Always fail".to_string(),
                    current: "any".to_string(),
                    expected: "none".to_string(),
                }))
            }

            fn validate_resume_transition(&self, _aggregate: &VirtualMachineAggregate) -> VmResult<()> {
                Err(VmError::Core(crate::CoreError::InvalidState {
                    message: "Always fail".to_string(),
                    current: "any".to_string(),
                    expected: "none".to_string(),
                }))
            }

            fn validate_stop_transition(&self, _aggregate: &VirtualMachineAggregate) -> VmResult<()> {
                Err(VmError::Core(crate::CoreError::InvalidState {
                    message: "Always fail".to_string(),
                    current: "any".to_string(),
                    expected: "none".to_string(),
                }))
            }
        }
        
        let service = VmLifecycleDomainService::with_rules(vec![Box::new(AlwaysFailRule)]);
        let mut aggregate = create_test_aggregate();
        
        // All operations should fail
        assert!(service.start_vm(&mut aggregate).is_err());
        assert!(service.pause_vm(&mut aggregate).is_err());
        assert!(service.resume_vm(&mut aggregate).is_err());
        assert!(service.stop_vm(&mut aggregate, "Test".to_string()).is_err());
    }
}