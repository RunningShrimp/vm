//! 插件管理器实现
//!
//! 提供插件的加载、卸载、生命周期管理和通信功能

use crate::{
    DependencyResolver, Plugin, PluginContext, PluginEvent, PluginEventBus, PluginId,
    PluginInstance, PluginManagerStats, PluginMetadata, PluginPermission, PluginState,
    PluginType, SecurityManager,
};
use libloading::Library;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use vm_core::VmError;

/// 插件管理器
pub struct PluginManager {
    /// 已加载的插件
    loaded_plugins: HashMap<PluginId, PluginInstance>,
    /// 插件搜索路径
    plugin_paths: Vec<PathBuf>,
    /// 插件上下文
    context: PluginContext,
    /// 安全管理器
    security_manager: Arc<RwLock<SecurityManager>>,
    /// 依赖解析器
    dependency_resolver: Arc<RwLock<DependencyResolver>>,
    /// 已加载的库（用于动态卸载）
    loaded_libraries: HashMap<PluginId, Library>,
    /// 插件间通信通道
    plugin_channels: Arc<RwLock<HashMap<PluginId, PluginChannel>>>,
}

/// 插件间通信通道
pub struct PluginChannel {
    /// 发送者
    sender: tokio::sync::mpsc::UnboundedSender<PluginMessage>,
    /// 接收者
    receiver: tokio::sync::mpsc::UnboundedReceiver<PluginMessage>,
}

impl PluginChannel {
    pub fn new() -> (Self, PluginChannelHandle) {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        let handle = PluginChannelHandle { sender: sender.clone() };
        (
            Self {
                sender,
                receiver,
            },
            handle,
        )
    }
}

/// 插件通道句柄
pub struct PluginChannelHandle {
    sender: tokio::sync::mpsc::UnboundedSender<PluginMessage>,
}

impl PluginChannelHandle {
    pub fn send(&self, message: PluginMessage) -> Result<(), VmError> {
        self.sender.send(message).map_err(|e| {
            VmError::Core(vm_core::CoreError::InvalidState {
                message: format!("Failed to send plugin message: {}", e),
                current: "send_failed".to_string(),
                expected: "sent".to_string(),
            })
        })
    }
}

/// 插件间消息
#[derive(Debug, Clone)]
pub struct PluginMessage {
    /// 发送者ID
    pub from: PluginId,
    /// 接收者ID（None表示广播）
    pub to: Option<PluginId>,
    /// 消息类型
    pub message_type: String,
    /// 消息数据
    pub data: serde_json::Value,
}

