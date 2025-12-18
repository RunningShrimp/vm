//! Cache Management Domain Service
//!
//! This service encapsulates business logic for cache management
//! including cache replacement strategies, promotion/demotion logic,
//! prefetch strategies, and cache sizing policies.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use crate::domain_services::events::{DomainEventBus, DomainEventEnum, OptimizationEvent};
use crate::domain_services::rules::optimization_pipeline_rules::OptimizationPipelineBusinessRule;
use crate::{VmError, VmResult};

/// Cache replacement policy
#[derive(Debug, Clone, PartialEq)]
pub enum CachePolicy {
    /// Least Recently Used
    LRU,
    /// Least Frequently Used
    LFU,
    /// First In First Out
    FIFO,
    /// Adaptive policy based on access patterns
    Adaptive,
    /// Random replacement
    Random,
}

/// Cache entry with metadata
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// Cache key
    pub key: u64,
    /// Cache data
    pub data: Vec<u8>,
    /// Entry size in bytes
    pub size: usize,
    /// Access count
    pub access_count: u64,
    /// Last access timestamp
    pub last_access: std::time::Instant,
    /// Creation timestamp
    pub created_at: std::time::Instant,
    /// Entry priority (higher = more important)
    pub priority: u32,
    /// Whether the entry is pinned (cannot be evicted)
    pub pinned: bool,
}

/// Cache statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStatistics {
    /// Total number of accesses
    pub total_accesses: u64,
    /// Number of cache hits
    pub hits: u64,
    /// Number of cache misses
    pub misses: u64,
    /// Number of evictions
    pub evictions: u64,
    /// Number of promotions
    pub promotions: u64,
    /// Number of demotions
    pub demotions: u64,
    /// Average access time
    pub avg_access_time: std::time::Duration,
}

impl CacheStatistics {
    /// Calculate cache hit rate
    pub fn hit_rate(&self) -> f64 {
        if self.total_accesses == 0 {
            0.0
        } else {
            self.hits as f64 / self.total_accesses as f64
        }
    }

    /// Calculate cache miss rate
    pub fn miss_rate(&self) -> f64 {
        if self.total_accesses == 0 {
            0.0
        } else {
            self.misses as f64 / self.total_accesses as f64
        }
    }
}

/// Access pattern for prefetching
#[derive(Debug, Clone)]
pub struct AccessPattern {
    /// Pattern type
    pub pattern_type: AccessPatternType,
    /// Pattern parameters
    pub parameters: HashMap<String, f64>,
    /// Confidence level (0.0 to 1.0)
    pub confidence: f64,
}

/// Access pattern type
#[derive(Debug, Clone, PartialEq)]
pub enum AccessPatternType {
    /// Sequential access pattern
    Sequential,
    /// Strided access pattern
    Strided,
    /// Random access pattern
    Random,
    /// Loop-based access pattern
    Loop,
    /// Unknown pattern
    Unknown,
}

/// Prefetch strategy
#[derive(Debug, Clone, PartialEq)]
pub enum PrefetchStrategy {
    /// No prefetching
    None,
    /// Always prefetch next cache line
    Always,
    /// Prefetch based on access patterns
    PatternBased,
    /// Adaptive prefetching
    Adaptive,
    /// Hardware-assisted prefetching
    HardwareAssisted,
}

/// Cache tier configuration
#[derive(Debug, Clone)]
pub struct CacheTierConfig {
    /// Tier name
    pub name: String,
    /// Tier capacity in bytes
    pub capacity: usize,
    /// Cache policy for this tier
    pub policy: CachePolicy,
    /// Promotion threshold to next tier
    pub promotion_threshold: f64,
    /// Demotion threshold to previous tier
    pub demotion_threshold: f64,
    /// Prefetch strategy for this tier
    pub prefetch_strategy: PrefetchStrategy,
}

