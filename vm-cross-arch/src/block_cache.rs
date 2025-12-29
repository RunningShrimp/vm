//! 跨架构块级翻译缓存
//!
//! 实现高效的块级翻译缓存，减少重复翻译开销

use super::{ArchTranslator, SourceArch, TargetArch, TranslationResult};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use vm_core::GuestAddr;
use vm_ir::IRBlock;

/// 源块缓存键
///
/// 用于唯一标识一个源IR块
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceBlockKey {
    /// 源架构
    pub source_arch: SourceArch,
    /// 目标架构
    pub target_arch: TargetArch,
    /// 块起始地址
    pub start_pc: GuestAddr,
    /// 块哈希值（用于检测块内容变化）
    pub block_hash: u64,
}

impl SourceBlockKey {
    /// 创建新的源块键
    pub fn new(
        source_arch: SourceArch,
        target_arch: TargetArch,
        start_pc: GuestAddr,
        block: &IRBlock,
    ) -> Self {
        use std::hash::{Hash, Hasher};

        // 计算块的哈希值
        let mut hasher = std::collections::hash_map::DefaultHasher::new();

        // 哈希块的关键属性
        start_pc.hash(&mut hasher);
        block.ops.len().hash(&mut hasher);

        // 哈希每个操作
        for op in &block.ops {
            op.hash(&mut hasher);
        }

        // 哈希终结符
        block.term.hash(&mut hasher);

        let block_hash = hasher.finish();

        Self {
            source_arch,
            target_arch,
            start_pc,
            block_hash,
        }
    }
}

impl Hash for SourceBlockKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.source_arch.hash(state);
        self.target_arch.hash(state);
        self.start_pc.hash(state);
        self.block_hash.hash(state);
    }
}

/// 缓存替换策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheReplacementPolicy {
    /// 最近最少使用（LRU）
    Lru,
    /// 先进先出（FIFO）
    Fifo,
    /// 最不经常使用（LFU）
    Lfu,
    /// 随机替换
    Random,
}

/// 缓存统计信息
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// 缓存命中次数
    pub hits: u64,
    /// 缓存未命中次数
    pub misses: u64,
    /// 缓存替换次数
    pub evictions: u64,
    /// 当前缓存大小
    pub current_size: usize,
    /// 最大缓存大小
    pub max_size: usize,
}

impl CacheStats {
    /// 计算缓存命中率
    pub fn hit_rate(&self) -> f64 {
        if self.hits + self.misses == 0 {
            return 0.0;
        }
        self.hits as f64 / (self.hits + self.misses) as f64
    }
}

/// 翻译块条目
#[derive(Debug, Clone)]
pub struct TranslatedBlock {
    /// 翻译后的指令序列
    pub instructions: Vec<super::TargetInstruction>,
    /// 翻译统计信息
    pub stats: super::TranslationStats,
    /// 创建时间戳
    pub created_at: std::time::Instant,
    /// 最后访问时间戳
    pub last_accessed: std::time::Instant,
    /// 访问次数
    pub access_count: u64,
}

impl TranslatedBlock {
    /// 创建新的翻译块
    pub fn new(
        instructions: Vec<super::TargetInstruction>,
        stats: super::TranslationStats,
    ) -> Self {
        let now = std::time::Instant::now();
        Self {
            instructions,
            stats,
            created_at: now,
            last_accessed: now,
            access_count: 0,
        }
    }

    /// 标记为已访问
    pub fn mark_accessed(&mut self) {
        self.last_accessed = std::time::Instant::now();
        self.access_count += 1;
    }
}

/// 跨架构块级翻译缓存
///
/// 缓存已翻译的代码块，避免重复翻译
pub struct CrossArchBlockCache {
    /// 缓存存储
    cache: HashMap<SourceBlockKey, TranslatedBlock>,
    /// LRU访问顺序（用于LRU策略）
    lru_order: Vec<SourceBlockKey>,
    /// FIFO队列（用于FIFO策略）
    fifo_queue: Vec<SourceBlockKey>,
    /// 访问计数（用于LFU策略）
    access_counts: HashMap<SourceBlockKey, u64>,
    /// 缓存替换策略
    policy: CacheReplacementPolicy,
    /// 最大缓存大小
    max_size: usize,
    /// 统计信息
    stats: CacheStats,
}

impl CrossArchBlockCache {
    /// 创建新的块级翻译缓存
    pub fn new(max_size: usize, policy: CacheReplacementPolicy) -> Self {
        Self {
            cache: HashMap::new(),
            lru_order: Vec::new(),
            fifo_queue: Vec::new(),
            access_counts: HashMap::new(),
            policy,
            max_size,
            stats: CacheStats {
                max_size,
                ..Default::default()
            },
        }
    }

