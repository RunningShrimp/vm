//! File-based event store implementation
//!
//! This module provides a file-backed event store for persistent event storage,
//! suitable for development, testing, and single-node deployments.

use std::sync::Arc;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write, Seek, SeekFrom};
use async_trait::async_trait;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use bincode;

use crate::event_store::{EventStore, StoredEvent, VmResult};
use crate::error::{VmError, CoreError};

/// File-based event store configuration
#[cfg(feature = "enhanced-event-sourcing")]
#[derive(Debug, Clone)]
pub struct FileEventStoreConfig {
    /// Base directory for event storage
    pub base_dir: PathBuf,
    /// Enable event compression
    pub enable_compression: bool,
    /// Event file rotation size in bytes
    pub rotation_size_bytes: u64,
    /// Maximum number of rotated files to keep
    pub max_rotated_files: usize,
    /// Enable file sync for durability
    pub enable_sync: bool,
    /// Buffer size for file operations
    pub buffer_size: usize,
}

#[cfg(feature = "enhanced-event-sourcing")]
impl Default for FileEventStoreConfig {
    fn default() -> Self {
        Self {
            base_dir: PathBuf::from("./vm_events"),
            enable_compression: true,
            rotation_size_bytes: 100 * 1024 * 1024, // 100MB
            max_rotated_files: 10,
            enable_sync: true,
            buffer_size: 64 * 1024, // 64KB
        }
    }
}

/// Event file metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
struct EventFileMetadata {
    /// VM ID this file belongs to
    vm_id: String,
    /// File sequence number (for rotation)
    file_sequence: u32,
    /// First event sequence number in this file
    first_sequence: u64,
    /// Last event sequence number in this file
    last_sequence: u64,
    /// Number of events in this file
    event_count: u64,
    /// File creation timestamp
    created_at: DateTime<Utc>,
    /// File size in bytes
    file_size: u64,
    /// Whether this file is compressed
    compressed: bool,
}

/// Event file index entry
#[derive(Debug, Clone, Serialize, Deserialize)]
struct EventIndexEntry {
    /// Event sequence number
    sequence_number: u64,
    /// Byte offset in the file
    offset: u64,
    /// Event data size in bytes
    size: u64,
    /// Event type for quick filtering
    event_type: String,
    /// Event timestamp
    occurred_at: DateTime<Utc>,
}

/// Event file
#[cfg(feature = "enhanced-event-sourcing")]
#[derive(Debug)]
pub struct FileEventStoreFile {
    /// File path
    pub path: PathBuf,
    /// File handle
    pub file: std::fs::File,
    /// Current size
    pub size: u64,
    /// Event count
    pub event_count: u64,
    /// File number
    pub file_number: u32,
}

/// File event store statistics
#[cfg(feature = "enhanced-event-sourcing")]
#[derive(Debug, Default)]
pub struct FileEventStoreStats {
    /// Total events stored
    pub total_events_stored: u64,
    /// Total events retrieved
    pub total_events_retrieved: u64,
    /// Total files created
    pub total_files_created: u64,
    /// Total files rotated
    pub total_files_rotated: u64,
    /// Total bytes written
    pub total_bytes_written: u64,
    /// Total bytes read
    pub total_bytes_read: u64,
    /// Total compression time (microseconds)
    pub total_compression_time: u64,
    /// Total decompression time (microseconds)
    pub total_decompression_time: u64,
    /// Average compression ratio
    pub avg_compression_ratio: f64,
    /// File system errors
    pub fs_errors: u64,
}

/// File-based event store implementation
/// 
/// This provides persistent event storage using files as the backend.
/// Each VM gets its own set of files with rotation based on size.
#[cfg(feature = "enhanced-event-sourcing")]
pub struct FileEventStore {
    config: FileEventStoreConfig,
    /// Cache of open file handles
    open_files: Arc<RwLock<HashMap<String, BufWriter<File>>>>,
    /// Cache of event indexes
    event_indexes: Arc<RwLock<HashMap<String, Vec<EventIndexEntry>>>>,
    /// Cache of file metadata
    file_metadata: Arc<RwLock<HashMap<String, Vec<EventFileMetadata>>>>,
}

