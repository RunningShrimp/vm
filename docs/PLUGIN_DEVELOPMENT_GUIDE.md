# VM项目插件开发指南

## 目录

1. [概述](#概述)
2. [开发环境搭建](#开发环境搭建)
3. [插件基础](#插件基础)
4. [插件开发示例](#插件开发示例)
5. [最佳实践](#最佳实践)
6. [测试指南](#测试指南)
7. [部署和发布](#部署和发布)
8. [常见问题](#常见问题)

## 概述

本指南为VM项目插件开发者提供详细的开发说明和最佳实践，帮助开发者快速上手插件开发，创建高质量、高性能的插件。

### 插件开发流程

1. **环境准备**：搭建插件开发环境
2. **项目创建**：创建插件项目结构
3. **接口实现**：实现插件接口
4. **配置定义**：定义插件配置
5. **测试验证**：编写和运行测试
6. **打包发布**：打包和发布插件

## 开发环境搭建

### 1. 安装Rust工具链

```bash
# 安装Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 安装最新稳定版
rustup update stable
rustup default stable

# 安装必要组件
rustup component add rustfmt clippy

# 安装常用工具
cargo install cargo-watch cargo-expand cargo-audit
```

### 2. 安装插件SDK

```bash
# 从 crates.io 安装
cargo add vm-plugin-sdk

# 或者从本地路径安装
cargo add --path /path/to/vm-plugin-sdk
```

### 3. 配置开发工具

创建 `.cargo/config.toml` 文件：

```toml
[build]
target-dir = "target"

[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=lld"]

[target.x86_64-apple-darwin]
rustflags = ["-C", "link-arg=-Wl,-ld_classic"]

[target.x86_64-pc-windows-msvc]
rustflags = ["-C", "target-feature=+crt-static"]
```

### 4. 创建开发项目模板

```bash
# 使用 cargo-generate 创建项目
cargo install cargo-generate
cargo generate --git https://github.com/vm-project/plugin-template.git --name my-plugin
```

## 插件基础

### 1. 插件结构

一个典型的插件项目结构如下：

```
my-plugin/
├── Cargo.toml              # 项目配置
├── README.md               # 项目说明
├── LICENSE                 # 许可证
├── src/
│   ├── lib.rs              # 库入口
│   ├── plugin.rs           # 插件实现
│   ├── config.rs           # 配置定义
│   └── error.rs            # 错误定义
├── tests/
│   ├── integration_tests.rs # 集成测试
│   └── unit_tests.rs       # 单元测试
├── examples/
│   └── usage.rs            # 使用示例
├── benches/
│   └── performance.rs      # 性能测试
└── docs/
    └── plugin_api.md       # API文档
```

### 2. Cargo.toml 配置

```toml
[package]
name = "my-plugin"
version = "0.1.0"
edition = "2021"
authors = ["Your Name <your.email@example.com>"]
description = "A VM plugin for ..."
license = "MIT OR Apache-2.0"
repository = "https://github.com/yourusername/my-plugin"
keywords = ["vm", "plugin"]
categories = ["development-tools"]

[lib]
crate-type = ["cdylib"] # 动态库

[dependencies]
vm-plugin-sdk = "0.1"
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tracing = "0.1"
thiserror = "1.0"
anyhow = "1.0"

[dev-dependencies]
tokio-test = "0.4"
tempfile = "3.0"

[profile.release]
lto = true
codegen-units = 1
panic = "abort"
```

### 3. 插件元数据

插件元数据描述了插件的基本信息和能力：

```rust
use vm_plugin_sdk::prelude::*;

/// 插件元数据
pub const PLUGIN_METADATA: PluginMetadata = PluginMetadata {
    id: PluginId::from("my-plugin"),
    name: "My Plugin".to_string(),
    version: PluginVersion::new(0, 1, 0),
    description: "A sample VM plugin".to_string(),
    author: "VM Team".to_string(),
    plugin_type: PluginType::JitCompiler,
    extension_points: vec!["jit.compiler".to_string()],
    required_extensions: vec![],
    entry_point: "my_plugin".to_string(),
    config: None,
    signature: None,
};
```

## 插件开发示例

### 1. JIT编译器插件

```rust
// src/plugin.rs
use vm_plugin_sdk::prelude::*;
use async_trait::async_trait;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// 示例JIT编译器插件
pub struct MyJitPlugin {
    /// 插件元数据
    metadata: PluginMetadata,
    /// 插件状态
    state: PluginState,
    /// 插件配置
    config: MyJitConfig,
    /// 编译统计
    stats: CompilationStats,
    /// 编译缓存
    compilation_cache: HashMap<String, CompiledMethod>,
}

/// JIT编译器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyJitConfig {
    /// 优化级别
    pub optimization_level: u32,
    /// 是否启用缓存
    pub enable_cache: bool,
    /// 缓存大小
    pub cache_size: usize,
}

impl Default for MyJitConfig {
    fn default() -> Self {
        Self {
            optimization_level: 2,
            enable_cache: true,
            cache_size: 1000,
        }
    }
}

impl MyJitPlugin {
    /// 创建新的插件实例
    pub fn new(config: MyJitConfig) -> Self {
        let metadata = PluginMetadata {
            id: PluginId::from("my-jit-plugin"),
            name: "My JIT Plugin".to_string(),
            version: PluginVersion::new(0, 1, 0),
            description: "A sample JIT compiler plugin".to_string(),
            author: "VM Team".to_string(),
            plugin_type: PluginType::JitCompiler,
            extension_points: vec!["jit.compiler".to_string()],
            required_extensions: vec![],
            entry_point: "my_jit_plugin".to_string(),
            config: None,
            signature: None,
        };

        Self {
            metadata,
            state: PluginState::Unloaded,
            config,
            stats: CompilationStats::default(),
            compilation_cache: HashMap::new(),
        }
    }

    /// 生成方法签名
    fn generate_method_signature(&self, method_info: &MethodInfo) -> String {
        format!("{}:{}", method_info.name, method_info.signature)
    }

    /// 生成机器码
    fn generate_machine_code(&self, method_info: &MethodInfo) -> Vec<u8> {
        // 简化实现：生成一些NOP指令
        vec![0x90; method_info.bytecode.len()]
    }

    /// 更新编译统计
    fn update_stats(&mut self, compilation_time: Duration) {
        self.stats.compiled_methods += 1;
        self.stats.total_compilation_time += compilation_time;
        self.stats.average_compilation_time = 
            self.stats.total_compilation_time / self.stats.compiled_methods as u32;
    }
}

#[async_trait]
impl Plugin for MyJitPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn initialize(&mut self) -> Result<(), PluginError> {
        tracing::info!("Initializing My JIT Plugin");

        // 验证配置
        if self.config.optimization_level > 3 {
            return Err(PluginError::ConfigurationError(
                "Optimization level must be between 0 and 3".to_string()
            ));
        }

        // 初始化编译缓存
        if self.config.enable_cache {
            self.compilation_cache.reserve(self.config.cache_size);
            tracing::info!("Compilation cache initialized with size {}", self.config.cache_size);
        }

        self.state = PluginState::Initialized;
        Ok(())
    }

    async fn start(&mut self) -> Result<(), PluginError> {
        tracing::info!("Starting My JIT Plugin");

        // 启动任何必要的后台任务
        self.state = PluginState::Running;
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), PluginError> {
        tracing::info!("Stopping My JIT Plugin");

        // 停止后台任务
        self.state = PluginState::Stopped;
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), PluginError> {
        tracing::info!("Cleaning up My JIT Plugin");

        // 清理资源
        self.compilation_cache.clear();
        self.state = PluginState::Loaded;
        Ok(())
    }

    async fn enable(&mut self) -> Result<(), PluginError> {
        tracing::info!("Enabling My JIT Plugin");
        self.state = PluginState::Running;
        Ok(())
    }

    async fn disable(&mut self) -> Result<(), PluginError> {
        tracing::info!("Disabling My JIT Plugin");
        self.state = PluginState::Disabled;
        Ok(())
    }

    fn state(&self) -> PluginState {
        self.state
    }

    async fn handle_config_change(&mut self, config: PluginConfig) -> Result<(), PluginError> {
        tracing::info!("Handling config change for My JIT Plugin");

        // 解析新配置
        let new_config: MyJitConfig = serde_json::from_value(
            config.data.get("jit_config").cloned().unwrap_or_default()
        ).map_err(|e| PluginError::ConfigurationError(e.to_string()))?;

        // 应用新配置
        self.config = new_config;

        // 如果禁用了缓存，清空现有缓存
        if !self.config.enable_cache {
            self.compilation_cache.clear();
        }

        Ok(())
    }

    async fn health_check(&self) -> Result<HealthStatus, PluginError> {
        let mut details = HashMap::new();
        details.insert("compiled_methods".to_string(), 
                      format!("{}", self.stats.compiled_methods));
        details.insert("cache_size".to_string(), 
                      format!("{}", self.compilation_cache.len()));
        details.insert("average_compilation_time".to_string(), 
                      format!("{:?}", self.stats.average_compilation_time));

        Ok(HealthStatus {
            healthy: self.state == PluginState::Running,
            message: "My JIT Plugin is healthy".to_string(),
            details,
            checked_at: Instant::now(),
        })
    }
}

#[async_trait]
impl JitCompilerPlugin for MyJitPlugin {
    async fn compile_method(
        &self,
        method_info: &MethodInfo,
        compilation_context: &CompilationContext,
    ) -> Result<CompiledMethod, CompilationError> {
        tracing::debug!("Compiling method: {}", method_info.name);

        let start_time = Instant::now();

        // 检查缓存
        if self.config.enable_cache {
            let method_signature = self.generate_method_signature(method_info);
            if let Some(cached_method) = self.compilation_cache.get(&method_signature) {
                tracing::debug!("Method {} found in cache", method_info.name);
                return Ok(cached_method.clone());
            }
        }

        // 生成机器码
        let machine_code = self.generate_machine_code(method_info);

        // 创建编译方法
        let compiled_method = CompiledMethod {
            method_id: method_info.id,
            machine_code,
            metadata: CompiledMethodMetadata::default(),
            compilation_time: start_time.elapsed(),
            optimization_info: OptimizationInfo::default(),
        };

        // 缓存结果
        if self.config.enable_cache {
            let method_signature = self.generate_method_signature(method_info);
            self.compilation_cache.insert(method_signature, compiled_method.clone());
        }

        tracing::debug!("Method {} compiled in {:?}", method_info.name, start_time.elapsed());

        Ok(compiled_method)
    }

    async fn optimize_compiled_method(
        &self,
        compiled_method: CompiledMethod,
        optimization_context: &OptimizationContext,
    ) -> Result<CompiledMethod, OptimizationError> {
        tracing::debug!("Optimizing compiled method");

        // 简化实现：直接返回原始方法
        Ok(compiled_method)
    }

    fn get_compilation_stats(&self) -> CompilationStats {
        self.stats.clone()
    }

    async fn configure_compiler(&mut self, config: JitCompilerConfig) -> Result<(), ConfigurationError> {
        tracing::info!("Configuring compiler");

        // 更新配置
        self.config.optimization_level = config.default_optimization_level as u32;

        Ok(())
    }
}

/// 插件工厂
pub struct MyJitPluginFactory;

impl PluginFactory for MyJitPluginFactory {
    fn create_plugin(&self, config: PluginConfig) -> Result<Box<dyn Plugin>, PluginError> {
        // 解析配置
        let jit_config: MyJitConfig = serde_json::from_value(
            config.data.get("jit_config").cloned().unwrap_or_default()
        ).map_err(|e| PluginError::ConfigurationError(e.to_string()))?;

        // 创建插件
        let plugin = MyJitPlugin::new(jit_config);

        Ok(Box::new(plugin))
    }

    fn get_metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: PluginId::from("my-jit-plugin"),
            name: "My JIT Plugin".to_string(),
            version: PluginVersion::new(0, 1, 0),
            description: "A sample JIT compiler plugin".to_string(),
            author: "VM Team".to_string(),
            plugin_type: PluginType::JitCompiler,
            extension_points: vec!["jit.compiler".to_string()],
            required_extensions: vec![],
            entry_point: "my_jit_plugin".to_string(),
            config: None,
            signature: None,
        }
    }

    fn validate_config(&self, config: &PluginConfig) -> Result<(), PluginError> {
        // 验证配置
        let _: MyJitConfig = serde_json::from_value(
            config.data.get("jit_config").cloned().unwrap_or_default()
        ).map_err(|e| PluginError::ConfigurationError(e.to_string()))?;

        Ok(())
    }
}

/// 插件入口点
#[no_mangle]
pub extern "C" fn create_plugin_factory() -> *mut dyn PluginFactory {
    let factory = MyJitPluginFactory;
    Box::into_raw(Box::new(factory))
}
```

### 2. GC策略插件

```rust
// src/plugin.rs
use vm_plugin_sdk::prelude::*;
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

/// 示例GC插件
pub struct MyGcPlugin {
    /// 插件元数据
    metadata: PluginMetadata,
    /// 插件状态
    state: PluginState,
    /// 插件配置
    config: MyGcConfig,
    /// GC统计
    stats: GcStats,
    /// 标记集合
    marked_objects: HashSet<ObjectId>,
}

/// GC配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyGcConfig {
    /// GC触发阈值（堆使用率）
    pub gc_threshold: f64,
    /// 最大GC时间
    pub max_gc_time: Duration,
    /// 是否启用并发GC
    pub enable_concurrent_gc: bool,
}

impl Default for MyGcConfig {
    fn default() -> Self {
        Self {
            gc_threshold: 0.8,
            max_gc_time: Duration::from_millis(100),
            enable_concurrent_gc: true,
        }
    }
}

impl MyGcPlugin {
    /// 创建新的插件实例
    pub fn new(config: MyGcConfig) -> Self {
        let metadata = PluginMetadata {
            id: PluginId::from("my-gc-plugin"),
            name: "My GC Plugin".to_string(),
            version: PluginVersion::new(0, 1, 0),
            description: "A sample garbage collector plugin".to_string(),
            author: "VM Team".to_string(),
            plugin_type: PluginType::GarbageCollector,
            extension_points: vec!["gc.algorithm".to_string()],
            required_extensions: vec![],
            entry_point: "my_gc_plugin".to_string(),
            config: None,
            signature: None,
        };

        Self {
            metadata,
            state: PluginState::Unloaded,
            config,
            stats: GcStats::default(),
            marked_objects: HashSet::new(),
        }
    }

    /// 检查是否需要触发GC
    fn should_trigger_gc(&self, heap: &Heap) -> bool {
        let utilization = heap.used_size as f64 / heap.size as f64;
        utilization >= self.config.gc_threshold
    }

    /// 标记可达对象
    fn mark_reachable_objects(&mut self, heap: &Heap, roots: &[GcRoot]) {
        self.marked_objects.clear();

        // 标记根对象
        for root in roots {
            self.marked_objects.insert(root.object_id);
        }

        // 简化实现：标记所有对象
        for (object_id, _) in &heap.objects {
            self.marked_objects.insert(*object_id);
        }
    }

    /// 回收不可达对象
    fn collect_unreachable_objects(&self, heap: &mut Heap) -> usize {
        let mut collected_count = 0;
        let mut collected_memory = 0;

        heap.objects.retain(|&object_id, object| {
            if self.marked_objects.contains(&object_id) {
                true // 保留可达对象
            } else {
                collected_count += 1;
                collected_memory += object.size;
                false // 删除不可达对象
            }
        });

        // 更新堆统计
        heap.used_size -= collected_memory;

        collected_count
    }

    /// 更新GC统计
    fn update_stats(&mut self, gc_time: Duration, collected_objects: u64, collected_memory: usize) {
        self.stats.total_gc_count += 1;
        self.stats.total_gc_time += gc_time;
        self.stats.average_gc_time = self.stats.total_gc_time / self.stats.total_gc_count as u32;
        self.stats.total_collected_objects += collected_objects;
        self.stats.total_collected_memory += collected_memory;
    }
}

#[async_trait]
impl Plugin for MyGcPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn initialize(&mut self) -> Result<(), PluginError> {
        tracing::info!("Initializing My GC Plugin");

        // 验证配置
        if self.config.gc_threshold <= 0.0 || self.config.gc_threshold > 1.0 {
            return Err(PluginError::ConfigurationError(
                "GC threshold must be between 0.0 and 1.0".to_string()
            ));
        }

        self.state = PluginState::Initialized;
        Ok(())
    }

    async fn start(&mut self) -> Result<(), PluginError> {
        tracing::info!("Starting My GC Plugin");

        // 启动GC监控线程
        if self.config.enable_concurrent_gc {
            // 启动并发GC线程
        }

        self.state = PluginState::Running;
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), PluginError> {
        tracing::info!("Stopping My GC Plugin");

        // 停止GC监控线程
        self.state = PluginState::Stopped;
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), PluginError> {
        tracing::info!("Cleaning up My GC Plugin");

        // 清理资源
        self.marked_objects.clear();
        self.state = PluginState::Loaded;
        Ok(())
    }

    async fn enable(&mut self) -> Result<(), PluginError> {
        tracing::info!("Enabling My GC Plugin");
        self.state = PluginState::Running;
        Ok(())
    }

    async fn disable(&mut self) -> Result<(), PluginError> {
        tracing::info!("Disabling My GC Plugin");
        self.state = PluginState::Disabled;
        Ok(())
    }

    fn state(&self) -> PluginState {
        self.state
    }

    async fn handle_config_change(&mut self, config: PluginConfig) -> Result<(), PluginError> {
        tracing::info!("Handling config change for My GC Plugin");

        // 解析新配置
        let new_config: MyGcConfig = serde_json::from_value(
            config.data.get("gc_config").cloned().unwrap_or_default()
        ).map_err(|e| PluginError::ConfigurationError(e.to_string()))?;

        // 应用新配置
        self.config = new_config;

        Ok(())
    }

    async fn health_check(&self) -> Result<HealthStatus, PluginError> {
        let mut details = HashMap::new();
        details.insert("total_gc_count".to_string(), 
                      format!("{}", self.stats.total_gc_count));
        details.insert("total_gc_time".to_string(), 
                      format!("{:?}", self.stats.total_gc_time));
        details.insert("average_gc_time".to_string(), 
                      format!("{:?}", self.stats.average_gc_time));
        details.insert("total_collected_objects".to_string(), 
                      format!("{}", self.stats.total_collected_objects));

        Ok(HealthStatus {
            healthy: self.state == PluginState::Running,
            message: "My GC Plugin is healthy".to_string(),
            details,
            checked_at: Instant::now(),
        })
    }
}

#[async_trait]
impl GcAlgorithmPlugin for MyGcPlugin {
    async fn perform_gc(
        &mut self,
        heap: &mut Heap,
        gc_context: &GcContext,
    ) -> Result<GcResult, GcError> {
        tracing::debug!("Performing garbage collection");

        let start_time = Instant::now();

        // 检查是否需要触发GC
        if !self.should_trigger_gc(heap) {
            return Ok(GcResult {
                collected_objects: 0,
                collected_memory: 0,
                gc_time: Duration::from_millis(0),
                phase_results: vec![],
            });
        }

        // 标记阶段
        let mark_start = Instant::now();
        self.mark_reachable_objects(heap, &gc_context.roots);
        let mark_time = mark_start.elapsed();

        // 回收阶段
        let collect_start = Instant::now();
        let collected_objects = self.collect_unreachable_objects(heap) as u64;
        let collect_time = collect_start.elapsed();

        let total_time = start_time.elapsed();

        // 更新统计
        self.update_stats(total_time, collected_objects, heap.used_size);

        // 创建GC结果
        let gc_result = GcResult {
            collected_objects,
            collected_memory: heap.used_size,
            gc_time: total_time,
            phase_results: vec![
                GcPhaseResult {
                    phase_name: "mark".to_string(),
                    phase_time: mark_time,
                    objects_processed: self.marked_objects.len() as u64,
                },
                GcPhaseResult {
                    phase_name: "collect".to_string(),
                    phase_time: collect_time,
                    objects_processed: collected_objects,
                },
            ],
        };

        tracing::debug!("GC completed: collected {} objects in {:?}", 
                       collected_objects, total_time);

        Ok(gc_result)
    }

    async fn mark_reachable_objects(
        &self,
        heap: &Heap,
        roots: &[GcRoot],
        marker: &mut ObjectMarker,
    ) -> Result<MarkingResult, GcError> {
        tracing::debug!("Marking reachable objects");

        // 标记根对象
        for root in roots {
            marker.mark_object(root.object_id)?;
        }

        // 简化实现：标记所有对象
        for (object_id, _) in &heap.objects {
            marker.mark_object(*object_id)?;
        }

        Ok(MarkingResult {
            marked_objects: heap.objects.len() as u64,
            marking_time: Duration::from_millis(1),
        })
    }

    async fn collect_unreachable_objects(
        &self,
        heap: &mut Heap,
        marked_objects: &HashSet<ObjectId>,
    ) -> Result<CollectionResult, GcError> {
        tracing::debug!("Collecting unreachable objects");

        let mut collected_count = 0;
        let mut collected_memory = 0;

        heap.objects.retain(|&object_id, object| {
            if marked_objects.contains(&object_id) {
                true // 保留可达对象
            } else {
                collected_count += 1;
                collected_memory += object.size;
                false // 删除不可达对象
            }
        });

        // 更新堆统计
        heap.used_size -= collected_memory;

        Ok(CollectionResult {
            collected_objects: collected_count,
            collected_memory,
            collection_time: Duration::from_millis(1),
        })
    }

    async fn compact_heap(&mut self, heap: &mut Heap) -> Result<CompactionResult, GcError> {
        tracing::debug!("Compacting heap");

        // 简化实现：不进行实际压缩
        Ok(CompactionResult {
            compacted_objects: 0,
            compacted_memory: 0,
            compaction_time: Duration::from_millis(0),
            fragmentation_before: 0.0,
            fragmentation_after: 0.0,
        })
    }

    fn get_gc_stats(&self) -> GcStats {
        self.stats.clone()
    }

    async fn configure_gc_algorithm(&mut self, config: GcAlgorithmConfig) -> Result<(), ConfigurationError> {
        tracing::info!("Configuring GC algorithm");

        // 更新配置
        self.config.gc_threshold = config.gc_threshold;
        self.config.max_gc_time = config.max_gc_time;

        Ok(())
    }
}

/// 插件工厂
pub struct MyGcPluginFactory;

impl PluginFactory for MyGcPluginFactory {
    fn create_plugin(&self, config: PluginConfig) -> Result<Box<dyn Plugin>, PluginError> {
        // 解析配置
        let gc_config: MyGcConfig = serde_json::from_value(
            config.data.get("gc_config").cloned().unwrap_or_default()
        ).map_err(|e| PluginError::ConfigurationError(e.to_string()))?;

        // 创建插件
        let plugin = MyGcPlugin::new(gc_config);

        Ok(Box::new(plugin))
    }

    fn get_metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: PluginId::from("my-gc-plugin"),
            name: "My GC Plugin".to_string(),
            version: PluginVersion::new(0, 1, 0),
            description: "A sample garbage collector plugin".to_string(),
            author: "VM Team".to_string(),
            plugin_type: PluginType::GarbageCollector,
            extension_points: vec!["gc.algorithm".to_string()],
            required_extensions: vec![],
            entry_point: "my_gc_plugin".to_string(),
            config: None,
            signature: None,
        }
    }

    fn validate_config(&self, config: &PluginConfig) -> Result<(), PluginError> {
        // 验证配置
        let _: MyGcConfig = serde_json::from_value(
            config.data.get("gc_config").cloned().unwrap_or_default()
        ).map_err(|e| PluginError::ConfigurationError(e.to_string()))?;

        Ok(())
    }
}

/// 插件入口点
#[no_mangle]
pub extern "C" fn create_plugin_factory() -> *mut dyn PluginFactory {
    let factory = MyGcPluginFactory;
    Box::into_raw(Box::new(factory))
}
```

## 最佳实践

### 1. 设计原则

#### 1.1 单一职责原则

每个插件应该只负责一个特定功能，避免功能过于复杂：

```rust
// 好的设计：专注于JIT编译
pub struct MyJitPlugin {
    // JIT编译相关字段
}

// 避免：混合多种功能
pub struct MyMultiPurposePlugin {
    // JIT编译相关字段
    // GC相关字段
    // 网络相关字段
    // ...
}
```

#### 1.2 接口隔离原则

插件应该只依赖它需要的接口，避免不必要的依赖：

```rust
// 好的设计：只实现需要的接口
#[async_trait]
impl JitCompilerPlugin for MyJitPlugin {
    // 只实现JIT编译相关方法
}

// 避免：实现所有可能的接口
#[async_trait]
impl JitCompilerPlugin for MyJitPlugin { /* ... */ }
#[async_trait]
impl GcAlgorithmPlugin for MyJitPlugin { /* ... */ }
#[async_trait]
impl VirtualizationBackendPlugin for MyJitPlugin { /* ... */ }
```

#### 1.3 依赖倒置原则

插件应该依赖抽象而不是具体实现：

```rust
// 好的设计：依赖抽象接口
pub struct MyJitPlugin {
    code_generator: Box<dyn CodeGenerator>,
    optimizer: Box<dyn Optimizer>,
}

// 避免：依赖具体实现
pub struct MyJitPlugin {
    code_generator: X86CodeGenerator,
    optimizer: AggressiveOptimizer,
}
```

### 2. 性能优化

#### 2.1 延迟初始化

只在需要时初始化资源：

```rust
pub struct MyJitPlugin {
    // 使用Option延迟初始化
    compilation_cache: Option<HashMap<String, CompiledMethod>>,
    code_generator: Option<Box<dyn CodeGenerator>>,
}

impl MyJitPlugin {
    fn get_cache(&mut self) -> &mut HashMap<String, CompiledMethod> {
        if self.compilation_cache.is_none() {
            self.compilation_cache = Some(HashMap::new());
        }
        self.compilation_cache.as_mut().unwrap()
    }
    
    fn get_code_generator(&mut self) -> &mut Box<dyn CodeGenerator> {
        if self.code_generator.is_none() {
            self.code_generator = Some(Box::new(DefaultCodeGenerator::new()));
        }
        self.code_generator.as_mut().unwrap()
    }
}
```

#### 2.2 缓存策略

合理使用缓存提高性能：

```rust
impl MyJitPlugin {
    async fn compile_method_with_cache(
        &mut self,
        method_info: &MethodInfo,
        compilation_context: &CompilationContext,
    ) -> Result<CompiledMethod, CompilationError> {
        // 生成缓存键
        let cache_key = self.generate_cache_key(method_info, compilation_context);
        
        // 检查缓存
        if let Some(cached_method) = self.get_from_cache(&cache_key) {
            return Ok(cached_method);
        }
        
        // 编译方法
        let compiled_method = self.compile_method_internal(method_info, compilation_context).await?;
        
        // 缓存结果
        self.put_to_cache(cache_key, compiled_method.clone());
        
        Ok(compiled_method)
    }
    
    fn generate_cache_key(&self, method_info: &MethodInfo, context: &CompilationContext) -> String {
        format!("{}:{}:{}", 
                method_info.name, 
                method_info.signature,
                context.optimization_level)
    }
}
```

#### 2.3 异步处理

使用异步处理提高并发性能：

```rust
#[async_trait]
impl JitCompilerPlugin for MyJitPlugin {
    async fn compile_method(
        &self,
        method_info: &MethodInfo,
        compilation_context: &CompilationContext,
    ) -> Result<CompiledMethod, CompilationError> {
        // 并行执行编译阶段
        let ir_future = self.generate_ir(method_info);
        let metadata_future = self.analyze_metadata(method_info);
        
        // 等待两个阶段完成
        let (ir, metadata) = tokio::try_join!(ir_future, metadata_future)?;
        
        // 继续优化和代码生成
        let optimized_ir = self.optimize_ir(ir, compilation_context).await?;
        let machine_code = self.generate_machine_code(&optimized_ir).await?;
        
        Ok(CompiledMethod {
            method_id: method_info.id,
            machine_code,
            metadata,
            compilation_time: Duration::from_millis(10),
            optimization_info: OptimizationInfo::default(),
        })
    }
}
```

### 3. 错误处理

#### 3.1 自定义错误类型

定义清晰的错误类型：

```rust
// src/error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MyPluginError {
    #[error("Compilation failed: {0}")]
    CompilationFailed(String),
    
    #[error("Optimization error: {0}")]
    OptimizationError(String),
    
    #[error("Cache error: {0}")]
    CacheError(String),
    
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    
    #[error("Resource error: {0}")]
    ResourceError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

impl From<MyPluginError> for PluginError {
    fn from(error: MyPluginError) -> Self {
        PluginError::RuntimeError(error.to_string())
    }
}
```

#### 3.2 错误恢复

实现错误恢复机制：

```rust
impl MyJitPlugin {
    async fn compile_method_with_fallback(
        &self,
        method_info: &MethodInfo,
        compilation_context: &CompilationContext,
    ) -> Result<CompiledMethod, CompilationError> {
        // 尝试优化编译
        match self.compile_method_optimized(method_info, compilation_context).await {
            Ok(method) => Ok(method),
            Err(e) => {
                tracing::warn!("Optimized compilation failed, falling back to basic compilation: {}", e);
                
                // 回退到基本编译
                self.compile_method_basic(method_info, compilation_context).await
            }
        }
    }
}
```

### 4. 配置管理

#### 4.1 配置验证

实现严格的配置验证：

```rust
impl MyJitConfig {
    pub fn validate(&self) -> Result<(), MyPluginError> {
        if self.optimization_level > 3 {
            return Err(MyPluginError::ConfigurationError(
                "Optimization level must be between 0 and 3".to_string()
            ));
        }
        
        if self.cache_size == 0 {
            return Err(MyPluginError::ConfigurationError(
                "Cache size must be greater than 0".to_string()
            ));
        }
        
        if self.max_compilation_time == Duration::from_millis(0) {
            return Err(MyPluginError::ConfigurationError(
                "Max compilation time must be greater than 0".to_string()
            ));
        }
        
        Ok(())
    }
}
```

#### 4.2 配置热更新

支持配置热更新：

```rust
#[async_trait]
impl Plugin for MyJitPlugin {
    async fn handle_config_change(&mut self, config: PluginConfig) -> Result<(), PluginError> {
        // 解析新配置
        let new_config: MyJitConfig = serde_json::from_value(
            config.data.get("jit_config").cloned().unwrap_or_default()
        ).map_err(|e| PluginError::ConfigurationError(e.to_string()))?;
        
        // 验证新配置
        new_config.validate().map_err(|e| PluginError::ConfigurationError(e.to_string()))?;
        
        // 检查是否需要重建缓存
        let needs_cache_rebuild = self.config.cache_size != new_config.cache_size || 
                                 !self.config.enable_cache && new_config.enable_cache;
        
        // 应用新配置
        let old_config = self.config.clone();
        self.config = new_config;
        
        // 处理配置变更
        if needs_cache_rebuild {
            self.rebuild_cache().await?;
        }
        
        if self.config.enable_logging != old_config.enable_logging {
            self.update_logging_config().await?;
        }
        
        tracing::info!("Configuration updated successfully");
        
        Ok(())
    }
}
```

### 5. 日志和监控

#### 5.1 结构化日志

使用结构化日志：

```rust
use tracing::{debug, error, info, warn, instrument, Span};

impl MyJitPlugin {
    #[instrument(skip(self, compilation_context))]
    pub async fn compile_method(
        &self,
        method_info: &MethodInfo,
        compilation_context: &CompilationContext,
    ) -> Result<CompiledMethod, CompilationError> {
        let span = Span::current();
        span.record("method_name", &method_info.name);
        span.record("optimization_level", &compilation_context.optimization_level);
        
        debug!("Starting method compilation");
        
        let start_time = Instant::now();
        
        // 编译逻辑...
        
        let compilation_time = start_time.elapsed();
        span.record("compilation_time", &compilation_time.as_millis());
        
        info!(
            method_name = %method_info.name,
            optimization_level = compilation_context.optimization_level,
            compilation_time_ms = compilation_time.as_millis(),
            "Method compiled successfully"
        );
        
        Ok(compiled_method)
    }
}
```

#### 5.2 指标收集

收集性能指标：

```rust
use metrics::{counter, histogram, gauge};

impl MyJitPlugin {
    pub async fn compile_method(
        &self,
        method_info: &MethodInfo,
        compilation_context: &CompilationContext,
    ) -> Result<CompiledMethod, CompilationError> {
        let start_time = Instant::now();
        
        // 编译逻辑...
        
        let compilation_time = start_time.elapsed();
        
        // 记录指标
        counter!("plugin.jit.compilations.total").increment(1);
        histogram!("plugin.jit.compilation_time").record(compilation_time);
        gauge!("plugin.jit.cache_size").set(self.compilation_cache.len() as f64);
        
        Ok(compiled_method)
    }
}
```

## 测试指南

### 1. 单元测试

```rust
// tests/unit_tests.rs
use my_plugin::*;
use vm_plugin_sdk::prelude::*;
use tokio_test;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_plugin_initialization() {
        let config = MyJitConfig::default();
        let mut plugin = MyJitPlugin::new(config);
        
        // 测试初始化
        assert_eq!(plugin.state(), PluginState::Unloaded);
        
        let result = plugin.initialize().await;
        assert!(result.is_ok());
        assert_eq!(plugin.state(), PluginState::Initialized);
    }

    #[tokio::test]
    async fn test_method_compilation() {
        let config = MyJitConfig::default();
        let plugin = MyJitPlugin::new(config);
        
        let method_info = MethodInfo {
            id: MethodId::from("test_method"),
            name: "test_method".to_string(),
            signature: "()V".to_string(),
            bytecode: vec![0x01, 0x02, 0x03],
            attributes: MethodAttributes::default(),
        };
        
        let compilation_context = CompilationContext {
            target_arch: TargetArchitecture::X86_64,
            optimization_level: OptimizationLevel::O2,
            compilation_options: CompilationOptions::default(),
            runtime_info: RuntimeInfo::default(),
        };
        
        let result = plugin.compile_method(&method_info, &compilation_context).await;
        assert!(result.is_ok());
        
        let compiled_method = result.unwrap();
        assert_eq!(compiled_method.method_id, method_info.id);
        assert!(!compiled_method.machine_code.is_empty());
    }

    #[test]
    fn test_config_validation() {
        // 有效配置
        let valid_config = MyJitConfig {
            optimization_level: 2,
            enable_cache: true,
            cache_size: 1000,
        };
        assert!(valid_config.validate().is_ok());
        
        // 无效配置
        let invalid_config = MyJitConfig {
            optimization_level: 5, // 超出范围
            enable_cache: true,
            cache_size: 1000,
        };
        assert!(invalid_config.validate().is_err());
    }

    #[tokio::test]
    async fn test_plugin_lifecycle() {
        let config = MyJitConfig::default();
        let mut plugin = MyJitPlugin::new(config);
        
        // 完整生命周期测试
        assert_eq!(plugin.state(), PluginState::Unloaded);
        
        plugin.initialize().await.unwrap();
        assert_eq!(plugin.state(), PluginState::Initialized);
        
        plugin.start().await.unwrap();
        assert_eq!(plugin.state(), PluginState::Running);
        
        plugin.stop().await.unwrap();
        assert_eq!(plugin.state(), PluginState::Stopped);
        
        plugin.cleanup().await.unwrap();
        assert_eq!(plugin.state(), PluginState::Loaded);
    }

    #[tokio::test]
    async fn test_health_check() {
        let config = MyJitConfig::default();
        let mut plugin = MyJitPlugin::new(config);
        
        plugin.initialize().await.unwrap();
        plugin.start().await.unwrap();
        
        let health_status = plugin.health_check().await.unwrap();
        assert!(health_status.healthy);
        assert!(health_status.details.contains_key("compiled_methods"));
    }
}
```

### 2. 集成测试

```rust
// tests/integration_tests.rs
use my_plugin::*;
use vm_plugin_sdk::prelude::*;
use tempfile::TempDir;

#[tokio::test]
async fn test_plugin_factory() {
    let factory = MyJitPluginFactory;
    
    // 测试元数据
    let metadata = factory.get_metadata();
    assert_eq!(metadata.id, PluginId::from("my-jit-plugin"));
    assert_eq!(metadata.name, "My JIT Plugin");
    
    // 测试插件创建
    let config = PluginConfig {
        plugin_id: PluginId::from("my-jit-plugin"),
        version: 1,
        data: {
            let mut data = HashMap::new();
            data.insert("jit_config".to_string(), serde_json::to_value(MyJitConfig::default()).unwrap());
            data
        },
        metadata: ConfigMetadata::default(),
    };
    
    let plugin = factory.create_plugin(config).unwrap();
    assert_eq!(plugin.metadata().id, PluginId::from("my-jit-plugin"));
}

#[tokio::test]
async fn test_plugin_with_file_config() {
    let temp_dir = TempDir::new().unwrap();
    let config_file = temp_dir.path().join("plugin_config.json");
    
    // 创建配置文件
    let config_data = serde_json::json!({
        "jit_config": {
            "optimization_level": 2,
            "enable_cache": true,
            "cache_size": 1000
        }
    });
    
    tokio::fs::write(&config_file, config_data.to_string()).await.unwrap();
    
    // 读取配置文件
    let config_content = tokio::fs::read_to_string(config_file).await.unwrap();
    let config_value: serde_json::Value = serde_json::from_str(&config_content).unwrap();
    
    let plugin_config = PluginConfig {
        plugin_id: PluginId::from("my-jit-plugin"),
        version: 1,
        data: {
            let mut data = HashMap::new();
            data.insert("jit_config".to_string(), config_value);
            data
        },
        metadata: ConfigMetadata::default(),
    };
    
    let factory = MyJitPluginFactory;
    let plugin = factory.create_plugin(plugin_config).unwrap();
    
    // 测试插件配置是否正确加载
    let jit_plugin = plugin.as_any().downcast_ref::<MyJitPlugin>().unwrap();
    assert_eq!(jit_plugin.config.optimization_level, 2);
    assert!(jit_plugin.config.enable_cache);
    assert_eq!(jit_plugin.config.cache_size, 1000);
}
```

### 3. 性能测试

```rust
// benches/performance.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use my_plugin::*;
use vm_plugin_sdk::prelude::*;

fn bench_method_compilation(c: &mut Criterion) {
    let plugin = MyJitPlugin::new(MyJitConfig::default());
    
    let method_info = MethodInfo {
        id: MethodId::from("bench_method"),
        name: "bench_method".to_string(),
        signature: "()V".to_string(),
        bytecode: vec![0x01; 1000], // 1KB字节码
        attributes: MethodAttributes::default(),
    };
    
    let compilation_context = CompilationContext {
        target_arch: TargetArchitecture::X86_64,
        optimization_level: OptimizationLevel::O2,
        compilation_options: CompilationOptions::default(),
        runtime_info: RuntimeInfo::default(),
    };
    
    c.bench_function("compile_method", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                plugin.compile_method(
                    black_box(&method_info),
                    black_box(&compilation_context),
                ).await
            });
    });
}

fn bench_cache_operations(c: &mut Criterion) {
    let plugin = MyJitPlugin::new(MyJitConfig {
        enable_cache: true,
        cache_size: 1000,
        ..Default::default()
    });
    
    let method_info = MethodInfo {
        id: MethodId::from("cache_bench_method"),
        name: "cache_bench_method".to_string(),
        signature: "()V".to_string(),
        bytecode: vec![0x01; 1000],
        attributes: MethodAttributes::default(),
    };
    
    let compilation_context = CompilationContext {
        target_arch: TargetArchitecture::X86_64,
        optimization_level: OptimizationLevel::O2,
        compilation_options: CompilationOptions::default(),
        runtime_info: RuntimeInfo::default(),
    };
    
    // 预热缓存
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        plugin.compile_method(&method_info, &compilation_context).await.unwrap();
    });
    
    c.bench_function("compile_method_cached", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                plugin.compile_method(
                    black_box(&method_info),
                    black_box(&compilation_context),
                ).await
            });
    });
}

criterion_group!(benches, bench_method_compilation, bench_cache_operations);
criterion_main!(benches);
```

## 部署和发布

### 1. 插件打包

```bash
# 构建发布版本
cargo build --release

# 创建插件包
mkdir -p my-plugin-v0.1.0
cp target/release/libmy_plugin.so my-plugin-v0.1.0/
cp plugin.json my-plugin-v0.1.0/
cp README.md my-plugin-v0.1.0/
cp LICENSE my-plugin-v0.1.0/

# 创建压缩包
tar -czf my-plugin-v0.1.0.tar.gz my-plugin-v0.1.0/
```

### 2. 插件配置文件

```json
{
  "id": "my-jit-plugin",
  "name": "My JIT Plugin",
  "version": "0.1.0",
  "description": "A sample JIT compiler plugin",
  "author": "VM Team",
  "license": "MIT OR Apache-2.0",
  "plugin_type": "JitCompiler",
  "entry_point": "libmy_plugin.so",
  "extension_points": ["jit.compiler"],
  "required_extensions": [],
  "dependencies": [],
  "config_schema": {
    "type": "object",
    "properties": {
      "jit_config": {
        "type": "object",
        "properties": {
          "optimization_level": {
            "type": "integer",
            "minimum": 0,
            "maximum": 3,
            "default": 2
          },
          "enable_cache": {
            "type": "boolean",
            "default": true
          },
          "cache_size": {
            "type": "integer",
            "minimum": 1,
            "default": 1000
          }
        },
        "required": []
      }
    },
    "required": []
  },
  "default_config": {
    "jit_config": {
      "optimization_level": 2,
      "enable_cache": true,
      "cache_size": 1000
    }
  }
}
```

### 3. 发布流程

```bash
# 1. 运行测试
cargo test
cargo test --release

# 2. 运行性能测试
cargo bench

# 3. 检查代码质量
cargo clippy -- -D warnings
cargo fmt --check

# 4. 安全审计
cargo audit

# 5. 构建发布版本
cargo build --release

# 6. 创建发布包
mkdir -p release
cp target/release/libmy_plugin.so release/
cp plugin.json release/
cp README.md release/
cp LICENSE release/

# 7. 创建签名（如果有私钥）
openssl dgst -sha256 -sign private_key.pem -out plugin.sig release/libmy_plugin.so

# 8. 创建发布压缩包
tar -czf my-plugin-v0.1.0.tar.gz release/

# 9. 上传到插件仓库
vm-plugin upload my-plugin-v0.1.0.tar.gz
```

## 常见问题

### 1. 编译问题

**问题**: 编译时出现链接错误

**解决方案**:
```toml
# Cargo.toml
[lib]
crate-type = ["cdylib"]

[profile.release]
lto = true
codegen-units = 1
panic = "abort"
```

### 2. 运行时问题

**问题**: 插件加载失败

**解决方案**:
1. 检查插件入口点是否正确导出
2. 确保插件库依赖与主程序兼容
3. 检查插件签名是否有效

### 3. 性能问题

**问题**: 插件运行性能不佳

**解决方案**:
1. 使用性能分析工具定位瓶颈
2. 优化算法和数据结构
3. 使用缓存减少重复计算
4. 考虑使用异步处理提高并发

### 4. 调试问题

**问题**: 插件调试困难

**解决方案**:
1. 添加详细的日志输出
2. 使用单元测试和集成测试
3. 使用调试器进行断点调试
4. 使用内存检查工具检测内存问题

### 5. 配置问题

**问题**: 插件配置不生效

**解决方案**:
1. 检查配置格式是否正确
2. 验证配置值是否在有效范围内
3. 确保配置热更新逻辑正确实现
4. 检查配置权限设置

---

*本指南最后更新于2025年12月5日*