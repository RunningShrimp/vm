//! # 插件配置和管理机制
//!
//! 提供插件的配置管理、验证、热更新和存储功能。

use crate::{PluginId, PluginMetadata};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use vm_core::VmError;

/// 插件配置管理器
pub struct PluginConfigManager {
    /// 配置存储
    storage: Arc<dyn ConfigStorage>,
    /// 配置验证器
    validator: Arc<dyn ConfigValidator>,
    /// 配置变更监听器
    change_listeners: Arc<RwLock<Vec<Box<dyn ConfigChangeListener>>>>,
    /// 配置缓存
    config_cache: Arc<RwLock<HashMap<PluginId, PluginConfig>>>,
    /// 管理器配置
    config: ConfigManagerConfig,
}

/// 配置管理器配置
#[derive(Debug, Clone)]
pub struct ConfigManagerConfig {
    /// 是否启用配置缓存
    pub enable_cache: bool,
    /// 缓存过期时间（秒）
    pub cache_ttl: u64,
    /// 是否启用配置热更新
    pub enable_hot_reload: bool,
    /// 配置文件监控间隔（毫秒）
    pub watch_interval: u64,
    /// 是否启用配置备份
    pub enable_backup: bool,
    /// 备份保留数量
    pub backup_retention: usize,
    /// 配置文件格式
    pub config_format: ConfigFormat,
}

impl Default for ConfigManagerConfig {
    fn default() -> Self {
        Self {
            enable_cache: true,
            cache_ttl: 300, // 5分钟
            enable_hot_reload: true,
            watch_interval: 1000, // 1秒
            enable_backup: true,
            backup_retention: 10,
            config_format: ConfigFormat::Json,
        }
    }
}

/// 配置格式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    /// JSON格式
    Json,
    /// TOML格式
    Toml,
    /// YAML格式
    Yaml,
    /// XML格式
    Xml,
}

impl ConfigFormat {
    /// 获取文件扩展名
    pub fn extension(&self) -> &'static str {
        match self {
            ConfigFormat::Json => "json",
            ConfigFormat::Toml => "toml",
            ConfigFormat::Yaml => "yaml",
            ConfigFormat::Xml => "xml",
        }
    }

    /// 获取MIME类型
    pub fn mime_type(&self) -> &'static str {
        match self {
            ConfigFormat::Json => "application/json",
            ConfigFormat::Toml => "application/toml",
            ConfigFormat::Yaml => "application/yaml",
            ConfigFormat::Xml => "application/xml",
        }
    }
}

/// 配置存储接口
#[async_trait::async_trait]
pub trait ConfigStorage: Send + Sync {
    /// 保存配置
    async fn save_config(&self, plugin_id: &PluginId, config: &PluginConfig) -> Result<(), ConfigStorageError>;
    
    /// 加载配置
    async fn load_config(&self, plugin_id: &PluginId) -> Result<Option<PluginConfig>, ConfigStorageError>;
    
    /// 删除配置
    async fn delete_config(&self, plugin_id: &PluginId) -> Result<(), ConfigStorageError>;
    
    /// 列出所有配置
    async fn list_configs(&self) -> Result<Vec<(PluginId, PluginConfig)>, ConfigStorageError>;
    
    /// 备份配置
    async fn backup_config(&self, plugin_id: &PluginId) -> Result<String, ConfigStorageError>;
    
    /// 恢复配置
    async fn restore_config(&self, plugin_id: &PluginId, backup_id: &str) -> Result<(), ConfigStorageError>;
    
    /// 获取配置历史
    async fn get_config_history(&self, plugin_id: &PluginId) -> Result<Vec<ConfigVersion>, ConfigStorageError>;
    
    /// 清理过期备份
    async fn cleanup_backups(&self) -> Result<(), ConfigStorageError>;
}

/// 配置验证器接口
#[async_trait::async_trait]
pub trait ConfigValidator: Send + Sync {
    /// 验证配置
    async fn validate_config(&self, plugin_id: &PluginId, config: &PluginConfig) -> Result<ValidationResult, ConfigValidationError>;
    
    /// 获取配置模式
    fn get_config_schema(&self, plugin_id: &PluginId) -> Result<ConfigSchema, ConfigValidationError>;
    
    /// 获取默认配置
    fn get_default_config(&self, plugin_id: &PluginId) -> Result<PluginConfig, ConfigValidationError>;
    
    /// 验证配置更新
    async fn validate_config_update(&self, plugin_id: &PluginId, old_config: &PluginConfig, new_config: &PluginConfig) -> Result<ValidationResult, ConfigValidationError>;
}

/// 配置变更监听器接口
pub trait ConfigChangeListener: Send + Sync {
    /// 处理配置变更
    fn on_config_changed(&self, event: &ConfigChangeEvent);
}

/// 配置变更事件
#[derive(Debug, Clone)]
pub struct ConfigChangeEvent {
    /// 事件ID
    pub id: String,
    /// 插件ID
    pub plugin_id: PluginId,
    /// 事件类型
    pub event_type: ConfigChangeType,
    /// 旧配置
    pub old_config: Option<PluginConfig>,
    /// 新配置
    pub new_config: PluginConfig,
    /// 变更时间
    pub timestamp: std::time::SystemTime,
    /// 变更来源
    pub source: String,
    /// 变更描述
    pub description: String,
    /// 变更详情
    pub details: HashMap<String, String>,
}

