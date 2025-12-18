//! Enhanced snapshot functionality for event sourcing
//!
//! This module provides comprehensive snapshot management including
//! snapshot creation, persistence, and snapshot-based event replay optimization.

#[cfg(feature = "enhanced-event-sourcing")]
use std::sync::Arc;
#[cfg(feature = "enhanced-event-sourcing")]
use std::collections::HashMap;
#[cfg(feature = "enhanced-event-sourcing")]
use std::path::{Path, PathBuf};
#[cfg(feature = "async")]
use async_trait::async_trait;
#[cfg(feature = "async")]
use tokio::sync::RwLock;
#[cfg(feature = "enhanced-event-sourcing")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "enhanced-event-sourcing")]
use chrono::{DateTime, Utc};
#[cfg(feature = "enhanced-event-sourcing")]
use bincode::serde::{serialize, deserialize};

#[cfg(feature = "enhanced-event-sourcing")]
use crate::error::{VmError, CoreError};

// 暂时注释掉不存在的模块引用
// use crate::aggregate_root::{VirtualMachineAggregate, AggregateRoot};
// use crate::event_store::{EventStore, StoredEvent, VmResult};

/// Snapshot configuration
#[cfg(feature = "enhanced-event-sourcing")]
#[derive(Debug, Clone)]
pub struct SnapshotConfig {
    /// Directory for snapshot storage
    pub snapshot_dir: PathBuf,
    /// Create snapshot after N events
    pub snapshot_interval: u64,
    /// Maximum number of snapshots to keep per VM
    pub max_snapshots_per_vm: usize,
    /// Enable snapshot compression
    pub enable_compression: bool,
    /// Snapshot retention period in days
    pub retention_days: u32,
    /// Enable automatic snapshot creation
    pub auto_snapshot: bool,
}

#[cfg(feature = "enhanced-event-sourcing")]
impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            snapshot_dir: PathBuf::from("./vm_snapshots"),
            snapshot_interval: 1000, // Create snapshot every 1000 events
            max_snapshots_per_vm: 10,
            enable_compression: true,
            retention_days: 30,
            auto_snapshot: true,
        }
    }
}

/// Snapshot metadata
#[cfg(feature = "enhanced-event-sourcing")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    /// VM ID this snapshot belongs to
    pub vm_id: String,
    /// Snapshot version (sequence number of last event)
    pub snapshot_version: u64,
    /// Snapshot creation timestamp
    pub created_at: DateTime<Utc>,
    /// Number of events included in this snapshot
    pub event_count: u64,
    /// Snapshot file size in bytes
    pub file_size: u64,
    /// Whether snapshot is compressed
    pub compressed: bool,
    /// Snapshot checksum for integrity verification
    pub checksum: String,
    /// Additional metadata
    pub metadata: String,
}

/// Snapshot data
#[cfg(feature = "enhanced-event-sourcing")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotData {
    /// Aggregate state data
    pub aggregate_data: Vec<u8>,
    /// Snapshot metadata
    pub metadata: SnapshotMetadata,
}

/// Snapshot store trait
#[cfg(feature = "enhanced-event-sourcing")]
#[async_trait]
pub trait SnapshotStore: Send + Sync {
    /// Store a snapshot
    async fn store_snapshot(&self, snapshot: SnapshotData) -> VmResult<()>;
    
    /// Retrieve the latest snapshot for a VM
    async fn get_latest_snapshot(&self, vm_id: &str) -> VmResult<Option<SnapshotData>>;
    
    /// Retrieve a specific snapshot version
    async fn get_snapshot(&self, vm_id: &str, version: u64) -> VmResult<Option<SnapshotData>>;
    
    /// List all snapshots for a VM
    async fn list_snapshots(&self, vm_id: &str) -> VmResult<Vec<SnapshotMetadata>>;
    
    /// Delete a snapshot
    async fn delete_snapshot(&self, vm_id: &str, version: u64) -> VmResult<()>;
    
    /// Delete old snapshots based on retention policy
    async fn cleanup_old_snapshots(&self, vm_id: &str) -> VmResult<u64>;
    
    /// Get snapshot count for a VM
    async fn get_snapshot_count(&self, vm_id: &str) -> VmResult<u64>;
}

/// File-based snapshot store implementation
#[cfg(feature = "enhanced-event-sourcing")]
pub struct FileSnapshotStore {
    config: SnapshotConfig,
    /// Cache of snapshot metadata
    metadata_cache: Arc<RwLock<HashMap<String, Vec<SnapshotMetadata>>>>,
}

