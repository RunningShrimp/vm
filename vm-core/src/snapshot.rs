use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Snapshot {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub description: String,
    pub memory_dump_path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SnapshotManager {
    pub snapshots: HashMap<String, Snapshot>,
    pub current_snapshot: Option<String>,
}

impl SnapshotManager {
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
}