impl PluginManager {
    /// 创建新的插件管理器
    pub fn new(vm_instance_id: String) -> Self {
        let context = PluginContext {
            vm_instance_id,
            config: HashMap::new(),
            shared_data: Arc::new(RwLock::new(HashMap::new())),
            event_bus: Arc::new(RwLock::new(PluginEventBus::new())),
        };

        Self {
            loaded_plugins: HashMap::new(),
            plugin_paths: vec![PathBuf::from("./plugins"), PathBuf::from("./vm-plugins")],
            context,
            security_manager: Arc::new(RwLock::new(SecurityManager::new())),
            dependency_resolver: Arc::new(RwLock::new(DependencyResolver::new())),
            loaded_libraries: HashMap::new(),
            plugin_channels: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 添加插件搜索路径
    pub fn add_plugin_path<P: AsRef<Path>>(&mut self, path: P) {
        self.plugin_paths.push(path.as_ref().to_path_buf());
    }

    /// 加载插件
    pub async fn load_plugin<P: AsRef<Path>>(
        &mut self,
        plugin_path: P,
    ) -> Result<PluginId, VmError> {
        let path = plugin_path.as_ref();

        // 检查插件文件是否存在
        if !path.exists() {
            return Err(VmError::Core(vm_core::CoreError::InvalidState {
                message: format!("Plugin file not found: {}", path.display()),
                current: "not_found".to_string(),
                expected: "exists".to_string(),
            }));
        }

        // 加载插件元信息
        let metadata = self.load_plugin_metadata(path)?;

        // 检查是否已加载
        if self.loaded_plugins.contains_key(&metadata.id) {
            return Err(VmError::Core(vm_core::CoreError::InvalidState {
                message: format!("Plugin {} already loaded", metadata.id),
                current: "loaded".to_string(),
                expected: "unloaded".to_string(),
            }));
        }

        // 检查权限
        self.security_manager
            .read()
            .unwrap()
            .check_permissions(&metadata.permissions)?;

        // 解析依赖
        self.dependency_resolver
            .write()
            .unwrap()
            .resolve_dependencies(&metadata)?;

        // 创建插件实例
        let mut instance = PluginInstance {
            metadata: metadata.clone(),
            state: PluginState::Loading,
            load_time: std::time::Instant::now(),
            handle: None,
            dependencies: metadata.dependencies.keys().cloned().collect(),
            stats: crate::PluginInstanceStats::default(),
        };

        // 加载插件库
        let (plugin, library) = self.load_plugin_library(&metadata, path).await?;
        instance.handle = Some(plugin);
        instance.state = PluginState::Loaded;

        // 创建通信通道
        let (channel, _handle) = PluginChannel::new();
        self.plugin_channels
            .write()
            .unwrap()
            .insert(metadata.id.clone(), channel);

        // 初始化插件上下文
        let plugin_context = self.context.clone();

        // 初始化插件
        if let Some(ref mut handle) = instance.handle {
            instance.state = PluginState::Initializing;
            handle.initialize(&plugin_context).await?;
            handle.start().await?;
            instance.state = PluginState::Active;
        }

        let plugin_id = metadata.id.clone();
        self.loaded_plugins.insert(plugin_id.clone(), instance);
        self.loaded_libraries.insert(plugin_id.clone(), library);

        // 注册到依赖解析器
        self.dependency_resolver
            .write()
            .unwrap()
            .register_plugin(plugin_id.clone(), metadata.version.clone());

        Ok(plugin_id)
    }

    /// 卸载插件
    pub async fn unload_plugin(&mut self, plugin_id: &str) -> Result<(), VmError> {
        if let Some(mut instance) = self.loaded_plugins.remove(plugin_id) {
            instance.state = PluginState::Unloading;

            if let Some(ref mut handle) = instance.handle {
                handle.stop().await?;
                handle.cleanup().await?;
            }

            // 移除通信通道
            self.plugin_channels.write().unwrap().remove(plugin_id);

            // 移除库（会触发卸载）
            self.loaded_libraries.remove(plugin_id);

            // 从依赖解析器注销
            self.dependency_resolver
                .write()
                .unwrap()
                .unregister_plugin(plugin_id);

            instance.state = PluginState::Unloaded;
        }

        Ok(())
    }

    /// 发送消息到插件
    pub fn send_message_to_plugin(
        &self,
        from: &str,
        to: &str,
        message_type: String,
        data: serde_json::Value,
    ) -> Result<(), VmError> {
        let channels = self.plugin_channels.read().unwrap();
        if let Some(channel) = channels.get(to) {
            let message = PluginMessage {
                from: from.to_string(),
                to: Some(to.to_string()),
                message_type,
                data,
            };
            channel.sender.send(message).map_err(|e| {
                VmError::Core(vm_core::CoreError::InvalidState {
                    message: format!("Failed to send message: {}", e),
                    current: "send_failed".to_string(),
                    expected: "sent".to_string(),
                })
            })
        } else {
            Err(VmError::Core(vm_core::CoreError::InvalidState {
                message: format!("Plugin {} not found", to),
                current: "not_found".to_string(),
                expected: "found".to_string(),
            }))
        }
    }

    /// 广播消息到所有插件
    pub fn broadcast_message(
        &self,
        from: &str,
        message_type: String,
        data: serde_json::Value,
    ) -> Result<(), VmError> {
        let channels = self.plugin_channels.read().unwrap();
        let message = PluginMessage {
            from: from.to_string(),
            to: None,
            message_type,
            data,
        };

        for (plugin_id, channel) in channels.iter() {
            if plugin_id != from {
                let _ = channel.sender.send(message.clone());
            }
        }

        Ok(())
    }

    /// 获取插件实例
    pub fn get_plugin(&self, plugin_id: &str) -> Option<&PluginInstance> {
        self.loaded_plugins.get(plugin_id)
    }

    /// 获取所有插件
    pub fn get_all_plugins(&self) -> Vec<&PluginInstance> {
        self.loaded_plugins.values().collect()
    }

    /// 发布事件到所有插件
    pub async fn broadcast_event(&mut self, event: PluginEvent) -> Result<(), VmError> {
        for instance in self.loaded_plugins.values_mut() {
            if let Some(ref mut handle) = instance.handle
                && instance.state == PluginState::Active {
                    instance.stats.last_activity = std::time::Instant::now();
                    match handle.handle_event(&event).await {
                        Ok(_) => {
                            instance.stats.events_processed += 1;
                        }
                        Err(e) => {
                            instance.stats.error_count += 1;
                            tracing::warn!("Plugin {} failed to handle event: {:?}", instance.metadata.id, e);
                            // 如果错误过多，将插件状态设置为Error
                            if instance.stats.error_count > 10 {
                                instance.state = PluginState::Error;
                            }
                        }
                    }
                }
        }

        // 发布到事件总线
        self.context.event_bus.write().unwrap().publish(&event);

        Ok(())
    }

    /// 扫描并加载所有可用插件
    pub async fn scan_and_load_plugins(&mut self) -> Result<Vec<PluginId>, VmError> {
        let mut loaded_plugins = Vec::new();
        let paths = self.plugin_paths.clone();
        for path in paths.iter() {
            if path.exists() && path.is_dir() {
                let plugins = self.scan_directory(path).await?;
                loaded_plugins.extend(plugins);
            }
        }

        Ok(loaded_plugins)
    }

    /// 扫描目录中的插件
    async fn scan_directory(&mut self, dir: &Path) -> Result<Vec<PluginId>, VmError> {
        let mut loaded_plugins = Vec::new();

        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if Self::is_plugin_file(&path) {
                    match self.load_plugin(&path).await {
                        Ok(plugin_id) => loaded_plugins.push(plugin_id),
                        Err(e) => {
                            tracing::warn!("Failed to load plugin {}: {:?}", path.display(), e);
                        }
                    }
                }
            }
        }

        Ok(loaded_plugins)
    }

