//! # vm-cross-arch - 跨架构指令转换层
//!
//! 实现不同架构之间等价转换层，支持AMD64、ARM64、RISC-V64架构指令之间的转换。
//!
//! ## 架构设计
//!
//! ```text
//! 源架构指令 (AMD64/ARM64/RISC-V64)
//!     ↓ (解码器)
//! 统一IR (vm-ir::IRBlock)
//!     ↓ (架构编码器)
//! 目标架构指令 (AMD64/ARM64/RISC-V64)
//! ```
//!
//! ## 使用示例
//!
//! ```rust,ignore
//! use vm_cross_arch::{ArchTranslator, SourceArch, TargetArch};
//! use vm_ir::IRBlock;
//!
//! // 创建转换器
//! let translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
//!
//! // 转换IR块
//! let source_block: IRBlock = ...;
//! let target_instructions = translator.translate_block(&source_block)?;
//! ```
//!
//! ## 支持的转换
//!
//! - AMD64 ↔ ARM64
//! - AMD64 ↔ RISC-V64
//! - ARM64 ↔ RISC-V64

mod auto_executor;
mod cache_optimizer;
mod cross_arch_aot;
mod cross_arch_runtime;
mod encoder;
mod instruction_patterns;
mod integration;
mod os_support;
mod performance_optimizer;
mod register_mapping;
mod runtime;
mod translator;
mod unified_executor;
mod vm_service_ext;

pub use auto_executor::{AutoExecutor, UnifiedDecoder};
pub use cache_optimizer::{CacheConfig, CacheOptimizer, CachePolicy, CacheStats};
pub use cross_arch_aot::{CrossArchAotCompiler, CrossArchAotConfig, CrossArchAotStats};
pub use cross_arch_runtime::{
    AotIntegrationConfig, CrossArchRuntime, CrossArchRuntimeConfig, GcIntegrationConfig,
    JitIntegrationConfig,
};
pub use encoder::{ArchEncoder, Arm64Encoder, Riscv64Encoder, X86_64Encoder};
pub use integration::{CrossArchVm, CrossArchVmBuilder};
pub use os_support::{
    DeviceEmulator, DeviceType, InterruptController, LinuxSyscallHandler, SyscallHandler,
};
pub use performance_optimizer::{PerformanceConfig, PerformanceOptimizer};
pub use register_mapping::{RegisterMapper, RegisterMapping};
pub use runtime::{CrossArchConfig, CrossArchStrategy, HostArch};
pub use translator::{ArchTranslator, SourceArch, TargetArch, TranslationError};
pub use unified_executor::{ExecutionStats, UnifiedExecutor};
pub use vm_service_ext::{VmConfigExt, create_auto_vm_config};

use vm_core::VmError;
use vm_ir::IRBlock;

/// 架构类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Architecture {
    /// x86-64 (AMD64/Intel64)
    X86_64,
    /// ARM64 (AArch64)
    ARM64,
    /// RISC-V 64-bit
    RISCV64,
}

impl std::fmt::Display for Architecture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Architecture::X86_64 => write!(f, "x86_64"),
            Architecture::ARM64 => write!(f, "arm64"),
            Architecture::RISCV64 => write!(f, "riscv64"),
        }
    }
}

impl std::error::Error for Architecture {}

impl Architecture {
    /// 获取架构的寄存器数量
    pub fn register_count(&self) -> usize {
        match self {
            Architecture::X86_64 => 16,  // 通用寄存器
            Architecture::ARM64 => 32,   // X0-X30 + SP
            Architecture::RISCV64 => 32, // X0-X31
        }
    }

    /// 获取架构的指针大小（字节）
    pub fn pointer_size(&self) -> usize {
        8
    }

    /// 获取架构的指令对齐（字节）
    pub fn instruction_alignment(&self) -> usize {
        match self {
            Architecture::X86_64 => 1,  // x86-64指令可变长
            Architecture::ARM64 => 4,   // ARM64指令固定4字节
            Architecture::RISCV64 => 2, // RISC-V支持16位压缩指令
        }
    }
}

/// 目标架构指令表示
#[derive(Debug, Clone)]
pub struct TargetInstruction {
    /// 指令字节码
    pub bytes: Vec<u8>,
    /// 指令长度（字节）
    pub length: usize,
    /// 指令助记符（用于调试）
    pub mnemonic: String,
    /// 是否影响控制流
    pub is_control_flow: bool,
    /// 是否访问内存
    pub is_memory_op: bool,
}

/// 转换结果
#[derive(Debug, Clone)]
pub struct TranslationResult {
    /// 转换后的指令序列
    pub instructions: Vec<TargetInstruction>,
    /// 转换统计信息
    pub stats: TranslationStats,
}

/// 转换统计信息
#[derive(Debug, Clone, Default)]
pub struct TranslationStats {
    /// 转换的IR操作数
    pub ir_ops_translated: usize,
    /// 生成的目标指令数
    pub target_instructions_generated: usize,
    /// 需要多指令序列的复杂操作数
    pub complex_operations: usize,
    /// 寄存器映射次数
    pub register_mappings: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_architecture_properties() {
        assert_eq!(Architecture::X86_64.register_count(), 16);
        assert_eq!(Architecture::ARM64.register_count(), 32);
        assert_eq!(Architecture::RISCV64.register_count(), 32);
    }
}

#[cfg(test)]
mod integration_tests;
