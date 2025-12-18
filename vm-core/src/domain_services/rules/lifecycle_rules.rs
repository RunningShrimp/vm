//! Lifecycle business rules
//!
//! This module contains business rules related to VM lifecycle management.

use crate::{VmError, VmResult, VmState};
use crate::aggregate_root::VirtualMachineAggregate;

/// Trait for lifecycle business rules
///
/// This trait defines the interface for business rules that validate
/// VM lifecycle state transitions.
pub trait LifecycleBusinessRule: Send + Sync {
    /// Validate start transition
    fn validate_start_transition(&self, aggregate: &VirtualMachineAggregate) -> VmResult<()>;
    
    /// Validate pause transition
    fn validate_pause_transition(&self, aggregate: &VirtualMachineAggregate) -> VmResult<()>;
    
    /// Validate resume transition
    fn validate_resume_transition(&self, aggregate: &VirtualMachineAggregate) -> VmResult<()>;
    
    /// Validate stop transition
    fn validate_stop_transition(&self, aggregate: &VirtualMachineAggregate) -> VmResult<()>;
}

/// VM state transition rule
///
/// This rule validates that VM state transitions follow the allowed
/// state machine transitions.
pub struct VmStateTransitionRule;

impl LifecycleBusinessRule for VmStateTransitionRule {
    fn validate_start_transition(&self, aggregate: &VirtualMachineAggregate) -> VmResult<()> {
        match aggregate.state() {
            VmState::Created | VmState::Paused => Ok(()),
            _ => Err(VmError::Core(crate::CoreError::InvalidState {
                message: "Cannot start VM in current state".to_string(),
                current: format!("{:?}", aggregate.state()),
                expected: "Created or Paused".to_string(),
            }))
        }
    }
    
    fn validate_pause_transition(&self, aggregate: &VirtualMachineAggregate) -> VmResult<()> {
        match aggregate.state() {
            VmState::Running => Ok(()),
            _ => Err(VmError::Core(crate::CoreError::InvalidState {
                message: "Cannot pause VM in current state".to_string(),
                current: format!("{:?}", aggregate.state()),
                expected: "Running".to_string(),
            }))
        }
    }
    
    fn validate_resume_transition(&self, aggregate: &VirtualMachineAggregate) -> VmResult<()> {
        match aggregate.state() {
            VmState::Paused => Ok(()),
            _ => Err(VmError::Core(crate::CoreError::InvalidState {
                message: "Cannot resume VM in current state".to_string(),
                current: format!("{:?}", aggregate.state()),
                expected: "Paused".to_string(),
            }))
        }
    }
    
    fn validate_stop_transition(&self, aggregate: &VirtualMachineAggregate) -> VmResult<()> {
        // VM can be stopped from any state except already stopped
        match aggregate.state() {
            VmState::Stopped => Err(VmError::Core(crate::CoreError::InvalidState {
                message: "VM is already stopped".to_string(),
                current: format!("{:?}", aggregate.state()),
                expected: "Any state except Stopped".to_string(),
            })),
            _ => Ok(())
        }
    }
}

/// VM resource availability rule
///
/// This rule validates that VM has sufficient resources to perform
/// lifecycle operations.
pub struct VmResourceAvailabilityRule;

impl LifecycleBusinessRule for VmResourceAvailabilityRule {
    fn validate_start_transition(&self, aggregate: &VirtualMachineAggregate) -> VmResult<()> {
        // Check if VM has sufficient memory allocated
        if aggregate.config().memory_size == 0 {
            return Err(VmError::Core(crate::CoreError::InvalidConfig {
                message: "VM cannot start without allocated memory".to_string(),
                field: "memory_size".to_string(),
            }));
        }
        
        // Check if VM has at least one vCPU
        if aggregate.config().vcpu_count == 0 {
            return Err(VmError::Core(crate::CoreError::InvalidConfig {
                message: "VM cannot start without vCPUs".to_string(),
                field: "vcpu_count".to_string(),
            }));
        }
        
        Ok(())
    }
    
    fn validate_pause_transition(&self, _aggregate: &VirtualMachineAggregate) -> VmResult<()> {
        // Pause operation doesn't require additional resources
        Ok(())
    }
    
    fn validate_resume_transition(&self, _aggregate: &VirtualMachineAggregate) -> VmResult<()> {
        // Resume operation doesn't require additional resources
        Ok(())
    }
    
    fn validate_stop_transition(&self, _aggregate: &VirtualMachineAggregate) -> VmResult<()> {
        // Stop operation doesn't require additional resources
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GuestArch, VmConfig};
    
    fn create_test_aggregate(state: VmState) -> VirtualMachineAggregate {
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 64 * 1024 * 1024,
            vcpu_count: 1,
            ..Default::default()
        };
        
        let mut aggregate = VirtualMachineAggregate::new("test-vm".to_string(), config);
        
        // Set desired state
        aggregate.set_state(state);
        
