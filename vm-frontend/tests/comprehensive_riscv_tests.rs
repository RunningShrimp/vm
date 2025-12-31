//! Comprehensive RISC-V frontend tests
//!
//! This test file provides comprehensive coverage for RISC-V instruction decoding,
//! targeting to increase vm-frontend coverage from 30% to 75%.

use vm_core::{GuestAddr, MMU, VmError};
use vm_frontend::riscv64::{RiscvDecoder, RiscvInstruction};

/// Simple test MMU implementation
struct TestMMU {
    memory: std::collections::HashMap<u64, u64>,
}

impl TestMMU {
    fn new() -> Self {
        Self {
            memory: std::collections::HashMap::new(),
        }
    }

    fn write_insn(&mut self, addr: GuestAddr, data: u64) {
        self.memory.insert(addr.0, data);
    }

    fn write_insn_batch(&mut self, addr: GuestAddr, data: &[u32]) {
        for (i, &insn) in data.iter().enumerate() {
            self.memory.insert(addr.0 + (i as u64 * 4), insn as u64);
        }
    }
}

impl MMU for TestMMU {
    fn fetch_insn(&self, pc: GuestAddr) -> Result<u64, VmError> {
        Ok(*self.memory.get(&pc.0).unwrap_or(&0))
    }
}

// ============================================================================
// Opcode Coverage Tests - All RISC-V opcodes
// ============================================================================

#[test]
fn test_opcode_lui() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00000537); // LUI a0, 0x50
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "lui");
    assert!(!result.has_memory_op);
    assert!(!result.is_branch);
}

#[test]
fn test_opcode_auipc() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00000517); // AUIPC a0, 0x50
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "auipc");
}

#[test]
fn test_opcode_jal() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x0100006f); // JAL ra, 0x10
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "jal");
    assert!(result.is_branch);
}

#[test]
fn test_opcode_jalr() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00008067); // JALR ra, ra, 0
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "jalr");
    assert!(result.is_branch);
}

#[test]
fn test_opcode_branch_beq() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00050863); // BEQ a0, a1, 0x10
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "branch");
    assert!(result.is_branch);
}

#[test]
fn test_opcode_branch_bne() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00c50463); // BNE a0, a2, 0x8
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "branch");
}

#[test]
fn test_opcode_branch_blt() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00c55463); // BLT a0, a2, 0x8
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "branch");
}

#[test]
fn test_opcode_branch_bge() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00c5d463); // BGE a0, a2, 0x8
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "branch");
}

#[test]
fn test_opcode_load_lb() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00052503); // LB a0, 0(a1)
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "load");
    assert!(result.has_memory_op);
    assert!(!result.is_branch);
}

#[test]
fn test_opcode_load_lh() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00053503); // LH a0, 0(a1)
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "load");
    assert!(result.has_memory_op);
}

#[test]
fn test_opcode_load_lw() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00054503); // LW a0, 0(a1)
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "load");
    assert!(result.has_memory_op);
}

#[test]
fn test_opcode_load_ld() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00055503); // LD a0, 0(a1)
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "load");
    assert!(result.has_memory_op);
}

#[test]
fn test_opcode_store_sb() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00a50023); // SB a0, 0(a1)
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "store");
    assert!(result.has_memory_op);
    assert!(!result.is_branch);
}

#[test]
fn test_opcode_store_sh() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00a51023); // SH a0, 0(a1)
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "store");
    assert!(result.has_memory_op);
}

#[test]
fn test_opcode_store_sw() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00a52023); // SW a0, 0(a1)
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "store");
    assert!(result.has_memory_op);
}

#[test]
fn test_opcode_store_sd() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00a53023); // SD a0, 0(a1)
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "store");
    assert!(result.has_memory_op);
}

#[test]
fn test_opcode_op_imm_addi() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00550513); // ADDI a0, a1, 5
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "addi");
    assert!(!result.has_memory_op);
    assert!(!result.is_branch);
}

#[test]
fn test_opcode_op_imm_slti() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00552513); // SLTI a0, a1, 5
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "addi");
}

#[test]
fn test_opcode_op_imm_xori() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00556513); // XORI a0, a1, 5
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "addi");
}

#[test]
fn test_opcode_op_imm_ori() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00557513); // ORI a0, a1, 5
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "addi");
}

