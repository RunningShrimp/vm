//! # vm-ir - 中间表示层
//!
//! 定义虚拟机的中间表示 (IR)，用于在前端解码器和后端执行引擎之间传递指令。
//!
//! ## 主要类型
//!
//! - [`IROp`][]: IR 操作码枚举，包含算术、逻辑、内存、向量等操作
//! - [`IRBlock`][]: IR 基本块，包含操作序列和终结符
//! - [`Terminator`][]: 基本块终结符，如跳转、条件分支、返回等
//! - [`IRBuilder`][]: 用于构建 IR 块的辅助工具
//! - [`DecodeCache`][]: 预解码缓存，用于缓存已解码的指令
//!
//! ## 内存语义
//!
//! - [`MemFlags`][]: 内存访问标志，支持原子操作和内存序
//! - [`MemOrder`][]: 内存序枚举 (Acquire, Release, AcqRel)
//! - [`AtomicOp`][]: 原子操作类型
//!
//! ## 示例
//!
//! ```rust,ignore
//! use vm_ir::{IRBuilder, IROp, Terminator, DecodeCache, MemFlags, MemOrder, AtomicOp};
//!
//! let mut builder = IRBuilder::new(0x1000);
//! builder.push(IROp::MovImm { dst: 1, imm: 42 });
//! builder.push(IROp::Add { dst: 2, src1: 1, src2: 1 });
//! builder.set_term(Terminator::Ret);
//! let block = builder.build();
//!
//! // 预解码缓存示例
//! let mut cache = DecodeCache::new(256);
//! cache.insert(0x1000, 8, vec![IROp::MovImm { dst: 0, imm: 42 }]);
//! let result = cache.get(0x1000, 8);
//! ```

mod decode_cache;
pub mod lift;
pub mod riscv_instruction_data;

pub use decode_cache::DecodeCache;
pub use riscv_instruction_data::{
    ExecutionUnitType, RiscvInstructionData, init_all_riscv_extension_data,
};
// Re-export GuestAddr and Architecture from vm-core/vm-error for public use
pub use vm_core::GuestAddr;
pub use vm_core::foundation::Architecture;

pub type RegId = u32;

/// Extension trait for RegId to provide constructor methods
pub trait RegIdExt {
    fn new(id: u32) -> Self;
    fn id(&self) -> u32;
}

impl RegIdExt for RegId {
    fn new(id: u32) -> Self {
        id
    }

