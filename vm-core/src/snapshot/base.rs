//! 虚拟机快照功能 - 基础实现
//!
//! 支持保存和恢复虚拟机的完整状态，包括增量快照和压缩功能

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::{File, create_dir_all};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::GuestAddr;

/// 快照错误
#[derive(Debug)]
pub enum SnapshotError {
    SaveFailed(String),
    LoadFailed(String),
    NotFound(String),
    InvalidFormat(String),
    Io(std::io::Error),
    CompressionError(String),
    MissingBaseSnapshot,
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SnapshotError::SaveFailed(msg) => write!(f, "Failed to save snapshot: {}", msg),
            SnapshotError::LoadFailed(msg) => write!(f, "Failed to load snapshot: {}", msg),
            SnapshotError::NotFound(name) => write!(f, "Snapshot not found: {}", name),
            SnapshotError::InvalidFormat(msg) => write!(f, "Invalid snapshot format: {}", msg),
            SnapshotError::Io(err) => write!(f, "I/O error: {}", err),
            SnapshotError::CompressionError(msg) => write!(f, "Compression error: {}", msg),
            SnapshotError::MissingBaseSnapshot => {
                write!(f, "Incremental snapshot requires base snapshot")
            }
        }
    }
}

impl From<std::io::Error> for SnapshotError {
    fn from(err: std::io::Error) -> Self {
        SnapshotError::Io(err)
    }
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
    pub fn new(
        name: impl Into<String>,
        arch: impl Into<String>,
        memory_size: usize,
        vcpu_count: u32,
    ) -> Self {
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
            self.description
                .as_ref()
                .map(|s| format!("\"{}\"", s))
                .unwrap_or_else(|| "null".to_string())
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
            name: fields
                .get("name")
                .ok_or_else(|| SnapshotError::InvalidFormat("Missing name".to_string()))?
                .trim_matches('"')
                .to_string(),
            timestamp: fields
                .get("timestamp")
                .ok_or_else(|| SnapshotError::InvalidFormat("Missing timestamp".to_string()))?
                .parse()
                .map_err(|_| SnapshotError::InvalidFormat("Invalid timestamp".to_string()))?,
            arch: fields
                .get("arch")
                .ok_or_else(|| SnapshotError::InvalidFormat("Missing arch".to_string()))?
                .trim_matches('"')
                .to_string(),
            memory_size: fields
                .get("memory_size")
                .ok_or_else(|| SnapshotError::InvalidFormat("Missing memory_size".to_string()))?
                .parse()
                .map_err(|_| SnapshotError::InvalidFormat("Invalid memory_size".to_string()))?,
            vcpu_count: fields
                .get("vcpu_count")
                .ok_or_else(|| SnapshotError::InvalidFormat("Missing vcpu_count".to_string()))?
                .parse()
                .map_err(|_| SnapshotError::InvalidFormat("Invalid vcpu_count".to_string()))?,
            description: fields.get("description").and_then(|s| {
                let s = s.trim_matches('"');
                if s == "null" {
                    None
                } else {
                    Some(s.to_string())
                }
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
#[derive(Debug, Clone)]
pub struct MemorySnapshot {
    /// 内存数据
    pub data: Vec<u8>,
    /// 基地址
    pub base_addr: GuestAddr,
}

/// 内存状态
#[derive(Debug, Clone)]
pub struct MemoryState {
    /// 内存数据
    pub data: Vec<u8>,
    /// 基地址
    pub base_addr: GuestAddr,
}

/// 增强的快照结构
///
/// 支持增量快照、脏页跟踪和压缩功能
#[derive(Debug, Clone)]
pub struct BaseSnapshot {
    /// 快照唯一ID
    pub id: String,
    /// 元数据
    pub metadata: SnapshotMetadata,
    /// 内存状态
    pub memory_state: MemoryState,
    /// 设备状态（使用 JSON 支持灵活的序列化）
    pub device_states: HashMap<String, serde_json::Value>,
    /// 脏页集合（用于增量快照）
    pub dirty_pages: HashSet<GuestAddr>,
    /// 是否启用压缩
    pub compression_enabled: bool,
    /// 父快照ID（用于增量快照链）
    pub parent_id: Option<String>,
}

impl BaseSnapshot {
    /// 创建新的快照
    pub fn new(id: String, metadata: SnapshotMetadata, memory_state: MemoryState) -> Self {
        Self {
            id,
            metadata,
            memory_state,
            device_states: HashMap::new(),
            dirty_pages: HashSet::new(),
            compression_enabled: false,
            parent_id: None,
        }
    }

    /// 创建增量快照（基于脏页跟踪）
    ///
    /// 只保存与基础快照相比发生变化的页
    pub fn create_incremental<B>(
        vm_state: &VirtualMachineState<B>,
        base_snapshot: &BaseSnapshot,
    ) -> Result<Self, SnapshotError>
    where
        B: std::ops::Deref<Target = [u8]>,
    {
        // 跟踪修改的页
        let dirty_pages = vm_state.track_dirty_pages(&base_snapshot.memory_state);

        // 只包含脏页的内存数据
        let incremental_memory =
            Self::extract_dirty_pages(&vm_state.memory, &dirty_pages, vm_state.page_size);

        let id = format_snapshot_id();
        let mut metadata = base_snapshot.metadata.clone();
        metadata.timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Ok(BaseSnapshot {
            id,
            metadata,
            memory_state: incremental_memory,
            device_states: vm_state.device_states.clone(),
            dirty_pages,
            compression_enabled: base_snapshot.compression_enabled,
            parent_id: Some(base_snapshot.id.clone()),
        })
    }

    /// 提取脏页数据
    fn extract_dirty_pages(
        memory: &[u8],
        dirty_pages: &HashSet<GuestAddr>,
        page_size: usize,
    ) -> MemoryState {
        let mut incremental_data = Vec::new();

        for &page_addr in dirty_pages {
            let start = page_addr.0 as usize;
            let end = start + page_size;
            if end <= memory.len() {
                incremental_data.extend_from_slice(&memory[start..end]);
            }
        }

        MemoryState {
            data: incremental_data,
            base_addr: GuestAddr(0), // 增量数据使用相对地址
        }
    }

    /// 压缩快照数据
    ///
    /// 使用 miniz_oxide 进行压缩，减少存储空间
    pub fn compress(&mut self) -> Result<(), SnapshotError> {
        if !self.compression_enabled {
            return Ok(());
        }

        use miniz_oxide::deflate::compress_to_vec_zlib;

        // 压缩内存数据
        let compressed = compress_to_vec_zlib(&self.memory_state.data, 6);
        self.memory_state.data = compressed;
        Ok(())
    }

    /// 解压快照数据
    pub fn decompress(&mut self) -> Result<(), SnapshotError> {
        if !self.compression_enabled {
            return Ok(());
        }

        use miniz_oxide::inflate::decompress_to_vec_zlib;

        let decompressed = decompress_to_vec_zlib(&self.memory_state.data).map_err(|e| {
            SnapshotError::CompressionError(format!("Decompression failed: {:?}", e))
        })?;

        self.memory_state.data = decompressed;
        Ok(())
    }

    /// 获取快照大小（字节）
    pub fn size(&self) -> usize {
        self.memory_state.data.len()
    }

    /// 是否为增量快照
    pub fn is_incremental(&self) -> bool {
        self.parent_id.is_some()
    }

    /// 启用压缩
    pub fn with_compression(mut self) -> Self {
        self.compression_enabled = true;
        self
    }
}

/// 虚拟机状态（通用表示）
///
/// 用于快照创建和恢复
pub struct VirtualMachineState<B> {
    /// 内存数据
    pub memory: B,
    /// 页大小
    pub page_size: usize,
    /// 设备状态
    pub device_states: HashMap<String, serde_json::Value>,
}

impl<B: std::ops::Deref<Target = [u8]>> VirtualMachineState<B> {
    /// 跟踪脏页（简化实现）
    fn track_dirty_pages(&self, base_memory: &MemoryState) -> HashSet<GuestAddr> {
        let mut dirty_pages = HashSet::new();
        let page_size = self.page_size;

        // 比较内存页
        let min_len = std::cmp::min(self.memory.len(), base_memory.data.len());

        for page_start in (0..min_len).step_by(page_size) {
            let page_end = std::cmp::min(page_start + page_size, min_len);

            // 比较这一页
            let base_page = &base_memory.data[page_start..page_end];
            let current_page = &self.memory[page_start..page_end];

            if base_page != current_page {
                dirty_pages.insert(GuestAddr(page_start as u64));
            }
        }

        dirty_pages
    }
}

/// 生成快照ID
fn format_snapshot_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{}-{}", uuid::Uuid::new_v4(), timestamp)
}

/// 完整的虚拟机快照（向后兼容）
#[derive(Debug, Clone)]
pub struct VmSnapshot {
    /// 元数据
    pub metadata: SnapshotMetadata,
    /// vCPU 状态
    pub vcpus: Vec<VcpuSnapshot>,
    /// 内存快照
    pub memory: MemorySnapshot,
}

impl From<BaseSnapshot> for VmSnapshot {
    fn from(snapshot: BaseSnapshot) -> Self {
        Self {
            metadata: snapshot.metadata,
            vcpus: Vec::new(), // 需要从 device_states 提取
            memory: MemorySnapshot {
                data: snapshot.memory_state.data,
                base_addr: snapshot.memory_state.base_addr,
            },
        }
    }
}

/// 快照文件管理器
///
/// 管理虚拟机快照的文件 I/O 操作，包括保存和加载完整的虚拟机状态。
///
/// 注意：这与 `vm-core` 中的 `SnapshotMetadataManager` 不同：
/// - `SnapshotMetadataManager`: 管理快照的元数据和快照树结构
/// - `SnapshotFileManager`: 管理快照文件的读写
pub struct SnapshotFileManager {
    /// 快照存储目录
    snapshot_dir: PathBuf,
}

impl SnapshotFileManager {
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
        writer.write_all(&snapshot.memory.base_addr.0.to_le_bytes())?;
        writer.write_all(&(snapshot.memory.data.len() as u64).to_le_bytes())?;
        writer.write_all(&snapshot.memory.data)?;

        log::info!(
            "Snapshot saved successfully: {} bytes",
            std::fs::metadata(&path)?.len()
        );

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
            return Err(SnapshotError::InvalidFormat(
                "Invalid magic number".to_string(),
            ));
        }

        // 读取版本号
        let mut version_bytes = [0u8; 4];
        reader.read_exact(&mut version_bytes)?;
        let version = u32::from_le_bytes(version_bytes);
        if version != 1 {
            return Err(SnapshotError::InvalidFormat(format!(
                "Unsupported version: {}",
                version
            )));
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
            memory: MemorySnapshot {
                data,
                base_addr: GuestAddr(base_addr),
            },
        })
    }

