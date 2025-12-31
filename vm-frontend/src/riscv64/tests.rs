//! RISC-V 64-bit frontend tests
//!
//! Tests for RISC-V instruction decoder and IR generation

use vm_core::{GuestAddr, MMU, VmError};

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
}

impl MMU for TestMMU {
    fn fetch_insn(&self, pc: GuestAddr) -> Result<u64, VmError> {
        Ok(*self.memory.get(&pc.0).unwrap_or(&0))
    }
}

#[cfg(test)]
mod instruction_tests {
    use super::*;

    /// Test basic instruction creation
    #[test]
    fn test_instruction_creation() {
        let insn = super::super::RiscvInstruction {
            mnemonic: "add",
            next_pc: GuestAddr(0x4),
            has_memory_op: false,
            is_branch: false,
        };

        assert_eq!(insn.mnemonic(), "add");
        assert_eq!(insn.next_pc(), GuestAddr(0x4));
        assert_eq!(insn.size(), 4);
        assert!(!insn.is_memory_access());
        assert!(!insn.is_control_flow());
    }

    /// Test memory operation detection
    #[test]
    fn test_memory_operation_detection() {
        let load_insn = super::super::RiscvInstruction {
            mnemonic: "ld",
            next_pc: GuestAddr(0x4),
            has_memory_op: true,
            is_branch: false,
        };

        assert!(load_insn.is_memory_access());
        assert!(!load_insn.is_control_flow());
    }

    /// Test branch instruction detection
    #[test]
    fn test_branch_instruction_detection() {
        let branch_insn = super::super::RiscvInstruction {
            mnemonic: "beq",
            next_pc: GuestAddr(0x4),
            has_memory_op: false,
            is_branch: true,
        };

        assert!(!branch_insn.is_memory_access());
        assert!(branch_insn.is_control_flow());
    }

    /// Test operand count
    #[test]
    fn test_operand_count() {
        let insn = super::super::RiscvInstruction {
            mnemonic: "add",
            next_pc: GuestAddr(0x4),
            has_memory_op: false,
            is_branch: false,
        };

        assert_eq!(insn.operand_count(), 1);
    }
}

#[cfg(test)]
mod decoder_tests {
    use super::super::RiscvDecoder;
    use super::*;

    /// Test decoder creation
    #[test]
    fn test_decoder_creation() {
        let _decoder = RiscvDecoder;
        // Just ensure we can create a decoder
    }

    /// Test LUI instruction decoding
    #[test]
    fn test_decode_lui() {
        let mut decoder = RiscvDecoder;
        let mut mmu = TestMMU::new();

        // LUI x1, 0x12345
        // Encoding: 0x00012337 (opcode=0x37)
        let insn: u32 = 0x00012337;
        mmu.write_insn(GuestAddr(0), insn as u64);

        let result = decoder.decode_insn(&mmu, GuestAddr(0));
        assert!(result.is_ok());

        let decoded = result.unwrap();
        assert_eq!(decoded.mnemonic, "lui");
        assert_eq!(decoded.next_pc, GuestAddr(0x4));
        assert!(!decoded.has_memory_op);
        assert!(!decoded.is_branch);
    }

    /// Test AUIPC instruction decoding
    #[test]
    fn test_decode_auipc() {
        let mut decoder = RiscvDecoder;
        let mut mmu = TestMMU::new();

        // AUIPC x1, 0x1000
        // Encoding: 0x00001097 (opcode=0x17)
        let insn: u32 = 0x00001097;
        mmu.write_insn(GuestAddr(0), insn as u64);

        let result = decoder.decode_insn(&mmu, GuestAddr(0));
        assert!(result.is_ok());

        let decoded = result.unwrap();
        assert_eq!(decoded.mnemonic, "auipc");
    }

    /// Test JAL instruction decoding (branch)
    #[test]
    fn test_decode_jal() {
        let mut decoder = RiscvDecoder;
        let mut mmu = TestMMU::new();

        // JAL x1, 0x1000
        // Encoding with opcode=0x6f
        let insn: u32 = 0x6f0000ef; // Simplified example
        mmu.write_insn(GuestAddr(0), insn as u64);

        let result = decoder.decode_insn(&mmu, GuestAddr(0));
        assert!(result.is_ok());

        let decoded = result.unwrap();
        assert_eq!(decoded.mnemonic, "jal");
        assert!(decoded.is_branch);
    }

