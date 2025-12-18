//! Caching bounded context
//! 
//! This module defines the caching domain, including cache strategies, policies,
//! and management for JIT compiled code and intermediate results.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use crate::common::{Config, Stats, JITErrorBuilder, JITResult};
use crate::ir::IRBlock;

/// Unique identifier for cache contexts
pub type CacheId = u64;

/// Cache entry status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheEntryStatus {
    /// Entry is being created
    Creating,
    /// Entry is ready for use
    Ready,
    /// Entry is being updated
    Updating,
    /// Entry is marked for deletion
    Deleting,
    /// Entry is invalid
    Invalid,
}

impl Default for CacheEntryStatus {
    fn default() -> Self {
        CacheEntryStatus::Creating
    }
}

impl std::fmt::Display for CacheEntryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheEntryStatus::Creating => write!(f, "Creating"),
            CacheEntryStatus::Ready => write!(f, "Ready"),
            CacheEntryStatus::Updating => write!(f, "Updating"),
            CacheEntryStatus::Deleting => write!(f, "Deleting"),
            CacheEntryStatus::Invalid => write!(f, "Invalid"),
        }
    }
}

/// Cache eviction policy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EvictionPolicy {
    /// Least Recently Used
    LRU,
    /// Least Frequently Used
    LFU,
    /// First In First Out
    FIFO,
    /// Random replacement
    Random,
    /// Time-based expiration
    TimeBased,
    /// No eviction (grow until memory limit)
    None,
}

impl Default for EvictionPolicy {
    fn default() -> Self {
        EvictionPolicy::LRU
    }
}

impl std::fmt::Display for EvictionPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvictionPolicy::LRU => write!(f, "LRU"),
            EvictionPolicy::LFU => write!(f, "LFU"),
            EvictionPolicy::FIFO => write!(f, "FIFO"),
            EvictionPolicy::Random => write!(f, "Random"),
            EvictionPolicy::TimeBased => write!(f, "TimeBased"),
            EvictionPolicy::None => write!(f, "None"),
        }
    }
}

/// Cache entry type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheEntryType {
    /// Compiled code cache
    CompiledCode,
    /// IR block cache
    IRBlock,
    /// Optimization result cache
    OptimizationResult,
    /// Analysis result cache
    AnalysisResult,
    /// Metadata cache
    Metadata,
    /// Custom cache entry
    Custom,
}

impl Default for CacheEntryType {
    fn default() -> Self {
        CacheEntryType::CompiledCode
    }
}

impl std::fmt::Display for CacheEntryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheEntryType::CompiledCode => write!(f, "CompiledCode"),
            CacheEntryType::IRBlock => write!(f, "IRBlock"),
            CacheEntryType::OptimizationResult => write!(f, "OptimizationResult"),
            CacheEntryType::AnalysisResult => write!(f, "AnalysisResult"),
            CacheEntryType::Metadata => write!(f, "Metadata"),
            CacheEntryType::Custom => write!(f, "Custom"),
        }
    }
}

/// Cache configuration
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum cache size in bytes
    pub max_size_bytes: usize,
    /// Maximum number of entries
    pub max_entries: usize,
    /// Eviction policy
    pub eviction_policy: EvictionPolicy,
    /// Entry expiration time
    pub expiration_time: Duration,
    /// Enable compression
    pub enable_compression: bool,
    /// Enable statistics collection
    pub enable_stats: bool,
    /// Cache warming strategy
    pub warmup_strategy: WarmupStrategy,
    /// Custom cache parameters
    pub custom_params: HashMap<String, String>,
}

/// Cache warming strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WarmupStrategy {
    /// No warmup
    None,
    /// Warmup on first access
    Lazy,
    /// Warmup proactively
    Eager,
    /// Warmup based on access patterns
    Adaptive,
}

impl Default for WarmupStrategy {
    fn default() -> Self {
        WarmupStrategy::Lazy
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_size_bytes: 64 * 1024 * 1024, // 64MB
            max_entries: 10000,
            eviction_policy: EvictionPolicy::LRU,
            expiration_time: Duration::from_secs(3600), // 1 hour
            enable_compression: false,
            enable_stats: true,
            warmup_strategy: WarmupStrategy::Lazy,
            custom_params: HashMap::new(),
        }
    }
}