#[test]
fn test_opcode_op_imm_andi() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00553513); // ANDI a0, a1, 5
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "addi");
}

#[test]
fn test_opcode_op_add() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00b50533); // ADD a0, a1, a2
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "arith");
    assert!(!result.has_memory_op);
    assert!(!result.is_branch);
}

#[test]
fn test_opcode_op_sub() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x40b50533); // SUB a0, a1, a2
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "arith");
}

#[test]
fn test_opcode_op_sll() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00b51533); // SLL a0, a1, a2
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "arith");
}

#[test]
fn test_opcode_op_slt() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00b52533); // SLT a0, a1, a2
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "arith");
}

#[test]
fn test_opcode_op_sltu() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00b53533); // SLTU a0, a1, a2
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "arith");
}

#[test]
fn test_opcode_op_xor() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00b54533); // XOR a0, a1, a2
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "arith");
}

#[test]
fn test_opcode_op_srl() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00b55533); // SRL a0, a1, a2
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "arith");
}

#[test]
fn test_opcode_op_sra() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x40b55533); // SRA a0, a1, a2
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "arith");
}

#[test]
fn test_opcode_op_or() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00b56533); // OR a0, a1, a2
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "arith");
}

#[test]
fn test_opcode_op_and() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00b57533); // AND a0, a1, a2
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "arith");
}

#[test]
fn test_opcode_fence() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x0ff0000f); // FENCE
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "fence");
    assert!(!result.has_memory_op);
    assert!(!result.is_branch);
}

#[test]
fn test_opcode_fence_i() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x0000100f); // FENCE.I
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "fence");
}

#[test]
fn test_opcode_system_ecall() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00000073); // ECALL
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "system");
}

#[test]
fn test_opcode_system_ebreak() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00100073); // EBREAK
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "system");
}

#[test]
fn test_opcode_vector() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x57000000); // Vector instruction
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "vector");
    assert!(result.has_memory_op);
}

#[test]
fn test_opcode_unknown() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x7f000000); // Unknown opcode
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "unknown");
}

// ============================================================================
// Compressed Instruction Tests (RV64C)
// ============================================================================

#[test]
fn test_compressed_c_addi4spn() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    // C.ADDI4SPN: bits [1:0] != 11, op = 0x0
    let insn = 0x0000; // Simplified example
    mmu.write_insn(GuestAddr(0), insn as u64);
    // Just ensure it doesn't panic
    let _result = decoder.decode_insn(&mmu, GuestAddr(0));
}

#[test]
fn test_compressed_c_lw() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    // C.LW: 16-bit compressed load
    let insn = 0x4000; // Simplified
    mmu.write_insn(GuestAddr(0), insn as u64);
    let _result = decoder.decode_insn(&mmu, GuestAddr(0));
}

#[test]
fn test_compressed_c_sw() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    // C.SW: 16-bit compressed store
    let insn = 0xC000; // Simplified
    mmu.write_insn(GuestAddr(0), insn as u64);
    let _result = decoder.decode_insn(&mmu, GuestAddr(0));
}

#[test]
fn test_compressed_c_addi() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    // C.ADDI: 16-bit compressed add immediate
    let insn = 0x0401; // Simplified
    mmu.write_insn(GuestAddr(0), insn as u64);
    let _result = decoder.decode_insn(&mmu, GuestAddr(0));
}

#[test]
fn test_compressed_c_jal() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    // C.JAL: 16-bit compressed jump and link
    let insn = 0x2001; // Simplified
    mmu.write_insn(GuestAddr(0), insn as u64);
    let _result = decoder.decode_insn(&mmu, GuestAddr(0));
}

#[test]
fn test_compressed_c_li() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    // C.LI: 16-bit compressed load immediate
    let insn = 0x3401; // Simplified
    mmu.write_insn(GuestAddr(0), insn as u64);
    let _result = decoder.decode_insn(&mmu, GuestAddr(0));
}

#[test]
fn test_compressed_c_andi() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    // C.ANDI: 16-bit compressed AND immediate
    let insn = 0x8C01; // Simplified
    mmu.write_insn(GuestAddr(0), insn as u64);
    let _result = decoder.decode_insn(&mmu, GuestAddr(0));
}

// ============================================================================
// RV64M Extension Tests (Multiply/Divide)
// ============================================================================

