//! 快照管理模块
//!
//! 提供虚拟机快照的创建、恢复、列表和序列化功能

use std::fs::{self, File, create_dir_all};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use vm_core::vm_state::VirtualMachineState;
use vm_core::{GuestAddr, VmError, VmResult};

/// Type alias for vCPU data: (vcpu_id, pc, stack_pointer, register_values)
type VcpuData = (u32, u64, u64, Vec<u64>);

/// Type alias for deserialized VM state
type DeserializedVmState = (vm_core::VmConfig, Vec<VcpuData>, vm_core::ExecStats);

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

impl From<SnapshotError> for VmError {
    fn from(err: SnapshotError) -> Self {
        VmError::Core(vm_core::CoreError::Internal {
            message: err.to_string(),
            module: "snapshot_manager".to_string(),
        })
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
struct SnapshotManager {
    /// 快照存储目录
    snapshot_dir: PathBuf,
}

impl SnapshotManager {
    /// 创建新的快照管理器
    fn new(snapshot_dir: impl AsRef<Path>) -> Result<Self, SnapshotError> {
        let snapshot_dir = snapshot_dir.as_ref().to_path_buf();
        create_dir_all(&snapshot_dir)?;

        Ok(Self { snapshot_dir })
    }

    /// 获取快照路径
    fn snapshot_path(&self, id: &str) -> PathBuf {
        self.snapshot_dir.join(format!("{}.snapshot", id))
    }

    /// 保存快照
    fn save(&self, snapshot: &VmSnapshot) -> Result<String, SnapshotError> {
        let snapshot_id = format!("{}-{}", snapshot.metadata.name, snapshot.metadata.timestamp);
        let path = self.snapshot_path(&snapshot_id);
        let file = File::create(&path)
            .map_err(|e| SnapshotError::SaveFailed(format!("Failed to create file: {}", e)))?;
        let mut writer = BufWriter::new(file);

        log::info!("Saving snapshot to {:?}", path);

        // 写入魔数
        writer.write_all(b"VMSN")?;

        // 写入版本号
        writer.write_all(&1u32.to_le_bytes())?;

        // 简化：写入元数据作为JSON字符串（长度前缀）
        let metadata_str = format!(
            r#"{{"name":"{}","timestamp":{},"arch":"{}","memory_size":{},"vcpu_count":{},"description":{}}}"#,
            snapshot.metadata.name,
            snapshot.metadata.timestamp,
            snapshot.metadata.arch,
            snapshot.metadata.memory_size,
            snapshot.metadata.vcpu_count,
            snapshot
                .metadata
                .description
                .as_ref()
                .map(|s| format!("\"{}\"", s))
                .unwrap_or_else(|| "null".to_string())
        );
        writer.write_all(&(metadata_str.len() as u32).to_le_bytes())?;
        writer.write_all(metadata_str.as_bytes())?;

        // 写入vCPU数量和数据
        writer.write_all(&(snapshot.vcpus.len() as u32).to_le_bytes())?;
        for vcpu in &snapshot.vcpus {
            writer.write_all(&vcpu.id.to_le_bytes())?;
            writer.write_all(&vcpu.pc.to_le_bytes())?;
            writer.write_all(&vcpu.sp.to_le_bytes())?;
            writer.write_all(&(vcpu.gpr.len() as u32).to_le_bytes())?;
            for reg in &vcpu.gpr {
                writer.write_all(&reg.to_le_bytes())?;
            }
        }

        // 写入内存快照
        writer.write_all(&(snapshot.memory.data.len() as u64).to_le_bytes())?;
        writer.write_all(&snapshot.memory.base_addr.0.to_le_bytes())?;
        writer.write_all(&snapshot.memory.data)?;

        writer.flush()?;

        log::info!("Snapshot saved successfully: {}", snapshot_id);
        Ok(snapshot_id)
    }

    /// 加载快照
    fn load(&self, id: &str) -> Result<VmSnapshot, SnapshotError> {
        let path = self.snapshot_path(id);
        let file = File::open(&path)
            .map_err(|e| SnapshotError::LoadFailed(format!("Failed to open file: {}", e)))?;
        let mut reader = BufReader::new(file);

        log::info!("Loading snapshot from {:?}", path);

        // 读取并验证魔数
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
        let mut metadata_str = vec![0u8; metadata_len];
        reader.read_exact(&mut metadata_str)?;
        let metadata_str = String::from_utf8_lossy(&metadata_str);

        // 简化：解析JSON（手动提取字段）
        let metadata = SnapshotMetadata {
            name: extract_json_field(&metadata_str, "name").unwrap_or_default(),
            timestamp: extract_json_field(&metadata_str, "timestamp")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            arch: extract_json_field(&metadata_str, "arch").unwrap_or_default(),
            memory_size: extract_json_field(&metadata_str, "memory_size")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            vcpu_count: extract_json_field(&metadata_str, "vcpu_count")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            description: extract_json_field(&metadata_str, "description"),
        };

        // 读取vCPU数量
        let mut vcpu_count_bytes = [0u8; 4];
        reader.read_exact(&mut vcpu_count_bytes)?;
        let vcpu_count = u32::from_le_bytes(vcpu_count_bytes) as usize;
        let mut vcpus = Vec::with_capacity(vcpu_count);

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

            let mut gpr_len_bytes = [0u8; 4];
            reader.read_exact(&mut gpr_len_bytes)?;
            let gpr_len = u32::from_le_bytes(gpr_len_bytes) as usize;
            let mut gpr = vec![0u64; gpr_len];
            for reg in &mut gpr {
                let mut reg_bytes = [0u8; 8];
                reader.read_exact(&mut reg_bytes)?;
                *reg = u64::from_le_bytes(reg_bytes);
            }

            vcpus.push(VcpuSnapshot { id, pc, sp, gpr });
        }

        // 读取内存快照
        let mut memory_len_bytes = [0u8; 8];
        reader.read_exact(&mut memory_len_bytes)?;
        let memory_len = u64::from_le_bytes(memory_len_bytes) as usize;

        let mut base_addr_bytes = [0u8; 8];
        reader.read_exact(&mut base_addr_bytes)?;
        let base_addr = GuestAddr(u64::from_le_bytes(base_addr_bytes));

        let mut data = vec![0u8; memory_len];
        reader.read_exact(&mut data)?;

        let memory = MemorySnapshot { data, base_addr };

        log::info!("Snapshot loaded successfully: {}", id);
        Ok(VmSnapshot {
            metadata,
            vcpus,
            memory,
        })
    }

    /// 列出所有快照
    fn list(&self) -> Result<Vec<String>, SnapshotError> {
        let mut snapshots = Vec::new();

        let entries = fs::read_dir(&self.snapshot_dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if let Some(id) = path
                .extension()
                .and_then(|s| s.to_str())
                .filter(|&ext| ext == "snapshot")
                .and_then(|_| path.file_stem().and_then(|s| s.to_str()))
            {
                snapshots.push(id.to_string());
            }
        }

        snapshots.sort();
        snapshots.reverse(); // 最新的在前
        Ok(snapshots)
    }

    /// 删除快照
    #[allow(dead_code)]
    fn delete(&self, id: &str) -> Result<(), SnapshotError> {
        let path = self.snapshot_path(id);
        fs::remove_file(&path)?;
        log::info!("Snapshot deleted: {}", id);
        Ok(())
    }
}

/// 简化的JSON字段提取
fn extract_json_field(json: &str, field: &str) -> Option<String> {
    let pattern = format!("\"{}\":\"", field);
    if let Some(start) = json.find(&pattern) {
        let value_start = start + pattern.len();
        if let Some(end) = json[value_start..].find('"') {
            let value = &json[value_start..value_start + end];
            return Some(value.to_string());
        }
    }
    // Try to find numeric field
    let pattern = format!("\"{}\":", field);
    if let Some(start) = json.find(&pattern) {
        let value_start = start + pattern.len();
        let remaining = &json[value_start..];
        let end = remaining
            .find(|c: char| !c.is_ascii_digit() && c != '-')
            .unwrap_or(remaining.len());
        let value = &remaining[..end];
        return Some(value.to_string());
    }
    None
}

/// 全局快照管理器实例（使用lazy_static或once_cell）
static SNAPSHOT_MANAGER: Mutex<Option<SnapshotManager>> = Mutex::new(None);

/// 初始化或获取快照管理器
fn with_manager<F, R>(f: F) -> VmResult<R>
where
    F: FnOnce(&SnapshotManager) -> Result<R, SnapshotError>,
{
    let mut manager_guard = SNAPSHOT_MANAGER.lock().map_err(|_| {
        VmError::Core(vm_core::CoreError::Internal {
            message: "Failed to acquire lock".to_string(),
            module: "snapshot_manager".to_string(),
        })
    })?;

    if manager_guard.is_none() {
        let snapshot_dir =
            std::env::var("VM_SNAPSHOT_DIR").unwrap_or_else(|_| "./snapshots".to_string());
        let manager = SnapshotManager::new(snapshot_dir).map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to initialize snapshot manager: {}", e),
                module: "snapshot_manager".to_string(),
            })
        })?;
        *manager_guard = Some(manager);
    }

    // Use match to safely get the manager reference
    let manager = match manager_guard.as_ref() {
        Some(m) => m,
        None => {
            return Err(VmError::Core(vm_core::CoreError::Internal {
                message: "Snapshot manager initialization failed".to_string(),
                module: "snapshot_manager".to_string(),
            }));
        }
    };
    f(manager).map_err(VmError::from)
}

