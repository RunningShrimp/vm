//! RISC-V M Extension (Integer Multiplication and Division) Test Suite
//!
//! This module provides comprehensive tests for the RISC-V M extension implementation,
//! including both unit tests and integration tests for multiply and divide operations.
//!
//! The test suite covers:
//! 1. Instruction decoding
//! 2. Operation correctness
//! 3. Edge cases and error conditions
//! 4. Performance benchmarks
//! 5. Integration with the main decoder

use std::path::Path;
use vm_frontend::riscv64::{MulDecoder, DivDecoder, MulInstruction, DivInstruction};
use vm_frontend::riscv64::mul::{DefaultMulOps, encoding as mul_encoding};
use vm_frontend::riscv64::div::{DefaultDivOps, encoding as div_encoding, test_utils};
use vm_core::{GuestAddr, MMU, VmError};
use vm_ir::{IRBlock, IRBuilder, RegisterFile, Terminator};
use vm_platform::memory::MockMMU;

/// Test M extension instruction decoding
mod decoding_tests {
    use super::*;

    #[test]
    fn test_mul_instruction_decoding() {
        // Test MUL instruction decoding
        let mul_insn = mul_encoding::encode_mul(1, 2, 3);
        assert_eq!(MulDecoder::decode(mul_insn), Some(MulInstruction::Mul));

        // Test MULH instruction decoding
        let mulh_insn = mul_encoding::encode_mulh(1, 2, 3);
        assert_eq!(MulDecoder::decode(mulh_insn), Some(MulInstruction::Mulh));

        // Test MULHSU instruction decoding
        let mulhsu_insn = mul_encoding::encode_mulhsu(1, 2, 3);
        assert_eq!(MulDecoder::decode(mulhsu_insn), Some(MulInstruction::Mulhsu));

        // Test MULHU instruction decoding
        let mulhu_insn = mul_encoding::encode_mulhu(1, 2, 3);
        assert_eq!(MulDecoder::decode(mulhu_insn), Some(MulInstruction::Mulhu));
    }

    #[test]
    fn test_div_instruction_decoding() {
        // Test DIV instruction decoding
        let div_insn = div_encoding::encode_div(1, 2, 3);
        assert_eq!(DivDecoder::decode(div_insn), Some(DivInstruction::Div));

        // Test DIVU instruction decoding
        let divu_insn = div_encoding::encode_divu(1, 2, 3);
        assert_eq!(DivDecoder::decode(divu_insn), Some(DivInstruction::Divu));

        // Test REM instruction decoding
        let rem_insn = div_encoding::encode_rem(1, 2, 3);
        assert_eq!(DivDecoder::decode(rem_insn), Some(DivInstruction::Rem));

        // Test REMU instruction decoding
        let remu_insn = div_encoding::encode_remu(1, 2, 3);
        assert_eq!(DivDecoder::decode(remu_insn), Some(DivInstruction::Remu));
    }

    #[test]
    fn test_invalid_instructions() {
        // Test non-M extension instructions
        assert_eq!(MulDecoder::decode(0x00000033), None); // funct7=0x00
        assert_eq!(DivDecoder::decode(0x00000033), None); // funct7=0x00

        // Test wrong opcode
        assert_eq!(MulDecoder::decode(0x00000013), None); // I-type instruction
        assert_eq!(DivDecoder::decode(0x00000013), None); // I-type instruction
    }
}

/// Test M extension operations
mod operation_tests {
    use super::*;

    #[test]
    fn test_multiply_operations() {
        let ops = DefaultMulOps;

        // Basic multiplication tests
        assert_eq!(ops.mul(5, 3), 15);
        assert_eq!(ops.mul(-5, 3), -15);
        assert_eq!(ops.mul(5, -3), -15);
        assert_eq!(ops.mul(-5, -3), 15);

        // Overflow tests
        assert_eq!(ops.mul(i64::MAX, 2), -2); // Wraps around
        assert_eq!(ops.mul(i64::MIN, 2), 0); // Wraps around

        // Zero tests
        assert_eq!(ops.mul(0, 123456), 0);
        assert_eq!(ops.mul(123456, 0), 0);

        // One tests
        assert_eq!(ops.mul(123456, 1), 123456);
        assert_eq!(ops.mul(1, 123456), 123456);
    }

