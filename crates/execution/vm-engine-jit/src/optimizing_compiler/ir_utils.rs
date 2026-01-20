//! IR工具函数
//!
//! 提供IR分析和操作的实用函数

use vm_ir::{IROp, RegId};

/// IR分析器
pub struct IrAnalyzer;

impl IrAnalyzer {
    /// 收集操作中读取的寄存器
    pub fn collect_read_regs(op: &IROp) -> Vec<RegId> {
        match op {
            IROp::Add { src1, src2, .. } => vec![*src1, *src2],
            IROp::Sub { src1, src2, .. } => vec![*src1, *src2],
            IROp::Mul { src1, src2, .. } => vec![*src1, *src2],
            IROp::Div { src1, src2, .. } => vec![*src1, *src2],
            IROp::Rem { src1, src2, .. } => vec![*src1, *src2],
            IROp::And { src1, src2, .. } => vec![*src1, *src2],
            IROp::Or { src1, src2, .. } => vec![*src1, *src2],
            IROp::Xor { src1, src2, .. } => vec![*src1, *src2],
            IROp::Not { src, .. } => vec![*src],
            IROp::Sll { src, shreg, .. } => vec![*src, *shreg],
            IROp::Srl { src, shreg, .. } => vec![*src, *shreg],
            IROp::Sra { src, shreg, .. } => vec![*src, *shreg],
            IROp::AddImm { src, .. } => vec![*src],
            IROp::MulImm { src, .. } => vec![*src],
            IROp::SllImm { src, .. } => vec![*src],
            IROp::SrlImm { src, .. } => vec![*src],
            IROp::SraImm { src, .. } => vec![*src],
            IROp::CmpEq { lhs, rhs, .. } => vec![*lhs, *rhs],
            IROp::CmpNe { lhs, rhs, .. } => vec![*lhs, *rhs],
            IROp::CmpLt { lhs, rhs, .. } => vec![*lhs, *rhs],
            IROp::CmpLtU { lhs, rhs, .. } => vec![*lhs, *rhs],
            IROp::CmpGe { lhs, rhs, .. } => vec![*lhs, *rhs],
            IROp::CmpGeU { lhs, rhs, .. } => vec![*lhs, *rhs],
            IROp::Select {
                cond,
                true_val,
                false_val,
                ..
            } => vec![*cond, *true_val, *false_val],
            IROp::Load { base, .. } => vec![*base],
            IROp::Store { src, base, .. } => vec![*src, *base],
            IROp::AtomicRMW { base, src, .. } => vec![*base, *src],
            IROp::AtomicRMWOrder { base, src, .. } => vec![*base, *src],
            IROp::AtomicCmpXchg {
                base,
                expected,
                new,
                ..
            } => vec![*base, *expected, *new],
            IROp::AtomicCmpXchgOrder {
                base,
                expected,
                new,
                ..
            } => vec![*base, *expected, *new],
            IROp::AtomicLoadReserve { base, .. } => vec![*base],
            IROp::AtomicStoreCond {
                src,
                base,
                dst_flag,
                ..
            } => vec![*src, *base, *dst_flag],
            IROp::AtomicCmpXchgFlag {
                base,
                expected,
                new,
                ..
            } => vec![*base, *expected, *new],
            IROp::AtomicRmwFlag { base, src, .. } => vec![*base, *src],
            IROp::VecAdd { src1, src2, .. } => vec![*src1, *src2],
            IROp::VecSub { src1, src2, .. } => vec![*src1, *src2],
            IROp::VecMul { src1, src2, .. } => vec![*src1, *src2],
            IROp::VecAddSat { src1, src2, .. } => vec![*src1, *src2],
            IROp::VecSubSat { src1, src2, .. } => vec![*src1, *src2],
            IROp::VecMulSat { src1, src2, .. } => vec![*src1, *src2],
            IROp::Vec128Add {
                src1_lo,
                src1_hi,
                src2_lo,
                src2_hi,
                ..
            } => vec![*src1_lo, *src1_hi, *src2_lo, *src2_hi],
            // 注意: Vec128Sub, Vec128Mul 等其他 Vec128 变体尚未在 vm-ir 中定义
            IROp::Vec256Add {
                src10,
                src11,
                src12,
                src13,
                src20,
                src21,
                src22,
                src23,
                ..
            } => vec![
                *src10, *src11, *src12, *src13, *src20, *src21, *src22, *src23,
            ],
            IROp::Vec256Sub {
                src10,
                src11,
                src12,
                src13,
                src20,
                src21,
                src22,
                src23,
                ..
            } => vec![
                *src10, *src11, *src12, *src13, *src20, *src21, *src22, *src23,
            ],
            IROp::Vec256Mul {
                src10,
                src11,
                src12,
                src13,
                src20,
                src21,
                src22,
                src23,
                ..
            } => vec![
                *src10, *src11, *src12, *src13, *src20, *src21, *src22, *src23,
            ],
            IROp::Fadd { src1, src2, .. } => vec![*src1, *src2],
            IROp::Fsub { src1, src2, .. } => vec![*src1, *src2],
            IROp::Fmul { src1, src2, .. } => vec![*src1, *src2],
            IROp::Fdiv { src1, src2, .. } => vec![*src1, *src2],
            IROp::Fsqrt { src, .. } => vec![*src],
            IROp::Fmin { src1, src2, .. } => vec![*src1, *src2],
            IROp::Fmax { src1, src2, .. } => vec![*src1, *src2],
            IROp::FaddS { src1, src2, .. } => vec![*src1, *src2],
            IROp::FsubS { src1, src2, .. } => vec![*src1, *src2],
            IROp::FmulS { src1, src2, .. } => vec![*src1, *src2],
            IROp::FdivS { src1, src2, .. } => vec![*src1, *src2],
            IROp::FsqrtS { src, .. } => vec![*src],
            IROp::FminS { src1, src2, .. } => vec![*src1, *src2],
            IROp::FmaxS { src1, src2, .. } => vec![*src1, *src2],
            IROp::Fmadd {
                src1, src2, src3, ..
            } => vec![*src1, *src2, *src3],
            IROp::Fmsub {
                src1, src2, src3, ..
            } => vec![*src1, *src2, *src3],
            IROp::Fnmadd {
                src1, src2, src3, ..
            } => vec![*src1, *src2, *src3],
            IROp::Fnmsub {
                src1, src2, src3, ..
            } => vec![*src1, *src2, *src3],
            IROp::FmaddS {
                src1, src2, src3, ..
            } => vec![*src1, *src2, *src3],
            IROp::FmsubS {
                src1, src2, src3, ..
            } => vec![*src1, *src2, *src3],
            IROp::FnmaddS {
                src1, src2, src3, ..
            } => vec![*src1, *src2, *src3],
            IROp::FnmsubS {
                src1, src2, src3, ..
            } => vec![*src1, *src2, *src3],
            IROp::Feq { src1, src2, .. } => vec![*src1, *src2],
            IROp::Flt { src1, src2, .. } => vec![*src1, *src2],
            IROp::Fle { src1, src2, .. } => vec![*src1, *src2],
            IROp::FeqS { src1, src2, .. } => vec![*src1, *src2],
            IROp::FltS { src1, src2, .. } => vec![*src1, *src2],
            IROp::FleS { src1, src2, .. } => vec![*src1, *src2],
            IROp::Fcvtws { src, .. } => vec![*src],
            IROp::Fcvtwus { src, .. } => vec![*src],
            IROp::Fcvtls { src, .. } => vec![*src],
            IROp::Fcvtlus { src, .. } => vec![*src],
            IROp::Fcvtsw { src, .. } => vec![*src],
            IROp::Fcvtswu { src, .. } => vec![*src],
            IROp::Fcvtsl { src, .. } => vec![*src],
            IROp::Fcvtslu { src, .. } => vec![*src],
            IROp::Fcvtwd { src, .. } => vec![*src],
            IROp::Fcvtwud { src, .. } => vec![*src],
            IROp::Fcvtld { src, .. } => vec![*src],
            IROp::Fcvtlud { src, .. } => vec![*src],
            IROp::Fcvtdw { src, .. } => vec![*src],
            IROp::Fcvtdwu { src, .. } => vec![*src],
            IROp::Fcvtdl { src, .. } => vec![*src],
            IROp::Fcvtdlu { src, .. } => vec![*src],
            IROp::Fcvtsd { src, .. } => vec![*src],
            IROp::Fcvtds { src, .. } => vec![*src],
            IROp::Fsgnj { src1, src2, .. } => vec![*src1, *src2],
            IROp::Fsgnjn { src1, src2, .. } => vec![*src1, *src2],
            IROp::Fsgnjx { src1, src2, .. } => vec![*src1, *src2],
            IROp::FsgnjS { src1, src2, .. } => vec![*src1, *src2],
            IROp::FsgnjnS { src1, src2, .. } => vec![*src1, *src2],
            IROp::FsgnjxS { src1, src2, .. } => vec![*src1, *src2],
            IROp::Fclass { src, .. } => vec![*src],
            IROp::FclassS { src, .. } => vec![*src],
            IROp::FmvXW { src, .. } => vec![*src],
            IROp::FmvWX { src, .. } => vec![*src],
            IROp::FmvXD { src, .. } => vec![*src],
            IROp::FmvDX { src, .. } => vec![*src],
            IROp::Fabs { src, .. } => vec![*src],
            IROp::Fneg { src, .. } => vec![*src],
            IROp::FabsS { src, .. } => vec![*src],
            IROp::FnegS { src, .. } => vec![*src],
            IROp::Fload { base, .. } => vec![*base],
            IROp::Fstore { src, base, .. } => vec![*src, *base],
            IROp::Beq { src1, src2, .. } => vec![*src1, *src2],
            IROp::Bne { src1, src2, .. } => vec![*src1, *src2],
            IROp::Blt { src1, src2, .. } => vec![*src1, *src2],
            IROp::Bge { src1, src2, .. } => vec![*src1, *src2],
            IROp::Bltu { src1, src2, .. } => vec![*src1, *src2],
            IROp::Bgeu { src1, src2, .. } => vec![*src1, *src2],
            IROp::Atomic { base, src, .. } => vec![*base, *src],
            IROp::Cpuid { leaf, subleaf, .. } => vec![*leaf, *subleaf],
            IROp::CsrRead { .. } => vec![],
            IROp::CsrWrite { src, .. } => vec![*src],
            IROp::CsrSet { src, .. } => vec![*src],
            IROp::CsrClear { src, .. } => vec![*src],
            IROp::CsrWriteImm { .. } => vec![],
            IROp::CsrSetImm { .. } => vec![],
            IROp::CsrClearImm { .. } => vec![],
            IROp::ReadPstateFlags { .. } => vec![],
            IROp::WritePstateFlags { src, .. } => vec![*src],
            IROp::EvalCondition { cond, .. } => vec![(*cond).into()],
            IROp::VendorLoad { base, .. } => vec![*base],
            IROp::VendorStore { src, base, .. } => vec![*src, *base],
            IROp::VendorMatrixOp { .. } => vec![],
            IROp::VendorConfig { .. } => vec![],
            IROp::VendorVectorOp { src1, src2, .. } => vec![*src1, *src2],
            _ => vec![],
        }
    }

