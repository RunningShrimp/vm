//! 增量快照功能
//!
//! 实现虚拟机的增量快照和恢复

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};

/// 快照元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    /// 快照 ID
    pub id: String,
    /// 父快照 ID（用于增量快照）
    pub parent_id: Option<String>,
    /// 创建时间
    pub created_at: u64,
    /// 快照描述
    pub description: String,
    /// 内存大小
    pub memory_size: u64,
    /// 脏页数量
    pub dirty_pages: usize,
}

/// 内存页状态
#[derive(Debug, Clone, Copy, PartialEq)]
enum PageState {
    Clean,
    Dirty,
}

/// 增量快照管理器
pub struct IncrementalSnapshotManager {
    /// 快照存储目录
    snapshot_dir: PathBuf,
    /// 快照元数据
    snapshots: HashMap<String, SnapshotMetadata>,
    /// 脏页跟踪
    dirty_pages: HashMap<u64, PageState>,
    /// 页大小（默认 4KB）
    page_size: usize,
}

impl IncrementalSnapshotManager {
    pub fn new<P: AsRef<Path>>(snapshot_dir: P) -> io::Result<Self> {
        let snapshot_dir = snapshot_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&snapshot_dir)?;
        
        Ok(Self {
            snapshot_dir,
            snapshots: HashMap::new(),
            dirty_pages: HashMap::new(),
            page_size: 4096,
        })
    }

    /// 标记页为脏页
    pub fn mark_dirty(&mut self, page_addr: u64) {
        self.dirty_pages.insert(page_addr, PageState::Dirty);
    }

    /// 创建增量快照
    pub fn create_snapshot(
        &mut self,
        snapshot_id: String,
        parent_id: Option<String>,
        description: String,
        memory: &[u8],
    ) -> io::Result<()> {
        let metadata = SnapshotMetadata {
            id: snapshot_id.clone(),
            parent_id: parent_id.clone(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            description,
            memory_size: memory.len() as u64,
            dirty_pages: self.dirty_pages.len(),
        };

        // 保存元数据
        let metadata_path = self.snapshot_dir.join(format!("{}.meta", snapshot_id));
        let metadata_json = serde_json::to_string_pretty(&metadata)?;
        std::fs::write(metadata_path, metadata_json)?;

        // 保存内存快照
        if parent_id.is_none() {
            // 全量快照
            self.save_full_snapshot(&snapshot_id, memory)?;
        } else {
            // 增量快照：只保存脏页
            self.save_incremental_snapshot(&snapshot_id, memory)?;
        }

        // 清除脏页标记
        self.dirty_pages.clear();
        
        self.snapshots.insert(snapshot_id, metadata);
        Ok(())
    }

    /// 保存全量快照
    fn save_full_snapshot(&self, snapshot_id: &str, memory: &[u8]) -> io::Result<()> {
        let snapshot_path = self.snapshot_dir.join(format!("{}.snap", snapshot_id));
        let mut file = File::create(snapshot_path)?;
        file.write_all(memory)?;
        Ok(())
    }

    /// 保存增量快照
    fn save_incremental_snapshot(&self, snapshot_id: &str, memory: &[u8]) -> io::Result<()> {
        let snapshot_path = self.snapshot_dir.join(format!("{}.snap", snapshot_id));
        let mut file = File::create(snapshot_path)?;

        // 写入脏页数量
        file.write_all(&(self.dirty_pages.len() as u64).to_le_bytes())?;

        // 写入每个脏页
        for (page_addr, _) in &self.dirty_pages {
            let page_offset = *page_addr as usize;
            if page_offset + self.page_size <= memory.len() {
                // 写入页地址
                file.write_all(&page_addr.to_le_bytes())?;
                // 写入页数据
                file.write_all(&memory[page_offset..page_offset + self.page_size])?;
            }
        }

        Ok(())
    }

    /// 恢复快照
    pub fn restore_snapshot(&self, snapshot_id: &str, memory: &mut [u8]) -> io::Result<()> {
        // 加载元数据
        let metadata_path = self.snapshot_dir.join(format!("{}.meta", snapshot_id));
        let metadata_json = std::fs::read_to_string(metadata_path)?;
        let metadata: SnapshotMetadata = serde_json::from_str(&metadata_json)?;

        // 如果有父快照，先恢复父快照
        if let Some(parent_id) = &metadata.parent_id {
            self.restore_snapshot(parent_id, memory)?;
        }

        // 恢复当前快照
        let snapshot_path = self.snapshot_dir.join(format!("{}.snap", snapshot_id));
        let mut file = File::open(snapshot_path)?;

        if metadata.parent_id.is_none() {
            // 全量快照：直接读取
            file.read_exact(memory)?;
        } else {
            // 增量快照：读取脏页
            let mut dirty_count_bytes = [0u8; 8];
            file.read_exact(&mut dirty_count_bytes)?;
            let dirty_count = u64::from_le_bytes(dirty_count_bytes) as usize;

            for _ in 0..dirty_count {
                let mut page_addr_bytes = [0u8; 8];
                file.read_exact(&mut page_addr_bytes)?;
                let page_addr = u64::from_le_bytes(page_addr_bytes) as usize;

                if page_addr + self.page_size <= memory.len() {
                    file.read_exact(&mut memory[page_addr..page_addr + self.page_size])?;
                }
            }
        }

        Ok(())
    }

    /// 列出所有快照
    pub fn list_snapshots(&self) -> Vec<&SnapshotMetadata> {
        self.snapshots.values().collect()
    }

    /// 删除快照
    pub fn delete_snapshot(&mut self, snapshot_id: &str) -> io::Result<()> {
        // 删除元数据文件
        let metadata_path = self.snapshot_dir.join(format!("{}.meta", snapshot_id));
        if metadata_path.exists() {
            std::fs::remove_file(metadata_path)?;
        }

        // 删除快照文件
        let snapshot_path = self.snapshot_dir.join(format!("{}.snap", snapshot_id));
        if snapshot_path.exists() {
            std::fs::remove_file(snapshot_path)?;
        }

        self.snapshots.remove(snapshot_id);
        Ok(())
    }

    /// 合并快照链
    pub fn merge_snapshots(&self, snapshot_ids: &[String], output_id: &str) -> io::Result<()> {
        // 创建一个临时内存缓冲区
        let mut memory = vec![0u8; 128 * 1024 * 1024]; // 假设 128MB

        // 依次恢复每个快照
        for snapshot_id in snapshot_ids {
            self.restore_snapshot(snapshot_id, &mut memory)?;
        }

        // 保存合并后的快照
        let snapshot_path = self.snapshot_dir.join(format!("{}.snap", output_id));
        let mut file = File::create(snapshot_path)?;
        file.write_all(&memory)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_incremental_snapshot() {
        let unique = format!(
            "vm_snapshots_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0)
        );
        let temp_dir = env::temp_dir().join(unique);
        let mut manager = IncrementalSnapshotManager::new(&temp_dir)
            .expect("Failed to create incremental snapshot manager");

        // 创建初始内存
        let mut memory = vec![0u8; 8192]; // 2 pages
        memory[0] = 0xAA;
        memory[4096] = 0xBB;

        // 创建全量快照
        manager.create_snapshot(
            "snap1".to_string(),
            None,
            "Initial snapshot".to_string(),
            &memory,
        ).expect("Failed to create full snapshot");

        // 修改内存并标记脏页
        memory[0] = 0xCC;
        manager.mark_dirty(0);

        // 创建增量快照
        manager.create_snapshot(
            "snap2".to_string(),
            Some("snap1".to_string()),
            "Incremental snapshot".to_string(),
            &memory,
        ).expect("Failed to create incremental snapshot");

        // 恢复快照
        let mut restored_memory = vec![0u8; 8192];
        manager.restore_snapshot("snap2", &mut restored_memory)
            .expect("Failed to restore incremental snapshot");

        assert_eq!(restored_memory[0], 0xCC);
        assert_eq!(restored_memory[4096], 0xBB);

        // 清理
        std::fs::remove_dir_all(temp_dir).ok();
    }
}