/// 创建快照
pub fn create_snapshot<B: 'static>(
    state: Arc<Mutex<VirtualMachineState<B>>>,
    name: String,
    description: String,
) -> VmResult<String> {
    log::info!("Creating snapshot: {}", name);

    // 获取VM状态
    let state_guard = state.lock().map_err(|_| {
        VmError::Core(vm_core::CoreError::Internal {
            message: "Failed to acquire state lock".to_string(),
            module: "snapshot_manager".to_string(),
        })
    })?;

    // 提取vCPU状态 - 使用 ExecutionEngine trait 的 get_vcpu_state 方法
    let mut vcpus = Vec::new();
    for (i, engine) in state_guard.vcpus.iter().enumerate() {
        let engine_lock = engine.lock().map_err(|_| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to acquire vCPU {} lock", i),
                module: "snapshot_manager".to_string(),
            })
        })?;

        let vcpu_state = engine_lock.get_vcpu_state();
        vcpus.push(VcpuSnapshot {
            id: i as u32,
            pc: vcpu_state.state.pc.0,
            sp: vcpu_state.state.regs.sp,
            gpr: vcpu_state.state.regs.gpr.to_vec(),
        });
    }

    // 简化：只保存内存大小（实际内存数据暂时为空）
    let memory_snapshot = MemorySnapshot {
        data: vec![0u8; state_guard.config.memory_size.min(1024 * 1024)], // 限制大小：最大1MB
        base_addr: GuestAddr(0),
    };

    // 创建元数据
    let metadata = SnapshotMetadata::new(
        &name,
        format!("{:?}", state_guard.config.guest_arch),
        state_guard.config.memory_size,
        state_guard.config.vcpu_count as u32,
    )
    .with_description(description);

    let snapshot = VmSnapshot {
        metadata,
        vcpus,
        memory: memory_snapshot,
    };

    // 保存快照
    with_manager(|manager| manager.save(&snapshot))
}