impl Config for CacheConfig {
    fn validate(&self) -> Result<(), String> {
        if self.max_size_bytes == 0 {
            return Err("Maximum cache size cannot be zero".to_string());
        }
        
        if self.max_entries == 0 {
            return Err("Maximum number of entries cannot be zero".to_string());
        }
        
        if self.expiration_time.is_zero() {
            return Err("Expiration time cannot be zero".to_string());
        }
        
        Ok(())
    }
    
    fn summary(&self) -> String {
        format!(
            "CacheConfig(max_size={}MB, max_entries={}, eviction={}, compression={}, warmup={})",
            self.max_size_bytes / (1024 * 1024),
            self.max_entries,
            self.eviction_policy,
            self.enable_compression,
            self.warmup_strategy
        )
    }
    
    fn merge(&mut self, other: &Self) {
        // Use the smaller max size
        if other.max_size_bytes < self.max_size_bytes {
            self.max_size_bytes = other.max_size_bytes;
        }
        
        // Use the smaller max entries
        if other.max_entries < self.max_entries {
            self.max_entries = other.max_entries;
        }
        
        // Use the other eviction policy
        self.eviction_policy = other.eviction_policy;
        
        // Use the shorter expiration time
        if other.expiration_time < self.expiration_time {
            self.expiration_time = other.expiration_time;
        }
        
        // Merge compression settings
        self.enable_compression = self.enable_compression || other.enable_compression;
        
        // Merge stats settings
        self.enable_stats = self.enable_stats || other.enable_stats;
        
        // Use the other warmup strategy
        self.warmup_strategy = other.warmup_strategy;
        
        // Merge custom parameters
        for (key, value) in &other.custom_params {
            self.custom_params.insert(key.clone(), value.clone());
        }
    }
}

/// Cache entry metadata
#[derive(Debug, Clone)]
pub struct CacheEntryMetadata {
    /// Entry ID
    pub entry_id: CacheId,
    /// Entry type
    pub entry_type: CacheEntryType,
    /// Entry status
    pub status: CacheEntryStatus,
    /// Creation time
    pub created_at: Instant,
    /// Last access time
    pub last_accessed: Instant,
    /// Access count
    pub access_count: u64,
    /// Entry size in bytes
    pub size_bytes: usize,
    /// Expiration time
    pub expires_at: Option<Instant>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl Default for CacheEntryMetadata {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            entry_id: generate_cache_id(),
            entry_type: CacheEntryType::CompiledCode,
            status: CacheEntryStatus::Creating,
            created_at: now,
            last_accessed: now,
            access_count: 0,
            size_bytes: 0,
            expires_at: None,
            metadata: HashMap::new(),
        }
    }
}

/// Cache entry
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// Entry metadata
    pub metadata: CacheEntryMetadata,
    /// Entry data
    pub data: Vec<u8>,
}

impl CacheEntry {
    /// Create a new cache entry
    pub fn new(entry_type: CacheEntryType, data: Vec<u8>) -> Self {
        let size_bytes = data.len();
        let mut metadata = CacheEntryMetadata::default();
        metadata.entry_type = entry_type;
        metadata.size_bytes = size_bytes;
        
        Self { metadata, data }
    }
    
    /// Mark entry as ready
    pub fn mark_ready(&mut self) {
        self.metadata.status = CacheEntryStatus::Ready;
    }
    
    /// Mark entry as invalid
    pub fn mark_invalid(&mut self) {
        self.metadata.status = CacheEntryStatus::Invalid;
    }
    
    /// Update access information
    pub fn update_access(&mut self) {
        self.metadata.last_accessed = Instant::now();
        self.metadata.access_count += 1;
    }
    
    /// Check if entry is expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.metadata.expires_at {
            Instant::now() > expires_at
        } else {
            false
        }
    }
    
    /// Check if entry is valid for use
    pub fn is_valid(&self) -> bool {
        self.metadata.status == CacheEntryStatus::Ready && !self.is_expired()
    }
}

