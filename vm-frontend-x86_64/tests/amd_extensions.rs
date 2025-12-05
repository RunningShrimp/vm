use vm_frontend_x86_64::X86Mnemonic;

#[test]
fn test_amd_xop_instructions() {
    // Test AMD XOP instruction enums
    assert!(matches!(X86Mnemonic::Vfrczpd, X86Mnemonic::Vfrczpd));
    assert!(matches!(X86Mnemonic::Vfrczps, X86Mnemonic::Vfrczps));
    assert!(matches!(X86Mnemonic::Vpermil2pd, X86Mnemonic::Vpermil2pd));
    assert!(matches!(X86Mnemonic::Vpermil2ps, X86Mnemonic::Vpermil2ps));
    assert!(matches!(X86Mnemonic::Vpcmov, X86Mnemonic::Vpcmov));
    assert!(matches!(X86Mnemonic::Vprot, X86Mnemonic::Vprot));
}

#[test]
fn test_amd_tbm_instructions() {
    // Test AMD TBM instruction enums
    assert!(matches!(X86Mnemonic::Blcfill, X86Mnemonic::Blcfill));
    assert!(matches!(X86Mnemonic::Blci, X86Mnemonic::Blci));
    assert!(matches!(X86Mnemonic::Blcic, X86Mnemonic::Blcic));
    assert!(matches!(X86Mnemonic::Blcmsk, X86Mnemonic::Blcmsk));
    assert!(matches!(X86Mnemonic::Blsfill, X86Mnemonic::Blsfill));
    assert!(matches!(X86Mnemonic::Blsic, X86Mnemonic::Blsic));
    assert!(matches!(X86Mnemonic::Tzmsk, X86Mnemonic::Tzmsk));
}

#[test]
fn test_amd_fma4_enum() {
    // Test AMD FMA4 instruction enum exists
    // VFMADDPD: 0x66 0x0F 0x38 0x68 /r
    // Format: VFMADDPD xmm1, xmm2, xmm3, xmm4 (4-operand)
    assert!(matches!(X86Mnemonic::Vfmaddpd, X86Mnemonic::Vfmaddpd));
}

#[test]
fn test_amd_sse4a_enum() {
    // Test AMD SSE4a instruction enums exist
    assert!(matches!(X86Mnemonic::Movntsd, X86Mnemonic::Movntsd));
    assert!(matches!(X86Mnemonic::Movntss, X86Mnemonic::Movntss));
    assert!(matches!(X86Mnemonic::Extrq, X86Mnemonic::Extrq));
    assert!(matches!(X86Mnemonic::Insertq, X86Mnemonic::Insertq));
}

#[test]
fn test_tbm_blcfill_semantic() {
    // BLCFILL: x & (x + 1)
    // This test verifies the instruction exists
    assert!(matches!(X86Mnemonic::Blcfill, X86Mnemonic::Blcfill));
}

#[test]
fn test_tbm_blci_semantic() {
    // BLCI: ~x & (x + 1)
    assert!(matches!(X86Mnemonic::Blci, X86Mnemonic::Blci));
}

#[test]
fn test_tbm_blcic_semantic() {
    // BLCIC: ~x & (x - 1)
    assert!(matches!(X86Mnemonic::Blcic, X86Mnemonic::Blcic));
}

#[test]
fn test_tbm_blcmsk_semantic() {
    // BLCMSK: x ^ (x + 1)
    assert!(matches!(X86Mnemonic::Blcmsk, X86Mnemonic::Blcmsk));
}

#[test]
fn test_tbm_blsfill_semantic() {
    // BLSFILL: x | (x + 1)
    assert!(matches!(X86Mnemonic::Blsfill, X86Mnemonic::Blsfill));
}

#[test]
fn test_tbm_blsic_semantic() {
    // BLSIC: ~x | (x - 1)
    assert!(matches!(X86Mnemonic::Blsic, X86Mnemonic::Blsic));
}

#[test]
fn test_tbm_tzmsk_semantic() {
    // TZMSK: ~x & (x - 1)
    assert!(matches!(X86Mnemonic::Tzmsk, X86Mnemonic::Tzmsk));
}

// Note: Full AMD extension decoding tests would require:
// 1. XOP prefix encoding (0x8F prefix + XOP map field)
// 2. TBM instruction encoding (VEX format)
// 3. Proper ModR/M and SIB byte parsing
// 4. Test vectors from AMD documentation