    /// 检查文件是否是插件文件
    fn is_plugin_file(path: &Path) -> bool {
        path.extension()
            .and_then(|s| s.to_str())
            .map(|ext| {
                matches!(
                    ext,
                    "plugin" | "so" | "dylib" | "dll" | "rlib" | "a"
                )
            })
            .unwrap_or(false)
    }

    /// 加载插件元信息
    fn load_plugin_metadata(&self, path: &Path) -> Result<PluginMetadata, VmError> {
        // 尝试从JSON文件加载元信息
        let metadata_path = path.with_extension("json");
        if metadata_path.exists() {
            let json_str = std::fs::read_to_string(&metadata_path)
                .map_err(|e| VmError::Core(vm_core::CoreError::InvalidState {
                    message: format!("Failed to read metadata file: {}", e),
                    current: "read_failed".to_string(),
                    expected: "read_success".to_string(),
                }))?;
            let metadata: PluginMetadata = serde_json::from_str(&json_str)
                .map_err(|e| VmError::Core(vm_core::CoreError::InvalidState {
                    message: format!("Failed to parse metadata: {}", e),
                    current: "parse_failed".to_string(),
                    expected: "parse_success".to_string(),
                }))?;
            return Ok(metadata);
        }

        // 如果没有元信息文件，从文件名推断
        let file_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        Ok(PluginMetadata {
            id: file_name.to_string(),
            name: format!("{} Plugin", file_name),
            version: crate::PluginVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            description: format!("A {} plugin", file_name),
            author: "Unknown".to_string(),
            license: "MIT".to_string(),
            dependencies: HashMap::new(),
            plugin_type: PluginType::Custom,
            entry_point: "plugin_entry".to_string(),
            permissions: vec![PluginPermission::ReadVmState],
        })
    }

