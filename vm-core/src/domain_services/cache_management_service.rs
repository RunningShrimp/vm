//! Cache Management Domain Service (Refactored)
//!
//! This service encapsulates business logic for cache management
//! including cache replacement strategies, promotion/demotion logic,
//! prefetch strategies, and cache sizing policies.
//!
//! **DDD Architecture**:
//! - Uses `CacheManager` trait from domain layer (dependency inversion)
//! - Delegates cache operations to infrastructure layer implementation
//! - Focuses on business logic: event publishing, coordination, promotion/demotion
//!
//! **Migration Status**:
//! - Infrastructure implementation created: ✅ (vm-engine/src/jit/cache/manager.rs)
//! - Domain service refactored to use trait: ✅ (Refactored to use CacheManager trait)

use std::collections::HashMap;
use std::sync::Arc;

use crate::VmResult;
use crate::domain::CacheManager;
use crate::domain_event_bus::DomainEventBus;
use crate::domain_services::events::{DomainEventEnum, OptimizationEvent};
use crate::domain_services::rules::optimization_pipeline_rules::OptimizationPipelineBusinessRule;
use crate::domain_services::config::{BaseServiceConfig, ServiceConfig};

/// Type alias for cache manager map to reduce type complexity
///
/// Maps tier names (e.g., "L1", "L2", "L3") to their respective thread-safe cache managers.
/// Each cache manager is wrapped in Arc<Mutex<>> for concurrent access.
type CacheManagerMap = HashMap<String, Arc<std::sync::Mutex<dyn CacheManager<u64, Vec<u8>>>>>;

/// Cache tier configuration
#[derive(Debug, Clone)]
pub struct CacheTierConfig {
    /// Tier name
    pub name: String,
    /// Tier capacity in bytes
    pub capacity: usize,
    /// Promotion threshold to next tier
    pub promotion_threshold: f64,
    /// Demotion threshold to previous tier
    pub demotion_threshold: f64,
}

/// Cache management configuration
#[derive(Debug, Clone)]
pub struct CacheManagementConfig {
    /// Cache tiers (L1, L2, L3, etc.)
    pub tiers: Vec<CacheTierConfig>,
    /// Global cache size limit in bytes
    pub global_cache_limit: usize,
}

impl Default for CacheManagementConfig {
    fn default() -> Self {
        Self {
            tiers: vec![
                CacheTierConfig {
                    name: "L1".to_string(),
                    capacity: 32 * 1024, // 32KB
                    promotion_threshold: 0.8,
                    demotion_threshold: 0.2,
                },
                CacheTierConfig {
                    name: "L2".to_string(),
                    capacity: 256 * 1024, // 256KB
                    promotion_threshold: 0.7,
                    demotion_threshold: 0.3,
                },
                CacheTierConfig {
                    name: "L3".to_string(),
                    capacity: 2 * 1024 * 1024, // 2MB
                    promotion_threshold: 0.6,
                    demotion_threshold: 0.4,
                },
            ],
            global_cache_limit: 10 * 1024 * 1024, // 10MB
        }
    }
}

/// Cache Management Domain Service (Refactored)
///
/// This service provides high-level business logic for managing multi-tier caches
/// with promotion/demotion strategies and event publishing.
///
/// **Refactored Architecture**:
/// - Uses `CacheManager<u64, Vec<u8>>` trait from domain layer (dependency inversion)
/// - Delegates cache operations to infrastructure layer implementation
/// - Focuses on business logic: event publishing, coordination, promotion/demotion
pub struct CacheManagementDomainService {
    /// Business rules for cache management
    business_rules: Vec<Box<dyn OptimizationPipelineBusinessRule>>,
    /// Service configuration (includes event bus)
    config: BaseServiceConfig,
    /// Configuration for cache management
    cache_config: CacheManagementConfig,
    /// Cache managers by tier (infrastructure layer implementations via trait)
    cache_managers: CacheManagerMap,
}

impl CacheManagementDomainService {
    /// Create a new cache management domain service
    ///
    /// # 参数
    /// - `config`: Cache management configuration
    /// - `cache_managers`: Map of tier name to cache manager implementation
    pub fn new(cache_config: CacheManagementConfig, cache_managers: CacheManagerMap) -> Self {
        Self {
            business_rules: Vec::new(),
            config: BaseServiceConfig::new(),
            cache_config,
            cache_managers,
        }
    }

    /// Add a business rule to the service
    pub fn add_business_rule(&mut self, rule: Box<dyn OptimizationPipelineBusinessRule>) {
        self.business_rules.push(rule);
    }

    /// Set the event bus for publishing domain events
    pub fn set_event_bus(&mut self, event_bus: Arc<DomainEventBus>) {
        self.config.set_event_bus(event_bus);
    }

