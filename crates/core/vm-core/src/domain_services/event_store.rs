//! Event Store Module
//!
//! This module provides persistence capabilities for domain events.
//! It implements event sourcing patterns with in-memory storage.

use crate::domain_services::events::DomainEventEnum;
use thiserror::Error;

/// Sequence number for events in the store
pub type SequenceNumber = u64;

/// Errors that can occur in the event store
#[derive(Debug, Error)]
pub enum EventStoreError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Event not found at sequence {0}")]
    EventNotFound(SequenceNumber),

    #[error("Invalid event data: {0}")]
    InvalidData(String),
}

/// Filter for querying events
#[derive(Debug, Clone, Default)]
pub struct EventFilter {
    /// Filter by event type (e.g., "optimization.*")
    pub event_type_pattern: Option<String>,

    /// Maximum number of events to return
    pub limit: Option<usize>,
}

/// Trait for event persistence storage
pub trait EventStore: Send + Sync {
    /// Append a single event to the store
    fn append(&self, event: DomainEventEnum) -> Result<SequenceNumber, EventStoreError>;

    /// Append multiple events atomically
    fn append_batch(&self, events: Vec<DomainEventEnum>) -> Result<(), EventStoreError>;

    /// Replay events from a specific sequence number
    fn replay(&self, from: SequenceNumber) -> Result<Vec<StoredEvent>, EventStoreError>;

    /// Query events with optional filters
    fn query(&self, filter: EventFilter) -> Result<Vec<StoredEvent>, EventStoreError>;

    /// Get the latest sequence number
    fn latest_sequence(&self) -> Result<SequenceNumber, EventStoreError>;

    /// Clear all events (useful for testing)
    fn clear(&self) -> Result<(), EventStoreError>;
}

/// Stored event with metadata
#[derive(Debug, Clone)]
pub struct StoredEvent {
    /// Sequence number
    pub sequence_number: SequenceNumber,

    /// Event type identifier
    pub event_type: String,

    /// Event data
    pub event_data: DomainEventEnum,
}

/// In-memory event store for testing and development
pub struct InMemoryEventStore {
    events: parking_lot::Mutex<Vec<StoredEvent>>,
    next_sequence: parking_lot::Mutex<SequenceNumber>,
}

impl InMemoryEventStore {
    /// Create a new in-memory event store
    pub fn new() -> Self {
        Self {
            events: parking_lot::Mutex::new(Vec::new()),
            next_sequence: parking_lot::Mutex::new(0),
        }
    }
}

impl Default for InMemoryEventStore {
    fn default() -> Self {
        Self::new()
    }
}

impl EventStore for InMemoryEventStore {
    fn append(&self, event: DomainEventEnum) -> Result<SequenceNumber, EventStoreError> {
        let mut events = self.events.lock();
        let mut seq = self.next_sequence.lock();

        let stored_event = StoredEvent {
            sequence_number: *seq,
            event_type: event.name().to_string(),
            event_data: event,
        };

        events.push(stored_event);
        let result = *seq;
        *seq += 1;

        Ok(result)
    }

    fn append_batch(&self, events: Vec<DomainEventEnum>) -> Result<(), EventStoreError> {
        let mut stored_events = self.events.lock();
        let mut seq = self.next_sequence.lock();

        for event in events {
            let stored_event = StoredEvent {
                sequence_number: *seq,
                event_type: event.name().to_string(),
                event_data: event,
            };

            stored_events.push(stored_event);
            *seq += 1;
        }

        Ok(())
    }

    fn replay(&self, from: SequenceNumber) -> Result<Vec<StoredEvent>, EventStoreError> {
        let events = self.events.lock();
        Ok(events
            .iter()
            .filter(|e| e.sequence_number >= from)
            .cloned()
            .collect())
    }

