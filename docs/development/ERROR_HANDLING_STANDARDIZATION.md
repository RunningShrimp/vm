# Error Handling Standardization Guide

## Overview

This document provides guidelines for consistent error handling across the VM project. It describes the unified error framework and best practices for error management.

## Unified Error Framework

The VM project uses a centralized error handling framework defined in `vm-core/src/error.rs` and `vm-core/src/foundation/error.rs`. All crates should use this unified framework for consistency.

### Core Error Types

The primary error type is `VmError`, which encompasses several specialized error categories:

- `CoreError` - Core system errors (config, state, internal errors)
- `MemoryError` - Memory management errors (access violations, mapping failures)
- `ExecutionError` - Execution engine errors (faults, invalid instructions)
- `DeviceError` - Device emulation errors
- `PlatformError` - Platform-specific and hardware virtualization errors
- `TranslationError` - Instruction translation errors
- `JitError` - JIT compilation errors
- `ConfigError` - Configuration errors
- `NetworkError` - Network-related errors

## Standardization Patterns

### 1. Use thiserror for Error Definitions

All error types should use the `thiserror` crate for automatic implementation of `std::error::Error` trait:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MyError {
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

### 2. Convert Local Errors to VmError

Each crate should implement `From` conversions from local errors to `VmError`:

```rust
impl From<MyError> for vm_core::VmError {
    fn from(err: MyError) -> Self {
        match err {
            MyError::InvalidParameter(msg) => {
                vm_core::VmError::Core(vm_core::CoreError::InvalidParameter {
                    name: "unknown".to_string(),
                    value: "".to_string(),
                    message: msg,
                })
            }
            MyError::Io(io_err) => {
                vm_core::VmError::Io(io_err.to_string())
            }
        }
    }
}
```

### 3. Use Error Context

Add context to errors using the `ErrorContext` trait:

```rust
use vm_core::error::ErrorContext;

let result = risky_operation()
    .context("Failed to initialize device")?;
```

### 4. Consistent Error Message Formatting

Error messages should follow these conventions:

- Start with a lowercase letter
- Be descriptive but concise
- Include relevant values (addresses, sizes, etc.)
- Use consistent formatting for hex values (`{:#x}`)

Good examples:
- `"memory access violation at address {:#x}"`
- `"failed to allocate {} bytes"`
- `"device '{}' not found"`

Poor examples:
- `"Error"` (too vague)
- `"ERROR: Failed"` (redundant prefix, inconsistent capitalization)
- `"something went wrong"` (not actionable)

### 5. Error Result Types

Define result type aliases for convenience:

```rust
pub type MyResult<T> = Result<T, MyError>;
```

When using VmError:
```rust
pub type VmResult<T> = Result<T, VmError>;
```

## Current State and Migration Plan

### Modules with Unified Error Handling

The following modules properly use the unified error framework:

- `vm-core/src/error.rs` - Core error definitions
- `vm-core/src/foundation/error.rs` - Foundation error types
- `vm-engine/src/jit/common/error.rs` - JIT-specific error handling
- `vm-device/src/io.rs` - I/O errors (with conversion to VmError)
- `vm-plugin/src/plugin_loader.rs` - Plugin loading errors

### Modules Requiring Standardization

The following modules have local error types that should be standardized:

1. **vm-smmu/src/error.rs** - `SmmuError` should convert to VmError
2. **vm-platform/src/memory.rs** - `MemoryError` conflicts with core MemoryError
3. **vm-boot/src/iso9660.rs** - `IsoError` should convert to VmError
4. **vm-boot/src/eltorito.rs** - `EltoritoError` should convert to VmError
5. **vm-boot/src/hotplug.rs** - `HotplugError` should convert to VmError
6. **vm-boot/src/snapshot.rs** - `SnapshotError` should convert to VmError
7. **vm-platform/src/passthrough.rs** - `PassthroughError` should convert to VmError
8. **vm-interface/src/config_validator.rs** - Partial conversion implemented
9. **vm-accel/src/lib.rs** - Multiple error types to standardize
10. **vm-cross-arch-support** - Multiple error types in different modules

### Migration Strategy

For each module requiring standardization:

1. Keep the local error type (for backward compatibility)
2. Add `From<LocalError> for VmError` implementation
3. Add `From<LocalError> for vm_core::VmError` if in different crate
4. Update public APIs to return `VmResult<T>` where appropriate
5. Use `.map_err(Into::into).context(...)` for intermediate conversions

### Example Migration

Before:
```rust
pub enum SmmuError {
    ConfigError(String),
    TranslationError(u64),
}

pub fn configure_smmu(&mut self) -> Result<(), SmmuError> {
    // ...
}
```

After:
```rust
use vm_core::{VmError, CoreError};

pub enum SmmuError {
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("Translation error at address {:#x}", address)]
    TranslationError { address: u64 },
}

impl From<SmmuError> for VmError {
    fn from(err: SmmuError) -> Self {
        match err {
            SmmuError::ConfigError(msg) => {
                VmError::Core(CoreError::Config {
                    message: msg,
                    path: Some("smmu".to_string()),
                })
            }
            SmmuError::TranslationError { address } => {
                VmError::Memory(MemoryError::PageTableError {
                    message: format!("Translation failed for address {:#x}", address),
                    level: None,
                })
            }
        }
    }
}

// Keep local API for compatibility
pub type SmmuResult<T> = Result<T, SmmuError>;

// New API using unified error
pub fn configure_smmu_unified(&mut self) -> VmResult<()> {
    self.configure_smmu().map_err(Into::into)
}
```

## Error Recovery Strategies

The framework provides built-in support for error recovery:

```rust
use vm_core::error::{ErrorRecovery, retry_with_strategy, ErrorRecoveryStrategy};

// Check if error is recoverable
if error.is_retryable() {
    // Retry with exponential backoff
    let result = retry_with_strategy(
        || risky_operation(),
        ErrorRecoveryStrategy::ExponentialBackoff {
            max_attempts: 3,
            initial_delay_ms: 100,
            max_delay_ms: 5000,
            multiplier: 2.0,
        }
    )?;
}
```

## Best Practices

### DO:
- Use `thiserror::Error` derive macro
- Implement `From<VmError>` for local errors if needed
- Add context with `.context()` method
- Use specific error variants (not just String)
- Document recoverable vs non-recoverable errors
- Include relevant diagnostic information in errors

### DON'T:
- Create duplicate error types across modules
- Use bare `String` or `&str` errors without structure
- Silently ignore errors without logging
- Use `.unwrap()` or `.expect()` in production code
- Create error types that don't implement `std::error::Error`

## Testing Error Handling

Test error paths and conversions:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_conversion() {
        let local_error = MyError::InvalidParameter("test".to_string());
        let vm_error: VmError = local_error.into();
        assert!(matches!(vm_error, VmError::Core(..)));
    }

    #[test]
    fn test_error_context() {
        let result: Result<(), _> = Err(MyError::InvalidParameter("test".into()));
        let result = result.context("during initialization");
        assert!(result.is_err());
    }
}
```

## Performance Considerations

- Error variants should be lightweight (prefer `&str` over `String` where possible)
- Avoid allocations in hot error paths
- Use error codes/ints in performance-critical sections
- Consider using `Arc` for large error data structures

## Related Documentation

- [vm-core/src/error.rs](../../vm-core/src/error.rs) - Core error definitions
- [vm-core/src/foundation/error.rs](../../vm-core/src/foundation/error.rs) - Foundation error types
- [CODE_STYLE.md](./CODE_STYLE.md) - General coding style guidelines
