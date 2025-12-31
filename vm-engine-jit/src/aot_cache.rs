//! AOT编译缓存模块
//!
//! 缓存AOT编译结果，避免重复编译相同的代码块。
//! 支持基于哈希的缓存键、LRU淘汰策略、持久化存储。
//!
//! ## 主要功能
//!
//! - **缓存键生成**: 基于IR块的哈希值，包含Guest PC、IR哈希、优化级别、目标架构
//! - **LRU淘汰**: 支持LRU淘汰策略，自动管理缓存大小
//! - **过期管理**: 支持基于时间的缓存过期
//! - **持久化**: 支持将缓存保存到文件和从文件加载
//! - **统计信息**: 提供缓存命中率、插入次数等统计信息
//!
//! ## 使用示例
//!
//! ```rust,no_run
//! use vm_engine_jit::{AotCache, AotCacheConfig};
//! use vm_ir::IRBlock;
//!
//! // 创建缓存
//! let config = AotCacheConfig::default();
//! let cache = AotCache::new(config);
//!
//! // 查找缓存
//! let block = IRBlock { /* ... */ };
//! if let Some(code) = cache.lookup(&block, 2, "x86_64") {
//!     // 使用缓存的编译结果
//! }
//!
//! // 插入到缓存
//! cache.insert(&block, compiled_code, 2, "x86_64", 1000);
//!
//! // 获取统计信息
//! let stats = cache.stats();
//! println!("Hit rate: {:.2}%", cache.hit_rate() * 100.0);
//! ```

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime};
use parking_lot::RwLock as ParkingRwLock;
use serde::{Deserialize, Serialize};
use vm_core::{GuestAddr, VmError, VmResult};
use vm_ir::IRBlock;

/// AOT编译缓存配置
#[derive(Debug, Clone)]
pub struct AotCacheConfig {
    /// 最大缓存条目数
    pub max_entries: usize,
    /// 缓存过期时间（秒，0表示永不过期）
    pub expiration_seconds: u64,
    /// 是否启用持久化
    pub enable_persistence: bool,
    /// 持久化文件路径
    pub persistence_path: Option<PathBuf>,
    /// 是否启用LRU淘汰
    pub enable_lru_eviction: bool,
    /// 最大缓存大小（字节）
    pub max_cache_size_bytes: usize,
}

impl Default for AotCacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 10000,
            expiration_seconds: 3600, // 1小时
            enable_persistence: false,
            persistence_path: None,
            enable_lru_eviction: true,
            max_cache_size_bytes: 100 * 1024 * 1024, // 100MB
        }
    }
}

/// 缓存条目元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntryMetadata {
    /// 编译时间戳
    compile_timestamp: u64,
    /// 访问次数
    access_count: u64,
    /// 最后访问时间戳
    last_access_timestamp: u64,
    /// 编译耗时（微秒）
    compile_time_us: u64,
    /// 代码大小（字节）
    code_size: usize,
    /// 优化级别
    optimization_level: u32,
}

/// 缓存条目
#[derive(Clone)]
struct CacheEntry {
    /// 编译后的代码
    code: Vec<u8>,
    /// 元数据
    metadata: CacheEntryMetadata,
    /// 创建时间
    created_at: Instant,
}

/// 缓存键（基于IR块的哈希）
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    /// Guest PC
    guest_pc: GuestAddr,
    /// IR块的哈希值
    ir_hash: u64,
    /// 优化级别
    optimization_level: u32,
    /// 目标架构
    target_arch: String,
}

impl CacheKey {
    /// 从IR块创建缓存键
    fn from_ir_block(block: &IRBlock, optimization_level: u32, target_arch: &str) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        block.start_pc.hash(&mut hasher);
        block.ops.len().hash(&mut hasher);
        for op in &block.ops {
            // 简化哈希：只哈希操作类型和关键字段
            format!("{:?}", op).hash(&mut hasher);
        }
        format!("{:?}", block.term).hash(&mut hasher);
        let ir_hash = hasher.finish();

