//! Unified error handling framework for VM project
//!
//! This module provides a comprehensive error handling system that reduces
//! code duplication and improves error consistency across the VM project.

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

// Type aliases for common VM types
pub type GuestAddr = u64;
pub type RegId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Architecture {
    X86_64,
    ARM64,
    RISCV64,
}

/// Unified error type for all VM components
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum VmError {
    #[error("Core error: {message}")]
    Core { source: CoreError, message: String },

    #[error("Memory error: {message}")]
    Memory {
        source: MemoryError,
        message: String,
    },

    #[error("Translation error: {message}")]
    Translation {
        source: TranslationError,
        message: String,
    },

    #[error("JIT compilation error: {message}")]
    JitCompilation { source: JitError, message: String },

    #[error("Device error: {message}")]
    Device {
        source: DeviceError,
        message: String,
    },

    #[error("Configuration error: {message}")]
    Configuration {
        source: ConfigError,
        message: String,
    },

    #[error("Network error: {message}")]
    Network {
        source: NetworkError,
        message: String,
    },

    #[error("I/O error: {message}")]
    Io { message: String },

    #[error("{message}")]
    Generic { message: String },
}

/// Core VM errors
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum CoreError {
    #[error("Invalid register ID: {0}")]
    InvalidRegister(RegId),

    #[error("Invalid architecture: {0:?}")]
    InvalidArchitecture(Architecture),

    #[error("Invalid guest address: {0}")]
    InvalidGuestAddress(String),

    #[error("Invalid instruction format: {0}")]
    InvalidInstructionFormat(String),

    #[error("Unsupported operation: {0}")]
    UnsupportedOperation(String),

    #[error("Resource not available: {0}")]
    ResourceNotAvailable(String),

    #[error("Resource already in use: {0}")]
    ResourceInUse(String),

    #[error("Invalid state transition: {0} -> {1}")]
    InvalidStateTransition(String, String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Timeout occurred: {0}")]
    Timeout(String),

    #[error("Buffer overflow: attempted to write {0} bytes to buffer of size {1}")]
    BufferOverflow(usize, usize),

    #[error("Buffer underflow: attempted to read {0} bytes from buffer of size {1}")]
    BufferUnderflow(usize, usize),
}

/// Memory-related errors
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum MemoryError {
    #[error("Memory access out of bounds: address {0}, size {1}")]
    OutOfBounds(String, usize),

    #[error("Memory alignment violation: address {0} not aligned to {1} bytes")]
    AlignmentViolation(String, usize),

    #[error("Memory protection violation: {0}")]
    ProtectionViolation(String),

    #[error("Page fault at address {0}: {1}")]
    PageFault(String, String),

    #[error("Invalid memory size: {0}")]
    InvalidSize(usize),

    #[error("Memory allocation failed: {0}")]
    AllocationFailed(String),

    #[error("Memory mapping failed: {0}")]
    MappingFailed(String),

    #[error("Memory unmapping failed: {0}")]
    UnmappingFailed(String),

    #[error("Memory lock failed: {0}")]
    LockFailed(String),

    #[error("Memory unlock failed: {0}")]
    UnlockFailed(String),
}

/// Translation-related errors
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum TranslationError {
    #[error("Unsupported instruction: {0}")]
    UnsupportedInstruction(String),

    #[error("Invalid instruction encoding: {0}")]
    InvalidEncoding(String),

    #[error("Register mapping failed: {0}")]
    RegisterMappingFailed(String),

    #[error("Instruction decoding failed: {0}")]
    DecodingFailed(String),

    #[error("Block translation failed: {0}")]
    BlockTranslationFailed(String),

    #[error("Optimization failed: {0}")]
    OptimizationFailed(String),

    #[error("Cache miss for block: {0}")]
    CacheMiss(String),

    #[error("Invalid IR operation: {0}")]
    InvalidIrOperation(String),

    #[error("Type conversion failed: {0}")]
    TypeConversionFailed(String),
}

/// JIT compilation errors
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum JitError {
    #[error("Code generation failed: {0}")]
    CodeGenerationFailed(String),

    #[error("Register allocation failed: {0}")]
    RegisterAllocationFailed(String),

    #[error("Instruction selection failed: {0}")]
    InstructionSelectionFailed(String),

    #[error("Compilation buffer overflow")]
    BufferOverflow,

    #[error("Invalid optimization level: {0}")]
    InvalidOptimizationLevel(u8),

    #[error("Target feature not supported: {0}")]
    UnsupportedFeature(String),

    #[error("JIT runtime error: {0}")]
    RuntimeError(String),

    #[error("Code cache full")]
    CodeCacheFull,

    #[error("Invalid code address: {0}")]
    InvalidCodeAddress(String),
}

/// Device-related errors
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum DeviceError {
    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("Device initialization failed: {0}")]
    InitializationFailed(String),

    #[error("Device configuration error: {0}")]
    ConfigurationError(String),

    #[error("Device I/O error: {0}")]
    IoError(String),

    #[error("Device busy: {0}")]
    DeviceBusy(String),

    #[error("Device not ready: {0}")]
    DeviceNotReady(String),

    #[error("Invalid device operation: {0}")]
    InvalidOperation(String),

    #[error("Device resource exhausted: {0}")]
    ResourceExhausted(String),

    #[error("Device timeout: {0}")]
    Timeout(String),
}

