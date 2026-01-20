//! PostgreSQL event store migrations
//!
//! This module handles database schema migrations for the PostgreSQL event store,
//! including table creation, index management, and version upgrades.

use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::RwLock;
use sqlx::{PgPool, postgres::PgConnection, query};
use tracing::{info, warn, error};
use super::postgres_event_store_types::{MigrationInfo, MigrationStatus};
use super::postgres_event_store_config::PostgresEventStoreConfig;

/// Migration manager for PostgreSQL event store
pub struct MigrationManager {
    /// Database configuration
    config: PostgresEventStoreConfig,
    /// Current migration info
    migration_info: Arc<RwLock<MigrationInfo>>,
    /// Applied migrations cache
    applied_migrations: Arc<RwLock<HashMap<String, MigrationRecord>>>,
}

/// Migration record
#[derive(Debug, Clone)]
pub struct MigrationRecord {
    /// Migration name
    pub name: String,
    /// Migration version
    pub version: i32,
    /// Applied timestamp
    pub applied_at: chrono::DateTime<chrono::Utc>,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// Success status
    pub successful: bool,
}

/// Database migration definition
pub trait Migration {
    /// Get migration name
    fn name(&self) -> &str;
    /// Get migration version
    fn version(&self) -> i32;
    /// Get migration dependencies
    fn dependencies(&self) -> Vec<String> {
        Vec::new()
    }
    /// Execute migration
    async fn execute(&self, connection: &mut PgConnection) -> Result<(), MigrationError>;
    /// Rollback migration
    async fn rollback(&self, connection: &mut PgConnection) -> Result<(), MigrationError>;
}

/// Migration errors
#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    #[error("Migration {0} failed: {1}")]
    MigrationFailed(String, String),
    #[error("Dependency not satisfied: {0}")]
    DependencyNotSatisfied(String),
    #[error("Migration already applied: {0}")]
    AlreadyApplied(String),
    #[error("Migration not found: {0}")]
    NotFound(String),
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
}

impl MigrationManager {
    /// Create a new migration manager
    pub async fn new(config: PostgresEventStoreConfig) -> Result<Self, MigrationError> {
        let migration_info = MigrationInfo::new(0, 1); // Current version 0, target version 1
        let applied_migrations = Self::load_applied_migrations().await?;

        Ok(Self {
            config,
            migration_info: Arc::new(RwLock::new(migration_info)),
            applied_migrations: Arc::new(RwLock::new(applied_migrations)),
        })
    }

    /// Load applied migrations from database
    async fn load_applied_migrations(pool: &PgPool) -> Result<HashMap<String, MigrationRecord>, MigrationError> {
        // Create migrations table if it doesn't exist
        query(
            r#"
            CREATE TABLE IF NOT EXISTS schema_migrations (
                name VARCHAR(255) PRIMARY KEY,
                version INTEGER NOT NULL,
                applied_at TIMESTAMP WITH TIME ZONE NOT NULL,
                execution_time_ms BIGINT NOT NULL,
                successful BOOLEAN NOT NULL
            )
            "#
        )
        .execute(pool)
        .await?;

        // Load applied migrations
        let migrations = query(
            r#"
            SELECT name, version, applied_at, execution_time_ms, successful
            FROM schema_migrations
            ORDER BY version ASC
            "#
        )
        .fetch_all(pool)
        .await?;

        let mut result = HashMap::new();
        for row in migrations {
            let record = MigrationRecord {
                name: row.get("name"),
                version: row.get("version"),
                applied_at: row.get("applied_at"),
                execution_time_ms: row.get("execution_time_ms"),
                successful: row.get("successful"),
            };
            result.insert(record.name.clone(), record);
        }

        Ok(result)
    }

    /// Get all available migrations
    fn get_migrations() -> Vec<Box<dyn Migration + Send + Sync>> {
        vec![
            Box::new(V1CreateEventsTableMigration),
            Box::new(V2CreateIndexesMigration),
            Box::new(V3AddEventMetadataMigration),
            Box::new(V4AddCompressionSupportMigration),
            Box::new(V5AddPartitioningMigration),
            Box::new(V6AddEventStatusMigration),
            Box::new(V7OptimizeQueryPerformanceMigration),
            Box::new(V8AddAuditTrailMigration),
        ]
    }

    /// Check if migrations are needed
    pub async fn check_migrations_needed(&self) -> bool {
        let migrations = Self::get_migrations();
        let applied_migrations = self.applied_migrations.read().await;

        for migration in migrations {
            if !applied_migrations.contains_key(migration.name()) {
                return true;
            }
        }

        false
    }

