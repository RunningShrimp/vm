//! PostgreSQL event store batch operations
//!
//! This module provides efficient batch operations for the PostgreSQL event store,
//! including bulk event insertion, deletion, and processing.

use std::time::Duration;
use tokio::sync::{Semaphore, RwLock};
use tokio::time::timeout;
use sqlx::PgPool;
use tracing::{info, warn, error};
use super::postgres_event_store_types::{BatchResult, EventStatus};
use super::postgres_event_store_queries::{QueryManager, BatchEvent};
use super::postgres_event_store_config::PostgresEventStoreConfig;

/// Batch operation manager
pub struct BatchManager {
    /// Query manager
    query_manager: QueryManager,
    /// Configuration
    config: PostgresEventStoreConfig,
    /// Concurrency limiter
    semaphore: Arc<Semaphore>,
    /// Batch operation statistics
    stats: Arc<RwLock<BatchStats>>,
}

/// Batch operation statistics
#[derive(Debug, Clone, Default)]
pub struct BatchStats {
    /// Total batch operations
    pub total_batches: u64,
    /// Successful batches
    pub successful_batches: u64,
    /// Failed batches
    pub failed_batches: u64,
    /// Total events processed
    pub total_events_processed: u64,
    /// Total processing time
    pub total_processing_time_ms: u64,
    /// Average batch size
    pub average_batch_size: f64,
    /// Largest batch size
    pub largest_batch_size: u32,
    /// Slowest batch time
    pub slowest_batch_time_ms: u64,
}

impl BatchManager {
    /// Create a new batch manager
    pub fn new(config: PostgresEventStoreConfig, query_manager: QueryManager) -> Self {
        Self {
            query_manager,
            config,
            semaphore: Arc::new(Semaphore::new(config.batch_size as usize)),
            stats: Arc::new(RwLock::new(BatchStats::default())),
        }
    }

    /// Insert events in batches
    pub async fn batch_insert_events(
        &self,
        pool: &PgPool,
        events: Vec<BatchEvent>,
    ) -> Result<BatchResult, BatchError> {
        if events.is_empty() {
            return Ok(BatchResult::new(0, 0, 0, 0));
        }

        let start_time = std::time::Instant::now();
        let batch_size = self.config.batch_size as usize;

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.total_batches += 1;
            stats.total_events_processed += events.len() as u64;
            stats.largest_batch_size = stats.largest_batch_size.max(events.len() as u32);
            stats.average_batch_size = (stats.average_batch_size * (stats.total_batches - 1) as f64 + events.len() as f64) / stats.total_batches as f64;
        }

        // Process events in batches
        let mut total_processed = 0;
        let mut total_success = 0;
        let mut total_failures = 0;
        let mut batch_errors = Vec::new();

        for chunk in events.chunks(batch_size) {
            match self.process_batch(pool, chunk.to_vec()).await {
                Ok(processed) => {
                    total_success += processed;
                    total_processed += processed;
                }
                Err(BatchError::PartialBatch { processed, errors }) => {
                    total_success += processed;
                    total_processed += processed;
                    total_failures += chunk.len() - processed;
                    batch_errors.extend(errors);
                }
                Err(e) => {
                    error!("Batch insert failed: {}", e);
                    total_failures += chunk.len();
                    total_processed += chunk.len();
                    batch_errors.push(format!("Batch failed: {}", e));
                }
            }
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_processing_time_ms += duration_ms;
            if duration_ms > stats.slowest_batch_time_ms {
                stats.slowest_batch_time_ms = duration_ms;
            }

            if total_failures == 0 {
                stats.successful_batches += 1;
            } else {
                stats.failed_batches += 1;
            }
        }

        let result = BatchResult::new(total_processed, total_success, total_failures, duration_ms)
            .add_errors(batch_errors);

        info!(
            "Batch insert completed: {} events ({} success, {} failures) in {}ms",
            total_processed, total_success, total_failures, duration_ms
        );

