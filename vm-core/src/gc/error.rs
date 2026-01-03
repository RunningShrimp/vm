//! GC 错误类型

use thiserror::Error;

/// GC 错误
#[derive(Error, Debug)]
pub enum GCError {
    /// 内存不足
    #[error("Out of memory: requested {requested} bytes, available {available} bytes")]
    OutOfMemory {
        /// 请求的大小
        requested: usize,
        /// 可用的大小
        available: usize,
    },

    /// 对象已释放
    #[error("Object {0:?} has been freed")]
    ObjectFreed(u64),

    /// 无效的对象指针
    #[error("Invalid object pointer: {0:?}")]
    InvalidPointer(u64),

    /// 堆损坏
    #[error("Heap corruption detected at address {0:#x}")]
    HeapCorruption(u64),

    /// GC 错误
    #[error("GC operation failed: {0}")]
    GCFailed(String),

    /// 配置错误
    #[error("Invalid GC configuration: {0}")]
    InvalidConfig(String),

    /// 不支持的操作
    #[error("Operation not supported: {0}")]
    NotSupported(String),
}

/// GC 结果类型
pub type GCResult<T> = Result<T, GCError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = GCError::OutOfMemory {
            requested: 1024,
            available: 512,
        };
        assert!(err.to_string().contains("Out of memory"));
    }

    #[test]
    fn test_object_freed_error() {
        let err = GCError::ObjectFreed(0x1234);
        assert!(err.to_string().contains("has been freed"));
    }
}
