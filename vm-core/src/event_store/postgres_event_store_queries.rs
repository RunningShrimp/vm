//! PostgreSQL event store queries
//!
//! This module contains all SQL queries and prepared statements for the PostgreSQL event store,
//! providing a centralized location for query management and optimization.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use sqlx::{postgres::PgPool, query, Row};
use tracing::{debug, info, warn};
use super::postgres_event_store_types::{EventQueryParams, EventStatus, SortOrder};
use super::postgres_event_store_config::PostgresEventStoreConfig;

/// Query manager for PostgreSQL event store
pub struct QueryManager {
    /// Configuration
    config: PostgresEventStoreConfig,
    /// Prepared statements cache
    prepared_statements: Arc<RwLock<HashMap<String, PreparedStatement>>>,
    /// Query execution statistics
    stats: Arc<RwLock<QueryStats>>,
}

/// Prepared statement wrapper
#[derive(Debug, Clone)]
pub struct PreparedStatement {
    /// SQL query
    pub sql: String,
    /// Parameters count
    pub param_count: usize,
    /// Last used timestamp
    pub last_used: chrono::DateTime<chrono::Utc>,
    /// Execution count
    pub execution_count: u64,
    /// Total execution time
    pub total_execution_time_ms: u64,
}

/// Query statistics
#[derive(Debug, Clone, Default)]
pub struct QueryStats {
    /// Total queries executed
    pub total_queries: u64,
    /// Failed queries
    pub failed_queries: u64,
    /// Average query time
    pub average_query_time_ms: f64,
    /// Slow query threshold
    pub slow_query_threshold_ms: u64,
    /// Slow query count
    pub slow_query_count: u64,
    /// Most frequent queries
    pub frequent_queries: HashMap<String, u64>,
}

