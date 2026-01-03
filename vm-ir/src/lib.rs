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

// Serde imports for serialization support
use serde::{Deserialize, Serialize};

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

/// 原子操作类型
///
/// 定义支持的原子读-修改-写(RMW)操作类型，用于多线程同步和锁实现。
///
/// # 使用场景
///
/// - 多线程同步：实现锁、信号量等同步原语
/// - 无锁数据结构：实现无锁队列、栈等
/// - 内存序保证：配合 [`MemOrder`] 确保内存访问顺序
///
/// # 变体说明
///
/// - **算术操作**: `Add`, `Sub` - 原子加法、减法
/// - **位操作**: `And`, `Or`, `Xor` - 原子位运算
/// - **交换操作**: `Xchg`, `Swap` - 原子交换
/// - **比较交换**: `CmpXchg` - 原子比较并交换(CAS)
/// - **最小/最大**: `Min`, `Max`, `MinS`, `MaxS`, `Minu`, `Maxu` - 原子最小/最大值（有符号/无符号）
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AtomicOp {
    /// 原子加法
    Add,
    /// 原子减法
    Sub,
    /// 原子与操作
    And,
    /// 原子或操作
    Or,
    /// 原子异或操作
    Xor,
    /// 原子交换
    Xchg,
    /// 原子比较并交换
    CmpXchg,
    /// 原子最小值
    Min,
    /// 原子最大值
    Max,
    /// 原子最小值（有符号）
    MinS,
    /// 原子最大值（有符号）
    MaxS,
    /// 原子最小值（无符号）
    Minu,
    /// 原子最大值（无符号）
    Maxu,
    /// 原子交换（别名）
    Swap,
}

/// 内存访问标志
///
/// 定义内存访问的属性和约束，用于volatile、原子操作和内存屏障。
///
/// # 字段
///
/// - `volatile`: volatile访问，禁止编译器优化（如重排、消除）
/// - `atomic`: 原子操作，需要特殊的内存序保证
/// - `align`: 对齐要求（以字节为单位，0表示自然对齐）
/// - `fence_before`: 操作前插入内存屏障
/// - `fence_after`: 操作后插入内存屏障
/// - `order`: 内存序类型（Acquire/Release/AcqRel/SeqCst）
///
/// # 示例
///
/// ```
/// use vm_ir::{MemFlags, MemOrder};
///
/// // 普通内存访问
/// let normal = MemFlags::default();
///
/// // volatile写（设备寄存器）
/// let volatile_write = MemFlags {
///     volatile: true,
///     ..Default::default()
/// };
///
/// // 原子操作（Acquire-Release语义）
/// let atomic_ar = MemFlags {
///     atomic: true,
///     order: MemOrder::AcqRel,
///     ..Default::default()
/// };
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MemFlags {
    /// volatile访问标志
    pub volatile: bool,
    /// 原子操作标志
    pub atomic: bool,
    /// 对齐要求（字节，0=自然对齐）
    pub align: u8,
    /// 操作前内存屏障
    pub fence_before: bool,
    /// 操作后内存屏障
    pub fence_after: bool,
    /// 内存序类型
    pub order: MemOrder,
}

/// 内存序类型
///
/// 定义原子操作的内存序约束，控制内存访问的可见性和重排序规则。
/// 这些对应C++20/C11的内存序模型。
///
/// # 变体说明
///
/// - `None`: 无特殊约束，允许任意重排（仅用于非原子操作）
/// - `Acquire`: 获取语义，确保后续读操作不会被重排到此操作之前
/// - `Release`: 释放语义，确保前面的写操作不会被重排到此操作之后
/// - `AcqRel`: 获取-释放语义，同时具有Acquire和Release的保证
/// - `SeqCst`: 顺序一致性，最强的内存序保证，所有线程看到一致的操作顺序
///
/// # 使用场景
///
/// - `Acquire`: 用于加载操作，如获取锁后的数据读取
/// - `Release`: 用于存储操作，如释放锁前的数据写入
/// - `AcqRel`: 用于读-修改-写操作，如原子加法
/// - `SeqCst`: 用于需要全局顺序一致性的场景，如顺序锁
///
/// # 示例
///
/// ```
/// use vm_ir::MemOrder;
///
/// // 加载-获取（对应C++的 memory_order_acquire）
/// let load_order = MemOrder::Acquire;
///
/// // 存储-释放（对应C++的 memory_order_release）
/// let store_order = MemOrder::Release;
///
/// // 顺序一致性（对应C++的 memory_order_seq_cst）
/// let sc_order = MemOrder::SeqCst;
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash, Serialize, Deserialize)]
pub enum MemOrder {
    /// 无内存序约束
    #[default]
    None,
    /// 获取语义（Acquire）
    Acquire,
    /// 释放语义（Release）
    Release,
    /// 获取-释放语义（Acquire-Release）
    AcqRel,
    /// 顺序一致性（Sequentially Consistent）
    SeqCst,
}

