//! # 插件系统演示程序
//!
//! 这个示例展示了如何使用VM插件系统，包括：
//! - 插件管理器的创建和配置
//! - 插件的加载、启动和卸载
//! - 扩展点的注册和使用
//! - 配置管理和热更新
//! - 事件处理和插件间通信

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use vm_plugin::{
    config::{ConfigFormat, ConfigManagerConfig, FileConfigStorage, DefaultConfigValidator, PluginConfig, ConfigValue},
    extension_points::{ExtensionPointManager, ExtensionPointRegistry, ExtensionContext, Extension},
    loader::PluginLoaderConfig,
    plugin_interface::{Plugin, PluginType, PluginVersion},
    plugin_registry::PluginRegistryConfig,
    plugin_sandbox::{SandboxConfig, SandboxPolicy},
    security::SecurityPolicy,
    PluginManager, PluginManagerConfig, PluginId, PluginContext, PluginEvent,
};
use vm_core::VmError;

// 导入示例插件
mod example_plugin;
use example_plugin::{
    create_example_plugin, create_example_plugin_metadata, ExampleConfigChangeListener,
    ExampleJitExtension, ExampleGcExtension, ExampleEventHandlerExtension,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== VM Plugin System Demo ===\n");

    // 创建临时目录用于演示
    let temp_dir = TempDir::new()?;
    let plugin_dir = temp_dir.path().join("plugins");
    let config_dir = temp_dir.path().join("config");
    
    std::fs::create_dir_all(&plugin_dir)?;
    std::fs::create_dir_all(&config_dir)?;

    // 1. 创建插件管理器
    println!("1. Creating Plugin Manager...");
    let plugin_manager = create_plugin_manager(&plugin_dir, &config_dir)?;
    println!("✓ Plugin manager created\n");

    // 2. 初始化插件管理器
    println!("2. Initializing Plugin Manager...");
    plugin_manager.initialize().await?;
    println!("✓ Plugin manager initialized\n");

    // 3. 创建和配置示例插件
    println!("3. Creating Example Plugin...");
    let mut example_plugin = create_example_plugin();
    println!("✓ Example plugin created");
    println!("  - ID: {}", example_plugin.info().id.as_str());
    println!("  - Name: {}", example_plugin.info().name);
    println!("  - Version: {}", example_plugin.info().version.to_string());
    println!();

    // 4. 加载插件
    println!("4. Loading Plugin...");
    let plugin_id = example_plugin.info().id.clone();
    plugin_manager.load_plugin(example_plugin).await?;
    println!("✓ Plugin loaded\n");

    // 5. 配置插件
    println!("5. Configuring Plugin...");
    configure_plugin(&plugin_manager, &plugin_id, &config_dir).await?;
    println!("✓ Plugin configured\n");

    // 6. 启动插件
    println!("6. Starting Plugin...");
    plugin_manager.start_plugin(&plugin_id).await?;
    println!("✓ Plugin started\n");

    // 7. 设置扩展点
    println!("7. Setting up Extension Points...");
    setup_extension_points(&plugin_manager).await?;
    println!("✓ Extension points configured\n");

    // 8. 发送事件到插件
    println!("8. Sending Events to Plugin...");
    send_events_to_plugin(&plugin_manager, &plugin_id).await?;
    println!("✓ Events sent\n");

    // 9. 演示配置热更新
    println!("9. Demonstrating Hot Configuration Update...");
    demonstrate_config_update(&plugin_manager, &plugin_id, &config_dir).await?;
    println!("✓ Configuration updated\n");

    // 10. 获取插件状态和统计信息
    println!("10. Getting Plugin Status and Statistics...");
    display_plugin_status(&plugin_manager, &plugin_id).await?;
    println!();

    // 11. 停止和卸载插件
    println!("11. Stopping and Unloading Plugin...");
    plugin_manager.stop_plugin(&plugin_id).await?;
    println!("✓ Plugin stopped");
    plugin_manager.unload_plugin(&plugin_id).await?;
    println!("✓ Plugin unloaded\n");

    // 12. 显示插件管理器统计信息
    println!("12. Plugin Manager Statistics:");
    display_manager_stats(&plugin_manager).await?;

    println!("=== Demo Completed Successfully ===");
    Ok(())
}

/// 创建插件管理器
fn create_plugin_manager(plugin_dir: &PathBuf, config_dir: &PathBuf) -> Result<PluginManager, VmError> {
    let config = PluginManagerConfig {
        enable_hot_reload: true,
        enable_sandbox: true,
        plugin_dirs: vec![plugin_dir.clone()],
        max_plugins: 10,
        security_policy: SecurityPolicy {
            allow_file_access: true,
            allow_network_access: false,
            allow_syscalls: vec!["read".to_string(), "write".to_string()],
            memory_limit_mb: 100,
            cpu_limit_percent: 50.0,
        },
        auto_discover: false,
    };

    Ok(PluginManager::new(config))
}

