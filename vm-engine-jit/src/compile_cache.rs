//! 编译缓存系统
//!
//! 提供热点代码块的编译结果缓存，重用机制提高性能

use crate::ExecutableBlock;
use crate::compiler_backend::{CompilerBackend, CompilerError};
use lru::LruCache;
use parking_lot::Mutex;
use std::num::NonZeroUsize;
use std::sync::Arc;
use vm_core::GuestAddr;
use vm_ir::IRBlock;

/// 使用 GuestAddr 作为缓存键（与 runtime 调用一致）
pub type IRHash = vm_core::GuestAddr;

/// 编译缓存条目
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// 编译后的可执行块
    pub code_block: ExecutableBlock,
    /// 缓存命中次数
    pub hit_count: u64,
    /// 最后访问时间
    pub last_access: std::time::Instant,
    /// 代码块大小
    pub block_size: usize,
}

/// 编译缓存统计信息
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// 总缓存命中次数
    pub total_hits: u64,
    /// 总缓存未命中次数
    pub total_misses: u64,
    /// 当前缓存条目数量
    pub entries_count: usize,
    /// 缓存大小（字节）
    pub total_size: usize,
    /// 平均命中率
    pub hit_rate: f64,
}

/// 编译缓存
pub struct CompileCache {
    /// 缓存存储，使用LRU策略
    cache: Arc<Mutex<LruCache<IRHash, CacheEntry>>>,
    /// 缓存统计信息
    stats: Arc<Mutex<CacheStats>>,
    /// 最大缓存大小（字节）
    max_size: usize,
    /// 最大条目数量
    ///
    /// 当前用于初始化 LRU 缓存，未来可能用于缓存统计和动态调整。
    #[allow(dead_code)] // Used in initialization, may be used for statistics in future
    max_entries: usize,
}

impl CompileCache {
    /// 创建新的编译缓存
    pub fn new(max_entries: usize, max_size: usize) -> Self {
        Self {
            cache: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(max_entries).unwrap(),
            ))),
            stats: Arc::new(Mutex::new(CacheStats::default())),
            max_size,
            max_entries,
        }
    }

    /// 查找缓存的编译结果
    pub fn get(&self, hash: &IRHash) -> Option<ExecutableBlock> {
        let mut cache = self.cache.lock();
        if let Some(entry) = cache.get_mut(hash) {
            // 更新统计信息
            entry.hit_count += 1;
            entry.last_access = std::time::Instant::now();

            let mut stats = self.stats.lock();
            stats.total_hits += 1;

            Some(entry.code_block.clone())
        } else {
            let mut stats = self.stats.lock();
            stats.total_misses += 1;
            None
        }
    }

    /// 将编译结果放入缓存
    pub fn put(&self, hash: IRHash, code_block: ExecutableBlock) -> bool {
        let code_size = code_block.code.len();
        let entry = CacheEntry {
            code_block: code_block.clone(),
            hit_count: 0,
            last_access: std::time::Instant::now(),
            block_size: code_size,
        };

        let mut cache = self.cache.lock();
        let mut stats = self.stats.lock();

        // 检查是否超过最大大小限制
        let current_size = stats.total_size;
        if current_size + code_size > self.max_size {
            // 如果超过限制，不缓存
            return false;
        }

        // 添加到缓存
        if let Some(old_entry) = cache.put(hash, entry) {
            stats.total_size -= old_entry.block_size;
        }

        stats.total_size += code_size;
        stats.entries_count = cache.len();

        true
    }

    /// 检查代码块是否已缓存
    pub fn contains(&self, addr: GuestAddr) -> bool {
        self.cache.lock().contains(&addr)
    }

    /// 获取或编译代码块（使用缓存）
    pub fn get_or_compile<B: CompilerBackend + Default>(
        &self,
        block: &IRBlock,
    ) -> Result<crate::ExecutableBlock, CompilerError> {
        // 使用块的起始地址作为缓存键
        let cache_key = block.start_pc;

        // 首先尝试从缓存获取
        if let Some(cached_code) = self.get(&cache_key) {
            return Ok(cached_code);
        }

        // 缓存未命中，进行编译
        let mut backend = B::default();
        let code = backend.compile(block)?;

        // 将编译结果放入缓存
        self.put(cache_key, code.clone());

        Ok(code)
    }

    /// 清除缓存
    pub fn clear(&self) {
        let mut cache = self.cache.lock();
        let mut stats = self.stats.lock();

        cache.clear();
        *stats = CacheStats::default();
    }

    /// 获取缓存统计信息
    pub fn stats(&self) -> CacheStats {
        let mut stats = self.stats.lock().clone();
        let cache = self.cache.lock();

        stats.entries_count = cache.len();
        stats.hit_rate = if stats.total_hits + stats.total_misses > 0 {
            stats.total_hits as f64 / (stats.total_hits + stats.total_misses) as f64
        } else {
            0.0
        };

        stats
    }

    /// 根据访问频率清理冷数据
    pub fn evict_cold_entries(&self, min_age_seconds: u64) {
        let mut cache = self.cache.lock();
        let mut stats = self.stats.lock();

        let now = std::time::Instant::now();
        let mut to_remove = Vec::new();

        // 收集需要移除的条目
        for (hash, entry) in cache.iter() {
            if now.duration_since(entry.last_access).as_secs() > min_age_seconds
                && entry.hit_count == 0
            {
                to_remove.push(*hash);
            }
        }

        // 移除冷数据
        for hash in to_remove {
            if let Some(entry) = cache.pop(&hash) {
                stats.total_size -= entry.block_size;
            }
        }

        stats.entries_count = cache.len();
    }

    /// 压缩缓存大小
    pub fn shrink_to_fit(&self, target_size: usize) {
        let mut cache = self.cache.lock();
        let mut stats = self.stats.lock();

        while stats.total_size > target_size && !cache.is_empty() {
            if let Some((_, entry)) = cache.pop_lru() {
                stats.total_size -= entry.block_size;
            }
        }

        stats.entries_count = cache.len();
    }
}