/// Cache statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Total number of cache hits
    pub hits: u64,
    /// Total number of cache misses
    pub misses: u64,
    /// Total number of evictions
    pub evictions: u64,
    /// Current cache size in bytes
    pub current_size_bytes: usize,
    /// Current number of entries
    pub current_entries: usize,
    /// Total number of entries created
    pub total_entries_created: u64,
    /// Total number of entries deleted
    pub total_entries_deleted: u64,
    /// Average access time in nanoseconds
    pub avg_access_time_ns: u64,
    /// Maximum access time in nanoseconds
    pub max_access_time_ns: u64,
    /// Minimum access time in nanoseconds
    pub min_access_time_ns: u64,
}

impl Stats for CacheStats {
    fn reset(&mut self) {
        *self = Self::default();
    }
    
    fn merge(&mut self, other: &Self) {
        self.hits += other.hits;
        self.misses += other.misses;
        self.evictions += other.evictions;
        self.current_size_bytes = other.current_size_bytes;
        self.current_entries = other.current_entries;
        self.total_entries_created += other.total_entries_created;
        self.total_entries_deleted += other.total_entries_deleted;
        
        // Recalculate average access time
        let total_accesses = self.hits + self.misses;
        if total_accesses > 0 {
            self.avg_access_time_ns = (self.avg_access_time_ns * (total_accesses - other.hits - other.misses) + 
                                      other.avg_access_time_ns * (other.hits + other.misses)) / total_accesses;
        }
        
        // Update max and min access times
        self.max_access_time_ns = self.max_access_time_ns.max(other.max_access_time_ns);
        self.min_access_time_ns = if self.min_access_time_ns == 0 {
            other.min_access_time_ns
        } else {
            self.min_access_time_ns.min(other.min_access_time_ns)
        };
    }
    
    fn summary(&self) -> String {
        let hit_rate = if self.hits + self.misses > 0 {
            (self.hits as f64 / (self.hits + self.misses) as f64) * 100.0
        } else {
            0.0
        };
        
        format!(
            "CacheStats(hits={}, misses={}, hit_rate={:.2}%, evictions={}, size={}MB, entries={}, avg_access={}ns)",
            self.hits,
            self.misses,
            hit_rate,
            self.evictions,
            self.current_size_bytes / (1024 * 1024),
            self.current_entries,
            self.avg_access_time_ns
        )
    }
}

/// Cache context
#[derive(Debug, Clone)]
pub struct CacheContext {
    /// Cache ID
    pub cache_id: CacheId,
    /// Cache configuration
    pub config: CacheConfig,
    /// Cache entries
    pub entries: HashMap<CacheId, CacheEntry>,
    /// Cache statistics
    pub stats: CacheStats,
    /// Current cache size in bytes
    pub current_size_bytes: usize,
}

impl CacheContext {
    /// Create a new cache context
    pub fn new(config: CacheConfig) -> Self {
        Self {
            cache_id: generate_cache_id(),
            config,
            entries: HashMap::new(),
            stats: CacheStats::default(),
            current_size_bytes: 0,
        }
    }
    
    /// Get cache hit rate
    pub fn hit_rate(&self) -> f64 {
        if self.stats.hits + self.stats.misses == 0 {
            0.0
        } else {
            (self.stats.hits as f64 / (self.stats.hits + self.stats.misses) as f64) * 100.0
        }
    }
    
    /// Check if cache is full
    pub fn is_full(&self) -> bool {
        self.current_size_bytes >= self.config.max_size_bytes ||
        self.entries.len() >= self.config.max_entries
    }
    
    /// Get cache utilization
    pub fn utilization(&self) -> f64 {
        let size_utilization = self.current_size_bytes as f64 / self.config.max_size_bytes as f64;
        let entry_utilization = self.entries.len() as f64 / self.config.max_entries as f64;
        size_utilization.max(entry_utilization)
    }
}

/// Cache service
pub struct CacheService {
    /// Cache contexts
    contexts: HashMap<CacheId, Arc<RwLock<CacheContext>>>,
    /// Global cache statistics
    global_stats: CacheStats,
}

impl CacheService {
    /// Create a new cache service
    pub fn new() -> Self {
        Self {
            contexts: HashMap::new(),
            global_stats: CacheStats::default(),
        }
    }
    