    /// Run all pending migrations
    pub async fn run_migrations(&self) -> Result<(), MigrationError> {
        let mut info = self.migration_info.write().await;
        info.status = MigrationStatus::InProgress;
        drop(info);

        let migrations = Self::get_migrations();
        let pool = self.get_connection_pool().await?;

        // Load applied migrations
        let applied_migrations = self.applied_migrations.read().await;

        // Check dependencies
        for migration in &migrations {
            if applied_migrations.contains_key(migration.name()) {
                continue;
            }

            // Check dependencies
            for dep in migration.dependencies() {
                if !applied_migrations.contains_key(&dep) {
                    return Err(MigrationError::DependencyNotSatisfied(dep));
                }
            }
        }
        drop(applied_migrations);

        // Execute migrations
        for migration in migrations {
            if self.applied_migrations.read().await.contains_key(migration.name()) {
                continue;
            }

            self.execute_migration(&pool, migration).await?;
        }

        // Update migration info
        let mut info = self.migration_info.write().await;
        info.complete_migration();
        drop(info);

        Ok(())
    }

    /// Execute a single migration
    async fn execute_migration(&self, pool: &PgPool, migration: Box<dyn Migration + Send + Sync>) -> Result<(), MigrationError> {
        let start_time = std::time::Instant::now();
        let name = migration.name().to_string();
        let version = migration.version();

        info!("Starting migration: {} (version {})", name, version);

        // Execute migration in transaction
        let mut conn = pool.acquire().await?;
        let tx = conn.begin().await?;

        match migration.execute(&mut conn).await {
            Ok(_) => {
                tx.commit().await?;

                let execution_time_ms = start_time.elapsed().as_millis() as u64;

                // Record migration
                let record = MigrationRecord {
                    name: name.clone(),
                    version,
                    applied_at: chrono::Utc::now(),
                    execution_time_ms,
                    successful: true,
                };

                {
                    let mut applied = self.applied_migrations.write().await;
                    applied.insert(name.clone(), record.clone());
                }

                // Record in database
                query(
                    r#"
                    INSERT INTO schema_migrations (name, version, applied_at, execution_time_ms, successful)
                    VALUES ($1, $2, $3, $4, $5)
                    "#
                )
                .bind(&name)
                .bind(version)
                .bind(record.applied_at)
                .bind(execution_time_ms)
                .bind(true)
                .execute(pool)
                .await?;

                info!("Migration completed successfully in {}ms", execution_time_ms);
                Ok(())
            }
            Err(e) => {
                tx.rollback().await?;
                error!("Migration failed: {}", e);
                Err(MigrationError::MigrationFailed(name, e.to_string()))
            }
        }
    }

    /// Rollback a migration
    pub async fn rollback_migration(&self, name: &str) -> Result<(), MigrationError> {
        let migrations = Self::get_migrations();

        // Find migration
        let migration = migrations
            .into_iter()
            .find(|m| m.name() == name)
            .ok_or_else(|| MigrationError::NotFound(name.to_string()))?;

        // Check if migration is applied
        let applied_migrations = self.applied_migrations.read().await;
        if !applied_migrations.contains_key(name) {
            return Err(MigrationError::AlreadyApplied(name.to_string()));
        }
        drop(applied_migrations);

        let pool = self.get_connection_pool().await?;
        let start_time = std::time::Instant::now();

        info!("Rolling back migration: {}", name);

        // Execute rollback in transaction
        let mut conn = pool.acquire().await?;
        let tx = conn.begin().await?;

        match migration.rollback(&mut conn).await {
            Ok(_) => {
                tx.commit().await?;

                let execution_time_ms = start_time.elapsed().as_millis() as u64;

                // Remove from applied migrations
                {
                    let mut applied = self.applied_migrations.write().await;
                    applied.remove(name);
                }

                // Remove from database
                query("DELETE FROM schema_migrations WHERE name = $1")
                    .bind(name)
                    .execute(pool)
                    .await?;

                info!("Rollback completed successfully in {}ms", execution_time_ms);
                Ok(())
            }
            Err(e) => {
                tx.rollback().await?;
                error!("Rollback failed: {}", e);
                Err(MigrationError::MigrationFailed(name.to_string(), e.to_string()))
            }
        }
    }

    /// Get migration info
    pub async fn get_migration_info(&self) -> MigrationInfo {
        self.migration_info.read().await.clone()
    }

    /// Get applied migrations
    pub async fn get_applied_migrations(&self) -> Vec<MigrationRecord> {
        self.applied_migrations.read().await.values().cloned().collect()
    }

    /// Get migration status
    pub async fn get_migration_status(&self) -> (MigrationStatus, Vec<String>) {
        let info = self.migration_info.read().await;
        let migrations = Self::get_migrations();
        let applied_migrations = self.applied_migrations.read().await;

        let pending_migrations: Vec<String> = migrations
            .into_iter()
            .filter(|m| !applied_migrations.contains_key(m.name()))
            .map(|m| m.name().to_string())
            .collect();

        (info.status.clone(), pending_migrations)
    }

    /// Get connection pool (helper method)
    async fn get_connection_pool(&self) -> Result<PgPool, MigrationError> {
        // This would normally use the connection manager
        // For now, we'll create a temporary pool for migrations
        PgPoolOptions::new()
            .max_connections(5)
            .min_connections(1)
            .connect(&self.config.connection_url)
            .await
            .map_err(MigrationError::DatabaseError)
    }
}

/// V1 Migration: Create events table
struct V1CreateEventsTableMigration;