        aggregate
    }
    
    #[test]
    fn test_vm_state_transition_rule_valid_start() {
        let rule = VmStateTransitionRule;
        
        // Test valid start from Created state
        let aggregate = create_test_aggregate(VmState::Created);
        assert!(rule.validate_start_transition(&aggregate).is_ok());
        
        // Test valid start from Paused state
        let aggregate = create_test_aggregate(VmState::Paused);
        assert!(rule.validate_start_transition(&aggregate).is_ok());
    }
    
    #[test]
    fn test_vm_state_transition_rule_invalid_start() {
        let rule = VmStateTransitionRule;
        
        // Test invalid start from Running state
        let aggregate = create_test_aggregate(VmState::Running);
        assert!(rule.validate_start_transition(&aggregate).is_err());
        
        // Test invalid start from Stopped state
        let aggregate = create_test_aggregate(VmState::Stopped);
        assert!(rule.validate_start_transition(&aggregate).is_err());
    }
    
    #[test]
    fn test_vm_state_transition_rule_valid_pause() {
        let rule = VmStateTransitionRule;
        
        // Test valid pause from Running state
        let aggregate = create_test_aggregate(VmState::Running);
        assert!(rule.validate_pause_transition(&aggregate).is_ok());
    }
    
    #[test]
    fn test_vm_state_transition_rule_invalid_pause() {
        let rule = VmStateTransitionRule;
        
        // Test invalid pause from Created state
        let aggregate = create_test_aggregate(VmState::Created);
        assert!(rule.validate_pause_transition(&aggregate).is_err());
        
        // Test invalid pause from Paused state
        let aggregate = create_test_aggregate(VmState::Paused);
        assert!(rule.validate_pause_transition(&aggregate).is_err());
        
        // Test invalid pause from Stopped state
        let aggregate = create_test_aggregate(VmState::Stopped);
        assert!(rule.validate_pause_transition(&aggregate).is_err());
    }
    
    #[test]
    fn test_vm_state_transition_rule_valid_resume() {
        let rule = VmStateTransitionRule;
        
        // Test valid resume from Paused state
        let aggregate = create_test_aggregate(VmState::Paused);
        assert!(rule.validate_resume_transition(&aggregate).is_ok());
    }
    
    #[test]
    fn test_vm_state_transition_rule_invalid_resume() {
        let rule = VmStateTransitionRule;
        
        // Test invalid resume from Created state
        let aggregate = create_test_aggregate(VmState::Created);
        assert!(rule.validate_resume_transition(&aggregate).is_err());
        
        // Test invalid resume from Running state
        let aggregate = create_test_aggregate(VmState::Running);
        assert!(rule.validate_resume_transition(&aggregate).is_err());
        
        // Test invalid resume from Stopped state
        let aggregate = create_test_aggregate(VmState::Stopped);
        assert!(rule.validate_resume_transition(&aggregate).is_err());
    }
    
    #[test]
    fn test_vm_state_transition_rule_valid_stop() {
        let rule = VmStateTransitionRule;
        
        // Test valid stop from Created state
        let aggregate = create_test_aggregate(VmState::Created);
        assert!(rule.validate_stop_transition(&aggregate).is_ok());
        
        // Test valid stop from Running state
        let aggregate = create_test_aggregate(VmState::Running);
        assert!(rule.validate_stop_transition(&aggregate).is_ok());
        
        // Test valid stop from Paused state
        let aggregate = create_test_aggregate(VmState::Paused);
        assert!(rule.validate_stop_transition(&aggregate).is_ok());
    }
    
    #[test]
    fn test_vm_state_transition_rule_invalid_stop() {
        let rule = VmStateTransitionRule;
        
        // Test invalid stop from Stopped state
        let aggregate = create_test_aggregate(VmState::Stopped);
        assert!(rule.validate_stop_transition(&aggregate).is_err());
    }
    
    #[test]
    fn test_vm_resource_availability_rule_valid() {
        let rule = VmResourceAvailabilityRule;
        
        // Test with valid configuration
        let aggregate = create_test_aggregate(VmState::Created);
        assert!(rule.validate_start_transition(&aggregate).is_ok());
    }
    
    #[test]
    fn test_vm_resource_availability_rule_invalid_memory() {
        let rule = VmResourceAvailabilityRule;
        
        // Create aggregate with zero memory
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 0,
            vcpu_count: 1,
            ..Default::default()
        };
        
        let aggregate = VirtualMachineAggregate::new("test-vm".to_string(), config);
        assert!(rule.validate_start_transition(&aggregate).is_err());
    }
    
    #[test]
    fn test_vm_resource_availability_rule_invalid_vcpu() {
        let rule = VmResourceAvailabilityRule;
        
        // Create aggregate with zero vCPUs
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 64 * 1024 * 1024,
            vcpu_count: 0,
            ..Default::default()
        };
        
        let aggregate = VirtualMachineAggregate::new("test-vm".to_string(), config);
        assert!(rule.validate_start_transition(&aggregate).is_err());
    }
}