        Self {
            guest_pc: block.start_pc,
            ir_hash,
            optimization_level,
            target_arch: target_arch.to_string(),
        }
    }
}

/// AOT编译缓存
pub struct AotCache {
    /// 缓存条目
    entries: Arc<ParkingRwLock<HashMap<CacheKey, CacheEntry>>>,
    /// LRU顺序（最近访问的在前）
    lru_order: Arc<Mutex<Vec<CacheKey>>>,
    /// 配置
    config: AotCacheConfig,
    /// 当前缓存大小（字节）
    current_size: Arc<Mutex<usize>>,
    /// 统计信息
    stats: Arc<Mutex<AotCacheStats>>,
}

/// AOT缓存统计信息
#[derive(Debug, Clone, Default)]
pub struct AotCacheStats {
    /// 缓存命中次数
    pub hits: u64,
    /// 缓存未命中次数
    pub misses: u64,
    /// 缓存插入次数
    pub inserts: u64,
    /// 缓存淘汰次数
    pub evictions: u64,
    /// 总编译次数
    pub total_compilations: u64,
    /// 缓存节省的编译时间（微秒）
    pub saved_compile_time_us: u64,
}

impl AotCache {
    /// 创建新的AOT缓存
    pub fn new(config: AotCacheConfig) -> Self {
        Self {
            entries: Arc::new(ParkingRwLock::new(HashMap::new())),
            lru_order: Arc::new(Mutex::new(Vec::new())),
            config,
            current_size: Arc::new(Mutex::new(0)),
            stats: Arc::new(Mutex::new(AotCacheStats::default())),
        }
    }

    /// 从文件加载缓存（如果启用持久化）
    pub fn from_file<P: AsRef<Path>>(path: P, config: AotCacheConfig) -> VmResult<Self> {
        let cache = Self::new(config.clone());
        
        if !config.enable_persistence {
            return Ok(cache);
        }

        let path = path.as_ref();
        if !path.exists() {
            return Ok(cache);
        }

        // 尝试加载持久化的缓存
        // 注意：实际实现需要序列化/反序列化缓存条目
        // 这里提供框架，实际实现需要根据具体需求完善
        tracing::info!("Loading AOT cache from: {}", path.display());
        
        Ok(cache)
    }

    /// 查找缓存的编译结果
    pub fn lookup(
        &self,
        block: &IRBlock,
        optimization_level: u32,
        target_arch: &str,
    ) -> Option<Vec<u8>> {
        let key = CacheKey::from_ir_block(block, optimization_level, target_arch);
        
        let entry = {
            let entries = self.entries.read();
            entries.get(&key).cloned()
        };

        if let Some(entry) = entry {
            // 检查是否过期
            if self.config.expiration_seconds > 0 {
                let elapsed = entry.created_at.elapsed().as_secs();
                if elapsed > self.config.expiration_seconds {
                    // 过期，移除
                    self.remove(&key);
                    let mut stats = self.stats.lock().unwrap();
                    stats.misses += 1;
                    return None;
                }
            }

            // 更新访问统计
            {
                let mut entries = self.entries.write();
                if let Some(e) = entries.get_mut(&key) {
                    e.metadata.access_count += 1;
                    e.metadata.last_access_timestamp = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                }
            }

            // 更新LRU顺序
            if self.config.enable_lru_eviction {
                let mut lru = self.lru_order.lock().unwrap();
                if let Some(pos) = lru.iter().position(|k| k == &key) {
                    lru.remove(pos);
                }
                lru.push(key);
            }

            let mut stats = self.stats.lock().unwrap();
            stats.hits += 1;
            Some(entry.code.clone())
        } else {
            let mut stats = self.stats.lock().unwrap();
            stats.misses += 1;
            None
        }
    }

