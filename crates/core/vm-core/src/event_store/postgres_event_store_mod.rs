//! PostgreSQL event store module exports
//!
//! This module re-exports all components of the PostgreSQL event store implementation.
//! It provides a clean interface for consumers to access the event store functionality.

pub use postgres_event_store_main::*;
pub use postgres_event_store_config::*;
pub use postgres_event_store_types::*;
pub use postgres_event_store_connection::*;
pub use postgres_event_store_migrations::*;
pub use postgres_event_store_queries::*;
pub use postgres_event_store_batch::*;
pub use postgres_event_store_compression::*;

// Re-export core types for backward compatibility
pub use crate::jit::event_store::{EventStore, StoredEvent, VmResult};
pub use crate::jit::domain_events::DomainEventEnum;
pub use crate::jit::error::{VmError, CoreError};