/// Cache management configuration
#[derive(Debug, Clone)]
pub struct CacheManagementConfig {
    /// Cache tiers (L1, L2, L3, etc.)
    pub tiers: Vec<CacheTierConfig>,
    /// Global cache size limit in bytes
    pub global_cache_limit: usize,
    /// Cache warming strategy
    pub warming_strategy: CacheWarmingStrategy,
    /// Cache cooling strategy
    pub cooling_strategy: CacheCoolingStrategy,
    /// Access pattern detection window
    pub pattern_detection_window: std::time::Duration,
    /// Minimum confidence for prefetching
    pub min_prefetch_confidence: f64,
    /// Maximum prefetch distance
    pub max_prefetch_distance: usize,
}

/// Cache warming strategy
#[derive(Debug, Clone, PartialEq)]
pub enum CacheWarmingStrategy {
    /// No warming
    None,
    /// Preload frequently accessed data
    PreloadFrequent,
    /// Preload based on access patterns
    PreloadPatterns,
    /// Adaptive warming
    Adaptive,
}

/// Cache cooling strategy
#[derive(Debug, Clone, PartialEq)]
pub enum CacheCoolingStrategy {
    /// No cooling
    None,
    /// Gradual cooling based on access frequency
    Gradual,
    /// Aggressive cooling for unused entries
    Aggressive,
    /// Time-based cooling
    TimeBased,
}

impl Default for CacheManagementConfig {
    fn default() -> Self {
        Self {
            tiers: vec![
                CacheTierConfig {
                    name: "L1".to_string(),
                    capacity: 32 * 1024, // 32KB
                    policy: CachePolicy::LRU,
                    promotion_threshold: 0.8,
                    demotion_threshold: 0.2,
                    prefetch_strategy: PrefetchStrategy::Adaptive,
                },
                CacheTierConfig {
                    name: "L2".to_string(),
                    capacity: 256 * 1024, // 256KB
                    policy: CachePolicy::LRU,
                    promotion_threshold: 0.7,
                    demotion_threshold: 0.3,
                    prefetch_strategy: PrefetchStrategy::PatternBased,
                },
                CacheTierConfig {
                    name: "L3".to_string(),
                    capacity: 2 * 1024 * 1024, // 2MB
                    policy: CachePolicy::LFU,
                    promotion_threshold: 0.6,
                    demotion_threshold: 0.4,
                    prefetch_strategy: PrefetchStrategy::Adaptive,
                },
            ],
            global_cache_limit: 10 * 1024 * 1024, // 10MB
            warming_strategy: CacheWarmingStrategy::Adaptive,
            cooling_strategy: CacheCoolingStrategy::Gradual,
            pattern_detection_window: std::time::Duration::from_secs(10),
            min_prefetch_confidence: 0.7,
            max_prefetch_distance: 10,
        }
    }
}

/// Cache Management Domain Service
/// 
/// This service encapsulates business logic for cache management
/// including cache replacement strategies, promotion/demotion logic,
/// prefetch strategies, and cache sizing policies.
#[derive(Debug)]
pub struct CacheManagementDomainService {
    /// Business rules for cache management
    business_rules: Vec<Box<dyn OptimizationPipelineBusinessRule>>,
    /// Event bus for publishing domain events
    event_bus: Option<Arc<dyn DomainEventBus>>,
    /// Configuration for cache management
    config: CacheManagementConfig,
    /// Cache entries by tier
    cache_tiers: HashMap<String, HashMap<u64, CacheEntry>>,
    /// LRU order for each tier
    lru_order: HashMap<String, VecDeque<u64>>,
    /// Access frequencies for LFU
    access_frequencies: HashMap<String, HashMap<u64, u64>>,
    /// Cache statistics by tier
    statistics: HashMap<String, CacheStatistics>,
    /// Detected access patterns
    access_patterns: HashMap<String, AccessPattern>,
}

