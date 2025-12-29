// VM配置管理（Configuration Management）
//
// 提供VM的配置管理功能，支持：
// - 层级配置（全局、VM级、模块级）
// - 动态配置更新
// - 配置序列化和持久化

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

/// 配置键类型
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConfigKey {
    /// 全局配置
    Global(&'static str),
    /// VM级别配置
    Vm(&'static str),
    /// 模块级别配置
    Module(&'static str, &'static str),
}

/// 配置值
#[derive(Debug, Clone)]
pub enum ConfigValue {
    /// 布尔值
    Bool(bool),
    /// 整数值
    Int(i64),
    /// 浮点数值
    Float(f64),
    /// 字符串值
    String(String),
    /// 列表值
    List(Vec<ConfigValue>),
}

impl ConfigValue {
    /// 获取布尔值
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ConfigValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// 获取整数值
    pub fn as_int(&self) -> Option<i64> {
        match self {
            ConfigValue::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// 获取浮点数值
    pub fn as_float(&self) -> Option<f64> {
        match self {
            ConfigValue::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// 获取字符串值
    pub fn as_string(&self) -> Option<&str> {
        match self {
            ConfigValue::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// 获取列表值
    pub fn as_list(&self) -> Option<&Vec<ConfigValue>> {
        match self {
            ConfigValue::List(l) => Some(l),
            _ => None,
        }
    }
}

/// 配置管理器
pub struct ConfigManager {
    /// 配置存储
    configs: Arc<RwLock<HashMap<ConfigKey, ConfigValue>>>,
    /// 配置文件路径
    config_path: Option<Path>,
    /// 是否启用自动保存
    auto_save: bool,
}

impl ConfigManager {
    /// 创建新的配置管理器
    ///
    /// # 参数
    /// - `config_path`: 配置文件路径（可选）
    /// - `auto_save`: 是否启用自动保存（默认true）
    ///
    /// # 示例
    /// ```ignore
    /// let manager = ConfigManager::new(None, true);
    /// ```
    pub fn new(config_path: Option<Path>, auto_save: bool) -> Self {
        Self {
            configs: Arc::new(RwLock::new(HashMap::new())),
            config_path,
            auto_save,
        }
    }

    /// 使用默认配置创建
    pub fn default() -> Self {
        Self::new(None, true)
    }

    /// Helper to acquire configs read lock with error handling
    fn lock_read(&self) -> Result<std::sync::RwLockReadGuard<HashMap<ConfigKey, ConfigValue>>, String> {
        self.configs.read().map_err(|e| format!("Configs read lock is poisoned: {:?}", e))
    }

    /// Helper to acquire configs write lock with error handling
    fn lock_write(&self) -> Result<std::sync::RwLockWriteGuard<HashMap<ConfigKey, ConfigValue>>, String> {
        self.configs.write().map_err(|e| format!("Configs write lock is poisoned: {:?}", e))
    }

    /// 获取配置值
    ///
    /// # 参数
    /// - `key`: 配置键
    ///
    /// # 返回
    /// - `Some(value)`: 配置存在
    /// - `None`: 配置不存在
    ///
    /// # 示例
    /// ```ignore
    /// let value = manager.get(ConfigKey::Global("max_tlb_entries"));
    /// ```
    pub fn get(&self, key: ConfigKey) -> Option<ConfigValue> {
        self.lock_read().ok()?.get(&key).cloned()
    }

    /// 设置配置值
    ///
    /// # 参数
    /// - `key`: 配置键
    /// - `value`: 配置值
    ///
    /// # 示例
    /// ```ignore
    /// manager.set(ConfigKey::Global("max_tlb_entries"), ConfigValue::Int(256));
    /// ```
    pub fn set(&self, key: ConfigKey, value: ConfigValue) -> Result<(), String> {
        let mut configs = self.lock_write()?;
        configs.insert(key, value);

        if self.auto_save {
            drop(configs);
            self.save()?;
        }

        Ok(())
    }

    /// 删除配置值
    pub fn remove(&self, key: ConfigKey) -> Result<(), String> {
        let mut configs = self.lock_write()?;
        configs.remove(&key);

        if self.auto_save {
            drop(configs);
            self.save()?;
        }

        Ok(())
    }

    /// 从文件加载配置
    ///
    /// # 示例
    /// ```ignore
    /// manager.load_from_file("/path/to/config.json")?;
    /// ```
    pub fn load_from_file(&self, path: &str) -> Result<(), String> {
        use std::fs;
        
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read config file: {}", e))?;
        
        // 解析配置（简化版：仅支持键值对）
        let mut configs = HashMap::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            
            let parts: Vec<&str> = line.split('=').collect();
            if parts.len() == 2 {
                let key = parts[0].trim();
                let value = parts[1].trim();
                
                // 简化的值解析
                let config_value = if value == "true" {
                    ConfigValue::Bool(true)
                } else if value == "false" {
                    ConfigValue::Bool(false)
                } else if let Ok(i) = value.parse::<i64>() {
                    ConfigValue::Int(i)
                } else if let Ok(f) = value.parse::<f64>() {
                    ConfigValue::Float(f)
                } else {
                    ConfigValue::String(value.to_string())
                };
                
                configs.insert(ConfigKey::Global(key.to_string()), config_value);
            }
        }

        let mut manager_configs = self.lock_write()?;
        *manager_configs = configs;
        self.config_path = Some(Path::new(path).to_path_buf());

        Ok(())
    }

    /// 保存配置到文件
    fn save(&self) -> Result<(), String> {
        if let Some(path) = &self.config_path {
            use std::fs;
            use std::io::Write;

            let configs = self.lock_read()?;

            let mut content = String::new();
            content.push_str("# VM Configuration\n");
            content.push_str("# Generated by vm-common\n\n");

            for (key, value) in configs.iter() {
                let value_str = match value {
                    ConfigValue::Bool(b) => b.to_string(),
                    ConfigValue::Int(i) => i.to_string(),
                    ConfigValue::Float(f) => f.to_string(),
                    ConfigValue::String(s) => s.clone(),
                    ConfigValue::List(l) => {
                        let inner: Vec<String> = l.iter()
                            .map(|v| match v {
                                ConfigValue::Bool(b) => b.to_string(),
                                ConfigValue::Int(i) => i.to_string(),
                                ConfigValue::Float(f) => f.to_string(),
                                ConfigValue::String(s) => s.clone(),
                            })
                            .collect();
                        format!("[{}]", inner.join(", "))
                    }
                };

                content.push_str(&format!("{} = {}\n", key, value_str));
            }

            fs::write(path, content)
                .map_err(|e| format!("Failed to save config file: {}", e))?;
        }

        Ok(())
    }

    /// 获取所有配置
    pub fn get_all_configs(&self) -> HashMap<ConfigKey, ConfigValue> {
        match self.lock_read() {
            Ok(configs) => configs.iter().map(|(k, v)| (*k, v.clone())).collect(),
            Err(_) => HashMap::new(),
        }
    }

    /// 清空所有配置
    pub fn clear(&self) {
        if let Ok(mut configs) = self.lock_write() {
            configs.clear();
        }
    }

    /// 获取配置统计
    pub fn get_stats(&self) -> ConfigStats {
        match self.lock_read() {
            Ok(configs) => ConfigStats {
                total_configs: configs.len(),
                global_configs: configs.keys().filter(|k| matches!(k, ConfigKey::Global(_))).count(),
                vm_configs: configs.keys().filter(|k| matches!(k, ConfigKey::Vm(_))).count(),
                module_configs: configs.keys().filter(|k| matches!(k, ConfigKey::Module(_, _))).count(),
            },
            Err(_) => ConfigStats {
                total_configs: 0,
                global_configs: 0,
                vm_configs: 0,
                module_configs: 0,
            },
        }
    }
}

/// 配置统计信息
#[derive(Debug, Clone)]
pub struct ConfigStats {
    pub total_configs: usize,
    pub global_configs: usize,
    pub vm_configs: usize,
    pub module_configs: usize,
}

impl ConfigKey {
    /// 创建全局配置键
    pub fn Global(key: &str) -> Self {
        Self::Global(key.to_string())
    }

    /// 创建VM级别配置键
    pub fn Vm(vm_name: &str, key: &str) -> Self {
        Self::Vm(vm_name.to_string(), key.to_string())
    }

    /// 创建模块级别配置键
    pub fn Module(module: &str, key: &str) -> Self {
        Self::Module(module.to_string(), key.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_key_creation() {
        let key = ConfigKey::Global("test_key");
        assert!(matches!(key, ConfigKey::Global("test_key")));
    }

    #[test]
    fn test_config_value_bool() {
        let value = ConfigValue::Bool(true);
        assert!(value.as_bool().unwrap());
        assert_eq!(value.as_int(), None);
    }

    #[test]
    fn test_config_value_int() {
        let value = ConfigValue::Int(42);
        assert!(value.as_int().unwrap());
        assert_eq!(value.as_bool(), None);
    }

    #[test]
    fn test_config_manager_creation() {
        let manager = ConfigManager::new(None, true);
        assert_eq!(manager.config_path, None);
        assert!(manager.auto_save);
    }

    #[test]
    fn test_config_get_set() {
        let mut manager = ConfigManager::new(None, false);
        
        let key = ConfigKey::Global("test_key");
        let value = ConfigValue::Int(256);
        
        // 测试获取不存在的配置
        assert!(manager.get(key).is_none());
        
        // 设置配置
        manager.set(key.clone(), value).unwrap();
        
        // 测试获取存在的配置
        assert_eq!(manager.get(key), Some(ConfigValue::Int(256)));
    }

    #[test]
    fn test_config_stats() {
        let mut manager = ConfigManager::new(None, false);
        
        let key1 = ConfigKey::Global("key1");
        let key2 = ConfigKey::V Global("vm1", "key2");
        let key3 = ConfigKey::Module("module1", "key3");
        
        manager.set(key1, ConfigValue::Bool(true)).unwrap();
        manager.set(key2, ConfigValue::String("value".to_string())).unwrap();
        manager.set(key3, ConfigValue::List(vec![
            ConfigValue::Bool(true),
            ConfigValue::Int(42)
        ])).unwrap();
        
        let stats = manager.get_stats();
        assert_eq!(stats.total_configs, 3);
        assert_eq!(stats.global_configs, 2);
        assert_eq!(stats.vm_configs, 1);
        assert_eq!(stats.module_configs, 1);
    }

    #[test]
    fn test_config_clear() {
        let mut manager = ConfigManager::new(None, false);
        
        manager.set(ConfigKey::Global("test"), ConfigValue::Bool(true)).unwrap();
        assert_eq!(manager.configs.read().unwrap().len(), 1);
        
        manager.clear();
        assert_eq!(manager.configs.read().unwrap().len(), 0);
    }
}