impl QueryManager {
    /// Create a new query manager
    pub fn new(config: PostgresEventStoreConfig) -> Self {
        Self {
            config,
            prepared_statements: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(QueryStats {
                slow_query_threshold_ms: 1000, // 1 second
                ..Default::default()
            })),
        }
    }

    /// Get or create a prepared statement
    pub async fn get_prepared_statement(&self, pool: &PgPool, name: &str, sql: &str) -> Result<sqlx::postgres::PgStatement, sqlx::Error> {
        let mut prepared = self.prepared_statements.write().await;

        if let Some(stmt) = prepared.get(name) {
            // Update last used timestamp
            stmt.last_used = chrono::Utc::now();
            return Ok(self.prepare_statement(pool, sql).await?);
        }

        // Create new prepared statement
        let statement = self.prepare_statement(pool, sql).await?;
        let param_count = self.count_params(sql);

        prepared.insert(name.to_string(), PreparedStatement {
            sql: sql.to_string(),
            param_count,
            last_used: chrono::Utc::now(),
            execution_count: 0,
            total_execution_time_ms: 0,
        });

        Ok(statement)
    }

    /// Prepare a SQL statement
    async fn prepare_statement(&self, pool: &PgPool, sql: &str) -> Result<sqlx::postgres::PgStatement, sqlx::Error> {
        if self.config.performance.enable_prepared_statements {
            pool.prepare(sql).await
        } else {
            // Return a dummy statement for non-prepared mode
            Ok(sqlx::postgres::PgStatement {
                sql: sql.to_string(),
                columns: vec![],
                param_types: vec![],
                column_types: vec![],
            })
        }
    }

    /// Count parameters in SQL statement
    fn count_params(&self, sql: &str) -> usize {
        sql.chars().filter(|c| *c == '$').count()
    }

    /// Insert a new event
    pub async fn insert_event(
        &self,
        pool: &PgPool,
        vm_id: &str,
        sequence_number: u64,
        event_type: &str,
        event_version: i32,
        event_data: &[u8],
        metadata: Option<&serde_json::Value>,
        event_size: u64,
        compressed_size: Option<u64>,
        checksum: Option<&str>,
    ) -> Result<u64, sqlx::Error> {
        let start_time = std::time::Instant::now();
        let sql = r#"
            INSERT INTO vm_events (
                sequence_number, vm_id, event_type, event_version,
                event_data, metadata, event_size, compressed_size, checksum
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id
        "#;

        let mut tx = pool.begin().await?;
        let id: i64 = query(sql)
            .bind(sequence_number as i64)
            .bind(vm_id)
            .bind(event_type)
            .bind(event_version)
            .bind(event_data)
            .bind(metadata)
            .bind(event_size as i64)
            .bind(compressed_size.map(|s| s as i64))
            .bind(checksum)
            .fetch_one(&mut tx)
            .await?;
        tx.commit().await?;

        self.record_query("insert_event", start_time.elapsed().as_millis() as u64).await;
        Ok(id as u64)
    }

    /// Load events with query parameters
    pub async fn load_events(
        &self,
        pool: &PgPool,
        params: &EventQueryParams,
    ) -> Result<Vec<sqlx::postgres::PgRow>, sqlx::Error> {
        let start_time = std::time::Instant::now();
        let (sql, binds) = self.build_load_events_query(params);

        let mut result = query(&sql)
            .bind_all(binds)
            .fetch_all(pool)
            .await?;

        self.record_query("load_events", start_time.elapsed().as_millis() as u64).await;
        Ok(result)
    }

    /// Build load events query
    fn build_load_events_query(&self, params: &EventQueryParams) -> (String, Vec<Box<dyn sqlx::postgres::PgArgument + Send + Sync>>) {
        let mut sql = "SELECT id, sequence_number, vm_id, event_type, event_version, event_data, metadata, created_at, updated_at FROM vm_events WHERE 1=1".to_string();
        let mut binds: Vec<Box<dyn sqlx::postgres::PgArgument + Send + Sync>> = Vec::new();

        // VM ID
        sql.push_str(" AND vm_id = $1");
        binds.push(Box::new(params.vm_id.clone()));

        // Sequence range
        if let Some(from_seq) = params.from_sequence {
            sql.push_str(" AND sequence_number >= $2");
            binds.push(Box::new(from_seq as i64));
        }

        if let Some(to_seq) = params.to_sequence {
            sql.push_str(" AND sequence_number <= $3");
            binds.push(Box::new(to_seq as i64));
        }

        // Event types
        if let Some(ref event_types) = params.event_types {
            sql.push_str(" AND event_type = ANY($4)");
            binds.push(Box::new(event_types.clone()));
        }

        // Statuses
        if let Some(ref statuses) = params.statuses {
            let status_values: Vec<String> = statuses.iter().map(|s| s.to_string()).collect();
            sql.push_str(" AND status = ANY($5)");
            binds.push(Box::new(status_values));
        }

        // Time range
        if let Some(after) = params.created_after {
            sql.push_str(" AND created_at >= $6");
            binds.push(Box::new(after));
        }

        if let Some(before) = params.created_before {
            sql.push_str(" AND created_at <= $7");
            binds.push(Box::new(before));
        }

        // Order by
        let order_by = match params.sort_order {
            SortOrder::Asc => "sequence_number ASC",
            SortOrder::Desc => "sequence_number DESC",
        };
        sql.push_str(&format!(" ORDER BY {}", order_by));

        // Limit and offset
        if let Some(limit) = params.limit {
            sql.push_str(" LIMIT $8");
            binds.push(Box::new(limit as i64));
        }

        if let Some(offset) = params.offset {
            sql.push_str(" OFFSET $9");
            binds.push(Box::new(offset as i64));
        }

        (sql, binds)
    }

    /// Get last sequence number
    pub async fn get_last_sequence_number(&self, pool: &PgPool, vm_id: &str) -> Result<Option<u64>, sqlx::Error> {
        let start_time = std::time::Instant::now();

        let result: Option<i64> = query(
            "SELECT MAX(sequence_number) FROM vm_events WHERE vm_id = $1"
        )
        .bind(vm_id)
        .fetch_optional(pool)
        .await?;

        self.record_query("get_last_sequence_number", start_time.elapsed().as_millis() as u64).await;
        Ok(result.map(|n| n as u64))
    }

    /// Get event count
    pub async fn get_event_count(&self, pool: &PgPool, vm_id: &str) -> Result<i64, sqlx::Error> {
        let start_time = std::time::Instant::now();

        let count: i64 = query(
            "SELECT COUNT(*) FROM vm_events WHERE vm_id = $1"
        )
        .bind(vm_id)
        .fetch_one(pool)
        .await?;

        self.record_query("get_event_count", start_time.elapsed().as_millis() as u64).await;
        Ok(count)
    }

    /// List all VM IDs
    pub async fn list_vm_ids(&self, pool: &PgPool, limit: Option<i64>) -> Result<Vec<String>, sqlx::Error> {
        let start_time = std::time::Instant::now();

        let mut sql = "SELECT DISTINCT vm_id FROM vm_events".to_string();
        if let Some(lim) = limit {
            sql.push_str(&format!(" LIMIT {}", lim));
        }

        let rows = query(&sql).fetch_all(pool).await?;
        let vm_ids: Vec<String> = rows.into_iter().map(|row| row.get(0)).collect();

        self.record_query("list_vm_ids", start_time.elapsed().as_millis() as u64).await;
        Ok(vm_ids)
    }

    /// Delete events for a VM
    pub async fn delete_events(&self, pool: &PgPool, vm_id: &str) -> Result<i64, sqlx::Error> {
        let start_time = std::time::Instant::now();

        let result = query(
            "DELETE FROM vm_events WHERE vm_id = $1 RETURNING id"
        )
        .bind(vm_id)
        .execute(pool)
        .await?;

        let deleted_count = result.rows_affected() as i64;
        self.record_query("delete_events", start_time.elapsed().as_millis() as u64).await;
        Ok(deleted_count)
    }

    /// Batch insert events
    pub async fn batch_insert_events(
        &self,
        pool: &PgPool,
        events: Vec<BatchEvent>,
    ) -> Result<u64, sqlx::Error> {
        if events.is_empty() {
            return Ok(0);
        }

        let start_time = std::time::Instant::now();

        let mut tx = pool.begin().await?;
        let mut inserted_count = 0;

        for event in events {
            query(r#"
                INSERT INTO vm_events (
                    sequence_number, vm_id, event_type, event_version,
                    event_data, metadata, event_size, compressed_size, checksum
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#)
            .bind(event.sequence_number as i64)
            .bind(&event.vm_id)
            .bind(&event.event_type)
            .bind(event.event_version)
            .bind(&event.event_data)
            .bind(&event.metadata)
            .bind(event.event_size as i64)
            .bind(event.compressed_size.map(|s| s as i64))
            .bind(&event.checksum)
            .execute(&mut tx)
            .await?;

            inserted_count += 1;
        }

        tx.commit().await?;

        self.record_query("batch_insert_events", start_time.elapsed().as_millis() as u64).await;
        Ok(inserted_count)
    }

    /// Update event status
    pub async fn update_event_status(
        &self,
        pool: &PgPool,
        vm_id: &str,
        sequence_number: u64,
        status: EventStatus,
    ) -> Result<(), sqlx::Error> {
        let start_time = std::time::Instant::now();

        query(r#"
            UPDATE vm_events
            SET status = $1, updated_at = NOW()
            WHERE vm_id = $2 AND sequence_number = $3
        "#)
        .bind(status.to_string())
        .bind(vm_id)
        .bind(sequence_number as i64)
        .execute(pool)
        .await?;

        self.record_query("update_event_status", start_time.elapsed().as_millis() as u64).await;
        Ok(())
    }

    /// Get event store statistics
    pub async fn get_event_store_stats(&self, pool: &PgPool) -> Result<serde_json::Value, sqlx::Error> {
        let start_time = std::time::Instant::now();

        let total_events: i64 = query("SELECT COUNT(*) FROM vm_events").fetch_one(pool).await?;
        let active_events: i64 = query("SELECT COUNT(*) FROM vm_events WHERE status = 'active'").fetch_one(pool).await?;
        let archived_events: i64 = query("SELECT COUNT(*) FROM vm_events WHERE status = 'archived'").fetch_one(pool).await?;
        let vm_count: i64 = query("SELECT COUNT(DISTINCT vm_id) FROM vm_events").fetch_one(pool).await?;

        let oldest_event: Option<chrono::DateTime<chrono::Utc>> = query(
            "SELECT MIN(created_at) FROM vm_events"
        ).fetch_optional(pool).await?;

        let newest_event: Option<chrono::DateTime<chrono::Utc>> = query(
            "SELECT MAX(created_at) FROM vm_events"
        ).fetch_optional(pool).await?;

        let total_storage: i64 = query(
            "SELECT COALESCE(SUM(event_size), 0) FROM vm_events"
        ).fetch_one(pool).await?;

        let stats = serde_json::json!({
            "total_events": total_events,
            "active_events": active_events,
            "archived_events": archived_events,
            "vm_count": vm_count,
            "total_storage_bytes": total_storage,
            "average_event_size": if total_events > 0 { total_storage / total_events } else { 0 },
            "oldest_event": oldest_event,
            "newest_event": newest_event,
        });

        self.record_query("get_event_store_stats", start_time.elapsed().as_millis() as u64).await;
        Ok(stats)
    }

    /// Record query execution
    async fn record_query(&self, query_name: &str, execution_time_ms: u64) {
        let mut stats = self.stats.write().await;
        stats.total_queries += 1;

        // Check if slow query
        if execution_time_ms > stats.slow_query_threshold_ms {
            stats.slow_query_count += 1;
            warn!("Slow query detected: {} took {}ms", query_name, execution_time_ms);
        }

        // Update frequent queries
        *stats.frequent_queries.entry(query_name.to_string()).or_insert(0) += 1;

        // Update average
        stats.average_query_time_ms =
            ((stats.average_query_time_ms * (stats.total_queries - 1) as f64) + execution_time_ms as f64)
            / stats.total_queries as f64;

        debug!("Query {} executed in {}ms", query_name, execution_time_ms);
    }

    /// Get query statistics
    pub async fn get_query_stats(&self) -> QueryStats {
        self.stats.read().await.clone()
    }

    /// Clear prepared statements cache
    pub async fn clear_prepared_statements(&self) {
        self.prepared_statements.write().await.clear();
        info!("Prepared statements cache cleared");
    }

    /// Optimize table statistics
    pub async fn analyze_table(&self, pool: &PgPool) -> Result<(), sqlx::Error> {
        query("ANALYZE vm_events").execute(pool).await?;
        Ok(())
    }
}

/// Batch event data structure
#[derive(Debug, Clone)]
pub struct BatchEvent {
    /// Sequence number
    pub sequence_number: u64,
    /// Virtual machine ID
    pub vm_id: String,
    /// Event type
    pub event_type: String,
    /// Event version
    pub event_version: i32,
    /// Event data
    pub event_data: Vec<u8>,
    /// Metadata
    pub metadata: Option<serde_json::Value>,
    /// Event size
    pub event_size: u64,
    /// Compressed size
    pub compressed_size: Option<u64>,
    /// Checksum
    pub checksum: Option<String>,
}

impl BatchEvent {
    pub fn new(
        sequence_number: u64,
        vm_id: String,
        event_type: String,
        event_version: i32,
        event_data: Vec<u8>,
    ) -> Self {
        Self {
            sequence_number,
            vm_id,
            event_type,
            event_version,
            event_data,
            metadata: None,
            event_size: event_data.len() as u64,
            compressed_size: None,
            checksum: None,
        }
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
        assert_eq!(event.event_type, "TestEvent");
        assert_eq!(event.event_size, 3);
    }

    #[test]
    fn test_query_manager_creation() {
        let config = PostgresEventStoreConfig::default();
        let manager = QueryManager::new(config);
        assert!(manager.config.enable_indexing);
    }
}