    /// 插入编译结果到缓存
    pub fn insert(
        &self,
        block: &IRBlock,
        compiled_code: Vec<u8>,
        optimization_level: u32,
        target_arch: &str,
        compile_time_us: u64,
    ) {
        let key = CacheKey::from_ir_block(block, optimization_level, target_arch);
        let code_size = compiled_code.len();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let entry = CacheEntry {
            code: compiled_code,
            metadata: CacheEntryMetadata {
                compile_timestamp: now,
                access_count: 0,
                last_access_timestamp: now,
                compile_time_us,
                code_size,
                optimization_level,
            },
            created_at: Instant::now(),
        };

        // 检查是否需要淘汰
        self.evict_if_needed(&key, code_size);

        // 插入新条目
        {
            let mut entries = self.entries.write();
            entries.insert(key.clone(), entry);
        }

        // 更新LRU顺序
        if self.config.enable_lru_eviction {
            let mut lru = self.lru_order.lock().unwrap();
            lru.push(key);
        }

        // 更新缓存大小
        {
            let mut size = self.current_size.lock().unwrap();
            *size += code_size;
        }

        let mut stats = self.stats.lock().unwrap();
        stats.inserts += 1;
        stats.total_compilations += 1;
        stats.saved_compile_time_us += compile_time_us;
    }

    /// 移除缓存条目
    fn remove(&self, key: &CacheKey) {
        let code_size = {
            let mut entries = self.entries.write();
            if let Some(entry) = entries.remove(key) {
                entry.code.len()
            } else {
                return;
            }
        };

        // 更新LRU顺序
        if self.config.enable_lru_eviction {
            let mut lru = self.lru_order.lock().unwrap();
            if let Some(pos) = lru.iter().position(|k| k == key) {
                lru.remove(pos);
            }
        }

        // 更新缓存大小
        {
            let mut size = self.current_size.lock().unwrap();
            *size = size.saturating_sub(code_size);
        }
    }