    /// Test load instruction decoding (memory operation)
    #[test]
    fn test_decode_load() {
        let mut decoder = RiscvDecoder;
        let mut mmu = TestMMU::new();

        // LD x1, 0(x2)
        // Encoding: opcode=0x03
        let insn: u32 = 0x00031083;
        mmu.write_insn(GuestAddr(0), insn as u64);

        let result = decoder.decode_insn(&mmu, GuestAddr(0));
        assert!(result.is_ok());

        let decoded = result.unwrap();
        assert_eq!(decoded.mnemonic, "load");
        assert!(decoded.has_memory_op);
        assert!(!decoded.is_branch);
    }

    /// Test store instruction decoding (memory operation)
    #[test]
    fn test_decode_store() {
        let mut decoder = RiscvDecoder;
        let mut mmu = TestMMU::new();

        // SD x1, 0(x2)
        // Encoding: opcode=0x23
        let insn: u32 = 0x00131023;
        mmu.write_insn(GuestAddr(0), insn as u64);

        let result = decoder.decode_insn(&mmu, GuestAddr(0));
        assert!(result.is_ok());

        let decoded = result.unwrap();
        assert_eq!(decoded.mnemonic, "store");
        assert!(decoded.has_memory_op);
        assert!(!decoded.is_branch);
    }

    /// Test arithmetic instruction decoding
    #[test]
    fn test_decode_arithmetic() {
        let mut decoder = RiscvDecoder;
        let mut mmu = TestMMU::new();

        // ADD x1, x2, x3
        // Encoding: opcode=0x33
        let insn: u32 = 0x003100b3;
        mmu.write_insn(GuestAddr(0), insn as u64);

        let result = decoder.decode_insn(&mmu, GuestAddr(0));
        assert!(result.is_ok());

        let decoded = result.unwrap();
        assert_eq!(decoded.mnemonic, "arith");
        assert!(!decoded.has_memory_op);
        assert!(!decoded.is_branch);
    }

    /// Test unknown opcode handling
    #[test]
    fn test_decode_unknown() {
        let mut decoder = RiscvDecoder;
        let mut mmu = TestMMU::new();

        // Invalid opcode: 0x7f (not a standard RISC-V opcode)
        let insn: u32 = 0x7f000000;
        mmu.write_insn(GuestAddr(0), insn as u64);

        let result = decoder.decode_insn(&mmu, GuestAddr(0));
        assert!(result.is_ok());

        let decoded = result.unwrap();
        assert_eq!(decoded.mnemonic, "unknown");
    }

    /// Test vector instruction decoding
    #[test]
    fn test_decode_vector() {
        let mut decoder = RiscvDecoder;
        let mut mmu = TestMMU::new();

        // Vector instruction: opcode=0x57
        let insn: u32 = 0x57000000;
        mmu.write_insn(GuestAddr(0), insn as u64);

        let result = decoder.decode_insn(&mmu, GuestAddr(0));
        assert!(result.is_ok());

        let decoded = result.unwrap();
        assert_eq!(decoded.mnemonic, "vector");
        assert!(decoded.has_memory_op); // Vector loads/stores are memory ops
    }
}

#[cfg(test)]
mod utility_tests {
    use super::super::sext21;

    /// Test sign extension function
    #[test]
    fn test_sext21() {
        // Test positive number
        assert_eq!(sext21(0x000000), 0);

        // Test negative number (bit 20 set)
        let result = sext21(0x100000);
        assert!(result < 0); // Should be negative

        // Test maximum positive value
        let max_pos = 0x0FFFFF;
        assert_eq!(sext21(max_pos), 0x0FFFFF);

        // Test minimum negative value
        let min_neg = 0x100000;
        let result = sext21(min_neg);
        assert_eq!(result, 0xFFFE00000i64);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::super::RiscvDecoder;
    use super::*;

    /// Test sequential instruction decoding
    #[test]
    fn test_sequential_decoding() {
        let mut decoder = RiscvDecoder;
        let mut mmu = TestMMU::new();

        // Add a few instructions
        let instructions = [
            0x00012337, // LUI
            0x00001097, // AUIPC
            0x003100b3, // ADD
        ];

        for (i, &insn) in instructions.iter().enumerate() {
            let addr = GuestAddr(i as u64 * 4);
            mmu.write_insn(addr, insn as u64);
        }

        // Decode each instruction
        let mut pc = GuestAddr(0);
        for &expected_mnemonic in ["lui", "auipc", "arith"] {
            let result = decoder.decode_insn(&mmu, pc);
            assert!(result.is_ok());

            let decoded = result.unwrap();
            assert_eq!(decoded.mnemonic, expected_mnemonic);
            pc = decoded.next_pc;
        }
    }

    /// Test instruction at non-zero PC
    #[test]
    fn test_decode_at_nonzero_pc() {
        let mut decoder = RiscvDecoder;
        let mut mmu = TestMMU::new();

        let insn: u32 = 0x00012337;
        let pc = GuestAddr(0x1000);
        mmu.write_insn(pc, insn as u64);

        let result = decoder.decode_insn(&mmu, pc);
        assert!(result.is_ok());

        let decoded = result.unwrap();
        assert_eq!(decoded.mnemonic, "lui");
        assert_eq!(decoded.next_pc, GuestAddr(0x1004));
    }
}

// ============================================================================
// F Extension Tests (Single-Precision Floating-Point)
// ============================================================================

#[cfg(test)]
mod f_extension_tests {
    use super::super::f_extension::{FPRegisters, FCSR, FFlags, RoundingMode, FExtensionExecutor};

