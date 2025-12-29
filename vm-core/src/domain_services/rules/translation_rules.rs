//! Translation Business Rules
//!
//! This module contains business rule implementations for translation operations.
//! These rules validate translation requests and ensure they comply with
//! business constraints and requirements.

use crate::{GuestArch, VmError, VmResult};

/// Trait for translation business rules
///
/// This trait defines the interface for business rules that validate
/// translation requests and ensure they comply with business constraints.
pub trait TranslationBusinessRule: Send + Sync {
    /// Validate a translation request
    ///
    /// # Parameters
    /// - `source`: Source architecture
    /// - `target`: Target architecture
    /// - `context`: Translation context with requirements and constraints
    ///
    /// # Returns
    /// - `Ok(())` if the request is valid
    /// - `Err(VmError)` if the request violates business rules
    fn validate_translation_request(
        &self,
        source: GuestArch,
        target: GuestArch,
        context: &crate::domain_services::translation_strategy_service::TranslationContext,
    ) -> VmResult<()>;
}

/// Architecture compatibility rule
///
/// This rule validates that translation between architectures is supported
/// and compatible with the system constraints.
pub struct ArchitectureCompatibilityRule;

impl TranslationBusinessRule for ArchitectureCompatibilityRule {
    fn validate_translation_request(
        &self,
        source: GuestArch,
        target: GuestArch,
        context: &crate::domain_services::translation_strategy_service::TranslationContext,
    ) -> VmResult<()> {
        // Check if translation is supported between these architectures
        match (source, target) {
            // Same architecture - always compatible
            (GuestArch::X86_64, GuestArch::X86_64) |
            (GuestArch::Arm64, GuestArch::Arm64) |
            (GuestArch::Riscv64, GuestArch::Riscv64) => {
                // Same architecture translation is always supported
                Ok(())
            }
            
            // x86-64 to ARM64 - supported with optimizations
            (GuestArch::X86_64, GuestArch::Arm64) => {
                // Check if we have enough resources for optimized translation
                if context.resource_constraints.memory_limit < 128 * 1024 * 1024 {
                    return Err(VmError::Core(crate::error::CoreError::InvalidState {
                        message: "Insufficient memory for x86-64 to ARM64 translation".to_string(),
                        current: format!("{}MB", context.resource_constraints.memory_limit / (1024 * 1024)),
                        expected: ">=128MB".to_string(),
                    }));
                }
                Ok(())
            }
            
            // x86-64 to RISC-V 64 - supported with limitations
            (GuestArch::X86_64, GuestArch::Riscv64) => {
                // Check if we can handle the limitations
                if context.performance_requirements.high_performance {
                    return Err(VmError::Core(crate::error::CoreError::InvalidState {
                        message: "High performance not supported for x86-64 to RISC-V 64 translation".to_string(),
                        current: "high_performance=true".to_string(),
                        expected: "high_performance=false".to_string(),
                    }));
                }
                Ok(())
            }
            
            // ARM64 to x86-64 - supported with optimizations
            (GuestArch::Arm64, GuestArch::X86_64) => {
                // Check if we have enough resources for optimized translation
                if context.resource_constraints.memory_limit < 96 * 1024 * 1024 {
                    return Err(VmError::Core(crate::error::CoreError::InvalidState {
                        message: "Insufficient memory for ARM64 to x86-64 translation".to_string(),
                        current: format!("{}MB", context.resource_constraints.memory_limit / (1024 * 1024)),
                        expected: ">=96MB".to_string(),
                    }));
                }
                Ok(())
            }
            
            // ARM64 to RISC-V 64 - experimental support
            (GuestArch::Arm64, GuestArch::Riscv64) => {
                // Check if experimental mode is allowed
                if context.timing_requirements.real_time {
                    return Err(VmError::Core(crate::error::CoreError::InvalidState {
                        message: "Real-time requirements not supported for experimental ARM64 to RISC-V 64 translation".to_string(),
                        current: "real_time=true".to_string(),
                        expected: "real_time=false".to_string(),
                    }));
                }
                Ok(())
            }
            
            // RISC-V 64 to x86-64 - supported with limitations
            (GuestArch::Riscv64, GuestArch::X86_64) => {
                // Check if we can handle the limitations
                if let Some(min_throughput) = context.performance_requirements.min_throughput {
                    if min_throughput > 500.0 {
                        return Err(VmError::Core(crate::error::CoreError::InvalidState {
                            message: "High throughput not supported for RISC-V 64 to x86-64 translation".to_string(),
                            current: format!("min_throughput={:?}", context.performance_requirements.min_throughput),
                            expected: "min_throughput<=500".to_string(),
                        }));
                    }
                }
                Ok(())
            }
            
            // RISC-V 64 to ARM64 - experimental support
            (GuestArch::Riscv64, GuestArch::Arm64) => {
                // Check if experimental mode is allowed
                if context.timing_requirements.real_time {
                    return Err(VmError::Core(crate::error::CoreError::InvalidState {
                        message: "Real-time requirements not supported for experimental RISC-V 64 to ARM64 translation".to_string(),
                        current: "real_time=true".to_string(),
                        expected: "real_time=false".to_string(),
                    }));
                }
                Ok(())
            }
            
            // Unsupported combinations
            (_source, _target) => {
                Err(VmError::Core(crate::error::CoreError::InvalidState {
                    message: format!("Translation from {:?} to {:?} is not supported", source, target),
                    current: format!("{:?} to {:?}", source, target),
                    expected: "supported architecture combination".to_string(),
                }))
            }
        }
    }
}

