//! # vm-ir-lift - 指令语义库与 LLVM IR 提升
//!
//! 将客体指令 (Guest ISA: x86-64/ARM64/RISC-V) 抬升为 LLVM IR 中间表示。
//!
//! ## 架构
//!
//! ```text
//! Guest Instruction Bytes
//!     ↓ (InstructionDecoder)
//! Normalized Instruction
//!     ↓ (Semantics)
//! LLVM IR
//!     ↓ (Optimization)
//! Optimized LLVM IR
//!     ↓ (Codegen)
//! Native Machine Code
//! ```
//!
//! ## 快速开始
//!
//! ```rust,no_run
//! use vm_ir_lift::{LiftingContext, ISA};
//! use vm_ir_lift::decoder::{create_decoder, InstructionDecoder};
//! use vm_ir_lift::semantics::create_semantics;
//!
//! // 创建上下文
//! let mut ctx = LiftingContext::new(ISA::X86_64);
//!
//! // 解码指令
//! let decoder = create_decoder(ISA::X86_64);
//! let bytes = vec![0x90]; // NOP
//! let (instr, _) = decoder.decode(&bytes)?;
//!
//! // 提升为语义
//! let semantics = create_semantics(ISA::X86_64);
//! let ir = semantics.lift(&instr, &mut ctx)?;
//!
//! println!("Generated IR:\n{}", ir);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use vm_core::{CoreError, ExecutionError, VmError};

pub mod arm64_semantics;
pub mod decoder;
pub mod ir_gen;
#[cfg(feature = "llvm")]
pub mod llvm_integration;
#[cfg(feature = "llvm")]
pub mod inkwell_integration;
pub mod optimizer;
pub mod riscv64_semantics;
pub mod semantics;

// Re-export commonly used types
pub use decoder::{ISA, Instruction, InstructionDecoder, OperandType, create_decoder};
pub use ir_gen::{BasicBlock, IRBuilder, IROptimizer, LLVMFunction};
#[cfg(feature = "llvm")]
pub use llvm_integration::{
    LLVMCodeGenerator, LLVMContext, LLVMFunction as LLVMFunc, LLVMFunctionBuilder, LLVMModule,
    LLVMPassExecutor, OptimizationRunStats,
};
#[cfg(feature = "llvm")]
pub use inkwell_integration::{
    InkwellCodeGenerator, InkwellContext, InkwellModule, InkwellBuilder,
};
pub use optimizer::{OptimizationLevel, OptimizationPreset, OptimizationStats, PassManager};
pub use semantics::{FlagsState, Semantics, X86_64Semantics, create_semantics};

/// 指令抬升上下文
#[derive(Clone)]
pub struct LiftingContext {
    /// ISA 类型
    pub isa: ISA,
    /// 指令缓存（已抬升的 IR）
    instruction_cache: Arc<RwLock<HashMap<Vec<u8>, String>>>,
}

impl LiftingContext {
    /// 创建新的提升上下文
    pub fn new(isa: ISA) -> Self {
        Self {
            isa,
            instruction_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 检查缓存中是否存在已抬升的 IR
    pub fn get_cached_ir(&self, bytes: &[u8]) -> Option<String> {
        self.instruction_cache.read().get(bytes).cloned()
    }

    /// 缓存已抬升的 IR
    pub fn cache_ir(&self, bytes: Vec<u8>, ir: String) {
        self.instruction_cache.write().insert(bytes, ir);
    }

    /// 获取缓存统计
    pub fn cache_stats(&self) -> usize {
        self.instruction_cache.read().len()
    }

    /// 清空缓存
    pub fn clear_cache(&self) {
        self.instruction_cache.write().clear();
    }
}

// LiftError 已被弃用，请使用 vm_core::VmError
//
// 注意：此类型在内部仍被广泛使用，保留以维护向后兼容性。
// 新代码应直接使用 vm_core::VmError，通过 From<LiftError> 转换。
//
// 迁移指南：
// - 旧代码: return Err(LiftError::DecodeError("...".to_string()))
// - 新代码: return Err(VmError::Execution(ExecutionError::InvalidInstruction { opcode: 0, pc: 0 }))
#[deprecated(note = "Use vm_core::VmError instead. This type is kept for backward compatibility.")]
#[derive(Debug, thiserror::Error)]
pub enum LiftError {
    #[error("Unsupported ISA: {0:?}")]
    UnsupportedISA(ISA),

    #[error("Decode error: {0}")]
    DecodeError(String),

    #[error("Semantic error: {0}")]
    SemanticError(String),

    #[error("IR generation error: {0}")]
    IRGenError(String),

    #[error("Unsupported instruction: {0}")]
    UnsupportedInstruction(String),
}

pub type LiftResult<T> = Result<T, VmError>;

impl From<LiftError> for VmError {
    fn from(err: LiftError) -> Self {
        match err {
            LiftError::UnsupportedISA(isa) => VmError::Core(CoreError::NotImplemented {
                feature: format!("ISA {:?}", isa),
                module: "vm-ir-lift".to_string(),
            }),
            LiftError::DecodeError(msg) => {
                VmError::Execution(ExecutionError::InvalidInstruction { opcode: 0, pc: 0 })
            }
            LiftError::SemanticError(msg) => {
                VmError::Execution(ExecutionError::InvalidInstruction { opcode: 0, pc: 0 })
            }
            LiftError::IRGenError(msg) => VmError::Execution(ExecutionError::JitError {
                message: msg,
                function_addr: None,
            }),
            LiftError::UnsupportedInstruction(msg) => VmError::Core(CoreError::NotImplemented {
                feature: msg,
                module: "vm-ir-lift".to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lifting_context_creation() {
        let ctx = LiftingContext::new(ISA::X86_64);
        assert_eq!(ctx.cache_stats(), 0);
    }

    #[test]
    fn test_instruction_cache() {
        let ctx = LiftingContext::new(ISA::X86_64);
        let bytes = vec![0x48, 0x89, 0xC3]; // mov rbx, rax
        let ir = "add i64 %a, %b".to_string();

        ctx.cache_ir(bytes.clone(), ir.clone());
        assert_eq!(ctx.cache_stats(), 1);
        assert_eq!(ctx.get_cached_ir(&bytes), Some(ir));
    }
}
