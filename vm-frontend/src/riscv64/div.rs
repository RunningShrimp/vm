//! RISC-V M Extension (Integer Division) Support
//!
//! Implements the RISC-V M extension for integer division and remainder operations.
//! The M extension adds 32-bit and 64-bit divide, divide unsigned, remainder, and remainder unsigned operations.
//!
//! Reference: RISC-V Volume I: User-Level ISA, Version 2.1
//! Section 7.1: Multiplication and Division Operations

use vm_core::{GuestAddr, MMU, VmError};
use vm_ir::{IRBlock, IRBuilder, IROp, RegisterFile, Terminator};

/// M extension division instruction types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DivInstruction {
    // 64-bit division operations (funct7 = 0x01, opcode = 0x33)
    Div,  // Divide (rd <- rs1 / rs2)
    Divu, // Divide unsigned (rd <- rs1 / rs2)
    Rem,  // Remainder (rd <- rs1 % rs2)
    Remu, // Remainder unsigned (rd <- rs1 % rs2)

    // 32-bit division operations for RV64 (funct7 = 0x01, opcode = 0x3B)
    // These operate on lower 32 bits and sign-extend the result
    Divw,  // Divide word (rd <- sext(rs1[31:0] / rs2[31:0]))
    Divuw, // Divide word unsigned (rd <- sext(rs1[31:0] / rs2[31:0]))
    Remw,  // Remainder word (rd <- sext(rs1[31:0] % rs2[31:0]))
    Remuw, // Remainder word unsigned (rd <- sext(rs1[31:0] % rs2[31:0]))
}

/// M extension division decoder
pub struct DivDecoder;

impl DivDecoder {
    /// Decode M extension division instruction
    ///
    /// M extension instructions use:
    /// - opcode: 0x33 for RV64I (64-bit operations)
    /// - opcode: 0x3B for RV64I word operations (32-bit operations on 64-bit registers)
    /// - funct3: varies by operation
    /// - funct7: 0x01 for division operations (same as multiply)
    pub fn decode(insn: u32) -> Option<DivInstruction> {
        let opcode = insn & 0x7f;
        let funct3 = (insn >> 12) & 0x7;
        let funct7 = (insn >> 25) & 0x7f;

        // Division operations have funct7 = 0x01 (same as multiply)
        if funct7 != 0x01 {
            return None;
        }

        match opcode {
            // RV64I standard instructions (64-bit operations)
            0x33 => {
                match funct3 {
                    0x4 => Some(DivInstruction::Div),  // DIV
                    0x5 => Some(DivInstruction::Divu), // DIVU
                    0x6 => Some(DivInstruction::Rem),  // REM
                    0x7 => Some(DivInstruction::Remu), // REMU
                    _ => None,
                }
            }
            // RV64I word instructions (32-bit operations, sign-extended to 64-bit)
            0x3B => {
                match funct3 {
                    0x4 => Some(DivInstruction::Divw),  // DIVW
                    0x5 => Some(DivInstruction::Divuw), // DIVUW
                    0x6 => Some(DivInstruction::Remw),  // REMW
                    0x7 => Some(DivInstruction::Remuw), // REMUW
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Convert M extension division instruction to IR
    pub fn to_ir(
        insn: u32,
        reg_file: &mut RegisterFile,
        mut builder: IRBuilder,
        _mmu: &dyn MMU,
        pc: GuestAddr,
    ) -> Result<IRBlock, VmError> {
        let rd = (insn >> 7) & 0x1f;
        let rs1 = (insn >> 15) & 0x1f;
        let rs2 = (insn >> 20) & 0x1f;

        match Self::decode(insn) {
            Some(DivInstruction::Div) => {
                // DIV: rd <- rs1 / rs2 (signed 64-bit division)
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = reg_file.read(rs2 as usize);
                builder.push(IROp::Div {
                    dst,
                    src1,
                    src2,
                    signed: true,
                });
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build())
            }

            Some(DivInstruction::Divu) => {
                // DIVU: rd <- rs1 / rs2 (unsigned 64-bit division)
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = reg_file.read(rs2 as usize);
                builder.push(IROp::Div {
                    dst,
                    src1,
                    src2,
                    signed: false,
                });
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build())
            }

            Some(DivInstruction::Rem) => {
                // REM: rd <- rs1 % rs2 (signed 64-bit remainder)
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = reg_file.read(rs2 as usize);
                builder.push(IROp::Rem {
                    dst,
                    src1,
                    src2,
                    signed: true,
                });
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build())
            }

            Some(DivInstruction::Remu) => {
                // REMU: rd <- rs1 % rs2 (unsigned 64-bit remainder)
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = reg_file.read(rs2 as usize);
                builder.push(IROp::Rem {
                    dst,
                    src1,
                    src2,
                    signed: false,
                });
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build())
            }

            Some(DivInstruction::Divw) => {
                // DIVW: rd <- sext(rs1[31:0] / rs2[31:0])
                // Operates on lower 32 bits, sign-extends result to 64 bits
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = reg_file.read(rs2 as usize);

                // Extract lower 32 bits and sign-extend
                let src1_low = Self::extract_and_sign_extend(&mut builder, reg_file, src1);
                let src2_low = Self::extract_and_sign_extend(&mut builder, reg_file, src2);

                // Perform signed division on sign-extended values
                // The IR Div operation will handle the division
                builder.push(IROp::Div {
                    dst,
                    src1: src1_low,
                    src2: src2_low,
                    signed: true,
                });

                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build())
            }

            Some(DivInstruction::Divuw) => {
                // DIVUW: rd <- sext(rs1[31:0] / rs2[31:0])
                // Operates on lower 32 bits as unsigned, sign-extends result to 64 bits
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = reg_file.read(rs2 as usize);

                // Extract lower 32 bits (zero-extend for unsigned operation)
                let src1_low = Self::extract_and_zero_extend(&mut builder, reg_file, src1);
                let src2_low = Self::extract_and_zero_extend(&mut builder, reg_file, src2);

                // Perform unsigned division
                builder.push(IROp::Div {
                    dst,
                    src1: src1_low,
                    src2: src2_low,
                    signed: false,
                });

                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build())
            }

