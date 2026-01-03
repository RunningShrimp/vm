//! RISC-V Decoder Tests
//!
//! Comprehensive tests for RISC-V instruction decoding covering:
//! - R-format instructions (register operations)
//! - I-format instructions (immediate operations)
//! - S-format instructions (store operations)
//! - B-format instructions (branch operations)
//! - U-format instructions (upper immediate operations)
//! - J-format instructions (jump operations)
//! - Compressed instructions (RV64C)

mod test_mmu;
use test_mmu::TestMMU;
use vm_core::{Decoder, GuestAddr};
use vm_frontend::riscv64::{RiscvDecoder, RiscvInstruction};

#[test]
fn test_riscv_decode_lui() {
    let mut mmu = TestMMU::new();
    // lui x1, 0x12345
    let insn = 0x123450b7;
    mmu.set_insn(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode_insn(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let insn_decoded = result.unwrap();
    assert_eq!(insn_decoded.mnemonic, "lui");
    assert_eq!(insn_decoded.next_pc, GuestAddr(0x1004));
    assert!(!insn_decoded.has_memory_op);
    assert!(!insn_decoded.is_branch);
}

#[test]
fn test_riscv_decode_auipc() {
    let mut mmu = TestMMU::new();
    // auipc x1, 0x12345
    let insn = 0x12345097;
    mmu.set_insn(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode_insn(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let insn_decoded = result.unwrap();
    assert_eq!(insn_decoded.mnemonic, "auipc");
    assert_eq!(insn_decoded.next_pc, GuestAddr(0x1004));
}

#[test]
fn test_riscv_decode_jal() {
    let mut mmu = TestMMU::new();
    // jal x1, 0 (jal x1, 0)
    let insn = 0x0000006f; // Proper jal encoding
    mmu.set_insn(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode_insn(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let insn_decoded = result.unwrap();
    assert_eq!(insn_decoded.mnemonic, "jal");
    assert!(insn_decoded.is_branch);
}

#[test]
fn test_riscv_decode_jalr() {
    let mut mmu = TestMMU::new();
    // jalr x1, x2, 0
    let insn = 0x67;
    mmu.set_insn(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode_insn(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let insn_decoded = result.unwrap();
    assert_eq!(insn_decoded.mnemonic, "jalr");
    assert!(insn_decoded.is_branch);
}

#[test]
fn test_riscv_decode_branch_beq() {
    let mut mmu = TestMMU::new();
    // beq x1, x2, 0x10
    let insn = 0x63;
    mmu.set_insn(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode_insn(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let insn_decoded = result.unwrap();
    assert_eq!(insn_decoded.mnemonic, "branch");
    assert!(insn_decoded.is_branch);
}

#[test]
fn test_riscv_decode_load_lb() {
    let mut mmu = TestMMU::new();
    // lb x1, 0(x2)
    let insn = 0x03;
    mmu.set_insn(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode_insn(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let insn_decoded = result.unwrap();
    assert_eq!(insn_decoded.mnemonic, "load");
    assert!(insn_decoded.has_memory_op);
}

#[test]
fn test_riscv_decode_load_lw() {
    let mut mmu = TestMMU::new();
    // lw x1, 0(x2)
    let insn = 0x00012003; // lw x1, 0(x2)
    mmu.set_insn(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode_insn(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let insn_decoded = result.unwrap();
    assert_eq!(insn_decoded.mnemonic, "load");
    assert!(insn_decoded.has_memory_op);
    assert!(!insn_decoded.is_branch);
}

#[test]
fn test_riscv_decode_store_sw() {
    let mut mmu = TestMMU::new();
    // sw x1, 0(x2)
    let insn = 0x02312023; // sw x1, 0(x2)
    mmu.set_insn(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode_insn(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let insn_decoded = result.unwrap();
    assert_eq!(insn_decoded.mnemonic, "store");
    assert!(insn_decoded.has_memory_op);
}

#[test]
fn test_riscv_decode_store_sb() {
    let mut mmu = TestMMU::new();
    // sb x1, 0(x2)
    let insn = 0x00312023; // sb x1, 0(x2)
    mmu.set_insn(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode_insn(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let insn_decoded = result.unwrap();
    assert_eq!(insn_decoded.mnemonic, "store");
    assert!(insn_decoded.has_memory_op);
}

#[test]
fn test_riscv_decode_arithmetic_add() {
    let mut mmu = TestMMU::new();
    // add x1, x2, x3
    let insn = 0x003100b3; // add x1, x2, x3
    mmu.set_insn(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode_insn(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let insn_decoded = result.unwrap();
    assert_eq!(insn_decoded.mnemonic, "arith");
}

#[test]
fn test_riscv_decode_arithmetic_sub() {
    let mut mmu = TestMMU::new();
    // sub x1, x2, x3
    let insn = 0x403100b3; // sub x1, x2, x3
    mmu.set_insn(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode_insn(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let insn_decoded = result.unwrap();
    assert_eq!(insn_decoded.mnemonic, "arith");
}

#[test]
fn test_riscv_decode_immediate_addi() {
    let mut mmu = TestMMU::new();
    // addi x1, x2, 0x10
    let insn = 0x01010093; // addi x1, x2, 0x10
    mmu.set_insn(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode_insn(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let insn_decoded = result.unwrap();
    assert_eq!(insn_decoded.mnemonic, "addi");
}

#[test]
fn test_riscv_decode_fence() {
    let mut mmu = TestMMU::new();
    // fence
    let insn = 0x0ff0000f;
    mmu.set_insn(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode_insn(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let insn_decoded = result.unwrap();
    assert_eq!(insn_decoded.mnemonic, "fence");
    assert!(!insn_decoded.is_branch);
}

#[test]
fn test_riscv_decode_system_ecall() {
    let mut mmu = TestMMU::new();
    // ecall
    let insn = 0x00000073;
    mmu.set_insn(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode_insn(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let insn_decoded = result.unwrap();
    assert_eq!(insn_decoded.mnemonic, "system");
}

#[test]
fn test_riscv_decode_system_ebreak() {
    let mut mmu = TestMMU::new();
    // ebreak
    let insn = 0x00100073;
    mmu.set_insn(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode_insn(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let insn_decoded = result.unwrap();
    assert_eq!(insn_decoded.mnemonic, "system");
}

#[test]
fn test_riscv_decode_compressed_addi4spn() {
    let mut mmu = TestMMU::new();
    // c.addi4spn x8, 0x10
    let insn = 0x1040; // c.addi4spn
    mmu.set_insn16(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
    // Successfully decoded a compressed instruction
}

#[test]
fn test_riscv_decode_compressed_lw() {
    let mut mmu = TestMMU::new();
    // c.lw x8, 0(x9)
    let insn = 0x4080; // c.lw
    mmu.set_insn16(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_riscv_decode_compressed_sw() {
    let mut mmu = TestMMU::new();
    // c.sw x8, 0(x9)
    let insn = 0xc080; // c.sw
    mmu.set_insn16(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_riscv_decode_compressed_addi() {
    let mut mmu = TestMMU::new();
    // c.addi x1, -16
    let insn = 0x7481; // c.addi
    mmu.set_insn16(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_riscv_decode_compressed_nop() {
    let mut mmu = TestMMU::new();
    // c.nop
    let insn = 0x0001;
    mmu.set_insn16(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_riscv_decode_compressed_li() {
    let mut mmu = TestMMU::new();
    // c.li x1, 16
    let insn = 0x4101; // c.li
    mmu.set_insn16(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_riscv_decode_compressed_lui() {
    let mut mmu = TestMMU::new();
    // c.lui x1, 0x10
    let insn = 0x6101; // c.lui
    mmu.set_insn16(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_riscv_decode_compressed_srli() {
    let mut mmu = TestMMU::new();
    // c.srli x8, 8
    let insn = 0x8241; // c.srli
    mmu.set_insn16(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_riscv_decode_compressed_srai() {
    let mut mmu = TestMMU::new();
    // c.srai x8, 8
    let insn = 0x8641; // c.srai
    mmu.set_insn16(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_riscv_decode_compressed_andi() {
    let mut mmu = TestMMU::new();
    // c.andi x8, 16
    let insn = 0x8b41; // c.andi
    mmu.set_insn16(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_riscv_decode_compressed_sub() {
    let mut mmu = TestMMU::new();
    // c.sub x8, x9
    let insn = 0x8c41; // c.sub
    mmu.set_insn16(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_riscv_decode_compressed_xor() {
    let mut mmu = TestMMU::new();
    // c.xor x8, x9
    let insn = 0x8c81; // c.xor
    mmu.set_insn16(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_riscv_decode_compressed_or() {
    let mut mmu = TestMMU::new();
    // c.or x8, x9
    let insn = 0x8cc1; // c.or
    mmu.set_insn16(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_riscv_decode_compressed_and() {
    let mut mmu = TestMMU::new();
    // c.and x8, x9
    let insn = 0x8d01; // c.and
    mmu.set_insn16(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_riscv_decode_compressed_j() {
    let mut mmu = TestMMU::new();
    // c.j 0
    let insn = 0xa001; // c.j
    mmu.set_insn16(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_riscv_decode_compressed_beqz() {
    let mut mmu = TestMMU::new();
    // c.beqz x8, 0
    let insn = 0xe101; // c.beqz
    mmu.set_insn16(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_riscv_decode_compressed_bnez() {
    let mut mmu = TestMMU::new();
    // c.bnez x8, 0
    let insn = 0xf101; // c.bnez
    mmu.set_insn16(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_riscv_decode_compressed_lwsp() {
    let mut mmu = TestMMU::new();
    // c.lwsp x1, 0(sp)
    let insn = 0x4102; // c.lwsp
    mmu.set_insn16(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_riscv_decode_compressed_swsp() {
    let mut mmu = TestMMU::new();
    // c.swsp x8, 0(sp)
    let insn = 0x0182; // c.swsp
    mmu.set_insn16(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_riscv_instruction_size() {
    let insn = RiscvInstruction {
        mnemonic: "test",
        next_pc: GuestAddr(0x1004),
        has_memory_op: false,
        is_branch: false,
    };

    assert_eq!(insn.size(), 4);
}

#[test]
fn test_riscv_instruction_mnemonic() {
    let insn = RiscvInstruction {
        mnemonic: "addi",
        next_pc: GuestAddr(0x1004),
        has_memory_op: false,
        is_branch: false,
    };

    assert_eq!(insn.mnemonic(), "addi");
}

#[test]
fn test_riscv_instruction_is_control_flow() {
    let branch_insn = RiscvInstruction {
        mnemonic: "jal",
        next_pc: GuestAddr(0x1004),
        has_memory_op: false,
        is_branch: true,
    };

    assert!(branch_insn.is_control_flow());

    let normal_insn = RiscvInstruction {
        mnemonic: "addi",
        next_pc: GuestAddr(0x1004),
        has_memory_op: false,
        is_branch: false,
    };

    assert!(!normal_insn.is_control_flow());
}

#[test]
fn test_riscv_instruction_is_memory_access() {
    let load_insn = RiscvInstruction {
        mnemonic: "lw",
        next_pc: GuestAddr(0x1004),
        has_memory_op: true,
        is_branch: false,
    };

    assert!(load_insn.is_memory_access());

    let normal_insn = RiscvInstruction {
        mnemonic: "addi",
        next_pc: GuestAddr(0x1004),
        has_memory_op: false,
        is_branch: false,
    };

    assert!(!normal_insn.is_memory_access());
}

#[test]
fn test_riscv_decode_unknown_opcode() {
    let mut mmu = TestMMU::new();
    // Invalid opcode
    let insn = 0xFFFFFFFF;
    mmu.set_insn(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode_insn(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let insn_decoded = result.unwrap();
    assert_eq!(insn_decoded.mnemonic, "unknown");
}

#[test]
fn test_riscv_decode_shift_slli() {
    let mut mmu = TestMMU::new();
    // slli x1, x2, 8
    let insn = 0x00810193; // slli x1, x2, 8
    mmu.set_insn(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode_insn(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let insn_decoded = result.unwrap();
    assert_eq!(insn_decoded.mnemonic, "addi"); // slli maps to addi
}

#[test]
fn test_riscv_decode_compare_slti() {
    let mut mmu = TestMMU::new();
    // slti x1, x2, 0x10
    let insn = 0x01012a13; // slti x1, x2, 0x10
    mmu.set_insn(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_riscv_decode_compare_sltiu() {
    let mut mmu = TestMMU::new();
    // sltiu x1, x2, 0x10
    let insn = 0x01013a13; // sltiu x1, x2, 0x10
    mmu.set_insn(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_riscv_decode_logical_and() {
    let mut mmu = TestMMU::new();
    // and x1, x2, x3
    let insn = 0x00311fb3; // and x1, x2, x3
    mmu.set_insn(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_riscv_decode_logical_or() {
    let mut mmu = TestMMU::new();
    // or x1, x2, x3
    let insn = 0x00311eb3; // or x1, x2, x3
    mmu.set_insn(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_riscv_decode_logical_xor() {
    let mut mmu = TestMMU::new();
    // xor x1, x2, x3
    let insn = 0x00311cb3; // xor x1, x2, x3
    mmu.set_insn(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}

#[test]
fn test_riscv_decode_shift_sll() {
    let mut mmu = TestMMU::new();
    // sll x1, x2, x3
    let insn = 0x00311bb3; // sll x1, x2, x3
    mmu.set_insn(0x1000, insn);

    let mut decoder = RiscvDecoder;
    let result = decoder.decode(&mut mmu, GuestAddr(0x1000));

    assert!(result.is_ok());
    let _block = result.unwrap();
}
