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

// Declare architecture modules at the top level
#[cfg(feature = "all")]
pub mod x86_64;

#[cfg(feature = "all")]
pub mod arm64;

#[cfg(feature = "all")]
pub mod riscv64;

/// Architecture support module group
///
/// This module conditionally compiles and exposes all architecture support
/// when the "all" feature is enabled.
#[cfg(feature = "all")]
pub mod architectures {
    // Re-export the top-level modules
    pub use crate::arm64;
    pub use crate::riscv64;
    pub use crate::x86_64;

    // Re-export common types from all architectures
    pub use arm64::Arm64Decoder;
    pub use riscv64::{RiscvDecoder as Riscv64Decoder, RiscvInstruction as Riscv64Instruction};
    pub use x86_64::{X86Decoder, X86Instruction, X86Mnemonic, X86Operand};
}

/// Re-export all architecture support when "all" feature is enabled
#[cfg(feature = "all")]
pub use architectures::*;