impl CacheManagementDomainService {
    /// Create a new cache management domain service
    pub fn new(config: CacheManagementConfig) -> Self {
        let mut cache_tiers = HashMap::new();
        let mut lru_order = HashMap::new();
        let mut access_frequencies = HashMap::new();
        let mut statistics = HashMap::new();

        // Initialize cache tiers
        for tier in &config.tiers {
            cache_tiers.insert(tier.name.clone(), HashMap::new());
            lru_order.insert(tier.name.clone(), VecDeque::new());
            access_frequencies.insert(tier.name.clone(), HashMap::new());
            statistics.insert(tier.name.clone(), CacheStatistics::default());
        }

        Self {
            business_rules: Vec::new(),
            event_bus: None,
            config,
            cache_tiers,
            lru_order,
            access_frequencies,
            statistics,
            access_patterns: HashMap::new(),
        }
    }

    /// Add a business rule to the service
    pub fn add_business_rule(&mut self, rule: Box<dyn OptimizationPipelineBusinessRule>) {
        self.business_rules.push(rule);
    }

    /// Set the event bus for publishing domain events
    pub fn set_event_bus(&mut self, event_bus: Arc<DomainEventBus>) {
        self.event_bus = Some(event_bus);
    }

    /// Get a cache entry from the appropriate tier
    pub fn get(&mut self, key: u64) -> VmResult<Option<Vec<u8>>> {
        // Validate business rules
        for rule in &self.business_rules {
            if let Err(e) = rule.validate_pipeline_config(&self.create_pipeline_config()) {
                return Err(e);
            }
        }

        // Search from L1 to L3
        for tier in &self.config.tiers {
            if let Some(entry) = self.cache_tiers.get_mut(&tier.name).unwrap().get_mut(&key) {
                // Update access metadata
                entry.access_count += 1;
                entry.last_access = std::time::Instant::now();

                // Update LRU order
                let lru_order = self.lru_order.get_mut(&tier.name).unwrap();
                if let Some(pos) = lru_order.iter().position(|&k| k == key) {
                    lru_order.remove(pos);
                }
                lru_order.push_back(key);

                // Update access frequencies
                let frequencies = self.access_frequencies.get_mut(&tier.name).unwrap();
                *frequencies.entry(key).or_insert(0) += 1;

                // Update statistics
                let stats = self.statistics.get_mut(&tier.name).unwrap();
                stats.total_accesses += 1;
                stats.hits += 1;

                // Promote to higher tier if needed
                if tier.name != "L1" && self.should_promote(entry, tier) {
                    self.promote_entry(key, &tier.name)?;
                }

                // Publish cache hit event
                self.publish_optimization_event(OptimizationEvent::CacheHit {
                    tier: tier.name.clone(),
                    key,
                    size: entry.size,
                })?;

                return Ok(Some(entry.data.clone()));
            } else {
                // Update statistics for miss
                let stats = self.statistics.get_mut(&tier.name).unwrap();
                stats.total_accesses += 1;
                stats.misses += 1;
            }
        }

        // Publish cache miss event
        self.publish_optimization_event(OptimizationEvent::CacheMiss { key })?;

        Ok(None)
    }

    /// Put a cache entry in the appropriate tier
    pub fn put(&mut self, key: u64, data: Vec<u8>, priority: u32) -> VmResult<()> {
        // Validate business rules
        for rule in &self.business_rules {
            if let Err(e) = rule.validate_pipeline_config(&self.create_pipeline_config()) {
                return Err(e);
            }
        }

        let entry = CacheEntry {
            key,
            data: data.clone(),
            size: data.len(),
            access_count: 1,
            last_access: std::time::Instant::now(),
            created_at: std::time::Instant::now(),
            priority,
            pinned: false,
        };

        // Try to put in L1 first, then L2, then L3
        for tier in &self.config.tiers {
            if self.can_fit_in_tier(&entry, &tier.name) {
                self.put_in_tier(entry.clone(), &tier.name)?;
                
                // Publish cache put event
                self.publish_optimization_event(OptimizationEvent::CachePut {
                    tier: tier.name.clone(),
                    key,
                    size: entry.size,
                })?;
                
                return Ok(());
            }
        }

        // If we can't fit in any tier, evict from L3 and try again
        if let Some(l3_tier) = self.config.tiers.last() {
            self.evict_from_tier(&l3_tier.name, entry.size)?;
            self.put_in_tier(entry, &l3_tier.name)?;
            
            // Publish cache put event
            self.publish_optimization_event(OptimizationEvent::CachePut {
                tier: l3_tier.name.clone(),
                key,
                size: entry.size,
            })?;
        }

        Ok(())
    }

