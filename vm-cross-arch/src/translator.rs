//! 架构转换器模块
//!
//! 实现跨架构指令转换的核心逻辑

use super::{
    ArchEncoder, Architecture, Arm64Encoder, RegisterMapper, Riscv64Encoder, TargetInstruction,
    TranslationResult, TranslationStats, X86_64Encoder,
};
use thiserror::Error;
use vm_core::{GuestAddr, VmError};
use vm_ir::{IRBlock, IROp, Terminator};

/// 源架构类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceArch {
    X86_64,
    ARM64,
    RISCV64,
}

impl From<SourceArch> for Architecture {
    fn from(arch: SourceArch) -> Self {
        match arch {
            SourceArch::X86_64 => Architecture::X86_64,
            SourceArch::ARM64 => Architecture::ARM64,
            SourceArch::RISCV64 => Architecture::RISCV64,
        }
    }
}

/// 目标架构类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetArch {
    X86_64,
    ARM64,
    RISCV64,
}

impl From<TargetArch> for Architecture {
    fn from(arch: TargetArch) -> Self {
        match arch {
            TargetArch::X86_64 => Architecture::X86_64,
            TargetArch::ARM64 => Architecture::ARM64,
            TargetArch::RISCV64 => Architecture::RISCV64,
        }
    }
}

/// 转换错误类型
#[derive(Error, Debug)]
pub enum TranslationError {
    #[error("不支持的IR操作: {op}")]
    UnsupportedOperation { op: String },

    #[error("立即数过大: {imm}")]
    ImmediateTooLarge { imm: i64 },

    #[error("无效的偏移量: {offset}")]
    InvalidOffset { offset: i64 },

    #[error("寄存器映射失败: {reason}")]
    RegisterMappingFailed { reason: String },

    #[error("编码错误: {message}")]
    EncodingError { message: String },

    #[error("不支持的架构转换: {source:?} -> {target:?}")]
    UnsupportedArchitecturePair {
        source: Architecture,
        target: Architecture,
    },
}

impl From<TranslationError> for VmError {
    fn from(err: TranslationError) -> Self {
        VmError::Core(vm_core::CoreError::NotImplemented {
            feature: format!("{:?}", err),
            module: "vm-cross-arch".to_string(),
        })
    }
}

/// 架构转换器
///
/// 负责将源架构的IR块转换为目标架构的指令序列
pub struct ArchTranslator {
    source_arch: Architecture,
    target_arch: Architecture,
    encoder: Box<dyn ArchEncoder>,
    register_mapper: RegisterMapper,
}

impl ArchTranslator {
    /// 创建新的架构转换器
    pub fn new(source_arch: SourceArch, target_arch: TargetArch) -> Self {
        let source: Architecture = source_arch.into();
        let target: Architecture = target_arch.into();

        // 选择编码器
        let encoder: Box<dyn ArchEncoder> = match target {
            Architecture::X86_64 => Box::new(X86_64Encoder),
            Architecture::ARM64 => Box::new(Arm64Encoder),
            Architecture::RISCV64 => Box::new(Riscv64Encoder),
        };

        // 创建寄存器映射器
        let register_mapper = RegisterMapper::new(source, target);

        Self {
            source_arch: source,
            target_arch: target,
            encoder,
            register_mapper,
        }
    }

    /// 转换IR块为目标架构指令序列
    pub fn translate_block(
        &mut self,
        block: &IRBlock,
    ) -> Result<TranslationResult, TranslationError> {
        let mut instructions = Vec::new();
        let mut stats = TranslationStats::default();

        // 重置临时寄存器分配器
        self.register_mapper.reset_temps();

        // 转换每个IR操作
        for op in &block.ops {
            stats.ir_ops_translated += 1;

            // 映射寄存器
            let mapped_op = self.map_registers_in_op(op)?;

            // 编码为目标架构指令
            let target_insns = self.encoder.encode_op(&mapped_op, block.start_pc)?;
            stats.target_instructions_generated += target_insns.len();

            if target_insns.len() > 1 {
                stats.complex_operations += 1;
            }

            instructions.extend(target_insns);
        }

        // 处理终结符（跳转、返回等）
        let term_insns = self.translate_terminator(&block.term, block.start_pc)?;
        instructions.extend(term_insns);

        Ok(TranslationResult {
            instructions,
            stats,
        })
    }

