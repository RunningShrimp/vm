//! x86-64 解码器 - 模块化设计
//! 分离为：前缀解码、操作码解码、操作数解码、指令转换

use vm_core::{Decoder, GuestAddr, Fault, MMU};
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator, MemFlags};

/// 指令流读取器
pub struct InsnStream<'a> {
    mmu: &'a dyn MMU,
    pc: GuestAddr,
}

impl<'a> InsnStream<'a> {
    pub fn new(mmu: &'a dyn MMU, pc: GuestAddr) -> Self {
        Self { mmu, pc }
    }

    pub fn read_u8(&mut self) -> Result<u8, Fault> {
        let v = self.mmu.read(self.pc, 1)? as u8;
        self.pc += 1;
        Ok(v)
    }

    pub fn read_u16(&mut self) -> Result<u16, Fault> {
        let v = self.mmu.read(self.pc, 2)? as u16;
        self.pc += 2;
        Ok(v)
    }

    pub fn read_u32(&mut self) -> Result<u32, Fault> {
        let v = self.mmu.read(self.pc, 4)? as u32;
        self.pc += 4;
        Ok(v)
    }

    pub fn read_u64(&mut self) -> Result<u64, Fault> {
        let v = self.mmu.read(self.pc, 8)?;
        self.pc += 8;
        Ok(v)
    }

    pub fn current_pc(&self) -> GuestAddr {
        self.pc
    }
}

/// 前缀信息
#[derive(Debug, Default, Clone)]
pub struct PrefixInfo {
    pub lock: bool,
    pub rep: bool,
    pub repne: bool,
    pub seg: Option<u8>,
    pub op_size: bool, // 0x66
    pub addr_size: bool, // 0x67
    pub rex: Option<u8>,
}

/// 前缀解码器
pub fn decode_prefixes(stream: &mut InsnStream) -> Result<(PrefixInfo, u8), Fault> {
    let mut prefix = PrefixInfo::default();

    loop {
        let b = stream.read_u8()?;
        match b {
            0xF0 => prefix.lock = true,
            0xF2 => prefix.repne = true,
            0xF3 => prefix.rep = true,
            0x2E | 0x36 | 0x3E | 0x26 | 0x64 | 0x65 => prefix.seg = Some(b),
            0x66 => prefix.op_size = true,
            0x67 => prefix.addr_size = true,
            0x40..=0x4F => {
                prefix.rex = Some(b);
                let opcode = stream.read_u8()?;
                return Ok((prefix, opcode));
            }
            _ => return Ok((prefix, b)),
        }
    }
}

/// 操作码和操作数配置
#[derive(Debug, Clone, Copy)]
pub enum OpKind {
    None,
    Reg,
    Rm,
    Imm,
    Rel,
    OpReg,
    XmmReg,
    XmmRm,
    Imm8,
}

/// 指令信息
#[derive(Debug, Clone)]
pub struct InsnInfo {
    pub is_two_byte: bool,
    pub opcode: u8,
    pub op_kinds: (OpKind, OpKind, OpKind),
}

/// 操作码解码器 - 返回指令信息
pub fn decode_opcode(opcode: u8, stream: &mut InsnStream) -> Result<InsnInfo, Fault> {
    let (is_two_byte, final_opcode) = if opcode == 0x0F {
        (true, stream.read_u8()?)
    } else {
        (false, opcode)
    };

    let op_kinds = if is_two_byte {
        match final_opcode {
            0x05 => (OpKind::None, OpKind::None, OpKind::None),          // syscall
            0xA2 => (OpKind::None, OpKind::None, OpKind::None),          // cpuid
            0x28 => (OpKind::XmmReg, OpKind::XmmRm, OpKind::None),       // movaps
            0x58 => (OpKind::XmmReg, OpKind::XmmRm, OpKind::None),       // addps
            0x80..=0x8F => (OpKind::Rel, OpKind::None, OpKind::None),    // jcc rel32
            _ => return Err(Fault::InvalidOpcode { pc: 0, opcode: 0 }),
        }
    } else {
        match final_opcode {
            0x90 => (OpKind::None, OpKind::None, OpKind::None),         // nop
            0x89 => (OpKind::Rm, OpKind::Reg, OpKind::None),             // mov
            0x8B => (OpKind::Reg, OpKind::Rm, OpKind::None),             // mov
            0xC3 => (OpKind::None, OpKind::None, OpKind::None),         // ret
            0xEB => (OpKind::Rel, OpKind::None, OpKind::None),           // jmp rel8
            0xE9 => (OpKind::Rel, OpKind::None, OpKind::None),           // jmp rel32
            0xF4 => (OpKind::None, OpKind::None, OpKind::None),         // hlt
            _ => return Err(Fault::InvalidOpcode { pc: 0, opcode: 0 }),
        }
    };

    Ok(InsnInfo {
        is_two_byte,
        opcode: final_opcode,
        op_kinds,
    })
}