            Some(DivInstruction::Remw) => {
                // REMW: rd <- sext(rs1[31:0] % rs2[31:0])
                // Operates on lower 32 bits, sign-extends result to 64 bits
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = reg_file.read(rs2 as usize);

                // Extract lower 32 bits and sign-extend
                let src1_low = Self::extract_and_sign_extend(&mut builder, reg_file, src1);
                let src2_low = Self::extract_and_sign_extend(&mut builder, reg_file, src2);

                // Perform signed remainder on sign-extended values
                builder.push(IROp::Rem {
                    dst,
                    src1: src1_low,
                    src2: src2_low,
                    signed: true,
                });

                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build())
            }

            Some(DivInstruction::Remuw) => {
                // REMUW: rd <- sext(rs1[31:0] % rs2[31:0])
                // Operates on lower 32 bits as unsigned, sign-extends result to 64 bits
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = reg_file.read(rs2 as usize);

                // Extract lower 32 bits (zero-extend for unsigned operation)
                let src1_low = Self::extract_and_zero_extend(&mut builder, reg_file, src1);
                let src2_low = Self::extract_and_zero_extend(&mut builder, reg_file, src2);

                // Perform unsigned remainder
                builder.push(IROp::Rem {
                    dst,
                    src1: src1_low,
                    src2: src2_low,
                    signed: false,
                });

                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build())
            }

            None => Err(VmError::Execution(
                vm_core::ExecutionError::InvalidInstruction {
                    opcode: insn as u64,
                    pc,
                },
            )),
        }
    }

    /// Helper: Extract lower 32 bits and sign-extend to 64 bits
    fn extract_and_sign_extend(
        builder: &mut IRBuilder,
        reg_file: &mut RegisterFile,
        src: u32,
    ) -> u32 {
        // Create temporary register for the masked value
        let temp = reg_file.write(100); // Use high-numbered temp registers

        // Mask to get lower 32 bits
        let mask_32 = reg_file.write(101);
        builder.push(IROp::MovImm {
            dst: mask_32,
            imm: 0xFFFFFFFF,
        });

        builder.push(IROp::And {
            dst: temp,
            src1: src,
            src2: mask_32,
        });

        // Sign-extend: if bit 31 is set, set bits 63:32
        let sign_bit = reg_file.write(102);
        let sign_mask = reg_file.write(103);

        // Extract bit 31
        builder.push(IROp::SrlImm {
            dst: sign_bit,
            src: temp,
            sh: 31,
        });

        builder.push(IROp::And {
            dst: sign_bit,
            src1: sign_bit,
            src2: mask_32, // mask_32 is also 1
        });

        // Create sign extension mask (0xFFFFFFFF00000000 if sign bit set, 0 otherwise)
        builder.push(IROp::MovImm {
            dst: sign_mask,
            imm: 0xFFFFFFFF00000000,
        });

        // Zero-extend the sign bit to 64 bits
        let sign_extended = reg_file.write(104);
        builder.push(IROp::Mul {
            dst: sign_extended,
            src1: sign_bit,
            src2: sign_mask,
        });

        // OR with the lower 32 bits
        let result = reg_file.write(105);
        builder.push(IROp::Or {
            dst: result,
            src1: temp,
            src2: sign_extended,
        });

        result
    }

    /// Helper: Extract lower 32 bits and zero-extend to 64 bits
    fn extract_and_zero_extend(
        builder: &mut IRBuilder,
        reg_file: &mut RegisterFile,
        src: u32,
    ) -> u32 {
        // Create temporary register for the masked value
        let temp = reg_file.write(110); // Use different high-numbered temp registers

        // Mask to get lower 32 bits (zero-extend)
        let mask_32 = reg_file.write(111);
        builder.push(IROp::MovImm {
            dst: mask_32,
            imm: 0xFFFFFFFF,
        });

        builder.push(IROp::And {
            dst: temp,
            src1: src,
            src2: mask_32,
        });

        temp
    }
}