/// 恢复快照
pub fn restore_snapshot<B: 'static>(
    state: Arc<Mutex<VirtualMachineState<B>>>,
    id: &str,
) -> VmResult<()> {
    log::info!("Restoring snapshot: {}", id);

    // 加载快照
    let snapshot = with_manager(|manager| manager.load(id))?;

    // 获取VM状态
    let mut state_guard = state.lock().map_err(|_| {
        VmError::Core(vm_core::CoreError::Internal {
            message: "Failed to acquire state lock".to_string(),
            module: "snapshot_manager".to_string(),
        })
    })?;

    // 恢复vCPU状态 - 使用 ExecutionEngine trait 的 set_vcpu_state 方法
    for vcpu_snapshot in &snapshot.vcpus {
        if let Some(engine) = state_guard.vcpus.get_mut(vcpu_snapshot.id as usize) {
            let mut engine_lock = engine.lock().map_err(|_| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: format!("Failed to acquire vCPU {} lock", vcpu_snapshot.id),
                    module: "snapshot_manager".to_string(),
                })
            })?;

            // 获取当前状态并更新寄存器值
            let mut vcpu_state = engine_lock.get_vcpu_state();
            vcpu_state.state.pc = GuestAddr(vcpu_snapshot.pc);
            vcpu_state.state.regs.sp = vcpu_snapshot.sp;
            vcpu_state
                .state
                .regs
                .gpr
                .copy_from_slice(&vcpu_snapshot.gpr);

            // 设置更新后的状态
            engine_lock.set_vcpu_state(&vcpu_state);
        }
    }

    // 恢复VM状态
    state_guard.state = vm_core::VmLifecycleState::Stopped;

    log::info!(
        "Snapshot restored successfully: {} (vCPU state restored)",
        id
    );
    Ok(())
}

