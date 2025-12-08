//! # 示例插件
//!
//! 这是一个完整的示例插件，展示了如何使用VM插件系统开发插件。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use vm_plugin::{
    config::{ConfigChangeListener, ConfigChangeEvent, ConfigValue},
    extension_points::{ExtensionContext, ExtensionResult},
    plugin_interface::{Plugin, PluginInfo, PluginMetadata, PluginType, PluginVersion},
    PluginId, PluginContext, PluginEvent, PluginStatus, PluginState,
};
use vm_core::VmError;

/// 示例插件实现
///
/// 这个插件展示了插件系统的各种功能：
/// - 基本的生命周期管理
/// - 事件处理
/// - 配置管理
/// - 扩展点实现
/// - 状态管理
pub struct ExamplePlugin {
    /// 插件信息
    info: PluginInfo,
    /// 插件状态
    state: Arc<Mutex<PluginState>>,
    /// 插件配置
    config: Arc<Mutex<HashMap<String, String>>>,
    /// 统计信息
    stats: Arc<Mutex<PluginStats>>,
    /// 事件计数器
    event_counter: Arc<Mutex<u64>>,
    /// 初始化时间
    init_time: Arc<Mutex<Option<std::time::Instant>>>,
}

/// 插件统计信息
#[derive(Debug, Clone, Default)]
pub struct PluginStats {
    /// 处理的事件数
    pub events_processed: u64,
    /// 错误数
    pub error_count: u64,
    /// 最后活动时间
    pub last_activity: Option<std::time::Instant>,
}

impl ExamplePlugin {
    /// 创建新的示例插件
    pub fn new() -> Self {
        Self {
            info: PluginInfo {
                id: PluginId::from("example_plugin"),
                name: "Example Plugin".to_string(),
                version: PluginVersion { major: 1, minor: 0, patch: 0 },
                description: Some("An example plugin demonstrating the VM plugin system".to_string()),
                authors: vec!["VM Plugin Team".to_string()],
                license: Some("MIT".to_string()),
                homepage: Some("https://github.com/vm-project/example-plugin".to_string()),
            },
            state: Arc::new(Mutex::new(PluginState::Unloaded)),
            config: Arc::new(Mutex::new(HashMap::new())),
            stats: Arc::new(Mutex::new(PluginStats::default())),
            event_counter: Arc::new(Mutex::new(0)),
            init_time: Arc::new(Mutex::new(None)),
        }
    }

    /// 获取插件统计信息
    pub fn get_stats(&self) -> PluginStats {
        self.stats.lock().unwrap().clone()
    }

    /// 重置统计信息
    pub fn reset_stats(&self) {
        let mut stats = self.stats.lock().unwrap();
        stats.events_processed = 0;
        stats.error_count = 0;
        stats.last_activity = None;
    }

    /// 处理自定义事件
    fn handle_custom_event(&self, event_type: &str, data: &[u8]) -> Result<Vec<u8>, VmError> {
        match event_type {
            "example.ping" => {
                let response = format!("Pong from example plugin at {:?}", std::time::SystemTime::now());
                Ok(response.into_bytes())
            }
            "example.echo" => {
                // 简单的回显
                Ok(data.to_vec())
            }
            "example.stats" => {
                let stats = self.get_stats();
                let response = format!(
                    "Events: {}, Errors: {}, Last Activity: {:?}",
                    stats.events_processed,
                    stats.error_count,
                    stats.last_activity
                );
                Ok(response.into_bytes())
            }
            _ => Err(VmError::PluginError(format!("Unknown custom event: {}", event_type))),
        }
    }

    /// 更新统计信息
    fn update_stats(&self, success: bool) {
        let mut stats = self.stats.lock().unwrap();
        stats.events_processed += 1;
        if !success {
            stats.error_count += 1;
        }
        stats.last_activity = Some(std::time::Instant::now());
    }

    /// 获取运行时间
    fn get_uptime(&self) -> Option<std::time::Duration> {
        if let Some(init_time) = *self.init_time.lock().unwrap() {
            Some(init_time.elapsed())
        } else {
            None
        }
    }
}

impl Plugin for ExamplePlugin {
    /// 获取插件信息
    fn info(&self) -> PluginInfo {
        self.info.clone()
    }