/// Division result type
#[derive(Debug, Clone, Copy)]
pub struct DivResult {
    pub quotient: i64,
    pub remainder: i64,
}

/// Helper trait for M extension division operations
pub trait DivOperations {
    /// Divide two 64-bit signed integers (DIV instruction)
    fn div(&self, a: i64, b: i64) -> i64;

    /// Divide two 64-bit unsigned integers (DIVU instruction)
    fn divu(&self, a: u64, b: u64) -> u64;

    /// Compute remainder of signed division (REM instruction)
    fn rem(&self, a: i64, b: i64) -> i64;

    /// Compute remainder of unsigned division (REMU instruction)
    fn remu(&self, a: u64, b: u64) -> u64;

    /// Divide two 32-bit signed integers and sign-extend (DIVW instruction)
    fn divw(&self, a: i64, b: i64) -> i64;

    /// Divide two 32-bit unsigned integers and sign-extend (DIVUW instruction)
    fn divuw(&self, a: u64, b: u64) -> i64;

    /// Compute remainder of 32-bit signed division and sign-extend (REMW instruction)
    fn remw(&self, a: i64, b: i64) -> i64;

    /// Compute remainder of 32-bit unsigned division and sign-extend (REMUW instruction)
    fn remuw(&self, a: u64, b: u64) -> i64;
}

/// Default implementation of M extension division operations
pub struct DefaultDivOps;

impl DivOperations for DefaultDivOps {
    fn div(&self, a: i64, b: i64) -> i64 {
        // RISC-V DIV specification:
        // - Division by zero: result is -1 (all bits set)
        // - MIN_INT / -1 = MIN_INT (no overflow)
        // - Other cases: standard signed division
        if b == 0 {
            // Division by zero
            -1
        } else if a == i64::MIN && b == -1 {
            // Special case: MIN_INT / -1 = MIN_INT (no overflow)
            i64::MIN
        } else {
            // Standard signed division
            a.wrapping_div(b)
        }
    }

    fn divu(&self, a: u64, b: u64) -> u64 {
        // RISC-V DIVU specification:
        // - Division by zero: result = all bits set (2^64 - 1)
        if b == 0 { u64::MAX } else { a / b }
    }

    fn rem(&self, a: i64, b: i64) -> i64 {
        // RISC-V REM specification:
        // - Division by zero: result = dividend (a)
        // - MIN_INT % -1 = 0
        if b == 0 {
            // Division by zero
            a
        } else if a == i64::MIN && b == -1 {
            // Special case: MIN_INT % -1 = 0
            0
        } else {
            // Standard signed remainder
            a.wrapping_rem(b)
        }
    }

    fn remu(&self, a: u64, b: u64) -> u64 {
        // RISC-V REMU specification:
        // - Division by zero: result = dividend (a)
        if b == 0 { a } else { a % b }
    }

