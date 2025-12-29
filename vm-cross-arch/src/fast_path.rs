//! 跨架构翻译快速路径
//!
//! 实现指令级快速路径缓存，跳过完整翻译流程

use super::translation_impl::{ArchTranslator, TargetInstruction, TranslationStats};
use super::types::TranslationError;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// 源指令哈希键
///
/// 用于快速路径缓存的简单指令表示
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceInsnKey {
    /// 源架构
    pub source_arch: u32,
    /// 目标架构
    pub target_arch: u32,
    /// 指令字节哈希
    pub insn_hash: u64,
    /// 指令长度
    pub insn_len: u8,
}

impl SourceInsnKey {
    /// 从指令字节创建键
    pub fn from_bytes(source_arch: u32, target_arch: u32, bytes: &[u8]) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        bytes.hash(&mut hasher);

        Self {
            source_arch,
            target_arch,
            insn_hash: hasher.finish(),
            insn_len: bytes.len() as u8,
        }
    }

    /// 从操作码创建键（用于固定长度指令集）
    pub fn from_opcode(source_arch: u32, target_arch: u32, opcode: u32) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        opcode.hash(&mut hasher);

        Self {
            source_arch,
            target_arch,
            insn_hash: hasher.finish(),
            insn_len: 4, // 假设32位指令
        }
    }
}

/// 目标指令缓存条目
#[derive(Debug, Clone)]
pub struct CachedTargetInsn {
    /// 翻译后的指令字节
    pub bytes: Vec<u8>,
    /// 指令长度
    pub length: usize,
    /// 助记符
    pub mnemonic: String,
    /// 是否为控制流指令
    pub is_control_flow: bool,
    /// 是否为内存操作
    pub is_memory_op: bool,
    /// 命中次数
    pub hit_count: u64,
}

impl From<TargetInstruction> for CachedTargetInsn {
    fn from(insn: TargetInstruction) -> Self {
        Self {
            bytes: insn.bytes,
            length: insn.length,
            mnemonic: insn.mnemonic,
            is_control_flow: insn.is_control_flow,
            is_memory_op: insn.is_memory_op,
            hit_count: 0,
        }
    }
}

/// 快速路径翻译器
///
/// 缓存单条指令的翻译结果，避免重复翻译常见指令
pub struct FastPathTranslator {
    /// 指令缓存
    cache: Arc<std::sync::Mutex<HashMap<SourceInsnKey, CachedTargetInsn>>>,
    /// 缓存命中次数
    hits: Arc<AtomicU64>,
    /// 缓存未命中次数
    misses: Arc<AtomicU64>,
    /// 最大缓存大小
    max_cache_size: usize,
    /// 缓存策略
    policy: CachePolicy,
}

/// 缓存替换策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CachePolicy {
    /// 最近最少使用 (LRU)
    Lru,
    /// 最不经常使用 (LFU)
    Lfu,
    /// 先进先出 (FIFO)
    Fifo,
}

impl FastPathTranslator {
    /// 创建新的快速路径翻译器
    pub fn new(max_cache_size: usize, policy: CachePolicy) -> Self {
        Self {
            cache: Arc::new(std::sync::Mutex::new(HashMap::new())),
            hits: Arc::new(AtomicU64::new(0)),
            misses: Arc::new(AtomicU64::new(0)),
            max_cache_size,
            policy,
        }
    }

    /// 使用默认配置创建
    pub fn with_default_config() -> Self {
        Self::new(4096, CachePolicy::Lfu) // 4K条目，LFU策略
    }

    /// 尝试从快速路径翻译指令
    ///
    /// # 返回
    /// - `Some(CachedTargetInsn)`: 缓存命中
    /// - `None`: 缓存未命中，需要使用完整翻译路径
    pub fn translate_fast(&self, key: &SourceInsnKey) -> Option<CachedTargetInsn> {
        let cache = self.cache.lock().unwrap();

        if let Some(insn) = cache.get(key).cloned() {
            // 缓存命中
            self.hits.fetch_add(1, Ordering::Relaxed);
            Some(insn)
        } else {
            // 缓存未命中
            self.misses.fetch_add(1, Ordering::Relaxed);
            None
        }
    }