#[cfg(feature = "enhanced-event-sourcing")]
impl FileSnapshotStore {
    /// Create a new file-based snapshot store
    pub async fn new(config: SnapshotConfig) -> VmResult<Self> {
        // Create snapshot directory if it doesn't exist
        std::fs::create_dir_all(&config.snapshot_dir)
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Failed to create snapshot directory: {}", e),
            }))?;

        let store = Self {
            config: config.clone(),
            metadata_cache: Arc::new(RwLock::new(HashMap::new())),
        };

        // Load existing snapshot metadata
        store.load_metadata().await?;

        Ok(store)
    }

    /// Load existing snapshot metadata from disk
    async fn load_metadata(&self) -> VmResult<()> {
        let entries = std::fs::read_dir(&self.config.snapshot_dir)
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Failed to read snapshot directory: {}", e),
            }))?;

        for entry in entries {
            let entry = entry
                .map_err(|e| VmError::Core(CoreError::IoError {
                    message: format!("Failed to read directory entry: {}", e),
                }))?;

            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("meta") {
                self.load_metadata_file(&path).await?;
            }
        }

        Ok(())
    }

    /// Load metadata from a .meta file
    async fn load_metadata_file(&self, path: &Path) -> VmResult<()> {
        let mut file = std::fs::File::open(path)
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Failed to open metadata file: {}", e),
            }))?;

        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Failed to read metadata file: {}", e),
            }))?;

#[cfg(feature = "enhanced-event-sourcing")]
#[cfg(feature = "enhanced-event-sourcing")]
        let metadata: SnapshotMetadata = serde_json::from_str(&contents)
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Failed to parse metadata file: {}", e),
            }))?;

        let mut metadata_cache = self.metadata_cache.write().await;
        metadata_cache.entry(metadata.vm_id.clone())
            .or_insert_with(Vec::new)
            .push(metadata);

        Ok(())
    }

    /// Get snapshot file path
    fn get_snapshot_file_path(&self, vm_id: &str, version: u64) -> PathBuf {
        self.config.snapshot_dir.join(format!("{}_snapshot_{:010}.snap", vm_id, version))
    }

    /// Get metadata file path
    fn get_metadata_file_path(&self, vm_id: &str, version: u64) -> PathBuf {
        self.config.snapshot_dir.join(format!("{}_snapshot_{:010}.meta", vm_id, version))
    }

    /// Compress snapshot data if compression is enabled
    fn compress_snapshot_data(&self, data: &[u8]) -> VmResult<Vec<u8>> {
        if self.config.enable_compression {
            let compressed = miniz_oxide::deflate::compress_to_vec_zlib(data, 6)
                .map_err(|e| VmError::Core(CoreError::IoError {
                    message: format!("Failed to compress snapshot data: {:?}", e),
                }))?;
            Ok(compressed)
        } else {
            Ok(data.to_vec())
        }
    }

    /// Decompress snapshot data if compression was used
    fn decompress_snapshot_data(&self, data: &[u8]) -> VmResult<Vec<u8>> {
        if self.config.enable_compression {
            let decompressed = miniz_oxide::inflate::decompress_to_vec_zlib(data)
                .map_err(|e| VmError::Core(CoreError::IoError {
                    message: format!("Failed to decompress snapshot data: {:?}", e),
                }))?;
            Ok(decompressed)
        } else {
            Ok(data.to_vec())
        }
    }

    /// Calculate checksum for snapshot data
    fn calculate_checksum(&self, data: &[u8]) -> String {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// Update metadata cache
    async fn update_metadata_cache(&self, metadata: SnapshotMetadata) {
        let mut cache = self.metadata_cache.write().await;
        let vm_snapshots = cache.entry(metadata.vm_id.clone()).or_insert_with(Vec::new);
        
        // Remove existing entry with same version if present
        vm_snapshots.retain(|m| m.snapshot_version != metadata.snapshot_version);
        
        // Add new metadata
        vm_snapshots.push(metadata);
        
        // Sort by version
        vm_snapshots.sort_by_key(|m| m.snapshot_version);
    }
}

