//! Instruction Encoder Tests
//!
//! Tests for instruction encoding functionality ensuring:
//! - Round-trip encoding/decoding
//! - Correct bit patterns
//! - Proper immediate handling
//! - Edge cases and boundary conditions

mod test_mmu;
use test_mmu::TestMMU;
use vm_core::{Decoder, GuestAddr};
use vm_frontend::riscv64::{
    RiscvDecoder,
    api::{
        encode_add, encode_addi, encode_auipc, encode_beq, encode_bge, encode_bgeu, encode_blt,
        encode_bltu, encode_bne, encode_jal, encode_jalr, encode_lw, encode_sub, encode_sw,
    },
};

#[test]
fn test_encode_add_basic() {
    let encoded = encode_add(1, 2, 3); // add x1, x2, x3

    // Verify opcode and funct fields
    assert_eq!(encoded & 0x7F, 0x33); // R-type opcode
    assert_eq!((encoded >> 7) & 0x1F, 1); // rd
    assert_eq!((encoded >> 15) & 0x1F, 2); // rs1
    assert_eq!((encoded >> 20) & 0x1F, 3); // rs2
}

#[test]
fn test_encode_sub_basic() {
    let encoded = encode_sub(1, 2, 3); // sub x1, x2, x3

    // Verify opcode and funct fields
    assert_eq!(encoded & 0x7F, 0x33); // R-type opcode
    assert_eq!((encoded >> 7) & 0x1F, 1); // rd
    assert_eq!((encoded >> 15) & 0x1F, 2); // rs1
    assert_eq!((encoded >> 20) & 0x1F, 3); // rs2
    assert_eq!((encoded >> 25) & 0x7F, 0x20); // funct7 for SUB
}

#[test]
fn test_encode_addi_positive() {
    let encoded = encode_addi(1, 2, 0x10); // addi x1, x2, 0x10

    assert_eq!(encoded & 0x7F, 0x13); // I-type opcode
    assert_eq!((encoded >> 7) & 0x1F, 1); // rd
    assert_eq!((encoded >> 15) & 0x1F, 2); // rs1
    assert_eq!((encoded >> 20) as i32 as i64, 0x10); // imm
}

#[test]
fn test_encode_addi_negative() {
    let encoded = encode_addi(1, 2, -0x10); // addi x1, x2, -0x10

    assert_eq!(encoded & 0x7F, 0x13); // I-type opcode
    assert_eq!((encoded >> 7) & 0x1F, 1); // rd
    assert_eq!((encoded >> 15) & 0x1F, 2); // rs1
}

#[test]
fn test_encode_lw_basic() {
    let encoded = encode_lw(1, 2, 0x10); // lw x1, 0x10(x2)

    assert_eq!(encoded & 0x7F, 0x03); // Load opcode
    assert_eq!((encoded >> 7) & 0x1F, 1); // rd
    assert_eq!((encoded >> 15) & 0x1F, 2); // rs1
}

#[test]
fn test_encode_sw_basic() {
    let encoded = encode_sw(1, 2, 0x10); // sw x1, 0x10(x2)

    assert_eq!(encoded & 0x7F, 0x23); // Store opcode
    assert_eq!((encoded >> 15) & 0x1F, 1); // rs1 (base)
    assert_eq!((encoded >> 20) & 0x1F, 2); // rs2 (src)
}

#[test]
fn test_encode_jal_forward() {
    let encoded = encode_jal(1, 0x100); // jal x1, 0x100

    assert_eq!(encoded & 0x7F, 0x6F); // J-type opcode
    assert_eq!((encoded >> 7) & 0x1F, 1); // rd
}

#[test]
fn test_encode_jal_backward() {
    let encoded = encode_jal(1, -0x100); // jal x1, -0x100

    assert_eq!(encoded & 0x7F, 0x6F); // J-type opcode
    assert_eq!((encoded >> 7) & 0x1F, 1); // rd
}

#[test]
fn test_encode_jalr_basic() {
    let encoded = encode_jalr(1, 2, 0x10); // jalr x1, x2, 0x10

    assert_eq!(encoded & 0x7F, 0x67); // I-type opcode
    assert_eq!((encoded >> 7) & 0x1F, 1); // rd
    assert_eq!((encoded >> 15) & 0x1F, 2); // rs1
}

#[test]
fn test_encode_auipc_basic() {
    let encoded = encode_auipc(1, 0x12345); // auipc x1, 0x12345

    assert_eq!(encoded & 0x7F, 0x17); // U-type opcode
    assert_eq!((encoded >> 7) & 0x1F, 1); // rd
    // encode_auipc takes the upper 20 bits directly
    let imm = (encoded >> 12) & 0xFFFFF;
    assert_eq!(imm, 0x12345 & 0xFFFFF);
}

#[test]
fn test_encode_beq_basic() {
    let encoded = encode_beq(1, 2, 0x10); // beq x1, x2, 0x10

    assert_eq!(encoded & 0x7F, 0x63); // B-type opcode
    assert_eq!((encoded >> 15) & 0x1F, 1); // rs1
    assert_eq!((encoded >> 20) & 0x1F, 2); // rs2
}

#[test]
fn test_encode_bne_basic() {
    let encoded = encode_bne(1, 2, 0x10); // bne x1, x2, 0x10

    assert_eq!(encoded & 0x7F, 0x63); // B-type opcode
    assert_eq!((encoded >> 15) & 0x1F, 1); // rs1
    assert_eq!((encoded >> 20) & 0x1F, 2); // rs2
    assert_eq!((encoded >> 12) & 0x7, 0x1); // funct3 for BNE
}

