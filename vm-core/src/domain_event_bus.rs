//! Domain Event Bus
//!
//! Simple event bus for domain events.

use crate::VmError;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Event subscription ID
pub type EventSubscriptionId = u64;

/// Domain Event Bus
///
/// Simple in-memory event bus for publishing domain events.
pub struct DomainEventBus {
    subscriptions: Arc<RwLock<HashMap<String, Vec<EventSubscriptionId>>>>,
    next_id: std::sync::atomic::AtomicU64,
}

impl DomainEventBus {
    /// Create a new event bus
    pub fn new() -> Self {
        Self {
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            next_id: std::sync::atomic::AtomicU64::new(1),
        }
    }

    /// Publish an event
    pub fn publish<E>(&self, _event: &E) -> Result<(), VmError>
    where
        E: std::fmt::Debug + Send + Sync,
    {
        // Minimal implementation - just log the event
        #[cfg(feature = "std")]
        eprintln!("Event published: {:?}", _event);
        Ok(())
    }

    /// Subscribe to events
    pub fn subscribe(&self, event_type: &str) -> Result<EventSubscriptionId, VmError> {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut subscriptions = self.subscriptions.write().map_err(|_| {
            VmError::Core(crate::error::CoreError::Concurrency {
                message: "Failed to acquire subscriptions lock".to_string(),
                operation: "subscribe".to_string(),
            })
        })?;
        subscriptions
            .entry(event_type.to_string())
            .or_insert_with(Vec::new)
            .push(id);
        Ok(id)
    }

    /// Unsubscribe from events
    pub fn unsubscribe(&self, _event_type: &str, _id: EventSubscriptionId) -> Result<(), VmError> {
        // Minimal implementation
        Ok(())
    }
}

impl Default for DomainEventBus {
    fn default() -> Self {
        Self::new()
    }
}
