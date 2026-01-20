//! # 插件注册表
//!
//! 提供插件的注册、发现、版本管理和依赖解析功能。

use crate::{
    PluginId, PluginMetadata, PluginType, PluginVersion,
};

// 临时的依赖定义，因为 PluginDependency 不存在
#[derive(Debug, Clone)]
pub struct PluginDependency {
    pub plugin_id: PluginId,
    pub version_requirement: VersionRequirement,
    pub optional: bool,
}

// 临时的版本要求定义，因为 VersionRequirement 不存在
#[derive(Debug, Clone)]
pub enum VersionRequirement {
    Exact(PluginVersion),
    Minimum(PluginVersion),
    Maximum(PluginVersion),
    Range(PluginVersion, PluginVersion),
    Any,
}

impl std::fmt::Display for VersionRequirement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VersionRequirement::Exact(v) => write!(f, "{}", v),
            VersionRequirement::Minimum(v) => write!(f, ">={}", v),
            VersionRequirement::Maximum(v) => write!(f, "<={}", v),
            VersionRequirement::Range(min, max) => write!(f, ">={},<={}", min, max),
            VersionRequirement::Any => write!(f, "*"),
        }
    }
}
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use vm_core::VmError;

/// 插件注册表
pub struct PluginRegistry {
    /// 已注册的插件
    plugins: HashMap<PluginId, RegisteredPlugin>,
    /// 按类型分组的插件
    plugins_by_type: HashMap<PluginType, Vec<PluginId>>,
    /// 按扩展点分组的插件
    plugins_by_extension_point: HashMap<String, Vec<PluginId>>,
    /// 插件索引
    index: PluginIndex,
    /// 注册表配置
    config: RegistryConfig,
}

/// 注册表配置
#[derive(Debug, Clone)]
pub struct RegistryConfig {
    /// 是否启用自动发现
    pub auto_discovery: bool,
    /// 插件搜索路径
    pub search_paths: Vec<PathBuf>,
    /// 是否启用版本检查
    pub version_check: bool,
    /// 是否启用依赖检查
    pub dependency_check: bool,
    /// 最大插件数量
    pub max_plugins: usize,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            auto_discovery: true,
            search_paths: vec![
                PathBuf::from("./plugins"),
                PathBuf::from("./vm-plugins"),
                PathBuf::from("/usr/lib/vm-plugins"),
            ],
            version_check: true,
            dependency_check: true,
            max_plugins: 1000,
        }
    }
}

/// 已注册的插件
#[derive(Debug, Clone)]
pub struct RegisteredPlugin {
    /// 插件元数据
    pub metadata: PluginMetadata,
    /// 插件依赖
    pub dependencies: Vec<PluginDependency>,
    /// 注册时间
    pub registered_at: std::time::Instant,
    /// 插件状态
    pub state: PluginState,
    /// 插件路径
    pub path: PathBuf,
    /// 插件签名验证结果
    pub signature_valid: bool,
    /// 插件统计信息
    pub stats: PluginStats,
}

/// 插件状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginState {
    /// 未加载
    Unloaded,
    /// 已加载
    Loaded,
    /// 已初始化
    Initialized,
    /// 运行中
    Running,
    /// 已停止
    Stopped,
    /// 错误
    Error,
    /// 已禁用
    Disabled,
}

/// 插件统计信息
#[derive(Debug, Clone, Default)]
pub struct PluginStats {
    /// 加载次数
    pub load_count: u64,
    /// 启动次数
    pub start_count: u64,
    /// 错误次数
    pub error_count: u64,
    /// 总运行时间（纳秒）
    pub total_runtime_ns: u64,
    /// 最后加载时间
    pub last_loaded: Option<std::time::Instant>,
    /// 最后启动时间
    pub last_started: Option<std::time::Instant>,
    /// 最后错误时间
    pub last_error: Option<std::time::Instant>,
}

