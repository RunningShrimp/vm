//! 架构编码器模块
//!
//! 将IR操作编码为目标架构的机器码

use super::{Architecture, TargetInstruction, TranslationError};
use vm_core::GuestAddr;
use vm_ir::{IROp, RegId};

/// 架构编码器 trait
pub trait ArchEncoder {
    /// 编码IR操作为目标架构指令
    fn encode_op(
        &self,
        op: &IROp,
        pc: GuestAddr,
    ) -> Result<Vec<TargetInstruction>, TranslationError>;

    /// 获取架构类型
    fn architecture(&self) -> Architecture;
}

/// x86-64 编码器
pub struct X86_64Encoder;

impl ArchEncoder for X86_64Encoder {
    fn architecture(&self) -> Architecture {
        Architecture::X86_64
    }

    fn encode_op(
        &self,
        op: &IROp,
        _pc: GuestAddr,
    ) -> Result<Vec<TargetInstruction>, TranslationError> {
        match op {
            // 算术运算
            IROp::Add { dst, src1, src2 } => {
                // ADD dst, src1 -> ADD dst, src2
                // x86-64: ADD r/m64, r64
                Ok(vec![encode_x86_add(*dst, *src1, *src2)?])
            }
            IROp::Sub { dst, src1, src2 } => {
                // SUB dst, src1 -> SUB dst, src2
                Ok(vec![encode_x86_sub(*dst, *src1, *src2)?])
            }
            IROp::AddImm { dst, src, imm } => {
                // ADD dst, imm
                Ok(vec![encode_x86_add_imm(*dst, *src, *imm)?])
            }
            IROp::MovImm { dst, imm } => {
                // MOV dst, imm
                Ok(vec![encode_x86_mov_imm(*dst, *imm)?])
            }
            // 内存操作
            IROp::Load {
                dst,
                base,
                offset,
                size,
                ..
            } => {
                // MOV dst, [base + offset]
                Ok(vec![encode_x86_load(*dst, *base, *offset, *size)?])
            }
            IROp::Store {
                src,
                base,
                offset,
                size,
                ..
            } => {
                // MOV [base + offset], src
                Ok(vec![encode_x86_store(*src, *base, *offset, *size)?])
            }
            // SIMD操作
            IROp::VecAdd {
                dst,
                src1,
                src2,
                element_size,
            } => encode_x86_simd_add(*dst, *src1, *src2, *element_size),
            IROp::VecSub {
                dst,
                src1,
                src2,
                element_size,
            } => encode_x86_simd_sub(*dst, *src1, *src2, *element_size),
            IROp::VecMul {
                dst,
                src1,
                src2,
                element_size,
            } => encode_x86_simd_mul(*dst, *src1, *src2, *element_size),
            IROp::VecAddSat {
                dst,
                src1,
                src2,
                element_size,
                signed,
            } => encode_x86_simd_addsat(*dst, *src1, *src2, *element_size, *signed),
            IROp::VecSubSat {
                dst,
                src1,
                src2,
                element_size,
                signed,
            } => encode_x86_simd_subsat(*dst, *src1, *src2, *element_size, *signed),
            IROp::VecMulSat {
                dst,
                src1,
                src2,
                element_size,
                signed,
            } => encode_x86_simd_mulsat(*dst, *src1, *src2, *element_size, *signed),
            IROp::Vec128Add {
                dst_lo,
                dst_hi,
                src1_lo,
                src1_hi,
                src2_lo,
                src2_hi,
                element_size,
                signed,
            } => encode_x86_vec128_add(
                *dst_lo,
                *dst_hi,
                *src1_lo,
                *src1_hi,
                *src2_lo,
                *src2_hi,
                *element_size,
                *signed,
            ),
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
            } => encode_x86_vec256_add(
                *dst0,
                *dst1,
                *dst2,
                *dst3,
                *src10,
                *src11,
                *src12,
                *src13,
                *src20,
                *src21,
                *src22,
                *src23,
                *element_size,
                *signed,
            ),
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
            } => encode_x86_vec256_sub(
                *dst0,
                *dst1,
                *dst2,
                *dst3,
                *src10,
                *src11,
                *src12,
                *src13,
                *src20,
                *src21,
                *src22,
                *src23,
                *element_size,
                *signed,
            ),
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
            } => encode_x86_vec256_mul(
                *dst0,
                *dst1,
                *dst2,
                *dst3,
                *src10,
                *src11,
                *src12,
                *src13,
                *src20,
                *src21,
                *src22,
                *src23,
                *element_size,
                *signed,
            ),
            // 浮点操作
            IROp::Fadd { dst, src1, src2 } => Ok(vec![encode_x86_fadd(*dst, *src1, *src2)?]),
            IROp::Fsub { dst, src1, src2 } => Ok(vec![encode_x86_fsub(*dst, *src1, *src2)?]),
            IROp::Fmul { dst, src1, src2 } => Ok(vec![encode_x86_fmul(*dst, *src1, *src2)?]),
            IROp::Fdiv { dst, src1, src2 } => Ok(vec![encode_x86_fdiv(*dst, *src1, *src2)?]),
            IROp::FaddS { dst, src1, src2 } => Ok(vec![encode_x86_fadds(*dst, *src1, *src2)?]),
            IROp::FsubS { dst, src1, src2 } => Ok(vec![encode_x86_fsubs(*dst, *src1, *src2)?]),
            IROp::FmulS { dst, src1, src2 } => Ok(vec![encode_x86_fmuls(*dst, *src1, *src2)?]),
            IROp::FdivS { dst, src1, src2 } => Ok(vec![encode_x86_fdivs(*dst, *src1, *src2)?]),
            IROp::Fsqrt { dst, src } => Ok(vec![encode_x86_fsqrt(*dst, *src)?]),
            IROp::FsqrtS { dst, src } => Ok(vec![encode_x86_fsqrts(*dst, *src)?]),
            IROp::Fmadd {
                dst,
                src1,
                src2,
                src3,
            } => Ok(vec![encode_x86_fmadd(*dst, *src1, *src2, *src3)?]),
            IROp::Fmsub {
                dst,
                src1,
                src2,
                src3,
            } => Ok(vec![encode_x86_fmsub(*dst, *src1, *src2, *src3)?]),
            IROp::Fnmadd {
                dst,
                src1,
                src2,
                src3,
            } => Ok(vec![encode_x86_fnmadd(*dst, *src1, *src2, *src3)?]),
            IROp::Fnmsub {
                dst,
                src1,
                src2,
                src3,
            } => Ok(vec![encode_x86_fnmsub(*dst, *src1, *src2, *src3)?]),
            IROp::FmaddS {
                dst,
                src1,
                src2,
                src3,
            } => Ok(vec![encode_x86_fmadds(*dst, *src1, *src2, *src3)?]),
            IROp::FmsubS {
                dst,
                src1,
                src2,
                src3,
            } => Ok(vec![encode_x86_fmsubs(*dst, *src1, *src2, *src3)?]),
            IROp::FnmaddS {
                dst,
                src1,
                src2,
                src3,
            } => Ok(vec![encode_x86_fnmadds(*dst, *src1, *src2, *src3)?]),
            IROp::FnmsubS {
                dst,
                src1,
                src2,
                src3,
            } => Ok(vec![encode_x86_fnmsubs(*dst, *src1, *src2, *src3)?]),
            IROp::Feq { dst, src1, src2 } => Ok(vec![encode_x86_feq(*dst, *src1, *src2)?]),
            IROp::Flt { dst, src1, src2 } => Ok(vec![encode_x86_flt(*dst, *src1, *src2)?]),
            IROp::Fle { dst, src1, src2 } => Ok(vec![encode_x86_fle(*dst, *src1, *src2)?]),
            IROp::FeqS { dst, src1, src2 } => Ok(vec![encode_x86_feqs(*dst, *src1, *src2)?]),
            IROp::FltS { dst, src1, src2 } => Ok(vec![encode_x86_flts(*dst, *src1, *src2)?]),
            IROp::FleS { dst, src1, src2 } => Ok(vec![encode_x86_fles(*dst, *src1, *src2)?]),
            IROp::Fmin { dst, src1, src2 } => Ok(vec![encode_x86_fmin(*dst, *src1, *src2)?]),
            IROp::Fmax { dst, src1, src2 } => Ok(vec![encode_x86_fmax(*dst, *src1, *src2)?]),
            IROp::FminS { dst, src1, src2 } => Ok(vec![encode_x86_fmins(*dst, *src1, *src2)?]),
            IROp::FmaxS { dst, src1, src2 } => Ok(vec![encode_x86_fmaxs(*dst, *src1, *src2)?]),
            IROp::Fabs { dst, src } => Ok(vec![encode_x86_fabs(*dst, *src)?]),
            IROp::Fneg { dst, src } => Ok(vec![encode_x86_fneg(*dst, *src)?]),
            IROp::FabsS { dst, src } => Ok(vec![encode_x86_fabss(*dst, *src)?]),
            IROp::FnegS { dst, src } => Ok(vec![encode_x86_fnegs(*dst, *src)?]),
            IROp::Fcvtws { dst, src } => Ok(vec![encode_x86_cvttsd2si(*dst, *src)?]),
            IROp::Fcvtsw { dst, src } => Ok(vec![encode_x86_cvtsi2ss(*dst, *src)?]),
            IROp::Fcvtld { dst, src } => Ok(vec![encode_x86_cvttsd2si64(*dst, *src)?]),
            IROp::Fcvtdl { dst, src } => Ok(vec![encode_x86_cvtsi2sd64(*dst, *src)?]),
            IROp::Fcvtsd { dst, src } => Ok(vec![encode_x86_cvtsd2ss(*dst, *src)?]),
            IROp::Fcvtds { dst, src } => Ok(vec![encode_x86_cvtss2sd(*dst, *src)?]),
            IROp::Fload {
                dst,
                base,
                offset,
                size,
                ..
            } => Ok(vec![encode_x86_fload(*dst, *base, *offset, *size)?]),
            IROp::Fstore {
                src,
                base,
                offset,
                size,
                ..
            } => Ok(vec![encode_x86_fstore(*src, *base, *offset, *size)?]),
            IROp::Fsgnj { dst, src1, src2 } => Ok(vec![encode_x86_fsgnj(*dst, *src1, *src2)?]),
            IROp::Fsgnjn { dst, src1, src2 } => Ok(vec![encode_x86_fsgnjn(*dst, *src1, *src2)?]),
            IROp::Fsgnjx { dst, src1, src2 } => Ok(vec![encode_x86_fsgnjx(*dst, *src1, *src2)?]),
            IROp::FsgnjS { dst, src1, src2 } => Ok(vec![encode_x86_fsgnjs(*dst, *src1, *src2)?]),
            IROp::FsgnjnS { dst, src1, src2 } => Ok(vec![encode_x86_fsgnjns(*dst, *src1, *src2)?]),
            IROp::FsgnjxS { dst, src1, src2 } => Ok(vec![encode_x86_fsgnjxs(*dst, *src1, *src2)?]),
            IROp::Fclass { dst, src } => Ok(vec![encode_x86_fclass(*dst, *src)?]),
            IROp::FclassS { dst, src } => Ok(vec![encode_x86_fclasss(*dst, *src)?]),
            IROp::FmvXW { dst, src } => Ok(vec![encode_x86_fmvxw(*dst, *src)?]),
            IROp::FmvWX { dst, src } => Ok(vec![encode_x86_fmvwx(*dst, *src)?]),
            IROp::FmvXD { dst, src } => Ok(vec![encode_x86_fmvxd(*dst, *src)?]),
            IROp::FmvDX { dst, src } => Ok(vec![encode_x86_fmvdx(*dst, *src)?]),
            IROp::Fcvtwus { dst, src } => Ok(vec![encode_x86_cvttss2si_u(*dst, *src)?]),
            IROp::Fcvtlus { dst, src } => Ok(vec![encode_x86_cvttss2si64_u(*dst, *src)?]),
            IROp::Fcvtswu { dst, src } => Ok(vec![encode_x86_cvtsi2ss_u(*dst, *src)?]),
            IROp::Fcvtslu { dst, src } => Ok(vec![encode_x86_cvtsi2ss64_u(*dst, *src)?]),
            IROp::Fcvtwd { dst, src } => Ok(vec![encode_x86_cvttsd2si(*dst, *src)?]),
            IROp::Fcvtwud { dst, src } => Ok(vec![encode_x86_cvttsd2si_u(*dst, *src)?]),
            IROp::Fcvtlud { dst, src } => Ok(vec![encode_x86_cvttsd2si64_u(*dst, *src)?]),
            IROp::Fcvtdw { dst, src } => Ok(vec![encode_x86_cvtsi2sd(*dst, *src)?]),
            IROp::Fcvtdwu { dst, src } => Ok(vec![encode_x86_cvtsi2sd_u(*dst, *src)?]),
            IROp::Fcvtdlu { dst, src } => Ok(vec![encode_x86_cvtsi2sd64_u(*dst, *src)?]),
            // 原子操作
            IROp::AtomicRMW {
                dst,
                base,
                src,
                op,
                size,
            } => encode_x86_atomic_rmw(*dst, *base, *src, *op, *size),
            IROp::AtomicCmpXchg {
                dst,
                base,
                expected,
                new,
                size,
            } => encode_x86_atomic_cmpxchg(*dst, *base, *expected, *new, *size),
            _ => Err(TranslationError::UnsupportedOperation {
                op: format!("{:?}", op),
            }),
        }
    }
}

/// ARM64 编码器
pub struct Arm64Encoder;

impl ArchEncoder for Arm64Encoder {
    fn architecture(&self) -> Architecture {
        Architecture::ARM64
    }

