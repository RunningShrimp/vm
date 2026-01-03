//! Tiered Translation Cache for JIT Optimization
//!
//! Implements a 3-level tiered cache system for JIT translation:
//! - L1: Instruction cache for hot instructions
//! - L2: Block cache for compiled blocks
//! - L3: Region cache for optimized superblocks
//!
//! Features:
//! - LRU eviction policy at all levels
//! - Cache preheating for hot code
//! - Adaptive cache sizing based on hit rates
//! - Comprehensive statistics tracking
//! - Thread-safe operations with atomic counters

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Instant;

use lru::LruCache;
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

/// Cache statistics with atomic counters for thread-safe access
#[derive(Debug, Default)]
pub struct CacheStatistics {
    /// L1 cache hits
    pub l1_hits: std::sync::atomic::AtomicU64,
    /// L1 cache misses
    pub l1_misses: std::sync::atomic::AtomicU64,
    /// L2 cache hits
    pub l2_hits: std::sync::atomic::AtomicU64,
    /// L2 cache misses
    pub l2_misses: std::sync::atomic::AtomicU64,
    /// L3 cache hits
    pub l3_hits: std::sync::atomic::AtomicU64,
    /// L3 cache misses
    pub l3_misses: std::sync::atomic::AtomicU64,
    /// L1 evictions
    pub l1_evictions: std::sync::atomic::AtomicU64,
    /// L2 evictions
    pub l2_evictions: std::sync::atomic::AtomicU64,
    /// L3 evictions
    pub l3_evictions: std::sync::atomic::AtomicU64,
    /// Promotions from L2 to L1
    pub l2_to_l1_promotions: std::sync::atomic::AtomicU64,
    /// Promotions from L3 to L2
    pub l3_to_l2_promotions: std::sync::atomic::AtomicU64,
}

impl CacheStatistics {
    /// Calculate overall hit rate across all levels
    pub fn overall_hit_rate(&self) -> f64 {
        let total_hits = self.total_hits();
        let total_accesses = total_hits + self.total_misses();

        if total_accesses == 0 {
            return 0.0;
        }

        total_hits as f64 / total_accesses as f64
    }

    /// Calculate hit rate for L1 cache
    pub fn l1_hit_rate(&self) -> f64 {
        let hits = self.l1_hits.load(std::sync::atomic::Ordering::Relaxed);
        let misses = self.l1_misses.load(std::sync::atomic::Ordering::Relaxed);
        let total = hits + misses;

        if total == 0 {
            return 0.0;
        }

        hits as f64 / total as f64
    }

    /// Calculate hit rate for L2 cache
    pub fn l2_hit_rate(&self) -> f64 {
        let hits = self.l2_hits.load(std::sync::atomic::Ordering::Relaxed);
        let misses = self.l2_misses.load(std::sync::atomic::Ordering::Relaxed);
        let total = hits + misses;

        if total == 0 {
            return 0.0;
        }

        hits as f64 / total as f64
    }

    /// Calculate hit rate for L3 cache
    pub fn l3_hit_rate(&self) -> f64 {
        let hits = self.l3_hits.load(std::sync::atomic::Ordering::Relaxed);
        let misses = self.l3_misses.load(std::sync::atomic::Ordering::Relaxed);
        let total = hits + misses;

        if total == 0 {
            return 0.0;
        }

        hits as f64 / total as f64
    }

