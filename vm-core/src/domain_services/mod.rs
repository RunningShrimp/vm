//! # Domain Services Module
//!
//! This module contains domain services that encapsulate business logic
//! that doesn't naturally fit within a single aggregate or entity.
//! Domain services are stateless and coordinate between multiple aggregates
//! or encapsulate complex business rules.
//!
//! ## Architecture Overview
//!
//! Domain services are a key part of Domain-Driven Design (DDD) and are used when:
//!
//! 1. **Business logic involves multiple aggregates** - Operations that require
//!    coordination between different domain entities
//! 2. **Business logic is stateless** - Operations that don't belong to a specific
//!    entity but are important to the domain
//! 3. **Business rules need to be enforced** - Cross-cutting concerns that apply
//!    across multiple parts of the domain
//!
//! ## Core Principles
//!
//! ### Stateless Operations
//! Domain services are stateless - they don't maintain internal state between
//! operations. All necessary state is passed as parameters or accessed through
//! repositories and aggregates.
//!
//! ### Business Rule Validation
//! Domain services enforce business rules through:
//! - **Business Rules** (`rules/`): Declarative business constraints
//! - **Domain Events**: Event-driven validation and notification
//! - **Repository Patterns**: Data access and persistence
//!
//! ### Domain-Driven Design Patterns
//!
//! This module implements several DDD patterns:
//!
//! - **Domain Services**: Stateless business logic coordination
//! - **Domain Events**: Event-driven communication (`events.rs`)
//! - **Business Rules**: Declarative rule engine (`rules/`)
//! - **Repositories**: Data access abstraction (via aggregate roots)
//! - **Value Objects**: Immutable domain concepts
//!
//! ## Module Organization
//!
//! ### Core Domain Services
//!
//! - **`vm_lifecycle_service`**: VM instance lifecycle management
//! - **`execution_manager_service`**: Execution context and scheduling
//! - **`translation_strategy_service`**: Translation strategy selection
//!
//! ### Optimization Services
//!
//! - **`optimization_pipeline_service`**: Multi-stage optimization coordination
//! - **`adaptive_optimization_service`**: Adaptive/hotspot-based optimization
//! - **`performance_optimization_service`**: Performance bottleneck analysis
//! - **`target_optimization_service`**: Target-specific optimizations
//!
//! ### Resource Management Services
//!
//! - **`resource_management_service`**: Resource allocation and constraints
//! - **`cache_management_service`**: Cache hierarchy management
//! - **`tlb_management_service`**: TLB management and optimization
//! - **`page_table_walker_service`**: Page table traversal
//!
//! ### Translation Services
//!
//! - **`cross_architecture_translation_service`**: Cross-architecture translation
//! - **`register_allocation_service`**: Register allocation algorithms
//! - **`architecture_compatibility_service`**: Architecture compatibility checks
//!
//! ### Supporting Modules
//!
//! - **`rules/`**: Business rule implementations
//! - **`events`**: Domain event definitions and event bus
//!
//! ## Domain Events Architecture
//!
//! All domain services publish domain events to notify other parts of the system:
//!
//! ```rust
//! use crate::domain_services::events::{DomainEventEnum, OptimizationEvent};
//!
//! // Services publish events when important state changes occur
//! service.publish_optimization_event(OptimizationEvent::HotspotsDetected {
//!     count: hotspots.len(),
//!     threshold: config.hotspot_threshold,
//! })?;
//! ```
//!
//! ### Event Types
//!
//! - **Translation Events**: Architecture compatibility, strategy selection
//! - **Optimization Events**: Pipeline stages, hotspots, resource allocation
//! - **Execution Events**: Context lifecycle, scheduling, completion
//! - **TLB Events**: Entry insertion, eviction, flush operations
//! - **Page Table Events**: Page faults, access violations
//!
//! ## Usage Patterns
//!
//! ### Creating a Domain Service
//!
//! ```rust
//! use crate::domain_services::MyDomainService;
//! use std::sync::Arc;
//!
//! let service = MyDomainService::new(config);
//! let service_with_event_bus = service.with_event_bus(event_bus);
//! ```
//!
//! ### Using Business Rules
//!
//! ```rust
//! use crate::domain_services::rules::MyBusinessRule;
//!
//! let mut service = MyDomainService::new(config);
//! service.add_business_rule(Box::new(MyBusinessRule::new()));
//! ```
//!
//! ### Publishing Domain Events
//!
//! Domain services automatically publish events when operations occur:
//! - Configuration creation/completion
//! - Stage execution (start/completion/failure)
//! - Resource operations (allocation/release)
//! - Cache operations (hit/miss/eviction)
//! - Performance metrics (thresholds, bottlenecks)
//!
//! ## Integration with Aggregates
//!
//! Domain services work with aggregate roots to coordinate business logic:
//!
//! ```rust
//! use crate::aggregate_root::VirtualMachineAggregate;
//!
//! let mut aggregate = VirtualMachineAggregate::new(...);
//! service.execute_optimization(&config, &mut aggregate)?;
//! ```
//!
//! ## Error Handling
//!
//! Domain services use `VmResult<T>` for error handling:
//! - Invalid configuration returns `VmError::Core(CoreError::InvalidConfig)`
//! - Invalid state returns `VmError::Core(CoreError::InvalidState)`
//! - Business rule violations return appropriate `VmError` variants

// Re-export DomainEventBus for use in other crates
pub use crate::domain_event_bus::DomainEventBus;

pub mod adaptive_optimization_service;
pub mod architecture_compatibility_service;
pub mod cache_management_service;
pub mod config;
pub mod cross_architecture_translation_service;
pub mod event_store;
pub mod events;
pub mod execution_manager_service;
pub mod persistent_event_bus;
pub mod optimization_pipeline_service;
pub mod page_table_walker_service;
pub mod performance_optimization_service;
pub mod register_allocation_service;
pub mod resource_management_service;
pub mod rules;
pub mod target_optimization_service;
pub mod tlb_management_service;
pub mod translation_strategy_service;
pub mod vm_lifecycle_service;

pub use adaptive_optimization_service::AdaptiveOptimizationDomainService;
pub use architecture_compatibility_service::ArchitectureCompatibilityDomainService;
pub use cache_management_service::CacheManagementDomainService;
pub use config::{BaseServiceConfig, ServiceConfig, ServiceConfigBuilder};
pub use cross_architecture_translation_service::CrossArchitectureTranslationDomainService;
pub use events::{DomainEventEnum, ExecutionEvent, PageTableEvent, TlbEvent};
pub use execution_manager_service::{
    ExecutionContext, ExecutionManagerDomainService, ExecutionPriority, ExecutionState,
    ExecutionStatistics,
};
pub use optimization_pipeline_service::OptimizationPipelineDomainService;
pub use page_table_walker_service::{
    PageTableEntry, PageTableEntryFlags, PageTableLevel, PageTableWalkerDomainService, WalkResult,
    WalkStatistics,
};
pub use performance_optimization_service::PerformanceOptimizationDomainService;
pub use register_allocation_service::RegisterAllocationDomainService;
pub use resource_management_service::ResourceManagementDomainService;
pub use target_optimization_service::TargetOptimizationDomainService;
pub use tlb_management_service::{TlbLevel, TlbManagementDomainService};
pub use translation_strategy_service::TranslationStrategyDomainService;
pub use vm_lifecycle_service::VmLifecycleDomainService;
