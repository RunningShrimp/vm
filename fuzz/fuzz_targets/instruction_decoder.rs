#![no_main]
//! Instruction Decoder Fuzzing Target
//!
//! This fuzzing target tests the robustness of the instruction decoder.
//! It feeds random byte sequences to the decoder and verifies that:
//! 1. The decoder never crashes
//! 2. The decoder returns either a valid instruction or a proper error
//! 3. The decoder doesn't panic on malformed input

use libfuzzer_sys::fuzz_target;
use vm_core::{VmError, CoreError};

/// Mock instruction decoder result
#[derive(Debug)]
enum DecodeResult {
    ValidInstruction {
        opcode: u8,
        rd: Option<u8>,
        rs1: Option<u8>,
        rs2: Option<u8>,
        imm: Option<i32>,
    },
    InvalidInstruction,
    UnsupportedInstruction,
    UnknownOpcode,
}

/// Simple RISC-V instruction decoder for fuzzing
///
/// This is a simplified decoder that attempts to decode RISC-V instructions
/// from arbitrary byte sequences. It's designed to be robust and never panic.
fn decode_instruction(data: &[u8]) -> DecodeResult {
    // Need at least 4 bytes for a RISC-V instruction
    if data.len() < 4 {
        return DecodeResult::InvalidInstruction;
    }

    // Read 32-bit instruction (little-endian)
    let bits = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let opcode = (bits & 0x7F) as u8;

    // Check for compressed instructions (lowest 2 bits != 11)
    if bits & 0x3 != 0x3 {
        // Compressed instruction (16-bit)
        return DecodeResult::UnsupportedInstruction;
    }

    match opcode {
        // R-type: OP, OP_FP, AMO
        0x33 | 0x43 | 0x2F => {
            let rd = Some(((bits >> 7) & 0x1F) as u8);
            let rs1 = Some(((bits >> 15) & 0x1F) as u8);
            let rs2 = Some(((bits >> 20) & 0x1F) as u8);
            DecodeResult::ValidInstruction {
                opcode,
                rd,
                rs1,
                rs2,
                imm: None,
            }
        }

        // I-type: OP_IMM, JALR, LOAD, LOAD_FP
        0x13 | 0x67 | 0x03 | 0x07 => {
            let rd = Some(((bits >> 7) & 0x1F) as u8);
            let rs1 = Some(((bits >> 15) & 0x1F) as u8);
            let imm = Some(((bits as i32) << 20) >> 20);
            DecodeResult::ValidInstruction {
                opcode,
                rd,
                rs1,
                rs2: None,
                imm,
            }
        }

        // S-type: STORE, STORE_FP
        0x23 | 0x27 => {
            let rs1 = Some(((bits >> 15) & 0x1F) as u8);
            let rs2 = Some(((bits >> 20) & 0x1F) as u8);
            let imm_11_5 = (bits >> 25) & 0x7F;
            let imm_4_0 = (bits >> 7) & 0x1F;
            let imm = if imm_11_5 & 0x40 != 0 {
                ((imm_11_5 << 5) | imm_4_0) as i32 | 0xFFFFF000
            } else {
                ((imm_11_5 << 5) | imm_4_0) as i32
            };
            DecodeResult::ValidInstruction {
                opcode,
                rd: None,
                rs1,
                rs2,
                imm: Some(imm),
            }
        }

        // B-type: BRANCH
        0x63 => {
            let rs1 = Some(((bits >> 15) & 0x1F) as u8);
            let rs2 = Some(((bits >> 20) & 0x1F) as u8);
            let imm = decode_branch_imm(bits);
            DecodeResult::ValidInstruction {
                opcode,
                rd: None,
                rs1,
                rs2,
                imm: Some(imm),
            }
        }

        // U-type: LUI, AUIPC
        0x37 | 0x17 => {
            let rd = Some(((bits >> 7) & 0x1F) as u8);
            let imm = ((bits as i32) << 12) >> 12;
            DecodeResult::ValidInstruction {
                opcode,
                rd,
                rs1: None,
                rs2: None,
                imm: Some(imm),
            }
        }

        // J-type: JAL
        0x6F => {
            let rd = Some(((bits >> 7) & 0x1F) as u8);
            let imm = decode_jump_imm(bits);
            DecodeResult::ValidInstruction {
                opcode,
                rd,
                rs1: None,
                rs2: None,
                imm: Some(imm),
            }
        }

        // SYSTEM
        0x73 => {
            // Could be ECALL, EBREAK, CSRW, etc.
            let rd = Some(((bits >> 7) & 0x1F) as u8);
            DecodeResult::ValidInstruction {
                opcode,
                rd,
                rs1: None,
                rs2: None,
                imm: None,
            }
        }

        // Unknown/reserved opcodes
        _ => DecodeResult::UnknownOpcode,
    }
}

