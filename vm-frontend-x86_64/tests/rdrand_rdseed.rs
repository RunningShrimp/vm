//! RDRAND/RDSEED 指令测试套件
//!
//! 测试Intel随机数生成指令的解码和语义实现

use vm_core::{Decoder, GuestAddr, MMU, VmError};
use vm_frontend_x86_64::X86Decoder;
use vm_ir::IRBlock;

/// 简单的测试MMU实现
struct TestMMU {
    memory: Vec<u8>,
}

impl TestMMU {
    fn new() -> Self {
        Self {
            memory: vec![0u8; 0x10000],
        }
    }
}

impl MMU for TestMMU {
    fn read(&self, addr: GuestAddr, size: usize) -> Result<Vec<u8>, VmError> {
        let start = addr as usize;
        let end = start + size;
        if end > self.memory.len() {
            return Err(VmError::from(vm_core::Fault::PageFault { vaddr: addr }));
        }
        Ok(self.memory[start..end].to_vec())
    }

    fn write(&mut self, addr: GuestAddr, data: &[u8]) -> Result<(), VmError> {
        let start = addr as usize;
        let end = start + data.len();
        if end > self.memory.len() {
            return Err(VmError::from(vm_core::Fault::PageFault { vaddr: addr }));
        }
        self.memory[start..end].copy_from_slice(data);
        Ok(())
    }
}

/// 测试RDRAND指令解码（64位）
#[test]
fn test_rdrand_decode_64bit() {
    let mut mmu = TestMMU::new();
    // RDRAND RAX: 0x48 0x0F 0xC7 0xF0
    // 0x48 = REX.W (64-bit)
    // 0x0F 0xC7 = RDRAND opcode
    // 0xF0 = ModR/M: mod=11 (register), reg=6 (RDRAND), rm=0 (RAX)
    mmu.write(0x1000, &[0x48, 0x0F, 0xC7, 0xF0]).unwrap();

    let mut decoder = X86Decoder::new();
    let result = decoder.decode_insn(&mmu, 0x1000);

    assert!(result.is_ok());
    let insn = result.unwrap();
    assert_eq!(insn.mnemonic(), "rdrand");
    assert_eq!(insn.op_size, 64);
}

/// 测试RDRAND指令解码（32位）
#[test]
fn test_rdrand_decode_32bit() {
    let mut mmu = TestMMU::new();
    // RDRAND EAX: 0x0F 0xC7 0xF0
    // 0x0F 0xC7 = RDRAND opcode
    // 0xF0 = ModR/M: mod=11 (register), reg=6 (RDRAND), rm=0 (EAX)
    mmu.write(0x1000, &[0x0F, 0xC7, 0xF0]).unwrap();

    let mut decoder = X86Decoder::new();
    let result = decoder.decode_insn(&mmu, 0x1000);

    assert!(result.is_ok());
    let insn = result.unwrap();
    assert_eq!(insn.mnemonic(), "rdrand");
    assert_eq!(insn.op_size, 32);
}

/// 测试RDRAND指令解码（16位）
#[test]
fn test_rdrand_decode_16bit() {
    let mut mmu = TestMMU::new();
    // RDRAND AX: 0x66 0x0F 0xC7 0xF0
    // 0x66 = operand size override (16-bit)
    // 0x0F 0xC7 = RDRAND opcode
    // 0xF0 = ModR/M: mod=11 (register), reg=6 (RDRAND), rm=0 (AX)
    mmu.write(0x1000, &[0x66, 0x0F, 0xC7, 0xF0]).unwrap();

    let mut decoder = X86Decoder::new();
    let result = decoder.decode_insn(&mmu, 0x1000);

    assert!(result.is_ok());
    let insn = result.unwrap();
    assert_eq!(insn.mnemonic(), "rdrand");
    assert_eq!(insn.op_size, 16);
}

/// 测试RDSEED指令解码（64位）
#[test]
fn test_rdseed_decode_64bit() {
    let mut mmu = TestMMU::new();
    // RDSEED RAX: 0x48 0x0F 0xC7 0xF8
    // 0x48 = REX.W (64-bit)
    // 0x0F 0xC7 = RDSEED opcode
    // 0xF8 = ModR/M: mod=11 (register), reg=7 (RDSEED), rm=0 (RAX)
    mmu.write(0x1000, &[0x48, 0x0F, 0xC7, 0xF8]).unwrap();

    let mut decoder = X86Decoder::new();
    let result = decoder.decode_insn(&mmu, 0x1000);

    assert!(result.is_ok());
    let insn = result.unwrap();
    assert_eq!(insn.mnemonic(), "rdseed");
    assert_eq!(insn.op_size, 64);
}

