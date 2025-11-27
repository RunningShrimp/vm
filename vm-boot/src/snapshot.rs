//! 虚拟机快照功能
//!
//! 支持保存和恢复虚拟机的完整状态

use std::fs::{File, create_dir_all};
use std::io::{Read, Write, BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use vm_core::GuestAddr;

/// 快照错误
#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    #[error("Failed to save snapshot: {0}")]
    SaveFailed(String),
    #[error("Failed to load snapshot: {0}")]
    LoadFailed(String),
    #[error("Snapshot not found: {0}")]
    NotFound(String),
    #[error("Invalid snapshot format: {0}")]
    InvalidFormat(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// 快照元数据
#[derive(Debug, Clone)]
pub struct SnapshotMetadata {
    /// 快照名称
    pub name: String,
    /// 创建时间戳
    pub timestamp: u64,
    /// 虚拟机架构
    pub arch: String,
    /// 内存大小
    pub memory_size: usize,
    /// vCPU 数量
    pub vcpu_count: u32,
    /// 描述
    pub description: Option<String>,
}

impl SnapshotMetadata {
    /// 创建新的快照元数据
    pub fn new(name: impl Into<String>, arch: impl Into<String>, memory_size: usize, vcpu_count: u32) -> Self {
        Self {
            name: name.into(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            arch: arch.into(),
            memory_size,
            vcpu_count,
            description: None,
        }
    }

    /// 设置描述
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// 序列化为 JSON
    pub fn to_json(&self) -> String {
        format!(
            r#"{{"name":"{}","timestamp":{},"arch":"{}","memory_size":{},"vcpu_count":{},"description":{}}}"#,
            self.name,
            self.timestamp,
            self.arch,
            self.memory_size,
            self.vcpu_count,
            self.description.as_ref().map(|s| format!("\"{}\"", s)).unwrap_or_else(|| "null".to_string())
        )
    }

    /// 从 JSON 反序列化（简化实现）
    pub fn from_json(json: &str) -> Result<Self, SnapshotError> {
        // 简化的 JSON 解析
        let json = json.trim_matches(|c| c == '{' || c == '}');
        let mut fields = std::collections::HashMap::new();
        
        for pair in json.split(',') {
            let parts: Vec<&str> = pair.splitn(2, ':').collect();
            if parts.len() == 2 {
                let key = parts[0].trim().trim_matches('"');
                let value = parts[1].trim();
                fields.insert(key, value);
            }
        }

        Ok(Self {
            name: fields.get("name")
                .ok_or_else(|| SnapshotError::InvalidFormat("Missing name".to_string()))?
                .trim_matches('"')
                .to_string(),
            timestamp: fields.get("timestamp")
                .ok_or_else(|| SnapshotError::InvalidFormat("Missing timestamp".to_string()))?
                .parse()
                .map_err(|_| SnapshotError::InvalidFormat("Invalid timestamp".to_string()))?,
            arch: fields.get("arch")
                .ok_or_else(|| SnapshotError::InvalidFormat("Missing arch".to_string()))?
                .trim_matches('"')
                .to_string(),
            memory_size: fields.get("memory_size")
                .ok_or_else(|| SnapshotError::InvalidFormat("Missing memory_size".to_string()))?
                .parse()
                .map_err(|_| SnapshotError::InvalidFormat("Invalid memory_size".to_string()))?,
            vcpu_count: fields.get("vcpu_count")
                .ok_or_else(|| SnapshotError::InvalidFormat("Missing vcpu_count".to_string()))?
                .parse()
                .map_err(|_| SnapshotError::InvalidFormat("Invalid vcpu_count".to_string()))?,
            description: fields.get("description")
                .and_then(|s| {
                    let s = s.trim_matches('"');
                    if s == "null" { None } else { Some(s.to_string()) }
                }),
        })
    }
}

/// vCPU 状态快照
#[derive(Debug, Clone)]
pub struct VcpuSnapshot {
    /// vCPU ID
    pub id: u32,
    /// 程序计数器
    pub pc: u64,
    /// 栈指针
    pub sp: u64,
    /// 通用寄存器
    pub gpr: Vec<u64>,
}

/// 内存快照
pub struct MemorySnapshot {
    /// 内存数据
    pub data: Vec<u8>,
    /// 基地址
    pub base_addr: GuestAddr,
}

/// 完整的虚拟机快照
pub struct VmSnapshot {
    /// 元数据
    pub metadata: SnapshotMetadata,
    /// vCPU 状态
    pub vcpus: Vec<VcpuSnapshot>,
    /// 内存快照
    pub memory: MemorySnapshot,
}

/// 快照管理器
pub struct SnapshotManager {
    /// 快照存储目录
    snapshot_dir: PathBuf,
}

impl SnapshotManager {
    /// 创建新的快照管理器
    pub fn new(snapshot_dir: impl AsRef<Path>) -> Result<Self, SnapshotError> {
        let snapshot_dir = snapshot_dir.as_ref().to_path_buf();
        create_dir_all(&snapshot_dir)?;
        
        Ok(Self { snapshot_dir })
    }

    /// 获取快照路径
    fn snapshot_path(&self, name: &str) -> PathBuf {
        self.snapshot_dir.join(format!("{}.snapshot", name))
    }

    /// 保存快照
    pub fn save(&self, snapshot: &VmSnapshot) -> Result<(), SnapshotError> {
        let path = self.snapshot_path(&snapshot.metadata.name);
        let file = File::create(&path)
            .map_err(|e| SnapshotError::SaveFailed(format!("Failed to create file: {}", e)))?;
        let mut writer = BufWriter::new(file);

        log::info!("Saving snapshot to {:?}", path);

        // 写入魔数
        writer.write_all(b"VMSN")?;

        // 写入版本号
        writer.write_all(&1u32.to_le_bytes())?;

        // 写入元数据
        let metadata_json = snapshot.metadata.to_json();
        writer.write_all(&(metadata_json.len() as u32).to_le_bytes())?;
        writer.write_all(metadata_json.as_bytes())?;

        // 写入 vCPU 数量
        writer.write_all(&(snapshot.vcpus.len() as u32).to_le_bytes())?;

        // 写入每个 vCPU 的状态
        for vcpu in &snapshot.vcpus {
            writer.write_all(&vcpu.id.to_le_bytes())?;
            writer.write_all(&vcpu.pc.to_le_bytes())?;
            writer.write_all(&vcpu.sp.to_le_bytes())?;
            writer.write_all(&(vcpu.gpr.len() as u32).to_le_bytes())?;
            for &reg in &vcpu.gpr {
                writer.write_all(&reg.to_le_bytes())?;
            }
        }

        // 写入内存快照
        writer.write_all(&snapshot.memory.base_addr.to_le_bytes())?;
        writer.write_all(&(snapshot.memory.data.len() as u64).to_le_bytes())?;
        writer.write_all(&snapshot.memory.data)?;

        log::info!("Snapshot saved successfully: {} bytes", 
            std::fs::metadata(&path)?.len());

        Ok(())
    }

    /// 加载快照
    pub fn load(&self, name: &str) -> Result<VmSnapshot, SnapshotError> {
        let path = self.snapshot_path(name);
        if !path.exists() {
            return Err(SnapshotError::NotFound(name.to_string()));
        }

        let file = File::open(&path)
            .map_err(|e| SnapshotError::LoadFailed(format!("Failed to open file: {}", e)))?;
        let mut reader = BufReader::new(file);

        log::info!("Loading snapshot from {:?}", path);

        // 读取魔数
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        if &magic != b"VMSN" {
            return Err(SnapshotError::InvalidFormat("Invalid magic number".to_string()));
        }

        // 读取版本号
        let mut version_bytes = [0u8; 4];
        reader.read_exact(&mut version_bytes)?;
        let version = u32::from_le_bytes(version_bytes);
        if version != 1 {
            return Err(SnapshotError::InvalidFormat(format!("Unsupported version: {}", version)));
        }

        // 读取元数据
        let mut metadata_len_bytes = [0u8; 4];
        reader.read_exact(&mut metadata_len_bytes)?;
        let metadata_len = u32::from_le_bytes(metadata_len_bytes) as usize;
        
        let mut metadata_json = vec![0u8; metadata_len];
        reader.read_exact(&mut metadata_json)?;
        let metadata = SnapshotMetadata::from_json(&String::from_utf8_lossy(&metadata_json))?;

        // 读取 vCPU 数量
        let mut vcpu_count_bytes = [0u8; 4];
        reader.read_exact(&mut vcpu_count_bytes)?;
        let vcpu_count = u32::from_le_bytes(vcpu_count_bytes);

        // 读取每个 vCPU 的状态
        let mut vcpus = Vec::new();
        for _ in 0..vcpu_count {
            let mut id_bytes = [0u8; 4];
            reader.read_exact(&mut id_bytes)?;
            let id = u32::from_le_bytes(id_bytes);

            let mut pc_bytes = [0u8; 8];
            reader.read_exact(&mut pc_bytes)?;
            let pc = u64::from_le_bytes(pc_bytes);

            let mut sp_bytes = [0u8; 8];
            reader.read_exact(&mut sp_bytes)?;
            let sp = u64::from_le_bytes(sp_bytes);

            let mut gpr_count_bytes = [0u8; 4];
            reader.read_exact(&mut gpr_count_bytes)?;
            let gpr_count = u32::from_le_bytes(gpr_count_bytes);

            let mut gpr = Vec::new();
            for _ in 0..gpr_count {
                let mut reg_bytes = [0u8; 8];
                reader.read_exact(&mut reg_bytes)?;
                gpr.push(u64::from_le_bytes(reg_bytes));
            }

            vcpus.push(VcpuSnapshot { id, pc, sp, gpr });
        }

        // 读取内存快照
        let mut base_addr_bytes = [0u8; 8];
        reader.read_exact(&mut base_addr_bytes)?;
        let base_addr = u64::from_le_bytes(base_addr_bytes);

        let mut mem_size_bytes = [0u8; 8];
        reader.read_exact(&mut mem_size_bytes)?;
        let mem_size = u64::from_le_bytes(mem_size_bytes) as usize;

        let mut data = vec![0u8; mem_size];
        reader.read_exact(&mut data)?;

        log::info!("Snapshot loaded successfully");

        Ok(VmSnapshot {
            metadata,
            vcpus,
            memory: MemorySnapshot { data, base_addr },
        })
    }

    /// 列出所有快照
    pub fn list(&self) -> Result<Vec<String>, SnapshotError> {
        let mut snapshots = Vec::new();
        
        for entry in std::fs::read_dir(&self.snapshot_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("snapshot") {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    snapshots.push(name.to_string());
                }
            }
        }

        Ok(snapshots)
    }

    /// 删除快照
    pub fn delete(&self, name: &str) -> Result<(), SnapshotError> {
        let path = self.snapshot_path(name);
        if !path.exists() {
            return Err(SnapshotError::NotFound(name.to_string()));
        }

        std::fs::remove_file(&path)?;
        log::info!("Snapshot deleted: {}", name);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_metadata() {
        let metadata = SnapshotMetadata::new("test", "riscv64", 1024 * 1024 * 1024, 2)
            .with_description("Test snapshot");
        
        let json = metadata.to_json();
        let loaded = SnapshotMetadata::from_json(&json).expect("Failed to deserialize snapshot metadata");
        
        assert_eq!(loaded.name, "test");
        assert_eq!(loaded.arch, "riscv64");
        assert_eq!(loaded.memory_size, 1024 * 1024 * 1024);
        assert_eq!(loaded.vcpu_count, 2);
    }

    #[test]
    fn test_snapshot_manager() {
        let temp_dir = std::env::temp_dir().join("vm_snapshots_test");
        let manager = SnapshotManager::new(&temp_dir).expect("Failed to create snapshot manager");
        
        let metadata = SnapshotMetadata::new("test", "riscv64", 1024, 1);
        let vcpu = VcpuSnapshot {
            id: 0,
            pc: 0x80000000,
            sp: 0x80100000,
            gpr: vec![0; 32],
        };
        let memory = MemorySnapshot {
            data: vec![0; 1024],
            base_addr: 0x80000000,
        };
        
        let snapshot = VmSnapshot {
            metadata,
            vcpus: vec![vcpu],
            memory,
        };
        
        manager.save(&snapshot).expect("Failed to save snapshot");
        let loaded = manager.load("test").expect("Failed to load snapshot");
        
        assert_eq!(loaded.metadata.name, "test");
        assert_eq!(loaded.vcpus.len(), 1);
        assert_eq!(loaded.memory.data.len(), 1024);
        
        // 清理
        manager.delete("test").expect("Failed to delete snapshot");
        std::fs::remove_dir_all(&temp_dir).ok();
    }
}
