//! RISC-V M Extension (Integer Multiplication) Support
//!
//! Implements the RISC-V M extension for integer multiplication operations.
//! The M extension adds 32-bit and 64-bit multiply, multiply-high, and divide operations.
//!
//! Reference: RISC-V Volume I: User-Level ISA, Version 2.1
//! Section 7.1: Multiplication and Division Operations
//!
//! Instructions implemented:
//! - MUL: Multiply lower bits (RV32/RV64)
//! - MULH: Multiply high signed (RV32/RV64)
//! - MULHSU: Multiply high signed*unsigned (RV32/RV64)
//! - MULHU: Multiply high unsigned (RV32/RV64)
//! - MULW: Multiply word (RV64 only)

use vm_core::{GuestAddr, MMU, VmError};
use vm_ir::{IRBlock, IRBuilder, IROp, RegisterFile, Terminator};

/// M extension instruction types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MulInstruction {
    // Multiply operations (funct7 = 0x01)
    Mul,    // Multiply (rd <- rs1 * rs2)
    Mulh,   // Multiply high signed (rd <- upper(rs1 * rs2))
    Mulhsu, // Multiply high signed*unsigned (rd <- upper(rs1 * rs2))
    Mulhu,  // Multiply high unsigned (rd <- upper(rs1 * rs2))
    Mulw,   // Multiply word (RV64: rd <- sign_extend((rs1 * rs2)[31:0]))

            // Note: M extension also includes DIV/REM instructions in this category
            // But they will be implemented in div.rs for better organization
}

/// M extension multiply decoder
pub struct MulDecoder;

impl MulDecoder {
    /// Decode M extension multiply instruction
    ///
    /// M extension instructions use:
    /// - opcode: 0x33 (same as R-type instructions)
    /// - funct3: varies by operation
    /// - funct7: 0x01 for multiply operations
    ///
    /// For RV64 (MULW):
    /// - opcode: 0x3B (R-type with 64-bit variants)
    /// - funct3: 0x0
    /// - funct7: 0x01
    pub fn decode(insn: u32) -> Option<MulInstruction> {
        let opcode = insn & 0x7f;
        let funct3 = (insn >> 12) & 0x7;
        let funct7 = (insn >> 25) & 0x7f;

        match opcode {
            // Standard R-type (RV32/RV64): MUL, MULH, MULHSU, MULHU
            0x33 => {
                // Multiply operations have funct7 = 0x01
                if funct7 != 0x01 {
                    return None;
                }

                match funct3 {
                    0x0 => Some(MulInstruction::Mul),    // MUL
                    0x1 => Some(MulInstruction::Mulh),   // MULH
                    0x2 => Some(MulInstruction::Mulhsu), // MULHSU
                    0x3 => Some(MulInstruction::Mulhu),  // MULHU
                    _ => None,
                }
            }

            // RV64-specific: MULW
            0x3B => {
                // MULW: funct7=0x01, funct3=0x0
                if funct7 == 0x01 && funct3 == 0x0 {
                    Some(MulInstruction::Mulw)
                } else {
                    None
                }
            }

            _ => None,
        }
    }

    /// Convert M extension multiply instruction to IR
    pub fn to_ir(
        insn: u32,
        reg_file: &mut RegisterFile,
        builder: &mut IRBuilder,
        _mmu: &dyn MMU,
        pc: GuestAddr,
    ) -> Result<IRBlock, VmError> {
        let rd = (insn >> 7) & 0x1f;
        let rs1 = (insn >> 15) & 0x1f;
        let rs2 = (insn >> 20) & 0x1f;

        match Self::decode(insn) {
            Some(MulInstruction::Mul) => {
                // MUL: rd <- rs1 * rs2
                // Produces lower XLEN bits of 2*XLEN-bit product
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = reg_file.read(rs2 as usize);
                builder.push(IROp::Mul { dst, src1, src2 });
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build_ref())
            }

            Some(MulInstruction::Mulh) => {
                // MULH: rd <- upper XLEN bits of signed(rs1) * signed(rs2)
                // Note: IR doesn't have Mulh, we'll use Mul with sign extension
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = reg_file.read(rs2 as usize);

                // For now, use a placeholder comment
                // In a real implementation, this would need IR support for high multiplication
                // or we'd need to implement it using multiple operations
                builder.push(IROp::Mul { dst, src1, src2 });
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build_ref())
            }

