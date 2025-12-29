//! JIT引擎通用配置基础设施
//!
//! 本模块提供了统一的配置基础设施，用于减少重复代码并提高一致性。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
#[cfg(feature = "vm-accel")]
use vm_accel::AccelKind;

// 条件导入 num_cpus
cfg_if::cfg_if! {
    if #[cfg(all(target_os = "linux", feature = "std"))] {
        use num_cpus;

        fn get_cpu_count() -> usize {
            num_cpus::get()
        }
    } else {
        fn get_cpu_count() -> usize {
            1 // 默认单线程
        }
    }
}

/// 配置特征
pub trait Config: Clone + Default + Send + Sync {
    /// 验证配置
    fn validate(&self) -> Result<(), String>;

    /// 获取配置摘要
    fn summary(&self) -> String;

    /// 合并另一个配置
    fn merge(&mut self, other: &Self);
}

/// 基础配置结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseConfig {
    /// 是否启用调试模式
    pub debug_enabled: bool,
    /// 日志级别
    pub log_level: LogLevel,
    /// 工作线程数
    pub worker_threads: usize,
    /// 最大内存使用量（字节）
    pub max_memory_bytes: Option<u64>,
    /// 操作超时时间
    pub operation_timeout: Duration,
    /// 配置版本
    pub config_version: String,
}

impl Default for BaseConfig {
    fn default() -> Self {
        Self {
            debug_enabled: false,
            log_level: LogLevel::Info,
            worker_threads: get_cpu_count(),
            max_memory_bytes: None,
            operation_timeout: Duration::from_secs(30),
            config_version: "1.0.0".to_string(),
        }
    }
}

impl BaseConfig {
    /// 创建新的基础配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置调试模式
    pub fn with_debug(mut self, enabled: bool) -> Self {
        self.debug_enabled = enabled;
        self
    }

    /// 设置日志级别
    pub fn with_log_level(mut self, level: LogLevel) -> Self {
        self.log_level = level;
        self
    }

    /// 设置工作线程数
    pub fn with_worker_threads(mut self, threads: usize) -> Self {
        self.worker_threads = threads;
        self
    }

    /// 设置最大内存使用量
    pub fn with_max_memory(mut self, bytes: u64) -> Self {
        self.max_memory_bytes = Some(bytes);
        self
    }

    /// 设置操作超时时间
    pub fn with_operation_timeout(mut self, timeout: Duration) -> Self {
        self.operation_timeout = timeout;
        self
    }
}

impl Config for BaseConfig {
    fn validate(&self) -> Result<(), String> {
        if self.worker_threads == 0 {
            return Err("工作线程数必须大于0".to_string());
        }

        if self.operation_timeout.is_zero() {
            return Err("操作超时时间必须大于0".to_string());
        }

        if let Some(max_memory) = self.max_memory_bytes
            && max_memory == 0
        {
            return Err("最大内存使用量必须大于0".to_string());
        }

        Ok(())
    }

    fn summary(&self) -> String {
        format!(
            "基础配置: 调试={}, 日志级别={}, 工作线程={}, 最大内存={}MB, 超时={}s",
            self.debug_enabled,
            self.log_level,
            self.worker_threads,
            self.max_memory_bytes
                .map(|bytes| bytes / (1024 * 1024))
                .unwrap_or(0),
            self.operation_timeout.as_secs()
        )
    }

    fn merge(&mut self, other: &Self) {
        // 只合并Some值，None值保持不变
        if other.debug_enabled {
            self.debug_enabled = other.debug_enabled;
        }

        if other.log_level != LogLevel::Info {
            self.log_level = other.log_level;
        }

        if other.worker_threads != get_cpu_count() {
            self.worker_threads = other.worker_threads;
        }

        if other.max_memory_bytes.is_some() {
            self.max_memory_bytes = other.max_memory_bytes;
        }

        if !other.operation_timeout.is_zero() && other.operation_timeout != Duration::from_secs(30)
        {
            self.operation_timeout = other.operation_timeout;
        }

        if !other.config_version.is_empty() && other.config_version != "1.0.0" {
            self.config_version = other.config_version.clone();
        }
    }
}

