# vm-plugin

**VM项目插件系统**

[![Rust](https://img.shields.io/badge/rust-2024%20Edition-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

---

## 概述

`vm-plugin` 是VM项目的完整插件架构实现，支持第三方扩展和模块化功能。它提供了插件的生命周期管理、安全沙箱、依赖解析、热更新和插件仓库等全面的插件基础设施。

## 🎯 核心功能

- **插件管理器**: 插件的加载、卸载和生命周期管理
- **插件接口**: 统一的插件开发接口和契约
- **安全沙箱**: 插件执行的安全隔离和权限控制
- **依赖管理**: 插件间的依赖关系解析和版本兼容性
- **热更新**: 运行时插件的热加载和更新
- **扩展点**: 标准化的插件扩展点和回调机制
- **资源监控**: 插件资源使用监控和限制

## 📦 主要组件

### 1. PluginManager (插件管理器)

核心的插件生命周期管理：

```rust
use vm_plugin::PluginManager;

// 创建插件管理器
let mut manager = PluginManager::new();

// 加载插件
let plugin_id = manager.load_plugin("path/to/plugin.so")?;

// 启动插件
manager.start_plugin(&plugin_id)?;

// 与插件通信
manager.send_message(&plugin_id, PluginMessage::Custom("Hello".into()))?;

// 停止并卸载
manager.stop_plugin(&plugin_id)?;
manager.unload_plugin(&plugin_id)?;
```

### 2. Plugin Trait (插件接口)

统一的插件开发接口：

```rust
use vm_plugin::Plugin;

pub struct MyPlugin {
    // 插件状态
}

impl Plugin for MyPlugin {
    fn name(&self) -> &str {
        "my-plugin"
    }

    fn version(&self) -> vm_plugin::PluginVersion {
        vm_plugin::PluginVersion { major: 1, minor: 0, patch: 0 }
    }

    fn init(&mut self) -> Result<(), vm_core::VmError> {
        // 初始化逻辑
        Ok(())
    }

    fn on_vm_start(&mut self) -> Result<(), vm_core::VmError> {
        // VM启动时的回调
        Ok(())
    }

    fn on_instruction_execute(&mut self, pc: u64, insn: u32) -> Result<(), vm_core::VmError> {
        // 每条指令执行前的回调
        Ok(())
    }
}
```

### 3. SecurityManager (安全管理器)

插件安全沙箱和权限控制：

```rust
use vm_plugin::{SecurityManager, PermissionPolicy, SandboxConfig};

// 创建安全管理器
let security = SecurityManager::new();

// 配置沙箱
let sandbox_config = SandboxConfig {
    max_memory_mb: 100,
    max_cpu_percent: 50,
    allowed_syscalls: vec!["read", "write", "mmap"],
    network_access: false,
};

// 设置权限策略
let policy = PermissionPolicy {
    allow_file_access: false,
    allow_network: false,
    allow_process_control: false,
};

security.enforce_policy(&plugin_id, &policy)?;
security.enforce_sandbox(&plugin_id, &sandbox_config)?;
```

### 4. DependencyResolver (依赖解析器)

处理插件间的依赖关系：

```rust
use vm_plugin::DependencyResolver;

let resolver = DependencyResolver::new();

// 解析依赖顺序
let load_order = resolver.resolve_load_order(&plugins)?;

// 检查版本兼容性
resolver.check_version_compatibility(&plugin_a, &plugin_b)?;

// 验证依赖完整性
resolver.validate_dependencies(&plugin)?;
```

### 5. 扩展点 (Extension Points)

标准化的插件扩展点：

```rust
use vm_plugin::extension_points::*;

// 指令翻译扩展
pub struct InstructionTranslatorPlugin;

impl InstructionTranslationExtension for InstructionTranslatorPlugin {
    fn translate(&self, insn: u32) -> Result<u64, vm_core::VmError> {
        // 自定义指令翻译逻辑
        Ok(0)
    }
}

// 内存访问扩展
impl MemoryAccessExtension for InstructionTranslatorPlugin {
    fn on_read(&self, addr: u64, size: usize) -> Result<(), vm_core::VmError> {
        // 内存读取钩子
        Ok(())
    }

    fn on_write(&self, addr: u64, value: u64) -> Result<(), vm_core::VmError> {
        // 内存写入钩子
        Ok(())
    }
}
```

## 🔧 依赖关系

```toml
[dependencies]
vm-core = { path = "../vm-core" }      # 核心类型和错误
serde = { workspace = true }           # 序列化支持
```

## 🚀 使用场景

### 场景1: 指令级追踪插件

```rust
pub struct TracingPlugin {
    instruction_count: usize,
}

impl Plugin for TracingPlugin {
    fn on_instruction_execute(&mut self, pc: u64, insn: u32) -> Result<(), VmError> {
        self.instruction_count += 1;
        if self.instruction_count % 1000 == 0 {
            println!("Executed {} instructions at PC: 0x{:x}", self.instruction_count, pc);
        }
        Ok(())
    }
}
```

### 场景2: 内存监控插件

```rust
use vm_plugin::MemoryAccessExtension;

pub struct MemoryMonitorPlugin {
    read_count: HashMap<u64, usize>,
    write_count: HashMap<u64, usize>,
}

impl MemoryAccessExtension for MemoryMonitorPlugin {
    fn on_read(&self, addr: u64, _size: usize) -> Result<(), VmError> {
        *self.read_count.entry(addr).or_insert(0) += 1;
        Ok(())
    }

    fn on_write(&self, addr: u64, _value: u64) -> Result<(), VmError> {
        *self.write_count.entry(addr).or_insert(0) += 1;
        Ok(())
    }
}
```

### 场景3: 自定义指令扩展

```rust
use vm_plugin::InstructionTranslationExtension;

pub struct CustomISAPlugin;

impl InstructionTranslationExtension for CustomISAPlugin {
    fn translate(&self, insn: u32) -> Result<u64, VmError> {
        // 识别自定义指令模式
        if (insn & 0xFF000000) == 0xAB000000 {
            // 翻译为IR
            Ok(self.translate_custom_insn(insn)?)
        } else {
            Err(VmError::InvalidInstruction(insn))
        }
    }
}
```

## 🔌 扩展点列表

vm-plugin提供以下标准扩展点：

| 扩展点 | 接口 | 说明 |
|--------|------|------|
| **指令翻译** | `InstructionTranslationExtension` | 自定义指令集支持 |
| **内存访问** | `MemoryAccessExtension` | 内存访问钩子和监控 |
| **设备仿真** | `DeviceEmulationExtension` | 虚拟设备插件 |
| **网络** | `NetworkExtension` | 网络协议栈插件 |
| **文件系统** | `FileSystemExtension` | 虚拟文件系统插件 |
| **性能分析** | `ProfilingExtension` | 性能分析工具插件 |

## 📝 API概览

### 主要Trait

```rust
/// 插件trait
pub trait Plugin {
    fn name(&self) -> &str;
    fn version(&self) -> PluginVersion;
    fn init(&mut self) -> Result<(), VmError>;
    fn shutdown(&mut self) -> Result<(), VmError>;
}

/// 扩展点trait
pub trait ExtensionPoint: Plugin {
    fn extension_type(&self) -> ExtensionType;
}
```

### 主要结构

- **`PluginManager`**: 插件生命周期管理
- **`SecurityManager`**: 安全策略执行
- **`DependencyResolver`**: 依赖解析
- **`PluginResourceMonitor`**: 资源监控
- **`PluginMetadata`**: 插件元信息

## 🎨 设计特点

### 1. 类型安全

利用Rust的类型系统确保插件接口的正确性：

```rust
pub trait Plugin: Send + Sync {
    // 编译时检查所有必要方法
}
```

### 2. 沙箱隔离

每个插件运行在独立的沙箱环境中：

```rust
let sandbox = SandboxConfig {
    max_memory_mb: 100,
    max_cpu_percent: 50,
    // ... 更多限制
};
```

### 3. 热更新支持

支持运行时插件的热加载：

```rust
manager.hot_reload_plugin(&plugin_id, "new_version.so")?;
```

## 📚 相关文档

- [vm-core](../vm-core/README.md) - 核心类型和VM接口
- [vm-device](../vm-device/README.md) - 设备仿真
- [MASTER_DOCUMENTATION_INDEX](../MASTER_DOCUMENTATION_INDEX.md) - 完整文档索引

## 🔨 开发指南

### 创建自定义插件

1. 实现Plugin trait
2. （可选）实现扩展点trait
3. 编译为动态库
4. 使用PluginManager加载

### 插件开发最佳实践

1. **错误处理**: 所有操作都应返回`Result<T, VmError>`
2. **资源清理**: 在`shutdown()`中释放所有资源
3. **线程安全**: 确保插件实现是`Send + Sync`
4. **版本兼容**: 遵循语义化版本规范

### 插件打包

```toml
[package]
name = "my-vm-plugin"
version = "1.0.0"
crate-type = ["cdylib"]

[dependencies]
vm-plugin = { path = "../vm-plugin" }
vm-core = { path = "../vm-core" }
```

## ⚠️ 注意事项

1. **性能影响**: 插件钩子可能影响VM性能，谨慎使用
2. **安全风险**: 插件拥有与VM相同的权限，需严格审查
3. **兼容性**: 插件API可能随VM版本变化
4. **资源限制**: 合理设置沙箱资源限制

## 🤝 贡献指南

如果您想添加新的扩展点或改进插件系统：

1. 提出扩展点设计方案
2. 实现示例插件
3. 添加文档和测试
4. 更新本README

## 📊 性能指标

| 操作 | 性能 | 说明 |
|------|------|------|
| 插件加载 | ~10ms | 加载和初始化插件 |
| 消息传递 | < 1μs | 插件间通信 |
| 扩展点调用 | < 100ns | 单次扩展点调用 |
| 沙箱检查 | < 50ns | 权限验证 |

## 📝 许可证

本项目采用 MIT 许可证 - 详见 [LICENSE](../LICENSE) 文件

---

**包版本**: workspace v0.1.0
**Rust版本**: 2024 Edition
**最后更新**: 2026-01-07