        Ok(result)
    }

    /// Process a single batch
    async fn process_batch(
        &self,
        pool: &PgPool,
        events: Vec<BatchEvent>,
    ) -> Result<u64, BatchError> {
        // Acquire semaphore for concurrency control
        let _permit = self.semaphore.acquire().await.map_err(|_| {
            BatchError::ConcurrencyLimit("Failed to acquire semaphore".to_string())
        })?;

        // Timeout protection
        timeout(
            Duration::from_secs(300), // 5 minutes
            self.insert_batch_with_retry(pool, events)
        ).await.map_err(|_| {
            BatchError::Timeout("Batch operation timed out".to_string())
        })?
    }

    /// Insert batch with retry logic
    async fn insert_batch_with_retry(
        &self,
        pool: &PgPool,
        mut events: Vec<BatchEvent>,
    ) -> Result<u64, BatchError> {
        let mut retry_count = 0;
        let max_retries = self.config.retry_settings.max_retries;
        let initial_backoff = self.config.retry_settings.initial_backoff;

        loop {
            match self.query_manager.batch_insert_events(pool, events.clone()).await {
                Ok(inserted_count) => {
                    return Ok(inserted_count as u64);
                }
                Err(sqlx::Error::Database(e)) => {
                    if retry_count < max_retries && self.is_retryable_error(&e) {
                        retry_count += 1;
                        let backoff = initial_backoff.mul_f64(self.config.retry_settings.backoff_multiplier.powi(retry_count as i32));
                        warn!(
                            "Retry {} after {}ms due to database error: {}",
                            retry_count, backoff.as_millis(), e
                        );
                        tokio::time::sleep(backoff).await;
                        continue;
                    } else {
                        return Err(BatchError::DatabaseError(e.to_string()));
                    }
                }
                Err(e) => {
                    return Err(BatchError::DatabaseError(e.to_string()));
                }
            }
        }
    }

    /// Check if error is retryable
    fn is_retryable_error(&self, error: &sqlx::Error) -> bool {
        let error_str = error.to_string().to_lowercase();
        self.config.retry_settings.retryable_errors.iter().any(|err| {
            error_str.contains(&err.to_lowercase())
        })
    }

    /// Delete events for multiple VMs in batch
    pub async fn batch_delete_vms(
        &self,
        pool: &PgPool,
        vm_ids: Vec<String>,
    ) -> Result<BatchResult, BatchError> {
        if vm_ids.is_empty() {
            return Ok(BatchResult::new(0, 0, 0, 0));
        }

        let start_time = std::time::Instant::now();
        let batch_size = self.config.batch_size as usize;

        let mut total_processed = 0;
        let mut total_success = 0;
        let mut total_failures = 0;
        let mut errors = Vec::new();

        for chunk in vm_ids.chunks(batch_size) {
            match self.process_delete_batch(pool, chunk.to_vec()).await {
                Ok(deleted_count) => {
                    total_success += deleted_count;
                    total_processed += chunk.len() as u64;
                }
                Err(e) => {
                    error!("Delete batch failed: {}", e);
                    total_failures += chunk.len() as u64;
                    total_processed += chunk.len() as u64;
                    errors.push(format!("Delete batch failed: {}", e));
                }
            }
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;

        Ok(BatchResult::new(total_processed, total_success, total_failures, duration_ms)
            .add_errors(errors))
    }

    /// Process delete batch
    async fn process_delete_batch(
        &self,
        pool: &PgPool,
        vm_ids: Vec<String>,
    ) -> Result<u64, BatchError> {
        let mut total_deleted = 0;

        for vm_id in vm_ids {
            match self.query_manager.delete_events(pool, &vm_id).await {
                Ok(deleted_count) => {
                    total_deleted += deleted_count;
                }
                Err(e) => {
                    return Err(BatchError::DeleteFailed(vm_id, e.to_string()));
                }
            }
        }

        Ok(total_deleted)
    }

    /// Update event statuses in batch
    pub async fn batch_update_statuses(
        &self,
        pool: &PgPool,
        updates: Vec<StatusUpdate>,
    ) -> Result<BatchResult, BatchError> {
        if updates.is_empty() {
            return Ok(BatchResult::new(0, 0, 0, 0));
        }

        let start_time = std::time::Instant::now();
        let batch_size = self.config.batch_size as usize;

        let mut total_processed = 0;
        let mut total_success = 0;
        let mut total_failures = 0;
        let mut errors = Vec::new();

        for chunk in updates.chunks(batch_size) {
            match self.process_status_update_batch(pool, chunk.to_vec()).await {
                Ok(updated_count) => {
                    total_success += updated_count;
                    total_processed += chunk.len() as u64;
                }
                Err(e) => {
                    error!("Status update batch failed: {}", e);
                    total_failures += chunk.len() as u64;
                    total_processed += chunk.len() as u64;
                    errors.push(format!("Status update failed: {}", e));
                }
            }
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;

        Ok(BatchResult::new(total_processed, total_success, total_failures, duration_ms)
            .add_errors(errors))
    }

    /// Process status update batch
    async fn process_status_update_batch(
        &self,
        pool: &PgPool,
        updates: Vec<StatusUpdate>,
    ) -> Result<u64, BatchError> {
        let mut total_updated = 0;

        for update in updates {
            match self.query_manager.update_event_status(
                pool,
                &update.vm_id,
                update.sequence_number,
                update.status,
            ).await {
                Ok(_) => {
                    total_updated += 1;
                }
                Err(e) => {
                    return Err(BatchError::StatusUpdateFailed(update.vm_id, update.sequence_number, e.to_string()));
                }
            }
        }

        Ok(total_updated)
    }

    /// Batch load events with multiple queries
    pub async fn batch_load_events(
        &self,
        pool: &PgPool,
        queries: Vec<EventQuery>,
    ) -> Result<Vec<BatchLoadResult>, BatchError> {
        let mut results = Vec::new();
        let semaphore = Arc::clone(&self.semaphore);

        // Use concurrent tasks for better performance
        let tasks: Vec<_> = queries.into_iter().map(|query| {
            let pool = pool.clone();
            let semaphore = Arc::clone(&semaphore);

            async move {
                // Acquire semaphore
                let _permit = semaphore.acquire().await.unwrap();

                // Execute query
                match self.query_manager.load_events(&pool, &query.params).await {
                    Ok(rows) => {
                        let events = rows.into_iter().map(|row| {
                            // Convert row to event data
                            serde_json::json!({
                                "id": row.get::<i64, _>("id"),
                                "sequence_number": row.get::<i64, _>("sequence_number"),
                                "vm_id": row.get::<String, _>("vm_id"),
                                "event_type": row.get::<String, _>("event_type"),
                                "event_version": row.get::<i32, _>("event_version"),
                                "event_data": row.get::<Vec<u8>, _>("event_data"),
                                "metadata": row.get::<Option<serde_json::Value>, _>("metadata"),
                                "created_at": row.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
                                "updated_at": row.get::<chrono::DateTime<chrono::Utc>, _>("updated_at"),
                            })
                        }).collect();

                        BatchLoadResult {
                            query_id: query.id,
                            success: true,
                            events,
                            error: None,
                        }
                    }
                    Err(e) => BatchLoadResult {
                        query_id: query.id,
                        success: false,
                        events: Vec::new(),
                        error: Some(e.to_string()),
                    },
                }
            }
        }).collect();

        // Wait for all tasks to complete
        let task_results = futures::future::join_all(tasks).await;
        results.extend(task_results);

        Ok(results)
    }

    /// Get batch operation statistics
    pub async fn get_batch_stats(&self) -> BatchStats {
        self.stats.read().await.clone()
    }

    /// Reset batch statistics
    pub async fn reset_batch_stats(&self) {
        *self.stats.write().await = BatchStats::default();
    }
}

/// Status update data
#[derive(Debug, Clone)]
pub struct StatusUpdate {
    /// Virtual machine ID
    pub vm_id: String,
    /// Sequence number
    pub sequence_number: u64,
    /// New status
    pub status: EventStatus,
}

/// Event query data
#[derive(Debug, Clone)]
pub struct EventQuery {
    /// Query ID
    pub id: String,
    /// Query parameters
    pub params: Box<super::postgres_event_store_types::EventQueryParams>,
}

/// Batch load result
#[derive(Debug, Clone, serde::Serialize)]
pub struct BatchLoadResult {
    /// Query ID
    pub query_id: String,
    /// Whether query was successful
    pub success: bool,
    /// Loaded events
    pub events: Vec<serde_json::Value>,
    /// Error message if failed
    pub error: Option<String>,
}

/// Batch operation errors
#[derive(Debug, thiserror::Error)]
pub enum BatchError {
    #[error("Batch operation failed: {0}")]
    OperationFailed(String),
    #[error("Concurrency limit exceeded: {0}")]
    ConcurrencyLimit(String),
    #[error("Batch operation timeout: {0}")]
    Timeout(String),
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Delete failed for VM {0}: {1}")]
    DeleteFailed(String, String),
    #[error("Status update failed for VM {0}, sequence {1}: {2}")]
    StatusUpdateFailed(String, u64, String),
    #[error("Partial batch processed: {0} successful, {1} failures")]
    PartialBatch { processed: u64, errors: Vec<String> },
}

impl BatchError {
    /// Create a partial batch error
    pub fn partial_batch(processed: u64, errors: Vec<String>) -> Self {
        Self::PartialBatch { processed, errors }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_event_creation() {
        let event = BatchEvent::new(
            1,
            "vm1".to_string(),
            "TestEvent".to_string(),
            1,
            vec![1, 2, 3],
        );

        assert_eq!(event.sequence_number, 1);
        assert_eq!(event.vm_id, "vm1");
        assert_eq!(event.event_size, 3);
    }

    #[test]
    fn test_status_update_creation() {
        let update = StatusUpdate {
            vm_id: "vm1".to_string(),
            sequence_number: 1,
            status: EventStatus::Active,
        };

        assert_eq!(update.vm_id, "vm1");
        assert_eq!(update.sequence_number, 1);
        assert_eq!(update.status, EventStatus::Active);
    }

    #[test]
    fn test_event_query_creation() {
        let query = EventQuery {
            id: "test_query".to_string(),
            params: Box::new(super::postgres_event_store_types::EventQueryParams::new("vm1".to_string())),
        };

        assert_eq!(query.id, "test_query");
        assert_eq!(query.params.vm_id, "vm1");
    }

    #[test]
    fn test_batch_error_creation() {
        let error = BatchError::ConcurrencyLimit("Limit exceeded".to_string());
        assert!(error.to_string().contains("Concurrency limit"));
    }
}