/// 日志级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LogLevel {
    /// 跟踪
    Trace,
    /// 调试
    Debug,
    /// 信息
    #[default]
    Info,
    /// 警告
    Warn,
    /// 错误
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trace => write!(f, "TRACE"),
            Self::Debug => write!(f, "DEBUG"),
            Self::Info => write!(f, "INFO"),
            Self::Warn => write!(f, "WARN"),
            Self::Error => write!(f, "ERROR"),
        }
    }
}

/// 缓存配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// 是否启用缓存
    pub enabled: bool,
    /// 缓存大小限制（字节）
    pub size_limit_bytes: usize,
    /// 缓存条目限制
    pub entry_limit: usize,
    /// 缓存清理策略
    pub eviction_policy: EvictionPolicy,
    /// 缓存TTL（生存时间）
    pub ttl: Option<Duration>,
    /// 是否启用预取
    pub prefetch_enabled: bool,
    /// 预取窗口大小
    pub prefetch_window_size: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            size_limit_bytes: 64 * 1024 * 1024, // 64MB
            entry_limit: 10000,
            eviction_policy: EvictionPolicy::LRU,
            ttl: None,
            prefetch_enabled: false,
            prefetch_window_size: 10,
        }
    }
}

impl CacheConfig {
    /// 创建新的缓存配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置缓存大小限制
    pub fn with_size_limit(mut self, bytes: usize) -> Self {
        self.size_limit_bytes = bytes;
        self
    }

    /// 设置条目限制
    pub fn with_entry_limit(mut self, limit: usize) -> Self {
        self.entry_limit = limit;
        self
    }

    /// 设置清理策略
    pub fn with_eviction_policy(mut self, policy: EvictionPolicy) -> Self {
        self.eviction_policy = policy;
        self
    }

    /// 设置TTL
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    /// 启用预取
    pub fn with_prefetch(mut self, enabled: bool, window_size: usize) -> Self {
        self.prefetch_enabled = enabled;
        self.prefetch_window_size = window_size;
        self
    }
}

impl Config for CacheConfig {
    fn validate(&self) -> Result<(), String> {
        if self.enabled {
            if self.size_limit_bytes == 0 {
                return Err("缓存大小限制必须大于0".to_string());
            }

            if self.entry_limit == 0 {
                return Err("缓存条目限制必须大于0".to_string());
            }

            if self.prefetch_enabled && self.prefetch_window_size == 0 {
                return Err("预取窗口大小必须大于0".to_string());
            }
        }

        Ok(())
    }

    fn summary(&self) -> String {
        if !self.enabled {
            return "缓存已禁用".to_string();
        }

        format!(
            "缓存配置: 大小={}MB, 条目={}, 策略={:?}, TTL={:?}, 预取={}",
            self.size_limit_bytes / (1024 * 1024),
            self.entry_limit,
            self.eviction_policy,
            self.ttl,
            self.prefetch_enabled
        )
    }

    fn merge(&mut self, other: &Self) {
        if other.enabled {
            self.enabled = other.enabled;
        }

        if other.size_limit_bytes != 64 * 1024 * 1024 {
            self.size_limit_bytes = other.size_limit_bytes;
        }

        if other.entry_limit != 10000 {
            self.entry_limit = other.entry_limit;
        }

        if other.eviction_policy != EvictionPolicy::LRU {
            self.eviction_policy = other.eviction_policy;
        }

        if other.ttl.is_some() {
            self.ttl = other.ttl;
        }

        if other.prefetch_enabled {
            self.prefetch_enabled = other.prefetch_enabled;
            self.prefetch_window_size = other.prefetch_window_size;
        }
    }
}

/// 缓存清理策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EvictionPolicy {
    /// 最近最少使用
    #[default]
    LRU,
    /// 先进先出
    FIFO,
    /// 最少使用
    LFU,
    /// 随机
    Random,
    /// 时钟算法
    Clock,
}

/// 硬件加速配置
#[cfg(feature = "vm-accel")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareAccelerationConfig {
    /// 是否启用硬件加速
    pub enabled: bool,
    /// 首选加速器类型
    pub preferred_accelerator: Option<AccelKind>,
    /// 是否启用自动检测
    pub auto_detection: bool,
    /// 是否启用SIMD优化
    pub simd_enabled: bool,
    /// 性能监控间隔
    pub performance_monitoring_interval: Duration,
    /// 回退阈值
    pub fallback_threshold: f64,
}