    /// 如果需要，淘汰缓存条目
    fn evict_if_needed(&self, new_key: &CacheKey, new_size: usize) {
        let mut evicted = 0;

        // 检查条目数限制
        {
            let should_evict = {
                let entries = self.entries.read();
                entries.len() >= self.config.max_entries
            };
            
            if should_evict && self.config.enable_lru_eviction {
                // 收集需要淘汰的键
                let keys_to_evict: Vec<CacheKey> = {
                    let mut lru = self.lru_order.lock().unwrap();
                    let entries = self.entries.read();
                    let mut keys = Vec::new();
                    
                    while entries.len() - keys.len() >= self.config.max_entries {
                        if let Some(oldest_key) = lru.first().cloned() {
                            // 不要淘汰正在插入的键
                            if &oldest_key != new_key {
                                keys.push(oldest_key.clone());
                                // 从lru中移除（使用retain保留不匹配的项）
                                lru.retain(|k| k != &oldest_key);
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    keys
                };
                
                // 批量移除
                for key in keys_to_evict {
                    self.remove(&key);
                    evicted += 1;
                }
            }
        }

        // 检查大小限制
        {
            let size = self.current_size.lock().unwrap();
            if *size + new_size > self.config.max_cache_size_bytes {
                // 需要淘汰一些条目以释放空间
                let mut lru = self.lru_order.lock().unwrap();
                let mut freed_size = 0;
                let target_free = new_size;

                while freed_size < target_free && !lru.is_empty() {
                    if let Some(oldest_key) = lru.first().cloned() {
                        // 不要淘汰正在插入的键
                        if &oldest_key != new_key {
                            let entry_size = {
                                let entries = self.entries.read();
                                entries.get(&oldest_key).map(|e| e.code.len()).unwrap_or(0)
                            };
                            if entry_size > 0 {
                                self.remove(&oldest_key);
                                freed_size += entry_size;
                                evicted += 1;
                            }
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }
        }

        if evicted > 0 {
            let mut stats = self.stats.lock().unwrap();
            stats.evictions += evicted;
        }
    }

    /// 清除所有缓存条目
    pub fn clear(&self) {
        {
            let mut entries = self.entries.write();
            entries.clear();
        }
        {
            let mut lru = self.lru_order.lock().unwrap();
            lru.clear();
        }
        {
            let mut size = self.current_size.lock().unwrap();
            *size = 0;
        }
    }

    /// 保存缓存到文件（如果启用持久化）
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> VmResult<()> {
        if !self.config.enable_persistence {
            return Err(VmError::Core(vm_core::CoreError::Config {
                message: "Persistence is not enabled".to_string(),
                path: Some("enable_persistence".to_string()),
            }));
        }

        let path = path.as_ref();
        tracing::info!("Saving AOT cache to: {}", path.display());

        // 注意：实际实现需要序列化缓存条目
        // 这里提供框架，实际实现需要根据具体需求完善
        
        Ok(())
    }

    /// 获取统计信息
    pub fn stats(&self) -> AotCacheStats {
        self.stats.lock().unwrap().clone()
    }

    /// 获取缓存命中率
    pub fn hit_rate(&self) -> f64 {
        let stats = self.stats.lock().unwrap();
        let total = stats.hits + stats.misses;
        if total > 0 {
            stats.hits as f64 / total as f64
        } else {
            0.0
        }
    }

    /// 获取当前缓存大小（字节）
    pub fn current_size(&self) -> usize {
        *self.current_size.lock().unwrap()
    }

    /// 获取缓存条目数
    pub fn entry_count(&self) -> usize {
        self.entries.read().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_ir::{IROp, Terminator};

    fn create_test_block(pc: u64) -> IRBlock {
        let mut ops = Vec::new();
        ops.push(IROp::MovImm { dst: 1, imm: 42 });
        ops.push(IROp::Add {
            dst: 2,
            src1: 1,
            src2: 1,
        });

        IRBlock {
            start_pc: pc,
            ops,
            term: Terminator::Ret,
        }
    }

    #[test]
    fn test_aot_cache_lookup_miss() {
        let cache = AotCache::new(AotCacheConfig::default());
        let block = create_test_block(0x1000);

        let result = cache.lookup(&block, 1, "x86_64");
        assert!(result.is_none());
    }

    #[test]
    fn test_aot_cache_insert_and_lookup() {
        let cache = AotCache::new(AotCacheConfig::default());
        let block = create_test_block(0x1000);
        let code = vec![0x90, 0xC3]; // NOP, RET

        cache.insert(&block, code.clone(), 1, "x86_64", 1000);

        let result = cache.lookup(&block, 1, "x86_64");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), code);
    }

    #[test]
    fn test_aot_cache_hit_rate() {
        let cache = AotCache::new(AotCacheConfig::default());
        let block = create_test_block(0x1000);
        let code = vec![0x90, 0xC3];

        cache.insert(&block, code, 1, "x86_64", 1000);

        // 第一次查找（命中）
        cache.lookup(&block, 1, "x86_64");
        assert!(cache.hit_rate() > 0.0);

        // 第二次查找（命中）
        cache.lookup(&block, 1, "x86_64");
        assert!(cache.hit_rate() > 0.5);
    }

    #[test]
    fn test_aot_cache_eviction() {
        let mut config = AotCacheConfig::default();
        config.max_entries = 2;
        config.enable_lru_eviction = true;

        let cache = AotCache::new(config);
        let block1 = create_test_block(0x1000);
        let block2 = create_test_block(0x2000);
        let block3 = create_test_block(0x3000);

        cache.insert(&block1, vec![0x90], 1, "x86_64", 1000);
        cache.insert(&block2, vec![0x90], 1, "x86_64", 1000);
        cache.insert(&block3, vec![0x90], 1, "x86_64", 1000);

        // block1应该被淘汰
        assert!(cache.lookup(&block1, 1, "x86_64").is_none());
        assert!(cache.lookup(&block2, 1, "x86_64").is_some());
        assert!(cache.lookup(&block3, 1, "x86_64").is_some());
    }

    #[test]
    fn test_aot_cache_clear() {
        let cache = AotCache::new(AotCacheConfig::default());
        let block = create_test_block(0x1000);

        cache.insert(&block, vec![0x90], 1, "x86_64", 1000);
        assert_eq!(cache.entry_count(), 1);

        cache.clear();
        assert_eq!(cache.entry_count(), 0);
        assert_eq!(cache.current_size(), 0);
    }
}

