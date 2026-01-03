//! Optimization Pipeline Domain Service (Refactored)
//!
//! This service manages multi-stage optimization pipelines for cross-architecture translation.
//! It coordinates different optimization stages and ensures proper sequencing of optimizations.
//!
//! **DDD Architecture**:
//! - Uses `OptimizationStrategy` trait from domain layer (dependency inversion)
//! - Delegates optimization operations to infrastructure layer implementation
//! - Focuses on business logic: event publishing, coordination, pipeline orchestration
//!
//! **Migration Status**:
//! - Infrastructure implementation created: ✅ (vm-engine/src/jit/optimizer_strategy/strategy.rs)
//! - Domain service refactored to use trait: ✅ (Refactored to use OptimizationStrategy trait)

use std::sync::Arc;

use crate::VmResult;
use crate::domain::{OptimizationStrategy, OptimizationType};
use crate::domain_event_bus::DomainEventBus;
use crate::domain_services::events::{DomainEventEnum, OptimizationEvent};
use crate::domain_services::rules::optimization_pipeline_rules::OptimizationPipelineBusinessRule;

/// Optimization stage in the pipeline
#[derive(Debug, Clone, PartialEq)]
pub enum OptimizationStage {
    /// Initial IR generation
    IrGeneration,
    /// Basic block optimization
    BasicBlockOptimization,
    /// Register allocation
    RegisterAllocation,
    /// Instruction scheduling
    InstructionScheduling,
    /// Target-specific optimizations
    TargetOptimization,
    /// Final code generation
    CodeGeneration,
}

impl OptimizationStage {
    pub fn name(&self) -> &'static str {
        match self {
            OptimizationStage::IrGeneration => "IR Generation",
            OptimizationStage::BasicBlockOptimization => "Basic Block Optimization",
            OptimizationStage::RegisterAllocation => "Register Allocation",
            OptimizationStage::InstructionScheduling => "Instruction Scheduling",
            OptimizationStage::TargetOptimization => "Target Optimization",
            OptimizationStage::CodeGeneration => "Code Generation",
        }
    }
}

/// Optimization pipeline configuration
#[derive(Debug, Clone)]
pub struct OptimizationPipelineConfig {
    /// Source architecture
    pub source_arch: crate::GuestArch,
    /// Target architecture
    pub target_arch: crate::GuestArch,
    /// Optimization level (0-3)
    pub optimization_level: u8,
    /// Enabled stages
    pub enabled_stages: Vec<OptimizationStage>,
}

impl OptimizationPipelineConfig {
    /// Create a new pipeline configuration
    pub fn new(
        source_arch: crate::GuestArch,
        target_arch: crate::GuestArch,
        optimization_level: u8,
    ) -> Self {
        let enabled_stages = match optimization_level {
            0 => vec![
                OptimizationStage::IrGeneration,
                OptimizationStage::CodeGeneration,
            ],
            1 => vec![
                OptimizationStage::IrGeneration,
                OptimizationStage::BasicBlockOptimization,
                OptimizationStage::CodeGeneration,
            ],
            2 => vec![
                OptimizationStage::IrGeneration,
                OptimizationStage::BasicBlockOptimization,
                OptimizationStage::RegisterAllocation,
                OptimizationStage::CodeGeneration,
            ],
            3 => vec![
                OptimizationStage::IrGeneration,
                OptimizationStage::BasicBlockOptimization,
                OptimizationStage::RegisterAllocation,
                OptimizationStage::InstructionScheduling,
                OptimizationStage::TargetOptimization,
                OptimizationStage::CodeGeneration,
            ],
            _ => vec![],
        };

        Self {
            source_arch,
            target_arch,
            optimization_level,
            enabled_stages,
        }
    }
}

impl Default for OptimizationPipelineConfig {
    fn default() -> Self {
        Self::new(crate::GuestArch::X86_64, crate::GuestArch::X86_64, 2)
    }
}

/// Pipeline execution result
#[derive(Debug, Clone)]
pub struct PipelineExecutionResult {
    /// Execution success
    pub success: bool,
    /// Completed stages
    pub completed_stages: Vec<OptimizationStage>,
    /// Total execution time (in milliseconds)
    pub total_time_ms: u64,
    /// Error message if failed
    pub error_message: Option<String>,
}

/// Optimization Pipeline Domain Service (Refactored)
///
/// This service provides high-level business logic for managing optimization pipelines
/// with stage orchestration and event publishing.
///
/// **Refactored Architecture**:
/// - Uses `OptimizationStrategy` trait from domain layer (dependency inversion)
/// - Delegates optimization operations to infrastructure layer implementation
/// - Focuses on business logic: event publishing, coordination, pipeline orchestration
pub struct OptimizationPipelineDomainService {
    /// Business rules for pipeline validation
    business_rules: Vec<Box<dyn OptimizationPipelineBusinessRule>>,
    /// Event bus for publishing domain events
    event_bus: Option<Arc<DomainEventBus>>,
    /// Optimization strategy (infrastructure layer implementation via trait)
    optimization_strategy: Arc<dyn OptimizationStrategy>,
}

