//! # vm-plugin - 虚拟机插件系统
//!
//! 提供完整的插件架构，支持第三方扩展和模块化功能。
//!
//! ## 主要功能
//!
//! - **插件管理器**: 插件的加载、卸载和生命周期管理
//! - **插件接口**: 统一的插件开发接口和契约
//! - **安全沙箱**: 插件执行的安全隔离和权限控制
//! - **依赖管理**: 插件间的依赖关系解析和版本兼容性
//! - **热更新**: 运行时插件的热加载和更新
//! - **插件仓库**: 插件的分发、发现和安装机制

mod dependency;
mod plugin_manager;
mod security;

pub use dependency::DependencyResolver;
pub use plugin_manager::{
    PluginChannel, PluginChannelHandle, PluginHealth, PluginManager, PluginMessage,
};
pub use security::{
    PermissionPolicy, PluginResourceMonitor, SandboxConfig, SecurityManager,
};

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use vm_core::VmError;

/// 插件ID类型
pub type PluginId = String;

/// 插件版本
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PluginVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl std::fmt::Display for PluginVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// 插件元信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// 插件ID
    pub id: PluginId,
    /// 插件名称
    pub name: String,
    /// 插件版本
    pub version: PluginVersion,
    /// 插件描述
    pub description: String,
    /// 作者
    pub author: String,
    /// 许可证
    pub license: String,
    /// 依赖的插件
    pub dependencies: HashMap<PluginId, PluginVersion>,
    /// 插件类型
    pub plugin_type: PluginType,
    /// 入口点
    pub entry_point: String,
    /// 权限要求
    pub permissions: Vec<PluginPermission>,
}

/// 插件类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginType {
    /// 执行引擎插件
    ExecutionEngine,
    /// 设备模拟插件
    DeviceEmulation,
    /// 调试工具插件
    DebugTool,
    /// 性能监控插件
    PerformanceMonitor,
    /// 网络插件
    Network,
    /// 存储插件
    Storage,
    /// 安全插件
    Security,
    /// 自定义插件
    Custom,
}

/// 插件权限
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginPermission {
    /// 读取虚拟机状态
    ReadVmState,
    /// 写入虚拟机状态
    WriteVmState,
    /// 执行系统调用
    ExecuteSyscall,
    /// 访问网络
    NetworkAccess,
    /// 访问文件系统
    FileSystemAccess,
    /// 分配内存
    MemoryAllocation,
    /// 自定义权限
    Custom(String),
}

/// 插件状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginState {
    /// 未加载
    Unloaded,
    /// 正在加载
    Loading,
    /// 已加载
    Loaded,
    /// 正在初始化
    Initializing,
    /// 活跃
    Active,
    /// 错误
    Error,
    /// 正在卸载
    Unloading,
}

/// 插件实例
pub struct PluginInstance {
    /// 插件元信息
    pub metadata: PluginMetadata,
    /// 插件状态
    pub state: PluginState,
    /// 加载时间
    pub load_time: std::time::Instant,
    /// 插件句柄
    pub handle: Option<Box<dyn Plugin>>,
    /// 依赖的插件
    pub dependencies: HashSet<PluginId>,
    /// 插件统计信息
    pub stats: PluginInstanceStats,
}

/// 插件实例统计信息
#[derive(Debug, Clone)]
pub struct PluginInstanceStats {
    /// 处理的事件数
    pub events_processed: u64,
    /// 错误数
    pub error_count: u64,
    /// 内存使用（字节）
    pub memory_usage_bytes: u64,
    /// CPU时间（微秒）
    pub cpu_time_us: u64,
    /// 最后活动时间
    pub last_activity: std::time::Instant,
}

impl Default for PluginInstanceStats {
    fn default() -> Self {
        Self {
            events_processed: 0,
            error_count: 0,
            memory_usage_bytes: 0,
            cpu_time_us: 0,
            last_activity: std::time::Instant::now(),
        }
    }
}

