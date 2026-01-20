//! 仓储模式实现
//!
//! 提供状态管理的仓储接口，符合DDD仓储模式。
//! 支持聚合根、事件溯源和快照管理。

use crate::jit::aggregate_root::VirtualMachineAggregate;
use crate::jit::domain_events::{DomainEventEnum, EventVersionMigrator};
use crate::jit::snapshot::Snapshot;
use crate::{VmConfig, VmError, VmId, VmResult, VmState};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

/// 聚合根仓储trait
///
/// 定义聚合根的持久化和检索接口，支持事件溯源。
/// 仓储模式是DDD中的核心模式之一，提供对聚合根的抽象访问。
///
/// # 使用场景
/// - 聚合持久化：保存和加载聚合根状态
/// - 事件溯源：通过事件历史重建聚合状态
/// - 乐观锁控制：通过版本号防止并发冲突
/// - 缓存管理：聚合状态的缓存和失效
///
/// # 设计原则
/// - 仓储是聚合的集合抽象，类似数据库表的抽象
/// - 每个聚合根对应一个仓储
/// - 仓储不包含业务逻辑，只负责持久化
///
/// # 示例
/// ```ignore
/// let repo = InMemoryAggregateRepository::new(event_repo);
/// repo.save_aggregate(&aggregate)?;
/// let loaded = repo.load_aggregate(&vm_id)?;
/// ```
pub trait AggregateRepository: Send + Sync {
    /// 保存聚合根
    ///
    /// 持久化聚合根到仓储中。
    /// 通常会保存聚合的当前状态和未提交的事件。
    ///
    /// # 参数
    /// - `aggregate`: 要保存的聚合根
    ///
    /// # 返回
    /// 保存成功返回Ok(())，失败返回错误
    ///
    /// # 注意
    /// - 保存不会自动提交事件，需要单独调用commit_events()
    /// - 如果聚合已存在，会更新状态
    fn save_aggregate(&self, aggregate: &VirtualMachineAggregate) -> VmResult<()>;

    /// 加载聚合根
    ///
    /// 从仓储中加载聚合根。
    /// 如果聚合不存在，会尝试从事件历史重建。
    ///
    /// # 参数
    /// - `vm_id`: 虚拟机ID
    ///
    /// # 返回
    /// 加载的聚合根（如果存在），否则返回None
    ///
    /// # 注意
    /// - 首先尝试从缓存加载
    /// - 如果缓存未命中，从事件存储重建聚合
    fn load_aggregate(&self, vm_id: &VmId) -> VmResult<Option<VirtualMachineAggregate>>;

    /// 删除聚合根
    ///
    /// 从仓储中删除聚合根及其相关数据。
    ///
    /// # 参数
    /// - `vm_id`: 要删除的虚拟机ID
    ///
    /// # 返回
    /// 删除成功返回Ok(())，失败返回错误
    ///
    /// # 注意
    /// - 删除操作通常不可逆
    /// - 需要考虑是否同时删除事件历史
    fn delete_aggregate(&self, vm_id: &VmId) -> VmResult<()>;

    /// 检查聚合根是否存在
    ///
    /// 检查指定ID的聚合根是否存在于仓储中。
    ///
    /// # 参数
    /// - `vm_id`: 虚拟机ID
    ///
    /// # 返回
    /// 存在返回true，不存在返回false
    fn aggregate_exists(&self, vm_id: &VmId) -> bool;

    /// 获取聚合根版本
    ///
    /// 获取聚合根的当前版本号。
    /// 版本号用于乐观锁控制，防止并发修改冲突。
    ///
    /// # 参数
    /// - `vm_id`: 虚拟机ID
    ///
    /// # 返回
    /// 当前版本号（如果存在），否则返回None
    fn get_aggregate_version(&self, vm_id: &VmId) -> VmResult<Option<u64>>;
}

