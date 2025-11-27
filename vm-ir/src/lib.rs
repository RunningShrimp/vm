//! # vm-ir - 中间表示层
//!
//! 定义虚拟机的中间表示 (IR)，用于在前端解码器和后端执行引擎之间传递指令。
//!
//! ## 主要类型
//!
//! - [`IROp`]: IR 操作码枚举，包含算术、逻辑、内存、向量等操作
//! - [`IRBlock`]: IR 基本块，包含操作序列和终结符
//! - [`Terminator`]: 基本块终结符，如跳转、条件分支、返回等
//! - [`IRBuilder`]: 用于构建 IR 块的辅助工具
//!
//! ## 内存语义
//!
//! - [`MemFlags`]: 内存访问标志，支持原子操作和内存序
//! - [`MemOrder`]: 内存序枚举 (Acquire, Release, AcqRel)
//! - [`AtomicOp`]: 原子操作类型
//!
//! ## 示例
//!
//! ```rust,ignore
//! use vm_ir::{IRBuilder, IROp, Terminator};
//!
//! let mut builder = IRBuilder::new(0x1000);
//! builder.push(IROp::MovImm { dst: 1, imm: 42 });
//! builder.push(IROp::Add { dst: 2, src1: 1, src2: 1 });
//! builder.set_term(Terminator::Ret);
//! let block = builder.build();
//! ```

use vm_core::GuestAddr;

