//! Domain services module
//!
//! This module contains domain services that encapsulate business logic
//! that doesn't naturally fit within a single aggregate or entity.
//! Domain services are stateless and coordinate between multiple aggregates
//! or encapsulate complex business rules.

pub mod vm_lifecycle_service;
pub mod translation_strategy_service;
pub mod optimization_pipeline_service;
pub mod adaptive_optimization_service;
pub mod resource_management_service;
pub mod cache_management_service;
pub mod target_optimization_service;
pub mod cross_architecture_translation_service;
pub mod register_allocation_service;
pub mod architecture_compatibility_service;
pub mod performance_optimization_service;
pub mod rules;
pub mod events;

pub use vm_lifecycle_service::VmLifecycleDomainService;
pub use translation_strategy_service::TranslationStrategyDomainService;
pub use optimization_pipeline_service::OptimizationPipelineDomainService;
pub use adaptive_optimization_service::AdaptiveOptimizationDomainService;
pub use resource_management_service::ResourceManagementDomainService;
pub use cache_management_service::CacheManagementDomainService;
pub use target_optimization_service::TargetOptimizationDomainService;
pub use cross_architecture_translation_service::CrossArchitectureTranslationDomainService;
pub use register_allocation_service::RegisterAllocationDomainService;
pub use architecture_compatibility_service::ArchitectureCompatibilityDomainService;
pub use performance_optimization_service::PerformanceOptimizationDomainService;
pub use events::{DomainEventBus, DomainEventEnum};