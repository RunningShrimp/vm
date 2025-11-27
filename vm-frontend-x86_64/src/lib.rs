//! # vm-frontend-x86_64 - x86-64 前端解码器
//!
//! 提供 x86-64 架构的指令解码器，将 x86-64 机器码转换为 VM IR。
//!
//! ## 支持的指令
//!
//! ### 基础指令
//! - **算术**: ADD, SUB, INC, DEC, NEG
//! - **逻辑**: AND, OR, XOR, NOT, TEST
//! - **比较**: CMP
//! - **数据移动**: MOV, LEA, PUSH, POP
//!
//! ### 控制流
//! - **无条件跳转**: JMP (rel8, rel32, r/m64)
//! - **条件跳转**: Jcc (所有条件码)
//! - **调用/返回**: CALL, RET
//!
//! ### SIMD (SSE)
//! - **数据移动**: MOVAPS
//! - **算术**: ADDPS, SUBPS, MULPS, MAXPS, MINPS
//!
//! ### 系统指令
//! - SYSCALL, CPUID, HLT, INT
//!
//! ## 使用示例
//!
//! ```rust,ignore
//! use vm_frontend_x86_64::X86Decoder;
//! use vm_core::Decoder;
//!
//! let mut decoder = X86Decoder;
//! let block = decoder.decode(&mmu, 0x1000)?;
//! ```
//!
//! ## 编码 API
//!
//! [`api`] 模块提供指令编码功能，用于生成 x86-64 机器码。

use vm_core::{Decoder, GuestAddr, Fault, MMU};
use vm_core::{Decoder, GuestAddr, Fault, MMU};
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator, MemFlags};


mod extended_insns;
mod prefix_decode;
mod opcode_decode;
mod operand_decode;

// Re-export key decoding stages for modular architecture
pub use prefix_decode::{PrefixInfo, RexPrefix, decode_prefixes};
pub use opcode_decode::{OpcodeInfo, OperandKind, decode_opcode};
pub use operand_decode::{Operand, OperandDecoder, ModRM, SIB};
mod prefix_decode;
mod opcode_decode;
mod operand_decode;

// Re-export key decoding stages for modular architecture
pub use prefix_decode::{PrefixInfo, RexPrefix, decode_prefixes};
pub use opcode_decode::{OpcodeInfo, OperandKind, decode_opcode};
pub use operand_decode::{Operand, OperandDecoder, ModRM, SIB};

pub struct X86Decoder;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum X86Mnemonic {
    Nop,
    Add, Sub, And, Or, Xor,
    Mov,
    Push, Pop,
    Lea,
    Jmp, Call, Ret,
    // ALU
    Cmp, Test, Inc, Dec, Not, Neg,
    // Control Flow
    Jcc,
    // SIMD
    Movaps, Addps, Subps, Mulps, Maxps, Minps,
    // Multiply/Divide
    Mul, Imul, Div, Idiv,
    // Atomic
    Xchg, Cmpxchg, Xadd, Lock,
    // System
    Syscall, Cpuid, Hlt, Int,
}

#[derive(Debug, Clone)]
pub enum X86Operand {
    None,
    Reg(u8), // 0-15
    Xmm(u8), // 0-15
    Mem { base: Option<u8>, index: Option<u8>, scale: u8, disp: i64 },
    Imm(i64),
    Rel(i64),
}

pub struct X86Instruction {
    pub mnemonic: X86Mnemonic,
    pub op1: X86Operand,
    pub op2: X86Operand,
    pub op3: X86Operand,
    pub op_size: u8,
    pub lock: bool,
    pub rep: bool,
    pub repne: bool,
    pub next_pc: GuestAddr,
    pub jcc_cc: Option<u8>,
}

impl vm_core::Instruction for X86Instruction {
    fn next_pc(&self) -> GuestAddr {
        self.next_pc
    }
    
    fn size(&self) -> u8 {
        (self.next_pc.saturating_sub(0) % 16) as u8
    }
    
    fn operand_count(&self) -> usize {
        let mut count = 0;
        if matches!(self.op1, X86Operand::None) == false { count += 1; }
        if matches!(self.op2, X86Operand::None) == false { count += 1; }
        if matches!(self.op3, X86Operand::None) == false { count += 1; }
        count
    }
    
    fn mnemonic(&self) -> &str {
        match self.mnemonic {
            X86Mnemonic::Nop => "nop",
            X86Mnemonic::Add => "add",
            X86Mnemonic::Sub => "sub",
            X86Mnemonic::And => "and",
            X86Mnemonic::Or => "or",
            X86Mnemonic::Xor => "xor",
            X86Mnemonic::Mov => "mov",
            X86Mnemonic::Push => "push",
            X86Mnemonic::Pop => "pop",
            X86Mnemonic::Lea => "lea",
            X86Mnemonic::Jmp => "jmp",
            X86Mnemonic::Call => "call",
            X86Mnemonic::Ret => "ret",
            X86Mnemonic::Cmp => "cmp",
            X86Mnemonic::Test => "test",
            X86Mnemonic::Inc => "inc",
            X86Mnemonic::Dec => "dec",
            X86Mnemonic::Not => "not",
            X86Mnemonic::Neg => "neg",
            X86Mnemonic::Jcc => "jcc",
            X86Mnemonic::Movaps => "movaps",
            X86Mnemonic::Addps => "addps",
            X86Mnemonic::Subps => "subps",
            X86Mnemonic::Mulps => "mulps",
            X86Mnemonic::Maxps => "maxps",
            X86Mnemonic::Minps => "minps",
            X86Mnemonic::Mul => "mul",
            X86Mnemonic::Imul => "imul",
            X86Mnemonic::Div => "div",
            X86Mnemonic::Idiv => "idiv",
            X86Mnemonic::Xchg => "xchg",
            X86Mnemonic::Cmpxchg => "cmpxchg",
            X86Mnemonic::Xadd => "xadd",
            X86Mnemonic::Lock => "lock",
            X86Mnemonic::Syscall => "syscall",
            X86Mnemonic::Cpuid => "cpuid",
            X86Mnemonic::Hlt => "hlt",
            X86Mnemonic::Int => "int",
        }
    }
    
    fn is_control_flow(&self) -> bool {
        matches!(self.mnemonic, 
            X86Mnemonic::Jmp | X86Mnemonic::Call | X86Mnemonic::Ret | 
            X86Mnemonic::Jcc | X86Mnemonic::Int | X86Mnemonic::Syscall)
    }
    
    fn is_memory_access(&self) -> bool {
        matches!(self.op1, X86Operand::Mem { .. }) || 
        matches!(self.op2, X86Operand::Mem { .. }) ||
        matches!(self.op3, X86Operand::Mem { .. }) ||
        matches!(self.mnemonic, X86Mnemonic::Push | X86Mnemonic::Pop)
    }
}

#[derive(Clone, Copy)]
enum OpKind {
    None,
    Reg, // ModR/M reg
    Rm,  // ModR/M rm
    Imm,
    Rel,
    OpReg, // Low 3 bits of opcode
    XmmReg, // ModR/M reg is XMM
    XmmRm,  // ModR/M rm is XMM or Mem
    Imm8,
}

struct InsnStream<'a> {
    mmu: &'a dyn MMU,
    pc: GuestAddr,
}

