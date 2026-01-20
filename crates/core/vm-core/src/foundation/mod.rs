//! Foundation layer for VM project
//!
//! This crate provides the foundational components for the VM project,
//! including error handling, validation, resource management, and support utilities.
//!
//! ## Modules
//!
//! - [`error`]: Unified error handling framework
//! - [`validation`]: Common validation framework
//! - [`resource`]: Resource management framework
//! - [`support`]: Support utilities, macros, and testing tools

pub mod error;
pub mod resource;
pub mod validation;

// Support modules (all enabled with std feature)
pub mod support_macros;
pub mod support_test_helpers;
pub mod support_utils;

// Re-export common types
pub use error::utils as error_utils;
pub use error::{
    Architecture, ConfigError, CoreError, DeviceError, ErrorContext, ErrorContextExt,
    ErrorSeverity, GuestAddr, JITErrorBuilder, JITResult, JitError, MemoryError, NetworkError,
    RegId, TranslationError, VmError, VmResult,
};
pub use resource::{
    PoolStats, Resource, ResourceConfig, ResourceGuard, ResourceManager, ResourcePool,
    ResourceState, ResourceStats,
};
// Support re-exports (always available with std)
pub use support_test_helpers::*;
pub use support_utils::*;
pub use validation::rules::{NonEmptyRule, RangeRule, RegexRule};
pub use validation::validators::{
    ArchitectureValidator, MemoryAddressValidator, RegisterValidator, StringLengthValidator,
};
pub use validation::{
    ErrorSeverity as ValidationSeverity, ValidationError, ValidationResult, ValidationRule,
    ValidationWarning, Validator,
};