/// 配置变更类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigChangeType {
    /// 创建配置
    Created,
    /// 更新配置
    Updated,
    /// 删除配置
    Deleted,
    /// 恢复配置
    Restored,
    /// 备份配置
    BackedUp,
}

/// 插件配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// 插件ID
    pub plugin_id: PluginId,
    /// 配置版本
    pub version: u32,
    /// 配置数据
    pub data: HashMap<String, ConfigValue>,
    /// 配置元数据
    pub metadata: ConfigMetadata,
}

/// 配置值
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConfigValue {
    /// 布尔值
    Bool(bool),
    /// 整数
    Integer(i64),
    /// 浮点数
    Float(f64),
    /// 字符串
    String(String),
    /// 数组
    Array(Vec<ConfigValue>),
    /// 对象
    Object(HashMap<String, ConfigValue>),
    /// 空值
    Null,
}

/// 配置元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigMetadata {
    /// 创建时间
    pub created_at: std::time::SystemTime,
    /// 更新时间
    pub updated_at: std::time::SystemTime,
    /// 创建者
    pub created_by: String,
    /// 更新者
    pub updated_by: String,
    /// 配置描述
    pub description: Option<String>,
    /// 配置标签
    pub tags: Vec<String>,
    /// 配置模式版本
    pub schema_version: String,
    /// 配置校验和
    pub checksum: Option<String>,
}

impl Default for ConfigMetadata {
    fn default() -> Self {
        let now = std::time::SystemTime::now();
        Self {
            created_at: now,
            updated_at: now,
            created_by: "system".to_string(),
            updated_by: "system".to_string(),
            description: None,
            tags: Vec::new(),
            schema_version: "1.0".to_string(),
            checksum: None,
        }
    }
}

/// 验证结果
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// 是否有效
    pub is_valid: bool,
    /// 错误信息
    pub errors: Vec<ValidationError>,
    /// 警告信息
    pub warnings: Vec<ValidationWarning>,
    /// 验证时间
    pub validated_at: std::time::SystemTime,
}

/// 验证错误
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// 错误代码
    pub code: String,
    /// 错误消息
    pub message: String,
    /// 错误路径
    pub path: String,
    /// 错误值
    pub value: ConfigValue,
    /// 错误详情
    pub details: HashMap<String, String>,
}

/// 验证警告
#[derive(Debug, Clone)]
pub struct ValidationWarning {
    /// 警告代码
    pub code: String,
    /// 警告消息
    pub message: String,
    /// 警告路径
    pub path: String,
    /// 警告值
    pub value: ConfigValue,
    /// 警告详情
    pub details: HashMap<String, String>,
}

/// 配置模式
#[derive(Debug, Clone)]
pub struct ConfigSchema {
    /// 模式版本
    pub version: String,
    /// 模式定义
    pub definition: SchemaDefinition,
    /// 模式元数据
    pub metadata: SchemaMetadata,
}

/// 模式定义
#[derive(Debug, Clone)]
pub struct SchemaDefinition {
    /// 属性定义
    pub properties: HashMap<String, PropertyDefinition>,
    /// 必需属性
    pub required: Vec<String>,
    /// 模式类型
    pub schema_type: SchemaType,
    /// 附加属性
    pub additional_properties: Option<bool>,
}

/// 模式类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaType {
    /// 对象
    Object,
    /// 数组
    Array,
    /// 字符串
    String,
    /// 数字
    Number,
    /// 整数
    Integer,
    /// 布尔值
    Boolean,
    /// 空值
    Null,
}

/// 属性定义
#[derive(Debug, Clone)]
pub struct PropertyDefinition {
    /// 属性类型
    pub property_type: PropertyType,
    /// 是否必需
    pub required: bool,
    /// 默认值
    pub default_value: Option<ConfigValue>,
    /// 验证规则
    pub validation_rules: Vec<ValidationRule>,
    /// 属性描述
    pub description: Option<String>,
    /// 属性标题
    pub title: Option<String>,
}

/// 属性类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyType {
    /// 字符串
    String,
    /// 整数
    Integer,
    /// 数字
    Number,
    /// 布尔值
    Boolean,
    /// 数组
    Array,
    /// 对象
    Object,
    /// 空值
    Null,
}

/// 验证规则
#[derive(Debug, Clone)]
pub enum ValidationRule {
    /// 最小值
    MinValue(f64),
    /// 最大值
    MaxValue(f64),
    /// 最小长度
    MinLength(usize),
    /// 最大长度
    MaxLength(usize),
    /// 正则表达式
    Regex(String),
    /// 枚举值
    Enum(Vec<ConfigValue>),
    /// 自定义验证
    Custom(String),
    /// 必需
    Required,
    /// 格式验证
    Format(String),
}

/// 模式元数据
#[derive(Debug, Clone)]
pub struct SchemaMetadata {
    /// 模式标题
    pub title: Option<String>,
    /// 模式描述
    pub description: Option<String>,
    /// 模式版本
    pub version: String,
    /// 模式作者
    pub author: Option<String>,
}

/// 配置版本
#[derive(Debug, Clone)]
pub struct ConfigVersion {
    /// 版本号
    pub version: u32,
    /// 创建时间
    pub created_at: std::time::SystemTime,
    /// 创建者
    pub created_by: String,
    /// 版本描述
    pub description: Option<String>,
    /// 配置快照
    pub config: PluginConfig,
}

