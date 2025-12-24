//! x86-64 Operand and Instruction Types
//!
//! Defines operand types and instruction structure

use vm_core::GuestAddr;

#[derive(Debug, Clone)]
pub enum X86Operand {
    None,
    Reg(u8),
    Xmm(u8),
    Mem {
        base: Option<u8>,
        index: Option<u8>,
        scale: u8,
        disp: i64,
    },
    Imm(i64),
    Rel(i64),
}

pub struct X86Instruction {
    pub mnemonic: super::mnemonic::X86Mnemonic,
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

impl X86Instruction {
    pub fn next_pc(&self) -> GuestAddr {
        self.next_pc
    }

    pub fn size(&self) -> u8 {
        (self.next_pc.0.saturating_sub(0) % 16) as u8
    }

    pub fn operand_count(&self) -> usize {
        let mut count = 0;
        if !matches!(self.op1, X86Operand::None) {
            count += 1;
        }
        if !matches!(self.op2, X86Operand::None) {
            count += 1;
        }
        if !matches!(self.op3, X86Operand::None) {
            count += 1;
        }
        count
    }

    pub fn is_branch(&self) -> bool {
        matches!(
            self.mnemonic,
            super::mnemonic::X86Mnemonic::Jmp
                | super::mnemonic::X86Mnemonic::Call
                | super::mnemonic::X86Mnemonic::Ret
                | super::mnemonic::X86Mnemonic::Jcc
        )
    }

    pub fn is_call(&self) -> bool {
        matches!(self.mnemonic, super::mnemonic::X86Mnemonic::Call)
    }

    pub fn is_ret(&self) -> bool {
        matches!(self.mnemonic, super::mnemonic::X86Mnemonic::Ret)
    }

    pub fn is_jmp(&self) -> bool {
        matches!(self.mnemonic, super::mnemonic::X86Mnemonic::Jmp)
    }

    pub fn is_jcc(&self) -> bool {
        matches!(self.mnemonic, super::mnemonic::X86Mnemonic::Jcc)
    }
}
