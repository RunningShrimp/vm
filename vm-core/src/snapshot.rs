use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

/// 向后兼容的类型别名
#[deprecated(since = "0.2.0", note = "Use SnapshotMetadataManager instead")]
pub type SnapshotManager = SnapshotMetadataManager;

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
        let id = uuid::Uuid::new_v4().to_string();
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
    
    /// 获取指定快照
    pub fn get_snapshot(&self, id: &str) -> Option<&Snapshot> {
        self.snapshots.get(id)
    }
    
    /// 删除快照
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
