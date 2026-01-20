//! # JIT Engine Domain Model
//!
//! This module defines the domain model and bounded contexts for the JIT engine,
//! following Domain-Driven Design (DDD) principles.
//!
//! ## Architecture Overview
//!
//! The domain layer is organized into several bounded contexts, each representing
//! a specific area of concern within the JIT compilation system:
//!
//! - **Compilation**: Manages the transformation of IR blocks to machine code
//! - **Optimization**: Handles various optimization passes and strategies
//! - **Execution**: Manages code execution environments and strategies
//! - **Caching**: Provides caching services for compiled code and intermediate results
//! - **Monitoring**: Tracks metrics, health checks, and alerts
//! - **Hardware Acceleration**: Manages hardware acceleration features
//! - **Service**: Provides a unified domain service integrating all bounded contexts
//!
//! ## Design Principles
//!
//! ### Domain-Driven Design (DDD)
//!
//! Each bounded context encapsulates:
//! - **Entities**: Objects with unique identities (e.g., `CompilationId`, `ExecutionId`)
//! - **Value Objects**: Immutable objects describing domain concepts (e.g., `CompilationConfig`)
//! - **Aggregates**: Clusters of domain objects treated as a unit (e.g., `CompilationContext`)
//! - **Domain Services**: Stateless operations that don't naturally fit within entities
//! - **Repositories**: Abstractions for data persistence (where applicable)
//!
//! ### Separation of Concerns
//!
//! Each bounded context is independently testable and maintainable:
//! - Clear interfaces between contexts
//! - Minimal dependencies between modules
//! - Shared types defined in common module
//!
//! ### Configuration Management
//!
//! All bounded contexts support:
//! - **Validation**: Ensures configuration correctness before use
//! - **Merging**: Combines multiple configurations intelligently
//! - **Summary**: Human-readable configuration descriptions
//!
//! ## Usage Examples
//!
//! ### Basic Compilation
//!
//! ```ignore
//! use vm_engine_jit::domain::{compilation::CompilationService, CompilationConfig};
//!
//! let service = CompilationService::new(
//!     Box::new(compiler_factory),
//!     Box::new(optimizer_factory),
//!     Box::new(codegen_factory),
//! );
//!
//! let config = CompilationConfig::default();
//! let result = service.compile(ir_block, config)?;
//! ```
//!
//! ### Using the Unified Domain Service
//!
//! ```ignore
//! use vm_engine_jit::domain::service::{JITEngineDomainService, JITEngineConfig};
//!
//! let config = JITEngineConfig::default();
//! let mut service = JITEngineDomainService::new(config)?;
//!
//! let request = JITEngineRequest {
//!     request_id: 1,
//!     ir_block: my_ir_block,
//!     options: JITEngineOptions::default(),
//! };
//!
//! let response = service.process_request(request)?;
//! ```
//!
//! ## Module Structure
//!
//! ### Bounded Contexts
//!
//! Each bounded context follows a consistent structure:
//! - **Types**: Domain-specific types (enums, structs)
//! - **Context**: Aggregate roots managing domain operations
//! - **Service**: Domain service for complex operations
//! - **Traits**: Abstract interfaces for extensibility
//! - **Stats**: Statistics tracking and reporting
//!
//! ### Common Patterns
//!
//! All contexts implement these common patterns:
//! - **Id Generation**: Atomic counters for unique IDs
//! - **Status Tracking**: State machines for lifecycle management
//! - **Error Handling**: Domain-specific error types
//! - **Statistics**: Performance metrics and monitoring
//!
//! ## Integration Points
//!
//! ### With VM Core
//! - Uses `GuestAddr` for memory addressing
//! - Uses `MMU` for memory management
//! - Uses `ExecStatus` and `ExecStats` for execution results
//!
//! ### With IR Layer
//! - Consumes `IRBlock` for compilation
//! - Produces optimized `IRBlock` after optimization
//!
//! ### With Foundation Layer
//! - Uses common configuration traits (`Config`, `Stats`)
//! - Uses common error types (`VmError`, `JITResult`)

pub mod compilation;
pub mod optimization;
pub mod execution;
pub mod caching;
pub mod monitoring;
pub mod hardware_acceleration;
pub mod service;

// Re-export main types from each bounded context
// (suppress ambiguous glob reexport warnings)
#[allow(ambiguous_glob_reexports)]
pub use compilation::*;
#[allow(ambiguous_glob_reexports)]
pub use optimization::*;
#[allow(ambiguous_glob_reexports)]
pub use execution::*;
#[allow(ambiguous_glob_reexports)]
pub use caching::*;
#[allow(ambiguous_glob_reexports)]
pub use monitoring::*;
#[allow(ambiguous_glob_reexports)]
pub use hardware_acceleration::*;
#[allow(ambiguous_glob_reexports)]
pub use service::*;
