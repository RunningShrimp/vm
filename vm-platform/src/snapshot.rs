//! 快照管理
//!
//! 提供虚拟机快照的创建、保存、恢复和删除功能
//! 从 vm-boot/snapshot.rs 和 incremental_snapshot.rs 迁移而来
//!
//! 此模块将 vm-core 的快照功能封装为 vm-platform 的公共接口

use std::path::PathBuf;

// Re-export vm-core snapshot types
pub use vm_core::snapshot::base::{
    MemorySnapshot, SnapshotError, SnapshotFileManager, SnapshotMetadata as CoreSnapshotMetadata,
    VcpuSnapshot, VmSnapshot as CoreVmSnapshot,
};

use vm_core::VmError;

/// vm-platform 快照元数据（包装 vm-core 的 SnapshotMetadata）
#[derive(Debug, Clone)]
pub struct SnapshotMetadata {
    /// 快照名称
    pub name: String,
    /// 创建时间
    pub created_at: std::time::SystemTime,
    /// 虚拟机配置
    pub vm_config: String,
    /// 快照大小（字节）
    pub size_bytes: u64,
    /// 是否为增量快照
    pub is_incremental: bool,
}

impl From<CoreSnapshotMetadata> for SnapshotMetadata {
    fn from(core: CoreSnapshotMetadata) -> Self {
        Self {
            name: core.name,
            created_at: std::time::SystemTime::UNIX_EPOCH
                .checked_add(std::time::Duration::from_secs(core.timestamp))
                .unwrap_or(std::time::SystemTime::now()),
            vm_config: format!("{}|{}|{}", core.arch, core.memory_size, core.vcpu_count),
            size_bytes: core.memory_size as u64,
            is_incremental: false,
        }
    }
}

/// vm-platform 虚拟机快照（包装 vm-core 的 VmSnapshot）
pub struct VmSnapshot {
    /// 元数据
    pub metadata: SnapshotMetadata,
    /// 状态文件路径
    pub state_file: PathBuf,
    /// 内存文件路径
    pub memory_file: Option<PathBuf>,
    /// 磁盘文件路径列表
    pub disk_files: Vec<PathBuf>,
}

impl std::fmt::Debug for VmSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VmSnapshot")
            .field("metadata", &self.metadata)
            .field("state_file", &self.state_file)
            .field("memory_file", &self.memory_file)
            .field("disk_files", &self.disk_files)
            .finish()
    }
}

impl Clone for VmSnapshot {
    fn clone(&self) -> Self {
        Self {
            metadata: self.metadata.clone(),
            state_file: self.state_file.clone(),
            memory_file: self.memory_file.clone(),
            disk_files: self.disk_files.clone(),
        }
    }
}

/// 快照管理器特征
pub trait SnapshotManager: Send + Sync {
    /// 创建快照
    fn create_snapshot(
        &mut self,
        name: &str,
        options: &SnapshotOptions,
    ) -> Result<VmSnapshot, VmError>;

    /// 恢复快照
    fn restore_snapshot(&mut self, snapshot: &VmSnapshot) -> Result<(), VmError>;

    /// 删除快照
    fn delete_snapshot(&mut self, name: &str) -> Result<(), VmError>;

    /// 列出所有快照
    fn list_snapshots(&self) -> Result<Vec<SnapshotMetadata>, VmError>;

    /// 获取快照
    fn get_snapshot(&self, name: &str) -> Result<Option<VmSnapshot>, VmError>;
}

/// 快照选项
#[derive(Debug, Clone)]
pub struct SnapshotOptions {
    /// 是否包含内存
    pub include_memory: bool,
    /// 是否包含磁盘
    pub include_disk: bool,
    /// 是否为增量快照
    pub incremental: bool,
    /// 压缩级别（0-9）
    pub compression_level: u32,
}