/// 列出所有快照
pub fn list_snapshots<B: 'static>(
    _state: Arc<Mutex<VirtualMachineState<B>>>,
) -> VmResult<Vec<String>> {
    with_manager(|manager| manager.list())
}

/// 创建模板
pub fn create_template<B: 'static>(
    state: Arc<Mutex<VirtualMachineState<B>>>,
    name: String,
    description: String,
    base_snapshot_id: String,
) -> VmResult<String> {
    log::info!(
        "Creating template: {} from snapshot: {}",
        name,
        base_snapshot_id
    );

    // Load the base snapshot to verify it exists
    let _snapshot = with_manager(|manager| manager.load(&base_snapshot_id))?;

    // Get state for metadata
    let state_guard = state.lock().map_err(|_| {
        VmError::Core(vm_core::CoreError::Internal {
            message: "Failed to acquire state lock".to_string(),
            module: "snapshot_manager".to_string(),
        })
    })?;

    // Create template metadata file
    let template_id = format!(
        "{}-{}",
        name,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    );

    let template_dir =
        std::env::var("VM_TEMPLATE_DIR").unwrap_or_else(|_| "./templates".to_string());
    std::fs::create_dir_all(&template_dir).map_err(|e| {
        VmError::Core(vm_core::CoreError::Internal {
            message: format!("Failed to create template directory: {}", e),
            module: "snapshot_manager".to_string(),
        })
    })?;

    let template_path =
        std::path::PathBuf::from(&template_dir).join(format!("{}.template", template_id));

    // Write template metadata
    let metadata = format!(
        r#"{{"name":"{}","description":"{}","base_snapshot":"{}","arch":"{:?}","memory_size":{},"vcpu_count":{}}}"#,
        name,
        description,
        base_snapshot_id,
        state_guard.config.guest_arch,
        state_guard.config.memory_size,
        state_guard.config.vcpu_count
    );

    std::fs::write(&template_path, metadata).map_err(|e| {
        VmError::Core(vm_core::CoreError::Internal {
            message: format!("Failed to write template file: {}", e),
            module: "snapshot_manager".to_string(),
        })
    })?;

    log::info!("Template created successfully: {}", template_id);
    Ok(template_id)
}