    fn id(&self) -> u32 {
        *self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AtomicOp {
    Add,
    Sub,
    And,
    Or,
    Xor,
    Xchg,
    CmpXchg,
    Min,
    Max,
    MinS,
    MaxS,
    Minu,
    Maxu,
    Swap,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MemFlags {
    pub volatile: bool,
    pub atomic: bool,
    pub align: u8,
    pub fence_before: bool,
    pub fence_after: bool,
    pub order: MemOrder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub enum MemOrder {
    #[default]
    None,
    Acquire,
    Release,
    AcqRel,
    SeqCst,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum IROp {
    Nop,

    // Arithmetic / Logic
    Add {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },
    Sub {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },
    Mul {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },
    Div {
        dst: RegId,
        src1: RegId,
        src2: RegId,
        signed: bool,
    },
    Rem {
        dst: RegId,
        src1: RegId,
        src2: RegId,
        signed: bool,
    },

    And {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },
    Or {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },
    Xor {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },
    Not {
        dst: RegId,
        src: RegId,
    },

    // Shifts
    Sll {
        dst: RegId,
        src: RegId,
        shreg: RegId,
    },
    Srl {
        dst: RegId,
        src: RegId,
        shreg: RegId,
    },
    Sra {
        dst: RegId,
        src: RegId,
        shreg: RegId,
    },

    // Immediates
    AddImm {
        dst: RegId,
        src: RegId,
        imm: i64,
    },
    MulImm {
        dst: RegId,
        src: RegId,
        imm: i64,
    },
    Mov {
        dst: RegId,
        src: RegId,
    },
    MovImm {
        dst: RegId,
        imm: u64,
    },
    SllImm {
        dst: RegId,
        src: RegId,
        sh: u8,
    },
    SrlImm {
        dst: RegId,
        src: RegId,
        sh: u8,
    },
    SraImm {
        dst: RegId,
        src: RegId,
        sh: u8,
    },

    // Comparisons
    CmpEq {
        dst: RegId,
        lhs: RegId,
        rhs: RegId,
    },
    CmpNe {
        dst: RegId,
        lhs: RegId,
        rhs: RegId,
    },
    CmpLt {
        dst: RegId,
        lhs: RegId,
        rhs: RegId,
    },
    CmpLtU {
        dst: RegId,
        lhs: RegId,
        rhs: RegId,
    },
    CmpGe {
        dst: RegId,
        lhs: RegId,
        rhs: RegId,
    },
    CmpGeU {
        dst: RegId,
        lhs: RegId,
        rhs: RegId,
    },

    // Select
    Select {
        dst: RegId,
        cond: RegId,
        true_val: RegId,
        false_val: RegId,
    },

    // Memory
    Load {
        dst: RegId,
        base: RegId,
        offset: i64,
        size: u8,
        flags: MemFlags,
    },
    Store {
        src: RegId,
        base: RegId,
        offset: i64,
        size: u8,
        flags: MemFlags,
    },
    AtomicRMW {
        dst: RegId,
        base: RegId,
        src: RegId,
        op: AtomicOp,
        size: u8,
    },
    AtomicRMWOrder {
        dst: RegId,
        base: RegId,
        src: RegId,
        op: AtomicOp,
        size: u8,
        flags: MemFlags,
    },
    AtomicCmpXchg {
        dst: RegId,
        base: RegId,
        expected: RegId,
        new: RegId,
        size: u8,
    },
    AtomicCmpXchgOrder {
        dst: RegId,
        base: RegId,
        expected: RegId,
        new: RegId,
        size: u8,
        flags: MemFlags,
    },
    AtomicLoadReserve {
        dst: RegId,
        base: RegId,
        offset: i64,
        size: u8,
        flags: MemFlags,
    },
    AtomicStoreCond {
        src: RegId,
        base: RegId,
        offset: i64,
        size: u8,
        dst_flag: RegId,
        flags: MemFlags,
    },
    AtomicCmpXchgFlag {
        dst_old: RegId,
        dst_flag: RegId,
        base: RegId,
        expected: RegId,
        new: RegId,
        size: u8,
    },
    AtomicRmwFlag {
        dst_old: RegId,
        dst_flag: RegId,
        base: RegId,
        src: RegId,
        op: AtomicOp,
        size: u8,
    },

    // SIMD
    VecAdd {
        dst: RegId,
        src1: RegId,
        src2: RegId,
        element_size: u8,
    },
    VecSub {
        dst: RegId,
        src1: RegId,
        src2: RegId,
        element_size: u8,
    },
    VecMul {
        dst: RegId,
        src1: RegId,
        src2: RegId,
        element_size: u8,
    },
    VecAddSat {
        dst: RegId,
        src1: RegId,
        src2: RegId,
        element_size: u8,
        signed: bool,
    },
    VecSubSat {
        dst: RegId,
        src1: RegId,
        src2: RegId,
        element_size: u8,
        signed: bool,
    },
    VecMulSat {
        dst: RegId,
        src1: RegId,
        src2: RegId,
        element_size: u8,
        signed: bool,
    },
    Vec128Add {
        dst_lo: RegId,
        dst_hi: RegId,
        src1_lo: RegId,
        src1_hi: RegId,
        src2_lo: RegId,
        src2_hi: RegId,
        element_size: u8,
        signed: bool,
    },
    Vec256Add {
        dst0: RegId,
        dst1: RegId,
        dst2: RegId,
        dst3: RegId,
        src10: RegId,
        src11: RegId,
        src12: RegId,
        src13: RegId,
        src20: RegId,
        src21: RegId,
        src22: RegId,
        src23: RegId,
        element_size: u8,
        signed: bool,
    },
    Vec256Sub {
        dst0: RegId,
        dst1: RegId,
        dst2: RegId,
        dst3: RegId,
        src10: RegId,
        src11: RegId,
        src12: RegId,
        src13: RegId,
        src20: RegId,
        src21: RegId,
        src22: RegId,
        src23: RegId,
        element_size: u8,
        signed: bool,
    },
    Vec256Mul {
        dst0: RegId,
        dst1: RegId,
        dst2: RegId,
        dst3: RegId,
        src10: RegId,
        src11: RegId,
        src12: RegId,
        src13: RegId,
        src20: RegId,
        src21: RegId,
        src22: RegId,
        src23: RegId,
        element_size: u8,
        signed: bool,
    },
    Broadcast {
        dst: RegId,
        src: RegId,
        size: u8,
    },

    // Floating Point - Basic Operations
    Fadd {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },
    Fsub {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },
    Fmul {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },
    Fdiv {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },
    Fsqrt {
        dst: RegId,
        src: RegId,
    },
    Fmin {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },
    Fmax {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },

    // Floating Point - Single Precision (F32)
    FaddS {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },
    FsubS {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },
    FmulS {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },
    FdivS {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },
    FsqrtS {
        dst: RegId,
        src: RegId,
    },
    FminS {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },
    FmaxS {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },

    // Floating Point - Fused Multiply-Add
    Fmadd {
        dst: RegId,
        src1: RegId,
        src2: RegId,
        src3: RegId,
    },
    Fmsub {
        dst: RegId,
        src1: RegId,
        src2: RegId,
        src3: RegId,
    },
    Fnmadd {
        dst: RegId,
        src1: RegId,
        src2: RegId,
        src3: RegId,
    },
    Fnmsub {
        dst: RegId,
        src1: RegId,
        src2: RegId,
        src3: RegId,
    },
    FmaddS {
        dst: RegId,
        src1: RegId,
        src2: RegId,
        src3: RegId,
    },
    FmsubS {
        dst: RegId,
        src1: RegId,
        src2: RegId,
        src3: RegId,
    },
    FnmaddS {
        dst: RegId,
        src1: RegId,
        src2: RegId,
        src3: RegId,
    },
    FnmsubS {
        dst: RegId,
        src1: RegId,
        src2: RegId,
        src3: RegId,
    },

    // Floating Point - Comparisons
    Feq {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },
    Flt {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },
    Fle {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },
    FeqS {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },
    FltS {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },
    FleS {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },

    // Floating Point - Conversions
    Fcvtws {
        dst: RegId,
        src: RegId,
    }, // F32 -> I32 (signed)
    Fcvtwus {
        dst: RegId,
        src: RegId,
    }, // F32 -> I32 (unsigned)
    Fcvtls {
        dst: RegId,
        src: RegId,
    }, // F32 -> I64 (signed)
    Fcvtlus {
        dst: RegId,
        src: RegId,
    }, // F32 -> I64 (unsigned)
    Fcvtsw {
        dst: RegId,
        src: RegId,
    }, // I32 (signed) -> F32
    Fcvtswu {
        dst: RegId,
        src: RegId,
    }, // I32 (unsigned) -> F32
    Fcvtsl {
        dst: RegId,
        src: RegId,
    }, // I64 (signed) -> F32
    Fcvtslu {
        dst: RegId,
        src: RegId,
    }, // I64 (unsigned) -> F32
    Fcvtwd {
        dst: RegId,
        src: RegId,
    }, // F64 -> I32 (signed)
    Fcvtwud {
        dst: RegId,
        src: RegId,
    }, // F64 -> I32 (unsigned)
    Fcvtld {
        dst: RegId,
        src: RegId,
    }, // F64 -> I64 (signed)
    Fcvtlud {
        dst: RegId,
        src: RegId,
    }, // F64 -> I64 (unsigned)
    Fcvtdw {
        dst: RegId,
        src: RegId,
    }, // I32 (signed) -> F64
    Fcvtdwu {
        dst: RegId,
        src: RegId,
    }, // I32 (unsigned) -> F64
    Fcvtdl {
        dst: RegId,
        src: RegId,
    }, // I64 (signed) -> F64
    Fcvtdlu {
        dst: RegId,
        src: RegId,
    }, // I64 (unsigned) -> F64
    Fcvtsd {
        dst: RegId,
        src: RegId,
    }, // F64 -> F32
    Fcvtds {
        dst: RegId,
        src: RegId,
    }, // F32 -> F64

    // Floating Point - Sign Operations
    Fsgnj {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },
    Fsgnjn {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },
    Fsgnjx {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },
    FsgnjS {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },
    FsgnjnS {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },
    FsgnjxS {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },

    // Floating Point - Classification
    Fclass {
        dst: RegId,
        src: RegId,
    },
    FclassS {
        dst: RegId,
        src: RegId,
    },

    // Floating Point - Move between integer and float registers
    FmvXW {
        dst: RegId,
        src: RegId,
    }, // FP -> Int (32-bit)
    FmvWX {
        dst: RegId,
        src: RegId,
    }, // Int -> FP (32-bit)
    FmvXD {
        dst: RegId,
        src: RegId,
    }, // FP -> Int (64-bit)
    FmvDX {
        dst: RegId,
        src: RegId,
    }, // Int -> FP (64-bit)

    // Floating Point - Absolute/Negate
    Fabs {
        dst: RegId,
        src: RegId,
    },
    Fneg {
        dst: RegId,
        src: RegId,
    },
    FabsS {
        dst: RegId,
        src: RegId,
    },
    FnegS {
        dst: RegId,
        src: RegId,
    },

    // Floating Point - Load/Store
    Fload {
        dst: RegId,
        base: RegId,
        offset: i64,
        size: u8,
    },
    Fstore {
        src: RegId,
        base: RegId,
        offset: i64,
        size: u8,
    },

    // Branches (for direct translation)
    Beq {
        src1: RegId,
        src2: RegId,
        target: GuestAddr,
    },
    Bne {
        src1: RegId,
        src2: RegId,
        target: GuestAddr,
    },
    Blt {
        src1: RegId,
        src2: RegId,
        target: GuestAddr,
    },
    Bge {
        src1: RegId,
        src2: RegId,
        target: GuestAddr,
    },
    Bltu {
        src1: RegId,
        src2: RegId,
        target: GuestAddr,
    },
    Bgeu {
        src1: RegId,
        src2: RegId,
        target: GuestAddr,
    },

    // Atomic (high-level)
    Atomic {
        dst: RegId,
        base: RegId,
        src: RegId,
        op: AtomicOp,
        size: u8,
    },

    // System
    SysCall,
    DebugBreak,
    /// CPUID instruction - returns CPU feature information
    /// EAX contains the leaf (function), ECX contains sub-leaf
    /// Results are returned in EAX, EBX, ECX, EDX
    Cpuid {
        leaf: RegId,
        subleaf: RegId,
        dst_eax: RegId,
        dst_ebx: RegId,
        dst_ecx: RegId,
        dst_edx: RegId,
    },
    TlbFlush {
        vaddr: Option<GuestAddr>,
    },
    CsrRead {
        dst: RegId,
        csr: u16,
    },
    CsrWrite {
        csr: u16,
        src: RegId,
    },
    CsrSet {
        csr: u16,
        src: RegId,
    },
    CsrClear {
        csr: u16,
        src: RegId,
    },
    CsrWriteImm {
        csr: u16,
        imm: u32,
        dst: RegId,
    },
    CsrSetImm {
        csr: u16,
        imm: u32,
        dst: RegId,
    },
    CsrClearImm {
        csr: u16,
        imm: u32,
        dst: RegId,
    },
    SysMret,
    SysSret,
    SysWfi,

    // ARM64 PSTATE flags access
    /// Read PSTATE flags (NZCV) into a register
    ReadPstateFlags {
        dst: RegId,
    },
    /// Write PSTATE flags (NZCV) from a register
    WritePstateFlags {
        src: RegId,
    },
    /// Evaluate condition code (EQ, NE, CS, CC, MI, PL, VS, VC, HI, LS, GE, LT, GT, LE, AL, NV)
    EvalCondition {
        dst: RegId,
        cond: u8,
    },

    // Vendor-specific extensions
    /// Vendor-specific load operation (e.g., AMX tile load)
    VendorLoad {
        dst: RegId,
        base: RegId,
        offset: i64,
        vendor: String,
        tile_id: u8,
    },
    /// Vendor-specific store operation (e.g., AMX tile store)
    VendorStore {
        src: RegId,
        base: RegId,
        offset: i64,
        vendor: String,
        tile_id: u8,
    },
    /// Vendor-specific matrix operation (e.g., AMX FMA/MUL/ADD)
    VendorMatrixOp {
        dst: RegId,
        op: String,
        tile_c: u8,
        tile_a: u8,
        tile_b: u8,
        precision: String,
    },
    /// Vendor-specific configuration operation
    VendorConfig {
        vendor: String,
        tile_id: u8,
        config: String,
    },
    /// Vendor-specific vector operation
    VendorVectorOp {
        dst: RegId,
        op: String,
        src1: RegId,
        src2: RegId,
        vendor: String,
    },

    // ============================================================================
    // Cross-Architecture Translation Support - Control Flow
    // ============================================================================
    /// Unconditional branch to target (for cross-arch translation)
    Branch {
        target: Operand,
        link: bool,
    },

    /// Conditional branch (for cross-arch translation)
    CondBranch {
        condition: Operand,
        target: Operand,
        link: bool,
    },

    /// Generic binary operation for cross-arch translation
    BinaryOp {
        op: BinaryOperator,
        dest: RegId,
        src1: Operand,
        src2: Operand,
    },

    /// Extended Load with Operand address (for cross-arch translation)
    LoadExt {
        dest: RegId,
        addr: Operand,
        size: u8,
        flags: MemFlags,
    },

    /// Extended Store with Operand address (for cross-arch translation)
    StoreExt {
        value: Operand,
        addr: Operand,
        size: u8,
        flags: MemFlags,
    },
}

#[derive(Clone, Debug, Hash)]
pub enum Terminator {
    Ret,
    Jmp {
        target: GuestAddr,
    },
    JmpReg {
        base: RegId,
        offset: i64,
    },
    CondJmp {
        cond: RegId,
        target_true: GuestAddr,
        target_false: GuestAddr,
    },
    Call {
        target: GuestAddr,
        ret_pc: GuestAddr,
    },
    Fault {
        cause: u64,
    },
    Interrupt {
        vector: u32,
    },
}

#[derive(Clone, Debug)]
pub struct IRBlock {
    pub start_pc: GuestAddr,
    pub ops: Vec<IROp>,
    pub term: Terminator,
}

pub struct IRBuilder {
    pub block: IRBlock,
}

impl IRBuilder {
    pub fn new(pc: GuestAddr) -> Self {
        Self {
            block: IRBlock {
                start_pc: pc,
                ops: Vec::new(),
                term: Terminator::Fault { cause: 0 },
            },
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
    pub fn build_ref(&self) -> IRBlock {
        self.block.clone()
    }
    /// 获取当前程序计数器地址
    pub fn pc(&self) -> GuestAddr {
        self.block.start_pc
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
            RegisterMode::Standard => (self.mapping.len() as u32) + t,
            RegisterMode::SSA => (0xFFFF << 16) | (t & 0xFFFF),
        }
    }
}

// ============================================================================
// Cross-Architecture Translation Support Types
// ============================================================================

/// IRInstruction - Alias for IROp for cross-arch translation compatibility
pub type IRInstruction = IROp;

/// Block - Alias for IRBlock for backward compatibility
pub type Block = IRBlock;

/// Operand types for IR instructions
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Operand {
    /// Register operand
    Register(RegId),
    /// Immediate value
    Immediate(i64),
    /// Memory address
    Memory { base: RegId, offset: i64, size: u8 },
    /// No operand
    None,
    /// Binary operation (for address calculations)
    Binary {
        op: BinaryOperator,
        left: Box<Operand>,
        right: Box<Operand>,
    },
    /// Register operand (alias for cross-arch compatibility)
    Reg(RegId),
    /// Immediate operand (alias for cross-arch compatibility, accepts u64)
    Imm64(u64),
}

// Compatibility methods for cross-arch translation
impl Operand {
    /// Get Register value if this is a Register operand
    pub fn as_reg(&self) -> Option<RegId> {
        match self {
            Operand::Register(reg) | Operand::Reg(reg) => Some(*reg),
            _ => None,
        }
    }

    /// Get Immediate value if this is an Immediate operand
    pub fn as_imm(&self) -> Option<i64> {
        match self {
            Operand::Immediate(imm) => Some(*imm),
            Operand::Imm64(imm) => Some(*imm as i64),
            _ => None,
        }
    }
}

/// Binary operator types for cross-arch translation
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinaryOperator {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Rem,

    // Logical
    And,
    Or,
    Xor,

    // Shifts
    ShiftLeft,
    ShiftRightLogical,
    ShiftRightArithmetic,

    // Comparisons
    Equal,
    NotEqual,
    LessThan,
    LessThanUnsigned,
    GreaterEqual,
    GreaterEqualUnsigned,

    // Floating-point
    FAdd,
    FSub,
    FMul,
    FDiv,

    // Custom
    Custom(u8),
}

impl BinaryOperator {
    /// Convert from IROp to BinaryOperator if applicable
    pub fn from_irop(op: &IROp) -> Option<Self> {
        match op {
            IROp::Add { .. } => Some(BinaryOperator::Add),
            IROp::Sub { .. } => Some(BinaryOperator::Sub),
            IROp::Mul { .. } => Some(BinaryOperator::Mul),
            IROp::Div { .. } => Some(BinaryOperator::Div),
            IROp::Rem { .. } => Some(BinaryOperator::Rem),
            IROp::And { .. } => Some(BinaryOperator::And),
            IROp::Or { .. } => Some(BinaryOperator::Or),
            IROp::Xor { .. } => Some(BinaryOperator::Xor),
            IROp::Sll { .. } => Some(BinaryOperator::ShiftLeft),
            IROp::Srl { .. } => Some(BinaryOperator::ShiftRightLogical),
            IROp::Sra { .. } => Some(BinaryOperator::ShiftRightArithmetic),
            IROp::CmpEq { .. } => Some(BinaryOperator::Equal),
            IROp::CmpNe { .. } => Some(BinaryOperator::NotEqual),
            IROp::CmpLt { .. } => Some(BinaryOperator::LessThan),
            IROp::CmpLtU { .. } => Some(BinaryOperator::LessThanUnsigned),
            IROp::CmpGe { .. } => Some(BinaryOperator::GreaterEqual),
            IROp::CmpGeU { .. } => Some(BinaryOperator::GreaterEqualUnsigned),
            IROp::Fadd { .. } => Some(BinaryOperator::FAdd),
            IROp::Fsub { .. } => Some(BinaryOperator::FSub),
            IROp::Fmul { .. } => Some(BinaryOperator::FMul),
            IROp::Fdiv { .. } => Some(BinaryOperator::FDiv),
            _ => None,
        }
    }

    /// Get mnemonic for this operator
    pub fn mnemonic(&self) -> &'static str {
        match self {
            BinaryOperator::Add => "add",
            BinaryOperator::Sub => "sub",
            BinaryOperator::Mul => "mul",
            BinaryOperator::Div => "div",
            BinaryOperator::Rem => "rem",
            BinaryOperator::And => "and",
            BinaryOperator::Or => "or",
            BinaryOperator::Xor => "xor",
            BinaryOperator::ShiftLeft => "sll",
            BinaryOperator::ShiftRightLogical => "srl",
            BinaryOperator::ShiftRightArithmetic => "sra",
            BinaryOperator::Equal => "cmp.eq",
            BinaryOperator::NotEqual => "cmp.ne",
            BinaryOperator::LessThan => "cmp.lt",
            BinaryOperator::LessThanUnsigned => "cmp.ltu",
            BinaryOperator::GreaterEqual => "cmp.ge",
            BinaryOperator::GreaterEqualUnsigned => "cmp.geu",
            BinaryOperator::FAdd => "fadd",
            BinaryOperator::FSub => "fsub",
            BinaryOperator::FMul => "fmul",
            BinaryOperator::FDiv => "fdiv",
            BinaryOperator::Custom(_) => "custom",
        }
    }
}