/// 插件trait
///
/// 所有插件都必须实现这个trait，提供插件的生命周期管理和事件处理能力。
#[async_trait::async_trait]
pub trait Plugin: Send + Sync {
    /// 获取插件元信息
    fn metadata(&self) -> &PluginMetadata;

    /// 初始化插件
    ///
    /// 在插件加载后调用，用于设置插件的基本配置和资源。
    /// 此时插件尚未启动，不应执行任何实际工作。
    async fn initialize(&mut self, context: &PluginContext) -> Result<(), VmError>;

    /// 启动插件
    ///
    /// 启动插件的运行逻辑，开始处理事件和执行任务。
    async fn start(&mut self) -> Result<(), VmError>;

    /// 停止插件
    ///
    /// 停止插件的运行，但保留状态以便后续重启。
    async fn stop(&mut self) -> Result<(), VmError>;

    /// 处理事件
    ///
    /// 处理来自系统或其他插件的事件。
    /// 插件应该快速处理事件，避免阻塞。
    async fn handle_event(&mut self, event: &PluginEvent) -> Result<(), VmError>;

    /// 获取插件状态
    ///
    /// 返回插件的当前状态信息，包括运行时间、内存使用等。
    fn status(&self) -> PluginStatus;

    /// 清理插件资源
    ///
    /// 在插件卸载前调用，用于释放所有资源。
    async fn cleanup(&mut self) -> Result<(), VmError>;

    /// 获取插件配置
    ///
    /// 返回插件的配置信息（可选实现）。
    fn get_config(&self) -> Option<&HashMap<String, String>> {
        None
    }

    /// 更新插件配置
    ///
    /// 更新插件的配置信息（可选实现）。
    async fn update_config(&mut self, _config: HashMap<String, String>) -> Result<(), VmError> {
        Ok(())
    }
}

/// 插件上下文
#[derive(Clone)]
pub struct PluginContext {
    /// 虚拟机实例ID
    pub vm_instance_id: String,
    /// 插件配置
    pub config: HashMap<String, String>,
    /// 共享数据
    pub shared_data: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    /// 事件总线
    pub event_bus: Arc<RwLock<PluginEventBus>>,
}

/// 插件事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginEvent {
    /// 虚拟机启动
    VmStarted { vm_id: String },
    /// 虚拟机停止
    VmStopped { vm_id: String },
    /// 设备添加
    DeviceAdded {
        device_id: String,
        device_type: String,
    },
    /// 设备移除
    DeviceRemoved { device_id: String },
    /// 性能指标更新
    PerformanceUpdate { metric: String, value: f64 },
    /// 自定义事件
    Custom {
        event_type: String,
        data: serde_json::Value,
    },
}

/// 插件状态信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginStatus {
    pub state: PluginState,
    pub uptime_seconds: u64,
    pub memory_usage_bytes: u64,
    pub processed_events: u64,
    pub error_count: u64,
}

/// 插件事件总线
///
/// 提供插件间的事件发布和订阅机制。
pub struct PluginEventBus {
    subscribers: HashMap<String, Vec<Box<dyn Fn(&PluginEvent) + Send + Sync>>>,
    /// 事件统计
    stats: EventBusStats,
}

/// 事件总线统计信息
#[derive(Debug, Clone, Default)]
pub struct EventBusStats {
    /// 发布的事件总数
    pub total_published: u64,
    /// 订阅者总数
    pub total_subscribers: usize,
    /// 按事件类型统计的发布数
    pub events_by_type: HashMap<String, u64>,
}

impl PluginEventBus {
    pub fn new() -> Self {
        Self {
            subscribers: HashMap::new(),
            stats: EventBusStats::default(),
        }
    }

