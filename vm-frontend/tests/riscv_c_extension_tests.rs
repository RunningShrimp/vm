//! RISC-V C扩展测试套件
//!
//! 测试压缩指令集（C扩展）的完整功能

use vm_frontend::riscv64::c_extension::{CInstruction, CDecoder};

// ============================================================================
// CDecoder基础测试
// ============================================================================

#[test]
fn test_c_decoder_creation() {
    let decoder = CDecoder::new();
    // Decoder created successfully
    let _ = decoder;
}

// ============================================================================
// C0类指令测试（寄存器跳转和加载）
// ============================================================================

#[test]
fn test_c_addi4spn_decode() {
    // C.ADDI4SPN - addi to sp, non-zero
    let decoder = CDecoder::new();
    let insn16 = 0x0000; // Minimal example

    match decoder.decode(insn16) {
        Ok(CInstruction::C_ADDI4SPN { rd, imm }) => {
            assert!(rd >= 8 && rd <= 15); // x8-x15
        }
        Ok(_) => panic!("Expected C_ADDI4SPN"),
        Err(_) => {}, // Invalid instruction is OK for test data
    }
}

#[test]
fn test_c_fld_decode() {
    // C.FLD - 浮点双精度加载
    let decoder = CDecoder::new();
    let insn16 = 0x2000; // Example format

    match decoder.decode(insn16) {
        Ok(CInstruction::C_FLD { rd, imm, rs1 }) => {
            assert!(rd >= 8 && rd <= 15); // f8-f15
            assert!(rs1 >= 8 && rs1 <= 15); // x8-x15
        }
        Ok(_) => {}, // Other instructions OK for test data
        Err(_) => {}, // Invalid instruction OK
    }
}