    /// 收集操作中写入的寄存器
    pub fn collect_written_regs(op: &IROp) -> Vec<RegId> {
        match op {
            IROp::MovImm { dst, .. } => vec![*dst],
            IROp::Add { dst, .. } => vec![*dst],
            IROp::Sub { dst, .. } => vec![*dst],
            IROp::Mul { dst, .. } => vec![*dst],
            IROp::Div { dst, .. } => vec![*dst],
            IROp::Rem { dst, .. } => vec![*dst],
            IROp::And { dst, .. } => vec![*dst],
            IROp::Or { dst, .. } => vec![*dst],
            IROp::Xor { dst, .. } => vec![*dst],
            IROp::Not { dst, .. } => vec![*dst],
            IROp::Sll { dst, .. } => vec![*dst],
            IROp::Srl { dst, .. } => vec![*dst],
            IROp::Sra { dst, .. } => vec![*dst],
            IROp::AddImm { dst, .. } => vec![*dst],
            IROp::MulImm { dst, .. } => vec![*dst],
            IROp::SllImm { dst, .. } => vec![*dst],
            IROp::SrlImm { dst, .. } => vec![*dst],
            IROp::SraImm { dst, .. } => vec![*dst],
            IROp::CmpEq { dst, .. } => vec![*dst],
            IROp::CmpNe { dst, .. } => vec![*dst],
            IROp::CmpLt { dst, .. } => vec![*dst],
            IROp::CmpLtU { dst, .. } => vec![*dst],
            IROp::CmpGe { dst, .. } => vec![*dst],
            IROp::CmpGeU { dst, .. } => vec![*dst],
            IROp::Select { dst, .. } => vec![*dst],
            IROp::Load { dst, .. } => vec![*dst],
            IROp::Store { .. } => vec![], // Store 不写入寄存器
            IROp::AtomicRMW { dst, .. } => vec![*dst],
            IROp::AtomicRMWOrder { dst, .. } => vec![*dst],
            IROp::AtomicCmpXchg { dst, .. } => vec![*dst],
            IROp::AtomicCmpXchgOrder { dst, .. } => vec![*dst],
            IROp::AtomicLoadReserve { dst, .. } => vec![*dst],
            IROp::AtomicStoreCond { dst_flag, .. } => vec![*dst_flag],
            IROp::AtomicCmpXchgFlag {
                dst_old, dst_flag, ..
            } => vec![*dst_old, *dst_flag],
            IROp::AtomicRmwFlag {
                dst_old, dst_flag, ..
            } => vec![*dst_old, *dst_flag],
            IROp::VecAdd { dst, .. } => vec![*dst],
            IROp::VecSub { dst, .. } => vec![*dst],
            IROp::VecMul { dst, .. } => vec![*dst],
            IROp::VecAddSat { dst, .. } => vec![*dst],
            IROp::VecSubSat { dst, .. } => vec![*dst],
            IROp::VecMulSat { dst, .. } => vec![*dst],
            IROp::Vec128Add { dst_lo, dst_hi, .. } => vec![*dst_lo, *dst_hi],
            // 注意: Vec128Sub, Vec128Mul 等其他 Vec128 变体尚未在 vm-ir 中定义
            IROp::Vec256Add {
                dst0,
                dst1,
                dst2,
                dst3,
                ..
            } => vec![*dst0, *dst1, *dst2, *dst3],
            IROp::Vec256Sub {
                dst0,
                dst1,
                dst2,
                dst3,
                ..
            } => vec![*dst0, *dst1, *dst2, *dst3],
            IROp::Vec256Mul {
                dst0,
                dst1,
                dst2,
                dst3,
                ..
            } => vec![*dst0, *dst1, *dst2, *dst3],
            IROp::Fadd { dst, .. } => vec![*dst],
            IROp::Fsub { dst, .. } => vec![*dst],
            IROp::Fmul { dst, .. } => vec![*dst],
            IROp::Fdiv { dst, .. } => vec![*dst],
            IROp::Fsqrt { dst, .. } => vec![*dst],
            IROp::Fmin { dst, .. } => vec![*dst],
            IROp::Fmax { dst, .. } => vec![*dst],
            IROp::FaddS { dst, .. } => vec![*dst],
            IROp::FsubS { dst, .. } => vec![*dst],
            IROp::FmulS { dst, .. } => vec![*dst],
            IROp::FdivS { dst, .. } => vec![*dst],
            IROp::FsqrtS { dst, .. } => vec![*dst],
            IROp::FminS { dst, .. } => vec![*dst],
            IROp::FmaxS { dst, .. } => vec![*dst],
            IROp::Fmadd { dst, .. } => vec![*dst],
            IROp::Fmsub { dst, .. } => vec![*dst],
            IROp::Fnmadd { dst, .. } => vec![*dst],
            IROp::Fnmsub { dst, .. } => vec![*dst],
            IROp::FmaddS { dst, .. } => vec![*dst],
            IROp::FmsubS { dst, .. } => vec![*dst],
            IROp::FnmaddS { dst, .. } => vec![*dst],
            IROp::FnmsubS { dst, .. } => vec![*dst],
            IROp::Feq { dst, .. } => vec![*dst],
            IROp::Flt { dst, .. } => vec![*dst],
            IROp::Fle { dst, .. } => vec![*dst],
            IROp::FeqS { dst, .. } => vec![*dst],
            IROp::FltS { dst, .. } => vec![*dst],
            IROp::FleS { dst, .. } => vec![*dst],
            IROp::Fcvtws { dst, .. } => vec![*dst],
            IROp::Fcvtwus { dst, .. } => vec![*dst],
            IROp::Fcvtls { dst, .. } => vec![*dst],
            IROp::Fcvtlus { dst, .. } => vec![*dst],
            IROp::Fcvtsw { dst, .. } => vec![*dst],
            IROp::Fcvtswu { dst, .. } => vec![*dst],
            IROp::Fcvtsl { dst, .. } => vec![*dst],
            IROp::Fcvtslu { dst, .. } => vec![*dst],
            IROp::Fcvtwd { dst, .. } => vec![*dst],
            IROp::Fcvtwud { dst, .. } => vec![*dst],
            IROp::Fcvtld { dst, .. } => vec![*dst],
            IROp::Fcvtlud { dst, .. } => vec![*dst],
            IROp::Fcvtdw { dst, .. } => vec![*dst],
            IROp::Fcvtdwu { dst, .. } => vec![*dst],
            IROp::Fcvtdl { dst, .. } => vec![*dst],
            IROp::Fcvtdlu { dst, .. } => vec![*dst],
            IROp::Fcvtsd { dst, .. } => vec![*dst],
            IROp::Fcvtds { dst, .. } => vec![*dst],
            IROp::Fsgnj { dst, .. } => vec![*dst],
            IROp::Fsgnjn { dst, .. } => vec![*dst],
            IROp::Fsgnjx { dst, .. } => vec![*dst],
            IROp::FsgnjS { dst, .. } => vec![*dst],
            IROp::FsgnjnS { dst, .. } => vec![*dst],
            IROp::FsgnjxS { dst, .. } => vec![*dst],
            IROp::Fclass { dst, .. } => vec![*dst],
            IROp::FclassS { dst, .. } => vec![*dst],
            IROp::FmvXW { dst, .. } => vec![*dst],
            IROp::FmvWX { dst, .. } => vec![*dst],
            IROp::FmvXD { dst, .. } => vec![*dst],
            IROp::FmvDX { dst, .. } => vec![*dst],
            IROp::Fabs { dst, .. } => vec![*dst],
            IROp::Fneg { dst, .. } => vec![*dst],
            IROp::FabsS { dst, .. } => vec![*dst],
            IROp::FnegS { dst, .. } => vec![*dst],
            IROp::Fload { dst, .. } => vec![*dst],
            IROp::Atomic { dst, .. } => vec![*dst],
            IROp::Cpuid {
                dst_eax,
                dst_ebx,
                dst_ecx,
                dst_edx,
                ..
            } => vec![*dst_eax, *dst_ebx, *dst_ecx, *dst_edx],
            IROp::CsrRead { dst, .. } => vec![*dst],
            IROp::CsrWriteImm { dst, .. } => vec![*dst],
            IROp::CsrSetImm { dst, .. } => vec![*dst],
            IROp::CsrClearImm { dst, .. } => vec![*dst],
            IROp::ReadPstateFlags { dst, .. } => vec![*dst],
            IROp::EvalCondition { dst, .. } => vec![*dst],
            IROp::VendorLoad { dst, .. } => vec![*dst],
            IROp::VendorMatrixOp { dst, .. } => vec![*dst],
            IROp::VendorVectorOp { dst, .. } => vec![*dst],
            _ => vec![],
        }
    }