    /// 初始化插件
    async fn initialize(&mut self, context: &PluginContext) -> Result<(), VmError> {
        // 更新状态
        *self.state.lock().unwrap() = PluginState::Initializing;

        // 从上下文加载配置
        let mut config = self.config.lock().unwrap();
        for (key, value) in &context.config {
            config.insert(key.clone(), value.clone());
        }

        // 设置默认配置
        if !config.contains_key("log_level") {
            config.insert("log_level".to_string(), "info".to_string());
        }
        if !config.contains_key("max_events") {
            config.insert("max_events".to_string(), "1000".to_string());
        }

        // 记录初始化时间
        *self.init_time.lock().unwrap() = Some(std::time::Instant::now());

        // 更新状态为已加载
        *self.state.lock().unwrap() = PluginState::Loaded;

        println!("Example plugin initialized successfully");
        Ok(())
    }

    /// 启动插件
    async fn start(&mut self) -> Result<(), VmError> {
        // 检查状态
        let current_state = *self.state.lock().unwrap();
        if current_state != PluginState::Loaded {
            return Err(VmError::PluginError(format!(
                "Cannot start plugin in state: {:?}",
                current_state
            )));
        }

        // 更新状态
        *self.state.lock().unwrap() = PluginState::Active;

        println!("Example plugin started successfully");
        Ok(())
    }

    /// 停止插件
    async fn stop(&mut self) -> Result<(), VmError> {
        // 检查状态
        let current_state = *self.state.lock().unwrap();
        if current_state != PluginState::Active {
            return Err(VmError::PluginError(format!(
                "Cannot stop plugin in state: {:?}",
                current_state
            )));
        }

        // 更新状态
        *self.state.lock().unwrap() = PluginState::Loaded;

        println!("Example plugin stopped successfully");
        Ok(())
    }

    /// 处理事件
    async fn handle_event(&mut self, event: &PluginEvent) -> Result<(), VmError> {
        let mut success = true;
        let result = match event {
            PluginEvent::VmStarted { vm_id } => {
                println!("Example plugin: VM {} started", vm_id);
                Ok(())
            }
            PluginEvent::VmStopped { vm_id } => {
                println!("Example plugin: VM {} stopped", vm_id);
                Ok(())
            }
            PluginEvent::DeviceAdded { device_id, device_type } => {
                println!("Example plugin: Device {} of type {} added", device_id, device_type);
                Ok(())
            }
            PluginEvent::DeviceRemoved { device_id } => {
                println!("Example plugin: Device {} removed", device_id);
                Ok(())
            }
            PluginEvent::PerformanceUpdate { metric, value } => {
                println!("Example plugin: Performance update - {}: {}", metric, value);
                Ok(())
            }
            PluginEvent::Custom { event_type, data } => {
                // 处理自定义事件
                let data_bytes = serde_json::to_vec(data).unwrap_or_default();
                match self.handle_custom_event(event_type, &data_bytes) {
                    Ok(_) => {
                        println!("Example plugin: Custom event {} handled successfully", event_type);
                        Ok(())
                    }
                    Err(e) => {
                        eprintln!("Example plugin: Error handling custom event {}: {}", event_type, e);
                        success = false;
                        Err(e)
                    }
                }
            }
        };

        // 更新事件计数器和统计信息
        {
            let mut counter = self.event_counter.lock().unwrap();
            *counter += 1;
        }
        self.update_stats(success);

        result
    }

    /// 获取插件状态
    fn status(&self) -> PluginStatus {
        let state = *self.state.lock().unwrap();
        let stats = self.stats.lock().unwrap();
        let uptime = self.get_uptime()
            .map(|d| d.as_secs())
            .unwrap_or(0);

        PluginStatus {
            state,
            uptime_seconds: uptime,
            memory_usage_bytes: 0, // 简化实现
            processed_events: stats.events_processed,
            error_count: stats.error_count,
        }
    }

    /// 清理插件资源
    async fn cleanup(&mut self) -> Result<(), VmError> {
        // 更新状态
        *self.state.lock().unwrap() = PluginState::Unloading;

        // 清理资源
        self.reset_stats();
        *self.event_counter.lock().unwrap() = 0;
        *self.init_time.lock().unwrap() = None;

        // 更新状态为未加载
        *self.state.lock().unwrap() = PluginState::Unloaded;

        println!("Example plugin cleaned up successfully");
        Ok(())
    }

    /// 获取插件配置
    fn get_config(&self) -> Option<&HashMap<String, String>> {
        Some(&self.config.lock().unwrap())
    }

    /// 更新插件配置
    async fn update_config(&mut self, config: HashMap<String, String>) -> Result<(), VmError> {
        let mut current_config = self.config.lock().unwrap();
        *current_config = config;
        println!("Example plugin configuration updated");
        Ok(())
    }
}