/// 插件索引
#[derive(Debug)]
struct PluginIndex {
    /// 按名称索引
    by_name: HashMap<String, PluginId>,
    /// 按作者索引
    by_author: HashMap<String, Vec<PluginId>>,
    /// 按标签索引
    by_tag: HashMap<String, Vec<PluginId>>,
    /// 按依赖索引
    by_dependency: HashMap<PluginId, Vec<PluginId>>,
}

/// 发现的插件
#[derive(Debug, Clone)]
pub struct DiscoveredPlugin {
    /// 插件元数据
    pub metadata: PluginMetadata,
    /// 插件路径
    pub path: PathBuf,
    /// 插件字节码
    pub bytecode: Vec<u8>,
    /// 发现时间
    pub discovered_at: std::time::Instant,
}

/// 插件信息
#[derive(Debug, Clone)]
pub struct PluginInfo {
    /// 插件ID
    pub id: PluginId,
    /// 插件名称
    pub name: String,
    /// 插件版本
    pub version: PluginVersion,
    /// 插件类型
    pub plugin_type: PluginType,
    /// 插件状态
    pub state: PluginState,
    /// 注册时间
    pub registered_at: std::time::Instant,
    /// 插件描述
    pub description: String,
    /// 插件作者
    pub author: String,
    /// 插件标签
    pub tags: Vec<String>,
}

impl PluginRegistry {
    /// 创建新的插件注册表
    pub fn new(config: RegistryConfig) -> Self {
        Self {
            plugins: HashMap::new(),
            plugins_by_type: HashMap::new(),
            plugins_by_extension_point: HashMap::new(),
            index: PluginIndex {
                by_name: HashMap::new(),
                by_author: HashMap::new(),
                by_tag: HashMap::new(),
                by_dependency: HashMap::new(),
            },
            config,
        }
    }

    /// 注册插件
    pub fn register_plugin(
        &mut self,
        discovered_plugin: DiscoveredPlugin,
        dependencies: Vec<PluginDependency>,
    ) -> Result<PluginId, VmError> {
        let plugin_id = discovered_plugin.metadata.id.clone();

        // 检查插件是否已注册
        if self.plugins.contains_key(&plugin_id) {
            return Err(VmError::Core(vm_core::CoreError::InvalidState {
                message: format!("Plugin {} already registered", plugin_id),
                current: "registered".to_string(),
                expected: "not_registered".to_string(),
            }));
        }

        // 检查插件数量限制
        if self.plugins.len() >= self.config.max_plugins {
            return Err(VmError::Core(vm_core::CoreError::InvalidState {
                message: format!("Maximum plugin limit {} reached", self.config.max_plugins),
                current: "limit_reached".to_string(),
                expected: "within_limit".to_string(),
            }));
        }

        // 版本检查
        if self.config.version_check {
            self.check_version_compatibility(&discovered_plugin.metadata)?;
        }

        // 依赖检查
        if self.config.dependency_check {
            self.check_dependencies(&discovered_plugin.metadata, &dependencies)?;
        }

        // 验证插件签名
        let signature_valid = self.verify_plugin_signature(&discovered_plugin)?;

        // 创建注册插件
        let registered_plugin = RegisteredPlugin {
            metadata: discovered_plugin.metadata.clone(),
            dependencies,
            registered_at: std::time::Instant::now(),
            state: PluginState::Unloaded,
            path: discovered_plugin.path.clone(),
            signature_valid,
            stats: PluginStats::default(),
        };

        // 更新索引
        self.update_index(&registered_plugin);

        // 按类型分组
        let plugin_type = registered_plugin.metadata.plugin_type.clone();
        self.plugins_by_type
            .entry(plugin_type)
            .or_default()
            .push(plugin_id.clone());

        // 按扩展点分组 - 暂时跳过，因为 PluginMetadata 没有 extension_points 字段
        // for extension_point in &registered_plugin.metadata.extension_points {
        //     self.plugins_by_extension_point
        //         .entry(extension_point.clone())
        //         .or_default()
        //         .push(plugin_id.clone());
        // }

        // 注册插件
        self.plugins.insert(plugin_id.clone(), registered_plugin);

        tracing::info!("Registered plugin: {}", plugin_id);
        Ok(plugin_id)
    }