    /// Prefetch cache entries based on access patterns
    pub fn prefetch(&mut self, base_key: u64) -> VmResult<Vec<u64>> {
        let mut prefetched_keys = Vec::new();

        // Detect access pattern
        let pattern = self.detect_access_pattern(base_key)?;
        
        // Store the detected pattern
        self.access_patterns.insert(format!("key_{}", base_key), pattern.clone());

        // Prefetch based on pattern and strategy
        for tier in &self.config.tiers {
            match tier.prefetch_strategy {
                PrefetchStrategy::None => continue,
                PrefetchStrategy::Always => {
                    for i in 1..=self.config.max_prefetch_distance {
                        let prefetch_key = base_key + i as u64;
                        if self.should_prefetch(prefetch_key, &tier.name) {
                            // In a real implementation, we would fetch the actual data
                            // For now, we'll just record the prefetch
                            prefetched_keys.push(prefetch_key);
                        }
                    }
                },
                PrefetchStrategy::PatternBased => {
                    if pattern.confidence >= self.config.min_prefetch_confidence {
                        match pattern.pattern_type {
                            AccessPatternType::Sequential => {
                                for i in 1..=self.config.max_prefetch_distance {
                                    let prefetch_key = base_key + i as u64;
                                    if self.should_prefetch(prefetch_key, &tier.name) {
                                        prefetched_keys.push(prefetch_key);
                                    }
                                }
                            },
                            AccessPatternType::Strided => {
                                if let Some(stride) = pattern.parameters.get("stride") {
                                    for i in 1..=self.config.max_prefetch_distance {
                                        let prefetch_key = base_key + (i as f64 * stride) as u64;
                                        if self.should_prefetch(prefetch_key, &tier.name) {
                                            prefetched_keys.push(prefetch_key);
                                        }
                                    }
                                }
                            },
                            _ => {}
                        }
                    }
                },
                PrefetchStrategy::Adaptive => {
                    // Adaptive prefetching based on recent hit rates
                    let stats = self.statistics.get(&tier.name).unwrap();
                    if stats.hit_rate() > 0.8 && pattern.confidence >= self.config.min_prefetch_confidence {
                        // High hit rate, be more aggressive with prefetching
                        for i in 1..=self.config.max_prefetch_distance {
                            let prefetch_key = base_key + i as u64;
                            if self.should_prefetch(prefetch_key, &tier.name) {
                                prefetched_keys.push(prefetch_key);
                            }
                        }
                    }
                },
                PrefetchStrategy::HardwareAssisted => {
                    // In a real implementation, this would use hardware prefetching
                    // For now, we'll use a simple sequential prefetch
                    for i in 1..=self.config.max_prefetch_distance {
                        let prefetch_key = base_key + i as u64;
                        if self.should_prefetch(prefetch_key, &tier.name) {
                            prefetched_keys.push(prefetch_key);
                        }
                    }
                },
            }
        }

        // Publish prefetch event
        if !prefetched_keys.is_empty() {
            self.publish_optimization_event(OptimizationEvent::CachePrefetch {
                base_key,
                prefetched_keys: prefetched_keys.clone(),
                pattern: format!("{:?}", pattern.pattern_type),
                confidence: pattern.confidence,
            })?;
        }

        Ok(prefetched_keys)
    }

