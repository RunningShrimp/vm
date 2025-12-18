//! Resource Management Domain Service
//!
//! This service encapsulates business logic for resource management
//! including resource constraint validation, allocation decisions,
//! performance threshold management, and memory/CPU budget management.

use std::collections::HashMap;
use std::sync::Arc;

use crate::domain_services::events::{DomainEventBus, DomainEventEnum, OptimizationEvent};
use crate::domain_services::rules::optimization_pipeline_rules::OptimizationPipelineBusinessRule;
use crate::{VmError, VmResult};

/// Resource type for management
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ResourceType {
    /// CPU resources
    Cpu,
    /// Memory resources
    Memory,
    /// Cache resources
    Cache,
    /// Storage resources
    Storage,
    /// Network resources
    Network,
}

/// Resource constraint definition
#[derive(Debug, Clone)]
pub struct ResourceConstraint {
    /// Type of resource
    pub resource_type: ResourceType,
    /// Maximum allowed usage
    pub max_usage: u64,
    /// Current usage
    pub current_usage: u64,
    /// Warning threshold (0.0 to 1.0)
    pub warning_threshold: f64,
    /// Critical threshold (0.0 to 1.0)
    pub critical_threshold: f64,
    /// Unit of measurement
    pub unit: String,
}

impl ResourceConstraint {
    /// Create a new resource constraint
    pub fn new(
        resource_type: ResourceType,
        max_usage: u64,
        warning_threshold: f64,
        critical_threshold: f64,
        unit: String,
    ) -> Self {
        Self {
            resource_type,
            max_usage,
            current_usage: 0,
            warning_threshold,
            critical_threshold,
            unit,
        }
    }

    /// Get the current utilization ratio (0.0 to 1.0)
    pub fn utilization_ratio(&self) -> f64 {
        if self.max_usage == 0 {
            0.0
        } else {
            self.current_usage as f64 / self.max_usage as f64
        }
    }

    /// Check if the resource is at warning level
    pub fn is_warning_level(&self) -> bool {
        self.utilization_ratio() >= self.warning_threshold
    }

    /// Check if the resource is at critical level
    pub fn is_critical_level(&self) -> bool {
        self.utilization_ratio() >= self.critical_threshold
    }

    /// Check if the resource is exhausted
    pub fn is_exhausted(&self) -> bool {
        self.current_usage >= self.max_usage
    }
}

/// Resource allocation request
#[derive(Debug, Clone)]
pub struct ResourceAllocationRequest {
    /// Type of resource to allocate
    pub resource_type: ResourceType,
    /// Amount of resource requested
    pub amount: u64,
    /// Priority of the allocation (higher = more important)
    pub priority: u32,
    /// Purpose of the allocation
    pub purpose: String,
    /// Optional allocation timeout
    pub timeout: Option<std::time::Duration>,
}

/// Resource allocation result
#[derive(Debug, Clone)]
pub struct ResourceAllocationResult {
    /// Whether the allocation was successful
    pub success: bool,
    /// Amount of resource allocated
    pub allocated_amount: u64,
    /// Remaining available resource
    pub remaining_amount: u64,
    /// Allocation ID for tracking
    pub allocation_id: Option<String>,
    /// Reason for failure (if any)
    pub failure_reason: Option<String>,
}

/// Performance threshold configuration
#[derive(Debug, Clone)]
pub struct PerformanceThreshold {
    /// Resource type this threshold applies to
    pub resource_type: ResourceType,
    /// Minimum performance requirement
    pub min_performance: f64,
    /// Maximum acceptable latency
    pub max_latency: std::time::Duration,
    /// Target throughput
    pub target_throughput: f64,
    /// Whether adaptive thresholding is enabled
    pub adaptive_thresholding: bool,
}

/// Resource budget for optimization operations
#[derive(Debug, Clone)]
pub struct ResourceBudget {
    /// Total CPU budget in percentage (0.0 to 1.0)
    pub cpu_budget: f64,
    /// Total memory budget in bytes
    pub memory_budget: u64,
    /// Total cache budget in bytes
    pub cache_budget: u64,
    /// Budget allocation strategy
    pub allocation_strategy: BudgetAllocationStrategy,
}

/// Budget allocation strategy
#[derive(Debug, Clone, PartialEq)]
pub enum BudgetAllocationStrategy {
    /// Equal distribution among all operations
    Equal,
    /// Priority-based allocation
    PriorityBased,
    /// Performance-based allocation
    PerformanceBased,
    /// Adaptive allocation based on current load
    Adaptive,
}

