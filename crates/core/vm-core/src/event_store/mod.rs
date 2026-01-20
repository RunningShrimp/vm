//! Event store module
//!
//! This module provides various event store implementations for VM system,
//! including in-memory, file-based, and PostgreSQL-backed stores.

use crate::{VmError, VmResult};
use crate::jit::domain_events::DomainEventEnum;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

// PostgreSQL event store implementation
pub mod postgres_event_store;

// Temporarily disabled
// pub mod file_event_store;
pub mod compatibility;

/// 存储的事件记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEvent {
    pub sequence_number: u64,
    pub vm_id: String,
    pub event: DomainEventEnum,
    pub stored_at: SystemTime,
}

/// 事件存储trait
///
/// 定义事件存储的接口，支持事件的追加、查询和回放。
pub trait EventStore: Send + Sync {
    fn append(
        &self,
        vm_id: &str,
        sequence_number: Option<u64>,
        event: DomainEventEnum,
    ) -> VmResult<u64>;

    fn load_events(
        &self,
        vm_id: &str,
        from_sequence: Option<u64>,
        to_sequence: Option<u64>,
    ) -> VmResult<Vec<StoredEvent>>;

    fn get_last_sequence_number(&self, vm_id: &str) -> VmResult<u64>;

    fn get_event_count(&self, vm_id: &str) -> VmResult<usize>;

    fn list_vm_ids(&self) -> VmResult<Vec<String>>;

    fn delete_events(&self, vm_id: &str) -> VmResult<()>;
}

/// 内存事件存储实现
///
/// 用于测试和开发，事件存储在内存中。
pub struct InMemoryEventStore {
    events: Arc<RwLock<HashMap<String, Vec<StoredEvent>>>>,
    sequences: Arc<RwLock<HashMap<String, u64>>>,
}

impl InMemoryEventStore {
    pub fn new() -> Self {
        Self {
            events: Arc::new(RwLock::new(HashMap::new())),
            sequences: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryEventStore {
    fn default() -> Self {
        Self::new()
    }
}

impl EventStore for InMemoryEventStore {
    fn append(
        &self,
        vm_id: &str,
        sequence_number: Option<u64>,
        event: DomainEventEnum,
    ) -> VmResult<u64> {
        let mut events = self.events.write().map_err(|_| {
            VmError::Core(crate::CoreError::Concurrency {
                message: "Failed to acquire events write lock".to_string(),
                operation: "append".to_string(),
            })
        })?;
        let mut sequences = self.sequences.write().map_err(|_| {
            VmError::Core(crate::CoreError::Concurrency {
                message: "Failed to acquire sequences write lock".to_string(),
                operation: "append".to_string(),
            })
        })?;

        let seq = match sequence_number {
            Some(seq) => {
                let current_seq = sequences.get(vm_id).copied().unwrap_or(0);
                if seq <= current_seq {
                    return Err(VmError::Core(crate::CoreError::InvalidState {
                        message: format!(
                            "Sequence number {} is not greater than current {}",
                            seq, current_seq
                        ),
                        current: format!("{}", seq),
                        expected: format!("> {}", current_seq),
                    }));
                }
                seq
            }
            None => {
                sequences.get(vm_id).copied().unwrap_or(0) + 1
            }
        };

        let stored_event = StoredEvent {
            sequence_number: seq,
            vm_id: vm_id.to_string(),
            event,
            stored_at: SystemTime::now(),
        };

        events.entry(vm_id.to_string())
            .or_insert_with(Vec::new)
            .push(stored_event);

        sequences.insert(vm_id.to_string(), seq);

        Ok(seq)
    }

    fn load_events(
        &self,
        vm_id: &str,
        from_sequence: Option<u64>,
        to_sequence: Option<u64>,
    ) -> VmResult<Vec<StoredEvent>> {
        let events = self.events.read().map_err(|_| {
            VmError::Core(crate::CoreError::Concurrency {
                message: "Failed to acquire events read lock".to_string(),
                operation: "load_events".to_string(),
            })
        })?;

        let vm_events = events.get(vm_id).cloned().unwrap_or_default();

        let filtered: Vec<StoredEvent> = vm_events
            .into_iter()
            .filter(|e| {
                let seq = e.sequence_number;
                let from_ok = from_sequence.is_none_or(|f| seq >= f);
                let to_ok = to_sequence.is_none_or(|t| seq <= t);
                from_ok && to_ok
            })
            .collect();

        Ok(filtered)
    }

    fn get_last_sequence_number(&self, vm_id: &str) -> VmResult<u64> {
        let sequences = self.sequences.read().map_err(|_| {
            VmError::Core(crate::CoreError::Concurrency {
                message: "Failed to acquire sequences read lock".to_string(),
                operation: "get_last_sequence_number".to_string(),
            })
        })?;
        Ok(sequences.get(vm_id).copied().unwrap_or(0))
    }

    fn get_event_count(&self, vm_id: &str) -> VmResult<usize> {
        let events = self.events.read().map_err(|_| {
            VmError::Core(crate::CoreError::Concurrency {
                message: "Failed to acquire events read lock".to_string(),
                operation: "get_event_count".to_string(),
            })
        })?;
        Ok(events.get(vm_id).map(|e| e.len()).unwrap_or(0))
    }

    fn list_vm_ids(&self) -> VmResult<Vec<String>> {
        let events = self.events.read().map_err(|_| {
            VmError::Core(crate::CoreError::Concurrency {
                message: "Failed to acquire events read lock".to_string(),
                operation: "list_vm_ids".to_string(),
            })
        })?;
        Ok(events.keys().cloned().collect())
    }

    fn delete_events(&self, vm_id: &str) -> VmResult<()> {
        let mut events = self.events.write().map_err(|_| {
            VmError::Core(crate::CoreError::Concurrency {
                message: "Failed to acquire events write lock".to_string(),
                operation: "delete_events".to_string(),
            })
        })?;
        let mut sequences = self.sequences.write().map_err(|_| {
            VmError::Core(crate::CoreError::Concurrency {
                message: "Failed to acquire sequences write lock".to_string(),
                operation: "delete_events".to_string(),
            })
        })?;
        events.remove(vm_id);
        sequences.remove(vm_id);
        Ok(())
    }
}

// Re-export new implementations
pub use postgres_event_store::{
    PostgresEventStore, PostgresEventStoreConfig, PostgresEventStoreBuilder
};

// pub use file_event_store::{
//     FileEventStore, FileEventStoreConfig, FileEventStoreBuilder
// };

// Re-export compatibility adapters
pub use compatibility::{
    PostgresEventStoreAdapter, FileEventStoreAdapter, EnhancedStoredEvent
};