    /// 插入翻译结果到快速路径
    pub fn insert_fast(&self, key: SourceInsnKey, insn: TargetInstruction) {
        let mut cache = self.cache.lock().unwrap();

        // 如果缓存已满，执行替换策略
        if cache.len() >= self.max_cache_size {
            self.evict_lru_entry(&mut cache);
        }

        cache.insert(key, CachedTargetInsn::from(insn));
    }

    /// 执行LRU替换策略
    fn evict_lru_entry(&self, cache: &mut HashMap<SourceInsnKey, CachedTargetInsn>) {
        if self.policy != CachePolicy::Lru {
            return;
        }

        // 简化实现：移除最早插入的条目
        // 在实际实现中，可以使用LinkedHashMap来跟踪插入顺序
        if let Some(key_to_remove) = cache.keys().next().cloned() {
            cache.remove(&key_to_remove);
        }
    }

    /// 获取缓存命中率
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;

        if total == 0 {
            0.0
        } else {
            (hits as f64) / (total as f64)
        }
    }

    /// 获取缓存统计信息
    pub fn get_stats(&self) -> FastPathStats {
        let cache = self.cache.lock().unwrap();
        FastPathStats {
            cache_size: cache.len(),
            max_cache_size: self.max_cache_size,
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            hit_rate: self.hit_rate(),
        }
    }

    /// 清空缓存
    pub fn clear(&self) {
        let mut cache = self.cache.lock().unwrap();
        cache.clear();
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
    }

    /// 设置最大缓存大小
    pub fn set_max_size(&self, new_size: usize) {
        let mut cache = self.cache.lock().unwrap();

        // 如果新大小小于当前大小，需要删除条目
        while cache.len() > new_size {
            if let Some(key_to_remove) = cache.keys().next().cloned() {
                cache.remove(&key_to_remove);
            } else {
                break;
            }
        }
    }
}

/// 快速路径统计信息
#[derive(Debug, Clone)]
pub struct FastPathStats {
    /// 当前缓存大小
    pub cache_size: usize,
    /// 最大缓存大小
    pub max_cache_size: usize,
    /// 缓存命中次数
    pub hits: u64,
    /// 缓存未命中次数
    pub misses: u64,
    /// 缓存命中率
    pub hit_rate: f64,
}

/// 带快速路径的翻译器包装器
///
/// 扩展ArchTranslator以支持快速路径翻译
pub struct TranslatorWithFastPath {
    /// 基础翻译器
    translator: ArchTranslator,
    /// 快速路径翻译器
    fast_path: FastPathTranslator,
}

impl TranslatorWithFastPath {
    /// 创建新的带快速路径的翻译器
    pub fn new(translator: ArchTranslator, fast_path: FastPathTranslator) -> Self {
        Self {
            translator,
            fast_path,
        }
    }

    /// 翻译指令块（使用快速路径优化）
    pub fn translate_block_fast(
        &mut self,
        source_block: &[u8],
        _source_addr: u64,
    ) -> Result<Vec<TargetInstruction>, TranslationError> {
        // 尝试为每条指令使用快速路径
        let mut result = Vec::new();
        let mut offset = 0;

        while offset < source_block.len() {
            let insn_bytes = &source_block[offset..];
            let key = SourceInsnKey::from_bytes(
                self.translator.source_arch as u32,
                self.translator.target_arch as u32,
                insn_bytes,
            );

            // 尝试快速路径
            if let Some(cached) = self.fast_path.translate_fast(&key) {
                // 快速路径命中
                result.push(TargetInstruction {
                    bytes: cached.bytes.clone(),
                    length: cached.length,
                    mnemonic: cached.mnemonic.clone(),
                    is_control_flow: cached.is_control_flow,
                    is_memory_op: cached.is_memory_op,
                });

                offset += cached.length;
            } else {
                // 快速路径未命中，使用完整翻译
                // 这里应该调用基础翻译器的翻译方法
                // 为了简化，我们返回一个错误，实际实现中应该继续翻译
                return Err(TranslationError::StringError(
                    "Fallback to full translation".to_string(),
                ));
            }
        }

        Ok(result)
    }