/// 事件仓储trait
///
/// 定义领域事件的持久化接口，支持事件溯源。
/// 事件溯源是一种持久化模式，通过保存领域事件来重建聚合状态。
///
/// # 使用场景
/// - 事件持久化：保存领域事件到事件存储
/// - 事件重放：从事件历史重建聚合状态
/// - 审计日志：完整的操作历史记录
/// - 事件版本迁移：升级事件schema
///
/// # 事件溯源优势
/// - 完整的审计日志
/// - 可以重放事件到任意历史状态
/// - 支持事件驱动的架构
/// - 天然支持分布式系统
///
/// # 示例
/// ```ignore
/// let repo = InMemoryEventRepository::new();
/// repo.save_event(&vm_id, event)?;
/// let events = repo.load_events(&vm_id, Some(1), Some(10))?;
/// ```
pub trait EventRepository: Send + Sync {
    /// 保存事件
    ///
    /// 将领域事件持久化到事件存储。
    /// 每个事件都会分配一个递增的序列号。
    ///
    /// # 参数
    /// - `vm_id`: 虚拟机ID
    /// - `event`: 要保存的领域事件
    ///
    /// # 返回
    /// 保存成功返回Ok(())，失败返回错误
    ///
    /// # 注意
    /// - 事件是不可变的，一旦保存不能修改
    /// - 序列号由仓储自动分配
    fn save_event(&self, vm_id: &VmId, event: DomainEventEnum) -> VmResult<()>;

    /// 加载事件历史
    fn load_events(&self, vm_id: &VmId, from_version: Option<u64>, to_version: Option<u64>) -> VmResult<Vec<crate::event_store::StoredEvent>>;

    /// 获取最新事件版本
    ///
    /// 获取指定虚拟机的最新事件版本号。
    ///
    /// # 参数
    /// - `vm_id`: 虚拟机ID
    ///
    /// # 返回
    /// 最新版本号（如果存在），否则返回None
    fn get_latest_version(&self, vm_id: &VmId) -> VmResult<Option<u64>>;

    /// 迁移事件版本
    ///
    /// 将事件迁移到最新版本。
    /// 用于事件schema升级和向后兼容。
    ///
    /// # 参数
    /// - `vm_id`: 虚拟机ID
    ///
    /// # 返回
    /// 迁移后的事件列表
    ///
    /// # 注意
    /// 默认实现会加载所有事件并迁移到最新版本。
    /// 实现可以覆盖此方法以提供更高效的迁移策略。
    fn migrate_events(&self, vm_id: &VmId) -> VmResult<Vec<DomainEventEnum>> {
        let stored_events = self.load_events(vm_id, None, None)?;
        let migrated_events = stored_events.into_iter()
            .map(|stored_event| EventVersionMigrator::migrate_to_latest(stored_event.event))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(migrated_events)
    }
}

/// 快照仓储trait
///
/// 定义快照的持久化接口，支持快照优化。
/// 快照是虚拟机在某个时间点的完整状态，用于快速恢复和备份。
///
/// # 使用场景
/// - 虚拟机快照：保存和恢复虚拟机状态
/// - 备份和恢复：定期快照作为备份
/// - 开发和测试：快速切换到不同状态
/// - 调试：保存问题发生时的状态
///
/// # 快照优化
/// 快照可以显著加速事件溯源，因为：
/// - 不需要从初始状态重放所有事件
/// - 从最近的快照开始重放
/// - 可以定期创建快照进行优化
///
/// # 示例
/// ```ignore
/// let snapshot = vm.take_snapshot()?;
/// repo.save_snapshot(&snapshot)?;
/// let loaded = repo.load_snapshot("vm-1", "snap-001")?;
/// vm.restore_snapshot(loaded)?;
/// ```
pub trait SnapshotRepository: Send + Sync {
    /// 保存快照
    ///
    /// 将快照持久化到仓储中。
    /// 快照包含虚拟机的完整状态（内存、寄存器、设备状态等）。
    ///
    /// # 参数
    /// - `snapshot`: 要保存的快照
    ///
    /// # 返回
    /// 保存成功返回Ok(())，失败返回错误
    ///
    /// # 注意
    /// - 快照可能很大，需要考虑存储空间
    /// - 快照ID必须唯一
    fn save_snapshot(&self, snapshot: &Snapshot) -> VmResult<()>;

