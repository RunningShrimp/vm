//! Optimization Pipeline Business Rules
//!
//! This module contains business rule implementations for optimization pipeline operations.
//! These rules validate pipeline configurations and stage executions.

use crate::{VmError, VmResult};

/// Trait for optimization pipeline business rules
///
/// This trait defines the interface for business rules that validate
/// optimization pipeline operations and ensure they comply with business constraints.
pub trait OptimizationPipelineBusinessRule: Send + Sync {
    /// Validate pipeline configuration
    ///
    /// # Parameters
    /// - `config`: Pipeline configuration to validate
    ///
    /// # Returns
    /// - `Ok(())` if the configuration is valid
    /// - `Err(VmError)` if the configuration violates business rules
    fn validate_pipeline_config(&self, config: &crate::domain_services::optimization_pipeline_service::OptimizationPipelineConfig) -> VmResult<()>;
    
    /// Validate stage execution
    ///
    /// # Parameters
    /// - `stage`: Stage to validate
    /// - `config`: Pipeline configuration
    ///
    /// # Returns
    /// - `Ok(())` if the stage can be executed
    /// - `Err(VmError)` if the stage execution violates business rules
    fn validate_stage_execution(
        &self,
        stage: &crate::domain_services::optimization_pipeline_service::OptimizationStage,
        config: &crate::domain_services::optimization_pipeline_service::OptimizationPipelineConfig,
    ) -> VmResult<()>;
    
    /// Check if pipeline should continue to next stage
    ///
    /// # Parameters
    /// - `current_stage`: Currently executing stage
    /// - `result`: Current pipeline execution result
    /// - `config`: Pipeline configuration
    ///
    /// # Returns
    /// - `Ok(true)` if pipeline should continue
    /// - `Ok(false)` if pipeline should stop
    /// - `Err(VmError)` if an error occurs
    fn should_continue_pipeline(
        &self,
        current_stage: &crate::domain_services::optimization_pipeline_service::OptimizationStage,
        result: &crate::domain_services::optimization_pipeline_service::PipelineExecutionResult,
        config: &crate::domain_services::optimization_pipeline_service::OptimizationPipelineConfig,
    ) -> VmResult<bool>;
}

/// Pipeline configuration validation rule
///
/// This rule validates that pipeline configuration is valid
/// and meets business requirements.
pub struct PipelineConfigValidationRule;

impl OptimizationPipelineBusinessRule for PipelineConfigValidationRule {
    fn validate_pipeline_config(&self, config: &crate::domain_services::optimization_pipeline_service::OptimizationPipelineConfig) -> VmResult<()> {
        // Validate optimization level
        if config.optimization_level > 3 {
            return Err(VmError::Core(crate::error::CoreError::InvalidConfig {
                field: "optimization_level".to_string(),
                message: "Optimization level must be between 0 and 3".to_string(),
            }));
        }
        
        // Validate that at least IR generation and code generation are enabled
        // if !config.enabled_stages.contains(&crate::domain_services::optimization_pipeline_service::OptimizationStage::IrGeneration) {
            return Err(VmError::Core(crate::error::CoreError::InvalidConfig {
                field: "enabled_stages".to_string(),
                message: "IR Generation stage must be enabled".to_string(),
            }));
        }
        
        // if !config.enabled_stages.contains(&crate::domain_services::optimization_pipeline_service::OptimizationStage::CodeGeneration) {
            return Err(VmError::Core(crate::error::CoreError::InvalidConfig {
                field: "enabled_stages".to_string(),
                message: "Code Generation stage must be enabled".to_string(),
            }));
        }
        
        // Validate stage ordering
        let mut last_order = 0;
        // for stage in &config.enabled_stages {
            if stage.order() <= last_order {
                return Err(VmError::Core(crate::error::CoreError::InvalidConfig {
                    field: "enabled_stages".to_string(),
                    message: "Stages must be in correct order".to_string(),
                }));
            }
            last_order = stage.order();
        }
        
        Ok(())
    }
    
    fn validate_stage_execution(
        &self,
        _stage: &crate::domain_services::optimization_pipeline_service::OptimizationStage,
        _config: &crate::domain_services::optimization_pipeline_service::OptimizationPipelineConfig,
    ) -> VmResult<()> {
        Ok(())
    }
    
    fn should_continue_pipeline(
        &self,
        _current_stage: &crate::domain_services::optimization_pipeline_service::OptimizationStage,
        _result: &crate::domain_services::optimization_pipeline_service::PipelineExecutionResult,
        _config: &crate::domain_services::optimization_pipeline_service::OptimizationPipelineConfig,
    ) -> VmResult<bool> {
        Ok(true)
    }
}

/// Stage execution validation rule
///
/// This rule validates that stage execution is allowed
/// and meets business requirements.
pub struct StageExecutionValidationRule;

impl OptimizationPipelineBusinessRule for StageExecutionValidationRule {
    fn validate_pipeline_config(&self, _config: &crate::domain_services::optimization_pipeline_service::OptimizationPipelineConfig) -> VmResult<()> {
        Ok(())
    }
    
    fn validate_stage_execution(
        &self,
        stage: &crate::domain_services::optimization_pipeline_service::OptimizationStage,
        config: &crate::domain_services::optimization_pipeline_service::OptimizationPipelineConfig,
    ) -> VmResult<()> {
        // Check if stage is enabled
        // if !config.enabled_stages.contains(stage) {
            return Err(VmError::Core(crate::error::CoreError::InvalidState {
                message: format!("Stage {} is not enabled in the configuration", stage.name()),
                current: "disabled".to_string(),
                expected: "enabled".to_string(),
            }));
        }
        
