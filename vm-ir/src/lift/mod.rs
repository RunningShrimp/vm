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
//! use vm_ir::lift::decoder::{InstructionDecoder, create_decoder};
//! use vm_ir::lift::semantics::create_semantics;
//! use vm_ir::lift::{ISA, LiftingContext};
//!
//! // 创建上下文
//! let mut ctx = LiftingContext::new(ISA::X86_64);
//!
//! // 解码指令
//! let decoder = create_decoder(ISA::X86_64);
//! let bytes = vec![0x90]; // NOP
//! let (instr, _) = decoder.decode(&bytes).expect("valid instruction bytes");
//!
//! // 提升为语义
//! let semantics = create_semantics(ISA::X86_64);
//! let ir = semantics.lift(&instr, &mut ctx).expect("valid instruction");
//!
//! println!("Generated IR:\n{}", ir);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;
use vm_core::VmError;

pub mod decoder;
#[cfg(feature = "llvm")]
pub mod inkwell_integration;
pub mod ir_gen;
pub mod optimizer;
pub mod riscv64_semantics;
pub mod semantics;

// Re-export commonly used types
pub use decoder::{ISA, Instruction, InstructionDecoder, OperandType, create_decoder};
pub use ir_gen::{BasicBlock, IRBuilder};
pub use optimizer::{OptimizationLevel, OptimizationPreset, OptimizationStats, PassManager};
// #[cfg(feature = "llvm")]
// pub use llvm_integration::{
//     LLVMCodeGenerator, LLVMContext, LLVMFunction as LLVMFunc, LLVMFunctionBuilder,
// LLVMModule,     LLVMPassExecutor, OptimizationRunStats,
// };
#[cfg(feature = "llvm")]
pub use inkwell_integration::{
    InkwellBuilder, InkwellCodeGenerator, InkwellContext, InkwellModule,
};
// pub use optimizer::{OptimizationLevel, OptimizationPreset, OptimizationStats, PassManager};
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

pub type LiftResult<T> = Result<T, VmError>;

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