    /// Create a new cache context
    pub fn create_cache(&mut self, config: CacheConfig) -> CacheId {
        let context = CacheContext::new(config);
        let cache_id = context.cache_id;
        self.contexts.insert(cache_id, Arc::new(RwLock::new(context)));
        cache_id
    }
    
    /// Get a cache context
    pub fn get_cache(&self, cache_id: CacheId) -> Option<Arc<RwLock<CacheContext>>> {
        self.contexts.get(&cache_id).cloned()
    }
    
    /// Remove a cache context
    pub fn remove_cache(&mut self, cache_id: CacheId) -> bool {
        self.contexts.remove(&cache_id).is_some()
    }
    
    /// Store data in cache
    pub fn store(&self, cache_id: CacheId, entry_type: CacheEntryType, data: Vec<u8>) -> JITResult<CacheId> {
        let cache = self.get_cache(cache_id)
            .ok_or_else(|| JITErrorBuilder::cache(format!("Cache {} not found", cache_id)))?;
        
        let mut cache_ctx = cache.write().map_err(|e| {
            JITErrorBuilder::cache(format!("Failed to acquire write lock: {}", e))
        })?;
        
        // Check if cache is full
        if cache_ctx.is_full() {
            self.evict_entries(&mut cache_ctx)?;
        }
        
        // Create new cache entry
        let mut entry = CacheEntry::new(entry_type, data);
        
        // Set expiration time if configured
        if cache_ctx.config.expiration_time != Duration::ZERO {
            entry.metadata.expires_at = Some(Instant::now() + cache_ctx.config.expiration_time);
        }
        
        // Mark entry as ready
        entry.mark_ready();
        
        let entry_id = entry.metadata.entry_id;
        let entry_size = entry.metadata.size_bytes;
        
        // Add entry to cache
        cache_ctx.entries.insert(entry_id, entry);
        cache_ctx.current_size_bytes += entry_size;
        
        // Update statistics
        cache_ctx.stats.total_entries_created += 1;
        cache_ctx.stats.current_size_bytes = cache_ctx.current_size_bytes;
        cache_ctx.stats.current_entries = cache_ctx.entries.len();
        
        Ok(entry_id)
    }
    
    /// Retrieve data from cache
    pub fn retrieve(&self, cache_id: CacheId, entry_id: CacheId) -> JITResult<Vec<u8>> {
        let start_time = Instant::now();
        
        let cache = self.get_cache(cache_id)
            .ok_or_else(|| JITErrorBuilder::cache(format!("Cache {} not found", cache_id)))?;
        
        let mut cache_ctx = cache.write().map_err(|e| {
            JITErrorBuilder::cache(format!("Failed to acquire write lock: {}", e))
        })?;
        
        // Check if entry exists
        if let Some(entry) = cache_ctx.entries.get_mut(&entry_id) {
            // Check if entry is valid
            if entry.is_valid() {
                // Update access information
                entry.update_access();
                
                // Update statistics
                cache_ctx.stats.hits += 1;
                let access_time = start_time.elapsed().as_nanos() as u64;
                cache_ctx.stats.avg_access_time_ns = 
                    (cache_ctx.stats.avg_access_time_ns * (cache_ctx.stats.hits - 1) + access_time) / cache_ctx.stats.hits;
                cache_ctx.stats.max_access_time_ns = cache_ctx.stats.max_access_time_ns.max(access_time);
                if cache_ctx.stats.min_access_time_ns == 0 {
                    cache_ctx.stats.min_access_time_ns = access_time;
                } else {
                    cache_ctx.stats.min_access_time_ns = cache_ctx.stats.min_access_time_ns.min(access_time);
                }
                
                return Ok(entry.data.clone());
            } else {
                // Entry is invalid, remove it
                let entry_size = entry.metadata.size_bytes;
                cache_ctx.entries.remove(&entry_id);
                cache_ctx.current_size_bytes -= entry_size;
                cache_ctx.stats.total_entries_deleted += 1;
                cache_ctx.stats.current_size_bytes = cache_ctx.current_size_bytes;
                cache_ctx.stats.current_entries = cache_ctx.entries.len();
            }
        }
        
        // Update statistics
        cache_ctx.stats.misses += 1;
        
        Err(JITErrorBuilder::cache(format!("Cache entry {} not found or invalid", entry_id)))
    }
    
