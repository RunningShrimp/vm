//! Base service configuration structures
//!
//! This module provides the fundamental configuration structures used across
//! all domain services, ensuring consistency and reducing code duplication.

use std::sync::Arc;

use crate::domain_event_bus::DomainEventBus;

/// Service configuration trait
///
/// This trait defines the common interface for all domain service configurations.
/// It provides access to shared configuration elements like event bus.
pub trait ServiceConfig: Send + Sync {
    /// Get the event bus if configured
    fn event_bus(&self) -> Option<&Arc<DomainEventBus>>;

    /// Set the event bus for this configuration
    fn set_event_bus(&mut self, event_bus: Arc<DomainEventBus>);
}

/// Base service configuration
///
/// This structure provides common configuration fields shared by all domain services.
/// It can be used directly or extended by service-specific configurations.
///
/// # Examples
///
/// ```rust
/// use crate::domain_services::config::BaseServiceConfig;
/// use std::sync::Arc;
///
/// // Create default configuration
/// let config = BaseServiceConfig::new();
///
/// // Add event bus
/// let event_bus = Arc::new(DomainEventBus::new());
/// let config = config.with_event_bus(event_bus);
/// ```
#[derive(Clone, Default)]
pub struct BaseServiceConfig {
    /// Event bus for publishing domain events
    event_bus: Option<Arc<DomainEventBus>>,
}

impl BaseServiceConfig {
    /// Create a new base service configuration with default values
    ///
    /// # Returns
    ///
    /// A new `BaseServiceConfig` with no event bus configured
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the event bus for this configuration
    ///
    /// # Parameters
    ///
    /// - `event_bus`: Event bus instance to use for publishing domain events
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Examples
    ///
    /// ```rust
    /// use crate::domain_services::config::BaseServiceConfig;
    /// use std::sync::Arc;
    ///
    /// let event_bus = Arc::new(DomainEventBus::new());
    /// let config = BaseServiceConfig::new()
    ///     .with_event_bus(event_bus);
    /// ```
    pub fn with_event_bus(mut self, event_bus: Arc<DomainEventBus>) -> Self {
        self.event_bus = Some(event_bus);
        self
    }

    /// Clear the event bus from this configuration
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn without_event_bus(mut self) -> Self {
        self.event_bus = None;
        self
    }
}

impl ServiceConfig for BaseServiceConfig {
    /// Get the event bus if configured
    fn event_bus(&self) -> Option<&Arc<DomainEventBus>> {
        self.event_bus.as_ref()
    }

    /// Set the event bus for this configuration
    fn set_event_bus(&mut self, event_bus: Arc<DomainEventBus>) {
        self.event_bus = Some(event_bus);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain_services::config::ServiceConfig as ServiceConfigTrait;

    #[test]
    fn test_default() {
        let config = BaseServiceConfig::default();
        assert!(config.event_bus().is_none());
    }

    #[test]
    fn test_new() {
        let config = BaseServiceConfig::new();
        assert!(config.event_bus().is_none());
    }

    #[test]
    fn test_with_event_bus() {
        let event_bus = Arc::new(DomainEventBus::new());
        let config = BaseServiceConfig::new().with_event_bus(event_bus.clone());
        assert!(config.event_bus().is_some());
        assert!(Arc::ptr_eq(config.event_bus().unwrap(), &event_bus));
    }

    #[test]
    fn test_without_event_bus() {
        let event_bus = Arc::new(DomainEventBus::new());
        let config = BaseServiceConfig::new()
            .with_event_bus(event_bus)
            .without_event_bus();
        assert!(config.event_bus().is_none());
    }

    #[test]
    fn test_set_event_bus() {
        let mut config = BaseServiceConfig::new();
        assert!(config.event_bus().is_none());

        let event_bus = Arc::new(DomainEventBus::new());
        config.set_event_bus(event_bus.clone());
        assert!(config.event_bus().is_some());
        assert!(Arc::ptr_eq(config.event_bus().unwrap(), &event_bus));

        let new_event_bus = Arc::new(DomainEventBus::new());
        config.set_event_bus(new_event_bus.clone());
        assert!(Arc::ptr_eq(config.event_bus().unwrap(), &new_event_bus));
    }

    #[test]
    fn test_clone() {
        let event_bus = Arc::new(DomainEventBus::new());
        let config = BaseServiceConfig::new().with_event_bus(event_bus.clone());
        let cloned = config.clone();

        assert!(Arc::ptr_eq(
            config.event_bus().unwrap(),
            cloned.event_bus().unwrap()
        ));
    }
}