    /// 检查操作是否是纯函数（无副作用）
    pub fn is_pure(op: &IROp) -> bool {
        match op {
            IROp::Add { .. }
            | IROp::Sub { .. }
            | IROp::Mul { .. }
            | IROp::Div { .. }
            | IROp::Rem { .. } => true,
            IROp::And { .. } | IROp::Or { .. } | IROp::Xor { .. } | IROp::Not { .. } => true,
            IROp::Sll { .. } | IROp::Srl { .. } | IROp::Sra { .. } => true,
            IROp::AddImm { .. } | IROp::MulImm { .. } => true,
            IROp::SllImm { .. } | IROp::SrlImm { .. } | IROp::SraImm { .. } => true,
            IROp::CmpEq { .. } | IROp::CmpNe { .. } | IROp::CmpLt { .. } | IROp::CmpLtU { .. } => {
                true
            }
            IROp::CmpGe { .. } | IROp::CmpGeU { .. } => true,
            IROp::Select { .. } => true,
            IROp::MovImm { .. } => true,
            IROp::Load { .. } => false,  // 内存读取可能有副作用
            IROp::Store { .. } => false, // 内存写入有副作用
            IROp::AtomicRMW { .. } | IROp::AtomicRMWOrder { .. } => false, // 原子操作有副作用
            IROp::AtomicCmpXchg { .. } | IROp::AtomicCmpXchgOrder { .. } => false, // 原子操作有副作用
            IROp::AtomicLoadReserve { .. } | IROp::AtomicStoreCond { .. } => false, // 原子操作有副作用
            IROp::AtomicCmpXchgFlag { .. } | IROp::AtomicRmwFlag { .. } => false, // 原子操作有副作用
            IROp::VecAdd { .. } | IROp::VecSub { .. } | IROp::VecMul { .. } => true,
            IROp::VecAddSat { .. } | IROp::VecSubSat { .. } | IROp::VecMulSat { .. } => true,
            IROp::Vec128Add { .. } => true,
            IROp::Vec256Add { .. } | IROp::Vec256Sub { .. } | IROp::Vec256Mul { .. } => true,
            IROp::Fadd { .. } | IROp::Fsub { .. } | IROp::Fmul { .. } | IROp::Fdiv { .. } => true,
            IROp::Fsqrt { .. } | IROp::Fmin { .. } | IROp::Fmax { .. } => true,
            IROp::FaddS { .. } | IROp::FsubS { .. } | IROp::FmulS { .. } | IROp::FdivS { .. } => {
                true
            }
            IROp::FsqrtS { .. } | IROp::FminS { .. } | IROp::FmaxS { .. } => true,
            IROp::Fmadd { .. } | IROp::Fmsub { .. } | IROp::Fnmadd { .. } | IROp::Fnmsub { .. } => {
                true
            }
            IROp::FmaddS { .. }
            | IROp::FmsubS { .. }
            | IROp::FnmaddS { .. }
            | IROp::FnmsubS { .. } => true,
            IROp::Feq { .. } | IROp::Flt { .. } | IROp::Fle { .. } => true,
            IROp::FeqS { .. } | IROp::FltS { .. } | IROp::FleS { .. } => true,
            IROp::Fcvtws { .. }
            | IROp::Fcvtwus { .. }
            | IROp::Fcvtls { .. }
            | IROp::Fcvtlus { .. } => true,
            IROp::Fcvtsw { .. }
            | IROp::Fcvtswu { .. }
            | IROp::Fcvtsl { .. }
            | IROp::Fcvtslu { .. } => true,
            IROp::Fcvtwd { .. }
            | IROp::Fcvtwud { .. }
            | IROp::Fcvtld { .. }
            | IROp::Fcvtlud { .. } => true,
            IROp::Fcvtdw { .. }
            | IROp::Fcvtdwu { .. }
            | IROp::Fcvtdl { .. }
            | IROp::Fcvtdlu { .. } => true,
            IROp::Fcvtsd { .. } | IROp::Fcvtds { .. } => true,
            IROp::Fsgnj { .. } | IROp::Fsgnjn { .. } | IROp::Fsgnjx { .. } => true,
            IROp::FsgnjS { .. } | IROp::FsgnjnS { .. } | IROp::FsgnjxS { .. } => true,
            IROp::Fclass { .. } | IROp::FclassS { .. } => true,
            IROp::FmvXW { .. } | IROp::FmvWX { .. } | IROp::FmvXD { .. } | IROp::FmvDX { .. } => {
                true
            }
            IROp::Fabs { .. } | IROp::Fneg { .. } | IROp::FabsS { .. } | IROp::FnegS { .. } => true,
            IROp::Fload { .. } => false,  // 内存读取可能有副作用
            IROp::Fstore { .. } => false, // 内存写入有副作用
            IROp::Beq { .. } | IROp::Bne { .. } | IROp::Blt { .. } | IROp::Bge { .. } => false, // 控制流改变
            IROp::Bltu { .. } | IROp::Bgeu { .. } => false, // 控制流改变
            IROp::Atomic { .. } => false,                   // 原子操作有副作用
            IROp::SysCall | IROp::DebugBreak => false,      // 系统调用有副作用
            IROp::Cpuid { .. } => false,                    // CPUID可能读取硬件状态
            IROp::TlbFlush { .. } => false,                 // TLB刷新有副作用
            IROp::CsrRead { .. } => false,                  // CSR读取可能有副作用
            IROp::CsrWrite { .. } | IROp::CsrSet { .. } | IROp::CsrClear { .. } => false, // CSR写入有副作用
            IROp::CsrWriteImm { .. } | IROp::CsrSetImm { .. } | IROp::CsrClearImm { .. } => false, // CSR写入有副作用
            IROp::SysMret | IROp::SysSret | IROp::SysWfi => false, // 系统指令有副作用
            IROp::ReadPstateFlags { .. } => false,                 // 读取状态可能有副作用
            IROp::WritePstateFlags { .. } => false,                // 写入状态有副作用
            IROp::EvalCondition { .. } => true,
            IROp::VendorLoad { .. } => false,  // 厂商特定内存操作
            IROp::VendorStore { .. } => false, // 厂商特定内存操作
            IROp::VendorMatrixOp { .. } => false, // 厂商特定操作
            IROp::VendorConfig { .. } => false, // 厂商特定配置
            IROp::VendorVectorOp { .. } => false, // 厂商特定向量操作
            _ => false,
        }
    }