/// 新的 x86 解码器实现
pub struct X86Decoder;

impl Decoder for X86Decoder {
    type Block = IRBlock;

    fn decode(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> Result<Self::Block, Fault> {
        let mut builder = IRBuilder::new(pc);
        let mut stream = InsnStream::new(mmu, pc);

        // 阶段 1：解码前缀
        let (_prefix, opcode) = decode_prefixes(&mut stream)?;

        // 阶段 2：解码操作码
        let insn_info = decode_opcode(opcode, &mut stream)?;

        // 阶段 3：处理 ModR/M 和操作数 (简化版本)
        // 实际实现中这里会根据 insn_info.op_kinds 解码操作数

        // 阶段 4：转换为 IR
        match opcode {
            0x90 => builder.push(IROp::Nop),
            0xF4 => builder.set_term(Terminator::Fault { cause: 0 }),
            0xC3 => builder.set_term(Terminator::Ret),
            _ => {}
        }

        Ok(builder.build())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockMMU {
        data: Vec<u8>,
    }

    impl MMU for MockMMU {
        fn translate(&mut self, va: vm_core::GuestAddr, _access: vm_core::AccessType) -> Result<vm_core::GuestPhysAddr, Fault> {
            Ok(va)
        }
        fn fetch_insn(&self, pc: vm_core::GuestAddr) -> Result<u64, Fault> {
            self.read(pc, 8)
        }
        fn read(&self, addr: vm_core::GuestAddr, size: u8) -> Result<u64, Fault> {
            let offset = addr as usize;
            if offset + size as usize > self.data.len() {
                return Err(Fault::PageFault { addr, access: vm_core::AccessType::Read });
            }
            let mut val = 0u64;
            for i in 0..size as usize {
                val |= (self.data[offset + i] as u64) << (i * 8);
            }
            Ok(val)
        }
        fn write(&mut self, _addr: vm_core::GuestAddr, _val: u64, _size: u8) -> Result<(), Fault> {
            Ok(())
        }
        fn map_mmio(&mut self, _base: u64, _size: u64, _device: Box<dyn vm_core::MmioDevice>) {}
        fn flush_tlb(&mut self) {}
        fn memory_size(&self) -> usize {
            self.data.len()
        }
        fn dump_memory(&self) -> Vec<u8> {
            self.data.clone()
        }
        fn restore_memory(&mut self, data: &[u8]) -> Result<(), String> {
            Ok(())
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    #[test]
    fn test_nop_decode() {
        let mmu = MockMMU { data: vec![0x90] };
        let mut decoder = X86Decoder;
        let block = decoder.decode(&mmu, 0x1000)
            .expect("Failed to decode NOP");
        assert!(matches!(block.ops[0], IROp::Nop));
    }

    #[test]
    fn test_hlt_decode() {
        let mmu = MockMMU { data: vec![0xF4] };
        let mut decoder = X86Decoder;
        let block = decoder.decode(&mmu, 0x1000)
            .expect("Failed to decode HLT");
        assert!(matches!(block.term, Terminator::Fault { .. }));
    }
}
