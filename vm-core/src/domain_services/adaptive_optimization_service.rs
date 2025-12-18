//! Adaptive Optimization Domain Service
//!
//! This service encapsulates business logic for adaptive optimization
//! including hotspot detection, performance profiling, tiered compilation,
//! and dynamic recompilation decisions.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::domain_services::events::{DomainEventBus, DomainEventEnum, OptimizationEvent};
use crate::domain_services::rules::optimization_pipeline_rules::OptimizationPipelineBusinessRule;
use crate::{VmError, VmResult};

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
#[derive(Debug)]
pub struct AdaptiveOptimizationDomainService {
    /// Business rules for adaptive optimization
    business_rules: Vec<Box<dyn OptimizationPipelineBusinessRule>>,
    /// Event bus for publishing domain events
    event_bus: Option<Arc<dyn DomainEventBus>>,
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

            let avg_execution_time = history.iter()
                .map(|dp| dp.duration)
                .sum::<Duration>() / history.len() as u32;

            let hotness_score = self.calculate_hotness_score(execution_count, avg_execution_time);
            
            if hotness_score >= self.config.hotness_threshold {
                let performance_trend = self.analyze_performance_trend(history);
                let last_execution = history.last().unwrap().timestamp;

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
        hotspots.sort_by(|a, b| b.hotness_score.partial_cmp(&a.hotness_score).unwrap());
        
        // Limit to max_hotspots
        hotspots.truncate(self.config.max_hotspots);

        // Publish hotspot detection event
        self.publish_optimization_event(OptimizationEvent::HotspotsDetected {
            count: hotspots.len(),
            threshold: self.config.hotspot_threshold,
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
            if let Err(e) = rule.validate_pipeline_config(&self.create_pipeline_config()) {
                return Err(e);
            }
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
        let recent_data: Vec<_> = history.iter()
            .filter(|dp| now.duration_since(dp.timestamp) <= self.config.trend_analysis_window)
            .collect();

        if recent_data.len() < 5 {
            return PerformanceTrend::Stable;
        }

        // Calculate performance trend using linear regression
        let n = recent_data.len() as f64;
        let sum_x: f64 = (0..recent_data.len()).map(|i| i as f64).sum();
        let sum_y: f64 = recent_data.iter().map(|dp| dp.duration.as_nanos() as f64).sum();
        let sum_xy: f64 = recent_data.iter().enumerate()
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
        hotspots.iter()
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
    use std::collections::HashMap;

    #[test]
    fn test_hotspot_detection() {
        let config = AdaptiveOptimizationConfig::default();
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
        
        for i in 0..1500 {
            history.push(ExecutionDataPoint {
                timestamp: base_time + Duration::from_micros(i * 100),
                duration: Duration::from_nanos(1000 + (i % 100) as u64),
                memory_usage: 1024,
                optimization_level: 1,
            });
        }
        
        profile.execution_history.insert(address, history);

        let hotspots = service.detect_hotspots(&profile).unwrap();
        assert_eq!(hotspots.len(), 1);
        assert_eq!(hotspots[0].address, address);
        assert_eq!(hotspots[0].execution_count, 1500);
        assert!(hotspots[0].hotness_score > 0.0);
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
        let strategy = service.determine_optimization_strategy(&[], &profile).unwrap();
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
        
        let strategy = service.determine_optimization_strategy(&hotspots, &profile).unwrap();
        assert_eq!(strategy, OptimizationStrategy::Hybrid);

        // Test with performance degradation
        let mut degrading_hotspots = hotspots;
        degrading_hotspots[0].performance_trend = PerformanceTrend::Degrading;
        
        let strategy = service.determine_optimization_strategy(&degrading_hotspots, &profile).unwrap();
        assert_eq!(strategy, OptimizationStrategy::DynamicRecompilation);
    }

    #[test]
    fn test_performance_trend_analysis() {
        let config = AdaptiveOptimizationConfig::default();
        let service = AdaptiveOptimizationDomainService::new(config);
        
        // Test improving performance
        let mut improving_history = Vec::new();
        let base_time = Instant::now();
        
        for i in 0..20 {
            // Decreasing execution times
            improving_history.push(ExecutionDataPoint {
                timestamp: base_time + Duration::from_millis(i * 100),
                duration: Duration::from_nanos(1000 - i * 10),
                memory_usage: 1024,
                optimization_level: 1,
            });
        }
        
        let trend = service.analyze_performance_trend(&improving_history);
        assert_eq!(trend, PerformanceTrend::Improving);

        // Test degrading performance
        let mut degrading_history = Vec::new();
        
        for i in 0..20 {
            // Increasing execution times
            degrading_history.push(ExecutionDataPoint {
                timestamp: base_time + Duration::from_millis(i * 100),
                duration: Duration::from_nanos(1000 + i * 10),
                memory_usage: 1024,
                optimization_level: 1,
            });
        }
        
        let trend = service.analyze_performance_trend(&degrading_history);
        assert_eq!(trend, PerformanceTrend::Degrading);
    }
}