/// IR操作码
///
/// 定义虚拟机中间表示(IR)的所有操作类型。IROp是VM的前端(解码器)和后端(执行引擎/JIT)之间的桥梁。
///
/// # 设计目标
///
/// - **架构无关**: 支持RISC-V、ARM64、x86-64等多种架构
/// - **类型安全**: 使用强类型确保操作数正确性
/// - **可扩展**: 易于添加新的操作类型
/// - **优化友好**: 便于进行各种编译器优化
///
/// # 操作分类
///
/// ## 1. 算术/逻辑操作
/// - 基础算术: [`Add`](Self::Add), [`Sub`](Self::Sub), [`Mul`](Self::Mul), [`Div`](Self::Div),
///   [`Rem`](Self::Rem)
/// - 位操作: [`And`](Self::And), [`Or`](Self::Or), [`Xor`](Self::Xor), [`Not`](Self::Not)
/// - 移位: [`Sll`](Self::Sll), [`Srl`](Self::Srl), [`Sra`](Self::Sra)
/// - 立即数: [`AddImm`](Self::AddImm), [`MovImm`](Self::MovImm), [`SllImm`](Self::SllImm), 等
///
/// ## 2. 比较操作
/// - [`CmpEq`](Self::CmpEq), [`CmpNe`](Self::CmpNe), [`CmpLt`](Self::CmpLt), [`CmpGe`](Self::CmpGe)
/// - [`Select`](Self::Select): 条件选择（三元操作符）
///
/// ## 3. 内存操作
/// - 加载/存储: [`Load`](Self::Load), [`Store`](Self::Store)
/// - 原子操作: [`AtomicRMW`](Self::AtomicRMW), [`AtomicCmpXchg`](Self::AtomicCmpXchg)
/// - LR/SC: [`AtomicLoadReserve`](Self::AtomicLoadReserve),
///   [`AtomicStoreCond`](Self::AtomicStoreCond)
///
/// ## 4. 浮点操作
/// - 基础运算: [`Fadd`](Self::Fadd), [`Fsub`](Self::Fsub), [`Fmul`](Self::Fmul),
///   [`Fdiv`](Self::Fdiv)
/// - 融合乘加: [`Fmadd`](Self::Fmadd), [`Fmsub`](Self::Fmsub), 等
/// - 比较: [`Feq`](Self::Feq), [`Flt`](Self::Flt), [`Fle`](Self::Fle)
/// - 转换: [`Fcvtws`](Self::Fcvtws), [`Fcvtsd`](Self::Fcvtsd), 等
///
/// ## 5. SIMD操作
/// - 向量算术: [`VecAdd`](Self::VecAdd), [`VecSub`](Self::VecSub), [`VecMul`](Self::VecMul)
/// - 饱和运算: [`VecAddSat`](Self::VecAddSat), [`VecSubSat`](Self::VecSubSat)
/// - 广播: `Broadcast`
///
/// ## 6. 分支操作
/// - 条件分支: [`Beq`](Self::Beq), [`Bne`](Self::Bne), [`Blt`](Self::Blt), 等
/// - 通用分支: [`Branch`](Self::Branch), [`CondBranch`](Self::CondBranch)
///
/// ## 7. 系统操作
/// - 系统调用: [`SysCall`](Self::SysCall)
/// - CSR访问: [`CsrRead`](Self::CsrRead), [`CsrWrite`](Self::CsrWrite)
/// - 特权指令: [`SysMret`](Self::SysMret), [`SysSret`](Self::SysSret)
/// - TLB管理: [`TlbFlush`](Self::TlbFlush)
///
/// ## 8. 扩展操作
/// - CPUID: [`Cpuid`](Self::Cpuid)
/// - ARM64 PSTATE: [`ReadPstateFlags`](Self::ReadPstateFlags),
///   [`EvalCondition`](Self::EvalCondition)
/// - 厂商特定: [`VendorLoad`](Self::VendorLoad), [`VendorMatrixOp`](Self::VendorMatrixOp)
///
/// # 示例
///
/// ```
/// use vm_ir::{IROp, RegId};
///
/// // 算术操作: r1 = r2 + r3
/// let add = IROp::Add {
///     dst: 1,
///     src1: 2,
///     src2: 3,
/// };
///
/// // 立即数加载: r1 = 42
/// let movi = IROp::MovImm { dst: 1, imm: 42 };
///
/// // 内存加载: r1 = [r2 + 8]
/// let load = IROp::Load {
///     dst: 1,
///     base: 2,
///     offset: 8,
///     size: 8,
///     flags: vm_ir::MemFlags::default(),
/// };
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    // 向量按位操作
    VecAnd {
        dst: RegId,
        src1: RegId,
        src2: RegId,
        element_size: u8,
    },
    VecOr {
        dst: RegId,
        src1: RegId,
        src2: RegId,
        element_size: u8,
    },
    VecXor {
        dst: RegId,
        src1: RegId,
        src2: RegId,
        element_size: u8,
    },
    VecNot {
        dst: RegId,
        src: RegId,
        element_size: u8,
    },
    // 向量移位操作
    VecShl {
        dst: RegId,
        src: RegId,
        shift: RegId,
        element_size: u8,
    },
    VecSrl {
        dst: RegId,
        src: RegId,
        shift: RegId,
        element_size: u8,
    },
    VecSra {
        dst: RegId,
        src: RegId,
        shift: RegId,
        element_size: u8,
    },
    // 向量立即数移位
    VecShlImm {
        dst: RegId,
        src: RegId,
        shift: u8,
        element_size: u8,
    },
    VecSrlImm {
        dst: RegId,
        src: RegId,
        shift: u8,
        element_size: u8,
    },
    VecSraImm {
        dst: RegId,
        src: RegId,
        shift: u8,
        element_size: u8,
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

/// 基本块终结符
///
/// 定义IR基本块的终止方式，控制程序流从一个基本块转移到下一个基本块。
/// 每个IRBlock必须有且仅有一个终结符。
///
/// # 终结符类型
///
/// ## 控制流终结符
/// - [`Ret`](Self::Ret): 函数返回
/// - [`Jmp`](Self::Jmp): 无条件跳转到固定地址
/// - [`JmpReg`](Self::JmpReg): 间接跳转（寄存器+偏移）
/// - [`CondJmp`](Self::CondJmp): 条件分支
/// - [`Call`](Self::Call): 函数调用
///
/// ## 异常终结符
/// - [`Fault`](Self::Fault): 执行故障（如页面错误、非法指令）
/// - [`Interrupt`](Self::Interrupt): 中断/异常
///
/// # 示例
///
/// ```
/// use vm_ir::{GuestAddr, RegId, Terminator};
///
/// // 函数返回
/// let ret = Terminator::Ret;
///
/// // 无条件跳转
/// let jmp = Terminator::Jmp {
///     target: GuestAddr(0x2000),
/// };
///
/// // 条件分支（if-else）
/// let condjmp = Terminator::CondJmp {
///     cond: 1, // 条件寄存器
///     target_true: GuestAddr(0x2000),
///     target_false: GuestAddr(0x3000),
/// };
///
/// // 函数调用
/// let call = Terminator::Call {
///     target: GuestAddr(0x5000),
///     ret_pc: GuestAddr(0x1008),
/// };
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Terminator {
    /// 函数返回
    ///
    /// 终止当前基本块并返回到调用者。
    Ret,

    /// 无条件跳转
    ///
    /// 跳转到指定的目标地址。
    ///
    /// # 字段
    /// - `target`: 跳转目标地址
    Jmp {
        /// 目标地址
        target: GuestAddr,
    },

    /// 间接跳转
    ///
    /// 跳转到寄存器+偏移量指定的地址。用于实现 switch-case、函数指针等。
    ///
    /// # 字段
    /// - `base`: 基址寄存器
    /// - `offset`: 偏移量
    JmpReg {
        /// 基址寄存器
        base: RegId,
        /// 偏移量
        offset: i64,
    },

    /// 条件分支
    ///
    /// 根据条件寄存器的值选择两个分支目标之一。
    /// 用于实现 if-else、三元操作符等。
    ///
    /// # 字段
    /// - `cond`: 条件寄存器（非零为真）
    /// - `target_true`: 条件为真时的跳转目标
    /// - `target_false`: 条件为假时的跳转目标
    CondJmp {
        /// 条件寄存器
        cond: RegId,
        /// 条件为真的跳转目标
        target_true: GuestAddr,
        /// 条件为假的跳转目标
        target_false: GuestAddr,
    },

    /// 函数调用
    ///
    /// 调用指定地址的函数，并保存返回地址。
    ///
    /// # 字段
    /// - `target`: 被调用函数的地址
    /// - `ret_pc`: 返回地址
    Call {
        /// 调用目标地址
        target: GuestAddr,
        /// 返回地址
        ret_pc: GuestAddr,
    },

    /// 执行故障
    ///
    /// 表示执行过程中发生错误，需要由上层处理。
    ///
    /// # 字段
    /// - `cause`: 故障原因码
    Fault {
        /// 故障原因码
        cause: u64,
    },

    /// 中断/异常
    ///
    /// 表示发生外部中断或内部异常。
    ///
    /// # 字段
    /// - `vector`: 中断向量号
    Interrupt {
        /// 中断向量号
        vector: u32,
    },
}

/// IR basic block - represents a sequence of operations with a terminator
///
/// An IRBlock is a fundamental unit of intermediate representation containing:
/// - A starting program counter address
/// - A sequence of IR operations
/// - A terminator that defines control flow
///
/// # Example
///
/// ```rust
/// use vm_ir::{IRBuilder, IROp, Terminator};
///
/// let mut builder = IRBuilder::new(0x1000);
/// builder.push(IROp::MovImm { dst: 1, imm: 42 });
/// builder.set_term(Terminator::Ret);
/// let block = builder.build();
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IRBlock {
    /// Starting program counter address
    pub start_pc: GuestAddr,
    /// Sequence of IR operations
    pub ops: Vec<IROp>,
    /// Block terminator (defines control flow)
    pub term: Terminator,
}

impl IRBlock {
    /// Create a new IR block with the given starting address
    pub fn new(start_pc: GuestAddr) -> Self {
        Self {
            start_pc,
            ops: Vec::new(),
            term: Terminator::Ret,
        }
    }

    /// Get the number of operations in this block
    pub fn op_count(&self) -> usize {
        self.ops.len()
    }

    /// Check if the block is empty (no operations)
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Get an iterator over the operations in this block
    pub fn iter_ops(&self) -> impl Iterator<Item = &IROp> {
        self.ops.iter()
    }

    /// Validate the IR block structure
    ///
    /// Returns Ok(()) if the block is valid, Err with description if invalid
    pub fn validate(&self) -> Result<(), String> {
        // Check if terminator is valid
        if let Terminator::CondJmp {
            cond,
            target_true: _,
            target_false: _,
        } = &self.term
        {
            // Ensure condition register is valid
            if *cond == u32::MAX {
                return Err("Invalid condition register in CondJmp".to_string());
            }
        }

        Ok(())
    }

    /// Calculate estimated size in bytes
    ///
    /// This provides a rough estimate of memory usage
    pub fn estimated_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + (self.ops.len() * std::mem::size_of::<IROp>())
            + std::mem::size_of::<Terminator>()
    }
}