#[cfg(feature = "enhanced-event-sourcing")]
impl FileEventStore {
    /// Create a new file-based event store with the given configuration
    pub async fn new(config: FileEventStoreConfig) -> VmResult<Self> {
        // Create base directory if it doesn't exist
        std::fs::create_dir_all(&config.base_dir)
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Failed to create event store directory: {}", e),
            }))?;

        let store = Self {
            config: config.clone(),
            open_files: Arc::new(RwLock::new(HashMap::new())),
            event_indexes: Arc::new(RwLock::new(HashMap::new())),
            file_metadata: Arc::new(RwLock::new(HashMap::new())),
        };

        // Load existing metadata and indexes
        store.load_metadata().await?;

        Ok(store)
    }

    /// Load existing metadata and indexes from disk
    async fn load_metadata(&self) -> VmResult<()> {
        let entries = std::fs::read_dir(&self.config.base_dir)
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Failed to read event store directory: {}", e),
            }))?;

        for entry in entries {
            let entry = entry
                .map_err(|e| VmError::Core(CoreError::IoError {
                    message: format!("Failed to read directory entry: {}", e),
                }))?;

            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("meta") {
                self.load_metadata_file(&path).await?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("idx") {
                self.load_index_file(&path).await?;
            }
        }

        Ok(())
    }

    /// Load metadata from a .meta file
    async fn load_metadata_file(&self, path: &Path) -> VmResult<()> {
        let mut file = File::open(path)
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Failed to open metadata file: {}", e),
            }))?;

        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Failed to read metadata file: {}", e),
            }))?;

        let metadata: EventFileMetadata = serde_json::from_str(&contents)
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Failed to parse metadata file: {}", e),
            }))?;

        let mut metadata_cache = self.file_metadata.write().await;
        metadata_cache.entry(metadata.vm_id.clone())
            .or_insert_with(Vec::new)
            .push(metadata);

        Ok(())
    }

    /// Load index from a .idx file
    async fn load_index_file(&self, path: &Path) -> VmResult<()> {
        let file = File::open(path)
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Failed to open index file: {}", e),
            }))?;

        let reader = BufReader::new(file);
        let index: Vec<EventIndexEntry> = bincode::deserialize_from(reader)
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Failed to deserialize index file: {}", e),
            }))?;

        // Extract VM ID from filename
        let vm_id = path.file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| VmError::Core(CoreError::IoError {
                message: "Invalid index file name".to_string(),
            }))?
            .to_string();

        let mut index_cache = self.event_indexes.write().await;
        index_cache.insert(vm_id, index);

        Ok(())
    }

    /// Get file path for VM events
    fn get_event_file_path(&self, vm_id: &str, file_sequence: u32) -> PathBuf {
        self.config.base_dir.join(format!("{}_events_{:03}.dat", vm_id, file_sequence))
    }

    /// Get metadata file path
    fn get_metadata_file_path(&self, vm_id: &str, file_sequence: u32) -> PathBuf {
        self.config.base_dir.join(format!("{}_events_{:03}.meta", vm_id, file_sequence))
    }

    /// Get index file path
    fn get_index_file_path(&self, vm_id: &str, file_sequence: u32) -> PathBuf {
        self.config.base_dir.join(format!("{}_events_{:03}.idx", vm_id, file_sequence))
    }

    /// Get or create a file writer for VM events
    async fn get_event_writer(&self, vm_id: &str) -> VmResult<(BufWriter<File>, u32)> {
        let mut open_files = self.open_files.write().await;
        
        if let Some(writer) = open_files.get(vm_id) {
            // Check if we need to rotate the file
            let file_sequence = self.get_current_file_sequence(vm_id).await?;
            let file_path = self.get_event_file_path(vm_id, file_sequence);
            
            if let Ok(metadata) = std::fs::metadata(&file_path) {
                if metadata.len() >= self.config.rotation_size_bytes {
                    // Need to rotate
                    drop(open_files);
                    return self.rotate_event_file(vm_id).await;
                }
            }
            
            // Return existing writer
            return Ok((writer.clone(), file_sequence));
        }

        // Create new file
        let file_sequence = self.get_current_file_sequence(vm_id).await?;
        let file_path = self.get_event_file_path(vm_id, file_sequence);
        
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Failed to create event file: {}", e),
            }))?;

        let writer = BufWriter::with_capacity(self.config.buffer_size, file);
        open_files.insert(vm_id.to_string(), writer.clone());

        Ok((writer, file_sequence))
    }

    /// Get current file sequence for a VM
    async fn get_current_file_sequence(&self, vm_id: &str) -> VmResult<u32> {
        let metadata_cache = self.file_metadata.read().await;
        if let Some(files) = metadata_cache.get(vm_id) {
            if let Some(latest) = files.last() {
                return Ok(latest.file_sequence);
            }
        }
        Ok(0)
    }

    /// Rotate event file when it gets too large
    async fn rotate_event_file(&self, vm_id: &str) -> VmResult<(BufWriter<File>, u32)> {
        let mut open_files = self.open_files.write().await;
        
        // Close current file
        if let Some(mut writer) = open_files.remove(vm_id) {
            writer.flush()
                .map_err(|e| VmError::Core(CoreError::IoError {
                    message: format!("Failed to flush event file: {}", e),
                }))?;
        }

        // Get next file sequence
        let current_sequence = self.get_current_file_sequence(vm_id).await?;
        let new_sequence = current_sequence + 1;

        // Clean up old files if needed
        self.cleanup_old_files(vm_id).await?;

        // Create new file
        let file_path = self.get_event_file_path(vm_id, new_sequence);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Failed to create new event file: {}", e),
            }))?;

        let writer = BufWriter::with_capacity(self.config.buffer_size, file);
        open_files.insert(vm_id.to_string(), writer.clone());

        Ok((writer, new_sequence))
    }

    /// Clean up old rotated files
    async fn cleanup_old_files(&self, vm_id: &str) -> VmResult<()> {
        let metadata_cache = self.file_metadata.read().await;
        if let Some(files) = metadata_cache.get(vm_id) {
            if files.len() > self.config.max_rotated_files {
                let files_to_remove = &files[..files.len() - self.config.max_rotated_files];
                
                for metadata in files_to_remove {
                    // Remove event file
                    let event_path = self.get_event_file_path(&metadata.vm_id, metadata.file_sequence);
                    if event_path.exists() {
                        std::fs::remove_file(&event_path)
                            .map_err(|e| VmError::Core(CoreError::IoError {
                                message: format!("Failed to remove old event file: {}", e),
                            }))?;
                    }

                    // Remove metadata file
                    let meta_path = self.get_metadata_file_path(&metadata.vm_id, metadata.file_sequence);
                    if meta_path.exists() {
                        std::fs::remove_file(&meta_path)
                            .map_err(|e| VmError::Core(CoreError::IoError {
                                message: format!("Failed to remove old metadata file: {}", e),
                            }))?;
                    }

                    // Remove index file
                    let idx_path = self.get_index_file_path(&metadata.vm_id, metadata.file_sequence);
                    if idx_path.exists() {
                        std::fs::remove_file(&idx_path)
                            .map_err(|e| VmError::Core(CoreError::IoError {
                                message: format!("Failed to remove old index file: {}", e),
                            }))?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Compress event data if compression is enabled
    fn compress_event_data(&self, data: &[u8]) -> VmResult<Vec<u8>> {
        if self.config.enable_compression {
            let compressed = miniz_oxide::deflate::compress_to_vec_zlib(data, 6)
                .map_err(|e| VmError::Core(CoreError::IoError {
                    message: format!("Failed to compress event data: {:?}", e),
                }))?;
            Ok(compressed)
        } else {
            Ok(data.to_vec())
        }
    }

    /// Decompress event data if compression was used
    fn decompress_event_data(&self, data: &[u8]) -> VmResult<Vec<u8>> {
        if self.config.enable_compression {
            let decompressed = miniz_oxide::inflate::decompress_to_vec_zlib(data)
                .map_err(|e| VmError::Core(CoreError::IoError {
                    message: format!("Failed to decompress event data: {:?}", e),
                }))?;
            Ok(decompressed)
        } else {
            Ok(data.to_vec())
        }
    }

    /// Write event to file and update index
    async fn write_event_to_file(
        &self,
        vm_id: &str,
        event: &StoredEvent,
        writer: &mut BufWriter<File>,
        file_sequence: u32,
    ) -> VmResult<()> {
        // Get current position for index
        let position = writer.stream_position()
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Failed to get file position: {}", e),
            }))?;

        // Serialize and compress event
        let event_data = bincode::serialize(&event)
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Failed to serialize event: {}", e),
            }))?;

        let compressed_data = self.compress_event_data(&event_data)?;

        // Write event data
        writer.write_all(&compressed_data)
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Failed to write event data: {}", e),
            }))?;

        // Update index
        let mut index_cache = self.event_indexes.write().await;
        let vm_index = index_cache.entry(vm_id.to_string()).or_insert_with(Vec::new);
        vm_index.push(EventIndexEntry {
            sequence_number: event.sequence_number,
            offset: position,
            size: compressed_data.len() as u64,
            event_type: event.event_type.clone(),
            occurred_at: event.occurred_at,
        });

        // Update metadata
        let mut metadata_cache = self.file_metadata.write().await;
        let vm_metadata = metadata_cache.entry(vm_id.to_string()).or_insert_with(Vec::new);
        
        if let Some(current_metadata) = vm_metadata.last_mut() {
            if current_metadata.file_sequence == file_sequence {
                current_metadata.last_sequence = event.sequence_number;
                current_metadata.event_count += 1;
                current_metadata.file_size += compressed_data.len() as u64;
            } else {
                // Create new metadata entry
                vm_metadata.push(EventFileMetadata {
                    vm_id: vm_id.to_string(),
                    file_sequence,
                    first_sequence: event.sequence_number,
                    last_sequence: event.sequence_number,
                    event_count: 1,
                    created_at: Utc::now(),
                    file_size: compressed_data.len() as u64,
                    compressed: self.config.enable_compression,
                });
            }
        } else {
            // First metadata entry
            vm_metadata.push(EventFileMetadata {
                vm_id: vm_id.to_string(),
                file_sequence,
                first_sequence: event.sequence_number,
                last_sequence: event.sequence_number,
                event_count: 1,
                created_at: Utc::now(),
                file_size: compressed_data.len() as u64,
                compressed: self.config.enable_compression,
            });
        }

        Ok(())
    }

    /// Save metadata and index to disk
    async fn persist_metadata(&self, vm_id: &str) -> VmResult<()> {
        let metadata_cache = self.file_metadata.read().await;
        let index_cache = self.event_indexes.read().await;

        // Save metadata files
        if let Some(metadata_list) = metadata_cache.get(vm_id) {
            for metadata in metadata_list {
                let meta_path = self.get_metadata_file_path(vm_id, metadata.file_sequence);
                let meta_json = serde_json::to_string_pretty(metadata)
                    .map_err(|e| VmError::Core(CoreError::IoError {
                        message: format!("Failed to serialize metadata: {}", e),
                    }))?;

                std::fs::write(&meta_path, meta_json)
                    .map_err(|e| VmError::Core(CoreError::IoError {
                        message: format!("Failed to write metadata file: {}", e),
                    }))?;
            }
        }

        // Save index files
        if let Some(index) = index_cache.get(vm_id) {
            let file_sequence = self.get_current_file_sequence(vm_id).await?;
            let idx_path = self.get_index_file_path(vm_id, file_sequence);
            
            let file = File::create(&idx_path)
                .map_err(|e| VmError::Core(CoreError::IoError {
                    message: format!("Failed to create index file: {}", e),
                }))?;

            let writer = BufWriter::new(file);
            bincode::serialize_into(writer, index)
                .map_err(|e| VmError::Core(CoreError::IoError {
                    message: format!("Failed to serialize index: {}", e),
                }))?;
        }

        Ok(())
    }

    /// Read events from file using index
    async fn read_events_from_file(
        &self,
        vm_id: &str,
        file_path: &Path,
        index: &[EventIndexEntry],
        from_sequence: u64,
        to_sequence: Option<u64>,
    ) -> VmResult<Vec<StoredEvent>> {
        let mut file = File::open(file_path)
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Failed to open event file: {}", e),
            }))?;

        let mut events = Vec::new();

        for index_entry in index {
            if index_entry.sequence_number < from_sequence {
                continue;
            }

            if let Some(to_seq) = to_sequence {
                if index_entry.sequence_number > to_seq {
                    break;
                }
            }

            // Seek to event position
            file.seek(SeekFrom::Start(index_entry.offset))
                .map_err(|e| VmError::Core(CoreError::IoError {
                    message: format!("Failed to seek to event position: {}", e),
                }))?;

            // Read event data
            let mut event_data = vec![0u8; index_entry.size as usize];
            file.read_exact(&mut event_data)
                .map_err(|e| VmError::Core(CoreError::IoError {
                    message: format!("Failed to read event data: {}", e),
                }))?;

            // Decompress and deserialize
            let decompressed_data = self.decompress_event_data(&event_data)?;
            let event: StoredEvent = bincode::deserialize(&decompressed_data)
                .map_err(|e| VmError::Core(CoreError::IoError {
                    message: format!("Failed to deserialize event: {}", e),
                }))?;

            events.push(event);
        }

        Ok(events)
    }
}

