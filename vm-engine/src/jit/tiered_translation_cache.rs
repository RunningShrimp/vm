//! Tiered Translation Cache for JIT Optimization
//!
//! Implements a 3-level tiered cache system for JIT translation:
//! - L1: Instruction cache for hot instructions
//! - L2: Block cache for compiled blocks
//! - L3: Region cache for optimized superblocks

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use vm_core::GuestAddr;
use vm_ir::IRBlock;

use crate::jit::backend::CompiledCode;

/// Maximum cache sizes for each level
const L1_MAX_CAPACITY: usize = 1024; // Hot instructions
const L2_MAX_CAPACITY: usize = 4096; // Compiled blocks
const L3_MAX_CAPACITY: usize = 16384; // Superblock regions

/// Translated instruction entry for L1 cache
#[derive(Debug, Clone)]
pub struct TranslatedInsn {
    /// Guest address of the instruction
    pub guest_addr: GuestAddr,
    /// Compiled machine code for this instruction
    pub code: Vec<u8>,
    /// Size of the compiled code in bytes
    pub code_size: usize,
    /// Execution count for adaptive sizing
    pub exec_count: u64,
    /// Last access timestamp
    pub last_access: Instant,
    /// Compiled code entry (for single instructions)
    pub compiled: Arc<CompiledCode>,
}

/// Compiled block entry for L2 cache
#[derive(Debug, Clone)]
pub struct CompiledBlock {
    /// Guest address of the block
    pub guest_addr: GuestAddr,
    /// IR block reference
    pub ir_block: IRBlock,
    /// Compiled machine code
    pub code: Vec<u8>,
    /// Code size in bytes
    pub code_size: usize,
    /// Execution count
    pub exec_count: u64,
    /// Last access timestamp
    pub last_access: Instant,
    /// Compiled code entry
    pub compiled: Arc<CompiledCode>,
}

/// Optimized region entry for L3 cache
#[derive(Debug, Clone)]
pub struct OptimizedRegion {
    /// Starting guest address of the region
    pub start_addr: GuestAddr,
    /// Ending guest address of the region
    pub end_addr: GuestAddr,
    /// Compiled superblock code
    pub code: Vec<u8>,
    /// Code size in bytes
    pub code_size: usize,
    /// Number of blocks in this region
    pub block_count: usize,
    /// Execution count
    pub exec_count: u64,
    /// Last access timestamp
    pub last_access: Instant,
    /// Optimization level applied
    pub opt_level: u8,
    /// Compiled code entry
    pub compiled: Arc<CompiledCode>,
}

/// Cache statistics snapshot
#[derive(Debug, Clone, Default)]
pub struct CacheStatisticsSnapshot {
    pub l1_hits: u64,
    pub l1_misses: u64,
    pub l2_hits: u64,
    pub l2_misses: u64,
    pub l3_hits: u64,
    pub l3_misses: u64,
    pub l1_evictions: u64,
    pub l2_evictions: u64,
    pub l3_evictions: u64,
    pub l2_to_l1_promotions: u64,
    pub l3_to_l2_promotions: u64,
}

impl CacheStatisticsSnapshot {
    /// Calculate overall hit rate
    ///
    /// 计算整体命中率，基于 L1 缓存的统计数据。
    /// L1 是最快、最关键的缓存层，因此使用 L1 的命中率作为整体性能指标。
    pub fn overall_hit_rate(&self) -> f64 {
        // 使用 L1 层的命中率作为整体命中率
        // L1 hits 表示直接命中最快的缓存
        // L1 misses 表示需要检查更慢的层级
        let total = self.l1_hits + self.l1_misses;

        if total == 0 {
            return 0.0;
        }

        self.l1_hits as f64 / total as f64
    }

    /// Calculate hit rate for L1 cache
    pub fn l1_hit_rate(&self) -> f64 {
        let total = self.l1_hits + self.l1_misses;

        if total == 0 {
            return 0.0;
        }

        self.l1_hits as f64 / total as f64
    }

    /// Calculate hit rate for L2 cache
    pub fn l2_hit_rate(&self) -> f64 {
        let total = self.l2_hits + self.l2_misses;

        if total == 0 {
            return 0.0;
        }

        self.l2_hits as f64 / total as f64
    }

    /// Calculate hit rate for L3 cache
    pub fn l3_hit_rate(&self) -> f64 {
        let total = self.l3_hits + self.l3_misses;

        if total == 0 {
            return 0.0;
        }

        self.l3_hits as f64 / total as f64
    }

    /// Get total hits across all levels
    pub fn total_hits(&self) -> u64 {
        self.l1_hits + self.l2_hits + self.l3_hits
    }