    /// 发布事件
    pub fn publish(&mut self, event: &PluginEvent) {
        let event_type = event.event_type();
        
        // 更新统计
        self.stats.total_published += 1;
        *self.stats.events_by_type.entry(event_type.clone()).or_insert(0) += 1;

        // 通知订阅者
        if let Some(subscribers) = self.subscribers.get(&event_type) {
            for subscriber in subscribers {
                subscriber(event);
            }
        }
    }

    /// 订阅事件
    pub fn subscribe<F>(&mut self, event_type: &str, callback: F)
    where
        F: Fn(&PluginEvent) + Send + Sync + 'static,
    {
        self.subscribers
            .entry(event_type.to_string())
            .or_insert_with(Vec::new)
            .push(Box::new(callback));
        
        // 更新统计
        self.stats.total_subscribers = self.subscribers.values().map(|v| v.len()).sum();
    }

    /// 取消订阅
    pub fn unsubscribe(&mut self, event_type: &str) {
        self.subscribers.remove(event_type);
        
        // 更新统计
        self.stats.total_subscribers = self.subscribers.values().map(|v| v.len()).sum();
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> &EventBusStats {
        &self.stats
    }

    /// 重置统计信息
    pub fn reset_stats(&mut self) {
        self.stats = EventBusStats::default();
    }
}

impl PluginEvent {
    /// 获取事件类型字符串
    pub fn event_type(&self) -> String {
        match self {
            PluginEvent::VmStarted { .. } => "vm.started".to_string(),
            PluginEvent::VmStopped { .. } => "vm.stopped".to_string(),
            PluginEvent::DeviceAdded { .. } => "device.added".to_string(),
            PluginEvent::DeviceRemoved { .. } => "device.removed".to_string(),
            PluginEvent::PerformanceUpdate { .. } => "performance.update".to_string(),
            PluginEvent::Custom { event_type, .. } => event_type.clone(),
        }
    }
}

// PluginManager 已移动到 plugin_manager.rs 模块

/// 插件管理器统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManagerStats {
    pub total_plugins: usize,
    pub active_plugins: usize,
    pub error_plugins: usize,
    pub loaded_plugin_types: HashMap<PluginType, usize>,
    /// 总事件处理数
    pub total_events_processed: u64,
    /// 总错误数
    pub total_errors: u64,
    /// 总内存使用（字节）
    pub total_memory_usage: u64,
}

// SecurityManager 已移动到 security.rs 模块

// DependencyResolver 已移动到 dependency.rs 模块

/// 插件仓库
pub struct PluginRepository {
    /// 可用插件列表
    available_plugins: HashMap<PluginId, PluginMetadata>,
    /// 插件下载URL
    download_urls: HashMap<PluginId, String>,
}

impl PluginRepository {
    pub fn new() -> Self {
        Self {
            available_plugins: HashMap::new(),
            download_urls: HashMap::new(),
        }
    }

    /// 添加插件到仓库
    pub fn add_plugin(&mut self, metadata: PluginMetadata, download_url: String) {
        let id = metadata.id.clone();
        self.available_plugins.insert(id.clone(), metadata);
        self.download_urls.insert(id, download_url);
    }

    /// 获取插件元信息
    pub fn get_plugin_metadata(&self, plugin_id: &str) -> Option<&PluginMetadata> {
        self.available_plugins.get(plugin_id)
    }

    /// 获取所有可用插件
    pub fn get_available_plugins(&self) -> Vec<&PluginMetadata> {
        self.available_plugins.values().collect()
    }

