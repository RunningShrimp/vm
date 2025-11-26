use vm_core::GuestAddr;

pub type RegId = u32;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AtomicOp {
    Add, Sub, And, Or, Xor, Xchg, CmpXchg, Min, Max, MinS, MaxS
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MemFlags {
    pub volatile: bool,
    pub atomic: bool,
    pub align: u8,
    pub fence_before: bool,
    pub fence_after: bool,
    pub order: MemOrder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemOrder { None, Acquire, Release, AcqRel }

impl Default for MemOrder { fn default() -> Self { MemOrder::None } }

#[derive(Clone, Debug)]
pub enum IROp {
    Nop,
    
    // Arithmetic / Logic
    Add { dst: RegId, src1: RegId, src2: RegId },
    Sub { dst: RegId, src1: RegId, src2: RegId },
    Mul { dst: RegId, src1: RegId, src2: RegId },
    Div { dst: RegId, src1: RegId, src2: RegId, signed: bool },
    Rem { dst: RegId, src1: RegId, src2: RegId, signed: bool },
    
    And { dst: RegId, src1: RegId, src2: RegId },
    Or { dst: RegId, src1: RegId, src2: RegId },
    Xor { dst: RegId, src1: RegId, src2: RegId },
    Not { dst: RegId, src: RegId },
    
    // Shifts
    Sll { dst: RegId, src: RegId, shreg: RegId },
    Srl { dst: RegId, src: RegId, shreg: RegId },
    Sra { dst: RegId, src: RegId, shreg: RegId },
    
    // Immediates
    AddImm { dst: RegId, src: RegId, imm: i64 },
    MulImm { dst: RegId, src: RegId, imm: i64 },
    MovImm { dst: RegId, imm: u64 },
    SllImm { dst: RegId, src: RegId, sh: u8 },
    SrlImm { dst: RegId, src: RegId, sh: u8 },
    SraImm { dst: RegId, src: RegId, sh: u8 },
    
    // Comparisons
    CmpEq { dst: RegId, lhs: RegId, rhs: RegId },
    CmpNe { dst: RegId, lhs: RegId, rhs: RegId },
    CmpLt { dst: RegId, lhs: RegId, rhs: RegId }, 
    CmpLtU { dst: RegId, lhs: RegId, rhs: RegId }, 
    CmpGe { dst: RegId, lhs: RegId, rhs: RegId }, 
    CmpGeU { dst: RegId, lhs: RegId, rhs: RegId }, 
    
    // Select
    Select { dst: RegId, cond: RegId, true_val: RegId, false_val: RegId },

    // Memory
    Load { dst: RegId, base: RegId, offset: i64, size: u8, flags: MemFlags },
    Store { src: RegId, base: RegId, offset: i64, size: u8, flags: MemFlags },
    AtomicRMW { dst: RegId, base: RegId, src: RegId, op: AtomicOp, size: u8 },
    AtomicCmpXchg { dst: RegId, base: RegId, expected: RegId, new: RegId, size: u8 },
    AtomicCmpXchgFlag { dst_old: RegId, dst_flag: RegId, base: RegId, expected: RegId, new: RegId, size: u8 },
    AtomicRmwFlag { dst_old: RegId, dst_flag: RegId, base: RegId, src: RegId, op: AtomicOp, size: u8 },
    
    // SIMD
    VecAdd { dst: RegId, src1: RegId, src2: RegId, element_size: u8 },
    VecSub { dst: RegId, src1: RegId, src2: RegId, element_size: u8 },
    VecMul { dst: RegId, src1: RegId, src2: RegId, element_size: u8 },
    VecAddSat { dst: RegId, src1: RegId, src2: RegId, element_size: u8, signed: bool },
    VecSubSat { dst: RegId, src1: RegId, src2: RegId, element_size: u8, signed: bool },
    VecMulSat { dst: RegId, src1: RegId, src2: RegId, element_size: u8, signed: bool },
    Vec128Add { dst_lo: RegId, dst_hi: RegId, src1_lo: RegId, src1_hi: RegId, src2_lo: RegId, src2_hi: RegId, element_size: u8, signed: bool },
    Vec256Add { dst0: RegId, dst1: RegId, dst2: RegId, dst3: RegId, src10: RegId, src11: RegId, src12: RegId, src13: RegId, src20: RegId, src21: RegId, src22: RegId, src23: RegId, element_size: u8, signed: bool },
    Vec256Sub { dst0: RegId, dst1: RegId, dst2: RegId, dst3: RegId, src10: RegId, src11: RegId, src12: RegId, src13: RegId, src20: RegId, src21: RegId, src22: RegId, src23: RegId, element_size: u8, signed: bool },
    Vec256Mul { dst0: RegId, dst1: RegId, dst2: RegId, dst3: RegId, src10: RegId, src11: RegId, src12: RegId, src13: RegId, src20: RegId, src21: RegId, src22: RegId, src23: RegId, element_size: u8, signed: bool },

    // System
    SysCall,
    DebugBreak,
    TlbFlush { vaddr: Option<GuestAddr> },
}

#[derive(Clone, Debug)]
pub enum Terminator {
    Ret,
    Jmp { target: GuestAddr },
    JmpReg { base: RegId, offset: i64 },
    CondJmp { cond: RegId, target_true: GuestAddr, target_false: GuestAddr },
    Call { target: GuestAddr, ret_pc: GuestAddr },
    Fault { cause: u64 },
    Interrupt { vector: u32 },
}

pub struct IRBlock {
    pub start_pc: GuestAddr,
    pub ops: Vec<IROp>,
    pub term: Terminator,
}

pub struct IRBuilder {
    block: IRBlock,
}

impl IRBuilder {
    pub fn new(pc: GuestAddr) -> Self {
        Self {
            block: IRBlock {
                start_pc: pc,
                ops: Vec::new(),
                term: Terminator::Fault { cause: 0 },
            }
        }
    }
    pub fn push(&mut self, op: IROp) {
        self.block.ops.push(op);
    }
    pub fn set_term(&mut self, term: Terminator) {
        self.block.term = term;
    }
    pub fn build(self) -> IRBlock {
        self.block
    }
}

// Register File Abstraction
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum RegisterMode {
    Standard,
    SSA,
}

pub struct RegisterFile {
    mode: RegisterMode,
    mapping: Vec<RegId>,
    versions: Vec<u32>,
    next_temp: RegId,
}

impl RegisterFile {
    pub fn new(guest_regs: usize, mode: RegisterMode) -> Self {
        let mut mapping = Vec::with_capacity(guest_regs);
        for i in 0..guest_regs {
            mapping.push(i as RegId);
        }
        Self {
            mode,
            mapping,
            versions: vec![0; guest_regs],
            next_temp: 0,
        }
    }

    pub fn read(&self, guest: usize) -> RegId {
        if guest < self.mapping.len() {
            self.mapping[guest]
        } else {
            0
        }
    }

    pub fn write(&mut self, guest: usize) -> RegId {
        if guest >= self.mapping.len() {
            return 0;
        }
        match self.mode {
            RegisterMode::Standard => self.mapping[guest],
            RegisterMode::SSA => {
                self.versions[guest] += 1;
                let ver = self.versions[guest];
                let reg = ((guest as u32) << 16) | (ver & 0xFFFF);
                self.mapping[guest] = reg;
                reg
            }
        }
    }

    pub fn alloc_temp(&mut self) -> RegId {
        let t = self.next_temp;
        self.next_temp += 1;
        match self.mode {
            RegisterMode::Standard => {
                (self.mapping.len() as u32) + t
            }
            RegisterMode::SSA => {
                (0xFFFF << 16) | (t & 0xFFFF)
            }
        }
    }
}
