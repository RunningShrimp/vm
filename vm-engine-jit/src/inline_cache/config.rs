use serde::{Deserialize, Serialize};

/// 内联缓存配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineCacheConfig {
    /// 单态缓存最大条目数
    pub max_monomorphic_entries: usize,
    /// 多态缓存最大条目数
    pub max_polymorphic_entries: usize,
    /// 升级到多态缓存的阈值
    pub polymorphic_threshold: u32,
    /// 缓存条目超时时间（毫秒）
    pub entry_timeout_ms: u64,
    /// 启用自适应缓存大小
    pub enable_adaptive_sizing: bool,
    /// 启用缓存预热
    pub enable_cache_warming: bool,
    /// 最大缓存大小（字节）
    pub max_cache_size_bytes: usize,
}

impl Default for InlineCacheConfig {
    fn default() -> Self {
        Self {
            max_monomorphic_entries: 1000,
            max_polymorphic_entries: 8,
            polymorphic_threshold: 3,
            entry_timeout_ms: 10000,
            enable_adaptive_sizing: true,
            enable_cache_warming: false,
            max_cache_size_bytes: 1024 * 1024, // 1MB
        }
    }
}