    /// Get cache statistics for all tiers
    pub fn get_statistics(&self) -> HashMap<String, CacheStatistics> {
        self.statistics.clone()
    }

    /// Resize a cache tier
    pub fn resize_tier(&mut self, tier_name: &str, new_capacity: usize) -> VmResult<()> {
        // Find the tier configuration
        let tier_config = self.config.tiers.iter_mut()
            .find(|t| t.name == tier_name)
            .ok_or_else(|| VmError::Core(crate::CoreError::InvalidConfig {
                message: format!("Tier not found: {}", tier_name),
            }))?;

        let old_capacity = tier_config.capacity;
        tier_config.capacity = new_capacity;

        // If shrinking, evict entries as needed
        if new_capacity < old_capacity {
            let current_usage = self.get_tier_usage(tier_name);
            if current_usage > new_capacity {
                let to_evict = current_usage - new_capacity;
                self.evict_from_tier_by_size(tier_name, to_evict)?;
            }
        }

        // Publish resize event
        self.publish_optimization_event(OptimizationEvent::CacheResized {
            tier: tier_name.to_string(),
            old_capacity,
            new_capacity,
        })?;

        Ok(())
    }

    /// Check if an entry should be promoted to a higher tier
    fn should_promote(&self, entry: &CacheEntry, tier: &CacheTierConfig) -> bool {
        if tier.name == "L1" {
            return false; // Already at highest tier
        }

        // Check promotion threshold
        let access_frequency = entry.access_count as f64 / 
            (std::time::Instant::now().duration_since(entry.created_at).as_secs_f64() + 1.0);
        
        access_frequency >= tier.promotion_threshold
    }

    /// Promote an entry to a higher tier
    fn promote_entry(&mut self, key: u64, from_tier: &str) -> VmResult<()> {
        // Find the current tier index
        let tier_index = self.config.tiers.iter()
            .position(|t| t.name == from_tier)
            .ok_or_else(|| VmError::Core(crate::CoreError::InvalidConfig {
                message: format!("Tier not found: {}", from_tier),
            }))?;

        // Can't promote from L1
        if tier_index == 0 {
            return Ok(());
        }

        // Get the entry
        let entry = self.cache_tiers.get_mut(from_tier).unwrap()
            .remove(&key)
            .ok_or_else(|| VmError::Core(crate::CoreError::InvalidState {
                message: format!("Entry not found in tier: {}", from_tier),
                current: format!("key {}", key),
                expected: "existing entry".to_string(),
            }))?;

        // Remove from LRU order
        let lru_order = self.lru_order.get_mut(from_tier).unwrap();
        if let Some(pos) = lru_order.iter().position(|&k| k == key) {
            lru_order.remove(pos);
        }

        // Get the target tier
        let target_tier = &self.config.tiers[tier_index - 1];
        
        // Make sure it fits in the target tier
        if !self.can_fit_in_tier(&entry, &target_tier.name) {
            // Evict from target tier if needed
            self.evict_from_tier(&target_tier.name, entry.size)?;
        }

        // Put in target tier
        self.put_in_tier(entry, &target_tier.name)?;

        // Update statistics
        let stats = self.statistics.get_mut(from_tier).unwrap();
        stats.promotions += 1;

        // Publish promotion event
        self.publish_optimization_event(OptimizationEvent::CachePromotion {
            from_tier: from_tier.to_string(),
            to_tier: target_tier.name.clone(),
            key,
        })?;

        Ok(())
    }

    /// Check if an entry fits in a tier
    fn can_fit_in_tier(&self, entry: &CacheEntry, tier_name: &str) -> bool {
        let _tier_config = self.config.tiers.iter()
            .find(|t| t.name == tier_name)
            .unwrap();

        let current_usage = self.get_tier_usage(tier_name);
        current_usage + entry.size <= tier_config.capacity
    }

