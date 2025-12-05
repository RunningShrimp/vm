# 插件系统文档

## 概述

虚拟机插件系统提供了完整的插件架构，支持第三方扩展和模块化功能。插件系统支持动态加载、安全沙箱、依赖管理、插件间通信等功能。

## 主要功能

### 1. 插件生命周期管理

插件具有以下生命周期状态：

- **Unloaded**: 未加载
- **Loading**: 正在加载
- **Loaded**: 已加载
- **Initializing**: 正在初始化
- **Active**: 活跃运行
- **Error**: 错误状态
- **Unloading**: 正在卸载

### 2. 安全沙箱

插件系统提供了完整的安全沙箱机制：

- **权限控制**: 基于白名单/黑名单的权限策略
- **资源限制**: 内存和CPU时间限制
- **文件系统隔离**: 限制文件系统访问路径
- **网络隔离**: 可选的网络访问限制

### 3. 依赖管理

- **版本检查**: 自动检查插件依赖版本兼容性
- **依赖解析**: 解析并验证插件依赖关系
- **依赖注册**: 跟踪已安装的插件及其版本

### 4. 插件间通信

- **消息通道**: 插件间点对点消息传递
- **事件广播**: 系统事件广播到所有插件
- **共享数据**: 插件间共享数据存储

## 使用方法

### 创建插件管理器

```rust
use vm_plugin::PluginManager;

let mut manager = PluginManager::new("vm_instance_001".to_string());

// 添加插件搜索路径
manager.add_plugin_path("./plugins");
manager.add_plugin_path("/usr/local/lib/vm-plugins");
```

### 加载插件

```rust
use vm_plugin::PluginManager;

let mut manager = PluginManager::new("vm_instance_001".to_string());

// 加载单个插件
let plugin_id = manager.load_plugin("./plugins/my_plugin.so").await?;

// 扫描并加载所有可用插件
let loaded_plugins = manager.scan_and_load_plugins().await?;
```

### 插件元信息

插件元信息存储在JSON文件中（与插件库文件同名，扩展名为`.json`）：

```json
{
  "id": "my_plugin",
  "name": "My Plugin",
  "version": {
    "major": 1,
    "minor": 0,
    "patch": 0
  },
  "description": "A sample plugin",
  "author": "Plugin Author",
  "license": "MIT",
  "dependencies": {
    "base_plugin": {
      "major": 1,
      "minor": 0,
      "patch": 0
    }
  },
  "plugin_type": "Custom",
  "entry_point": "plugin_entry",
  "permissions": [
    "ReadVmState",
    "FileSystemAccess"
  ]
}
```

### 插件间通信

```rust
use vm_plugin::{PluginManager, PluginMessage};
use serde_json::json;

let manager = PluginManager::new("vm_instance_001".to_string());

// 发送消息到特定插件
manager.send_message_to_plugin(
    "sender_plugin",
    "receiver_plugin",
    "custom_message".to_string(),
    json!({"data": "value"}),
)?;

// 广播消息到所有插件
manager.broadcast_message(
    "sender_plugin",
    "broadcast_message".to_string(),
    json!({"data": "value"}),
)?;

// 发布事件
use vm_plugin::PluginEvent;
let event = PluginEvent::VmStarted {
    vm_id: "vm_001".to_string(),
};
manager.broadcast_event(event).await?;
```

### 安全沙箱配置

```rust
use vm_plugin::{SecurityManager, SandboxConfig, PermissionPolicy};

// 创建安全管理器
let mut security = SecurityManager::new();

// 配置沙箱
let mut config = SandboxConfig::default();
config.memory_limit = Some(512 * 1024 * 1024); // 512MB
config.cpu_time_limit = Some(120); // 120秒
config.filesystem_restricted = true;
config.allowed_paths.insert("/tmp/plugins".to_string());
config.forbidden_paths.insert("/etc".to_string());

security.set_sandbox_config(config);
security.set_permission_policy(PermissionPolicy::Whitelist);

// 添加允许的权限
security.add_permission(PluginPermission::ReadVmState);
security.add_permission(PluginPermission::FileSystemAccess);
```

### 资源监控

```rust
use vm_plugin::{PluginResourceMonitor, SecurityManager};

let security_manager = Arc::new(RwLock::new(SecurityManager::new()));
let monitor = PluginResourceMonitor::new(Arc::clone(&security_manager));

// 记录内存使用
monitor.record_memory_usage("plugin_id", 1024 * 1024)?; // 1MB

// 记录CPU时间
monitor.record_cpu_time("plugin_id", 10); // 10秒

// 获取资源使用情况
let memory = monitor.get_memory_usage("plugin_id");
let cpu_time = monitor.get_cpu_time("plugin_id");
```

### 卸载插件

```rust
use vm_plugin::PluginManager;

let mut manager = PluginManager::new("vm_instance_001".to_string());

// 卸载插件
manager.unload_plugin("plugin_id").await?;
```

## 开发插件

### 插件接口

插件需要实现`Plugin` trait：

