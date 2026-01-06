//! Service configuration builder
//!
//! This module provides a builder pattern for constructing service configurations
//! with type-safe and fluent API.

use std::sync::Arc;

use crate::domain_event_bus::DomainEventBus;

use super::base::BaseServiceConfig;

/// Builder for creating service configurations
///
/// This builder provides a fluent API for constructing service configurations
/// with optional parameters.
///
/// # Examples
///
/// ```rust
/// use crate::domain_services::config::ServiceConfigBuilder;
/// use std::sync::Arc;
///
/// let event_bus = Arc::new(DomainEventBus::new());
/// let config = ServiceConfigBuilder::new()
///     .with_event_bus(event_bus)
///     .build();
/// ```
pub struct ServiceConfigBuilder {
    event_bus: Option<Arc<DomainEventBus>>,
}

impl ServiceConfigBuilder {
    /// Create a new builder with default configuration
    ///
    /// # Returns
    ///
    /// A new `ServiceConfigBuilder` with no event bus configured
    pub fn new() -> Self {
        Self { event_bus: None }
    }

    /// Set the event bus for the configuration
    ///
    /// # Parameters
    ///
    /// - `event_bus`: Event bus instance to use for publishing domain events
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn with_event_bus(mut self, event_bus: Arc<DomainEventBus>) -> Self {
        self.event_bus = Some(event_bus);
        self
    }

    /// Build the configuration
    ///
    /// # Returns
    ///
    /// A `BaseServiceConfig` with the configured values
    pub fn build(self) -> BaseServiceConfig {
        let mut config = BaseServiceConfig::new();
        if let Some(event_bus) = self.event_bus {
            config = config.with_event_bus(event_bus);
        }
        config
    }
}

impl Default for ServiceConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain_services::config::ServiceConfig as ServiceConfigTrait;

    #[test]
    fn test_builder_default() {
        let builder = ServiceConfigBuilder::default();
        let config = builder.build();
        assert!(config.event_bus().is_none());
    }

    #[test]
    fn test_builder_new() {
        let builder = ServiceConfigBuilder::new();
        let config = builder.build();
        assert!(config.event_bus().is_none());
    }

    #[test]
    fn test_builder_with_event_bus() {
        let event_bus = Arc::new(DomainEventBus::new());
        let config = ServiceConfigBuilder::new()
            .with_event_bus(event_bus.clone())
            .build();

        assert!(config.event_bus().is_some());
        assert!(Arc::ptr_eq(config.event_bus().unwrap(), &event_bus));
    }

    #[test]
    fn test_builder_chaining() {
        let event_bus = Arc::new(DomainEventBus::new());
        let config = ServiceConfigBuilder::new()
            .with_event_bus(event_bus)
            .build();

        assert!(config.event_bus().is_some());
    }

    #[test]
    fn test_builder_reusable() {
        let event_bus1 = Arc::new(DomainEventBus::new());
        let event_bus2 = Arc::new(DomainEventBus::new());

        let builder = ServiceConfigBuilder::new();
        let config1 = builder
            .with_event_bus(event_bus1.clone())
            .build();

        let config2 = ServiceConfigBuilder::new()
            .with_event_bus(event_bus2.clone())
            .build();

        assert!(Arc::ptr_eq(
            config1.event_bus().unwrap(),
            &event_bus1
        ));
        assert!(Arc::ptr_eq(
            config2.event_bus().unwrap(),
            &event_bus2
        ));
        // Different event buses
        assert!(!Arc::ptr_eq(
            config1.event_bus().unwrap(),
            config2.event_bus().unwrap()
        ));
    }
}