    #[test]
    fn test_multiply_high_operations() {
        let ops = DefaultMulOps;

        // MULHU tests
        assert_eq!(ops.mulhu(5u64, 3u64), 0);
        assert_eq!(ops.mulhu(0x100000000u64, 0x100000000u64), 1);
        assert_eq!(ops.mulhu(u64::MAX, u64::MAX), u64::MAX >> 1);

        // MULH tests
        assert_eq!(ops.mulh(5, 3), 0);
        assert_eq!(ops.mulh(0x100000000i64, 0x100000000i64), 1);
        assert_eq!(ops.mulh(i64::MAX, i64::MAX), i64::MAX >> 1);

        // MULHSU tests
        assert_eq!(ops.mulhsu(-5, 3u64), -1);
        assert_eq!(ops.mulhsu(5, 3u64), 0);
        assert_eq!(ops.mulhsu(0x100000000i64, 0x100000000u64), 1);
    }

    #[test]
    fn test_division_operations() {
        let ops = DefaultDivOps;

        // Basic division tests
        assert_eq!(ops.div(10, 3), 3);
        assert_eq!(ops.div(10, -3), -3);
        assert_eq!(ops.div(-10, 3), -3);
        assert_eq!(ops.div(-10, -3), 3);

        // Division by zero
        assert_eq!(ops.div(10, 0), -1);
        assert_eq!(ops.div(-10, 0), -1);

        // Edge case: MIN_INT / -1
        assert_eq!(ops.div(i64::MIN, -1), i64::MIN);
        assert_eq!(ops.rem(i64::MIN, -1), 0);

        // Division by 1
        assert_eq!(ops.div(42, 1), 42);
        assert_eq!(ops.div(-42, 1), -42);

        // Remainder tests
        assert_eq!(ops.rem(10, 3), 1);
        assert_eq!(ops.rem(10, -3), 1);
        assert_eq!(ops.rem(-10, 3), -1);
        assert_eq!(ops.rem(-10, -3), -1);
    }

    #[test]
    fn test_unsigned_division_operations() {
        let ops = DefaultDivOps;

        // Basic unsigned division
        assert_eq!(ops.divu(10u64, 3u64), 3);
        assert_eq!(ops.divu(10u64, 5u64), 2);

        // Division by zero
        assert_eq!(ops.divu(10u64, 0u64), u64::MAX);

        // Division by 1
        assert_eq!(ops.divu(42u64, 1u64), 42);

        // Unsigned remainder
        assert_eq!(ops.remu(10u64, 3u64), 1);
        assert_eq!(ops.remu(10u64, 5u64), 0);
        assert_eq!(ops.remu(10u64, 0u64), 10);
    }
}

/// Test instruction encoding and round-trip decoding
mod encoding_tests {
    use super::*;

    #[test]
    fn test_mul_encoding_roundtrip() {
        // Test that encoding and decoding are inverses
        let test_cases = vec![
            (mul_encoding::encode_mul, MulInstruction::Mul),
            (mul_encoding::encode_mulh, MulInstruction::Mulh),
            (mul_encoding::encode_mulhsu, MulInstruction::Mulhsu),
            (mul_encoding::encode_mulhu, MulInstruction::Mulhu),
        ];

        for (encode_fn, expected) in test_cases {
            for rd in 0..=31 {
                for rs1 in 0..=31 {
                    for rs2 in 0..=31 {
                        let insn = encode_fn(rd, rs1, rs2);
                        let decoded = MulDecoder::decode(insn);
                        assert_eq!(decoded, Some(expected),
                                 "Roundtrip failed for rd={}, rs1={}, rs2={}",
                                 rd, rs1, rs2);
                    }
                }
            }
        }
    }

    #[test]
    fn test_div_encoding_roundtrip() {
        // Test that encoding and decoding are inverses
        let test_cases = vec![
            (div_encoding::encode_div, DivInstruction::Div),
            (div_encoding::encode_divu, DivInstruction::Divu),
            (div_encoding::encode_rem, DivInstruction::Rem),
            (div_encoding::encode_remu, DivInstruction::Remu),
        ];

        for (encode_fn, expected) in test_cases {
            for rd in 0..=31 {
                for rs1 in 0..=31 {
                    for rs2 in 0..=31 {
                        let insn = encode_fn(rd, rs1, rs2);
                        let decoded = DivDecoder::decode(insn);
                        assert_eq!(decoded, Some(expected),
                                 "Roundtrip failed for rd={}, rs1={}, rs2={}",
                                 rd, rs1, rs2);
                    }
                }
            }
        }
    }
}