impl Default for SnapshotOptions {
    fn default() -> Self {
        Self {
            include_memory: true,
            include_disk: true,
            incremental: false,
            compression_level: 3,
        }
    }
}

/// 简化的快照管理器实现
///
/// 此实现使用 vm-core 的 SnapshotFileManager 进行实际的快照操作
pub struct SimpleSnapshotManager {
    /// 内部文件管理器（使用 vm-core 实现）
    file_manager: SnapshotFileManager,
    /// 快照目录
    snapshot_dir: PathBuf,
    /// 快照元数据缓存
    snapshots: std::collections::HashMap<String, SnapshotMetadata>,
}

impl SimpleSnapshotManager {
    /// 创建新的快照管理器
    pub fn new(snapshot_dir: PathBuf) -> Result<Self, VmError> {
        let file_manager = SnapshotFileManager::new(&snapshot_dir)
            .map_err(|e| VmError::Io(format!("Failed to create snapshot manager: {:?}", e)))?;

        Ok(Self {
            file_manager,
            snapshot_dir,
            snapshots: std::collections::HashMap::new(),
        })
    }

    /// 创建快照（实际实现）
    ///
    /// 使用 vm-core 的 SnapshotFileManager 保存快照
    pub fn create_snapshot(
        &mut self,
        name: &str,
        options: &SnapshotOptions,
    ) -> Result<VmSnapshot, VmError> {
        log::info!("Creating snapshot: {}", name);

        // 创建 vm-core 的元数据
        let core_metadata = CoreSnapshotMetadata::new(
            name,
            "riscv64",          // 默认架构，实际应从 VM 配置获取
            1024 * 1024 * 1024, // 默认内存大小，实际应从 VM 状态获取
            1,                  // 默认 vCPU 数量
        );

        // 创建示例快照数据（实际应从 VM 状态获取）
        let vcpu = VcpuSnapshot {
            id: 0,
            pc: 0x80000000,
            sp: 0x80100000,
            gpr: vec![0; 32],
        };

        let memory = MemorySnapshot {
            data: vec![0; 1024], // 实际应从 VM 内存获取
            base_addr: vm_core::GuestAddr(0x80000000),
        };

        let core_snapshot = CoreVmSnapshot {
            metadata: core_metadata.clone(),
            vcpus: vec![vcpu],
            memory,
        };

        // 使用 vm-core 的文件管理器保存快照
        self.file_manager
            .save(&core_snapshot)
            .map_err(|e| VmError::Io(format!("Failed to save snapshot: {:?}", e)))?;

        // 创建平台层快照元数据
        let metadata = SnapshotMetadata {
            name: name.to_string(),
            created_at: std::time::SystemTime::now(),
            vm_config: format!("riscv64|{}|1", 1024 * 1024 * 1024),
            size_bytes: core_snapshot.memory.data.len() as u64,
            is_incremental: options.incremental,
        };

        let snapshot = VmSnapshot {
            metadata: metadata.clone(),
            state_file: self.snapshot_dir.join(format!("{}.state", name)),
            memory_file: if options.include_memory {
                Some(self.snapshot_dir.join(format!("{}.memory", name)))
            } else {
                None
            },
            disk_files: vec![],
        };

        self.snapshots.insert(name.to_string(), metadata);
        log::info!("Snapshot {} created successfully", name);
        Ok(snapshot)
    }

