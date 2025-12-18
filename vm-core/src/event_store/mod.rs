//! Event store module
//!
//! This module provides various event store implementations for VM system,
//! including in-memory, file-based, and PostgreSQL-backed stores.

pub mod postgres_event_store;
pub mod file_event_store;
pub mod compatibility;

// Re-export main event store trait and types
pub use crate::event_store::{
    EventStore, StoredEvent, InMemoryEventStore
};

// Re-export new implementations
pub use postgres_event_store::{
    PostgresEventStore, PostgresEventStoreConfig, PostgresEventStoreBuilder
};

pub use file_event_store::{
    FileEventStore, FileEventStoreConfig, FileEventStoreBuilder
};

// Re-export compatibility adapters
pub use compatibility::{
    PostgresEventStoreAdapter, FileEventStoreAdapter, EnhancedStoredEvent
};