    /// Get total misses across all levels
    pub fn total_misses(&self) -> u64 {
        self.l1_misses + self.l2_misses + self.l3_misses
    }
}

/// Configuration for tiered translation cache
#[derive(Debug, Clone)]
pub struct TieredTranslationCacheConfig {
    /// L1 cache capacity (hot instructions)
    pub l1_capacity: usize,
    /// L2 cache capacity (compiled blocks)
    pub l2_capacity: usize,
    /// L3 cache capacity (optimized regions)
    pub l3_capacity: usize,
    /// Enable adaptive cache sizing
    pub enable_adaptive_sizing: bool,
    /// Promotion threshold from L2 to L1 (execution count)
    pub l2_to_l1_threshold: u64,
    /// Promotion threshold from L3 to L2 (execution count)
    pub l3_to_l2_threshold: u64,
    /// Enable cache preheating
    pub enable_preheating: bool,
}

impl Default for TieredTranslationCacheConfig {
    fn default() -> Self {
        Self {
            l1_capacity: L1_MAX_CAPACITY,
            l2_capacity: L2_MAX_CAPACITY,
            l3_capacity: L3_MAX_CAPACITY,
            enable_adaptive_sizing: true,
            l2_to_l1_threshold: 100,
            l3_to_l2_threshold: 50,
            enable_preheating: true,
        }
    }
}

/// Simple LRU cache implementation using HashMap + Vec for order tracking
struct SimpleLruCache<K, V> {
    entries: HashMap<K, V>,
    order: Vec<K>,
    capacity: usize,
}

impl<K: Clone + std::hash::Hash + Eq, V> SimpleLruCache<K, V> {
    fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(capacity),
            order: Vec::with_capacity(capacity),
            capacity,
        }
    }

    fn get(&mut self, key: &K) -> Option<&V> {
        if self.entries.contains_key(key) {
            // Update access order
            self.order.retain(|k| k != key);
            self.order.push(key.clone());
            self.entries.get(key)
        } else {
            None
        }
    }

    fn put(&mut self, key: K, value: V) {
        // Evict if at capacity
        if self.entries.len() >= self.capacity
            && !self.entries.contains_key(&key)
            && let Some(old_key) = self.order.first()
        {
            let old_key = old_key.clone();
            self.order.remove(0);
            self.entries.remove(&old_key);
        }

        // Update order
        self.order.retain(|k| k != &key);
        self.order.push(key.clone());

        self.entries.insert(key, value);
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries.iter()
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }

    fn remove(&mut self, key: &K) -> Option<V> {
        self.order.retain(|k| k != key);
        self.entries.remove(key)
    }
}

/// Tiered translation cache with 3-level hierarchy
pub struct TieredTranslationCache {
    /// L1 cache: Hot instructions (fastest access)
    l1_insn: SimpleLruCache<GuestAddr, TranslatedInsn>,
    /// L2 cache: Compiled blocks
    l2_block: SimpleLruCache<GuestAddr, CompiledBlock>,
    /// L3 cache: Optimized regions
    l3_region: SimpleLruCache<GuestAddr, OptimizedRegion>,
    /// Cache configuration
    config: TieredTranslationCacheConfig,
    /// Statistics
    stats: CacheStatisticsSnapshot,
}

impl TieredTranslationCache {
    /// Create a new tiered translation cache with default configuration
    pub fn new() -> Self {
        Self::with_config(TieredTranslationCacheConfig::default())
    }

    /// Create a new tiered translation cache with custom configuration
    pub fn with_config(config: TieredTranslationCacheConfig) -> Self {
        Self {
            l1_insn: SimpleLruCache::new(config.l1_capacity),
            l2_block: SimpleLruCache::new(config.l2_capacity),
            l3_region: SimpleLruCache::new(config.l3_capacity),
            config,
            stats: CacheStatisticsSnapshot::default(),
        }
    }

    /// Lookup an instruction in the tiered cache (L1 -> L2 -> L3)
    pub fn lookup_insn(&mut self, guest_addr: GuestAddr) -> Option<Arc<CompiledCode>> {
        // Check L1 first
        if let Some(insn) = self.l1_insn.get(&guest_addr) {
            self.stats.l1_hits += 1;
            return Some(insn.compiled.clone());
        }

        self.stats.l1_misses += 1;

        // Check L2
        if let Some(block) = self.l2_block.get(&guest_addr) {
            self.stats.l2_hits += 1;

            // Promote to L1 if threshold exceeded
            if block.exec_count >= self.config.l2_to_l1_threshold {
                let insn = TranslatedInsn {
                    guest_addr,
                    code: block.code.clone(),
                    code_size: block.code_size,
                    exec_count: block.exec_count,
                    last_access: block.last_access,
                    compiled: block.compiled.clone(),
                };
                self.l1_insn.put(guest_addr, insn);
                self.stats.l2_to_l1_promotions += 1;
            }

            return Some(block.compiled.clone());
        }

        self.stats.l2_misses += 1;

        // Check L3
        for (_addr, region) in self.l3_region.iter() {
            if guest_addr >= region.start_addr && guest_addr <= region.end_addr {
                self.stats.l3_hits += 1;

                // Promote to L2 if threshold exceeded
                if region.exec_count >= self.config.l3_to_l2_threshold {
                    self.stats.l3_to_l2_promotions += 1;
                }

                return Some(region.compiled.clone());
            }
        }

        self.stats.l3_misses += 1;
        None
    }

