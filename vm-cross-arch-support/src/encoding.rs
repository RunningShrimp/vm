//! Common encoding framework for VM cross-architecture translation
//!
//! This module provides a unified interface for encoding instructions across different
//! architectures, reducing code duplication and improving maintainability.

use std::collections::HashMap;
use std::fmt;
use thiserror::Error;

/// Supported CPU architectures
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Architecture {
    X86_64,
    ARM64,
    RISCV64,
}

impl Architecture {
    /// Get the number of general-purpose registers for this architecture
    pub fn register_count(&self) -> usize {
        match self {
            Architecture::X86_64 => 16, // RAX, RCX, RDX, RBX, RSP, RBP, RSI, RDI, R8-R15
            Architecture::ARM64 => 31,  // X0-X30 (X31 is SP/ZR)
            Architecture::RISCV64 => 32, // x0-x31
        }
    }
}

impl fmt::Display for Architecture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Architecture::X86_64 => write!(f, "x86_64"),
            Architecture::ARM64 => write!(f, "aarch64"),
            Architecture::RISCV64 => write!(f, "riscv64"),
        }
    }
}

/// Register ID wrapper
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RegId(pub u16);

impl fmt::Display for RegId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Errors that can occur during instruction encoding
#[derive(Error, Debug, Clone, PartialEq)]
pub enum EncodingError {
    #[error("Invalid register ID: {0}")]
    InvalidRegister(RegId),
    #[error("Immediate value {0} out of range for format {1:?}")]
    ImmediateOutOfRange(i64, ImmediateFormat),
    #[error("Memory operand invalid: {0}")]
    InvalidMemoryOperand(String),
    #[error("Unsupported operation for architecture {0:?}")]
    UnsupportedOperation(Architecture),
    #[error("Encoding buffer overflow")]
    BufferOverflow,
    #[error("Invalid instruction format")]
    InvalidFormat,
}

/// Endianness for different architectures
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endianness {
    Little,
    Big,
}

/// Immediate encoding formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImmediateFormat {
    Unsigned5,  // 5-bit unsigned
    Unsigned12, // 12-bit unsigned
    Unsigned16, // 16-bit unsigned
    Unsigned20, // 20-bit unsigned
    Unsigned32, // 32-bit unsigned
    Signed5,    // 5-bit signed
    Signed12,   // 12-bit signed
    Signed16,   // 16-bit signed
    Signed20,   // 20-bit signed
    Signed32,   // 32-bit signed
    Shifted5,   // 5-bit shifted (imm * 4)
    Shifted12,  // 12-bit shifted (imm * 4)
    Shifted16,  // 16-bit shifted (imm * 4)
    Variable,   // Variable length encoding
}

/// Register field positions in instruction words
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterField {
    Rd,         // Destination register
    Rs1,        // Source register 1
    Rs2,        // Source register 2
    Rs3,        // Source register 3 (for some architectures)
    Fd,         // Floating-point destination
    Fs1,        // Floating-point source 1
    Fs2,        // Floating-point source 2
    Fs3,        // Floating-point source 3
    Vd,         // Vector destination
    Vs1,        // Vector source 1
    Vs2,        // Vector source 2
    Vs3,        // Vector source 3
    Custom(u8), // Custom field position
}

/// Memory operand representation
#[derive(Debug, Clone)]
pub struct MemoryOperand {
    pub base_reg: RegId,
    pub offset: i64,
    pub size: u8,
    pub alignment: u8,
    pub flags: MemoryFlags,
}

/// Memory access flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MemoryFlags {
    pub is_volatile: bool,
    pub is_atomic: bool,
    pub is_acquire: bool,
    pub is_release: bool,
    pub is_aligned: bool,
}

/// Encoding context for instruction generation
#[derive(Debug, Clone)]
pub struct EncodingContext {
    pub architecture: Architecture,
    pub endianness: Endianness,
    pub address_size: u8,
    pub features: HashMap<String, bool>,
}

