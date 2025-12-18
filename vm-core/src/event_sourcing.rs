//! Enhanced event sourcing service
//!
//! This module provides a comprehensive event sourcing service that combines
//! persistent event storage with snapshot optimization for efficient state reconstruction.

use std::sync::Arc;
// use async_trait::async_trait; // Commented out for now
use serde::{Deserialize, Serialize};
// use chrono::{DateTime, Utc}; // Commented out for now

#[cfg(feature = "enhanced-event-sourcing")]
use crate::event_store::{EventStore, StoredEvent, VmResult};
#[cfg(feature = "enhanced-event-sourcing")]
use crate::snapshot::{SnapshotStore, SnapshotManager, SnapshotConfig, SnapshotData, SnapshotMetadata};
use crate::aggregate_root::{VirtualMachineAggregate, AggregateRoot};
use crate::domain_events::DomainEventEnum;
use crate::error::{VmError, CoreError};
#[cfg(feature = "enhanced-event-sourcing")]
use chrono::{DateTime, Utc};

/// Enhanced event sourcing configuration
#[cfg(feature = "enhanced-event-sourcing")]
#[derive(Debug, Clone)]
pub struct EventSourcingConfig {
    /// Event store to use
    pub event_store: Arc<dyn EventStore>,
    /// Snapshot store to use
    pub snapshot_store: Box<dyn SnapshotStore>,
    /// Snapshot configuration
    pub snapshot_config: SnapshotConfig,
    /// Enable automatic snapshot creation
    pub auto_snapshot: bool,
    /// Maximum events to replay before creating snapshot
    pub max_events_before_snapshot: u64,
    /// Enable event compression
    pub enable_compression: bool,
}

#[cfg(feature = "enhanced-event-sourcing")]
impl Default for EventSourcingConfig {
    fn default() -> Self {
        Self {
            event_store: Arc::new(crate::event_store::InMemoryEventStore::new()),
            snapshot_store: Box::new(crate::snapshot::FileSnapshotStore::new(
                SnapshotConfig::default()
            ).blocking_recv().unwrap()),
            snapshot_config: SnapshotConfig::default(),
            auto_snapshot: true,
            max_events_before_snapshot: 1000,
            enable_compression: true,
        }
    }
}

/// Event sourcing statistics
#[cfg(feature = "enhanced-event-sourcing")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSourcingStats {
    /// Total number of events stored
    pub total_events: u64,
    /// Number of snapshots created
    pub snapshot_count: u64,
    /// Size of event store in bytes
    pub event_store_size_bytes: u64,
    /// Size of snapshot store in bytes
    pub snapshot_store_size_bytes: u64,
    /// Average events per snapshot
    pub avg_events_per_snapshot: f64,
    /// Last snapshot timestamp
    pub last_snapshot_at: Option<DateTime<Utc>>,
    /// Event replay performance (events/second)
    pub replay_performance_events_per_sec: f64,
}

/// Enhanced event sourcing service
/// 
/// This service provides comprehensive event sourcing capabilities including:
/// - Persistent event storage with multiple backend options
/// - Automatic snapshot creation and management
/// - Optimized event replay using snapshots
/// - Performance monitoring and statistics
/// - Event compression and optimization
#[cfg(feature = "enhanced-event-sourcing")]
pub struct EnhancedEventSourcingService {
    config: EventSourcingConfig,
    snapshot_manager: SnapshotManager,
    stats: Arc<std::sync::RwLock<EventSourcingStats>>,
}

#[cfg(feature = "enhanced-event-sourcing")]
impl EnhancedEventSourcingService {
    /// Create a new enhanced event sourcing service
    pub async fn new(config: EventSourcingConfig) -> VmResult<Self> {
        let snapshot_manager = SnapshotManager::new(
            config.snapshot_store.clone(),
            config.event_store.clone(),
            config.snapshot_config.clone(),
        );

        let service = Self {
            config: config.clone(),
            snapshot_manager,
            stats: Arc::new(std::sync::RwLock::new(EventSourcingStats {
                total_events: 0,
                snapshot_count: 0,
                event_store_size_bytes: 0,
                snapshot_store_size_bytes: 0,
                avg_events_per_snapshot: 0.0,
                last_snapshot_at: None,
                replay_performance_events_per_sec: 0.0,
            })),
        };

        // Initialize statistics
        service.update_stats().await?;

        Ok(service)
    }

    /// Store events for a VM
    pub async fn store_events(&self, vm_id: &str, events: Vec<DomainEventEnum>) -> VmResult<u64> {
        if events.is_empty() {
            return Ok(0);
        }

        // Get current sequence number
        let current_sequence = self.config.event_store.get_last_sequence_number(vm_id)?;
        
        // Store events
        let mut stored_count = 0;
        for (i, event) in events.into_iter().enumerate() {
            let sequence = current_sequence + i as u64 + 1;
            self.config.event_store.append(vm_id, Some(sequence), event)?;
            stored_count += 1;
        }

        // Check if we should create a snapshot
        if self.config.auto_snapshot {
            let new_sequence = current_sequence + stored_count;
            if self.should_create_snapshot(vm_id, new_sequence).await? {
                self.create_snapshot_for_vm(vm_id).await?;
            }
        }

        // Update statistics
        self.update_stats().await?;

        Ok(stored_count)
    }