/// 配置插件
async fn configure_plugin(
    plugin_manager: &PluginManager,
    plugin_id: &PluginId,
    config_dir: &PathBuf,
) -> Result<(), VmError> {
    // 创建配置管理器
    let storage_config = vm_plugin::config::ConfigManagerConfig {
        enable_cache: true,
        cache_ttl: 300,
        enable_hot_reload: true,
        watch_interval: 1000,
        enable_backup: true,
        backup_retention: 5,
        config_format: ConfigFormat::Json,
    };

    let storage = Arc::new(FileConfigStorage::new(
        config_dir.to_path_buf(),
        ConfigFormat::Json,
        storage_config.clone(),
    ));

    let validator = Arc::new(DefaultConfigValidator::new());
    let config_manager = Arc::new(PluginConfigManager::new(
        storage,
        validator,
        storage_config,
    ));

    // 创建插件配置
    let plugin_config = PluginConfig {
        plugin_id: plugin_id.clone(),
        version: 1,
        data: HashMap::from([
            ("enabled".to_string(), ConfigValue::Bool(true)),
            ("log_level".to_string(), ConfigValue::String("info".to_string())),
            ("max_events".to_string(), ConfigValue::Integer(1000)),
            ("timeout".to_string(), ConfigValue::Integer(30)),
            ("settings".to_string(), ConfigValue::Object(HashMap::from([
                ("retry_count".to_string(), ConfigValue::Integer(3)),
                ("debug_mode".to_string(), ConfigValue::Bool(false)),
            ]))),
        ]),
        metadata: vm_plugin::config::ConfigMetadata {
            created_at: std::time::SystemTime::now(),
            updated_at: std::time::SystemTime::now(),
            created_by: "demo".to_string(),
            updated_by: "demo".to_string(),
            description: Some("Demo configuration for example plugin".to_string()),
            tags: vec!["demo".to_string(), "example".to_string()],
            schema_version: "1.0".to_string(),
            checksum: None,
        },
    };

    // 保存配置
    config_manager.set_plugin_config(plugin_id, plugin_config).await
        .map_err(|e| VmError::PluginError(e.to_string()))?;

    // 添加配置变更监听器
    let listener = Box::new(ExampleConfigChangeListener::new(plugin_id.clone()));
    config_manager.add_config_change_listener(listener);

    Ok(())
}

/// 设置扩展点
async fn setup_extension_points(plugin_manager: &PluginManager) -> Result<(), VmError> {
    // 创建扩展点注册表和管理器
    let registry = ExtensionPointRegistry::new();
    let extension_manager = ExtensionPointManager::new(registry);

    // 注册扩展点
    extension_manager.register_extension_point("jit.optimize", "JIT compilation optimization").await?;
    extension_manager.register_extension_point("gc.collect", "Garbage collection strategy").await?;
    extension_manager.register_extension_point("event.handle", "Event handling").await?;

    // 注册JIT扩展
    let jit_extension = Extension::new(
        "example_jit",
        "Example JIT Extension",
        "1.0.0",
        Box::new(|ctx| {
            let code = ctx.get_parameter("code").unwrap_or(&vec![]);
            let ext = ExampleJitExtension;
            match ext.optimize_code(ctx, code) {
                Ok(optimized) => {
                    let mut result = HashMap::new();
                    result.insert("optimized_code".to_string(), optimized);
                    Ok(result)
                }
                Err(e) => Err(e),
            }
        }),
    );
    extension_manager.register_extension("jit.optimize", jit_extension).await?;

    // 注册GC扩展
    let gc_extension = Extension::new(
        "example_gc",
        "Example GC Extension",
        "1.0.0",
        Box::new(|ctx| {
            let ext = ExampleGcExtension;
            match ext.collect_garbage(ctx) {
                Ok(collected) => {
                    let mut result = HashMap::new();
                    result.insert("collected_objects".to_string(), collected.to_string());
                    Ok(result)
                }
                Err(e) => Err(e),
            }
        }),
    );
    extension_manager.register_extension("gc.collect", gc_extension).await?;

    // 注册事件处理扩展
    let event_extension = Extension::new(
        "example_event_handler",
        "Example Event Handler Extension",
        "1.0.0",
        Box::new(|ctx| {
            let event_type = ctx.get_parameter("event_type").unwrap_or(&"").to_string();
            let event_data = ctx.get_parameter("event_data").unwrap_or(&vec![]);
            let ext = ExampleEventHandlerExtension;
            match ext.handle_event(ctx, &event_type, event_data) {
                Ok(response) => {
                    let mut result = HashMap::new();
                    result.insert("response".to_string(), String::from_utf8_lossy(&response).to_string());
                    Ok(result)
                }
                Err(e) => Err(e),
            }
        }),
    );
    extension_manager.register_extension("event.handle", event_extension).await?;

    println!("  - JIT optimization extension point registered");
    println!("  - GC collection extension point registered");
    println!("  - Event handling extension point registered");

    Ok(())
}

