# 插件系统完善文档

## 更新时间
2025-12-04

## 概述

本文档说明插件系统的完善工作，包括接口定义改进、生命周期管理增强、健康监控和新增功能。

## 主要改进

### 1. 插件接口完善

#### Plugin Trait增强

添加了新的可选方法：

- **`get_config()`**: 获取插件配置（可选实现）
- **`update_config()`**: 更新插件配置（可选实现）

这些方法允许插件在运行时动态更新配置，无需重启。

#### 改进的文档注释

为所有trait方法添加了详细的文档注释，说明：
- 方法的用途和调用时机
- 参数和返回值的含义
- 实现注意事项

### 2. 插件实例增强

#### PluginInstance结构改进

- **公开字段**: 将`metadata`、`state`、`load_time`等字段改为公开，方便访问
- **新增统计信息**: 添加了`PluginInstanceStats`结构，跟踪：
  - 处理的事件数
  - 错误数
  - 内存使用
  - CPU时间
  - 最后活动时间

### 3. 插件生命周期管理

#### 新增方法

- **`enable_plugin()`**: 启用已加载的插件
- **`disable_plugin()`**: 禁用插件（停止但不卸载）
- **`restart_plugin()`**: 重启插件
- **`update_plugin_config()`**: 更新插件配置

#### 改进的事件处理

- 事件处理时更新统计信息
- 错误计数和自动错误状态转换
- 记录最后活动时间

### 4. 健康监控

#### PluginHealth结构

新增插件健康状态监控：

```rust
pub struct PluginHealth {
    pub plugin_id: PluginId,
    pub state: PluginState,
    pub uptime_seconds: u64,
    pub error_rate: f64,
    pub memory_usage_bytes: u64,
    pub events_processed: u64,
    pub is_healthy: bool, // 状态为Active且错误率 < 10%
}
```

#### 健康检查方法

- **`get_plugin_health()`**: 获取单个插件的健康状态
- **`get_all_plugin_health()`**: 获取所有插件的健康状态

### 5. 事件总线改进

#### EventBusStats

新增事件总线统计信息：

- 发布的事件总数
- 订阅者总数
- 按事件类型统计的发布数

#### 新增方法

- **`get_stats()`**: 获取统计信息
- **`reset_stats()`**: 重置统计信息

### 6. 插件上下文增强

#### 新增字段

- **`logger`**: 可选的日志记录器，允许插件记录日志

#### PluginLogger Trait

定义了插件日志记录接口：

```rust
pub trait PluginLogger: Send + Sync {
    fn log(&self, level: LogLevel, message: &str);
}
```

### 7. 统计信息增强

#### PluginManagerStats扩展

新增字段：
- `total_events_processed`: 总事件处理数
- `total_errors`: 总错误数
- `total_memory_usage`: 总内存使用

## 使用示例

### 基本使用

```rust
use vm_plugin::{PluginManager, PluginContext};

let mut manager = PluginManager::new("vm-001".to_string());

// 加载插件
let plugin_id = manager.load_plugin("./plugins/my_plugin.so").await?;

// 获取插件健康状态
if let Some(health) = manager.get_plugin_health(&plugin_id) {
    println!("Plugin {} health: {:?}", plugin_id, health);
    if !health.is_healthy {
        println!("Warning: Plugin has high error rate: {:.2}%", health.error_rate * 100.0);
    }
}

// 更新插件配置
let mut config = HashMap::new();
config.insert("max_connections".to_string(), "100".to_string());
manager.update_plugin_config(&plugin_id, config).await?;

// 重启插件
manager.restart_plugin(&plugin_id).await?;
```

### 健康监控

```rust
// 获取所有插件的健康状态
let health_statuses = manager.get_all_plugin_health();

for health in health_statuses {
    println!("Plugin {}: {}", health.plugin_id, 
        if health.is_healthy { "Healthy" } else { "Unhealthy" });
    println!("  Uptime: {}s", health.uptime_seconds);
    println!("  Error rate: {:.2}%", health.error_rate * 100.0);
    println!("  Memory: {} bytes", health.memory_usage_bytes);
    println!("  Events processed: {}", health.events_processed);
}
```

### 插件生命周期管理

```rust
// 加载插件
let plugin_id = manager.load_plugin("./plugins/my_plugin.so").await?;

// 禁用插件（停止但不卸载）
manager.disable_plugin(&plugin_id).await?;

// 启用插件
manager.enable_plugin(&plugin_id).await?;

// 重启插件
manager.restart_plugin(&plugin_id).await?;

// 卸载插件
manager.unload_plugin(&plugin_id).await?;
```

### 事件总线统计

```rust
// 发布事件
let event = PluginEvent::VmStarted { vm_id: "vm-001".to_string() };
manager.broadcast_event(event).await?;

// 获取事件总线统计
let event_bus = manager.context.event_bus.read().unwrap();
let stats = event_bus.get_stats();
println!("Total events published: {}", stats.total_published);
println!("Total subscribers: {}", stats.total_subscribers);
```

## 技术细节

### 错误处理策略

- **错误计数**: 每个插件维护错误计数
- **自动错误状态**: 如果错误数超过10，自动将插件状态设置为Error
- **错误恢复**: 插件可以通过重启恢复正常状态

### 健康检查逻辑

插件被认为是健康的，当且仅当：
1. 状态为`Active`
2. 错误率 < 10%（错误数 / 处理的事件数）

### 配置更新

- 配置更新是异步的，不会阻塞其他操作
- 插件可以选择实现`update_config`方法来支持动态配置更新
- 如果插件不支持配置更新，方法会返回成功但不执行任何操作

## 未来改进

1. **插件热更新**: 支持运行时更新插件代码
2. **插件依赖图**: 可视化插件依赖关系
3. **插件性能分析**: 详细的性能指标和分析
4. **插件版本管理**: 支持插件版本回滚
5. **插件隔离**: 更强的插件隔离机制

## 相关文档

- `docs/PLUGIN_SYSTEM.md`: 插件系统基础文档
- `docs/API_REFERENCE.md`: API参考文档