    /// 查找缓存中的翻译块
    pub fn lookup(&mut self, key: &SourceBlockKey) -> Option<&TranslatedBlock> {
        if self.cache.contains_key(key) {
            // 更新访问信息
            self.update_access_info(key);
            self.stats.hits += 1;

            // 返回不可变引用
            self.cache.get(key)
        } else {
            self.stats.misses += 1;
            None
        }
    }

    /// 添加翻译块到缓存
    pub fn insert(&mut self, key: SourceBlockKey, block: TranslatedBlock) {
        // 如果缓存已满，需要替换
        if self.cache.len() >= self.max_size {
            self.evict_block();
        }

        // 插入新块
        self.cache.insert(key.clone(), block);
        self.initialize_access_info(&key);
        self.stats.current_size = self.cache.len();
    }

    /// 获取或翻译块
    ///
    /// 如果缓存中存在，则返回缓存的结果；否则使用提供的翻译器翻译并缓存
    pub fn get_or_translate(
        &mut self,
        translator: &mut ArchTranslator,
        block: &IRBlock,
    ) -> Result<TranslationResult, super::TranslationError> {
        // 创建缓存键
        let source_arch = SourceArch::try_from(translator.source_arch())
            .map_err(|_| super::TranslationError::UnsupportedArchitecturePair)?;
        let target_arch = TargetArch::try_from(translator.target_arch())
            .map_err(|_| super::TranslationError::UnsupportedArchitecturePair)?;

        let key = SourceBlockKey::new(source_arch, target_arch, block.start_pc, block);

        // 尝试从缓存获取
        if let Some(cached_block) = self.lookup(&key) {
            return Ok(TranslationResult {
                instructions: cached_block.instructions.clone(),
                stats: cached_block.stats.clone(),
            });
        }

        // 缓存未命中，翻译块
        let result = translator.translate_block_internal(block)?;

        // 创建翻译块并缓存
        let translated_block =
            TranslatedBlock::new(result.instructions.clone(), result.stats.clone());
        self.insert(key, translated_block);

        Ok(result)
    }

    /// 清空缓存
    pub fn clear(&mut self) {
        self.cache.clear();
        self.lru_order.clear();
        self.fifo_queue.clear();
        self.access_counts.clear();
        self.stats.current_size = 0;
    }

    /// 获取缓存统计信息
    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    /// 设置最大缓存大小
    pub fn set_max_size(&mut self, max_size: usize) {
        self.max_size = max_size;
        self.stats.max_size = max_size;

        // 如果当前缓存大小超过新的最大值，进行替换
        while self.cache.len() > max_size {
            self.evict_block();
        }
    }

    /// 替换一个块
    fn evict_block(&mut self) {
        if self.cache.is_empty() {
            return;
        }

        let key_to_evict = match self.policy {
            CacheReplacementPolicy::Lru => {
                // 找到最久未访问的块
                self.lru_order.first().cloned()
            }
            CacheReplacementPolicy::Fifo => {
                // 找到最早插入的块
                self.fifo_queue.first().cloned()
            }
            CacheReplacementPolicy::Lfu => {
                // 找到访问次数最少的块
                self.access_counts
                    .iter()
                    .min_by_key(|&(_, count)| count)
                    .map(|(key, _)| key.clone())
            }
            CacheReplacementPolicy::Random => {
                // 随机选择一个块
                let keys: Vec<_> = self.cache.keys().cloned().collect();
                if !keys.is_empty() {
                    let index = fastrand::usize(..keys.len());
                    Some(keys[index].clone())
                } else {
                    None
                }
            }
        };

        if let Some(key) = key_to_evict {
            self.cache.remove(&key);
            self.remove_access_info(&key);
            self.stats.evictions += 1;
            self.stats.current_size = self.cache.len();
        }
    }

    /// 初始化访问信息
    fn initialize_access_info(&mut self, key: &SourceBlockKey) {
        match self.policy {
            CacheReplacementPolicy::Lru => {
                self.lru_order.push(key.clone());
            }
            CacheReplacementPolicy::Fifo => {
                self.fifo_queue.push(key.clone());
            }
            CacheReplacementPolicy::Lfu => {
                self.access_counts.insert(key.clone(), 1);
            }
            CacheReplacementPolicy::Random => {
                // 随机策略不需要额外信息
            }
        }
    }

    /// 更新访问信息
    fn update_access_info(&mut self, key: &SourceBlockKey) {
        match self.policy {
            CacheReplacementPolicy::Lru => {
                // 移动到LRU列表末尾（最近使用）
                if let Some(pos) = self.lru_order.iter().position(|k| k == key) {
                    self.lru_order.remove(pos);
                    self.lru_order.push(key.clone());
                }
            }
            CacheReplacementPolicy::Lfu => {
                // 增加访问计数
                *self.access_counts.entry(key.clone()).or_insert(0) += 1;
            }
            CacheReplacementPolicy::Fifo | CacheReplacementPolicy::Random => {
                // FIFO和随机策略不需要更新访问信息
            }
        }

        // 更新缓存中的块访问信息
        if let Some(block) = self.cache.get_mut(key) {
            block.mark_accessed();
        }
    }

