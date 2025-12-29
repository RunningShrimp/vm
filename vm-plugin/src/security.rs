//! 插件安全沙箱
//!
//! 提供插件执行的安全隔离和权限控制

use crate::PluginPermission;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use vm_core::VmError;

/// 安全管理器
pub struct SecurityManager {
    /// 允许的权限
    allowed_permissions: HashSet<PluginPermission>,
    /// 沙箱配置
    sandbox_config: SandboxConfig,
    /// 权限策略
    permission_policy: PermissionPolicy,
}

/// 沙箱配置
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// 是否启用沙箱
    pub enabled: bool,
    /// 内存限制（字节）
    pub memory_limit: Option<u64>,
    /// CPU时间限制（秒）
    pub cpu_time_limit: Option<u64>,
    /// 文件系统访问限制
    pub filesystem_restricted: bool,
    /// 网络访问限制
    pub network_restricted: bool,
    /// 允许的文件路径
    pub allowed_paths: HashSet<String>,
    /// 禁止的文件路径
    pub forbidden_paths: HashSet<String>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            memory_limit: Some(256 * 1024 * 1024), // 256MB
            cpu_time_limit: Some(60),              // 60秒
            filesystem_restricted: true,
            network_restricted: true,
            allowed_paths: HashSet::new(),
            forbidden_paths: HashSet::new(),
        }
    }
}

/// 权限策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionPolicy {
    /// 白名单策略（只允许明确授权的权限）
    Whitelist,
    /// 黑名单策略（禁止明确拒绝的权限）
    Blacklist,
}

impl Default for SecurityManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SecurityManager {
    pub fn new() -> Self {
        Self {
            allowed_permissions: HashSet::from([
                PluginPermission::ReadVmState,
                PluginPermission::NetworkAccess,
                PluginPermission::FileSystemAccess,
            ]),
            sandbox_config: SandboxConfig::default(),
            permission_policy: PermissionPolicy::Whitelist,
        }
    }

    /// 使用自定义配置创建安全管理器
    pub fn with_config(config: SandboxConfig) -> Self {
        Self {
            allowed_permissions: HashSet::new(),
            sandbox_config: config,
            permission_policy: PermissionPolicy::Whitelist,
        }
    }

    /// 检查权限
    pub fn check_permissions(&self, permissions: &[PluginPermission]) -> Result<(), VmError> {
        match self.permission_policy {
            PermissionPolicy::Whitelist => {
                // 白名单：所有请求的权限都必须在允许列表中
                for permission in permissions {
                    if !self.allowed_permissions.contains(permission) {
                        return Err(VmError::Core(vm_core::CoreError::InvalidState {
                            message: format!("Permission denied: {:?}", permission),
                            current: "denied".to_string(),
                            expected: "allowed".to_string(),
                        }));
                    }
                }
            }
            PermissionPolicy::Blacklist => {
                // 黑名单：检查是否有禁止的权限
                // 这里简化实现，实际应该维护一个禁止列表
            }
        }
        Ok(())
    }

    /// 添加允许的权限
    pub fn add_permission(&mut self, permission: PluginPermission) {
        self.allowed_permissions.insert(permission);
    }

    /// 移除权限
    pub fn remove_permission(&mut self, permission: &PluginPermission) {
        self.allowed_permissions.remove(permission);
    }

    /// 检查文件系统访问权限
    pub fn check_filesystem_access(&self, path: &str) -> Result<(), VmError> {
        if !self.sandbox_config.filesystem_restricted {
            return Ok(());
        }

        // 检查禁止路径
        for forbidden in &self.sandbox_config.forbidden_paths {
            if path.starts_with(forbidden) {
                return Err(VmError::Core(vm_core::CoreError::InvalidState {
                    message: format!("Access to path {} is forbidden", path),
                    current: "forbidden".to_string(),
                    expected: "allowed".to_string(),
                }));
            }
        }

        // 如果设置了允许路径，检查是否在允许列表中
        if !self.sandbox_config.allowed_paths.is_empty() {
            let mut allowed = false;
            for allowed_path in &self.sandbox_config.allowed_paths {
                if path.starts_with(allowed_path) {
                    allowed = true;
                    break;
                }
            }
            if !allowed {
                return Err(VmError::Core(vm_core::CoreError::InvalidState {
                    message: format!("Access to path {} is not allowed", path),
                    current: "not_allowed".to_string(),
                    expected: "allowed".to_string(),
                }));
            }
        }

        Ok(())
    }