    /// 加载快照
    ///
    /// 从仓储中加载指定快照。
    ///
    /// # 参数
    /// - `vm_id`: 虚拟机ID
    /// - `snapshot_id`: 快照ID
    ///
    /// # 返回
    /// 加载的快照（如果存在），否则返回None
    ///
    /// # 注意
    /// - 快照加载后需要应用到虚拟机
    /// - 可以从快照重建聚合状态
    fn load_snapshot(&self, vm_id: &str, snapshot_id: &str)
    -> VmResult<Option<Snapshot>>;

    /// 删除快照
    ///
    /// 从仓储中删除指定快照。
    ///
    /// # 参数
    /// - `vm_id`: 虚拟机ID
    /// - `snapshot_id`: 快照ID
    ///
    /// # 返回
    /// 删除成功返回Ok(())，失败返回错误
    ///
    /// # 注意
    /// - 删除操作通常不可逆
    /// - 删除快照不会影响虚拟机运行
    fn delete_snapshot(&self, vm_id: &str, snapshot_id: &str) -> VmResult<()>;

    /// 列出快照
    ///
    /// 列出指定虚拟机的所有快照。
    ///
    /// # 参数
    /// - `vm_id`: 虚拟机ID
    ///
    /// # 返回
    /// 快照列表，按创建时间排序
    fn list_snapshots(&self, vm_id: &str) -> VmResult<Vec<Snapshot>>;

    /// 获取最新快照
    ///
    /// 获取指定虚拟机的最新快照。
    ///
    /// # 参数
    /// - `vm_id`: 虚拟机ID
    ///
    /// # 返回
    /// 最新快照（如果存在），否则返回None
    ///
    /// # 注意
    /// - 通常基于创建时间判断最新
    /// - 用于事件溯源优化，从最新快照开始重放
    fn get_latest_snapshot(&self, vm_id: &str) -> VmResult<Option<Snapshot>>;
}

/// 虚拟机状态仓储trait
///
/// 定义虚拟机状态的持久化和检索接口
pub trait VmStateRepository: Send + Sync {
    /// 保存虚拟机状态
    fn save(&self, vm_id: &str, state: &VmStateSnapshot) -> VmResult<()>;

    /// 加载虚拟机状态
    fn load(&self, vm_id: &str) -> VmResult<Option<VmStateSnapshot>>;

    /// 删除虚拟机状态
    fn delete(&self, vm_id: &str) -> VmResult<()>;

    /// 列出所有虚拟机ID
    fn list_vm_ids(&self) -> VmResult<Vec<String>>;

    /// 检查虚拟机是否存在
    fn exists(&self, vm_id: &str) -> bool {
        self.load(vm_id)
            .map(|s| s.is_some())
            .unwrap_or_else(|e| {
                log::warn!("Failed to check if VM exists: {}", e);
                false
            })
    }
}

/// 虚拟机状态快照
#[derive(Debug, Clone)]
pub struct VmStateSnapshot {
    /// 虚拟机ID
    pub vm_id: String,
    /// 配置
    pub config: VmConfig,
    /// 状态
    pub state: VmState,
    /// 版本号
    pub version: u64,
    /// 时间戳
    pub timestamp: u64,
    /// 聚合根版本（用于事件溯源）
    pub aggregate_version: Option<u64>,
    /// 元数据
    pub metadata: HashMap<String, String>,
}

/// 内存仓储实现（用于测试和开发）
pub struct InMemoryVmStateRepository {
    states: Arc<std::sync::RwLock<std::collections::HashMap<String, VmStateSnapshot>>>,
}

impl InMemoryVmStateRepository {
    /// 创建新的内存仓储
    pub fn new() -> Self {
        Self {
            states: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }
}

impl Default for InMemoryVmStateRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl VmStateRepository for InMemoryVmStateRepository {
    fn save(&self, vm_id: &str, state: &VmStateSnapshot) -> VmResult<()> {
        let mut states = self.states.write().map_err(|_| {
            VmError::Core(crate::CoreError::Concurrency {
                message: "Failed to acquire write lock".to_string(),
                operation: "save".to_string(),
            })
        })?;
        states.insert(vm_id.to_string(), state.clone());
        Ok(())
    }