/// 示例插件配置变更监听器
pub struct ExampleConfigChangeListener {
    plugin_id: PluginId,
}

impl ExampleConfigChangeListener {
    pub fn new(plugin_id: PluginId) -> Self {
        Self { plugin_id }
    }
}

impl ConfigChangeListener for ExampleConfigChangeListener {
    fn on_config_changed(&self, event: &ConfigChangeEvent) {
        if event.plugin_id != self.plugin_id {
            return;
        }

        println!(
            "Example plugin: Configuration changed - {:?} at {:?}",
            event.event_type, event.timestamp
        );

        match event.event_type {
            vm_plugin::ConfigChangeType::Updated => {
                println!("Example plugin: Configuration updated");
                // 这里可以根据配置变更执行相应操作
            }
            vm_plugin::ConfigChangeType::Created => {
                println!("Example plugin: Configuration created");
            }
            vm_plugin::ConfigChangeType::Deleted => {
                println!("Example plugin: Configuration deleted");
            }
            _ => {}
        }
    }
}

/// 示例JIT编译器扩展
pub struct ExampleJitExtension;

impl vm_plugin::extension_points::JitCompilerExtension for ExampleJitExtension {
    fn optimize_code(&self, _context: &ExtensionContext, code: &[u8]) -> Result<Vec<u8>, VmError> {
        // 简单的代码优化示例：添加NOP指令
        let mut optimized = code.to_vec();
        optimized.push(0x90); // x86 NOP指令
        println!("Example JIT extension: Optimized {} bytes of code", code.len());
        Ok(optimized)
    }

    fn compile_function(&self, _context: &ExtensionContext, ir: &[u8]) -> Result<Vec<u8>, VmError> {
        // 简单的函数编译示例
        println!("Example JIT extension: Compiling function from {} bytes of IR", ir.len());
        Ok(ir.to_vec())
    }
}

/// 示例GC策略扩展
pub struct ExampleGcExtension;

impl vm_plugin::extension_points::GcStrategyExtension for ExampleGcExtension {
    fn collect_garbage(&self, _context: &ExtensionContext) -> Result<usize, VmError> {
        // 简单的GC实现示例
        println!("Example GC extension: Performing garbage collection");
        Ok(42) // 返回回收的对象数量
    }

    fn should_collect(&self, _context: &ExtensionContext) -> Result<bool, VmError> {
        // 简单的GC触发条件检查
        println!("Example GC extension: Checking if GC should run");
        Ok(true)
    }
}

/// 示例事件处理扩展
pub struct ExampleEventHandlerExtension;

impl vm_plugin::extension_points::EventHandlerExtension for ExampleEventHandlerExtension {
    fn handle_event(&self, _context: &ExtensionContext, event_type: &str, event_data: &[u8]) -> Result<Vec<u8>, VmError> {
        match event_type {
            "example.custom" => {
                let response = format!("Event handled by example extension: {}", String::from_utf8_lossy(event_data));
                Ok(response.into_bytes())
            }
            _ => Ok(vec![]),
        }
    }

    fn register_event_types(&self) -> Vec<String> {
        vec![
            "example.custom".to_string(),
            "example.notification".to_string(),
        ]
    }
}

/// 创建示例插件实例
pub fn create_example_plugin() -> Box<dyn Plugin> {
    Box::new(ExamplePlugin::new())
}