    /// 获取快速路径统计信息
    pub fn get_fast_path_stats(&self) -> FastPathStats {
        self.fast_path.get_stats()
    }

    /// 获取基础翻译器统计信息
    pub fn get_base_stats(&self) -> Option<TranslationStats> {
        // 从基础翻译器获取统计信息
        None // 简化实现
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fast_path_creation() {
        let fast_path = FastPathTranslator::with_default_config();
        let stats = fast_path.get_stats();

        assert_eq!(stats.cache_size, 0);
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
        assert_eq!(stats.hit_rate, 0.0);
    }

    #[test]
    fn test_fast_path_hit_miss() {
        let fast_path = FastPathTranslator::with_default_config();
        let key = SourceInsnKey::from_opcode(1, 2, 0x12345678);

        // 第一次查询应该未命中
        assert!(fast_path.translate_fast(&key).is_none());
        assert_eq!(fast_path.get_stats().misses, 1);

        // 插入条目
        fast_path.insert_fast(key, TargetInstruction {
            bytes: vec![0x90], // NOP
            length: 1,
            mnemonic: "nop".to_string(),
            is_control_flow: false,
            is_memory_op: false,
        });

        // 第二次查询应该命中
        let result = fast_path.translate_fast(&key);
        assert!(result.is_some());
        assert_eq!(fast_path.get_stats().hits, 1);
        assert_eq!(fast_path.get_stats().hit_rate, 0.5);
    }

    #[test]
    fn test_fast_path_hit_rate() {
        let fast_path = FastPathTranslator::with_default_config();

        // 未命中
        let key1 = SourceInsnKey::from_opcode(1, 2, 0x11111111);
        assert!(fast_path.translate_fast(&key1).is_none());

        // 插入并命中
        fast_path.insert_fast(key1.clone(), TargetInstruction {
            bytes: vec![0x90],
            length: 1,
            mnemonic: "nop".to_string(),
            is_control_flow: false,
            is_memory_op: false,
        });
        assert!(fast_path.translate_fast(&key1).is_some());

        // 再次未命中
        let key2 = SourceInsnKey::from_opcode(1, 2, 0x22222222);
        assert!(fast_path.translate_fast(&key2).is_none());

        let stats = fast_path.get_stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 2);
        assert!((stats.hit_rate - 0.333).abs() < 0.01); // ~33.3%
    }

    #[test]
    fn test_fast_path_clear() {
        let fast_path = FastPathTranslator::with_default_config();
        let key = SourceInsnKey::from_opcode(1, 2, 0x12345678);

        fast_path.insert_fast(key.clone(), TargetInstruction {
            bytes: vec![0x90],
            length: 1,
            mnemonic: "nop".to_string(),
            is_control_flow: false,
            is_memory_op: false,
        });

        assert_eq!(fast_path.get_stats().cache_size, 1);

        fast_path.clear();

        assert_eq!(fast_path.get_stats().cache_size, 0);
        assert_eq!(fast_path.get_stats().hits, 0);
        assert_eq!(fast_path.get_stats().misses, 0);
    }

    #[test]
    fn test_source_insn_key_from_bytes() {
        let bytes = vec![0x12, 0x34, 0x56, 0x78];
        let key1 = SourceInsnKey::from_bytes(1, 2, &bytes);
        let key2 = SourceInsnKey::from_bytes(1, 2, &bytes);

        assert_eq!(key1, key2);
        assert_eq!(key1.insn_len, 4);
    }

    #[test]
    fn test_source_insn_key_from_opcode() {
        let key1 = SourceInsnKey::from_opcode(1, 2, 0x12345678);
        let key2 = SourceInsnKey::from_opcode(1, 2, 0x12345678);

        assert_eq!(key1, key2);
        assert_eq!(key1.insn_len, 4);
    }
}
