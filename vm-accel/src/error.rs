//! Error handling utilities for vm-accel
//!
//! This module provides helper macros and utilities for consistent
//! error handling across the vm-accel module, ensuring all errors
//! are properly converted to VmError with useful context.

// Imports are used in macro expansions (expanded at call site)
#[allow(unused_imports)]
use vm_core::{VmError, PlatformError, ExecutionError};

/// Add context to an error for better debugging
///
/// This macro wraps an error with module and operation context,
/// making it easier to trace where errors occurred.
///
/// # Example
///
/// ```rust,ignore
/// use vm_accel::error::error_context;
///
/// return Err(error_context!(
///     VmError::Platform(PlatformError::AccessDenied(
///         "Failed to map memory".to_string()
///     )),
///     "vm-accel::hvf",
///     "map_memory"
/// ));
/// ```
#[macro_export]
macro_rules! error_context {
    ($error:expr, $module:expr, $operation:expr) => {
        $error.with_context($module, $operation)
    };
}

/// Create a platform error with consistent formatting
///
/// # Example
///
/// ```rust,ignore
/// use vm_accel::error::platform_error;
///
/// return Err(platform_error!("Failed to create vCPU {}", id));
/// ```
#[macro_export]
macro_rules! platform_error {
    ($message:expr) => {
        VmError::Platform(PlatformError::AccessDenied($message.to_string()))
    };
    ($variant:ident, $message:expr) => {
        VmError::Platform(PlatformError::$variant($message.to_string()))
    };
}

/// Create a resource allocation failed error
///
/// # Example
///
/// ```rust,ignore
/// use vm_accel::error::resource_error;
///
/// return Err(resource_error!("Failed to allocate memory for vCPU {}", id));
/// ```
#[macro_export]
macro_rules! resource_error {
    ($message:expr) => {
        VmError::Platform(PlatformError::ResourceAllocationFailed($message.to_string()))
    };
}

/// Create an execution failed error
///
/// # Example
///
/// ```rust,ignore
/// use vm_accel::error::execution_error;
///
/// return Err(execution_error!("vCPU run failed: {}", e));
/// ```
#[macro_export]
macro_rules! execution_error {
    ($message:expr) => {
        VmError::Execution(ExecutionError::JitError {
            message: $message.to_string(),
            function_addr: None,
        })
    };
}

/// Helper trait for adding context to errors
pub trait ErrorContext<T> {
    /// Add module and operation context to an error
    fn with_context(self, module: &'static str, operation: &'static str) -> T;
    /// Add detailed context message to an error
    fn detail(self, message: String) -> T;
}

impl ErrorContext<VmError> for VmError {
    fn with_context(self, module: &'static str, operation: &'static str) -> VmError {
        VmError::WithContext {
            error: Box::new(self),
            context: format!("[{}::{}] {}", module, operation, "operation failed"),
            backtrace: None,
        }
    }

    fn detail(self, message: String) -> VmError {
        VmError::WithContext {
            error: Box::new(self),
            context: message,
            backtrace: None,
        }
    }
}

/// Result type alias for vm-accel operations
pub type AccelResult<T> = Result<T, VmError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_error_macro() {
        let err = platform_error!("Test error");
        assert!(matches!(err, VmError::Platform(_)));
    }

    #[test]
    fn test_resource_error_macro() {
        let err = resource_error!("Test resource");
        assert!(matches!(err, VmError::Platform(PlatformError::ResourceAllocationFailed(_))));
    }

    #[test]
    fn test_execution_error_macro() {
        let err = execution_error!("Test execution");
        assert!(matches!(err, VmError::Execution(_)));
    }

    #[test]
    fn test_error_context() {
        let base_err = VmError::Platform(PlatformError::AccessDenied("test".to_string()));
        let ctx_err = base_err.with_context("vm-accel", "test_op");
        assert!(matches!(ctx_err, VmError::WithContext { .. }));
    }
}