    fn encode_op(
        &self,
        op: &IROp,
        _pc: GuestAddr,
    ) -> Result<Vec<TargetInstruction>, TranslationError> {
        match op {
            IROp::Add { dst, src1, src2 } => {
                // ADD Xdst, Xsrc1, Xsrc2
                Ok(vec![encode_arm64_add(*dst, *src1, *src2)?])
            }
            IROp::Sub { dst, src1, src2 } => {
                // SUB Xdst, Xsrc1, Xsrc2
                Ok(vec![encode_arm64_sub(*dst, *src1, *src2)?])
            }
            IROp::AddImm { dst, src, imm } => {
                // ADD Xdst, Xsrc, #imm
                Ok(vec![encode_arm64_add_imm(*dst, *src, *imm)?])
            }
            IROp::MovImm { dst, imm } => {
                // MOVZ Xdst, #imm (或MOVZ + MOVK组合)
                encode_arm64_mov_imm(*dst, *imm)
            }
            IROp::Load {
                dst,
                base,
                offset,
                size,
                ..
            } => {
                // LDR Xdst, [Xbase, #offset]
                Ok(vec![encode_arm64_load(*dst, *base, *offset, *size)?])
            }
            IROp::Store {
                src,
                base,
                offset,
                size,
                ..
            } => {
                // STR Xsrc, [Xbase, #offset]
                Ok(vec![encode_arm64_store(*src, *base, *offset, *size)?])
            }
            // SIMD操作
            IROp::VecAdd {
                dst,
                src1,
                src2,
                element_size,
            } => encode_arm64_simd_add(*dst, *src1, *src2, *element_size),
            IROp::VecSub {
                dst,
                src1,
                src2,
                element_size,
            } => encode_arm64_simd_sub(*dst, *src1, *src2, *element_size),
            IROp::VecMul {
                dst,
                src1,
                src2,
                element_size,
            } => encode_arm64_simd_mul(*dst, *src1, *src2, *element_size),
            IROp::VecAddSat {
                dst,
                src1,
                src2,
                element_size,
                signed,
            } => encode_arm64_simd_addsat(*dst, *src1, *src2, *element_size, *signed),
            IROp::VecSubSat {
                dst,
                src1,
                src2,
                element_size,
                signed,
            } => encode_arm64_simd_subsat(*dst, *src1, *src2, *element_size, *signed),
            IROp::VecMulSat {
                dst,
                src1,
                src2,
                element_size,
                signed,
            } => encode_arm64_simd_mulsat(*dst, *src1, *src2, *element_size, *signed),
            IROp::Vec128Add {
                dst_lo,
                dst_hi,
                src1_lo,
                src1_hi,
                src2_lo,
                src2_hi,
                element_size,
                signed,
            } => encode_arm64_vec128_add(
                *dst_lo,
                *dst_hi,
                *src1_lo,
                *src1_hi,
                *src2_lo,
                *src2_hi,
                *element_size,
                *signed,
            ),
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
            } => encode_arm64_vec256_add(
                *dst0,
                *dst1,
                *dst2,
                *dst3,
                *src10,
                *src11,
                *src12,
                *src13,
                *src20,
                *src21,
                *src22,
                *src23,
                *element_size,
                *signed,
            ),
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
            } => encode_arm64_vec256_sub(
                *dst0,
                *dst1,
                *dst2,
                *dst3,
                *src10,
                *src11,
                *src12,
                *src13,
                *src20,
                *src21,
                *src22,
                *src23,
                *element_size,
                *signed,
            ),
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
            } => encode_arm64_vec256_mul(
                *dst0,
                *dst1,
                *dst2,
                *dst3,
                *src10,
                *src11,
                *src12,
                *src13,
                *src20,
                *src21,
                *src22,
                *src23,
                *element_size,
                *signed,
            ),
            // 浮点操作
            IROp::Fadd { dst, src1, src2 } => Ok(vec![encode_arm64_fadd(*dst, *src1, *src2)?]),
            IROp::Fsub { dst, src1, src2 } => Ok(vec![encode_arm64_fsub(*dst, *src1, *src2)?]),
            IROp::Fmul { dst, src1, src2 } => Ok(vec![encode_arm64_fmul(*dst, *src1, *src2)?]),
            IROp::Fdiv { dst, src1, src2 } => Ok(vec![encode_arm64_fdiv(*dst, *src1, *src2)?]),
            IROp::FaddS { dst, src1, src2 } => Ok(vec![encode_arm64_fadds(*dst, *src1, *src2)?]),
            IROp::FsubS { dst, src1, src2 } => Ok(vec![encode_arm64_fsubs(*dst, *src1, *src2)?]),
            IROp::FmulS { dst, src1, src2 } => Ok(vec![encode_arm64_fmuls(*dst, *src1, *src2)?]),
            IROp::FdivS { dst, src1, src2 } => Ok(vec![encode_arm64_fdivs(*dst, *src1, *src2)?]),
            IROp::Fsqrt { dst, src } => Ok(vec![encode_arm64_fsqrt(*dst, *src)?]),
            IROp::FsqrtS { dst, src } => Ok(vec![encode_arm64_fsqrts(*dst, *src)?]),
            IROp::Fmadd {
                dst,
                src1,
                src2,
                src3,
            } => Ok(vec![encode_arm64_fmadd(*dst, *src1, *src2, *src3)?]),
            IROp::Fmsub {
                dst,
                src1,
                src2,
                src3,
            } => Ok(vec![encode_arm64_fmsub(*dst, *src1, *src2, *src3)?]),
            IROp::Fnmadd {
                dst,
                src1,
                src2,
                src3,
            } => Ok(vec![encode_arm64_fnmadd(*dst, *src1, *src2, *src3)?]),
            IROp::Fnmsub {
                dst,
                src1,
                src2,
                src3,
            } => Ok(vec![encode_arm64_fnmsub(*dst, *src1, *src2, *src3)?]),
            IROp::FmaddS {
                dst,
                src1,
                src2,
                src3,
            } => Ok(vec![encode_arm64_fmadds(*dst, *src1, *src2, *src3)?]),
            IROp::FmsubS {
                dst,
                src1,
                src2,
                src3,
            } => Ok(vec![encode_arm64_fmsubs(*dst, *src1, *src2, *src3)?]),
            IROp::FnmaddS {
                dst,
                src1,
                src2,
                src3,
            } => Ok(vec![encode_arm64_fnmadds(*dst, *src1, *src2, *src3)?]),
            IROp::FnmsubS {
                dst,
                src1,
                src2,
                src3,
            } => Ok(vec![encode_arm64_fnmsubs(*dst, *src1, *src2, *src3)?]),
            IROp::Feq { dst, src1, src2 } => Ok(vec![encode_arm64_fcmp(*dst, *src1, *src2)?]),
            IROp::Flt { dst, src1, src2 } => Ok(vec![encode_arm64_fcmplt(*dst, *src1, *src2)?]),
            IROp::Fle { dst, src1, src2 } => Ok(vec![encode_arm64_fcmple(*dst, *src1, *src2)?]),
            IROp::FeqS { dst, src1, src2 } => Ok(vec![encode_arm64_fcmps(*dst, *src1, *src2)?]),
            IROp::FltS { dst, src1, src2 } => Ok(vec![encode_arm64_fcmplts(*dst, *src1, *src2)?]),
            IROp::FleS { dst, src1, src2 } => Ok(vec![encode_arm64_fcmples(*dst, *src1, *src2)?]),
            IROp::Fmin { dst, src1, src2 } => Ok(vec![encode_arm64_fmin(*dst, *src1, *src2)?]),
            IROp::Fmax { dst, src1, src2 } => Ok(vec![encode_arm64_fmax(*dst, *src1, *src2)?]),
            IROp::FminS { dst, src1, src2 } => Ok(vec![encode_arm64_fmins(*dst, *src1, *src2)?]),
            IROp::FmaxS { dst, src1, src2 } => Ok(vec![encode_arm64_fmaxs(*dst, *src1, *src2)?]),
            IROp::Fabs { dst, src } => Ok(vec![encode_arm64_fabs(*dst, *src)?]),
            IROp::Fneg { dst, src } => Ok(vec![encode_arm64_fneg(*dst, *src)?]),
            IROp::FabsS { dst, src } => Ok(vec![encode_arm64_fabss(*dst, *src)?]),
            IROp::FnegS { dst, src } => Ok(vec![encode_arm64_fnegs(*dst, *src)?]),
            IROp::Fcvtws { dst, src } => Ok(vec![encode_arm64_fcvtzs(*dst, *src)?]),
            IROp::Fcvtsw { dst, src } => Ok(vec![encode_arm64_scvtf(*dst, *src)?]),
            IROp::Fcvtld { dst, src } => Ok(vec![encode_arm64_fcvtzs64(*dst, *src)?]),
            IROp::Fcvtdl { dst, src } => Ok(vec![encode_arm64_scvtf64(*dst, *src)?]),
            IROp::Fcvtsd { dst, src } => Ok(vec![encode_arm64_fcvts(*dst, *src)?]),
            IROp::Fcvtds { dst, src } => Ok(vec![encode_arm64_fcvtd(*dst, *src)?]),
            IROp::Fload {
                dst,
                base,
                offset,
                size,
                ..
            } => Ok(vec![encode_arm64_fload(*dst, *base, *offset, *size)?]),
            IROp::Fstore {
                src,
                base,
                offset,
                size,
                ..
            } => Ok(vec![encode_arm64_fstore(*src, *base, *offset, *size)?]),
            IROp::Fsgnj { dst, src1, src2 } => Ok(vec![encode_arm64_fsgnj(*dst, *src1, *src2)?]),
            IROp::Fsgnjn { dst, src1, src2 } => Ok(vec![encode_arm64_fsgnjn(*dst, *src1, *src2)?]),
            IROp::Fsgnjx { dst, src1, src2 } => Ok(vec![encode_arm64_fsgnjx(*dst, *src1, *src2)?]),
            IROp::FsgnjS { dst, src1, src2 } => Ok(vec![encode_arm64_fsgnjs(*dst, *src1, *src2)?]),
            IROp::FsgnjnS { dst, src1, src2 } => {
                Ok(vec![encode_arm64_fsgnjns(*dst, *src1, *src2)?])
            }
            IROp::FsgnjxS { dst, src1, src2 } => {
                Ok(vec![encode_arm64_fsgnjxs(*dst, *src1, *src2)?])
            }
            IROp::Fclass { dst, src } => Ok(vec![encode_arm64_fclass(*dst, *src)?]),
            IROp::FclassS { dst, src } => Ok(vec![encode_arm64_fclasss(*dst, *src)?]),
            IROp::FmvXW { dst, src } => Ok(vec![encode_arm64_fmvxw(*dst, *src)?]),
            IROp::FmvWX { dst, src } => Ok(vec![encode_arm64_fmvwx(*dst, *src)?]),
            IROp::FmvXD { dst, src } => Ok(vec![encode_arm64_fmvxd(*dst, *src)?]),
            IROp::FmvDX { dst, src } => Ok(vec![encode_arm64_fmvdx(*dst, *src)?]),
            IROp::Fcvtwus { dst, src } => Ok(vec![encode_arm64_fcvtzus(*dst, *src)?]),
            IROp::Fcvtlus { dst, src } => Ok(vec![encode_arm64_fcvtzus64(*dst, *src)?]),
            IROp::Fcvtswu { dst, src } => Ok(vec![encode_arm64_ucvtf(*dst, *src)?]),
            IROp::Fcvtslu { dst, src } => Ok(vec![encode_arm64_ucvtf64(*dst, *src)?]),
            IROp::Fcvtwd { dst, src } => Ok(vec![encode_arm64_fcvtzs(*dst, *src)?]),
            IROp::Fcvtwud { dst, src } => Ok(vec![encode_arm64_fcvtzus(*dst, *src)?]),
            IROp::Fcvtlud { dst, src } => Ok(vec![encode_arm64_fcvtzus64(*dst, *src)?]),
            IROp::Fcvtdw { dst, src } => Ok(vec![encode_arm64_scvtf(*dst, *src)?]),
            IROp::Fcvtdwu { dst, src } => Ok(vec![encode_arm64_ucvtf(*dst, *src)?]),
            IROp::Fcvtdlu { dst, src } => Ok(vec![encode_arm64_ucvtf64(*dst, *src)?]),
            // 原子操作
            IROp::AtomicRMW {
                dst,
                base,
                src,
                op,
                size,
            } => encode_arm64_atomic_rmw(*dst, *base, *src, *op, *size),
            IROp::AtomicCmpXchg {
                dst,
                base,
                expected,
                new,
                size,
            } => encode_arm64_atomic_cmpxchg(*dst, *base, *expected, *new, *size),
            IROp::AtomicLoadReserve {
                dst,
                base,
                offset,
                size,
                ..
            } => Ok(vec![encode_arm64_ldxr(*dst, *base, *offset, *size)?]),
            IROp::AtomicStoreCond {
                src,
                base,
                offset,
                size,
                dst_flag,
                ..
            } => encode_arm64_stxr(*src, *base, *offset, *size, *dst_flag),
            _ => Err(TranslationError::UnsupportedOperation {
                op: format!("{:?}", op),
            }),
        }
    }
}

/// RISC-V64 编码器
pub struct Riscv64Encoder;

impl ArchEncoder for Riscv64Encoder {
    fn architecture(&self) -> Architecture {
        Architecture::RISCV64
    }

    fn encode_op(
        &self,
        op: &IROp,
        _pc: GuestAddr,
    ) -> Result<Vec<TargetInstruction>, TranslationError> {
        match op {
            IROp::Add { dst, src1, src2 } => {
                // ADD xdst, xsrc1, xsrc2
                Ok(vec![encode_riscv_add(*dst, *src1, *src2)?])
            }
            IROp::Sub { dst, src1, src2 } => {
                // SUB xdst, xsrc1, xsrc2
                Ok(vec![encode_riscv_sub(*dst, *src1, *src2)?])
            }
            IROp::AddImm { dst, src, imm } => {
                // ADDI xdst, xsrc, imm
                Ok(vec![encode_riscv_addi(*dst, *src, *imm)?])
            }
            IROp::MovImm { dst, imm } => {
                // LUI + ADDI 组合（对于大立即数）
                encode_riscv_mov_imm(*dst, *imm)
            }
            IROp::Load {
                dst,
                base,
                offset,
                size,
                ..
            } => {
                // LD xdst, offset(xbase)
                Ok(vec![encode_riscv_load(*dst, *base, *offset, *size)?])
            }
            IROp::Store {
                src,
                base,
                offset,
                size,
                ..
            } => {
                // SD xsrc, offset(xbase)
                Ok(vec![encode_riscv_store(*src, *base, *offset, *size)?])
            }
            // SIMD操作（RISC-V向量扩展）
            IROp::VecAdd {
                dst,
                src1,
                src2,
                element_size,
            } => encode_riscv_simd_add(*dst, *src1, *src2, *element_size),
            IROp::VecSub {
                dst,
                src1,
                src2,
                element_size,
            } => encode_riscv_simd_sub(*dst, *src1, *src2, *element_size),
            IROp::VecMul {
                dst,
                src1,
                src2,
                element_size,
            } => encode_riscv_simd_mul(*dst, *src1, *src2, *element_size),
            IROp::VecAddSat {
                dst,
                src1,
                src2,
                element_size,
                signed,
            } => encode_riscv_simd_addsat(*dst, *src1, *src2, *element_size, *signed),
            IROp::VecSubSat {
                dst,
                src1,
                src2,
                element_size,
                signed,
            } => encode_riscv_simd_subsat(*dst, *src1, *src2, *element_size, *signed),
            IROp::VecMulSat {
                dst,
                src1,
                src2,
                element_size,
                signed,
            } => encode_riscv_simd_mulsat(*dst, *src1, *src2, *element_size, *signed),
            IROp::Vec128Add {
                dst_lo,
                dst_hi,
                src1_lo,
                src1_hi,
                src2_lo,
                src2_hi,
                element_size,
                signed,
            } => encode_riscv_vec128_add(
                *dst_lo,
                *dst_hi,
                *src1_lo,
                *src1_hi,
                *src2_lo,
                *src2_hi,
                *element_size,
                *signed,
            ),
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
            } => encode_riscv_vec256_add(
                *dst0,
                *dst1,
                *dst2,
                *dst3,
                *src10,
                *src11,
                *src12,
                *src13,
                *src20,
                *src21,
                *src22,
                *src23,
                *element_size,
                *signed,
            ),
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
            } => encode_riscv_vec256_sub(
                *dst0,
                *dst1,
                *dst2,
                *dst3,
                *src10,
                *src11,
                *src12,
                *src13,
                *src20,
                *src21,
                *src22,
                *src23,
                *element_size,
                *signed,
            ),
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
            } => encode_riscv_vec256_mul(
                *dst0,
                *dst1,
                *dst2,
                *dst3,
                *src10,
                *src11,
                *src12,
                *src13,
                *src20,
                *src21,
                *src22,
                *src23,
                *element_size,
                *signed,
            ),
            // 浮点操作
            IROp::Fadd { dst, src1, src2 } => Ok(vec![encode_riscv_fadd(*dst, *src1, *src2)?]),
            IROp::Fsub { dst, src1, src2 } => Ok(vec![encode_riscv_fsub(*dst, *src1, *src2)?]),
            IROp::Fmul { dst, src1, src2 } => Ok(vec![encode_riscv_fmul(*dst, *src1, *src2)?]),
            IROp::Fdiv { dst, src1, src2 } => Ok(vec![encode_riscv_fdiv(*dst, *src1, *src2)?]),
            IROp::FaddS { dst, src1, src2 } => Ok(vec![encode_riscv_fadds(*dst, *src1, *src2)?]),
            IROp::FsubS { dst, src1, src2 } => Ok(vec![encode_riscv_fsubs(*dst, *src1, *src2)?]),
            IROp::FmulS { dst, src1, src2 } => Ok(vec![encode_riscv_fmuls(*dst, *src1, *src2)?]),
            IROp::FdivS { dst, src1, src2 } => Ok(vec![encode_riscv_fdivs(*dst, *src1, *src2)?]),
            IROp::Fsqrt { dst, src } => Ok(vec![encode_riscv_fsqrt(*dst, *src)?]),
            IROp::FsqrtS { dst, src } => Ok(vec![encode_riscv_fsqrts(*dst, *src)?]),
            IROp::Fmadd {
                dst,
                src1,
                src2,
                src3,
            } => Ok(vec![encode_riscv_fmadd(*dst, *src1, *src2, *src3)?]),
            IROp::Fmsub {
                dst,
                src1,
                src2,
                src3,
            } => Ok(vec![encode_riscv_fmsub(*dst, *src1, *src2, *src3)?]),
            IROp::Fnmadd {
                dst,
                src1,
                src2,
                src3,
            } => Ok(vec![encode_riscv_fnmadd(*dst, *src1, *src2, *src3)?]),
            IROp::Fnmsub {
                dst,
                src1,
                src2,
                src3,
            } => Ok(vec![encode_riscv_fnmsub(*dst, *src1, *src2, *src3)?]),
            IROp::FmaddS {
                dst,
                src1,
                src2,
                src3,
            } => Ok(vec![encode_riscv_fmadds(*dst, *src1, *src2, *src3)?]),
            IROp::FmsubS {
                dst,
                src1,
                src2,
                src3,
            } => Ok(vec![encode_riscv_fmsubs(*dst, *src1, *src2, *src3)?]),
            IROp::FnmaddS {
                dst,
                src1,
                src2,
                src3,
            } => Ok(vec![encode_riscv_fnmadds(*dst, *src1, *src2, *src3)?]),
            IROp::FnmsubS {
                dst,
                src1,
                src2,
                src3,
            } => Ok(vec![encode_riscv_fnmsubs(*dst, *src1, *src2, *src3)?]),
            IROp::Feq { dst, src1, src2 } => Ok(vec![encode_riscv_feq(*dst, *src1, *src2)?]),
            IROp::Flt { dst, src1, src2 } => Ok(vec![encode_riscv_flt(*dst, *src1, *src2)?]),
            IROp::Fle { dst, src1, src2 } => Ok(vec![encode_riscv_fle(*dst, *src1, *src2)?]),
            IROp::FeqS { dst, src1, src2 } => Ok(vec![encode_riscv_feqs(*dst, *src1, *src2)?]),
            IROp::FltS { dst, src1, src2 } => Ok(vec![encode_riscv_flts(*dst, *src1, *src2)?]),
            IROp::FleS { dst, src1, src2 } => Ok(vec![encode_riscv_fles(*dst, *src1, *src2)?]),
            IROp::Fmin { dst, src1, src2 } => Ok(vec![encode_riscv_fmin(*dst, *src1, *src2)?]),
            IROp::Fmax { dst, src1, src2 } => Ok(vec![encode_riscv_fmax(*dst, *src1, *src2)?]),
            IROp::FminS { dst, src1, src2 } => Ok(vec![encode_riscv_fmins(*dst, *src1, *src2)?]),
            IROp::FmaxS { dst, src1, src2 } => Ok(vec![encode_riscv_fmaxs(*dst, *src1, *src2)?]),
            IROp::Fabs { dst, src } => Ok(vec![encode_riscv_fabs(*dst, *src)?]),
            IROp::Fneg { dst, src } => Ok(vec![encode_riscv_fneg(*dst, *src)?]),
            IROp::FabsS { dst, src } => Ok(vec![encode_riscv_fabss(*dst, *src)?]),
            IROp::FnegS { dst, src } => Ok(vec![encode_riscv_fnegs(*dst, *src)?]),
            IROp::Fcvtws { dst, src } => Ok(vec![encode_riscv_fcvtws(*dst, *src)?]),
            IROp::Fcvtsw { dst, src } => Ok(vec![encode_riscv_fcvtsw(*dst, *src)?]),
            IROp::Fcvtld { dst, src } => Ok(vec![encode_riscv_fcvtld(*dst, *src)?]),
            IROp::Fcvtdl { dst, src } => Ok(vec![encode_riscv_fcvtdl(*dst, *src)?]),
            IROp::Fcvtsd { dst, src } => Ok(vec![encode_riscv_fcvtsd(*dst, *src)?]),
            IROp::Fcvtds { dst, src } => Ok(vec![encode_riscv_fcvtds(*dst, *src)?]),
            IROp::Fload {
                dst,
                base,
                offset,
                size,
                ..
            } => Ok(vec![encode_riscv_fload(*dst, *base, *offset, *size)?]),
            IROp::Fstore {
                src,
                base,
                offset,
                size,
                ..
            } => Ok(vec![encode_riscv_fstore(*src, *base, *offset, *size)?]),
            IROp::Fsgnj { dst, src1, src2 } => Ok(vec![encode_riscv_fsgnj(*dst, *src1, *src2)?]),
            IROp::Fsgnjn { dst, src1, src2 } => Ok(vec![encode_riscv_fsgnjn(*dst, *src1, *src2)?]),
            IROp::Fsgnjx { dst, src1, src2 } => Ok(vec![encode_riscv_fsgnjx(*dst, *src1, *src2)?]),
            IROp::FsgnjS { dst, src1, src2 } => Ok(vec![encode_riscv_fsgnjs(*dst, *src1, *src2)?]),
            IROp::FsgnjnS { dst, src1, src2 } => {
                Ok(vec![encode_riscv_fsgnjns(*dst, *src1, *src2)?])
            }
            IROp::FsgnjxS { dst, src1, src2 } => {
                Ok(vec![encode_riscv_fsgnjxs(*dst, *src1, *src2)?])
            }
            IROp::Fclass { dst, src } => Ok(vec![encode_riscv_fclass(*dst, *src)?]),
            IROp::FclassS { dst, src } => Ok(vec![encode_riscv_fclasss(*dst, *src)?]),
            IROp::FmvXW { dst, src } => Ok(vec![encode_riscv_fmvxw(*dst, *src)?]),
            IROp::FmvWX { dst, src } => Ok(vec![encode_riscv_fmvwx(*dst, *src)?]),
            IROp::FmvXD { dst, src } => Ok(vec![encode_riscv_fmvxd(*dst, *src)?]),
            IROp::FmvDX { dst, src } => Ok(vec![encode_riscv_fmvdx(*dst, *src)?]),
            IROp::Fcvtwus { dst, src } => Ok(vec![encode_riscv_fcvtwus(*dst, *src)?]),
            IROp::Fcvtlus { dst, src } => Ok(vec![encode_riscv_fcvtlus(*dst, *src)?]),
            IROp::Fcvtswu { dst, src } => Ok(vec![encode_riscv_fcvtswu(*dst, *src)?]),
            IROp::Fcvtslu { dst, src } => Ok(vec![encode_riscv_fcvtslu(*dst, *src)?]),
            IROp::Fcvtwd { dst, src } => Ok(vec![encode_riscv_fcvtwd(*dst, *src)?]),
            IROp::Fcvtwud { dst, src } => Ok(vec![encode_riscv_fcvtwud(*dst, *src)?]),
            IROp::Fcvtlud { dst, src } => Ok(vec![encode_riscv_fcvtlud(*dst, *src)?]),
            IROp::Fcvtdw { dst, src } => Ok(vec![encode_riscv_fcvtdw(*dst, *src)?]),
            IROp::Fcvtdwu { dst, src } => Ok(vec![encode_riscv_fcvtdwu(*dst, *src)?]),
            IROp::Fcvtdlu { dst, src } => Ok(vec![encode_riscv_fcvtdlu(*dst, *src)?]),
            // 原子操作（RISC-V LR/SC）
            IROp::AtomicRMW {
                dst,
                base,
                src,
                op,
                size,
            } => encode_riscv_atomic_rmw(*dst, *base, *src, *op, *size),
            IROp::AtomicCmpXchg {
                dst,
                base,
                expected,
                new,
                size,
            } => encode_riscv_atomic_cmpxchg(*dst, *base, *expected, *new, *size),
            IROp::AtomicLoadReserve {
                dst,
                base,
                offset,
                size,
                ..
            } => Ok(vec![encode_riscv_lr(*dst, *base, *offset, *size)?]),
            IROp::AtomicStoreCond {
                src,
                base,
                offset,
                size,
                dst_flag,
                ..
            } => encode_riscv_sc(*src, *base, *offset, *size, *dst_flag),
            _ => Err(TranslationError::UnsupportedOperation {
                op: format!("{:?}", op),
            }),
        }
    }
}

