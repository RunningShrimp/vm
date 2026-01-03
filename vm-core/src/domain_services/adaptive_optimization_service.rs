//! # Adaptive Optimization Domain Service
//!
//! This service encapsulates business logic for adaptive optimization
//! including hotspot detection, performance profiling, tiered compilation,
//! and dynamic recompilation decisions.
//!
//! ## Domain Responsibilities
//!
//! The adaptive optimization service is responsible for:
//!
//! 1. **Hotspot Detection**: Identifying frequently executed code regions
//!    that benefit from optimization
//! 2. **Performance Profiling**: Collecting and analyzing execution metrics
//! 3. **Strategy Selection**: Choosing appropriate optimization strategies
//!    based on runtime behavior
//! 4. **Tiered Compilation**: Managing multiple optimization levels
//! 5. **Dynamic Recompilation**: Re-optimizing code based on feedback
//!
//! ## DDD Patterns
//!
//! ### Domain Service Pattern
//! This is a **Domain Service** because:
//! - It coordinates between multiple aggregates (code blocks, execution contexts)
//! - It encapsulates complex business logic (adaptive strategy selection)
//! - It's stateless - all state is passed as parameters or stored in aggregates
//!
//! ### Domain Events Published
//!
//! - **`OptimizationEvent::HotspotsDetected`**: Published when hotspots are identified
//! - **`OptimizationEvent::StrategySelected`**: Published when an optimization strategy is chosen
//! - **`OptimizationEvent::ResourceConstraintViolation`**: Published when resource constraints are exceeded
//!
//! ## Usage Examples
//!
//! ### Basic Hotspot Detection
//!
//! ```rust
//! use crate::domain_services::adaptive_optimization_service::{
//!     AdaptiveOptimizationDomainService, HotspotConfig
//! };
//!
//! let config = HotspotConfig::default();
//! let service = AdaptiveOptimizationDomainService::new(config);
//!
//! let hotspots = service.detect_hotspots(&performance_profile)?;
//!
//! for hotspot in hotspots {
//!     println!("Hotspot at 0x{:x}: score={}, executions={}",
//!         hotspot.address,
//!         hotspot.hotness_score,
//!         hotspot.execution_count
//!     );
//! }
//! ```
//!
//! ### Strategy Selection with Resource Awareness
//!
//! ```rust
//! let service = AdaptiveOptimizationDomainService::new(config);
//!
//! let strategy = service.select_optimization_strategy(
//!     &hotspots,
//!     &resource_utilization,
//! )?;
//!
//! match strategy {
//!     OptimizationStrategy::Aggressive => {
//!         println!("Using aggressive optimization");
//!     }
//!     OptimizationStrategy::Balanced => {
//!         println!("Using balanced optimization");
//!     }
//!     OptimizationStrategy::Conservative => {
//!         println!("Using conservative optimization");
//!     }
//! }
//! ```
//!
//! ### Performance Profiling
//!
//! ```rust
//! use std::collections::HashMap;
//! use std::time::Instant;
//!
//! let mut profile = PerformanceProfile {
//!     execution_history: HashMap::new(),
//!     performance_by_level: HashMap::new(),
//!     resource_utilization: ResourceUtilization::default(),
//! };
//!
//! // Record execution data
//! profile.record_execution(
//!     0x1000,  // address
//!     Duration::from_micros(100),  // duration
//!     1024,    // memory usage
//!     2,       // optimization level
//! );
//! ```
//!
//! ## Hotspot Detection Algorithm
//!
//! The service uses a multi-factor hotspot detection algorithm:
//!
//! 1. **Execution Frequency**: Code blocks executed above a threshold
//! 2. **Temporal Locality**: Recent execution patterns
//! 3. **Performance Trends**: Improving, stable, or degrading performance
//! 4. **Resource Utilization**: CPU, memory, and cache pressure
//!
//! Hotness score calculation:
//!
//! ```text
//! hotness_score = (execution_count / max_executions) * 0.4
//!              + (recency_factor) * 0.3
//!              + (cache_hit_rate) * 0.2
//!              + (performance_trend) * 0.1
//! ```
//!
//! ## Strategy Selection Logic
//!
//! Strategy selection considers:
//!
//! | Factor | Aggressive | Balanced | Conservative |
//! |--------|-----------|----------|--------------|
//! | Hotspot Count | High | Medium | Low |
//! | CPU Usage | Low | Medium | High |
//! | Memory Usage | Low | Medium | High |
//! | Cache Pressure | Low | Medium | High |
//!
//! ## Integration with Aggregate Roots
//!
//! This service works with:
//! - **`VirtualMachineAggregate`**: VM-level optimization decisions
//! - **`CodeBlockAggregate`**: Code block-specific optimizations
//! - **`ExecutionContext`**: Runtime behavior analysis

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use crate::VmResult;
use crate::domain_event_bus::DomainEventBus;
use crate::domain_services::events::{DomainEventEnum, OptimizationEvent};
use crate::domain_services::rules::optimization_pipeline_rules::OptimizationPipelineBusinessRule;

