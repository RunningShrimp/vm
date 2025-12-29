//! 动态代码热替换模块
//!
//! 实现代码版本管理和回滚机制

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use vm_core::VmError;

/// Helper function to get current timestamp safely
fn get_current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 代码版本标识
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CodeVersion(u64);

impl CodeVersion {
    pub fn new(version: u64) -> Self {
        Self(version)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }

    pub fn next(&self) -> Self {
        Self(self.0 + 1)
    }
}

/// 编译后的代码块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledCodeBlock {
    /// 代码版本
    pub version: CodeVersion,
    /// 编译时间戳
    pub timestamp: u64,
    /// IR 块哈希（用于识别相同的代码）
    pub ir_hash: u64,
    /// 机器码
    pub code: Vec<u8>,
    /// 入口点地址
    pub entry_point: usize,
    /// 代码大小
    pub size: usize,
    /// 编译选项
    pub compile_flags: u32,
    /// 是否启用
    pub enabled: bool,
}

impl CompiledCodeBlock {
    pub fn new(version: CodeVersion, ir_hash: u64, code: Vec<u8>, entry_point: usize) -> Self {
        let timestamp = get_current_timestamp();

        Self {
            version,
            timestamp,
            ir_hash,
            size: code.len(),
            code,
            entry_point,
            compile_flags: 0,
            enabled: true,
        }
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }
}

/// 版本历史记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionHistory {
    /// 版本号
    pub version: CodeVersion,
    /// 时间戳
    pub timestamp: u64,
    /// 变更类型
    pub change_type: VersionChangeType,
    /// 描述
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VersionChangeType {
    /// 初始版本
    Initial,
    /// 更新版本
    Update,
    /// 回滚版本
    Rollback { from: CodeVersion },
    /// 禁用版本
    Disable,
    /// 启用版本
    Enable,
}

/// 热替换配置
#[derive(Debug, Clone)]
pub struct HotReloadConfig {
    /// 最大保留版本数
    pub max_versions: usize,
    /// 启用自动回滚
    pub auto_rollback: bool,
    /// 回滚触发条件
    pub rollback_threshold: RollbackThreshold,
}

#[derive(Debug, Clone)]
pub enum RollbackThreshold {
    /// 基于错误率
    ErrorRate { max_errors_per_minute: u32 },
    /// 基于性能下降
    PerformanceDrop { threshold_percent: f64 },
    /// 手动回滚
    Manual,
}

impl Default for HotReloadConfig {
    fn default() -> Self {
        Self {
            max_versions: 10,
            auto_rollback: false,
            rollback_threshold: RollbackThreshold::Manual,
        }
    }
}

/// 代码版本管理器
pub struct CodeVersionManager {
    /// 当前版本
    current_version: CodeVersion,
    /// 代码存储（按版本号索引）
    code_store: HashMap<CodeVersion, Arc<CompiledCodeBlock>>,
    /// IR 哈希到版本的映射（用于快速查找）
    hash_to_version: HashMap<u64, CodeVersion>,
    /// 版本历史
    history: Vec<VersionHistory>,
    /// 配置
    config: HotReloadConfig,
}

impl CodeVersionManager {
    pub fn new(config: HotReloadConfig) -> Self {
        Self {
            current_version: CodeVersion::new(0),
            code_store: HashMap::new(),
            hash_to_version: HashMap::new(),
            history: Vec::new(),
            config,
        }
    }

    /// 注册新的代码版本
    pub fn register_version(&mut self, code: CompiledCodeBlock) -> Result<CodeVersion, VmError> {
        let version = code.version;

        if self.code_store.contains_key(&version) {
            return Err(VmError::Core(vm_core::CoreError::InvalidState {
                message: format!("Version {} already exists", version.0),
                current: "version_exists".to_string(),
                expected: "new_version".to_string(),
            }));
        }

        let code_arc = Arc::new(code);

        self.code_store.insert(version, Arc::clone(&code_arc));
        self.hash_to_version.insert(code_arc.ir_hash, version);

        self.current_version = version;

        self.history.push(VersionHistory {
            version,
            timestamp: code_arc.timestamp,
            change_type: VersionChangeType::Update,
            description: format!("Registered version {}", version.0),
        });

        self.cleanup_old_versions();

        Ok(version)
    }

    /// 获取当前版本的代码
    pub fn get_current(&self) -> Option<Arc<CompiledCodeBlock>> {
        self.code_store.get(&self.current_version).cloned()
    }

    /// 获取指定版本的代码
    pub fn get_version(&self, version: CodeVersion) -> Option<Arc<CompiledCodeBlock>> {
        self.code_store.get(&version).cloned()
    }

    /// 通过 IR 哈希查找代码
    pub fn find_by_hash(&self, ir_hash: u64) -> Option<Arc<CompiledCodeBlock>> {
        self.hash_to_version
            .get(&ir_hash)
            .and_then(|v| self.code_store.get(v).cloned())
    }

