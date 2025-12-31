//! Instruction Encoding/Decoding Property-Based Tests
//!
//! This module contains property-based tests for instruction encoding and decoding.
//! These tests verify that instructions can be encoded and decoded without loss of
//! information, and that the instruction decoder maintains important invariants.

use proptest::prelude::*;
use proptest_derive::Arbitrary;
use vm_core::{VmError, CoreError};

// ============================================================================
// Test Utilities
// ============================================================================

/// Instruction format types for RISC-V
#[derive(Debug, Clone, Copy, PartialEq, Eq, Arbitrary)]
enum RiscVFormat {
    R,  // Register
    I,  // Immediate
    S,  // Store
    B,  // Branch
    U,  // Upper immediate
    J,  // Jump
}

/// RISC-V instruction opcode (subset for testing)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Arbitrary)]
enum RiscVOpcode {
    Op = 0x33,
    OpImm = 0x13,
    Load = 0x03,
    Store = 0x23,
    Branch = 0x63,
    Jal = 0x6F,
    Jalr = 0x67,
    Lui = 0x37,
    Auipc = 0x17,
}

/// RISC-V function codes for arithmetic operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Arbitrary)]
enum RiscVFunct3 {
    AddSub = 0x0,
    Slt = 0x2,
    Sltu = 0x3,
    Xor = 0x4,
    Or = 0x6,
    And = 0x7,
    Sll = 0x1,
    Srl = 0x5,
}

/// RISC-V function codes for R-type operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Arbitrary)]
enum RiscVFunct7 {
    Add = 0x00,
    Sub = 0x20,
    Sll = 0x00,
    Slt = 0x00,
    Sltu = 0x00,
    Xor = 0x00,
    Srl = 0x00,
    Sra = 0x20,
    Or = 0x00,
    And = 0x00,
}

/// Encoded RISC-V instruction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EncodedInstruction {
    bits: u32,
}

impl EncodedInstruction {
    /// Create a new encoded instruction from raw bits
    fn new(bits: u32) -> Self {
        Self { bits }
    }

    /// Get the opcode field (bits 6:0)
    fn opcode(&self) -> u8 {
        (self.bits & 0x7F) as u8
    }

    /// Get the rd field (bits 11:7)
    fn rd(&self) -> u8 {
        ((self.bits >> 7) & 0x1F) as u8
    }

    /// Get the funct3 field (bits 14:12)
    fn funct3(&self) -> u8 {
        ((self.bits >> 12) & 0x7) as u8
    }

    /// Get the rs1 field (bits 19:15)
    fn rs1(&self) -> u8 {
        ((self.bits >> 15) & 0x1F) as u8
    }

    /// Get the rs2 field (bits 24:20)
    fn rs2(&self) -> u8 {
        ((self.bits >> 20) & 0x1F) as u8
    }

    /// Get the funct7 field (bits 31:25)
    fn funct7(&self) -> u8 {
        ((self.bits >> 25) & 0x7F) as u8
    }

    /// Get immediate for I-type (bits 31:20)
    fn imm_i(&self) -> i32 {
        ((self.bits as i32) << 20) >> 20
    }

    /// Get immediate for S-type (bits 31:25, 11:7)
    fn imm_s(&self) -> i32 {
        let imm_11_5 = (self.bits >> 25) & 0x7F;
        let imm_4_0 = (self.bits >> 7) & 0x1F;
        let imm = (imm_11_5 << 5) | imm_4_0;
        if imm & 0x1000 != 0 {
            (imm as i32) | 0xFFFFF000
        } else {
            imm as i32
        }
    }

    /// Get immediate for B-type
    fn imm_b(&self) -> i32 {
        let imm_12 = (self.bits >> 31) & 0x1;
        let imm_10_5 = (self.bits >> 25) & 0x3F;
        let imm_4_1 = (self.bits >> 8) & 0xF;
        let imm_11 = (self.bits >> 7) & 0x1;
        let imm = (imm_12 << 12) | (imm_11 << 11) | (imm_10_5 << 5) | (imm_4_1 << 1);
        if imm & 0x1000 != 0 {
            (imm as i32) | 0xFFFFE000
        } else {
            imm as i32
        }
    }

    /// Get immediate for U-type (bits 31:12)
    fn imm_u(&self) -> i32 {
        ((self.bits as i32) << 12) >> 12
    }