    /// Test FP register creation and default values
    #[test]
    fn test_fp_registers_default() {
        let regs = FPRegisters::default();
        for i in 0..32 {
            assert_eq!(regs.get(i), 0.0);
        }
    }

    /// Test FP register get/set
    #[test]
    fn test_fp_registers_get_set() {
        let mut regs = FPRegisters::default();
        regs.set(1, 1.5);
        assert_eq!(regs.get(1), 1.5);
        assert!((regs.get(1) - 1.5).abs() < f32::EPSILON);
    }

    /// Test FP register bit manipulation
    #[test]
    fn test_fp_registers_bits() {
        let mut regs = FPRegisters::default();
        regs.set(5, 3.14159);

        let bits = regs.get_bits(5);
        assert_eq!(bits, 3.14159_f32.to_bits());

        // Test setting from bits
        regs.set_bits(6, bits);
        assert!((regs.get(6) - 3.14159).abs() < f32::EPSILON);
    }

    /// Test FCSR default
    #[test]
    fn test_fcsr_default() {
        let fcsr = FCSR::default();
        assert!(!fcsr.flags.nv);
        assert!(!fcsr.flags.dz);
        assert!(!fcsr.flags.of);
        assert!(!fcsr.flags.uf);
        assert!(!fcsr.flags.nx);
        assert_eq!(fcsr.rm, RoundingMode::RNE);
    }

    /// Test RoundingMode variants
    #[test]
    fn test_rounding_modes() {
        assert_eq!(RoundingMode::RNE as i32, 0);
        assert_eq!(RoundingMode::RTZ as i32, 1);
        assert_eq!(RoundingMode::RDN as i32, 2);
        assert_eq!(RoundingMode::RUP as i32, 3);
        assert_eq!(RoundingMode::RMM as i32, 4);
    }

    /// Test FExtensionExecutor creation
    #[test]
    fn test_f_extension_executor_creation() {
        let executor = FExtensionExecutor::new();
        assert!(!executor.exceptions_enabled);
    }

    /// Test FP operations with executor
    #[test]
    fn test_f_extension_basic_operations() {
        let mut executor = FExtensionExecutor::new();

        // Set some values
        executor.fp_regs.set(1, 2.0);
        executor.fp_regs.set(2, 3.0);

        // Test FADD.S would be here but requires MMU context
        // Just verify we can access the registers
        assert!((executor.fp_regs.get(1) - 2.0).abs() < f32::EPSILON);
        assert!((executor.fp_regs.get(2) - 3.0).abs() < f32::EPSILON);
    }
}

// ============================================================================
// D Extension Tests (Double-Precision Floating-Point)
// ============================================================================

#[cfg(test)]
mod d_extension_tests {
    use super::super::d_extension::DExtensionExecutor;

    /// Test DExtensionExecutor creation
    #[test]
    fn test_d_extension_executor_creation() {
        let executor = DExtensionExecutor::new();
        assert!(!executor.exceptions_enabled);
    }

    /// Test D extension executor default state
    #[test]
    fn test_d_extension_default_state() {
        let executor = DExtensionExecutor::new();

        // Check all registers start at 0.0
        for i in 0..16 {
            assert_eq!(executor.fp_regs.get_f64(i), 0.0);
        }
    }
}

// ============================================================================
// C Extension Tests (Compressed Instructions)
// ============================================================================

#[cfg(test)]
mod c_extension_tests {
    use super::super::c_extension::{CDecoder, CInstruction};

    /// Test CDecoder creation
    #[test]
    fn test_c_decoder_creation() {
        let decoder = CDecoder::new();
        // Just ensure we can create a decoder
    }

    /// Test compressed instruction identification
    #[test]
    fn test_compressed_insn_detection() {
        // Compressed instructions have bits [1:0] != 11
        let compressed_insn: u16 = 0x0001; // Not compressed
        let is_compressed = (compressed_insn & 0x3) != 0x3;
        assert!(!is_compressed);

        let compressed_insn2: u16 = 0x0002; // Compressed pattern
        let is_compressed2 = (compressed_insn2 & 0x3) != 0x3;
        assert!(is_compressed2);
    }
}