impl Migration for V1CreateEventsTableMigration {
    fn name(&self) -> &str {
        "create_events_table"
    }

    fn version(&self) -> i32 {
        1
    }

    async fn execute(&self, conn: &mut PgConnection) -> Result<(), MigrationError> {
        query(
            r#"
            CREATE TABLE vm_events (
                id BIGSERIAL PRIMARY KEY,
                sequence_number BIGINT NOT NULL,
                vm_id VARCHAR(255) NOT NULL,
                event_type VARCHAR(255) NOT NULL,
                event_version INTEGER NOT NULL,
                event_data BYTEA NOT NULL,
                metadata JSONB,
                created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
            )
            "#
        )
        .execute(conn)
        .await?;

        query("CREATE INDEX idx_vm_events_vm_id ON vm_events(vm_id)")
            .execute(conn)
            .await?;

        query("CREATE INDEX idx_vm_events_sequence ON vm_events(vm_id, sequence_number)")
            .execute(conn)
            .await?;

        Ok(())
    }

    async fn rollback(&self, conn: &mut PgConnection) -> Result<(), MigrationError> {
        query("DROP TABLE IF EXISTS vm_events")
            .execute(conn)
            .await?;
        Ok(())
    }
}

/// V2 Migration: Create additional indexes
struct V2CreateIndexesMigration;

impl Migration for V2CreateIndexesMigration {
    fn name(&self) -> &str {
        "create_indexes"
    }

    fn version(&self) -> i32 {
        2
    }

    async fn execute(&self, conn: &mut PgConnection) -> Result<(), MigrationError> {
        query("CREATE INDEX idx_vm_events_created_at ON vm_events(created_at)")
            .execute(conn)
            .await?;

        query("CREATE INDEX idx_vm_events_event_type ON vm_events(event_type)")
            .execute(conn)
            .await?;

        Ok(())
    }

    async fn rollback(&self, conn: &mut PgConnection) -> Result<(), MigrationError> {
        query("DROP INDEX idx_vm_events_created_at")
            .execute(conn)
            .await?;
        query("DROP INDEX idx_vm_events_event_type")
            .execute(conn)
            .await?;
        Ok(())
    }
}

/// V3 Migration: Add event metadata
struct V3AddEventMetadataMigration;

impl Migration for V3AddEventMetadataMigration {
    fn name(&self) -> &str {
        "add_event_metadata"
    }

    fn version(&self) -> i32 {
        3
    }

    async fn execute(&self, conn: &mut PgConnection) -> Result<(), MigrationError> {
        query("ALTER TABLE vm_events ADD COLUMN event_size BIGINT")
            .execute(conn)
            .await?;

        query("ALTER TABLE vm_events ADD COLUMN compressed_size BIGINT")
            .execute(conn)
            .await?;

        query("ALTER TABLE vm_events ADD COLUMN checksum VARCHAR(64)")
            .execute(conn)
            .await?;

        query("CREATE INDEX idx_vm_events_checksum ON vm_events(checksum)")
            .execute(conn)
            .await?;

        Ok(())
    }

    async fn rollback(&self, conn: &mut PgConnection) -> Result<(), MigrationError> {
        query("DROP INDEX idx_vm_events_checksum")
            .execute(conn)
            .await?;
        query("ALTER TABLE vm_events DROP COLUMN IF EXISTS checksum")
            .execute(conn)
            .await?;
        query("ALTER TABLE vm_events DROP COLUMN IF EXISTS compressed_size")
            .execute(conn)
            .await?;
        query("ALTER TABLE vm_events DROP COLUMN IF EXISTS event_size")
            .execute(conn)
            .await?;
        Ok(())
    }
}

// Add more migrations as needed...

/// V4 Migration: Add compression support
struct V4AddCompressionSupportMigration;

impl Migration for V4AddCompressionSupportMigration {
    fn name(&self) -> &str {
        "add_compression_support"
    }

    fn version(&self) -> i32 {
        4
    }

    async fn execute(&self, conn: &mut PgConnection) -> Result<(), MigrationError> {
        query("ALTER TABLE vm_events ADD COLUMN compression_method VARCHAR(50)")
            .execute(conn)
            .await?;
        Ok(())
    }

    async fn rollback(&self, conn: &mut PgConnection) -> Result<(), MigrationError> {
        query("ALTER TABLE vm_events DROP COLUMN IF EXISTS compression_method")
            .execute(conn)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_migration_manager() {
        let config = PostgresEventStoreConfig::default();
        let manager = MigrationManager::new(config).await;

        match manager {
            Ok(_) => {
                // Successfully created
            }
            Err(MigrationError::DatabaseError(_)) => {
                // Expected for testing without database
            }
            Err(e) => panic!("Unexpected error: {}", e),
        }
    }

    #[test]
    fn test_migration_versions() {
        let migrations = MigrationManager::get_migrations();
        let versions: Vec<i32> = migrations.iter().map(|m| m.version()).collect();

        // Check versions are unique
        assert_eq!(versions.len(), versions.iter().collect::<std::collections::HashSet<_>>().len());
    }
}