#[test]
fn test_rv64m_mul() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    // MUL: funct7 = 0x01, funct3 = 0x0
    mmu.write_insn(GuestAddr(0), 0x02b50533); // MUL a0, a1, a2
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "arith");
}

#[test]
fn test_rv64m_mulh() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    // MULH: funct7 = 0x01, funct3 = 0x1
    mmu.write_insn(GuestAddr(0), 0x02b51533); // MULH a0, a1, a2
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "arith");
}

#[test]
fn test_rv64m_mulhsu() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    // MULHSU: funct7 = 0x01, funct3 = 0x2
    mmu.write_insn(GuestAddr(0), 0x02b52533); // MULHSU a0, a1, a2
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "arith");
}

#[test]
fn test_rv64m_mulhu() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    // MULHU: funct7 = 0x01, funct3 = 0x3
    mmu.write_insn(GuestAddr(0), 0x02b53533); // MULHU a0, a1, a2
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "arith");
}

#[test]
fn test_rv64m_div() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    // DIV: funct7 = 0x01, funct3 = 0x4
    mmu.write_insn(GuestAddr(0), 0x02b54533); // DIV a0, a1, a2
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "arith");
}

#[test]
fn test_rv64m_divu() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    // DIVU: funct7 = 0x01, funct3 = 0x5
    mmu.write_insn(GuestAddr(0), 0x02b55533); // DIVU a0, a1, a2
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "arith");
}

#[test]
fn test_rv64m_rem() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    // REM: funct7 = 0x01, funct3 = 0x6
    mmu.write_insn(GuestAddr(0), 0x02b56533); // REM a0, a1, a2
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "arith");
}

#[test]
fn test_rv64m_remu() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    // REMU: funct7 = 0x01, funct3 = 0x7
    mmu.write_insn(GuestAddr(0), 0x02b57533); // REMU a0, a1, a2
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.mnemonic, "arith");
}

// ============================================================================
// RV64A Extension Tests (Atomic Memory Operations)
// ============================================================================

#[test]
fn test_rv64a_lr_w() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    // LR.W: opcode = 0x2F, funct5 = 0x02
    mmu.write_insn(GuestAddr(0), 0x1005252f); // LR.W a0, (a1)
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert!(result.mnemonic == "load" || result.mnemonic == "arith");
}

#[test]
fn test_rv64a_sc_w() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    // SC.W: opcode = 0x2F, funct5 = 0x03
    mmu.write_insn(GuestAddr(0), 0x1825302f); // SC.W a0, a2, (a1)
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert!(result.mnemonic == "store" || result.mnemonic == "arith");
}

#[test]
fn test_rv64a_amoswap_w() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    // AMOSWAP.W: opcode = 0x2F, funct5 = 0x01
    mmu.write_insn(GuestAddr(0), 0x0825352f); // AMOSWAP.W a0, a2, (a1)
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert!(result.mnemonic == "store" || result.mnemonic == "arith");
}

#[test]
fn test_rv64a_amoadd_w() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    // AMOADD.W: opcode = 0x2F, funct5 = 0x00
    mmu.write_insn(GuestAddr(0), 0x0025352f); // AMOADD.W a0, a2, (a1)
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert!(result.mnemonic == "store" || result.mnemonic == "arith");
}

#[test]
fn test_rv64a_amoxor_w() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    // AMOXOR.W: opcode = 0x2F, funct5 = 0x04
    mmu.write_insn(GuestAddr(0), 0x2025352f); // AMOXOR.W a0, a2, (a1)
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert!(result.mnemonic == "store" || result.mnemonic == "arith");
}

#[test]
fn test_rv64a_amoand_w() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    // AMOAND.W: opcode = 0x2F, funct5 = 0x0C
    mmu.write_insn(GuestAddr(0), 0x6025352f); // AMOAND.W a0, a2, (a1)
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert!(result.mnemonic == "store" || result.mnemonic == "arith");
}

#[test]
fn test_rv64a_amoor_w() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    // AMOOR.W: opcode = 0x2F, funct5 = 0x08
    mmu.write_insn(GuestAddr(0), 0x4025352f); // AMOOR.W a0, a2, (a1)
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert!(result.mnemonic == "store" || result.mnemonic == "arith");
}