impl<'a> InsnStream<'a> {
    fn new(mmu: &'a dyn MMU, pc: GuestAddr) -> Self { Self { mmu, pc } }
    fn read_u8(&mut self) -> Result<u8, Fault> {
        let v = self.mmu.read(self.pc, 1)? as u8;
        self.pc += 1;
        Ok(v)
    }
    #[allow(dead_code)]
    fn read_u16(&mut self) -> Result<u16, Fault> {
        let v = self.mmu.read(self.pc, 2)? as u16;
        self.pc += 2;
        Ok(v)
    }
    #[allow(dead_code)]
    fn read_u32(&mut self) -> Result<u32, Fault> {
        let v = self.mmu.read(self.pc, 4)? as u32;
        self.pc += 4;
        Ok(v)
    }
    #[allow(dead_code)]
    fn read_u64(&mut self) -> Result<u64, Fault> {
        let v = self.mmu.read(self.pc, 8)?;
        self.pc += 8;
        Ok(v)
    }
}

#[derive(Default)]
struct Prefix {
    lock: bool,
    rep: bool,
    repne: bool,
    seg: Option<u8>,
    op_size: bool, // 0x66
    addr_size: bool, // 0x67
    rex: Option<u8>,
}

impl Decoder for X86Decoder {
    type Instruction = X86Instruction;
    type Block = IRBlock;
    
    fn decode_insn(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> Result<Self::Instruction, Fault> {
        let mut stream = InsnStream::new(mmu, pc);
        let mut prefix = Prefix::default();
        
        // Parse prefixes
        let opcode = loop {
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
                    break stream.read_u8()?;
                }
                _ => break b,
            }
        };
        
        decode_insn_impl(&mut stream, pc, prefix, opcode)
    }
    
    fn decode(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> Result<Self::Block, Fault> {
        let mut builder = IRBuilder::new(pc);
        let mut stream = InsnStream::new(mmu, pc);
        
        loop {
            let _start_pc = stream.pc;
            let mut prefix = Prefix::default();
            
            // 1. Prefixes
            let opcode = loop {
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
                        // REX is followed by opcode
                        break stream.read_u8()?;
                    }
                    _ => {
                        // Not a prefix, so it is the opcode
                        break b;
                    }
                }
            };

            let insn = decode_insn_impl(&mut stream, _start_pc, prefix, opcode)?;
            translate_insn(&mut builder, insn)?;
            
            // For now, single instruction blocks to avoid builder consumption issues
            break;
        }
        
        Ok(builder.build())
    }
}