    fn load(&self, vm_id: &str) -> VmResult<Option<VmStateSnapshot>> {
        let states = self.states.read().map_err(|_| {
            VmError::Core(crate::CoreError::Concurrency {
                message: "Failed to acquire read lock".to_string(),
                operation: "load".to_string(),
            })
        })?;
        Ok(states.get(vm_id).cloned())
    }

    fn delete(&self, vm_id: &str) -> VmResult<()> {
        let mut states = self.states.write().map_err(|_| {
            VmError::Core(crate::CoreError::Concurrency {
                message: "Failed to acquire write lock".to_string(),
                operation: "delete".to_string(),
            })
        })?;
        states.remove(vm_id);
        Ok(())
    }

    fn list_vm_ids(&self) -> VmResult<Vec<String>> {
        let states = self.states.read().map_err(|_| {
            VmError::Core(crate::CoreError::Concurrency {
                message: "Failed to acquire read lock".to_string(),
                operation: "list_vm_ids".to_string(),
            })
        })?;
        Ok(states.keys().cloned().collect())
    }
}

/// 内存聚合根仓储实现
pub struct InMemoryAggregateRepository {
    aggregates: Arc<std::sync::RwLock<HashMap<String, VirtualMachineAggregate>>>,
    event_repo: Arc<dyn EventRepository>,
}

impl InMemoryAggregateRepository {
    pub fn new(event_repo: Arc<dyn EventRepository>) -> Self {
        Self {
            aggregates: Arc::new(std::sync::RwLock::new(HashMap::new())),
            event_repo,
        }
    }
}

impl AggregateRepository for InMemoryAggregateRepository {
    fn save_aggregate(&self, aggregate: &VirtualMachineAggregate) -> VmResult<()> {
        let mut aggregates = self.aggregates.write().map_err(|_| {
            VmError::Core(crate::CoreError::Concurrency {
                message: "Failed to acquire write lock".to_string(),
                operation: "save_aggregate".to_string(),
            })
        })?;
        aggregates.insert(aggregate.vm_id().to_string(), aggregate.clone());
        Ok(())
    }

    fn load_aggregate(&self, vm_id: &VmId) -> VmResult<Option<VirtualMachineAggregate>> {
        let aggregates = self.aggregates.read().map_err(|_| {
            VmError::Core(crate::CoreError::Concurrency {
                message: "Failed to acquire read lock".to_string(),
                operation: "load_aggregate".to_string(),
            })
        })?;

        if let Some(aggregate) = aggregates.get(vm_id.as_str()) {
            Ok(Some(aggregate.clone()))
        } else {
            // 从事件历史重建聚合根
            let stored_events = self.event_repo.load_events(vm_id, None, None)?;
            if stored_events.is_empty() {
                Ok(None)
            } else {
                // 从事件中提取配置：查找VmCreated事件获取初始配置
                let domain_events: Vec<DomainEventEnum> = stored_events.iter()
                    .map(|stored_event| stored_event.event.clone())
                    .collect();
                let config = Self::extract_config_from_events(&domain_events);
                let aggregate = VirtualMachineAggregate::from_events(
                    vm_id.as_str().to_string(),
                    config,
                    stored_events,
                );
                Ok(Some(aggregate))
            }
        }
    }

    fn delete_aggregate(&self, vm_id: &VmId) -> VmResult<()> {
        let mut aggregates = self.aggregates.write().map_err(|_| {
            VmError::Core(crate::CoreError::Concurrency {
                message: "Failed to acquire write lock".to_string(),
                operation: "delete_aggregate".to_string(),
            })
        })?;
        aggregates.remove(vm_id.as_str());
        Ok(())
    }

    fn aggregate_exists(&self, vm_id: &VmId) -> bool {
        // Handle poisoned lock gracefully by returning false
        let aggregates = match self.aggregates.read() {
            Ok(guard) => guard,
            Err(_) => {
                log::warn!("Aggregate lock is poisoned, assuming aggregate does not exist");
                return false;
            }
        };

        aggregates.contains_key(vm_id.as_str())
            || self
                .event_repo
                .get_latest_version(vm_id)
                .unwrap_or(None)
                .is_some()
    }

