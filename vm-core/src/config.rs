//! 统一配置管理
//!
//! 提供虚拟机各组件的统一配置 Trait 和实现。

use serde::Serialize;
use std::collections::HashMap;

/// 统一配置 Trait
///
/// 为所有虚拟机组件提供一致的配置接口。
///
/// # 示例
///
/// ```rust,ignore
/// use vm_core::config::Config;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct MyConfig {
///     pub enabled: bool,
///     pub threshold: u32,
/// }
///
/// impl Config for MyConfig {
///     fn validate(&self) -> Result<(), ConfigError> {
///         if self.threshold > 1000 {
///             return Err(ConfigError::Invalid(
///                 "threshold must be <= 1000".to_string()
///             ));
///         }
///         Ok(())
///     }
///
///     fn defaults() -> Self {
///         Self {
///             enabled: true,
///             threshold: 100,
///         }
///     }
///
///     fn merge(&self, other: &Self) -> Result<Self, ConfigError>
///     where
///         Self: Sized,
///     {
///         Ok(Self {
///             enabled: other.enabled, // 使用 other 的值
///             threshold: self.threshold.max(other.threshold),
///         })
///     }
/// }
/// ```
pub trait Config: Serialize + serde::de::DeserializeOwned {
    /// 验证配置的有效性
    ///
    /// # 错误
    ///
    /// 如果配置无效，返回 `ConfigError::Invalid`
    fn validate(&self) -> Result<(), ConfigError>;

    /// 获取默认配置
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let config = MyConfig::defaults();
    /// assert_eq!(config.enabled, true);
    /// ```
    fn defaults() -> Self;

    /// 合并两个配置
    ///
    /// 当存在多个配置源时（如文件、环境变量、命令行），
    /// 合并策略为：`self` 为基础配置，`other` 优先级更高
    ///
    /// # 参数
    ///
    /// * `other` - 优先级更高的配置
    ///
    /// # 错误
    ///
    /// 如果配置冲突无法合并，返回 `ConfigError::MergeConflict`
    fn merge(&self, other: &Self) -> Result<Self, ConfigError>
    where
        Self: Sized;

    /// 从 TOML 字符串加载配置
    ///
    /// # 参数
    ///
    /// * `toml` - TOML 格式的配置字符串
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let toml = r#"
    ///     enabled = true
    ///     threshold = 200
    /// "#;
    /// let config = MyConfig::from_toml(toml)?;
    /// ```
    fn from_toml(toml: &str) -> Result<Self, ConfigError>
    where
        Self: Sized,
    {
        toml::from_str(toml).map_err(|e| ConfigError::Parse(format!("TOML parse error: {}", e)))
    }

    /// 从 JSON 字符串加载配置
    ///
    /// # 参数
    ///
    /// * `json` - JSON 格式的配置字符串
    fn from_json(json: &str) -> Result<Self, ConfigError>
    where
        Self: Sized,
    {
        serde_json::from_str(json)
            .map_err(|e| ConfigError::Parse(format!("JSON parse error: {}", e)))
    }

    /// 将配置序列化为 TOML
    fn to_toml(&self) -> Result<String, ConfigError> {
        toml::to_string_pretty(self)
            .map_err(|e| ConfigError::Serialize(format!("TOML serialize error: {}", e)))
    }

    /// 将配置序列化为 JSON
    fn to_json(&self) -> Result<String, ConfigError> {
        serde_json::to_string_pretty(self)
            .map_err(|e| ConfigError::Serialize(format!("JSON serialize error: {}", e)))
    }

    /// 从环境变量加载配置
    ///
    /// 环境变量格式: `PREFIX_KEY`
    ///
    /// # 参数
    ///
    /// * `prefix` - 环境变量前缀，如 "VM"
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// // 设置环境变量: VM_ENABLED=true, VM_THRESHOLD=200
    /// std::env::set_var("VM_ENABLED", "true");
    /// std::env::set_var("VM_THRESHOLD", "200");
    ///
    /// let config = MyConfig::from_env("VM")?;
    /// ```
    fn from_env(_prefix: &str) -> Result<Self, ConfigError>
    where
        Self: Sized,
    {
        // 默认实现：环境变量支持需要手动实现
        Err(ConfigError::NotSupported(
            "Environment variable loading not implemented for this config type".to_string(),
        ))
    }

