//! 性能数据存储

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde_json;
use std::collections::HashMap;
use std::sync::Mutex;

use super::collector::{PerformanceMetrics, TestContext};

/// 性能数据存储接口
pub trait PerformanceStorage: Send + Sync {
    /// 存储性能指标
    fn store_metrics(&self, metrics: &PerformanceMetrics) -> Result<()>;

    /// 获取指定测试的历史数据
    fn get_history(
        &self,
        context: &TestContext,
        limit: Option<usize>,
    ) -> Result<Vec<PerformanceMetrics>>;

    /// 获取指定时间范围内的数据
    fn get_metrics_in_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<PerformanceMetrics>>;

    /// 获取最新的性能指标
    fn get_latest_metrics(&self, context: &TestContext) -> Result<Option<PerformanceMetrics>>;

    /// 清理旧数据
    fn cleanup_old_data(&self, days: u32) -> Result<usize>;
}

/// SQLite性能数据存储实现
pub struct SqlitePerformanceStorage {
    connection: Mutex<Connection>,
}

impl SqlitePerformanceStorage {
    /// 创建新的SQLite存储
    pub fn new(database_path: &str) -> Result<Self> {
        let connection = Connection::open(database_path)?;

        // 启用外键约束
        connection.execute("PRAGMA foreign_keys = ON", [])?;

        // 创建表
        Self::create_tables(&connection)?;

        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// 创建数据库表
    fn create_tables(conn: &Connection) -> Result<()> {
        // 测试上下文表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS test_contexts (
                id INTEGER PRIMARY KEY,
                src_arch TEXT NOT NULL,
                dst_arch TEXT NOT NULL,
                test_name TEXT NOT NULL,
                version TEXT NOT NULL,
                cpu_cores INTEGER NOT NULL,
                memory_mb INTEGER NOT NULL,
                os TEXT NOT NULL,
                rustc_version TEXT NOT NULL,
                opt_level TEXT NOT NULL,
                UNIQUE(src_arch, dst_arch, test_name, version)
            )",
            [],
        )?;

        // 性能指标表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS performance_metrics (
                id INTEGER PRIMARY KEY,
                context_id INTEGER NOT NULL,
                timestamp TEXT NOT NULL,
                execution_time_us INTEGER NOT NULL,
                jit_compilation_time_us INTEGER NOT NULL,
                memory_usage_bytes INTEGER NOT NULL,
                instructions_translated INTEGER NOT NULL,
                instruction_throughput REAL NOT NULL,
                cache_hit_rate REAL NOT NULL,
                custom_metrics TEXT,
                FOREIGN KEY(context_id) REFERENCES test_contexts(id)
            )",
            [],
        )?;

        // 创建索引
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_metrics_timestamp ON performance_metrics(timestamp)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_metrics_context ON performance_metrics(context_id)",
            [],
        )?;

        Ok(())
    }

    /// 获取或创建测试上下文ID
    fn get_or_create_context_id(&self, context: &TestContext) -> Result<i64> {
        // 尝试获取现有上下文
        let conn = self
            .connection
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to acquire connection lock: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id FROM test_contexts
             WHERE src_arch = ? AND dst_arch = ? AND test_name = ? AND version = ?",
        )?;

        let context_id = stmt
            .query_row(
                params![
                    format!("{:?}", context.src_arch),
                    format!("{:?}", context.dst_arch),
                    context.test_name,
                    context.version
                ],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(id) = context_id {
            return Ok(id);
        }

        // 创建新上下文
        conn.execute(
            "INSERT INTO test_contexts 
             (src_arch, dst_arch, test_name, version, cpu_cores, memory_mb, os, rustc_version, opt_level)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                format!("{:?}", context.src_arch),
                format!("{:?}", context.dst_arch),
                context.test_name,
                context.version,
                context.environment.cpu_cores,
                context.environment.memory_mb,
                context.environment.os,
                context.environment.rustc_version,
                context.environment.opt_level,
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }
}

impl PerformanceStorage for SqlitePerformanceStorage {
    fn store_metrics(&self, metrics: &PerformanceMetrics) -> Result<()> {
        let context_id = self.get_or_create_context_id(&metrics.context)?;

        let custom_metrics_json = serde_json::to_string(&metrics.custom_metrics)?;

        let conn = self
            .connection
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to acquire connection lock: {}", e))?;
        conn.execute(
            "INSERT INTO performance_metrics
             (context_id, timestamp, execution_time_us, jit_compilation_time_us,
              memory_usage_bytes, instructions_translated, instruction_throughput,
              cache_hit_rate, custom_metrics)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                context_id,
                metrics.timestamp.to_rfc3339(),
                metrics.execution_time_us,
                metrics.jit_compilation_time_us,
                metrics.memory_usage_bytes,
                metrics.instructions_translated,
                metrics.instruction_throughput,
                metrics.cache_hit_rate,
                custom_metrics_json,
            ],
        )?;