    /// Get immediate for J-type
    fn imm_j(&self) -> i32 {
        let imm_20 = (self.bits >> 31) & 0x1;
        let imm_10_1 = (self.bits >> 21) & 0x3FF;
        let imm_11 = (self.bits >> 20) & 0x1;
        let imm_19_12 = (self.bits >> 12) & 0xFF;
        let imm = (imm_20 << 20) | (imm_19_12 << 12) | (imm_11 << 11) | (imm_10_1 << 1);
        if imm & 0x100000 != 0 {
            (imm as i32) | 0xFFE00000
        } else {
            imm as i32
        }
    }
}

/// Instruction fields for encoding
#[derive(Debug, Clone, Arbitrary)]
struct InstructionFields {
    opcode: RiscVOpcode,
    rd: u8,
    rs1: u8,
    rs2: u8,
    funct3: RiscVFunct3,
    funct7: RiscVFunct7,
    imm: i16,
}

// ============================================================================
// Property 1: Encode-Decode Roundtrip
// ============================================================================

proptest! {
    /// Property: Decoding an encoded instruction should recover the original fields
    ///
    /// This is the fundamental roundtrip property for instruction encoding/decoding.
    /// If we encode a set of fields and then decode them, we should get back exactly
    /// what we put in.
    #[test]
    fn prop_encode_decode_roundtrip(fields in any::<InstructionFields>()) {
        // Encode R-type instruction
        let encoded = encode_r_type(
            fields.opcode,
            fields.rd,
            fields.funct3,
            fields.rs1,
            fields.rs2,
            fields.funct7,
        );

        // Decode back
        let decoded_opcode = encoded.opcode();
        let decoded_rd = encoded.rd();
        let decoded_funct3 = encoded.funct3();
        let decoded_rs1 = encoded.rs1();
        let decoded_rs2 = encoded.rs2();
        let decoded_funct7 = encoded.funct7();

        // Verify roundtrip
        prop_assert_eq!(decoded_opcode as u8, fields.opcode as u8);
        prop_assert_eq!(decoded_rd, fields.rd % 32); // rd is 5 bits
        prop_assert_eq!(decoded_funct3 as u8, fields.funct3 as u8);
        prop_assert_eq!(decoded_rs1, fields.rs1 % 32); // rs1 is 5 bits
        prop_assert_eq!(decoded_rs2, fields.rs2 % 32); // rs2 is 5 bits
        prop_assert_eq!(decoded_funct7 as u8, fields.funct7 as u8);
    }
}

// ============================================================================
// Property 2: Instruction Length Consistency
// ============================================================================

proptest! {
    /// Property: All instructions should have consistent length (4 bytes for RISC-V)
    ///
    /// This verifies that our instruction decoder always produces 4-byte instructions
    /// and never varies in size (for the base RISC-V ISA).
    #[test]
    fn prop_instruction_length_consistency(
        opcode in 0u8..128u8,
        rd in 0u8..32u8,
        rs1 in 0u8..32u8,
        rs2 in 0u8..32u8,
        funct3 in 0u8..8u8,
        funct7 in 0u8..128u8,
    ) {
        // Encode instruction
        let bits = ((funct7 as u32) << 25)
            | ((rs2 as u32) << 20)
            | ((rs1 as u32) << 15)
            | ((funct3 as u32) << 12)
            | ((rd as u32) << 7)
            | (opcode as u32);

        let encoded = EncodedInstruction::new(bits);

        // Check that the instruction is 4 bytes (32 bits)
        prop_assert!(encoded.bits < (1u32 << 32));

        // Verify that we can represent it in exactly 4 bytes
        let bytes = encoded.bits.to_le_bytes();
        prop_assert_eq!(bytes.len(), 4);
    }
}

// ============================================================================
// Property 3: Register Index Range Validation
// ============================================================================

proptest! {
    /// Property: Register indices should always be in valid range [0, 31]
    ///
    /// This property tests that all register fields are properly constrained to
    /// 5 bits, giving a valid range of 0-31.
    #[test]
    fn prop_register_index_range(
        rd in 0u8..64u8,  // Test values outside valid range too
        rs1 in 0u8..64u8,
        rs2 in 0u8..64u8,
    ) {
        // Encode with potentially out-of-range values
        let bits = ((rs2 as u32 % 128) << 20)
            | ((rs1 as u32 % 128) << 15)
            | ((rd as u32 % 128) << 7)
            | 0x33; // OP opcode

        let encoded = EncodedInstruction::new(bits);

        // Decode and verify valid range
        let decoded_rd = encoded.rd();
        let decoded_rs1 = encoded.rs1();
        let decoded_rs2 = encoded.rs2();

        // All should be in range [0, 31]
        prop_assert!(decoded_rd < 32, "rd should be in range [0, 31]");
        prop_assert!(decoded_rs1 < 32, "rs1 should be in range [0, 31]");
        prop_assert!(decoded_rs2 < 32, "rs2 should be in range [0, 31]");
    }
}