    fn divw(&self, a: i64, b: i64) -> i64 {
        // RISC-V DIVW specification:
        // - Operates on lower 32 bits of operands
        // - Result is sign-extended to 64 bits
        // - Division by zero: result is -1 (all bits set)
        // - MIN_INT_32 / -1 = MIN_INT_32 (no overflow, then sign-extended)

        // Extract lower 32 bits and sign-extend
        let a_32 = (a as i32) as i64;
        let b_32 = (b as i32) as i64;

        if b_32 == 0 {
            // Division by zero: return -1 (all bits set)
            -1i64
        } else if a_32 == i32::MIN as i64 && b_32 == -1 {
            // Special case: MIN_32 / -1 = MIN_32, then sign-extended
            i32::MIN as i64
        } else {
            // Standard 32-bit signed division, sign-extend result
            (a_32.wrapping_div(b_32) as i32) as i64
        }
    }

    fn divuw(&self, a: u64, b: u64) -> i64 {
        // RISC-V DIVUW specification:
        // - Operates on lower 32 bits of operands (unsigned)
        // - Result is sign-extended to 64 bits
        // - Division by zero: result = -1 (all bits set, sign-extended)

        // Extract lower 32 bits (zero-extended)
        let a_32 = (a & 0xFFFFFFFF) as u32;
        let b_32 = (b & 0xFFFFFFFF) as u32;

        let result_32 = if b_32 == 0 {
            // Division by zero: return all bits set for 32-bit
            u32::MAX
        } else {
            a_32 / b_32
        };

        // Sign-extend the 32-bit result to 64 bits
        (result_32 as i32) as i64
    }

    fn remw(&self, a: i64, b: i64) -> i64 {
        // RISC-V REMW specification:
        // - Operates on lower 32 bits of operands
        // - Result is sign-extended to 64 bits
        // - Division by zero: result = dividend (sign-extended lower 32 bits)
        // - MIN_INT_32 % -1 = 0

        // Extract lower 32 bits and sign-extend
        let a_32 = (a as i32) as i64;
        let b_32 = (b as i32) as i64;

        if b_32 == 0 {
            // Division by zero: return dividend (sign-extended)
            a_32
        } else if a_32 == i32::MIN as i64 && b_32 == -1 {
            // Special case: MIN_32 % -1 = 0
            0
        } else {
            // Standard 32-bit signed remainder, sign-extend result
            (a_32.wrapping_rem(b_32) as i32) as i64
        }
    }

    fn remuw(&self, a: u64, b: u64) -> i64 {
        // RISC-V REMUW specification:
        // - Operates on lower 32 bits of operands (unsigned)
        // - Result is sign-extended to 64 bits
        // - Division by zero: result = dividend (sign-extended lower 32 bits)

        // Extract lower 32 bits (zero-extended)
        let a_32 = (a & 0xFFFFFFFF) as u32;
        let b_32 = (b & 0xFFFFFFFF) as u32;

        let result_32 = if b_32 == 0 {
            // Division by zero: return dividend
            a_32
        } else {
            a_32 % b_32
        };

        // Sign-extend the 32-bit result to 64 bits
        (result_32 as i32) as i64
    }
}

/// Encode M extension division instructions
pub mod encoding {

    /// DIV instruction encoding (RV64I 64-bit division)
    /// R-type: funct7=0x01, funct3=0x4, opcode=0x33
    pub fn encode_div(rd: u32, rs1: u32, rs2: u32) -> u32 {
        (0x01 << 25) | (rs2 << 20) | (rs1 << 15) | (0x4 << 12) | (rd << 7) | 0x33
    }

    /// DIVU instruction encoding (RV64I 64-bit unsigned division)
    /// R-type: funct7=0x01, funct3=0x5, opcode=0x33
    pub fn encode_divu(rd: u32, rs1: u32, rs2: u32) -> u32 {
        (0x01 << 25) | (rs2 << 20) | (rs1 << 15) | (0x5 << 12) | (rd << 7) | 0x33
    }

    /// REM instruction encoding (RV64I 64-bit remainder)
    /// R-type: funct7=0x01, funct3=0x6, opcode=0x33
    pub fn encode_rem(rd: u32, rs1: u32, rs2: u32) -> u32 {
        (0x01 << 25) | (rs2 << 20) | (rs1 << 15) | (0x6 << 12) | (rd << 7) | 0x33
    }