    /// 移除访问信息
    fn remove_access_info(&mut self, key: &SourceBlockKey) {
        match self.policy {
            CacheReplacementPolicy::Lru => {
                self.lru_order.retain(|k| k != key);
            }
            CacheReplacementPolicy::Fifo => {
                self.fifo_queue.retain(|k| k != key);
            }
            CacheReplacementPolicy::Lfu => {
                self.access_counts.remove(key);
            }
            CacheReplacementPolicy::Random => {
                // 随机策略不需要额外信息
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_ir::{IRBuilder, IROp};

    #[test]
    fn test_cache_key_creation() {
        let mut builder = IRBuilder::new(0x1000);
        builder.push(IROp::Add {
            dst: 0,
            src1: 1,
            src2: 2,
        });
        let block = builder.build();

        let key1 = SourceBlockKey::new(SourceArch::X86_64, TargetArch::ARM64, 0x1000, &block);
        let key2 = SourceBlockKey::new(SourceArch::X86_64, TargetArch::ARM64, 0x1000, &block);
        let key3 = SourceBlockKey::new(SourceArch::X86_64, TargetArch::RISCV64, 0x1000, &block);

        assert_eq!(key1, key2); // 相同的块应该有相同的键
        assert_ne!(key1, key3); // 不同的目标架构应该有不同的键
    }

    #[test]
    fn test_cache_lru_policy() {
        let mut cache = CrossArchBlockCache::new(2, CacheReplacementPolicy::Lru);

        // 创建测试块
        let mut builder1 = IRBuilder::new(0x1000);
        builder1.push(IROp::Add {
            dst: 0,
            src1: 1,
            src2: 2,
        });
        let block1 = builder1.build();

        let mut builder2 = IRBuilder::new(0x2000);
        builder2.push(IROp::Sub {
            dst: 0,
            src1: 1,
            src2: 2,
        });
        let block2 = builder2.build();

        let mut builder3 = IRBuilder::new(0x3000);
        builder3.push(IROp::Mul {
            dst: 0,
            src1: 1,
            src2: 2,
        });
        let block3 = builder3.build();

        // 插入两个块
        let key1 = SourceBlockKey::new(SourceArch::X86_64, TargetArch::ARM64, 0x1000, &block1);
        let key2 = SourceBlockKey::new(SourceArch::X86_64, TargetArch::ARM64, 0x2000, &block2);

        let translated_block1 = TranslatedBlock::new(vec![], Default::default());
        let translated_block2 = TranslatedBlock::new(vec![], Default::default());

        cache.insert(key1.clone(), translated_block1);
        cache.insert(key2.clone(), translated_block2);

        // 访问第一个块（使其成为最近使用）
        cache.lookup(&key1);

        // 插入第三个块，应该替换最久未使用的块（key2）
        let key3 = SourceBlockKey::new(SourceArch::X86_64, TargetArch::ARM64, 0x3000, &block3);
        let translated_block3 = TranslatedBlock::new(vec![], Default::default());
        cache.insert(key3, translated_block3);

        // 验证key1仍在缓存中，key2被替换
        assert!(cache.lookup(&key1).is_some());
        assert!(cache.lookup(&key2).is_none());
        assert!(cache.lookup(&key3).is_some());
    }

    #[test]
    fn test_cache_stats() {
        let mut cache = CrossArchBlockCache::new(10, CacheReplacementPolicy::Lru);

        // 初始统计
        assert_eq!(cache.stats().hits, 0);
        assert_eq!(cache.stats().misses, 0);
        assert_eq!(cache.stats().hit_rate(), 0.0);

        // 创建测试块
        let mut builder = IRBuilder::new(0x1000);
        builder.push(IROp::Add {
            dst: 0,
            src1: 1,
            src2: 2,
        });
        let block = builder.build();

        let key = SourceBlockKey::new(SourceArch::X86_64, TargetArch::ARM64, 0x1000, &block);

        // 未命中
        assert!(cache.lookup(&key).is_none());
        assert_eq!(cache.stats().misses, 1);

        // 插入块
        let translated_block = TranslatedBlock::new(vec![], Default::default());
        cache.insert(key.clone(), translated_block);

        // 命中
        assert!(cache.lookup(&key).is_some());
        assert_eq!(cache.stats().hits, 1);

        // 验证命中率
        assert_eq!(cache.stats().hit_rate(), 0.5); // 1 hit / (1 hit + 1 miss)
    }
}