        Ok(())
    }

    fn get_history(
        &self,
        context: &TestContext,
        limit: Option<usize>,
    ) -> Result<Vec<PerformanceMetrics>> {
        let conn = self
            .connection
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to acquire connection lock: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT m.*, c.src_arch, c.dst_arch, c.test_name, c.version,
                   c.cpu_cores, c.memory_mb, c.os, c.rustc_version, c.opt_level
             FROM performance_metrics m
             JOIN test_contexts c ON m.context_id = c.id
             WHERE c.src_arch = ? AND c.dst_arch = ? AND c.test_name = ? AND c.version = ?
             ORDER BY m.timestamp DESC
             LIMIT ?",
        )?;

        let limit = limit.unwrap_or(100) as i64;

        let rows = stmt.query_map(
            params![
                format!("{:?}", context.src_arch),
                format!("{:?}", context.dst_arch),
                context.test_name,
                context.version,
                limit,
            ],
            |row| self.row_to_metrics(row),
        )?;

        let mut metrics = Vec::new();
        for row in rows {
            metrics.push(row?);
        }

        Ok(metrics)
    }

    fn get_metrics_in_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<PerformanceMetrics>> {
        let conn = self
            .connection
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to acquire connection lock: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT m.*, c.src_arch, c.dst_arch, c.test_name, c.version,
                   c.cpu_cores, c.memory_mb, c.os, c.rustc_version, c.opt_level
             FROM performance_metrics m
             JOIN test_contexts c ON m.context_id = c.id
             WHERE m.timestamp BETWEEN ? AND ?
             ORDER BY m.timestamp",
        )?;
        let rows = stmt.query_map(params![start.to_rfc3339(), end.to_rfc3339()], |row| {
            self.row_to_metrics(row)
        })?;

        let mut metrics = Vec::new();
        for row in rows {
            metrics.push(row?);
        }

        Ok(metrics)
    }

    fn get_latest_metrics(&self, context: &TestContext) -> Result<Option<PerformanceMetrics>> {
        let conn = self
            .connection
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to acquire connection lock: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT m.*, c.src_arch, c.dst_arch, c.test_name, c.version,
                   c.cpu_cores, c.memory_mb, c.os, c.rustc_version, c.opt_level
             FROM performance_metrics m
             JOIN test_contexts c ON m.context_id = c.id
             WHERE c.src_arch = ? AND c.dst_arch = ? AND c.test_name = ? AND c.version = ?
             ORDER BY m.timestamp DESC
             LIMIT 1",
        )?;
        let row = stmt
            .query_row(
                params![
                    format!("{:?}", context.src_arch),
                    format!("{:?}", context.dst_arch),
                    context.test_name,
                    context.version,
                ],
                |row| self.row_to_metrics(row),
            )
            .optional()?;

        Ok(row)
    }

    fn cleanup_old_data(&self, days: u32) -> Result<usize> {
        let cutoff_date = Utc::now() - chrono::Duration::days(days as i64);

        let conn = self
            .connection
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to acquire connection lock: {}", e))?;
        let rows_deleted = conn.execute(
            "DELETE FROM performance_metrics WHERE timestamp < ?",
            params![cutoff_date.to_rfc3339()],
        )?;

        Ok(rows_deleted)
    }
}

impl SqlitePerformanceStorage {
    /// 将数据库行转换为性能指标
    fn row_to_metrics(&self, row: &Row) -> rusqlite::Result<PerformanceMetrics> {
        let custom_metrics_json: String = row.get(9)?;
        let custom_metrics: HashMap<String, f64> = serde_json::from_str(&custom_metrics_json)
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;

        // 重建测试上下文
        let context = TestContext {
            src_arch: Self::parse_arch(&row.get::<_, String>(10)?),
            dst_arch: Self::parse_arch(&row.get::<_, String>(11)?),
            test_name: row.get(12)?,
            version: row.get(13)?,
            environment: super::collector::EnvironmentInfo {
                cpu_cores: row.get(14)?,
                memory_mb: row.get(15)?,
                os: row.get(16)?,
                rustc_version: row.get(17)?,
                opt_level: row.get(18)?,
            },
        };

        Ok(PerformanceMetrics {
            context,
            execution_time_us: row.get(2)?,
            jit_compilation_time_us: row.get(3)?,
            memory_usage_bytes: row.get(4)?,
            instructions_translated: row.get(5)?,
            instruction_throughput: row.get(6)?,
            cache_hit_rate: row.get(7)?,
            custom_metrics,
            timestamp: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(1)?)
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?
                .with_timezone(&Utc),
        })
    }

    /// 解析架构字符串
    fn parse_arch(arch_str: &str) -> vm_core::GuestArch {
        match arch_str {
            "X86_64" => vm_core::GuestArch::X86_64,
            "ARM64" => vm_core::GuestArch::Arm64,
            "RISCV64" => vm_core::GuestArch::Riscv64,
            _ => vm_core::GuestArch::X86_64, // 默认值
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use vm_core::GuestArch;
    use vm_engine::jit::collector::PerformanceCollector;

    #[test]
    fn test_sqlite_storage() -> Result<()> {
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("test.db");
        let db_path_str = db_path.to_str().ok_or_else(|| {
            anyhow::anyhow!("Invalid database path: contains non-UTF-8 characters")
        })?;
        let storage = SqlitePerformanceStorage::new(db_path_str)?;

        // 创建测试指标
        let context = TestContext {
            src_arch: GuestArch::X86_64,
            dst_arch: GuestArch::Arm64,
            test_name: "test".to_string(),
            version: "1.0.0".to_string(),
            environment: PerformanceCollector::collect_environment_info(),
        };

        let metrics = PerformanceMetrics {
            context: context.clone(),
            execution_time_us: 1000,
            jit_compilation_time_us: 500,
            memory_usage_bytes: 1024 * 1024,
            instructions_translated: 1000,
            instruction_throughput: 1000000.0,
            cache_hit_rate: 0.95,
            custom_metrics: HashMap::new(),
            timestamp: Utc::now(),
        };

        // 存储指标
        storage.store_metrics(&metrics)?;

        // 获取历史数据
        let history = storage.get_history(&context, Some(10))?;
        assert_eq!(history.len(), 1);

        // 获取最新指标
        let latest = storage.get_latest_metrics(&context)?;
        assert!(latest.is_some());

        Ok(())
    }
}