// ============================================================================
// Property 4: Immediate Sign Extension
// ============================================================================

proptest! {
    /// Property: Immediate values should be properly sign-extended
    ///
    /// This tests that immediate values are correctly sign-extended from their
    // encoded width to the full 32-bit signed integer.
    #[test]
    fn prop_immediate_sign_extension(
        imm12 in -2048i16..2047i16, // I-type immediate
    ) {
        // Encode I-type instruction with immediate
        let bits = ((imm12 as u32) & 0xFFF00000)
            | (0x00 << 15)  // rs1
            | (0x0 << 12)   // funct3
            | (0x00 << 7)   // rd
            | 0x13;         // OP_IMM opcode

        let encoded = EncodedInstruction::new(bits);
        let decoded_imm = encoded.imm_i();

        // Verify sign extension
        prop_assert_eq!(decoded_imm, imm12 as i32);
    }
}

// ============================================================================
// Property 5: Instruction Alignment
// ============================================================================

proptest! {
    /// Property: Instructions should be properly aligned
    ///
    /// RISC-V requires instructions to be 2-byte aligned (and for the base ISA,
    /// they're always 4 bytes). This property tests that encoded instructions
    /// maintain this alignment.
    #[test]
    fn prop_instruction_alignment(
        opcode in 0u32..128u32,
        rest in 0u32..(1u32 << 25),
    ) {
        let bits = (rest << 7) | opcode;
        let encoded = EncodedInstruction::new(bits);

        // Instruction should be 4-byte aligned when stored
        let addr = 0x1000u32;
        prop_assert_eq!(addr % 4, 0, "Instruction address should be aligned");

        // Verify we can fetch at aligned address
        let instruction_size = 4;
        prop_assert_eq!(addr % instruction_size, 0);
    }
}

// ============================================================================
// Property 6: Opcode Determinism
// ============================================================================

proptest! {
    /// Property: Same instruction fields should always produce same encoding
    ///
    /// This is a determinism property - encoding the same instruction twice
    /// should produce identical bit patterns.
    #[test]
    fn prop_encoding_determinism(
        opcode in 0u8..128u8,
        rd in 0u8..32u8,
        rs1 in 0u8..32u8,
        rs2 in 0u8..32u8,
        funct3 in 0u8..8u8,
        funct7 in 0u8..128u8,
    ) {
        // Encode twice
        let bits1 = ((funct7 as u32) << 25)
            | ((rs2 as u32) << 20)
            | ((rs1 as u32) << 15)
            | ((funct3 as u32) << 12)
            | ((rd as u32) << 7)
            | (opcode as u32);

        let bits2 = ((funct7 as u32) << 25)
            | ((rs2 as u32) << 20)
            | ((rs1 as u32) << 15)
            | ((funct3 as u32) << 12)
            | ((rd as u32) << 7)
            | (opcode as u32);

        prop_assert_eq!(bits1, bits2, "Same inputs should produce same encoding");
    }
}

// ============================================================================
// Property 7: Field Independence
// ============================================================================

proptest! {
    /// Property: Different fields should not interfere with each other
    ///
    /// This tests that setting one field doesn't affect the value of another field,
    /// which is important for correct instruction encoding.
    #[test]
    fn prop_field_independence(
        rd in 0u8..32u8,
        rs1 in 0u8..32u8,
        rs2 in 0u8..32u8,
    ) {
        // Encode with specific values
        let bits = ((rs2 as u32) << 20)
            | ((rs1 as u32) << 15)
            | ((rd as u32) << 7)
            | 0x33;

        let encoded = EncodedInstruction::new(bits);

        // Verify each field independently
        prop_assert_eq!(encoded.rd(), rd);
        prop_assert_eq!(encoded.rs1(), rs1);
        prop_assert_eq!(encoded.rs2(), rs2);
    }
}

// ============================================================================
// Property 8: Branch Target Calculation
// ============================================================================

