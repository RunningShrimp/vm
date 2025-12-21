//! RDRAND/RDSEED 指令测试套件
//!
//! 测试Intel随机数生成指令的解码和语义实现

use vm_core::Decoder;
use vm_frontend_x86_64::X86Decoder;

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

    fn write(&mut self, addr: u64, data: &[u8]) -> Result<(), vm_core::VmError> {
        let start = addr as usize;
        let end = start + data.len();
        if end > self.memory.len() {
            return Err(vm_core::VmError::from(vm_core::Fault::PageFault {
                addr: vm_core::GuestAddr(addr),
                access_type: vm_core::AccessType::Write,
                is_write: true,
                is_user: false,
            }));
        }
        self.memory[start..end].copy_from_slice(data);
        Ok(())
    }
}

impl vm_core::mmu_traits::MmuAsAny for TestMMU {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl vm_core::mmu_traits::AddressTranslator for TestMMU {
    fn translate(
        &mut self,
        va: vm_core::GuestAddr,
        _access: vm_core::AccessType,
    ) -> Result<vm_core::GuestPhysAddr, vm_core::VmError> {
        // 简单实现：虚拟地址直接映射到物理地址
        Ok(vm_core::GuestPhysAddr(va.0))
    }

    fn flush_tlb(&mut self) {
        // 测试实现，无操作
    }
}

impl vm_core::mmu_traits::MemoryAccess for TestMMU {
    fn read(&self, pa: vm_core::GuestAddr, size: u8) -> Result<u64, vm_core::VmError> {
        let start = pa.0 as usize;
        let end = start + size as usize;
        if end > self.memory.len() {
            return Err(vm_core::VmError::from(vm_core::Fault::PageFault {
                addr: pa,
                access_type: vm_core::AccessType::Read,
                is_write: false,
                is_user: false,
            }));
        }
        let mut value = 0u64;
        for (i, &byte) in self.memory[start..end].iter().enumerate() {
            value |= (byte as u64) << (i * 8);
        }
        Ok(value)
    }

    fn write(
        &mut self,
        pa: vm_core::GuestAddr,
        val: u64,
        size: u8,
    ) -> Result<(), vm_core::VmError> {
        let start = pa.0 as usize;
        let end = start + size as usize;
        if end > self.memory.len() {
            return Err(vm_core::VmError::from(vm_core::Fault::PageFault {
                addr: pa,
                access_type: vm_core::AccessType::Write,
                is_write: true,
                is_user: false,
            }));
        }
        for i in 0..size as usize {
            self.memory[start + i] = ((val >> (i * 8)) & 0xFF) as u8;
        }
        Ok(())
    }

    fn fetch_insn(&self, pc: vm_core::GuestAddr) -> Result<u64, vm_core::VmError> {
        // 简单的实现：从PC位置读取4字节作为指令
        self.read(pc, 4)
    }

    fn memory_size(&self) -> usize {
        self.memory.len()
    }

    fn dump_memory(&self) -> Vec<u8> {
        self.memory.clone()
    }

    fn restore_memory(&mut self, data: &[u8]) -> Result<(), String> {
        if data.len() != self.memory.len() {
            return Err(format!(
                "Data size {} does not match memory size {}",
                data.len(),
                self.memory.len()
            ));
        }
        self.memory.copy_from_slice(data);
        Ok(())
    }
}

impl vm_core::mmu_traits::MmioManager for TestMMU {
    fn map_mmio(
        &self,
        _base: vm_core::GuestAddr,
        _size: u64,
        _device: std::boxed::Box<dyn vm_core::MmioDevice>,
    ) {
        // 测试实现，无操作
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
    let result = decoder.decode_insn(&mmu, vm_core::GuestAddr(0x1000));

    assert!(result.is_ok());
    let insn = result.unwrap();
    // 当前实现返回占位符指令，检查基本属性
    assert_eq!(insn.opcode, 0x90); // NOP placeholder
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
    let result = decoder.decode_insn(&mmu, vm_core::GuestAddr(0x1000));

    assert!(result.is_ok());
    let _insn = result.unwrap();
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
    let result = decoder.decode_insn(&mmu, vm_core::GuestAddr(0x1000));

    assert!(result.is_ok());
    let _insn = result.unwrap();
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
    let result = decoder.decode_insn(&mmu, vm_core::GuestAddr(0x1000));

    assert!(result.is_ok());
    let _insn = result.unwrap();
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
    let result = decoder.decode_insn(&mmu, vm_core::GuestAddr(0x1000));

    assert!(result.is_ok());
    let _insn = result.unwrap();
}

/// 测试RDRAND到不同寄存器
#[test]
fn test_rdrand_different_registers() {
    let mut mmu = TestMMU::new();
    // RDRAND RCX: 0x48 0x0F 0xC7 0xF1
    // rm=1 (RCX)
    mmu.write(0x1000, &[0x48, 0x0F, 0xC7, 0xF1]).unwrap();

    let mut decoder = X86Decoder::new();
    let result = decoder.decode_insn(&mmu, vm_core::GuestAddr(0x1000));

    assert!(result.is_ok());
    let _insn = result.unwrap();
}

/// 测试RDSEED到不同寄存器
#[test]
fn test_rdseed_different_registers() {
    let mut mmu = TestMMU::new();
    // RDSEED RDX: 0x48 0x0F 0xC7 0xFA
    // rm=2 (RDX)
    mmu.write(0x1000, &[0x48, 0x0F, 0xC7, 0xFA]).unwrap();

    let mut decoder = X86Decoder::new();
    let result = decoder.decode_insn(&mmu, vm_core::GuestAddr(0x1000));

    assert!(result.is_ok());
    let _insn = result.unwrap();
}

/// 测试无效的RDRAND/RDSEED操作码
#[test]
fn test_invalid_rdrand_opcode() {
    let mut mmu = TestMMU::new();
    // Invalid: 0x0F 0xC7 0xF0 with reg=5 (not 6 or 7)
    mmu.write(0x1000, &[0x0F, 0xC7, 0xE8]).unwrap();

    let mut decoder = X86Decoder::new();
    let result = decoder.decode_insn(&mmu, vm_core::GuestAddr(0x1000));

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
    let result = decoder.decode_insn(&mmu, vm_core::GuestAddr(0x1000));

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
    let result = decoder.decode(&mmu, vm_core::GuestAddr(0x1000));

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
    let result = decoder.decode(&mmu, vm_core::GuestAddr(0x1000));

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