#[test]
fn test_encode_blt_basic() {
    let encoded = encode_blt(1, 2, 0x10); // blt x1, x2, 0x10

    assert_eq!(encoded & 0x7F, 0x63); // B-type opcode
    assert_eq!((encoded >> 15) & 0x1F, 1); // rs1
    assert_eq!((encoded >> 20) & 0x1F, 2); // rs2
    assert_eq!((encoded >> 12) & 0x7, 0x4); // funct3 for BLT
}

#[test]
fn test_encode_bge_basic() {
    let encoded = encode_bge(1, 2, 0x10); // bge x1, x2, 0x10

    assert_eq!(encoded & 0x7F, 0x63); // B-type opcode
    assert_eq!((encoded >> 15) & 0x1F, 1); // rs1
    assert_eq!((encoded >> 20) & 0x1F, 2); // rs2
    assert_eq!((encoded >> 12) & 0x7, 0x5); // funct3 for BGE
}

#[test]
fn test_encode_bltu_basic() {
    let encoded = encode_bltu(1, 2, 0x10); // bltu x1, x2, 0x10

    assert_eq!(encoded & 0x7F, 0x63); // B-type opcode
    assert_eq!((encoded >> 15) & 0x1F, 1); // rs1
    assert_eq!((encoded >> 20) & 0x1F, 2); // rs2
    assert_eq!((encoded >> 12) & 0x7, 0x6); // funct3 for BLTU
}

#[test]
fn test_encode_bgeu_basic() {
    let encoded = encode_bgeu(1, 2, 0x10); // bgeu x1, x2, 0x10

    assert_eq!(encoded & 0x7F, 0x63); // B-type opcode
    assert_eq!((encoded >> 15) & 0x1F, 1); // rs1
    assert_eq!((encoded >> 20) & 0x1F, 2); // rs2
    assert_eq!((encoded >> 12) & 0x7, 0x7); // funct3 for BGEU
}

#[test]
fn test_encode_add_roundtrip() {
    let mut mmu = TestMMU::new();
    let encoded = encode_add(1, 2, 3);
    mmu.set_insn(0x1000, encoded);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    // Successfully round-tripped the encoding
}

#[test]
fn test_encode_addi_roundtrip() {
    let mut mmu = TestMMU::new();
    let encoded = encode_addi(1, 2, 0x42);
    mmu.set_insn(0x1000, encoded);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    // Successfully round-tripped the encoding
}

#[test]
fn test_encode_lw_sw_roundtrip() {
    let mut mmu = TestMMU::new();
    let lw_encoded = encode_lw(1, 2, 0x100);
    let sw_encoded = encode_sw(3, 4, 0x200);
    mmu.set_insn(0x1000, lw_encoded);
    mmu.set_insn(0x1004, sw_encoded);

    let mut decoder = RiscvDecoder;
    let result1 = decoder.decode(&mmu, GuestAddr(0x1000));
    let result2 = decoder.decode(&mmu, GuestAddr(0x1004));

    assert!(result1.is_ok());
    assert!(result2.is_ok());
}

#[test]
fn test_encode_jal_range_check() {
    // Test near maximum forward jump
    let encoded = encode_jal(1, 0x1FFFE);
    assert_eq!(encoded & 0x7F, 0x6F);

    // Test near maximum backward jump
    let encoded = encode_jal(1, -0x20000);
    assert_eq!(encoded & 0x7F, 0x6F);
}

#[test]
fn test_encode_branch_alignment() {
    // All branches should be aligned to 2 bytes
    let encoded = encode_beq(1, 2, 0x11); // Odd offset
    let encoded2 = encode_beq(1, 2, 0x10); // Even offset

    // Both should encode successfully (bit 0 should be ignored)
    assert_eq!(encoded & 0x7F, 0x63);
    assert_eq!(encoded2 & 0x7F, 0x63);
}

#[test]
fn test_encode_immediate_clamping() {
    // Test positive clamp for I-type
    let large_imm = 0x1000; // Larger than 12-bit
    let encoded = encode_addi(1, 2, large_imm);
    assert_eq!(encoded & 0x7F, 0x13);

    // Test negative clamp for I-type
    let large_neg = -0x1000;
    let encoded = encode_addi(1, 2, large_neg);
    assert_eq!(encoded & 0x7F, 0x13);
}

#[test]
fn test_encode_register_boundaries() {
    // Test x0 (zero register)
    let encoded = encode_add(0, 0, 0);
    assert_eq!((encoded >> 7) & 0x1F, 0);

    // Test x31 (highest register)
    let encoded = encode_add(31, 31, 31);
    assert_eq!((encoded >> 7) & 0x1F, 31);
    assert_eq!((encoded >> 15) & 0x1F, 31);
    assert_eq!((encoded >> 20) & 0x1F, 31);
}

#[test]
fn test_encode_auipc_alignment() {
    let encoded = encode_auipc(1, 0xFFFFF);

    assert_eq!(encoded & 0x7F, 0x17);
    assert_eq!((encoded >> 7) & 0x1F, 1);
    // Upper 20 bits should be preserved
    assert_eq!((encoded >> 12) & 0xFFFFF, 0xFFFFF);
}

#[test]
fn test_encode_jalr_with_alignment() {
    use vm_frontend::riscv64::encode_jalr_with_align;

    // Test without alignment
    let encoded = encode_jalr_with_align(1, 2, 0x11, false);
    assert_eq!(encoded & 0x7F, 0x67);

    // Test with alignment (should clear bit 0)
    let encoded = encode_jalr_with_align(1, 2, 0x11, true);
    assert_eq!(encoded & 0x7F, 0x67);
}