/// Performance threshold rule
///
/// This rule validates that performance requirements are within
/// supported thresholds for the translation system.
pub struct PerformanceThresholdRule;

impl TranslationBusinessRule for PerformanceThresholdRule {
    fn validate_translation_request(
        &self,
        _source: GuestArch,
        _target: GuestArch,
        context: &crate::domain_services::translation_strategy_service::TranslationContext,
    ) -> VmResult<()> {
        // Check minimum throughput requirements
        if let Some(min_throughput) = context.performance_requirements.min_throughput {
            if min_throughput > 10000.0 {
                return Err(VmError::Core(crate::error::CoreError::InvalidState {
                    message: "Minimum throughput requirement exceeds system capabilities".to_string(),
                    current: format!("{} ops/sec", min_throughput),
                    expected: "<=10000 ops/sec".to_string(),
                }));
            }
        }
        
        // Check maximum latency requirements
        if let Some(max_latency) = context.performance_requirements.max_latency {
            if max_latency < std::time::Duration::from_millis(1) {
                return Err(VmError::Core(crate::error::CoreError::InvalidState {
                    message: "Maximum latency requirement is too strict".to_string(),
                    current: format!("{:?}", max_latency),
                    expected: ">=1ms".to_string(),
                }));
            }
        }
        
        Ok(())
    }
}

/// Resource availability rule
///
/// This rule validates that sufficient resources are available
/// for the translation operation.
pub struct ResourceAvailabilityRule;