impl EncodingContext {
    pub fn new(architecture: Architecture) -> Self {
        let endianness = match architecture {
            Architecture::X86_64 => Endianness::Little,
            Architecture::ARM64 => Endianness::Little,
            Architecture::RISCV64 => Endianness::Little,
        };

        Self {
            architecture,
            endianness,
            address_size: 64, // Default to 64-bit
            features: HashMap::new(),
        }
    }

    pub fn with_feature(mut self, feature: impl Into<String>, enabled: bool) -> Self {
        self.features.insert(feature.into(), enabled);
        self
    }

    pub fn has_feature(&self, feature: &str) -> bool {
        self.features.get(feature).copied().unwrap_or(false)
    }
}

/// Trait for architecture-specific instruction encoding
pub trait InstructionEncoding {
    /// The word type for this architecture (u32 for RISC-V/ARM64, u8 array for x86)
    type Word;
    /// Register specification for this architecture
    type RegisterSpec;

    /// Encode a register ID to architecture-specific format
    fn encode_register(&self, reg: RegId) -> Result<Self::RegisterSpec, EncodingError>;

    /// Encode an immediate value with the specified format
    fn encode_immediate(
        &self,
        imm: i64,
        format: ImmediateFormat,
    ) -> Result<Self::Word, EncodingError>;

    /// Encode a memory operand
    fn encode_memory_operand(
        &self,
        base: RegId,
        offset: i64,
        size: u8,
    ) -> Result<MemoryOperand, EncodingError>;

    /// Get the encoding context
    fn context(&self) -> &EncodingContext;

    /// Check if a register ID is valid for this architecture
    fn is_valid_register(&self, reg: RegId) -> bool;

    /// Get the maximum immediate value for a given format
    fn max_immediate(&self, format: ImmediateFormat) -> i64;

    /// Get the minimum immediate value for a given format
    fn min_immediate(&self, format: ImmediateFormat) -> i64;
}

/// Trait for building instructions incrementally
pub trait InstructionBuilder {
    /// Add an opcode byte/word to the instruction
    fn add_opcode(&mut self, opcode: u8) -> &mut Self;

    /// Add a register to the instruction in the specified field
    fn add_register(&mut self, reg: RegId, field: RegisterField) -> &mut Self;

    /// Add an immediate value to the instruction
    fn add_immediate(&mut self, imm: i64, format: ImmediateFormat) -> &mut Self;

    /// Add a memory operand to the instruction
    fn add_memory_operand(&mut self, base: RegId, offset: i64, size: u8) -> &mut Self;

    /// Set a flag in the instruction
    fn set_flag(&mut self, flag: InstructionFlag) -> &mut Self;

    /// Build the final instruction
    fn build(self) -> Result<Vec<u8>, EncodingError>;

    /// Get the current size of the instruction being built
    fn size(&self) -> usize;
}

/// Instruction flags that can be set during encoding
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstructionFlag {
    /// Set condition codes
    SetConditionCodes,
    /// Don't set condition codes
    NoSetConditionCodes,
    /// Update flags register
    UpdateFlags,
    /// Don't update flags register
    NoUpdateFlags,
    /// 64-bit operation
    Wide,
    /// 32-bit operation
    Narrow,
    /// Signed operation
    Signed,
    /// Unsigned operation
    Unsigned,
    /// Atomic operation
    Atomic,
    /// Volatile operation
    Volatile,
    /// Acquire semantics
    Acquire,
    /// Release semantics
    Release,
}

/// Utility functions for common encoding operations
pub mod utils {
    use super::*;