    /// 检查操作是否是算术运算
    pub fn is_arithmetic(op: &IROp) -> bool {
        matches!(
            op,
            IROp::Add { .. } | IROp::Sub { .. } | IROp::Mul { .. } | IROp::Div { .. }
        )
    }

    /// 检查操作是否是内存访问
    pub fn is_memory_access(op: &IROp) -> bool {
        matches!(op, IROp::Load { .. } | IROp::Store { .. })
    }

    /// 检查操作是否是控制流指令
    pub fn is_control_flow(op: &IROp) -> bool {
        matches!(
            op,
            IROp::Beq { .. }
                | IROp::Bne { .. }
                | IROp::Blt { .. }
                | IROp::Bge { .. }
                | IROp::Bltu { .. }
                | IROp::Bgeu { .. }
                | IROp::SysCall
                | IROp::DebugBreak
                | IROp::TlbFlush { .. }
                | IROp::SysMret
                | IROp::SysSret
                | IROp::SysWfi
        )
    }

    /// 获取操作的复杂度评分（用于调度）
    pub fn get_complexity_score(op: &IROp) -> u32 {
        match op {
            IROp::MovImm { .. } => 1,
            IROp::Add { .. } | IROp::Sub { .. } => 3,
            IROp::Mul { .. } => 4,
            IROp::Div { .. } | IROp::Rem { .. } => 5, // 除法和取模通常更慢
            IROp::And { .. } | IROp::Or { .. } | IROp::Xor { .. } | IROp::Not { .. } => 2,
            IROp::Sll { .. } | IROp::Srl { .. } | IROp::Sra { .. } => 2,
            IROp::AddImm { .. } | IROp::MulImm { .. } => 2,
            IROp::SllImm { .. } | IROp::SrlImm { .. } | IROp::SraImm { .. } => 2,
            IROp::CmpEq { .. } | IROp::CmpNe { .. } | IROp::CmpLt { .. } | IROp::CmpLtU { .. } => 2,
            IROp::CmpGe { .. } | IROp::CmpGeU { .. } => 2,
            IROp::Select { .. } => 3,
            IROp::Load { .. } => 3,  // 内存访问
            IROp::Store { .. } => 3, // 内存访问
            IROp::AtomicRMW { .. } | IROp::AtomicRMWOrder { .. } => 8, // 原子操作复杂
            IROp::AtomicCmpXchg { .. } | IROp::AtomicCmpXchgOrder { .. } => 8, // 原子操作复杂
            IROp::AtomicLoadReserve { .. } | IROp::AtomicStoreCond { .. } => 8, // 原子操作复杂
            IROp::AtomicCmpXchgFlag { .. } | IROp::AtomicRmwFlag { .. } => 8, // 原子操作复杂
            IROp::VecAdd { .. } | IROp::VecSub { .. } | IROp::VecMul { .. } => 4,
            IROp::VecAddSat { .. } | IROp::VecSubSat { .. } | IROp::VecMulSat { .. } => 5,
            IROp::Vec128Add { .. } => 6,
            IROp::Vec256Add { .. } | IROp::Vec256Sub { .. } | IROp::Vec256Mul { .. } => 8,
            IROp::Fadd { .. } | IROp::Fsub { .. } | IROp::Fmul { .. } | IROp::Fdiv { .. } => 4,
            IROp::Fsqrt { .. } | IROp::Fmin { .. } | IROp::Fmax { .. } => 5,
            IROp::FaddS { .. } | IROp::FsubS { .. } | IROp::FmulS { .. } | IROp::FdivS { .. } => 4,
            IROp::FsqrtS { .. } | IROp::FminS { .. } | IROp::FmaxS { .. } => 5,
            IROp::Fmadd { .. } | IROp::Fmsub { .. } | IROp::Fnmadd { .. } | IROp::Fnmsub { .. } => {
                6
            }
            IROp::FmaddS { .. }
            | IROp::FmsubS { .. }
            | IROp::FnmaddS { .. }
            | IROp::FnmsubS { .. } => 6,
            IROp::Feq { .. } | IROp::Flt { .. } | IROp::Fle { .. } => 3,
            IROp::FeqS { .. } | IROp::FltS { .. } | IROp::FleS { .. } => 3,
            IROp::Fcvtws { .. }
            | IROp::Fcvtwus { .. }
            | IROp::Fcvtls { .. }
            | IROp::Fcvtlus { .. } => 5,
            IROp::Fcvtsw { .. }
            | IROp::Fcvtswu { .. }
            | IROp::Fcvtsl { .. }
            | IROp::Fcvtslu { .. } => 5,
            IROp::Fcvtwd { .. }
            | IROp::Fcvtwud { .. }
            | IROp::Fcvtld { .. }
            | IROp::Fcvtlud { .. } => 5,
            IROp::Fcvtdw { .. }
            | IROp::Fcvtdwu { .. }
            | IROp::Fcvtdl { .. }
            | IROp::Fcvtdlu { .. } => 5,
            IROp::Fcvtsd { .. } | IROp::Fcvtds { .. } => 5,
            IROp::Fsgnj { .. } | IROp::Fsgnjn { .. } | IROp::Fsgnjx { .. } => 2,
            IROp::FsgnjS { .. } | IROp::FsgnjnS { .. } | IROp::FsgnjxS { .. } => 2,
            IROp::Fclass { .. } | IROp::FclassS { .. } => 3,
            IROp::FmvXW { .. } | IROp::FmvWX { .. } | IROp::FmvXD { .. } | IROp::FmvDX { .. } => 2,
            IROp::Fabs { .. } | IROp::Fneg { .. } | IROp::FabsS { .. } | IROp::FnegS { .. } => 2,
            IROp::Fload { .. } => 3,  // 内存访问
            IROp::Fstore { .. } => 3, // 内存访问
            IROp::Beq { .. } | IROp::Bne { .. } | IROp::Blt { .. } | IROp::Bge { .. } => 2,
            IROp::Bltu { .. } | IROp::Bgeu { .. } => 2,
            IROp::Atomic { .. } => 8, // 原子操作复杂
            IROp::SysCall => 10,      // 系统调用最复杂
            IROp::DebugBreak => 5,
            IROp::Cpuid { .. } => 6,
            IROp::TlbFlush { .. } => 7,
            IROp::CsrRead { .. } => 4,
            IROp::CsrWrite { .. } | IROp::CsrSet { .. } | IROp::CsrClear { .. } => 4,
            IROp::CsrWriteImm { .. } | IROp::CsrSetImm { .. } | IROp::CsrClearImm { .. } => 4,
            IROp::SysMret | IROp::SysSret | IROp::SysWfi => 8,
            IROp::ReadPstateFlags { .. } => 3,
            IROp::WritePstateFlags { .. } => 3,
            IROp::EvalCondition { .. } => 2,
            IROp::VendorLoad { .. } => 6,
            IROp::VendorStore { .. } => 6,
            IROp::VendorMatrixOp { .. } => 10,
            IROp::VendorConfig { .. } => 5,
            IROp::VendorVectorOp { .. } => 7,
            _ => 3,
        }
    }