#[cfg(feature = "vm-accel")]
impl Default for HardwareAccelerationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            preferred_accelerator: None,
            auto_detection: true,
            simd_enabled: true,
            performance_monitoring_interval: Duration::from_secs(10),
            fallback_threshold: 0.8,
        }
    }
}

#[cfg(feature = "vm-accel")]
impl HardwareAccelerationConfig {
    /// 创建新的硬件加速配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置首选加速器
    pub fn with_preferred_accelerator(mut self, accelerator: AccelKind) -> Self {
        self.preferred_accelerator = Some(accelerator);
        self
    }

    /// 启用/禁用自动检测
    pub fn with_auto_detection(mut self, enabled: bool) -> Self {
        self.auto_detection = enabled;
        self
    }

    /// 启用/禁用SIMD
    pub fn with_simd(mut self, enabled: bool) -> Self {
        self.simd_enabled = enabled;
        self
    }

    /// 设置性能监控间隔
    pub fn with_performance_monitoring_interval(mut self, interval: Duration) -> Self {
        self.performance_monitoring_interval = interval;
        self
    }

    /// 设置回退阈值
    pub fn with_fallback_threshold(mut self, threshold: f64) -> Self {
        self.fallback_threshold = threshold;
        self
    }
}

#[cfg(feature = "vm-accel")]
impl Config for HardwareAccelerationConfig {
    fn validate(&self) -> Result<(), String> {
        if self.enabled {
            if self.fallback_threshold < 0.0 || self.fallback_threshold > 1.0 {
                return Err("回退阈值必须在0.0到1.0之间".to_string());
            }

            if self.performance_monitoring_interval.is_zero() {
                return Err("性能监控间隔必须大于0".to_string());
            }
        }

        Ok(())
    }

    fn summary(&self) -> String {
        if !self.enabled {
            return "硬件加速已禁用".to_string();
        }

        format!(
            "硬件加速配置: 首选加速器={:?}, 自动检测={}, SIMD={}, 监控间隔={}s, 回退阈值={:.2}",
            self.preferred_accelerator,
            self.auto_detection,
            self.simd_enabled,
            self.performance_monitoring_interval.as_secs(),
            self.fallback_threshold
        )
    }

    fn merge(&mut self, other: &Self) {
        if other.enabled {
            self.enabled = other.enabled;
        }

        if other.preferred_accelerator.is_some() {
            self.preferred_accelerator = other.preferred_accelerator;
        }

        if other.auto_detection {
            self.auto_detection = other.auto_detection;
        }

        if other.simd_enabled {
            self.simd_enabled = other.simd_enabled;
        }

        if other.performance_monitoring_interval != Duration::from_secs(10) {
            self.performance_monitoring_interval = other.performance_monitoring_interval;
        }

        if other.fallback_threshold != 0.8 {
            self.fallback_threshold = other.fallback_threshold;
        }
    }
}

/// 配置管理器
pub struct ConfigManager {
    configs: HashMap<String, String>, // 存储配置的JSON序列化
    base_config: BaseConfig,
}

impl ConfigManager {
    /// 创建新的配置管理器
    pub fn new(base_config: BaseConfig) -> Self {
        Self {
            configs: HashMap::new(),
            base_config,
        }
    }