    /// Check if an immediate value fits in the specified format
    pub fn immediate_fits(imm: i64, format: ImmediateFormat) -> bool {
        let (min, max) = match format {
            ImmediateFormat::Unsigned5 => (0, 0x1F),
            ImmediateFormat::Unsigned12 => (0, 0xFFF),
            ImmediateFormat::Unsigned16 => (0, 0xFFFF),
            ImmediateFormat::Unsigned20 => (0, 0xFFFFF),
            ImmediateFormat::Unsigned32 => (0, 0xFFFFFFFF),
            ImmediateFormat::Signed5 => (-16, 15),
            ImmediateFormat::Signed12 => (-2048, 2047),
            ImmediateFormat::Signed16 => (-32768, 32767),
            ImmediateFormat::Signed20 => (-524288, 524287),
            ImmediateFormat::Signed32 => (-2147483648, 2147483647),
            ImmediateFormat::Shifted5 => (0, 0x7C), // 5-bit shifted by 2
            ImmediateFormat::Shifted12 => (0, 0x3FFC), // 12-bit shifted by 2
            ImmediateFormat::Shifted16 => (0, 0xFFFC), // 16-bit shifted by 2
            ImmediateFormat::Variable => return true, // Variable length can fit any
        };
        imm >= min && imm <= max
    }

    /// Extract bits from a value
    pub fn extract_bits(value: u64, start: u8, count: u8) -> u64 {
        let mask = (1u64 << count) - 1;
        (value >> start) & mask
    }

    /// Insert bits into a value
    pub fn insert_bits(value: u64, bits: u64, start: u8, count: u8) -> u64 {
        let mask = ((1u64 << count) - 1) << start;
        (value & !mask) | ((bits << start) & mask)
    }

    /// Sign-extend a value
    pub fn sign_extend(value: u64, bits: u8) -> u64 {
        if bits == 0 || bits >= 64 {
            return value;
        }
        let sign_bit = 1u64 << (bits - 1);
        if value & sign_bit != 0 {
            value | (!0u64 << bits)
        } else {
            value
        }
    }

    /// Convert endianness of a byte slice
    pub fn convert_endianness(bytes: &mut [u8], from: Endianness, to: Endianness) {
        if from == to {
            return;
        }
        bytes.reverse();
    }

    /// Align a value to the specified alignment
    pub fn align_up(value: u64, alignment: u64) -> u64 {
        (value + alignment - 1) & !(alignment - 1)
    }

    /// Check if a value is aligned to the specified alignment
    pub fn is_aligned(value: u64, alignment: u64) -> bool {
        value.is_multiple_of(alignment)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_immediate_fits() {
        assert!(utils::immediate_fits(10, ImmediateFormat::Unsigned5));
        assert!(!utils::immediate_fits(32, ImmediateFormat::Unsigned5));
        assert!(utils::immediate_fits(-10, ImmediateFormat::Signed5));
        assert!(!utils::immediate_fits(-17, ImmediateFormat::Signed5));
    }

    #[test]
    fn test_extract_bits() {
        assert_eq!(utils::extract_bits(0b11010100, 2, 3), 0b101);
        assert_eq!(utils::extract_bits(0xFF00, 8, 8), 0xFF);
    }

    #[test]
    fn test_insert_bits() {
        assert_eq!(utils::insert_bits(0b11000000, 0b101, 2, 3), 0b11010100);
        assert_eq!(utils::insert_bits(0x00, 0xFF, 8, 8), 0xFF00);
    }

    #[test]
    fn test_sign_extend() {
        // 0b101 with 3 bits should be sign-extended to all 1s in the upper bits
        assert_eq!(utils::sign_extend(0b101, 3), 0xFFFFFFFFFFFFFFFD_u64);
        assert_eq!(utils::sign_extend(0b010, 3), 0b010);
    }

    #[test]
    fn test_align_up() {
        assert_eq!(utils::align_up(10, 8), 16);
        assert_eq!(utils::align_up(16, 8), 16);
        assert_eq!(utils::align_up(0, 8), 0);
    }

    #[test]
    fn test_is_aligned() {
        assert!(utils::is_aligned(16, 8));
        assert!(!utils::is_aligned(10, 8));
        assert!(utils::is_aligned(0, 8));
    }
}