#[async_trait]
#[cfg(feature = "enhanced-event-sourcing")]
impl SnapshotStore for FileSnapshotStore {
    async fn store_snapshot(&self, snapshot: SnapshotData) -> VmResult<()> {
        let vm_id = &snapshot.metadata.vm_id;
        let version = snapshot.metadata.snapshot_version;

        // Compress snapshot data if enabled
        let compressed_data = self.compress_snapshot_data(&snapshot.aggregate_data)?;

        // Write snapshot file
        let snapshot_path = self.get_snapshot_file_path(vm_id, version);
        std::fs::write(&snapshot_path, compressed_data)
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Failed to write snapshot file: {}", e),
            }))?;

        // Write metadata file
        let metadata_path = self.get_metadata_file_path(vm_id, version);
#[cfg(feature = "enhanced-event-sourcing")]
#[cfg(feature = "enhanced-event-sourcing")]
        let metadata_json = serde_json::to_string_pretty(&snapshot.metadata)
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Failed to serialize metadata: {}", e),
            }))?;

        let metadata_json = serde_json::to_string(&metadata)
                .map_err(|e| VmError::Core(CoreError::IoError {
                    message: format!("Failed to serialize metadata: {}", e),
                }))?;
            std::fs::write(&metadata_path, &metadata_json)
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Failed to write metadata file: {}", e),
            }))?;

        // Update metadata cache
        self.update_metadata_cache(snapshot.metadata.clone()).await;

        // Clean up old snapshots if needed
        self.cleanup_old_snapshots(vm_id).await?;

        Ok(())
    }

    async fn get_latest_snapshot(&self, vm_id: &str) -> VmResult<Option<SnapshotData>> {
        let metadata_cache = self.metadata_cache.read().await;
        
        if let Some(snapshots) = metadata_cache.get(vm_id) {
            if let Some(latest_metadata) = snapshots.last() {
                return self.get_snapshot(vm_id, latest_metadata.snapshot_version).await;
            }
        }

        Ok(None)
    }

    async fn get_snapshot(&self, vm_id: &str, version: u64) -> VmResult<Option<SnapshotData>> {
        let snapshot_path = self.get_snapshot_file_path(vm_id, version);
        
        if !snapshot_path.exists() {
            return Ok(None);
        }

        // Read snapshot data
        let compressed_data = std::fs::read(&snapshot_path)
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Failed to read snapshot file: {}", e),
            }))?;

        // Decompress if needed
        let aggregate_data = self.decompress_snapshot_data(&compressed_data)?;

        // Read metadata
        let metadata_path = self.get_metadata_file_path(vm_id, version);
        let metadata_json = std::fs::read_to_string(&metadata_path)
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Failed to read metadata file: {}", e),
            }))?;

        #[cfg(feature = "enhanced-event-sourcing")]
        let metadata: SnapshotMetadata = serde_json::from_str(&metadata_json)
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Failed to parse metadata: {}", e),
            }))?;

        // Verify checksum
        let calculated_checksum = self.calculate_checksum(&aggregate_data);
        if calculated_checksum != metadata.checksum {
            return Err(VmError::Core(CoreError::InvalidState {
                message: "Snapshot checksum verification failed".to_string(),
                current: calculated_checksum,
                expected: metadata.checksum.clone()
            }));
        }

        Ok(Some(SnapshotData {
            aggregate_data,
            metadata,
        }))
    }

    async fn list_snapshots(&self, vm_id: &str) -> VmResult<Vec<SnapshotMetadata>> {
        let metadata_cache = self.metadata_cache.read().await;
        
        if let Some(snapshots) = metadata_cache.get(vm_id) {
            Ok(snapshots.clone())
        } else {
            Ok(Vec::new())
        }
    }

    async fn delete_snapshot(&self, vm_id: &str, version: u64) -> VmResult<()> {
        // Delete snapshot file
        let snapshot_path = self.get_snapshot_file_path(vm_id, version);
        if snapshot_path.exists() {
            std::fs::remove_file(&snapshot_path)
                .map_err(|e| VmError::Core(CoreError::IoError {
                    message: format!("Failed to delete snapshot file: {}", e),
                }))?;
        }

        // Delete metadata file
        let metadata_path = self.get_metadata_file_path(vm_id, version);
        if metadata_path.exists() {
            std::fs::remove_file(&metadata_path)
                .map_err(|e| VmError::Core(CoreError::IoError {
                    message: format!("Failed to delete metadata file: {}", e),
                }))?;
        }

        // Update metadata cache
        let mut cache = self.metadata_cache.write().await;
        if let Some(snapshots) = cache.get_mut(vm_id) {
            snapshots.retain(|m| m.snapshot_version != version);
        }

        Ok(())
    }

    async fn cleanup_old_snapshots(&self, vm_id: &str) -> VmResult<u64> {
        let mut deleted_count = 0u64;
        let metadata_cache = self.metadata_cache.read().await;

        if let Some(snapshots) = metadata_cache.get(vm_id) {
#[cfg(feature = "enhanced-event-sourcing")]
            #[cfg(feature = "enhanced-event-sourcing")]            let cutoff_date = Utc::now() - chrono::Duration::days(self.config.retention_days as i64);
            
            // Delete snapshots older than retention period
            for metadata in snapshots {
                if metadata.created_at < chrono::Utc::now() - chrono::Duration::days(7) {
                    self.delete_snapshot(vm_id, metadata.snapshot_version).await?;
                    deleted_count += 1;
                }
            }

            // Keep only the most recent snapshots if we have too many
            if snapshots.len() > self.config.max_snapshots_per_vm {
                let snapshots_to_keep = snapshots.len() - self.config.max_snapshots_per_vm;
                for i in 0..snapshots_to_keep {
                    if let Some(metadata) = snapshots.get(i) {
                        self.delete_snapshot(vm_id, metadata.snapshot_version).await?;
                        deleted_count += 1;
                    }
                }
            }
        }

        Ok(deleted_count)
    }

    async fn get_snapshot_count(&self, vm_id: &str) -> VmResult<u64> {
        let metadata_cache = self.metadata_cache.read().await;
        
        if let Some(snapshots) = metadata_cache.get(vm_id) {
            Ok(snapshots.len() as u64)
        } else {
            Ok(0)
        }
    }
}