    /// 添加配置
    pub fn add_config<C: Config + Serialize + 'static>(
        &mut self,
        name: &str,
        config: C,
    ) -> Result<(), serde_json::Error> {
        let json = serde_json::to_string(&config)?;
        self.configs.insert(name.to_string(), json);
        Ok(())
    }

    /// 获取配置
    pub fn get_config<C: serde::de::DeserializeOwned>(
        &self,
        name: &str,
    ) -> Result<Option<C>, serde_json::Error> {
        match self.configs.get(name) {
            Some(json) => {
                let config: C = serde_json::from_str(json)?;
                Ok(Some(config))
            }
            None => Ok(None),
        }
    }

    /// 获取可变配置
    pub fn get_config_mut<C: serde::de::DeserializeOwned + serde::Serialize>(
        &mut self,
        name: &str,
    ) -> Result<Option<C>, serde_json::Error> {
        match self.configs.get(name) {
            Some(json) => {
                let config: C = serde_json::from_str(json)?;
                Ok(Some(config))
            }
            None => Ok(None),
        }
    }

    /// 验证所有配置
    pub fn validate_all(&self) -> Result<(), String> {
        // 验证基础配置
        self.base_config.validate()?;

        // 验证其他配置
        for (name, json) in &self.configs {
            // 尝试解析为BaseConfig进行基本验证
            if serde_json::from_str::<BaseConfig>(json).is_ok() {
                // 基本JSON格式正确
            } else {
                return Err(format!("配置 '{}' 格式无效", name));
            }
        }

        Ok(())
    }

    /// 生成配置报告
    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        report.push_str("=== 配置报告 ===\n");
        report.push_str(&format!("{}\n", self.base_config.summary()));

        for (name, json) in &self.configs {
            report.push_str(&format!("{}: {}\n", name, json));
        }

        report
    }

    /// 获取基础配置
    pub fn base_config(&self) -> &BaseConfig {
        &self.base_config
    }

    /// 获取可变基础配置
    pub fn base_config_mut(&mut self) -> &mut BaseConfig {
        &mut self.base_config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_config() {
        let config = BaseConfig::new()
            .with_debug(true)
            .with_log_level(LogLevel::Debug)
            .with_worker_threads(4)
            .with_max_memory(1024 * 1024 * 1024)
            .with_operation_timeout(Duration::from_secs(60));

        assert!(config.validate().is_ok());
        assert!(config.debug_enabled);
        assert_eq!(config.log_level, LogLevel::Debug);
        assert_eq!(config.worker_threads, 4);
        assert_eq!(config.max_memory_bytes, Some(1024 * 1024 * 1024));
        assert_eq!(config.operation_timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_cache_config() {
        let config = CacheConfig::new()
            .with_size_limit(32 * 1024 * 1024)
            .with_entry_limit(5000)
            .with_eviction_policy(EvictionPolicy::LFU)
            .with_ttl(Duration::from_secs(300))
            .with_prefetch(true, 20);

        assert!(config.validate().is_ok());
        assert_eq!(config.size_limit_bytes, 32 * 1024 * 1024);
        assert_eq!(config.entry_limit, 5000);
        assert_eq!(config.eviction_policy, EvictionPolicy::LFU);
        assert_eq!(config.ttl, Some(Duration::from_secs(300)));
        assert!(config.prefetch_enabled);
        assert_eq!(config.prefetch_window_size, 20);
    }

    #[test]
    fn test_hardware_acceleration_config() {
        let config = HardwareAccelerationConfig::new()
            .with_preferred_accelerator(AccelKind::Kvm)
            .with_auto_detection(false)
            .with_simd(true)
            .with_performance_monitoring_interval(Duration::from_secs(5))
            .with_fallback_threshold(0.7);

        assert!(config.validate().is_ok());
        assert_eq!(config.preferred_accelerator, Some(AccelKind::Kvm));
        assert!(!config.auto_detection);
        assert!(config.simd_enabled);
        assert_eq!(
            config.performance_monitoring_interval,
            Duration::from_secs(5)
        );
        assert_eq!(config.fallback_threshold, 0.7);
    }

    #[test]
    fn test_config_merge() {
        let mut config1 = BaseConfig::new().with_debug(true);
        let config2 = BaseConfig::new().with_log_level(LogLevel::Error);

        config1.merge(&config2);

        assert!(config1.debug_enabled); // 保持原有值
        assert_eq!(config1.log_level, LogLevel::Error); // 合并新值
    }

    #[test]
    fn test_config_manager() {
        let mut manager = ConfigManager::new(BaseConfig::new());
        manager.add_config("cache", CacheConfig::new());
        manager.add_config("hw_accel", HardwareAccelerationConfig::new());

        assert!(manager.validate_all().is_ok());

        let cache_config = manager.get_config::<CacheConfig>("cache");
        assert!(cache_config.is_ok() && cache_config.expect("Cache config should be Ok").is_some());

        let hw_config = manager.get_config::<HardwareAccelerationConfig>("hw_accel");
        assert!(hw_config.is_ok() && hw_config.expect("HW config should be Ok").is_some());
    }
}