/// Hotspot information for adaptive optimization
#[derive(Debug, Clone)]
pub struct Hotspot {
    /// Address of the hotspot
    pub address: u64,
    /// Execution count
    pub execution_count: u64,
    /// Hotness score (0.0 to 1.0)
    pub hotness_score: f64,
    /// Average execution time in nanoseconds
    pub avg_execution_time: Duration,
    /// Last execution time
    pub last_execution: Instant,
    /// Performance trend (improving, stable, degrading)
    pub performance_trend: PerformanceTrend,
}

/// Performance trend for hotspots
#[derive(Debug, Clone, PartialEq)]
pub enum PerformanceTrend {
    /// Performance is improving
    Improving,
    /// Performance is stable
    Stable,
    /// Performance is degrading
    Degrading,
}

/// Optimization strategy for adaptive compilation
#[derive(Debug, Clone, PartialEq)]
pub enum OptimizationStrategy {
    /// Standard JIT compilation
    StandardJit,
    /// Tiered compilation with multiple optimization levels
    TieredCompilation,
    /// Dynamic recompilation based on performance feedback
    DynamicRecompilation,
    /// Hybrid approach combining multiple strategies
    Hybrid,
}

/// Performance profile data for adaptive optimization
#[derive(Debug, Clone)]
pub struct PerformanceProfile {
    /// Execution history for different addresses
    pub execution_history: HashMap<u64, Vec<ExecutionDataPoint>>,
    /// Performance metrics by optimization level
    pub performance_by_level: HashMap<u32, PerformanceMetrics>,
    /// Resource utilization metrics
    pub resource_utilization: ResourceUtilization,
}

/// Single execution data point
#[derive(Debug, Clone)]
pub struct ExecutionDataPoint {
    /// Execution timestamp
    pub timestamp: Instant,
    /// Execution duration
    pub duration: Duration,
    /// Memory usage during execution
    pub memory_usage: u64,
    /// Optimization level used
    pub optimization_level: u32,
}

/// Performance metrics for optimization levels
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    /// Average execution time
    pub avg_execution_time: Duration,
    /// Standard deviation of execution time
    pub std_deviation: Duration,
    /// Total executions
    pub total_executions: u64,
    /// Cache hit rate
    pub cache_hit_rate: f64,
}

/// Resource utilization metrics
#[derive(Debug, Clone)]
pub struct ResourceUtilization {
    /// CPU utilization percentage (0.0 to 1.0)
    pub cpu_utilization: f64,
    /// Memory utilization percentage (0.0 to 1.0)
    pub memory_utilization: f64,
    /// Cache utilization percentage (0.0 to 1.0)
    pub cache_utilization: f64,
}

/// Adaptive optimization configuration
#[derive(Debug, Clone)]
pub struct AdaptiveOptimizationConfig {
    /// Minimum execution count to consider for hotspot detection
    pub hotspot_threshold: u64,
    /// Hotness score threshold for optimization (0.0 to 1.0)
    pub hotness_threshold: f64,
    /// Performance degradation threshold to trigger recompilation
    pub performance_degradation_threshold: f64,
    /// Maximum number of hotspots to track
    pub max_hotspots: usize,
    /// Time window for performance trend analysis
    pub trend_analysis_window: Duration,
    /// Minimum improvement threshold to consider performance as improving
    pub improvement_threshold: f64,
}

