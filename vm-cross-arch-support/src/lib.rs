//! # VM Cross-Architecture Support
//!
//! This crate provides consolidated cross-architecture utilities for VM translation,
//! combining encoding, memory access, instruction patterns, and register management.
//!
//! ## Modules
//!
//! - [`encoding`]: Common encoding framework for instruction encoding across architectures
//! - [`memory_access`]: Memory access patterns, alignment handling, and endianness conversion
//! - [`instruction_patterns`]: Instruction pattern matching and semantic analysis
//! - [`register`]: Register management, mapping, and allocation
//!
//! ## Features
//!
//! - Unified interfaces for multiple architectures (x86_64, ARM64, RISC-V64)
//! - Cross-architecture translation support
//! - Pattern matching for instruction optimization
//! - Register mapping and allocation utilities
//! - Memory access optimization and analysis

pub mod encoding;
pub mod instruction_patterns;
pub mod memory_access;
pub mod register;

// Re-exports for convenience
pub use encoding::{
    Architecture, EncodingContext, EncodingError, Endianness, ImmediateFormat, InstructionBuilder,
    InstructionEncoding, InstructionFlag, MemoryFlags as EncodingMemoryFlags,
    MemoryOperand as EncodingMemoryOperand, RegId, RegisterField,
};

pub use memory_access::{
    AccessType, AccessWidth, Alignment, AlignmentIssue, AnalysisResult, ConversionStrategy,
    DefaultMemoryAccessOptimizer, Endianness as MemoryEndianness, EndiannessConverter, Fix,
    FixCost, FixType, IssueSeverity, MemoryAccessAnalyzer, MemoryAccessOptimizer,
    MemoryAccessPattern, MemoryError, MemoryFlags, OptimizationType, OptimizedPattern,
};

pub use instruction_patterns::{
    ArithmeticType, BranchType, CompareType, ConvertType, DefaultPatternMatcher, IROp,
    InstructionCategory, InstructionFlags, InstructionPattern, LogicalType, MemoryType,
    OperandType, PatternError, PatternMatcher, SemanticDescription, SystemType, VectorType,
};

pub use register::{
    MappingStats, MappingStrategy, RegisterAllocator, RegisterClass, RegisterError, RegisterInfo,
    RegisterMapper, RegisterSet, RegisterType,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_architecture_enum() {
        let arch = Architecture::X86_64;
        assert_eq!(arch, Architecture::X86_64);
    }

    #[test]
    fn test_reg_id() {
        let reg_id = RegId(5);
        assert_eq!(reg_id.0, 5);
        assert_eq!(reg_id.to_string(), "5");
    }

    #[test]
    fn test_encoding_context() {
        let ctx = EncodingContext::new(Architecture::ARM64);
        assert_eq!(ctx.architecture, Architecture::ARM64);
        assert_eq!(ctx.address_size, 64);
    }
}
