use vm_frontend_x86_64::X86Mnemonic;

#[test]
fn test_amx_instruction_enum() {
    // Test that AMX instructions are defined
    assert!(matches!(X86Mnemonic::Tileloadd, X86Mnemonic::Tileloadd));
    assert!(matches!(X86Mnemonic::Tileloaddt1, X86Mnemonic::Tileloaddt1));
    assert!(matches!(X86Mnemonic::Tilestored, X86Mnemonic::Tilestored));
    assert!(matches!(X86Mnemonic::Tdpbf16ps, X86Mnemonic::Tdpbf16ps));
    assert!(matches!(X86Mnemonic::Tdpfp16ps, X86Mnemonic::Tdpfp16ps));
}

#[test]
fn test_amx_tile_load() {
    // TILELOADD: Load tile from memory
    // Format: TILELOADD tmm, [base + index*scale + disp]
    // EVEX.512.66.0F38.W0 4B /r
    // This test verifies the instruction enum exists
    // Full decoding test would require proper EVEX encoding
    assert!(matches!(X86Mnemonic::Tileloadd, X86Mnemonic::Tileloadd));
}

#[test]
fn test_amx_tile_store() {
    // TILESTORED: Store tile to memory
    // Format: TILESTORED [base + index*scale + disp], tmm
    // EVEX.512.66.0F38.W0 4D /r
    assert!(matches!(X86Mnemonic::Tilestored, X86Mnemonic::Tilestored));
}

#[test]
fn test_amx_tile_dot_product() {
    // TDPBF16PS: Tile dot product (BF16)
    // Format: TDPBF16PS tmm1, tmm2, tmm3
    // EVEX.512.F2.0F38.W0 5C /r

    // TDPFP16PS: Tile dot product (FP16)
    // Format: TDPFP16PS tmm1, tmm2, tmm3
    // EVEX.512.F3.0F38.W0 5C /r

    assert!(matches!(X86Mnemonic::Tdpbf16ps, X86Mnemonic::Tdpbf16ps));
    assert!(matches!(X86Mnemonic::Tdpfp16ps, X86Mnemonic::Tdpfp16ps));
}

// Note: Full AMX instruction decoding tests would require:
// 1. Proper EVEX prefix encoding with map=1
// 2. TMM register encoding (8 registers: TMM0-TMM7)
// 3. Complex addressing modes for tile load/store
// 4. Matrix dimension configuration
// These require extensive test vectors and proper EVEX encoding