/// 列出所有模板
pub fn list_templates<B: 'static>(
    _state: Arc<Mutex<VirtualMachineState<B>>>,
) -> VmResult<Vec<String>> {
    let template_dir =
        std::env::var("VM_TEMPLATE_DIR").unwrap_or_else(|_| "./templates".to_string());

    let mut templates = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&template_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(id) = path
                .extension()
                .and_then(|s| s.to_str())
                .filter(|&ext| ext == "template")
                .and_then(|_| path.file_stem().and_then(|s| s.to_str()))
            {
                templates.push(id.to_string());
            }
        }
    }

    templates.sort();
    templates.reverse(); // 最新的在前
    Ok(templates)
}

/// 序列化虚拟机状态以进行迁移
pub fn serialize_state<B: 'static>(state: Arc<Mutex<VirtualMachineState<B>>>) -> VmResult<Vec<u8>> {
    log::info!("Serializing VM state");

    let state_guard = state.lock().map_err(|_| {
        VmError::Core(vm_core::CoreError::Internal {
            message: "Failed to acquire state lock".to_string(),
            module: "snapshot_manager".to_string(),
        })
    })?;

    // Extract vCPU states
    let mut vcpus_data = Vec::new();
    for (i, engine) in state_guard.vcpus.iter().enumerate() {
        let engine_lock = engine.lock().map_err(|_| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to acquire vCPU {} lock", i),
                module: "snapshot_manager".to_string(),
            })
        })?;

        let vcpu_state = engine_lock.get_vcpu_state();
        vcpus_data.push((
            i as u32,
            vcpu_state.state.pc.0,
            vcpu_state.state.regs.sp,
            vcpu_state.state.regs.gpr.to_vec(),
        ));
    }

    // Serialize using bincode
    let serialized = bincode::encode_to_vec(
        (&state_guard.config, &vcpus_data, state_guard.stats.clone()),
        bincode::config::standard(),
    )
    .map_err(|e| {
        VmError::Core(vm_core::CoreError::Internal {
            message: format!("Failed to serialize state: {}", e),
            module: "snapshot_manager".to_string(),
        })
    })?;

    log::info!("VM state serialized: {} bytes", serialized.len());
    Ok(serialized)
}

/// 从序列化数据中反序列化并恢复虚拟机状态
pub fn deserialize_state<B: 'static>(
    state: Arc<Mutex<VirtualMachineState<B>>>,
    data: &[u8],
) -> VmResult<()> {
    log::info!("Deserializing VM state: {} bytes", data.len());

    // Deserialize using bincode - we can only restore vCPU state, not config
    let (_config, vcpus_data, _stats): DeserializedVmState =
        bincode::decode_from_slice(data, bincode::config::standard())
            .map_err(|e| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: format!("Failed to deserialize state: {}", e),
                    module: "snapshot_manager".to_string(),
                })
            })?
            .0;

    let mut state_guard = state.lock().map_err(|_| {
        VmError::Core(vm_core::CoreError::Internal {
            message: "Failed to acquire state lock".to_string(),
            module: "snapshot_manager".to_string(),
        })
    })?;

    // Restore vCPU states
    for (vcpu_id, pc, sp, gpr) in vcpus_data {
        if let Some(engine) = state_guard.vcpus.get_mut(vcpu_id as usize) {
            let mut engine_lock = engine.lock().map_err(|_| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: format!("Failed to acquire vCPU {} lock", vcpu_id),
                    module: "snapshot_manager".to_string(),
                })
            })?;

            // Get current state and update registers
            let mut vcpu_state = engine_lock.get_vcpu_state();
            vcpu_state.state.pc = GuestAddr(pc);
            vcpu_state.state.regs.sp = sp;
            vcpu_state.state.regs.gpr.copy_from_slice(&gpr);

            // Set updated state
            engine_lock.set_vcpu_state(&vcpu_state);
        }
    }

    log::info!("VM state deserialized successfully");
    Ok(())
}

