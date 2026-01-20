//! PostgreSQL connection management
//!
//! This module handles database connection pooling, connection lifecycle management,
//! and connection health checks for the PostgreSQL event store.

use std::time::Duration;
use std::sync::Arc;
use tokio::sync::{RwLock, Mutex};
use tokio::time::{sleep, interval};
use sqlx::{PgPool, postgres::PgPoolOptions, Row};
use tracing::{info, warn, error};
use super::postgres_event_store_config::{PostgresEventStoreConfig, SslMode};

/// Database connection manager
pub struct ConnectionManager {
    /// Configuration for the connection manager
    config: PostgresEventStoreConfig,
    /// Connection pool
    pool: Arc<RwLock<Option<PgPool>>>,
    /// Health check running flag
    health_check_running: Arc<Mutex<bool>>,
    /// Connection statistics
    stats: Arc<RwLock<ConnectionStats>>,
}

/// Connection statistics
#[derive(Debug, Clone, Default)]
pub struct ConnectionStats {
    /// Total connections created
    pub connections_created: u64,
    /// Total connections closed
    pub connections_closed: u64,
    /// Current active connections
    pub active_connections: u32,
    /// Maximum connections used
    pub max_connections_used: u32,
    /// Connection failures
    pub connection_failures: u64,
    /// Health check failures
    pub health_check_failures: u64,
    /// Last health check timestamp
    pub last_health_check: Option<std::time::Instant>,
}

impl ConnectionManager {
    /// Create a new connection manager
    pub async fn new(config: PostgresEventStoreConfig) -> Result<Self, ConnectionError> {
        let pool = Self::create_pool(&config).await?;

        let manager = Self {
            config,
            pool: Arc::new(RwLock::new(Some(pool))),
            health_check_running: Arc::new(Mutex::new(false)),
            stats: Arc::new(RwLock::new(ConnectionStats::default())),
        };

        Ok(manager)
    }

    /// Create a connection pool
    async fn create_pool(config: &PostgresEventStoreConfig) -> Result<PgPool, ConnectionError> {
        // Build connection URL with SSL mode
        let connection_url = Self::build_connection_url(config);

        info!("Creating PostgreSQL connection pool with size {}", config.pool_size);

        let pool = PgPoolOptions::new()
            .max_connections(config.max_pool_size)
            .min_connections(config.pool_size)
            .max_lifetime(Some(config.connection_timeout))
            .idle_timeout(Some(Duration::from_secs(300))) // 5 minutes
            .connection_timeout(config.connection_timeout)
            .connect(&connection_url)
            .await?;

        // Validate connection
        let mut conn = pool.acquire().await?;
        sqlx::query("SELECT 1")
            .fetch_one(&mut conn)
            .await?;

        info!("PostgreSQL connection pool created successfully");
        Ok(pool)
    }

    /// Build connection URL with SSL mode
    fn build_connection_url(config: &PostgresEventStoreConfig) -> String {
        let mut url = config.connection_url.clone();

        // Extract existing query parameters
        let mut url_parts: Vec<&str> = url.split('?').collect();
        let base_url = url_parts[0];
        let mut query_params = if url_parts.len() > 1 {
            url_parts[1..].join("?")
        } else {
            String::new()
        };

        // Add SSL mode parameter if not already present
        if !query_params.contains("sslmode") && !query_params.contains("ssl_mode") {
            if !query_params.is_empty() {
                query_params.push('&');
            }
            query_params.push_str(&format!("sslmode={}", Self::ssl_mode_to_string(config.ssl_mode)));
        }

        // Reconstruct URL
        if query_params.is_empty() {
            base_url.to_string()
        } else {
            format!("{}?{}", base_url, query_params)
        }
    }