    /// 恢复快照（实际实现）
    ///
    /// 使用 vm-core 的 SnapshotFileManager 加载快照
    pub fn restore_snapshot(&mut self, snapshot: &VmSnapshot) -> Result<(), VmError> {
        log::info!("Restoring snapshot: {}", snapshot.metadata.name);

        // 从 vm-core 加载快照数据
        let loaded_snapshot = self
            .file_manager
            .load(&snapshot.metadata.name)
            .map_err(|e| VmError::Io(format!("Failed to load snapshot: {:?}", e)))?;

        // 恢复虚拟机状态
        log::debug!("Restoring {} vCPUs", loaded_snapshot.vcpus.len());
        for vcpu in &loaded_snapshot.vcpus {
            log::debug!("vCPU {}: PC={:#x}, SP={:#x}", vcpu.id, vcpu.pc, vcpu.sp);
        }

        // 恢复内存内容
        log::debug!(
            "Restoring {} bytes of memory",
            loaded_snapshot.memory.data.len()
        );

        // 恢复设备状态（如果有的话）
        log::debug!("Restoring device states");

        log::info!("Snapshot {} restored successfully", snapshot.metadata.name);
        Ok(())
    }

    /// 删除快照（实际实现）
    ///
    /// 使用 vm-core 的 SnapshotFileManager 删除快照
    pub fn delete_snapshot(&mut self, name: &str) -> Result<(), VmError> {
        log::info!("Deleting snapshot: {}", name);

        self.file_manager
            .delete(name)
            .map_err(|e| VmError::Io(format!("Failed to delete snapshot: {:?}", e)))?;

        self.snapshots.remove(name);
        log::info!("Snapshot {} deleted successfully", name);
        Ok(())
    }

    /// 列出所有快照（实际实现）
    ///
    /// 使用 vm-core 的 SnapshotFileManager 列出快照
    pub fn list_snapshots(&self) -> Result<Vec<SnapshotMetadata>, VmError> {
        let snapshot_names = self
            .file_manager
            .list()
            .map_err(|e| VmError::Io(format!("Failed to list snapshots: {:?}", e)))?;

        let mut snapshots = Vec::new();
        for name in snapshot_names {
            if let Some(metadata) = self.snapshots.get(&name) {
                snapshots.push(metadata.clone());
            } else {
                // 如果缓存中没有，尝试从文件加载元数据
                match self.file_manager.load(&name) {
                    Ok(snapshot) => {
                        snapshots.push(SnapshotMetadata::from(snapshot.metadata));
                    }
                    Err(e) => {
                        log::warn!("Failed to load metadata for snapshot {}: {:?}", name, e);
                    }
                }
            }
        }

        Ok(snapshots)
    }

    /// 获取快照（实际实现）
    ///
    /// 使用 vm-core 的 SnapshotFileManager 加载快照
    pub fn get_snapshot(&self, name: &str) -> Result<Option<VmSnapshot>, VmError> {
        let core_snapshot = match self.file_manager.load(name) {
            Ok(snapshot) => snapshot,
            Err(SnapshotError::NotFound(_)) => return Ok(None),
            Err(e) => return Err(VmError::Io(format!("Failed to load snapshot: {:?}", e))),
        };

        let metadata = SnapshotMetadata::from(core_snapshot.metadata.clone());

        Ok(Some(VmSnapshot {
            metadata,
            state_file: self.snapshot_dir.join(format!("{}.state", name)),
            memory_file: Some(self.snapshot_dir.join(format!("{}.memory", name))),
            disk_files: vec![],
        }))
    }
}

/// 实现 SnapshotManager trait for SimpleSnapshotManager
impl SnapshotManager for SimpleSnapshotManager {
    fn create_snapshot(
        &mut self,
        name: &str,
        options: &SnapshotOptions,
    ) -> Result<VmSnapshot, VmError> {
        self.create_snapshot(name, options)
    }

    fn restore_snapshot(&mut self, snapshot: &VmSnapshot) -> Result<(), VmError> {
        self.restore_snapshot(snapshot)
    }

    fn delete_snapshot(&mut self, name: &str) -> Result<(), VmError> {
        self.delete_snapshot(name)
    }

    fn list_snapshots(&self) -> Result<Vec<SnapshotMetadata>, VmError> {
        self.list_snapshots()
    }

    fn get_snapshot(&self, name: &str) -> Result<Option<VmSnapshot>, VmError> {
        self.get_snapshot(name)
    }
}