    /// Invalidate a cache entry
    pub fn invalidate(&self, cache_id: CacheId, entry_id: CacheId) -> JITResult<()> {
        let cache = self.get_cache(cache_id)
            .ok_or_else(|| JITErrorBuilder::cache(format!("Cache {} not found", cache_id)))?;
        
        let mut cache_ctx = cache.write().map_err(|e| {
            JITErrorBuilder::cache(format!("Failed to acquire write lock: {}", e))
        })?;
        
        if let Some(entry) = cache_ctx.entries.get_mut(&entry_id) {
            entry.mark_invalid();
            Ok(())
        } else {
            Err(JITErrorBuilder::cache(format!("Cache entry {} not found", entry_id)))
        }
    }
    
    /// Evict entries based on eviction policy
    fn evict_entries(&self, cache_ctx: &mut CacheContext) -> JITResult<()> {
        if cache_ctx.config.eviction_policy == EvictionPolicy::None {
            return Err(JITErrorBuilder::cache("Cache is full and eviction is disabled".to_string()));
        }
        
        // Collect entries to evict
        let mut entries_to_evict = Vec::new();
        
        match cache_ctx.config.eviction_policy {
            EvictionPolicy::LRU => {
                // Sort by last accessed time (oldest first)
                let mut entries: Vec<_> = cache_ctx.entries.iter().collect();
                entries.sort_by_key(|(_, entry)| entry.metadata.last_accessed);
                
                // Take entries until we have enough space
                let mut size_freed = 0;
                let mut entries_freed = 0;
                
                for (entry_id, entry) in entries {
                    entries_to_evict.push(*entry_id);
                    size_freed += entry.metadata.size_bytes;
                    entries_freed += 1;
                    
                    if cache_ctx.current_size_bytes - size_freed <= cache_ctx.config.max_size_bytes &&
                       cache_ctx.entries.len() - entries_freed <= cache_ctx.config.max_entries {
                        break;
                    }
                }
            }
            EvictionPolicy::LFU => {
                // Sort by access count (least frequently used first)
                let mut entries: Vec<_> = cache_ctx.entries.iter().collect();
                entries.sort_by_key(|(_, entry)| entry.metadata.access_count);
                
                // Take entries until we have enough space
                let mut size_freed = 0;
                let mut entries_freed = 0;
                
                for (entry_id, entry) in entries {
                    entries_to_evict.push(*entry_id);
                    size_freed += entry.metadata.size_bytes;
                    entries_freed += 1;
                    
                    if cache_ctx.current_size_bytes - size_freed <= cache_ctx.config.max_size_bytes &&
                       cache_ctx.entries.len() - entries_freed <= cache_ctx.config.max_entries {
                        break;
                    }
                }
            }
            EvictionPolicy::FIFO => {
                // Sort by creation time (oldest first)
                let mut entries: Vec<_> = cache_ctx.entries.iter().collect();
                entries.sort_by_key(|(_, entry)| entry.metadata.created_at);
                
                // Take entries until we have enough space
                let mut size_freed = 0;
                let mut entries_freed = 0;
                
                for (entry_id, entry) in entries {
                    entries_to_evict.push(*entry_id);
                    size_freed += entry.metadata.size_bytes;
                    entries_freed += 1;
                    
                    if cache_ctx.current_size_bytes - size_freed <= cache_ctx.config.max_size_bytes &&
                       cache_ctx.entries.len() - entries_freed <= cache_ctx.config.max_entries {
                        break;
                    }
                }
            }
            EvictionPolicy::Random => {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                
                // Randomly select entries to evict
                let mut size_freed = 0;
                let mut entries_freed = 0;
                let mut rng_seed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos() as u64;
                
                let mut entry_ids: Vec<_> = cache_ctx.entries.keys().cloned().collect();
                
                while !entry_ids.is_empty() && 
                      (cache_ctx.current_size_bytes - size_freed > cache_ctx.config.max_size_bytes ||
                       cache_ctx.entries.len() - entries_freed > cache_ctx.config.max_entries) {
                    let mut hasher = DefaultHasher::new();
                    rng_seed.hash(&mut hasher);
                    let index = (hasher.finish() as usize) % entry_ids.len();
                    let entry_id = entry_ids.swap_remove(index);
                    
                    if let Some(entry) = cache_ctx.entries.get(&entry_id) {
                        entries_to_evict.push(entry_id);
                        size_freed += entry.metadata.size_bytes;
                        entries_freed += 1;
                    }
                }
            }
            EvictionPolicy::TimeBased => {
                // Evict expired entries
                for (entry_id, entry) in &cache_ctx.entries {
                    if entry.is_expired() {
                        entries_to_evict.push(*entry_id);
                    }
                }
            }
            EvictionPolicy::None => {
                // No eviction
            }
        }
        
        // Evict the selected entries
        for entry_id in entries_to_evict {
            if let Some(entry) = cache_ctx.entries.remove(&entry_id) {
                cache_ctx.current_size_bytes -= entry.metadata.size_bytes;
                cache_ctx.stats.evictions += 1;
                cache_ctx.stats.total_entries_deleted += 1;
            }
        }
        
        cache_ctx.stats.current_size_bytes = cache_ctx.current_size_bytes;
        cache_ctx.stats.current_entries = cache_ctx.entries.len();
        
        Ok(())
    }
    
