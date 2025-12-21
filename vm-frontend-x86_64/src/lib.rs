pub mod decoder_pipeline;
mod extended_insns;
pub mod opcode_decode;
pub mod operand_decode;
pub mod prefix_decode;
pub mod x86_regs;

/// x86_64 指令助记符
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86Mnemonic {
    Nop,
    Ret,
    Jmp,
    Mov,
    Push,
    Pop,
    Add,
    Sub,
    Inc,
    Dec,
    Cmp,
    Lea,
    // 其他助记符...
    Unknown,
}

/// x86_64 指令操作数
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum X86Operand {
    None,
    Reg(u8),
    Mem {
        base: Option<u8>,
        index: Option<u8>,
        scale: u8,
        disp: i64,
    },
    Imm(i64),
    Rel(i64),
    Xmm(u8),
}

use crate::extended_insns::ExtendedDecoder;
pub use decoder_pipeline::{DecoderPipeline, InsnStream};
use vm_core::{Decoder, GuestAddr, Instruction as CoreInstruction, MMU, VmError, VmResult};
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator};

/// x86_64 指令表示
#[derive(Debug, Clone)]
pub struct X86Instruction {
    pub mnemonic: &'static str,
    pub op1: X86Operand,
    pub op2: X86Operand,
    pub op3: X86Operand,
    pub op_size: u8,
    pub lock: bool,
    pub rep: bool,
    pub repne: bool,
    pub next_pc: GuestAddr,
    pub jcc_cc: Option<u8>,
    pub has_memory_op: bool,
    pub is_branch: bool,
    /// 指令字节码
    pub bytes: Vec<u8>,
    /// 操作数信息
    pub operands: Vec<String>,
}

impl X86Instruction {
    /// 创建新指令
    pub fn new(mnemonic: &'static str, pc: GuestAddr, length: usize) -> Self {
        Self {
            mnemonic,
            op1: X86Operand::None,
            op2: X86Operand::None,
            op3: X86Operand::None,
            op_size: 32,
            lock: false,
            rep: false,
            repne: false,
            next_pc: GuestAddr(pc.0 + length as u64),
            jcc_cc: None,
            has_memory_op: false,
            is_branch: false,
            bytes: Vec::with_capacity(length),
            operands: Vec::new(),
        }
    }

    /// 添加内存操作标记
    pub fn with_memory_op(mut self) -> Self {
        self.has_memory_op = true;
        self
    }

    /// 添加分支标记
    pub fn with_branch(mut self) -> Self {
        self.is_branch = true;
        self
    }

    /// 添加字节码
    pub fn with_bytes(mut self, bytes: Vec<u8>) -> Self {
        self.bytes = bytes;
        self
    }

    /// 添加操作数
    pub fn with_operands(mut self, operands: Vec<String>) -> Self {
        self.operands = operands;
        self
    }

    /// 转换为 vm-core 的指令格式
    pub fn to_core_instruction(&self) -> CoreInstruction {
        CoreInstruction {
            opcode: self.bytes.first().copied().unwrap_or(0), // 第一个字节作为操作码
            operands: self.bytes.iter().map(|&b| b as u64).collect(),
            length: self.bytes.len(),
        }
    }
}

pub struct X86Decoder {
    extended_decoder: ExtendedDecoder,
}

impl Default for X86Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl X86Decoder {
    pub fn new() -> Self {
        let mut decoder = Self {
            extended_decoder: ExtendedDecoder::new(),
        };
        // 初始化CPU特性支持
        decoder.extended_decoder.set_cpu_features();
        decoder
    }

    /// 获取指令字节
    fn get_instruction_bytes(&self, mmu: &dyn MMU, pc: GuestAddr) -> Result<Vec<u8>, VmError> {
        // 简化实现，只读取前15字节（x86最大指令长度）
        let mut bytes = Vec::with_capacity(15);
        for i in 0..15 {
            let addr = GuestAddr(pc.0 + i as u64);
            let byte = mmu.read(addr, 1)? as u8;
            bytes.push(byte);
        }
        Ok(bytes)
    }

