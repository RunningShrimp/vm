//! Business rules module
//!
//! This module contains business rule implementations that can be used
//! by domain services to validate business logic.

pub mod lifecycle_rules;
pub mod translation_rules;
pub mod optimization_pipeline_rules;

pub use lifecycle_rules::{LifecycleBusinessRule, VmStateTransitionRule, VmResourceAvailabilityRule};
pub use translation_rules::{TranslationBusinessRule, ArchitectureCompatibilityRule, PerformanceThresholdRule, ResourceAvailabilityRule};
pub use optimization_pipeline_rules::{OptimizationPipelineBusinessRule, PipelineConfigValidationRule, StageExecutionValidationRule, PipelineContinuationRule};