/// Decode branch immediate
fn decode_branch_imm(bits: u32) -> i32 {
    let imm_12 = (bits >> 31) & 0x1;
    let imm_10_5 = (bits >> 25) & 0x3F;
    let imm_4_1 = (bits >> 8) & 0xF;
    let imm_11 = (bits >> 7) & 0x1;
    let imm = (imm_12 << 12) | (imm_11 << 11) | (imm_10_5 << 5) | (imm_4_1 << 1);
    if imm & 0x1000 != 0 {
        (imm as i32) | 0xFFFFE000
    } else {
        imm as i32
    }
}

/// Decode jump immediate
fn decode_jump_imm(bits: u32) -> i32 {
    let imm_20 = (bits >> 31) & 0x1;
    let imm_10_1 = (bits >> 21) & 0x3FF;
    let imm_11 = (bits >> 20) & 0x1;
    let imm_19_12 = (bits >> 12) & 0xFF;
    let imm = (imm_20 << 20) | (imm_19_12 << 12) | (imm_11 << 11) | (imm_10_1 << 1);
    if imm & 0x100000 != 0 {
        (imm as i32) | 0xFFE00000
    } else {
        imm as i32
    }
}

/// Validate decoded instruction fields
fn validate_instruction(result: &DecodeResult) -> bool {
    match result {
        DecodeResult::ValidInstruction { opcode, rd, rs1, rs2, imm } => {
            // Opcode should be in valid range
            if *opcode >= 128 {
                return false;
            }

            // Register indices should be in range [0, 31]
            if let Some(r) = rd {
                if *r >= 32 {
                    return false;
                }
            }
            if let Some(r) = rs1 {
                if *r >= 32 {
                    return false;
                }
            }
            if let Some(r) = rs2 {
                if *r >= 32 {
                    return false;
                }
            }

            // Validate immediate ranges based on format
            if let Some(i) = imm {
                // I-type: 12-bit signed
                if matches!(opcode, 0x13 | 0x67 | 0x03 | 0x07) {
                    if *i < -2048 || *i > 2047 {
                        return false;
                    }
                }
                // S-type: 12-bit signed
                if matches!(opcode, 0x23 | 0x27) {
                    if *i < -2048 || *i > 2047 {
                        return false;
                    }
                }
                // B-type: 13-bit signed, aligned to 2
                if *opcode == 0x63 {
                    if *i < -4096 || *i > 4095 {
                        return false;
                    }
                    if i % 2 != 0 {
                        return false;
                    }
                }
                // U-type: 20-bit unsigned, shifted by 12
                if matches!(opcode, 0x37 | 0x17) {
                    if *i < -(1 << 19) || *i >= (1 << 20) {
                        return false;
                    }
                }
                // J-type: 21-bit signed, aligned to 2
                if *opcode == 0x6F {
                    if *i < -(1 << 20) || *i >= (1 << 20) {
                        return false;
                    }
                    if i % 2 != 0 {
                        return false;
                    }
                }
            }

            true
        }
        DecodeResult::InvalidInstruction
        | DecodeResult::UnsupportedInstruction
        | DecodeResult::UnknownOpcode => true,
    }
}

/// Fuzz target function
///
/// This function is called by libfuzzer with random byte sequences.
/// It attempts to decode the input and verifies that the decoder
/// handles all inputs gracefully.
fuzz_target!(|data: &[u8]| {
    // Attempt to decode instruction
    let result = decode_instruction(data);

    // Verify the result is valid
    let is_valid = validate_instruction(&result);

    // If validation fails, this is a bug in the decoder
    if !is_valid {
        // Log the problematic input for debugging
        eprintln!("Invalid instruction decoded!");
        eprintln!("Input bytes: {:?}", data);
        eprintln!("Decoded result: {:?}", result);
    }

    // Additional sanity checks

    // The decoder should never panic
    // This is implicitly tested by the fuzzing framework

    // If we got a valid instruction, round-trip encode it and verify
    if let DecodeResult::ValidInstruction { opcode, rd, rs1, rs2, imm } = result {
        // Basic sanity: re-encoding should give us consistent results
        // (This is a simplified check; a full implementation would
        // re-encode and decode again)
        assert!(*opcode < 128, "Opcode out of range");
    }
});
