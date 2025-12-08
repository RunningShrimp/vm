//! # 插件系统测试
//!
//! 包含插件系统各个组件的单元测试和集成测试。

use crate::{
    config::{ConfigManagerConfig, ConfigFormat, FileConfigStorage, PluginConfig, PluginConfigManager, DefaultConfigValidator},
    dependency::{DependencyManager, DependencyResolver},
    extension_points::{ExtensionPointManager, ExtensionPointRegistry, ExtensionContext},
    loader::{PluginLoader, PluginLoaderConfig},
    plugin_interface::{Plugin, PluginMetadata, PluginInfo, PluginVersion, PluginType},
    plugin_registry::{PluginRegistry, PluginRegistryConfig},
    plugin_sandbox::{PluginSandbox, SandboxConfig, SandboxPolicy},
    security::{SecurityManager, SecurityPolicy},
    PluginId, PluginManager, PluginManagerConfig,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use vm_core::VmError;

/// 创建测试用的插件元数据
pub fn create_test_plugin_metadata(id: &str, version: &str) -> PluginMetadata {
    PluginMetadata {
        id: PluginId::from(id),
        name: format!("Test Plugin {}", id),
        version: PluginVersion::parse(version).unwrap(),
        description: Some(format!("Test plugin for {}", id)),
        authors: vec!["Test Author".to_string()],
        license: Some("MIT".to_string()),
        homepage: None,
        plugin_type: PluginType::Library,
        entry_point: PathBuf::from("test_plugin.so"),
        dependencies: vec![],
        optional_dependencies: vec![],
        provides: vec![],
        conflicts: vec![],
        minimum_vm_version: None,
        maximum_vm_version: None,
        tags: vec!["test".to_string()],
        metadata: HashMap::new(),
    }
}

/// 创建测试用的插件配置
pub fn create_test_plugin_config(id: &str) -> PluginConfig {
    PluginConfig {
        plugin_id: PluginId::from(id),
        version: 1,
        data: HashMap::from([
            ("enabled".to_string(), crate::config::ConfigValue::Bool(true)),
            ("priority".to_string(), crate::config::ConfigValue::Integer(5)),
            ("settings".to_string(), crate::config::ConfigValue::Object(HashMap::from([
                ("timeout".to_string(), crate::config::ConfigValue::Integer(30)),
                ("retries".to_string(), crate::config::ConfigValue::Integer(3)),
            ]))),
        ]),
        metadata: crate::config::ConfigMetadata::default(),
    }
}

/// 创建测试用的临时目录
pub fn create_temp_dir() -> TempDir {
    tempfile::tempdir().expect("Failed to create temp dir")
}

/// 测试用的简单插件实现
#[derive(Debug)]
pub struct TestPlugin {
    id: PluginId,
    name: String,
    version: PluginVersion,
    initialized: Arc<Mutex<bool>>,
    started: Arc<Mutex<bool>>,
}

impl TestPlugin {
    pub fn new(id: &str, name: &str, version: &str) -> Self {
        Self {
            id: PluginId::from(id),
            name: name.to_string(),
            version: PluginVersion::parse(version).unwrap(),
            initialized: Arc::new(Mutex::new(false)),
            started: Arc::new(Mutex::new(false)),
        }
    }

    pub fn is_initialized(&self) -> bool {
        *self.initialized.lock().unwrap()
    }

    pub fn is_started(&self) -> bool {
        *self.started.lock().unwrap()
    }
}

impl Plugin for TestPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            id: self.id.clone(),
            name: self.name.clone(),
            version: self.version.clone(),
            description: Some("Test plugin implementation".to_string()),
            authors: vec!["Test Author".to_string()],
            license: Some("MIT".to_string()),
            homepage: None,
        }
    }

    fn initialize(&mut self) -> Result<(), VmError> {
        *self.initialized.lock().unwrap() = true;
        Ok(())
    }

    fn start(&mut self) -> Result<(), VmError> {
        if !self.is_initialized() {
            return Err(VmError::PluginError("Plugin not initialized".to_string()));
        }
        *self.started.lock().unwrap() = true;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), VmError> {
        *self.started.lock().unwrap() = false;
        Ok(())
    }

    fn cleanup(&mut self) -> Result<(), VmError> {
        *self.initialized.lock().unwrap() = false;
        *self.started.lock().unwrap() = false;
        Ok(())
    }

    fn handle_event(&mut self, event_type: &str, event_data: &[u8]) -> Result<Vec<u8>, VmError> {
        match event_type {
            "test" => Ok(format!("Test plugin {} handled event", self.id.as_str()).into_bytes()),
            _ => Err(VmError::PluginError(format!("Unknown event type: {}", event_type))),
        }
    }

    fn get_state(&self) -> Result<Vec<u8>, VmError> {
        let state = format!(
            "Plugin {}: initialized={}, started={}",
            self.id.as_str(),
            self.is_initialized(),
            self.is_started()
        );
        Ok(state.into_bytes())
    }

    fn set_state(&mut self, state_data: &[u8]) -> Result<(), VmError> {
        let state = String::from_utf8(state_data.to_vec())
            .map_err(|_| VmError::PluginError("Invalid state data".to_string()))?;
        
        // 简单的状态解析
        if state.contains("initialized=true") {
            *self.initialized.lock().unwrap() = true;
        }
        if state.contains("started=true") {
            *self.started.lock().unwrap() = true;
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod plugin_tests {
    use super::*;

    #[test]
    fn test_plugin_creation() {
        let plugin = TestPlugin::new("test", "Test Plugin", "1.0.0");
        assert_eq!(plugin.id.as_str(), "test");
        assert_eq!(plugin.name, "Test Plugin");
        assert_eq!(plugin.version.to_string(), "1.0.0");
        assert!(!plugin.is_initialized());
        assert!(!plugin.is_started());
    }

    #[test]
    fn test_plugin_lifecycle() {
        let mut plugin = TestPlugin::new("test", "Test Plugin", "1.0.0");
        
        // 初始化
        assert!(plugin.initialize().is_ok());
        assert!(plugin.is_initialized());
        assert!(!plugin.is_started());
        
        // 启动
        assert!(plugin.start().is_ok());
        assert!(plugin.is_initialized());
        assert!(plugin.is_started());
        
        // 停止
        assert!(plugin.stop().is_ok());
        assert!(plugin.is_initialized());
        assert!(!plugin.is_started());
        
        // 清理
        assert!(plugin.cleanup().is_ok());
        assert!(!plugin.is_initialized());
        assert!(!plugin.is_started());
    }

    #[test]
    fn test_plugin_event_handling() {
        let mut plugin = TestPlugin::new("test", "Test Plugin", "1.0.0");
        assert!(plugin.initialize().is_ok());
        assert!(plugin.start().is_ok());
        
        let result = plugin.handle_event("test", b"test data");
        assert!(result.is_ok());
        assert_eq!(
            String::from_utf8(result.unwrap()).unwrap(),
            "Test plugin test handled event"
        );
    }

    #[test]
    fn test_plugin_state_management() {
        let mut plugin = TestPlugin::new("test", "Test Plugin", "1.0.0");
        
        // 获取初始状态
        let state = plugin.get_state().unwrap();
        let state_str = String::from_utf8(state).unwrap();
        assert!(state_str.contains("initialized=false"));
        assert!(state_str.contains("started=false"));
        
        // 初始化并启动
        assert!(plugin.initialize().is_ok());
        assert!(plugin.start().is_ok());
        
        // 获取更新后状态
        let state = plugin.get_state().unwrap();
        let state_str = String::from_utf8(state).unwrap();
        assert!(state_str.contains("initialized=true"));
        assert!(state_str.contains("started=true"));
        
        // 创建新插件并恢复状态
        let mut new_plugin = TestPlugin::new("test", "Test Plugin", "1.0.0");
        assert!(new_plugin.set_state(&state).is_ok());
        assert!(new_plugin.is_initialized());
        assert!(new_plugin.is_started());
    }
}

#[cfg(test)]
mod plugin_registry_tests {
    use super::*;

    #[tokio::test]
    async fn test_plugin_registry_registration() {
        let temp_dir = create_temp_dir();
        let config = PluginRegistryConfig {
            plugin_dirs: vec![temp_dir.path().to_path_buf()],
            auto_discover: false,
            enable_indexing: true,
            cache_enabled: true,
        };
        
        let registry = PluginRegistry::new(config);
        
        // 创建测试插件元数据
        let metadata = create_test_plugin_metadata("test", "1.0.0");
        
        // 注册插件
        let result = registry.register_plugin(metadata.clone()).await;
        assert!(result.is_ok());
        
        // 检查插件是否已注册
        let is_registered = registry.is_plugin_registered(&metadata.id).await;
        assert!(is_registered);
        
        // 获取插件元数据
        let retrieved_metadata = registry.get_plugin_metadata(&metadata.id).await;
        assert!(retrieved_metadata.is_ok());
        let retrieved_metadata = retrieved_metadata.unwrap();
        assert_eq!(retrieved_metadata.id, metadata.id);
        assert_eq!(retrieved_metadata.name, metadata.name);
        assert_eq!(retrieved_metadata.version, metadata.version);
    }

    #[tokio::test]
    async fn test_plugin_registry_search() {
        let temp_dir = create_temp_dir();
        let config = PluginRegistryConfig {
            plugin_dirs: vec![temp_dir.path().to_path_buf()],
            auto_discover: false,
            enable_indexing: true,
            cache_enabled: true,
        };
        
        let registry = PluginRegistry::new(config);
        
        // 注册多个插件
        let metadata1 = create_test_plugin_metadata("test1", "1.0.0");
        let metadata2 = create_test_plugin_metadata("test2", "2.0.0");
        let metadata3 = create_test_plugin_metadata("other", "1.0.0");
        
        registry.register_plugin(metadata1).await.unwrap();
        registry.register_plugin(metadata2).await.unwrap();
        registry.register_plugin(metadata3).await.unwrap();
        
        // 搜索插件
        let results = registry.search_plugins("test").await.unwrap();
        assert_eq!(results.len(), 2);
        
        let results = registry.search_plugins("other").await.unwrap();
        assert_eq!(results.len(), 1);
        
        let results = registry.search_plugins("nonexistent").await.unwrap();
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_plugin_registry_unregistration() {
        let temp_dir = create_temp_dir();
        let config = PluginRegistryConfig {
            plugin_dirs: vec![temp_dir.path().to_path_buf()],
            auto_discover: false,
            enable_indexing: true,
            cache_enabled: true,
        };
        
        let registry = PluginRegistry::new(config);
        
        // 注册插件
        let metadata = create_test_plugin_metadata("test", "1.0.0");
        registry.register_plugin(metadata.clone()).await.unwrap();
        
        // 验证插件已注册
        assert!(registry.is_plugin_registered(&metadata.id).await);
        
        // 注销插件
        let result = registry.unregister_plugin(&metadata.id).await;
        assert!(result.is_ok());
        
        // 验证插件已注销
        assert!(!registry.is_plugin_registered(&metadata.id).await);
    }
}

#[cfg(test)]
mod plugin_loader_tests {
    use super::*;

    #[tokio::test]
    async fn test_plugin_loader_config() {
        let config = PluginLoaderConfig {
            enable_sandbox: true,
            enable_hot_reload: false,
            max_load_time: 30,
            security_policy: SecurityPolicy::default(),
        };
        
        let loader = PluginLoader::new(config);
        assert!(loader.config().enable_sandbox);
        assert!(!loader.config().enable_hot_reload);
        assert_eq!(loader.config().max_load_time, 30);
    }

    #[tokio::test]
    async fn test_plugin_loader_statistics() {
        let config = PluginLoaderConfig::default();
        let loader = PluginLoader::new(config);
        
        let stats = loader.get_statistics().await;
        assert_eq!(stats.total_loaded, 0);
        assert_eq!(stats.total_failed, 0);
        assert_eq!(stats.currently_loaded, 0);
    }
}

#[cfg(test)]
mod plugin_sandbox_tests {
    use super::*;

    #[test]
    fn test_sandbox_config() {
        let config = SandboxConfig {
            enable_memory_limit: true,
            memory_limit_mb: 100,
            enable_cpu_limit: true,
            cpu_limit_percent: 50.0,
            enable_file_access_restrictions: true,
            allowed_paths: vec![PathBuf::from("/tmp")],
            enable_network_restrictions: true,
            allowed_network_hosts: vec![],
            enable_syscall_filtering: true,
            allowed_syscalls: vec!["read".to_string(), "write".to_string()],
            enable_time_limit: true,
            time_limit_seconds: 60,
            policy: SandboxPolicy::Restricted,
        };
        
        assert!(config.enable_memory_limit);
        assert_eq!(config.memory_limit_mb, 100);
        assert!(config.enable_cpu_limit);
        assert_eq!(config.cpu_limit_percent, 50.0);
    }

    #[test]
    fn test_sandbox_creation() {
        let config = SandboxConfig::default();
        let sandbox = PluginSandbox::new(config);
        
        let stats = sandbox.get_statistics();
        assert_eq!(stats.active_sandboxes, 0);
        assert_eq!(stats.total_created, 0);
        assert_eq!(stats.total_destroyed, 0);
    }
}

#[cfg(test)]
mod extension_points_tests {
    use super::*;

    #[tokio::test]
    async fn test_extension_point_registration() {
        let registry = ExtensionPointRegistry::new();
        let manager = ExtensionPointManager::new(registry);
        
        // 注册扩展点
        let result = manager.register_extension_point("test_point", "Test extension point").await;
        assert!(result.is_ok());
        
        // 检查扩展点是否已注册
        let is_registered = manager.is_extension_point_registered("test_point").await;
        assert!(is_registered);
        
        // 获取扩展点信息
        let info = manager.get_extension_point_info("test_point").await;
        assert!(info.is_ok());
        let info = info.unwrap();
        assert_eq!(info.name, "test_point");
        assert_eq!(info.description, "Test extension point");
    }

    #[tokio::test]
    async fn test_extension_execution() {
        let registry = ExtensionPointRegistry::new();
        let manager = ExtensionPointManager::new(registry);
        
        // 注册扩展点
        manager.register_extension_point("test_point", "Test extension point").await.unwrap();
        
        // 创建测试扩展
        let extension = crate::extension_points::Extension::new(
            "test_extension",
            "Test extension",
            "1.0.0",
            Box::new(|ctx| {
                let mut result = HashMap::new();
                result.insert("result".to_string(), "test_result".to_string());
                Ok(result)
            }),
        );
        
        // 注册扩展
        manager.register_extension("test_point", extension).await.unwrap();
        
        // 执行扩展点
        let context = ExtensionContext::new(HashMap::new());
        let results = manager.execute_extension_point("test_point", &context).await.unwrap();
        
        assert_eq!(results.len(), 1);
        assert!(results.contains_key("test_extension"));
        
        let extension_result = &results["test_extension"];
        assert!(extension_result.success);
        assert_eq!(extension_result.data.get("result"), Some(&"test_result".to_string()));
    }
}

#[cfg(test)]
mod dependency_tests {
    use super::*;

    #[tokio::test]
    async fn test_dependency_resolution() {
        let manager = DependencyManager::new();
        let resolver = DependencyResolver::new();
        
        // 创建测试依赖
        let plugin_a = create_test_plugin_metadata("plugin_a", "1.0.0");
        let plugin_b = create_test_plugin_metadata("plugin_b", "1.0.0");
        
        // 设置依赖关系：plugin_b 依赖 plugin_a
        let mut plugin_b_with_deps = plugin_b.clone();
        plugin_b_with_deps.dependencies = vec![crate::dependency::Dependency {
            id: plugin_a.id.clone(),
            version_requirement: crate::dependency::VersionRequirement::exact("1.0.0"),
            optional: false,
        }];
        
        // 注册插件
        manager.add_dependency(&plugin_b_with_deps.id, plugin_b_with_deps.dependencies.clone()).await.unwrap();
        manager.add_dependency(&plugin_a.id, plugin_a.dependencies.clone()).await.unwrap();
        
        // 解析依赖
        let resolution = resolver.resolve_dependencies(&plugin_b_with_deps.id, &manager).await;
        assert!(resolution.is_ok());
        
        let resolution = resolution.unwrap();
        assert_eq!(resolution.resolved_order.len(), 2);
        assert_eq!(resolution.resolved_order[0], plugin_a.id);
        assert_eq!(resolution.resolved_order[1], plugin_b.id);
    }

    #[tokio::test]
    async fn test_circular_dependency_detection() {
        let manager = DependencyManager::new();
        let resolver = DependencyResolver::new();
        
        // 创建循环依赖：plugin_a 依赖 plugin_b，plugin_b 依赖 plugin_a
        let plugin_a = create_test_plugin_metadata("plugin_a", "1.0.0");
        let plugin_b = create_test_plugin_metadata("plugin_b", "1.0.0");
        
        let mut plugin_a_with_deps = plugin_a.clone();
        plugin_a_with_deps.dependencies = vec![crate::dependency::Dependency {
            id: plugin_b.id.clone(),
            version_requirement: crate::dependency::VersionRequirement::exact("1.0.0"),
            optional: false,
        }];
        
        let mut plugin_b_with_deps = plugin_b.clone();
        plugin_b_with_deps.dependencies = vec![crate::dependency::Dependency {
            id: plugin_a.id.clone(),
            version_requirement: crate::dependency::VersionRequirement::exact("1.0.0"),
            optional: false,
        }];
        
        // 注册插件
        manager.add_dependency(&plugin_a_with_deps.id, plugin_a_with_deps.dependencies.clone()).await.unwrap();
        manager.add_dependency(&plugin_b_with_deps.id, plugin_b_with_deps.dependencies.clone()).await.unwrap();
        
        // 尝试解析依赖（应该检测到循环依赖）
        let resolution = resolver.resolve_dependencies(&plugin_a_with_deps.id, &manager).await;
        assert!(resolution.is_err());
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_config_storage() {
        let temp_dir = create_temp_dir();
        let manager_config = ConfigManagerConfig::default();
        let storage = FileConfigStorage::new(temp_dir.path().to_path_buf(), ConfigFormat::Json, manager_config);
        
        let plugin_id = PluginId::from("test");
        let config = create_test_plugin_config("test");
        
        // 保存配置
        let result = storage.save_config(&plugin_id, &config).await;
        assert!(result.is_ok());
        
        // 加载配置
        let result = storage.load_config(&plugin_id).await;
        assert!(result.is_ok());
        assert!(result.is_some());
        
        let loaded_config = result.unwrap();
        assert_eq!(loaded_config.plugin_id, plugin_id);
        assert_eq!(loaded_config.version, config.version);
        assert_eq!(loaded_config.data.len(), config.data.len());
    }

    #[tokio::test]
    async fn test_config_validation() {
        let validator = DefaultConfigValidator::new();
        let plugin_id = PluginId::from("test");
        let config = create_test_plugin_config("test");
        
        // 验证配置
        let result = validator.validate_config(&plugin_id, &config).await;
        assert!(result.is_ok());
        
        let validation_result = result.unwrap();
        assert!(validation_result.is_valid);
        assert!(validation_result.errors.is_empty());
    }

    #[tokio::test]
    async fn test_config_manager() {
        let temp_dir = create_temp_dir();
        let manager_config = ConfigManagerConfig::default();
        let storage = Arc::new(FileConfigStorage::new(temp_dir.path().to_path_buf(), ConfigFormat::Json, manager_config.clone()));
        let validator = Arc::new(DefaultConfigValidator::new());
        
        let config_manager = PluginConfigManager::new(storage, validator, manager_config);
        
        let plugin_id = PluginId::from("test");
        let config = create_test_plugin_config("test");
        
        // 设置配置
        let result = config_manager.set_plugin_config(&plugin_id, config.clone()).await;
        assert!(result.is_ok());
        
        // 获取配置
        let result = config_manager.get_plugin_config(&plugin_id).await;
        assert!(result.is_ok());
        
        let loaded_config = result.unwrap();
        assert_eq!(loaded_config.plugin_id, plugin_id);
        assert_eq!(loaded_config.version, config.version);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_plugin_manager_integration() {
        let temp_dir = create_temp_dir();
        
        // 创建插件管理器配置
        let manager_config = PluginManagerConfig {
            enable_hot_reload: false,
            enable_sandbox: true,
            plugin_dirs: vec![temp_dir.path().to_path_buf()],
            max_plugins: 10,
            security_policy: SecurityPolicy::default(),
            auto_discover: false,
        };
        
        // 创建插件管理器
        let mut plugin_manager = PluginManager::new(manager_config);
        
        // 初始化插件管理器
        let result = plugin_manager.initialize().await;
        assert!(result.is_ok());
        
        // 创建测试插件
        let plugin = Box::new(TestPlugin::new("test", "Test Plugin", "1.0.0"));
        
        // 加载插件
        let result = plugin_manager.load_plugin(plugin).await;
        assert!(result.is_ok());
        
        // 检查插件是否已加载
        let is_loaded = plugin_manager.is_plugin_loaded(&PluginId::from("test")).await;
        assert!(is_loaded);
        
        // 启动插件
        let result = plugin_manager.start_plugin(&PluginId::from("test")).await;
        assert!(result.is_ok());
        
        // 停止插件
        let result = plugin_manager.stop_plugin(&PluginId::from("test")).await;
        assert!(result.is_ok());
        
        // 卸载插件
        let result = plugin_manager.unload_plugin(&PluginId::from("test")).await;
        assert!(result.is_ok());
        
        // 检查插件是否已卸载
        let is_loaded = plugin_manager.is_plugin_loaded(&PluginId::from("test")).await;
        assert!(!is_loaded);
    }

    #[tokio::test]
    async fn test_plugin_system_end_to_end() {
        let temp_dir = create_temp_dir();
        
        // 创建完整的插件系统
        let manager_config = PluginManagerConfig {
            enable_hot_reload: true,
            enable_sandbox: true,
            plugin_dirs: vec![temp_dir.path().to_path_buf()],
            max_plugins: 10,
            security_policy: SecurityPolicy::default(),
            auto_discover: true,
        };
        
        let mut plugin_manager = PluginManager::new(manager_config);
        
        // 初始化系统
        assert!(plugin_manager.initialize().await.is_ok());
        
        // 创建多个插件
        let plugin1 = Box::new(TestPlugin::new("plugin1", "Plugin 1", "1.0.0"));
        let plugin2 = Box::new(TestPlugin::new("plugin2", "Plugin 2", "1.0.0"));
        
        // 加载插件
        assert!(plugin_manager.load_plugin(plugin1).await.is_ok());
        assert!(plugin_manager.load_plugin(plugin2).await.is_ok());
        
        // 启动所有插件
        assert!(plugin_manager.start_all_plugins().await.is_ok());
        
        // 检查插件状态
        let stats = plugin_manager.get_statistics().await;
        assert_eq!(stats.total_loaded, 2);
        assert_eq!(stats.total_running, 2);
        
        // 发送事件到所有插件
        let event_results = plugin_manager.broadcast_event("test", b"test data").await;
        assert_eq!(event_results.len(), 2);
        
        // 停止所有插件
        assert!(plugin_manager.stop_all_plugins().await.is_ok());
        
        // 卸载所有插件
        assert!(plugin_manager.unload_all_plugins().await.is_ok());
        
        // 检查最终状态
        let stats = plugin_manager.get_statistics().await;
        assert_eq!(stats.total_loaded, 0);
        assert_eq!(stats.total_running, 0);
    }
}