/// 创建示例插件元数据
pub fn create_example_plugin_metadata() -> vm_plugin::plugin_interface::PluginMetadata {
    vm_plugin::plugin_interface::PluginMetadata {
        id: PluginId::from("example_plugin"),
        name: "Example Plugin".to_string(),
        version: PluginVersion { major: 1, minor: 0, patch: 0 },
        description: Some("An example plugin demonstrating the VM plugin system".to_string()),
        authors: vec!["VM Plugin Team".to_string()],
        license: Some("MIT".to_string()),
        homepage: Some("https://github.com/vm-project/example-plugin".to_string()),
        plugin_type: PluginType::Custom,
        entry_point: std::path::PathBuf::from("example_plugin.so"),
        dependencies: vec![],
        optional_dependencies: vec![],
        provides: vec!["example.functionality".to_string()],
        conflicts: vec![],
        minimum_vm_version: None,
        maximum_vm_version: None,
        tags: vec!["example".to_string(), "demo".to_string()],
        metadata: HashMap::from([
            ("category".to_string(), "utility".to_string()),
            ("priority".to_string(), "normal".to_string()),
        ]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_example_plugin_lifecycle() {
        let mut plugin = ExamplePlugin::new();

        // 创建测试上下文
        let context = PluginContext {
            vm_instance_id: "test_vm".to_string(),
            config: HashMap::from([
                ("log_level".to_string(), "debug".to_string()),
                ("max_events".to_string(), "500".to_string()),
            ]),
            shared_data: Arc::new(RwLock::new(HashMap::new())),
            event_bus: Arc::new(RwLock::new(vm_plugin::PluginEventBus::new())),
        };

        // 测试初始化
        assert!(plugin.initialize(&context).await.is_ok());
        assert_eq!(plugin.status().state, PluginState::Loaded);

        // 测试启动
        assert!(plugin.start().await.is_ok());
        assert_eq!(plugin.status().state, PluginState::Active);

        // 测试事件处理
        let vm_started_event = PluginEvent::VmStarted {
            vm_id: "test_vm".to_string(),
        };
        assert!(plugin.handle_event(&vm_started_event).await.is_ok());

        // 测试自定义事件
        let custom_event = PluginEvent::Custom {
            event_type: "example.ping".to_string(),
            data: serde_json::Value::Null,
        };
        assert!(plugin.handle_event(&custom_event).await.is_ok());

        // 检查统计信息
        let stats = plugin.get_stats();
        assert_eq!(stats.events_processed, 2);
        assert_eq!(stats.error_count, 0);

        // 测试停止
        assert!(plugin.stop().await.is_ok());
        assert_eq!(plugin.status().state, PluginState::Loaded);

        // 测试清理
        assert!(plugin.cleanup().await.is_ok());
        assert_eq!(plugin.status().state, PluginState::Unloaded);
    }

    #[test]
    fn test_example_plugin_info() {
        let plugin = ExamplePlugin::new();
        let info = plugin.info();

        assert_eq!(info.id.as_str(), "example_plugin");
        assert_eq!(info.name, "Example Plugin");
        assert_eq!(info.version.to_string(), "1.0.0");
        assert!(info.description.is_some());
        assert!(info.license.is_some());
    }

    #[test]
    fn test_example_plugin_config() {
        let mut plugin = ExamplePlugin::new();

        // 测试默认配置
        let config = plugin.get_config();
        assert!(config.is_none()); // 初始化前没有配置

        // 测试配置更新
        let new_config = HashMap::from([
            ("log_level".to_string(), "warn".to_string()),
            ("timeout".to_string(), "30".to_string()),
        ]);

        // 注意：这个测试需要异步运行时
        // 在实际使用中，配置更新应该在插件初始化后进行
    }

    #[test]
    fn test_example_config_change_listener() {
        let listener = ExampleConfigChangeListener::new(PluginId::from("example_plugin"));
        
        // 创建配置变更事件
        let event = vm_plugin::config::ConfigChangeEvent {
            id: "test_change".to_string(),
            plugin_id: PluginId::from("example_plugin"),
            event_type: vm_plugin::ConfigChangeType::Updated,
            old_config: None,
            new_config: vm_plugin::config::PluginConfig {
                plugin_id: PluginId::from("example_plugin"),
                version: 1,
                data: HashMap::from([
                    ("key".to_string(), vm_plugin::config::ConfigValue::String("value".to_string())),
                ]),
                metadata: vm_plugin::config::ConfigMetadata::default(),
            },
            timestamp: std::time::SystemTime::now(),
            source: "test".to_string(),
            description: "Test configuration change".to_string(),
            details: HashMap::new(),
        };

        // 测试配置变更监听
        listener.on_config_changed(&event);
    }

    #[test]
    fn test_example_extensions() {
        let context = ExtensionContext::new(HashMap::new());

        // 测试JIT扩展
        let jit_ext = ExampleJitExtension;
        let code = vec![0x01, 0x02, 0x03];
        let result = jit_ext.optimize_code(&context, &code);
        assert!(result.is_ok());
        let optimized = result.unwrap();
        assert_eq!(optimized.len(), code.len() + 1); // 应该添加了一个NOP指令

        // 测试GC扩展
        let gc_ext = ExampleGcExtension;
        let result = gc_ext.collect_garbage(&context);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);

        let result = gc_ext.should_collect(&context);
        assert!(result.is_ok());
        assert!(result.unwrap());

        // 测试事件处理扩展
        let event_ext = ExampleEventHandlerExtension;
        let event_types = event_ext.register_event_types();
        assert!(event_types.contains(&"example.custom".to_string()));

        let result = event_ext.handle_event(&context, "example.custom", b"test data");
        assert!(result.is_ok());
    }
}