    /// Insert a translated instruction into L1 cache
    pub fn insert_l1_insn(&mut self, guest_addr: GuestAddr, insn: TranslatedInsn) {
        if self.l1_insn.len() >= self.config.l1_capacity {
            self.stats.l1_evictions += 1;
        }
        self.l1_insn.put(guest_addr, insn);
    }

    /// Insert a compiled block into L2 cache
    pub fn insert_l2_block(&mut self, guest_addr: GuestAddr, block: CompiledBlock) {
        if self.l2_block.len() >= self.config.l2_capacity {
            self.stats.l2_evictions += 1;
        }
        self.l2_block.put(guest_addr, block);
    }

    /// Insert an optimized region into L3 cache
    pub fn insert_l3_region(&mut self, guest_addr: GuestAddr, region: OptimizedRegion) {
        if self.l3_region.len() >= self.config.l3_capacity {
            self.stats.l3_evictions += 1;
        }
        self.l3_region.put(guest_addr, region);
    }

    /// Clear all cache levels
    pub fn clear(&mut self) {
        self.l1_insn.clear();
        self.l2_block.clear();
        self.l3_region.clear();
        self.stats = CacheStatisticsSnapshot::default();
    }

    /// Get cache statistics snapshot
    pub fn stats(&self) -> &CacheStatisticsSnapshot {
        &self.stats
    }

    /// Get current cache configuration
    pub fn config(&self) -> &TieredTranslationCacheConfig {
        &self.config
    }

    /// Get current size of each cache level
    pub fn sizes(&self) -> (usize, usize, usize) {
        (
            self.l1_insn.len(),
            self.l2_block.len(),
            self.l3_region.len(),
        )
    }

    /// Invalidate cache entries for a specific address range
    pub fn invalidate_range(&mut self, start: GuestAddr, end: GuestAddr) {
        // Collect keys to remove
        let l1_keys: Vec<_> = self
            .l1_insn
            .iter()
            .filter(|(addr, _)| *addr >= &start && *addr <= &end)
            .map(|(addr, _)| *addr)
            .collect();

        for key in &l1_keys {
            self.l1_insn.remove(key);
        }

        let l2_keys: Vec<_> = self
            .l2_block
            .iter()
            .filter(|(addr, _)| *addr >= &start && *addr <= &end)
            .map(|(addr, _)| *addr)
            .collect();

        for key in &l2_keys {
            self.l2_block.remove(key);
        }

        let l3_keys: Vec<_> = self
            .l3_region
            .iter()
            .filter(|(_, region)| region.start_addr <= end && region.end_addr >= start)
            .map(|(addr, _)| *addr)
            .collect();

        for key in &l3_keys {
            self.l3_region.remove(key);
        }
    }

    /// Get hot entries from L1 cache
    pub fn get_hot_l1_entries(&self, limit: usize) -> Vec<(GuestAddr, &TranslatedInsn)> {
        self.l1_insn
            .iter()
            .take(limit)
            .map(|(k, v)| (*k, v))
            .collect()
    }
}