impl OptimizationPipelineDomainService {
    /// Create a new optimization pipeline domain service
    ///
    /// # 参数
    /// - `optimization_strategy`: Optimization strategy implementation (from infrastructure layer)
    /// - `event_bus`: Event bus for publishing domain events (optional)
    pub fn new(
        optimization_strategy: Arc<dyn OptimizationStrategy>,
        event_bus: Option<Arc<DomainEventBus>>,
    ) -> Self {
        Self {
            business_rules: Vec::new(),
            event_bus,
            optimization_strategy,
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

    /// Execute the optimization pipeline
    ///
    /// Delegates optimization operations to the infrastructure layer implementation.
    pub async fn execute_pipeline(
        &self,
        config: &OptimizationPipelineConfig,
        ir_code: &[u8],
    ) -> VmResult<PipelineExecutionResult> {
        let start_time = std::time::Instant::now();
        let mut completed_stages = Vec::new();

        // Validate business rules
        for rule in &self.business_rules {
            rule.validate_pipeline_config(config)?;
        }

        // Execute enabled stages
        let mut current_ir = ir_code.to_vec();

        for stage in &config.enabled_stages {
            // Validate stage execution
            for rule in &self.business_rules {
                rule.validate_stage_execution(stage, config)?;
            }

            // Execute the stage
            match stage {
                OptimizationStage::BasicBlockOptimization
                | OptimizationStage::TargetOptimization => {
                    // Use optimization strategy for these stages
                    current_ir = self.optimization_strategy.optimize_ir(&current_ir)?;
                    completed_stages.push(stage.clone());

                    // Publish stage completed event
                    // Track memory usage: estimate based on current IR size
                    // In production, this would query system memory usage or use a memory tracker
                    let estimated_memory_mb =
                        ((current_ir.len() as f64) / (1024.0 * 1024.0)) as f32;
                    self.publish_optimization_event(OptimizationEvent::StageCompleted {
                        stage_name: stage.name().to_string(),
                        execution_time_ms: start_time.elapsed().as_millis() as u64,
                        memory_usage_mb: estimated_memory_mb,
                        success: true,
                        occurred_at: std::time::SystemTime::now(),
                    })?;
                }
                OptimizationStage::IrGeneration
                | OptimizationStage::RegisterAllocation
                | OptimizationStage::InstructionScheduling
                | OptimizationStage::CodeGeneration => {
                    // These stages are handled by other services/components
                    // For now, we just mark them as completed
                    completed_stages.push(stage.clone());
                }
            }

            // Check if pipeline should continue
            for rule in &self.business_rules {
                let should_continue = rule.should_continue_pipeline(
                    stage,
                    &PipelineExecutionResult {
                        success: true,
                        completed_stages: completed_stages.clone(),
                        total_time_ms: start_time.elapsed().as_millis() as u64,
                        error_message: None,
                    },
                    config,
                )?;

                if !should_continue {
                    return Ok(PipelineExecutionResult {
                        success: false,
                        completed_stages,
                        total_time_ms: start_time.elapsed().as_millis() as u64,
                        error_message: Some("Pipeline stopped by business rule".to_string()),
                    });
                }
            }
        }

        let total_time_ms = start_time.elapsed().as_millis() as u64;

        // Calculate peak memory usage based on final IR size
        // In production, this would track maximum memory across all stages
        let peak_memory_usage_mb = ((current_ir.len() as f64) / (1024.0 * 1024.0)) as f32;

        // Publish pipeline completed event
        self.publish_optimization_event(OptimizationEvent::PipelineCompleted {
            success: true,
            total_time_ms,
            stages_completed: completed_stages.len(),
            peak_memory_usage_mb,
            occurred_at: std::time::SystemTime::now(),
        })?;

        Ok(PipelineExecutionResult {
            success: true,
            completed_stages,
            total_time_ms,
            error_message: None,
        })
    }

    /// Get optimization level
    ///
    /// Delegates to the infrastructure layer implementation.
    pub fn optimization_level(&self) -> u32 {
        self.optimization_strategy.optimization_level()
    }

    /// Check if a specific optimization is supported
    ///
    /// Delegates to the infrastructure layer implementation.
    pub fn supports_optimization(&self, opt_type: OptimizationType) -> bool {
        self.optimization_strategy.supports_optimization(opt_type)
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
    fn test_optimization_stage_name() {
        assert_eq!(OptimizationStage::IrGeneration.name(), "IR Generation");
        assert_eq!(
            OptimizationStage::BasicBlockOptimization.name(),
            "Basic Block Optimization"
        );
    }

    #[test]
    fn test_pipeline_config_creation() {
        let config =
            OptimizationPipelineConfig::new(crate::GuestArch::X86_64, crate::GuestArch::Arm64, 2);
        assert_eq!(config.optimization_level, 2);
        assert!(!config.enabled_stages.is_empty());
    }
}