/// Snapshot manager for coordinating snapshot operations
#[cfg(feature = "enhanced-event-sourcing")]
pub struct SnapshotManager {
    snapshot_store: Box<dyn SnapshotStore>,
    event_store: Arc<dyn EventStore>,
    config: SnapshotConfig,
}

#[cfg(feature = "enhanced-event-sourcing")]
impl SnapshotManager {
    /// Create a new snapshot manager
    pub fn new(
        snapshot_store: Box<dyn SnapshotStore>,
        event_store: Arc<dyn EventStore>,
        config: SnapshotConfig,
    ) -> Self {
        Self {
            snapshot_store,
            event_store,
            config,
        }
    }

    /// Create a snapshot from the current aggregate state
    pub async fn create_snapshot(
        &self,
        aggregate: &VirtualMachineAggregate,
    ) -> VmResult<SnapshotMetadata> {
        let vm_id = aggregate.vm_id();
        let current_version = aggregate.version();

        // Serialize aggregate state
        let aggregate_data = serialize(aggregate)
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Failed to serialize aggregate: {}", e),
            }))?;

        // Calculate checksum
        let checksum = self.calculate_checksum(&aggregate_data);

        // Create metadata
        let metadata = SnapshotMetadata {
            vm_id: vm_id.clone(),
            snapshot_version: current_version,
            created_at: Utc::now(),
            event_count: current_version,
            file_size: aggregate_data.len() as u64,
            compressed: self.config.enable_compression,
            checksum,
#[cfg(feature = "enhanced-event-sourcing")]
#[cfg(feature = "enhanced-event-sourcing")]
            metadata: serde_json::to_string(&aggregate.get_metadata())
                .unwrap_or_default(),
        };

        // Create snapshot data
        let snapshot = SnapshotData {
            aggregate_data,
            metadata: metadata.clone(),
        };

        // Store snapshot
        self.snapshot_store.store_snapshot(snapshot).await?;

        Ok(metadata)
    }

    /// Restore aggregate from snapshot
    pub async fn restore_from_snapshot(
        &self,
        vm_id: &str,
        version: Option<u64>,
    ) -> VmResult<VirtualMachineAggregate> {
        // Get snapshot (latest or specific version)
        let snapshot = if let Some(v) = version {
            self.snapshot_store.get_snapshot(vm_id, v).await?
                .ok_or_else(|| VmError::Core(CoreError::InvalidState {
                    message: format!("Snapshot version {} not found", v),
                    current: "N/A".to_string(),
                    expected: format!("Snapshot version {}", v),
                }))?
        } else {
            self.snapshot_store.get_latest_snapshot(vm_id).await?
                .ok_or_else(|| VmError::Core(CoreError::InvalidState {
                    message: format!("No snapshots found for VM {}", vm_id),
                    current: "N/A".to_string(),
                    expected: "At least one snapshot".to_string(),
                }))?
        };

        // Deserialize aggregate
        let aggregate: VirtualMachineAggregate = deserialize(&snapshot.aggregate_data)
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Failed to deserialize aggregate: {}", e),
            }))?;

        Ok(aggregate)
    }

    /// Replay events from snapshot version to current state
    pub async fn replay_from_snapshot(
        &self,
        vm_id: &str,
        target_version: Option<u64>,
    ) -> VmResult<VirtualMachineAggregate> {
        // Get latest snapshot
        let snapshot = self.snapshot_store.get_latest_snapshot(vm_id).await?
            .ok_or_else(|| VmError::Core(CoreError::InvalidState {
                message: format!("No snapshots found for VM {}", vm_id),
                current: "N/A".to_string(),
                expected: "At least one snapshot".to_string(),
            }))?;

        // Restore aggregate from snapshot
        let mut aggregate: VirtualMachineAggregate = deserialize(&snapshot.aggregate_data)
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Failed to deserialize aggregate: {}", e),
            }))?;

        // Get events since snapshot
        let from_sequence = snapshot.metadata.snapshot_version + 1;
        let to_sequence = target_version.unwrap_or(u64::MAX);
        
        let events = if to_sequence == u64::MAX {
            self.event_store.get_events(vm_id, from_sequence).await?
        } else {
            self.event_store.get_events_range(vm_id, from_sequence, to_sequence).await?
        };

        // Apply events to aggregate
        for event in events {
            aggregate.apply_event(event)?;
        }

        Ok(aggregate)
    }

    /// Check if snapshot should be created based on configuration
    pub async fn should_create_snapshot(&self, vm_id: &str, event_count: u64) -> VmResult<bool> {
        if !self.config.auto_snapshot {
            return Ok(false);
        }

        // Check if we've reached the snapshot interval
        if event_count % self.config.snapshot_interval == 0 {
            return Ok(true);
        }

        // Check if we have any snapshots at all
        let snapshot_count = self.snapshot_store.get_snapshot_count(vm_id).await?;
        if snapshot_count == 0 {
            return Ok(true);
        }

        Ok(false)
    }

    /// Get snapshot statistics
    pub async fn get_snapshot_stats(&self, vm_id: &str) -> VmResult<SnapshotStats> {
        let snapshots = self.snapshot_store.list_snapshots(vm_id).await?;
        let snapshot_count = snapshots.len() as u64;
        
        let total_size: u64 = snapshots.iter().map(|s| s.file_size).sum();
        let latest_version = snapshots.last().map(|s| s.snapshot_version).unwrap_or(0);
        
        Ok(SnapshotStats {
            snapshot_count,
            total_size_bytes: total_size,
            latest_version,
            oldest_snapshot: snapshots.first().map(|s| s.created_at),
            newest_snapshot: snapshots.last().map(|s| s.created_at),
        })
    }

    /// Calculate checksum for data
    fn calculate_checksum(&self, data: &[u8]) -> String {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }
}