    /// REMU instruction encoding (RV64I 64-bit unsigned remainder)
    /// R-type: funct7=0x01, funct3=0x7, opcode=0x33
    pub fn encode_remu(rd: u32, rs1: u32, rs2: u32) -> u32 {
        (0x01 << 25) | (rs2 << 20) | (rs1 << 15) | (0x7 << 12) | (rd << 7) | 0x33
    }

    /// DIVW instruction encoding (RV64I 32-bit division, sign-extended)
    /// R-type: funct7=0x01, funct3=0x4, opcode=0x3B
    pub fn encode_divw(rd: u32, rs1: u32, rs2: u32) -> u32 {
        (0x01 << 25) | (rs2 << 20) | (rs1 << 15) | (0x4 << 12) | (rd << 7) | 0x3B
    }

    /// DIVUW instruction encoding (RV64I 32-bit unsigned division, sign-extended)
    /// R-type: funct7=0x01, funct3=0x5, opcode=0x3B
    pub fn encode_divuw(rd: u32, rs1: u32, rs2: u32) -> u32 {
        (0x01 << 25) | (rs2 << 20) | (rs1 << 15) | (0x5 << 12) | (rd << 7) | 0x3B
    }

    /// REMW instruction encoding (RV64I 32-bit remainder, sign-extended)
    /// R-type: funct7=0x01, funct3=0x6, opcode=0x3B
    pub fn encode_remw(rd: u32, rs1: u32, rs2: u32) -> u32 {
        (0x01 << 25) | (rs2 << 20) | (rs1 << 15) | (0x6 << 12) | (rd << 7) | 0x3B
    }

    /// REMUW instruction encoding (RV64I 32-bit unsigned remainder, sign-extended)
    /// R-type: funct7=0x01, funct3=0x7, opcode=0x3B
    pub fn encode_remuw(rd: u32, rs1: u32, rs2: u32) -> u32 {
        (0x01 << 25) | (rs2 << 20) | (rs1 << 15) | (0x7 << 12) | (rd << 7) | 0x3B
    }
}

/// Test utilities for M extension division
pub mod test_utils {

    /// Test case for division operations
    #[derive(Debug, Clone)]
    pub struct DivTestCase {
        pub a: i64,
        pub b: i64,
        pub expected_quotient: i64,
        pub expected_remainder: i64,
    }

    /// Test cases for DIV/REM (64-bit operations)
    pub fn get_signed_div_test_cases() -> Vec<DivTestCase> {
        vec![
            DivTestCase {
                a: 10,
                b: 3,
                expected_quotient: 3,
                expected_remainder: 1,
            },
            DivTestCase {
                a: 10,
                b: -3,
                expected_quotient: -3,
                expected_remainder: 1,
            },
            DivTestCase {
                a: -10,
                b: 3,
                expected_quotient: -3,
                expected_remainder: -1,
            },
            DivTestCase {
                a: -10,
                b: -3,
                expected_quotient: 3,
                expected_remainder: -1,
            },
            // Division by zero
            DivTestCase {
                a: 10,
                b: 0,
                expected_quotient: -1,
                expected_remainder: 10,
            },
            // MIN_INT / -1 edge case
            DivTestCase {
                a: i64::MIN,
                b: -1,
                expected_quotient: i64::MIN,
                expected_remainder: 0,
            },
        ]
    }

    /// Test cases for DIVU/REMU (64-bit unsigned operations)
    pub fn get_unsigned_div_test_cases() -> Vec<DivTestCase> {
        vec![
            DivTestCase {
                a: 10,
                b: 3,
                expected_quotient: 3,
                expected_remainder: 1,
            },
            DivTestCase {
                a: 0x100000000u64 as i64,
                b: 4,
                expected_quotient: 0x40000000,
                expected_remainder: 0,
            },
            // Division by zero
            DivTestCase {
                a: 10,
                b: 0,
                expected_quotient: -1i64 as u64 as i64, // u64::MAX as i64
                expected_remainder: 10,
            },
        ]
    }