    /// 判断是否为扩展指令
    fn is_extended_instruction(&self, bytes: &[u8]) -> bool {
        // 简化实现，假设某些前缀表示扩展指令
        !bytes.is_empty() && (bytes[0] == 0x0F || bytes[0] == 0xC4 || bytes[0] == 0xC5)
    }

    /// 解析操作数
    fn parse_operands(&self, _bytes: &[u8]) -> Vec<String> {
        // 简化实现，返回空操作数列表
        Vec::new()
    }

    /// 设置CPU特性
    pub fn configure_cpu_features(&mut self, features: Vec<String>) {
        self.extended_decoder.set_cpu_features_by_list(features);
    }

    /// 检查是否支持特定特性
    pub fn supports_extension(&self, feature: &str) -> bool {
        self.extended_decoder.supports_feature(feature)
    }

    /// 重置解码器状态
    pub fn reset_state(&mut self) {
        self.extended_decoder.reset();
    }

    /// 获取支持的扩展列表
    pub fn get_extensions(&self) -> Vec<String> {
        self.extended_decoder.get_supported_extensions()
    }

    /// 检查是否是大端序
    pub fn is_big_endian(&self) -> bool {
        self.extended_decoder.is_big_endian()
    }
}

impl Decoder for X86Decoder {
    type Instruction = vm_core::Instruction;
    type Block = IRBlock;

    fn decode_insn(&mut self, _mmu: &dyn MMU, pc: GuestAddr) -> VmResult<Self::Instruction> {
        // 简化实现：创建一个基本的指令对象
        // 在实际实现中，这里会根据pc地址读取内存并解码指令
        let instruction = vm_core::Instruction {
            opcode: 0x90, // NOP as placeholder
            operands: Vec::new(),
            length: 1,
        };

        // 记录解码的地址用于调试
        println!("Decoding instruction at address: {:?}", pc);

        Ok(instruction)
    }

    fn decode(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> VmResult<Self::Block> {
        // 获取指令字节
        let bytes = self.get_instruction_bytes(mmu, pc)?;

        // 检查是否为扩展指令
        let is_extended = self.is_extended_instruction(&bytes);

        // 如果是扩展指令，使用扩展解码器
        if is_extended {
            let extended_instruction = self.extended_decoder.decode_extended(&bytes, pc)?;
            println!(
                "Decoded extended instruction: {} (len={}, modrm={}, imm={}, priv={}, operands={:?})",
                extended_instruction.mnemonic,
                extended_instruction.length,
                extended_instruction.has_modrm,
                extended_instruction.has_imm,
                extended_instruction.is_privileged,
                extended_instruction.get_operands()
            );

            // 检查CPU特性支持
            let mnemonic = extended_instruction.mnemonic;
            if self.extended_decoder.supports(mnemonic) {
                println!("CPU supports {}", mnemonic);
            } else {
                println!("CPU does not support {}", mnemonic);
            }
        }

        // 解码单条指令
        let instruction = self.decode_insn(mmu, pc)?;

        // 解析操作数（用于更精确的IR生成）
        let operands = self.parse_operands(&bytes);

        // 构建IR块
        let mut builder = IRBuilder::new(pc);

        // 根据指令操作码和特性选择合适的IROp
        let ir_op = match instruction.opcode {
            0x90 => IROp::Nop, // NOP
            0xC3 => {
                // RET - 这应该是终止符，不是操作
                builder.set_term(Terminator::Ret);
                IROp::Nop // 临时使用Nop
            }
            _ => {
                // 其他指令的简化处理
                if is_extended {
                    println!("Extended instruction with {} operands", operands.len());
                }
                println!("Unknown opcode: {:02x}, using Nop", instruction.opcode);
                IROp::Nop
            }
        };

        if instruction.opcode != 0xC3 {
            // 如果不是RET，添加操作并设置默认终止符
            builder.push(ir_op);
            builder.set_term(Terminator::Ret);
        }

        Ok(builder.build())
    }
}