    /// 切换到指定版本
    pub fn switch_version(&mut self, version: CodeVersion) -> Result<(), VmError> {
        if !self.code_store.contains_key(&version) {
            return Err(VmError::Core(vm_core::CoreError::InvalidState {
                message: format!("Version {} does not exist", version.0),
                current: "current_version".to_string(),
                expected: "existing_version".to_string(),
            }));
        }

        self.current_version = version;

        self.history.push(VersionHistory {
            version,
            timestamp: get_current_timestamp(),
            change_type: VersionChangeType::Update,
            description: format!("Switched to version {}", version.0),
        });

        Ok(())
    }

    /// 回滚到上一个版本
    pub fn rollback(&mut self) -> Result<CodeVersion, VmError> {
        if self.history.len() < 2 {
            return Err(VmError::Core(vm_core::CoreError::InvalidState {
                message: "No previous version to rollback to".to_string(),
                current: "history_too_short".to_string(),
                expected: "history_with_previous_version".to_string(),
            }));
        }

        let prev_history = &self.history[self.history.len() - 2];
        let target_version = prev_history.version;

        if !self.code_store.contains_key(&target_version) {
            return Err(VmError::Core(vm_core::CoreError::InvalidState {
                message: format!("Version {} no longer available", target_version.0),
                current: "version_missing".to_string(),
                expected: "version_available".to_string(),
            }));
        }

        self.current_version = target_version;

        let from_version = self.history.last().map(|h| h.version).ok_or_else(|| {
            VmError::Core(vm_core::CoreError::InvalidState {
                message: "History is empty".to_string(),
                current: "empty_history".to_string(),
                expected: "non_empty_history".to_string(),
            })
        })?;

        self.history.push(VersionHistory {
            version: target_version,
            timestamp: get_current_timestamp(),
            change_type: VersionChangeType::Rollback { from: from_version },
            description: format!("Rolled back to version {}", target_version.0),
        });

        Ok(target_version)
    }

    /// 禁用指定版本
    pub fn disable_version(&mut self, version: CodeVersion) -> Result<(), VmError> {
        let code = self.code_store.get_mut(&version).ok_or_else(|| {
            VmError::Core(vm_core::CoreError::InvalidState {
                message: format!("Version {} does not exist", version.0),
                current: "version_missing".to_string(),
                expected: "version_exists".to_string(),
            })
        })?;

        let code = Arc::make_mut(code);

        code.disable();

        self.history.push(VersionHistory {
            version,
            timestamp: get_current_timestamp(),
            change_type: VersionChangeType::Disable,
            description: format!("Disabled version {}", version.0),
        });

        Ok(())
    }

    /// 启用指定版本
    pub fn enable_version(&mut self, version: CodeVersion) -> Result<(), VmError> {
        let code = self.code_store.get_mut(&version).ok_or_else(|| {
            VmError::Core(vm_core::CoreError::InvalidState {
                message: format!("Version {} does not exist", version.0),
                current: "version_missing".to_string(),
                expected: "version_exists".to_string(),
            })
        })?;

        let code = Arc::make_mut(code);

        code.enable();

        self.history.push(VersionHistory {
            version,
            timestamp: get_current_timestamp(),
            change_type: VersionChangeType::Enable,
            description: format!("Enabled version {}", version.0),
        });

        Ok(())
    }

    /// 清理旧版本
    fn cleanup_old_versions(&mut self) {
        while self.code_store.len() > self.config.max_versions {
            let oldest_version = *self
                .code_store
                .keys()
                .min()
                .expect("code_store should not be empty when cleanup_old_versions is called");
            self.code_store.remove(&oldest_version);
        }
    }

    /// 获取当前版本号
    pub fn current_version(&self) -> CodeVersion {
        self.current_version
    }

    /// 获取版本历史
    pub fn history(&self) -> &[VersionHistory] {
        &self.history
    }

    /// 获取所有版本
    pub fn all_versions(&self) -> Vec<CodeVersion> {
        let mut versions: Vec<_> = self.code_store.keys().copied().collect();
        versions.sort_by_key(|v| std::cmp::Reverse(v.0));
        versions
    }
}

/// 线程安全的代码版本管理器
pub struct ThreadSafeCodeVersionManager {
    inner: RwLock<CodeVersionManager>,
}

impl ThreadSafeCodeVersionManager {
    pub fn new(config: HotReloadConfig) -> Self {
        Self {
            inner: RwLock::new(CodeVersionManager::new(config)),
        }
    }

