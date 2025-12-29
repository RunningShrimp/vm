//! Unified Frontend for Multiple Architectures
//!
//! This crate provides frontend decoders for multiple CPU architectures:
//! - x86_64 (Intel/AMD 64-bit)
//! - ARM64 (AArch64)
//! - RISC-V 64-bit
//!
//! All architectures are enabled when the "all" feature is active (recommended).
//! Individual architecture features (x86_64, arm64, riscv64) are deprecated but
//! still available for backward compatibility - they all enable "all".

/// Architecture support module group
///
/// This module conditionally compiles and exposes all architecture support
/// when the "all" feature is enabled.
#[cfg(feature = "all")]
pub mod architectures {
    /// x86_64 (Intel/AMD 64-bit) architecture support
    pub mod x86_64;

    /// ARM64 (AArch64) architecture support
    pub mod arm64;

    /// RISC-V 64-bit architecture support
    pub mod riscv64;

    // Re-export common types from all architectures
    pub use x86_64::{X86Decoder, X86Instruction, X86Mnemonic, X86Operand};
    pub use arm64::Arm64Decoder;
    pub use riscv64::{RiscvDecoder as Riscv64Decoder, RiscvInstruction as Riscv64Instruction};
}

/// Re-export all architecture support when "all" feature is enabled
#[cfg(feature = "all")]
pub use architectures::*;

/// Direct module access for backward compatibility
#[cfg(feature = "all")]
pub use architectures::x86_64;

#[cfg(feature = "all")]
pub use architectures::arm64;

#[cfg(feature = "all")]
pub use architectures::riscv64;