    fn get_aggregate_version(&self, vm_id: &VmId) -> VmResult<Option<u64>> {
        self.event_repo.get_latest_version(vm_id)
    }
}

impl InMemoryAggregateRepository {
    /// 从事件历史中提取VM配置
    /// 
    /// 遍历事件列表，查找VmCreated或VmCreatedV2事件以获取初始配置，
    /// 然后应用后续的配置变更事件（如果有）
    fn extract_config_from_events(events: &[DomainEventEnum]) -> VmConfig {
        use crate::domain_events::{VmLifecycleEvent, VmConfigSnapshot};
        use crate::{GuestArch, ExecMode};
        
        // 查找VmCreated事件获取初始配置
        for event in events.iter() {
            if let DomainEventEnum::VmLifecycle(lifecycle_event) = event {
                match lifecycle_event {
                    VmLifecycleEvent::VmCreated { config, .. } => {
                        return Self::config_from_snapshot(config);
                    }
                    VmLifecycleEvent::VmCreatedV2 { config, .. } => {
                        return Self::config_from_snapshot(config);
                    }
                    _ => continue,
                }
            }
        }
        
        // 如果没有找到创建事件，返回默认配置
        VmConfig::default()
    }
    
    /// 从VmConfigSnapshot转换为VmConfig
    fn config_from_snapshot(snapshot: &crate::domain_events::VmConfigSnapshot) -> VmConfig {
        use crate::{GuestArch, ExecMode};
        
        // 解析guest_arch
        let guest_arch = match snapshot.guest_arch.to_lowercase().as_str() {
            "x86_64" | "x86-64" | "amd64" => GuestArch::X86_64,
            "arm64" | "aarch64" => GuestArch::Arm64,
            "riscv64" | "riscv-64" => GuestArch::Riscv64,
            _ => GuestArch::Riscv64, // 默认使用RISC-V
        };
        
        // 解析exec_mode
        let exec_mode = if snapshot.exec_mode.contains("Jit") || snapshot.exec_mode.contains("JIT") {
            ExecMode::Jit
        } else if snapshot.exec_mode.contains("Hybrid") {
            ExecMode::Hybrid
        } else if snapshot.exec_mode.contains("Accel") {
            ExecMode::Accelerated
        } else {
            ExecMode::Interpreter
        };
        
        VmConfig {
            guest_arch,
            memory_size: snapshot.memory_size as usize,
            vcpu_count: snapshot.vcpu_count,
            exec_mode,
            ..VmConfig::default()
        }
    }
}

/// 内存事件仓储实现
pub struct InMemoryEventRepository {
    events: Arc<std::sync::RwLock<HashMap<String, Vec<DomainEventEnum>>>>,
}

impl InMemoryEventRepository {
    pub fn new() -> Self {
        Self {
            events: Arc::new(std::sync::RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryEventRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl EventRepository for InMemoryEventRepository {
    fn save_event(&self, vm_id: &VmId, event: DomainEventEnum) -> VmResult<()> {
        let mut events = self.events.write().map_err(|_| {
            VmError::Core(crate::CoreError::Concurrency {
                message: "Failed to acquire write lock".to_string(),
                operation: "save_event".to_string(),
            })
        })?;

        let vm_id_str = vm_id.as_str().to_string();
        events.entry(vm_id_str).or_insert_with(Vec::new).push(event);
        Ok(())
    }

    fn load_events(&self, vm_id: &VmId, from_version: Option<u64>, to_version: Option<u64>) -> VmResult<Vec<crate::event_store::StoredEvent>> {
        let events = self.events.read().map_err(|_| {
            VmError::Core(crate::CoreError::Concurrency {
                message: "Failed to acquire read lock".to_string(),
                operation: "load_events".to_string(),
            })
        })?;

        let vm_events = events.get(vm_id.as_str()).cloned().unwrap_or_default();

        let filtered_events = vm_events
            .into_iter()
            .enumerate()
            .filter_map(|(idx, event)| {
                let version = idx as u64 + 1;
                let from_ok = from_version.is_none_or(|from| version >= from);
                let to_ok = to_version.is_none_or(|to| version <= to);
                if from_ok && to_ok {
                    Some(crate::event_store::StoredEvent {
                        sequence_number: version,
                        vm_id: vm_id.as_str().to_string(),
                        event,
                        stored_at: std::time::SystemTime::now(),
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(filtered_events)
    }

    fn get_latest_version(&self, vm_id: &VmId) -> VmResult<Option<u64>> {
        let events = self.events.read().map_err(|_| {
            VmError::Core(crate::CoreError::Concurrency {
                message: "Failed to acquire read lock".to_string(),
                operation: "get_latest_version".to_string(),
            })
        })?;

        Ok(events.get(vm_id.as_str()).map(|events| events.len() as u64))
    }
}

/// 仓储工厂
///
/// 提供统一的仓储创建接口
pub struct RepositoryFactory;

impl RepositoryFactory {
    /// 创建内存仓储套件（用于测试）
    pub fn create_in_memory_suite() -> RepositorySuite {
        let event_repo = Arc::new(InMemoryEventRepository::new());
        let aggregate_repo = Arc::new(InMemoryAggregateRepository::new(event_repo.clone()));
        let state_repo = Arc::new(InMemoryVmStateRepository::new());
        let snapshot_repo = Arc::new(InMemorySnapshotRepository::new());

        RepositorySuite {
            aggregate_repo,
            event_repo,
            state_repo,
            snapshot_repo,
        }
    }
}

/// 仓储套件
///
/// 包含所有仓储接口的实现
pub struct RepositorySuite {
    pub aggregate_repo: Arc<dyn AggregateRepository>,
    pub event_repo: Arc<dyn EventRepository>,
    pub state_repo: Arc<dyn VmStateRepository>,
    pub snapshot_repo: Arc<dyn SnapshotRepository>,
}

/// 内存快照仓储实现
pub struct InMemorySnapshotRepository {
    snapshots: Arc<std::sync::RwLock<HashMap<String, HashMap<String, Snapshot>>>>,
}

impl Default for InMemorySnapshotRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemorySnapshotRepository {
    pub fn new() -> Self {
        Self {
            snapshots: Arc::new(std::sync::RwLock::new(HashMap::new())),
        }
    }
    
    /// 从snapshot中提取vm_id
    /// 
    /// 使用多种策略提取vm_id：
    /// 1. 如果snapshot.id包含冒号，使用冒号前的部分作为vm_id
    /// 2. 如果snapshot.name不为空，使用name的第一个单词作为vm_id
    /// 3. 否则，尝试从memory_dump_path中提取
    /// 4. 最后返回默认值
    fn extract_vm_id_from_snapshot(snapshot: &snapshot::Snapshot) -> String {
        // 策略1：从id中提取（格式：vm_id:snapshot_uuid 或 vm_id-snapshot_uuid）
        if let Some(colon_pos) = snapshot.id.find(':') {
            return snapshot.id[..colon_pos].to_string();
        }
        if let Some(dash_pos) = snapshot.id.find('-') {
            // 检查是否看起来像UUID（包含多个短横线）
            let dash_count = snapshot.id.chars().filter(|&c| c == '-').count();
            if dash_count <= 2 {
                // 可能是 vm_id-xxx 格式
                return snapshot.id[..dash_pos].to_string();
            }
        }
        
        // 策略2：从name中提取
        if !snapshot.name.is_empty() {
            let name_parts: Vec<&str> = snapshot.name.split(|c: char| c.is_whitespace() || c == '-' || c == '_').collect();
            if !name_parts.is_empty() && name_parts[0].len() > 2 {
                return name_parts[0].to_string();
            }
        }
        
        // 策略3：从memory_dump_path中提取
        if !snapshot.memory_dump_path.is_empty() {
            // 尝试从路径中提取目录名作为vm_id
            if let Some(path) = std::path::Path::new(&snapshot.memory_dump_path).parent() {
                if let Some(dir_name) = path.file_name() {
                    if let Some(name) = dir_name.to_str() {
                        if !name.is_empty() && name != "snapshots" && name != "." {
                            return name.to_string();
                        }
                    }
                }
            }
        }
        
        // 策略4：返回默认值
        "default-vm".to_string()
    }
}

impl SnapshotRepository for InMemorySnapshotRepository {
    fn save_snapshot(&self, snapshot: &snapshot::Snapshot) -> VmResult<()> {
        // 从snapshot的id或name中提取vm_id
        // 约定：snapshot.id格式为 "vm_id:snapshot_uuid" 或 snapshot.name以vm_id开头
        let vm_id = Self::extract_vm_id_from_snapshot(snapshot);

        let mut snapshots = self.snapshots.write().map_err(|_| {
            VmError::Core(crate::CoreError::Concurrency {
                message: "Failed to acquire write lock".to_string(),
                operation: "save_snapshot".to_string(),
            })
        })?;

        let vm_snapshots = snapshots
            .entry(vm_id.to_string())
            .or_insert_with(HashMap::new);
        vm_snapshots.insert(snapshot.id.clone(), snapshot.clone());
        Ok(())
    }

    fn load_snapshot(
        &self,
        vm_id: &str,
        snapshot_id: &str,
    ) -> VmResult<Option<Snapshot>> {
        let snapshots = self.snapshots.read().map_err(|_| {
            VmError::Core(crate::CoreError::Concurrency {
                message: "Failed to acquire read lock".to_string(),
                operation: "load_snapshot".to_string(),
            })
        })?;

        Ok(snapshots
            .get(vm_id)
            .and_then(|vm_snapshots| vm_snapshots.get(snapshot_id))
            .cloned())
    }

    fn delete_snapshot(&self, vm_id: &str, snapshot_id: &str) -> VmResult<()> {
        let mut snapshots = self.snapshots.write().map_err(|_| {
            VmError::Core(crate::CoreError::Concurrency {
                message: "Failed to acquire write lock".to_string(),
                operation: "delete_snapshot".to_string(),
            })
        })?;

        if let Some(vm_snapshots) = snapshots.get_mut(vm_id) {
            vm_snapshots.remove(snapshot_id);
        }
        Ok(())
    }

    fn list_snapshots(&self, vm_id: &str) -> VmResult<Vec<Snapshot>> {
        let snapshots = self.snapshots.read().map_err(|_| {
            VmError::Core(crate::CoreError::Concurrency {
                message: "Failed to acquire read lock".to_string(),
                operation: "list_snapshots".to_string(),
            })
        })?;

        Ok(snapshots
            .get(vm_id)
            .map(|vm_snapshots| vm_snapshots.values().cloned().collect())
            .unwrap_or_default())
    }

    fn get_latest_snapshot(&self, vm_id: &str) -> VmResult<Option<Snapshot>> {
        let snapshots = self.list_snapshots(vm_id)?;
        // 注意：当前Snapshot结构没有created_at字段，我们按ID排序作为临时方案
        // 这是一个临时解决方案，需要扩展Snapshot结构
        Ok(snapshots.into_iter().max_by_key(|s| s.id.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_repository() {
        let repo = InMemoryVmStateRepository::new();

        let snapshot = VmStateSnapshot {
            vm_id: "test-vm".to_string(),
            config: VmConfig::default(),
            state: VmState::Created,
            version: 1,
            timestamp: 0,
            aggregate_version: None,
            metadata: HashMap::new(),
        };

        // 保存
        assert!(repo.save("test-vm", &snapshot).is_ok());

        // 加载
        let loaded = repo.load("test-vm").unwrap_or_else(|e| {
            panic!("Failed to load VM state: {}", e);
        });
        assert!(loaded.is_some());
        assert_eq!(
            loaded.unwrap_or_else(|| panic!("No VM state found")).vm_id,
            "test-vm"
        );

        // 检查存在
        assert!(repo.exists("test-vm"));

        // 列出
        let ids = repo.list_vm_ids().unwrap_or_else(|e| {
            panic!("Failed to list VM IDs: {}", e);
        });
        assert!(ids.contains(&"test-vm".to_string()));

        // 删除
        assert!(repo.delete("test-vm").is_ok());
        assert!(!repo.exists("test-vm"));
    }

    #[test]
    fn test_aggregate_repository() {
        let event_repo = Arc::new(InMemoryEventRepository::new());
        let repo = InMemoryAggregateRepository::new(event_repo);
        let vm_id = VmId::new("test-vm").unwrap_or_else(|e| {
            panic!("Failed to create VmId: {}", e);
        });

        // 聚合根不存在
        assert!(!repo.aggregate_exists(&vm_id));

        // 创建一个基本的聚合根进行测试
        let config = VmConfig {
            guest_arch: crate::GuestArch::RiscV64,
            memory_size: 128 * 1024 * 1024,
            vcpu_count: 2,
            exec_mode: crate::ExecMode::JIT,
        };

        let aggregate = VirtualMachineAggregate::with_event_bus(
            vm_id.as_str().to_string(),
            config,
            Arc::new(crate::domain_event_bus::DomainEventBus::new()),
        );

        // 保存聚合根
        repo.save_aggregate(&aggregate).unwrap_or_else(|e| {
            panic!("Failed to save aggregate: {}", e);
        });

        // 检查存在性
        assert!(repo.aggregate_exists(&vm_id));

        // 加载聚合根
        let loaded = repo.load_aggregate(&vm_id)
            .unwrap_or_else(|e| panic!("Failed to load aggregate: {}", e))
            .unwrap_or_else(|| panic!("No aggregate found"));
        assert_eq!(loaded.vm_id(), vm_id.as_str());

        // 获取版本
        let version = repo.get_aggregate_version(&vm_id).unwrap_or_else(|e| {
            panic!("Failed to get aggregate version: {}", e);
        });
        assert_eq!(version, Some(0)); // 新创建的聚合根版本为0
    }

    #[test]
    fn test_event_repository() {
        let repo = InMemoryEventRepository::new();
        let vm_id = VmId::new("test-vm").unwrap_or_else(|e| {
            panic!("Failed to create VmId: {}", e);
        });

        // 初始状态
        assert_eq!(
            repo.get_latest_version(&vm_id).unwrap_or_else(|e| {
                panic!("Failed to get latest version: {}", e);
            }),
            None
        );

        // 保存事件
        use crate::jit::domain_events::{DomainEventEnum, VmLifecycleEvent};

        let event = DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmStarted {
            vm_id: vm_id.as_str().to_string(),
            occurred_at: SystemTime::now(),
        });

        repo.save_event(&vm_id, event).unwrap_or_else(|e| {
            panic!("Failed to save event: {}", e);
        });

        // 检查版本
        assert_eq!(
            repo.get_latest_version(&vm_id).unwrap_or_else(|e| {
                panic!("Failed to get latest version: {}", e);
            }),
            Some(1)
        );

        // 加载事件
        let events = repo.load_events(&vm_id, None, None).unwrap_or_else(|e| {
            panic!("Failed to load events: {}", e);
        });
        assert_eq!(events.len(), 1);
        // 验证返回的是StoredEvent
        assert_eq!(events[0].sequence_number, 1);
        assert_eq!(events[0].vm_id, vm_id.as_str());
    }

    #[test]
    fn test_repository_factory() {
        let suite = RepositoryFactory::create_in_memory_suite();

        // 验证所有仓储都已创建（通过简单的操作验证）
        // Test that repositories are functional by checking basic operations
        let vm_id = "test-vm-factory";

        // Test state repo
        let snapshot = VmStateSnapshot {
            vm_id: vm_id.to_string(),
            config: VmConfig::default(),
            state: VmState::Created,
            version: 1,
            timestamp: 0,
            aggregate_version: None,
            metadata: std::collections::HashMap::new(),
        };
        assert!(suite.state_repo.save(vm_id, &snapshot).is_ok());

        // Test aggregate repo
        let vm_id_obj = VmId::new(vm_id.to_string()).unwrap_or_else(|e| {
            panic!("Failed to create VmId: {}", e);
        });
        assert!(!suite.aggregate_repo.aggregate_exists(&vm_id_obj));

        // Test event repo
        assert_eq!(
            suite.event_repo.get_latest_version(&vm_id_obj).unwrap_or_else(|e| {
                panic!("Failed to get version: {}", e);
            }),
            None
        );

        // Test snapshot repo
        assert!(suite.snapshot_repo.list_snapshots(vm_id).is_ok());
    }
}