    /// 映射IR操作中的寄存器
    fn map_registers_in_op(&mut self, op: &IROp) -> Result<IROp, TranslationError> {
        match op {
            IROp::Add { dst, src1, src2 } => Ok(IROp::Add {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::Sub { dst, src1, src2 } => Ok(IROp::Sub {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::AddImm { dst, src, imm } => Ok(IROp::AddImm {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
                imm: *imm,
            }),
            IROp::MovImm { dst, imm } => Ok(IROp::MovImm {
                dst: self.register_mapper.map_register(*dst),
                imm: *imm,
            }),
            IROp::Load {
                dst,
                base,
                offset,
                size,
                flags,
            } => Ok(IROp::Load {
                dst: self.register_mapper.map_register(*dst),
                base: self.register_mapper.map_register(*base),
                offset: *offset,
                size: *size,
                flags: *flags,
            }),
            IROp::Store {
                src,
                base,
                offset,
                size,
                flags,
            } => Ok(IROp::Store {
                src: self.register_mapper.map_register(*src),
                base: self.register_mapper.map_register(*base),
                offset: *offset,
                size: *size,
                flags: *flags,
            }),
            // SIMD操作
            IROp::VecAdd {
                dst,
                src1,
                src2,
                element_size,
            } => Ok(IROp::VecAdd {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
                element_size: *element_size,
            }),
            IROp::VecSub {
                dst,
                src1,
                src2,
                element_size,
            } => Ok(IROp::VecSub {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
                element_size: *element_size,
            }),
            IROp::VecMul {
                dst,
                src1,
                src2,
                element_size,
            } => Ok(IROp::VecMul {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
                element_size: *element_size,
            }),
            // 浮点操作
            IROp::Fadd { dst, src1, src2 } => Ok(IROp::Fadd {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::Fsub { dst, src1, src2 } => Ok(IROp::Fsub {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::Fmul { dst, src1, src2 } => Ok(IROp::Fmul {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::Fdiv { dst, src1, src2 } => Ok(IROp::Fdiv {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::FaddS { dst, src1, src2 } => Ok(IROp::FaddS {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::FsubS { dst, src1, src2 } => Ok(IROp::FsubS {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::FmulS { dst, src1, src2 } => Ok(IROp::FmulS {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::FdivS { dst, src1, src2 } => Ok(IROp::FdivS {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::Fsqrt { dst, src } => Ok(IROp::Fsqrt {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            IROp::FsqrtS { dst, src } => Ok(IROp::FsqrtS {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            // 原子操作
            IROp::AtomicRMW {
                dst,
                base,
                src,
                op,
                size,
            } => Ok(IROp::AtomicRMW {
                dst: self.register_mapper.map_register(*dst),
                base: self.register_mapper.map_register(*base),
                src: self.register_mapper.map_register(*src),
                op: *op,
                size: *size,
            }),
            IROp::AtomicCmpXchg {
                dst,
                base,
                expected,
                new,
                size,
            } => Ok(IROp::AtomicCmpXchg {
                dst: self.register_mapper.map_register(*dst),
                base: self.register_mapper.map_register(*base),
                expected: self.register_mapper.map_register(*expected),
                new: self.register_mapper.map_register(*new),
                size: *size,
            }),
            IROp::AtomicLoadReserve {
                dst,
                base,
                offset,
                size,
                flags,
            } => Ok(IROp::AtomicLoadReserve {
                dst: self.register_mapper.map_register(*dst),
                base: self.register_mapper.map_register(*base),
                offset: *offset,
                size: *size,
                flags: *flags,
            }),
            IROp::AtomicStoreCond {
                src,
                base,
                offset,
                size,
                dst_flag,
                flags,
            } => Ok(IROp::AtomicStoreCond {
                src: self.register_mapper.map_register(*src),
                base: self.register_mapper.map_register(*base),
                offset: *offset,
                size: *size,
                dst_flag: self.register_mapper.map_register(*dst_flag),
                flags: *flags,
            }),
            // SIMD饱和运算
            IROp::VecAddSat {
                dst,
                src1,
                src2,
                element_size,
                signed,
            } => Ok(IROp::VecAddSat {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
                element_size: *element_size,
                signed: *signed,
            }),
            IROp::VecSubSat {
                dst,
                src1,
                src2,
                element_size,
                signed,
            } => Ok(IROp::VecSubSat {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
                element_size: *element_size,
                signed: *signed,
            }),
            // 浮点融合乘加
            IROp::Fmadd {
                dst,
                src1,
                src2,
                src3,
            } => Ok(IROp::Fmadd {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
                src3: self.register_mapper.map_register(*src3),
            }),
            IROp::Fmsub {
                dst,
                src1,
                src2,
                src3,
            } => Ok(IROp::Fmsub {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
                src3: self.register_mapper.map_register(*src3),
            }),
            IROp::Fnmadd {
                dst,
                src1,
                src2,
                src3,
            } => Ok(IROp::Fnmadd {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
                src3: self.register_mapper.map_register(*src3),
            }),
            IROp::Fnmsub {
                dst,
                src1,
                src2,
                src3,
            } => Ok(IROp::Fnmsub {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
                src3: self.register_mapper.map_register(*src3),
            }),
            IROp::FmaddS {
                dst,
                src1,
                src2,
                src3,
            } => Ok(IROp::FmaddS {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
                src3: self.register_mapper.map_register(*src3),
            }),
            IROp::FmsubS {
                dst,
                src1,
                src2,
                src3,
            } => Ok(IROp::FmsubS {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
                src3: self.register_mapper.map_register(*src3),
            }),
            IROp::FnmaddS {
                dst,
                src1,
                src2,
                src3,
            } => Ok(IROp::FnmaddS {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
                src3: self.register_mapper.map_register(*src3),
            }),
            IROp::FnmsubS {
                dst,
                src1,
                src2,
                src3,
            } => Ok(IROp::FnmsubS {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
                src3: self.register_mapper.map_register(*src3),
            }),
            // 浮点比较
            IROp::Feq { dst, src1, src2 } => Ok(IROp::Feq {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::Flt { dst, src1, src2 } => Ok(IROp::Flt {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::Fle { dst, src1, src2 } => Ok(IROp::Fle {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::FeqS { dst, src1, src2 } => Ok(IROp::FeqS {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::FltS { dst, src1, src2 } => Ok(IROp::FltS {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::FleS { dst, src1, src2 } => Ok(IROp::FleS {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            // 浮点最小/最大
            IROp::Fmin { dst, src1, src2 } => Ok(IROp::Fmin {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::Fmax { dst, src1, src2 } => Ok(IROp::Fmax {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::FminS { dst, src1, src2 } => Ok(IROp::FminS {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::FmaxS { dst, src1, src2 } => Ok(IROp::FmaxS {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            // 浮点绝对值/取反
            IROp::Fabs { dst, src } => Ok(IROp::Fabs {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            IROp::Fneg { dst, src } => Ok(IROp::Fneg {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            IROp::FabsS { dst, src } => Ok(IROp::FabsS {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            IROp::FnegS { dst, src } => Ok(IROp::FnegS {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            // 浮点转换
            IROp::Fcvtws { dst, src } => Ok(IROp::Fcvtws {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            IROp::Fcvtsw { dst, src } => Ok(IROp::Fcvtsw {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            IROp::Fcvtld { dst, src } => Ok(IROp::Fcvtld {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            IROp::Fcvtdl { dst, src } => Ok(IROp::Fcvtdl {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            IROp::Fcvtsd { dst, src } => Ok(IROp::Fcvtsd {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            IROp::Fcvtds { dst, src } => Ok(IROp::Fcvtds {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            // 浮点加载/存储
            IROp::Fload {
                dst,
                base,
                offset,
                size,
                ..
            } => Ok(IROp::Fload {
                dst: self.register_mapper.map_register(*dst),
                base: self.register_mapper.map_register(*base),
                offset: *offset,
                size: *size,
            }),
            IROp::Fstore {
                src,
                base,
                offset,
                size,
                ..
            } => Ok(IROp::Fstore {
                src: self.register_mapper.map_register(*src),
                base: self.register_mapper.map_register(*base),
                offset: *offset,
                size: *size,
            }),
            // SIMD饱和乘法
            IROp::VecMulSat {
                dst,
                src1,
                src2,
                element_size,
                signed,
            } => Ok(IROp::VecMulSat {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
                element_size: *element_size,
                signed: *signed,
            }),
            // 浮点符号操作
            IROp::Fsgnj { dst, src1, src2 } => Ok(IROp::Fsgnj {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::Fsgnjn { dst, src1, src2 } => Ok(IROp::Fsgnjn {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::Fsgnjx { dst, src1, src2 } => Ok(IROp::Fsgnjx {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::FsgnjS { dst, src1, src2 } => Ok(IROp::FsgnjS {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::FsgnjnS { dst, src1, src2 } => Ok(IROp::FsgnjnS {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::FsgnjxS { dst, src1, src2 } => Ok(IROp::FsgnjxS {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            // 浮点分类
            IROp::Fclass { dst, src } => Ok(IROp::Fclass {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            IROp::FclassS { dst, src } => Ok(IROp::FclassS {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            // 浮点寄存器移动
            IROp::FmvXW { dst, src } => Ok(IROp::FmvXW {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            IROp::FmvWX { dst, src } => Ok(IROp::FmvWX {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            IROp::FmvXD { dst, src } => Ok(IROp::FmvXD {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            IROp::FmvDX { dst, src } => Ok(IROp::FmvDX {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            // 更多浮点转换
            IROp::Fcvtwus { dst, src } => Ok(IROp::Fcvtwus {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            IROp::Fcvtlus { dst, src } => Ok(IROp::Fcvtlus {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            IROp::Fcvtswu { dst, src } => Ok(IROp::Fcvtswu {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            IROp::Fcvtslu { dst, src } => Ok(IROp::Fcvtslu {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            IROp::Fcvtwd { dst, src } => Ok(IROp::Fcvtwd {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            IROp::Fcvtwud { dst, src } => Ok(IROp::Fcvtwud {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            IROp::Fcvtlud { dst, src } => Ok(IROp::Fcvtlud {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            IROp::Fcvtdw { dst, src } => Ok(IROp::Fcvtdw {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            IROp::Fcvtdwu { dst, src } => Ok(IROp::Fcvtdwu {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            IROp::Fcvtdl { dst, src } => Ok(IROp::Fcvtdl {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            IROp::Fcvtdlu { dst, src } => Ok(IROp::Fcvtdlu {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            // 大向量操作
            IROp::Vec128Add {
                dst_lo,
                dst_hi,
                src1_lo,
                src1_hi,
                src2_lo,
                src2_hi,
                element_size,
                signed,
            } => Ok(IROp::Vec128Add {
                dst_lo: self.register_mapper.map_register(*dst_lo),
                dst_hi: self.register_mapper.map_register(*dst_hi),
                src1_lo: self.register_mapper.map_register(*src1_lo),
                src1_hi: self.register_mapper.map_register(*src1_hi),
                src2_lo: self.register_mapper.map_register(*src2_lo),
                src2_hi: self.register_mapper.map_register(*src2_hi),
                element_size: *element_size,
                signed: *signed,
            }),
            IROp::Vec256Add {
                dst0,
                dst1,
                dst2,
                dst3,
                src10,
                src11,
                src12,
                src13,
                src20,
                src21,
                src22,
                src23,
                element_size,
                signed,
            } => Ok(IROp::Vec256Add {
                dst0: self.register_mapper.map_register(*dst0),
                dst1: self.register_mapper.map_register(*dst1),
                dst2: self.register_mapper.map_register(*dst2),
                dst3: self.register_mapper.map_register(*dst3),
                src10: self.register_mapper.map_register(*src10),
                src11: self.register_mapper.map_register(*src11),
                src12: self.register_mapper.map_register(*src12),
                src13: self.register_mapper.map_register(*src13),
                src20: self.register_mapper.map_register(*src20),
                src21: self.register_mapper.map_register(*src21),
                src22: self.register_mapper.map_register(*src22),
                src23: self.register_mapper.map_register(*src23),
                element_size: *element_size,
                signed: *signed,
            }),
            IROp::Vec256Sub {
                dst0,
                dst1,
                dst2,
                dst3,
                src10,
                src11,
                src12,
                src13,
                src20,
                src21,
                src22,
                src23,
                element_size,
                signed,
            } => Ok(IROp::Vec256Sub {
                dst0: self.register_mapper.map_register(*dst0),
                dst1: self.register_mapper.map_register(*dst1),
                dst2: self.register_mapper.map_register(*dst2),
                dst3: self.register_mapper.map_register(*dst3),
                src10: self.register_mapper.map_register(*src10),
                src11: self.register_mapper.map_register(*src11),
                src12: self.register_mapper.map_register(*src12),
                src13: self.register_mapper.map_register(*src13),
                src20: self.register_mapper.map_register(*src20),
                src21: self.register_mapper.map_register(*src21),
                src22: self.register_mapper.map_register(*src22),
                src23: self.register_mapper.map_register(*src23),
                element_size: *element_size,
                signed: *signed,
            }),
            IROp::Vec256Mul {
                dst0,
                dst1,
                dst2,
                dst3,
                src10,
                src11,
                src12,
                src13,
                src20,
                src21,
                src22,
                src23,
                element_size,
                signed,
            } => Ok(IROp::Vec256Mul {
                dst0: self.register_mapper.map_register(*dst0),
                dst1: self.register_mapper.map_register(*dst1),
                dst2: self.register_mapper.map_register(*dst2),
                dst3: self.register_mapper.map_register(*dst3),
                src10: self.register_mapper.map_register(*src10),
                src11: self.register_mapper.map_register(*src11),
                src12: self.register_mapper.map_register(*src12),
                src13: self.register_mapper.map_register(*src13),
                src20: self.register_mapper.map_register(*src20),
                src21: self.register_mapper.map_register(*src21),
                src22: self.register_mapper.map_register(*src22),
                src23: self.register_mapper.map_register(*src23),
                element_size: *element_size,
                signed: *signed,
            }),
            _ => {
                // 对于未实现的操作，尝试通用映射
                Err(TranslationError::UnsupportedOperation {
                    op: format!("{:?}", op),
                })
            }
        }
    }

    /// 转换终结符
    fn translate_terminator(
        &self,
        term: &Terminator,
        pc: GuestAddr,
    ) -> Result<Vec<TargetInstruction>, TranslationError> {
        match term {
            Terminator::Ret => {
                // 返回指令
                match self.target_arch {
                    Architecture::X86_64 => {
                        // x86-64: RET
                        Ok(vec![TargetInstruction {
                            bytes: vec![0xC3],
                            length: 1,
                            mnemonic: "ret".to_string(),
                            is_control_flow: true,
                            is_memory_op: false,
                        }])
                    }
                    Architecture::ARM64 => {
                        // ARM64: RET X30 (LR)
                        let word: u32 = 0xD65F03C0; // RET
                        Ok(vec![TargetInstruction {
                            bytes: word.to_le_bytes().to_vec(),
                            length: 4,
                            mnemonic: "ret".to_string(),
                            is_control_flow: true,
                            is_memory_op: false,
                        }])
                    }
                    Architecture::RISCV64 => {
                        // RISC-V: RET (JALR x0, x1, 0)
                        let word: u32 = 0x00008067;
                        Ok(vec![TargetInstruction {
                            bytes: word.to_le_bytes().to_vec(),
                            length: 4,
                            mnemonic: "ret".to_string(),
                            is_control_flow: true,
                            is_memory_op: false,
                        }])
                    }
                }
            }
            Terminator::Jmp { target } => {
                // 无条件跳转
                self.encode_jump(*target, pc)
            }
            Terminator::CondJmp {
                cond,
                target_true,
                target_false,
            } => {
                // 条件跳转（需要多指令序列）
                self.encode_conditional_jump(*cond, *target_true, *target_false, pc)
            }
            _ => Err(TranslationError::UnsupportedOperation {
                op: format!("{:?}", term),
            }),
        }
    }

    /// 编码无条件跳转
    fn encode_jump(
        &self,
        target: GuestAddr,
        pc: GuestAddr,
    ) -> Result<Vec<TargetInstruction>, TranslationError> {
        let offset = target.wrapping_sub(pc.wrapping_add(4)) as i64;

        match self.target_arch {
            Architecture::X86_64 => {
                // x86-64: JMP rel32
                let mut bytes = vec![0xE9];
                bytes.extend_from_slice(&(offset as i32).to_le_bytes());
                Ok(vec![TargetInstruction {
                    bytes,
                    length: 5,
                    mnemonic: format!("jmp 0x{:x}", target),
                    is_control_flow: true,
                    is_memory_op: false,
                }])
            }
            Architecture::ARM64 => {
                // ARM64: B label
                let imm26 = (offset >> 2) & 0x3FFFFFF;
                if imm26 < -0x2000000 || imm26 >= 0x2000000 {
                    return Err(TranslationError::InvalidOffset { offset });
                }
                let word: u32 = 0x14000000 | ((imm26 as u32) & 0x3FFFFFF);
                Ok(vec![TargetInstruction {
                    bytes: word.to_le_bytes().to_vec(),
                    length: 4,
                    mnemonic: format!("b 0x{:x}", target),
                    is_control_flow: true,
                    is_memory_op: false,
                }])
            }
            Architecture::RISCV64 => {
                // RISC-V: JAL x0, offset
                let imm20 = (offset >> 1) & 0xFFFFF;
                if imm20 < -0x100000 || imm20 >= 0x100000 {
                    return Err(TranslationError::InvalidOffset { offset });
                }
                let word: u32 = 0x0000006F | (((imm20 as u32) & 0xFFFFF) << 12);
                Ok(vec![TargetInstruction {
                    bytes: word.to_le_bytes().to_vec(),
                    length: 4,
                    mnemonic: format!("jal x0, 0x{:x}", target),
                    is_control_flow: true,
                    is_memory_op: false,
                }])
            }
        }
    }

    /// 编码条件跳转
    fn encode_conditional_jump(
        &self,
        cond: vm_ir::RegId,
        target_true: GuestAddr,
        target_false: GuestAddr,
        pc: GuestAddr,
    ) -> Result<Vec<TargetInstruction>, TranslationError> {
        // 简化实现：假设cond寄存器包含比较结果（0或非0）
        // 实际实现需要根据源架构的条件码格式进行转换

        let offset_true = target_true.wrapping_sub(pc.wrapping_add(4)) as i64;
        let offset_false = target_false.wrapping_sub(pc.wrapping_add(4)) as i64;

        match self.target_arch {
            Architecture::X86_64 => {
                // x86-64: TEST cond, cond; JNZ target_true; JMP target_false
                let mut insns = Vec::new();

                // TEST cond, cond
                insns.push(TargetInstruction {
                    bytes: vec![0x48, 0x85, (0xC0 | ((cond & 7) << 3) | (cond & 7)) as u8],
                    length: 3,
                    mnemonic: format!("test r{}, r{}", cond, cond),
                    is_control_flow: false,
                    is_memory_op: false,
                });

                // JNZ target_true
                insns.push(TargetInstruction {
                    bytes: {
                        let mut b = vec![0x0F, 0x85];
                        b.extend_from_slice(&(offset_true as i32).to_le_bytes());
                        b
                    },
                    length: 6,
                    mnemonic: format!("jnz 0x{:x}", target_true),
                    is_control_flow: true,
                    is_memory_op: false,
                });

                // JMP target_false
                insns.push(TargetInstruction {
                    bytes: {
                        let mut b = vec![0xE9];
                        b.extend_from_slice(&(offset_false as i32).to_le_bytes());
                        b
                    },
                    length: 5,
                    mnemonic: format!("jmp 0x{:x}", target_false),
                    is_control_flow: true,
                    is_memory_op: false,
                });

                Ok(insns)
            }
            Architecture::ARM64 => {
                // ARM64: CMP cond, #0; B.NE target_true; B target_false
                let mut insns = Vec::new();

                // CMP cond, #0
                let word1: u32 =
                    0xF1000000 | (((cond & 0x1F) as u32) << 5) | ((cond & 0x1F) as u32);
                insns.push(TargetInstruction {
                    bytes: word1.to_le_bytes().to_vec(),
                    length: 4,
                    mnemonic: format!("cmp x{}, #0", cond),
                    is_control_flow: false,
                    is_memory_op: false,
                });

                // B.NE target_true
                let imm26 = (offset_true >> 2) & 0x3FFFFFF;
                let word2: u32 = 0x54000000 | 0x1 | ((imm26 as u32) & 0x3FFFFFF);
                insns.push(TargetInstruction {
                    bytes: word2.to_le_bytes().to_vec(),
                    length: 4,
                    mnemonic: format!("b.ne 0x{:x}", target_true),
                    is_control_flow: true,
                    is_memory_op: false,
                });

                // B target_false
                let imm26_false = (offset_false >> 2) & 0x3FFFFFF;
                let word3: u32 = 0x14000000 | ((imm26_false as u32) & 0x3FFFFFF);
                insns.push(TargetInstruction {
                    bytes: word3.to_le_bytes().to_vec(),
                    length: 4,
                    mnemonic: format!("b 0x{:x}", target_false),
                    is_control_flow: true,
                    is_memory_op: false,
                });

                Ok(insns)
            }
            Architecture::RISCV64 => {
                // RISC-V: BEQ cond, x0, target_false; JAL x0, target_true
                let mut insns = Vec::new();

                // BEQ cond, x0, target_false
                let imm12 = (offset_false >> 1) & 0xFFF;
                let word1: u32 = (0x00000063
                    | ((imm12 & 0x1E) << 7)
                    | ((imm12 & 0x800) >> 4)
                    | ((imm12 & 0x7E0) << 20)
                    | ((imm12 & 0x800) << 19)) as u32;
                insns.push(TargetInstruction {
                    bytes: word1.to_le_bytes().to_vec(),
                    length: 4,
                    mnemonic: format!("beq x{}, x0, 0x{:x}", cond, target_false),
                    is_control_flow: true,
                    is_memory_op: false,
                });

                // JAL x0, target_true
                let imm20 = (offset_true >> 1) & 0xFFFFF;
                let word2: u32 = 0x0000006F | (((imm20 as u32) & 0xFFFFF) << 12);
                insns.push(TargetInstruction {
                    bytes: word2.to_le_bytes().to_vec(),
                    length: 4,
                    mnemonic: format!("jal x0, 0x{:x}", target_true),
                    is_control_flow: true,
                    is_memory_op: false,
                });

                Ok(insns)
            }
        }
    }

    /// 获取源架构
    pub fn source_arch(&self) -> Architecture {
        self.source_arch
    }

    /// 获取目标架构
    pub fn target_arch(&self) -> Architecture {
        self.target_arch
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_ir::{IRBuilder, IROp};

    #[test]
    fn test_translator_creation() {
        let translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
        assert_eq!(translator.source_arch(), Architecture::X86_64);
        assert_eq!(translator.target_arch(), Architecture::ARM64);
    }

    #[test]
    fn test_simple_translation() {
        let mut translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
        let mut builder = IRBuilder::new(0x1000);
        builder.push(IROp::Add {
            dst: 0,
            src1: 1,
            src2: 2,
        });
        builder.set_term(Terminator::Ret);
        let block = builder.build();

        let result = translator.translate_block(&block);
        assert!(result.is_ok());
    }
}