    /// 注销插件
    pub fn unregister_plugin(&mut self, plugin_id: &PluginId) -> Result<(), VmError> {
        let plugin = self.plugins.get(plugin_id)
            .ok_or_else(|| VmError::Core(vm_core::CoreError::InvalidState {
                message: format!("Plugin {} not found", plugin_id),
                current: "not_found".to_string(),
                expected: "found".to_string(),
            }))?;

        // 从按类型分组中移除
        if let Some(plugins) = self.plugins_by_type.get_mut(&plugin.metadata.plugin_type) {
            plugins.retain(|id| id != plugin_id);
            if plugins.is_empty() {
                self.plugins_by_type.remove(&plugin.metadata.plugin_type);
            }
        }

        // 从按扩展点分组中移除 - 暂时跳过，因为 PluginMetadata 没有 extension_points 字段
        // for extension_point in &plugin.metadata.extension_points {
        //     if let Some(plugins) = self.plugins_by_extension_point.get_mut(extension_point) {
        //         plugins.retain(|id| id != plugin_id);
        //         if plugins.is_empty() {
        //             self.plugins_by_extension_point.remove(extension_point);
        //         }
        //     }
        // }

        // 从索引中移除
        self.remove_from_index(plugin_id);

        // 移除插件
        self.plugins.remove(plugin_id);

        tracing::info!("Unregistered plugin: {}", plugin_id);
        Ok(())
    }

    /// 获取插件
    pub fn get_plugin(&self, plugin_id: &PluginId) -> Option<&RegisteredPlugin> {
        self.plugins.get(plugin_id)
    }

    /// 获取可变插件
    pub fn get_plugin_mut(&mut self, plugin_id: &PluginId) -> Option<&mut RegisteredPlugin> {
        self.plugins.get_mut(plugin_id)
    }