/// 配置存储错误
#[derive(Debug, thiserror::Error)]
pub enum ConfigStorageError {
    #[error("Storage error: {0}")]
    StorageError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Config not found: {0}")]
    ConfigNotFound(String),
    
    #[error("Backup not found: {0}")]
    BackupNotFound(String),
    
    #[error("Invalid path: {0}")]
    InvalidPath(String),
}

/// 配置验证错误
#[derive(Debug, thiserror::Error)]
pub enum ConfigValidationError {
    #[error("Validation error: {0}")]
    ValidationError(String),
    
    #[error("Schema error: {0}")]
    SchemaError(String),
    
    #[error("Invalid value: {0}")]
    InvalidValue(String),
    
    #[error("Missing required field: {0}")]
    MissingRequiredField(String),
}

impl PluginConfigManager {
    /// 创建新的配置管理器
    pub fn new(
        storage: Arc<dyn ConfigStorage>,
        validator: Arc<dyn ConfigValidator>,
        config: ConfigManagerConfig,
    ) -> Self {
        Self {
            storage,
            validator,
            change_listeners: Arc::new(RwLock::new(Vec::new())),
            config_cache: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// 获取插件配置
    pub async fn get_plugin_config(&self, plugin_id: &PluginId) -> Result<PluginConfig, ConfigValidationError> {
        // 先检查缓存
        if self.config.enable_cache {
            let cache = self.config_cache.read().unwrap();
            if let Some(config) = cache.get(plugin_id) {
                return Ok(config.clone());
            }
        }

        // 从存储加载
        let config = self.storage.load_config(plugin_id).await
            .map_err(|e| ConfigValidationError::ValidationError(e.to_string()))?
            .ok_or_else(|| {
                // 如果没有配置，返回默认配置
                self.validator.get_default_config(plugin_id)?
            })?;

        // 验证配置
        let validation_result = self.validator.validate_config(plugin_id, &config).await?;
        if !validation_result.is_valid {
            return Err(ConfigValidationError::ValidationError(format!(
                "Config validation failed: {}",
                validation_result.errors.iter()
                    .map(|e| &e.message)
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }

        // 更新缓存
        if self.config.enable_cache {
            let mut cache = self.config_cache.write().unwrap();
            cache.insert(plugin_id.clone(), config.clone());
        }

        Ok(config)
    }

    /// 设置插件配置
    pub async fn set_plugin_config(&self, plugin_id: &PluginId, new_config: PluginConfig) -> Result<(), ConfigValidationError> {
        // 获取旧配置
        let old_config = self.get_plugin_config(plugin_id).await.ok();

        // 验证新配置
        let validation_result = self.validator.validate_config_update(plugin_id, &old_config, &new_config).await?;
        if !validation_result.is_valid {
            return Err(ConfigValidationError::ValidationError(format!(
                "Config validation failed: {}",
                validation_result.errors.iter()
                    .map(|e| &e.message)
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }

        // 保存配置
        self.storage.save_config(plugin_id, &new_config).await
            .map_err(|e| ConfigValidationError::ValidationError(e.to_string()))?;

        // 更新缓存
        if self.config.enable_cache {
            let mut cache = self.config_cache.write().unwrap();
            cache.insert(plugin_id.clone(), new_config.clone());
        }

        // 通知变更监听器
        self.notify_config_change(ConfigChangeEvent {
            id: uuid::Uuid::new_v4().to_string(),
            plugin_id: plugin_id.clone(),
            event_type: if old_config.is_some() {
                ConfigChangeType::Updated
            } else {
                ConfigChangeType::Created
            },
            old_config,
            new_config,
            timestamp: std::time::SystemTime::now(),
            source: "config_manager".to_string(),
            description: "Configuration updated".to_string(),
            details: HashMap::new(),
        });

        Ok(())
    }

    /// 重置插件配置
    pub async fn reset_plugin_config(&self, plugin_id: &PluginId) -> Result<(), ConfigValidationError> {
        let default_config = self.validator.get_default_config(plugin_id)?;
        self.set_plugin_config(plugin_id, default_config).await
    }

    /// 备份插件配置
    pub async fn backup_plugin_config(&self, plugin_id: &PluginId) -> Result<String, ConfigValidationError> {
        self.storage.backup_config(plugin_id).await
            .map_err(|e| ConfigValidationError::ValidationError(e.to_string()))
    }

    /// 恢复插件配置
    pub async fn restore_plugin_config(&self, plugin_id: &PluginId, backup_id: &str) -> Result<(), ConfigValidationError> {
        // 获取备份配置
        let backup_config = self.storage.load_config(plugin_id).await
            .map_err(|e| ConfigValidationError::ValidationError(e.to_string()))?
            .ok_or_else(|| {
                Err(ConfigValidationError::ConfigNotFound(format!("Backup {} not found", backup_id)))
            })?;

        // 设置为当前配置
        self.set_plugin_config(plugin_id, backup_config).await?;

        // 通知恢复事件
        self.notify_config_change(ConfigChangeEvent {
            id: uuid::Uuid::new_v4().to_string(),
            plugin_id: plugin_id.clone(),
            event_type: ConfigChangeType::Restored,
            old_config: None,
            new_config: backup_config,
            timestamp: std::time::SystemTime::now(),
            source: "config_manager".to_string(),
            description: format!("Configuration restored from backup {}", backup_id),
            details: HashMap::from([("backup_id".to_string(), backup_id.to_string())]),
        });

        Ok(())
    }

    /// 删除插件配置
    pub async fn delete_plugin_config(&self, plugin_id: &PluginId) -> Result<(), ConfigValidationError> {
        // 获取当前配置
        let current_config = self.get_plugin_config(plugin_id).await.ok();

        // 从存储删除
        self.storage.delete_config(plugin_id).await
            .map_err(|e| ConfigValidationError::ValidationError(e.to_string()))?;

        // 从缓存删除
        if self.config.enable_cache {
            let mut cache = self.config_cache.write().unwrap();
            cache.remove(plugin_id);
        }

        // 通知删除事件
        if let Some(config) = current_config {
            self.notify_config_change(ConfigChangeEvent {
                id: uuid::Uuid::new_v4().to_string(),
                plugin_id: plugin_id.clone(),
                event_type: ConfigChangeType::Deleted,
                old_config: Some(config),
                new_config: PluginConfig {
                    plugin_id: plugin_id.clone(),
                    version: 0,
                    data: HashMap::new(),
                    metadata: ConfigMetadata::default(),
                },
                timestamp: std::time::SystemTime::now(),
                source: "config_manager".to_string(),
                description: "Configuration deleted".to_string(),
                details: HashMap::new(),
            });
        }

        Ok(())
    }

    /// 列出所有配置
    pub async fn list_all_configs(&self) -> Result<Vec<(PluginId, PluginConfig)>, ConfigValidationError> {
        self.storage.list_configs().await
            .map_err(|e| ConfigValidationError::ValidationError(e.to_string()))
    }

    /// 获取配置历史
    pub async fn get_config_history(&self, plugin_id: &PluginId) -> Result<Vec<ConfigVersion>, ConfigValidationError> {
        self.storage.get_config_history(plugin_id).await
            .map_err(|e| ConfigValidationError::ValidationError(e.to_string()))
    }

    /// 添加配置变更监听器
    pub fn add_config_change_listener(&self, listener: Box<dyn ConfigChangeListener>) {
        self.change_listeners.write().unwrap().push(listener);
    }

    /// 移除配置变更监听器
    pub fn remove_config_change_listener(&self, index: usize) {
        let mut listeners = self.change_listeners.write().unwrap();
        if index < listeners.len() {
            listeners.remove(index);
        }
    }

    /// 清除配置缓存
    pub fn clear_config_cache(&self, plugin_id: &PluginId) {
        if self.config.enable_cache {
            let mut cache = self.config_cache.write().unwrap();
            cache.remove(plugin_id);
        }
    }

    /// 清除所有配置缓存
    pub fn clear_all_config_cache(&self) {
        if self.config.enable_cache {
            let mut cache = self.config_cache.write().unwrap();
            cache.clear();
        }
    }

    /// 清理过期备份
    pub async fn cleanup_expired_backups(&self) -> Result<(), ConfigValidationError> {
        self.storage.cleanup_backups().await
            .map_err(|e| ConfigValidationError::ValidationError(e.to_string()))
    }

    /// 通知配置变更
    fn notify_config_change(&self, event: ConfigChangeEvent) {
        let listeners = self.change_listeners.read().unwrap();
        for listener in listeners.iter() {
            listener.on_config_changed(&event);
        }
    }

    /// 获取配置模式
    pub fn get_config_schema(&self, plugin_id: &PluginId) -> Result<ConfigSchema, ConfigValidationError> {
        self.validator.get_config_schema(plugin_id)
    }

    /// 验证配置
    pub async fn validate_config(&self, plugin_id: &PluginId, config: &PluginConfig) -> Result<ValidationResult, ConfigValidationError> {
        self.validator.validate_config(plugin_id, config).await
    }
}

/// 文件配置存储
pub struct FileConfigStorage {
    /// 配置目录
    config_dir: PathBuf,
    /// 配置格式
    config_format: ConfigFormat,
    /// 管理器配置
    manager_config: ConfigManagerConfig,
}

impl FileConfigStorage {
    /// 创建新的文件配置存储
    pub fn new(config_dir: PathBuf, config_format: ConfigFormat, manager_config: ConfigManagerConfig) -> Self {
        Self {
            config_dir,
            config_format,
            manager_config,
        }
    }

    /// 获取配置文件路径
    fn get_config_file_path(&self, plugin_id: &PluginId) -> PathBuf {
        self.config_dir.join(format!("{}.{}", plugin_id.as_str(), self.config_format.extension()))
    }

    /// 获取备份目录路径
    fn get_backup_dir(&self) -> PathBuf {
        self.config_dir.join("backups")
    }

    /// 获取备份文件路径
    fn get_backup_file_path(&self, plugin_id: &PluginId, backup_id: &str) -> PathBuf {
        self.get_backup_dir().join(format!("{}.{}.backup", plugin_id.as_str(), backup_id))
    }
}

#[async_trait::async_trait]
impl ConfigStorage for FileConfigStorage {
    async fn save_config(&self, plugin_id: &PluginId, config: &PluginConfig) -> Result<(), ConfigStorageError> {
        let config_file = self.get_config_file_path(plugin_id);
        
        // 确保配置目录存在
        if let Some(parent) = config_file.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // 序列化配置
        let config_data = match self.config_format {
            ConfigFormat::Json => serde_json::to_string_pretty(config)?,
            ConfigFormat::Toml => toml::to_string_pretty(config)?,
            ConfigFormat::Yaml => serde_yaml::to_string(config)?,
            ConfigFormat::Xml => {
                return Err(ConfigStorageError::ValidationError("XML format not supported".to_string()));
            }
        };

        // 写入文件
        tokio::fs::write(config_file, config_data).await?;

        // 创建备份
        if self.manager_config.enable_backup {
            let backup_id = format!("{}-{}", plugin_id.as_str(), chrono::Utc::now().timestamp());
            let backup_file = self.get_backup_file_path(plugin_id, &backup_id);
            
            // 确保备份目录存在
            if let Some(parent) = backup_file.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            
            tokio::fs::write(backup_file, config_data).await?;
        }

        Ok(())
    }

    async fn load_config(&self, plugin_id: &PluginId) -> Result<Option<PluginConfig>, ConfigStorageError> {
        let config_file = self.get_config_file_path(plugin_id);
        
        // 检查文件是否存在
        if !tokio::fs::metadata(&config_file).await.is_ok() {
            return Ok(None);
        }

        // 读取文件
        let config_data = tokio::fs::read_to_string(config_file).await?;
        
        // 反序列化配置
        let config = match self.config_format {
            ConfigFormat::Json => serde_json::from_str(&config_data)?,
            ConfigFormat::Toml => toml::from_str(&config_data)?,
            ConfigFormat::Yaml => serde_yaml::from_str(&config_data)?,
            ConfigFormat::Xml => {
                return Err(ConfigStorageError::ValidationError("XML format not supported".to_string()));
            }
        };

        Ok(Some(config))
    }

    async fn delete_config(&self, plugin_id: &PluginId) -> Result<(), ConfigStorageError> {
        let config_file = self.get_config_file_path(plugin_id);
        
        // 检查文件是否存在
        if tokio::fs::metadata(&config_file).await.is_ok() {
            tokio::fs::remove_file(config_file).await?;
        }

        Ok(())
    }

    async fn list_configs(&self) -> Result<Vec<(PluginId, PluginConfig)>, ConfigStorageError> {
        let mut configs = Vec::new();
        
        // 读取配置目录
        let mut entries = tokio::fs::read_dir(&self.config_dir).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            
            // 只处理配置文件
            if self.is_config_file(&path) {
                // 从文件名提取插件ID
                let plugin_id = self.extract_plugin_id_from_path(&path)?;
                
                // 加载配置
                if let Some(config) = self.load_config(&plugin_id).await? {
                    configs.push((plugin_id, config));
                }
            }
        }

        Ok(configs)
    }

    async fn backup_config(&self, plugin_id: &PluginId) -> Result<String, ConfigStorageError> {
        let backup_id = format!("{}-{}", plugin_id.as_str(), chrono::Utc::now().timestamp());
        let backup_file = self.get_backup_file_path(plugin_id, &backup_id);
        let config_file = self.get_config_file_path(plugin_id);
        
        // 确保备份目录存在
        if let Some(parent) = backup_file.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // 复制配置文件到备份目录
        tokio::fs::copy(config_file, backup_file).await?;
        
        Ok(backup_id)
    }

    async fn restore_config(&self, plugin_id: &PluginId, backup_id: &str) -> Result<(), ConfigStorageError> {
        let backup_file = self.get_backup_file_path(plugin_id, backup_id);
        let config_file = self.get_config_file_path(plugin_id);
        
        // 检查备份文件是否存在
        if !tokio::fs::metadata(&backup_file).await.is_ok() {
            return Err(ConfigStorageError::BackupNotFound(backup_id.to_string()));
        }

        // 复制备份文件到配置文件
        tokio::fs::copy(backup_file, config_file).await?;
        
        Ok(())
    }

    async fn get_config_history(&self, plugin_id: &PluginId) -> Result<Vec<ConfigVersion>, ConfigStorageError> {
        let backup_dir = self.get_backup_dir();
        let mut history = Vec::new();
        
        // 读取备份目录
        if !tokio::fs::metadata(&backup_dir).await.is_ok() {
            return Ok(history);
        }

        let mut entries = tokio::fs::read_dir(&backup_dir).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            
            // 只处理备份文件
            if self.is_backup_file(plugin_id, &path) {
                // 从文件名提取备份ID
                let backup_id = self.extract_backup_id_from_path(plugin_id, &path)?;
                
                // 读取备份配置
                let config_data = tokio::fs::read_to_string(&path).await?;
                let config: PluginConfig = match self.config_format {
                    ConfigFormat::Json => serde_json::from_str(&config_data)?,
                    ConfigFormat::Toml => toml::from_str(&config_data)?,
                    ConfigFormat::Yaml => serde_yaml::from_str(&config_data)?,
                    ConfigFormat::Xml => {
                        return Err(ConfigStorageError::ValidationError("XML format not supported".to_string()));
                    }
                };
                
                history.push(ConfigVersion {
                    version: 0, // 简化实现
                    created_at: entry.metadata().modified().ok_or_else(|_| std::time::SystemTime::now()),
                    created_by: "system".to_string(),
                    description: Some(format!("Backup {}", backup_id)),
                    config,
                });
            }
        }

        // 按时间排序（最新的在前）
        history.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        
        Ok(history)
    }

    async fn cleanup_backups(&self) -> Result<(), ConfigStorageError> {
        let backup_dir = self.get_backup_dir();
        
        if !tokio::fs::metadata(&backup_dir).await.is_ok() {
            return Ok(());
        }

        let mut entries = tokio::fs::read_dir(&backup_dir).await?;
        let mut backup_files = Vec::new();
        
        // 收集所有备份文件
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("backup") {
                backup_files.push((entry.metadata().modified().ok_or_else(|_| std::time::SystemTime::now()), path));
            }
        }

        // 按修改时间排序（最新的在前）
        backup_files.sort_by(|a, b| b.0.cmp(&a.0));
        
        // 保留指定数量的备份
        let retention = self.manager_config.backup_retention;
        if backup_files.len() > retention {
            for (_, path) in backup_files.iter().skip(retention) {
                let _ = tokio::fs::remove_file(path).await;
            }
        }

        Ok(())
    }

    /// 检查是否是配置文件
    fn is_config_file(&self, path: &Path) -> bool {
        if let Some(extension) = path.extension() {
            if let Some(ext_str) = extension.to_str() {
                return ext_str == self.config_format.extension();
            }
        }
        false
    }

    /// 检查是否是备份文件
    fn is_backup_file(&self, plugin_id: &PluginId, path: &Path) -> bool {
        if let Some(file_name) = path.file_stem() {
            if let Some(name_str) = file_name.to_str() {
                return name_str.starts_with(&format!("{}-", plugin_id.as_str()));
            }
        }
        false
    }

    /// 从路径提取插件ID
    fn extract_plugin_id_from_path(&self, path: &Path) -> Result<PluginId, ConfigStorageError> {
        if let Some(file_name) = path.file_stem() {
            if let Some(name_str) = file_name.to_str() {
                return Ok(PluginId::from(name_str));
            }
        }
        Err(ConfigStorageError::InvalidPath(format!("Invalid config file path: {:?}", path)))
    }

    /// 从路径提取备份ID
    fn extract_backup_id_from_path(&self, plugin_id: &PluginId, path: &Path) -> Result<String, ConfigStorageError> {
        if let Some(file_name) = path.file_stem() {
            if let Some(name_str) = file_name.to_str() {
                if let Some(backup_id) = name_str.strip_prefix(&format!("{}-", plugin_id.as_str())) {
                    return Ok(backup_id.to_string());
                }
            }
        }
        Err(ConfigStorageError::InvalidPath(format!("Invalid backup file path: {:?}", path)))
    }
}

/// 默认配置验证器
pub struct DefaultConfigValidator {
    /// 配置模式缓存
    schema_cache: Arc<RwLock<HashMap<PluginId, ConfigSchema>>>,
}

impl DefaultConfigValidator {
    /// 创建新的默认配置验证器
    pub fn new() -> Self {
        Self {
            schema_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl ConfigValidator for DefaultConfigValidator {
    async fn validate_config(&self, plugin_id: &PluginId, config: &PluginConfig) -> Result<ValidationResult, ConfigValidationError> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // 基本验证
        if config.data.is_empty() {
            warnings.push(ValidationWarning {
                code: "empty_config".to_string(),
                message: "Configuration is empty".to_string(),
                path: "/".to_string(),
                value: ConfigValue::Null,
                details: HashMap::new(),
            });
        }

        // 获取配置模式（如果存在）
        if let Ok(schema) = self.get_config_schema(plugin_id) {
            // 根据模式验证配置
            self.validate_against_schema(&schema, config, &mut errors, &mut warnings);
        } else {
            // 基本类型验证
            self.validate_basic_types(config, &mut errors, &mut warnings);
        }

        Ok(ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
            validated_at: std::time::SystemTime::now(),
        })
    }

    fn get_config_schema(&self, plugin_id: &PluginId) -> Result<ConfigSchema, ConfigValidationError> {
        let cache = self.schema_cache.read().unwrap();
        
        // 简化实现，返回默认模式
        if let Some(schema) = cache.get(plugin_id) {
            Ok(schema.clone())
        } else {
            let default_schema = self.create_default_schema(plugin_id);
            Ok(default_schema)
        }
    }

    fn get_default_config(&self, plugin_id: &PluginId) -> Result<PluginConfig, ConfigValidationError> {
        Ok(PluginConfig {
            plugin_id: plugin_id.clone(),
            version: 1,
            data: HashMap::new(),
            metadata: ConfigMetadata::default(),
        })
    }

    async fn validate_config_update(&self, plugin_id: &PluginId, old_config: &PluginConfig, new_config: &PluginConfig) -> Result<ValidationResult, ConfigValidationError> {
        // 验证新配置
        self.validate_config(plugin_id, new_config).await
    }
}

impl DefaultConfigValidator {
    /// 创建默认配置模式
    fn create_default_schema(&self, plugin_id: &PluginId) -> ConfigSchema {
        ConfigSchema {
            version: "1.0".to_string(),
            definition: SchemaDefinition {
                properties: HashMap::new(),
                required: Vec::new(),
                schema_type: SchemaType::Object,
                additional_properties: Some(true),
            },
            metadata: SchemaMetadata {
                title: Some(format!("{} Configuration", plugin_id.as_str())),
                description: Some(format!("Default configuration schema for {}", plugin_id.as_str())),
                version: "1.0".to_string(),
                author: Some("VM Plugin System".to_string()),
            },
        }
    }

    /// 根据模式验证配置
    fn validate_against_schema(&self, schema: &ConfigSchema, config: &PluginConfig, errors: &mut Vec<ValidationError>, warnings: &mut Vec<ValidationWarning>) {
        for (path, value) in &config.data {
            if let Some(property) = schema.definition.properties.get(path) {
                self.validate_property(path, property, value, errors, warnings);
            } else {
                // 未知属性，添加警告
                warnings.push(ValidationWarning {
                    code: "unknown_property".to_string(),
                    message: format!("Unknown property: {}", path),
                    path: path.clone(),
                    value: value.clone(),
                    details: HashMap::new(),
                });
            }
        }

        // 检查必需属性
        for required_path in &schema.definition.required {
            if !config.data.contains_key(required_path) {
                errors.push(ValidationError {
                    code: "missing_required".to_string(),
                    message: format!("Missing required property: {}", required_path),
                    path: required_path.clone(),
                    value: ConfigValue::Null,
                    details: HashMap::new(),
                });
            }
        }
    }

    /// 验证属性
    fn validate_property(&self, path: &str, property: &PropertyDefinition, value: &ConfigValue, errors: &mut Vec<ValidationError>, warnings: &mut Vec<ValidationWarning>) {
        // 验证类型
        if !self.is_type_compatible(&property.property_type, value) {
            errors.push(ValidationError {
                code: "type_mismatch".to_string(),
                message: format!("Type mismatch for property: {}", path),
                path: path.to_string(),
                value: value.clone(),
                details: HashMap::new(),
            });
            return;
        }

        // 验证规则
        for rule in &property.validation_rules {
            self.validate_validation_rule(path, rule, value, errors, warnings);
        }
    }

    /// 检查类型兼容性
    fn is_type_compatible(&self, property_type: &PropertyType, value: &ConfigValue) -> bool {
        match (property_type, value) {
            (PropertyType::String, ConfigValue::String(_)) => true,
            (PropertyType::Integer, ConfigValue::Integer(_)) => true,
            (PropertyType::Number, ConfigValue::Integer(_)) => true,
            (PropertyType::Number, ConfigValue::Float(_)) => true,
            (PropertyType::Boolean, ConfigValue::Bool(_)) => true,
            (PropertyType::Array, ConfigValue::Array(_)) => true,
            (PropertyType::Object, ConfigValue::Object(_)) => true,
            (PropertyType::Null, ConfigValue::Null) => true,
            _ => false,
        }
    }

    /// 验证验证规则
    fn validate_validation_rule(&self, path: &str, rule: &ValidationRule, value: &ConfigValue, errors: &mut Vec<ValidationError>, warnings: &mut Vec<ValidationWarning>) {
        match rule {
            ValidationRule::Required => {
                if matches!(value, ConfigValue::Null) {
                    errors.push(ValidationError {
                        code: "required".to_string(),
                        message: format!("Property {} is required", path),
                        path: path.to_string(),
                        value: value.clone(),
                        details: HashMap::new(),
                    });
                }
            }
            ValidationRule::MinValue(min_val) => {
                if let Some(num_val) = self.get_numeric_value(value) {
                    if num_val < *min_val {
                        errors.push(ValidationError {
                            code: "min_value".to_string(),
                            message: format!("Property {} value {} is less than minimum {}", path, num_val, min_val),
                            path: path.to_string(),
                            value: value.clone(),
                            details: HashMap::new(),
                        });
                    }
                }
            }
            ValidationRule::MaxValue(max_val) => {
                if let Some(num_val) = self.get_numeric_value(value) {
                    if num_val > *max_val {
                        errors.push(ValidationError {
                            code: "max_value".to_string(),
                            message: format!("Property {} value {} is greater than maximum {}", path, num_val, max_val),
                            path: path.to_string(),
                            value: value.clone(),
                            details: HashMap::new(),
                        });
                    }
                }
            }
            ValidationRule::MinLength(min_len) => {
                if let Some(str_val) = self.get_string_value(value) {
                    if str_val.len() < *min_len {
                        errors.push(ValidationError {
                            code: "min_length".to_string(),
                            message: format!("Property {} length {} is less than minimum {}", path, str_val.len(), min_len),
                            path: path.to_string(),
                            value: value.clone(),
                            details: HashMap::new(),
                        });
                    }
                }
            }
            ValidationRule::MaxLength(max_len) => {
                if let Some(str_val) = self.get_string_value(value) {
                    if str_val.len() > *max_len {
                        errors.push(ValidationError {
                            code: "max_length".to_string(),
                            message: format!("Property {} length {} is greater than maximum {}", path, str_val.len(), max_len),
                            path: path.to_string(),
                            value: value.clone(),
                            details: HashMap::new(),
                        });
                    }
                }
            }
            ValidationRule::Regex(pattern) => {
                if let Some(str_val) = self.get_string_value(value) {
                    let regex = regex::Regex::new(pattern).map_err(|_| {
                        ConfigValidationError::ValidationError(format!("Invalid regex pattern: {}", pattern))
                    })?;
                    if !regex.is_match(str_val) {
                        errors.push(ValidationError {
                            code: "regex_mismatch".to_string(),
                            message: format!("Property {} value {} does not match pattern {}", path, str_val, pattern),
                            path: path.to_string(),
                            value: value.clone(),
                            details: HashMap::new(),
                        });
                    }
                }
            }
            ValidationRule::Enum(allowed_values) => {
                if !allowed_values.contains(value) {
                    errors.push(ValidationError {
                        code: "enum_mismatch".to_string(),
                        message: format!("Property {} value {:?} is not in allowed values {:?}", path, value, allowed_values),
                        path: path.to_string(),
                        value: value.clone(),
                        details: HashMap::new(),
                    });
                }
            }
            ValidationRule::Custom(_) => {
                // 自定义验证需要具体实现
                warnings.push(ValidationWarning {
                    code: "custom_validation".to_string(),
                    message: format!("Custom validation for property {}", path),
                    path: path.to_string(),
                    value: value.clone(),
                    details: HashMap::new(),
                });
            }
            ValidationRule::Format(format_str) => {
                if let Some(str_val) = self.get_string_value(value) {
                    // 简化的格式验证
                    if format_str == "email" && !str_val.contains('@') {
                        errors.push(ValidationError {
                            code: "format_mismatch".to_string(),
                            message: format!("Property {} value {} is not a valid email", path, str_val),
                            path: path.to_string(),
                            value: value.clone(),
                            details: HashMap::new(),
                        });
                    }
                }
            }
        }
    }

    /// 获取数值
    fn get_numeric_value(&self, value: &ConfigValue) -> Option<f64> {
        match value {
            ConfigValue::Integer(i) => Some(*i as f64),
            ConfigValue::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// 获取字符串值
    fn get_string_value(&self, value: &ConfigValue) -> Option<&str> {
        match value {
            ConfigValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// 基本类型验证
    fn validate_basic_types(&self, config: &PluginConfig, errors: &mut Vec<ValidationError>, warnings: &mut Vec<ValidationWarning>) {
        // 基本类型验证
        for (path, value) in &config.data {
            match value {
                ConfigValue::String(s) => {
                    if s.len() > 10000 {
                        warnings.push(ValidationWarning {
                            code: "long_string".to_string(),
                            message: format!("Property {} string value is very long ({} characters)", path, s.len()),
                            path: path.clone(),
                            value: value.clone(),
                            details: HashMap::new(),
                        });
                    }
                }
                ConfigValue::Array(arr) => {
                    if arr.len() > 1000 {
                        warnings.push(ValidationWarning {
                            code: "large_array".to_string(),
                            message: format!("Property {} array has {} elements", path, arr.len()),
                            path: path.clone(),
                            value: value.clone(),
                            details: HashMap::new(),
                        });
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_creation() {
        let config = PluginConfig {
            plugin_id: PluginId::from("test"),
            version: 1,
            data: HashMap::new(),
            metadata: ConfigMetadata::default(),
        };
        
        assert_eq!(config.plugin_id.as_str(), "test");
        assert_eq!(config.version, 1);
    }

    #[test]
    fn test_config_value_conversions() {
        let bool_val = ConfigValue::Bool(true);
        assert_eq!(bool_val.as_bool(), Some(true));
        
        let int_val = ConfigValue::Integer(42);
        assert_eq!(int_val.as_integer(), Some(42));
        
        let float_val = ConfigValue::Float(3.14);
        assert_eq!(float_val.as_float(), Some(3.14));
        
        let string_val = ConfigValue::String("test".to_string());
        assert_eq!(string_val.as_string(), Some(&"test".to_string()));
    }

    #[tokio::test]
    async fn test_file_config_storage() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_dir = temp_dir.path();
        let manager_config = ConfigManagerConfig::default();
        
        let storage = FileConfigStorage::new(config_dir, ConfigFormat::Json, manager_config);
        let plugin_id = PluginId::from("test");
        
        let config = PluginConfig {
            plugin_id: plugin_id.clone(),
            version: 1,
            data: HashMap::from([("key".to_string(), ConfigValue::String("value".to_string()))]),
            metadata: ConfigMetadata::default(),
        };
        
        // 保存配置
        storage.save_config(&plugin_id, &config).await.unwrap();
        
        // 加载配置
        let loaded_config = storage.load_config(&plugin_id).await.unwrap();
        assert!(loaded_config.is_some());
        
        let loaded_config = loaded_config.unwrap();
        assert_eq!(loaded_config.plugin_id, plugin_id);
        assert_eq!(loaded_config.data.get("key"), Some(&ConfigValue::String("value".to_string())));
    }

    #[tokio::test]
    async fn test_config_validation() {
        let validator = DefaultConfigValidator::new();
        let plugin_id = PluginId::from("test");
        
        let config = PluginConfig {
            plugin_id: plugin_id.clone(),
            version: 1,
            data: HashMap::from([
                ("string_field".to_string(), ConfigValue::String("test".to_string())),
                ("int_field".to_string(), ConfigValue::Integer(42)),
                ("bool_field".to_string(), ConfigValue::Bool(true)),
            ]),
            metadata: ConfigMetadata::default(),
        };
        
        let result = validator.validate_config(&plugin_id, &config).await.unwrap();
        assert!(result.is_valid);
        assert!(result.errors.is_empty());
    }
}