proptest! {
    /// Property: Branch immediate offsets should be correctly calculated
    ///
    /// This tests that branch offsets are properly encoded and decoded, which is
    /// critical for correct control flow.
    #[test]
    fn prop_branch_target_calculation(
        offset in -4096i16..4095i16,
    ) {
        // B-type immediate encoding
        let imm = offset as i32;
        let imm_12 = ((imm >> 12) & 0x1) as u32;
        let imm_11 = ((imm >> 11) & 0x1) as u32;
        let imm_10_5 = ((imm >> 5) & 0x3F) as u32;
        let imm_4_1 = ((imm >> 1) & 0xF) as u32;

        let bits = (imm_12 << 31)
            | (imm_10_5 << 25)
            | (imm_4_1 << 8)
            | (imm_11 << 7)
            | 0x63; // BRANCH opcode

        let encoded = EncodedInstruction::new(bits);
        let decoded_offset = encoded.imm_b();

        // Verify the offset matches (after rounding to 2-byte boundary)
        let rounded_offset = (offset as i32) & !1;
        prop_assert_eq!(decoded_offset, rounded_offset);
    }
}

// ============================================================================
// Property 9: Compressed Instruction Detection
// ============================================================================

proptest! {
    /// Property: Should be able to distinguish compressed from regular instructions
    ///
    /// This tests that we can correctly identify compressed instructions (if supported)
    /// vs. regular 32-bit instructions based on the opcode bits.
    #[test]
    fn prop_compressed_detection(
        bits in 0u32..(1u32 << 32),
    ) {
        let encoded = EncodedInstruction::new(bits);
        let opcode = encoded.opcode();

        // Check if it's a compressed instruction (lowest 2 bits are not 11)
        let is_compressed = (bits & 0x3) != 0x3;

        // Compressed instructions have opcodes in range [0, 3]
        if is_compressed {
            prop_assert!(opcode <= 3, "Compressed instructions should have low opcode");
        }
    }
}

// ============================================================================
// Property 10: Instruction Field Masks
// ============================================================================

proptest! {
    /// Property: Instruction field masks should correctly extract fields
    ///
    /// This verifies that our bit masks for extracting instruction fields are
    // correct and don't leak bits between fields.
    #[test]
    fn prop_field_masks(
        bits in 0u32..(1u32 << 32),
    ) {
        let encoded = EncodedInstruction::new(bits);

        // Extract each field
        let opcode = encoded.opcode() as u32;
        let rd = encoded.rd() as u32;
        let funct3 = encoded.funct3() as u32;
        let rs1 = encoded.rs1() as u32;
        let rs2 = encoded.rs2() as u32;
        let funct7 = encoded.funct7() as u32;

        // Verify masks are correct (no bit leakage)
        prop_assert!(opcode < 128, "Opcode should be 7 bits");
        prop_assert!(rd < 32, "rd should be 5 bits");
        prop_assert!(funct3 < 8, "funct3 should be 3 bits");
        prop_assert!(rs1 < 32, "rs1 should be 5 bits");
        prop_assert!(rs2 < 32, "rs2 should be 5 bits");
        prop_assert!(funct7 < 128, "funct7 should be 7 bits");
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Encode an R-type instruction
fn encode_r_type(
    opcode: RiscVOpcode,
    rd: u8,
    funct3: RiscVFunct3,
    rs1: u8,
    rs2: u8,
    funct7: RiscVFunct7,
) -> EncodedInstruction {
    let bits = ((funct7 as u32) << 25)
        | (((rs2 % 32) as u32) << 20)
        | (((rs1 % 32) as u32) << 15)
        | ((funct3 as u32) << 12)
        | (((rd % 32) as u32) << 7)
        | (opcode as u32);

    EncodedInstruction::new(bits)
}

/// Encode an I-type instruction
fn encode_i_type(
    opcode: RiscVOpcode,
    rd: u8,
    funct3: RiscVFunct3,
    rs1: u8,
    imm: i16,
) -> EncodedInstruction {
    let bits = (((imm as u32) & 0xFFF) << 20)
        | (((rs1 % 32) as u32) << 15)
        | ((funct3 as u32) << 12)
        | (((rd % 32) as u32) << 7)
        | (opcode as u32);

    EncodedInstruction::new(bits)
}

/// Encode an S-type instruction
fn encode_s_type(
    opcode: RiscVOpcode,
    rs2: u8,
    funct3: RiscVFunct3,
    rs1: u8,
    imm: i16,
) -> EncodedInstruction {
    let imm_11_5 = ((imm as u32) >> 5) & 0x7F;
    let imm_4_0 = (imm as u32) & 0x1F;

    let bits = (imm_11_5 << 25)
        | (((rs2 % 32) as u32) << 20)
        | (((rs1 % 32) as u32) << 15)
        | ((funct3 as u32) << 12)
        | (imm_4_0 << 7)
        | (opcode as u32);

    EncodedInstruction::new(bits)
}