    /// 检查网络访问权限
    pub fn check_network_access(&self) -> Result<(), VmError> {
        if self.sandbox_config.network_restricted {
            return Err(VmError::Core(vm_core::CoreError::InvalidState {
                message: "Network access is restricted".to_string(),
                current: "restricted".to_string(),
                expected: "allowed".to_string(),
            }));
        }
        Ok(())
    }

    /// 检查内存使用限制
    pub fn check_memory_limit(&self, current_usage: u64) -> Result<(), VmError> {
        if let Some(limit) = self.sandbox_config.memory_limit
            && current_usage > limit
        {
            return Err(VmError::Core(vm_core::CoreError::InvalidState {
                message: format!("Memory limit exceeded: {} > {}", current_usage, limit),
                current: "exceeded".to_string(),
                expected: "within_limit".to_string(),
            }));
        }
        Ok(())
    }

    /// 获取沙箱配置
    pub fn get_sandbox_config(&self) -> &SandboxConfig {
        &self.sandbox_config
    }

    /// 设置沙箱配置
    pub fn set_sandbox_config(&mut self, config: SandboxConfig) {
        self.sandbox_config = config;
    }

    /// 设置权限策略
    pub fn set_permission_policy(&mut self, policy: PermissionPolicy) {
        self.permission_policy = policy;
    }
}

/// 插件资源监控器
pub struct PluginResourceMonitor {
    /// 插件内存使用
    memory_usage: Arc<RwLock<HashMap<String, u64>>>,
    /// 插件CPU时间
    cpu_time: Arc<RwLock<HashMap<String, u64>>>,
    /// 安全管理器引用
    security_manager: Arc<RwLock<SecurityManager>>,
}

impl PluginResourceMonitor {
    pub fn new(security_manager: Arc<RwLock<SecurityManager>>) -> Self {
        Self {
            memory_usage: Arc::new(RwLock::new(HashMap::new())),
            cpu_time: Arc::new(RwLock::new(HashMap::new())),
            security_manager,
        }
    }

    /// 记录内存使用
    pub fn record_memory_usage(&self, plugin_id: &str, usage: u64) -> Result<(), VmError> {
        {
            let mut memory = self.memory_usage.write().map_err(|e| {
                VmError::Core(vm_core::CoreError::InvalidState {
                    message: format!("Memory usage lock is poisoned: {:?}", e),
                    current: "poisoned".to_string(),
                    expected: "unlocked".to_string(),
                })
            })?;
            memory.insert(plugin_id.to_string(), usage);
        }

        // 检查内存限制
        let security = self.security_manager.read().map_err(|e| {
            VmError::Core(vm_core::CoreError::InvalidState {
                message: format!("Security manager lock is poisoned: {:?}", e),
                current: "poisoned".to_string(),
                expected: "unlocked".to_string(),
            })
        })?;
        security.check_memory_limit(usage)?;

        Ok(())
    }

    /// 记录CPU时间
    pub fn record_cpu_time(&self, plugin_id: &str, time: u64) {
        if let Ok(mut cpu) = self.cpu_time.write() {
            cpu.insert(plugin_id.to_string(), time);
        }
    }

    /// 获取内存使用
    pub fn get_memory_usage(&self, plugin_id: &str) -> Option<u64> {
        self.memory_usage.read().ok()?.get(plugin_id).copied()
    }

    /// 获取CPU时间
    pub fn get_cpu_time(&self, plugin_id: &str) -> Option<u64> {
        self.cpu_time.read().ok()?.get(plugin_id).copied()
    }

    /// 清理插件资源记录
    pub fn cleanup(&self, plugin_id: &str) {
        if let Ok(mut memory) = self.memory_usage.write() {
            memory.remove(plugin_id);
        }

        if let Ok(mut cpu) = self.cpu_time.write() {
            cpu.remove(plugin_id);
        }
    }
}