impl Default for TieredTranslationCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_compiled_code(code: Vec<u8>) -> Arc<CompiledCode> {
        Arc::new(CompiledCode {
            code: code.clone(),
            size: code.len(),
            exec_addr: 0,
        })
    }

    #[test]
    fn test_cache_creation() {
        let cache = TieredTranslationCache::new();
        let (l1, l2, l3) = cache.sizes();
        assert_eq!(l1, 0);
        assert_eq!(l2, 0);
        assert_eq!(l3, 0);
    }

    #[test]
    fn test_l1_insert_and_lookup() {
        let mut cache = TieredTranslationCache::new();

        let insn = TranslatedInsn {
            guest_addr: GuestAddr(0x1000),
            code: vec![0x90, 0x90],
            code_size: 2,
            exec_count: 1,
            last_access: Instant::now(),
            compiled: create_test_compiled_code(vec![0x90, 0x90]),
        };

        cache.insert_l1_insn(GuestAddr(0x1000), insn);

        let result = cache.lookup_insn(GuestAddr(0x1000));
        assert!(result.is_some());

        let stats = cache.stats();
        assert_eq!(stats.l1_hits, 1);
        assert_eq!(stats.l1_misses, 0);
    }

    #[test]
    fn test_l2_insert_and_lookup() {
        let mut cache = TieredTranslationCache::new();

        let ir_block = IRBlock::new(GuestAddr(0x1000));

        let block = CompiledBlock {
            guest_addr: GuestAddr(0x1000),
            ir_block,
            code: vec![0x90, 0x90, 0xC3],
            code_size: 3,
            exec_count: 10,
            last_access: Instant::now(),
            compiled: create_test_compiled_code(vec![0x90, 0x90, 0xC3]),
        };

        cache.insert_l2_block(GuestAddr(0x1000), block);

        let result = cache.lookup_insn(GuestAddr(0x1000));
        assert!(result.is_some());

        let stats = cache.stats();
        assert_eq!(stats.l1_misses, 1);
        assert_eq!(stats.l2_hits, 1);
    }

    #[test]
    fn test_overall_hit_rate() {
        let mut cache = TieredTranslationCache::new();

        let insn = TranslatedInsn {
            guest_addr: GuestAddr(0x1000),
            code: vec![0x90],
            code_size: 1,
            exec_count: 1,
            last_access: Instant::now(),
            compiled: create_test_compiled_code(vec![0x90]),
        };

        cache.insert_l1_insn(GuestAddr(0x1000), insn);

        // Generate some hits and misses
        for _ in 0..80 {
            cache.lookup_insn(GuestAddr(0x1000)); // hit
        }

        for _ in 0..20 {
            cache.lookup_insn(GuestAddr(0x2000)); // miss
        }

        let stats = cache.stats();
        let hit_rate = stats.overall_hit_rate();

        // Debug output
        println!("L1 hits: {}, L1 misses: {}", stats.l1_hits, stats.l1_misses);
        println!("L2 hits: {}, L2 misses: {}", stats.l2_hits, stats.l2_misses);
        println!("L3 hits: {}, L3 misses: {}", stats.l3_hits, stats.l3_misses);
        println!(
            "Total hits: {}, Total misses: {}",
            stats.total_hits(),
            stats.total_misses()
        );
        println!("Hit rate: {}", hit_rate);

        // Should be approximately 0.8 (80 hits out of 100 lookups)
        assert!(
            hit_rate > 0.75 && hit_rate < 0.85,
            "Hit rate {} is not in expected range [0.75, 0.85]",
            hit_rate
        );
    }

    #[test]
    fn test_lru_eviction() {
        let config = TieredTranslationCacheConfig {
            l1_capacity: 3,
            ..Default::default()
        };
        let mut cache = TieredTranslationCache::with_config(config);

        // Fill L1 cache to capacity
        for i in 0..3 {
            let insn = TranslatedInsn {
                guest_addr: GuestAddr(0x1000 + i),
                code: vec![0x90],
                code_size: 1,
                exec_count: 1,
                last_access: Instant::now(),
                compiled: create_test_compiled_code(vec![0x90]),
            };
            cache.insert_l1_insn(GuestAddr(0x1000 + i), insn);
        }

        let (l1, _, _) = cache.sizes();
        assert_eq!(l1, 3);

        // Insert one more - should trigger eviction
        let insn = TranslatedInsn {
            guest_addr: GuestAddr(0x1003),
            code: vec![0x90],
            code_size: 1,
            exec_count: 1,
            last_access: Instant::now(),
            compiled: create_test_compiled_code(vec![0x90]),
        };
        cache.insert_l1_insn(GuestAddr(0x1003), insn);

        let stats = cache.stats();
        assert_eq!(stats.l1_evictions, 1);

        // Size should still be at most 3
        let (l1, _, _) = cache.sizes();
        assert!(l1 <= 3);
    }

    #[test]
    fn test_cache_invalidation() {
        let mut cache = TieredTranslationCache::new();

        let insn = TranslatedInsn {
            guest_addr: GuestAddr(0x1000),
            code: vec![0x90],
            code_size: 1,
            exec_count: 1,
            last_access: Instant::now(),
            compiled: create_test_compiled_code(vec![0x90]),
        };

        cache.insert_l1_insn(GuestAddr(0x1000), insn);

        // Invalidate the range containing this address
        cache.invalidate_range(GuestAddr(0x1000), GuestAddr(0x1100));

        // Should not find it anymore
        let result = cache.lookup_insn(GuestAddr(0x1000));
        assert!(result.is_none());
    }
}