    /// 按类型获取插件
    pub fn get_plugins_by_type(&self, plugin_type: &PluginType) -> Vec<&RegisteredPlugin> {
        self.plugins_by_type
            .get(plugin_type)
            .map(|plugin_ids| {
                plugin_ids.iter()
                    .filter_map(|id| self.plugins.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 按扩展点获取插件
    pub fn get_plugins_by_extension_point(&self, extension_point: &str) -> Vec<&RegisteredPlugin> {
        self.plugins_by_extension_point
            .get(extension_point)
            .map(|plugin_ids| {
                plugin_ids.iter()
                    .filter_map(|id| self.plugins.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 列出所有插件
    pub fn list_plugins(&self) -> Vec<PluginInfo> {
        self.plugins.values()
            .map(|plugin| PluginInfo {
                id: plugin.metadata.id.clone(),
                name: plugin.metadata.name.clone(),
                version: plugin.metadata.version.clone(),
                plugin_type: plugin.metadata.plugin_type.clone(),
                state: plugin.state,
                registered_at: plugin.registered_at,
                description: plugin.metadata.description.clone(),
                author: plugin.metadata.author.clone(),
                tags: vec![], // PluginMetadata 没有 tags 字段，暂时使用空向量
            })
            .collect()
    }

    /// 搜索插件
    pub fn search_plugins(&self, query: &PluginSearchQuery) -> Vec<&RegisteredPlugin> {
        let mut results = Vec::new();

        for plugin in self.plugins.values() {
            if self.matches_query(plugin, query) {
                results.push(plugin);
            }
        }

        results
    }

    /// 发现插件
    pub async fn discover_plugins(&self) -> Result<Vec<DiscoveredPlugin>, VmError> {
        let mut discovered_plugins = Vec::new();

        for search_path in &self.config.search_paths {
            if !search_path.exists() {
                continue;
            }

            let plugins = self.scan_directory(search_path).await?;
            discovered_plugins.extend(plugins);
        }

        Ok(discovered_plugins)
    }

    /// 扫描目录中的插件
    async fn scan_directory(&self, dir: &Path) -> Result<Vec<DiscoveredPlugin>, VmError> {
        let mut discovered_plugins = Vec::new();

        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if self.is_plugin_file(&path) {
                    match self.load_plugin_metadata(&path).await {
                        Ok(plugin) => discovered_plugins.push(plugin),
                        Err(e) => {
                            tracing::warn!("Failed to load plugin metadata from {}: {:?}", path.display(), e);
                        }
                    }
                }
            }
        }

        Ok(discovered_plugins)
    }

    /// 检查文件是否是插件文件
    fn is_plugin_file(&self, path: &Path) -> bool {
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

    /// 加载插件元数据
    async fn load_plugin_metadata(&self, path: &Path) -> Result<DiscoveredPlugin, VmError> {
        // 尝试从JSON文件加载元数据
        let metadata_path = path.with_extension("json");
        let metadata = if metadata_path.exists() {
            let json_str = tokio::fs::read_to_string(&metadata_path).await
                .map_err(|e| VmError::Core(vm_core::CoreError::InvalidState {
                    message: format!("Failed to read metadata file: {}", e),
                    current: "read_failed".to_string(),
                    expected: "read_success".to_string(),
                }))?;
            serde_json::from_str(&json_str)
                .map_err(|e| VmError::Core(vm_core::CoreError::InvalidState {
                    message: format!("Failed to parse metadata: {}", e),
                    current: "parse_failed".to_string(),
                    expected: "parse_success".to_string(),
                }))?
        } else {
            // 如果没有元数据文件，从文件名推断
            self.infer_metadata_from_path(path)?
        };

        // 读取插件字节码
        let bytecode = tokio::fs::read(path).await
            .map_err(|e| VmError::Core(vm_core::CoreError::InvalidState {
                message: format!("Failed to read plugin file: {}", e),
                current: "read_failed".to_string(),
                expected: "read_success".to_string(),
            }))?;

        Ok(DiscoveredPlugin {
            metadata,
            path: path.to_path_buf(),
            bytecode,
            discovered_at: std::time::Instant::now(),
        })
    }

    /// 从路径推断插件元数据
    fn infer_metadata_from_path(&self, path: &Path) -> Result<PluginMetadata, VmError> {
        let file_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        Ok(PluginMetadata {
            id: PluginId::from(file_name),
            name: format!("{} Plugin", file_name),
            version: PluginVersion { major: 1, minor: 0, patch: 0 },
            description: format!("A {} plugin", file_name),
            author: "Unknown".to_string(),
            license: "MIT".to_string(),
            dependencies: HashMap::new(),
            plugin_type: PluginType::Custom,
            entry_point: "plugin_entry".to_string(),
            permissions: vec![],
        })
    }

    /// 检查版本兼容性
    fn check_version_compatibility(&self, metadata: &PluginMetadata) -> Result<(), VmError> {
        // 这里应该检查插件版本与系统版本的兼容性
        // 简化实现，总是返回Ok
        Ok(())
    }

    /// 检查依赖
    fn check_dependencies(
        &self,
        metadata: &PluginMetadata,
        dependencies: &[PluginDependency],
    ) -> Result<(), VmError> {
        for dep in dependencies {
            // 检查依赖的插件是否存在
            if !self.plugins.contains_key(&dep.plugin_id) {
                if !dep.optional {
                    return Err(VmError::Core(vm_core::CoreError::InvalidState {
                        message: format!("Required dependency {} not found", dep.plugin_id),
                        current: "dependency_missing".to_string(),
                        expected: "dependency_found".to_string(),
                    }));
                }
            }

            // 检查版本要求
            if let Some(registered_plugin) = self.plugins.get(&dep.plugin_id) {
                if !self.is_version_compatible(
                    &registered_plugin.metadata.version,
                    &dep.version_requirement,
                ) {
                    return Err(VmError::Core(vm_core::CoreError::InvalidState {
                        message: format!(
                            "Dependency {} version {} required, but {} found",
                            dep.plugin_id,
                            dep.version_requirement,
                            registered_plugin.metadata.version
                        ),
                        current: "version_incompatible".to_string(),
                        expected: "version_compatible".to_string(),
                    }));
                }
            }
        }

        Ok(())
    }

    /// 检查版本兼容性
    fn is_version_compatible(
        &self,
        installed_version: &PluginVersion,
        requirement: &VersionRequirement,
    ) -> bool {
        match requirement {
            VersionRequirement::Exact(version) => installed_version == version,
            VersionRequirement::Minimum(min_version) => installed_version >= min_version,
            VersionRequirement::Maximum(max_version) => installed_version <= max_version,
            VersionRequirement::Range(min_version, max_version) => {
                installed_version >= min_version && installed_version <= max_version
            }
            VersionRequirement::Any => true,
        }
    }

    /// 验证插件签名
    fn verify_plugin_signature(&self, plugin: &DiscoveredPlugin) -> Result<bool, VmError> {
        // 简化实现，总是返回true
        // 实际实现应该验证插件的数字签名
        Ok(true)
    }

    /// 更新索引
    fn update_index(&mut self, plugin: &RegisteredPlugin) {
        let plugin_id = &plugin.metadata.id;

        // 按名称索引
        self.index.by_name.insert(plugin.metadata.name.clone(), plugin_id.clone());

        // 按作者索引
        self.index.by_author
            .entry(plugin.metadata.author.clone())
            .or_default()
            .push(plugin_id.clone());

        // 按标签索引 - 暂时跳过，因为 PluginMetadata 没有 tags 字段
        // for tag in &plugin.metadata.tags {
        //     self.index.by_tag
        //         .entry(tag.clone())
        //         .or_default()
        //         .push(plugin_id.clone());
        // }

        // 按依赖索引
        for (dep_id, _) in &plugin.metadata.dependencies {
            self.index.by_dependency
                .entry(dep_id.clone())
                .or_default()
                .push(plugin_id.clone());
        }
    }

    /// 从索引中移除
    fn remove_from_index(&mut self, plugin_id: &PluginId) {
        // 从名称索引移除
        self.index.by_name.retain(|_, id| id != plugin_id);

        // 从作者索引移除
        for plugins in self.index.by_author.values_mut() {
            plugins.retain(|id| id != plugin_id);
        }

        // 从标签索引移除
        for plugins in self.index.by_tag.values_mut() {
            plugins.retain(|id| id != plugin_id);
        }

        // 从依赖索引移除
        for plugins in self.index.by_dependency.values_mut() {
            plugins.retain(|id| id != plugin_id);
        }
    }

    /// 检查插件是否匹配查询
    fn matches_query(&self, plugin: &RegisteredPlugin, query: &PluginSearchQuery) -> bool {
        // 检查名称
        if let Some(name) = &query.name {
            if !plugin.metadata.name.contains(name) {
                return false;
            }
        }

        // 检查作者
        if let Some(author) = &query.author {
            if !plugin.metadata.author.contains(author) {
                return false;
            }
        }

        // 检查类型
        if let Some(plugin_type) = &query.plugin_type {
            if &plugin.metadata.plugin_type != plugin_type {
                return false;
            }
        }

        // 检查标签 - 暂时跳过，因为 PluginMetadata 没有 tags 字段
        // if !query.tags.is_empty() {
        //     for tag in &query.tags {
        //         if !plugin.metadata.tags.contains(tag) {
        //             return false;
        //         }
        //     }
        // }

        // 检查状态
        if let Some(state) = &query.state {
            if plugin.state != *state {
                return false;
            }
        }

        true
    }

    /// 更新插件状态
    pub fn update_plugin_state(&mut self, plugin_id: &PluginId, state: PluginState) -> Result<(), VmError> {
        if let Some(plugin) = self.plugins.get_mut(plugin_id) {
            plugin.state = state;
            Ok(())
        } else {
            Err(VmError::Core(vm_core::CoreError::InvalidState {
                message: format!("Plugin {} not found", plugin_id),
                current: "not_found".to_string(),
                expected: "found".to_string(),
            }))
        }
    }

    /// 更新插件统计信息
    pub fn update_plugin_stats<F>(&mut self, plugin_id: &PluginId, updater: F) -> Result<(), VmError>
    where
        F: FnOnce(&mut PluginStats),
    {
        if let Some(plugin) = self.plugins.get_mut(plugin_id) {
            updater(&mut plugin.stats);
            Ok(())
        } else {
            Err(VmError::Core(vm_core::CoreError::InvalidState {
                message: format!("Plugin {} not found", plugin_id),
                current: "not_found".to_string(),
                expected: "found".to_string(),
            }))
        }
    }

    /// 获取注册表统计信息
    pub fn get_registry_stats(&self) -> RegistryStats {
        let total_plugins = self.plugins.len();
        let mut stats_by_type = HashMap::new();
        let mut stats_by_state = HashMap::new();

        for plugin in self.plugins.values() {
            // 按类型统计
            *stats_by_type.entry(plugin.metadata.plugin_type).or_insert(0) += 1;
            
            // 按状态统计
            *stats_by_state.entry(plugin.state).or_insert(0) += 1;
        }

        RegistryStats {
            total_plugins,
            stats_by_type,
            stats_by_state,
            total_extension_points: self.plugins_by_extension_point.len(),
        }
    }
}

/// 插件搜索查询
#[derive(Debug, Clone, Default)]
pub struct PluginSearchQuery {
    /// 插件名称
    pub name: Option<String>,
    /// 插件作者
    pub author: Option<String>,
    /// 插件类型
    pub plugin_type: Option<PluginType>,
    /// 插件标签
    pub tags: Vec<String>,
    /// 插件状态
    pub state: Option<PluginState>,
}

/// 注册表统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryStats {
    /// 总插件数量
    pub total_plugins: usize,
    /// 按类型统计
    pub stats_by_type: HashMap<PluginType, usize>,
    /// 按状态统计
    pub stats_by_state: HashMap<PluginState, usize>,
    /// 总扩展点数量
    pub total_extension_points: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_registry_creation() {
        let config = RegistryConfig::default();
        let registry = PluginRegistry::new(config);
        assert_eq!(registry.plugins.len(), 0);
    }

    #[test]
    fn test_plugin_registration() {
        let config = RegistryConfig::default();
        let mut registry = PluginRegistry::new(config);

        let plugin = create_test_plugin();
        let discovered = DiscoveredPlugin {
            metadata: plugin.clone(),
            path: PathBuf::from("test.plugin"),
            bytecode: vec![1, 2, 3],
            discovered_at: std::time::Instant::now(),
        };

        let plugin_id = registry.register_plugin(discovered, vec![]).map_err(|e| {
            std::format!("Failed to register plugin: {:?}", e)
        }).expect("Plugin registration should succeed");
        assert_eq!(plugin_id, plugin.id);
        assert_eq!(registry.plugins.len(), 1);
    }

    #[test]
    fn test_plugin_search() {
        let config = RegistryConfig::default();
        let mut registry = PluginRegistry::new(config);

        let plugin = create_test_plugin();
        let discovered = DiscoveredPlugin {
            metadata: plugin.clone(),
            path: PathBuf::from("test.plugin"),
            bytecode: vec![1, 2, 3],
            discovered_at: std::time::Instant::now(),
        };

        registry.register_plugin(discovered, vec![]).map_err(|e| {
            std::format!("Failed to register plugin: {:?}", e)
        }).expect("Plugin registration should succeed");

        let query = PluginSearchQuery {
            name: Some("Test".to_string()),
            ..Default::default()
        };

        let results = registry.search_plugins(&query);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].metadata.name, "Test Plugin");
    }

    fn create_test_plugin() -> PluginMetadata {
        PluginMetadata {
            id: PluginId::from("test-plugin"),
            name: "Test Plugin".to_string(),
            version: PluginVersion { major: 1, minor: 0, patch: 0 },
            description: "A test plugin".to_string(),
            author: "Test Author".to_string(),
            license: "MIT".to_string(),
            dependencies: HashMap::new(),
            plugin_type: PluginType::Custom,
            entry_point: "test_entry".to_string(),
            permissions: vec![],
        }
    }
}