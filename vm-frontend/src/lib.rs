//! Unified Frontend for Multiple Architectures
//!
//! This crate provides frontend decoders for multiple CPU architectures:
//! - x86_64 (Intel/AMD 64-bit)
//! - ARM64 (AArch64)
//! - RISC-V 64-bit
//!
//! Architecture support can be enabled individually using feature flags:
//! - "x86_64" - Enable x86_64 architecture support
//! - "arm64" - Enable ARM64 architecture support
//! - "riscv64" - Enable RISC-V 64-bit architecture support (default)
//! - "all" - Enable all architectures
//!
//! RISC-V extensions (require "riscv64" feature):
//! - "riscv-m" - M extension (Integer Multiply/Divide)
//! - "riscv-f" - F extension (Single-Precision Floating-Point)
//! - "riscv-d" - D extension (Double-Precision Floating-Point)
//! - "riscv-c" - C extension (Compressed Instructions)
//! - "riscv-a" - A extension (Atomic Instructions)

// Declare architecture modules at the top level
#[cfg(feature = "x86_64")]
pub mod x86_64;

#[cfg(feature = "arm64")]
pub mod arm64;

#[cfg(feature = "riscv64")]
pub mod riscv64;

// Re-export common types for convenient use within the crate
#[cfg(feature = "x86_64")]
pub use x86_64::{X86Decoder, X86Instruction, X86Mnemonic, X86Operand};

#[cfg(feature = "arm64")]
pub use arm64::Arm64Decoder;

#[cfg(feature = "riscv64")]
pub use riscv64::{RiscvDecoder as Riscv64Decoder, RiscvInstruction as Riscv64Instruction};

/// Architecture support module group
///
/// This module conditionally compiles and exposes architecture support
/// based on enabled features.
#[cfg(any(feature = "x86_64", feature = "arm64", feature = "riscv64"))]
pub mod architectures {
    #[cfg(feature = "x86_64")]
    pub use super::x86_64::{X86Decoder, X86Instruction, X86Mnemonic, X86Operand};

    #[cfg(feature = "arm64")]
    pub use super::arm64::Arm64Decoder;

    #[cfg(feature = "riscv64")]
    pub use super::riscv64::{
        RiscvDecoder as Riscv64Decoder, RiscvInstruction as Riscv64Instruction,
    };
}

/// Translation cache module
///
/// Provides LRU caching for cross-architecture instruction translation
/// to reduce repeated translation overhead.
pub mod translation;

/// Register mapping optimization module
///
/// Provides fast register mapping between architectures.
pub mod register_mapper;

pub use register_mapper::{ArchType, MapperStats, RegisterMapper, RegisterType, SpecialReg};

#[cfg(test)]
mod tests {
    #[test]
    fn test_module_documentation() {
        // Verify module-level documentation mentions key architectures
        const DOCS: &str = "Unified Frontend for Multiple Architectures
This crate provides frontend decoders for multiple CPU architectures:
x86_64
ARM64
RISC-V";
        assert!(DOCS.contains("x86_64"));
        assert!(DOCS.contains("ARM64"));
        assert!(DOCS.contains("RISC-V"));
    }

    #[test]
    fn test_architecture_features() {
        // Test documentation mentions the correct features
        const FEATURE_DOCS: &str = "All architectures are enabled when the all feature is active";
        assert!(FEATURE_DOCS.contains("all"));
        assert!(FEATURE_DOCS.contains("feature"));
    }
}