    /// 加载插件库
    ///
    /// 从文件系统加载插件动态库并创建插件实例。
    /// 
    /// # 插件ABI要求
    /// 
    /// 插件库必须导出以下函数：
    /// - `plugin_create`: 创建插件实例的函数指针
    /// - `plugin_version`: 返回插件API版本
    /// 
    /// # 实现说明
    /// 
    /// 当前实现使用mock插件作为占位符。实际实现应该：
    /// 1. 使用`libloading`加载动态库
    /// 2. 调用库中的入口点函数创建插件实例
    /// 3. 验证插件API版本兼容性
    async fn load_plugin_library(
        &self,
        metadata: &PluginMetadata,
        path: &Path,
    ) -> Result<(Box<dyn Plugin>, Library), VmError> {
        // 注意：实际的动态库加载需要插件实现特定的ABI
        // 这里提供一个简化的实现，实际使用时需要根据插件ABI调整
        
        // 对于Rust插件，可以使用cdylib crate类型和特定的导出函数
        // 这里提供一个mock实现作为占位符
        
        struct MockPlugin {
            metadata: PluginMetadata,
            config: Option<HashMap<String, String>>,
        }

        impl MockPlugin {
            fn new(metadata: PluginMetadata) -> Self {
                Self {
                    metadata,
                    config: None,
                }
            }
        }

        #[async_trait::async_trait]
        impl Plugin for MockPlugin {
            fn metadata(&self) -> &PluginMetadata {
                &self.metadata
            }

            async fn initialize(&mut self, context: &PluginContext) -> Result<(), VmError> {
                self.config = Some(context.config.clone());
                Ok(())
            }

            async fn start(&mut self) -> Result<(), VmError> {
                Ok(())
            }

            async fn stop(&mut self) -> Result<(), VmError> {
                Ok(())
            }

            async fn handle_event(&mut self, _event: &PluginEvent) -> Result<(), VmError> {
                Ok(())
            }

            fn status(&self) -> crate::PluginStatus {
                crate::PluginStatus {
                    state: PluginState::Active,
                    uptime_seconds: 0,
                    memory_usage_bytes: 0,
                    processed_events: 0,
                    error_count: 0,
                }
            }

            async fn cleanup(&mut self) -> Result<(), VmError> {
                Ok(())
            }

            fn get_config(&self) -> Option<&HashMap<String, String>> {
                self.config.as_ref()
            }

            async fn update_config(&mut self, config: HashMap<String, String>) -> Result<(), VmError> {
                self.config = Some(config);
                Ok(())
            }
        }

        // 尝试加载动态库
        // 注意：实际实现应该使用libloading加载库并调用入口点
        // 这里提供一个简化的实现，使用mock库
        let library = unsafe {
            // 实际实现应该使用：
            // Library::new(path).map_err(|e| {
            //     VmError::Core(vm_core::CoreError::InvalidState {
            //         message: format!("Failed to load plugin library: {}", e),
            //         current: "load_failed".to_string(),
            //         expected: "loaded".to_string(),
            //     })
            // })?
            // 
            // 然后调用库中的入口点函数：
            // let create_fn: Symbol<unsafe extern "C" fn() -> *mut dyn Plugin> = 
            //     library.get(b"plugin_create")?;
            // let plugin = unsafe { Box::from_raw(create_fn()) };
            
            // 为了编译通过，这里使用mock实现
            // 实际应该返回错误或加载真实的库
            match Library::new(path) {
                Ok(lib) => lib,
                Err(_) => {
                    // 如果加载失败，返回错误（实际实现）
                    // 这里为了演示，返回mock实现
                    return Err(VmError::Core(vm_core::CoreError::InvalidState {
                        message: format!("Failed to load plugin library: {}", path.display()),
                        current: "load_failed".to_string(),
                        expected: "loaded".to_string(),
                    }));
                }
            }
        };

        // 返回mock插件和库
        // 实际实现应该调用库中的入口点函数创建插件实例
        Ok((Box::new(MockPlugin::new(metadata.clone())), library))
    }

    /// 获取插件统计信息
    pub fn get_plugin_stats(&self) -> PluginManagerStats {
        let total_plugins = self.loaded_plugins.len();
        let active_plugins = self
            .loaded_plugins
            .values()
            .filter(|p| p.state == PluginState::Active)
            .count();
        let error_plugins = self
            .loaded_plugins
            .values()
            .filter(|p| p.state == PluginState::Error)
            .count();

        let total_events_processed: u64 = self
            .loaded_plugins
            .values()
            .map(|p| p.stats.events_processed)
            .sum();
        let total_errors: u64 = self
            .loaded_plugins
            .values()
            .map(|p| p.stats.error_count)
            .sum();
        let total_memory_usage: u64 = self
            .loaded_plugins
            .values()
            .map(|p| p.stats.memory_usage_bytes)
            .sum();

        PluginManagerStats {
            total_plugins,
            active_plugins,
            error_plugins,
            loaded_plugin_types: self.count_plugin_types(),
            total_events_processed,
            total_errors,
            total_memory_usage,
        }
    }

    /// 统计插件类型
    fn count_plugin_types(&self) -> HashMap<PluginType, usize> {
        let mut counts = HashMap::new();

        for instance in self.loaded_plugins.values() {
            *counts.entry(instance.metadata.plugin_type).or_insert(0) += 1;
        }

        counts
    }