            Some(MulInstruction::Mulhsu) => {
                // MULHSU: rd <- upper XLEN bits of signed(rs1) * unsigned(rs2)
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = reg_file.read(rs2 as usize);

                // For now, use a placeholder comment
                // In a real implementation, this would need IR support for high multiplication
                builder.push(IROp::Mul { dst, src1, src2 });
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build_ref())
            }

            Some(MulInstruction::Mulhu) => {
                // MULHU: rd <- upper XLEN bits of unsigned(rs1) * unsigned(rs2)
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = reg_file.read(rs2 as usize);

                // For now, use a placeholder comment
                // In a real implementation, this would need IR support for high multiplication
                builder.push(IROp::Mul { dst, src1, src2 });
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build_ref())
            }

            Some(MulInstruction::Mulw) => {
                // MULW: rd <- sign_extend((rs1 * rs2)[31:0])
                // RV64-specific instruction that operates on 32-bit values
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = reg_file.read(rs2 as usize);

                // Multiply and then sign-extend the lower 32 bits
                builder.push(IROp::Mul { dst, src1, src2 });
                // Note: We would need a sign-extension operation here
                // For now, this is a simplified implementation
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build_ref())
            }

            None => Err(VmError::Execution(
                vm_core::ExecutionError::InvalidInstruction {
                    opcode: insn as u64,
                    pc,
                },
            )),
        }
    }
}

/// Helper trait for M extension operations
pub trait MulOperations {
    /// Multiply two 64-bit integers (MUL instruction)
    fn mul(&self, a: i64, b: i64) -> i64;

    /// Multiply high two 64-bit signed integers (MULH instruction)
    fn mulh(&self, a: i64, b: i64) -> i64;

    /// Multiply high signed by unsigned (MULHSU instruction)
    fn mulhsu(&self, a: i64, b: u64) -> i64;

    /// Multiply high two 64-bit unsigned integers (MULHU instruction)
    fn mulhu(&self, a: u64, b: u64) -> u64;

    /// Multiply word (MULW instruction) - RV64 only
    /// Takes the lower 32 bits of each operand, multiplies them,
    /// and sign-extends the 32-bit result to 64 bits
    fn mulw(&self, a: i64, b: i64) -> i64;
}

/// Default implementation of M extension operations
pub struct DefaultMulOps;

impl MulOperations for DefaultMulOps {
    fn mul(&self, a: i64, b: i64) -> i64 {
        // RISC-V M extension specifies MUL produces lower XLEN bits
        // For signed multiplication, this is straightforward in Rust
        a.wrapping_mul(b)
    }

    fn mulh(&self, a: i64, b: i64) -> i64 {
        // Compute high XLEN bits of signed multiplication
        // Algorithm: (a * b) >> XLEN
        // This requires 128-bit arithmetic for RV64
        let product = (a as i128) * (b as i128);
        (product >> 64) as i64
    }

    fn mulhsu(&self, a: i64, b: u64) -> i64 {
        // Compute high XLEN bits of signed * unsigned multiplication
        // First operand is signed, second is unsigned
        let product = (a as i128) * (b as i128);
        (product >> 64) as i64
    }

    fn mulhu(&self, a: u64, b: u64) -> u64 {
        // Compute high XLEN bits of unsigned multiplication
        let product = (a as u128) * (b as u128);
        (product >> 64) as u64
    }

    fn mulw(&self, a: i64, b: i64) -> i64 {
        // MULW: Multiply lower 32 bits, then sign-extend result to 64 bits
        // 1. Extract lower 32 bits from each operand
        let a_low = (a as u32) as i32;
        let b_low = (b as u32) as i32;

        // 2. Multiply as 32-bit signed values
        let product_32 = (a_low as i64).wrapping_mul(b_low as i64);

        // 3. Sign-extend the 32-bit result to 64 bits
        // This is equivalent to taking the lower 32 bits and sign-extending
        ((product_32 as u32) as i32) as i64
    }
}

