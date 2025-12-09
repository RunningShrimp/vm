//! 统一配置管理系统
//!
//! 提供集中化的配置管理，支持运行时配置更新和验证。

use crate::VmError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// 配置源类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigSource {
    /// 默认配置
    Default,
    /// 文件配置
    File(String),
    /// 环境变量
    Environment,
    /// 运行时配置
    Runtime,
}

/// 配置项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigItem {
    /// 配置键
    pub key: String,
    /// 配置值
    pub value: serde_json::Value,
    /// 配置源
    pub source: ConfigSource,
    /// 是否可运行时修改
    pub runtime_modifiable: bool,
    /// 描述
    pub description: String,
}

/// 配置验证器trait
pub trait ConfigValidator {
    /// 验证配置项
    fn validate(&self, key: &str, value: &serde_json::Value) -> Result<(), VmError>;

    /// 获取支持的配置键
    fn supported_keys(&self) -> Vec<String>;
}

/// 统一配置管理器
pub struct ConfigManager {
    /// 配置存储
    configs: Arc<RwLock<HashMap<String, ConfigItem>>>,
    /// 验证器
    validators: HashMap<String, Box<dyn ConfigValidator + Send + Sync>>,
    /// 配置监听器
    listeners: Vec<Box<dyn Fn(&str, &ConfigItem) + Send + Sync>>,
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigManager {
    pub fn new() -> Self {
        Self {
            configs: Arc::new(RwLock::new(HashMap::new())),
            validators: HashMap::new(),
            listeners: Vec::new(),
        }
    }

    /// 设置配置项
    pub fn set(
        &self,
        key: String,
        value: serde_json::Value,
        source: ConfigSource,
        runtime_modifiable: bool,
        description: String,
    ) -> Result<(), VmError> {
        // 验证配置
        if let Some(validator) = self.validators.get(&key) {
            validator.validate(&key, &value)?;
        }

        let item = ConfigItem {
            key: key.clone(),
            value,
            source,
            runtime_modifiable,
            description,
        };

        {
            let mut configs = self.configs.write().map_err(|_| {
                VmError::Core(vm_core::CoreError::Concurrency {
                    message: "Failed to acquire config lock".to_string(),
                    operation: "set_config".to_string(),
                })
            })?;
            configs.insert(key.clone(), item.clone());
        }

        // 通知监听器
        for listener in &self.listeners {
            listener(&key, &item);
        }

        Ok(())
    }

    /// 获取配置项
    pub fn get(&self, key: &str) -> Result<Option<ConfigItem>, VmError> {
        let configs = self.configs.read().map_err(|_| {
            VmError::Core(vm_core::CoreError::Concurrency {
                message: "Failed to acquire config lock".to_string(),
                operation: "get_config".to_string(),
            })
        })?;
        Ok(configs.get(key).cloned())
    }

    /// 获取配置值
    pub fn get_value(&self, key: &str) -> Result<Option<serde_json::Value>, VmError> {
        Ok(self.get(key)?.map(|item| item.value))
    }

    /// 更新运行时配置
    pub fn update_runtime(&self, key: String, value: serde_json::Value) -> Result<(), VmError> {
        let configs = self.configs.read().map_err(|_| {
            VmError::Core(vm_core::CoreError::Concurrency {
                message: "Failed to acquire config lock".to_string(),
                operation: "update_runtime".to_string(),
            })
        })?;

        if let Some(existing) = configs.get(&key)
            && !existing.runtime_modifiable {
                return Err(VmError::Core(vm_core::CoreError::Config {
                    message: format!("Configuration '{}' is not runtime modifiable", key),
                    path: Some(key),
                }));
            }

        drop(configs);
        self.set(
            key,
            value,
            ConfigSource::Runtime,
            true,
            "Runtime updated".to_string(),
        )
    }

    /// 注册验证器
    pub fn register_validator(
        &mut self,
        component: &str,
        validator: Box<dyn ConfigValidator + Send + Sync>,
    ) {
        self.validators.insert(component.to_string(), validator);
    }

    /// 添加配置监听器
    pub fn add_listener<F>(&mut self, listener: F)
    where
        F: Fn(&str, &ConfigItem) + Send + Sync + 'static,
    {
        self.listeners.push(Box::new(listener));
    }

    /// 加载配置文件
    pub fn load_from_file(&self, path: &str) -> Result<(), VmError> {
        let content = std::fs::read_to_string(path).map_err(|e| VmError::Io(e.to_string()))?;
        let configs: HashMap<String, serde_json::Value> =
            serde_json::from_str(&content).map_err(|e| {
                VmError::Core(vm_core::CoreError::Config {
                    message: format!("Failed to parse config file: {}", e),
                    path: Some(path.to_string()),
                })
            })?;

        for (key, value) in configs {
            self.set(
                key,
                value,
                ConfigSource::File(path.to_string()),
                false,
                format!("Loaded from {}", path),
            )?;
        }

        Ok(())
    }

    /// 从环境变量加载配置
    pub fn load_from_env(&self, prefix: &str) -> Result<(), VmError> {
        for (key, value) in std::env::vars() {
            if key.starts_with(prefix) {
                let config_key = key.strip_prefix(prefix).unwrap_or(&key).to_lowercase();
                let json_value = serde_json::Value::String(value);
                self.set(
                    config_key,
                    json_value,
                    ConfigSource::Environment,
                    true,
                    format!("Environment variable {}", key),
                )?;
            }
        }
        Ok(())
    }

    /// 获取所有配置
    pub fn all_configs(&self) -> Result<HashMap<String, ConfigItem>, VmError> {
        let configs = self.configs.read().map_err(|_| {
            VmError::Core(vm_core::CoreError::Concurrency {
                message: "Failed to acquire config lock".to_string(),
                operation: "all_configs".to_string(),
            })
        })?;
        Ok(configs.clone())
    }

    /// 导出配置到文件
    pub fn export_to_file(&self, path: &str) -> Result<(), VmError> {
        let configs = self.all_configs()?;
        let values: HashMap<String, serde_json::Value> =
            configs.into_iter().map(|(k, v)| (k, v.value)).collect();

        let content = serde_json::to_string_pretty(&values).map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to serialize config: {}", e),
                module: "config_manager".to_string(),
            })
        })?;

        std::fs::write(path, content).map_err(|e| VmError::Io(e.to_string()))?;
        Ok(())
    }
}

