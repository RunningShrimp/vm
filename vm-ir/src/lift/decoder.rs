//! 指令解码器 - 支持 x86-64/ARM64/RISC-V
//!
//! 利用 Capstone 库进行多 ISA 指令解码，输出规范化的指令表示。

use crate::lift::LiftResult;
use std::fmt;
use vm_core::{CoreError, VmError};

/// 支持的 ISA 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ISA {
    /// x86-64 (Intel/AMD)
    X86_64,
    /// ARM64 (ARM)
    ARM64,
    /// RISC-V 64-bit
    RISCV64,
}

/// 操作数类型
#[derive(Debug, Clone, PartialEq)]
pub enum OperandType {
    /// 寄存器操作数
    Register(String),
    /// 立即数操作数
    Immediate(i64),
    /// 内存操作数 (base + offset)
    Memory {
        base: Option<String>,
        offset: i64,
        size: usize,
    },
}

impl fmt::Display for OperandType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            OperandType::Register(name) => write!(f, "%{}", name),
            OperandType::Immediate(val) => write!(f, "${}", val),
            OperandType::Memory {
                base,
                offset,
                size: _,
            } => {
                write!(f, "{}(%{})", offset, base.as_deref().unwrap_or("?"))
            }
        }
    }
}

/// 规范化指令表示
#[derive(Debug, Clone)]
pub struct Instruction {
    /// 指令助记符 (e.g., "mov", "add", "jmp")
    pub mnemonic: String,
    /// 操作数列表
    pub operands: Vec<OperandType>,
    /// 指令长度（字节）
    pub length: usize,
    /// 隐式读取的寄存器（如 FLAGS）
    pub implicit_reads: Vec<String>,
    /// 隐式写入的寄存器（如 FLAGS）
    pub implicit_writes: Vec<String>,
    /// 是否为分支指令
    pub is_branch: bool,
    /// 是否为系统调用指令
    pub is_syscall: bool,
    /// 是否为内存操作（load/store）
    pub is_memory_op: bool,
    /// 原始指令字节
    pub bytes: Vec<u8>,
}

impl Instruction {
    /// 创建新的指令对象
    pub fn new(mnemonic: String, operands: Vec<OperandType>, length: usize) -> Self {
        Self {
            mnemonic,
            operands,
            length,
            implicit_reads: Vec::new(),
            implicit_writes: Vec::new(),
            is_branch: false,
            is_syscall: false,
            is_memory_op: false,
            bytes: Vec::new(),
        }
    }

    /// 标记隐式读取
    pub fn with_implicit_reads(mut self, reads: Vec<String>) -> Self {
        self.implicit_reads = reads;
        self
    }

    /// 标记隐式写入
    pub fn with_implicit_writes(mut self, writes: Vec<String>) -> Self {
        self.implicit_writes = writes;
        self
    }

    /// 标记为分支指令
    pub fn as_branch(mut self) -> Self {
        self.is_branch = true;
        self
    }

    /// 标记为系统调用
    pub fn as_syscall(mut self) -> Self {
        self.is_syscall = true;
        self
    }

    /// 标记为内存操作
    pub fn as_memory_op(mut self) -> Self {
        self.is_memory_op = true;
        self
    }

    /// 标记原始字节
    pub fn with_bytes(mut self, bytes: Vec<u8>) -> Self {
        self.bytes = bytes;
        self
    }
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.mnemonic)?;
        if !self.operands.is_empty() {
            write!(f, " ")?;
            for (i, op) in self.operands.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}", op)?;
            }
        }
        Ok(())
    }
}

/// 指令解码器 Trait
pub trait InstructionDecoder {
    /// 解码单条指令
    /// 返回 (Instruction, 消耗的字节数)
    fn decode(&self, bytes: &[u8]) -> LiftResult<(Instruction, usize)>;

    /// 解码指令序列
    fn decode_sequence(&self, bytes: &[u8], max_instrs: usize) -> LiftResult<Vec<Instruction>> {
        let mut instrs = Vec::new();
        let mut offset = 0;

        while offset < bytes.len() && instrs.len() < max_instrs {
            match self.decode(&bytes[offset..]) {
                Ok((instr, size)) => {
                    offset += size;
                    instrs.push(instr);
                }
                Err(_) => break,
            }
        }

        Ok(instrs)
    }
}

/// x86-64 指令解码器（基于 Capstone）
pub struct X86_64Decoder;