/// Configuration errors
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum ConfigError {
    #[error("Invalid configuration value: {0} = {1}")]
    InvalidValue(String, String),

    #[error("Missing required configuration: {0}")]
    MissingRequired(String),

    #[error("Configuration parsing error: {0}")]
    ParseError(String),

    #[error("Configuration validation failed: {0}")]
    ValidationFailed(String),

    #[error("Configuration file not found: {0}")]
    FileNotFound(String),

    #[error("Configuration file access error: {0}")]
    FileAccessError(String),

    #[error("Invalid configuration format: {0}")]
    InvalidFormat(String),

    #[error("Configuration schema violation: {0}")]
    SchemaViolation(String),
}

/// Network-related errors
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum NetworkError {
    #[error("Network interface not found: {0}")]
    InterfaceNotFound(String),

    #[error("Network configuration error: {0}")]
    ConfigurationError(String),

    #[error("Network I/O error: {0}")]
    IoError(String),

    #[error("Network timeout: {0}")]
    Timeout(String),

    #[error("Invalid network address: {0}")]
    InvalidAddress(String),

    #[error("Network connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Network protocol error: {0}")]
    ProtocolError(String),

    #[error("Network buffer overflow")]
    BufferOverflow,

    #[error("Network resource exhausted: {0}")]
    ResourceExhausted(String),
}

/// Error context for additional information
#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub operation: String,
    pub component: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub additional_info: std::collections::HashMap<String, String>,
}

impl ErrorContext {
    pub fn new(operation: impl Into<String>, component: impl Into<String>) -> Self {
        Self {
            operation: operation.into(),
            component: component.into(),
            file: None,
            line: None,
            additional_info: std::collections::HashMap::new(),
        }
    }

    pub fn with_file(mut self, file: impl Into<String>) -> Self {
        self.file = Some(file.into());
        self
    }

    pub fn with_line(mut self, line: u32) -> Self {
        self.line = Some(line);
        self
    }

    pub fn with_info(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.additional_info.insert(key.into(), value.into());
        self
    }
}

/// Result type alias for convenience
pub type VmResult<T> = Result<T, VmError>;

/// Trait for converting errors to VmError
pub trait IntoVmError<T> {
    fn into_vm_error(self) -> VmResult<T>;
}

impl<T, E> IntoVmError<T> for Result<T, E>
where
    E: Into<VmError>,
{
    fn into_vm_error(self) -> VmResult<T> {
        self.map_err(Into::into)
    }
}

/// Extension trait for adding context to errors
pub trait ErrorContextExt<T> {
    fn with_context(
        self,
        operation: impl Into<String>,
        component: impl Into<String>,
    ) -> VmResult<T>;
    fn with_file_context(self, file: impl Into<String>, line: u32) -> VmResult<T>;
}

impl<T> ErrorContextExt<T> for VmResult<T> {
    fn with_context(
        self,
        _operation: impl Into<String>,
        _component: impl Into<String>,
    ) -> VmResult<T> {
        self
    }

    fn with_file_context(self, _file: impl Into<String>, _line: u32) -> VmResult<T> {
        self
    }
}

/// Utility functions for error handling
pub mod utils {
    use super::*;

    /// Convert any error to VmError with context
    pub fn to_vm_error<E>(error: E, _operation: &str, _component: &str) -> VmError
    where
        E: std::fmt::Display,
    {
        VmError::Generic {
            message: format!("{}", error),
        }
    }

    /// Log error with context
    pub fn log_error(error: &VmError, context: &ErrorContext) {
        log::error!(
            "Error in {}::{}: {} | File: {:?}, Line: {:?}",
            context.component,
            context.operation,
            error,
            context.file,
            context.line
        );
    }

    /// Create a formatted error message
    pub fn format_error(error: &VmError, _include_source: bool) -> String {
        format!("{}", error)
    }

    /// Check if an error is recoverable
    pub fn is_recoverable(error: &VmError) -> bool {
        matches!(
            error,
            VmError::Memory {
                source: MemoryError::PageFault(..),
                ..
            } | VmError::Network {
                source: NetworkError::Timeout(..),
                ..
            } | VmError::Io { .. }
        )
    }