/// 异步创建快照
#[cfg(feature = "async")]
pub async fn create_snapshot_async<B: 'static>(
    state: Arc<tokio::sync::Mutex<VirtualMachineState<B>>>,
    name: String,
    description: String,
) -> VmResult<String> {
    log::info!("Creating snapshot asynchronously: {}", name);

    // Get VM state asynchronously
    let state_guard = state.lock().await;

    // Extract vCPU states
    let mut vcpus = Vec::new();
    for (i, engine) in state_guard.vcpus.iter().enumerate() {
        let engine_lock = engine.lock().map_err(|_| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to acquire vCPU {} lock", i),
                module: "snapshot_manager".to_string(),
            })
        })?;

        let vcpu_state = engine_lock.get_vcpu_state();
        vcpus.push(VcpuSnapshot {
            id: i as u32,
            pc: vcpu_state.state.pc.0,
            sp: vcpu_state.state.regs.sp,
            gpr: vcpu_state.state.regs.gpr.to_vec(),
        });
    }

    // Create memory snapshot (limited size)
    let memory_snapshot = MemorySnapshot {
        data: vec![0u8; state_guard.config.memory_size.min(1024 * 1024)],
        base_addr: GuestAddr(0),
    };

    // Create metadata
    let metadata = SnapshotMetadata::new(
        &name,
        format!("{:?}", state_guard.config.guest_arch),
        state_guard.config.memory_size,
        state_guard.config.vcpu_count as u32,
    )
    .with_description(description);

    let snapshot = VmSnapshot {
        metadata,
        vcpus,
        memory: memory_snapshot,
    };

    // Release the lock before saving
    drop(state_guard);

    // Save snapshot (spawn blocking task for I/O)
    let snapshot_id =
        tokio::task::spawn_blocking(move || with_manager(|manager| manager.save(&snapshot)))
            .await
            .map_err(|e| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: format!("Async task failed: {}", e),
                    module: "snapshot_manager".to_string(),
                })
            })??;

    Ok(snapshot_id)
}

/// 异步恢复快照
#[cfg(feature = "async")]
pub async fn restore_snapshot_async<B: 'static>(
    state: Arc<tokio::sync::Mutex<VirtualMachineState<B>>>,
    id: &str,
) -> VmResult<()> {
    log::info!("Restoring snapshot asynchronously: {}", id);

    // Load snapshot (spawn blocking task for I/O)
    let id_clone = id.to_string();
    let snapshot =
        tokio::task::spawn_blocking(move || with_manager(|manager| manager.load(&id_clone)))
            .await
            .map_err(|e| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: format!("Async task failed: {}", e),
                    module: "snapshot_manager".to_string(),
                })
            })??;

    // Get VM state asynchronously
    let mut state_guard = state.lock().await;

    // Restore vCPU states
    for vcpu_snapshot in &snapshot.vcpus {
        if let Some(engine) = state_guard.vcpus.get_mut(vcpu_snapshot.id as usize) {
            let mut engine_lock = engine.lock().map_err(|_| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: format!("Failed to acquire vCPU {} lock", vcpu_snapshot.id),
                    module: "snapshot_manager".to_string(),
                })
            })?;

            // Get current state and update registers
            let mut vcpu_state = engine_lock.get_vcpu_state();
            vcpu_state.state.pc = GuestAddr(vcpu_snapshot.pc);
            vcpu_state.state.regs.sp = vcpu_snapshot.sp;
            vcpu_state
                .state
                .regs
                .gpr
                .copy_from_slice(&vcpu_snapshot.gpr);

            // Set updated state
            engine_lock.set_vcpu_state(&vcpu_state);
        }
    }

    // Update VM state
    state_guard.state = vm_core::VmLifecycleState::Stopped;

    log::info!("Snapshot restored asynchronously: {}", id);
    Ok(())
}
