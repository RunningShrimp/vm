//! PostgreSQL event store implementation
//!
//! This module provides a PostgreSQL-backed event store implementation for the VM system.
//! It combines all sub-modules into a cohesive interface that implements the EventStore trait.
//!
//! ## Module Structure
//!
//! The implementation is split into focused modules:
//! - `postgres_event_store_config` - Configuration and builder
//! - `postgres_event_store_types` - Common types and enums
//! - `postgres_event_store_connection` - Connection management
//! - `postgres_event_store_migrations` - Database schema migrations
//! - `postgres_event_store_queries` - SQL query operations
//! - `postgres_event_store_batch` - Batch operations
//! - `postgres_event_store_compression` - Data compression
//! - `postgres_event_store_main` - Main implementation
//!
//! ## Usage
//!
//! ```rust,no_run
//! use vm_core::jit::event_store::postgres_event_store::{PostgresEventStore, PostgresEventStoreConfig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = PostgresEventStoreConfig::builder()
//!     .connection_url("postgresql://localhost/vm_events")
//!     .build()
//!     .unwrap();
//!
//! let store = PostgresEventStore::new(config).await?;
//! # Ok(())
//! # }
//! ```

// Re-export all sub-modules
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