impl InstructionDecoder for X86_64Decoder {
    fn decode(&self, bytes: &[u8]) -> LiftResult<(Instruction, usize)> {
        if bytes.is_empty() {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "Empty instruction bytes".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        // 简化实现：基于常见 x86-64 指令字节进行识别
        // 完整实现应使用 Capstone FFI
        let mut instr = decode_x86_64_manual(bytes)?;
        let length = instr.length;
        instr.bytes = bytes[..length].to_vec();
        Ok((instr, length))
    }
}

/// x86-64 指令手工解码（简化版，真实实现应使用 Capstone）
fn decode_x86_64_manual(bytes: &[u8]) -> LiftResult<Instruction> {
    if bytes.is_empty() {
        return Err(VmError::Core(CoreError::DecodeError {
            message: "Empty bytes".to_string(),
            position: None,
            module: "vm-ir".to_string(),
        }));
    }

    let mut offset = 0;

    // Skip REX prefix (4x)
    let mut rex = false;
    if bytes[offset] >= 0x40 && bytes[offset] <= 0x4F {
        rex = true;
        offset += 1;
    }

    if offset >= bytes.len() {
        return Err(VmError::Core(CoreError::DecodeError {
            message: "Incomplete instruction".to_string(),
            position: None,
            module: "vm-ir".to_string(),
        }));
    }

    let opcode = bytes[offset];
    offset += 1;

    // 常见指令识别
    match opcode {
        0x50..=0x57 => {
            // PUSH rax-rdi
            let reg_idx = opcode - 0x50;
            let reg = get_x86_reg_name(reg_idx as usize, rex);
            Ok(
                Instruction::new("push".to_string(), vec![OperandType::Register(reg)], 1)
                    .with_implicit_writes(vec!["RSP".to_string()]),
            )
        }
        0x58..=0x5F => {
            // POP rax-rdi
            let reg_idx = opcode - 0x58;
            let reg = get_x86_reg_name(reg_idx as usize, rex);
            Ok(
                Instruction::new("pop".to_string(), vec![OperandType::Register(reg)], 1)
                    .with_implicit_reads(vec!["RSP".to_string()])
                    .with_implicit_writes(vec!["RSP".to_string()]),
            )
        }
        0x89 => {
            // MOV r64, r64 (ModR/M)
            if offset >= bytes.len() {
                return Err(VmError::Core(CoreError::DecodeError {
                    message: "Incomplete MOV instruction".to_string(),
                    position: None,
                    module: "vm-ir".to_string(),
                }));
            }
            let modrm = bytes[offset];
            let (dst, src, _) = decode_modrm_reg_reg(modrm, rex);
            offset += 1;
            Ok(Instruction::new(
                "mov".to_string(),
                vec![OperandType::Register(dst), OperandType::Register(src)],
                offset,
            ))
        }
        0x01 => {
            // ADD r64, r64 (ModR/M)
            if offset >= bytes.len() {
                return Err(VmError::Core(CoreError::DecodeError {
                    message: "Incomplete ADD instruction".to_string(),
                    position: None,
                    module: "vm-ir".to_string(),
                }));
            }
            let modrm = bytes[offset];
            let (dst, src, _) = decode_modrm_reg_reg(modrm, rex);
            offset += 1;
            Ok(Instruction::new(
                "add".to_string(),
                vec![OperandType::Register(dst), OperandType::Register(src)],
                offset,
            )
            .with_implicit_writes(vec!["FLAGS".to_string()]))
        }
        0x29 => {
            // SUB r64, r64 (ModR/M)
            if offset >= bytes.len() {
                return Err(VmError::Core(CoreError::DecodeError {
                    message: "Incomplete SUB instruction".to_string(),
                    position: None,
                    module: "vm-ir".to_string(),
                }));
            }
            let modrm = bytes[offset];
            let (dst, src, _) = decode_modrm_reg_reg(modrm, rex);
            offset += 1;
            Ok(Instruction::new(
                "sub".to_string(),
                vec![OperandType::Register(dst), OperandType::Register(src)],
                offset,
            )
            .with_implicit_writes(vec!["FLAGS".to_string()]))
        }
        0x90 => {
            // NOP
            Ok(Instruction::new("nop".to_string(), vec![], 1))
        }
        0xC3 => {
            // RET
            Ok(Instruction::new("ret".to_string(), vec![], 1)
                .as_branch()
                .with_implicit_reads(vec!["RSP".to_string()]))
        }
        0xFF => {
            // CALL/JMP indirect
            if offset >= bytes.len() {
                return Err(VmError::Core(CoreError::DecodeError {
                    message: "Incomplete FF instruction".to_string(),
                    position: None,
                    module: "vm-ir".to_string(),
                }));
            }
            let modrm = bytes[offset];
            let reg_opcode = (modrm >> 3) & 0x7;
            offset += 1;

            match reg_opcode {
                2 => {
                    // CALL r64
                    let (_, reg, _) = decode_modrm_reg_reg(modrm, rex);
                    Ok(Instruction::new(
                        "call".to_string(),
                        vec![OperandType::Register(reg)],
                        offset,
                    )
                    .as_branch()
                    .with_implicit_writes(vec!["RSP".to_string()]))
                }
                4 => {
                    // JMP r64
                    let (_, reg, _) = decode_modrm_reg_reg(modrm, rex);
                    Ok(Instruction::new(
                        "jmp".to_string(),
                        vec![OperandType::Register(reg)],
                        offset,
                    )
                    .as_branch())
                }
                _ => Err(VmError::Core(CoreError::DecodeError {
                    message: format!("Unsupported FF variant: {}", reg_opcode),
                    position: None,
                    module: "vm-ir".to_string(),
                })),
            }
        }
        _ => Err(VmError::Core(CoreError::DecodeError {
            message: format!("Unsupported x86-64 opcode: 0x{:02X}", opcode),
            position: None,
            module: "vm-ir".to_string(),
        })),
    }
}

/// 解码 ModR/M 字节中的两个寄存器
fn decode_modrm_reg_reg(modrm: u8, rex: bool) -> (String, String, u8) {
    let mod_bits = (modrm >> 6) & 0x3;
    let reg_bits = (modrm >> 3) & 0x7;
    let rm_bits = modrm & 0x7;

    let dst = get_x86_reg_name(reg_bits as usize, rex);
    let src = get_x86_reg_name(rm_bits as usize, rex);

    (dst, src, mod_bits)
}

/// 获取 x86-64 寄存器名称
fn get_x86_reg_name(index: usize, rex: bool) -> String {
    let names_no_rex = ["RAX", "RCX", "RDX", "RBX", "RSP", "RBP", "RSI", "RDI"];
    let names_rex = ["R8", "R9", "R10", "R11", "R12", "R13", "R14", "R15"];

    if rex && index >= 8 {
        names_rex[index - 8].to_string()
    } else if index < 8 {
        names_no_rex[index].to_string()
    } else {
        format!("REG{}", index)
    }
}

/// ARM64 指令解码器
pub struct ARM64Decoder;

impl InstructionDecoder for ARM64Decoder {
    fn decode(&self, bytes: &[u8]) -> LiftResult<(Instruction, usize)> {
        if bytes.len() < 4 {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "ARM64 instruction must be 4 bytes".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        // ARM64 is fixed 4-byte width
        let word = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);

        let mut instr = decode_arm64_manual(word)?;
        instr.bytes = bytes[0..4].to_vec();
        Ok((instr, 4))
    }
}

/// ARM64 指令手工解码（简化版）
fn decode_arm64_manual(word: u32) -> LiftResult<Instruction> {
    // 高 8 位用于指令分类
    // ARM64 NOP = 0xD503201F (hint instruction)

    // NOP 特殊检查
    if word == 0xD503201F {
        return Ok(Instruction::new("nop".to_string(), vec![], 4));
    }

    let top_bits = (word >> 24) & 0xFF;

    match top_bits {
        0x1F => {
            // NOP (hint instruction)
            Ok(Instruction::new("nop".to_string(), vec![], 4))
        }
        0x8B => {
            // ADD (Data Processing Immediate)
            let rd = (word & 0x1F) as usize;
            let rn = ((word >> 5) & 0x1F) as usize;
            Ok(Instruction::new(
                "add".to_string(),
                vec![
                    OperandType::Register(get_arm64_reg_name(rd)),
                    OperandType::Register(get_arm64_reg_name(rn)),
                    OperandType::Immediate((word >> 10) as i64 & 0xFFF),
                ],
                4,
            )
            .with_implicit_writes(vec!["NZCV".to_string()]))
        }
        _ => Err(VmError::Core(CoreError::DecodeError {
            message: format!("Unsupported ARM64 instruction: 0x{:08X}", word),
            position: None,
            module: "vm-ir".to_string(),
        })),
    }
}

/// 获取 ARM64 寄存器名称
fn get_arm64_reg_name(index: usize) -> String {
    match index {
        0..=30 => format!("X{}", index),
        31 => "SP".to_string(),
        _ => format!("REG{}", index),
    }
}

/// RISC-V 指令解码器
pub struct RISCV64Decoder;

impl InstructionDecoder for RISCV64Decoder {
    fn decode(&self, bytes: &[u8]) -> LiftResult<(Instruction, usize)> {
        if bytes.is_empty() {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "Empty bytes".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        // RISC-V 指令可能是 2 或 4 字节
        // 简化：都当作 4 字节
        if bytes.len() < 4 {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "Incomplete RISC-V instruction".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let word = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);

        let mut instr = decode_riscv_manual(word)?;
        instr.bytes = bytes[0..4].to_vec();
        Ok((instr, 4))
    }
}

/// RISC-V 指令手工解码（简化版）
fn decode_riscv_manual(word: u32) -> LiftResult<Instruction> {
    let opcode = word & 0x7F;

    match opcode {
        0x13 => {
            // ADDI
            let rd = ((word >> 7) & 0x1F) as usize;
            let rs1 = ((word >> 15) & 0x1F) as usize;
            let imm = ((word >> 20) as i32) as i64; // Sign-extend 12-bit immediate
            Ok(Instruction::new(
                "addi".to_string(),
                vec![
                    OperandType::Register(get_riscv_reg_name(rd)),
                    OperandType::Register(get_riscv_reg_name(rs1)),
                    OperandType::Immediate(imm),
                ],
                4,
            ))
        }
        0x33 => {
            // ADD
            let rd = ((word >> 7) & 0x1F) as usize;
            let rs1 = ((word >> 15) & 0x1F) as usize;
            let rs2 = ((word >> 20) & 0x1F) as usize;
            Ok(Instruction::new(
                "add".to_string(),
                vec![
                    OperandType::Register(get_riscv_reg_name(rd)),
                    OperandType::Register(get_riscv_reg_name(rs1)),
                    OperandType::Register(get_riscv_reg_name(rs2)),
                ],
                4,
            ))
        }
        _ => Err(VmError::Core(CoreError::DecodeError {
            message: format!("Unsupported RISC-V opcode: 0x{:02X}", opcode),
            position: None,
            module: "vm-ir".to_string(),
        })),
    }
}

/// 获取 RISC-V 寄存器名称
fn get_riscv_reg_name(index: usize) -> String {
    let abi_names = [
        "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "s0", "s1", "a0", "a1", "a2", "a3", "a4",
        "a5", "a6", "a7", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11", "t3", "t4",
        "t5", "t6",
    ];

    if index < abi_names.len() {
        format!("x{}({})", index, abi_names[index])
    } else {
        format!("x{}", index)
    }
}

/// 创建指定 ISA 的解码器
pub fn create_decoder(isa: ISA) -> Box<dyn InstructionDecoder> {
    match isa {
        ISA::X86_64 => Box::new(X86_64Decoder),
        ISA::ARM64 => Box::new(ARM64Decoder),
        ISA::RISCV64 => Box::new(RISCV64Decoder),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_x86_64_nop_decode() {
        let decoder = X86_64Decoder;
        let bytes = vec![0x90]; // NOP
        let (instr, size) = decoder.decode(&bytes).unwrap();
        assert_eq!(instr.mnemonic, "nop");
        assert_eq!(size, 1);
    }

    #[test]
    fn test_x86_64_push_decode() {
        let decoder = X86_64Decoder;
        let bytes = vec![0x50]; // PUSH RAX
        let (instr, size) = decoder.decode(&bytes).unwrap();
        assert_eq!(instr.mnemonic, "push");
        assert_eq!(size, 1);
        assert!(!instr.implicit_writes.is_empty());
    }

    #[test]
    fn test_arm64_nop_decode() {
        let decoder = ARM64Decoder;
        let bytes = vec![0x1F, 0x20, 0x03, 0xD5]; // NOP
        let (instr, size) = decoder.decode(&bytes).unwrap();
        assert_eq!(instr.mnemonic, "nop");
        assert_eq!(size, 4);
    }

    #[test]
    fn test_riscv_addi_decode() {
        let decoder = RISCV64Decoder;
        // ADDI x1, x1, 0 (pseudo-encoding)
        let bytes = vec![0x13, 0x05, 0x00, 0x00];
        let (instr, size) = decoder.decode(&bytes).unwrap();
        assert_eq!(instr.mnemonic, "addi");
        assert_eq!(size, 4);
    }

    #[test]
    fn test_instruction_display() {
        let instr = Instruction::new(
            "add".to_string(),
            vec![
                OperandType::Register("RAX".to_string()),
                OperandType::Register("RBX".to_string()),
            ],
            2,
        );
        let s = format!("{}", instr);
        assert!(s.contains("add"));
        assert!(s.contains("RAX"));
    }

    #[test]
    fn test_create_decoder() {
        let decoder = create_decoder(ISA::X86_64);
        let bytes = vec![0x90];
        let result = decoder.decode(&bytes);
        assert!(result.is_ok());
    }
}