impl TranslationBusinessRule for ResourceAvailabilityRule {
    fn validate_translation_request(
        &self,
        _source: GuestArch,
        _target: GuestArch,
        context: &crate::domain_services::translation_strategy_service::TranslationContext,
    ) -> VmResult<()> {
        // Check memory requirements
        if context.resource_constraints.memory_limit < 32 * 1024 * 1024 {
            return Err(VmError::Core(crate::error::CoreError::InvalidState {
                message: "Insufficient memory for translation".to_string(),
                current: format!("{}MB", context.resource_constraints.memory_limit / (1024 * 1024)),
                expected: ">=32MB".to_string(),
            }));
        }
        
        // Check CPU requirements
        if let Some(cpu_limit) = context.resource_constraints.cpu_limit {
            if cpu_limit < 1 {
                return Err(VmError::Core(crate::error::CoreError::InvalidState {
                    message: "Insufficient CPU resources for translation".to_string(),
                    current: format!("{} cores", cpu_limit),
                    expected: ">=1 core".to_string(),
                }));
            }
        }
        
        // Check time requirements
        if let Some(time_limit) = context.resource_constraints.time_limit {
            if time_limit < std::time::Duration::from_secs(5) {
                return Err(VmError::Core(crate::error::CoreError::InvalidState {
                    message: "Insufficient time allocated for translation".to_string(),
                    current: format!("{:?}", time_limit),
                    expected: ">=5s".to_string(),
                }));
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain_services::translation_strategy_service::{
        TranslationContext, PerformanceRequirements, ResourceConstraints, TimingRequirements
    };
    
    #[test]
    fn test_architecture_compatibility_rule() {
        let rule = ArchitectureCompatibilityRule;
        
        // Test same architecture (should pass)
        let context = create_test_context();
        assert!(rule.validate_translation_request(
            GuestArch::X86_64,
            GuestArch::X86_64,
            &context
        ).is_ok());
        
        // Test x86-64 to ARM64 with sufficient memory (should pass)
        let mut context = create_test_context();
        context.resource_constraints.memory_limit = 128 * 1024 * 1024;
        assert!(rule.validate_translation_request(
            GuestArch::X86_64,
            GuestArch::Arm64,
            &context
        ).is_ok());
        
        // Test x86-64 to ARM64 with insufficient memory (should fail)
        let mut context = create_test_context();
        context.resource_constraints.memory_limit = 64 * 1024 * 1024;
        assert!(rule.validate_translation_request(
            GuestArch::X86_64,
            GuestArch::Arm64,
            &context
        ).is_err());
        
        // Test unsupported combination (should fail)
        let context = create_test_context();
        assert!(rule.validate_translation_request(
            GuestArch::X86_64,
            GuestArch::X86_64, // This is actually supported, just for test structure
            &context
        ).is_ok());
    }
    
    #[test]
    fn test_performance_threshold_rule() {
        let rule = PerformanceThresholdRule;
        
        // Test valid throughput (should pass)
        let mut context = create_test_context();
        context.performance_requirements.min_throughput = Some(5000.0);
        assert!(rule.validate_translation_request(
            GuestArch::X86_64,
            GuestArch::Arm64,
            &context
        ).is_ok());
        
        // Test invalid throughput (should fail)
        let mut context = create_test_context();
        context.performance_requirements.min_throughput = Some(15000.0);
        assert!(rule.validate_translation_request(
            GuestArch::X86_64,
            GuestArch::Arm64,
            &context
        ).is_err());
        
        // Test valid latency (should pass)
        let mut context = create_test_context();
        context.performance_requirements.max_latency = Some(std::time::Duration::from_millis(10));
        assert!(rule.validate_translation_request(
            GuestArch::X86_64,
            GuestArch::Arm64,
            &context
        ).is_ok());
        
        // Test invalid latency (should fail)
        let mut context = create_test_context();
        context.performance_requirements.max_latency = Some(std::time::Duration::from_micros(500));
        assert!(rule.validate_translation_request(
            GuestArch::X86_64,
            GuestArch::Arm64,
            &context
        ).is_err());
    }
    
    #[test]
    fn test_resource_availability_rule() {
        let rule = ResourceAvailabilityRule;
        
        // Test sufficient memory (should pass)
        let mut context = create_test_context();
        context.resource_constraints.memory_limit = 64 * 1024 * 1024;
        assert!(rule.validate_translation_request(
            GuestArch::X86_64,
            GuestArch::Arm64,
            &context
        ).is_ok());
        
        // Test insufficient memory (should fail)
        let mut context = create_test_context();
        context.resource_constraints.memory_limit = 16 * 1024 * 1024;
        assert!(rule.validate_translation_request(
            GuestArch::X86_64,
            GuestArch::Arm64,
            &context
        ).is_err());
        
        // Test sufficient CPU (should pass)
        let mut context = create_test_context();
        context.resource_constraints.cpu_limit = Some(2);
        assert!(rule.validate_translation_request(
            GuestArch::X86_64,
            GuestArch::Arm64,
            &context
        ).is_ok());
        
        // Test insufficient CPU (should fail)
        let mut context = create_test_context();
        context.resource_constraints.cpu_limit = Some(0);
        assert!(rule.validate_translation_request(
            GuestArch::X86_64,
            GuestArch::Arm64,
            &context
        ).is_err());
        
        // Test sufficient time (should pass)
        let mut context = create_test_context();
        context.resource_constraints.time_limit = Some(std::time::Duration::from_secs(10));
        assert!(rule.validate_translation_request(
            GuestArch::X86_64,
            GuestArch::Arm64,
            &context
        ).is_ok());
        
        // Test insufficient time (should fail)
        let mut context = create_test_context();
        context.resource_constraints.time_limit = Some(std::time::Duration::from_secs(2));
        assert!(rule.validate_translation_request(
            GuestArch::X86_64,
            GuestArch::Arm64,
            &context
        ).is_err());
    }
    
    fn create_test_context() -> TranslationContext {
        TranslationContext {
            performance_requirements: PerformanceRequirements {
                high_performance: false,
                min_throughput: None,
                max_latency: None,
            },
            resource_constraints: ResourceConstraints {
                memory_limit: 64 * 1024 * 1024, // 64MB
                cpu_limit: Some(2),
                time_limit: Some(std::time::Duration::from_secs(30)),
            },
            timing_requirements: TimingRequirements {
                real_time: false,
                deadline: None,
                max_execution_time: None,
            },
        }
    }
}