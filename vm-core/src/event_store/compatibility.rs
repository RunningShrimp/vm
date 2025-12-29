//! Event store compatibility adapters
//!
//! This module provides compatibility adapters to make new event store implementations
//! work with the existing EventStore trait interface.

#![cfg(feature = "enhanced-event-sourcing")]

use std::sync::Arc;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

use crate::jit::event_store::{EventStore, StoredEvent as LegacyStoredEvent, VmResult};
use crate::jit::domain_events::DomainEventEnum;
use crate::jit::error::{VmError, CoreError};
use super::{PostgresEventStore, FileEventStore};

/// Enhanced stored event with additional metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedStoredEvent {
    /// Event sequence number
    pub sequence_number: u64,
    /// Event type
    pub event_type: String,
    /// Event version
    pub event_version: i32,
    /// Serialized event data
    pub event_data: Vec<u8>,
    /// Event metadata
    pub metadata: String,
    /// When the event occurred
    pub occurred_at: DateTime<Utc>,
}

/// Convert from enhanced to legacy stored event
impl From<EnhancedStoredEvent> for LegacyStoredEvent {
    fn from(enhanced: EnhancedStoredEvent) -> Self {
        // Deserialize the event data to get the actual domain event
        let event: DomainEventEnum = bincode::deserialize(&enhanced.event_data)
            .unwrap_or_else(|_| {
                // Fallback to a default event if deserialization fails
                DomainEventEnum::VmLifecycle(crate::domain_events::VmLifecycleEvent::VmCreated {
                    vm_id: "unknown".to_string(),
                    config: crate::domain_events::VmConfigSnapshot::default(),
                    occurred_at: enhanced.occurred_at.into(),
                })
            });

        Self {
            sequence_number: enhanced.sequence_number,
            vm_id: enhanced.metadata, // Use metadata as vm_id for compatibility
            event,
            stored_at: enhanced.occurred_at.into(),
        }
    }
}

/// Convert from legacy to enhanced stored event
impl From<LegacyStoredEvent> for EnhancedStoredEvent {
    fn from(legacy: LegacyStoredEvent) -> Self {
        // Serialize the domain event
        let event_data = bincode::serialize(&legacy.event)
            .unwrap_or_default();

        Self {
            sequence_number: legacy.sequence_number,
            event_type: format!("{:?}", std::mem::discriminant(&legacy.event)),
            event_version: 1,
            event_data,
            metadata: legacy.vm_id.clone(),
            occurred_at: legacy.stored_at.into(),
        }
    }
}

/// PostgreSQL event store adapter for compatibility with existing EventStore trait
pub struct PostgresEventStoreAdapter {
    inner: PostgresEventStore,
}

impl PostgresEventStoreAdapter {
    /// Create a new adapter wrapping a PostgreSQL event store
    pub fn new(inner: PostgresEventStore) -> Self {
        Self { inner }
    }

    /// Get reference to inner event store
    pub fn inner(&self) -> &PostgresEventStore {
        &self.inner
    }

    /// Helper method to block on async operations, using Handle when available
    fn block_on_async<F, R>(&self, f: F) -> VmResult<R>
    where
        F: std::future::Future<Output = VmResult<R>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => handle.block_on(f),
            Err(_) => {
                // Only create a new runtime if we're not already in a tokio context
                tokio::runtime::Runtime::new()
                    .map_err(|e| VmError::Core(CoreError::IoError {
                        message: format!("Failed to create tokio runtime: {}", e),
                    }))?
                    .block_on(f)
            }
        }
    }
}

impl EventStore for PostgresEventStoreAdapter {
    fn append(
        &self,
        vm_id: &str,
        sequence_number: Option<u64>,
        event: DomainEventEnum,
    ) -> VmResult<u64> {
        // Convert to enhanced event format
        let enhanced = EnhancedStoredEvent::from(LegacyStoredEvent {
            sequence_number: sequence_number.unwrap_or(0),
            vm_id: vm_id.to_string(),
            event,
            stored_at: std::time::SystemTime::now(),
        });

        // Store using helper method
        self.block_on_async(async {
            self.inner.store_events(vm_id, vec![enhanced]).await
        })?;

        // Return the sequence number
        Ok(enhanced.sequence_number)
    }

    fn load_events(
        &self,
        vm_id: &str,
        from_sequence: Option<u64>,
        to_sequence: Option<u64>,
    ) -> VmResult<Vec<LegacyStoredEvent>> {
        let enhanced_events = self.block_on_async(async {
            if let Some(to_seq) = to_sequence {
                self.inner.get_events_range(vm_id, from_sequence.unwrap_or(0), to_seq).await
            } else {
                self.inner.get_events(vm_id, from_sequence.unwrap_or(0)).await
            }
        })?;

        // Convert to legacy format
        let legacy_events: Vec<LegacyStoredEvent> = enhanced_events
            .into_iter()
            .map(|e| {
                let mut legacy = LegacyStoredEvent::from(e);
                // Ensure vm_id is set correctly
                legacy.vm_id = vm_id.to_string();
                legacy
            })
            .collect();

        Ok(legacy_events)
    }

    fn get_last_sequence_number(&self, vm_id: &str) -> VmResult<u64> {
        self.block_on_async(async {
            self.inner.get_last_sequence_number(vm_id).await
        }).map(|opt| opt.unwrap_or(0))
    }

    fn get_event_count(&self, vm_id: &str) -> VmResult<usize> {
        self.block_on_async(async {
            self.inner.get_event_count(vm_id).await
        }).map(|count| count as usize)
    }