/// 发送事件到插件
async fn send_events_to_plugin(
    plugin_manager: &PluginManager,
    plugin_id: &PluginId,
) -> Result<(), VmError> {
    // 发送VM启动事件
    let vm_started_event = PluginEvent::VmStarted {
        vm_id: "demo_vm".to_string(),
    };
    plugin_manager.send_event_to_plugin(plugin_id, &vm_started_event).await?;

    // 发送设备添加事件
    let device_added_event = PluginEvent::DeviceAdded {
        device_id: "demo_device".to_string(),
        device_type: "network".to_string(),
    };
    plugin_manager.send_event_to_plugin(plugin_id, &device_added_event).await?;

    // 发送性能更新事件
    let performance_event = PluginEvent::PerformanceUpdate {
        metric: "cpu_usage".to_string(),
        value: 75.5,
    };
    plugin_manager.send_event_to_plugin(plugin_id, &performance_event).await?;

    // 发送自定义事件
    let custom_event = PluginEvent::Custom {
        event_type: "example.ping".to_string(),
        data: serde_json::json!({"message": "Hello from demo"}),
    };
    plugin_manager.send_event_to_plugin(plugin_id, &custom_event).await?;

    println!("  - VM started event sent");
    println!("  - Device added event sent");
    println!("  - Performance update event sent");
    println!("  - Custom ping event sent");

    Ok(())
}

/// 演示配置热更新
async fn demonstrate_config_update(
    plugin_manager: &PluginManager,
    plugin_id: &PluginId,
    config_dir: &PathBuf,
) -> Result<(), VmError> {
    // 创建配置管理器
    let storage_config = vm_plugin::config::ConfigManagerConfig::default();
    let storage = Arc::new(FileConfigStorage::new(
        config_dir.to_path_buf(),
        ConfigFormat::Json,
        storage_config.clone(),
    ));
    let validator = Arc::new(DefaultConfigValidator::new());
    let config_manager = Arc::new(PluginConfigManager::new(
        storage,
        validator,
        storage_config,
    ));

    // 获取当前配置
    let current_config = config_manager.get_plugin_config(plugin_id).await
        .map_err(|e| VmError::PluginError(e.to_string()))?;

    // 更新配置
    let mut new_config = current_config;
    new_config.data.insert("log_level".to_string(), ConfigValue::String("debug".to_string()));
    new_config.data.insert("max_events".to_string(), ConfigValue::Integer(2000));
    new_config.version += 1;

    // 保存更新后的配置
    config_manager.set_plugin_config(plugin_id, new_config).await
        .map_err(|e| VmError::PluginError(e.to_string()))?;

    println!("  - Log level changed to 'debug'");
    println!("  - Max events changed to 2000");

    Ok(())
}

/// 显示插件状态
async fn display_plugin_status(
    plugin_manager: &PluginManager,
    plugin_id: &PluginId,
) -> Result<(), VmError> {
    let plugin_info = plugin_manager.get_plugin_info(plugin_id).await?;
    let plugin_status = plugin_manager.get_plugin_status(plugin_id).await?;

    println!("Plugin Status:");
    println!("  - ID: {}", plugin_info.id.as_str());
    println!("  - Name: {}", plugin_info.name);
    println!("  - Version: {}", plugin_info.version.to_string());
    println!("  - State: {:?}", plugin_status.state);
    println!("  - Uptime: {} seconds", plugin_status.uptime_seconds);
    println!("  - Memory Usage: {} bytes", plugin_status.memory_usage_bytes);
    println!("  - Events Processed: {}", plugin_status.processed_events);
    println!("  - Error Count: {}", plugin_status.error_count);

    Ok(())
}

/// 显示管理器统计信息
async fn display_manager_stats(plugin_manager: &PluginManager) -> Result<(), VmError> {
    let stats = plugin_manager.get_statistics().await;

    println!("Manager Statistics:");
    println!("  - Total Plugins: {}", stats.total_plugins);
    println!("  - Active Plugins: {}", stats.active_plugins);
    println!("  - Error Plugins: {}", stats.error_plugins);
    println!("  - Total Events Processed: {}", stats.total_events_processed);
    println!("  - Total Errors: {}", stats.total_errors);
    println!("  - Total Memory Usage: {} bytes", stats.total_memory_usage);

    Ok(())
}