    /// Get total hits across all levels
    pub fn total_hits(&self) -> u64 {
        self.l1_hits.load(std::sync::atomic::Ordering::Relaxed)
            + self.l2_hits.load(std::sync::atomic::Ordering::Relaxed)
            + self.l3_hits.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Get total misses across all levels
    pub fn total_misses(&self) -> u64 {
        self.l1_misses.load(std::sync::atomic::Ordering::Relaxed)
            + self.l2_misses.load(std::sync::atomic::Ordering::Relaxed)
            + self.l3_misses.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Reset all statistics
    pub fn reset(&self) {
        self.l1_hits.store(0, std::sync::atomic::Ordering::Relaxed);
        self.l1_misses
            .store(0, std::sync::atomic::Ordering::Relaxed);
        self.l2_hits.store(0, std::sync::atomic::Ordering::Relaxed);
        self.l2_misses
            .store(0, std::sync::atomic::Ordering::Relaxed);
        self.l3_hits.store(0, std::sync::atomic::Ordering::Relaxed);
        self.l3_misses
            .store(0, std::sync::atomic::Ordering::Relaxed);
        self.l1_evictions
            .store(0, std::sync::atomic::Ordering::Relaxed);
        self.l2_evictions
            .store(0, std::sync::atomic::Ordering::Relaxed);
        self.l3_evictions
            .store(0, std::sync::atomic::Ordering::Relaxed);
        self.l2_to_l1_promotions
            .store(0, std::sync::atomic::Ordering::Relaxed);
        self.l3_to_l2_promotions
            .store(0, std::sync::atomic::Ordering::Relaxed);
    }

    /// Create a snapshot of current statistics
    pub fn snapshot(&self) -> CacheStatisticsSnapshot {
        CacheStatisticsSnapshot {
            l1_hits: self.l1_hits.load(std::sync::atomic::Ordering::Relaxed),
            l1_misses: self.l1_misses.load(std::sync::atomic::Ordering::Relaxed),
            l2_hits: self.l2_hits.load(std::sync::atomic::Ordering::Relaxed),
            l2_misses: self.l2_misses.load(std::sync::atomic::Ordering::Relaxed),
            l3_hits: self.l3_hits.load(std::sync::atomic::Ordering::Relaxed),
            l3_misses: self.l3_misses.load(std::sync::atomic::Ordering::Relaxed),
            l1_evictions: self.l1_evictions.load(std::sync::atomic::Ordering::Relaxed),
            l2_evictions: self.l2_evictions.load(std::sync::atomic::Ordering::Relaxed),
            l3_evictions: self.l3_evictions.load(std::sync::atomic::Ordering::Relaxed),
            l2_to_l1_promotions: self
                .l2_to_l1_promotions
                .load(std::sync::atomic::Ordering::Relaxed),
            l3_to_l2_promotions: self
                .l3_to_l2_promotions
                .load(std::sync::atomic::Ordering::Relaxed),
        }
    }
}

/// Immutable snapshot of cache statistics
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
    pub fn overall_hit_rate(&self) -> f64 {
        let total_hits = self.l1_hits + self.l2_hits + self.l3_hits;
        let total_accesses = total_hits + self.l1_misses + self.l2_misses + self.l3_misses;

        if total_accesses == 0 {
            return 0.0;
        }

        total_hits as f64 / total_accesses as f64
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
pub struct TranslationCacheConfig {
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

impl Default for TranslationCacheConfig {
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

/// Tiered translation cache with 3-level hierarchy
pub struct TranslationCache {
    /// L1 cache: Hot instructions (fastest access)
    l1_insn: LruCache<GuestAddr, TranslatedInsn>,
    /// L2 cache: Compiled blocks
    l2_block: LruCache<GuestAddr, CompiledBlock>,
    /// L3 cache: Optimized regions
    l3_region: LruCache<GuestAddr, OptimizedRegion>,
    /// Cache statistics
    stats: CacheStatistics,
    /// Cache configuration
    config: TranslationCacheConfig,
}

impl TranslationCache {
    /// Create a new tiered translation cache with default configuration
    pub fn new() -> Self {
        Self::with_config(TranslationCacheConfig::default())
    }

    /// Create a new tiered translation cache with custom configuration
    pub fn with_config(config: TranslationCacheConfig) -> Self {
        Self {
            l1_insn: LruCache::new(NonZeroUsize::new(config.l1_capacity).unwrap()),
            l2_block: LruCache::new(NonZeroUsize::new(config.l2_capacity).unwrap()),
            l3_region: LruCache::new(NonZeroUsize::new(config.l3_capacity).unwrap()),
            stats: CacheStatistics::default(),
            config,
        }
    }

    /// Lookup an instruction in the tiered cache (L1 -> L2 -> L3)
    ///
    /// Returns the compiled code if found in any level
    pub fn lookup_insn(&mut self, guest_addr: GuestAddr) -> Option<Arc<CompiledCode>> {
        // Check L1 first
        if let Some(insn) = self.l1_insn.get(&guest_addr) {
            self.stats
                .l1_hits
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Some(insn.compiled.clone());
        }

        self.stats
            .l1_misses
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Check L2 (block-level lookup might need range check)
        let (_should_promote, l2_result) = if let Some(block) = self.l2_block.get(&guest_addr) {
            self.stats
                .l2_hits
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

            // Check if we should promote to L1
            let should_promote = block.exec_count >= self.config.l2_to_l1_threshold;

            if should_promote {
                // Clone block data for promotion
                let guest_addr = block.guest_addr;
                let code = block.code.clone();
                let code_size = block.code_size;
                let exec_count = block.exec_count;
                let last_access = block.last_access;
                let compiled = block.compiled.clone();

                // Create translated instruction
                let insn = TranslatedInsn {
                    guest_addr,
                    code,
                    code_size,
                    exec_count,
                    last_access,
                    compiled,
                };
                (true, Some((Some(insn), block.compiled.clone())))
            } else {
                (false, Some((None, block.compiled.clone())))
            }
        } else {
            self.stats
                .l2_misses
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            (false, None)
        };

        // Insert promoted entry if needed (after the borrow ends)
        if let Some((Some(insn), _)) = l2_result.clone() {
            self.insert_l1_insn(guest_addr, insn);
            self.stats
                .l2_to_l1_promotions
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        // Return the compiled code from L2 if found
        if let Some((_, compiled)) = l2_result {
            return Some(compiled);
        }

        // Check L3 (region-level lookup needs range check)
        for (_addr, region) in self.l3_region.iter() {
            if guest_addr >= region.start_addr && guest_addr <= region.end_addr {
                self.stats
                    .l3_hits
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                // Promote to L2 if threshold exceeded
                if region.exec_count >= self.config.l3_to_l2_threshold {
                    // Note: Region promotion is more complex and may require splitting
                    self.stats
                        .l3_to_l2_promotions
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }

                return Some(region.compiled.clone());
            }
        }

        self.stats
            .l3_misses
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        None
    }

    /// Insert a translated instruction into L1 cache
    pub fn insert_l1_insn(&mut self, guest_addr: GuestAddr, insn: TranslatedInsn) {
        // LRU eviction happens automatically
        if self.l1_insn.len() >= self.config.l1_capacity {
            self.stats
                .l1_evictions
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        self.l1_insn.put(guest_addr, insn);
    }

    /// Insert a compiled block into L2 cache
    pub fn insert_l2_block(&mut self, guest_addr: GuestAddr, block: CompiledBlock) {
        // LRU eviction happens automatically
        if self.l2_block.len() >= self.config.l2_capacity {
            self.stats
                .l2_evictions
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        self.l2_block.put(guest_addr, block);
    }

    /// Insert an optimized region into L3 cache
    pub fn insert_l3_region(&mut self, guest_addr: GuestAddr, region: OptimizedRegion) {
        // LRU eviction happens automatically
        if self.l3_region.len() >= self.config.l3_capacity {
            self.stats
                .l3_evictions
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        self.l3_region.put(guest_addr, region);
    }

    /// Promote a block from L2 to L1 cache
    fn promote_l2_to_l1(&mut self, guest_addr: GuestAddr, block: &CompiledBlock) {
        // Create a translated instruction from the block
        let insn = TranslatedInsn {
            guest_addr,
            code: block.code.clone(),
            code_size: block.code_size,
            exec_count: block.exec_count,
            last_access: block.last_access,
            compiled: block.compiled.clone(),
        };

        self.insert_l1_insn(guest_addr, insn);
        self.stats
            .l2_to_l1_promotions
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// Clear all cache levels
    pub fn clear(&mut self) {
        self.l1_insn.clear();
        self.l2_block.clear();
        self.l3_region.clear();
        self.stats.reset();
    }

    /// Get cache statistics snapshot
    pub fn stats(&self) -> CacheStatisticsSnapshot {
        self.stats.snapshot()
    }

    /// Get reference to statistics (for direct atomic access)
    pub fn stats_ref(&self) -> &CacheStatistics {
        &self.stats
    }

    /// Get current cache configuration
    pub fn config(&self) -> &TranslationCacheConfig {
        &self.config
    }

    /// Update cache configuration
    pub fn set_config(&mut self, config: TranslationCacheConfig) {
        // Resize caches if needed
        if config.l1_capacity != self.config.l1_capacity {
            self.l1_insn = LruCache::new(NonZeroUsize::new(config.l1_capacity).unwrap());
            // Note: This clears the cache, in production you'd want to preserve entries
        }

        if config.l2_capacity != self.config.l2_capacity {
            self.l2_block = LruCache::new(NonZeroUsize::new(config.l2_capacity).unwrap());
        }

        if config.l3_capacity != self.config.l3_capacity {
            self.l3_region = LruCache::new(NonZeroUsize::new(config.l3_capacity).unwrap());
        }

        self.config = config;
    }

    /// Preheat the cache with hot instructions/blocks
    pub fn preheat_l1(&mut self, entries: Vec<(GuestAddr, TranslatedInsn)>) {
        if !self.config.enable_preheating {
            return;
        }

        for (addr, insn) in entries {
            if self.l1_insn.len() < self.config.l1_capacity {
                self.l1_insn.put(addr, insn);
            }
        }
    }

    /// Preheat L2 cache with compiled blocks
    pub fn preheat_l2(&mut self, entries: Vec<(GuestAddr, CompiledBlock)>) {
        if !self.config.enable_preheating {
            return;
        }

        for (addr, block) in entries {
            if self.l2_block.len() < self.config.l2_capacity {
                self.l2_block.put(addr, block);
            }
        }
    }

    /// Preheat L3 cache with optimized regions
    pub fn preheat_l3(&mut self, entries: Vec<(GuestAddr, OptimizedRegion)>) {
        if !self.config.enable_preheating {
            return;
        }

        for (addr, region) in entries {
            if self.l3_region.len() < self.config.l3_capacity {
                self.l3_region.put(addr, region);
            }
        }
    }

    /// Invalidate cache entries for a specific address range
    pub fn invalidate_range(&mut self, start: GuestAddr, end: GuestAddr) {
        // Remove from L1 - collect keys to remove first
        let l1_keys: Vec<GuestAddr> = self
            .l1_insn
            .iter()
            .filter(|(addr, _)| *addr >= &start && *addr <= &end)
            .map(|(addr, _)| *addr)
            .collect();

        for key in &l1_keys {
            self.l1_insn.pop(key);
        }

        // Remove from L2 - collect keys to remove first
        let l2_keys: Vec<GuestAddr> = self
            .l2_block
            .iter()
            .filter(|(addr, _)| *addr >= &start && *addr <= &end)
            .map(|(addr, _)| *addr)
            .collect();

        for key in &l2_keys {
            self.l2_block.pop(key);
        }

        // Remove from L3 (check ranges) - collect keys to remove first
        let l3_keys: Vec<GuestAddr> = self
            .l3_region
            .iter()
            .filter(|(_, region)| region.start_addr <= end && region.end_addr >= start)
            .map(|(addr, _)| *addr)
            .collect();

        for key in &l3_keys {
            self.l3_region.pop(key);
        }
    }

    /// Get current size of each cache level
    pub fn sizes(&self) -> (usize, usize, usize) {
        (
            self.l1_insn.len(),
            self.l2_block.len(),
            self.l3_region.len(),
        )
    }

    /// Perform adaptive cache sizing based on hit rates
    pub fn adapt_cache_sizes(&mut self) {
        if !self.config.enable_adaptive_sizing {
            return;
        }

        let snapshot = self.stats.snapshot();

        // If L1 hit rate is low, increase its size
        if snapshot.l1_hit_rate() < 0.7 && self.config.l1_capacity < L1_MAX_CAPACITY * 2 {
            let new_size = (self.config.l1_capacity * 3) / 2;
            self.config.l1_capacity = new_size.min(L1_MAX_CAPACITY * 2);
            // Note: Resizing would require rebuilding the cache
        }

        // If L2 hit rate is low, increase its size
        if snapshot.l2_hit_rate() < 0.6 && self.config.l2_capacity < L2_MAX_CAPACITY * 2 {
            let new_size = (self.config.l2_capacity * 3) / 2;
            self.config.l2_capacity = new_size.min(L2_MAX_CAPACITY * 2);
        }

        // If L3 hit rate is low, increase its size
        if snapshot.l3_hit_rate() < 0.5 && self.config.l3_capacity < L3_MAX_CAPACITY * 2 {
            let new_size = (self.config.l3_capacity * 3) / 2;
            self.config.l3_capacity = new_size.min(L3_MAX_CAPACITY * 2);
        }
    }

    /// Get hot entries from L1 cache (most frequently accessed)
    pub fn get_hot_l1_entries(&self, limit: usize) -> Vec<(GuestAddr, &TranslatedInsn)> {
        self.l1_insn
            .iter()
            .take(limit)
            .map(|(k, v)| (*k, v))
            .collect()
    }

    /// Get hot entries from L2 cache
    pub fn get_hot_l2_entries(&self, limit: usize) -> Vec<(GuestAddr, &CompiledBlock)> {
        self.l2_block
            .iter()
            .take(limit)
            .map(|(k, v)| (*k, v))
            .collect()
    }
}

impl Default for TranslationCache {
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
        let cache = TranslationCache::new();
        let (l1, l2, l3) = cache.sizes();
        assert_eq!(l1, 0);
        assert_eq!(l2, 0);
        assert_eq!(l3, 0);
    }

    #[test]
    fn test_l1_insert_and_lookup() {
        let mut cache = TranslationCache::new();

        let insn = TranslatedInsn {
            guest_addr: GuestAddr(0x1000),
            code: vec![0x90, 0x90], // NOP
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
    fn test_l1_miss() {
        let mut cache = TranslationCache::new();

        let result = cache.lookup_insn(GuestAddr(0x2000));
        assert!(result.is_none());

        let stats = cache.stats();
        assert_eq!(stats.l1_misses, 1);
    }

    #[test]
    fn test_l2_insert_and_lookup() {
        let mut cache = TranslationCache::new();

        let ir_block = IRBlock::new(GuestAddr(0x1000));

        let block = CompiledBlock {
            guest_addr: GuestAddr(0x1000),
            ir_block,
            code: vec![0x90, 0x90, 0xC3], // NOP, NOP, RET
            code_size: 3,
            exec_count: 10,
            last_access: Instant::now(),
            compiled: create_test_compiled_code(vec![0x90, 0x90, 0xC3]),
        };

        cache.insert_l2_block(GuestAddr(0x1000), block);

        // First lookup will miss L1 but hit L2
        let result = cache.lookup_insn(GuestAddr(0x1000));
        assert!(result.is_some());

        let stats = cache.stats();
        assert_eq!(stats.l1_misses, 1);
        assert_eq!(stats.l2_hits, 1);
    }

    #[test]
    fn test_l3_insert_and_lookup() {
        let mut cache = TranslationCache::new();

        let region = OptimizedRegion {
            start_addr: GuestAddr(0x1000),
            end_addr: GuestAddr(0x1100),
            code: vec![0x90; 256],
            code_size: 256,
            block_count: 10,
            exec_count: 5,
            last_access: Instant::now(),
            opt_level: 2,
            compiled: create_test_compiled_code(vec![0x90; 256]),
        };

        cache.insert_l3_region(GuestAddr(0x1000), region);

        // Lookup an address within the region
        let result = cache.lookup_insn(GuestAddr(0x1050));
        assert!(result.is_some());

        let stats = cache.stats();
        assert_eq!(stats.l1_misses, 1);
        assert_eq!(stats.l2_misses, 1);
        assert_eq!(stats.l3_hits, 1);
    }

    #[test]
    fn test_cache_statistics() {
        let cache = TranslationCache::new();
        let stats = cache.stats();

        assert_eq!(stats.overall_hit_rate(), 0.0);
        assert_eq!(stats.l1_hit_rate(), 0.0);
        assert_eq!(stats.l2_hit_rate(), 0.0);
        assert_eq!(stats.l3_hit_rate(), 0.0);
    }

    #[test]
    fn test_l1_promotion_from_l2() {
        let mut cache = TranslationCache::with_config(TranslationCacheConfig {
            l2_to_l1_threshold: 50,
            ..Default::default()
        });

        let ir_block = IRBlock::new(GuestAddr(0x1000));

        let block = CompiledBlock {
            guest_addr: GuestAddr(0x1000),
            ir_block,
            code: vec![0x90, 0x90],
            code_size: 2,
            exec_count: 100, // Above threshold
            last_access: Instant::now(),
            compiled: create_test_compiled_code(vec![0x90, 0x90]),
        };

        cache.insert_l2_block(GuestAddr(0x1000), block);

        // This lookup should trigger promotion to L1
        cache.lookup_insn(GuestAddr(0x1000));

        let stats = cache.stats();
        assert_eq!(stats.l2_to_l1_promotions, 1);
    }

    #[test]
    fn test_cache_invalidation() {
        let mut cache = TranslationCache::new();

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

    #[test]
    fn test_preheating() {
        let mut cache = TranslationCache::new();

        let entries = vec![(
            GuestAddr(0x1000),
            TranslatedInsn {
                guest_addr: GuestAddr(0x1000),
                code: vec![0x90],
                code_size: 1,
                exec_count: 1,
                last_access: Instant::now(),
                compiled: create_test_compiled_code(vec![0x90]),
            },
        )];

        cache.preheat_l1(entries);

        let (l1, _, _) = cache.sizes();
        assert_eq!(l1, 1);

        // Should find the preheated entry
        let result = cache.lookup_insn(GuestAddr(0x1000));
        assert!(result.is_some());
    }

    #[test]
    fn test_hot_entries() {
        let mut cache = TranslationCache::new();

        for i in 0..5 {
            let insn = TranslatedInsn {
                guest_addr: GuestAddr(0x1000 + i as u64),
                code: vec![0x90],
                code_size: 1,
                exec_count: (i + 1) as u64,
                last_access: Instant::now(),
                compiled: create_test_compiled_code(vec![0x90]),
            };
            cache.insert_l1_insn(GuestAddr(0x1000 + i as u64), insn);
        }

        let hot_entries = cache.get_hot_l1_entries(3);
        assert_eq!(hot_entries.len(), 3);
    }

    #[test]
    fn test_clear_cache() {
        let mut cache = TranslationCache::new();

        let insn = TranslatedInsn {
            guest_addr: GuestAddr(0x1000),
            code: vec![0x90],
            code_size: 1,
            exec_count: 1,
            last_access: Instant::now(),
            compiled: create_test_compiled_code(vec![0x90]),
        };

        cache.insert_l1_insn(GuestAddr(0x1000), insn);
        cache.clear();

        let (l1, l2, l3) = cache.sizes();
        assert_eq!(l1, 0);
        assert_eq!(l2, 0);
        assert_eq!(l3, 0);

        let stats = cache.stats();
        assert_eq!(stats.total_hits(), 0);
        assert_eq!(stats.total_misses(), 0);
    }

    #[test]
    fn test_overall_hit_rate() {
        let mut cache = TranslationCache::new();

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

        // Should be approximately 0.8 (80 hits out of 100 lookups)
        assert!(hit_rate > 0.75 && hit_rate < 0.85);
    }

    #[test]
    fn test_lru_eviction() {
        let config = TranslationCacheConfig {
            l1_capacity: 3,
            ..Default::default()
        };
        let mut cache = TranslationCache::with_config(config);

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
}
