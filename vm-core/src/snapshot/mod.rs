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

/// ============================================================================
/// 测试模块
/// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_metadata_manager_creation() {
        let manager = SnapshotMetadataManager::new();
        assert_eq!(manager.snapshots.len(), 0);
        assert!(manager.current_snapshot.is_none());
    }

    #[test]
    fn test_snapshot_metadata_manager_default() {
        let manager = SnapshotMetadataManager::default();
        assert_eq!(manager.snapshots.len(), 0);
        assert!(manager.current_snapshot.is_none());
    }

    #[test]
    fn test_create_snapshot() {
        let mut manager = SnapshotMetadataManager::new();
        
        let id = manager.create_snapshot(
            "Test Snapshot".to_string(),
            "A test snapshot".to_string(),
            "/path/to/dump".to_string(),
        );
        
        // ID应该被生成
        assert!(!id.is_empty());
        
        // 快照应该被存储
        assert_eq!(manager.snapshots.len(), 1);
        
        // current_snapshot应该被设置
        assert_eq!(manager.current_snapshot, Some(id.clone()));
        
        // 验证快照内容
        let snapshot = manager.get_snapshot(&id).unwrap();
        assert_eq!(snapshot.name, "Test Snapshot");
        assert_eq!(snapshot.description, "A test snapshot");
        assert_eq!(snapshot.memory_dump_path, "/path/to/dump");
        assert_eq!(snapshot.id, id);
        assert!(snapshot.parent_id.is_none()); // 第一个快照没有父节点
    }

    #[test]
    fn test_create_child_snapshot() {
        let mut manager = SnapshotMetadataManager::new();
        
        // 创建父快照
        let parent_id = manager.create_snapshot(
            "Parent".to_string(),
            "Parent snapshot".to_string(),
            "/parent/dump".to_string(),
        );
        
        // 创建子快照
        let child_id = manager.create_snapshot(
            "Child".to_string(),
            "Child snapshot".to_string(),
            "/child/dump".to_string(),
        );
        
        // 验证父-子关系
        let child = manager.get_snapshot(&child_id).unwrap();
        assert_eq!(child.parent_id, Some(parent_id));
        
        // current_snapshot应该指向子快照
        assert_eq!(manager.current_snapshot, Some(child_id));
    }

    #[test]
    fn test_restore_snapshot_exists() {
        let mut manager = SnapshotMetadataManager::new();
        
        let id = manager.create_snapshot(
            "Test".to_string(),
            "Description".to_string(),
            "/dump".to_string(),
        );
        
        // 修改current_snapshot
        manager.current_snapshot = None;
        
        // 恢复快照
        let result = manager.restore_snapshot(&id);
        assert!(result.is_ok());
        assert_eq!(manager.current_snapshot, Some(id));
    }

    #[test]
    fn test_restore_snapshot_not_exists() {
        let mut manager = SnapshotMetadataManager::new();
        
        let result = manager.restore_snapshot("non-existent-id");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Snapshot not found");
    }

    #[test]
    fn test_get_snapshot_exists() {
        let mut manager = SnapshotMetadataManager::new();
        
        let id = manager.create_snapshot(
            "Test".to_string(),
            "Description".to_string(),
            "/dump".to_string(),
        );
        
        let snapshot = manager.get_snapshot(&id);
        assert!(snapshot.is_some());
        assert_eq!(snapshot.unwrap().name, "Test");
    }

    #[test]
    fn test_get_snapshot_not_exists() {
        let manager = SnapshotMetadataManager::new();
        
        let snapshot = manager.get_snapshot("non-existent-id");
        assert!(snapshot.is_none());
    }

    #[test]
    fn test_delete_snapshot_exists() {
        let mut manager = SnapshotMetadataManager::new();
        
        let id = manager.create_snapshot(
            "Test".to_string(),
            "Description".to_string(),
            "/dump".to_string(),
        );
        
        assert_eq!(manager.snapshots.len(), 1);
        
        let result = manager.delete_snapshot(&id);
        assert!(result.is_ok());
        assert_eq!(manager.snapshots.len(), 0);
        assert!(manager.current_snapshot.is_none());
    }

    #[test]
    fn test_delete_snapshot_that_is_current() {
        let mut manager = SnapshotMetadataManager::new();
        
        let id = manager.create_snapshot(
            "Test".to_string(),
            "Description".to_string(),
            "/dump".to_string(),
        );
        
        assert_eq!(manager.current_snapshot, Some(id.clone()));
        
        let result = manager.delete_snapshot(&id);
        assert!(result.is_ok());
        
        // current_snapshot应该被清除
        assert!(manager.current_snapshot.is_none());
    }

    #[test]
    fn test_delete_snapshot_not_exists() {
        let mut manager = SnapshotMetadataManager::new();
        
        let result = manager.delete_snapshot("non-existent-id");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Snapshot not found");
    }

    #[test]
    fn test_get_snapshot_tree_empty() {
        let manager = SnapshotMetadataManager::new();
        let tree = manager.get_snapshot_tree();
        
        assert_eq!(tree.len(), 0);
    }

    #[test]
    fn test_get_snapshot_tree_multiple() {
        let mut manager = SnapshotMetadataManager::new();
        
        // 创建3个快照
        let _id1 = manager.create_snapshot(
            "Snapshot 1".to_string(),
            "Description 1".to_string(),
            "/dump1".to_string(),
        );
        
        let _id2 = manager.create_snapshot(
            "Snapshot 2".to_string(),
            "Description 2".to_string(),
            "/dump2".to_string(),
        );
        
        let _id3 = manager.create_snapshot(
            "Snapshot 3".to_string(),
            "Description 3".to_string(),
            "/dump3".to_string(),
        );
        
        let tree = manager.get_snapshot_tree();
        assert_eq!(tree.len(), 3);
    }

    #[test]
    fn test_snapshot_chain() {
        let mut manager = SnapshotMetadataManager::new();
        
        // 创建快照链: A -> B -> C
        let id_a = manager.create_snapshot("A".to_string(), "Desc A".to_string(), "/a".to_string());
        let id_b = manager.create_snapshot("B".to_string(), "Desc B".to_string(), "/b".to_string());
        let id_c = manager.create_snapshot("C".to_string(), "Desc C".to_string(), "/c".to_string());
        
        // 验证链关系
        let snapshot_b = manager.get_snapshot(&id_b).unwrap();
        assert_eq!(snapshot_b.parent_id, Some(id_a.clone()));
        
        let snapshot_c = manager.get_snapshot(&id_c).unwrap();
        assert_eq!(snapshot_c.parent_id, Some(id_b));
        
        // 第一个快照没有父节点
        let snapshot_a = manager.get_snapshot(&id_a).unwrap();
        assert!(snapshot_a.parent_id.is_none());
    }

    #[test]
    fn test_snapshot_manager_clone() {
        let mut manager = SnapshotMetadataManager::new();
        
        let _id = manager.create_snapshot(
            "Test".to_string(),
            "Description".to_string(),
            "/dump".to_string(),
        );
        
        let cloned = manager.clone();
        assert_eq!(cloned.snapshots.len(), 1);
        assert_eq!(cloned.current_snapshot, manager.current_snapshot);
    }

    #[test]
    fn test_snapshot_serialization() {
        let snapshot = Snapshot {
            id: "test-id".to_string(),
            parent_id: Some("parent-id".to_string()),
            name: "Test Snapshot".to_string(),
            description: "Test Description".to_string(),
            memory_dump_path: "/test/dump".to_string(),
        };
        
        // 测试序列化/反序列化
        let serialized = serde_json::to_string(&snapshot).unwrap();
        let deserialized: Snapshot = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(deserialized.id, snapshot.id);
        assert_eq!(deserialized.name, snapshot.name);
        assert_eq!(deserialized.parent_id, snapshot.parent_id);
    }

    #[test]
    fn test_metadata_manager_serialization() {
        let mut manager = SnapshotMetadataManager::new();
        
        let _id = manager.create_snapshot(
            "Test".to_string(),
            "Description".to_string(),
            "/dump".to_string(),
        );
        
        // 测试序列化/反序列化
        let serialized = serde_json::to_string(&manager).unwrap();
        let deserialized: SnapshotMetadataManager = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(deserialized.snapshots.len(), 1);
    }

    #[test]
    fn test_format_snapshot_id() {
        let id1 = format_snapshot_id();
        let id2 = format_snapshot_id();
        
        // ID应该包含UUID和时间戳
        assert!(id1.contains('-'));
        assert!(id2.contains('-'));
        
        // ID应该是唯一的
        assert_ne!(id1, id2);
    }
}