    /// Convert SSL mode to string
    fn ssl_mode_to_string(mode: SslMode) -> &'static str {
        match mode {
            SslMode::Disabled => "disable",
            SslMode::Allow => "allow",
            SslMode::Prefer => "prefer",
            SslMode::Require => "require",
            SslMode::VerifyCa => "verify-ca",
            SslMode::VerifyFull => "verify-full",
        }
    }

    /// Get a connection from the pool
    pub async fn get_connection(&self) -> Result<PgConnection, ConnectionError> {
        let pool = self.pool.read().await;
        let pool = pool.as_ref().ok_or(ConnectionError::PoolNotInitialized)?;

        let mut stats = self.stats.write().await;
        stats.connections_created += 1;
        stats.active_connections += 1;
        stats.max_connections_used = stats.max_connections_used.max(stats.active_connections);

        drop(stats);

        let connection = pool.acquire().await.map_err(|e| {
            error!("Failed to acquire connection: {}", e);
            ConnectionError::AcquisitionFailed(e.to_string())
        })?;

        Ok(PgConnection::new(connection, Arc::clone(&self.stats)))
    }

    /// Get the connection pool (advanced usage)
    pub async fn get_pool(&self) -> Result<PgPool, ConnectionError> {
        let pool = self.pool.read().await;
        pool.as_ref().cloned().ok_or(ConnectionError::PoolNotInitialized)
    }

    /// Check database health
    pub async fn health_check(&self) -> Result<HealthStatus, ConnectionError> {
        let start_time = std::time::Instant::now();

        let pool = self.get_pool().await?;
        let mut conn = pool.acquire().await?;

        // Test query
        let result: i32 = sqlx::query("SELECT 1")
            .fetch_one(&mut conn)
            .await?;

        if result != 1 {
            return Err(ConnectionError::HealthCheckFailed("Invalid response".to_string()));
        }

        // Check table existence
        let table_exists: bool = sqlx::query(
            r#"
            SELECT EXISTS (
                SELECT FROM information_schema.tables
                WHERE table_schema = 'public'
                AND table_name = 'vm_events'
            )
            "#
        )
        .fetch_one(&mut conn)
        .await?;

        let mut stats = self.stats.write().await;
        stats.last_health_check = Some(start_time);
        stats.health_check_failures = 0;

        let health_status = HealthStatus {
            is_healthy: true,
            response_time_ms: start_time.elapsed().as_millis() as u64,
            tables_exist: vec![("vm_events".to_string(), table_exists)],
        };

        info!("Health check completed in {}ms", health_status.response_time_ms);
        Ok(health_status)
    }

    /// Start background health checks
    pub async fn start_health_checks(&self) {
        let health_check_running = self.health_check_running.lock().await;
        if *health_check_running {
            return; // Already running
        }
        drop(health_check_running);
        *self.health_check_running.lock().await = true;

        let manager = Arc::new(self);
        let interval_duration = Duration::from_secs(30); // Check every 30 seconds

        tokio::spawn(async move {
            let mut interval = interval(interval_duration);

            while *manager.health_check_running.lock().await {
                interval.tick().await;

                match manager.health_check().await {
                    Ok(status) => {
                        if status.is_healthy {
                            info!("Health check passed");
                        } else {
                            warn!("Health check failed: {:?}", status);
                        }
                    }
                    Err(e) => {
                        warn!("Health check error: {}", e);

                        let mut stats = manager.stats.write().await;
                        stats.health_check_failures += 1;
                    }
                }
            }
        });
    }

    /// Stop background health checks
    pub async fn stop_health_checks(&self) {
        *self.health_check_running.lock() = false;
    }

    /// Get connection statistics
    pub async fn get_stats(&self) -> ConnectionStats {
        self.stats.read().await.clone()
    }

    /// Reset connection pool
    pub async fn reset_pool(&self) -> Result<(), ConnectionError> {
        info!("Resetting PostgreSQL connection pool");

        let new_pool = Self::create_pool(&self.config).await?;
        {
            let mut pool = self.pool.write().await;
            *pool = Some(new_pool);
        }

        let mut stats = self.stats.write().await;
        stats.connections_created = 0;
        stats.active_connections = 0;
        stats.connection_failures = 0;

        Ok(())
    }

    /// Close all connections gracefully
    pub async fn close(&self) -> Result<(), ConnectionError> {
        info!("Closing PostgreSQL connection pool");

        self.stop_health_checks().await;

        let pool = self.pool.write().await.take();
        if let Some(pool) = pool {
            pool.close().await;
        }

        let mut stats = self.stats.write().await;
        stats.connections_closed += stats.active_connections;
        stats.active_connections = 0;

        Ok(())
    }
}