    /// 获取配置的差异
    ///
    /// 返回与另一个配置的差异
    fn diff(&self, other: &Self) -> HashMap<String, ConfigDiff>
    where
        Self: Sized,
    {
        let mut diffs = HashMap::new();

        // 简化实现：序列化后比较
        let self_json = serde_json::to_string(self).unwrap_or_default();
        let other_json = serde_json::to_string(other).unwrap_or_default();

        if self_json != other_json {
            diffs.insert(
                "root".to_string(),
                ConfigDiff::Changed {
                    old: self_json,
                    new: other_json,
                },
            );
        }

        diffs
    }
}

/// 配置差异类型
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigDiff {
    /// 添加了新配置
    Added { value: String },
    /// 删除了配置
    Removed { value: String },
    /// 配置改变
    Changed { old: String, new: String },
    /// 无变化
    Unchanged,
}

/// 配置错误类型
#[derive(Debug, Clone, thiserror::Error)]
pub enum ConfigError {
    /// 无效的配置
    #[error("Invalid configuration: {0}")]
    Invalid(String),

    /// 配置合并冲突
    #[error("Configuration merge conflict: {0}")]
    MergeConflict(String),

    /// 解析错误
    #[error("Failed to parse configuration: {0}")]
    Parse(String),

    /// 序列化错误
    #[error("Failed to serialize configuration: {0}")]
    Serialize(String),

    /// IO 错误
    #[error("IO error: {0}")]
    Io(String),

    /// 不支持的操作
    #[error("Operation not supported: {0}")]
    NotSupported(String),

    /// 验证错误
    #[error("Validation failed: {0}")]
    Validation(String),
}

/// 为 Vec<T: Config> 提供批量验证
///
/// # 示例
///
/// ```rust,ignore
/// let configs = vec![config1, config2, config3];
/// configs.validate_all()?;
/// ```
pub trait ConfigVecExt<T: Config> {
    fn validate_all(&self) -> Result<(), Vec<ConfigError>>;
}

impl<T: Config> ConfigVecExt<T> for Vec<T> {
    fn validate_all(&self) -> Result<(), Vec<ConfigError>> {
        let errors: Vec<_> = self
            .iter()
            .enumerate()
            .filter_map(|(i, config)| config.validate().err().map(|e| (i, e)))
            .map(|(i, e)| ConfigError::Validation(format!("Config at index {} failed: {}", i, e)))
            .collect();

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// 配置构建器
///
/// 提供流式 API 构建配置
///
/// # 示例
///
/// ```rust,ignore
/// use vm_core::config::ConfigBuilder;
///
/// let config = ConfigBuilder::new()
///     .with_file("config.toml")?
///     .with_env("VM")?
///     .with_cli_args(args)?
///     .build()?;
/// ```
pub struct ConfigBuilder<C: Config> {
    base: C,
    overrides: Vec<C>,
}

impl<C: Config + Default> Default for ConfigBuilder<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C: Config> ConfigBuilder<C> {
    /// 创建新的配置构建器
    pub fn new() -> Self
    where
        C: Default,
    {
        Self {
            base: C::default(),
            overrides: Vec::new(),
        }
    }

    /// 使用默认配置
    pub fn with_defaults() -> Self {
        Self {
            base: C::defaults(),
            overrides: Vec::new(),
        }
    }

    /// 从文件加载配置
    pub fn with_file(mut self, path: &str) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::Io(format!("Failed to read {}: {}", path, e)))?;

        let config = C::from_toml(&content)?;
        self.overrides.push(config);
        Ok(self)
    }

    /// 从环境变量加载配置
    pub fn with_env(mut self, prefix: &str) -> Result<Self, ConfigError> {
        let config = C::from_env(prefix)?;
        self.overrides.push(config);
        Ok(self)
    }

    /// 添加配置覆盖
    pub fn with_override(mut self, config: C) -> Self {
        self.overrides.push(config);
        self
    }