    /// Load aggregate state for a VM
    pub async fn load_aggregate(&self, vm_id: &str, config: crate::VmConfig) -> VmResult<VirtualMachineAggregate> {
        // Try to load from latest snapshot first
        if let Ok(aggregate) = self.snapshot_manager.restore_from_snapshot(vm_id, None).await {
            return Ok(aggregate);
        }

        // If no snapshot exists, create from scratch and replay all events
        let mut aggregate = VirtualMachineAggregate::new(vm_id.to_string(), config);
        
        // Load all events
        let events = self.config.event_store.load_events(vm_id, None, None)?;
        
        // Apply events to aggregate
        for event in events {
            aggregate.apply_event(event)?;
        }

        Ok(aggregate)
    }

    /// Replay events from a specific point in time
    pub async fn replay_events(
        &self,
        vm_id: &str,
        from_sequence: Option<u64>,
        to_sequence: Option<u64>,
    ) -> VmResult<Vec<StoredEvent>> {
        let start_time = std::time::Instant::now();
        
        // Load events from store
        let events = self.config.event_store.load_events(vm_id, from_sequence, to_sequence)?;
        
        // Update replay performance statistics
        let elapsed = start_time.elapsed();
        let events_per_sec = if elapsed.as_secs_f64() > 0.0 {
            events.len() as f64 / elapsed.as_secs_f64()
        } else {
            0.0
        };

        let mut stats = self.stats.write().await;
        stats.replay_performance_events_per_sec = events_per_sec;

        Ok(events)
    }

    /// Create a snapshot for a VM
    pub async fn create_snapshot_for_vm(&self, vm_id: &str) -> VmResult<SnapshotMetadata> {
        // Load current aggregate state
        let config = crate::VmConfig::default(); // In real implementation, load from repository
        let aggregate = self.load_aggregate(vm_id, config).await?;
        
        // Create snapshot
        let metadata = self.snapshot_manager.create_snapshot(&aggregate).await?;
        
        // Update statistics
        self.update_stats().await?;
        
        Ok(metadata)
    }

    /// Restore VM state from a specific snapshot
    pub async fn restore_from_snapshot(
        &self,
        vm_id: &str,
        snapshot_version: Option<u64>,
    ) -> VmResult<VirtualMachineAggregate> {
        let aggregate = self.snapshot_manager.restore_from_snapshot(vm_id, snapshot_version).await?;
        Ok(aggregate)
    }

    /// Get event sourcing statistics
    pub async fn get_stats(&self) -> EventSourcingStats {
        self.stats.read().await.clone()
    }

    /// Get VM-specific statistics
    pub async fn get_vm_stats(&self, vm_id: &str) -> VmResult<VmEventSourcingStats> {
        let event_count = self.config.event_store.get_event_count(vm_id)?;
        let last_sequence = self.config.event_store.get_last_sequence_number(vm_id)?;
        let snapshot_stats = self.snapshot_manager.get_snapshot_stats(vm_id).await?;
        
        Ok(VmEventSourcingStats {
            vm_id: vm_id.to_string(),
            event_count,
            last_sequence,
            snapshot_count: snapshot_stats.snapshot_count,
            total_snapshot_size_bytes: snapshot_stats.total_size_bytes,
            latest_snapshot_version: snapshot_stats.latest_version,
            last_snapshot_at: snapshot_stats.newest_snapshot,
        })
    }

    /// Delete all events and snapshots for a VM
    pub async fn delete_vm(&self, vm_id: &str) -> VmResult<()> {
        // Delete events
        self.config.event_store.delete_events(vm_id)?;
        
        // Delete all snapshots
        let snapshots = self.snapshot_manager.snapshot_store.list_snapshots(vm_id).await?;
        for snapshot in snapshots {
            self.snapshot_manager.snapshot_store.delete_snapshot(vm_id, snapshot.snapshot_version).await?;
        }
        
        // Update statistics
        self.update_stats().await?;
        
        Ok(())
    }

    /// Compact event store by removing events that are included in snapshots
    pub async fn compact_event_store(&self, vm_id: &str) -> VmResult<u64> {
        // Get latest snapshot
        let latest_snapshot = self.snapshot_manager.snapshot_store.get_latest_snapshot(vm_id).await?;
        
        if let Some(snapshot) = latest_snapshot {
            // Delete events up to snapshot version
            let deleted_count = self.config.event_store.delete_events(vm_id, snapshot.metadata.snapshot_version)?;
            
            // Update statistics
            self.update_stats().await?;
            
            Ok(deleted_count)
        } else {
            Ok(0)
        }
    }

