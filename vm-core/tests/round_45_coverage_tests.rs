//! Round 45: Test Coverage Enhancement
//!
//! This test file is created as part of Round 45 to improve test coverage
//! for the vm-core crate, focusing on basic functionality that compiles.

// ============================================================================
// Domain Services Tests
// ============================================================================

#[cfg(test)]
mod domain_services_tests {
    use std::sync::Arc;
    use vm_core::domain_event_bus::DomainEventBus;
    use vm_core::domain_services::{
        VmLifecycleDomainService,
        config::{BaseServiceConfig, ServiceConfig},
    };

    #[test]
    fn test_lifecycle_service_creation() {
        let service = VmLifecycleDomainService::new();
        // Service can be created successfully
        let _ = service;
    }

    #[test]
    fn test_base_service_config() {
        let config = BaseServiceConfig::new();
        assert!(config.event_bus().is_none());
    }

    #[test]
    fn test_base_service_config_with_event_bus() {
        let event_bus = Arc::new(DomainEventBus::new());
        let config = BaseServiceConfig::new().with_event_bus(event_bus);
        assert!(config.event_bus().is_some());
    }

    #[test]
    fn test_base_service_config_without_event_bus() {
        let event_bus = Arc::new(DomainEventBus::new());
        let config = BaseServiceConfig::new()
            .with_event_bus(event_bus)
            .without_event_bus();
        assert!(config.event_bus().is_none());
    }

    #[test]
    fn test_base_service_config_clone() {
        let config1 = BaseServiceConfig::new();
        let config2 = config1.clone();
        assert!(config2.event_bus().is_none());
    }
}

// ============================================================================
// Value Objects Tests
// ============================================================================

#[cfg(test)]
mod value_objects_tests {
    use vm_core::{GuestAddr, GuestPhysAddr, VmId};

    #[test]
    fn test_guest_addr_creation() {
        let addr = GuestAddr(0x1000);
        assert_eq!(addr.0, 0x1000);
    }

    #[test]
    fn test_guest_phys_addr_creation() {
        let addr = GuestPhysAddr(0x2000);
        assert_eq!(addr.0, 0x2000);
    }

    #[test]
    fn test_vm_id_creation() {
        let id = VmId::new("test-vm".to_string()).expect("VmId creation");
        assert_eq!(id.as_str(), "test-vm");
    }

    #[test]
    fn test_vm_id_clone() {
        let id1 = VmId::new("test-vm".to_string()).expect("VmId creation");
        let id2 = id1.clone();
        assert_eq!(id1, id2);
    }
}

// ============================================================================
// VM State Tests
// ============================================================================

#[cfg(test)]
mod vm_state_tests {
    use vm_core::VmState;

    #[test]
    fn test_vm_state_default() {
        let state = VmState::default();
        assert!(matches!(state, VmState::Created));
    }

    #[test]
    fn test_vm_state_variants() {
        let created = VmState::Created;
        let running = VmState::Running;
        let paused = VmState::Paused;
        let stopped = VmState::Stopped;

        // Ensure all variants exist and can be created
        assert!(matches!(created, VmState::Created));
        assert!(matches!(running, VmState::Running));
        assert!(matches!(paused, VmState::Paused));
        assert!(matches!(stopped, VmState::Stopped));
    }
}

// ============================================================================
// Scheduling Module Tests
// ============================================================================

#[cfg(test)]
mod scheduling_tests {
    use vm_core::scheduling::QoSClass;

    #[test]
    fn test_qos_class_values() {
        // Test QoS class enum values exist
        let _user_interactive = QoSClass::UserInteractive;
        let _user_initiated = QoSClass::UserInitiated;
        let _utility = QoSClass::Utility;
        let _background = QoSClass::Background;
        let _unspecified = QoSClass::Unspecified;
    }
}

// ============================================================================
// Domain Event Bus Tests
// ============================================================================

#[cfg(test)]
mod domain_event_bus_tests {
    use vm_core::domain_event_bus::DomainEventBus;

    #[test]
    fn test_event_bus_creation() {
        let bus = DomainEventBus::new();
        // Just ensure it can be created without panic
        let _ = bus;
    }

    #[test]
    fn test_event_bus_default() {
        let bus = DomainEventBus::default();
        // Ensure default trait works
        let _ = bus;
    }
}
