//! PostgreSQL event store implementation
//!
//! This is the main module that provides the PostgreSQL-backed event store
//! implementation for the VM system, combining all sub-modules into a
//! cohesive interface that implements the EventStore trait.

use std::sync::Arc;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use tracing::{info, warn, error, debug};

use crate::jit::event_store::{EventStore, StoredEvent, VmResult};
use crate::jit::domain_events::DomainEventEnum;
use crate::jit::error::{VmError, CoreError};

// Import types from sibling modules
use super::postgres_event_store_config::*;
use super::postgres_event_store_types::*;
use super::postgres_event_store_connection::*;
use super::postgres_event_store_migrations::*;
use super::postgres_event_store_queries::*;
use super::postgres_event_store_batch::*;
use super::postgres_event_store_compression::*;

/// PostgreSQL event store implementation
pub struct PostgresEventStore {
    /// Configuration
    config: PostgresEventStoreConfig,
    /// Connection manager
    connection_manager: Arc<ConnectionManager>,
    /// Migration manager
    migration_manager: Arc<MigrationManager>,
    /// Query manager
    query_manager: Arc<QueryManager>,
    /// Batch manager
    batch_manager: Arc<BatchManager>,
    /// Compression manager
    compression_manager: Arc<std::sync::Mutex<CompressionManager>>,
    /// Event store statistics
    stats: Arc<StoreStatistics>,
}

/// Store statistics
#[derive(Debug, Clone, Default)]
pub struct StoreStatistics {
    /// Total events stored
    pub total_events: u64,
    /// Events processed
    pub events_processed: u64,
    /// Errors encountered
    pub errors_encountered: u64,
    /// Average processing time
    pub avg_processing_time_ms: f64,
    /// Last maintenance run
    pub last_maintenance: Option<DateTime<Utc>>,
}

impl PostgresEventStore {
    /// Create a new PostgreSQL event store
    pub async fn new(config: PostgresEventStoreConfig) -> Result<Self, StoreError> {
        info!("Creating PostgreSQL event store with config: {:?}", config);

        // Validate configuration
        config.validate()?;

        // Create connection manager
        let connection_manager = Arc::new(ConnectionManager::new(config.clone()).await?);
        connection_manager.start_health_checks().await;

        // Create migration manager
        let migration_manager = Arc::new(MigrationManager::new(config.clone()).await?);

        // Create query manager
        let query_manager = Arc::new(QueryManager::new(config.clone()));

        // Create compression manager
        let compression_manager = Arc::new(std::sync::Mutex::new(CompressionManager::new(
            CompressionConfig::default()
        )));

        // Create batch manager
        let batch_manager = Arc::new(BatchManager::new(config.clone(), query_manager.clone()));

        // Run migrations
        info!("Running database migrations...");
        match migration_manager.run_migrations().await {
            Ok(_) => info!("Migrations completed successfully"),
            Err(e) => {
                error!("Migration failed: {}", e);
                return Err(StoreError::MigrationFailed(e.to_string()));
            }
        }

        Ok(Self {
            config,
            connection_manager,
            migration_manager,
            query_manager,
            batch_manager,
            compression_manager,
            stats: Arc::new(StoreStatistics::default()),
        })
    }

    /// Get event store configuration
    pub fn get_config(&self) -> &PostgresEventStoreConfig {
        &self.config
    }

    /// Get event store statistics
    pub async fn get_statistics(&self) -> StoreStatistics {
        self.stats.clone()
    }

    /// Get detailed statistics from all managers
    pub async fn get_detailed_statistics(&self) -> DetailedStatistics {
        let pool = self.connection_manager.get_pool().await.ok();
        let event_store_stats = if let Some(ref p) = pool {
            self.query_manager.get_event_store_stats(p).await.ok()
        } else {
            None
        };

        DetailedStatistics {
            store_stats: self.stats.clone(),
            connection_stats: self.connection_manager.get_stats().await,
            batch_stats: self.batch_manager.get_batch_stats().await,
            compression_stats: self.compression_manager.lock().unwrap().get_stats(),
            event_store_stats,
            migration_info: self.migration_manager.get_migration_info().await,
            query_stats: self.query_manager.get_query_stats().await,
        }
    }

    /// Run maintenance operations
    pub async fn run_maintenance(&self) -> Result<MaintenanceResult, StoreError> {
        info!("Running maintenance operations...");

        let start_time = std::time::Instant::now();
        let mut operations = Vec::new();

        // Analyze table statistics
        if let Err(e) = self.query_manager.analyze_table(&self.connection_manager.get_pool().await?).await {
            warn!("Failed to analyze table: {}", e);
            operations.push(MaintenanceOperation::Failed {
                operation: "analyze_table".to_string(),
                error: e.to_string(),
            });
        } else {
            operations.push(MaintenanceOperation::Completed {
                operation: "analyze_table".to_string(),
                duration_ms: start_time.elapsed().as_millis() as u64,
            });
        }

        // Reset prepared statements cache
        self.query_manager.clear_prepared_statements().await;

        // Update statistics
        {
            let mut stats = Arc::make_mut(&mut self.stats);
            stats.last_maintenance = Some(Utc::now());
        }

        let duration = start_time.elapsed();
        info!("Maintenance completed in {:?}", duration);

        Ok(MaintenanceResult {
            operations,
            duration_ms: duration.as_millis() as u64,
        })
    }