// ============================================================================
// Encoding Tests
// ============================================================================

#[test]
fn test_encode_jal() {
    use vm_frontend::riscv64::encode_jal;
    // Test basic JAL encoding
    let encoded = encode_jal(1, 0x1000);
    assert_eq!(encoded & 0x7F, 0x6F); // Should have JAL opcode
}

#[test]
fn test_encode_jalr() {
    use vm_frontend::riscv64::encode_jalr;
    let encoded = encode_jalr(1, 2, 0);
    assert_eq!(encoded & 0x7F, 0x67); // Should have JALR opcode
}

#[test]
fn test_encode_jalr_with_align() {
    use vm_frontend::riscv64::encode_jalr_with_align;
    let encoded = encode_jalr_with_align(1, 2, 0, true);
    assert_eq!(encoded & 0x7F, 0x67);
}

#[test]
fn test_encode_auipc() {
    use vm_frontend::riscv64::encode_auipc;
    let encoded = encode_auipc(1, 0x12);
    assert_eq!(encoded & 0x7F, 0x17); // Should have AUIPC opcode
}

#[test]
fn test_encode_branch() {
    use vm_frontend::riscv64::encode_branch;
    let encoded = encode_branch(0, 1, 2, 0x10);
    assert_eq!(encoded & 0x7F, 0x63); // Should have branch opcode
}

#[test]
fn test_encode_beq() {
    use vm_frontend::riscv64::encode_beq;
    let encoded = encode_beq(1, 2, 0x10);
    assert_eq!(encoded & 0x7F, 0x63);
}

#[test]
fn test_encode_bne() {
    use vm_frontend::riscv64::encode_bne;
    let encoded = encode_bne(1, 2, 0x10);
    assert_eq!(encoded & 0x7F, 0x63);
}

#[test]
fn test_encode_blt() {
    use vm_frontend::riscv64::encode_blt;
    let encoded = encode_blt(1, 2, 0x10);
    assert_eq!(encoded & 0x7F, 0x63);
}

#[test]
fn test_encode_bge() {
    use vm_frontend::riscv64::encode_bge;
    let encoded = encode_bge(1, 2, 0x10);
    assert_eq!(encoded & 0x7F, 0x63);
}

#[test]
fn test_encode_bltu() {
    use vm_frontend::riscv64::encode_bltu;
    let encoded = encode_bltu(1, 2, 0x10);
    assert_eq!(encoded & 0x7F, 0x63);
}

#[test]
fn test_encode_bgeu() {
    use vm_frontend::riscv64::encode_bgeu;
    let encoded = encode_bgeu(1, 2, 0x10);
    assert_eq!(encoded & 0x7F, 0x63);
}

#[test]
fn test_encode_add() {
    use vm_frontend::riscv64::encode_add;
    let encoded = encode_add(1, 2, 3);
    assert_eq!(encoded & 0x7F, 0x33); // Should have OP opcode
}

#[test]
fn test_encode_sub() {
    use vm_frontend::riscv64::encode_sub;
    let encoded = encode_sub(1, 2, 3);
    assert_eq!(encoded & 0x7F, 0x33);
}

#[test]
fn test_encode_addi() {
    use vm_frontend::riscv64::encode_addi;
    let encoded = encode_addi(1, 2, 0x10);
    assert_eq!(encoded & 0x7F, 0x13); // Should have OP-IMM opcode
}

#[test]
fn test_encode_lw() {
    use vm_frontend::riscv64::encode_lw;
    let encoded = encode_lw(1, 2, 0x10);
    assert_eq!(encoded & 0x7F, 0x03); // Should have LOAD opcode
}

#[test]
fn test_encode_sw() {
    use vm_frontend::riscv64::encode_sw;
    let encoded = encode_sw(1, 2, 0x10);
    assert_eq!(encoded & 0x7F, 0x23); // Should have STORE opcode
}

// ============================================================================
// Sequential Decoding Tests
// ============================================================================

#[test]
fn test_sequential_decode_basic() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    let instructions = [0x00000537, 0x00050513, 0x00b50533];
    mmu.write_insn_batch(GuestAddr(0), &instructions);

    let mut pc = GuestAddr(0);
    for _ in 0..3 {
        let result = decoder.decode_insn(&mmu, pc).unwrap();
        pc = result.next_pc;
    }
    assert_eq!(pc.0, 12);
}