#[async_trait]
impl EventStore for FileEventStore {
    async fn store_events(&self, vm_id: &str, events: Vec<StoredEvent>) -> VmResult<()> {
        if events.is_empty() {
            return Ok(());
        }

        let (mut writer, file_sequence) = self.get_event_writer(vm_id).await?;

        for event in &events {
            self.write_event_to_file(vm_id, event, &mut writer, file_sequence).await?;
        }

        // Flush and sync if enabled
        writer.flush()
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Failed to flush event writer: {}", e),
            }))?;

        if self.config.enable_sync {
            writer.get_ref().sync_all()
                .map_err(|e| VmError::Core(CoreError::IoError {
                    message: format!("Failed to sync event file: {}", e),
                }))?;
        }

        // Persist metadata and index
        self.persist_metadata(vm_id).await?;

        Ok(())
    }

    async fn get_events(&self, vm_id: &str, from_sequence: u64) -> VmResult<Vec<StoredEvent>> {
        let index_cache = self.event_indexes.read().await;
        let metadata_cache = self.file_metadata.read().await;

        let mut all_events = Vec::new();

        if let Some(index) = index_cache.get(vm_id) {
            if let Some(metadata_list) = metadata_cache.get(vm_id) {
                for metadata in metadata_list {
                    if metadata.last_sequence < from_sequence {
                        continue;
                    }

                    let file_path = self.get_event_file_path(vm_id, metadata.file_sequence);
                    let events = self.read_events_from_file(
                        vm_id,
                        &file_path,
                        index,
                        from_sequence,
                        None,
                    ).await?;

                    all_events.extend(events);
                }
            }
        }

        // Sort by sequence number
        all_events.sort_by_key(|e| e.sequence_number);

        Ok(all_events)
    }

    async fn get_events_range(&self, vm_id: &str, from_sequence: u64, to_sequence: u64) -> VmResult<Vec<StoredEvent>> {
        let index_cache = self.event_indexes.read().await;
        let metadata_cache = self.file_metadata.read().await;

        let mut all_events = Vec::new();

        if let Some(index) = index_cache.get(vm_id) {
            if let Some(metadata_list) = metadata_cache.get(vm_id) {
                for metadata in metadata_list {
                    if metadata.last_sequence < from_sequence || metadata.first_sequence > to_sequence {
                        continue;
                    }

                    let file_path = self.get_event_file_path(vm_id, metadata.file_sequence);
                    let events = self.read_events_from_file(
                        vm_id,
                        &file_path,
                        index,
                        from_sequence,
                        Some(to_sequence),
                    ).await?;

                    all_events.extend(events);
                }
            }
        }

        // Sort by sequence number
        all_events.sort_by_key(|e| e.sequence_number);

        Ok(all_events)
    }

    async fn get_last_sequence_number(&self, vm_id: &str) -> VmResult<Option<u64>> {
        let metadata_cache = self.file_metadata.read().await;
        
        if let Some(metadata_list) = metadata_cache.get(vm_id) {
            if let Some(latest) = metadata_list.last() {
                return Ok(Some(latest.last_sequence));
            }
        }

        Ok(None)
    }

    async fn delete_events(&self, vm_id: &str, up_to_sequence: u64) -> VmResult<u64> {
        let mut deleted_count = 0u64;
        let metadata_cache = self.file_metadata.read().await;

        if let Some(metadata_list) = metadata_cache.get(vm_id) {
            for metadata in metadata_list {
                if metadata.last_sequence <= up_to_sequence {
                    // Delete entire file
                    let event_path = self.get_event_file_path(vm_id, metadata.file_sequence);
                    if event_path.exists() {
                        std::fs::remove_file(&event_path)
                            .map_err(|e| VmError::Core(CoreError::IoError {
                                message: format!("Failed to delete event file: {}", e),
                            }))?;
                        deleted_count += metadata.event_count;
                    }

                    // Delete metadata file
                    let meta_path = self.get_metadata_file_path(vm_id, metadata.file_sequence);
                    if meta_path.exists() {
                        std::fs::remove_file(&meta_path)
                            .map_err(|e| VmError::Core(CoreError::IoError {
                                message: format!("Failed to delete metadata file: {}", e),
                            }))?;
                    }

                    // Delete index file
                    let idx_path = self.get_index_file_path(vm_id, metadata.file_sequence);
                    if idx_path.exists() {
                        std::fs::remove_file(&idx_path)
                            .map_err(|e| VmError::Core(CoreError::IoError {
                                message: format!("Failed to delete index file: {}", e),
                            }))?;
                    }
                } else if metadata.first_sequence <= up_to_sequence {
                    // Partial deletion - more complex, would need to rewrite file
                    // For simplicity, we'll just count events to delete
                    deleted_count += up_to_sequence - metadata.first_sequence + 1;
                }
            }
        }

        // Update caches
        let mut index_cache = self.event_indexes.write().await;
        if let Some(index) = index_cache.get_mut(vm_id) {
            index.retain(|entry| entry.sequence_number > up_to_sequence);
        }

        Ok(deleted_count)
    }

    async fn list_vms(&self) -> VmResult<Vec<String>> {
        let metadata_cache = self.file_metadata.read().await;
        let vm_ids: Vec<String> = metadata_cache.keys().cloned().collect();
        Ok(vm_ids)
    }

    async fn get_event_count(&self, vm_id: &str) -> VmResult<u64> {
        let metadata_cache = self.file_metadata.read().await;
        
        if let Some(metadata_list) = metadata_cache.get(vm_id) {
            let total_count: u64 = metadata_list.iter().map(|m| m.event_count).sum();
            Ok(total_count)
        } else {
            Ok(0)
        }
    }
}