    /// 列出所有快照
    pub fn list(&self) -> Result<Vec<String>, SnapshotError> {
        let mut snapshots = Vec::new();

        for entry in std::fs::read_dir(&self.snapshot_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("snapshot")
                && let Some(name) = path.file_stem().and_then(|s| s.to_str())
            {
                snapshots.push(name.to_string());
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
        let loaded =
            SnapshotMetadata::from_json(&json).expect("Failed to deserialize snapshot metadata");

        assert_eq!(loaded.name, "test");
        assert_eq!(loaded.arch, "riscv64");
        assert_eq!(loaded.memory_size, 1024 * 1024 * 1024);
        assert_eq!(loaded.vcpu_count, 2);
    }

    #[test]
    fn test_snapshot_manager() {
        let temp_dir = std::env::temp_dir().join("vm_snapshots_test");
        let manager =
            SnapshotFileManager::new(&temp_dir).expect("Failed to create snapshot manager");

        let metadata = SnapshotMetadata::new("test", "riscv64", 1024, 1);
        let vcpu = VcpuSnapshot {
            id: 0,
            pc: 0x80000000,
            sp: 0x80100000,
            gpr: vec![0; 32],
        };
        let memory = MemorySnapshot {
            data: vec![0; 1024],
            base_addr: GuestAddr(0x80000000),
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
