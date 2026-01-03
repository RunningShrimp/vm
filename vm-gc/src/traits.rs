//! GC strategy traits

use crate::{GcConfig, GcResult};

/// GC collection policy
///
/// Defines when and how garbage collection should be triggered
pub trait GcPolicy: Send + Sync {
    /// Check if collection should be triggered
    fn should_collect(&self, config: &GcConfig) -> bool;

    /// Get policy name
    fn name(&self) -> &str;
}

/// GC strategy trait
///
/// Implementations of this trait provide different garbage collection algorithms
pub trait GcStrategy: Send + Sync {
    /// Perform a garbage collection cycle
    ///
    /// # Errors
    ///
    /// Returns an error if collection fails
    fn collect(&mut self) -> GcResult<()>;

    /// Allocate memory for a new object
    ///
    /// # Errors
    ///
    /// Returns an error if allocation fails
    fn allocate(&mut self, size: usize) -> GcResult<*mut u8>;

    /// Check if collection should be triggered
    fn should_collect(&self, config: &GcConfig) -> bool;

    /// Get current heap size
    fn heap_size(&self) -> usize;

    /// Get the strategy name
    fn name(&self) -> &str;
}

/// Simple generational GC policy
///
/// Triggers GC based on heap size threshold
#[derive(Debug, Clone)]
pub struct GenerationalPolicy {
    /// Current heap size
    heap_size: usize,
}

impl GenerationalPolicy {
    /// Create a new generational policy
    pub fn new() -> Self {
        Self { heap_size: 0 }
    }

    /// Update heap size
    pub fn update_heap_size(&mut self, size: usize) {
        self.heap_size = size;
    }
}

impl Default for GenerationalPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl GcPolicy for GenerationalPolicy {
    fn should_collect(&self, config: &GcConfig) -> bool {
        self.heap_size >= config.heap_threshold
    }

    fn name(&self) -> &str {
        "generational"
    }
}

/// Simple incremental GC policy
///
/// Triggers GC based on allocation rate
#[derive(Debug, Clone)]
pub struct IncrementalPolicy {
    /// Allocations since last collection
    allocations_since_gc: usize,

    /// Allocation threshold
    allocation_threshold: usize,
}

impl IncrementalPolicy {
    /// Create a new incremental policy
    pub fn new(threshold: usize) -> Self {
        Self {
            allocations_since_gc: 0,
            allocation_threshold: threshold,
        }
    }

    /// Record an allocation
    pub fn record_allocation(&mut self, size: usize) {
        self.allocations_since_gc += size;
    }

    /// Reset after collection
    pub fn reset(&mut self) {
        self.allocations_since_gc = 0;
    }
}

impl Default for IncrementalPolicy {
    fn default() -> Self {
        Self::new(1024 * 1024) // 1MB default
    }
}

impl GcPolicy for IncrementalPolicy {
    fn should_collect(&self, _config: &GcConfig) -> bool {
        self.allocations_since_gc >= self.allocation_threshold
    }

    fn name(&self) -> &str {
        "incremental"
    }
}

/// Adaptive GC policy
///
/// Combines multiple policies and adapts based on performance
#[derive(Debug, Clone)]
pub struct AdaptivePolicy {
    /// Generational policy
    generational: GenerationalPolicy,

    /// Incremental policy
    incremental: IncrementalPolicy,

    /// Active policy
    active: PolicyType,
}

/// Which policy is currently active
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyType {
    /// Generational policy active
    Generational,

    /// Incremental policy active
    Incremental,
}

impl AdaptivePolicy {
    /// Create a new adaptive policy
    pub fn new() -> Self {
        Self {
            generational: GenerationalPolicy::new(),
            incremental: IncrementalPolicy::new(1000), // Default threshold: 1000 allocations
            active: PolicyType::Generational,
        }
    }

    /// Get the active policy type
    pub fn active_policy(&self) -> PolicyType {
        self.active
    }

    /// Switch to a different policy
    pub fn switch_policy(&mut self, policy: PolicyType) {
        self.active = policy;
    }
}

impl Default for AdaptivePolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl GcPolicy for AdaptivePolicy {
    fn should_collect(&self, config: &GcConfig) -> bool {
        match self.active {
            PolicyType::Generational => self.generational.should_collect(config),
            PolicyType::Incremental => self.incremental.should_collect(config),
        }
    }

    fn name(&self) -> &str {
        match self.active {
            PolicyType::Generational => "adaptive-generational",
            PolicyType::Incremental => "adaptive-incremental",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generational_policy() {
        let policy = GenerationalPolicy::new();
        let mut config = GcConfig::default();
        config.heap_threshold = 1024;

        assert!(!policy.should_collect(&config));

        let mut policy = policy;
        policy.update_heap_size(2048);
        assert!(policy.should_collect(&config));
    }

    #[test]
    fn test_incremental_policy() {
        let mut policy = IncrementalPolicy::new(1000);
        let config = GcConfig::default();

        assert!(!policy.should_collect(&config));

        policy.record_allocation(1500);
        assert!(policy.should_collect(&config));

        policy.reset();
        assert!(!policy.should_collect(&config));
    }

    #[test]
    fn test_adaptive_policy() {
        let mut policy = AdaptivePolicy::new();
        assert_eq!(policy.active_policy(), PolicyType::Generational);

        policy.switch_policy(PolicyType::Incremental);
        assert_eq!(policy.active_policy(), PolicyType::Incremental);
    }
}