    /// 获取操作的延迟周期数（用于调度）
    pub fn get_latency_cycles(op: &IROp) -> u32 {
        match op {
            IROp::MovImm { .. } => 1,
            IROp::Add { .. } | IROp::Sub { .. } => 2,
            IROp::Mul { .. } => 3,
            IROp::Div { .. } | IROp::Rem { .. } => 10, // 除法和取模延迟高
            IROp::And { .. } | IROp::Or { .. } | IROp::Xor { .. } | IROp::Not { .. } => 1,
            IROp::Sll { .. } | IROp::Srl { .. } | IROp::Sra { .. } => 1,
            IROp::AddImm { .. } | IROp::MulImm { .. } => 1,
            IROp::SllImm { .. } | IROp::SrlImm { .. } | IROp::SraImm { .. } => 1,
            IROp::CmpEq { .. } | IROp::CmpNe { .. } | IROp::CmpLt { .. } | IROp::CmpLtU { .. } => 1,
            IROp::CmpGe { .. } | IROp::CmpGeU { .. } => 1,
            IROp::Select { .. } => 2,
            IROp::Load { .. } => 3,  // 内存加载延迟
            IROp::Store { .. } => 2, // 内存存储延迟
            IROp::AtomicRMW { .. } | IROp::AtomicRMWOrder { .. } => 15, // 原子操作延迟高
            IROp::AtomicCmpXchg { .. } | IROp::AtomicCmpXchgOrder { .. } => 15, // 原子操作延迟高
            IROp::AtomicLoadReserve { .. } | IROp::AtomicStoreCond { .. } => 15, // 原子操作延迟高
            IROp::AtomicCmpXchgFlag { .. } | IROp::AtomicRmwFlag { .. } => 15, // 原子操作延迟高
            IROp::VecAdd { .. } | IROp::VecSub { .. } | IROp::VecMul { .. } => 4,
            IROp::VecAddSat { .. } | IROp::VecSubSat { .. } | IROp::VecMulSat { .. } => 5,
            IROp::Vec128Add { .. } => 6,
            IROp::Vec256Add { .. } | IROp::Vec256Sub { .. } | IROp::Vec256Mul { .. } => 8,
            IROp::Fadd { .. } | IROp::Fsub { .. } | IROp::Fmul { .. } | IROp::Fdiv { .. } => 4,
            IROp::Fsqrt { .. } | IROp::Fmin { .. } | IROp::Fmax { .. } => 6,
            IROp::FaddS { .. } | IROp::FsubS { .. } | IROp::FmulS { .. } | IROp::FdivS { .. } => 4,
            IROp::FsqrtS { .. } | IROp::FminS { .. } | IROp::FmaxS { .. } => 6,
            IROp::Fmadd { .. } | IROp::Fmsub { .. } | IROp::Fnmadd { .. } | IROp::Fnmsub { .. } => {
                7
            }
            IROp::FmaddS { .. }
            | IROp::FmsubS { .. }
            | IROp::FnmaddS { .. }
            | IROp::FnmsubS { .. } => 7,
            IROp::Feq { .. } | IROp::Flt { .. } | IROp::Fle { .. } => 3,
            IROp::FeqS { .. } | IROp::FltS { .. } | IROp::FleS { .. } => 3,
            IROp::Fcvtws { .. }
            | IROp::Fcvtwus { .. }
            | IROp::Fcvtls { .. }
            | IROp::Fcvtlus { .. } => 8,
            IROp::Fcvtsw { .. }
            | IROp::Fcvtswu { .. }
            | IROp::Fcvtsl { .. }
            | IROp::Fcvtslu { .. } => 8,
            IROp::Fcvtwd { .. }
            | IROp::Fcvtwud { .. }
            | IROp::Fcvtld { .. }
            | IROp::Fcvtlud { .. } => 8,
            IROp::Fcvtdw { .. }
            | IROp::Fcvtdwu { .. }
            | IROp::Fcvtdl { .. }
            | IROp::Fcvtdlu { .. } => 8,
            IROp::Fcvtsd { .. } | IROp::Fcvtds { .. } => 8,
            IROp::Fsgnj { .. } | IROp::Fsgnjn { .. } | IROp::Fsgnjx { .. } => 1,
            IROp::FsgnjS { .. } | IROp::FsgnjnS { .. } | IROp::FsgnjxS { .. } => 1,
            IROp::Fclass { .. } | IROp::FclassS { .. } => 3,
            IROp::FmvXW { .. } | IROp::FmvWX { .. } | IROp::FmvXD { .. } | IROp::FmvDX { .. } => 1,
            IROp::Fabs { .. } | IROp::Fneg { .. } | IROp::FabsS { .. } | IROp::FnegS { .. } => 1,
            IROp::Fload { .. } => 3,  // 内存加载延迟
            IROp::Fstore { .. } => 2, // 内存存储延迟
            IROp::Beq { .. } | IROp::Bne { .. } | IROp::Blt { .. } | IROp::Bge { .. } => 2,
            IROp::Bltu { .. } | IROp::Bgeu { .. } => 2,
            IROp::Atomic { .. } => 15, // 原子操作延迟高
            IROp::SysCall => 20,       // 系统调用延迟很高
            IROp::DebugBreak => 5,
            IROp::Cpuid { .. } => 10,
            IROp::TlbFlush { .. } => 12,
            IROp::CsrRead { .. } => 4,
            IROp::CsrWrite { .. } | IROp::CsrSet { .. } | IROp::CsrClear { .. } => 4,
            IROp::CsrWriteImm { .. } | IROp::CsrSetImm { .. } | IROp::CsrClearImm { .. } => 4,
            IROp::SysMret | IROp::SysSret | IROp::SysWfi => 10,
            IROp::ReadPstateFlags { .. } => 3,
            IROp::WritePstateFlags { .. } => 3,
            IROp::EvalCondition { .. } => 1,
            IROp::VendorLoad { .. } => 8,
            IROp::VendorStore { .. } => 8,
            IROp::VendorMatrixOp { .. } => 20,
            IROp::VendorConfig { .. } => 6,
            IROp::VendorVectorOp { .. } => 12,
            _ => 2,
        }
    }

    /// 检查两个操作是否可以交换（用于调度优化）
    pub fn can_swap(op1: &IROp, op2: &IROp) -> bool {
        // 如果两个操作都是纯函数且没有数据依赖，则可以交换
        if !Self::is_pure(op1) || !Self::is_pure(op2) {
            return false;
        }

        // 检查数据依赖
        let written1 = Self::collect_written_regs(op1);
        let read2 = Self::collect_read_regs(op2);

        // 如果op1写入的寄存器被op2读取，则不能交换
        for &reg1 in &written1 {
            if read2.contains(&reg1) {
                return false;
            }
        }

        let written2 = Self::collect_written_regs(op2);
        let read1 = Self::collect_read_regs(op1);

        // 如果op2写入的寄存器被op1读取，则不能交换
        for &reg2 in &written2 {
            if read1.contains(&reg2) {
                return false;
            }
        }

        // 如果两个操作写入同一个寄存器，则不能交换
        for &reg1 in &written1 {
            if written2.contains(&reg1) {
                return false;
            }
        }

        true
    }
}