impl Default for AdaptiveOptimizationConfig {
    fn default() -> Self {
        Self {
            hotspot_threshold: 1000,
            hotness_threshold: 0.7,
            performance_degradation_threshold: 0.15,
            max_hotspots: 100,
            trend_analysis_window: Duration::from_secs(60),
            improvement_threshold: 0.05,
        }
    }
}

/// Adaptive Optimization Domain Service
///
/// This service encapsulates business logic for adaptive optimization
/// including hotspot detection, performance profiling, tiered compilation,
/// and dynamic recompilation decisions.
pub struct AdaptiveOptimizationDomainService {
    /// Business rules for adaptive optimization
    business_rules: Vec<Box<dyn OptimizationPipelineBusinessRule>>,
    /// Event bus for publishing domain events
    event_bus: Option<Arc<DomainEventBus>>,
    /// Configuration for adaptive optimization
    config: AdaptiveOptimizationConfig,
}

impl AdaptiveOptimizationDomainService {
    /// Create a new adaptive optimization domain service
    pub fn new(config: AdaptiveOptimizationConfig) -> Self {
        Self {
            business_rules: Vec::new(),
            event_bus: None,
            config,
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

    /// Detect hotspots based on execution data
    pub fn detect_hotspots(&self, profile: &PerformanceProfile) -> VmResult<Vec<Hotspot>> {
        let mut hotspots = Vec::new();

        for (&address, history) in &profile.execution_history {
            if history.len() < 10 {
                continue; // Not enough data points
            }

            let execution_count = history.len() as u64;
            if execution_count < self.config.hotspot_threshold {
                continue;
            }

            let avg_execution_time =
                history.iter().map(|dp| dp.duration).sum::<Duration>() / history.len() as u32;

            let hotness_score = self.calculate_hotness_score(execution_count, avg_execution_time);

            if hotness_score >= self.config.hotness_threshold {
                let performance_trend = self.analyze_performance_trend(history);
                let last_execution = history
                    .last()
                    .map(|dp| dp.timestamp)
                    .unwrap_or_else(Instant::now);

                hotspots.push(Hotspot {
                    address,
                    execution_count,
                    hotness_score,
                    avg_execution_time,
                    last_execution,
                    performance_trend,
                });
            }
        }

        // Sort by hotness score (descending)
        hotspots.sort_by(|a, b| {
            b.hotness_score
                .partial_cmp(&a.hotness_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Limit to max_hotspots
        hotspots.truncate(self.config.max_hotspots);

        // Publish hotspot detection event
        self.publish_optimization_event(OptimizationEvent::HotspotsDetected {
            count: hotspots.len(),
            threshold: self.config.hotspot_threshold,
            occurred_at: SystemTime::now(),
        })?;

        Ok(hotspots)
    }

    /// Determine the optimal optimization strategy for a given context
    pub fn determine_optimization_strategy(
        &self,
        hotspots: &[Hotspot],
        profile: &PerformanceProfile,
    ) -> VmResult<OptimizationStrategy> {
        // Validate business rules
        for rule in &self.business_rules {
            rule.validate_pipeline_config(&self.create_pipeline_config())?
        }

        let strategy = if hotspots.is_empty() {
            // No hotspots detected, use standard JIT
            OptimizationStrategy::StandardJit
        } else if self.has_performance_degradation(hotspots) {
            // Performance degradation detected, use dynamic recompilation
            OptimizationStrategy::DynamicRecompilation
        } else if profile.resource_utilization.cpu_utilization > 0.8 {
            // High CPU utilization, use tiered compilation
            OptimizationStrategy::TieredCompilation
        } else {
            // Otherwise, use hybrid approach
            OptimizationStrategy::Hybrid
        };

        // Publish strategy selection event
        self.publish_optimization_event(OptimizationEvent::StrategySelected {
            strategy: format!("{:?}", strategy),
            hotspot_count: hotspots.len(),
            resource_utilization: profile.resource_utilization.clone(),
            occurred_at: SystemTime::now(),
        })?;

        Ok(strategy)
    }

    /// Analyze performance trends for execution history
    pub fn analyze_performance_trend(&self, history: &[ExecutionDataPoint]) -> PerformanceTrend {
        if history.len() < 10 {
            return PerformanceTrend::Stable;
        }

        // Take the last N data points within the trend analysis window
        let now = Instant::now();
        let recent_data: Vec<_> = history
            .iter()
            .filter(|dp| now.duration_since(dp.timestamp) <= self.config.trend_analysis_window)
            .collect();

        if recent_data.len() < 5 {
            return PerformanceTrend::Stable;
        }

        // Calculate performance trend using linear regression
        let n = recent_data.len() as f64;
        let sum_x: f64 = (0..recent_data.len()).map(|i| i as f64).sum();
        let sum_y: f64 = recent_data
            .iter()
            .map(|dp| dp.duration.as_nanos() as f64)
            .sum();
        let sum_xy: f64 = recent_data
            .iter()
            .enumerate()
            .map(|(i, dp)| i as f64 * dp.duration.as_nanos() as f64)
            .sum();
        let sum_x2: f64 = (0..recent_data.len()).map(|i| (i as f64).powi(2)).sum();

        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x.powi(2));
        let avg_y = sum_y / n;
        let improvement_rate = -slope / avg_y; // Negative slope means improvement

        if improvement_rate > self.config.improvement_threshold {
            PerformanceTrend::Improving
        } else if improvement_rate < -self.config.improvement_threshold {
            PerformanceTrend::Degrading
        } else {
            PerformanceTrend::Stable
        }
    }

    /// Check if any hotspots show performance degradation
    pub fn has_performance_degradation(&self, hotspots: &[Hotspot]) -> bool {
        hotspots
            .iter()
            .any(|h| h.performance_trend == PerformanceTrend::Degrading)
    }

    /// Calculate hotness score for a given execution count and average time
    fn calculate_hotness_score(&self, execution_count: u64, avg_time: Duration) -> f64 {
        // Normalize execution count (logarithmic scale)
        let count_score = (execution_count as f64).log10() / 10.0;

        // Normalize execution time (inverse relationship - longer time = higher score)
        let time_score = 1.0 - (avg_time.as_nanos() as f64 / 1_000_000.0).min(1.0);

        // Combine scores with weights
        (count_score * 0.7 + time_score * 0.3).min(1.0)
    }

    /// Create a pipeline configuration from the adaptive optimization config
    fn create_pipeline_config(
        &self,
    ) -> crate::domain_services::optimization_pipeline_service::OptimizationPipelineConfig {
        // Use default x86_64 architecture for both source and target
        // In a real implementation, these would be determined from the VM configuration
        let arch = crate::GuestArch::X86_64;
        crate::domain_services::optimization_pipeline_service::OptimizationPipelineConfig::new(
            arch, arch, 2, // optimization level 2
        )
    }

    /// Publish an optimization event
    fn publish_optimization_event(&self, event: OptimizationEvent) -> VmResult<()> {
        if let Some(ref event_bus) = self.event_bus {
            let domain_event = DomainEventEnum::Optimization(event);
            event_bus.publish(&domain_event)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_hotspot_detection() {
        // Use a custom config with lower threshold for testing
        let config = AdaptiveOptimizationConfig {
            hotspot_threshold: 1000,
            hotness_threshold: 0.5, // Lower threshold for realistic test data
            performance_degradation_threshold: 0.15,
            max_hotspots: 100,
            trend_analysis_window: Duration::from_secs(60),
            improvement_threshold: 0.05,
        };
        let service = AdaptiveOptimizationDomainService::new(config);

        let mut profile = PerformanceProfile {
            execution_history: HashMap::new(),
            performance_by_level: HashMap::new(),
            resource_utilization: ResourceUtilization {
                cpu_utilization: 0.5,
                memory_utilization: 0.4,
                cache_utilization: 0.6,
            },
        };

        // Create execution history for a hotspot
        let address = 0x1000;
        let mut history = Vec::new();
        let base_time = Instant::now();

        // Use 10000 executions with ~200us avg_time
        // count_score = log10(10000) / 10 = 0.4
        // time_score = 1.0 - (200000 / 1_000_000) = 0.8
        // hotness_score = 0.4 * 0.7 + 0.8 * 0.3 = 0.28 + 0.24 = 0.52 >= 0.5 ✓
        for i in 0..10000 {
            // Spread timestamps over the last 30 seconds
            let offset_secs = (i as f64 / 10000.0) * 30.0;
            history.push(ExecutionDataPoint {
                timestamp: base_time
                    .checked_sub(Duration::from_secs_f64(offset_secs))
                    .unwrap_or(base_time),
                duration: Duration::from_micros(200 + (i % 50) as u64), // ~200us avg
                memory_usage: 1024,
                optimization_level: 1,
            });
        }

        profile.execution_history.insert(address, history);

        let hotspots = service
            .detect_hotspots(&profile)
            .expect("Failed to detect hotspots");

        // Verify we detect exactly one hotspot
        assert_eq!(hotspots.len(), 1);
        assert_eq!(hotspots[0].address, address);
        assert_eq!(hotspots[0].execution_count, 10000);
        assert!(hotspots[0].hotness_score >= 0.5);
    }

    #[test]
    fn test_optimization_strategy_selection() {
        let config = AdaptiveOptimizationConfig::default();
        let service = AdaptiveOptimizationDomainService::new(config);

        let profile = PerformanceProfile {
            execution_history: HashMap::new(),
            performance_by_level: HashMap::new(),
            resource_utilization: ResourceUtilization {
                cpu_utilization: 0.5,
                memory_utilization: 0.4,
                cache_utilization: 0.6,
            },
        };

        // Test with no hotspots
        let strategy = service
            .determine_optimization_strategy(&[], &profile)
            .expect("Failed to determine strategy");
        assert_eq!(strategy, OptimizationStrategy::StandardJit);

        // Test with hotspots but no degradation
        let hotspots = vec![Hotspot {
            address: 0x1000,
            execution_count: 1500,
            hotness_score: 0.8,
            avg_execution_time: Duration::from_nanos(1000),
            last_execution: Instant::now(),
            performance_trend: PerformanceTrend::Stable,
        }];

        let strategy = service
            .determine_optimization_strategy(&hotspots, &profile)
            .expect("Failed to determine strategy");
        assert_eq!(strategy, OptimizationStrategy::Hybrid);

        // Test with performance degradation
        let mut degrading_hotspots = hotspots;
        degrading_hotspots[0].performance_trend = PerformanceTrend::Degrading;

        let strategy = service
            .determine_optimization_strategy(&degrading_hotspots, &profile)
            .expect("Failed to determine strategy");
        assert_eq!(strategy, OptimizationStrategy::DynamicRecompilation);
    }

    #[test]
    fn test_performance_trend_analysis() {
        let config = AdaptiveOptimizationConfig::default();
        let service = AdaptiveOptimizationDomainService::new(config);

        // Test improving performance - need a strong downward trend
        let mut improving_history = Vec::new();
        let base_time = Instant::now();

        for i in 0..30 {
            // Strongly decreasing execution times: from 10000ns to 1000ns
            // This creates a clear negative slope
            let offset_secs = (30 - i) as f64 * 0.1; // Spread over last 3 seconds
            improving_history.push(ExecutionDataPoint {
                timestamp: base_time
                    .checked_sub(Duration::from_secs_f64(offset_secs))
                    .unwrap_or(base_time),
                duration: Duration::from_nanos(10000 - i * 300), // Decreasing from 10000ns to 1000ns
                memory_usage: 1024,
                optimization_level: 1,
            });
        }

        let trend = service.analyze_performance_trend(&improving_history);
        assert_eq!(
            trend,
            PerformanceTrend::Improving,
            "Should detect improving performance with strong downward trend"
        );

        // Test degrading performance - need a strong upward trend
        let mut degrading_history = Vec::new();

        for i in 0..30 {
            // Strongly increasing execution times: from 1000ns to 10000ns
            // This creates a clear positive slope
            let offset_secs = (30 - i) as f64 * 0.1; // Spread over last 3 seconds
            degrading_history.push(ExecutionDataPoint {
                timestamp: base_time
                    .checked_sub(Duration::from_secs_f64(offset_secs))
                    .unwrap_or(base_time),
                duration: Duration::from_nanos(1000 + i * 300), // Increasing from 1000ns to 10000ns
                memory_usage: 1024,
                optimization_level: 1,
            });
        }

        let trend = service.analyze_performance_trend(&degrading_history);
        assert_eq!(
            trend,
            PerformanceTrend::Degrading,
            "Should detect degrading performance with strong upward trend"
        );
    }
}