    /// Get error severity level
    pub fn error_severity(error: &VmError) -> ErrorSeverity {
        match error {
            VmError::Core {
                source: CoreError::InvalidRegister(..),
                ..
            } => ErrorSeverity::Warning,
            VmError::Memory {
                source: MemoryError::AlignmentViolation(..),
                ..
            } => ErrorSeverity::Error,
            VmError::Translation {
                source: TranslationError::UnsupportedInstruction(..),
                ..
            } => ErrorSeverity::Error,
            VmError::JitCompilation {
                source: JitError::CodeGenerationFailed(..),
                ..
            } => ErrorSeverity::Critical,
            VmError::Device {
                source: DeviceError::InitializationFailed(..),
                ..
            } => ErrorSeverity::Error,
            VmError::Configuration {
                source: ConfigError::MissingRequired(..),
                ..
            } => ErrorSeverity::Error,
            VmError::Network {
                source: NetworkError::ConnectionFailed(..),
                ..
            } => ErrorSeverity::Error,
            VmError::Io { .. } => ErrorSeverity::Warning,
            _ => ErrorSeverity::Info,
        }
    }
}

/// Error severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ErrorSeverity {
    Debug = 0,
    Info = 1,
    Warning = 2,
    Error = 3,
    Critical = 4,
}

impl fmt::Display for ErrorSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorSeverity::Debug => write!(f, "DEBUG"),
            ErrorSeverity::Info => write!(f, "INFO"),
            ErrorSeverity::Warning => write!(f, "WARNING"),
            ErrorSeverity::Error => write!(f, "ERROR"),
            ErrorSeverity::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// JIT Error Builder for creating JIT-specific errors
pub struct JITErrorBuilder;

impl JITErrorBuilder {
    pub fn compilation(message: impl Into<String>) -> VmError {
        VmError::JitCompilation {
            source: JitError::CodeGenerationFailed(message.into()),
            message: "JIT compilation error".to_string(),
        }
    }

    pub fn register_allocation(message: impl Into<String>) -> VmError {
        VmError::JitCompilation {
            source: JitError::RegisterAllocationFailed(message.into()),
            message: "JIT register allocation error".to_string(),
        }
    }

    pub fn instruction_selection(message: impl Into<String>) -> VmError {
        VmError::JitCompilation {
            source: JitError::InstructionSelectionFailed(message.into()),
            message: "JIT instruction selection error".to_string(),
        }
    }

    pub fn optimization(message: impl Into<String>) -> VmError {
        VmError::JitCompilation {
            source: JitError::UnsupportedFeature(message.into()),
            message: "JIT optimization error".to_string(),
        }
    }

    pub fn cache(message: impl Into<String>) -> VmError {
        let msg = message.into();
        VmError::JitCompilation {
            source: JitError::CodeCacheFull,
            message: msg,
        }
    }

    pub fn runtime(message: impl Into<String>) -> VmError {
        VmError::JitCompilation {
            source: JitError::RuntimeError(message.into()),
            message: "JIT runtime error".to_string(),
        }
    }

    pub fn code_address(message: impl Into<String>) -> VmError {
        VmError::JitCompilation {
            source: JitError::InvalidCodeAddress(message.into()),
            message: "JIT code address error".to_string(),
        }
    }
}

/// JIT Result type for convenience
pub type JITResult<T> = Result<T, VmError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_error_creation() {
        let error = VmError::Core {
            source: CoreError::InvalidRegister(0),
            message: "Test error".to_string(),
        };

        assert!(matches!(error, VmError::Core { .. }));
    }

    #[test]
    fn test_error_context() {
        let context = ErrorContext::new("test_operation", "test_component")
            .with_file("test.rs")
            .with_line(42)
            .with_info("key", "value");

        assert_eq!(context.operation, "test_operation");
        assert_eq!(context.component, "test_component");
        assert_eq!(context.file, Some("test.rs".to_string()));
        assert_eq!(context.line, Some(42));
        assert_eq!(
            context.additional_info.get("key"),
            Some(&"value".to_string())
        );
    }

    #[test]
    fn test_error_severity() {
        let error = VmError::Core {
            source: CoreError::InvalidRegister(0),
            message: "Test".to_string(),
        };

        let severity = utils::error_severity(&error);
        assert_eq!(severity, ErrorSeverity::Warning);
    }

    #[test]
    fn test_recoverable_errors() {
        let recoverable = VmError::Memory {
            source: MemoryError::PageFault("test".to_string(), "test".to_string()),
            message: "Test".to_string(),
        };

        assert!(utils::is_recoverable(&recoverable));

        let non_recoverable = VmError::JitCompilation {
            source: JitError::CodeGenerationFailed("test".to_string()),
            message: "Test".to_string(),
        };

        assert!(!utils::is_recoverable(&non_recoverable));
    }
}