    /// Test cases for DIVW/REMW (32-bit operations, sign-extended)
    pub fn get_signed_word_div_test_cases() -> Vec<DivTestCase> {
        vec![
            DivTestCase {
                a: 10,
                b: 3,
                expected_quotient: 3,
                expected_remainder: 1,
            },
            DivTestCase {
                a: 10,
                b: -3,
                expected_quotient: -3,
                expected_remainder: 1,
            },
            DivTestCase {
                a: -10,
                b: 3,
                expected_quotient: -3,
                expected_remainder: -1,
            },
            DivTestCase {
                a: -10,
                b: -3,
                expected_quotient: 3,
                expected_remainder: -1,
            },
            // Division by zero
            DivTestCase {
                a: 10,
                b: 0,
                expected_quotient: -1,
                expected_remainder: 10,
            },
            // MIN_INT_32 / -1 edge case
            DivTestCase {
                a: i32::MIN as i64,
                b: -1,
                expected_quotient: i32::MIN as i64,
                expected_remainder: 0,
            },
            // Test with values that have high bits set (should be ignored)
            DivTestCase {
                a: 0xFFFFFFFF0000000Au64 as i64,
                b: 3, // High bits should be ignored
                expected_quotient: 3,
                expected_remainder: 1,
            },
        ]
    }