    /// 构建最终配置
    ///
    /// 按优先级合并所有配置：base < file < env < overrides
    pub fn build(mut self) -> Result<C, ConfigError> {
        let mut current = self.base;

        for override_config in self.overrides.drain(..) {
            current = current.merge(&override_config)?;
        }

        current.validate()?;
        Ok(current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestConfig {
        pub enabled: bool,
        pub threshold: u32,
        pub name: String,
    }

    impl Config for TestConfig {
        fn validate(&self) -> Result<(), ConfigError> {
            if self.threshold > 1000 {
                return Err(ConfigError::Invalid(
                    "threshold must be <= 1000".to_string(),
                ));
            }
            if self.name.is_empty() {
                return Err(ConfigError::Invalid("name cannot be empty".to_string()));
            }
            Ok(())
        }

        fn defaults() -> Self {
            Self {
                enabled: true,
                threshold: 100,
                name: "default".to_string(),
            }
        }

        fn merge(&self, other: &Self) -> Result<Self, ConfigError> {
            Ok(Self {
                enabled: other.enabled,
                threshold: self.threshold.max(other.threshold),
                name: if other.name.is_empty() {
                    self.name.clone()
                } else {
                    other.name.clone()
                },
            })
        }
    }

    #[test]
    fn test_config_validate() {
        let config = TestConfig {
            enabled: true,
            threshold: 500,
            name: "test".to_string(),
        };
        assert!(config.validate().is_ok());

        let invalid_config = TestConfig {
            enabled: true,
            threshold: 2000,
            name: "test".to_string(),
        };
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_config_defaults() {
        let config = TestConfig::defaults();
        assert!(config.enabled);
        assert_eq!(config.threshold, 100);
        assert_eq!(config.name, "default");
    }

    #[test]
    fn test_config_merge() {
        let base = TestConfig {
            enabled: false,
            threshold: 100,
            name: "base".to_string(),
        };

        let override_config = TestConfig {
            enabled: true,
            threshold: 200,
            name: "override".to_string(),
        };

        let merged = base.merge(&override_config).unwrap();
        assert!(merged.enabled); // 来自 override
        assert_eq!(merged.threshold, 200); // max(100, 200)
        assert_eq!(merged.name, "override"); // 来自 override（非空）
    }

    #[test]
    fn test_config_from_toml() {
        let toml = r#"
            enabled = false
            threshold = 300
            name = "from_toml"
        "#;

        let config = TestConfig::from_toml(toml).unwrap();
        assert!(!config.enabled);
        assert_eq!(config.threshold, 300);
        assert_eq!(config.name, "from_toml");
    }

    #[test]
    fn test_config_to_toml() {
        let config = TestConfig {
            enabled: true,
            threshold: 150,
            name: "test".to_string(),
        };

        let toml = config.to_toml().unwrap();
        assert!(toml.contains("enabled = true"));
        assert!(toml.contains("threshold = 150"));
        assert!(toml.contains("name = \"test\""));
    }

    #[test]
    fn test_config_validate_all() {
        let configs = vec![
            TestConfig {
                enabled: true,
                threshold: 100,
                name: "config1".to_string(),
            },
            TestConfig {
                enabled: true,
                threshold: 200,
                name: "config2".to_string(),
            },
        ];

        assert!(configs.validate_all().is_ok());

        let invalid_configs = vec![
            TestConfig {
                enabled: true,
                threshold: 100,
                name: "valid".to_string(),
            },
            TestConfig {
                enabled: true,
                threshold: 2000, // 无效
                name: "invalid".to_string(),
            },
        ];

        assert!(invalid_configs.validate_all().is_err());
    }

    #[test]
    fn test_config_builder() {
        let base = TestConfig {
            enabled: false,
            threshold: 50,
            name: "base".to_string(),
        };

        let override_config = TestConfig {
            enabled: true,
            threshold: 150,
            name: "override".to_string(),
        };

        let result = ConfigBuilder::with_defaults()
            .with_override(base)
            .with_override(override_config)
            .build()
            .unwrap();

        assert!(result.enabled);
        assert_eq!(result.threshold, 150);
        assert_eq!(result.name, "override");
    }
}