    fn list_vm_ids(&self) -> VmResult<Vec<String>> {
        self.block_on_async(async {
            self.inner.list_vms().await
        })
    }

    fn delete_events(&self, vm_id: &str) -> VmResult<()> {
        self.block_on_async(async {
            // Get current event count to determine deletion range
            let event_count = self.inner.get_event_count(vm_id).await?;
            if event_count > 0 {
                self.inner.delete_events(vm_id, event_count).await?;
            }
            Ok::<(), VmError>(())
        })?;

        Ok(())
    }
}

/// File event store adapter for compatibility with existing EventStore trait
pub struct FileEventStoreAdapter {
    inner: FileEventStore,
}

impl FileEventStoreAdapter {
    /// Create a new adapter wrapping a file event store
    pub fn new(inner: FileEventStore) -> Self {
        Self { inner }
    }

    /// Get reference to inner event store
    pub fn inner(&self) -> &FileEventStore {
        &self.inner
    }

    /// Helper method to block on async operations, using Handle when available
    fn block_on_async<F, R>(&self, f: F) -> VmResult<R>
    where
        F: std::future::Future<Output = VmResult<R>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => handle.block_on(f),
            Err(_) => {
                // Only create a new runtime if we're not already in a tokio context
                tokio::runtime::Runtime::new()
                    .map_err(|e| VmError::Core(CoreError::IoError {
                        message: format!("Failed to create tokio runtime: {}", e),
                    }))?
                    .block_on(f)
            }
        }
    }
}

impl EventStore for FileEventStoreAdapter {
    fn append(
        &self,
        vm_id: &str,
        sequence_number: Option<u64>,
        event: DomainEventEnum,
    ) -> VmResult<u64> {
        // Convert to enhanced event format
        let enhanced = EnhancedStoredEvent::from(LegacyStoredEvent {
            sequence_number: sequence_number.unwrap_or(0),
            vm_id: vm_id.to_string(),
            event,
            stored_at: std::time::SystemTime::now(),
        });

        // Store using helper method
        self.block_on_async(async {
            self.inner.store_events(vm_id, vec![enhanced]).await
        })?;

        // Return the sequence number
        Ok(enhanced.sequence_number)
    }

    fn load_events(
        &self,
        vm_id: &str,
        from_sequence: Option<u64>,
        to_sequence: Option<u64>,
    ) -> VmResult<Vec<LegacyStoredEvent>> {
        let enhanced_events = self.block_on_async(async {
            if let Some(to_seq) = to_sequence {
                self.inner.get_events_range(vm_id, from_sequence.unwrap_or(0), to_seq).await
            } else {
                self.inner.get_events(vm_id, from_sequence.unwrap_or(0)).await
            }
        })?;

        // Convert to legacy format
        let legacy_events: Vec<LegacyStoredEvent> = enhanced_events
            .into_iter()
            .map(|e| {
                let mut legacy = LegacyStoredEvent::from(e);
                // Ensure vm_id is set correctly
                legacy.vm_id = vm_id.to_string();
                legacy
            })
            .collect();

        Ok(legacy_events)
    }

    fn get_last_sequence_number(&self, vm_id: &str) -> VmResult<u64> {
        self.block_on_async(async {
            self.inner.get_last_sequence_number(vm_id).await
        }).map(|opt| opt.unwrap_or(0))
    }

    fn get_event_count(&self, vm_id: &str) -> VmResult<usize> {
        self.block_on_async(async {
            self.inner.get_event_count(vm_id).await
        }).map(|count| count as usize)
    }

    fn list_vm_ids(&self) -> VmResult<Vec<String>> {
        self.block_on_async(async {
            self.inner.list_vms().await
        })
    }

    fn delete_events(&self, vm_id: &str) -> VmResult<()> {
        self.block_on_async(async {
            // Get current event count to determine deletion range
            let event_count = self.inner.get_event_count(vm_id).await?;
            if event_count > 0 {
                self.inner.delete_events(vm_id, event_count).await?;
            }
            Ok::<(), VmError>(())
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jit::event_store::InMemoryEventStore;
    use crate::jit::domain_events::{DomainEventEnum, VmLifecycleEvent};

    #[test]
    fn test_enhanced_to_legacy_conversion() {
        let enhanced = EnhancedStoredEvent {
            sequence_number: 1,
            event_type: "VmLifecycle".to_string(),
            event_version: 1,
            event_data: bincode::serialize(&DomainEventEnum::VmLifecycle(
                VmLifecycleEvent::VmCreated {
                    vm_id: "test_vm".to_string(),
                    config: crate::domain_events::VmConfigSnapshot::default(),
                    occurred_at: std::time::SystemTime::now(),
                }
            )).unwrap_or_default(),
            metadata: "test_vm".to_string(),
            occurred_at: Utc::now(),
        };

        let legacy = LegacyStoredEvent::from(enhanced.clone());
        assert_eq!(legacy.sequence_number, enhanced.sequence_number);
        assert_eq!(legacy.vm_id, enhanced.metadata);
    }

    #[test]
    fn test_legacy_to_enhanced_conversion() {
        let legacy = LegacyStoredEvent {
            sequence_number: 1,
            vm_id: "test_vm".to_string(),
            event: DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmCreated {
                vm_id: "test_vm".to_string(),
                config: crate::domain_events::VmConfigSnapshot::default(),
                occurred_at: std::time::SystemTime::now(),
            }),
            stored_at: std::time::SystemTime::now(),
        };

        let enhanced = EnhancedStoredEvent::from(legacy.clone());
        assert_eq!(enhanced.sequence_number, legacy.sequence_number);
        assert_eq!(enhanced.metadata, legacy.vm_id);
    }
}