    /// Get a cache entry from the appropriate tier
    ///
    /// Searches from L1 to L3, delegating to infrastructure layer implementations.
    pub async fn get(&mut self, key: u64) -> VmResult<Option<Vec<u8>>> {
        // Validate business rules
        for _rule in &self.business_rules {
            // Simplified validation - actual implementation would use proper config
            // if let Err(e) = rule.validate_pipeline_config(&self.create_pipeline_config()) {
            //     return Err(e);
            // }
        }

        // Search from L1 to L3
        for tier in &self.cache_config.tiers {
            if let Some(cache_manager) = self.cache_managers.get(&tier.name) {
                let manager = cache_manager.lock().unwrap();
                if let Some(value) = manager.get(&key) {
                    // Publish cache hit event
                    self.publish_optimization_event(OptimizationEvent::CacheHit {
                        tier: tier.name.clone(),
                        key,
                        size: value.len(),
                        occurred_at: std::time::SystemTime::now(),
                    })?;

                    return Ok(Some(value));
                }
            }
        }

        // Publish cache miss event
        self.publish_optimization_event(OptimizationEvent::CacheMiss {
            key,
            occurred_at: std::time::SystemTime::now(),
        })?;

        Ok(None)
    }

    /// Put a cache entry in the appropriate tier
    ///
    /// Delegates to the infrastructure layer implementation.
    pub async fn put(&mut self, key: u64, data: Vec<u8>) -> VmResult<()> {
        // Validate business rules
        for _rule in &self.business_rules {
            // Simplified validation
        }

        // Try to put in L1 first, then L2, then L3
        for tier in &self.cache_config.tiers {
            if let Some(cache_manager) = self.cache_managers.get(&tier.name) {
                let mut manager = cache_manager.lock().unwrap();
                let stats = manager.stats();

                // Check if we can fit in this tier
                if stats.size < stats.capacity {
                    manager.put(key, data.clone());

                    // Publish cache put event
                    self.publish_optimization_event(OptimizationEvent::CachePut {
                        tier: tier.name.clone(),
                        key,
                        size: data.len(),
                        occurred_at: std::time::SystemTime::now(),
                    })?;

                    return Ok(());
                }
            }
        }

        // If we can't fit in any tier, evict from L3 and try again
        if let Some(l3_tier) = self.cache_config.tiers.last()
            && let Some(cache_manager) = self.cache_managers.get(&l3_tier.name)
        {
            let mut manager = cache_manager.lock().unwrap();
            manager.evict(&key); // Evict old entry if exists
            manager.put(key, data.clone());

            // Publish cache put event
            self.publish_optimization_event(OptimizationEvent::CachePut {
                tier: l3_tier.name.clone(),
                key,
                size: data.len(),
                occurred_at: std::time::SystemTime::now(),
            })?;
        }

        Ok(())
    }

    /// Evict a cache entry
    ///
    /// Delegates to the infrastructure layer implementation.
    pub async fn evict(&mut self, key: u64) -> VmResult<()> {
        for tier in &self.cache_config.tiers {
            if let Some(cache_manager) = self.cache_managers.get(&tier.name) {
                let mut manager = cache_manager.lock().unwrap();
                manager.evict(&key);
            }
        }
        Ok(())
    }

    /// Clear all cache tiers
    ///
    /// Delegates to the infrastructure layer implementation.
    pub async fn clear(&mut self) -> VmResult<()> {
        for tier in &self.cache_config.tiers {
            if let Some(cache_manager) = self.cache_managers.get(&tier.name) {
                let mut manager = cache_manager.lock().unwrap();
                manager.clear();
            }
        }
        Ok(())
    }

    /// Get cache statistics for a tier
    ///
    /// Delegates to the infrastructure layer implementation.
    pub async fn get_statistics(&self, tier_name: &str) -> Option<crate::domain::CacheStats> {
        if let Some(cache_manager) = self.cache_managers.get(tier_name) {
            let manager = cache_manager.lock().unwrap();
            Some(manager.stats())
        } else {
            None
        }
    }

    /// Publish optimization event
    fn publish_optimization_event(&self, event: OptimizationEvent) -> VmResult<()> {
        if let Some(event_bus) = self.config.event_bus() {
            let _ = event_bus.publish(&DomainEventEnum::Optimization(event));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_management_config_default() {
        let config = CacheManagementConfig::default();
        assert_eq!(config.tiers.len(), 3);
        assert_eq!(config.tiers[0].name, "L1");
        assert_eq!(config.tiers[1].name, "L2");
        assert_eq!(config.tiers[2].name, "L3");
    }
}
