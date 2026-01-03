//! Error types for garbage collection

/// GC operation result type
pub type GcResult<T> = Result<T, GcError>;

/// Errors that can occur during garbage collection
#[derive(Debug, thiserror::Error)]
pub enum GcError {
    /// Allocation failed
    #[error("GC allocation failed: {0}")]
    AllocationFailed(String),

    /// Collection failed
    #[error("GC collection failed: {0}")]
    CollectionFailed(String),

    /// Invalid configuration
    #[error("Invalid GC configuration: {0}")]
    InvalidConfig(String),

    /// Heap overflow
    #[error("Heap overflow: requested {requested} bytes, available {available} bytes")]
    HeapOverflow {
        /// Requested bytes
        requested: usize,
        /// Available bytes
        available: usize,
    },

    /// GC cycle already in progress
    #[error("GC cycle already in progress")]
    CycleInProgress,

    /// Object not found
    #[error("Object not found: {0}")]
    ObjectNotFound(String),

    /// Strategy-specific error
    #[error("Strategy error: {0}")]
    StrategyError(String),
}

impl GcError {
    /// Create an allocation failed error
    pub fn allocation_failed(msg: impl Into<String>) -> Self {
        Self::AllocationFailed(msg.into())
    }

    /// Create a collection failed error
    pub fn collection_failed(msg: impl Into<String>) -> Self {
        Self::CollectionFailed(msg.into())
    }

    /// Create an invalid config error
    pub fn invalid_config(msg: impl Into<String>) -> Self {
        Self::InvalidConfig(msg.into())
    }

    /// Create a heap overflow error
    pub fn heap_overflow(requested: usize, available: usize) -> Self {
        Self::HeapOverflow {
            requested,
            available,
        }
    }

    /// Create a strategy error
    pub fn strategy_error(msg: impl Into<String>) -> Self {
        Self::StrategyError(msg.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = GcError::allocation_failed("test");
        assert!(err.to_string().contains("test"));
    }

    #[test]
    fn test_heap_overflow() {
        let err = GcError::heap_overflow(1024, 512);
        assert!(err.to_string().contains("1024"));
        assert!(err.to_string().contains("512"));
    }
}
