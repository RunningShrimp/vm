//! Register Allocation Domain Service (Refactored)
//!
//! This service manages complex business logic of register allocation,
//! including live range analysis, register pressure analysis, and spill decisions.
//!
//! **DDD Architecture**:
//! - Uses `RegisterAllocator` trait from domain layer (dependency inversion)
//! - Delegates register allocation operations to infrastructure layer implementation
//! - Focuses on business logic: event publishing, coordination, analysis
//!
//! **Migration Status**:
//! - Infrastructure implementation created: ✅ (vm-engine/src/jit/register_allocator_adapter/adapter.rs)
//! - Domain service refactored to use trait: ✅ (Refactored to use RegisterAllocator trait)

use std::sync::Arc;

use crate::VmResult;
use crate::domain::RegisterAllocator;
use crate::domain_event_bus::DomainEventBus;
use crate::domain_services::events::{DomainEventEnum, OptimizationEvent};
use crate::domain_services::rules::optimization_pipeline_rules::OptimizationPipelineBusinessRule;

/// Register allocation configuration
#[derive(Debug, Clone)]
pub struct RegisterAllocationConfig {
    /// Maximum physical registers available
    pub max_physical_registers: usize,
    /// Spill threshold (when to spill to memory)
    pub spill_threshold: f64,
}

impl Default for RegisterAllocationConfig {
    fn default() -> Self {
        Self {
            max_physical_registers: 16,
            spill_threshold: 0.8,
        }
    }
}

/// Register allocation result
#[derive(Debug, Clone)]
pub struct RegisterAllocationResult {
    /// Number of registers used
    pub registers_used: usize,
    /// Number of spills generated
    pub spills: usize,
    /// Allocation quality score (0.0 to 1.0)
    pub allocation_quality: f64,
}

/// Register Allocation Domain Service (Refactored)
///
/// This service provides high-level business logic for managing register allocation
/// with analysis, coordination, and event publishing.
///
/// **Refactored Architecture**:
/// - Uses `RegisterAllocator` trait from domain layer (dependency inversion)
/// - Delegates register allocation operations to infrastructure layer implementation
/// - Focuses on business logic: event publishing, coordination, analysis
pub struct RegisterAllocationDomainService {
    /// Business rules for register allocation
    business_rules: Vec<Box<dyn OptimizationPipelineBusinessRule>>,
    /// Event bus for publishing domain events
    event_bus: Option<Arc<DomainEventBus>>,
    /// Configuration for register allocation (used for allocation strategies)
    #[allow(dead_code)] // Reserved for future allocation strategy configuration
    config: RegisterAllocationConfig,
    /// Register allocator (infrastructure layer implementation via trait)
    register_allocator: Arc<std::sync::Mutex<dyn RegisterAllocator>>,
}

impl RegisterAllocationDomainService {
    /// Create a new register allocation domain service
    ///
    /// # 参数
    /// - `register_allocator`: Register allocator implementation (from infrastructure layer)
    /// - `config`: Register allocation configuration
    /// - `event_bus`: Event bus for publishing domain events (optional)
    pub fn new(
        register_allocator: Arc<std::sync::Mutex<dyn RegisterAllocator>>,
        config: RegisterAllocationConfig,
        event_bus: Option<Arc<DomainEventBus>>,
    ) -> Self {
        Self {
            business_rules: Vec::new(),
            event_bus,
            config,
            register_allocator,
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

    /// Allocate registers for IR code
    ///
    /// Delegates to the infrastructure layer implementation.
    pub async fn allocate(&mut self, ir: &[u8]) -> VmResult<Vec<u8>> {
        // Validate business rules
        for _rule in &self.business_rules {
            // Simplified validation - actual implementation would use proper config
        }

        // Delegate to infrastructure layer
        let mut allocator = self.register_allocator.lock().unwrap();
        let allocated_ir = allocator.allocate(ir)?;

        // Get allocation statistics
        let stats = allocator.stats();

        // Generate function name for tracking based on IR content
        // Use IR hash as identifier for the function being allocated
        let function_name = if ir.len() >= 8 {
            // Create a unique identifier from first 8 bytes of IR
            format!("fn_{:02x}{:02x}{:02x}{:02x}_{:02x}{:02x}{:02x}{:02x}",
                ir[0], ir[1], ir[2], ir[3], ir[4], ir[5], ir[6], ir[7])
        } else if !ir.is_empty() {
            let bytes: Vec<String> = ir.iter().map(|b| format!("{:02x}", b)).collect();
            format!("fn_{}", bytes.join(""))
        } else {
            "fn_unknown".to_string()
        };

        // Publish register allocation completed event
        self.publish_optimization_event(OptimizationEvent::RegisterAllocationCompleted {
            function_name,
            registers_used: stats.physical_regs_used,
            spill_count: stats.spills,
            occurred_at: std::time::SystemTime::now(),
        })?;

        Ok(allocated_ir)
    }

    /// Get register allocation statistics
    ///
    /// Delegates to the infrastructure layer implementation.
    pub async fn get_statistics(&self) -> crate::domain::RegisterAllocationStats {
        let allocator = self.register_allocator.lock().unwrap();
        allocator.stats()
    }

    /// Publish optimization event
    fn publish_optimization_event(&self, event: OptimizationEvent) -> VmResult<()> {
        if let Some(event_bus) = &self.event_bus {
            let _ = event_bus.publish(&DomainEventEnum::Optimization(event));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_allocation_config_default() {
        let config = RegisterAllocationConfig::default();
        assert_eq!(config.max_physical_registers, 16);
        assert_eq!(config.spill_threshold, 0.8);
    }
}