    /// Put an entry in a specific tier
    fn put_in_tier(&mut self, entry: CacheEntry, tier_name: &str) -> VmResult<()> {
        let _tier_config = self.config.tiers.iter()
            .find(|t| t.name == tier_name)
            .unwrap();

        // Evict if needed
        if !self.can_fit_in_tier(&entry, tier_name) {
            self.evict_from_tier(tier_name, entry.size)?;
        }

        // Insert entry
        self.cache_tiers.get_mut(tier_name).unwrap().insert(entry.key, entry.clone());
        
        // Update LRU order
        self.lru_order.get_mut(tier_name).unwrap().push_back(entry.key);
        
        // Update access frequencies
        self.access_frequencies.get_mut(tier_name).unwrap().insert(entry.key, 1);

        Ok(())
    }

    /// Evict entries from a tier to make space
    fn evict_from_tier(&mut self, tier_name: &str, needed_space: usize) -> VmResult<()> {
        let _tier_config = self.config.tiers.iter()
            .find(|t| t.name == tier_name)
            .unwrap();

        let mut freed_space = 0;
        let cache = self.cache_tiers.get_mut(tier_name).unwrap();
        let lru_order = self.lru_order.get_mut(tier_name).unwrap();

        while freed_space < needed_space && !lru_order.is_empty() {
            let key_to_evict = match tier_config.policy {
                CachePolicy::LRU => lru_order.front().cloned(),
                CachePolicy::LFU => {
                    let frequencies = self.access_frequencies.get(tier_name).unwrap();
                    frequencies.iter()
                        .min_by_key(|(_, &count)| count)
                        .map(|(&key, _)| key)
                },
                CachePolicy::FIFO => lru_order.front().cloned(),
                CachePolicy::Adaptive => {
                    // Use a combination of LRU and LFU
                    let frequencies = self.access_frequencies.get(tier_name).unwrap();
                    lru_order.iter()
                        .min_by_key(|&key| {
                            let freq = frequencies.get(key).unwrap_or(&1);
                            // Lower score means higher eviction priority
                            *freq as f64 / (cache.get(key).map(|e| e.priority).unwrap_or(1) as f64 + 1.0)
                        })
                        .cloned()
                },
                CachePolicy::Random => {
                    let keys: Vec<u64> = cache.keys().cloned().collect();
                    if keys.is_empty() {
                        None
                    } else {
                        Some(keys[keys.len() / 2]) // Simple deterministic selection
                    }
                },
            };

            if let Some(key) = key_to_evict {
                if let Some(entry) = cache.remove(&key) {
                    freed_space += entry.size;
                    
                    // Remove from LRU order
                    if let Some(pos) = lru_order.iter().position(|&k| k == key) {
                        lru_order.remove(pos);
                    }
                    
                    // Remove from access frequencies
                    self.access_frequencies.get_mut(tier_name).unwrap().remove(&key);
                    
                    // Update statistics
                    let stats = self.statistics.get_mut(tier_name).unwrap();
                    stats.evictions += 1;
                    
                    // Publish eviction event
                    self.publish_optimization_event(OptimizationEvent::CacheEviction {
                        tier: tier_name.to_string(),
                        key,
                        size: entry.size,
                    })?;
                }
            } else {
                break;
            }
        }

        Ok(())
    }

    /// Evict entries from a tier by size
    fn evict_from_tier_by_size(&mut self, tier_name: &str, size_to_evict: usize) -> VmResult<()> {
        self.evict_from_tier(tier_name, size_to_evict)
    }

    /// Get current usage of a tier
    fn get_tier_usage(&self, tier_name: &str) -> usize {
        self.cache_tiers.get(tier_name).unwrap()
            .values()
            .map(|entry| entry.size)
            .sum()
    }

