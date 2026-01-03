//! 插件依赖管理
//!
//! 提供插件依赖关系解析和版本兼容性检查

use std::collections::HashMap;

use vm_core::VmError;

use crate::{PluginId, PluginMetadata, PluginVersion};

/// 依赖解析器
pub struct DependencyResolver {
    /// 已安装的插件及其版本
    installed_plugins: HashMap<PluginId, PluginVersion>,
}

impl Default for DependencyResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl DependencyResolver {
    pub fn new() -> Self {
        Self {
            installed_plugins: HashMap::new(),
        }
    }

    /// 解析插件依赖
    pub fn resolve_dependencies(&mut self, metadata: &PluginMetadata) -> Result<(), VmError> {
        for (dep_id, required_version) in &metadata.dependencies {
            if let Some(installed_version) = self.installed_plugins.get(dep_id) {
                if installed_version < required_version {
                    return Err(VmError::Core(vm_core::CoreError::InvalidState {
                        message: format!(
                            "Dependency {} version {} required, but {} installed",
                            dep_id, required_version, installed_version
                        ),
                        current: installed_version.to_string(),
                        expected: required_version.to_string(),
                    }));
                }
            } else {
                return Err(VmError::Core(vm_core::CoreError::InvalidState {
                    message: format!("Missing dependency: {}", dep_id),
                    current: "not_installed".to_string(),
                    expected: "installed".to_string(),
                }));
            }
        }

        // 记录已安装的插件
        self.installed_plugins
            .insert(metadata.id.clone(), metadata.version.clone());

        Ok(())
    }

    /// 注册已安装的插件
    pub fn register_plugin(&mut self, plugin_id: PluginId, version: PluginVersion) {
        self.installed_plugins.insert(plugin_id, version);
    }

    /// 卸载插件
    pub fn unregister_plugin(&mut self, plugin_id: &str) {
        self.installed_plugins.remove(plugin_id);
    }

    /// 检查依赖是否满足
    pub fn check_dependencies(&self, metadata: &PluginMetadata) -> Result<(), VmError> {
        for (dep_id, required_version) in &metadata.dependencies {
            if let Some(installed_version) = self.installed_plugins.get(dep_id) {
                if installed_version < required_version {
                    return Err(VmError::Core(vm_core::CoreError::InvalidState {
                        message: format!(
                            "Dependency {} version {} required, but {} installed",
                            dep_id, required_version, installed_version
                        ),
                        current: installed_version.to_string(),
                        expected: required_version.to_string(),
                    }));
                }
            } else {
                return Err(VmError::Core(vm_core::CoreError::InvalidState {
                    message: format!("Missing dependency: {}", dep_id),
                    current: "not_installed".to_string(),
                    expected: "installed".to_string(),
                }));
            }
        }
        Ok(())
    }

    /// 获取所有已安装的插件
    pub fn get_installed_plugins(&self) -> &HashMap<PluginId, PluginVersion> {
        &self.installed_plugins
    }
}