/// Builder for constructing IR blocks
///
/// IRBuilder provides a convenient interface for building IR blocks
/// with proper initialization and structure.
pub struct IRBuilder {
    pub block: IRBlock,
}

impl IRBuilder {
    /// Create a new IR builder for the given program counter address
    pub fn new(pc: GuestAddr) -> Self {
        Self {
            block: IRBlock {
                start_pc: pc,
                ops: Vec::new(),
                term: Terminator::Fault { cause: 0 },
            },
        }
    }

    /// Add an IR operation to the block
    pub fn push(&mut self, op: IROp) {
        self.block.ops.push(op);
    }

    /// Add multiple IR operations to the block
    pub fn push_all(&mut self, ops: impl IntoIterator<Item = IROp>) {
        self.block.ops.extend(ops);
    }

    /// Set the block terminator
    pub fn set_term(&mut self, term: Terminator) {
        self.block.term = term;
    }

    /// Build and consume the builder, returning the IR block
    pub fn build(self) -> IRBlock {
        self.block
    }

    /// Build a reference to the current IR block (clones the block)
    pub fn build_ref(&self) -> IRBlock {
        self.block.clone()
    }

    /// Get the current program counter address
    pub fn pc(&self) -> GuestAddr {
        self.block.start_pc
    }

    /// Get the number of operations in the current block
    pub fn op_count(&self) -> usize {
        self.block.ops.len()
    }

    /// Check if the current block is empty
    pub fn is_empty(&self) -> bool {
        self.block.ops.is_empty()
    }
}

// ============================================================================
// IR Utility Functions
// ============================================================================

/// IR validation and utility functions
#[allow(dead_code)]
mod ir_utils {
    use super::*;

