//! PostgreSQL event store configuration
//!
//! This module provides configuration structs and builders for the PostgreSQL event store.
//! It handles database connection settings, retry policies, and performance tuning.

use std::time::Duration;
use serde::{Deserialize, Serialize};

/// PostgreSQL event store configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresEventStoreConfig {
    /// Database connection URL
    pub connection_url: String,
    /// Database connection pool size
    pub pool_size: u32,
    /// Maximum connection pool size
    pub max_pool_size: u32,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Query timeout
    pub query_timeout: Duration,
    /// Enable SSL
    pub ssl_mode: SslMode,
    /// Retry settings
    pub retry_settings: RetrySettings,
    /// Performance settings
    pub performance: PerformanceSettings,
    /// Table settings
    pub table_settings: TableSettings,
    /// Enable event indexing
    pub enable_indexing: bool,
    /// Event batch size for bulk operations
    pub batch_size: u32,
    /// Enable connection keep-alive
    pub keep_alive: bool,
    /// Keep-alive interval
    pub keep_alive_interval: Duration,
}

/// SSL mode for PostgreSQL connection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SslMode {
    /// Disable SSL
    Disabled,
    /// Allow SSL connection (prefer non-SSL)
    Allow,
    /// Prefer SSL connection
    Prefer,
    /// Require SSL connection
    Require,
    /// Verify full certificate chain
    VerifyCa,
    /// Verify full certificate chain and hostname
    VerifyFull,
}

impl Default for SslMode {
    fn default() -> Self {
        SslMode::Prefer
    }
}

/// Retry settings for failed operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrySettings {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial backoff duration
    pub initial_backoff: Duration,
    /// Maximum backoff duration
    pub max_backoff: Duration,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
    /// Enable jitter for backoff
    pub enable_jitter: bool,
    /// Retryable error types
    pub retryable_errors: Vec<String>,
}

impl Default for RetrySettings {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            enable_jitter: true,
            retryable_errors: vec![
                "connection_error".to_string(),
                "timeout".to_string(),
                "deadlock_detected".to_string(),
            ],
        }
    }
}

/// Performance settings for PostgreSQL event store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSettings {
    /// Enable prepared statements
    pub enable_prepared_statements: bool,
    /// Enable statement caching
    pub enable_statement_cache: bool,
    /// Cache size in number of statements
    pub statement_cache_size: usize,
    /// Enable connection pooling
    pub enable_connection_pooling: bool,
    /// Enable query logging
    pub enable_query_logging: bool,
    /// Enable metrics collection
    pub enable_metrics: bool,
    /// Query sampling rate for logging (0.0 to 1.0)
    pub query_sampling_rate: f64,
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            enable_prepared_statements: true,
            enable_statement_cache: true,
            statement_cache_size: 100,
            enable_connection_pooling: true,
            enable_query_logging: false,
            enable_metrics: true,
            query_sampling_rate: 0.01, // 1%
        }
    }
}

/// Table settings for PostgreSQL event store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSettings {
    /// Events table name
    pub events_table_name: String,
    /// Metadata table name
    pub metadata_table_name: String,
    /// Enable partitioning
    pub enable_partitioning: bool,
    /// Partition interval (in days)
    pub partition_interval_days: u32,
    /// Enable vacuuming
    pub enable_vacuum: bool,
    /// Analyze statistics periodically
    pub enable_analyze: bool,
}

impl Default for TableSettings {
    fn default() -> Self {
        Self {
            events_table_name: "vm_events".to_string(),
            metadata_table_name: "vm_event_metadata".to_string(),
            enable_partitioning: true,
            partition_interval_days: 7,
            enable_vacuum: true,
            enable_analyze: true,
        }
    }
}

/// Builder for PostgreSQL event store configuration
#[derive(Debug, Clone)]
pub struct PostgresEventStoreConfigBuilder {
    config: PostgresEventStoreConfig,
}

impl PostgresEventStoreConfigBuilder {
    /// Create a new builder with default configuration
    pub fn new() -> Self {
        Self {
            config: PostgresEventStoreConfig::default(),
        }
    }