/// Database connection wrapper
pub struct PgConnection {
    /// The underlying SQLx connection
    connection: sqlx::PgConnection,
    /// Connection statistics
    stats: Arc<RwLock<ConnectionStats>>,
}

impl PgConnection {
    /// Create a new connection wrapper
    fn new(connection: sqlx::PgConnection, stats: Arc<RwLock<ConnectionStats>>) -> Self {
        Self { connection, stats }
    }

    /// Execute a query
    pub async fn execute(&mut self, query: &str) -> Result<sqlx::postgres::PgQueryResult, sqlx::Error> {
        self.connection.execute(query).await
    }

    /// Execute a query with parameters
    pub async fn execute_with_params(&mut self, query: &str, params: Vec<Box<dyn sqlx::postgres::PgArgument + Send + Sync>>) -> Result<sqlx::postgres::PgQueryResult, sqlx::Error> {
        self.connection.execute(query).bind_all(params).await
    }

    /// Fetch a single row
    pub async fn fetch_one<T>(&mut self, query: &str) -> Result<T, sqlx::Error>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Unpin + Send,
    {
        self.connection.fetch_one(query).await
    }

    /// Fetch multiple rows
    pub async fn fetch<T>(&mut self, query: &str) -> Result<Vec<T>, sqlx::Error>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Unpin + Send,
    {
        self.connection.fetch(query).await
    }

    /// Begin a transaction
    pub async fn begin(&mut self) -> Result<sqlx::postgres::PgTransaction<'_>, sqlx::Error> {
        self.connection.begin().await
    }

    /// Commit a transaction
    pub async fn commit(&mut self) -> Result<(), sqlx::Error> {
        self.connection.commit().await
    }

    /// Rollback a transaction
    pub async fn rollback(&mut self) -> Result<(), sqlx::Error> {
        self.connection.rollback().await
    }
}

impl Drop for PgConnection {
    fn drop(&mut self) {
        // Note: This is called when the connection wrapper is dropped
        // The actual SQLx connection is managed by the pool
    }
}

/// Connection-related errors
#[derive(Debug, thiserror::Error)]
pub enum ConnectionError {
    #[error("Pool not initialized")]
    PoolNotInitialized,
    #[error("Failed to create connection pool: {0}")]
    PoolCreationFailed(String),
    #[error("Failed to acquire connection: {0}")]
    AcquisitionFailed(String),
    #[error("Connection pool error: {0}")]
    PoolError(#[from] sqlx::Error),
    #[error("Health check failed: {0}")]
    HealthCheckFailed(String),
    #[error("Connection timeout")]
    ConnectionTimeout,
    #[error("SSL configuration error: {0}")]
    SslError(String),
}

/// Database health status
#[derive(Debug, Clone)]
pub struct HealthStatus {
    /// Whether the database is healthy
    pub is_healthy: bool,
    /// Response time in milliseconds
    pub response_time_ms: u64,
    /// Whether required tables exist
    pub tables_exist: Vec<(String, bool)>,
}

impl HealthStatus {
    /// Check if all required tables exist
    pub fn all_tables_exist(&self) -> bool {
        self.tables_exist.iter().all(|(_, exists)| *exists)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_connection_manager_creation() {
        let config = PostgresEventStoreConfig::default();
        let manager = ConnectionManager::new(config).await;

        // Note: This will fail without a real PostgreSQL database
        // but tests the configuration logic
        match manager {
            Ok(_) => {
                // Successfully connected
            }
            Err(ConnectionError::PoolCreationFailed(_)) => {
                // Expected for testing without database
            }
            Err(e) => panic!("Unexpected error: {}", e),
        }
    }

    #[test]
    fn test_ssl_mode_conversion() {
        assert_eq!(ConnectionManager::ssl_mode_to_string(SslMode::Prefer), "prefer");
        assert_eq!(ConnectionManager::ssl_mode_to_string(SslMode::Disable), "disable");
    }

    #[test]
    fn test_connection_stats() {
        let stats = ConnectionStats::default();
        assert_eq!(stats.connections_created, 0);
        assert_eq!(stats.active_connections, 0);
    }

    #[test]
    fn test_health_status() {
        let status = HealthStatus {
            is_healthy: true,
            response_time_ms: 100,
            tables_exist: vec![("vm_events".to_string(), true)],
        };

        assert!(status.all_tables_exist());
    }
}