    /// Close the event store gracefully
    pub async fn close(&self) -> Result<(), StoreError> {
        info!("Closing PostgreSQL event store...");

        // Stop health checks
        self.connection_manager.stop_health_checks().await;

        // Close connection pool
        self.connection_manager.close().await?;

        info!("PostgreSQL event store closed successfully");
        Ok(())
    }
}

#[async_trait]
impl EventStore for PostgresEventStore {
    async fn append(
        &self,
        vm_id: &str,
        sequence_number: Option<u64>,
        event: DomainEventEnum,
    ) -> VmResult<u64> {
        let start_time = std::time::Instant::now();

        // Serialize event
        let event_data = bincode::serialize(&event)
            .map_err(|e| VmError::Core(CoreError::Serialization {
                message: format!("Failed to serialize event: {}", e),
            }))?;

        // Compress event data
        let mut compression_manager = self.compression_manager.lock().unwrap();
        let compressed_event = compression_manager.compress_event(&event_data, None)
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Compression failed: {}", e),
            }))?;

        // Insert event
        let pool = self.connection_manager.get_pool().await
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Database connection failed: {}", e),
            }))?;

        let sequence = match sequence_number {
            Some(seq) => {
                // Validate sequence number
                if seq <= self.get_last_sequence_number(vm_id).await.unwrap_or(0) {
                    return Err(VmError::Core(CoreError::InvalidState {
                        message: format!("Sequence number {} is not greater than current", seq),
                        current: seq.to_string(),
                        expected: "greater than current".to_string(),
                    }));
                }
                seq
            }
            None => self.get_last_sequence_number(vm_id).await.unwrap_or(0) + 1,
        };

        match self.query_manager.insert_event(
            &pool,
            vm_id,
            sequence,
            &event.event_type_name(),
            event.version(),
            &compressed_event.compressed_data,
            None,
            compressed_event.original_size as u64,
            compressed_event.compressed_size.map(|s| s as u64),
            Some(&compressed_event.checksum),
        ).await {
            Ok(id) => {
                // Update statistics
                {
                    let mut stats = Arc::make_mut(&mut self.stats);
                    stats.total_events += 1;
                    stats.events_processed += 1;
                    stats.avg_processing_time_ms =
                        (stats.avg_processing_time_ms * (stats.events_processed - 1) as f64 +
                         start_time.elapsed().as_millis() as f64) / stats.events_processed as f64;
                }

                debug!("Event appended successfully: VM={}, sequence={}, ID={}", vm_id, sequence, id);
                Ok(sequence)
            }
            Err(e) => {
                let mut stats = Arc::make_mut(&mut self.stats);
                stats.errors_encountered += 1;

                error!("Failed to append event: {}", e);
                Err(VmError::Core(CoreError::IoError {
                    message: format!("Failed to append event: {}", e),
                }))
            }
        }
    }

    async fn load_events(
        &self,
        vm_id: &str,
        from_sequence: Option<u64>,
        to_sequence: Option<u64>,
    ) -> VmResult<Vec<StoredEvent>> {
        let pool = self.connection_manager.get_pool().await
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Database connection failed: {}", e),
            }))?;

        let params = EventQueryParams::new(vm_id.to_string())
            .with_sequence_range(from_sequence, to_sequence)
            .with_sort_order(SortOrder::Asc);

        match self.query_manager.load_events(&pool, &params).await {
            Ok(rows) => {
                let mut events = Vec::new();
                let mut compression_manager = self.compression_manager.lock().unwrap();

                for row in rows {
                    let event_data: Vec<u8> = row.get("event_data");
                    let compressed_event = CompressedEvent {
                        method: CompressionMethod::None, // Assuming stored as-is
                        compressed_data: event_data,
                        original_size: row.get::<i64, _>("event_size") as usize,
                        compressed_size: row.get::<i64, _>("compressed_size").unwrap_or_default() as usize,
                        checksum: row.get::<Option<String>, _>("checksum").unwrap_or_default().unwrap_or_default(),
                    };

                    let event_data = compression_manager.decompress_event(&compressed_event)
                        .map_err(|e| VmError::Core(CoreError::Decompression {
                            message: format!("Failed to decompress event: {}", e),
                        }))?;

                    let event: DomainEventEnum = bincode::deserialize(&event_data)
                        .map_err(|e| VmError::Core(CoreError::Deserialization {
                            message: format!("Failed to deserialize event: {}", e),
                        }))?;

                    events.push(StoredEvent {
                        sequence_number: row.get::<i64, _>("sequence_number") as u64,
                        vm_id: row.get("vm_id"),
                        event,
                        stored_at: row.get::<DateTime<Utc>, _>("created_at").into(),
                    });
                }

                Ok(events)
            }
            Err(e) => {
                error!("Failed to load events: {}", e);
                Err(VmError::Core(CoreError::IoError {
                    message: format!("Failed to load events: {}", e),
                }))
            }
        }
    }

    async fn get_last_sequence_number(&self, vm_id: &str) -> VmResult<u64> {
        let pool = self.connection_manager.get_pool().await
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Database connection failed: {}", e),
            }))?;

        match self.query_manager.get_last_sequence_number(&pool, vm_id).await {
            Ok(Some(seq)) => Ok(seq),
            Ok(None) => Ok(0),
            Err(e) => {
                error!("Failed to get last sequence number: {}", e);
                Err(VmError::Core(CoreError::IoError {
                    message: format!("Failed to get last sequence number: {}", e),
                }))
            }
        }
    }

    async fn get_event_count(&self, vm_id: &str) -> VmResult<usize> {
        let pool = self.connection_manager.get_pool().await
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Database connection failed: {}", e),
            }))?;

        match self.query_manager.get_event_count(&pool, vm_id).await {
            Ok(count) => Ok(count as usize),
            Err(e) => {
                error!("Failed to get event count: {}", e);
                Err(VmError::Core(CoreError::IoError {
                    message: format!("Failed to get event count: {}", e),
                }))
            }
        }
    }

    async fn list_vm_ids(&self) -> VmResult<Vec<String>> {
        let pool = self.connection_manager.get_pool().await
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Database connection failed: {}", e),
            }))?;

        match self.query_manager.list_vm_ids(&pool, None).await {
            Ok(vm_ids) => Ok(vm_ids),
            Err(e) => {
                error!("Failed to list VM IDs: {}", e);
                Err(VmError::Core(CoreError::IoError {
                    message: format!("Failed to list VM IDs: {}", e),
                }))
            }
        }
    }

    async fn delete_events(&self, vm_id: &str) -> VmResult<()> {
        let pool = self.connection_manager.get_pool().await
            .map_err(|e| VmError::Core(CoreError::IoError {
                message: format!("Database connection failed: {}", e),
            }))?;

        match self.query_manager.delete_events(&pool, vm_id).await {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("Failed to delete events: {}", e);
                Err(VmError::Core(CoreError::IoError {
                    message: format!("Failed to delete events: {}", e),
                }))
            }
        }
    }
}