    /// Acquire read lock with proper error handling
    fn read_lock(&self) -> Result<std::sync::RwLockReadGuard<'_, CodeVersionManager>, VmError> {
        self.inner.read().map_err(|e| {
            VmError::Core(vm_core::CoreError::InvalidState {
                message: format!("Failed to acquire read lock: {}", e),
                current: "lock_poisoned".to_string(),
                expected: "valid_lock".to_string(),
            })
        })
    }

    /// Acquire write lock with proper error handling
    fn write_lock(&self) -> Result<std::sync::RwLockWriteGuard<'_, CodeVersionManager>, VmError> {
        self.inner.write().map_err(|e| {
            VmError::Core(vm_core::CoreError::InvalidState {
                message: format!("Failed to acquire write lock: {}", e),
                current: "lock_poisoned".to_string(),
                expected: "valid_lock".to_string(),
            })
        })
    }

    pub fn register_version(&self, code: CompiledCodeBlock) -> Result<CodeVersion, VmError> {
        let mut inner = self.write_lock()?;
        inner.register_version(code)
    }

    pub fn get_current(&self) -> Option<Arc<CompiledCodeBlock>> {
        let inner = self.read_lock().ok()?;
        inner.get_current()
    }

    pub fn get_version(&self, version: CodeVersion) -> Option<Arc<CompiledCodeBlock>> {
        let inner = self.read_lock().ok()?;
        inner.get_version(version)
    }

    pub fn find_by_hash(&self, ir_hash: u64) -> Option<Arc<CompiledCodeBlock>> {
        let inner = self.read_lock().ok()?;
        inner.find_by_hash(ir_hash)
    }

    pub fn switch_version(&self, version: CodeVersion) -> Result<(), VmError> {
        let mut inner = self.write_lock()?;
        inner.switch_version(version)
    }

    pub fn rollback(&self) -> Result<CodeVersion, VmError> {
        let mut inner = self.write_lock()?;
        inner.rollback()
    }

    pub fn current_version(&self) -> CodeVersion {
        let inner = self.read_lock().unwrap_or_else(|e| {
            panic!("Failed to acquire read lock for current_version - this may indicate a poisoned lock: {}", e)
        });
        inner.current_version()
    }

    pub fn history(&self) -> Vec<VersionHistory> {
        let inner = self.read_lock().unwrap_or_else(|e| {
            panic!(
                "Failed to acquire read lock for history - this may indicate a poisoned lock: {}",
                e
            )
        });
        inner.history().to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_version_manager() {
        let config = HotReloadConfig::default();
        let mut manager = CodeVersionManager::new(config);

        let code1 =
            CompiledCodeBlock::new(CodeVersion::new(1), 0x12345678, vec![0x90, 0x90, 0xC3], 0);

        let version = manager
            .register_version(code1)
            .expect("Failed to register version 1 in test_code_version_manager");
        assert_eq!(version, CodeVersion::new(1));
        assert_eq!(manager.current_version(), CodeVersion::new(1));
    }

    #[test]
    fn test_version_switch() {
        let config = HotReloadConfig::default();
        let mut manager = CodeVersionManager::new(config);

        let code1 =
            CompiledCodeBlock::new(CodeVersion::new(1), 0x12345678, vec![0x90, 0x90, 0xC3], 0);
        let code2 = CompiledCodeBlock::new(
            CodeVersion::new(2),
            0x87654321,
            vec![0x90, 0x90, 0x90, 0xC3],
            0,
        );

        manager
            .register_version(code1)
            .expect("Failed to register version 1 in test_version_switch");
        manager
            .register_version(code2)
            .expect("Failed to register version 2 in test_version_switch");

        manager
            .switch_version(CodeVersion::new(1))
            .expect("Failed to switch to version 1 in test_version_switch");
        assert_eq!(manager.current_version(), CodeVersion::new(1));

        manager
            .switch_version(CodeVersion::new(2))
            .expect("Failed to switch to version 2 in test_version_switch");
        assert_eq!(manager.current_version(), CodeVersion::new(2));
    }

    #[test]
    fn test_version_rollback() {
        let config = HotReloadConfig::default();
        let mut manager = CodeVersionManager::new(config);

        let code1 =
            CompiledCodeBlock::new(CodeVersion::new(1), 0x12345678, vec![0x90, 0x90, 0xC3], 0);
        let code2 = CompiledCodeBlock::new(
            CodeVersion::new(2),
            0x87654321,
            vec![0x90, 0x90, 0x90, 0xC3],
            0,
        );

        manager
            .register_version(code1)
            .expect("Failed to register version 1 in test_version_rollback");
        manager
            .register_version(code2)
            .expect("Failed to register version 2 in test_version_rollback");

        manager
            .rollback()
            .expect("Failed to rollback in test_version_rollback");
        assert_eq!(manager.current_version(), CodeVersion::new(1));
    }

    #[test]
    fn test_find_by_hash() {
        let config = HotReloadConfig::default();
        let mut manager = CodeVersionManager::new(config);

        let code1 =
            CompiledCodeBlock::new(CodeVersion::new(1), 0x12345678, vec![0x90, 0x90, 0xC3], 0);

        manager
            .register_version(code1)
            .expect("Failed to register version 1 in test_find_by_hash");

        let found = manager.find_by_hash(0x12345678);
        assert!(found.is_some());
        assert_eq!(
            found
                .expect("Expected to find code block with hash 0x12345678 in test_find_by_hash")
                .ir_hash,
            0x12345678
        );
    }
}
