//! Minimal CompilerBackend trait and a small DirectBackend implementation for tests

use crate::ExecutableBlock;
use std::fmt::Display;
use vm_ir::IRBlock;

/// Simple error type for backend compilation
#[derive(Debug)]
pub struct CompilerError {
    pub message: String,
}

impl Display for CompilerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CompilerError: {}", self.message)
    }
}

impl std::error::Error for CompilerError {}

/// Trait for compiler backends used by compile cache
pub trait CompilerBackend {
    fn compile(&mut self, block: &IRBlock) -> Result<ExecutableBlock, CompilerError>;
}

/// A trivial direct backend used in tests (returns a small dummy "machine code")
#[derive(Default)]
pub struct DirectBackend {}

impl CompilerBackend for DirectBackend {
    fn compile(&mut self, _block: &IRBlock) -> Result<ExecutableBlock, CompilerError> {
        // For test purposes, return a single byte representing a no-op/ret
        Ok(ExecutableBlock::new(vec![0xC3], 0)) // x86 RET (placeholder)
    }
}

pub type CompilerErrorBox = CompilerError;

/// 后端工厂 trait（用于动态选择后端）
pub trait BackendFactory {
    /// 创建后端实例
    fn create(&self) -> Box<dyn CompilerBackend>;
}

/// 默认后端工厂实现
pub struct DefaultBackendFactory;

impl BackendFactory for DefaultBackendFactory {
    fn create(&self) -> Box<dyn CompilerBackend> {
        #[cfg(feature = "cranelift-backend")]
        {
            use crate::cranelift_backend::CraneliftBackend;
            Box::new(CraneliftBackend::new())
        }

        #[cfg(not(feature = "cranelift-backend"))]
        {
            use crate::direct_backend::DirectBackend;
            Box::new(DirectBackend::default())
        }
    }
}

impl Default for DefaultBackendFactory {
    fn default() -> Self {
        Self
    }
}