    /// Check if snapshot should be created
    async fn should_create_snapshot(&self, vm_id: &str, event_count: u64) -> VmResult<bool> {
        // Check event count threshold
        if event_count % self.config.max_events_before_snapshot == 0 {
            return Ok(true);
        }

        // Check if we have any snapshots at all
        let snapshot_count = self.snapshot_manager.snapshot_store.get_snapshot_count(vm_id).await?;
        if snapshot_count == 0 {
            return Ok(true);
        }

        // Check time-based snapshot creation
        if let Some(last_snapshot) = self.stats.read().await.last_snapshot_at {
            let hours_since_snapshot = (Utc::now() - last_snapshot).num_hours();
            if hours_since_snapshot >= 24 { // Create snapshot at least once per day
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Update event sourcing statistics
    async fn update_stats(&self) -> VmResult<()> {
        // This is a simplified implementation
        // In a real system, you would gather more detailed statistics
        
        let mut stats = self.stats.write().await;
        
        // Update snapshot timestamp
        if stats.last_snapshot_at.is_none() {
            stats.last_snapshot_at = Some(Utc::now());
        }
        
        Ok(())
    }
}

/// VM-specific event sourcing statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(feature = "enhanced-event-sourcing")]
pub struct VmEventSourcingStats {
    /// VM ID
    pub vm_id: String,
    /// Number of events for this VM
    pub event_count: usize,
    /// Last event sequence number
    pub last_sequence: u64,
    /// Number of snapshots for this VM
    pub snapshot_count: u64,
    /// Total size of all snapshots in bytes
    pub total_snapshot_size_bytes: u64,
    /// Version of the latest snapshot
    pub latest_snapshot_version: u64,
    /// When the latest snapshot was created
    pub last_snapshot_at: Option<DateTime<Utc>>,
}

/// Event sourcing service builder for easier configuration
#[cfg(feature = "enhanced-event-sourcing")]
pub struct EventSourcingServiceBuilder {
    config: EventSourcingConfig,
}

#[cfg(feature = "enhanced-event-sourcing")]
impl EventSourcingServiceBuilder {
    /// Create a new builder with default configuration
    pub fn new() -> Self {
        Self {
#[cfg(feature = "enhanced-event-sourcing")]
#[cfg(feature = "enhanced-event-sourcing")]
            config: EventSourcingBuilder::default(),
        }
    }

    /// Set event store
    pub fn event_store(mut self, event_store: Arc<dyn EventStore>) -> Self {
        self.config.event_store = event_store;
        self
    }

    /// Set snapshot store
    pub fn snapshot_store(mut self, snapshot_store: Box<dyn SnapshotStore>) -> Self {
        self.config.snapshot_store = snapshot_store;
        self
    }

    /// Set snapshot configuration
    pub fn snapshot_config(mut self, config: SnapshotConfig) -> Self {
        self.config.snapshot_config = config;
        self
    }

    /// Enable or disable automatic snapshot creation
    pub fn auto_snapshot(mut self, enable: bool) -> Self {
        self.config.auto_snapshot = enable;
        self
    }

    /// Set maximum events before snapshot creation
    pub fn max_events_before_snapshot(mut self, max_events: u64) -> Self {
        self.config.max_events_before_snapshot = max_events;
        self
    }

    /// Enable or disable event compression
    pub fn enable_compression(mut self, enable: bool) -> Self {
        self.config.enable_compression = enable;
        self
    }

    /// Build enhanced event sourcing service
    #[cfg(feature = "enhanced-event-sourcing")]
    pub async fn build(self) -> VmResult<EnhancedEventSourcingService> {
        #[cfg(feature = "enhanced-event-sourcing")]
        EventSourcingBuilder::default().build(self.config).await
    }
}

#[cfg(feature = "enhanced-event-sourcing")]
impl Default for EventSourcingServiceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_store::InMemoryEventStore;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_event_sourcing_service_creation() {
        let event_store = Arc::new(InMemoryEventStore::new());
        let temp_dir = TempDir::new().unwrap();
        
        let snapshot_config = SnapshotConfig {
            snapshot_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
        
        let snapshot_store = Box::new(crate::snapshot::FileSnapshotStore::new(snapshot_config).await.unwrap());
        
        let config = EventSourcingConfig {
            event_store,
            snapshot_store,
            snapshot_config: SnapshotConfig::default(),
            auto_snapshot: true,
            max_events_before_snapshot: 10,
            enable_compression: false,
        };
        
        #[cfg(feature = "enhanced-event-sourcing")]
        let service = EventSourcingBuilder::default().build(config).await.unwrap();
        let stats = service.get_stats().await;
        assert_eq!(stats.total_events, 0);
        assert_eq!(stats.snapshot_count, 0);
    }

    #[test]
    fn test_event_sourcing_service_builder() {
        let event_store = Arc::new(InMemoryEventStore::new());
        let temp_dir = TempDir::new().unwrap();
        
        let builder = EventSourcingServiceBuilder::new()
            .event_store(event_store.clone())
            .auto_snapshot(true)
            .max_events_before_snapshot(500)
            .enable_compression(true);
        
        assert_eq!(builder.config.event_store.as_ptr(), event_store.as_ptr());
        assert!(builder.config.auto_snapshot);
        assert_eq!(builder.config.max_events_before_snapshot, 500);
        assert!(builder.config.enable_compression);
    }
}
