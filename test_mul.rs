//! Test RISC-V M extension multiplication operations
extern crate vm_frontend;

use vm_frontend::riscv64::mul::{
    MulDecoder, MulInstruction, DefaultMulOps, MulOperations,
    encoding,
};

fn main() {
    println!("Testing RISC-V M Extension - Multiplication Operations\n");

    // Test 1: Instruction decoding
    println!("=== Test 1: Instruction Decoding ===");
    test_decode();
    println!();

    // Test 2: MUL operation
    println!("=== Test 2: MUL Operation ===");
    test_mul();
    println!();

    // Test 3: MULH operation
    println!("=== Test 3: MULH Operation ===");
    test_mulh();
    println!();

    // Test 4: MULHSU operation
    println!("=== Test 4: MULHSU Operation ===");
    test_mulhsu();
    println!();

    // Test 5: MULHU operation
    println!("=== Test 5: MULHU Operation ===");
    test_mulhu();
    println!();

    // Test 6: MULW operation
    println!("=== Test 6: MULW Operation ===");
    test_mulw();
    println!();

    println!("=== All Tests Passed! ===");
}

fn test_decode() {
    // MUL x10, x11, x12
    let mul_insn = encoding::encode_mul(10, 11, 12);
    let decoded = MulDecoder::decode(mul_insn);
    assert_eq!(decoded, Some(MulInstruction::Mul));
    println!("✓ MUL instruction decoded correctly");

    // MULH x10, x11, x12
    let mulh_insn = encoding::encode_mulh(10, 11, 12);
    let decoded = MulDecoder::decode(mulh_insn);
    assert_eq!(decoded, Some(MulInstruction::Mulh));
    println!("✓ MULH instruction decoded correctly");

    // MULHSU x10, x11, x12
    let mulhsu_insn = encoding::encode_mulhsu(10, 11, 12);
    let decoded = MulDecoder::decode(mulhsu_insn);
    assert_eq!(decoded, Some(MulInstruction::Mulhsu));
    println!("✓ MULHSU instruction decoded correctly");

    // MULHU x10, x11, x12
    let mulhu_insn = encoding::encode_mulhu(10, 11, 12);
    let decoded = MulDecoder::decode(mulhu_insn);
    assert_eq!(decoded, Some(MulInstruction::Mulhu));
    println!("✓ MULHU instruction decoded correctly");

    // MULW x10, x11, x12
    let mulw_insn = encoding::encode_mulw(10, 11, 12);
    let decoded = MulDecoder::decode(mulw_insn);
    assert_eq!(decoded, Some(MulInstruction::Mulw));
    println!("✓ MULW instruction decoded correctly");
}

fn test_mul() {
    let ops = DefaultMulOps;

    // Basic positive multiplication
    assert_eq!(ops.mul(5, 3), 15);
    println!("✓ 5 * 3 = 15");

    assert_eq!(ops.mul(-5, 3), -15);
    println!("✓ -5 * 3 = -15");

    assert_eq!(ops.mul(5, -3), -15);
    println!("✓ 5 * -3 = -15");

    assert_eq!(ops.mul(-5, -3), 15);
    println!("✓ -5 * -3 = 15");

    // Zero cases
    assert_eq!(ops.mul(0, 42), 0);
    assert_eq!(ops.mul(42, 0), 0);
    println!("✓ Zero multiplication works");

    // Overflow behavior
    assert_eq!(ops.mul(i64::MAX, i64::MAX), 1);
    println!("✓ Overflow handling correct");
}

fn test_mulh() {
    let ops = DefaultMulOps;

    // Small values - high bits should be zero
    assert_eq!(ops.mulh(5, 3), 0);
    println!("✓ MULH(5, 3) = 0");

    assert_eq!(ops.mulh(-5, 3), -1);  // Sign extension
    println!("✓ MULH(-5, 3) = -1");

    // Large values that produce non-zero high bits
    assert_eq!(ops.mulh(0x100000000i64, 0x100000000i64), 1);
    println!("✓ MULH(2^32, 2^32) = 1");

    // Edge cases
    assert_eq!(ops.mulh(i64::MIN, i64::MIN), 0x4000000000000000);
    println!("✓ MULH(MIN, MIN) = 0x4000000000000000");
}

fn test_mulhsu() {
    let ops = DefaultMulOps;

    // Small values
    assert_eq!(ops.mulhsu(5, 3u64), 0);
    println!("✓ MULHSU(5, 3) = 0");

    assert_eq!(ops.mulhsu(-5, 3u64), -1);
    println!("✓ MULHSU(-5, 3) = -1");

    // Large values
    assert_eq!(ops.mulhsu(0x100000000i64, 0x100000000u64), 1);
    println!("✓ MULHSU(2^32, 2^32) = 1");
}

fn test_mulhu() {
    let ops = DefaultMulOps;

    // Small values - high bits should be zero
    assert_eq!(ops.mulhu(5u64, 3u64), 0);
    println!("✓ MULHU(5, 3) = 0");

    // Large values that produce non-zero high bits
    assert_eq!(ops.mulhu(0x100000000u64, 0x100000000u64), 1);
    println!("✓ MULHU(2^32, 2^32) = 1");

    // Edge case: MAX × MAX
    let result = ops.mulhu(u64::MAX, u64::MAX);
    assert_eq!(result, u64::MAX - 1);
    println!("✓ MULHU(MAX, MAX) = MAX - 1");
}

fn test_mulw() {
    let ops = DefaultMulOps;

    // Basic 32-bit multiplication
    assert_eq!(ops.mulw(5, 3), 15);
    println!("✓ MULW(5, 3) = 15");

    assert_eq!(ops.mulw(-5, 3), -15);
    println!("✓ MULW(-5, 3) = -15");

    // Test sign-extension
    // 0xFFFFFF00 × 2 = 0x1FFFFFE00, lower 32 bits = 0xFFFFFE00 = -512
    assert_eq!(ops.mulw(0xFFFFFF00i64, 2), -512);
    println!("✓ MULW(0xFFFFFF00, 2) = -512 (sign-extended)");

    // Test that only lower 32 bits are used
    let large_val = 0x123456789ABCDEF0i64;
    assert_eq!(ops.mulw(large_val, 1), (large_val as u32 as i32) as i64);
    println!("✓ MULW uses only lower 32 bits");

    // Test overflow in 32-bit space
    assert_eq!(ops.mulw(0x80000000i64, 2), 0);
    println!("✓ MULW overflow handling correct");
}