pub type RegId = u32;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AtomicOp {
    Add, Sub, And, Or, Xor, Xchg, CmpXchg, Min, Max, MinS, MaxS, Minu, Maxu, Swap
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
pub enum MemOrder { None, Acquire, Release, AcqRel, SeqCst }

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
    AtomicRMWOrder { dst: RegId, base: RegId, src: RegId, op: AtomicOp, size: u8, flags: MemFlags },
    AtomicCmpXchg { dst: RegId, base: RegId, expected: RegId, new: RegId, size: u8 },
    AtomicCmpXchgOrder { dst: RegId, base: RegId, expected: RegId, new: RegId, size: u8, flags: MemFlags },
    AtomicLoadReserve { dst: RegId, base: RegId, offset: i64, size: u8, flags: MemFlags },
    AtomicStoreCond { src: RegId, base: RegId, offset: i64, size: u8, dst_flag: RegId, flags: MemFlags },
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

    // Floating Point - Basic Operations
    Fadd { dst: RegId, src1: RegId, src2: RegId },
    Fsub { dst: RegId, src1: RegId, src2: RegId },
    Fmul { dst: RegId, src1: RegId, src2: RegId },
    Fdiv { dst: RegId, src1: RegId, src2: RegId },
    Fsqrt { dst: RegId, src: RegId },
    Fmin { dst: RegId, src1: RegId, src2: RegId },
    Fmax { dst: RegId, src1: RegId, src2: RegId },
    
    // Floating Point - Single Precision (F32)
    FaddS { dst: RegId, src1: RegId, src2: RegId },
    FsubS { dst: RegId, src1: RegId, src2: RegId },
    FmulS { dst: RegId, src1: RegId, src2: RegId },
    FdivS { dst: RegId, src1: RegId, src2: RegId },
    FsqrtS { dst: RegId, src: RegId },
    FminS { dst: RegId, src1: RegId, src2: RegId },
    FmaxS { dst: RegId, src1: RegId, src2: RegId },
    
    // Floating Point - Fused Multiply-Add
    Fmadd { dst: RegId, src1: RegId, src2: RegId, src3: RegId },
    Fmsub { dst: RegId, src1: RegId, src2: RegId, src3: RegId },
    Fnmadd { dst: RegId, src1: RegId, src2: RegId, src3: RegId },
    Fnmsub { dst: RegId, src1: RegId, src2: RegId, src3: RegId },
    FmaddS { dst: RegId, src1: RegId, src2: RegId, src3: RegId },
    FmsubS { dst: RegId, src1: RegId, src2: RegId, src3: RegId },
    FnmaddS { dst: RegId, src1: RegId, src2: RegId, src3: RegId },
    FnmsubS { dst: RegId, src1: RegId, src2: RegId, src3: RegId },
    
    // Floating Point - Comparisons
    Feq { dst: RegId, src1: RegId, src2: RegId },
    Flt { dst: RegId, src1: RegId, src2: RegId },
    Fle { dst: RegId, src1: RegId, src2: RegId },
    FeqS { dst: RegId, src1: RegId, src2: RegId },
    FltS { dst: RegId, src1: RegId, src2: RegId },
    FleS { dst: RegId, src1: RegId, src2: RegId },
    
    // Floating Point - Conversions
    Fcvtws { dst: RegId, src: RegId },   // F32 -> I32 (signed)
    Fcvtwus { dst: RegId, src: RegId },  // F32 -> I32 (unsigned)
    Fcvtls { dst: RegId, src: RegId },   // F32 -> I64 (signed)
    Fcvtlus { dst: RegId, src: RegId },  // F32 -> I64 (unsigned)
    Fcvtsw { dst: RegId, src: RegId },   // I32 (signed) -> F32
    Fcvtswu { dst: RegId, src: RegId },  // I32 (unsigned) -> F32
    Fcvtsl { dst: RegId, src: RegId },   // I64 (signed) -> F32
    Fcvtslu { dst: RegId, src: RegId },  // I64 (unsigned) -> F32
    Fcvtwd { dst: RegId, src: RegId },   // F64 -> I32 (signed)
    Fcvtwud { dst: RegId, src: RegId },  // F64 -> I32 (unsigned)
    Fcvtld { dst: RegId, src: RegId },   // F64 -> I64 (signed)
    Fcvtlud { dst: RegId, src: RegId },  // F64 -> I64 (unsigned)
    Fcvtdw { dst: RegId, src: RegId },   // I32 (signed) -> F64
    Fcvtdwu { dst: RegId, src: RegId },  // I32 (unsigned) -> F64
    Fcvtdl { dst: RegId, src: RegId },   // I64 (signed) -> F64
    Fcvtdlu { dst: RegId, src: RegId },  // I64 (unsigned) -> F64
    Fcvtsd { dst: RegId, src: RegId },   // F64 -> F32
    Fcvtds { dst: RegId, src: RegId },   // F32 -> F64
    
    // Floating Point - Sign Operations
    Fsgnj { dst: RegId, src1: RegId, src2: RegId },
    Fsgnjn { dst: RegId, src1: RegId, src2: RegId },
    Fsgnjx { dst: RegId, src1: RegId, src2: RegId },
    FsgnjS { dst: RegId, src1: RegId, src2: RegId },
    FsgnjnS { dst: RegId, src1: RegId, src2: RegId },
    FsgnjxS { dst: RegId, src1: RegId, src2: RegId },
    
    // Floating Point - Classification
    Fclass { dst: RegId, src: RegId },
    FclassS { dst: RegId, src: RegId },
    
    // Floating Point - Move between integer and float registers
    FmvXW { dst: RegId, src: RegId },    // FP -> Int (32-bit)
    FmvWX { dst: RegId, src: RegId },    // Int -> FP (32-bit)
    FmvXD { dst: RegId, src: RegId },    // FP -> Int (64-bit)
    FmvDX { dst: RegId, src: RegId },    // Int -> FP (64-bit)
    
    // Floating Point - Absolute/Negate
    Fabs { dst: RegId, src: RegId },
    Fneg { dst: RegId, src: RegId },
    FabsS { dst: RegId, src: RegId },
    FnegS { dst: RegId, src: RegId },
    
    // Floating Point - Load/Store
    Fload { dst: RegId, base: RegId, offset: i64, size: u8 },
    Fstore { src: RegId, base: RegId, offset: i64, size: u8 },
    
    // Branches (for direct translation)
    Beq { src1: RegId, src2: RegId, target: GuestAddr },
    Bne { src1: RegId, src2: RegId, target: GuestAddr },
    Blt { src1: RegId, src2: RegId, target: GuestAddr },
    Bge { src1: RegId, src2: RegId, target: GuestAddr },
    Bltu { src1: RegId, src2: RegId, target: GuestAddr },
    Bgeu { src1: RegId, src2: RegId, target: GuestAddr },
    
    // Atomic (high-level)
    Atomic { dst: RegId, base: RegId, src: RegId, op: AtomicOp, size: u8 },

    // System
    SysCall,
    DebugBreak,
    TlbFlush { vaddr: Option<GuestAddr> },
    CsrRead { dst: RegId, csr: u16 },
    CsrWrite { csr: u16, src: RegId },
    CsrSet { csr: u16, src: RegId },
    CsrClear { csr: u16, src: RegId },
    CsrWriteImm { csr: u16, imm: u32, dst: RegId },
    CsrSetImm { csr: u16, imm: u32, dst: RegId },
    CsrClearImm { csr: u16, imm: u32, dst: RegId },
    SysMret,
    SysSret,
    SysWfi,
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

#[derive(Clone)]
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
