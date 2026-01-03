//! Snapshot module
//!
//! This module provides comprehensive snapshot management for VM system,
//! including metadata management, data persistence, and snapshot-based replay optimization.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub mod base;

/// 快照基本信息
///
/// 表示快照的元数据，包括 ID、名称、父快照等。
/// 这是快照的轻量级表示，不包含实际数据。
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Snapshot {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub description: String,
    pub memory_dump_path: String,
}

/// 快照元数据管理器
///
/// 管理快照的元数据和快照树结构。
/// 注意：这个管理器只管理元数据，不处理实际的快照文件 I/O。
/// 对于完整的快照保存/加载功能，请使用 `vm-boot` 中的 `SnapshotFileManager`。
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SnapshotMetadataManager {
    pub snapshots: HashMap<String, Snapshot>,
    pub current_snapshot: Option<String>,
}

impl SnapshotMetadataManager {
    pub fn new() -> Self {
        Self {
            snapshots: HashMap::new(),
            current_snapshot: None,
        }
    }

    pub fn create_snapshot(
        &mut self,
        name: String,
        description: String,
        memory_dump_path: String,
    ) -> String {
        let id = format_snapshot_id();
        let parent_id = self.current_snapshot.clone();
        let snapshot = Snapshot {
            id: id.clone(),
            parent_id,
            name,
            description,
            memory_dump_path,
        };
        self.snapshots.insert(id.clone(), snapshot);
        self.current_snapshot = Some(id.clone());
        id
    }

    pub fn restore_snapshot(&mut self, id: &str) -> Result<(), String> {
        if self.snapshots.contains_key(id) {
            self.current_snapshot = Some(id.to_string());
            Ok(())
        } else {
            Err("Snapshot not found".to_string())
        }
    }

    pub fn get_snapshot_tree(&self) -> Vec<&Snapshot> {
        self.snapshots.values().collect()
    }

    pub fn get_snapshot(&self, id: &str) -> Option<&Snapshot> {
        self.snapshots.get(id)
    }

    pub fn delete_snapshot(&mut self, id: &str) -> Result<(), String> {
        if self.snapshots.remove(id).is_some() {
            if self.current_snapshot.as_ref() == Some(&id.to_string()) {
                self.current_snapshot = None;
            }
            Ok(())
        } else {
            Err("Snapshot not found".to_string())
        }
    }
}

impl Default for SnapshotMetadataManager {
    fn default() -> Self {
        Self::new()
    }
}

// Re-export base snapshot functionality
pub use base::{
    BaseSnapshot, MemorySnapshot, MemoryState, SnapshotError, SnapshotFileManager,
    SnapshotMetadata, VcpuSnapshot, VirtualMachineState, VmSnapshot,
};

/// Generate snapshot ID
fn format_snapshot_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{}-{}", uuid::Uuid::new_v4(), timestamp)
}