#[test]
fn test_sequential_decode_with_branch() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    let instructions = [0x00050513, 0x00050863, 0x00b50533];
    mmu.write_insn_batch(GuestAddr(0), &instructions);

    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    assert_eq!(result.next_pc, GuestAddr(4));
}

#[test]
fn test_decode_at_nonzero_pc() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    let pc = GuestAddr(0x1000);
    mmu.write_insn(pc, 0x00000537);

    let result = decoder.decode_insn(&mmu, pc).unwrap();
    assert_eq!(result.next_pc, GuestAddr(0x1004));
}

// ============================================================================
// Boundary Condition Tests
// ============================================================================

#[test]
fn test_minimal_pc() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00050513);
    let _result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
}

#[test]
fn test_large_pc() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    let pc = GuestAddr(0xFFFF_F000);
    mmu.write_insn(pc, 0x00050513);
    let _result = decoder.decode_insn(&mmu, pc).unwrap();
}

#[test]
fn test_zero_instruction() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0x00000000);
    let result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
    // Should decode as ADDI x0, x0, 0 (NOP)
    assert_eq!(result.mnemonic, "addi");
}

#[test]
fn test_maximal_instruction() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    mmu.write_insn(GuestAddr(0), 0xFFFFFFFF);
    let _result = decoder.decode_insn(&mmu, GuestAddr(0)).unwrap();
}

// ============================================================================
// Memory Access Pattern Tests
// ============================================================================

#[test]
fn test_multiple_loads_in_sequence() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    let instructions = [0x00052503, 0x00053503, 0x00054503, 0x00055503];
    mmu.write_insn_batch(GuestAddr(0), &instructions);

    for i in 0..4 {
        let pc = GuestAddr(i as u64 * 4);
        let result = decoder.decode_insn(&mmu, pc).unwrap();
        assert!(result.has_memory_op);
        assert!(!result.is_branch);
    }
}

#[test]
fn test_multiple_stores_in_sequence() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    let instructions = [0x00a50023, 0x00a51023, 0x00a52023, 0x00a53023];
    mmu.write_insn_batch(GuestAddr(0), &instructions);

    for i in 0..4 {
        let pc = GuestAddr(i as u64 * 4);
        let result = decoder.decode_insn(&mmu, pc).unwrap();
        assert!(result.has_memory_op);
        assert!(!result.is_branch);
    }
}

// ============================================================================
// Mixed Instruction Pattern Tests
// ============================================================================

#[test]
fn test_arithmetic_load_store_mix() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    let instructions = [0x00b50533, 0x00054503, 0x00a52023, 0x00b51533];
    mmu.write_insn_batch(GuestAddr(0), &instructions);

    let expected = [false, true, true, false];
    for (i, &expected_mem) in expected.iter().enumerate() {
        let pc = GuestAddr(i as u64 * 4);
        let result = decoder.decode_insn(&mmu, pc).unwrap();
        assert_eq!(result.has_memory_op, expected_mem);
    }
}

#[test]
fn test_branch_arithmetic_mix() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    let instructions = [0x00b50533, 0x00050863, 0x00b51533, 0x00c5d463];
    mmu.write_insn_batch(GuestAddr(0), &instructions);

    let expected = [false, true, false, true];
    for (i, &expected_branch) in expected.iter().enumerate() {
        let pc = GuestAddr(i as u64 * 4);
        let result = decoder.decode_insn(&mmu, pc).unwrap();
        assert_eq!(result.is_branch, expected_branch);
    }
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn test_empty_memory() {
    let decoder = RiscvDecoder;
    let mmu = TestMMU::new();
    // Read from address with no instruction
    let result = decoder.decode_insn(&mmu, GuestAddr(0x1000));
    assert!(result.is_ok());
    // Should return zero and decode as addi x0, x0, 0
}

#[test]
fn test_pc_overflow() {
    let mut decoder = RiscvDecoder;
    let mut mmu = TestMMU::new();
    let pc = GuestAddr(0xFFFF_FFFC);
    mmu.write_insn(pc, 0x00050513);
    let result = decoder.decode_insn(&mmu, pc).unwrap();
    // next_pc should wrap to 0
    assert_eq!(result.next_pc, GuestAddr(0));
}