    /// 下载插件
    pub async fn download_plugin(
        &self,
        plugin_id: &str,
        _target_path: &Path,
    ) -> Result<(), VmError> {
        if let Some(url) = self.download_urls.get(plugin_id) {
            // 简化的下载实现
            // 实际应该使用HTTP客户端下载插件
            println!("Downloading plugin {} from {}", plugin_id, url);
            Ok(())
        } else {
            Err(VmError::Core(vm_core::CoreError::InvalidState {
                message: format!("Plugin {} not found in repository", plugin_id),
                current: "not_found".to_string(),
                expected: "available".to_string(),
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_manager_creation() {
        let manager = PluginManager::new("test_vm".to_string());
        assert_eq!(manager.loaded_plugins.len(), 0);
    }

    #[test]
    fn test_plugin_version_comparison() {
        let v1 = PluginVersion {
            major: 1,
            minor: 0,
            patch: 0,
        };
        let v2 = PluginVersion {
            major: 1,
            minor: 1,
            patch: 0,
        };
        let v3 = PluginVersion {
            major: 2,
            minor: 0,
            patch: 0,
        };

        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v1 < v3);
    }

    #[test]
    fn test_security_manager() {
        let mut security = SecurityManager::new();

        // 默认允许的权限
        assert!(
            security
                .check_permissions(&[PluginPermission::ReadVmState])
                .is_ok()
        );
        assert!(
            security
                .check_permissions(&[PluginPermission::WriteVmState])
                .is_err()
        );

        // 添加权限
        security.add_permission(PluginPermission::WriteVmState);
        assert!(
            security
                .check_permissions(&[PluginPermission::WriteVmState])
                .is_ok()
        );

        // 移除权限
        security.remove_permission(&PluginPermission::WriteVmState);
        assert!(
            security
                .check_permissions(&[PluginPermission::WriteVmState])
                .is_err()
        );
    }

    #[test]
    fn test_dependency_resolver() {
        let mut resolver = DependencyResolver::new();

        // 注册插件
        resolver.register_plugin(
            "base".to_string(),
            PluginVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
        );

        // 创建依赖base插件的插件元信息
        let metadata = PluginMetadata {
            id: "dependent".to_string(),
            name: "Dependent Plugin".to_string(),
            version: PluginVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            description: "A plugin with dependencies".to_string(),
            author: "Test".to_string(),
            license: "MIT".to_string(),
            dependencies: HashMap::from([(
                "base".to_string(),
                PluginVersion {
                    major: 1,
                    minor: 0,
                    patch: 0,
                },
            )]),
            plugin_type: PluginType::Custom,
            entry_point: "entry".to_string(),
            permissions: vec![],
        };

        // 解析依赖应该成功
        assert!(resolver.resolve_dependencies(&metadata).is_ok());

        // 尝试解析需要更高版本的依赖应该失败
        let metadata2 = PluginMetadata {
            id: "dependent2".to_string(),
            name: "Dependent Plugin 2".to_string(),
            version: PluginVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            description: "A plugin with unmet dependencies".to_string(),
            author: "Test".to_string(),
            license: "MIT".to_string(),
            dependencies: HashMap::from([(
                "base".to_string(),
                PluginVersion {
                    major: 2,
                    minor: 0,
                    patch: 0,
                },
            )]),
            plugin_type: PluginType::Custom,
            entry_point: "entry".to_string(),
            permissions: vec![],
        };

        assert!(resolver.resolve_dependencies(&metadata2).is_err());
    }

    #[test]
    fn test_plugin_event_bus() {
        let mut event_bus = PluginEventBus::new();
        let mut received_events = Vec::new();

        // 订阅事件
        event_bus.subscribe("test", |event| {
            received_events.push(event.clone());
        });

        // 发布事件
        let event = PluginEvent::VmStarted {
            vm_id: "test_vm".to_string(),
        };
        event_bus.publish(&event);

        // 检查事件是否被接收
        assert_eq!(received_events.len(), 1);
        match &received_events[0] {
            PluginEvent::VmStarted { vm_id } => assert_eq!(vm_id, "test_vm"),
            _ => panic!("Unexpected event type"),
        }
    }

    #[tokio::test]
    async fn test_plugin_manager_stats() {
        let manager = PluginManager::new("test_vm".to_string());
        let stats = manager.get_plugin_stats();

        assert_eq!(stats.total_plugins, 0);
        assert_eq!(stats.active_plugins, 0);
        assert_eq!(stats.error_plugins, 0);
    }
}