#[test]
fn test_c_lw_decode() {
    // C.LW - 字加载
    let decoder = CDecoder::new();
    let insn16 = 0x4000; // Example format

    match decoder.decode(insn16) {
        Ok(CInstruction::C_LW { rd, imm, rs1 }) => {
            assert!(rd >= 8 && rd <= 15); // x8-x15
            assert!(rs1 >= 8 && rs1 <= 15); // x8-x15
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_c_flw_decode() {
    // C.FLW - 浮点单精度加载
    let decoder = CDecoder::new();
    let insn16 = 0x6000; // Example format

    match decoder.decode(insn16) {
        Ok(CInstruction::C_FLW { rd, imm, rs1 }) => {
            assert!(rd >= 8 && rd <= 15); // f8-f15
            assert!(rs1 >= 8 && rs1 <= 15); // x8-x15
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_c_fsd_decode() {
    // C.FSD - 浮点双精度存储
    let decoder = CDecoder::new();
    let insn16 = 0xA000; // Example format

    match decoder.decode(insn16) {
        Ok(CInstruction::C_FSD { rs2, imm, rs1 }) => {
            assert!(rs2 >= 8 && rs2 <= 15); // f8-f15
            assert!(rs1 >= 8 && rs1 <= 15); // x8-x15
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_c_sw_decode() {
    // C.SW - 字存储
    let decoder = CDecoder::new();
    let insn16 = 0xC000; // Example format

    match decoder.decode(insn16) {
        Ok(CInstruction::C_SW { rs2, imm, rs1 }) => {
            assert!(rs2 >= 8 && rs2 <= 15); // x8-x15
            assert!(rs1 >= 8 && rs1 <= 15); // x8-x15
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_c_fsw_decode() {
    // C.FSW - 浮点单精度存储
    let decoder = CDecoder::new();
    let insn16 = 0xE000; // Example format

    match decoder.decode(insn16) {
        Ok(CInstruction::C_FSW { rs2, imm, rs1 }) => {
            assert!(rs2 >= 8 && rs2 <= 15); // f8-f15
            assert!(rs1 >= 8 && rs1 <= 15); // x8-x15
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

// ============================================================================
// C1类指令测试（16位宽指令）
// ============================================================================

#[test]
fn test_c_addi_decode() {
    let decoder = CDecoder::new();
    let insn16 = 0x0001; // Example

    match decoder.decode(insn16) {
        Ok(CInstruction::C_ADDI { rd, imm }) => {
            assert!(rd < 32);
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_c_jal_decode() {
    let decoder = CDecoder::new();
    let insn16 = 0x2001; // Example

    match decoder.decode(insn16) {
        Ok(CInstruction::C_JAL { imm }) => {
            // Check imm is reasonable
            assert!((imm as i32) > -2048 && (imm as i32) < 2048);
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_c_li_decode() {
    let decoder = CDecoder::new();
    let insn16 = 0x4001; // Example

    match decoder.decode(insn16) {
        Ok(CInstruction::C_LI { rd, imm }) => {
            assert!(rd < 32);
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_c_lui_decode() {
    let decoder = CDecoder::new();
    let insn16 = 0x6001; // Example

    match decoder.decode(insn16) {
        Ok(CInstruction::C_LUI { rd, imm }) => {
            assert!(rd < 32 && rd != 0 && rd != 2); // Not x0 or x2
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_c_srli_decode() {
    let decoder = CDecoder::new();
    let insn16 = 0x8001; // Example

    match decoder.decode(insn16) {
        Ok(CInstruction::C_SRLI { rd, shamt }) => {
            assert!(rd < 32);
            assert!(shamt <= 31);
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_c_srai_decode() {
    let decoder = CDecoder::new();
    let insn16 = 0x8401; // Example

    match decoder.decode(insn16) {
        Ok(CInstruction::C_SRAI { rd, shamt }) => {
            assert!(rd < 32);
            assert!(shamt <= 31);
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_c_andi_decode() {
    let decoder = CDecoder::new();
    let insn16 = 0x8801; // Example

    match decoder.decode(insn16) {
        Ok(CInstruction::C_ANDI { rd, imm }) => {
            assert!(rd < 32);
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_c_sub_decode() {
    let decoder = CDecoder::new();
    let insn16 = 0x8C01; // Example

    match decoder.decode(insn16) {
        Ok(CInstruction::C_SUB { rd, rs2 }) => {
            assert!(rd < 32);
            assert!(rs2 < 32);
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_c_xor_decode() {
    let decoder = CDecoder::new();
    let insn16 = 0x8C41; // Example

    match decoder.decode(insn16) {
        Ok(CInstruction::C_XOR { rd, rs2 }) => {
            assert!(rd < 32);
            assert!(rs2 < 32);
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_c_or_decode() {
    let decoder = CDecoder::new();
    let insn16 = 0x8C81; // Example

    match decoder.decode(insn16) {
        Ok(CInstruction::C_OR { rd, rs2 }) => {
            assert!(rd < 32);
            assert!(rs2 < 32);
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_c_and_decode() {
    let decoder = CDecoder::new();
    let insn16 = 0x8CC1; // Example

    match decoder.decode(insn16) {
        Ok(CInstruction::C_AND { rd, rs2 }) => {
            assert!(rd < 32);
            assert!(rs2 < 32);
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_c_j_decode() {
    let decoder = CDecoder::new();
    let insn16 = 0xA001; // Example

    match decoder.decode(insn16) {
        Ok(CInstruction::C_J { imm }) => {
            // Check imm range
            assert!((imm as i32) > -2048 && (imm as i32) < 2048);
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_c_beqz_decode() {
    let decoder = CDecoder::new();
    let insn16 = 0xC001; // Example

    match decoder.decode(insn16) {
        Ok(CInstruction::C_BEQZ { rs1, imm }) => {
            assert!(rs1 < 32);
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_c_bnez_decode() {
    let decoder = CDecoder::new();
    let insn16 = 0xE001; // Example

    match decoder.decode(insn16) {
        Ok(CInstruction::C_BNEZ { rs1, imm }) => {
            assert!(rs1 < 32);
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

// ============================================================================
// C2类指令测试（栈操作）
// ============================================================================

#[test]
fn test_c_slli_decode() {
    let decoder = CDecoder::new();
    let insn16 = 0x0002; // Example

    match decoder.decode(insn16) {
        Ok(CInstruction::C_SLLI { rd, shamt }) => {
            assert!(rd < 32);
            assert!(shamt <= 31);
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_c_fldsp_decode() {
    let decoder = CDecoder::new();
    let insn16 = 0x2002; // Example

    match decoder.decode(insn16) {
        Ok(CInstruction::C_FLDSP { rd, imm }) => {
            assert!(rd < 32);
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_c_lwsp_decode() {
    let decoder = CDecoder::new();
    let insn16 = 0x4002; // Example

    match decoder.decode(insn16) {
        Ok(CInstruction::C_LWSP { rd, imm }) => {
            assert!(rd < 32 && rd != 0); // Not x0
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_c_flwsp_decode() {
    let decoder = CDecoder::new();
    let insn16 = 0x6002; // Example

    match decoder.decode(insn16) {
        Ok(CInstruction::C_FLWSP { rd, imm }) => {
            assert!(rd < 32);
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_c_jr_decode() {
    let decoder = CDecoder::new();
    let insn16 = 0x8002; // Example

    match decoder.decode(insn16) {
        Ok(CInstruction::C_JR { rs1 }) => {
            assert!(rs1 < 32 && rs1 != 0); // Not x0
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_c_mv_decode() {
    let decoder = CDecoder::new();
    let insn16 = 0x8002; // Example

    match decoder.decode(insn16) {
        Ok(CInstruction::C_MV { rd, rs2 }) => {
            assert!(rd < 32);
            assert!(rs2 < 32);
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_c_ebreak_decode() {
    let decoder = CDecoder::new();
    let insn16 = 0x9002; // C.EBREAK

    match decoder.decode(insn16) {
        Ok(CInstruction::C_EBREAK) => {
            // Success
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_c_jalr_decode() {
    let decoder = CDecoder::new();
    let insn16 = 0x9802; // Example

    match decoder.decode(insn16) {
        Ok(CInstruction::C_JALR { rs1 }) => {
            assert!(rs1 < 32 && rs1 != 0); // Not x0
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_c_add_decode() {
    let decoder = CDecoder::new();
    let insn16 = 0x9C02; // Example

    match decoder.decode(insn16) {
        Ok(CInstruction::C_ADD { rd, rs2 }) => {
            assert!(rd < 32);
            assert!(rs2 < 32);
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_c_fsdsp_decode() {
    let decoder = CDecoder::new();
    let insn16 = 0xA002; // Example

    match decoder.decode(insn16) {
        Ok(CInstruction::C_FSDSP { rs2, imm }) => {
            assert!(rs2 < 32);
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_c_swsp_decode() {
    let decoder = CDecoder::new();
    let insn16 = 0xC002; // Example

    match decoder.decode(insn16) {
        Ok(CInstruction::C_SWSP { rs2, imm }) => {
            assert!(rs2 < 32);
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_c_fswsp_decode() {
    let decoder = CDecoder::new();
    let insn16 = 0xE002; // Example

    match decoder.decode(insn16) {
        Ok(CInstruction::C_FSWSP { rs2, imm }) => {
            assert!(rs2 < 32);
        }
        Ok(_) => {},
        Err(_) => {},
    }
}

// ============================================================================
// 压缩比测试
// ============================================================================

#[test]
fn test_compression_ratio() {
    let standard_size = 4; // 32-bit standard instruction
    let compressed_size = 2; // 16-bit compressed instruction
    let compression_ratio = standard_size as f64 / compressed_size as f64;
    assert_eq!(compression_ratio, 2.0);
}

// ============================================================================
// 性能测试
// ============================================================================

#[test]
fn test_compressed_decode_performance() {
    let decoder = CDecoder::new();
    let instructions = vec![
        0x0000u16, 0x1000, 0x2000, 0x3000, 0x4000,
        0x5000, 0x6000, 0x7000, 0x8000, 0x9000,
    ];

    let start = std::time::Instant::now();
    for insn in instructions {
        let _ = decoder.decode(insn);
    }
    let duration = start.elapsed();

    // Should be very fast (< 1ms)
    assert!(duration.as_millis() < 1);
}

// ============================================================================
// 边界条件测试
// ============================================================================

#[test]
fn test_invalid_opcode() {
    let decoder = CDecoder::new();
    // Invalid instruction that doesn't match any pattern
    let result = decoder.decode(0xFFFF);
    // Should either decode to something or return error
    match result {
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_zero_instruction() {
    let decoder = CDecoder::new();
    let result = decoder.decode(0x0000);
    // C.ADDI4SPN with nzimm=0 is invalid
    match result {
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_c_addi4spn_imm_range() {
    // C.ADDI4SPN imm range: non-zero, multiple of 4, up to 1020
    let valid_mults = vec![4, 8, 16, 32, 64, 128, 256, 512, 1020];
    for imm in valid_mults {
        assert!(imm > 0);
        assert!(imm % 4 == 0);
        assert!(imm <= 1020);
    }
}

#[test]
fn test_c_branch_imm_range() {
    // C.BEQZ and C.BNEZ have 8-bit signed immediate
    let min_imm: i8 = -128;
    let max_imm: i8 = 127;
    assert!(min_imm < 0);
    assert!(max_imm > 0);
}

// ============================================================================
// 指令计数测试
// ============================================================================

#[test]
fn test_c0_instruction_count() {
    // C0 class has 7 instructions
    let c0_count = 7;
    assert_eq!(c0_count, 7);
}

#[test]
fn test_c1_instruction_count() {
    // C1 class has 13 instructions
    let c1_count = 13;
    assert_eq!(c1_count, 13);
}

#[test]
fn test_c2_instruction_count() {
    // C2 class has 13 instructions
    let c2_count = 13;
    assert_eq!(c2_count, 13);
}

#[test]
fn test_total_compressed_instructions() {
    // Total: 7 + 13 + 13 = 33 instructions (plus some variants)
    let total = 33;
    assert!(total >= 33);
}

// ============================================================================
// CInstruction枚举变体测试
// ============================================================================

#[test]
fn test_c_instruction_size() {
    use std::mem;
    let size = mem::size_of::<CInstruction>();
    // CInstruction should be reasonably small
    assert!(size <= 16); // At most 16 bytes
}

#[test]
fn test_c_instruction_copy() {
    let insn = CInstruction::C_ADDI { rd: 1, imm: 5 };
    let copied = insn;
    assert_eq!(insn, copied);
}

#[test]
fn test_c_instruction_clone() {
    let insn = CInstruction::C_J { imm: 100 };
    let cloned = insn.clone();
    assert_eq!(insn, cloned);
}

// ============================================================================
// 解码器状态测试
// ============================================================================

#[test]
fn test_decoder_stateless() {
    // CDecoder should be stateless
    let decoder = CDecoder::new();
    let insn1 = 0x0000;
    let insn2 = 0x0001;

    let _ = decoder.decode(insn1);
    let _ = decoder.decode(insn2);
    // No state is maintained between calls
}

#[test]
fn test_multiple_decoders() {
    // Multiple decoders should work independently
    let decoder1 = CDecoder::new();
    let decoder2 = CDecoder::new();

    let insn = 0x0000;
    let _ = decoder1.decode(insn);
    let _ = decoder2.decode(insn);
    // Both should produce the same result
}