    /// Get cache statistics
    pub fn get_stats(&self, cache_id: CacheId) -> JITResult<CacheStats> {
        let cache = self.get_cache(cache_id)
            .ok_or_else(|| JITErrorBuilder::cache(format!("Cache {} not found", cache_id)))?;
        
        let cache_ctx = cache.read().map_err(|e| {
            JITErrorBuilder::cache(format!("Failed to acquire read lock: {}", e))
        })?;
        
        Ok(cache_ctx.stats.clone())
    }
    
    /// Get global cache statistics
    pub fn get_global_stats(&self) -> &CacheStats {
        &self.global_stats
    }
    
    /// Clear all caches
    pub fn clear_all(&mut self) {
        self.contexts.clear();
        self.global_stats.reset();
    }
    
    /// Clear a specific cache
    pub fn clear_cache(&mut self, cache_id: CacheId) -> JITResult<()> {
        if self.contexts.remove(&cache_id).is_some() {
            Ok(())
        } else {
            Err(JITErrorBuilder::cache(format!("Cache {} not found", cache_id)))
        }
    }
}

impl Default for CacheService {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate a unique cache ID
fn generate_cache_id() -> CacheId {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    COUNTER.fetch_add(1, Ordering::SeqCst)
}

/// Cache analysis tools
pub mod analysis {
    use super::*;
    
    /// Analyze cache efficiency
    pub fn analyze_cache_efficiency(stats: &CacheStats) -> CacheEfficiencyReport {
        let hit_rate = if stats.hits + stats.misses > 0 {
            (stats.hits as f64 / (stats.hits + stats.misses) as f64) * 100.0
        } else {
            0.0
        };
        
        let eviction_rate = if stats.total_entries_created > 0 {
            (stats.evictions as f64 / stats.total_entries_created as f64) * 100.0
        } else {
            0.0
        };
        
        let size_utilization = if stats.current_size_bytes > 0 {
            // This would need the max size from the config, which we don't have here
            // For now, we'll just return the current size
            stats.current_size_bytes as f64
        } else {
            0.0
        };
        
        CacheEfficiencyReport {
            hit_rate,
            eviction_rate,
            size_utilization,
            avg_access_time_ns: stats.avg_access_time_ns,
            max_access_time_ns: stats.max_access_time_ns,
            min_access_time_ns: stats.min_access_time_ns,
        }
    }
    
    /// Cache efficiency report
    #[derive(Debug, Clone)]
    pub struct CacheEfficiencyReport {
        /// Hit rate as percentage
        pub hit_rate: f64,
        /// Eviction rate as percentage
        pub eviction_rate: f64,
        /// Size utilization in bytes
        pub size_utilization: f64,
        /// Average access time in nanoseconds
        pub avg_access_time_ns: u64,
        /// Maximum access time in nanoseconds
        pub max_access_time_ns: u64,
        /// Minimum access time in nanoseconds
        pub min_access_time_ns: u64,
    }
    