    /// Create a new builder with connection URL
    pub fn with_connection_url(mut self, url: String) -> Self {
        self.config.connection_url = url;
        self
    }

    /// Set connection pool size
    pub fn with_pool_size(mut self, size: u32) -> Self {
        self.config.pool_size = size;
        self
    }

    /// Set maximum connection pool size
    pub fn with_max_pool_size(mut self, max_size: u32) -> Self {
        self.config.max_pool_size = max_size;
        self
    }

    /// Set connection timeout
    pub fn with_connection_timeout(mut self, timeout: Duration) -> Self {
        self.config.connection_timeout = timeout;
        self
    }

    /// Set query timeout
    pub fn with_query_timeout(mut self, timeout: Duration) -> Self {
        self.config.query_timeout = timeout;
        self
    }

    /// Set SSL mode
    pub fn with_ssl_mode(mut self, mode: SslMode) -> Self {
        self.config.ssl_mode = mode;
        self
    }

    /// Set retry settings
    pub fn with_retry_settings(mut self, settings: RetrySettings) -> Self {
        self.config.retry_settings = settings;
        self
    }

    /// Set performance settings
    pub fn with_performance_settings(mut self, settings: PerformanceSettings) -> Self {
        self.config.performance = settings;
        self
    }

    /// Set table settings
    pub fn with_table_settings(mut self, settings: TableSettings) -> Self {
        self.config.table_settings = settings;
        self
    }

    /// Set event batch size
    pub fn with_batch_size(mut self, batch_size: u32) -> Self {
        self.config.batch_size = batch_size;
        self
    }

    /// Enable or disable indexing
    pub fn with_indexing(mut self, enable: bool) -> Self {
        self.config.enable_indexing = enable;
        self
    }

    /// Enable or disable keep-alive
    pub fn with_keep_alive(mut self, enable: bool) -> Self {
        self.config.keep_alive = enable;
        self
    }

    /// Set keep-alive interval
    pub fn with_keep_alive_interval(mut self, interval: Duration) -> Self {
        self.config.keep_alive_interval = interval;
        self
    }

    /// Build the configuration
    pub fn build(self) -> PostgresEventStoreConfig {
        self.config
    }
}

impl Default for PostgresEventStoreConfig {
    fn default() -> Self {
        Self {
            connection_url: "postgresql://localhost:5432/vm".to_string(),
            pool_size: 10,
            max_pool_size: 20,
            connection_timeout: Duration::from_secs(30),
            query_timeout: Duration::from_secs(30),
            ssl_mode: SslMode::default(),
            retry_settings: RetrySettings::default(),
            performance: PerformanceSettings::default(),
            table_settings: TableSettings::default(),
            enable_indexing: true,
            batch_size: 100,
            keep_alive: true,
            keep_alive_interval: Duration::from_secs(60),
        }
    }
}

impl PostgresEventStoreConfig {
    /// Create a new builder
    pub fn builder() -> PostgresEventStoreConfigBuilder {
        PostgresEventStoreConfigBuilder::new()
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.connection_url.is_empty() {
            return Err("Connection URL cannot be empty".to_string());
        }
        if self.pool_size == 0 {
            return Err("Pool size must be greater than 0".to_string());
        }
        if self.max_pool_size < self.pool_size {
            return Err("Max pool size must be greater than or equal to pool size".to_string());
        }
        if self.batch_size == 0 {
            return Err("Batch size must be greater than 0".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let config = PostgresEventStoreConfig::default();
        assert!(config.validate().is_ok());

        let invalid_config = PostgresEventStoreConfig {
            connection_url: "".to_string(),
            ..config
        };
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_config_builder() {
        let config = PostgresEventStoreConfig::builder()
            .with_connection_url("postgresql://localhost:5432/test".to_string())
            .with_pool_size(5)
            .with_max_pool_size(15)
            .build();

        assert_eq!(config.connection_url, "postgresql://localhost:5432/test");
        assert_eq!(config.pool_size, 5);
        assert_eq!(config.max_pool_size, 15);
    }
}