        Ok(())
    }
    
    fn should_continue_pipeline(
        &self,
        _current_stage: &crate::domain_services::optimization_pipeline_service::OptimizationStage,
        _result: &crate::domain_services::optimization_pipeline_service::PipelineExecutionResult,
        _config: &crate::domain_services::optimization_pipeline_service::OptimizationPipelineConfig,
    ) -> VmResult<bool> {
        Ok(true)
    }
}

/// Pipeline continuation rule
///
/// This rule determines if pipeline should continue to next stage
/// based on performance constraints and execution results.
pub struct PipelineContinuationRule;

impl OptimizationPipelineBusinessRule for PipelineContinuationRule {
    fn validate_pipeline_config(&self, _config: &crate::domain_services::optimization_pipeline_service::OptimizationPipelineConfig) -> VmResult<()> {
        Ok(())
    }
    
    fn validate_stage_execution(
        &self,
        _stage: &crate::domain_services::optimization_pipeline_service::OptimizationStage,
        _config: &crate::domain_services::optimization_pipeline_service::OptimizationPipelineConfig,
    ) -> VmResult<()> {
        Ok(())
    }
    
    fn should_continue_pipeline(
        &self,
        _current_stage: &crate::domain_services::optimization_pipeline_service::OptimizationStage,
        result: &crate::domain_services::optimization_pipeline_service::PipelineExecutionResult,
        config: &crate::domain_services::optimization_pipeline_service::OptimizationPipelineConfig,
    ) -> VmResult<bool> {
        // Check if we've exceeded maximum compilation time
        // if let Some(max_time) = config.performance_requirements.max_compilation_time_ms {
            if result.total_time_ms > max_time {
                return Ok(false);
            }
        }
        
        // Check if we've exceeded maximum memory usage
        // if let Some(max_memory) = config.performance_requirements.max_memory_usage_mb {
            if result.peak_memory_usage_mb > max_memory as f32 {
                return Ok(false);
            }
        }
        
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain_services::optimization_pipeline_service::{
        OptimizationPipelineConfig, OptimizationStage, PerformanceRequirements, OptimizationPriority
    };
    
    #[test]
    fn test_pipeline_config_validation_rule() {
        let rule = PipelineConfigValidationRule;
        
        // Test valid configuration
        let config = OptimizationPipelineConfig::new(
            crate::GuestArch::X86_64,
            crate::GuestArch::Arm64,
            2,
        );
        assert!(rule.validate_pipeline_config(&config).is_ok());
        
        // Test invalid optimization level
        let mut config = OptimizationPipelineConfig::new(
            crate::GuestArch::X86_64,
            crate::GuestArch::Arm64,
            5, // Invalid level
        );
        assert!(rule.validate_pipeline_config(&config).is_err());
        
        // Test missing IR generation stage
        let mut config = OptimizationPipelineConfig::new(
            crate::GuestArch::X86_64,
            crate::GuestArch::Arm64,
            2,
        );
        config.enabled_stages.retain(|s| s != &OptimizationStage::IrGeneration);
        assert!(rule.validate_pipeline_config(&config).is_err());
        
        // Test missing code generation stage
        let mut config = OptimizationPipelineConfig::new(
            crate::GuestArch::X86_64,
            crate::GuestArch::Arm64,
            2,
        );
        config.enabled_stages.retain(|s| s != &OptimizationStage::CodeGeneration);
        assert!(rule.validate_pipeline_config(&config).is_err());
        
        // Test incorrect stage ordering
        let mut config = OptimizationPipelineConfig::new(
            crate::GuestArch::X86_64,
            crate::GuestArch::Arm64,
            2,
        );
        config.enabled_stages.reverse(); // Reverse order
        assert!(rule.validate_pipeline_config(&config).is_err());
    }
    
    #[test]
    fn test_stage_execution_validation_rule() {
        let rule = StageExecutionValidationRule;
        
        let config = OptimizationPipelineConfig::new(
            crate::GuestArch::X86_64,
            crate::GuestArch::Arm64,
            2,
        );
        
        // Test enabled stage (should pass)
        assert!(rule.validate_stage_execution(&OptimizationStage::IrGeneration, &config).is_ok());
        
        // Test disabled stage (should fail)
        assert!(rule.validate_stage_execution(&OptimizationStage::InstructionScheduling, &config).is_err());
    }
    
    #[test]
    fn test_pipeline_continuation_rule() {
        let rule = PipelineContinuationRule;
        
        let config = OptimizationPipelineConfig::new(
            crate::GuestArch::X86_64,
            crate::GuestArch::Arm64,
            2,
        );
        
        // Create a result that exceeds time limit
        let mut result = crate::domain_services::optimization_pipeline_service::PipelineExecutionResult {
            success: false,
            completed_stages: vec![],
            stage_times: std::collections::HashMap::new(),
            total_time_ms: 10000, // Exceeds default limit
            stage_memory_usage: std::collections::HashMap::new(),
            peak_memory_usage_mb: 100.0,
            optimization_stats: crate::domain_services::optimization_pipeline_service::OptimizationStats::default(),
            error_message: None,
        };
        
        // Should stop due to time limit
        assert_eq!(rule.should_continue_pipeline(&OptimizationStage::IrGeneration, &result, &config), Ok(false));

        // Create a result that exceeds memory limit
        result.total_time_ms = 1000; // Within time limit
        result.peak_memory_usage_mb = 1000.0; // Exceeds default limit

        // Should stop due to memory limit
        assert_eq!(rule.should_continue_pipeline(&OptimizationStage::IrGeneration, &result, &config), Ok(false));

        // Create a result within limits
        result.peak_memory_usage_mb = 100.0; // Within memory limit

        // Should continue
        assert_eq!(rule.should_continue_pipeline(&OptimizationStage::IrGeneration, &result, &config), Ok(true));
    }
}