/// 组件配置trait
pub trait ComponentConfig: Serialize + for<'de> Deserialize<'de> + Clone {
    /// 获取组件名称
    fn component_name() -> &'static str;

    /// 验证配置
    fn validate(&self) -> Result<(), VmError>;

    /// 获取默认配置
    fn default() -> Self;

    /// 从配置管理器加载
    fn from_config_manager(manager: &ConfigManager) -> Result<Self, VmError> {
        let component_name = Self::component_name();
        let config_value = manager.get_value(component_name)?;

        if let Some(value) = config_value {
            let config: Self = serde_json::from_value(value).map_err(|e| {
                VmError::Core(vm_core::CoreError::Config {
                    message: format!("Failed to parse {} config: {}", component_name, e),
                    path: Some(component_name.to_string()),
                })
            })?;
            config.validate()?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    /// 保存到配置管理器
    fn save_to_config_manager(&self, manager: &ConfigManager) -> Result<(), VmError> {
        let component_name = Self::component_name();
        let value = serde_json::to_value(self).map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to serialize {} config: {}", component_name, e),
                module: "component_config".to_string(),
            })
        })?;

        manager.set(
            component_name.to_string(),
            value,
            ConfigSource::Runtime,
            true,
            format!("{} component configuration", component_name),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct TestConfig {
        pub value: i32,
    }

    impl ComponentConfig for TestConfig {
        fn component_name() -> &'static str {
            "test"
        }

        fn validate(&self) -> Result<(), VmError> {
            if self.value < 0 {
                return Err(VmError::Core(vm_core::CoreError::Config {
                    message: "Value must be non-negative".to_string(),
                    path: Some("value".to_string()),
                }));
            }
            Ok(())
        }

        fn default() -> Self {
            Self { value: 42 }
        }
    }

    #[test]
    fn test_config_manager() {
        let manager = ConfigManager::new();

        // 设置配置
        manager
            .set(
                "test_key".to_string(),
                serde_json::Value::String("test_value".to_string()),
                ConfigSource::Default,
                true,
                "Test configuration".to_string(),
            )
            .unwrap();

        // 获取配置
        let item = manager.get("test_key").unwrap().unwrap();
        assert_eq!(item.key, "test_key");
        assert_eq!(item.value.as_str().unwrap(), "test_value");
    }

    #[test]
    fn test_component_config() {
        let manager = ConfigManager::new();
        let config = TestConfig { value: 100 };

        // 保存配置
        config.save_to_config_manager(&manager).unwrap();

        // 加载配置
        let loaded = TestConfig::from_config_manager(&manager).unwrap();
        assert_eq!(loaded.value, 100);
    }

    #[test]
    fn test_config_validation() {
        let manager = ConfigManager::new();
        let invalid_config = TestConfig { value: -1 };

        // 验证应该失败
        assert!(invalid_config.validate().is_err());
    }
}