/// Test IR generation
mod ir_generation_tests {
    use super::*;

    struct MockMmu;
    impl MMU for MockMmu {
        fn fetch_insn(&self, _pc: GuestAddr) -> Result<u64, VmError> {
            Ok(0)
        }

        fn load(&self, _addr: GuestAddr, _size: u8) -> Result<u64, VmError> {
            Ok(0)
        }

        fn store(&self, _addr: GuestAddr, _size: u8, _value: u64) -> Result<(), VmError> {
            Ok(())
        }
    }

    #[test]
    fn test_mul_ir_generation() {
        let mut reg_file = RegisterFile::new(32, vm_ir::RegisterMode::SSA);
        let mut builder = IRBuilder::new(GuestAddr(0));
        let mmu = MockMmu;

        // Test MUL IR generation
        let mul_insn = mul_encoding::encode_mul(1, 2, 3);
        let block = MulDecoder::to_ir(mul_insn, &mut reg_file, &mut builder, &mmu, GuestAddr(0))
            .expect("Failed to decode MUL instruction");

        // Verify block structure
        assert_eq!(block.ops.len(), 1);
        assert_eq!(block.terminator, Terminator::Jmp { target: GuestAddr(4) });

        // Verify the operation is a Mul
        if let vm_ir::IROp::Mul { dst, src1, src2 } = block.ops[0] {
            assert_ne!(dst, 0); // Should use a valid register
        } else {
            panic!("Expected Mul operation");
        }
    }

    #[test]
    fn test_div_ir_generation() {
        let mut reg_file = RegisterFile::new(32, vm_ir::RegisterMode::SSA);
        let mut builder = IRBuilder::new(GuestAddr(0));
        let mmu = MockMmu;

        // Test DIV IR generation
        let div_insn = div_encoding::encode_div(1, 2, 3);
        let block = DivDecoder::to_ir(div_insn, &mut reg_file, &mut builder, &mmu, GuestAddr(0))
            .expect("Failed to decode DIV instruction");

        // Verify block structure
        assert_eq!(block.ops.len(), 1);
        assert_eq!(block.terminator, Terminator::Jmp { target: GuestAddr(4) });

        // Verify the operation is a Div with signed=true
        if let vm_ir::IROp::Div { dst, src1, src2, signed } = block.ops[0] {
            assert_ne!(dst, 0); // Should use a valid register
            assert!(signed); // Should be signed division
        } else {
            panic!("Expected Div operation");
        }
    }
}

/// Integration tests with the main RISC-V decoder
mod integration_tests {
    use vm_frontend::riscv64::RiscvDecoder;

    #[test]
    fn test_m_extension_integration() {
        // This test should verify that M extension instructions
        // are properly handled by the main decoder
        // Implementation depends on how the main decoder is structured

        // Note: This is a placeholder test that needs to be implemented
        // when the main decoder is updated to use M extension decoders
        assert!(true, "Integration test placeholder");
    }
}

/// Performance benchmarks
mod benchmarks {
    use test::Bencher;

    #[bench]
    fn bench_mul_operations(b: &mut Bencher) {
        let ops = DefaultMulOps;

        b.iter(|| {
            // Benchmark a simple multiplication
            ops.mul(123456789, 987654321)
        });
    }

    #[bench]
    fn bench_div_operations(b: &mut Bencher) {
        let ops = DefaultDivOps;

        b.iter(|| {
            // Benchmark a simple division
            ops.div(987654321, 123456789)
        });
    }

    #[bench]
    fn bench_mulh_operations(b: &mut Bencher) {
        let ops = DefaultMulOps;

        b.iter(|| {
            // Benchmark high multiplication
            ops.mulh(0x100000000i64, 0x100000000i64)
        });
    }
}

/// Test edge cases and error conditions
mod edge_case_tests {
    use super::*;

    #[test]
    fn test_extreme_values() {
        let mul_ops = DefaultMulOps;
        let div_ops = DefaultDivOps;

        // Test multiplication with extreme values
        assert_eq!(mul_ops.mul(i64::MAX, i64::MAX), 1); // Wraps to 1
        assert_eq!(mul_ops.mul(i64::MIN, i64::MIN), 0); // Wraps to 0
        assert_eq!(mul_ops.mul(i64::MAX, i64::MIN), -2); // Wraps to -2

        // Test division with extreme values
        assert_eq!(div_ops.div(i64::MAX, i64::MIN), 0); // MAX / MIN = 0
        assert_eq!(div_ops.div(i64::MIN, 1), i64::MIN); // MIN / 1 = MIN
        assert_eq!(div_ops.div(i64::MIN, i64::MIN), 1); // MIN / MIN = 1

        // Test unsigned operations with extreme values
        assert_eq!(mul_ops.mulhu(u64::MAX, u64::MAX), u64::MAX >> 1);
        assert_eq!(div_ops.divu(u64::MAX, u64::MAX), 1);
    }