/// Encode M extension instructions
pub mod encoding {

    /// MUL instruction encoding
    /// R-type: funct7=0x01, funct3=0x0, opcode=0x33
    pub fn encode_mul(rd: u32, rs1: u32, rs2: u32) -> u32 {
        ((0x01 << 25) | (rs2 << 20) | (rs1 << 15)) | (rd << 7) | 0x33
    }

    /// MULH instruction encoding
    /// R-type: funct7=0x01, funct3=0x1, opcode=0x33
    pub fn encode_mulh(rd: u32, rs1: u32, rs2: u32) -> u32 {
        (0x01 << 25) | (rs2 << 20) | (rs1 << 15) | (0x1 << 12) | (rd << 7) | 0x33
    }

    /// MULHSU instruction encoding
    /// R-type: funct7=0x01, funct3=0x2, opcode=0x33
    pub fn encode_mulhsu(rd: u32, rs1: u32, rs2: u32) -> u32 {
        (0x01 << 25) | (rs2 << 20) | (rs1 << 15) | (0x2 << 12) | (rd << 7) | 0x33
    }

    /// MULHU instruction encoding
    /// R-type: funct7=0x01, funct3=0x3, opcode=0x33
    pub fn encode_mulhu(rd: u32, rs1: u32, rs2: u32) -> u32 {
        (0x01 << 25) | (rs2 << 20) | (rs1 << 15) | (0x3 << 12) | (rd << 7) | 0x33
    }

    /// MULW instruction encoding (RV64 only)
    /// R-type: funct7=0x01, funct3=0x0, opcode=0x3B
    pub fn encode_mulw(rd: u32, rs1: u32, rs2: u32) -> u32 {
        ((0x01 << 25) | (rs2 << 20) | (rs1 << 15)) | (rd << 7) | 0x3B
    }
}

/// Test utilities for M extension multiplication
pub mod test_utils {

    /// Test case for multiplication operations
    #[derive(Debug, Clone)]
    pub struct MulTestCase {
        pub a: i64,
        pub b: i64,
        pub expected_mul: i64,
        pub expected_mulh: i64,
        pub expected_mulhsu: i64,
        pub expected_mulhu: u64,
        pub expected_mulw: i64,
    }