    /// Validate an IR operation
    ///
    /// Checks if the operation is well-formed (e.g., valid register IDs,
    /// sensible immediate values, etc.)
    #[allow(dead_code)]
    pub fn validate_op(op: &IROp) -> Result<(), String> {
        match op {
            IROp::Load { size, .. } | IROp::Store { size, .. } => {
                if *size == 0 || *size > 16 {
                    return Err(format!("Invalid memory size: {}", size));
                }
            }
            IROp::Div { src2, .. } | IROp::Rem { src2, .. } => {
                if *src2 == 0 {
                    return Err("Division by zero register".to_string());
                }
            }
            IROp::SllImm { sh, .. } | IROp::SrlImm { sh, .. } | IROp::SraImm { sh, .. } => {
                if *sh > 64 {
                    return Err(format!("Invalid shift amount: {}", sh));
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Validate a sequence of IR operations
    pub fn validate_ops(ops: &[IROp]) -> Result<(), String> {
        for (i, op) in ops.iter().enumerate() {
            validate_op(op).map_err(|e| format!("Operation {}: {}", i, e))?;
        }
        Ok(())
    }

    /// Get a human-readable name for an IR operation
    pub fn op_name(op: &IROp) -> &'static str {
        match op {
            IROp::Nop => "nop",
            IROp::Add { .. } => "add",
            IROp::Sub { .. } => "sub",
            IROp::Mul { .. } => "mul",
            IROp::Div { .. } => "div",
            IROp::Rem { .. } => "rem",
            IROp::And { .. } => "and",
            IROp::Or { .. } => "or",
            IROp::Xor { .. } => "xor",
            IROp::Not { .. } => "not",
            IROp::Sll { .. } => "sll",
            IROp::Srl { .. } => "srl",
            IROp::Sra { .. } => "sra",
            IROp::AddImm { .. } => "addi",
            IROp::MulImm { .. } => "muli",
            IROp::Mov { .. } => "mov",
            IROp::MovImm { .. } => "movi",
            IROp::SllImm { .. } => "slli",
            IROp::SrlImm { .. } => "srli",
            IROp::SraImm { .. } => "srai",
            IROp::CmpEq { .. } => "cmpeq",
            IROp::CmpNe { .. } => "cmpne",
            IROp::CmpLt { .. } => "cmplt",
            IROp::CmpLtU { .. } => "cmpltu",
            IROp::CmpGe { .. } => "cmpge",
            IROp::CmpGeU { .. } => "cmpgeu",
            IROp::Select { .. } => "select",
            IROp::Load { .. } => "load",
            IROp::Store { .. } => "store",
            IROp::AtomicRMW { .. } => "atomic_rmw",
            IROp::AtomicRMWOrder { .. } => "atomic_rmw_order",
            IROp::AtomicCmpXchg { .. } => "atomic_cmpxchg",
            IROp::AtomicCmpXchgOrder { .. } => "atomic_cmpxchg_order",
            IROp::AtomicLoadReserve { .. } => "atomic_load_reserve",
            IROp::AtomicStoreCond { .. } => "atomic_store_cond",
            IROp::AtomicCmpXchgFlag { .. } => "atomic_cmpxchg_flag",
            IROp::AtomicRmwFlag { .. } => "atomic_rmw_flag",
            IROp::VecAdd { .. } => "vec_add",
            IROp::VecSub { .. } => "vec_sub",
            IROp::VecMul { .. } => "vec_mul",
            IROp::VecAddSat { .. } => "vec_add_sat",
            IROp::VecSubSat { .. } => "vec_sub_sat",
            IROp::VecMulSat { .. } => "vec_mul_sat",
            IROp::VecAnd { .. } => "vec_and",
            IROp::VecOr { .. } => "vec_or",
            IROp::VecXor { .. } => "vec_xor",
            IROp::VecNot { .. } => "vec_not",
            IROp::VecShl { .. } => "vec_shl",
            IROp::VecSrl { .. } => "vec_srl",
            IROp::VecSra { .. } => "vec_sra",
            IROp::VecShlImm { .. } => "vec_shl_imm",
            IROp::VecSrlImm { .. } => "vec_srl_imm",
            IROp::VecSraImm { .. } => "vec_sra_imm",
            IROp::Vec128Add { .. } => "vec128_add",
            IROp::Vec256Add { .. } => "vec256_add",
            IROp::Vec256Sub { .. } => "vec256_sub",
            IROp::Vec256Mul { .. } => "vec256_mul",
            IROp::Broadcast { .. } => "broadcast",
            IROp::Fadd { .. } => "fadd",
            IROp::Fsub { .. } => "fsub",
            IROp::Fmul { .. } => "fmul",
            IROp::Fdiv { .. } => "fdiv",
            IROp::Fsqrt { .. } => "fsqrt",
            IROp::Fmin { .. } => "fmin",
            IROp::Fmax { .. } => "fmax",
            IROp::FaddS { .. } => "fadd_s",
            IROp::FsubS { .. } => "fsub_s",
            IROp::FmulS { .. } => "fmul_s",
            IROp::FdivS { .. } => "fdiv_s",
            IROp::FsqrtS { .. } => "fsqrt_s",
            IROp::FminS { .. } => "fmin_s",
            IROp::FmaxS { .. } => "fmax_s",
            IROp::Fmadd { .. } => "fmadd",
            IROp::Fmsub { .. } => "fmsub",
            IROp::Fnmadd { .. } => "fnmadd",
            IROp::Fnmsub { .. } => "fnmsub",
            IROp::FmaddS { .. } => "fmadd_s",
            IROp::FmsubS { .. } => "fmsub_s",
            IROp::FnmaddS { .. } => "fnmadd_s",
            IROp::FnmsubS { .. } => "fnmsub_s",
            IROp::Feq { .. } => "feq",
            IROp::Flt { .. } => "flt",
            IROp::Fle { .. } => "fle",
            IROp::FeqS { .. } => "feq_s",
            IROp::FltS { .. } => "flt_s",
            IROp::FleS { .. } => "fle_s",
            IROp::Fcvtws { .. } => "fcvt_ws",
            IROp::Fcvtwus { .. } => "fcvt_wus",
            IROp::Fcvtls { .. } => "fcvt_ls",
            IROp::Fcvtlus { .. } => "fcvt_lus",
            IROp::Fcvtsw { .. } => "fcvt_sw",
            IROp::Fcvtswu { .. } => "fcvt_swu",
            IROp::Fcvtsl { .. } => "fcvt_sl",
            IROp::Fcvtslu { .. } => "fcvt_slu",
            IROp::Fcvtwd { .. } => "fcvt_wd",
            IROp::Fcvtwud { .. } => "fcvt_wud",
            IROp::Fcvtld { .. } => "fcvt_ld",
            IROp::Fcvtlud { .. } => "fcvt_lud",
            IROp::Fcvtdw { .. } => "fcvt_dw",
            IROp::Fcvtdwu { .. } => "fcvt_dwu",
            IROp::Fcvtdl { .. } => "fcvt_dl",
            IROp::Fcvtdlu { .. } => "fcvt_dlu",
            IROp::Fcvtsd { .. } => "fcvt_sd",
            IROp::Fcvtds { .. } => "fcvt_ds",
            IROp::Fsgnj { .. } => "fsgnj",
            IROp::Fsgnjn { .. } => "fsgnjn",
            IROp::Fsgnjx { .. } => "fsgnjx",
            IROp::FsgnjS { .. } => "fsgnj_s",
            IROp::FsgnjnS { .. } => "fsgnjn_s",
            IROp::FsgnjxS { .. } => "fsgnjx_s",
            IROp::Fclass { .. } => "fclass",
            IROp::FclassS { .. } => "fclass_s",
            IROp::FmvXW { .. } => "fmv_x_w",
            IROp::FmvWX { .. } => "fmv_w_x",
            IROp::FmvXD { .. } => "fmv_x_d",
            IROp::FmvDX { .. } => "fmv_d_x",
            IROp::Fabs { .. } => "fabs",
            IROp::Fneg { .. } => "fneg",
            IROp::FabsS { .. } => "fabs_s",
            IROp::FnegS { .. } => "fneg_s",
            IROp::Fload { .. } => "fload",
            IROp::Fstore { .. } => "fstore",
            IROp::Beq { .. } => "beq",
            IROp::Bne { .. } => "bne",
            IROp::Blt { .. } => "blt",
            IROp::Bge { .. } => "bge",
            IROp::Bltu { .. } => "bltu",
            IROp::Bgeu { .. } => "bgeu",
            IROp::Atomic { .. } => "atomic",
            IROp::SysCall => "syscall",
            IROp::DebugBreak => "debugbreak",
            IROp::Cpuid { .. } => "cpuid",
            IROp::TlbFlush { .. } => "tlb_flush",
            IROp::CsrRead { .. } => "csr_read",
            IROp::CsrWrite { .. } => "csr_write",
            IROp::CsrSet { .. } => "csr_set",
            IROp::CsrClear { .. } => "csr_clear",
            IROp::CsrWriteImm { .. } => "csr_write_imm",
            IROp::CsrSetImm { .. } => "csr_set_imm",
            IROp::CsrClearImm { .. } => "csr_clear_imm",
            IROp::SysMret => "mret",
            IROp::SysSret => "sret",
            IROp::SysWfi => "wfi",
            IROp::ReadPstateFlags { .. } => "read_pstate_flags",
            IROp::WritePstateFlags { .. } => "write_pstate_flags",
            IROp::EvalCondition { .. } => "eval_condition",
            IROp::VendorLoad { .. } => "vendor_load",
            IROp::VendorStore { .. } => "vendor_store",
            IROp::VendorMatrixOp { .. } => "vendor_matrix_op",
            IROp::VendorConfig { .. } => "vendor_config",
            IROp::VendorVectorOp { .. } => "vendor_vector_op",
            IROp::Branch { .. } => "branch",
            IROp::CondBranch { .. } => "cond_branch",
            IROp::BinaryOp { .. } => "binary_op",
            IROp::LoadExt { .. } => "load_ext",
            IROp::StoreExt { .. } => "store_ext",
        }
    }

    /// Format an IR operation for display
    pub fn format_op(op: &IROp) -> String {
        format_op_with_indent(op, 0)
    }

    /// Format an IR operation with indentation
    pub fn format_op_with_indent(op: &IROp, indent: usize) -> String {
        let indent_str = "  ".repeat(indent);
        let name = op_name(op);

        let operands = match op {
            IROp::Nop
            | IROp::SysCall
            | IROp::DebugBreak
            | IROp::SysMret
            | IROp::SysSret
            | IROp::SysWfi => String::new(),
            IROp::Add { dst, src1, src2 }
            | IROp::Sub { dst, src1, src2 }
            | IROp::Mul { dst, src1, src2 }
            | IROp::And { dst, src1, src2 }
            | IROp::Or { dst, src1, src2 }
            | IROp::Xor { dst, src1, src2 } => {
                format!("%{}, %{}, %{}", dst, src1, src2)
            }
            IROp::MovImm { dst, imm } => format!("%{}, {}", dst, imm),
            IROp::Mov { dst, src } => format!("%{}, %{}", dst, src),
            _ => "...".to_string(),
        };

        format!("{}{} {}", indent_str, name, operands)
    }

    /// Count operations by type
    pub fn count_op_types(ops: &[IROp]) -> std::collections::HashMap<&'static str, usize> {
        let mut counts = std::collections::HashMap::new();
        for op in ops {
            let name = op_name(op);
            *counts.entry(name).or_insert(0) += 1;
        }
        counts
    }

    /// Check if an IR operation is a branch
    pub fn is_branch(op: &IROp) -> bool {
        matches!(
            op,
            IROp::Beq { .. }
                | IROp::Bne { .. }
                | IROp::Blt { .. }
                | IROp::Bge { .. }
                | IROp::Bltu { .. }
                | IROp::Bgeu { .. }
                | IROp::Branch { .. }
                | IROp::CondBranch { .. }
        )
    }

    /// Check if an IR operation is a memory operation
    pub fn is_memory_op(op: &IROp) -> bool {
        matches!(
            op,
            IROp::Load { .. }
                | IROp::Store { .. }
                | IROp::Fload { .. }
                | IROp::Fstore { .. }
                | IROp::LoadExt { .. }
                | IROp::StoreExt { .. }
        )
    }

    /// Check if an IR operation is an atomic operation
    pub fn is_atomic_op(op: &IROp) -> bool {
        matches!(
            op,
            IROp::AtomicRMW { .. }
                | IROp::AtomicRMWOrder { .. }
                | IROp::AtomicCmpXchg { .. }
                | IROp::AtomicCmpXchgOrder { .. }
                | IROp::AtomicLoadReserve { .. }
                | IROp::AtomicStoreCond { .. }
                | IROp::AtomicCmpXchgFlag { .. }
                | IROp::AtomicRmwFlag { .. }
                | IROp::Atomic { .. }
        )
    }

    /// Check if an IR operation is a floating-point operation
    pub fn is_float_op(op: &IROp) -> bool {
        matches!(
            op,
            IROp::Fadd { .. }
                | IROp::Fsub { .. }
                | IROp::Fmul { .. }
                | IROp::Fdiv { .. }
                | IROp::Fsqrt { .. }
                | IROp::Fmin { .. }
                | IROp::Fmax { .. }
                | IROp::FaddS { .. }
                | IROp::FsubS { .. }
                | IROp::FmulS { .. }
                | IROp::FdivS { .. }
                | IROp::FsqrtS { .. }
                | IROp::FminS { .. }
                | IROp::FmaxS { .. }
        )
    }
}

/// 寄存器模式
///
/// 定义寄存器文件的运作模式，影响虚拟寄存器到物理寄存器的映射策略。
///
/// # 模式类型
///
/// - [`Standard`](Self::Standard): 标准模式，每个虚拟寄存器直接映射到一个物理寄存器
/// - [`SSA`](Self::SSA): SSA（静态单赋值）模式，每次写入创建新版本的寄存器
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum RegisterMode {
    /// 标准模式
    ///
    /// 每个虚拟寄存器直接映射到物理寄存器，多次写入会覆盖同一寄存器。
    Standard,

    /// SSA模式
    ///
    /// 每次写入创建新版本的寄存器，便于数据流分析和优化。
    SSA,
}

/// 寄存器文件
///
/// 管理虚拟寄存器到物理寄存器的映射，支持标准模式和SSA模式。
///
/// # 功能
///
/// - **寄存器映射**: 维护虚拟寄存器到物理寄存器的映射表
/// - **版本管理**: 在SSA模式下跟踪每个寄存器的版本号
/// - **临时寄存器**: 分配临时寄存器用于中间计算
///
/// # 使用场景
///
/// - 解释器: 使用标准模式，直接读写寄存器
/// - JIT编译: 使用SSA模式，便于寄存器分配和优化
/// - 调试器: 跟踪寄存器值的生命周期
///
/// # 示例
///
/// ```
/// use vm_ir::{RegisterFile, RegisterMode};
///
/// // 标准模式
/// let mut rf_std = RegisterFile::new(32, RegisterMode::Standard);
/// let r1 = rf_std.write(1); // 返回寄存器1
/// let r2 = rf_std.read(1); // 读取寄存器1
///
/// // SSA模式
/// let mut rf_ssa = RegisterFile::new(32, RegisterMode::SSA);
/// let r1_v1 = rf_ssa.write(1); // 写入寄存器1，版本1
/// let r1_v2 = rf_ssa.write(1); // 写入寄存器1，版本2（不同于v1）
/// assert_ne!(r1_v1, r1_v2);
///
/// // 分配临时寄存器
/// let temp1 = rf_ssa.alloc_temp();
/// let temp2 = rf_ssa.alloc_temp();
/// assert_ne!(temp1, temp2);
/// ```
pub struct RegisterFile {
    mode: RegisterMode,
    mapping: Vec<RegId>,
    versions: Vec<u32>,
    next_temp: RegId,
}

impl RegisterFile {
    /// 创建新的寄存器文件
    ///
    /// # 参数
    ///
    /// - `guest_regs`: Guest寄存器数量
    /// - `mode`: 寄存器模式（标准或SSA）
    ///
    /// # 返回
    ///
    /// 新创建的寄存器文件实例
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

    /// 读取虚拟寄存器的映射
    ///
    /// # 参数
    ///
    /// - `guest`: 虚拟寄存器索引
    ///
    /// # 返回
    ///
    /// 映射后的物理寄存器ID，如果索引无效则返回0
    pub fn read(&self, guest: usize) -> RegId {
        if guest < self.mapping.len() {
            self.mapping[guest]
        } else {
            0
        }
    }

    /// 写入虚拟寄存器
    ///
    /// 在标准模式下，返回映射的物理寄存器。
    /// 在SSA模式下，创建新版本的寄存器并更新映射。
    ///
    /// # 参数
    ///
    /// - `guest`: 虚拟寄存器索引
    ///
    /// # 返回
    ///
    /// 映射后的物理寄存器ID，如果索引无效则返回0
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

    /// 分配临时寄存器
    ///
    /// 临时寄存器用于中间计算，不映射到任何Guest寄存器。
    /// 每次调用返回唯一的寄存器ID。
    ///
    /// # 返回
    ///
    /// 新分配的临时寄存器ID
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

/// IR指令操作数
///
/// 定义IR指令中使用的各种操作数类型，用于跨架构翻译和灵活的IR表示。
///
/// # 操作数类型
///
/// - **寄存器**: [`Register`](Self::Register) / [`Reg`](Self::Reg) - 虚拟寄存器
/// - **立即数**: [`Immediate`](Self::Immediate) / [`Imm64`](Self::Imm64) - 常量值
/// - **内存**: [`Memory`](Self::Memory) - 内存地址（基址+偏移）
/// - **二元操作**: [`Binary`](Self::Binary) - 复杂地址计算（如 `r1 + r2 << 2`）
/// - **无操作数**: [`None`](Self::None) - 用于不需要操作数的指令
///
/// # 跨架构兼容性
///
/// 为了支持不同架构的习惯用法，某些类型有别名：
/// - `Register` 和 `Reg` 都表示寄存器操作数
/// - `Immediate` (i64) 和 `Imm64` (u64) 都表示立即数
///
/// # 示例
///
/// ```
/// use vm_ir::{BinaryOperator, Operand, RegId};
///
/// // 寄存器操作数
/// let reg = Operand::Register(5);
///
/// // 立即数操作数
/// let imm = Operand::Immediate(42);
///
/// // 内存操作数: [r1 + 8]
/// let mem = Operand::Memory {
///     base: 1,
///     offset: 8,
///     size: 8,
/// };
///
/// // 复杂地址: r1 + (r2 << 2)
/// let complex = Operand::Binary {
///     op: BinaryOperator::Add,
///     left: Box::new(Operand::Register(1)),
///     right: Box::new(Operand::Binary {
///         op: BinaryOperator::ShiftLeft,
///         left: Box::new(Operand::Register(2)),
///         right: Box::new(Operand::Immediate(2)),
///     }),
/// };
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Operand {
    /// 寄存器操作数
    ///
    /// 表示虚拟寄存器的值。
    Register(RegId),

    /// 有符号立即数
    ///
    /// 64位有符号常量值。
    Immediate(i64),

    /// 内存操作数
    ///
    /// 表示内存地址，格式为 `[base + offset]`。
    ///
    /// # 字段
    /// - `base`: 基址寄存器
    /// - `offset`: 字节偏移量
    /// - `size`: 访问大小（字节）
    Memory { base: RegId, offset: i64, size: u8 },

    /// 无操作数
    ///
    /// 用于不需要操作数的指令（如 `SysCall`）。
    None,

    /// 二元操作表达式
    ///
    /// 用于表示复杂的地址计算，如数组索引。
    ///
    /// # 字段
    /// - `op`: 二元操作符
    /// - `left`: 左操作数
    /// - `right`: 右操作数
    Binary {
        /// 二元操作符
        op: BinaryOperator,
        /// 左操作数
        left: Box<Operand>,
        /// 右操作数
        right: Box<Operand>,
    },

    /// 寄存器操作数（别名）
    ///
    /// 与 [`Register`](Self::Register) 相同，用于跨架构兼容。
    Reg(RegId),

    /// 无符号立即数（别名）
    ///
    /// 与 [`Immediate`](Self::Immediate) 相同，但类型为 u64。
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

/// 二元操作符
///
/// 定义跨架构翻译支持的二元操作符类型，用于 [`Operand::Binary`] 表达式。
///
/// # 操作符分类
///
/// ## 算术操作
/// - [`Add`](Self::Add), [`Sub`](Self::Sub), [`Mul`](Self::Mul), [`Div`](Self::Div),
///   [`Rem`](Self::Rem)
///
/// ## 位操作
/// - [`And`](Self::And), [`Or`](Self::Or), [`Xor`](Self::Xor)
///
/// ## 移位操作
/// - [`ShiftLeft`](Self::ShiftLeft): 左移
/// - [`ShiftRightLogical`](Self::ShiftRightLogical): 逻辑右移（零填充）
/// - [`ShiftRightArithmetic`](Self::ShiftRightArithmetic): 算术右移（符号填充）
///
/// ## 比较操作
/// - [`Equal`](Self::Equal), [`NotEqual`](Self::NotEqual)
/// - [`LessThan`](Self::LessThan), [`LessThanUnsigned`](Self::LessThanUnsigned)
/// - [`GreaterEqual`](Self::GreaterEqual), [`GreaterEqualUnsigned`](Self::GreaterEqualUnsigned)
///
/// ## 浮点操作
/// - [`FAdd`](Self::FAdd), [`FSub`](Self::FSub), `FMul`, `FDiv`
///
/// ## 自定义操作
/// - [`Custom`](Self::Custom): 架构特定的自定义操作
///
/// # 示例
///
/// ```
/// use vm_ir::BinaryOperator;
///
/// // 算术加法
/// let add = BinaryOperator::Add;
///
/// // 逻辑左移
/// let shl = BinaryOperator::ShiftLeft;
///
/// // 自定义操作
/// let custom = BinaryOperator::Custom(0xFF);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BinaryOperator {
    // Arithmetic
    /// 加法
    Add,
    /// 减法
    Sub,
    /// 乘法
    Mul,
    /// 除法
    Div,
    /// 取模/余数
    Rem,

    // Logical
    /// 位与
    And,
    /// 位或
    Or,
    /// 位异或
    Xor,

    // Shifts
    /// 左移
    ShiftLeft,
    /// 逻辑右移（零填充）
    ShiftRightLogical,
    /// 算术右移（符号填充）
    ShiftRightArithmetic,

    // Comparisons
    /// 相等比较
    Equal,
    /// 不等比较
    NotEqual,
    /// 小于（有符号）
    LessThan,
    /// 小于（无符号）
    LessThanUnsigned,
    /// 大于等于（有符号）
    GreaterEqual,
    /// 大于等于（无符号）
    GreaterEqualUnsigned,

    // Floating-point
    /// 浮点加法
    FAdd,
    /// 浮点减法
    FSub,
    /// 浮点乘法
    FMul,
    /// 浮点除法
    FDiv,

    // Custom
    /// 自定义操作符
    ///
    /// 用于架构特定的操作，操作码为自定义值。
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::ir_utils::*;
    use super::*;

    // IRBlock tests
    #[test]
    fn test_irblock_creation() {
        let block = IRBlock::new(GuestAddr(0x1000));
        assert_eq!(block.start_pc, GuestAddr(0x1000));
        assert_eq!(block.op_count(), 0);
        assert!(block.is_empty());
    }

    #[test]
    fn test_irblock_validation() {
        let block = IRBlock::new(GuestAddr(0x1000));
        assert!(block.validate().is_ok());
    }

    #[test]
    fn test_irblock_iterator() {
        let mut block = IRBlock::new(GuestAddr(0x1000));
        block.ops.push(IROp::Nop);
        block.ops.push(IROp::MovImm { dst: 1, imm: 42 });

        let ops: Vec<_> = block.iter_ops().collect();
        assert_eq!(ops.len(), 2);
    }

    #[test]
    fn test_irblock_estimated_size() {
        let block = IRBlock::new(GuestAddr(0x1000));
        let size = block.estimated_size();
        assert!(size > 0);
    }

    // IRBuilder tests
    #[test]
    fn test_irbuilder_basic() {
        let mut builder = IRBuilder::new(GuestAddr(0x1000));
        builder.push(IROp::MovImm { dst: 1, imm: 42 });
        builder.set_term(Terminator::Ret);

        assert_eq!(builder.pc(), GuestAddr(0x1000));
        assert_eq!(builder.op_count(), 1);
        assert!(!builder.is_empty());

        let block = builder.build();
        assert_eq!(block.ops.len(), 1);
    }

    #[test]
    fn test_irbuilder_push_all() {
        let mut builder = IRBuilder::new(GuestAddr(0x1000));
        let ops = vec![
            IROp::MovImm { dst: 1, imm: 10 },
            IROp::MovImm { dst: 2, imm: 20 },
        ];
        builder.push_all(ops);

        assert_eq!(builder.op_count(), 2);
    }

    #[test]
    fn test_irbuilder_build_ref() {
        let mut builder = IRBuilder::new(GuestAddr(0x1000));
        builder.push(IROp::Nop);
        builder.set_term(Terminator::Ret);

        let block1 = builder.build_ref();
        assert_eq!(block1.ops.len(), 1);

        // Builder should still be usable
        assert_eq!(builder.op_count(), 1);
    }

    // IR operation validation tests
    #[test]
    fn test_validate_op_valid() {
        let ops = vec![
            IROp::Add {
                dst: 1,
                src1: 2,
                src2: 3,
            },
            IROp::Load {
                dst: 1,
                base: 2,
                offset: 0,
                size: 8,
                flags: MemFlags::default(),
            },
        ];

        assert!(
            validate_op(&IROp::Add {
                dst: 1,
                src1: 2,
                src2: 3,
            })
            .is_ok()
        );
        assert!(validate_ops(&ops).is_ok());
    }

    #[test]
    fn test_validate_op_invalid_size() {
        let op = IROp::Load {
            dst: 1,
            base: 2,
            offset: 0,
            size: 0, // Invalid size
            flags: MemFlags::default(),
        };

        assert!(validate_op(&op).is_err());
    }

    #[test]
    fn test_validate_op_invalid_shift() {
        let op = IROp::SllImm {
            dst: 1,
            src: 2,
            sh: 100, // Invalid shift amount
        };

        assert!(validate_op(&op).is_err());
    }

    // IR operation classification tests
    #[test]
    fn test_is_branch() {
        assert!(is_branch(&IROp::Beq {
            src1: 1,
            src2: 2,
            target: GuestAddr(0x1000)
        }));

        assert!(is_branch(&IROp::Bne {
            src1: 1,
            src2: 2,
            target: GuestAddr(0x1000)
        }));

        assert!(!is_branch(&IROp::Add {
            dst: 1,
            src1: 2,
            src2: 3
        }));
    }

    #[test]
    fn test_is_memory_op() {
        assert!(is_memory_op(&IROp::Load {
            dst: 1,
            base: 2,
            offset: 0,
            size: 8,
            flags: MemFlags::default(),
        }));

        assert!(is_memory_op(&IROp::Store {
            src: 1,
            base: 2,
            offset: 0,
            size: 8,
            flags: MemFlags::default(),
        }));

        assert!(!is_memory_op(&IROp::Add {
            dst: 1,
            src1: 2,
            src2: 3
        }));
    }

    #[test]
    fn test_is_atomic_op() {
        assert!(is_atomic_op(&IROp::AtomicRMW {
            dst: 1,
            base: 2,
            src: 3,
            op: AtomicOp::Add,
            size: 8,
        }));

        assert!(!is_atomic_op(&IROp::Add {
            dst: 1,
            src1: 2,
            src2: 3
        }));
    }

    #[test]
    fn test_is_float_op() {
        assert!(is_float_op(&IROp::Fadd {
            dst: 1,
            src1: 2,
            src2: 3,
        }));

        assert!(is_float_op(&IROp::Fmul {
            dst: 1,
            src1: 2,
            src2: 3,
        }));

        assert!(!is_float_op(&IROp::Add {
            dst: 1,
            src1: 2,
            src2: 3
        }));
    }

    // IR operation naming tests
    #[test]
    fn test_op_name() {
        assert_eq!(
            op_name(&IROp::Add {
                dst: 1,
                src1: 2,
                src2: 3
            }),
            "add"
        );

        assert_eq!(op_name(&IROp::MovImm { dst: 1, imm: 42 }), "movi");

        assert_eq!(op_name(&IROp::Nop), "nop");
    }

    // IR operation formatting tests
    #[test]
    fn test_format_op() {
        let op = IROp::Add {
            dst: 1,
            src1: 2,
            src2: 3,
        };
        let formatted = format_op(&op);
        assert!(formatted.contains("add"));
        assert!(formatted.contains("%1"));
    }

    #[test]
    fn test_count_op_types() {
        let ops = vec![
            IROp::Add {
                dst: 1,
                src1: 2,
                src2: 3,
            },
            IROp::Add {
                dst: 4,
                src1: 5,
                src2: 6,
            },
            IROp::Sub {
                dst: 1,
                src1: 2,
                src2: 3,
            },
        ];

        let counts = count_op_types(&ops);
        assert_eq!(*counts.get("add").unwrap_or(&0), 2);
        assert_eq!(*counts.get("sub").unwrap_or(&0), 1);
    }

    // Complex IR block test
    #[test]
    fn test_complex_ir_block() {
        let mut builder = IRBuilder::new(GuestAddr(0x1000));

        // Build a simple function
        builder.push(IROp::MovImm { dst: 1, imm: 10 });
        builder.push(IROp::MovImm { dst: 2, imm: 20 });
        builder.push(IROp::Add {
            dst: 3,
            src1: 1,
            src2: 2,
        });
        builder.set_term(Terminator::Ret);

        let block = builder.build();

        assert_eq!(block.op_count(), 3);
        assert!(block.validate().is_ok());

        // Count operations
        let counts = count_op_types(&block.ops);
        assert_eq!(*counts.get("movi").unwrap_or(&0), 2);
        assert_eq!(*counts.get("add").unwrap_or(&0), 1);
    }

    // RegisterFile tests
    #[test]
    fn test_register_file_standard() {
        let mut rf = RegisterFile::new(10, RegisterMode::Standard);
        assert_eq!(rf.read(0), 0);
        assert_eq!(rf.write(1), 1);
    }

    #[test]
    fn test_register_file_ssa() {
        let mut rf = RegisterFile::new(10, RegisterMode::SSA);

        // First write to register 1 should give version 1
        let reg1 = rf.write(1);
        assert_ne!(reg1, 1);

        // Second write should give a different version
        let reg2 = rf.write(1);
        assert_ne!(reg1, reg2);
    }

    #[test]
    fn test_register_file_alloc_temp() {
        let mut rf = RegisterFile::new(10, RegisterMode::Standard);
        let temp1 = rf.alloc_temp();
        let temp2 = rf.alloc_temp();
        assert_ne!(temp1, temp2);
    }

    // Operand compatibility tests
    #[test]
    fn test_operand_as_reg() {
        let reg_op = Operand::Register(5);
        assert_eq!(reg_op.as_reg(), Some(5));

        let imm_op = Operand::Immediate(42);
        assert_eq!(imm_op.as_reg(), None);
    }

    #[test]
    fn test_operand_as_imm() {
        let imm_op = Operand::Immediate(42);
        assert_eq!(imm_op.as_imm(), Some(42));

        let reg_op = Operand::Register(5);
        assert_eq!(reg_op.as_imm(), None);
    }

    // BinaryOperator conversion tests
    #[test]
    fn test_binary_operator_from_irop() {
        let add_op = IROp::Add {
            dst: 1,
            src1: 2,
            src2: 3,
        };
        assert_eq!(
            BinaryOperator::from_irop(&add_op),
            Some(BinaryOperator::Add)
        );

        let nop_op = IROp::Nop;
        assert_eq!(BinaryOperator::from_irop(&nop_op), None);
    }

    #[test]
    fn test_binary_operator_mnemonic() {
        assert_eq!(BinaryOperator::Add.mnemonic(), "add");
        assert_eq!(BinaryOperator::Sub.mnemonic(), "sub");
        assert_eq!(BinaryOperator::FAdd.mnemonic(), "fadd");
    }

    // Memory flags tests
    #[test]
    fn test_mem_flags_default() {
        let flags = MemFlags::default();
        assert!(!flags.volatile);
        assert!(!flags.atomic);
        assert_eq!(flags.align, 0);
    }

    // AtomicOp tests
    #[test]
    fn test_atomic_op() {
        let ops = [
            AtomicOp::Add,
            AtomicOp::Sub,
            AtomicOp::And,
            AtomicOp::Or,
            AtomicOp::Xor,
            AtomicOp::Xchg,
        ];

        for op in ops {
            // Ensure atomic ops can be created and compared
            let _ = op;
        }
    }

    // Terminator tests
    #[test]
    fn test_terminator_types() {
        let terminators = vec![
            Terminator::Ret,
            Terminator::Jmp {
                target: GuestAddr(0x1000),
            },
            Terminator::JmpReg { base: 1, offset: 0 },
            Terminator::CondJmp {
                cond: 1,
                target_true: GuestAddr(0x1000),
                target_false: GuestAddr(0x2000),
            },
            Terminator::Call {
                target: GuestAddr(0x1000),
                ret_pc: GuestAddr(0x2000),
            },
        ];

        for term in terminators {
            let _ = term;
        }
    }
}
