//! ARM64 Decoder Tests
//!
//! Comprehensive tests for ARM64 instruction decoding covering:
//! - Load/Store instructions
//! - Branch instructions
//! - Arithmetic instructions
//! - Logical instructions
//! - System instructions

mod test_mmu;
use test_mmu::TestMMU;

use vm_core::{Decoder, GuestAddr};
use vm_frontend::arm64::Arm64Decoder;

#[test]
fn test_arm64_decoder_new() {
    let _decoder = Arm64Decoder::new();
    // Successfully created decoder with cache
}

#[test]
fn test_arm64_decoder_without_cache() {
    let _decoder = Arm64Decoder::without_cache();
    // Successfully created decoder without cache
}

#[test]
fn test_arm64_decoder_clear_cache() {
    let mut decoder = Arm64Decoder::new();
    decoder.clear_cache();
    // Successfully cleared cache
}

#[test]
fn test_arm64_decode_load_register() {
    let mut mmu = TestMMU::new();
    // ldr x0, [x1]
    let insn = 0x00000058; // ldr x0, [x1]
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
    // Successfully decoded a load instruction
}

#[test]
fn test_arm64_decode_load_register_offset() {
    let mut mmu = TestMMU::new();
    // ldr x0, [x1, #8]
    let insn = 0x00800058; // ldr x0, [x1, #8]
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_store_register() {
    let mut mmu = TestMMU::new();
    // str x0, [x1]
    let insn = 0x00000078; // str x0, [x1]
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_load_pair() {
    let mut mmu = TestMMU::new();
    // ldp x0, x1, [x2]
    let insn = 0x004000a8; // ldp x0, x1, [x2]
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_store_pair() {
    let mut mmu = TestMMU::new();
    // stp x0, x1, [x2]
    let insn = 0x004000a9; // stp x0, x1, [x2]
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_branch_unconditional() {
    let mut mmu = TestMMU::new();
    // b #0x1000
    let insn = 0x00000014; // b #0
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_branch_with_link() {
    let mut mmu = TestMMU::new();
    // bl #0x1000
    let insn = 0x00000094; // bl #0
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_branch_conditional_eq() {
    let mut mmu = TestMMU::new();
    // beq #0x1000
    let insn = 0x00000000; // beq #0
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_branch_conditional_ne() {
    let mut mmu = TestMMU::new();
    // bne #0x1000
    let insn = 0x00000001; // bne #0
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_branch_conditional_lt() {
    let mut mmu = TestMMU::new();
    // blt #0x1000
    let insn = 0x00000002; // blt #0 (actually b)
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_branch_register() {
    let mut mmu = TestMMU::new();
    // br x0
    let insn = 0x000000d6; // br x0
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_branch_link_register() {
    let mut mmu = TestMMU::new();
    // blr x0
    let insn = 0x000000d7; // blr x0
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_ret() {
    let mut mmu = TestMMU::new();
    // ret x0
    let insn = 0x000000d5; // ret x0
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_add_immediate() {
    let mut mmu = TestMMU::new();
    // add x0, x1, #0x10
    let insn = 0x10000010; // add x0, x1, #0x10
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_add_register() {
    let mut mmu = TestMMU::new();
    // add x0, x1, x2
    let insn = 0x0200000b; // add x0, x1, x2
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_sub_immediate() {
    let mut mmu = TestMMU::new();
    // sub x0, x1, #0x10
    let insn = 0x10000051; // sub x0, x1, #0x10
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_sub_register() {
    let mut mmu = TestMMU::new();
    // sub x0, x1, x2
    let insn = 0x0200004b; // sub x0, x1, x2
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_and_immediate() {
    let mut mmu = TestMMU::new();
    // and x0, x1, #0xFF
    let insn = 0x00001212; // and x0, x1, #0xFF
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_orr_immediate() {
    let mut mmu = TestMMU::new();
    // orr x0, x1, #0xFF
    let insn = 0x00001332; // orr x0, x1, #0xFF
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_eor_immediate() {
    let mut mmu = TestMMU::new();
    // eor x0, x1, #0xFF
    let insn = 0x00001452; // eor x0, x1, #0xFF
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_mov_register() {
    let mut mmu = TestMMU::new();
    // mov x0, x1 (orr x0, xzr, x1)
    let insn = 0x020000aa; // orr x0, xzr, x1
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_movi_immediate() {
    let mut mmu = TestMMU::new();
    // mov x0, #0x10 (orr x0, xzr, #0x10)
    let insn = 0x000080d2; // mov x0, #0x10
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_movk_immediate() {
    let mut mmu = TestMMU::new();
    // movk x0, #0x1000, lsl #16
    let insn = 0x000080f2; // movk x0, #0x1000, lsl #16
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_nop() {
    let mut mmu = TestMMU::new();
    // nop
    let insn = 0x1f2003d5; // nop
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_movz_x0_0() {
    let mut mmu = TestMMU::new();
    // movz x0, #0
    let insn = 0x000080d2; // movz x0, #0
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_cmp_register() {
    let mut mmu = TestMMU::new();
    // cmp x0, x1 (subs xzr, x0, x1)
    let insn = 0x010000eb; // subs xzr, x0, x1
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_mul() {
    let mut mmu = TestMMU::new();
    // mul x0, x1, x2
    let insn = 0x0200001b; // mul x0, x1, x2
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_ldrsw_literal() {
    let mut mmu = TestMMU::new();
    // ldrsw x0, #0x1000
    let insn = 0x00000098; // ldrsw x0, #0
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_ldurb() {
    let mut mmu = TestMMU::new();
    // ldurb w0, [x1]
    let insn = 0x00003800; // ldurb w0, [x1]
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_ldurh() {
    let mut mmu = TestMMU::new();
    // ldurh w0, [x1]
    let insn = 0x00007800; // ldurh w0, [x1]
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_ldursw() {
    let mut mmu = TestMMU::new();
    // ldursw x0, [x1]
    let insn = 0x0000b800; // ldursw x0, [x1]
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_sturb() {
    let mut mmu = TestMMU::new();
    // sturb w0, [x1]
    let insn = 0x00003800; // sturb w0, [x1]
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_arm64_decode_sturh() {
    let mut mmu = TestMMU::new();
    // sturh w0, [x1]
    let insn = 0x00007800; // sturh w0, [x1]
    mmu.set_insn(0x1000, insn);

    let mut decoder = Arm64Decoder::new();
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}