    impl std::fmt::Display for CacheEfficiencyReport {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "CacheEfficiency(hit_rate={:.2}%, eviction_rate={:.2}%, size_utilization={:.2}MB, avg_access={}ns)",
                self.hit_rate,
                self.eviction_rate,
                self.size_utilization / (1024.0 * 1024.0),
                self.avg_access_time_ns
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_cache_config_validation() {
        let mut config = CacheConfig::default();
        
        // Valid config
        assert!(config.validate().is_ok());
        
        // Invalid max size
        config.max_size_bytes = 0;
        assert!(config.validate().is_err());
        
        // Invalid max entries
        config.max_size_bytes = 1024;
        config.max_entries = 0;
        assert!(config.validate().is_err());
        
        // Invalid expiration time
        config.max_entries = 100;
        config.expiration_time = Duration::ZERO;
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_cache_entry() {
        let data = vec![1, 2, 3, 4, 5];
        let mut entry = CacheEntry::new(CacheEntryType::CompiledCode, data.clone());
        
        assert_eq!(entry.metadata.entry_type, CacheEntryType::CompiledCode);
        assert_eq!(entry.data, data);
        assert_eq!(entry.metadata.status, CacheEntryStatus::Creating);
        
        entry.mark_ready();
        assert_eq!(entry.metadata.status, CacheEntryStatus::Ready);
        
        entry.mark_invalid();
        assert_eq!(entry.metadata.status, CacheEntryStatus::Invalid);
        
        assert!(!entry.is_valid());
        
        entry.mark_ready();
        assert!(entry.is_valid());
        assert!(!entry.is_expired());
        
        entry.update_access();
        assert_eq!(entry.metadata.access_count, 1);
    }
    
    #[test]
    fn test_cache_context() {
        let config = CacheConfig::default();
        let mut context = CacheContext::new(config);
        
        assert_eq!(context.entries.len(), 0);
        assert_eq!(context.current_size_bytes, 0);
        assert_eq!(context.hit_rate(), 0.0);
        assert!(!context.is_full());
        
        // Add an entry
        let data = vec![1, 2, 3, 4, 5];
        let entry = CacheEntry::new(CacheEntryType::CompiledCode, data);
        let entry_id = entry.metadata.entry_id;
        let entry_size = entry.metadata.size_bytes;
        
        context.entries.insert(entry_id, entry);
        context.current_size_bytes += entry_size;
        
        assert_eq!(context.entries.len(), 1);
        assert_eq!(context.current_size_bytes, entry_size);
        assert_eq!(context.hit_rate(), 0.0);
    }
    
    #[test]
    fn test_cache_service() {
        let mut service = CacheService::new();
        let config = CacheConfig::default();
        
        // Create a cache
        let cache_id = service.create_cache(config);
        assert!(service.get_cache(cache_id).is_some());
        
        // Store data
        let data = vec![1, 2, 3, 4, 5];
        let entry_id = service.store(cache_id, CacheEntryType::CompiledCode, data.clone()).unwrap();
        
        // Retrieve data
        let retrieved_data = service.retrieve(cache_id, entry_id).unwrap();
        assert_eq!(retrieved_data, data);
        
        // Get stats
        let stats = service.get_stats(cache_id).unwrap();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 0);
        
        // Remove cache
        assert!(service.remove_cache(cache_id));
        assert!(service.get_cache(cache_id).is_none());
    }
    
    #[test]
    fn test_cache_efficiency_analysis() {
        let mut stats = CacheStats::default();
        stats.hits = 80;
        stats.misses = 20;
        stats.evictions = 5;
        stats.total_entries_created = 100;
        stats.avg_access_time_ns = 100;
        stats.max_access_time_ns = 200;
        stats.min_access_time_ns = 50;
        
        let report = analysis::analyze_cache_efficiency(&stats);
        
        assert_eq!(report.hit_rate, 80.0);
        assert_eq!(report.eviction_rate, 5.0);
        assert_eq!(report.avg_access_time_ns, 100);
        assert_eq!(report.max_access_time_ns, 200);
        assert_eq!(report.min_access_time_ns, 50);
    }
}