/// File event store builder for easier configuration
#[cfg(feature = "enhanced-event-sourcing")]
pub struct FileEventStoreBuilder {
    config: FileEventStoreConfig,
}

#[cfg(feature = "enhanced-event-sourcing")]
impl FileEventStoreBuilder {
    /// Create a new builder with default configuration
    pub fn new() -> Self {
        Self {
            config: FileEventStoreConfig::default(),
        }
    }

    /// Set base directory for event storage
    pub fn base_dir(mut self, base_dir: impl AsRef<Path>) -> Self {
        self.config.base_dir = base_dir.as_ref().to_path_buf();
        self
    }

    /// Enable or disable compression
    pub fn enable_compression(mut self, enable: bool) -> Self {
        self.config.enable_compression = enable;
        self
    }

    /// Set rotation size in bytes
    pub fn rotation_size(mut self, size_bytes: u64) -> Self {
        self.config.rotation_size_bytes = size_bytes;
        self
    }

    /// Set maximum number of rotated files
    pub fn max_rotated_files(mut self, max_files: usize) -> Self {
        self.config.max_rotated_files = max_files;
        self
    }

    /// Enable or disable file sync
    pub fn enable_sync(mut self, enable: bool) -> Self {
        self.config.enable_sync = enable;
        self
    }