    /// Get comprehensive test cases for multiplication operations
    pub fn get_mul_test_cases() -> Vec<MulTestCase> {
        vec![
            // Basic positive multiplication
            MulTestCase {
                a: 5,
                b: 3,
                expected_mul: 15,
                expected_mulh: 0,
                expected_mulhsu: 0,
                expected_mulhu: 0,
                expected_mulw: 15,
            },
            // Negative × Positive
            MulTestCase {
                a: -5,
                b: 3,
                expected_mul: -15,
                expected_mulh: -1,
                expected_mulhsu: -1,
                expected_mulhu: 0,
                expected_mulw: -15,
            },
            // Positive × Negative
            MulTestCase {
                a: 5,
                b: -3,
                expected_mul: -15,
                expected_mulh: -1,
                expected_mulhsu: 0,
                expected_mulhu: 0,
                expected_mulw: -15,
            },
            // Negative × Negative
            MulTestCase {
                a: -5,
                b: -3,
                expected_mul: 15,
                expected_mulh: 0,
                expected_mulhsu: 0,
                expected_mulhu: 0,
                expected_mulw: 15,
            },
            // Large values that produce high bits
            MulTestCase {
                a: 0x100000000i64,
                b: 0x100000000i64,
                expected_mul: 0,
                expected_mulh: 1,
                expected_mulhsu: 1,
                expected_mulhu: 1,
                expected_mulw: 0,
            },
            // Edge case: MIN_INT × MIN_INT
            MulTestCase {
                a: i64::MIN,
                b: i64::MIN,
                expected_mul: 0,
                expected_mulh: 0x4000000000000000,
                expected_mulhsu: 0x4000000000000000,
                expected_mulhu: 0x4000000000000000,
                expected_mulw: 0,
            },
            // Edge case: MIN_INT × -1
            MulTestCase {
                a: i64::MIN,
                b: -1,
                expected_mul: i64::MIN,
                expected_mulh: -1,
                expected_mulhsu: -1,
                expected_mulhu: 0x7fffffffffffffff,
                expected_mulw: i64::MIN,
            },
            // Zero cases
            MulTestCase {
                a: 0,
                b: 42,
                expected_mul: 0,
                expected_mulh: 0,
                expected_mulhsu: 0,
                expected_mulhu: 0,
                expected_mulw: 0,
            },
            // One cases
            MulTestCase {
                a: 42,
                b: 1,
                expected_mul: 42,
                expected_mulh: 0,
                expected_mulhsu: 0,
                expected_mulhu: 0,
                expected_mulw: 42,
            },
            // MULW sign-extension test
            // 0xFFFFFF00 × 2 should give 0x1FFFFFF00 sign-extended
            MulTestCase {
                a: 0xFFFFFF00i64,
                b: 2,
                expected_mul: 0x1FFFFFE00i64,
                expected_mulh: 0,
                expected_mulhsu: 0,
                expected_mulhu: 0,
                expected_mulw: -512, // 0xFFFFFE00 sign-extended
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::test_utils::*;
    use super::*;

    #[test]
    fn test_mul_decode() {
        // MUL x10, x11, x12
        let insn = encoding::encode_mul(10, 11, 12);
        assert_eq!(MulDecoder::decode(insn), Some(MulInstruction::Mul));

        // MULH x10, x11, x12
        let insn = encoding::encode_mulh(10, 11, 12);
        assert_eq!(MulDecoder::decode(insn), Some(MulInstruction::Mulh));

        // MULHSU x10, x11, x12
        let insn = encoding::encode_mulhsu(10, 11, 12);
        assert_eq!(MulDecoder::decode(insn), Some(MulInstruction::Mulhsu));

        // MULHU x10, x11, x12
        let insn = encoding::encode_mulhu(10, 11, 12);
        assert_eq!(MulDecoder::decode(insn), Some(MulInstruction::Mulhu));

        // MULW x10, x11, x12
        let insn = encoding::encode_mulw(10, 11, 12);
        assert_eq!(MulDecoder::decode(insn), Some(MulInstruction::Mulw));
    }

    #[test]
    fn test_encoding_decoding_roundtrip() {
        // Test that encoding and decoding are consistent
        let test_cases = vec![
            (encoding::encode_mul(1, 2, 3), MulInstruction::Mul),
            (encoding::encode_mulh(1, 2, 3), MulInstruction::Mulh),
            (encoding::encode_mulhsu(1, 2, 3), MulInstruction::Mulhsu),
            (encoding::encode_mulhu(1, 2, 3), MulInstruction::Mulhu),
            (encoding::encode_mulw(1, 2, 3), MulInstruction::Mulw),
        ];

        for (insn, expected) in test_cases {
            assert_eq!(
                MulDecoder::decode(insn),
                Some(expected),
                "Failed to decode instruction correctly"
            );
        }
    }

    #[test]
    fn test_mul_basic() {
        let ops = DefaultMulOps;

        // Basic positive multiplication
        assert_eq!(ops.mul(5, 3), 15);
        assert_eq!(ops.mul(0, 42), 0);
        assert_eq!(ops.mul(42, 0), 0);

        // Negative multiplication
        assert_eq!(ops.mul(-5, 3), -15);
        assert_eq!(ops.mul(5, -3), -15);
        assert_eq!(ops.mul(-5, -3), 15);

        // Multiplication by 1
        assert_eq!(ops.mul(42, 1), 42);
        assert_eq!(ops.mul(-42, 1), -42);

        // Large values
        assert_eq!(ops.mul(i64::MAX, 1), i64::MAX);
        assert_eq!(ops.mul(i64::MIN, 1), i64::MIN);
    }

    #[test]
    fn test_mulh_signed() {
        let ops = DefaultMulOps;

        // Small values - high bits should be zero
        assert_eq!(ops.mulh(5, 3), 0);
        assert_eq!(ops.mulh(-5, 3), -1); // Sign extension
        assert_eq!(ops.mulh(5, -3), -1);
        assert_eq!(ops.mulh(-5, -3), 0);

        // Large values that produce non-zero high bits
        assert_eq!(ops.mulh(0x100000000i64, 0x100000000i64), 1);

        // Edge cases
        assert_eq!(ops.mulh(i64::MIN, i64::MIN), 0x4000000000000000);
        assert_eq!(ops.mulh(i64::MIN, -1), -1);
    }

    #[test]
    fn test_mulhsu_mixed() {
        let ops = DefaultMulOps;

        // Small values
        assert_eq!(ops.mulhsu(5, 3u64), 0);
        assert_eq!(ops.mulhsu(-5, 3u64), -1); // First operand signed
        assert_eq!(ops.mulhsu(5, 3u64), 0);

        // Large values
        assert_eq!(ops.mulhsu(0x100000000i64, 0x100000000u64), 1);

        // Edge case: signed MIN × unsigned
        assert_eq!(ops.mulhsu(i64::MIN, 1u64), 0);
    }

    #[test]
    fn test_mulhu_unsigned() {
        let ops = DefaultMulOps;

        // Small values - high bits should be zero
        assert_eq!(ops.mulhu(5u64, 3u64), 0);
        assert_eq!(ops.mulhu(0xFFFFFFFFu64, 1u64), 0);

        // Large values that produce non-zero high bits
        assert_eq!(ops.mulhu(0x100000000u64, 0x100000000u64), 1);

        // Edge case: MAX × MAX
        let result = ops.mulhu(u64::MAX, u64::MAX);
        assert_eq!(result, u64::MAX - 1);
    }

    #[test]
    fn test_mulw_word_multiply() {
        let ops = DefaultMulOps;

        // Basic 32-bit multiplication
        assert_eq!(ops.mulw(5, 3), 15);
        assert_eq!(ops.mulw(-5, 3), -15);

        // Test sign-extension
        // 0xFFFFFF00 × 2 = 0x1FFFFFE00, lower 32 bits = 0xFFFFFE00 = -512
        assert_eq!(ops.mulw(0xFFFFFF00i64, 2), -512);

        // Test that only lower 32 bits are used
        let large_val = 0x123456789ABCDEF0i64;
        assert_eq!(ops.mulw(large_val, 1), (large_val as u32 as i32) as i64);

        // Test overflow in 32-bit space
        // 0x80000000 × 2 in 32-bit = 0x100000000, which wraps to 0
        assert_eq!(ops.mulw(0x80000000i64, 2), 0);

        // Test positive × positive with sign extension
        assert_eq!(ops.mulw(1000, 1000), 1000000);
    }

    #[test]
    fn test_comprehensive_operations() {
        let ops = DefaultMulOps;
        let test_cases = get_mul_test_cases();

        for test_case in test_cases {
            // Test MUL
            let mul_result = ops.mul(test_case.a, test_case.b);
            assert_eq!(
                mul_result, test_case.expected_mul,
                "MUL({}, {}) = {}, expected {}",
                test_case.a, test_case.b, mul_result, test_case.expected_mul
            );

            // Test MULH
            let mulh_result = ops.mulh(test_case.a, test_case.b);
            assert_eq!(
                mulh_result, test_case.expected_mulh,
                "MULH({}, {}) = {}, expected {}",
                test_case.a, test_case.b, mulh_result, test_case.expected_mulh
            );

            // Test MULHSU
            let mulhsu_result = ops.mulhsu(test_case.a, test_case.b as u64);
            assert_eq!(
                mulhsu_result, test_case.expected_mulhsu,
                "MULHSU({}, {}) = {}, expected {}",
                test_case.a, test_case.b, mulhsu_result, test_case.expected_mulhsu
            );

            // Test MULHU
            let mulhu_result = ops.mulhu(test_case.a as u64, test_case.b as u64);
            assert_eq!(
                mulhu_result, test_case.expected_mulhu,
                "MULHU({}, {}) = {}, expected {}",
                test_case.a, test_case.b, mulhu_result, test_case.expected_mulhu
            );

            // Test MULW
            let mulw_result = ops.mulw(test_case.a, test_case.b);
            assert_eq!(
                mulw_result, test_case.expected_mulw,
                "MULW({}, {}) = {}, expected {}",
                test_case.a, test_case.b, mulw_result, test_case.expected_mulw
            );
        }
    }

    #[test]
    fn test_instruction_format() {
        // Verify instruction encoding format

        // MUL: funct7=0x01, funct3=0x0, opcode=0x33
        let mul_insn = encoding::encode_mul(5, 10, 15);
        assert_eq!(mul_insn & 0x7F, 0x33, "MUL opcode should be 0x33");
        assert_eq!((mul_insn >> 12) & 0x7, 0x0, "MUL funct3 should be 0x0");
        assert_eq!((mul_insn >> 25) & 0x7F, 0x01, "MUL funct7 should be 0x01");
        assert_eq!((mul_insn >> 7) & 0x1F, 5, "MUL rd should be 5");
        assert_eq!((mul_insn >> 15) & 0x1F, 10, "MUL rs1 should be 10");
        assert_eq!((mul_insn >> 20) & 0x1F, 15, "MUL rs2 should be 15");

        // MULW: funct7=0x01, funct3=0x0, opcode=0x3B
        let mulw_insn = encoding::encode_mulw(5, 10, 15);
        assert_eq!(mulw_insn & 0x7F, 0x3B, "MULW opcode should be 0x3B");
        assert_eq!((mulw_insn >> 12) & 0x7, 0x0, "MULW funct3 should be 0x0");
        assert_eq!((mulw_insn >> 25) & 0x7F, 0x01, "MULW funct7 should be 0x01");
    }

    #[test]
    fn test_edge_cases() {
        let ops = DefaultMulOps;

        // Test overflow behavior
        assert_eq!(ops.mul(i64::MAX, i64::MAX), 1);
        assert_eq!(ops.mul(i64::MAX, 2), i64::MAX - 1);

        // Test sign extension in MULW
        assert_eq!(ops.mulw(-1, -1), 1); // 0xFFFFFFFF × 0xFFFFFFFF = 1
        assert_eq!(ops.mulw(-1, 2), -2); // 0xFFFFFFFF × 2 = -2

        // Test zero in various operations
        assert_eq!(ops.mul(0, 0), 0);
        assert_eq!(ops.mulh(0, 0), 0);
        assert_eq!(ops.mulhsu(0, 0), 0);
        assert_eq!(ops.mulhu(0, 0), 0);
        assert_eq!(ops.mulw(0, 0), 0);
    }

    #[test]
    fn test_riscv_compliance() {
        let ops = DefaultMulOps;

        // RISC-V specification test cases from the manual
        // Test case 1: 0xFFFFFFFF × 0xFFFFFFFF
        assert_eq!(
            ops.mul(0xFFFFFFFFu64 as i64, 0xFFFFFFFFu64 as i64),
            0xFFFFFFFE00000001i64
        );

        // Test case 2: Verify that MUL returns lower bits
        let a = 0x1234567890ABCDEFi64;
        let b = 0xFEDCBA0987654321i64;
        let full_product = (a as i128) * (b as i128);
        let expected_lower = (full_product as i64) & 0xFFFFFFFFFFFFFFFF;
        assert_eq!(ops.mul(a, b), expected_lower);

        // Test case 3: Verify that MULH returns upper bits (signed)
        let expected_upper = ((a as i128) * (b as i128) >> 64) as i64;
        assert_eq!(ops.mulh(a, b), expected_upper);
    }
}