/// Detailed statistics combining all manager statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedStatistics {
    /// Store-level statistics
    pub store_stats: StoreStatistics,
    /// Connection manager statistics
    pub connection_stats: ConnectionStats,
    /// Batch manager statistics
    pub batch_stats: BatchStats,
    /// Compression manager statistics
    pub compression_stats: CompressionStats,
    /// Event store statistics
    pub event_store_stats: Option<serde_json::Value>,
    /// Migration information
    pub migration_info: MigrationInfo,
    /// Query manager statistics
    pub query_stats: QueryStats,
}

/// Maintenance operation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MaintenanceOperation {
    /// Operation completed successfully
    Completed {
        operation: String,
        duration_ms: u64,
    },
    /// Operation failed
    Failed {
        operation: String,
        error: String,
    },
}

/// Maintenance result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceResult {
    /// Operations performed
    pub operations: Vec<MaintenanceOperation>,
    /// Total duration in milliseconds
    pub duration_ms: u64,
}

/// Store error types
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("Connection error: {0}")]
    ConnectionError(#[from] ConnectionError),
    #[error("Migration error: {0}")]
    MigrationFailed(String),
    #[error("Query error: {0}")]
    QueryError(String),
    #[error("Batch operation error: {0}")]
    BatchError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_store_creation() {
        let config = PostgresEventStoreConfig::default();
        let store = PostgresEventStore::new(config).await;

        match store {
            Ok(_) => {
                // Successfully created
            }
            Err(StoreError::ConnectionError(_)) => {
                // Expected for testing without database
            }
            Err(e) => panic!("Unexpected error: {}", e),
        }
    }

    #[test]
    fn test_store_statistics_creation() {
        let stats = StoreStatistics::default();
        assert_eq!(stats.total_events, 0);
        assert_eq!(stats.errors_encountered, 0);
        assert!(stats.last_maintenance.is_none());
    }

    #[test]
    fn test_detailed_statistics_creation() {
        let detailed = DetailedStatistics {
            store_stats: StoreStatistics::default(),
            connection_stats: ConnectionStats::default(),
            batch_stats: BatchStats::default(),
            compression_stats: CompressionStats::default(),
            event_store_stats: None,
            migration_info: MigrationInfo::new(0, 1),
            query_stats: QueryStats::default(),
        };

        assert_eq!(detailed.store_stats.total_events, 0);
        assert_eq!(detailed.migration_info.current_version, 0);
        assert_eq!(detailed.migration_info.target_version, 1);
    }
}