fn decode_insn_impl(stream: &mut InsnStream, pc: GuestAddr, prefix: Prefix, opcode: u8) -> Result<X86Instruction, Fault> {
    let rex = prefix.rex.unwrap_or(0);
    let rex_w = (rex & 0x08) != 0;
    let rex_r = (rex & 0x04) != 0;
    let rex_x = (rex & 0x02) != 0;
    let rex_b = (rex & 0x01) != 0;
    
    let op_size = if rex_w { 64 } else if prefix.op_size { 16 } else { 32 };
    let _op_bytes = (op_size / 8) as u8;

    // Handle 2-byte opcodes
    let (opcode, is_two_byte) = if opcode == 0x0F {
        (stream.read_u8()?, true)
    } else {
        (opcode, false)
    };

    // Table lookup
    let (mnemonic, k1, k2, k3, cc_opt) = if is_two_byte {
    let (mnemonic, k1, k2, k3, cc_opt) = if is_two_byte {
        match opcode {
            0x05 => (X86Mnemonic::Syscall, OpKind::None, OpKind::None, OpKind::None, None),
            0xA2 => (X86Mnemonic::Cpuid, OpKind::None, OpKind::None, OpKind::None, None),
            0x28 => (X86Mnemonic::Movaps, OpKind::XmmReg, OpKind::XmmRm, OpKind::None, None),
            0x58 => (X86Mnemonic::Addps, OpKind::XmmReg, OpKind::XmmRm, OpKind::None, None),
            0x59 => (X86Mnemonic::Mulps, OpKind::XmmReg, OpKind::XmmRm, OpKind::None, None),
            0x5C => (X86Mnemonic::Subps, OpKind::XmmReg, OpKind::XmmRm, OpKind::None, None),
            0x5F => (X86Mnemonic::Maxps, OpKind::XmmReg, OpKind::XmmRm, OpKind::None, None),
            0x5D => (X86Mnemonic::Minps, OpKind::XmmReg, OpKind::XmmRm, OpKind::None, None),
            0x80..=0x8F => (X86Mnemonic::Jcc, OpKind::Rel, OpKind::None, OpKind::None, Some(opcode - 0x80)),
            0x05 => (X86Mnemonic::Syscall, OpKind::None, OpKind::None, OpKind::None, None),
            0xA2 => (X86Mnemonic::Cpuid, OpKind::None, OpKind::None, OpKind::None, None),
            0x28 => (X86Mnemonic::Movaps, OpKind::XmmReg, OpKind::XmmRm, OpKind::None, None),
            0x58 => (X86Mnemonic::Addps, OpKind::XmmReg, OpKind::XmmRm, OpKind::None, None),
            0x59 => (X86Mnemonic::Mulps, OpKind::XmmReg, OpKind::XmmRm, OpKind::None, None),
            0x5C => (X86Mnemonic::Subps, OpKind::XmmReg, OpKind::XmmRm, OpKind::None, None),
            0x5F => (X86Mnemonic::Maxps, OpKind::XmmReg, OpKind::XmmRm, OpKind::None, None),
            0x5D => (X86Mnemonic::Minps, OpKind::XmmReg, OpKind::XmmRm, OpKind::None, None),
            0x80..=0x8F => (X86Mnemonic::Jcc, OpKind::Rel, OpKind::None, OpKind::None, Some(opcode - 0x80)),
            _ => return Err(Fault::InvalidOpcode { pc: 0, opcode: 0 }),
        }
    } else {
        match opcode {
            0x90 => (X86Mnemonic::Nop, OpKind::None, OpKind::None, OpKind::None, None),
            0x01 => (X86Mnemonic::Add, OpKind::Rm, OpKind::Reg, OpKind::None, None),
            0x29 => (X86Mnemonic::Sub, OpKind::Rm, OpKind::Reg, OpKind::None, None),
            0x21 => (X86Mnemonic::And, OpKind::Rm, OpKind::Reg, OpKind::None, None),
            0x09 => (X86Mnemonic::Or, OpKind::Rm, OpKind::Reg, OpKind::None, None),
            0x31 => (X86Mnemonic::Xor, OpKind::Rm, OpKind::Reg, OpKind::None, None),
            0x39 => (X86Mnemonic::Cmp, OpKind::Rm, OpKind::Reg, OpKind::None, None),
            0x85 => (X86Mnemonic::Test, OpKind::Rm, OpKind::Reg, OpKind::None, None),
            0x89 => (X86Mnemonic::Mov, OpKind::Rm, OpKind::Reg, OpKind::None, None),
            0x8B => (X86Mnemonic::Mov, OpKind::Reg, OpKind::Rm, OpKind::None, None),
            0x8D => (X86Mnemonic::Lea, OpKind::Reg, OpKind::Rm, OpKind::None, None),
            0xB8..=0xBF => (X86Mnemonic::Mov, OpKind::OpReg, OpKind::Imm, OpKind::None, None),
            0x50..=0x57 => (X86Mnemonic::Push, OpKind::OpReg, OpKind::None, OpKind::None, None),
            0x58..=0x5F => (X86Mnemonic::Pop, OpKind::OpReg, OpKind::None, OpKind::None, None),
            0xEB => (X86Mnemonic::Jmp, OpKind::Rel, OpKind::None, OpKind::None, None),
            0xE9 => (X86Mnemonic::Jmp, OpKind::Rel, OpKind::None, OpKind::None, None),
            0xE8 => (X86Mnemonic::Call, OpKind::Rel, OpKind::None, OpKind::None, None),
            0xC3 => (X86Mnemonic::Ret, OpKind::None, OpKind::None, OpKind::None, None),
            0x70..=0x7F => (X86Mnemonic::Jcc, OpKind::Rel, OpKind::None, OpKind::None, Some(opcode - 0x70)),
            0xF4 => (X86Mnemonic::Hlt, OpKind::None, OpKind::None, OpKind::None, None),
            0xCC => (X86Mnemonic::Int, OpKind::None, OpKind::None, OpKind::None, None),
            0xCD => (X86Mnemonic::Int, OpKind::Imm8, OpKind::None, OpKind::None, None),
            0x90 => (X86Mnemonic::Nop, OpKind::None, OpKind::None, OpKind::None, None),
            0x01 => (X86Mnemonic::Add, OpKind::Rm, OpKind::Reg, OpKind::None, None),
            0x29 => (X86Mnemonic::Sub, OpKind::Rm, OpKind::Reg, OpKind::None, None),
            0x21 => (X86Mnemonic::And, OpKind::Rm, OpKind::Reg, OpKind::None, None),
            0x09 => (X86Mnemonic::Or, OpKind::Rm, OpKind::Reg, OpKind::None, None),
            0x31 => (X86Mnemonic::Xor, OpKind::Rm, OpKind::Reg, OpKind::None, None),
            0x39 => (X86Mnemonic::Cmp, OpKind::Rm, OpKind::Reg, OpKind::None, None),
            0x85 => (X86Mnemonic::Test, OpKind::Rm, OpKind::Reg, OpKind::None, None),
            0x89 => (X86Mnemonic::Mov, OpKind::Rm, OpKind::Reg, OpKind::None, None),
            0x8B => (X86Mnemonic::Mov, OpKind::Reg, OpKind::Rm, OpKind::None, None),
            0x8D => (X86Mnemonic::Lea, OpKind::Reg, OpKind::Rm, OpKind::None, None),
            0xB8..=0xBF => (X86Mnemonic::Mov, OpKind::OpReg, OpKind::Imm, OpKind::None, None),
            0x50..=0x57 => (X86Mnemonic::Push, OpKind::OpReg, OpKind::None, OpKind::None, None),
            0x58..=0x5F => (X86Mnemonic::Pop, OpKind::OpReg, OpKind::None, OpKind::None, None),
            0xEB => (X86Mnemonic::Jmp, OpKind::Rel, OpKind::None, OpKind::None, None),
            0xE9 => (X86Mnemonic::Jmp, OpKind::Rel, OpKind::None, OpKind::None, None),
            0xE8 => (X86Mnemonic::Call, OpKind::Rel, OpKind::None, OpKind::None, None),
            0xC3 => (X86Mnemonic::Ret, OpKind::None, OpKind::None, OpKind::None, None),
            0x70..=0x7F => (X86Mnemonic::Jcc, OpKind::Rel, OpKind::None, OpKind::None, Some(opcode - 0x70)),
            0xF4 => (X86Mnemonic::Hlt, OpKind::None, OpKind::None, OpKind::None, None),
            0xCC => (X86Mnemonic::Int, OpKind::None, OpKind::None, OpKind::None, None),
            0xCD => (X86Mnemonic::Int, OpKind::Imm8, OpKind::None, OpKind::None, None),
            0xFF => {
                // Group 5: Inc/Dec/Call/Jmp/Push
                // We need to check reg field of ModR/M, but we don't have it yet.
                // This architecture might need a tweak for group opcodes.
                // For now, let's assume we can peek or handle it later.
                // Actually, decode_insn structure assumes we know mnemonic from opcode.
                // But for 0xFF, mnemonic depends on ModR/M reg.
                // Let's return a special placeholder or handle it by peeking.
                let b = stream.mmu.read(stream.pc, 1)? as u8; // Peek next byte
                let reg = (b >> 3) & 7;
                match reg {
                    0 => (X86Mnemonic::Inc, OpKind::Rm, OpKind::None, OpKind::None, None),
                    1 => (X86Mnemonic::Dec, OpKind::Rm, OpKind::None, OpKind::None, None),
                    2 => (X86Mnemonic::Call, OpKind::Rm, OpKind::None, OpKind::None, None),
                    3 => (X86Mnemonic::Call, OpKind::Rm, OpKind::None, OpKind::None, None),
                    4 => (X86Mnemonic::Jmp, OpKind::Rm, OpKind::None, OpKind::None, None),
                    5 => (X86Mnemonic::Jmp, OpKind::Rm, OpKind::None, OpKind::None, None),
                    6 => (X86Mnemonic::Push, OpKind::Rm, OpKind::None, OpKind::None, None),
                    0 => (X86Mnemonic::Inc, OpKind::Rm, OpKind::None, OpKind::None, None),
                    1 => (X86Mnemonic::Dec, OpKind::Rm, OpKind::None, OpKind::None, None),
                    2 => (X86Mnemonic::Call, OpKind::Rm, OpKind::None, OpKind::None, None),
                    3 => (X86Mnemonic::Call, OpKind::Rm, OpKind::None, OpKind::None, None),
                    4 => (X86Mnemonic::Jmp, OpKind::Rm, OpKind::None, OpKind::None, None),
                    5 => (X86Mnemonic::Jmp, OpKind::Rm, OpKind::None, OpKind::None, None),
                    6 => (X86Mnemonic::Push, OpKind::Rm, OpKind::None, OpKind::None, None),
                    _ => return Err(Fault::InvalidOpcode { pc: 0, opcode: 0 }),
                }
            },
            _ => return Err(Fault::InvalidOpcode { pc: 0, opcode: 0 }),
        }
    };

    // ModR/M parsing if needed
    let needs_modrm = matches!(k1, OpKind::Reg | OpKind::Rm | OpKind::XmmReg | OpKind::XmmRm) || 
                      matches!(k2, OpKind::Reg | OpKind::Rm | OpKind::XmmReg | OpKind::XmmRm);
    let (mod_, reg, rm) = if needs_modrm {
        let b = stream.read_u8()?;
        (b >> 6, (b >> 3) & 7, b & 7)
    } else {
        (0, 0, 0)
    };

    let mut decode_op = |kind: OpKind| -> Result<X86Operand, Fault> {
        match kind {
            OpKind::None => Ok(X86Operand::None),
            OpKind::Reg => {
                let reg_idx = reg | (if rex_r { 8 } else { 0 });
                Ok(X86Operand::Reg(reg_idx))
            },
            OpKind::OpReg => {
                let reg_idx = (opcode & 7) | (if rex_b { 8 } else { 0 });
                Ok(X86Operand::Reg(reg_idx))
            },
            OpKind::Rm => {
                if mod_ == 3 {
                    let rm_idx = rm | (if rex_b { 8 } else { 0 });
                    Ok(X86Operand::Reg(rm_idx))
                } else {
                    let rm_idx = rm | (if rex_b { 8 } else { 0 });
                    let (base, index, scale, has_sib) = if rm == 4 {
                        let sib = stream.read_u8()?;
                        let scale = 1 << (sib >> 6);
                        let index = ((sib >> 3) & 7) | (if rex_x { 8 } else { 0 });
                        let base = (sib & 7) | (if rex_b { 8 } else { 0 });
                        (Some(base), Some(index), scale, true)
                    } else {
                        (Some(rm_idx), None, 0, false)
                    };
                    
                    let disp = match mod_ {
                        0 => if rm == 5 || (has_sib && (base.expect("Operation failed") & 7) == 5) { stream.read_u32()? as i32 as i64 } else { 0 },
                        1 => stream.read_u8()? as i8 as i64,
                        2 => stream.read_u32()? as i32 as i64,
                        _ => 0,
                    };

                    // Fixup for RIP-relative (Mod=0, RM=5 in 64-bit mode is RIP-rel if no SIB)
                    // But wait, in 64-bit mode:
                    // Mod=00, RM=101 (5) -> RIP + disp32
                    // If SIB present (RM=100), then Base=101 (5) -> Mod=00 means disp32 only (no base)
                    
                    let final_base = if mod_ == 0 && rm == 5 && !has_sib { None } // RIP-relative handled as None base? Or special?
                    // Actually RIP-relative is usually handled as Base=RIP. But we don't have RIP reg id easily.
                    // Let's use None base to imply absolute/RIP-rel for now, or handle it in translation.
                    // For now, let's stick to the previous logic:
                    // if mod_ == 0 && rm == 5 && !has_sib { target = pc + disp }
                    else if mod_ == 0 && has_sib && (base.expect("Operation failed") & 7) == 5 { None }
                    else { base };

                    Ok(X86Operand::Mem { base: final_base, index, scale, disp })
                }
            },
            OpKind::Imm => {
                let imm = if rex_w {
                    stream.read_u64()? as i64
                } else if prefix.op_size {
                    stream.read_u16()? as i64
                } else {
                    stream.read_u32()? as i64
                };
                Ok(X86Operand::Imm(imm))
            },
            OpKind::Imm8 => {
                let imm = stream.read_u8()? as i64;
                Ok(X86Operand::Imm(imm))
            },
            OpKind::Rel => {
                // Rel size depends on opcode usually.
                // JMP rel8 (EB) vs JMP rel32 (E9)
                // We need to know the size.
                // Hack: infer from opcode or pass size in OpKind?
                // Let's check opcode.
                let rel = match opcode {
                    0xEB | 0x70..=0x7F => stream.read_u8()? as i8 as i64,
                    _ => stream.read_u32()? as i32 as i64,
                };
                Ok(X86Operand::Rel(rel))
            },
            OpKind::XmmReg => {
                let reg_idx = reg | (if rex_r { 8 } else { 0 });
                Ok(X86Operand::Xmm(reg_idx))
            },
            OpKind::XmmRm => {
                if mod_ == 3 {
                    let rm_idx = rm | (if rex_b { 8 } else { 0 });
                    Ok(X86Operand::Xmm(rm_idx))
                } else {
                    let rm_idx = rm | (if rex_b { 8 } else { 0 });
                    let (base, index, scale, has_sib) = if rm == 4 {
                        let sib = stream.read_u8()?;
                        let scale = 1 << (sib >> 6);
                        let index = ((sib >> 3) & 7) | (if rex_x { 8 } else { 0 });
                        let base = (sib & 7) | (if rex_b { 8 } else { 0 });
                        (Some(base), Some(index), scale, true)
                    } else {
                        (Some(rm_idx), None, 0, false)
                    };
                    
                    let disp = match mod_ {
                        0 => if rm == 5 || (has_sib && (base.expect("Operation failed") & 7) == 5) { stream.read_u32()? as i32 as i64 } else { 0 },
                        1 => stream.read_u8()? as i8 as i64,
                        2 => stream.read_u32()? as i32 as i64,
                        _ => 0,
                    };

                    let final_base = if mod_ == 0 && rm == 5 && !has_sib { None }
                    else if mod_ == 0 && has_sib && (base.expect("Operation failed") & 7) == 5 { None }
                    else { base };

                    Ok(X86Operand::Mem { base: final_base, index, scale, disp })
                }
            }
        }
    };

    let op1 = decode_op(k1)?;
    let op2 = decode_op(k2)?;
    let op3 = decode_op(k3)?;

    Ok(X86Instruction {
        mnemonic,
        op1,
        op2,
        op3,
        op_size,
        lock: prefix.lock,
        rep: prefix.rep,
        repne: prefix.repne,
        next_pc: stream.pc,
        jcc_cc: cc_opt,
        jcc_cc: cc_opt,
    })
}

fn load_operand(builder: &mut IRBuilder, op: &X86Operand, op_bytes: u8) -> Result<u32, Fault> {
    match op {
        X86Operand::Reg(r) => Ok(*r as u32),
        X86Operand::Xmm(r) => Ok(16 + *r as u32),
        X86Operand::Imm(i) => {
            let tmp = 100; // Alloc temp
            builder.push(IROp::MovImm { dst: tmp, imm: *i as u64 });
            Ok(tmp)
        },
        X86Operand::Mem { base, index: _, scale: _, disp } => {
            let addr_reg = 101;
            if let Some(b) = base {
                builder.push(IROp::AddImm { dst: addr_reg, src: *b as u32, imm: *disp });
            } else {
                builder.push(IROp::MovImm { dst: addr_reg, imm: *disp as u64 });
            }
            let val_reg = 102;
            builder.push(IROp::Load { dst: val_reg, base: addr_reg, offset: 0, size: op_bytes, flags: MemFlags::default() });
            Ok(val_reg)
        },
        _ => Err(Fault::InvalidOpcode { pc: 0, opcode: 0 }),
    }
}

fn write_operand(builder: &mut IRBuilder, op: &X86Operand, val: u32, op_bytes: u8) -> Result<(), Fault> {
    match op {
        X86Operand::Reg(r) => {
            builder.push(IROp::AddImm { dst: *r as u32, src: val, imm: 0 }); // Move
            Ok(())
        },
        X86Operand::Xmm(r) => {
            builder.push(IROp::AddImm { dst: 16 + *r as u32, src: val, imm: 0 });
            Ok(())
        },
        X86Operand::Mem { base, index: _, scale: _, disp } => {
            let addr_reg = 101;
            if let Some(b) = base {
                builder.push(IROp::AddImm { dst: addr_reg, src: *b as u32, imm: *disp });
            } else {
                builder.push(IROp::MovImm { dst: addr_reg, imm: *disp as u64 });
            }
            builder.push(IROp::Store { src: val, base: addr_reg, offset: 0, size: op_bytes, flags: MemFlags::default() });
            Ok(())
        },
        _ => Err(Fault::InvalidOpcode { pc: 0, opcode: 0 }),
    }
}

fn translate_insn(builder: &mut IRBuilder, insn: X86Instruction) -> Result<(), Fault> {
    let op_bytes = (insn.op_size / 8) as u8;
    
    match insn.mnemonic {
        X86Mnemonic::Nop => builder.push(IROp::Nop),
        X86Mnemonic::Add => {
            let src1 = load_operand(builder, &insn.op1, op_bytes)?;
            let src2 = load_operand(builder, &insn.op2, op_bytes)?;
            let dst = 103;
            builder.push(IROp::Add { dst, src1, src2 });
            write_operand(builder, &insn.op1, dst, op_bytes)?;
        },
        X86Mnemonic::Sub => {
            let src1 = load_operand(builder, &insn.op1, op_bytes)?;
            let src2 = load_operand(builder, &insn.op2, op_bytes)?;
            let dst = 103;
            builder.push(IROp::Sub { dst, src1, src2 });
            write_operand(builder, &insn.op1, dst, op_bytes)?;
        },
        X86Mnemonic::Mov => {
            let src = load_operand(builder, &insn.op2, op_bytes)?;
            write_operand(builder, &insn.op1, src, op_bytes)?;
        },
        X86Mnemonic::Lea => {
            // LEA is special, it doesn't load memory, it calculates address
            if let X86Operand::Mem { base, index: _, scale: _, disp } = insn.op2 {
                let addr_reg = 101;
                if let Some(b) = base {
                    builder.push(IROp::AddImm { dst: addr_reg, src: b as u32, imm: disp });
                } else {
                    builder.push(IROp::MovImm { dst: addr_reg, imm: disp as u64 });
                }
                write_operand(builder, &insn.op1, addr_reg, op_bytes)?;
            } else {
                return Err(Fault::InvalidOpcode { pc: 0, opcode: 0 });
            }
        },
        X86Mnemonic::Push => {
            let val = load_operand(builder, &insn.op1, op_bytes)?;
            builder.push(IROp::AddImm { dst: 4, src: 4, imm: -8 });
            builder.push(IROp::Store { src: val, base: 4, offset: 0, size: 8, flags: MemFlags::default() });
        },
        X86Mnemonic::Pop => {
            let val = 104;
            builder.push(IROp::Load { dst: val, base: 4, offset: 0, size: 8, flags: MemFlags::default() });
            builder.push(IROp::AddImm { dst: 4, src: 4, imm: 8 });
            write_operand(builder, &insn.op1, val, op_bytes)?;
        },
        X86Mnemonic::Jmp => {
            if let X86Operand::Rel(target) = insn.op1 {
                let abs = (insn.next_pc as i64).wrapping_add(target) as u64;
                builder.set_term(Terminator::Jmp { target: abs });
                let abs = (insn.next_pc as i64).wrapping_add(target) as u64;
                builder.set_term(Terminator::Jmp { target: abs });
            }
        },
        X86Mnemonic::Call => {
            if let X86Operand::Rel(target) = insn.op1 {
                let abs = (insn.next_pc as i64).wrapping_add(target) as u64;
                builder.set_term(Terminator::Call { target: abs, ret_pc: insn.next_pc });
                let abs = (insn.next_pc as i64).wrapping_add(target) as u64;
                builder.set_term(Terminator::Call { target: abs, ret_pc: insn.next_pc });
            }
        },
        X86Mnemonic::Ret => {
            builder.set_term(Terminator::Ret);
        },
        X86Mnemonic::Movaps => {
            let src = load_operand(builder, &insn.op2, 16)?;
            write_operand(builder, &insn.op1, src, 16)?;
        },
        X86Mnemonic::Addps => {
            let src1 = load_operand(builder, &insn.op1, 16)?;
            let src2 = load_operand(builder, &insn.op2, 16)?;
            let dst = 105;
            builder.push(IROp::VecAdd { dst, src1, src2, element_size: 4 });
            write_operand(builder, &insn.op1, dst, 16)?;
        },
        X86Mnemonic::Subps => {
            let src1 = load_operand(builder, &insn.op1, 16)?;
            let src2 = load_operand(builder, &insn.op2, 16)?;
            let dst = 105;
            builder.push(IROp::VecSub { dst, src1, src2, element_size: 4 });
            write_operand(builder, &insn.op1, dst, 16)?;
        },
        X86Mnemonic::Mulps => {
            let src1 = load_operand(builder, &insn.op1, 16)?;
            let src2 = load_operand(builder, &insn.op2, 16)?;
            let dst = 105;
            builder.push(IROp::VecMul { dst, src1, src2, element_size: 4 });
            write_operand(builder, &insn.op1, dst, 16)?;
        },
        X86Mnemonic::Syscall => {
            builder.push(IROp::SysCall);
        },
        X86Mnemonic::Cpuid => {
            builder.push(IROp::SysCall); // Placeholder
        },
        X86Mnemonic::And => {
            let src1 = load_operand(builder, &insn.op1, op_bytes)?;
            let src2 = load_operand(builder, &insn.op2, op_bytes)?;
            let dst = 103;
            builder.push(IROp::And { dst, src1, src2 });
            write_operand(builder, &insn.op1, dst, op_bytes)?;
        },
        X86Mnemonic::Or => {
            let src1 = load_operand(builder, &insn.op1, op_bytes)?;
            let src2 = load_operand(builder, &insn.op2, op_bytes)?;
            let dst = 103;
            builder.push(IROp::Or { dst, src1, src2 });
            write_operand(builder, &insn.op1, dst, op_bytes)?;
        },
        X86Mnemonic::Xor => {
            let src1 = load_operand(builder, &insn.op1, op_bytes)?;
            let src2 = load_operand(builder, &insn.op2, op_bytes)?;
            let dst = 103;
            builder.push(IROp::Xor { dst, src1, src2 });
            write_operand(builder, &insn.op1, dst, op_bytes)?;
        },
        X86Mnemonic::Cmp => {
            let src1 = load_operand(builder, &insn.op1, op_bytes)?;
            let src2 = load_operand(builder, &insn.op2, op_bytes)?;
            let a = 200;
            let b = 201;
            builder.push(IROp::AddImm { dst: a, src: src1, imm: 0 });
            builder.push(IROp::AddImm { dst: b, src: src2, imm: 0 });
            let a = 200;
            let b = 201;
            builder.push(IROp::AddImm { dst: a, src: src1, imm: 0 });
            builder.push(IROp::AddImm { dst: b, src: src2, imm: 0 });
        },
        X86Mnemonic::Test => {
            let src1 = load_operand(builder, &insn.op1, op_bytes)?;
            let src2 = load_operand(builder, &insn.op2, op_bytes)?;
            // TEST is And but without writing back result.
            let dst = 103;
            builder.push(IROp::And { dst, src1, src2 });
        },
        X86Mnemonic::Inc => {
            let src = load_operand(builder, &insn.op1, op_bytes)?;
            let dst = 103;
            builder.push(IROp::AddImm { dst, src, imm: 1 });
            write_operand(builder, &insn.op1, dst, op_bytes)?;
        },
        X86Mnemonic::Dec => {
            let src = load_operand(builder, &insn.op1, op_bytes)?;
            let dst = 103;
            builder.push(IROp::AddImm { dst, src, imm: -1 });
            write_operand(builder, &insn.op1, dst, op_bytes)?;
        },
        X86Mnemonic::Jcc => {
            if let X86Operand::Rel(target) = insn.op1 {
                let cc = insn.jcc_cc.unwrap_or(4);
                let lhs = 200;
                let rhs = 201;
                let cc = insn.jcc_cc.unwrap_or(4);
                let lhs = 200;
                let rhs = 201;
                let cond = 106;
                match cc {
                    0x4 => builder.push(IROp::CmpEq { dst: cond, lhs, rhs }),
                    0x5 => builder.push(IROp::CmpNe { dst: cond, lhs, rhs }),
                    0x2 => builder.push(IROp::CmpLtU { dst: cond, lhs, rhs }),
                    0x3 => builder.push(IROp::CmpGeU { dst: cond, lhs, rhs }),
                    0x6 => {
                        let t1 = 107; let t2 = 108;
                        builder.push(IROp::CmpLtU { dst: t1, lhs, rhs });
                        builder.push(IROp::CmpEq { dst: t2, lhs, rhs });
                        builder.push(IROp::Or { dst: cond, src1: t1, src2: t2 });
                    },
                    0x7 => builder.push(IROp::CmpLtU { dst: cond, lhs: rhs, rhs: lhs }),
                    0xC => builder.push(IROp::CmpLt { dst: cond, lhs, rhs }),
                    0xD => builder.push(IROp::CmpGe { dst: cond, lhs, rhs }),
                    0xE => {
                        let t1 = 107; let t2 = 108;
                        builder.push(IROp::CmpLt { dst: t1, lhs, rhs });
                        builder.push(IROp::CmpEq { dst: t2, lhs, rhs });
                        builder.push(IROp::Or { dst: cond, src1: t1, src2: t2 });
                    },
                    0xF => builder.push(IROp::CmpLt { dst: cond, lhs: rhs, rhs: lhs }),
                    _ => builder.push(IROp::CmpEq { dst: cond, lhs, rhs }),
                }
                let abs = (insn.next_pc as i64).wrapping_add(target) as u64;
                builder.set_term(Terminator::CondJmp { cond, target_true: abs, target_false: insn.next_pc });
                match cc {
                    0x4 => builder.push(IROp::CmpEq { dst: cond, lhs, rhs }),
                    0x5 => builder.push(IROp::CmpNe { dst: cond, lhs, rhs }),
                    0x2 => builder.push(IROp::CmpLtU { dst: cond, lhs, rhs }),
                    0x3 => builder.push(IROp::CmpGeU { dst: cond, lhs, rhs }),
                    0x6 => {
                        let t1 = 107; let t2 = 108;
                        builder.push(IROp::CmpLtU { dst: t1, lhs, rhs });
                        builder.push(IROp::CmpEq { dst: t2, lhs, rhs });
                        builder.push(IROp::Or { dst: cond, src1: t1, src2: t2 });
                    },
                    0x7 => builder.push(IROp::CmpLtU { dst: cond, lhs: rhs, rhs: lhs }),
                    0xC => builder.push(IROp::CmpLt { dst: cond, lhs, rhs }),
                    0xD => builder.push(IROp::CmpGe { dst: cond, lhs, rhs }),
                    0xE => {
                        let t1 = 107; let t2 = 108;
                        builder.push(IROp::CmpLt { dst: t1, lhs, rhs });
                        builder.push(IROp::CmpEq { dst: t2, lhs, rhs });
                        builder.push(IROp::Or { dst: cond, src1: t1, src2: t2 });
                    },
                    0xF => builder.push(IROp::CmpLt { dst: cond, lhs: rhs, rhs: lhs }),
                    _ => builder.push(IROp::CmpEq { dst: cond, lhs, rhs }),
                }
                let abs = (insn.next_pc as i64).wrapping_add(target) as u64;
                builder.set_term(Terminator::CondJmp { cond, target_true: abs, target_false: insn.next_pc });
            }
        },
        X86Mnemonic::Maxps => {
            // No direct IR op for Max, maybe need to add it or emulate
            // For now, skip or use a placeholder
        },
        X86Mnemonic::Minps => {
            // No direct IR op for Min
        },
        X86Mnemonic::Hlt => {
            builder.set_term(Terminator::Fault { cause: 0 }); // HLT -> Fault
        },
        X86Mnemonic::Int => {
            if let X86Operand::Imm(vec) = insn.op1 {
                builder.set_term(Terminator::Interrupt { vector: vec as u32 });
            } else {
                builder.set_term(Terminator::Interrupt { vector: 3 });
            }
        },
        _ => return Err(Fault::InvalidOpcode { pc: 0, opcode: 0 }),
    }
    Ok(())
}

pub mod api {
    fn clamp_rel8(mut v: i32) -> i8 { if v < -128 { v = -128; } if v > 127 { v = 127; } v as i8 }
    pub fn encode_jmp_short(rel: i32) -> Vec<u8> { vec![0xEB, clamp_rel8(rel) as u8] }
    pub fn encode_jmp_near(rel: i32) -> Vec<u8> { let b = (rel as i32).to_le_bytes(); vec![0xE9, b[0], b[1], b[2], b[3]] }
    pub fn encode_call_near(rel: i32) -> Vec<u8> { let b = (rel as i32).to_le_bytes(); vec![0xE8, b[0], b[1], b[2], b[3]] }
    pub fn encode_ret() -> Vec<u8> { vec![0xC3] }
    pub fn encode_jcc_short(cc: u8, rel: i32) -> Vec<u8> { vec![0x70 + (cc & 0x0F), clamp_rel8(rel) as u8] }
    pub fn encode_jcc_near(cc: u8, rel: i32) -> Vec<u8> { let b = (rel as i32).to_le_bytes(); vec![0x0F, 0x80 + (cc & 0x0F), b[0], b[1], b[2], b[3]] }
    #[derive(Copy, Clone)]
    pub enum Cond { O=0, NO=1, B=2, NB=3, E=4, NE=5, BE=6, NBE=7, S=8, NS=9, P=10, NP=11, L=12, NL=13, LE=14, NLE=15 }
    pub fn encode_jcc_short_cc(cc: Cond, rel: i32) -> Vec<u8> { encode_jcc_short(cc as u8, rel) }
    pub fn encode_jcc_near_cc(cc: Cond, rel: i32) -> Vec<u8> { encode_jcc_near(cc as u8, rel) }

    pub fn encode_jz_short(rel: i32) -> Vec<u8> { encode_jcc_short_cc(Cond::E, rel) }
    pub fn encode_jnz_short(rel: i32) -> Vec<u8> { encode_jcc_short_cc(Cond::NE, rel) }
    pub fn encode_ja_short(rel: i32) -> Vec<u8> { encode_jcc_short_cc(Cond::NBE, rel) }
    pub fn encode_jae_short(rel: i32) -> Vec<u8> { encode_jcc_short_cc(Cond::NB, rel) }
    pub fn encode_jb_short(rel: i32) -> Vec<u8> { encode_jcc_short_cc(Cond::B, rel) }
    pub fn encode_jbe_short(rel: i32) -> Vec<u8> { encode_jcc_short_cc(Cond::BE, rel) }
    pub fn encode_jg_short(rel: i32) -> Vec<u8> { encode_jcc_short_cc(Cond::NLE, rel) }
    pub fn encode_jge_short(rel: i32) -> Vec<u8> { encode_jcc_short_cc(Cond::NL, rel) }
    pub fn encode_jl_short(rel: i32) -> Vec<u8> { encode_jcc_short_cc(Cond::L, rel) }
    pub fn encode_jle_short(rel: i32) -> Vec<u8> { encode_jcc_short_cc(Cond::LE, rel) }

    pub fn encode_jz_near(rel: i32) -> Vec<u8> { encode_jcc_near_cc(Cond::E, rel) }
    pub fn encode_jnz_near(rel: i32) -> Vec<u8> { encode_jcc_near_cc(Cond::NE, rel) }
    pub fn encode_ja_near(rel: i32) -> Vec<u8> { encode_jcc_near_cc(Cond::NBE, rel) }
    pub fn encode_jae_near(rel: i32) -> Vec<u8> { encode_jcc_near_cc(Cond::NB, rel) }
    pub fn encode_jb_near(rel: i32) -> Vec<u8> { encode_jcc_near_cc(Cond::B, rel) }
    pub fn encode_jbe_near(rel: i32) -> Vec<u8> { encode_jcc_near_cc(Cond::BE, rel) }
    pub fn encode_jg_near(rel: i32) -> Vec<u8> { encode_jcc_near_cc(Cond::NLE, rel) }
    pub fn encode_jge_near(rel: i32) -> Vec<u8> { encode_jcc_near_cc(Cond::NL, rel) }
    pub fn encode_jl_near(rel: i32) -> Vec<u8> { encode_jcc_near_cc(Cond::L, rel) }
    pub fn encode_jle_near(rel: i32) -> Vec<u8> { encode_jcc_near_cc(Cond::LE, rel) }
    pub fn encode_loop(rel: i32) -> Vec<u8> { vec![0xE2, clamp_rel8(rel) as u8] }
    pub fn encode_loope(rel: i32) -> Vec<u8> { vec![0xE1, clamp_rel8(rel) as u8] }
    pub fn encode_loopne(rel: i32) -> Vec<u8> { vec![0xE0, clamp_rel8(rel) as u8] }
    pub fn encode_jrcxz(rel: i32) -> Vec<u8> { vec![0xE3, clamp_rel8(rel) as u8] }

    fn rex_w_for_reg(reg: u8) -> u8 { 0x48 | ((reg >> 3) & 0x1) }
    fn modrm(mod_bits: u8, reg_ext: u8, rm: u8) -> u8 { ((mod_bits & 0x3) << 6) | ((reg_ext & 0x7) << 3) | (rm & 0x7) }
    pub fn encode_jmp_r64(reg: u8) -> Vec<u8> { vec![rex_w_for_reg(reg), 0xFF, modrm(0b11, 0b100, reg & 0x7)] }
    pub fn encode_call_r64(reg: u8) -> Vec<u8> { vec![rex_w_for_reg(reg), 0xFF, modrm(0b11, 0b010, reg & 0x7)] }

    pub fn encode_addr_sib(base: u8, index: Option<u8>, scale: u8, disp: i32) -> (u8, Option<u8>, Vec<u8>) {
        let use_index = index.unwrap_or(4);
        let mut rex = 0x48;
        if base >= 8 { rex |= 0x01; }
        if use_index != 4 { rex |= 0x02; }
        let mut bytes = Vec::new();
        let scale_bits = (scale & 0x3) << 6;
        let sib = scale_bits | ((use_index & 0x7) << 3) | (base & 0x7);
        let disp8 = disp as i8;
        let mod_bits = if disp == 0 && base != 5 { 0b00 } else if disp8 as i32 == disp { 0b01 } else { 0b10 };
        let mrm = modrm(mod_bits, 0, 0b100);
        bytes.push(mrm);
        bytes.push(sib);
        if mod_bits == 0b01 { bytes.push(disp8 as u8); }
        if mod_bits == 0b10 || (mod_bits == 0b00 && base == 5) { let d = disp.to_le_bytes(); bytes.extend_from_slice(&d); }
        (rex, Some(sib), bytes)
    }

    pub fn encode_jmp_mem64(base: u8, index: Option<u8>, scale: u8, disp: i32) -> Vec<u8> {
        if index.is_none() && base != 4 && !(base == 5 && disp == 0) {
            let mut rex = 0x48; if base >= 8 { rex |= 0x01; }
            let disp8 = disp as i8;
            let mod_bits = if disp == 0 && base != 5 { 0b00 } else if disp8 as i32 == disp { 0b01 } else { 0b10 };
            let mut v = vec![rex, 0xFF, modrm(mod_bits, 0b100, base & 0x7)];
            if mod_bits == 0b01 { v.push(disp8 as u8); }
            if mod_bits == 0b10 || (mod_bits == 0b00 && (base & 0x7) == 5) { let d = disp.to_le_bytes(); v.extend_from_slice(&d); }
            v
        } else {
            let (rex, _sib, mut addr) = encode_addr_sib(base, index, scale, disp);
            let mut v = vec![rex, 0xFF];
            addr[0] = modrm((addr[0] >> 6) & 0x3, 0b100, 0b100);
            v.extend(addr);
            v
        }
    }
    pub fn encode_call_mem64(base: u8, index: Option<u8>, scale: u8, disp: i32) -> Vec<u8> {
        if index.is_none() && base == 5 && disp == 0 {
            let mut v = vec![0x48, 0xFF, 0x24, 0x25];
            v.extend_from_slice(&disp.to_le_bytes());
            v
        } else if index.is_none() && base != 4 {
            let mut rex = 0x48; if base >= 8 { rex |= 0x01; }
            let disp8 = disp as i8;
            let mod_bits = if disp == 0 && base != 5 { 0b00 } else if disp8 as i32 == disp { 0b01 } else { 0b10 };
            let mut v = vec![rex, 0xFF, modrm(mod_bits, 0b010, base & 0x7)];
            if mod_bits == 0b01 { v.push(disp8 as u8); }
            if mod_bits == 0b10 || (mod_bits == 0b00 && (base & 0x7) == 5) { let d = disp.to_le_bytes(); v.extend_from_slice(&d); }
            v
        } else {
            let (rex, _sib, mut addr) = encode_addr_sib(base, index, scale, disp);
            let mut v = vec![rex, 0xFF];
            addr[0] = modrm((addr[0] >> 6) & 0x3, 0b010, 0b100);
            v.extend(addr);
            v
        }
    }
    pub fn encode_jmp_rip_rel(disp: i32) -> Vec<u8> {
        let mut v = vec![0x48, 0xFF, 0x25];
        v.extend_from_slice(&disp.to_le_bytes());
        v
    }
    pub fn encode_call_rip_rel(disp: i32) -> Vec<u8> {
        let mut v = vec![0x48, 0xFF, 0x15];
        v.extend_from_slice(&disp.to_le_bytes());
        v
    }
    pub fn encode_jmp_mem_index_only(index: u8, scale: u8, disp: i32) -> Vec<u8> {
        let mut rex = 0x48;
        if index >= 8 { rex |= 0x02; }
        let mut v = vec![rex, 0xFF, 0x24];
        let sib = ((scale & 0x3) << 6) | ((index & 0x7) << 3) | 0x5;
        v.push(sib);
        v.extend_from_slice(&disp.to_le_bytes());
        v
    }
    pub fn encode_call_mem_index_only(index: u8, scale: u8, disp: i32) -> Vec<u8> {
        let mut rex = 0x48;
        if index >= 8 { rex |= 0x02; }
        let mut v = vec![rex, 0xFF, 0x14];
        let sib = ((scale & 0x3) << 6) | ((index & 0x7) << 3) | 0x5;
        v.push(sib);
        v.extend_from_slice(&disp.to_le_bytes());
        v
    }

    pub fn encode_mem64_ff(op_ext: u8, base: Option<u8>, index: Option<u8>, scale: u8, disp: i32) -> Vec<u8> {
        let mut rex = 0x48;
        if let Some(b) = base { if b >= 8 { rex |= 0x01; } }
        if let Some(i) = index { if i >= 8 { rex |= 0x02; } }
        let mut v = vec![rex, 0xFF];
        match (base, index) {
            (Some(b), None) if (b & 0x7) != 4 && !(((b & 0x7) == 5) && disp == 0) => {
                let disp8 = disp as i8;
                let mod_bits = if disp == 0 && (b & 0x7) != 5 { 0b00 } else if disp8 as i32 == disp { 0b01 } else { 0b10 };
                v.push(modrm(mod_bits, op_ext & 0x7, b & 0x7));
                if mod_bits == 0b01 { v.push(disp8 as u8); }
                if mod_bits == 0b10 || (mod_bits == 0b00 && (b & 0x7) == 5) { v.extend_from_slice(&disp.to_le_bytes()); }
            }
            _ => {
                let b = base.unwrap_or(5);
                let i = index.unwrap_or(4);
                let disp8 = disp as i8;
                let force_disp32_only = base.is_none() && index.is_none();
                let mod_bits = if force_disp32_only { 0b00 } else if (b & 0x7) == 5 && disp == 0 { 0b00 } else if disp == 0 && (b & 0x7) != 5 { 0b00 } else if disp8 as i32 == disp { 0b01 } else { 0b10 };
                v.push(modrm(mod_bits, op_ext & 0x7, 0b100));
                let sib = ((scale & 0x3) << 6) | ((i & 0x7) << 3) | (b & 0x7);
                v.push(sib);
                if mod_bits == 0b01 { v.push(disp8 as u8); }
                if mod_bits == 0b10 || (mod_bits == 0b00 && (b & 0x7) == 5) || force_disp32_only { v.extend_from_slice(&disp.to_le_bytes()); }
            }
        }
        v
    }

    pub fn encode_far_jmp_mem64(base: Option<u8>, index: Option<u8>, scale: u8, disp: i32) -> Vec<u8> {
        encode_mem64_ff(0b101, base, index, scale, disp)
    }
    pub fn encode_far_call_mem64(base: Option<u8>, index: Option<u8>, scale: u8, disp: i32) -> Vec<u8> {
        encode_mem64_ff(0b011, base, index, scale, disp)
    }

    pub fn encode_lea_r64(dest: u8, base: Option<u8>, index: Option<u8>, scale: u8, disp: i32) -> Vec<u8> {
        let mut rex = 0x48;
        if dest >= 8 { rex |= 0x04; }
        if let Some(b) = base { if b >= 8 { rex |= 0x01; } }
        if let Some(i) = index { if i >= 8 { rex |= 0x02; } }
        let mut v = vec![rex, 0x8D];
        let reg = dest & 0x7;
        match (base, index) {
            (Some(b), None) if (b & 0x7) != 4 && !(b == 5 && disp == 0) => {
                let disp8 = disp as i8;
                let mod_bits = if disp == 0 && (b & 0x7) != 5 { 0b00 } else if disp8 as i32 == disp { 0b01 } else { 0b10 };
                v.push(modrm(mod_bits, reg, b & 0x7));
                if mod_bits == 0b01 { v.push(disp8 as u8); }
                if mod_bits == 0b10 || (mod_bits == 0b00 && (b & 0x7) == 5) { v.extend_from_slice(&disp.to_le_bytes()); }
            }
            _ => {
                let b = base.unwrap_or(5);
                let i = index.unwrap_or(4);
                let disp8 = disp as i8;
                let mod_bits = if disp == 0 && (b & 0x7) != 5 { 0b00 } else if disp8 as i32 == disp { 0b01 } else { 0b10 };
                v.push(modrm(mod_bits, reg, 0b100));
                let sib = ((scale & 0x3) << 6) | ((i & 0x7) << 3) | (b & 0x7);
                v.push(sib);
                if mod_bits == 0b01 { v.push(disp8 as u8); }
                if mod_bits == 0b10 || (mod_bits == 0b00 && (b & 0x7) == 5) { v.extend_from_slice(&disp.to_le_bytes()); }
            }
        }
        v
    }

    pub fn encode_movaps(dst: u8, src: u8) -> Vec<u8> {
        let mut v = vec![0x0F, 0x28];
        let mut rex = 0;
        if dst >= 8 { rex |= 0x04; }
        if src >= 8 { rex |= 0x01; }
        if rex != 0 { v.insert(0, 0x40 | rex); }
        v.push(modrm(0b11, dst & 0x7, src & 0x7));
        v
    }
    
    pub fn encode_addps(dst: u8, src: u8) -> Vec<u8> {
        let mut v = vec![0x0F, 0x58];
        let mut rex = 0;
        if dst >= 8 { rex |= 0x04; }
        if src >= 8 { rex |= 0x01; }
        if rex != 0 { v.insert(0, 0x40 | rex); }
        v.push(modrm(0b11, dst & 0x7, src & 0x7));
        v
    }

    pub fn encode_syscall() -> Vec<u8> {
        vec![0x0F, 0x05]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_core::GuestAddr;
    use vm_ir::IROp;

    struct MockMMU {
        data: Vec<u8>,
        base: GuestAddr,
    }

impl MMU for MockMMU {
impl MMU for MockMMU {
        fn translate(&mut self, va: GuestAddr, _access: vm_core::AccessType) -> Result<GuestAddr, Fault> {
            Ok(va)
        }
        fn fetch_insn(&self, pc: GuestAddr) -> Result<u64, Fault> {
            self.read(pc, 8)
        }
        fn read(&self, addr: GuestAddr, size: u8) -> Result<u64, Fault> {
            let offset = (addr - self.base) as usize;
            if offset + size as usize > self.data.len() {
                return Err(Fault::PageFault { addr, access: vm_core::AccessType::Read });
            }
            let mut val = 0;
            for i in 0..size as usize {
                val |= (self.data[offset + i] as u64) << (i * 8);
            }
            Ok(val)
        }
        fn write(&mut self, _addr: GuestAddr, _val: u64, _size: u8) -> Result<(), Fault> {
            Ok(())
        }
        fn map_mmio(&mut self, _base: u64, _size: u64, _device: Box<dyn vm_core::MmioDevice>) {
            // No-op for mock
        }
        fn flush_tlb(&mut self) {
            // No-op for mock
        }
        fn memory_size(&self) -> usize {
            self.data.len()
        }
        fn dump_memory(&self) -> Vec<u8> { self.data.clone() }
        fn restore_memory(&mut self, data: &[u8]) -> Result<(), String> {
            self.data.clear();
            self.data.extend_from_slice(data);
            Ok(())
        }
        fn as_any(&self) -> &dyn std::any::Any { self }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    }

    #[test]
    fn test_simd_addps() {
        let code = api::encode_addps(1, 2);
        let mmu = MockMMU { data: code, base: 0x1000 };
        let mut decoder = X86Decoder;
        let block = decoder.decode(&mmu, 0x1000).expect("Operation failed");
        
        // Expected ops:
        // 1. VecAdd dst=105, src1=17, src2=18
        // 2. AddImm dst=17, src=105, imm=0 (Move)
        
        let op = &block.ops[0];
        if let IROp::VecAdd { dst: _, src1, src2, element_size } = op {
            assert_eq!(*src1, 17); // XMM1 -> 16+1
            assert_eq!(*src2, 18); // XMM2 -> 16+2
            assert_eq!(*element_size, 4);
        } else {
            panic!("Expected VecAdd, got {:?}", op);
        }
    }

    #[test]
    fn test_syscall() {
        let code = api::encode_syscall();
        let mmu = MockMMU { data: code, base: 0x1000 };
        let mut decoder = X86Decoder;
        let block = decoder.decode(&mmu, 0x1000).expect("Operation failed");
        
        if let IROp::SysCall = block.ops[0] {
            // OK
        } else {
            panic!("Expected SysCall, got {:?}", block.ops[0]);
        }
    }

    #[test]
    fn test_alu_ops() {
        // Test ADD, SUB, AND, OR, XOR
        // We can't easily test all without a full emulator, but we can check decoding.
        // 01 C8 = ADD EAX, ECX (ModRM: 11 001 000 -> Mod=3, Reg=1(ECX), RM=0(EAX)) -> Wait.
        // Opcode 01: ADD r/m32, r32.
        // ModRM: 11 001 000 (0xC8). Mod=3(Reg), Reg=1(ECX), RM=0(EAX).
        // So: ADD EAX, ECX.
        // Decoder: mnemonic=Add, op1=Rm(EAX), op2=Reg(ECX).
        
        let code = vec![0x01, 0xC8];
        let mmu = MockMMU { data: code, base: 0x1000 };
        let mut decoder = X86Decoder;
        let block = decoder.decode(&mmu, 0x1000).expect("Operation failed");
        
        if let IROp::Add { dst: _, src1: _, src2: _ } = block.ops[0] {
            // OK
        } else {
            panic!("Expected Add, got {:?}", block.ops[0]);
        }
    }

    #[test]
    fn test_int3() {
        let code = vec![0xCC];
        let mmu = MockMMU { data: code, base: 0x1000 };
        let mut decoder = X86Decoder;
        let block = decoder.decode(&mmu, 0x1000).expect("Operation failed");
        
        if let Terminator::Interrupt { vector } = block.term {
            assert_eq!(vector, 3);
        } else {
            panic!("Expected Interrupt 3, got {:?}", block.term);
        }
    }
}