    /// 获取插件实例（可变引用）
    pub fn get_plugin_mut(&mut self, plugin_id: &str) -> Option<&mut PluginInstance> {
        self.loaded_plugins.get_mut(plugin_id)
    }

    /// 更新插件配置
    pub async fn update_plugin_config(
        &mut self,
        plugin_id: &str,
        config: HashMap<String, String>,
    ) -> Result<(), VmError> {
        if let Some(instance) = self.loaded_plugins.get_mut(plugin_id) {
            if let Some(ref mut handle) = instance.handle {
                handle.update_config(config).await?;
            }
            Ok(())
        } else {
            Err(VmError::Core(vm_core::CoreError::InvalidState {
                message: format!("Plugin {} not found", plugin_id),
                current: "not_found".to_string(),
                expected: "found".to_string(),
            }))
        }
    }

    /// 重启插件
    pub async fn restart_plugin(&mut self, plugin_id: &str) -> Result<(), VmError> {
        if let Some(instance) = self.loaded_plugins.get_mut(plugin_id) {
            if let Some(ref mut handle) = instance.handle {
                handle.stop().await?;
                handle.start().await?;
                instance.state = PluginState::Active;
            }
            Ok(())
        } else {
            Err(VmError::Core(vm_core::CoreError::InvalidState {
                message: format!("Plugin {} not found", plugin_id),
                current: "not_found".to_string(),
                expected: "found".to_string(),
            }))
        }
    }

    /// 获取插件健康状态
    pub fn get_plugin_health(&self, plugin_id: &str) -> Option<PluginHealth> {
        self.loaded_plugins.get(plugin_id).map(|instance| {
            let uptime = instance.load_time.elapsed().as_secs();
            let error_rate = if instance.stats.events_processed > 0 {
                instance.stats.error_count as f64 / instance.stats.events_processed as f64
            } else {
                0.0
            };

            PluginHealth {
                plugin_id: instance.metadata.id.clone(),
                state: instance.state,
                uptime_seconds: uptime,
                error_rate,
                memory_usage_bytes: instance.stats.memory_usage_bytes,
                events_processed: instance.stats.events_processed,
                is_healthy: instance.state == PluginState::Active && error_rate < 0.1,
            }
        })
    }

    /// 获取所有插件的健康状态
    pub fn get_all_plugin_health(&self) -> Vec<PluginHealth> {
        self.loaded_plugins
            .keys()
            .filter_map(|id| self.get_plugin_health(id))
            .collect()
    }

    /// 启用插件
    pub async fn enable_plugin(&mut self, plugin_id: &str) -> Result<(), VmError> {
        if let Some(instance) = self.loaded_plugins.get_mut(plugin_id) {
            if instance.state == PluginState::Loaded
                && let Some(ref mut handle) = instance.handle {
                    handle.start().await?;
                    instance.state = PluginState::Active;
                }
            Ok(())
        } else {
            Err(VmError::Core(vm_core::CoreError::InvalidState {
                message: format!("Plugin {} not found", plugin_id),
                current: "not_found".to_string(),
                expected: "found".to_string(),
            }))
        }
    }

    /// 禁用插件
    pub async fn disable_plugin(&mut self, plugin_id: &str) -> Result<(), VmError> {
        if let Some(instance) = self.loaded_plugins.get_mut(plugin_id) {
            if instance.state == PluginState::Active
                && let Some(ref mut handle) = instance.handle {
                    handle.stop().await?;
                    instance.state = PluginState::Loaded;
                }
            Ok(())
        } else {
            Err(VmError::Core(vm_core::CoreError::InvalidState {
                message: format!("Plugin {} not found", plugin_id),
                current: "not_found".to_string(),
                expected: "found".to_string(),
            }))
        }
    }
}

/// 插件健康状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginHealth {
    /// 插件ID
    pub plugin_id: PluginId,
    /// 插件状态
    pub state: PluginState,
    /// 运行时间（秒）
    pub uptime_seconds: u64,
    /// 错误率（错误数 / 处理的事件数）
    pub error_rate: f64,
    /// 内存使用（字节）
    pub memory_usage_bytes: u64,
    /// 处理的事件数
    pub events_processed: u64,
    /// 是否健康（状态为Active且错误率 < 10%）
    pub is_healthy: bool,
}