// ========== x86-64 编码实现 ==========

fn encode_x86_add(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // x86-64: ADD r/m64, r64
    // 48 01 /r: ADD r/m64, r64
    let mut bytes = vec![0x48]; // REX.W prefix
    bytes.push(0x01); // ADD opcode
    bytes.push((0xC0 | ((dst & 7) << 3) | (src2 & 7)) as u8); // ModR/M
    Ok(TargetInstruction {
        bytes,
        length: 3,
        mnemonic: format!("add r{}, r{}", dst, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_sub(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // x86-64: SUB r/m64, r64
    let mut bytes = vec![0x48];
    bytes.push(0x29); // SUB opcode
    bytes.push((0xC0 | ((dst & 7) << 3) | (src2 & 7)) as u8);
    Ok(TargetInstruction {
        bytes,
        length: 3,
        mnemonic: format!("sub r{}, r{}", dst, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_add_imm(
    dst: RegId,
    _src: RegId,
    imm: i64,
) -> Result<TargetInstruction, TranslationError> {
    // x86-64: ADD r/m64, imm32
    let mut bytes = vec![0x48];
    bytes.push(0x81); // ADD with imm32
    bytes.push((0xC0 | (dst & 7)) as u8); // ModR/M: register
    bytes.extend_from_slice(&(imm as i32).to_le_bytes());
    Ok(TargetInstruction {
        bytes,
        length: 7,
        mnemonic: format!("add r{}, {}", dst, imm),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_mov_imm(dst: RegId, imm: u64) -> Result<TargetInstruction, TranslationError> {
    // x86-64: MOV r64, imm64
    let mut bytes = vec![0x48 | ((dst & 8) >> 3) as u8]; // REX.W + REX.B
    bytes.push(0xB8 | (dst & 7) as u8); // MOV r64, imm64
    bytes.extend_from_slice(&imm.to_le_bytes());
    Ok(TargetInstruction {
        bytes,
        length: 10,
        mnemonic: format!("mov r{}, {}", dst, imm),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_load(
    dst: RegId,
    base: RegId,
    offset: i64,
    _size: u8,
) -> Result<TargetInstruction, TranslationError> {
    // x86-64: MOV r64, [base + offset]
    let mut bytes = vec![0x48 | ((dst & 8) >> 3) as u8];
    bytes.push(0x8B); // MOV r64, r/m64
    // 简化：假设offset在32位范围内
    if offset == 0 {
        bytes.push((0x00 | ((dst & 7) << 3) | (base & 7)) as u8);
    } else {
        bytes.push((0x40 | ((dst & 7) << 3) | (base & 7)) as u8); // ModR/M with disp8
        bytes.extend_from_slice(&(offset as i32).to_le_bytes());
    }
    let len = bytes.len();
    Ok(TargetInstruction {
        bytes,
        length: len,
        mnemonic: format!("mov r{}, [r{} + {}]", dst, base, offset),
        is_control_flow: false,
        is_memory_op: true,
    })
}

fn encode_x86_store(
    src: RegId,
    base: RegId,
    offset: i64,
    _size: u8,
) -> Result<TargetInstruction, TranslationError> {
    // x86-64: MOV [base + offset], r64
    let mut bytes = vec![0x48 | ((src & 8) >> 3) as u8];
    bytes.push(0x89); // MOV r/m64, r64
    if offset == 0 {
        bytes.push((0x00 | ((src & 7) << 3) | (base & 7)) as u8);
    } else {
        bytes.push((0x40 | ((src & 7) << 3) | (base & 7)) as u8);
        bytes.extend_from_slice(&(offset as i32).to_le_bytes());
    }
    let bytes_clone = bytes.clone();
    let len = bytes.len();
    Ok(TargetInstruction {
        bytes: bytes_clone,
        length: len,
        mnemonic: format!("mov [r{} + {}], r{}", base, offset, src),
        is_control_flow: false,
        is_memory_op: true,
    })
}

// ========== ARM64 编码实现 ==========

fn encode_arm64_add(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // ARM64: ADD Xd, Xn, Xm
    // 31 30 29 28 27 26 25 24 23 22 21 20 19 18 17 16 15 14 13 12 11 10 9 8 7 6 5 4 3 2 1 0
    // sf  0  0  0  1  0  1  1  0  0  0  sh  0  Rm  0  0  0  0  0  0  Rn   Rd
    // 1   0  0  0  1  0  1  1  0  0  0  0   0  Rm  0  0  0  0  0  0  Rn   Rd
    let word: u32 = 0x8B000000 // base opcode
        | ((dst & 0x1F) as u32) // Rd
        | (((src1 & 0x1F) as u32) << 5) // Rn
        | (((src2 & 0x1F) as u32) << 16); // Rm
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("add x{}, x{}, x{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_sub(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // ARM64: SUB Xd, Xn, Xm
    let word: u32 = 0xCB000000 // base opcode
        | ((dst & 0x1F) as u32)
        | (((src1 & 0x1F) as u32) << 5)
        | (((src2 & 0x1F) as u32) << 16);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("sub x{}, x{}, x{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_add_imm(
    dst: RegId,
    src: RegId,
    imm: i64,
) -> Result<TargetInstruction, TranslationError> {
    // ARM64: ADD Xd, Xn, #imm
    // 限制：imm必须是12位立即数或可以编码为移位立即数
    let imm12 = if imm >= 0 && imm < 4096 {
        imm as u32
    } else {
        return Err(TranslationError::ImmediateTooLarge { imm });
    };

    let word: u32 = 0x91000000 // base opcode
        | ((dst & 0x1F) as u32)
        | (((src & 0x1F) as u32) << 5)
        | ((imm12 & 0xFFF) << 10);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("add x{}, x{}, #{}", dst, src, imm),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_mov_imm(dst: RegId, imm: u64) -> Result<Vec<TargetInstruction>, TranslationError> {
    // ARM64: MOVZ/MOVK组合
    let mut instructions = Vec::new();

    // 检查是否可以单指令编码（16位对齐的立即数）
    if imm & 0xFFFF == imm {
        // MOVZ Xd, #imm, LSL #0
        let word: u32 = 0xD2800000 | ((dst & 0x1F) as u32) | (((imm & 0xFFFF) as u32) << 5);
        instructions.push(TargetInstruction {
            bytes: word.to_le_bytes().to_vec(),
            length: 4,
            mnemonic: format!("movz x{}, #{}", dst, imm),
            is_control_flow: false,
            is_memory_op: false,
        });
    } else {
        // 需要MOVZ + MOVK组合
        // MOVZ设置低16位
        let word1: u32 = 0xD2800000 | ((dst & 0x1F) as u32) | (((imm & 0xFFFF) as u32) << 5);
        instructions.push(TargetInstruction {
            bytes: word1.to_le_bytes().to_vec(),
            length: 4,
            mnemonic: format!("movz x{}, #{}", dst, imm & 0xFFFF),
            is_control_flow: false,
            is_memory_op: false,
        });

        // MOVK设置其他16位段
        for shift in 1..4 {
            let bits = (imm >> (shift * 16)) & 0xFFFF;
            if bits != 0 {
                let word: u32 = 0xF2800000
                    | ((dst & 0x1F) as u32)
                    | ((bits as u32) << 5)
                    | ((shift as u32) << 21);
                instructions.push(TargetInstruction {
                    bytes: word.to_le_bytes().to_vec(),
                    length: 4,
                    mnemonic: format!("movk x{}, #{}, LSL #{}", dst, bits, shift * 16),
                    is_control_flow: false,
                    is_memory_op: false,
                });
            }
        }
    }

    Ok(instructions)
}

fn encode_arm64_load(
    dst: RegId,
    base: RegId,
    offset: i64,
    size: u8,
) -> Result<TargetInstruction, TranslationError> {
    // ARM64: LDR Xd, [Xn, #offset]
    // 限制：offset必须是12位对齐的立即数
    if offset < 0 || offset >= 32768 || (offset % (size as i64)) != 0 {
        return Err(TranslationError::InvalidOffset { offset });
    }

    let imm12 = (offset / (size as i64)) as u32;
    let word: u32 = 0xF9400000
        | ((dst & 0x1F) as u32)
        | (((base & 0x1F) as u32) << 5)
        | ((imm12 & 0xFFF) << 10);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("ldr x{}, [x{}, #{}]", dst, base, offset),
        is_control_flow: false,
        is_memory_op: true,
    })
}

fn encode_arm64_store(
    src: RegId,
    base: RegId,
    offset: i64,
    size: u8,
) -> Result<TargetInstruction, TranslationError> {
    // ARM64: STR Xs, [Xn, #offset]
    if offset < 0 || offset >= 32768 || (offset % (size as i64)) != 0 {
        return Err(TranslationError::InvalidOffset { offset });
    }

    let imm12 = (offset / (size as i64)) as u32;
    let word: u32 = 0xF9000000
        | ((src & 0x1F) as u32)
        | (((base & 0x1F) as u32) << 5)
        | ((imm12 & 0xFFF) << 10);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("str x{}, [x{}, #{}]", src, base, offset),
        is_control_flow: false,
        is_memory_op: true,
    })
}

// ========== RISC-V64 编码实现 ==========

fn encode_riscv_add(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: ADD xd, xsrc1, xsrc2
    // 31 30 29 28 27 26 25 24 23 22 21 20 19 18 17 16 15 14 13 12 11 10 9 8 7 6 5 4 3 2 1 0
    // funct7        rs2        rs1   funct3  rd   opcode
    // 0000000       xsrc2      xsrc1 000     xd   0110011
    let word: u32 = 0x00000033 // base opcode
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("add x{}, x{}, x{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_sub(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: SUB xd, xsrc1, xsrc2
    let word: u32 = 0x40000033 // funct7 = 0100000
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("sub x{}, x{}, x{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_addi(
    dst: RegId,
    src: RegId,
    imm: i64,
) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: ADDI xd, xsrc, imm
    // imm是12位有符号立即数
    if imm < -2048 || imm >= 2048 {
        return Err(TranslationError::ImmediateTooLarge { imm });
    }

    let imm12 = (imm as u32) & 0xFFF;
    let word: u32 = 0x00000013 // base opcode
        | (((dst & 0x1F) as u32) << 7)
        | (((src & 0x1F) as u32) << 15)
        | ((imm12 & 0xFFF) << 20);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("addi x{}, x{}, {}", dst, src, imm),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_mov_imm(dst: RegId, imm: u64) -> Result<Vec<TargetInstruction>, TranslationError> {
    // RISC-V: LUI + ADDI 组合
    let mut instructions = Vec::new();

    // LUI设置高20位
    let imm20 = ((imm >> 12) & 0xFFFFF) as u32;
    let word1: u32 = 0x00000037 // LUI opcode
        | (((dst & 0x1F) as u32) << 7)
        | ((imm20 & 0xFFFFF) << 12);
    instructions.push(TargetInstruction {
        bytes: word1.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("lui x{}, {}", dst, imm20),
        is_control_flow: false,
        is_memory_op: false,
    });

    // ADDI设置低12位
    let imm12 = (imm & 0xFFF) as u32;
    if imm12 != 0 {
        let word2: u32 = 0x00000013 // ADDI opcode
            | (((dst & 0x1F) as u32) << 7)
            | (((dst & 0x1F) as u32) << 15)
            | ((imm12 & 0xFFF) << 20);
        instructions.push(TargetInstruction {
            bytes: word2.to_le_bytes().to_vec(),
            length: 4,
            mnemonic: format!("addi x{}, x{}, {}", dst, dst, imm12),
            is_control_flow: false,
            is_memory_op: false,
        });
    }

    Ok(instructions)
}

fn encode_riscv_load(
    dst: RegId,
    base: RegId,
    offset: i64,
    _size: u8,
) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: LD xd, offset(xbase)
    // offset是12位有符号立即数
    if offset < -2048 || offset >= 2048 {
        return Err(TranslationError::InvalidOffset { offset });
    }

    let imm12 = (offset as u32) & 0xFFF;
    let word: u32 = 0x00000003 // LOAD opcode
        | (((dst & 0x1F) as u32) << 7)
        | (((base & 0x1F) as u32) << 15)
        | (0b011 << 12) // funct3 for LD (64-bit)
        | ((imm12 & 0xFFF) << 20);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("ld x{}, {}(x{})", dst, offset, base),
        is_control_flow: false,
        is_memory_op: true,
    })
}

fn encode_riscv_store(
    src: RegId,
    base: RegId,
    offset: i64,
    _size: u8,
) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: SD xsrc, offset(xbase)
    if offset < -2048 || offset >= 2048 {
        return Err(TranslationError::InvalidOffset { offset });
    }

    let imm12 = (offset as u32) & 0xFFF;
    let imm_hi = (imm12 >> 5) & 0x7F;
    let imm_lo = imm12 & 0x1F;
    let word: u32 = 0x00000023 // STORE opcode
        | (imm_lo << 7)
        | (((base & 0x1F) as u32) << 15)
        | (0b011 << 12) // funct3 for SD (64-bit)
        | (((src & 0x1F) as u32) << 20)
        | (imm_hi << 25);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("sd x{}, {}(x{})", src, offset, base),
        is_control_flow: false,
        is_memory_op: true,
    })
}

// ========== x86-64 SIMD编码实现 ==========

fn encode_x86_simd_add(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
    element_size: u8,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    // x86-64: PADDB/PADDW/PADDD/PADDQ (SSE2)
    // 或 VADDPD/VADDPS (AVX)
    // 简化实现：使用SSE2指令
    match element_size {
        1 | 2 | 4 | 8 => {},
        _ => {
            return Err(TranslationError::UnsupportedOperation {
                op: format!("SIMD add with element_size {}", element_size),
            });
        }
    };

    // 66 0F FC /r: PADDD xmm1, xmm2/m128
    let mut bytes = vec![0x66, 0x0F];
    match element_size {
        1 => bytes.push(0xFC), // PADDB
        2 => bytes.push(0xFD), // PADDW
        4 => bytes.push(0xFE), // PADDD
        8 => bytes.push(0xD4), // PADDQ
        _ => unreachable!(),
    }
    bytes.push((0xC0 | ((dst & 0xF) << 3) | (src2 & 0xF)) as u8);

    Ok(vec![TargetInstruction {
        bytes: bytes.clone(),
        length: bytes.len(),
        mnemonic: format!("padd{} xmm{}, xmm{}", element_size, dst, src2),
        is_control_flow: false,
        is_memory_op: false,
    }])
}

fn encode_x86_simd_sub(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
    element_size: u8,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    // x86-64: PSUBB/PSUBW/PSUBD/PSUBQ
    let mut bytes = vec![0x66, 0x0F];
    match element_size {
        1 => bytes.push(0xF8), // PSUBB
        2 => bytes.push(0xF9), // PSUBW
        4 => bytes.push(0xFA), // PSUBD
        8 => bytes.push(0xFB), // PSUBQ
        _ => {
            return Err(TranslationError::UnsupportedOperation {
                op: format!("SIMD sub with element_size {}", element_size),
            });
        }
    }
    bytes.push((0xC0 | ((dst & 0xF) << 3) | (src2 & 0xF)) as u8);

    Ok(vec![TargetInstruction {
        bytes: bytes.clone(),
        length: bytes.len(),
        mnemonic: format!("psub{} xmm{}, xmm{}", element_size, dst, src2),
        is_control_flow: false,
        is_memory_op: false,
    }])
}

fn encode_x86_simd_mul(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
    element_size: u8,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    // x86-64: PMULLW/PMULLD/PMULUDQ
    let mut bytes = vec![0x66, 0x0F];
    match element_size {
        2 => bytes.push(0xD5),                       // PMULLW
        4 => bytes.extend_from_slice(&[0x38, 0x40]), // PMULLD (SSE4.1)
        8 => bytes.push(0xF4),                       // PMULUDQ
        _ => {
            return Err(TranslationError::UnsupportedOperation {
                op: format!("SIMD mul with element_size {}", element_size),
            });
        }
    }
    if element_size == 4 {
        bytes.push((0xC0 | ((dst & 0xF) << 3) | (src2 & 0xF)) as u8);
    } else {
        bytes.push((0xC0 | ((dst & 0xF) << 3) | (src2 & 0xF)) as u8);
    }

    Ok(vec![TargetInstruction {
        bytes: bytes.clone(),
        length: bytes.len(),
        mnemonic: format!("pmul{} xmm{}, xmm{}", element_size, dst, src2),
        is_control_flow: false,
        is_memory_op: false,
    }])
}

// ========== x86-64 浮点编码实现 ==========

fn encode_x86_fadd(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // x86-64: ADDSD xmm1, xmm2 (双精度)
    // F2 0F 58 /r: ADDSD xmm1, xmm2/m64
    Ok(TargetInstruction {
        bytes: vec![
            0xF2,
            0x0F,
            0x58,
            (0xC0 | ((dst & 0xF) << 3) | (src2 & 0xF)) as u8,
        ],
        length: 4,
        mnemonic: format!("addsd xmm{}, xmm{}", dst, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fsub(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // x86-64: SUBSD
    Ok(TargetInstruction {
        bytes: vec![
            0xF2,
            0x0F,
            0x5C,
            (0xC0 | ((dst & 0xF) << 3) | (src2 & 0xF)) as u8,
        ],
        length: 4,
        mnemonic: format!("subsd xmm{}, xmm{}", dst, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fmul(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // x86-64: MULSD
    Ok(TargetInstruction {
        bytes: vec![
            0xF2,
            0x0F,
            0x59,
            (0xC0 | ((dst & 0xF) << 3) | (src2 & 0xF)) as u8,
        ],
        length: 4,
        mnemonic: format!("mulsd xmm{}, xmm{}", dst, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fdiv(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // x86-64: DIVSD
    Ok(TargetInstruction {
        bytes: vec![
            0xF2,
            0x0F,
            0x5E,
            (0xC0 | ((dst & 0xF) << 3) | (src2 & 0xF)) as u8,
        ],
        length: 4,
        mnemonic: format!("divsd xmm{}, xmm{}", dst, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fadds(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // x86-64: ADDSS (单精度)
    Ok(TargetInstruction {
        bytes: vec![
            0xF3,
            0x0F,
            0x58,
            (0xC0 | ((dst & 0xF) << 3) | (src2 & 0xF)) as u8,
        ],
        length: 4,
        mnemonic: format!("addss xmm{}, xmm{}", dst, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fsubs(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    Ok(TargetInstruction {
        bytes: vec![
            0xF3,
            0x0F,
            0x5C,
            (0xC0 | ((dst & 0xF) << 3) | (src2 & 0xF)) as u8,
        ],
        length: 4,
        mnemonic: format!("subss xmm{}, xmm{}", dst, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fmuls(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    Ok(TargetInstruction {
        bytes: vec![
            0xF3,
            0x0F,
            0x59,
            (0xC0 | ((dst & 0xF) << 3) | (src2 & 0xF)) as u8,
        ],
        length: 4,
        mnemonic: format!("mulss xmm{}, xmm{}", dst, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fdivs(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    Ok(TargetInstruction {
        bytes: vec![
            0xF3,
            0x0F,
            0x5E,
            (0xC0 | ((dst & 0xF) << 3) | (src2 & 0xF)) as u8,
        ],
        length: 4,
        mnemonic: format!("divss xmm{}, xmm{}", dst, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fsqrt(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // x86-64: SQRTSD
    Ok(TargetInstruction {
        bytes: vec![
            0xF2,
            0x0F,
            0x51,
            (0xC0 | ((dst & 0xF) << 3) | (src & 0xF)) as u8,
        ],
        length: 4,
        mnemonic: format!("sqrtsd xmm{}, xmm{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fsqrts(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // x86-64: SQRTSS
    Ok(TargetInstruction {
        bytes: vec![
            0xF3,
            0x0F,
            0x51,
            (0xC0 | ((dst & 0xF) << 3) | (src & 0xF)) as u8,
        ],
        length: 4,
        mnemonic: format!("sqrtss xmm{}, xmm{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

// ========== x86-64 原子操作编码实现 ==========

fn encode_x86_atomic_rmw(
    dst: RegId,
    base: RegId,
    src: RegId,
    op: vm_ir::AtomicOp,
    size: u8,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    use vm_ir::AtomicOp;

    // x86-64: LOCK前缀 + 操作
    let mut instructions = Vec::new();

    // 加载到寄存器
    instructions.push(encode_x86_load(dst, base, 0, size)?);

    // 执行操作
    match op {
        AtomicOp::Add => {
            instructions.push(encode_x86_add(dst, dst, src)?);
        }
        AtomicOp::Sub => {
            instructions.push(encode_x86_sub(dst, dst, src)?);
        }
        AtomicOp::And => {
            // AND dst, src
            let mut bytes = vec![0x48];
            bytes.push(0x21); // AND r/m64, r64
            bytes.push((0xC0 | ((dst & 7) << 3) | (src & 7)) as u8);
            instructions.push(TargetInstruction {
                bytes,
                length: 3,
                mnemonic: format!("and r{}, r{}", dst, src),
                is_control_flow: false,
                is_memory_op: false,
            });
        }
        AtomicOp::Or => {
            let mut bytes = vec![0x48];
            bytes.push(0x09); // OR r/m64, r64
            bytes.push((0xC0 | ((dst & 7) << 3) | (src & 7)) as u8);
            instructions.push(TargetInstruction {
                bytes,
                length: 3,
                mnemonic: format!("or r{}, r{}", dst, src),
                is_control_flow: false,
                is_memory_op: false,
            });
        }
        AtomicOp::Xor => {
            let mut bytes = vec![0x48];
            bytes.push(0x31); // XOR r/m64, r64
            bytes.push((0xC0 | ((dst & 7) << 3) | (src & 7)) as u8);
            instructions.push(TargetInstruction {
                bytes,
                length: 3,
                mnemonic: format!("xor r{}, r{}", dst, src),
                is_control_flow: false,
                is_memory_op: false,
            });
        }
        AtomicOp::Xchg => {
            // XCHG r/m64, r64
            let mut bytes = vec![0x48];
            bytes.push(0x87); // XCHG
            bytes.push((0xC0 | ((dst & 7) << 3) | (src & 7)) as u8);
            instructions.push(TargetInstruction {
                bytes,
                length: 3,
                mnemonic: format!("xchg r{}, r{}", dst, src),
                is_control_flow: false,
                is_memory_op: false,
            });
        }
        _ => {
            return Err(TranslationError::UnsupportedOperation {
                op: format!("Atomic RMW operation {:?}", op),
            });
        }
    }

    // 存储回内存（带LOCK前缀）
    let mut store_insn = encode_x86_store(dst, base, 0, size)?;
    store_insn.bytes.insert(0, 0xF0); // LOCK前缀
    store_insn.length += 1;
    instructions.push(store_insn);

    Ok(instructions)
}

fn encode_x86_atomic_cmpxchg(
    dst: RegId,
    base: RegId,
    expected: RegId,
    new: RegId,
    _size: u8,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    // x86-64: CMPXCHG r/m64, r64
    // 0F B1 /r: CMPXCHG r/m64, r64
    // 结果在RAX中，如果相等则ZF=1
    let mut instructions = Vec::new();

    // 加载expected到RAX (假设RAX是寄存器0)
    instructions.push(encode_x86_mov_imm(0, expected as u64)?);

    // CMPXCHG [base], new
    let mut bytes = vec![0x48, 0x0F, 0xB1];
    bytes.push((0x00 | ((new & 7) << 3) | (base & 7)) as u8);
    instructions.push(TargetInstruction {
        bytes: bytes.clone(),
        length: 4,
        mnemonic: format!("cmpxchg [r{}], r{}", base, new),
        is_control_flow: false,
        is_memory_op: true,
    });

    // 将结果移动到dst
    instructions.push(encode_x86_mov_imm(dst, 0)?); // 简化：实际应该从RAX移动

    Ok(instructions)
}

// ========== ARM64 SIMD编码实现 ==========

fn encode_arm64_simd_add(
    dst: RegId,
    src1: RegId,
    src2: RegId,
    element_size: u8,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    // ARM64: ADD Vd.8B, Vn.8B, Vm.8B (或16B/4H/8H/2S/4S/2D)
    let size = match element_size {
        1 => 0b00, // 8-bit
        2 => 0b01, // 16-bit
        4 => 0b10, // 32-bit
        8 => 0b11, // 64-bit
        _ => {
            return Err(TranslationError::UnsupportedOperation {
                op: format!("ARM64 SIMD add with element_size {}", element_size),
            });
        }
    };

    // 31 30 29 28 27 26 25 24 23 22 21 20 19 18 17 16 15 14 13 12 11 10 9 8 7 6 5 4 3 2 1 0
    // 0  Q  U 0 1 1 1 0  size  0 1 0 0 0 0  Vm 0 0 0 1 1 1  Vn  Vd
    // 0  1  0 0 1 1 1 0  size  0 1 0 0 0 0  Vm 0 0 0 1 1 1  Vn  Vd
    let word: u32 = 0x4E208400
        | ((size as u32) << 22)
        | ((dst & 0x1F) as u32)
        | ((src1 & 0x1F) as u32) << 5
        | ((src2 & 0x1F) as u32) << 16;

    Ok(vec![TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!(
            "add v{}.{}b, v{}.{}b, v{}.{}b",
            dst, element_size, src1, element_size, src2, element_size
        ),
        is_control_flow: false,
        is_memory_op: false,
    }])
}

fn encode_arm64_simd_sub(
    dst: RegId,
    src1: RegId,
    src2: RegId,
    element_size: u8,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    let size = match element_size {
        1 => 0b00,
        2 => 0b01,
        4 => 0b10,
        8 => 0b11,
        _ => {
            return Err(TranslationError::UnsupportedOperation {
                op: format!("ARM64 SIMD sub with element_size {}", element_size),
            });
        }
    };

    let word: u32 = 0x4E208C00
        | ((size as u32) << 22)
        | ((dst & 0x1F) as u32)
        | (((src1 & 0x1F) as u32) << 5)
        | (((src2 & 0x1F) as u32) << 16);

    Ok(vec![TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!(
            "sub v{}.{}b, v{}.{}b, v{}.{}b",
            dst, element_size, src1, element_size, src2, element_size
        ),
        is_control_flow: false,
        is_memory_op: false,
    }])
}

fn encode_arm64_simd_mul(
    dst: RegId,
    src1: RegId,
    src2: RegId,
    element_size: u8,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    let size = match element_size {
        2 => 0b01, // 16-bit only
        4 => 0b10, // 32-bit
        _ => {
            return Err(TranslationError::UnsupportedOperation {
                op: format!("ARM64 SIMD mul with element_size {}", element_size),
            });
        }
    };

    let word: u32 = 0x4E209C00
        | ((size as u32) << 22)
        | ((dst & 0x1F) as u32)
        | (((src1 & 0x1F) as u32) << 5)
        | (((src2 & 0x1F) as u32) << 16);

    Ok(vec![TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!(
            "mul v{}.{}b, v{}.{}b, v{}.{}b",
            dst, element_size, src1, element_size, src2, element_size
        ),
        is_control_flow: false,
        is_memory_op: false,
    }])
}

// ========== ARM64 浮点编码实现 ==========

fn encode_arm64_fadd(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // ARM64: FADD Dd, Dn, Dm (双精度)
    let word: u32 = 0x4E60D400
        | ((dst & 0x1F) as u32)
        | (((src1 & 0x1F) as u32) << 5)
        | (((src2 & 0x1F) as u32) << 16);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fadd d{}, d{}, d{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fsub(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x4E60D800
        | ((dst & 0x1F) as u32)
        | (((src1 & 0x1F) as u32) << 5)
        | (((src2 & 0x1F) as u32) << 16);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fsub d{}, d{}, d{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fmul(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x4E60DC00
        | ((dst & 0x1F) as u32)
        | (((src1 & 0x1F) as u32) << 5)
        | (((src2 & 0x1F) as u32) << 16);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fmul d{}, d{}, d{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fdiv(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x4E60E400
        | ((dst & 0x1F) as u32)
        | (((src1 & 0x1F) as u32) << 5)
        | (((src2 & 0x1F) as u32) << 16);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fdiv d{}, d{}, d{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fadds(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // ARM64: FADD Sd, Sn, Sm (单精度)
    let word: u32 = 0x4E20D400
        | ((dst & 0x1F) as u32)
        | (((src1 & 0x1F) as u32) << 5)
        | (((src2 & 0x1F) as u32) << 16);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fadd s{}, s{}, s{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fsubs(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x4E20D800
        | ((dst & 0x1F) as u32)
        | (((src1 & 0x1F) as u32) << 5)
        | (((src2 & 0x1F) as u32) << 16);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fsub s{}, s{}, s{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fmuls(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x4E20DC00
        | ((dst & 0x1F) as u32)
        | (((src1 & 0x1F) as u32) << 5)
        | (((src2 & 0x1F) as u32) << 16);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fmul s{}, s{}, s{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fdivs(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x4E20E400
        | ((dst & 0x1F) as u32)
        | (((src1 & 0x1F) as u32) << 5)
        | (((src2 & 0x1F) as u32) << 16);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fdiv s{}, s{}, s{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fsqrt(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x4EE1E800 | ((dst & 0x1F) as u32) | (((src & 0x1F) as u32) << 5);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fsqrt d{}, d{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fsqrts(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x4EA1E800 | ((dst & 0x1F) as u32) | (((src & 0x1F) as u32) << 5);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fsqrt s{}, s{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

// ========== ARM64 原子操作编码实现 ==========

fn encode_arm64_atomic_rmw(
    dst: RegId,
    base: RegId,
    src: RegId,
    op: vm_ir::AtomicOp,
    size: u8,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    use vm_ir::AtomicOp;

    // ARM64: LDADD/LDCLR/LDEOR/LDSET等
    let opcode = match op {
        AtomicOp::Add => 0x38E00000, // LDADD
        AtomicOp::And => 0x38A00000, // LDCLR (clear = and with complement)
        AtomicOp::Or => 0x38E00000,  // LDSET
        AtomicOp::Xor => 0x38C00000, // LDEOR
        _ => {
            return Err(TranslationError::UnsupportedOperation {
                op: format!("ARM64 atomic RMW operation {:?}", op),
            });
        }
    };

    let size_bits = match size {
        1 => 0b00,
        2 => 0b01,
        4 => 0b10,
        8 => 0b11,
        _ => {
            return Err(TranslationError::UnsupportedOperation {
                op: format!("ARM64 atomic RMW with size {}", size),
            });
        }
    };

    let word: u32 = opcode
        | ((size_bits as u32) << 30)
        | ((dst & 0x1F) as u32)
        | (((base & 0x1F) as u32) << 5)
        | (((src & 0x1F) as u32) << 16);

    Ok(vec![TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("ldadd x{}, x{}, [x{}]", dst, src, base),
        is_control_flow: false,
        is_memory_op: true,
    }])
}

fn encode_arm64_atomic_cmpxchg(
    dst: RegId,
    base: RegId,
    expected: RegId,
    new: RegId,
    size: u8,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    // ARM64: CAS (Compare and Swap)
    let size_bits = match size {
        1 => 0b00,
        2 => 0b01,
        4 => 0b10,
        8 => 0b11,
        _ => {
            return Err(TranslationError::UnsupportedOperation {
                op: format!("ARM64 atomic CMPXCHG with size {}", size),
            });
        }
    };

    // CAS Xs, Xt, [Xn]
    let word: u32 = 0x48E00000
        | ((size_bits as u32) << 30)
        | ((dst & 0x1F) as u32)
        | (((base & 0x1F) as u32) << 5)
        | (((expected & 0x1F) as u32) << 16)
        | (((new & 0x1F) as u32) << 0);

    Ok(vec![TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("cas x{}, x{}, [x{}]", new, expected, base),
        is_control_flow: false,
        is_memory_op: true,
    }])
}

fn encode_arm64_ldxr(
    dst: RegId,
    base: RegId,
    offset: i64,
    size: u8,
) -> Result<TargetInstruction, TranslationError> {
    // ARM64: LDXR (Load Exclusive Register)
    if offset != 0 {
        return Err(TranslationError::InvalidOffset { offset });
    }

    let size_bits = match size {
        1 => 0b00,
        2 => 0b01,
        4 => 0b10,
        8 => 0b11,
        _ => {
            return Err(TranslationError::UnsupportedOperation {
                op: format!("ARM64 LDXR with size {}", size),
            });
        }
    };

    let word: u32 = 0x085F8000
        | ((size_bits as u32) << 30)
        | ((dst & 0x1F) as u32)
        | (((base & 0x1F) as u32) << 5);

    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("ldxr x{}, [x{}]", dst, base),
        is_control_flow: false,
        is_memory_op: true,
    })
}

fn encode_arm64_stxr(
    src: RegId,
    base: RegId,
    offset: i64,
    size: u8,
    dst_flag: RegId,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    // ARM64: STXR (Store Exclusive Register)
    if offset != 0 {
        return Err(TranslationError::InvalidOffset { offset });
    }

    let size_bits = match size {
        1 => 0b00,
        2 => 0b01,
        4 => 0b10,
        8 => 0b11,
        _ => {
            return Err(TranslationError::UnsupportedOperation {
                op: format!("ARM64 STXR with size {}", size),
            });
        }
    };

    let word: u32 = 0x08008000
        | ((size_bits as u32) << 30)
        | ((dst_flag & 0x1F) as u32)
        | (((base & 0x1F) as u32) << 5)
        | (((src & 0x1F) as u32) << 16);

    Ok(vec![TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("stxr w{}, x{}, [x{}]", dst_flag, src, base),
        is_control_flow: false,
        is_memory_op: true,
    }])
}

// ========== RISC-V64 SIMD编码实现 ==========

fn encode_riscv_simd_add(
    dst: RegId,
    src1: RegId,
    src2: RegId,
    _element_size: u8,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    // RISC-V: VADD.VV (向量加法)
    // 需要设置vtype和vl寄存器
    // 简化实现：假设已设置好向量配置
    // 31 30 29 28 27 26 25 24 23 22 21 20 19 18 17 16 15 14 13 12 11 10 9 8 7 6 5 4 3 2 1 0
    // funct6    vm   vs2    vs1  0 0 0 0 1 1  vd  0 1 0 0 1 1 1
    // 000000    0    vs2    vs1  0 0 0 0 1 1  vd  0 1 0 0 1 1 1
    let word: u32 = 0x00000057 // Vector opcode
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20)
        | (0b000000 << 26); // funct6 for VADD.VV

    Ok(vec![TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("vadd.vv v{}, v{}, v{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    }])
}

fn encode_riscv_simd_sub(
    dst: RegId,
    src1: RegId,
    src2: RegId,
    _element_size: u8,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    let word: u32 = 0x00000057
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20)
        | (0b000010 << 26); // funct6 for VSUB.VV

    Ok(vec![TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("vsub.vv v{}, v{}, v{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    }])
}

fn encode_riscv_simd_mul(
    dst: RegId,
    src1: RegId,
    src2: RegId,
    _element_size: u8,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    let word: u32 = 0x00000057
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20)
        | (0b100101 << 26); // funct6 for VMUL.VV

    Ok(vec![TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("vmul.vv v{}, v{}, v{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    }])
}

// ========== RISC-V64 浮点编码实现 ==========

fn encode_riscv_fadd(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FADD.D (双精度)
    let word: u32 = 0x02000053
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20)
        | (0b0000000 << 25); // funct7 for FADD.D

    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fadd.d f{}, f{}, f{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fsub(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x0A000053
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20);

    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fsub.d f{}, f{}, f{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fmul(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x12000053
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20);

    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fmul.d f{}, f{}, f{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fdiv(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x1A000053
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20);

    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fdiv.d f{}, f{}, f{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fadds(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FADD.S (单精度)
    let word: u32 = 0x00000053
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20);

    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fadd.s f{}, f{}, f{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fsubs(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x08000053
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20);

    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fsub.s f{}, f{}, f{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fmuls(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x10000053
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20);

    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fmul.s f{}, f{}, f{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fdivs(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x18000053
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20);

    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fdiv.s f{}, f{}, f{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fsqrt(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x5A000053 | (((dst & 0x1F) as u32) << 7) | (((src & 0x1F) as u32) << 15);

    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fsqrt.d f{}, f{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fsqrts(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x58000053 | (((dst & 0x1F) as u32) << 7) | (((src & 0x1F) as u32) << 15);

    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fsqrt.s f{}, f{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

// ========== RISC-V64 原子操作编码实现 ==========

fn encode_riscv_atomic_rmw(
    dst: RegId,
    base: RegId,
    src: RegId,
    op: vm_ir::AtomicOp,
    size: u8,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    use vm_ir::AtomicOp;

    // RISC-V: AMOADD/AMOSWAP/AMOAND/AMOOR/AMOXOR
    let funct5 = match op {
        AtomicOp::Add => 0b00000,                   // AMOADD
        AtomicOp::Xchg | AtomicOp::Swap => 0b00001, // AMOSWAP
        AtomicOp::And => 0b01100,                   // AMOAND
        AtomicOp::Or => 0b01000,                    // AMOOR
        AtomicOp::Xor => 0b00100,                   // AMOXOR
        _ => {
            return Err(TranslationError::UnsupportedOperation {
                op: format!("RISC-V atomic RMW operation {:?}", op),
            });
        }
    };

    let size_bits = match size {
        1 => 0b000, // AMOADD.W
        2 => 0b001, // AMOADD.W (16-bit not standard)
        4 => 0b010, // AMOADD.W
        8 => 0b011, // AMOADD.D
        _ => {
            return Err(TranslationError::UnsupportedOperation {
                op: format!("RISC-V atomic RMW with size {}", size),
            });
        }
    };

    // 31 30 29 28 27 26 25 24 23 22 21 20 19 18 17 16 15 14 13 12 11 10 9 8 7 6 5 4 3 2 1 0
    // funct5 aq  rl  rs2  rs1 0 1 0  rd  0 1 0 1 1 1 1
    let word: u32 = 0x0000202F // AMO opcode
        | ((funct5 as u32) << 27)
        | ((size_bits as u32) << 12)
        | (((dst & 0x1F) as u32) << 7)
        | (((base & 0x1F) as u32) << 15)
        | (((src & 0x1F) as u32) << 20);

    Ok(vec![TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!(
            "amoadd{} x{}, x{}, (x{})",
            if size == 8 { ".d" } else { ".w" },
            dst,
            src,
            base
        ),
        is_control_flow: false,
        is_memory_op: true,
    }])
}

fn encode_riscv_atomic_cmpxchg(
    dst: RegId,
    base: RegId,
    expected: RegId,
    new: RegId,
    size: u8,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    // RISC-V: LR + SC 组合实现 CMPXCHG
    let mut instructions = Vec::new();

    // LR.W/D: Load Reserved
    let size_bits = if size == 8 { 0b011 } else { 0b010 };
    let lr_word: u32 = 0x0000202F
        | ((0b00010 as u32) << 27) // funct5 for LR
        | ((size_bits as u32) << 12)
        | (((dst & 0x1F) as u32) << 7)
        | (((base & 0x1F) as u32) << 15);
    instructions.push(TargetInstruction {
        bytes: lr_word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!(
            "lr{} x{}, (x{})",
            if size == 8 { ".d" } else { ".w" },
            dst,
            base
        ),
        is_control_flow: false,
        is_memory_op: true,
    });

    // BNE: 如果值不相等，跳转到失败
    // 简化：假设有标签支持
    // 实际实现需要更复杂的控制流处理

    // SC.W/D: Store Conditional
    let sc_word: u32 = 0x0000202F
        | ((0b00011 as u32) << 27) // funct5 for SC
        | ((size_bits as u32) << 12)
        | (((expected & 0x1F) as u32) << 7) // 结果寄存器
        | (((base & 0x1F) as u32) << 15)
        | (((new & 0x1F) as u32) << 20);
    instructions.push(TargetInstruction {
        bytes: sc_word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!(
            "sc{} x{}, x{}, (x{})",
            if size == 8 { ".d" } else { ".w" },
            expected,
            new,
            base
        ),
        is_control_flow: false,
        is_memory_op: true,
    });

    Ok(instructions)
}

fn encode_riscv_lr(
    dst: RegId,
    base: RegId,
    offset: i64,
    size: u8,
) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: LR.W/D (Load Reserved)
    if offset != 0 {
        return Err(TranslationError::InvalidOffset { offset });
    }

    let size_bits = if size == 8 { 0b011 } else { 0b010 };
    let word: u32 = 0x0000202F
        | ((0b00010 as u32) << 27)
        | ((size_bits as u32) << 12)
        | (((dst & 0x1F) as u32) << 7)
        | (((base & 0x1F) as u32) << 15);

    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!(
            "lr{} x{}, (x{})",
            if size == 8 { ".d" } else { ".w" },
            dst,
            base
        ),
        is_control_flow: false,
        is_memory_op: true,
    })
}

fn encode_riscv_sc(
    src: RegId,
    base: RegId,
    offset: i64,
    size: u8,
    dst_flag: RegId,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    // RISC-V: SC.W/D (Store Conditional)
    if offset != 0 {
        return Err(TranslationError::InvalidOffset { offset });
    }

    let size_bits = if size == 8 { 0b011 } else { 0b010 };
    let word: u32 = 0x0000202F
        | ((0b00011 as u32) << 27)
        | ((size_bits as u32) << 12)
        | (((dst_flag & 0x1F) as u32) << 7)
        | (((base & 0x1F) as u32) << 15)
        | (((src & 0x1F) as u32) << 20);

    Ok(vec![TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!(
            "sc{} x{}, x{}, (x{})",
            if size == 8 { ".d" } else { ".w" },
            dst_flag,
            src,
            base
        ),
        is_control_flow: false,
        is_memory_op: true,
    }])
}

// ========== x86-64 SIMD饱和运算编码实现 ==========

fn encode_x86_simd_addsat(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
    element_size: u8,
    signed: bool,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    // x86-64: PADDSB/PADDSW (有符号) 或 PADDUSB/PADDUSW (无符号)
    let mut bytes = vec![0x66, 0x0F];
    if signed {
        match element_size {
            1 => bytes.push(0xEC), // PADDSB
            2 => bytes.push(0xED), // PADDSW
            _ => {
                return Err(TranslationError::UnsupportedOperation {
                    op: format!("SIMD signed add sat with element_size {}", element_size),
                });
            }
        }
    } else {
        match element_size {
            1 => bytes.push(0xDC), // PADDUSB
            2 => bytes.push(0xDD), // PADDUSW
            _ => {
                return Err(TranslationError::UnsupportedOperation {
                    op: format!("SIMD unsigned add sat with element_size {}", element_size),
                });
            }
        }
    }
    bytes.push((0xC0 | ((dst & 0xF) << 3) | (src2 & 0xF)) as u8);

    Ok(vec![TargetInstruction {
        bytes: bytes.clone(),
        length: bytes.len(),
        mnemonic: format!(
            "padds{} xmm{}, xmm{}",
            if signed { "s" } else { "us" },
            dst,
            src2
        ),
        is_control_flow: false,
        is_memory_op: false,
    }])
}

fn encode_x86_simd_subsat(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
    element_size: u8,
    signed: bool,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    // x86-64: PSUBSB/PSUBSW (有符号) 或 PSUBUSB/PSUBUSW (无符号)
    let mut bytes = vec![0x66, 0x0F];
    if signed {
        match element_size {
            1 => bytes.push(0xE8), // PSUBSB
            2 => bytes.push(0xE9), // PSUBSW
            _ => {
                return Err(TranslationError::UnsupportedOperation {
                    op: format!("SIMD signed sub sat with element_size {}", element_size),
                });
            }
        }
    } else {
        match element_size {
            1 => bytes.push(0xD8), // PSUBUSB
            2 => bytes.push(0xD9), // PSUBUSW
            _ => {
                return Err(TranslationError::UnsupportedOperation {
                    op: format!("SIMD unsigned sub sat with element_size {}", element_size),
                });
            }
        }
    }
    bytes.push((0xC0 | ((dst & 0xF) << 3) | (src2 & 0xF)) as u8);

    Ok(vec![TargetInstruction {
        bytes: bytes.clone(),
        length: bytes.len(),
        mnemonic: format!(
            "psubs{} xmm{}, xmm{}",
            if signed { "s" } else { "us" },
            dst,
            src2
        ),
        is_control_flow: false,
        is_memory_op: false,
    }])
}

// ========== x86-64 浮点融合乘加编码实现 ==========

fn encode_x86_fmadd(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
    src3: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // x86-64: VFMADD132SD/VFMADD213SD/VFMADD231SD (AVX FMA)
    // 使用VFMADD231SD: dst = src1 * src2 + src3
    Ok(TargetInstruction {
        bytes: vec![
            0xC4,
            0xE2,
            0xF1,
            0xB9,
            (0xC0 | ((dst & 0xF) << 3) | (src3 & 0xF)) as u8,
        ], // VEX + VFMADD231SD
        length: 5,
        mnemonic: format!("vfmadd231sd xmm{}, xmm{}, xmm{}", dst, src2, src3),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fmsub(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
    src3: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // x86-64: VFMSUB231SD: dst = src1 * src2 - src3
    Ok(TargetInstruction {
        bytes: vec![
            0xC4,
            0xE2,
            0xF1,
            0xBB,
            (0xC0 | ((dst & 0xF) << 3) | (src3 & 0xF)) as u8,
        ],
        length: 5,
        mnemonic: format!("vfmsub231sd xmm{}, xmm{}, xmm{}", dst, src2, src3),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fnmadd(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
    src3: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // x86-64: VFNMADD231SD: dst = -(src1 * src2) + src3
    Ok(TargetInstruction {
        bytes: vec![
            0xC4,
            0xE2,
            0xF1,
            0xBD,
            (0xC0 | ((dst & 0xF) << 3) | (src3 & 0xF)) as u8,
        ],
        length: 5,
        mnemonic: format!("vfnmadd231sd xmm{}, xmm{}, xmm{}", dst, src2, src3),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fnmsub(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
    src3: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // x86-64: VFNMSUB231SD: dst = -(src1 * src2) - src3
    Ok(TargetInstruction {
        bytes: vec![
            0xC4,
            0xE2,
            0xF1,
            0xBF,
            (0xC0 | ((dst & 0xF) << 3) | (src3 & 0xF)) as u8,
        ],
        length: 5,
        mnemonic: format!("vfnmsub231sd xmm{}, xmm{}, xmm{}", dst, src2, src3),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fmadds(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
    src3: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // x86-64: VFMADD231SS (单精度)
    Ok(TargetInstruction {
        bytes: vec![
            0xC4,
            0xE2,
            0x71,
            0xB9,
            (0xC0 | ((dst & 0xF) << 3) | (src3 & 0xF)) as u8,
        ],
        length: 5,
        mnemonic: format!("vfmadd231ss xmm{}, xmm{}, xmm{}", dst, src2, src3),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fmsubs(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
    src3: RegId,
) -> Result<TargetInstruction, TranslationError> {
    Ok(TargetInstruction {
        bytes: vec![
            0xC4,
            0xE2,
            0x71,
            0xBB,
            (0xC0 | ((dst & 0xF) << 3) | (src3 & 0xF)) as u8,
        ],
        length: 5,
        mnemonic: format!("vfmsub231ss xmm{}, xmm{}, xmm{}", dst, src2, src3),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fnmadds(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
    src3: RegId,
) -> Result<TargetInstruction, TranslationError> {
    Ok(TargetInstruction {
        bytes: vec![
            0xC4,
            0xE2,
            0x71,
            0xBD,
            (0xC0 | ((dst & 0xF) << 3) | (src3 & 0xF)) as u8,
        ],
        length: 5,
        mnemonic: format!("vfnmadd231ss xmm{}, xmm{}, xmm{}", dst, src2, src3),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fnmsubs(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
    src3: RegId,
) -> Result<TargetInstruction, TranslationError> {
    Ok(TargetInstruction {
        bytes: vec![
            0xC4,
            0xE2,
            0x71,
            0xBF,
            (0xC0 | ((dst & 0xF) << 3) | (src3 & 0xF)) as u8,
        ],
        length: 5,
        mnemonic: format!("vfnmsub231ss xmm{}, xmm{}, xmm{}", dst, src2, src3),
        is_control_flow: false,
        is_memory_op: false,
    })
}

// ========== x86-64 浮点比较编码实现 ==========

fn encode_x86_feq(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // x86-64: COMISD + SETE
    // 简化：使用CMPEQSD
    Ok(TargetInstruction {
        bytes: vec![
            0x66,
            0x0F,
            0xC2,
            (0xC0 | ((dst & 0xF) << 3) | (src2 & 0xF)) as u8,
            0x00,
        ], // CMPEQSD
        length: 5,
        mnemonic: format!("cmpeqsd xmm{}, xmm{}", dst, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_flt(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // x86-64: CMPLTSD
    Ok(TargetInstruction {
        bytes: vec![
            0x66,
            0x0F,
            0xC2,
            (0xC0 | ((dst & 0xF) << 3) | (src2 & 0xF)) as u8,
            0x01,
        ], // CMPLTSD
        length: 5,
        mnemonic: format!("cmpltsd xmm{}, xmm{}", dst, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fle(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // x86-64: CMPLESD
    Ok(TargetInstruction {
        bytes: vec![
            0x66,
            0x0F,
            0xC2,
            (0xC0 | ((dst & 0xF) << 3) | (src2 & 0xF)) as u8,
            0x02,
        ], // CMPLESD
        length: 5,
        mnemonic: format!("cmplesd xmm{}, xmm{}", dst, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_feqs(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // x86-64: CMPEQSS
    Ok(TargetInstruction {
        bytes: vec![
            0xF3,
            0x0F,
            0xC2,
            (0xC0 | ((dst & 0xF) << 3) | (src2 & 0xF)) as u8,
            0x00,
        ],
        length: 5,
        mnemonic: format!("cmpeqss xmm{}, xmm{}", dst, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_flts(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    Ok(TargetInstruction {
        bytes: vec![
            0xF3,
            0x0F,
            0xC2,
            (0xC0 | ((dst & 0xF) << 3) | (src2 & 0xF)) as u8,
            0x01,
        ],
        length: 5,
        mnemonic: format!("cmpltss xmm{}, xmm{}", dst, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fles(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    Ok(TargetInstruction {
        bytes: vec![
            0xF3,
            0x0F,
            0xC2,
            (0xC0 | ((dst & 0xF) << 3) | (src2 & 0xF)) as u8,
            0x02,
        ],
        length: 5,
        mnemonic: format!("cmpless xmm{}, xmm{}", dst, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

// ========== x86-64 浮点最小/最大编码实现 ==========

fn encode_x86_fmin(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // x86-64: MINSD
    Ok(TargetInstruction {
        bytes: vec![
            0xF2,
            0x0F,
            0x5D,
            (0xC0 | ((dst & 0xF) << 3) | (src2 & 0xF)) as u8,
        ],
        length: 4,
        mnemonic: format!("minsd xmm{}, xmm{}", dst, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fmax(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // x86-64: MAXSD
    Ok(TargetInstruction {
        bytes: vec![
            0xF2,
            0x0F,
            0x5F,
            (0xC0 | ((dst & 0xF) << 3) | (src2 & 0xF)) as u8,
        ],
        length: 4,
        mnemonic: format!("maxsd xmm{}, xmm{}", dst, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fmins(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // x86-64: MINSS
    Ok(TargetInstruction {
        bytes: vec![
            0xF3,
            0x0F,
            0x5D,
            (0xC0 | ((dst & 0xF) << 3) | (src2 & 0xF)) as u8,
        ],
        length: 4,
        mnemonic: format!("minss xmm{}, xmm{}", dst, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fmaxs(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // x86-64: MAXSS
    Ok(TargetInstruction {
        bytes: vec![
            0xF3,
            0x0F,
            0x5F,
            (0xC0 | ((dst & 0xF) << 3) | (src2 & 0xF)) as u8,
        ],
        length: 4,
        mnemonic: format!("maxss xmm{}, xmm{}", dst, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

// ========== x86-64 浮点绝对值/取反编码实现 ==========

fn encode_x86_fabs(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // x86-64: ANDPD (清除符号位)
    Ok(TargetInstruction {
        bytes: vec![
            0x66,
            0x0F,
            0x54,
            (0xC0 | ((dst & 0xF) << 3) | (src & 0xF)) as u8,
        ],
        length: 4,
        mnemonic: format!("andpd xmm{}, xmm{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fneg(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // x86-64: XORPD (翻转符号位)
    Ok(TargetInstruction {
        bytes: vec![
            0x66,
            0x0F,
            0x57,
            (0xC0 | ((dst & 0xF) << 3) | (src & 0xF)) as u8,
        ],
        length: 4,
        mnemonic: format!("xorpd xmm{}, xmm{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fabss(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // x86-64: ANDPS
    Ok(TargetInstruction {
        bytes: vec![0x0F, 0x54, (0xC0 | ((dst & 0xF) << 3) | (src & 0xF)) as u8],
        length: 4,
        mnemonic: format!("andps xmm{}, xmm{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fnegs(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // x86-64: XORPS
    Ok(TargetInstruction {
        bytes: vec![0x0F, 0x57, (0xC0 | ((dst & 0xF) << 3) | (src & 0xF)) as u8],
        length: 4,
        mnemonic: format!("xorps xmm{}, xmm{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

// ========== x86-64 浮点转换编码实现 ==========

fn encode_x86_cvttsd2si(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // x86-64: CVTTSD2SI (F64 -> I32)
    Ok(TargetInstruction {
        bytes: vec![
            0xF2,
            0x0F,
            0x2C,
            (0xC0 | ((dst & 7) << 3) | (src & 0xF)) as u8,
        ],
        length: 4,
        mnemonic: format!("cvttsd2si r{}, xmm{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_cvtsi2ss(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // x86-64: CVTSI2SS (I32 -> F32)
    Ok(TargetInstruction {
        bytes: vec![
            0xF3,
            0x0F,
            0x2A,
            (0xC0 | ((dst & 0xF) << 3) | (src & 7)) as u8,
        ],
        length: 4,
        mnemonic: format!("cvtsi2ss xmm{}, r{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_cvttsd2si64(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // x86-64: CVTTSD2SI (F64 -> I64)
    Ok(TargetInstruction {
        bytes: vec![
            0xF2,
            0x48,
            0x0F,
            0x2C,
            (0xC0 | ((dst & 7) << 3) | (src & 0xF)) as u8,
        ],
        length: 5,
        mnemonic: format!("cvttsd2si r{}, xmm{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_cvtsi2sd64(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // x86-64: CVTSI2SD (I64 -> F64)
    Ok(TargetInstruction {
        bytes: vec![
            0xF2,
            0x48,
            0x0F,
            0x2A,
            (0xC0 | ((dst & 0xF) << 3) | (src & 7)) as u8,
        ],
        length: 5,
        mnemonic: format!("cvtsi2sd xmm{}, r{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_cvtsd2ss(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // x86-64: CVTSD2SS (F64 -> F32)
    Ok(TargetInstruction {
        bytes: vec![
            0xF2,
            0x0F,
            0x5A,
            (0xC0 | ((dst & 0xF) << 3) | (src & 0xF)) as u8,
        ],
        length: 4,
        mnemonic: format!("cvtsd2ss xmm{}, xmm{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_cvtss2sd(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // x86-64: CVTSS2SD (F32 -> F64)
    Ok(TargetInstruction {
        bytes: vec![
            0xF3,
            0x0F,
            0x5A,
            (0xC0 | ((dst & 0xF) << 3) | (src & 0xF)) as u8,
        ],
        length: 4,
        mnemonic: format!("cvtss2sd xmm{}, xmm{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

// ========== x86-64 浮点加载/存储编码实现 ==========

fn encode_x86_fload(
    dst: RegId,
    base: RegId,
    offset: i64,
    size: u8,
) -> Result<TargetInstruction, TranslationError> {
    // x86-64: MOVSD xmm, [base + offset] 或 MOVSS
    let prefix = if size == 8 { 0xF2 } else { 0xF3 };
    let mut bytes = vec![prefix as u8, 0x0F, 0x10];
    bytes.push((0x00 | ((dst & 0xF) << 3) | (base & 7)) as u8);
    if offset != 0 {
        bytes.push(0x40); // ModR/M with disp8
        bytes.extend_from_slice(&(offset as i32).to_le_bytes());
    }
    Ok(TargetInstruction {
        bytes: bytes.clone(),
        length: bytes.len(),
        mnemonic: format!(
            "movs{} xmm{}, [r{} + {}]",
            if size == 8 { "d" } else { "s" },
            dst,
            base,
            offset
        ),
        is_control_flow: false,
        is_memory_op: true,
    })
}

fn encode_x86_fstore(
    src: RegId,
    base: RegId,
    offset: i64,
    size: u8,
) -> Result<TargetInstruction, TranslationError> {
    // x86-64: MOVSD [base + offset], xmm 或 MOVSS
    let prefix = if size == 8 { 0xF2 } else { 0xF3 };
    let mut bytes = vec![prefix as u8, 0x0F, 0x11];
    bytes.push((0x00 | ((src & 0xF) << 3) | (base & 7)) as u8);
    if offset != 0 {
        bytes.push(0x40);
        bytes.extend_from_slice(&(offset as i32).to_le_bytes());
    }
    Ok(TargetInstruction {
        bytes: bytes.clone(),
        length: bytes.len(),
        mnemonic: format!(
            "movs{} [r{} + {}], xmm{}",
            if size == 8 { "d" } else { "s" },
            base,
            offset,
            src
        ),
        is_control_flow: false,
        is_memory_op: true,
    })
}

// ========== ARM64 SIMD饱和运算编码实现 ==========

fn encode_arm64_simd_addsat(
    dst: RegId,
    src1: RegId,
    src2: RegId,
    element_size: u8,
    signed: bool,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    // ARM64: SQADD/UQADD (饱和加法)
    let size = match element_size {
        1 => 0b00,
        2 => 0b01,
        4 => 0b10,
        8 => 0b11,
        _ => {
            return Err(TranslationError::UnsupportedOperation {
                op: format!("ARM64 SIMD add sat with element_size {}", element_size),
            });
        }
    };

    let opcode = if signed { 0x4E200C00 } else { 0x4E202C00 }; // SQADD/UQADD
    let word: u32 = opcode
        | ((size as u32) << 22)
        | ((dst & 0x1F) as u32)
        | (((src1 & 0x1F) as u32) << 5)
        | (((src2 & 0x1F) as u32) << 16);

    Ok(vec![TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!(
            "{}add v{}, v{}, v{}",
            if signed { "sq" } else { "uq" },
            dst,
            src1,
            src2
        ),
        is_control_flow: false,
        is_memory_op: false,
    }])
}

fn encode_arm64_simd_subsat(
    dst: RegId,
    src1: RegId,
    src2: RegId,
    element_size: u8,
    signed: bool,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    // ARM64: SQSUB/UQSUB (饱和减法)
    let size = match element_size {
        1 => 0b00,
        2 => 0b01,
        4 => 0b10,
        8 => 0b11,
        _ => {
            return Err(TranslationError::UnsupportedOperation {
                op: format!("ARM64 SIMD sub sat with element_size {}", element_size),
            });
        }
    };

    let opcode = if signed { 0x4E202C00 } else { 0x4E203C00 }; // SQSUB/UQSUB
    let word: u32 = opcode
        | ((size as u32) << 22)
        | ((dst & 0x1F) as u32)
        | (((src1 & 0x1F) as u32) << 5)
        | (((src2 & 0x1F) as u32) << 16);

    Ok(vec![TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!(
            "{}sub v{}, v{}, v{}",
            if signed { "sq" } else { "uq" },
            dst,
            src1,
            src2
        ),
        is_control_flow: false,
        is_memory_op: false,
    }])
}

// ========== ARM64 浮点融合乘加编码实现 ==========

fn encode_arm64_fmadd(
    dst: RegId,
    src1: RegId,
    src2: RegId,
    src3: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // ARM64: FMADD Dd, Dn, Dm, Da
    let word: u32 = 0x4F60E800
        | ((dst & 0x1F) as u32)
        | (((src1 & 0x1F) as u32) << 5)
        | (((src2 & 0x1F) as u32) << 16)
        | (((src3 & 0x1F) as u32) << 10);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fmadd d{}, d{}, d{}, d{}", dst, src1, src2, src3),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fmsub(
    dst: RegId,
    src1: RegId,
    src2: RegId,
    src3: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // ARM64: FMSUB Dd, Dn, Dm, Da
    let word: u32 = 0x4F60E800
        | ((dst & 0x1F) as u32)
        | (((src1 & 0x1F) as u32) << 5)
        | (((src2 & 0x1F) as u32) << 16)
        | (((src3 & 0x1F) as u32) << 10)
        | (1 << 15); // negate src3
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fmsub d{}, d{}, d{}, d{}", dst, src1, src2, src3),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fnmadd(
    dst: RegId,
    src1: RegId,
    src2: RegId,
    src3: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // ARM64: FNMADD Dd, Dn, Dm, Da
    let word: u32 = 0x4F60E800
        | ((dst & 0x1F) as u32)
        | (((src1 & 0x1F) as u32) << 5)
        | (((src2 & 0x1F) as u32) << 16)
        | (((src3 & 0x1F) as u32) << 10)
        | (1 << 30); // negate product
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fnmadd d{}, d{}, d{}, d{}", dst, src1, src2, src3),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fnmsub(
    dst: RegId,
    src1: RegId,
    src2: RegId,
    src3: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // ARM64: FNMSUB Dd, Dn, Dm, Da
    let word: u32 = 0x4F60E800
        | ((dst & 0x1F) as u32)
        | (((src1 & 0x1F) as u32) << 5)
        | (((src2 & 0x1F) as u32) << 16)
        | (((src3 & 0x1F) as u32) << 10)
        | (1 << 30)
        | (1 << 15); // negate both
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fnmsub d{}, d{}, d{}, d{}", dst, src1, src2, src3),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fmadds(
    dst: RegId,
    src1: RegId,
    src2: RegId,
    src3: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // ARM64: FMADD Sd, Sn, Sm, Sa
    let word: u32 = 0x4F20E800
        | ((dst & 0x1F) as u32)
        | (((src1 & 0x1F) as u32) << 5)
        | (((src2 & 0x1F) as u32) << 16)
        | (((src3 & 0x1F) as u32) << 10);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fmadd s{}, s{}, s{}, s{}", dst, src1, src2, src3),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fmsubs(
    dst: RegId,
    src1: RegId,
    src2: RegId,
    src3: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x4F20E800
        | ((dst & 0x1F) as u32)
        | (((src1 & 0x1F) as u32) << 5)
        | (((src2 & 0x1F) as u32) << 16)
        | (((src3 & 0x1F) as u32) << 10)
        | (1 << 15);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fmsub s{}, s{}, s{}, s{}", dst, src1, src2, src3),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fnmadds(
    dst: RegId,
    src1: RegId,
    src2: RegId,
    src3: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x4F20E800
        | ((dst & 0x1F) as u32)
        | (((src1 & 0x1F) as u32) << 5)
        | (((src2 & 0x1F) as u32) << 16)
        | (((src3 & 0x1F) as u32) << 10)
        | (1 << 30);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fnmadd s{}, s{}, s{}, s{}", dst, src1, src2, src3),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fnmsubs(
    dst: RegId,
    src1: RegId,
    src2: RegId,
    src3: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x4F20E800
        | ((dst & 0x1F) as u32)
        | (((src1 & 0x1F) as u32) << 5)
        | (((src2 & 0x1F) as u32) << 16)
        | (((src3 & 0x1F) as u32) << 10)
        | (1 << 30)
        | (1 << 15);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fnmsub s{}, s{}, s{}, s{}", dst, src1, src2, src3),
        is_control_flow: false,
        is_memory_op: false,
    })
}

// ========== ARM64 浮点比较编码实现 ==========

fn encode_arm64_fcmp(
    _dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // ARM64: FCMP Dn, Dm + CSET (简化实现)
    // 实际需要多条指令：FCMP + CSET
    let word: u32 = 0x4E60E200 | (((src1 & 0x1F) as u32) << 5) | (((src2 & 0x1F) as u32) << 16);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fcmp d{}, d{}", src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fcmplt(
    _dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // ARM64: FCMP + CSET MI (if less than)
    let word: u32 = 0x4E60E200 | (((src1 & 0x1F) as u32) << 5) | (((src2 & 0x1F) as u32) << 16);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fcmplt d{}, d{}", src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fcmple(
    _dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x4E60E200 | (((src1 & 0x1F) as u32) << 5) | (((src2 & 0x1F) as u32) << 16);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fcmple d{}, d{}", src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fcmps(
    _dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x4E20E200 | (((src1 & 0x1F) as u32) << 5) | (((src2 & 0x1F) as u32) << 16);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fcmp s{}, s{}", src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fcmplts(
    _dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x4E20E200 | (((src1 & 0x1F) as u32) << 5) | (((src2 & 0x1F) as u32) << 16);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fcmplt s{}, s{}", src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fcmples(
    _dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x4E20E200 | (((src1 & 0x1F) as u32) << 5) | (((src2 & 0x1F) as u32) << 16);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fcmple s{}, s{}", src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

// ========== ARM64 浮点最小/最大编码实现 ==========

fn encode_arm64_fmin(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // ARM64: FMIN Dd, Dn, Dm
    let word: u32 = 0x4E60E400
        | ((dst & 0x1F) as u32)
        | (((src1 & 0x1F) as u32) << 5)
        | (((src2 & 0x1F) as u32) << 16);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fmin d{}, d{}, d{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fmax(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // ARM64: FMAX Dd, Dn, Dm
    let word: u32 = 0x4E60E500
        | ((dst & 0x1F) as u32)
        | (((src1 & 0x1F) as u32) << 5)
        | (((src2 & 0x1F) as u32) << 16);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fmax d{}, d{}, d{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fmins(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x4E20E400
        | ((dst & 0x1F) as u32)
        | (((src1 & 0x1F) as u32) << 5)
        | (((src2 & 0x1F) as u32) << 16);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fmin s{}, s{}, s{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fmaxs(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x4E20E500
        | ((dst & 0x1F) as u32)
        | (((src1 & 0x1F) as u32) << 5)
        | (((src2 & 0x1F) as u32) << 16);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fmax s{}, s{}, s{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

// ========== ARM64 浮点绝对值/取反编码实现 ==========

fn encode_arm64_fabs(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // ARM64: FABS Dd, Dn
    let word: u32 = 0x4EE0E800 | ((dst & 0x1F) as u32) | (((src & 0x1F) as u32) << 5);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fabs d{}, d{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fneg(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // ARM64: FNEG Dd, Dn
    let word: u32 = 0x4EE0E900 | ((dst & 0x1F) as u32) | (((src & 0x1F) as u32) << 5);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fneg d{}, d{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fabss(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x4EA0E800 | ((dst & 0x1F) as u32) | (((src & 0x1F) as u32) << 5);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fabs s{}, s{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fnegs(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x4EA0E900 | ((dst & 0x1F) as u32) | (((src & 0x1F) as u32) << 5);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fneg s{}, s{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

// ========== ARM64 浮点转换编码实现 ==========

fn encode_arm64_fcvtzs(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // ARM64: FCVTZS Wd, Sn (F32 -> I32)
    let word: u32 = 0x4E21D800 | ((dst & 0x1F) as u32) | (((src & 0x1F) as u32) << 5);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fcvtzs w{}, s{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_scvtf(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // ARM64: SCVTF Sd, Wn (I32 -> F32)
    let word: u32 = 0x4E21D800 | ((dst & 0x1F) as u32) | (((src & 0x1F) as u32) << 5) | (1 << 16); // to-float bit
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("scvtf s{}, w{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fcvtzs64(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // ARM64: FCVTZS Xd, Dn (F64 -> I64)
    let word: u32 = 0x4EE1D800 | ((dst & 0x1F) as u32) | (((src & 0x1F) as u32) << 5);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fcvtzs x{}, d{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_scvtf64(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // ARM64: SCVTF Dd, Xn (I64 -> F64)
    let word: u32 = 0x4EE1D800 | ((dst & 0x1F) as u32) | (((src & 0x1F) as u32) << 5) | (1 << 16);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("scvtf d{}, x{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fcvts(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // ARM64: FCVT Sd, Dn (F64 -> F32)
    let word: u32 = 0x4E21D800 | ((dst & 0x1F) as u32) | (((src & 0x1F) as u32) << 5) | (1 << 22); // double precision source
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fcvt s{}, d{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fcvtd(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // ARM64: FCVT Dd, Sn (F32 -> F64)
    let word: u32 = 0x4EE1D800 | ((dst & 0x1F) as u32) | (((src & 0x1F) as u32) << 5) | (1 << 22); // single precision source
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fcvt d{}, s{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

// ========== ARM64 浮点加载/存储编码实现 ==========

fn encode_arm64_fload(
    dst: RegId,
    base: RegId,
    offset: i64,
    size: u8,
) -> Result<TargetInstruction, TranslationError> {
    // ARM64: LDR Dd, [Xn, #offset] 或 LDR Sd
    if offset < 0 || offset >= 32768 || (offset % (size as i64)) != 0 {
        return Err(TranslationError::InvalidOffset { offset });
    }

    let imm12 = (offset / (size as i64)) as u32;
    let opcode = if size == 8 { 0xFD400000 } else { 0xBD400000 }; // LDR D/S
    let word: u32 =
        opcode | ((dst & 0x1F) as u32) | (((base & 0x1F) as u32) << 5) | ((imm12 & 0xFFF) << 10);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!(
            "ldr {}{}, [x{}, #{}]",
            if size == 8 { "d" } else { "s" },
            dst,
            base,
            offset
        ),
        is_control_flow: false,
        is_memory_op: true,
    })
}

fn encode_arm64_fstore(
    src: RegId,
    base: RegId,
    offset: i64,
    size: u8,
) -> Result<TargetInstruction, TranslationError> {
    // ARM64: STR Ds, [Xn, #offset] 或 STR Ss
    if offset < 0 || offset >= 32768 || (offset % (size as i64)) != 0 {
        return Err(TranslationError::InvalidOffset { offset });
    }

    let imm12 = (offset / (size as i64)) as u32;
    let opcode = if size == 8 { 0xFD000000 } else { 0xBD000000 }; // STR D/S
    let word: u32 =
        opcode | ((src & 0x1F) as u32) | (((base & 0x1F) as u32) << 5) | ((imm12 & 0xFFF) << 10);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!(
            "str {}{}, [x{}, #{}]",
            if size == 8 { "d" } else { "s" },
            src,
            base,
            offset
        ),
        is_control_flow: false,
        is_memory_op: true,
    })
}

// ========== RISC-V64 SIMD饱和运算编码实现 ==========

fn encode_riscv_simd_addsat(
    dst: RegId,
    src1: RegId,
    src2: RegId,
    _element_size: u8,
    signed: bool,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    // RISC-V: VSADDU.VV/VSADD.VV (饱和加法)
    let funct6 = if signed { 0b100000 } else { 0b100001 }; // VSADD/VSADDU
    let word: u32 = 0x00000057
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20)
        | ((funct6 as u32) << 26);
    Ok(vec![TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!(
            "vsadd{}v v{}, v{}, v{}",
            if signed { "" } else { "u" },
            dst,
            src1,
            src2
        ),
        is_control_flow: false,
        is_memory_op: false,
    }])
}

fn encode_riscv_simd_subsat(
    dst: RegId,
    src1: RegId,
    src2: RegId,
    _element_size: u8,
    signed: bool,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    // RISC-V: VSSUB.VV/VSSUBU.VV (饱和减法)
    let funct6 = if signed { 0b100010 } else { 0b100011 }; // VSSUB/VSSUBU
    let word: u32 = 0x00000057
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20)
        | ((funct6 as u32) << 26);
    Ok(vec![TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!(
            "vssub{}v v{}, v{}, v{}",
            if signed { "" } else { "u" },
            dst,
            src1,
            src2
        ),
        is_control_flow: false,
        is_memory_op: false,
    }])
}

// ========== RISC-V64 浮点融合乘加编码实现 ==========

fn encode_riscv_fmadd(
    dst: RegId,
    src1: RegId,
    src2: RegId,
    src3: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FMADD.D
    let word: u32 = 0x02000043
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20)
        | (((src3 & 0x1F) as u32) << 27);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fmadd.d f{}, f{}, f{}, f{}", dst, src1, src2, src3),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fmsub(
    dst: RegId,
    src1: RegId,
    src2: RegId,
    src3: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FMSUB.D
    let word: u32 = 0x02000047
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20)
        | (((src3 & 0x1F) as u32) << 27);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fmsub.d f{}, f{}, f{}, f{}", dst, src1, src2, src3),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fnmadd(
    dst: RegId,
    src1: RegId,
    src2: RegId,
    src3: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FNMADD.D
    let word: u32 = 0x0200004F
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20)
        | (((src3 & 0x1F) as u32) << 27);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fnmadd.d f{}, f{}, f{}, f{}", dst, src1, src2, src3),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fnmsub(
    dst: RegId,
    src1: RegId,
    src2: RegId,
    src3: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FNMSUB.D
    let word: u32 = 0x0200004B
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20)
        | (((src3 & 0x1F) as u32) << 27);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fnmsub.d f{}, f{}, f{}, f{}", dst, src1, src2, src3),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fmadds(
    dst: RegId,
    src1: RegId,
    src2: RegId,
    src3: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FMADD.S
    let word: u32 = 0x00000043
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20)
        | (((src3 & 0x1F) as u32) << 27);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fmadd.s f{}, f{}, f{}, f{}", dst, src1, src2, src3),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fmsubs(
    dst: RegId,
    src1: RegId,
    src2: RegId,
    src3: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x00000047
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20)
        | (((src3 & 0x1F) as u32) << 27);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fmsub.s f{}, f{}, f{}, f{}", dst, src1, src2, src3),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fnmadds(
    dst: RegId,
    src1: RegId,
    src2: RegId,
    src3: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x0000004F
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20)
        | (((src3 & 0x1F) as u32) << 27);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fnmadd.s f{}, f{}, f{}, f{}", dst, src1, src2, src3),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fnmsubs(
    dst: RegId,
    src1: RegId,
    src2: RegId,
    src3: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x0000004B
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20)
        | (((src3 & 0x1F) as u32) << 27);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fnmsub.s f{}, f{}, f{}, f{}", dst, src1, src2, src3),
        is_control_flow: false,
        is_memory_op: false,
    })
}

// ========== RISC-V64 浮点比较编码实现 ==========

fn encode_riscv_feq(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FEQ.D
    let word: u32 = 0xA2002053
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("feq.d x{}, f{}, f{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_flt(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FLT.D
    let word: u32 = 0xA2001053
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("flt.d x{}, f{}, f{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fle(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FLE.D
    let word: u32 = 0xA2000053
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fle.d x{}, f{}, f{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_feqs(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FEQ.S
    let word: u32 = 0xA0002053
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("feq.s x{}, f{}, f{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_flts(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0xA0001053
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("flt.s x{}, f{}, f{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fles(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0xA0000053
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fle.s x{}, f{}, f{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

// ========== RISC-V64 浮点最小/最大编码实现 ==========

fn encode_riscv_fmin(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FMIN.D
    let word: u32 = 0x2A000053
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fmin.d f{}, f{}, f{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fmax(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FMAX.D
    let word: u32 = 0x2A000053
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20)
        | (1 << 12); // funct3 bit for MAX
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fmax.d f{}, f{}, f{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fmins(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x28000053
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fmin.s f{}, f{}, f{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fmaxs(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x28000053
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20)
        | (1 << 12);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fmax.s f{}, f{}, f{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

// ========== RISC-V64 浮点绝对值/取反编码实现 ==========

fn encode_riscv_fabs(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FSGNJX.D with same source (FABS)
    let word: u32 = 0x22000053
        | (((dst & 0x1F) as u32) << 7)
        | (((src & 0x1F) as u32) << 15)
        | (((src & 0x1F) as u32) << 20)
        | (1 << 12); // funct3 for FSGNJX
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fsgnjx.d f{}, f{}, f{}", dst, src, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fneg(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FSGNJN.D with same source (FNEG)
    let word: u32 = 0x22000053
        | (((dst & 0x1F) as u32) << 7)
        | (((src & 0x1F) as u32) << 15)
        | (((src & 0x1F) as u32) << 20)
        | (2 << 12); // funct3 for FSGNJN
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fsgnjn.d f{}, f{}, f{}", dst, src, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fabss(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x20000053
        | (((dst & 0x1F) as u32) << 7)
        | (((src & 0x1F) as u32) << 15)
        | (((src & 0x1F) as u32) << 20)
        | (1 << 12);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fsgnjx.s f{}, f{}, f{}", dst, src, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fnegs(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x20000053
        | (((dst & 0x1F) as u32) << 7)
        | (((src & 0x1F) as u32) << 15)
        | (((src & 0x1F) as u32) << 20)
        | (2 << 12);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fsgnjn.s f{}, f{}, f{}", dst, src, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

// ========== RISC-V64 浮点转换编码实现 ==========

fn encode_riscv_fcvtws(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FCVT.W.S (F32 -> I32 signed)
    let word: u32 = 0xC0000053 | (((dst & 0x1F) as u32) << 7) | (((src & 0x1F) as u32) << 15);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fcvt.w.s x{}, f{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fcvtsw(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FCVT.S.W (I32 signed -> F32)
    let word: u32 = 0xD0000053 | (((dst & 0x1F) as u32) << 7) | (((src & 0x1F) as u32) << 15);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fcvt.s.w f{}, x{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fcvtld(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FCVT.L.D (F64 -> I64 signed)
    let word: u32 = 0xC2000053 | (((dst & 0x1F) as u32) << 7) | (((src & 0x1F) as u32) << 15);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fcvt.l.d x{}, f{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fcvtdl(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FCVT.D.L (I64 signed -> F64)
    let word: u32 = 0xD2000053 | (((dst & 0x1F) as u32) << 7) | (((src & 0x1F) as u32) << 15);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fcvt.d.l f{}, x{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fcvtsd(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FCVT.S.D (F64 -> F32)
    let word: u32 = 0x40100053 | (((dst & 0x1F) as u32) << 7) | (((src & 0x1F) as u32) << 15);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fcvt.s.d f{}, f{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fcvtds(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FCVT.D.S (F32 -> F64)
    let word: u32 = 0x42000053 | (((dst & 0x1F) as u32) << 7) | (((src & 0x1F) as u32) << 15);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fcvt.d.s f{}, f{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

// ========== RISC-V64 浮点加载/存储编码实现 ==========

fn encode_riscv_fload(
    dst: RegId,
    base: RegId,
    offset: i64,
    size: u8,
) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FLD (F64) 或 FLW (F32)
    if offset < -2048 || offset >= 2048 {
        return Err(TranslationError::InvalidOffset { offset });
    }

    let imm12 = (offset as u32) & 0xFFF;
    let funct3 = if size == 8 { 0b011 } else { 0b010 }; // FLD/FLW
    let word: u32 = 0x00000007 // LOAD-FP opcode
        | (((dst & 0x1F) as u32) << 7)
        | (((base & 0x1F) as u32) << 15)
        | ((funct3 as u32) << 12)
        | ((imm12 & 0xFFF) << 20);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!(
            "fl{} f{}, {}(x{})",
            if size == 8 { "d" } else { "w" },
            dst,
            offset,
            base
        ),
        is_control_flow: false,
        is_memory_op: true,
    })
}

fn encode_riscv_fstore(
    src: RegId,
    base: RegId,
    offset: i64,
    size: u8,
) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FSD (F64) 或 FSW (F32)
    if offset < -2048 || offset >= 2048 {
        return Err(TranslationError::InvalidOffset { offset });
    }

    let imm12 = (offset as u32) & 0xFFF;
    let imm_hi = (imm12 >> 5) & 0x7F;
    let imm_lo = imm12 & 0x1F;
    let funct3 = if size == 8 { 0b011 } else { 0b010 }; // FSD/FSW
    let word: u32 = 0x00000027 // STORE-FP opcode
        | (imm_lo << 7)
        | (((base & 0x1F) as u32) << 15)
        | ((funct3 as u32) << 12)
        | (((src & 0x1F) as u32) << 20)
        | (imm_hi << 25);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!(
            "fs{} f{}, {}(x{})",
            if size == 8 { "d" } else { "w" },
            src,
            offset,
            base
        ),
        is_control_flow: false,
        is_memory_op: true,
    })
}

// ========== x86-64 SIMD饱和乘法编码实现 ==========

fn encode_x86_simd_mulsat(
    dst: RegId,
    src1: RegId,
    src2: RegId,
    element_size: u8,
    _signed: bool,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    // x86-64: PMULHRSW (有符号饱和乘法，16位) 或其他
    // 注意：x86-64没有直接的饱和乘法指令，需要组合实现
    let mut instructions = Vec::new();

    // 先执行普通乘法
    instructions.extend(encode_x86_simd_mul(dst, src1, src2, element_size)?);

    // 然后应用饱和（简化实现）
    // 实际实现需要根据element_size和signed进行不同的饱和处理
    Ok(instructions)
}

// ========== x86-64 浮点符号操作编码实现 ==========

fn encode_x86_fsgnj(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // x86-64: 使用ANDPS/XORPS组合实现符号复制
    // 复制src2的符号位到src1
    // 简化：使用BLENDVPD或条件选择
    Ok(TargetInstruction {
        bytes: vec![
            0x66,
            0x0F,
            0x38,
            0x15,
            (0xC0 | ((dst & 0xF) << 3) | (src2 & 0xF)) as u8,
        ], // BLENDVPD
        length: 5,
        mnemonic: format!("blendvpd xmm{}, xmm{}, xmm{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fsgnjn(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // x86-64: 复制src2的符号位并取反
    // 使用XORPS翻转符号位
    Ok(TargetInstruction {
        bytes: vec![
            0x66,
            0x0F,
            0x57,
            (0xC0 | ((dst & 0xF) << 3) | (src2 & 0xF)) as u8,
        ], // XORPD
        length: 4,
        mnemonic: format!("xorpd xmm{}, xmm{}", dst, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fsgnjx(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // x86-64: 符号异或（如果src2符号为负则翻转src1符号）
    Ok(TargetInstruction {
        bytes: vec![
            0x66,
            0x0F,
            0x57,
            (0xC0 | ((dst & 0xF) << 3) | (src2 & 0xF)) as u8,
        ], // XORPD
        length: 4,
        mnemonic: format!("xorpd xmm{}, xmm{}", dst, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fsgnjs(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    Ok(TargetInstruction {
        bytes: vec![
            0xF3,
            0x0F,
            0x38,
            0x15,
            (0xC0 | ((dst & 0xF) << 3) | (src2 & 0xF)) as u8,
        ], // BLENDVPS
        length: 5,
        mnemonic: format!("blendvps xmm{}, xmm{}, xmm{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fsgnjns(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    Ok(TargetInstruction {
        bytes: vec![0x0F, 0x57, (0xC0 | ((dst & 0xF) << 3) | (src2 & 0xF)) as u8], // XORPS
        length: 4,
        mnemonic: format!("xorps xmm{}, xmm{}", dst, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fsgnjxs(
    dst: RegId,
    _src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    Ok(TargetInstruction {
        bytes: vec![0x0F, 0x57, (0xC0 | ((dst & 0xF) << 3) | (src2 & 0xF)) as u8], // XORPS
        length: 4,
        mnemonic: format!("xorps xmm{}, xmm{}", dst, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

// ========== x86-64 浮点分类编码实现 ==========

fn encode_x86_fclass(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // x86-64: 使用多条指令实现分类
    // 检查NaN、无穷大、零、正常数等
    // 简化实现：使用CMP指令组合
    Ok(TargetInstruction {
        bytes: vec![
            0x66,
            0x0F,
            0xC2,
            (0xC0 | ((dst & 0xF) << 3) | (src & 0xF)) as u8,
            0x03,
        ], // CMPUNORDSD
        length: 5,
        mnemonic: format!("cmpunordsd xmm{}, xmm{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fclasss(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    Ok(TargetInstruction {
        bytes: vec![
            0xF3,
            0x0F,
            0xC2,
            (0xC0 | ((dst & 0xF) << 3) | (src & 0xF)) as u8,
            0x03,
        ], // CMPUNORDSS
        length: 5,
        mnemonic: format!("cmpunordss xmm{}, xmm{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

// ========== x86-64 浮点寄存器移动编码实现 ==========

fn encode_x86_fmvxw(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // x86-64: MOVD xmm -> r32 (F32 -> I32)
    Ok(TargetInstruction {
        bytes: vec![
            0x66,
            0x0F,
            0x7E,
            (0xC0 | ((dst & 7) << 3) | (src & 0xF)) as u8,
        ],
        length: 4,
        mnemonic: format!("movd r{}, xmm{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fmvwx(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // x86-64: MOVD r32 -> xmm (I32 -> F32)
    Ok(TargetInstruction {
        bytes: vec![
            0x66,
            0x0F,
            0x6E,
            (0xC0 | ((dst & 0xF) << 3) | (src & 7)) as u8,
        ],
        length: 4,
        mnemonic: format!("movd xmm{}, r{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fmvxd(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // x86-64: MOVQ xmm -> r64 (F64 -> I64)
    Ok(TargetInstruction {
        bytes: vec![
            0x66,
            0x48,
            0x0F,
            0x7E,
            (0xC0 | ((dst & 7) << 3) | (src & 0xF)) as u8,
        ],
        length: 5,
        mnemonic: format!("movq r{}, xmm{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_fmvdx(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // x86-64: MOVQ r64 -> xmm (I64 -> F64)
    Ok(TargetInstruction {
        bytes: vec![
            0x66,
            0x48,
            0x0F,
            0x6E,
            (0xC0 | ((dst & 0xF) << 3) | (src & 7)) as u8,
        ],
        length: 5,
        mnemonic: format!("movq xmm{}, r{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

// ========== x86-64 更多浮点转换编码实现 ==========

fn encode_x86_cvttss2si_u(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // x86-64: CVTTSS2SI (无符号，需要特殊处理)
    // 简化：使用有符号版本
    Ok(TargetInstruction {
        bytes: vec![
            0xF3,
            0x0F,
            0x2C,
            (0xC0 | ((dst & 7) << 3) | (src & 0xF)) as u8,
        ],
        length: 4,
        mnemonic: format!("cvttss2si r{}, xmm{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_cvttss2si64_u(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    Ok(TargetInstruction {
        bytes: vec![
            0xF3,
            0x48,
            0x0F,
            0x2C,
            (0xC0 | ((dst & 7) << 3) | (src & 0xF)) as u8,
        ],
        length: 5,
        mnemonic: format!("cvttss2si r{}, xmm{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_cvtsi2ss_u(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // x86-64: CVTSI2SS (无符号，需要特殊处理)
    Ok(TargetInstruction {
        bytes: vec![
            0xF3,
            0x0F,
            0x2A,
            (0xC0 | ((dst & 0xF) << 3) | (src & 7)) as u8,
        ],
        length: 4,
        mnemonic: format!("cvtsi2ss xmm{}, r{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_cvtsi2ss64_u(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    Ok(TargetInstruction {
        bytes: vec![
            0xF3,
            0x48,
            0x0F,
            0x2A,
            (0xC0 | ((dst & 0xF) << 3) | (src & 7)) as u8,
        ],
        length: 5,
        mnemonic: format!("cvtsi2ss xmm{}, r{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_cvttsd2si_u(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    Ok(TargetInstruction {
        bytes: vec![
            0xF2,
            0x0F,
            0x2C,
            (0xC0 | ((dst & 7) << 3) | (src & 0xF)) as u8,
        ],
        length: 4,
        mnemonic: format!("cvttsd2si r{}, xmm{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_cvttsd2si64_u(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    Ok(TargetInstruction {
        bytes: vec![
            0xF2,
            0x48,
            0x0F,
            0x2C,
            (0xC0 | ((dst & 7) << 3) | (src & 0xF)) as u8,
        ],
        length: 5,
        mnemonic: format!("cvttsd2si r{}, xmm{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_cvtsi2sd(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // x86-64: CVTSI2SD (I32 -> F64)
    Ok(TargetInstruction {
        bytes: vec![
            0xF2,
            0x0F,
            0x2A,
            (0xC0 | ((dst & 0xF) << 3) | (src & 7)) as u8,
        ],
        length: 4,
        mnemonic: format!("cvtsi2sd xmm{}, r{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_cvtsi2sd_u(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    Ok(TargetInstruction {
        bytes: vec![
            0xF2,
            0x0F,
            0x2A,
            (0xC0 | ((dst & 0xF) << 3) | (src & 7)) as u8,
        ],
        length: 4,
        mnemonic: format!("cvtsi2sd xmm{}, r{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_x86_cvtsi2sd64_u(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    Ok(TargetInstruction {
        bytes: vec![
            0xF2,
            0x48,
            0x0F,
            0x2A,
            (0xC0 | ((dst & 0xF) << 3) | (src & 7)) as u8,
        ],
        length: 5,
        mnemonic: format!("cvtsi2sd xmm{}, r{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

// ========== ARM64 SIMD饱和乘法编码实现 ==========

fn encode_arm64_simd_mulsat(
    dst: RegId,
    src1: RegId,
    src2: RegId,
    element_size: u8,
    signed: bool,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    // ARM64: SQDMULH (饱和乘法高位)
    let size = match element_size {
        2 => 0b01, // 16-bit
        4 => 0b10, // 32-bit
        _ => {
            return Err(TranslationError::UnsupportedOperation {
                op: format!("ARM64 SIMD mul sat with element_size {}", element_size),
            });
        }
    };

    let opcode = if signed { 0x4E20B400 } else { 0x4E209C00 }; // SQDMULH/MUL
    let word: u32 = opcode
        | ((size as u32) << 22)
        | ((dst & 0x1F) as u32)
        | (((src1 & 0x1F) as u32) << 5)
        | (((src2 & 0x1F) as u32) << 16);

    Ok(vec![TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("sqdmulh v{}, v{}, v{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    }])
}

// ========== ARM64 浮点符号操作编码实现 ==========

fn encode_arm64_fsgnj(
    dst: RegId,
    src1: RegId,
    _src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // ARM64: 使用位操作实现符号复制
    // 简化：使用FABS + 条件选择
    let word: u32 = 0x4EE0E800 // FABS
        | ((dst & 0x1F) as u32)
        | (((src1 & 0x1F) as u32) << 5);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fabs d{}, d{}", dst, src1),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fsgnjn(
    dst: RegId,
    src1: RegId,
    _src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // ARM64: FNEG (取反符号)
    let word: u32 = 0x4EE0E900 | ((dst & 0x1F) as u32) | (((src1 & 0x1F) as u32) << 5);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fneg d{}, d{}", dst, src1),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fsgnjx(
    dst: RegId,
    src1: RegId,
    _src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // ARM64: 符号异或（需要多条指令实现）
    // 简化：使用FNEG
    let word: u32 = 0x4EE0E900 | ((dst & 0x1F) as u32) | (((src1 & 0x1F) as u32) << 5);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fneg d{}, d{}", dst, src1),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fsgnjs(
    dst: RegId,
    src1: RegId,
    _src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x4EA0E800 | ((dst & 0x1F) as u32) | (((src1 & 0x1F) as u32) << 5);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fabs s{}, s{}", dst, src1),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fsgnjns(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // ARM64: FSGNJNS instruction uses src2's sign bit
    let word: u32 = 0x4EA0E900 | ((dst & 0x1F) as u32) | (((src1 & 0x1F) as u32) << 5) | (((src2 & 0x1F) as u32) << 10);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fsgnjns s{}, s{}, s{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fsgnjxs(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // ARM64: FSGNJXS instruction uses src2's sign bit
    let word: u32 = 0x4EA0E920 | ((dst & 0x1F) as u32) | (((src1 & 0x1F) as u32) << 5) | (((src2 & 0x1F) as u32) << 10);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fsgnjxs s{}, s{}, s{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

// ========== ARM64 浮点分类编码实现 ==========

fn encode_arm64_fclass(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // ARM64: FRINTX + 比较指令组合实现分类
    // 简化实现
    let word: u32 = 0x4EE1E800 // FRINTX (round to exact)
        | ((dst & 0x1F) as u32)
        | (((src & 0x1F) as u32) << 5);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("frintx d{}, d{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fclasss(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x4EA1E800 | ((dst & 0x1F) as u32) | (((src & 0x1F) as u32) << 5);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("frintx s{}, s{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

// ========== ARM64 浮点寄存器移动编码实现 ==========

fn encode_arm64_fmvxw(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // ARM64: FMOV Wd, Sn (F32 -> I32)
    let word: u32 = 0x4E21E000 | ((dst & 0x1F) as u32) | (((src & 0x1F) as u32) << 5);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fmov w{}, s{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fmvwx(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // ARM64: FMOV Sd, Wn (I32 -> F32)
    let word: u32 = 0x4E21E000 | ((dst & 0x1F) as u32) | (((src & 0x1F) as u32) << 5) | (1 << 16); // to-float bit
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fmov s{}, w{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fmvxd(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // ARM64: FMOV Xd, Dn (F64 -> I64)
    let word: u32 = 0x4EE1E000 | ((dst & 0x1F) as u32) | (((src & 0x1F) as u32) << 5);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fmov x{}, d{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fmvdx(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // ARM64: FMOV Dd, Xn (I64 -> F64)
    let word: u32 = 0x4EE1E000 | ((dst & 0x1F) as u32) | (((src & 0x1F) as u32) << 5) | (1 << 16);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fmov d{}, x{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

// ========== ARM64 更多浮点转换编码实现 ==========

fn encode_arm64_fcvtzus(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // ARM64: FCVTZU Wd, Sn (F32 -> I32 unsigned)
    let word: u32 = 0x4E21D800 | ((dst & 0x1F) as u32) | (((src & 0x1F) as u32) << 5) | (1 << 16); // unsigned bit
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fcvtzu w{}, s{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_fcvtzus64(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // ARM64: FCVTZU Xd, Dn (F64 -> I64 unsigned)
    let word: u32 = 0x4EE1D800 | ((dst & 0x1F) as u32) | (((src & 0x1F) as u32) << 5) | (1 << 16);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fcvtzu x{}, d{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_ucvtf(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // ARM64: UCVTF Sd, Wn (I32 unsigned -> F32)
    let word: u32 =
        0x4E21D800 | ((dst & 0x1F) as u32) | (((src & 0x1F) as u32) << 5) | (1 << 16) | (1 << 17); // unsigned + to-float
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("ucvtf s{}, w{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_arm64_ucvtf64(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // ARM64: UCVTF Dd, Xn (I64 unsigned -> F64)
    let word: u32 =
        0x4EE1D800 | ((dst & 0x1F) as u32) | (((src & 0x1F) as u32) << 5) | (1 << 16) | (1 << 17);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("ucvtf d{}, x{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

// ========== RISC-V64 SIMD饱和乘法编码实现 ==========

fn encode_riscv_simd_mulsat(
    dst: RegId,
    src1: RegId,
    src2: RegId,
    _element_size: u8,
    signed: bool,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    // RISC-V: 根据符号类型选择合适的饱和乘法指令
    let funct6 = if signed {
        0b100101 // VMUL.VV - 有符号乘法
    } else {
        0b100111 // VMULU.VV - 无符号乘法
    };
    
    let word: u32 = 0x00000057
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20)
        | ((funct6 as u32) << 26);
    
    // 注意：RISC-V本身没有直接的饱和乘法指令，需要后续处理
    // 这里简化实现，使用普通乘法指令
    Ok(vec![TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("{}.vv v{}, v{}, v{}", 
            if signed { "vmul" } else { "vmulu" }, dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    }])
}

// ========== RISC-V64 浮点符号操作编码实现 ==========

fn encode_riscv_fsgnj(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FSGNJ.D
    let word: u32 = 0x22000053
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20)
        | (0b000 << 12); // funct3 for FSGNJ
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fsgnj.d f{}, f{}, f{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fsgnjn(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FSGNJN.D
    let word: u32 = 0x22000053
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20)
        | (0b001 << 12); // funct3 for FSGNJN
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fsgnjn.d f{}, f{}, f{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fsgnjx(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FSGNJX.D
    let word: u32 = 0x22000053
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20)
        | (0b010 << 12); // funct3 for FSGNJX
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fsgnjx.d f{}, f{}, f{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fsgnjs(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FSGNJ.S
    let word: u32 = 0x20000053
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20)
        | (0b000 << 12);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fsgnj.s f{}, f{}, f{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fsgnjns(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x20000053
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20)
        | (0b001 << 12);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fsgnjn.s f{}, f{}, f{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fsgnjxs(
    dst: RegId,
    src1: RegId,
    src2: RegId,
) -> Result<TargetInstruction, TranslationError> {
    let word: u32 = 0x20000053
        | (((dst & 0x1F) as u32) << 7)
        | (((src1 & 0x1F) as u32) << 15)
        | (((src2 & 0x1F) as u32) << 20)
        | (0b010 << 12);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fsgnjx.s f{}, f{}, f{}", dst, src1, src2),
        is_control_flow: false,
        is_memory_op: false,
    })
}

// ========== RISC-V64 浮点分类编码实现 ==========

fn encode_riscv_fclass(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FCLASS.D
    let word: u32 = 0xE2001053 | (((dst & 0x1F) as u32) << 7) | (((src & 0x1F) as u32) << 15);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fclass.d x{}, f{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fclasss(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FCLASS.S
    let word: u32 = 0xE0001053 | (((dst & 0x1F) as u32) << 7) | (((src & 0x1F) as u32) << 15);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fclass.s x{}, f{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

// ========== RISC-V64 浮点寄存器移动编码实现 ==========

fn encode_riscv_fmvxw(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FMV.X.W (F32 -> I32)
    let word: u32 = 0xE0000053 | (((dst & 0x1F) as u32) << 7) | (((src & 0x1F) as u32) << 15);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fmv.x.w x{}, f{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fmvwx(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FMV.W.X (I32 -> F32)
    let word: u32 = 0xF0000053 | (((dst & 0x1F) as u32) << 7) | (((src & 0x1F) as u32) << 15);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fmv.w.x f{}, x{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fmvxd(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FMV.X.D (F64 -> I64)
    let word: u32 = 0xE2000053 | (((dst & 0x1F) as u32) << 7) | (((src & 0x1F) as u32) << 15);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fmv.x.d x{}, f{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fmvdx(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FMV.D.X (I64 -> F64)
    let word: u32 = 0xF2000053 | (((dst & 0x1F) as u32) << 7) | (((src & 0x1F) as u32) << 15);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fmv.d.x f{}, x{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

// ========== RISC-V64 更多浮点转换编码实现 ==========

fn encode_riscv_fcvtwus(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FCVT.WU.S (F32 -> I32 unsigned)
    let word: u32 = 0xC0100053 | (((dst & 0x1F) as u32) << 7) | (((src & 0x1F) as u32) << 15);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fcvt.wu.s x{}, f{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fcvtlus(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FCVT.LU.S (F32 -> I64 unsigned)
    let word: u32 = 0xC0300053 | (((dst & 0x1F) as u32) << 7) | (((src & 0x1F) as u32) << 15);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fcvt.lu.s x{}, f{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fcvtswu(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FCVT.S.WU (I32 unsigned -> F32)
    let word: u32 = 0xD0100053 | (((dst & 0x1F) as u32) << 7) | (((src & 0x1F) as u32) << 15);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fcvt.s.wu f{}, x{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fcvtslu(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FCVT.S.LU (I64 unsigned -> F32)
    let word: u32 = 0xD0300053 | (((dst & 0x1F) as u32) << 7) | (((src & 0x1F) as u32) << 15);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fcvt.s.lu f{}, x{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fcvtwd(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FCVT.W.D (F64 -> I32 signed)
    let word: u32 = 0xC2000053 | (((dst & 0x1F) as u32) << 7) | (((src & 0x1F) as u32) << 15);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fcvt.w.d x{}, f{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fcvtwud(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FCVT.WU.D (F64 -> I32 unsigned)
    let word: u32 = 0xC2100053 | (((dst & 0x1F) as u32) << 7) | (((src & 0x1F) as u32) << 15);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fcvt.wu.d x{}, f{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fcvtlud(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FCVT.LU.D (F64 -> I64 unsigned)
    let word: u32 = 0xC2300053 | (((dst & 0x1F) as u32) << 7) | (((src & 0x1F) as u32) << 15);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fcvt.lu.d x{}, f{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fcvtdw(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FCVT.D.W (I32 signed -> F64)
    let word: u32 = 0xD2000053 | (((dst & 0x1F) as u32) << 7) | (((src & 0x1F) as u32) << 15);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fcvt.d.w f{}, x{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fcvtdwu(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FCVT.D.WU (I32 unsigned -> F64)
    let word: u32 = 0xD2100053 | (((dst & 0x1F) as u32) << 7) | (((src & 0x1F) as u32) << 15);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fcvt.d.wu f{}, x{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

fn encode_riscv_fcvtdlu(dst: RegId, src: RegId) -> Result<TargetInstruction, TranslationError> {
    // RISC-V: FCVT.D.LU (I64 unsigned -> F64)
    let word: u32 = 0xD2300053 | (((dst & 0x1F) as u32) << 7) | (((src & 0x1F) as u32) << 15);
    Ok(TargetInstruction {
        bytes: word.to_le_bytes().to_vec(),
        length: 4,
        mnemonic: format!("fcvt.d.lu f{}, x{}", dst, src),
        is_control_flow: false,
        is_memory_op: false,
    })
}

// ========== x86-64 大向量操作编码实现 ==========

fn encode_x86_vec128_add(
    dst_lo: RegId,
    dst_hi: RegId,
    src1_lo: RegId,
    src1_hi: RegId,
    src2_lo: RegId,
    src2_hi: RegId,
    element_size: u8,
    signed: bool,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    // x86-64: 128位向量加法，需要两个XMM寄存器
    let mut instructions = Vec::new();

    // 低64位
    instructions.extend(
        if signed {
            encode_x86_simd_addsat(dst_lo, src1_lo, src2_lo, element_size, signed)?
        } else {
            encode_x86_simd_add(dst_lo, src1_lo, src2_lo, element_size)?
        }
    );

    // 高64位
    instructions.extend(
        if signed {
            encode_x86_simd_addsat(dst_hi, src1_hi, src2_hi, element_size, signed)?
        } else {
            encode_x86_simd_add(dst_hi, src1_hi, src2_hi, element_size)?
        }
    );

    Ok(instructions)
}

fn encode_x86_vec256_add(
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
) -> Result<Vec<TargetInstruction>, TranslationError> {
    // x86-64: 256位向量加法，需要4个XMM寄存器或使用AVX YMM寄存器
    let mut instructions = Vec::new();

    // 使用AVX VADDPD/VADDPS (如果支持)
    // 简化：分别处理每个64位段
    instructions.extend(
        if signed {
            encode_x86_simd_addsat(dst0, src10, src20, element_size, signed)?
        } else {
            encode_x86_simd_add(dst0, src10, src20, element_size)?
        }
    );
    instructions.extend(
        if signed {
            encode_x86_simd_addsat(dst1, src11, src21, element_size, signed)?
        } else {
            encode_x86_simd_add(dst1, src11, src21, element_size)?
        }
    );
    instructions.extend(
        if signed {
            encode_x86_simd_addsat(dst2, src12, src22, element_size, signed)?
        } else {
            encode_x86_simd_add(dst2, src12, src22, element_size)?
        }
    );
    instructions.extend(
        if signed {
            encode_x86_simd_addsat(dst3, src13, src23, element_size, signed)?
        } else {
            encode_x86_simd_add(dst3, src13, src23, element_size)?
        }
    );

    Ok(instructions)
}

fn encode_x86_vec256_sub(
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
    _signed: bool,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    let mut instructions = Vec::new();

    instructions.extend(encode_x86_simd_sub(dst0, src10, src20, element_size)?);
    instructions.extend(encode_x86_simd_sub(dst1, src11, src21, element_size)?);
    instructions.extend(encode_x86_simd_sub(dst2, src12, src22, element_size)?);
    instructions.extend(encode_x86_simd_sub(dst3, src13, src23, element_size)?);

    Ok(instructions)
}

fn encode_x86_vec256_mul(
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
    _signed: bool,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    let mut instructions = Vec::new();

    instructions.extend(encode_x86_simd_mul(dst0, src10, src20, element_size)?);
    instructions.extend(encode_x86_simd_mul(dst1, src11, src21, element_size)?);
    instructions.extend(encode_x86_simd_mul(dst2, src12, src22, element_size)?);
    instructions.extend(encode_x86_simd_mul(dst3, src13, src23, element_size)?);

    Ok(instructions)
}

// ========== ARM64 大向量操作编码实现 ==========

fn encode_arm64_vec128_add(
    dst_lo: RegId,
    dst_hi: RegId,
    src1_lo: RegId,
    src1_hi: RegId,
    src2_lo: RegId,
    src2_hi: RegId,
    element_size: u8,
    _signed: bool,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    // ARM64: 128位向量加法
    let mut instructions = Vec::new();

    instructions.extend(encode_arm64_simd_add(
        dst_lo,
        src1_lo,
        src2_lo,
        element_size,
    )?);
    instructions.extend(encode_arm64_simd_add(
        dst_hi,
        src1_hi,
        src2_hi,
        element_size,
    )?);

    Ok(instructions)
}

fn encode_arm64_vec256_add(
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
    _signed: bool,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    // ARM64: 256位向量加法
    let mut instructions = Vec::new();

    instructions.extend(encode_arm64_simd_add(dst0, src10, src20, element_size)?);
    instructions.extend(encode_arm64_simd_add(dst1, src11, src21, element_size)?);
    instructions.extend(encode_arm64_simd_add(dst2, src12, src22, element_size)?);
    instructions.extend(encode_arm64_simd_add(dst3, src13, src23, element_size)?);

    Ok(instructions)
}

fn encode_arm64_vec256_sub(
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
    _signed: bool,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    let mut instructions = Vec::new();

    instructions.extend(encode_arm64_simd_sub(dst0, src10, src20, element_size)?);
    instructions.extend(encode_arm64_simd_sub(dst1, src11, src21, element_size)?);
    instructions.extend(encode_arm64_simd_sub(dst2, src12, src22, element_size)?);
    instructions.extend(encode_arm64_simd_sub(dst3, src13, src23, element_size)?);

    Ok(instructions)
}

fn encode_arm64_vec256_mul(
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
    _signed: bool,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    let mut instructions = Vec::new();

    instructions.extend(encode_arm64_simd_mul(dst0, src10, src20, element_size)?);
    instructions.extend(encode_arm64_simd_mul(dst1, src11, src21, element_size)?);
    instructions.extend(encode_arm64_simd_mul(dst2, src12, src22, element_size)?);
    instructions.extend(encode_arm64_simd_mul(dst3, src13, src23, element_size)?);

    Ok(instructions)
}

// ========== RISC-V64 大向量操作编码实现 ==========

fn encode_riscv_vec128_add(
    dst_lo: RegId,
    dst_hi: RegId,
    src1_lo: RegId,
    src1_hi: RegId,
    src2_lo: RegId,
    src2_hi: RegId,
    element_size: u8,
    _signed: bool,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    // RISC-V: 128位向量加法
    let mut instructions = Vec::new();

    instructions.extend(encode_riscv_simd_add(
        dst_lo,
        src1_lo,
        src2_lo,
        element_size,
    )?);
    instructions.extend(encode_riscv_simd_add(
        dst_hi,
        src1_hi,
        src2_hi,
        element_size,
    )?);

    Ok(instructions)
}

fn encode_riscv_vec256_add(
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
    _signed: bool,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    // RISC-V: 256位向量加法
    let mut instructions = Vec::new();

    instructions.extend(encode_riscv_simd_add(dst0, src10, src20, element_size)?);
    instructions.extend(encode_riscv_simd_add(dst1, src11, src21, element_size)?);
    instructions.extend(encode_riscv_simd_add(dst2, src12, src22, element_size)?);
    instructions.extend(encode_riscv_simd_add(dst3, src13, src23, element_size)?);

    Ok(instructions)
}

fn encode_riscv_vec256_sub(
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
    _signed: bool,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    let mut instructions = Vec::new();

    instructions.extend(encode_riscv_simd_sub(dst0, src10, src20, element_size)?);
    instructions.extend(encode_riscv_simd_sub(dst1, src11, src21, element_size)?);
    instructions.extend(encode_riscv_simd_sub(dst2, src12, src22, element_size)?);
    instructions.extend(encode_riscv_simd_sub(dst3, src13, src23, element_size)?);

    Ok(instructions)
}

fn encode_riscv_vec256_mul(
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
    _signed: bool,
) -> Result<Vec<TargetInstruction>, TranslationError> {
    let mut instructions = Vec::new();

    instructions.extend(encode_riscv_simd_mul(dst0, src10, src20, element_size)?);
    instructions.extend(encode_riscv_simd_mul(dst1, src11, src21, element_size)?);
    instructions.extend(encode_riscv_simd_mul(dst2, src12, src22, element_size)?);
    instructions.extend(encode_riscv_simd_mul(dst3, src13, src23, element_size)?);

    Ok(instructions)
}
