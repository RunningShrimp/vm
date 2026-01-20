//! Persistent Domain Event Bus
//!
//! This module provides a persistent implementation of the domain event bus
//! that combines in-memory handling with persistent storage.

use super::event_store::{EventStore, EventStoreError, SequenceNumber};
use super::events::{DomainEventBus, DomainEventEnum};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;

/// Persistent domain event bus that combines in-memory and persistent storage
pub struct PersistentDomainEventBus {
    /// Persistent storage
    store: Arc<dyn EventStore>,

    /// In-memory event cache for fast access
    memory_events: Arc<Mutex<VecDeque<DomainEventEnum>>>,

    /// Maximum number of events to keep in memory
    max_memory_events: usize,
}

impl PersistentDomainEventBus {
    /// Create a new persistent event bus
    pub fn new(store: Arc<dyn EventStore>) -> Self {
        Self {
            store,
            memory_events: Arc::new(Mutex::new(VecDeque::new())),
            max_memory_events: 1000,
        }
    }

    /// Create with custom memory limit
    pub fn with_max_memory_events(store: Arc<dyn EventStore>, max: usize) -> Self {
        Self {
            store,
            memory_events: Arc::new(Mutex::new(VecDeque::new())),
            max_memory_events: max,
        }
    }

    /// Replay events from persistent storage into memory
    pub fn replay(&self) -> Result<(), EventStoreError> {
        let events = self.store.replay(0)?;

        let mut memory = self.memory_events.lock();
        memory.clear();

        for stored_event in events {
            memory.push_back(stored_event.event_data);
        }

        Ok(())
    }

    /// Replay events from a specific sequence number
    pub fn replay_from(&self, seq: SequenceNumber) -> Result<(), EventStoreError> {
        let events = self.store.replay(seq)?;

        let mut memory = self.memory_events.lock();

        for stored_event in events {
            memory.push_back(stored_event.event_data);
        }

        Ok(())
    }

    /// Get all events from memory
    pub fn get_events(&self) -> Vec<DomainEventEnum> {
        let events = self.memory_events.lock();
        events.iter().cloned().collect()
    }

    /// Query events from persistent storage
    pub fn query(
        &self,
        filter: super::event_store::EventFilter,
    ) -> Result<Vec<super::event_store::StoredEvent>, EventStoreError> {
        self.store.query(filter)
    }

    /// Get the latest sequence number from storage
    pub fn latest_sequence(&self) -> Result<SequenceNumber, EventStoreError> {
        self.store.latest_sequence()
    }

    /// Clear all events from both memory and storage
    pub fn clear(&self) -> Result<(), EventStoreError> {
        self.memory_events.lock().clear();
        self.store.clear()
    }
}

impl DomainEventBus for PersistentDomainEventBus {
    fn publish(&self, event: DomainEventEnum) {
        // Store to persistent storage
        if let Err(e) = self.store.append(event.clone()) {
            log::error!("Failed to persist event: {:?}", e);
            // Continue anyway - event is still published to handlers
        }

        // Add to memory
        {
            let mut events = self.memory_events.lock();
            events.push_back(event.clone());

            // Enforce memory limit
            while events.len() > self.max_memory_events {
                events.pop_front();
            }
        }

        // Notify handlers through the in-memory bus
        // Note: This is a simplified implementation
        // In a full implementation, we'd have proper handler management
    }

    fn subscribe(&self, _handler: Arc<dyn super::events::DomainEventHandler>) {
        // TODO: Implement handler subscription
        log::warn!("Handler subscription not yet implemented for PersistentDomainEventBus");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain_services::event_store::InMemoryEventStore;
    use crate::domain_services::events::OptimizationEvent;
    use std::time::SystemTime;

    #[test]
    fn test_persistent_event_bus_publish() {
        let store = Arc::new(InMemoryEventStore::new());
        let bus = PersistentDomainEventBus::new(store.clone());

        let event = DomainEventEnum::Optimization(OptimizationEvent::PipelineConfigCreated {
            source_arch: "x86_64".to_string(),
            target_arch: "aarch64".to_string(),
            optimization_level: 2,
            stages_count: 5,
            occurred_at: SystemTime::UNIX_EPOCH,
        });

        bus.publish(event);

        // Check it's in storage
        let stored = store.replay(0).unwrap();
        assert_eq!(stored.len(), 1);

        // Check it's in memory
        let memory_events = bus.get_events();
        assert_eq!(memory_events.len(), 1);
    }

    #[test]
    fn test_persistent_event_bus_replay() {
        let store = Arc::new(InMemoryEventStore::new());
        let bus = PersistentDomainEventBus::new(store.clone());

        let event = DomainEventEnum::Optimization(OptimizationEvent::PipelineConfigCreated {
            source_arch: "x86_64".to_string(),
            target_arch: "aarch64".to_string(),
            optimization_level: 2,
            stages_count: 5,
            occurred_at: SystemTime::UNIX_EPOCH,
        });

        bus.publish(event);

        // Clear memory
        {
            let mut events = bus.memory_events.lock();
            events.clear();
        }

        // Replay from storage
        bus.replay().unwrap();

        // Should be back in memory
        let events = bus.get_events();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_persistent_event_bus_query() {
        let store = Arc::new(InMemoryEventStore::new());
        let bus = PersistentDomainEventBus::new(store);

        let event = DomainEventEnum::Optimization(OptimizationEvent::PipelineConfigCreated {
            source_arch: "x86_64".to_string(),
            target_arch: "aarch64".to_string(),
            optimization_level: 2,
            stages_count: 5,
            occurred_at: SystemTime::UNIX_EPOCH,
        });

        bus.publish(event);

        // Query events
        let filter = super::super::event_store::EventFilter {
            event_type_pattern: Some("optimization.*".to_string()),
            ..Default::default()
        };

        let results = bus.query(filter).unwrap();
        assert_eq!(results.len(), 1);
    }
}