    /// Detect access pattern for a given key
    fn detect_access_pattern(&self, _key: u64) -> VmResult<AccessPattern> {
        // In a real implementation, this would analyze recent access history
        // For now, we'll return a simple sequential pattern with moderate confidence
        Ok(AccessPattern {
            pattern_type: AccessPatternType::Sequential,
            parameters: {
                let mut params = HashMap::new();
                params.insert("stride".to_string(), 1.0);
                params
            },
            confidence: 0.7,
        })
    }

    /// Check if a key should be prefetched
    fn should_prefetch(&self, key: u64, tier_name: &str) -> bool {
        // Check if the key is already cached
        !self.cache_tiers.get(tier_name).unwrap().contains_key(&key)
    }

    /// Create a pipeline configuration from the cache management config
    fn create_pipeline_config(&self) -> crate::domain_services::optimization_pipeline_service::OptimizationPipelineConfig {
        crate::domain_services::optimization_pipeline_service::OptimizationPipelineConfig {
            enable_instruction_scheduling: true,
            enable_loop_optimization: true,
            enable_constant_folding: true,
            enable_dead_code_elimination: true,
            enable_common_subexpression_elimination: true,
            enable_register_allocation: true,
            optimization_level: 2,
            max_inline_size: 50,
            loop_unroll_factor: 4,
            enable_vectorization: true,
        }
    }

    /// Publish an optimization event
    fn publish_optimization_event(&self, event: OptimizationEvent) -> VmResult<()> {
        if let Some(ref event_bus) = self.event_bus {
            let domain_event = DomainEventEnum::Optimization(event);
            event_bus.publish(domain_event)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_put_and_get() {
        let config = CacheManagementConfig::default();
        let mut service = CacheManagementDomainService::new(config);
        
        let key = 0x1000;
        let data = vec![1, 2, 3, 4, 5];
        
        // Put data in cache
        service.put(key, data.clone(), 50).unwrap();
        
        // Get data from cache
        let result = service.get(key).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), data);
    }

    #[test]
    fn test_cache_miss() {
        let config = CacheManagementConfig::default();
        let mut service = CacheManagementDomainService::new(config);
        
        let key = 0x1000;
        
        // Get data that doesn't exist
        let result = service.get(key).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_cache_statistics() {
        let config = CacheManagementConfig::default();
        let mut service = CacheManagementDomainService::new(config);
        
        let key = 0x1000;
        let data = vec![1, 2, 3, 4, 5];
        
        // Put data in cache
        service.put(key, data.clone(), 50).unwrap();
        
        // Get data from cache (hit)
        service.get(key).unwrap();
        
        // Get non-existent data (miss)
        service.get(0x2000).unwrap();
        
        // Check statistics
        let stats = service.get_statistics();
        let l1_stats = stats.get("L1").unwrap();
        assert_eq!(l1_stats.total_accesses, 2);
        assert_eq!(l1_stats.hits, 1);
        assert_eq!(l1_stats.misses, 1);
        assert_eq!(l1_stats.hit_rate(), 0.5);
    }

    #[test]
    fn test_prefetch() {
        let config = CacheManagementConfig::default();
        let mut service = CacheManagementDomainService::new(config);
        
        let base_key = 0x1000;
        
        // Prefetch based on access pattern
        let prefetched = service.prefetch(base_key).unwrap();
        
        // Should prefetch some keys based on the detected pattern
        assert!(!prefetched.is_empty());
    }

    #[test]
    fn test_tier_resize() {
        let config = CacheManagementConfig::default();
        let mut service = CacheManagementDomainService::new(config);
        
        // Resize L1 tier
        service.resize_tier("L1", 64 * 1024).unwrap();
        
        // Check that the tier was resized
        let tier_config = service.config.tiers.iter()
            .find(|t| t.name == "L1")
            .unwrap();
        assert_eq!(tier_config.capacity, 64 * 1024);
    }
}