/// Resource management configuration
#[derive(Debug, Clone)]
pub struct ResourceManagementConfig {
    /// Default resource constraints
    pub default_constraints: HashMap<ResourceType, ResourceConstraint>,
    /// Performance thresholds
    pub performance_thresholds: HashMap<ResourceType, PerformanceThreshold>,
    /// Resource budget for optimization
    pub optimization_budget: ResourceBudget,
    /// GC trigger threshold
    pub gc_trigger_threshold: f64,
    /// Resource monitoring interval
    pub monitoring_interval: std::time::Duration,
    /// Whether to enable automatic resource recovery
    pub enable_auto_recovery: bool,
}

impl Default for ResourceManagementConfig {
    fn default() -> Self {
        let mut default_constraints = HashMap::new();
        
        // Default memory constraint (2GB max, 80% warning, 95% critical)
        default_constraints.insert(
            ResourceType::Memory,
            ResourceConstraint::new(
                ResourceType::Memory,
                2 * 1024 * 1024 * 1024, // 2GB
                0.8,
                0.95,
                "bytes".to_string(),
            ),
        );
        
        // Default CPU constraint (100% max, 70% warning, 90% critical)
        default_constraints.insert(
            ResourceType::Cpu,
            ResourceConstraint::new(
                ResourceType::Cpu,
                100,
                0.7,
                0.9,
                "percent".to_string(),
            ),
        );
        
        // Default cache constraint (256MB max, 75% warning, 90% critical)
        default_constraints.insert(
            ResourceType::Cache,
            ResourceConstraint::new(
                ResourceType::Cache,
                256 * 1024 * 1024, // 256MB
                0.75,
                0.9,
                "bytes".to_string(),
            ),
        );

        let mut performance_thresholds = HashMap::new();
        
        // Memory performance threshold
        performance_thresholds.insert(
            ResourceType::Memory,
            PerformanceThreshold {
                resource_type: ResourceType::Memory,
                min_performance: 0.8,
                max_latency: std::time::Duration::from_millis(10),
                target_throughput: 1000.0,
                adaptive_thresholding: true,
            },
        );
        
        // CPU performance threshold
        performance_thresholds.insert(
            ResourceType::Cpu,
            PerformanceThreshold {
                resource_type: ResourceType::Cpu,
                min_performance: 0.7,
                max_latency: std::time::Duration::from_millis(5),
                target_throughput: 2000.0,
                adaptive_thresholding: true,
            },
        );

        Self {
            default_constraints,
            performance_thresholds,
            optimization_budget: ResourceBudget {
                cpu_budget: 0.3, // 30% of CPU
                memory_budget: 512 * 1024 * 1024, // 512MB
                cache_budget: 64 * 1024 * 1024, // 64MB
                allocation_strategy: BudgetAllocationStrategy::Adaptive,
            },
            gc_trigger_threshold: 0.85,
            monitoring_interval: std::time::Duration::from_secs(5),
            enable_auto_recovery: true,
        }
    }
}

/// Resource Management Domain Service
/// 
/// This service encapsulates business logic for resource management
/// including resource constraint validation, allocation decisions,
/// performance threshold management, and memory/CPU budget management.
#[derive(Debug)]
pub struct ResourceManagementDomainService {
    /// Business rules for resource management
    business_rules: Vec<Box<dyn OptimizationPipelineBusinessRule>>,
    /// Event bus for publishing domain events
    event_bus: Option<Arc<dyn DomainEventBus>>,
    /// Configuration for resource management
    config: ResourceManagementConfig,
    /// Current resource constraints
    current_constraints: HashMap<ResourceType, ResourceConstraint>,
}

