//! ARM64 Instruction Types
//!
//! Defines ARM64 instruction structure and conditions

use vm_core::GuestAddr;

pub enum Cond {
    EQ = 0,
    NE = 1,
    CS = 2,
    CC = 3,
    MI = 4,
    PL = 5,
    VS = 6,
    VC = 7,
    HI = 8,
    LS = 9,
    GE = 10,
    LT = 11,
    GT = 12,
    LE = 13,
}

pub struct Arm64Instruction {
    pub mnemonic: &'static str,
    pub next_pc: GuestAddr,
    pub has_memory_op: bool,
    pub is_branch: bool,
}

impl Arm64Instruction {
    pub fn next_pc(&self) -> GuestAddr {
        self.next_pc
    }

    pub fn size(&self) -> u8 {
        4
    }

    pub fn operand_count(&self) -> usize {
        1
    }

    pub fn mnemonic(&self) -> &str {
        self.mnemonic
    }

    pub fn is_control_flow(&self) -> bool {
        self.is_branch
    }

    pub fn is_memory_access(&self) -> bool {
        self.has_memory_op
    }
}