```rust
use vm_plugin::{Plugin, PluginContext, PluginEvent, PluginMetadata, PluginStatus};
use async_trait::async_trait;

pub struct MyPlugin {
    metadata: PluginMetadata,
}

#[async_trait]
impl Plugin for MyPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn initialize(&mut self, context: &PluginContext) -> Result<(), VmError> {
        // 初始化插件
        Ok(())
    }

    async fn start(&mut self) -> Result<(), VmError> {
        // 启动插件
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), VmError> {
        // 停止插件
        Ok(())
    }

    async fn handle_event(&mut self, event: &PluginEvent) -> Result<(), VmError> {
        // 处理事件
        match event {
            PluginEvent::VmStarted { vm_id } => {
                println!("VM {} started", vm_id);
            }
            _ => {}
        }
        Ok(())
    }

    fn status(&self) -> PluginStatus {
        PluginStatus {
            state: PluginState::Active,
            uptime_seconds: 0,
            memory_usage_bytes: 0,
            processed_events: 0,
            error_count: 0,
        }
    }

    async fn cleanup(&mut self) -> Result<(), VmError> {
        // 清理资源
        Ok(())
    }
}

// 插件入口点（需要导出为C函数）
#[no_mangle]
pub extern "C" fn plugin_entry() -> *mut dyn Plugin {
    let metadata = PluginMetadata {
        id: "my_plugin".to_string(),
        name: "My Plugin".to_string(),
        // ... 其他字段
    };
    Box::into_raw(Box::new(MyPlugin { metadata }))
}
```

### Cargo.toml 配置

```toml
[package]
name = "my-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
vm-plugin = { path = "../../vm-plugin" }
async-trait = "0.1"
```

## 权限类型

- **ReadVmState**: 读取虚拟机状态
- **WriteVmState**: 写入虚拟机状态
- **ExecuteSyscall**: 执行系统调用
- **NetworkAccess**: 访问网络
- **FileSystemAccess**: 访问文件系统
- **MemoryAllocation**: 分配内存
- **Custom(String)**: 自定义权限

## 插件类型

- **ExecutionEngine**: 执行引擎插件
- **DeviceEmulation**: 设备模拟插件
- **DebugTool**: 调试工具插件
- **PerformanceMonitor**: 性能监控插件
- **Network**: 网络插件
- **Storage**: 存储插件
- **Security**: 安全插件
- **Custom**: 自定义插件

## 最佳实践

1. **权限最小化**: 只请求必要的权限
2. **错误处理**: 妥善处理所有可能的错误
3. **资源清理**: 在cleanup中释放所有资源
4. **异步操作**: 使用async/await进行异步操作
5. **事件处理**: 正确处理系统事件
6. **依赖管理**: 明确声明插件依赖
7. **版本兼容**: 确保插件版本兼容性

## 安全注意事项

1. **权限检查**: 始终检查插件权限
2. **资源限制**: 设置合理的资源限制
3. **沙箱隔离**: 使用沙箱隔离插件执行
4. **输入验证**: 验证所有插件输入
5. **错误隔离**: 确保插件错误不影响主系统

## 示例：完整插件

```rust
use vm_plugin::{
    Plugin, PluginContext, PluginEvent, PluginMetadata, PluginPermission,
    PluginState, PluginStatus, PluginType, PluginVersion,
};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct ExamplePlugin {
    metadata: PluginMetadata,
    context: Option<Arc<PluginContext>>,
}

impl ExamplePlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata {
                id: "example_plugin".to_string(),
                name: "Example Plugin".to_string(),
                version: PluginVersion {
                    major: 1,
                    minor: 0,
                    patch: 0,
                },
                description: "An example plugin".to_string(),
                author: "Example Author".to_string(),
                license: "MIT".to_string(),
                dependencies: HashMap::new(),
                plugin_type: PluginType::Custom,
                entry_point: "plugin_entry".to_string(),
                permissions: vec![PluginPermission::ReadVmState],
            },
            context: None,
        }
    }
}

#[async_trait]
impl Plugin for ExamplePlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn initialize(&mut self, context: &PluginContext) -> Result<(), VmError> {
        self.context = Some(Arc::new(context.clone()));
        println!("Plugin {} initialized", self.metadata.id);
        Ok(())
    }

    async fn start(&mut self) -> Result<(), VmError> {
        println!("Plugin {} started", self.metadata.id);
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), VmError> {
        println!("Plugin {} stopped", self.metadata.id);
        Ok(())
    }

    async fn handle_event(&mut self, event: &PluginEvent) -> Result<(), VmError> {
        match event {
            PluginEvent::VmStarted { vm_id } => {
                println!("VM {} started, plugin {} handling event", vm_id, self.metadata.id);
            }
            _ => {}
        }
        Ok(())
    }

    fn status(&self) -> PluginStatus {
        PluginStatus {
            state: PluginState::Active,
            uptime_seconds: 0,
            memory_usage_bytes: 0,
            processed_events: 0,
            error_count: 0,
        }
    }

    async fn cleanup(&mut self) -> Result<(), VmError> {
        println!("Plugin {} cleaned up", self.metadata.id);
        self.context = None;
        Ok(())
    }
}

#[no_mangle]
pub extern "C" fn plugin_entry() -> *mut dyn Plugin {
    Box::into_raw(Box::new(ExamplePlugin::new()))
}
```

## 故障排除

### 插件加载失败

- 检查插件文件是否存在
- 验证插件元信息格式
- 检查权限是否足够
- 确认依赖已安装

### 插件通信失败

- 确认目标插件已加载
- 检查消息格式是否正确
- 验证插件状态是否为Active

### 资源限制错误

- 检查内存使用是否超限
- 验证CPU时间限制
- 调整沙箱配置

## 未来改进

1. **插件仓库**: 实现插件分发和发现机制
2. **热更新**: 支持运行时插件更新
3. **插件验证**: 插件签名和验证机制
4. **性能优化**: 优化插件加载和通信性能
5. **监控工具**: 插件监控和调试工具