impl Default for CompileCache {
    /// 创建具有默认大小的编译缓存
    fn default() -> Self {
        Self::new(1000, 10 * 1024 * 1024) // 1000个条目，10MB
    }
}

/// 智能编译缓存管理器
pub struct SmartCompileCache {
    /// 基础缓存
    cache: CompileCache,
    /// 热点阈值（命中次数）
    hot_threshold: u64,
    /// 冷数据清理间隔（秒）
    cleanup_interval: u64,
    /// 上次清理时间
    last_cleanup: std::time::Instant,
}

impl SmartCompileCache {
    /// 创建智能编译缓存
    pub fn new(max_entries: usize, max_size: usize, hot_threshold: u64) -> Self {
        Self {
            cache: CompileCache::new(max_entries, max_size),
            hot_threshold,
            cleanup_interval: 300, // 5分钟
            last_cleanup: std::time::Instant::now(),
        }
    }

    /// 获取或编译代码块（带智能缓存管理）
    pub fn get_or_compile_smart<B: CompilerBackend + Default>(
        &mut self,
        block: &IRBlock,
    ) -> Result<crate::ExecutableBlock, CompilerError> {
        // 定期清理冷数据
        let now = std::time::Instant::now();
        if now.duration_since(self.last_cleanup).as_secs() > self.cleanup_interval {
            self.cache.evict_cold_entries(600); // 清理10分钟未访问的冷数据
            self.last_cleanup = now;
        }

        // 检查缓存大小，如果过大则压缩
        let stats = self.cache.stats();
        if stats.total_size > self.cache.max_size * 9 / 10 {
            // 超过90%
            self.cache.shrink_to_fit(self.cache.max_size * 8 / 10); // 压缩到80%
        }

        self.cache.get_or_compile::<B>(block)
    }

    /// 获取缓存统计信息
    pub fn stats(&self) -> CacheStats {
        self.cache.stats()
    }

    /// 获取热点代码块
    pub fn get_hot_blocks(&self) -> Vec<(IRHash, u64)> {
        let cache = self.cache.cache.lock();
        let mut hot_blocks = Vec::new();

        for (hash, entry) in cache.iter() {
            if entry.hit_count >= self.hot_threshold {
                hot_blocks.push((*hash, entry.hit_count));
            }
        }

        hot_blocks.sort_by(|a, b| b.1.cmp(&a.1)); // 按命中次数降序排序
        hot_blocks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_ir::IRBuilder;

    fn create_test_block(pc: u64, value: u64) -> IRBlock {
        let mut builder = IRBuilder::new(vm_core::GuestAddr(pc));
        builder.push(vm_ir::IROp::MovImm { dst: 0, imm: value });
        builder.set_term(vm_ir::Terminator::Ret);
        builder.build()
    }

    #[test]
    fn test_compile_cache() {
        let cache = CompileCache::new(10, 1024 * 1024);

        // 创建测试代码块
        let block = create_test_block(0x1000, 42);
        let hash = block.start_pc;

        // 第一次编译（缓存未命中）
        let result1 = cache.get_or_compile::<crate::direct_backend::DirectBackend>(&block);
        assert!(result1.is_ok());

        // 第二次获取（应该从缓存命中）
        let result2 = cache.get(&hash);
        assert!(result2.is_some());
        assert_eq!(result2.unwrap().code, result1.unwrap().code);

        // 检查统计信息
        let stats = cache.stats();
        assert_eq!(stats.total_hits, 1);
        assert_eq!(stats.total_misses, 1);
        assert_eq!(stats.hit_rate, 0.5);
    }

    #[test]
    fn test_cache_eviction() {
        let cache = CompileCache::new(2, 100); // 只允许2个条目

        let block1 = create_test_block(0x1000, 1);
        let block2 = create_test_block(0x2000, 2);
        let block3 = create_test_block(0x3000, 3);

        // 添加三个块，第三个应该导致第一个被驱逐
        cache
            .get_or_compile::<crate::direct_backend::DirectBackend>(&block1)
            .unwrap();
        cache
            .get_or_compile::<crate::direct_backend::DirectBackend>(&block2)
            .unwrap();
        cache
            .get_or_compile::<crate::direct_backend::DirectBackend>(&block3)
            .unwrap();

        let stats = cache.stats();
        assert_eq!(stats.entries_count, 2); // 应该只剩下2个条目
    }

    #[test]
    fn test_smart_cache() {
        let mut smart_cache = SmartCompileCache::new(10, 1024 * 1024, 5);

        let block = create_test_block(0x1000, 42);

        // 多次编译同一个块
        for _ in 0..10 {
            smart_cache
                .get_or_compile_smart::<crate::direct_backend::DirectBackend>(&block)
                .unwrap();
        }

        let stats = smart_cache.stats();
        assert_eq!(stats.total_hits, 9); // 第一次是miss，后9次是hit
        assert_eq!(stats.total_misses, 1);

        // 检查热点块
        let hot_blocks = smart_cache.get_hot_blocks();
        assert!(!hot_blocks.is_empty());
    }
}