    fn query(&self, filter: EventFilter) -> Result<Vec<StoredEvent>, EventStoreError> {
        let events = self.events.lock();
        let mut result: Vec<StoredEvent> = events
            .iter()
            .filter(|e| {
                // Type filter (simple prefix matching)
                if let Some(pattern) = &filter.event_type_pattern {
                    let pattern_str = pattern.as_str();
                    if !pattern_str.ends_with('*') {
                        // Exact match
                        if e.event_type != pattern_str {
                            return false;
                        }
                    } else {
                        // Prefix match
                        let prefix = &pattern_str[..pattern_str.len() - 1];
                        if !e.event_type.starts_with(prefix) {
                            return false;
                        }
                    }
                }

                true
            })
            .cloned()
            .collect();

        // Apply limit
        if let Some(limit) = filter.limit {
            result.truncate(limit);
        }

        Ok(result)
    }

    fn latest_sequence(&self) -> Result<SequenceNumber, EventStoreError> {
        let seq = *self.next_sequence.lock();
        Ok(if seq > 0 { seq - 1 } else { 0 })
    }

    fn clear(&self) -> Result<(), EventStoreError> {
        let mut events = self.events.lock();
        events.clear();
        let mut seq = self.next_sequence.lock();
        *seq = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain_services::events::OptimizationEvent;

    #[test]
    fn test_in_memory_event_store_append() {
        let store = InMemoryEventStore::new();

        let event = DomainEventEnum::Optimization(OptimizationEvent::PipelineConfigCreated {
            source_arch: "x86_64".to_string(),
            target_arch: "aarch64".to_string(),
            optimization_level: 2,
            stages_count: 5,
            occurred_at: std::time::SystemTime::UNIX_EPOCH,
        });

        let seq = store.append(event).unwrap();
        assert_eq!(seq, 0);

        let latest = store.latest_sequence().unwrap();
        assert_eq!(latest, 0);
    }

    #[test]
    fn test_in_memory_event_store_replay() {
        let store = InMemoryEventStore::new();

        let event1 = DomainEventEnum::Optimization(OptimizationEvent::PipelineConfigCreated {
            source_arch: "x86_64".to_string(),
            target_arch: "aarch64".to_string(),
            optimization_level: 2,
            stages_count: 5,
            occurred_at: std::time::SystemTime::UNIX_EPOCH,
        });

        let event2 = DomainEventEnum::Optimization(OptimizationEvent::StageCompleted {
            stage_name: "stage1".to_string(),
            execution_time_ms: 100,
            memory_usage_mb: 10.0,
            success: true,
            occurred_at: std::time::SystemTime::UNIX_EPOCH,
        });

        store.append(event1).unwrap();
        store.append(event2).unwrap();

        // Replay from sequence 1
        let events = store.replay(1).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].sequence_number, 1);
    }

    #[test]
    fn test_in_memory_event_store_query() {
        let store = InMemoryEventStore::new();

        let event = DomainEventEnum::Optimization(OptimizationEvent::PipelineConfigCreated {
            source_arch: "x86_64".to_string(),
            target_arch: "aarch64".to_string(),
            optimization_level: 2,
            stages_count: 5,
            occurred_at: std::time::SystemTime::UNIX_EPOCH,
        });

        store.append(event).unwrap();

        // Query with type filter
        let filter = EventFilter {
            event_type_pattern: Some("optimization.*".to_string()),
            ..Default::default()
        };

        let events = store.query(filter).unwrap();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_in_memory_event_store_clear() {
        let store = InMemoryEventStore::new();

        let event = DomainEventEnum::Optimization(OptimizationEvent::PipelineConfigCreated {
            source_arch: "x86_64".to_string(),
            target_arch: "aarch64".to_string(),
            optimization_level: 2,
            stages_count: 5,
            occurred_at: std::time::SystemTime::UNIX_EPOCH,
        });

        store.append(event).unwrap();
        store.clear().unwrap();

        let events = store.replay(0).unwrap();
        assert_eq!(events.len(), 0);
    }
}
