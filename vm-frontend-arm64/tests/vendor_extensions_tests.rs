//! 厂商扩展指令测试

use vm_frontend_arm64::{AmxDecoder, AmxInstruction, AmxPrecision};
use vm_frontend_arm64::{ApuDecoder, ApuInstruction};
use vm_frontend_arm64::{HexagonDecoder, HexagonInstruction};
use vm_frontend_arm64::{NpuDecoder, NpuInstruction};

#[test]
fn test_amx_decode_ld() {
    let decoder = AmxDecoder::new();
    // 构造 AMX_LD 指令
    let insn = 0xF_A0_10_00 | (0x100 & 0xFFF);
    let result = decoder.decode(insn, vm_core::GuestAddr(0x1000));
    assert!(result.is_ok());
    if let Ok(Some(AmxInstruction::AmxLd { tile, base, offset })) = result {
        assert_eq!(tile, 0);
        assert_eq!(base, 1);
        assert_eq!(offset, 0x100);
    } else {
        panic!("Failed to decode AMX_LD");
    }
}

#[test]
fn test_amx_decode_fma() {
    let decoder = AmxDecoder::new();
    // 构造 AMX_FMA 指令
    let insn = 0xF_A2_01_23 | (3 << 8);
    let result = decoder.decode(insn, vm_core::GuestAddr(0x1000));
    assert!(result.is_ok());
    if let Ok(Some(AmxInstruction::AmxFma {
        tile_c,
        tile_a,
        tile_b,
        precision,
    })) = result
    {
        assert_eq!(tile_c, 0);
        assert_eq!(tile_a, 1);
        assert_eq!(tile_b, 2);
        assert_eq!(precision, AmxPrecision::Fp32);
    } else {
        panic!("Failed to decode AMX_FMA");
    }
}

#[test]
fn test_hexagon_decode_add() {
    let decoder = HexagonDecoder::new();
    let insn = 0xE2_00_01_02;
    let result = decoder.decode(insn, vm_core::GuestAddr(0x1000));
    assert!(result.is_ok());
    if let Ok(Some(HexagonInstruction::HexAdd { dst, src1, src2 })) = result {
        assert_eq!(dst, 0);
        assert_eq!(src1, 1);
        assert_eq!(src2, 2);
    } else {
        panic!("Failed to decode HEX_ADD");
    }
}

#[test]
fn test_apu_decode_conv() {
    let decoder = ApuDecoder::new();
    let insn = 0xB2_00_01_02 | (3 << 4); // kernel_size=3
    let result = decoder.decode(insn, vm_core::GuestAddr(0x1000));
    assert!(result.is_ok());
    if let Ok(Some(ApuInstruction::ApuConv {
        dst,
        src,
        kernel,
        kernel_size,
    })) = result
    {
        assert_eq!(dst, 0);
        assert_eq!(src, 1);
        assert_eq!(kernel, 2);
        assert_eq!(kernel_size, 3);
    } else {
        panic!("Failed to decode APU_CONV");
    }
}

#[test]
fn test_npu_decode_conv() {
    let decoder = NpuDecoder::new();
    let insn = 0xC2_00_01_02;
    let result = decoder.decode(insn, vm_core::GuestAddr(0x1000));
    assert!(result.is_ok());
    if let Ok(Some(NpuInstruction::NpuConv { dst, src, kernel })) = result {
        assert_eq!(dst, 0);
        assert_eq!(src, 1);
        assert_eq!(kernel, 2);
    } else {
        panic!("Failed to decode NPU_CONV");
    }
}
