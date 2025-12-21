use vm_core::{Decoder, GuestAddr, GuestArch, MMU, VmError};
use vm_frontend_arm64::Arm64Decoder;
use vm_frontend_riscv64::RiscvDecoder;
use vm_frontend_x86_64::X86Decoder;
use vm_ir::IRBlock;

/// 统一的服务解码器枚举
pub enum ServiceDecoder {
    Riscv64(RiscvDecoder),
    Arm64(Arm64Decoder),
    X86_64(X86Decoder),
}

/// 服务指令包装器，适配前端解码器的指令类型
pub struct ServiceInstruction {
    /// 内部指令指针，使用动态类型存储不同架构的指令
    inner: Box<dyn InstructionTrait>,
}

/// 指令trait，定义前端解码器指令需要实现的方法
pub trait InstructionTrait {
    fn next_pc(&self) -> GuestAddr;
    fn size(&self) -> u8;
    fn operand_count(&self) -> usize;
    fn mnemonic(&self) -> &str;
    fn is_control_flow(&self) -> bool;
    fn is_memory_access(&self) -> bool;
}

impl InstructionTrait for vm_frontend_arm64::Arm64Instruction {
    fn next_pc(&self) -> GuestAddr {
        self.next_pc()
    }
    fn size(&self) -> u8 {
        self.size()
    }
    fn operand_count(&self) -> usize {
        self.operand_count()
    }
    fn mnemonic(&self) -> &str {
        self.mnemonic()
    }
    fn is_control_flow(&self) -> bool {
        self.is_branch
    }
    fn is_memory_access(&self) -> bool {
        self.has_memory_op
    }
}

impl InstructionTrait for vm_frontend_riscv64::RiscvInstruction {
    fn next_pc(&self) -> GuestAddr {
        self.next_pc()
    }
    fn size(&self) -> u8 {
        self.size()
    }
    fn operand_count(&self) -> usize {
        self.operand_count()
    }
    fn mnemonic(&self) -> &str {
        self.mnemonic()
    }
    fn is_control_flow(&self) -> bool {
        self.is_branch
    }
    fn is_memory_access(&self) -> bool {
        self.has_memory_op
    }
}

impl InstructionTrait for vm_core::Instruction {
    fn next_pc(&self) -> GuestAddr {
        // vm_core::Instruction 没有next_pc方法，这里返回占位符
        GuestAddr(0)
    }
    fn size(&self) -> u8 {
        self.length as u8
    }
    fn operand_count(&self) -> usize {
        self.operands.len()
    }
    fn mnemonic(&self) -> &str {
        // 根据opcode返回助记符，这里简化实现
        match self.opcode {
            0x90 => "nop",
            _ => "unknown",
        }
    }
    fn is_control_flow(&self) -> bool {
        // 简化实现：某些opcode被认为是控制流
        matches!(self.opcode, 0xE8 | 0xE9 | 0xEB | 0x74 | 0x75)
    }
    fn is_memory_access(&self) -> bool {
        // 简化实现：某些opcode被认为是内存访问
        matches!(self.opcode, 0x8B | 0x89 | 0xA1 | 0xA3)
    }
}

impl ServiceInstruction {
    /// 创建新的服务指令
    pub fn new<I: InstructionTrait + 'static>(instruction: I) -> Self {
        ServiceInstruction {
            inner: Box::new(instruction),
        }
    }

    /// 获取下一条指令的地址
    pub fn next_pc(&self) -> GuestAddr {
        self.inner.next_pc()
    }

    /// 获取指令大小
    pub fn size(&self) -> u8 {
        self.inner.size()
    }
}

impl Decoder for ServiceDecoder {
    type Instruction = ServiceInstruction;
    type Block = IRBlock;

    fn decode_insn(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> Result<Self::Instruction, VmError> {
        match self {
            ServiceDecoder::Riscv64(d) => {
                let insn = d.decode_insn(mmu, pc)?;
                Ok(ServiceInstruction::new(insn))
            }
            ServiceDecoder::Arm64(d) => {
                let insn = d.decode_insn(mmu, pc)?;
                Ok(ServiceInstruction::new(insn))
            }
            ServiceDecoder::X86_64(d) => {
                let insn = d.decode_insn(mmu, pc)?;
                Ok(ServiceInstruction::new(insn))
            }
        }
    }

    fn decode(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> Result<Self::Block, VmError> {
        match self {
            ServiceDecoder::Riscv64(d) => d.decode(mmu, pc),
            ServiceDecoder::Arm64(d) => d.decode(mmu, pc),
            ServiceDecoder::X86_64(d) => d.decode(mmu, pc),
        }
    }
}

/// 解码器工厂
pub struct DecoderFactory;

impl DecoderFactory {
    pub fn create(arch: GuestArch) -> ServiceDecoder {
        match arch {
            GuestArch::Riscv64 => ServiceDecoder::Riscv64(RiscvDecoder),
            GuestArch::Arm64 => ServiceDecoder::Arm64(Arm64Decoder::new()),
            GuestArch::X86_64 => ServiceDecoder::X86_64(X86Decoder::new()),
        }
    }
}