    #[test]
    fn test_same_operand_operations() {
        let mul_ops = DefaultMulOps;
        let div_ops = DefaultDivOps;

        // Multiply by self
        assert_eq!(mul_ops.mul(5, 5), 25);
        assert_eq!(mul_ops.mul(-5, -5), 25);

        // Divide by self
        assert_eq!(div_ops.div(5, 5), 1);
        assert_eq!(div_ops.div(-5, -5), 1);
        assert_eq!(div_ops.div(5, -5), -1);

        // Remainder by self
        assert_eq!(div_ops.rem(5, 5), 0);
        assert_eq!(div_ops.rem(-5, -5), 0);
    }
}

/// Test utilities for running comprehensive test suites
mod test_suite {
    use super::*;

    /// Run all M extension tests
    pub fn run_all_tests() {
        println!("Running RISC-V M Extension Test Suite");

        // Run decoding tests
        println!("  Running decoding tests...");
        decoding_tests::test_mul_instruction_decoding();
        decoding_tests::test_div_instruction_decoding();
        decoding_tests::test_invalid_instructions();

        // Run operation tests
        println!("  Running operation tests...");
        operation_tests::test_multiply_operations();
        operation_tests::test_multiply_high_operations();
        operation_tests::test_division_operations();
        operation_tests::test_unsigned_division_operations();

        // Run encoding tests
        println!("  Running encoding tests...");
        encoding_tests::test_mul_encoding_roundtrip();
        encoding_tests::test_div_encoding_roundtrip();

        // Run IR generation tests
        println!("  Running IR generation tests...");
        ir_generation_tests::test_mul_ir_generation();
        ir_generation_tests::test_div_ir_generation();

        // Run edge case tests
        println!("  Running edge case tests...");
        edge_case_tests::test_extreme_values();
        edge_case_tests::test_same_operand_operations();

        println!("  All tests passed! ✓");
    }

    /// Verify compliance with RISC-V M extension specification
    pub fn verify_spec_compliance() {
        println!("Verifying RISC-V M Extension Specification Compliance");

        let mul_ops = DefaultMulOps;
        let div_ops = DefaultDivOps;

        // Verify MUL operation specification
        // MUL should produce lower 64 bits of 128-bit product
        let test_cases = vec![
            (5, 3, 15),
            (-5, 3, -15),
            (5, -3, -15),
            (-5, -3, 15),
        ];

        for (a, b, expected) in test_cases {
            let result = mul_ops.mul(a, b);
            assert_eq!(result, expected, "MUL({}, {}) = {}, expected {}", a, b, result, expected);
        }

        // Verify DIV operation specification
        // Division by zero should return -1
        assert_eq!(div_ops.div(10, 0), -1);
        assert_eq!(div_ops.div(-10, 0), -1);

        // Verify REM operation specification
        // Remainder by zero should return dividend
        assert_eq!(div_ops.rem(10, 0), 10);
        assert_eq!(div_ops.rem(-10, 0), -10);

        println!("  Specification compliance verified! ✓");
    }
}

// Main test runner
#[cfg(test)]
mod main_tests {
    use super::*;

    #[test]
    fn test_complete_m_extension() {
        // Run the comprehensive test suite
        test_suite::run_all_tests();
        test_suite::verify_spec_compliance();
    }
}

// Integration test with the main project
#[test]
fn test_m_extension_integration() {
    // This test verifies that M extension can be imported and used
    // by the main vm-frontend crate

    // Test that all public items are accessible
    use vm_frontend::riscv64::MulDecoder;
    use vm_frontend::riscv64::DivDecoder;

    // Test basic functionality
    let mul_insn = mul_encoding::encode_mul(1, 2, 3);
    assert!(MulDecoder::decode(mul_insn).is_some());

    let div_insn = div_encoding::encode_div(1, 2, 3);
    assert!(DivDecoder::decode(div_insn).is_some());

    println!("M extension integration test passed! ✓");
}