    /// Test cases for DIVUW/REMUW (32-bit unsigned operations, sign-extended)
    pub fn get_unsigned_word_div_test_cases() -> Vec<DivTestCase> {
        vec![
            DivTestCase {
                a: 10,
                b: 3,
                expected_quotient: 3,
                expected_remainder: 1,
            },
            DivTestCase {
                a: 100,
                b: 7,
                expected_quotient: 14,
                expected_remainder: 2,
            },
            // Division by zero - result is sign-extended u32::MAX
            DivTestCase {
                a: 10,
                b: 0,
                expected_quotient: -1, // u32::MAX sign-extended to i64
                expected_remainder: 10,
            },
            // Test with values that have high bits set (should be ignored)
            DivTestCase {
                a: 0xFFFFFFFF0000000Au64 as i64,
                b: 3, // Only lower 32 bits used
                expected_quotient: 3,
                expected_remainder: 1,
            },
            // Large unsigned 32-bit values
            DivTestCase {
                a: 0xFFFFFF00u64 as i64,
                b: 0x10,
                expected_quotient: 0xFFFFFFF,
                expected_remainder: 0,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::test_utils::*;
    use super::*;

    #[test]
    fn test_div_decode() {
        // Test 64-bit division instruction decoding (opcode 0x33)
        let insn = encoding::encode_div(10, 11, 12);
        assert_eq!(DivDecoder::decode(insn), Some(DivInstruction::Div));

        let insn = encoding::encode_divu(10, 11, 12);
        assert_eq!(DivDecoder::decode(insn), Some(DivInstruction::Divu));

        let insn = encoding::encode_rem(10, 11, 12);
        assert_eq!(DivDecoder::decode(insn), Some(DivInstruction::Rem));

        let insn = encoding::encode_remu(10, 11, 12);
        assert_eq!(DivDecoder::decode(insn), Some(DivInstruction::Remu));

        // Test 32-bit word instruction decoding (opcode 0x3B)
        let insn = encoding::encode_divw(10, 11, 12);
        assert_eq!(DivDecoder::decode(insn), Some(DivInstruction::Divw));

        let insn = encoding::encode_divuw(10, 11, 12);
        assert_eq!(DivDecoder::decode(insn), Some(DivInstruction::Divuw));

        let insn = encoding::encode_remw(10, 11, 12);
        assert_eq!(DivDecoder::decode(insn), Some(DivInstruction::Remw));

        let insn = encoding::encode_remuw(10, 11, 12);
        assert_eq!(DivDecoder::decode(insn), Some(DivInstruction::Remuw));
    }

    #[test]
    fn test_signed_division_64bit() {
        let ops = DefaultDivOps;
        let test_cases = get_signed_div_test_cases();

        for test_case in test_cases {
            let quotient = ops.div(test_case.a, test_case.b);
            let remainder = ops.rem(test_case.a, test_case.b);

            assert_eq!(
                quotient, test_case.expected_quotient,
                "DIV({},{}) = {}, expected {}",
                test_case.a, test_case.b, quotient, test_case.expected_quotient
            );
            assert_eq!(
                remainder, test_case.expected_remainder,
                "REM({},{}) = {}, expected {}",
                test_case.a, test_case.b, remainder, test_case.expected_remainder
            );
        }
    }

    #[test]
    fn test_unsigned_division_64bit() {
        let ops = DefaultDivOps;
        let test_cases = get_unsigned_div_test_cases();

        for test_case in test_cases {
            let a_u64 = test_case.a as u64;
            let b_u64 = test_case.b as u64;
            let expected_quotient_u64 = test_case.expected_quotient as u64;
            let expected_remainder_u64 = test_case.expected_remainder as u64;

            let quotient = ops.divu(a_u64, b_u64);
            let remainder = ops.remu(a_u64, b_u64);

            assert_eq!(
                quotient, expected_quotient_u64,
                "DIVU({},{}) = {}, expected {}",
                a_u64, b_u64, quotient, expected_quotient_u64
            );
            assert_eq!(
                remainder, expected_remainder_u64,
                "REMU({},{}) = {}, expected {}",
                a_u64, b_u64, remainder, expected_remainder_u64
            );
        }
    }

    #[test]
    fn test_signed_division_32bit_word() {
        let ops = DefaultDivOps;
        let test_cases = get_signed_word_div_test_cases();

        for test_case in test_cases {
            let quotient = ops.divw(test_case.a, test_case.b);
            let remainder = ops.remw(test_case.a, test_case.b);

            assert_eq!(
                quotient, test_case.expected_quotient,
                "DIVW({:#x},{:#x}) = {:#x}, expected {:#x}",
                test_case.a, test_case.b, quotient, test_case.expected_quotient
            );
            assert_eq!(
                remainder, test_case.expected_remainder,
                "REMW({:#x},{:#x}) = {:#x}, expected {:#x}",
                test_case.a, test_case.b, remainder, test_case.expected_remainder
            );
        }
    }

    #[test]
    fn test_unsigned_division_32bit_word() {
        let ops = DefaultDivOps;
        let test_cases = get_unsigned_word_div_test_cases();

        for test_case in test_cases {
            let a_u64 = test_case.a as u64;
            let b_u64 = test_case.b as u64;

            let quotient = ops.divuw(a_u64, b_u64);
            let remainder = ops.remuw(a_u64, b_u64);

            assert_eq!(
                quotient, test_case.expected_quotient,
                "DIVUW({:#x},{:#x}) = {:#x}, expected {:#x}",
                test_case.a, test_case.b, quotient, test_case.expected_quotient
            );
            assert_eq!(
                remainder, test_case.expected_remainder,
                "REMUW({:#x},{:#x}) = {:#x}, expected {:#x}",
                test_case.a, test_case.b, remainder, test_case.expected_remainder
            );
        }
    }

    #[test]
    fn test_edge_cases_64bit() {
        let ops = DefaultDivOps;

        // Test 0 / 0
        assert_eq!(ops.div(0, 0), -1);
        assert_eq!(ops.divu(0, 0), u64::MAX);
        assert_eq!(ops.rem(0, 0), 0);
        assert_eq!(ops.remu(0, 0), 0);

        // Test division by 1
        assert_eq!(ops.div(42, 1), 42);
        assert_eq!(ops.divu(42, 1), 42);
        assert_eq!(ops.rem(42, 1), 0);
        assert_eq!(ops.remu(42, 1), 0);

        // Test division by -1 (except MIN_INT)
        assert_eq!(ops.div(42, -1), -42);
        assert_eq!(ops.rem(42, -1), 0);
    }

    #[test]
    fn test_edge_cases_32bit_word() {
        let ops = DefaultDivOps;

        // Test 0 / 0 (word instructions)
        assert_eq!(ops.divw(0, 0), -1);
        assert_eq!(ops.divuw(0, 0), -1); // u32::MAX sign-extended
        assert_eq!(ops.remw(0, 0), 0);
        assert_eq!(ops.remuw(0, 0), 0);

        // Test division by 1
        assert_eq!(ops.divw(42, 1), 42);
        assert_eq!(ops.divuw(42, 1), 42);
        assert_eq!(ops.remw(42, 1), 0);
        assert_eq!(ops.remuw(42, 1), 0);

        // Test sign extension behavior
        // Positive result should be sign-extended (no change for small values)
        assert_eq!(ops.divw(100, 5), 20);
        // Negative result should be sign-extended
        assert_eq!(ops.divw(-100, 5), -20);

        // Test that high bits are ignored
        assert_eq!(ops.divw(0xFFFFFFFF0000000Ai64, 3), 3);
        assert_eq!(ops.divuw(0xFFFFFFFF0000000Au64, 3), 3);
    }

    #[test]
    fn test_encoding_consistency() {
        // Test that encoding produces correct instruction words for all variants
        let div = encoding::encode_div(1, 2, 3);
        assert_eq!(DivDecoder::decode(div), Some(DivInstruction::Div));

        let divu = encoding::encode_divu(1, 2, 3);
        assert_eq!(DivDecoder::decode(divu), Some(DivInstruction::Divu));

        let rem = encoding::encode_rem(1, 2, 3);
        assert_eq!(DivDecoder::decode(rem), Some(DivInstruction::Rem));

        let remu = encoding::encode_remu(1, 2, 3);
        assert_eq!(DivDecoder::decode(remu), Some(DivInstruction::Remu));

        let divw = encoding::encode_divw(1, 2, 3);
        assert_eq!(DivDecoder::decode(divw), Some(DivInstruction::Divw));

        let divuw = encoding::encode_divuw(1, 2, 3);
        assert_eq!(DivDecoder::decode(divuw), Some(DivInstruction::Divuw));

        let remw = encoding::encode_remw(1, 2, 3);
        assert_eq!(DivDecoder::decode(remw), Some(DivInstruction::Remw));

        let remuw = encoding::encode_remuw(1, 2, 3);
        assert_eq!(DivDecoder::decode(remuw), Some(DivInstruction::Remuw));
    }

    #[test]
    fn test_opcode_differences() {
        // Verify that 64-bit and 32-bit word instructions use different opcodes
        let div = encoding::encode_div(5, 6, 7);
        let divw = encoding::encode_divw(5, 6, 7);

        // Extract opcode (lower 7 bits)
        let div_opcode = div & 0x7F;
        let divw_opcode = divw & 0x7F;

        assert_eq!(div_opcode, 0x33, "DIV should use opcode 0x33");
        assert_eq!(divw_opcode, 0x3B, "DIVW should use opcode 0x3B");
        assert_ne!(
            div_opcode, divw_opcode,
            "DIV and DIVW should use different opcodes"
        );

        // Funct3 and funct7 should be the same
        let div_funct3 = (div >> 12) & 0x7;
        let divw_funct3 = (divw >> 12) & 0x7;
        let div_funct7 = (div >> 25) & 0x7F;
        let divw_funct7 = (divw >> 25) & 0x7F;

        assert_eq!(div_funct3, divw_funct3, "funct3 should be the same");
        assert_eq!(div_funct7, divw_funct7, "funct7 should be the same");
    }

    #[test]
    fn test_risc_v_spec_compliance() {
        let ops = DefaultDivOps;

        // Test RISC-V specified division by zero behavior
        // For DIV: division by zero returns -1 (all bits set)
        assert_eq!(ops.div(42, 0), -1);
        assert_eq!(ops.divw(42, 0), -1);

        // For DIVU: division by zero returns all bits set
        assert_eq!(ops.divu(42, 0), u64::MAX);
        assert_eq!(ops.divuw(42, 0) as i64, -1i64); // u32::MAX sign-extended

        // For REM: division by zero returns dividend
        assert_eq!(ops.rem(42, 0), 42);
        assert_eq!(ops.remw(42, 0), 42);

        // For REMU: division by zero returns dividend
        assert_eq!(ops.remu(42, 0), 42);
        assert_eq!(ops.remuw(42, 0), 42);

        // Test overflow case: MIN_INT / -1 = MIN_INT (no overflow trap)
        assert_eq!(ops.div(i64::MIN, -1), i64::MIN);
        assert_eq!(ops.divw(i32::MIN as i64, -1), i32::MIN as i64);

        // Test overflow case for remainder: MIN_INT % -1 = 0
        assert_eq!(ops.rem(i64::MIN, -1), 0);
        assert_eq!(ops.remw(i32::MIN as i64, -1), 0);
    }

    #[test]
    fn test_sign_extension_behavior() {
        let ops = DefaultDivOps;

        // Test that positive results are properly sign-extended (no change for small positive)
        assert_eq!(ops.divw(10, 2), 5);
        assert_eq!(ops.divuw(10, 2), 5);

        // Test that negative results are properly sign-extended
        assert_eq!(ops.divw(-10, 2), -5);

        // Test that unsigned results are sign-extended (high bit set causes sign extension)
        // 0xFFFFFF00 / 0x10 = 0x0FFFFFFF, but sign-extended as i32 = -1
        // Actually in RISC-V, the result is sign-extended, so:
        // u32::MAX / 2 = 0x7FFFFFFF, which sign-extended is still 0x7FFFFFFF
        assert_eq!(ops.divuw(u32::MAX as u64, 2), (u32::MAX / 2) as i32 as i64);
    }
}