impl ResourceManagementDomainService {
    /// Create a new resource management domain service
    pub fn new(config: ResourceManagementConfig) -> Self {
        let current_constraints = config.default_constraints.clone();
        
        Self {
            business_rules: Vec::new(),
            event_bus: None,
            config,
            current_constraints,
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

    /// Validate resource constraints
    pub fn validate_resource_constraints(&self) -> VmResult<Vec<ResourceType>> {
        let mut violated_constraints = Vec::new();

        for (resource_type, constraint) in &self.current_constraints {
            if constraint.is_critical_level() {
                violated_constraints.push(resource_type.clone());
            }
        }

        // Publish constraint validation event
        if !violated_constraints.is_empty() {
            let violated_resources: Vec<String> = violated_constraints.iter()
                .map(|rt| format!("{:?}", rt))
                .collect();
            
            self.publish_optimization_event(OptimizationEvent::ResourceConstraintViolation {
                violated_resources,
            })?;
        }

        Ok(violated_constraints)
    }

    /// Allocate resources for a specific request
    pub fn allocate_resources(
        &mut self,
        request: &ResourceAllocationRequest,
    ) -> VmResult<ResourceAllocationResult> {
        // Validate business rules
        for rule in &self.business_rules {
            if let Err(e) = rule.validate_pipeline_config(&self.create_pipeline_config()) {
                return Err(e);
            }
        }

        let constraint = self.current_constraints
            .get_mut(&request.resource_type)
            .ok_or_else(|| VmError::Core(crate::CoreError::InvalidConfig {
                message: format!("No constraint found for resource type: {:?}", request.resource_type),
            }))?;

        let available_amount = constraint.max_usage - constraint.current_usage;
        let allocated_amount = request.amount.min(available_amount);
        let success = allocated_amount == request.amount;

        if success {
            constraint.current_usage += allocated_amount;
        }

        let result = ResourceAllocationResult {
            success,
            allocated_amount,
            remaining_amount: available_amount - allocated_amount,
            allocation_id: if success { 
                Some(format!("alloc_{}_{}", request.resource_type as u32, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos())) 
            } else { 
                None 
            },
            failure_reason: if !success { 
                Some(format!("Insufficient resources: requested {}, available {}", request.amount, available_amount)) 
            } else { 
                None 
            },
        };

        // Publish resource allocation event
        self.publish_optimization_event(OptimizationEvent::ResourceAllocated {
            resource_type: format!("{:?}", request.resource_type),
            requested_amount: request.amount,
            allocated_amount: result.allocated_amount,
            success: result.success,
        })?;

        Ok(result)
    }

    /// Release previously allocated resources
    pub fn release_resources(
        &mut self,
        resource_type: ResourceType,
        amount: u64,
        allocation_id: &str,
    ) -> VmResult<()> {
        let constraint = self.current_constraints
            .get_mut(&resource_type)
            .ok_or_else(|| VmError::Core(crate::CoreError::InvalidConfig {
                message: format!("No constraint found for resource type: {:?}", resource_type),
            }))?;

        // Ensure we don't release more than we've allocated
        let release_amount = amount.min(constraint.current_usage);
        constraint.current_usage -= release_amount;

        // Publish resource release event
        self.publish_optimization_event(OptimizationEvent::ResourceReleased {
            resource_type: format!("{:?}", resource_type),
            released_amount: release_amount,
            allocation_id: allocation_id.to_string(),
        })?;

        Ok(())
    }

    /// Check if garbage collection should be triggered
    pub fn should_trigger_gc(&self) -> bool {
        if let Some(memory_constraint) = self.current_constraints.get(&ResourceType::Memory) {
            memory_constraint.utilization_ratio() >= self.config.gc_trigger_threshold
        } else {
            false
        }
    }

    /// Get current resource utilization
    pub fn get_resource_utilization(&self) -> HashMap<ResourceType, f64> {
        self.current_constraints
            .iter()
            .map(|(resource_type, constraint)| {
                (resource_type.clone(), constraint.utilization_ratio())
            })
            .collect()
    }

    /// Update performance thresholds based on current performance
    pub fn update_performance_thresholds(
        &mut self,
        resource_type: ResourceType,
        current_performance: f64,
    ) -> VmResult<()> {
        if let Some(threshold) = self.config.performance_thresholds.get_mut(&resource_type) {
            if threshold.adaptive_thresholding {
                // Adjust thresholds based on current performance
                if current_performance < threshold.min_performance {
                    // Performance is below threshold, relax requirements
                    threshold.min_performance *= 0.9;
                    threshold.max_latency *= 1.1;
                } else if current_performance > threshold.min_performance * 1.2 {
                    // Performance is good, tighten requirements
                    threshold.min_performance *= 1.05;
                    threshold.max_latency *= 0.95;
                }

                // Publish threshold update event
                self.publish_optimization_event(OptimizationEvent::PerformanceThresholdUpdated {
                    resource_type: format!("{:?}", resource_type),
                    new_min_performance: threshold.min_performance,
                    new_max_latency: threshold.max_latency,
                })?;
            }
        }

        Ok(())
    }

    /// Allocate budget for optimization operations
    pub fn allocate_optimization_budget(
        &self,
        operation_type: &str,
        priority: u32,
    ) -> VmResult<HashMap<ResourceType, u64>> {
        let mut allocation = HashMap::new();

        match self.config.optimization_budget.allocation_strategy {
            BudgetAllocationStrategy::Equal => {
                allocation.insert(ResourceType::Cpu, (self.config.optimization_budget.cpu_budget * 100.0) as u64 / 4);
                allocation.insert(ResourceType::Memory, self.config.optimization_budget.memory_budget / 4);
                allocation.insert(ResourceType::Cache, self.config.optimization_budget.cache_budget / 4);
            },
            BudgetAllocationStrategy::PriorityBased => {
                let weight = priority as f64 / 100.0;
                allocation.insert(ResourceType::Cpu, (self.config.optimization_budget.cpu_budget * 100.0 * weight) as u64);
                allocation.insert(ResourceType::Memory, (self.config.optimization_budget.memory_budget as f64 * weight) as u64);
                allocation.insert(ResourceType::Cache, (self.config.optimization_budget.cache_budget as f64 * weight) as u64);
            },
            BudgetAllocationStrategy::PerformanceBased => {
                // Allocate based on current performance metrics
                let utilization = self.get_resource_utilization();
                let cpu_weight = 1.0 - utilization.get(&ResourceType::Cpu).unwrap_or(&0.0);
                let memory_weight = 1.0 - utilization.get(&ResourceType::Memory).unwrap_or(&0.0);
                
                allocation.insert(ResourceType::Cpu, (self.config.optimization_budget.cpu_budget * 100.0 * cpu_weight) as u64);
                allocation.insert(ResourceType::Memory, (self.config.optimization_budget.memory_budget as f64 * memory_weight) as u64);
                allocation.insert(ResourceType::Cache, self.config.optimization_budget.cache_budget / 2);
            },
            BudgetAllocationStrategy::Adaptive => {
                // Adaptive allocation based on current load and operation type
                let utilization = self.get_resource_utilization();
                let cpu_available = 1.0 - utilization.get(&ResourceType::Cpu).unwrap_or(&0.0);
                let memory_available = 1.0 - utilization.get(&ResourceType::Memory).unwrap_or(&0.0);
                
                // Adjust allocation based on operation type
                let (cpu_factor, memory_factor) = match operation_type {
                    "compilation" => (0.7, 0.3),
                    "optimization" => (0.5, 0.5),
                    "translation" => (0.6, 0.4),
                    _ => (0.5, 0.5),
                };
                
                allocation.insert(ResourceType::Cpu, (self.config.optimization_budget.cpu_budget * 100.0 * cpu_available * cpu_factor) as u64);
                allocation.insert(ResourceType::Memory, (self.config.optimization_budget.memory_budget as f64 * memory_available * memory_factor) as u64);
                allocation.insert(ResourceType::Cache, self.config.optimization_budget.cache_budget / 2);
            },
        }

        Ok(allocation)
    }

    /// Create a pipeline configuration from the resource management config
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
    fn test_resource_constraint_validation() {
        let config = ResourceManagementConfig::default();
        let service = ResourceManagementDomainService::new(config);
        
        // Initially no constraints should be violated
        let violated = service.validate_resource_constraints().unwrap();
        assert!(violated.is_empty());
    }

    #[test]
    fn test_resource_allocation() {
        let config = ResourceManagementConfig::default();
        let mut service = ResourceManagementDomainService::new(config);
        
        let request = ResourceAllocationRequest {
            resource_type: ResourceType::Memory,
            amount: 1024 * 1024, // 1MB
            priority: 50,
            purpose: "test allocation".to_string(),
            timeout: None,
        };
        
        let result = service.allocate_resources(&request).unwrap();
        assert!(result.success);
        assert_eq!(result.allocated_amount, 1024 * 1024);
        assert!(result.allocation_id.is_some());
    }

    #[test]
    fn test_resource_exhaustion() {
        let config = ResourceManagementConfig::default();
        let mut service = ResourceManagementDomainService::new(config);
        
        // Request more memory than available
        let request = ResourceAllocationRequest {
            resource_type: ResourceType::Memory,
            amount: 3 * 1024 * 1024 * 1024, // 3GB (more than 2GB max)
            priority: 50,
            purpose: "test exhaustion".to_string(),
            timeout: None,
        };
        
        let result = service.allocate_resources(&request).unwrap();
        assert!(!result.success);
        assert!(result.failure_reason.is_some());
    }

    #[test]
    fn test_gc_trigger() {
        let config = ResourceManagementConfig::default();
        let mut service = ResourceManagementDomainService::new(config);
        
        // Initially GC should not be triggered
        assert!(!service.should_trigger_gc());
        
        // Allocate memory beyond GC threshold
        let request = ResourceAllocationRequest {
            resource_type: ResourceType::Memory,
            amount: (2.0 * 1024.0 * 1024.0 * 1024.0 * 0.9) as u64, // 90% of 2GB
            priority: 50,
            purpose: "test gc trigger".to_string(),
            timeout: None,
        };
        
        service.allocate_resources(&request).unwrap();
        assert!(service.should_trigger_gc());
    }

    #[test]
    fn test_budget_allocation() {
        let config = ResourceManagementConfig::default();
        let service = ResourceManagementDomainService::new(config);
        
        let allocation = service.allocate_optimization_budget("compilation", 75).unwrap();
        
        assert!(allocation.contains_key(&ResourceType::Cpu));
        assert!(allocation.contains_key(&ResourceType::Memory));
        assert!(allocation.contains_key(&ResourceType::Cache));
    }
}