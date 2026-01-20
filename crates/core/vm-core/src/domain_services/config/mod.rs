//! # Domain Services Configuration Module
//!
//! This module provides unified configuration structures and traits for domain services.
//! It eliminates code duplication and provides a consistent configuration API across all services.
//!
//! ## Architecture
//!
//! - **`ServiceConfig` trait**: Common configuration interface for all services
//! - **`BaseServiceConfig`**: Base configuration structure with common fields
//! - **`ServiceConfigBuilder`**: Builder pattern for constructing configurations
//!
//! ## Usage
//!
//! ```rust
//! use crate::domain_services::config::{ServiceConfig, BaseServiceConfig};
//! use std::sync::Arc;
//!
//! // Create base configuration
//! let config = BaseServiceConfig::new();
//!
//! // Set event bus
//! let config = config.with_event_bus(Arc::new(DomainEventBus::new()));
//!
//! // Use in service
//! let service = MyDomainService::new(config);
//! ```

pub mod base;
pub mod builder;

pub use base::{BaseServiceConfig, ServiceConfig};
pub use builder::ServiceConfigBuilder;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain_event_bus::DomainEventBus;
    use std::sync::Arc;

    #[test]
    fn test_base_service_config_creation() {
        let config = BaseServiceConfig::new();
        assert!(config.event_bus().is_none());
    }

    #[test]
    fn test_base_service_config_with_event_bus() {
        let event_bus = Arc::new(DomainEventBus::new());
        let config = BaseServiceConfig::new().with_event_bus(event_bus.clone());
        assert!(config.event_bus().is_some());
        assert!(Arc::ptr_eq(config.event_bus().unwrap(), &event_bus));
    }

    #[test]
    fn test_service_config_builder() {
        let event_bus = Arc::new(DomainEventBus::new());
        let config = ServiceConfigBuilder::new()
            .with_event_bus(event_bus)
            .build();

        assert!(config.event_bus().is_some());
    }
}