/// Snapshot statistics
#[derive(Debug, Clone)]
#[cfg(feature = "enhanced-event-sourcing")]
pub struct SnapshotStats {
    /// Number of snapshots
    pub snapshot_count: u64,
    /// Total size of all snapshots in bytes
    pub total_size_bytes: u64,
    /// Version of the latest snapshot
    pub latest_version: u64,
    /// Creation time of the oldest snapshot
    pub oldest_snapshot: Option<DateTime<Utc>>,
    /// Creation time of the newest snapshot
    pub newest_snapshot: Option<DateTime<Utc>>,
}

/// Snapshot store statistics
#[cfg(feature = "enhanced-event-sourcing")]
#[derive(Debug, Default, Clone)]
pub struct SnapshotStoreStats {
    /// Total snapshots created
    pub total_snapshots_created: u64,
    /// Total snapshots restored
    pub total_snapshots_restored: u64,
    /// Total snapshots deleted
    pub total_snapshots_deleted: u64,
    /// Total bytes written
    pub total_bytes_written: u64,
    /// Total bytes read
    pub total_bytes_read: u64,
    /// Total compression time (microseconds)
    pub total_compression_time: u64,
    /// Total decompression time (microseconds)
    pub total_decompression_time: u64,
    /// Average snapshot size (bytes)
    pub avg_snapshot_size: u64,
    /// Average compression ratio
    pub avg_compression_ratio: f64,
}