    /// Set buffer size for file operations
    pub fn buffer_size(mut self, size: usize) -> Self {
        self.config.buffer_size = size;
        self
    }

    /// Build file event store
    pub async fn build(self) -> VmResult<FileEventStore> {
        FileEventStore::new(self.config).await
    }
}

impl Default for FileEventStoreBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_store::StoredEvent;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_file_event_store_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = FileEventStoreConfig {
            base_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let store = FileEventStore::new(config).await.unwrap();
        assert_eq!(store.config.base_dir, temp_dir.path());
    }

    #[test]
    fn test_file_event_store_builder() {
        let temp_dir = TempDir::new().unwrap();
        let builder = FileEventStoreBuilder::new()
            .base_dir(temp_dir.path())
            .enable_compression(true)
            .rotation_size(50 * 1024 * 1024)
            .max_rotated_files(5);

        assert_eq!(builder.config.base_dir, temp_dir.path());
        assert!(builder.config.enable_compression);
        assert_eq!(builder.config.rotation_size_bytes, 50 * 1024 * 1024);
        assert_eq!(builder.config.max_rotated_files, 5);
    }

    #[test]
    fn test_compression_decompression() {
        let config = FileEventStoreConfig::default();
        let store = FileEventStore {
            config,
            open_files: Arc::new(RwLock::new(HashMap::new())),
            event_indexes: Arc::new(RwLock::new(HashMap::new())),
            file_metadata: Arc::new(RwLock::new(HashMap::new())),
        };

        let original_data = b"Hello, file-based event store!";
        let compressed = store.compress_event_data(original_data).unwrap();
        let decompressed = store.decompress_event_data(&compressed).unwrap();

        assert_eq!(original_data, decompressed.as_slice());
    }

    #[tokio::test]
    async fn test_store_and_retrieve_events() {
        let temp_dir = TempDir::new().unwrap();
        let config = FileEventStoreConfig {
            base_dir: temp_dir.path().to_path_buf(),
            enable_compression: false, // Disable for easier testing
            ..Default::default()
        };

        let store = FileEventStore::new(config).await.unwrap();
        let vm_id = "test_vm";

        // Create test events
        let events = vec![
            StoredEvent {
                sequence_number: 1,
                event_type: "TestEvent".to_string(),
                event_version: 1,
                event_data: b"Event 1 data".to_vec(),
                metadata: "{}".to_string(),
                occurred_at: Utc::now(),
            },
            StoredEvent {
                sequence_number: 2,
                event_type: "TestEvent".to_string(),
                event_version: 1,
                event_data: b"Event 2 data".to_vec(),
                metadata: "{}".to_string(),
                occurred_at: Utc::now(),
            },
        ];

        // Store events
        store.store_events(vm_id, events.clone()).await.unwrap();

        // Retrieve events
        let retrieved = store.get_events(vm_id, 1).await.unwrap();
        assert_eq!(retrieved.len(), 2);
        assert_eq!(retrieved[0].sequence_number, 1);
        assert_eq!(retrieved[1].sequence_number, 2);
    }
}