/// 测试RDSEED指令解码（32位）
#[test]
fn test_rdseed_decode_32bit() {
    let mut mmu = TestMMU::new();
    // RDSEED EAX: 0x0F 0xC7 0xF8
    // 0x0F 0xC7 = RDSEED opcode
    // 0xF8 = ModR/M: mod=11 (register), reg=7 (RDSEED), rm=0 (EAX)
    mmu.write(0x1000, &[0x0F, 0xC7, 0xF8]).unwrap();

    let mut decoder = X86Decoder::new();
    let result = decoder.decode_insn(&mmu, 0x1000);

    assert!(result.is_ok());
    let insn = result.unwrap();
    assert_eq!(insn.mnemonic(), "rdseed");
    assert_eq!(insn.op_size, 32);
}

/// 测试RDRAND到不同寄存器
#[test]
fn test_rdrand_different_registers() {
    let mut mmu = TestMMU::new();
    // RDRAND RCX: 0x48 0x0F 0xC7 0xF1
    // rm=1 (RCX)
    mmu.write(0x1000, &[0x48, 0x0F, 0xC7, 0xF1]).unwrap();

    let mut decoder = X86Decoder::new();
    let result = decoder.decode_insn(&mmu, 0x1000);

    assert!(result.is_ok());
    let insn = result.unwrap();
    assert_eq!(insn.mnemonic(), "rdrand");
}

/// 测试RDSEED到不同寄存器
#[test]
fn test_rdseed_different_registers() {
    let mut mmu = TestMMU::new();
    // RDSEED RDX: 0x48 0x0F 0xC7 0xFA
    // rm=2 (RDX)
    mmu.write(0x1000, &[0x48, 0x0F, 0xC7, 0xFA]).unwrap();

    let mut decoder = X86Decoder::new();
    let result = decoder.decode_insn(&mmu, 0x1000);

    assert!(result.is_ok());
    let insn = result.unwrap();
    assert_eq!(insn.mnemonic(), "rdseed");
}

/// 测试无效的RDRAND/RDSEED操作码
#[test]
fn test_invalid_rdrand_opcode() {
    let mut mmu = TestMMU::new();
    // Invalid: 0x0F 0xC7 0xF0 with reg=5 (not 6 or 7)
    mmu.write(0x1000, &[0x0F, 0xC7, 0xE8]).unwrap();

    let mut decoder = X86Decoder::new();
    let result = decoder.decode_insn(&mmu, 0x1000);

    // Should fail because reg=5 is not RDRAND (6) or RDSEED (7)
    assert!(result.is_err());
}

/// 测试RDRAND/RDSEED必须使用寄存器模式
#[test]
fn test_rdrand_must_be_register_mode() {
    let mut mmu = TestMMU::new();
    // Invalid: mod=00 (memory mode) instead of mod=11 (register mode)
    // 0x0F 0xC7 0x00 = mod=00, reg=0, rm=0 (memory mode, invalid)
    mmu.write(0x1000, &[0x0F, 0xC7, 0x00]).unwrap();

    let mut decoder = X86Decoder::new();
    let result = decoder.decode_insn(&mmu, 0x1000);

    // Should fail because RDRAND/RDSEED must use register mode (mod=11)
    assert!(result.is_err());
}

/// 测试RDRAND IR生成
#[test]
fn test_rdrand_ir_generation() {
    let mut mmu = TestMMU::new();
    // RDRAND RAX: 0x48 0x0F 0xC7 0xF0
    mmu.write(0x1000, &[0x48, 0x0F, 0xC7, 0xF0]).unwrap();

    let mut decoder = X86Decoder::new();
    let result = decoder.decode(&mmu, 0x1000);

    assert!(result.is_ok());
    let block = result.unwrap();

    // Check that IR block contains operations
    assert!(!block.ops.is_empty());

    // Verify that the block accesses the special random number region (0xF0003000)
    let has_random_access = block.ops.iter().any(|op| match op {
        vm_ir::IROp::MovImm { imm, .. } => *imm == 0xF0003000,
        _ => false,
    });
    assert!(has_random_access, "IR should access random number region");
}

/// 测试RDSEED IR生成
#[test]
fn test_rdseed_ir_generation() {
    let mut mmu = TestMMU::new();
    // RDSEED RAX: 0x48 0x0F 0xC7 0xF8
    mmu.write(0x1000, &[0x48, 0x0F, 0xC7, 0xF8]).unwrap();

    let mut decoder = X86Decoder::new();
    let result = decoder.decode(&mmu, 0x1000);

    assert!(result.is_ok());
    let block = result.unwrap();

    // Check that IR block contains operations
    assert!(!block.ops.is_empty());

    // Verify that the block accesses the special seed region (0xF0003008)
    let has_seed_access = block.ops.iter().any(|op| match op {
        vm_ir::IROp::MovImm { imm, .. } => *imm == 0xF0003008,
        _ => false,
    });
    assert!(has_seed_access, "IR should access random seed region");
}