/// Snapshot manager statistics
#[cfg(feature = "enhanced-event-sourcing")]
#[derive(Debug, Default, Clone)]
pub struct SnapshotManagerStats {
    /// Total snapshots created
    pub total_snapshots_created: u64,
    /// Total snapshots restored
    pub total_snapshots_restored: u64,
    /// Total snapshots deleted
    pub total_snapshots_deleted: u64,
    /// Total events processed
    pub total_events_processed: u64,
    /// Average snapshot creation time (microseconds)
    pub avg_snapshot_creation_time: u64,
    /// Average snapshot restoration time (microseconds)
    pub avg_snapshot_restoration_time: u64,
    /// Snapshot hit ratio
    pub snapshot_hit_ratio: f64,
}

/// Snapshot store builder for easier configuration
#[cfg(feature = "enhanced-event-sourcing")]
pub struct SnapshotStoreBuilder {
    config: SnapshotConfig,
}

#[cfg(feature = "enhanced-event-sourcing")]
impl SnapshotStoreBuilder {
    /// Create a new builder with default configuration
    pub fn new() -> Self {
        Self {
            config: SnapshotConfig::default(),
        }
    }

    /// Set snapshot directory
    pub fn snapshot_dir(mut self, dir: impl AsRef<Path>) -> Self {
        self.config.snapshot_dir = dir.as_ref().to_path_buf();
        self
    }

    /// Set snapshot interval
    pub fn snapshot_interval(mut self, interval: u64) -> Self {
        self.config.snapshot_interval = interval;
        self
    }

    /// Set maximum snapshots per VM
    pub fn max_snapshots_per_vm(mut self, max: usize) -> Self {
        self.config.max_snapshots_per_vm = max;
        self
    }

    /// Enable or disable compression
    pub fn enable_compression(mut self, enable: bool) -> Self {
        self.config.enable_compression = enable;
        self
    }

    /// Set retention period in days
    pub fn retention_days(mut self, days: u32) -> Self {
        self.config.retention_days = days;
        self
    }

    /// Enable or disable automatic snapshot creation
    pub fn auto_snapshot(mut self, enable: bool) -> Self {
        self.config.auto_snapshot = enable;
        self
    }

    /// Build file-based snapshot store
    pub async fn build(self) -> VmResult<FileSnapshotStore> {
        FileSnapshotStore::new(self.config).await
    }
}

#[cfg(feature = "enhanced-event-sourcing")]
impl Default for SnapshotStoreBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, feature = "enhanced-event-sourcing"))]
mod tests {
    use super::*;
    use crate::event_store::InMemoryEventStore;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_snapshot_store_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = SnapshotConfig {
            snapshot_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let store = FileSnapshotStore::new(config).await.unwrap();
        assert_eq!(store.config.snapshot_dir, temp_dir.path());
    }

    #[test]
    fn test_snapshot_store_builder() {
        let temp_dir = TempDir::new().unwrap();
        let builder = SnapshotStoreBuilder::new()
            .snapshot_dir(temp_dir.path())
            .snapshot_interval(500)
            .max_snapshots_per_vm(5)
            .enable_compression(true);

        assert_eq!(builder.config.snapshot_dir, temp_dir.path());
        assert_eq!(builder.config.snapshot_interval, 500);
        assert_eq!(builder.config.max_snapshots_per_vm, 5);
        assert!(builder.config.enable_compression);
    }

    #[tokio::test]
    async fn test_snapshot_creation_and_retrieval() {
        let temp_dir = TempDir::new().unwrap();
        let config = SnapshotConfig {
            snapshot_dir: temp_dir.path().to_path_buf(),
            enable_compression: false, // Disable for easier testing
            ..Default::default()
        };

        let store = FileSnapshotStore::new(config).await.unwrap();
        let vm_id = "test_vm";

        // Create test snapshot data
        let snapshot_data = SnapshotData {
            aggregate_data: b"Test aggregate data".to_vec(),
            metadata: SnapshotMetadata {
                vm_id: vm_id.to_string(),
                snapshot_version: 100,
                created_at: Utc::now(),
                event_count: 100,
                file_size: 20,
                compressed: false,
                checksum: "test_checksum".to_string(),
                metadata: "{}".to_string(),
            },
        };

        // Store snapshot
        store.store_snapshot(snapshot_data.clone()).await.unwrap();

        // Retrieve snapshot
        let retrieved = store.get_snapshot(vm_id, 100).await.unwrap();
        assert!(retrieved.is_some());
        
        let retrieved_data = retrieved.unwrap();
        assert_eq!(retrieved_data.aggregate_data, snapshot_data.aggregate_data);
        assert_eq!(retrieved_data.metadata.vm_id, vm_id);
        assert_eq!(retrieved_data.metadata.snapshot_version, 100);
    }
}
