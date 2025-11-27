//! Cranelift IR 转换器
//!
//! 将 vm-ir 转换为 Cranelift IR。
//! 独立于具体的 Module 实现（JIT 或 Object），仅负责 IR 生成。

use vm_ir::{IROp, IRBlock, Terminator, RegId, AtomicOp};
use cranelift::prelude::*;
use std::collections::HashMap;
use rayon::prelude::*;
use std::sync::Arc;

/// 编译器错误类型
#[derive(Debug, Clone)]
pub enum TranslatorError {
    /// 不支持的操作
    UnsupportedOperation(String),
    /// 变量未定义
    UndefinedVariable(RegId),
}

impl std::fmt::Display for TranslatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            TranslatorError::UnsupportedOperation(msg) => write!(f, "Unsupported operation: {}", msg),
            TranslatorError::UndefinedVariable(reg) => write!(f, "Undefined variable: {}", reg),
        }
    }
}

impl std::error::Error for TranslatorError {}

/// Cranelift 转换器
pub struct CraneliftTranslator<'a> {
    /// Cranelift 函数构建器
    builder: FunctionBuilder<'a>,
    /// 变量映射表 (vm_ir RegId -> Cranelift Variable)
    var_map: HashMap<RegId, Variable>,
}

impl<'a> CraneliftTranslator<'a> {
    /// 并行翻译多个IR块
    pub fn translate_blocks_parallel<'b>(
        blocks: &'b [IRBlock],
        module: &'a mut cranelift_module::Module<'a, cranelift_jit::JITModule>,
    ) -> Result<Vec<FunctionBuilderContext>, TranslatorError> {
        blocks
            .par_iter()
            .map(|block| {
                let mut ctx = module.make_context();
                ctx.func.signature.clear();
                
                // 为当前模块配置适当的函数签名
                let mut builder_context = FunctionBuilderContext::new();
                let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_context);
                
                // 创建翻译器
                let mut translator = CraneliftTranslator::new(builder);
                
                // 翻译块
                translator.translate(block)?;
                
                // 完成翻译并保存上下文
                Ok(builder_context)
            })
            .collect()
    }
    /// 创建新的转换器
    pub fn new(builder: FunctionBuilder<'a>) -> Self {
        Self { 
            builder,
            var_map: HashMap::new(),
        }
    }

    /// 获取或创建变量
    fn get_or_create_var(&mut self, reg: RegId, ty: Type) -> Variable {
        if let Some(var) = self.var_map.get(&reg) {
            *var
        } else {
            let var = Variable::new(reg as usize);
            self.builder.declare_var(var, ty);
            self.var_map.insert(reg, var);
            var
        }
    }

    /// 翻译 IR 块
    pub fn translate(&mut self, block: &IRBlock) -> Result<(), TranslatorError> {
        // 创建入口块
        let entry_block = self.builder.create_block();
        self.builder.switch_to_block(entry_block);
        self.builder.seal_block(entry_block); // 假设只有一个块，或者是入口块

        // 翻译操作
        for op in &block.ops {
            self.translate_op(op)?;
        }

        // 翻译终结符
        self.translate_terminator(&block.terminator)?;

        Ok(())
    }

    fn translate_op(&mut self, op: &IROp) -> Result<(), TranslatorError> {
        match op {
            IROp::Add { dst, src1, src2 } => {
                let v1 = self.get_or_create_var(*src1, types::I64); // 默认 I64，实际应根据上下文
                let v2 = self.get_or_create_var(*src2, types::I64);
                let val1 = self.builder.use_var(v1);
                let val2 = self.builder.use_var(v2);
                let res = self.builder.ins().iadd(val1, val2);
                let v_dst = self.get_or_create_var(*dst, types::I64);
                self.builder.def_var(v_dst, res);
            }
            IROp::Sub { dst, src1, src2 } => {
                let v1 = self.get_or_create_var(*src1, types::I64);
                let v2 = self.get_or_create_var(*src2, types::I64);
                let val1 = self.builder.use_var(v1);
                let val2 = self.builder.use_var(v2);
                let res = self.builder.ins().isub(val1, val2);
                let v_dst = self.get_or_create_var(*dst, types::I64);
                self.builder.def_var(v_dst, res);
            }
            IROp::Mul { dst, src1, src2 } => {
                let v1 = self.get_or_create_var(*src1, types::I64);
                let v2 = self.get_or_create_var(*src2, types::I64);
                let val1 = self.builder.use_var(v1);
                let val2 = self.builder.use_var(v2);
                let res = self.builder.ins().imul(val1, val2);
                let v_dst = self.get_or_create_var(*dst, types::I64);
                self.builder.def_var(v_dst, res);
            }
            IROp::MovImm { dst, imm } => {
                let res = self.builder.ins().iconst(types::I64, *imm as i64);
                let v_dst = self.get_or_create_var(*dst, types::I64);
                self.builder.def_var(v_dst, res);
            }

            // Arithmetic
            IROp::Div { dst, src1, src2, signed } => {
                let v1 = self.get_or_create_var(*src1, types::I64);
                let v2 = self.get_or_create_var(*src2, types::I64);
                let val1 = self.builder.use_var(v1);
                let val2 = self.builder.use_var(v2);
                let res = if *signed {
                    self.builder.ins().sdiv(val1, val2)
                } else {
                    self.builder.ins().udiv(val1, val2)
                };
                let v_dst = self.get_or_create_var(*dst, types::I64);
                self.builder.def_var(v_dst, res);
            }
            IROp::Rem { dst, src1, src2, signed } => {
                let v1 = self.get_or_create_var(*src1, types::I64);
                let v2 = self.get_or_create_var(*src2, types::I64);
                let val1 = self.builder.use_var(v1);
                let val2 = self.builder.use_var(v2);
                let res = if *signed {
                    self.builder.ins().srem(val1, val2)
                } else {
                    self.builder.ins().urem(val1, val2)
                };
                let v_dst = self.get_or_create_var(*dst, types::I64);
                self.builder.def_var(v_dst, res);
            }

            // Bitwise
            IROp::And { dst, src1, src2 } => {
                let v1 = self.get_or_create_var(*src1, types::I64);
                let v2 = self.get_or_create_var(*src2, types::I64);
                let val1 = self.builder.use_var(v1);
                let val2 = self.builder.use_var(v2);
                let res = self.builder.ins().band(val1, val2);
                let v_dst = self.get_or_create_var(*dst, types::I64);
                self.builder.def_var(v_dst, res);
            }
            IROp::Or { dst, src1, src2 } => {
                let v1 = self.get_or_create_var(*src1, types::I64);
                let v2 = self.get_or_create_var(*src2, types::I64);
                let val1 = self.builder.use_var(v1);
                let val2 = self.builder.use_var(v2);
                let res = self.builder.ins().bor(val1, val2);
                let v_dst = self.get_or_create_var(*dst, types::I64);
                self.builder.def_var(v_dst, res);
            }
            IROp::Xor { dst, src1, src2 } => {
                let v1 = self.get_or_create_var(*src1, types::I64);
                let v2 = self.get_or_create_var(*src2, types::I64);
                let val1 = self.builder.use_var(v1);
                let val2 = self.builder.use_var(v2);
                let res = self.builder.ins().bxor(val1, val2);
                let v_dst = self.get_or_create_var(*dst, types::I64);
                self.builder.def_var(v_dst, res);
            }
            IROp::Not { dst, src } => {
                let v = self.get_or_create_var(*src, types::I64);
                let val = self.builder.use_var(v);
                let res = self.builder.ins().bnot(val);
                let v_dst = self.get_or_create_var(*dst, types::I64);
                self.builder.def_var(v_dst, res);
            }

            // Shifts
            IROp::Sll { dst, src, shreg } => {
                let v = self.get_or_create_var(*src, types::I64);
                let sh = self.get_or_create_var(*shreg, types::I64);
                let val = self.builder.use_var(v);
                let shift = self.builder.use_var(sh);
                let res = self.builder.ins().ishl(val, shift);
                let v_dst = self.get_or_create_var(*dst, types::I64);
                self.builder.def_var(v_dst, res);
            }
            IROp::Srl { dst, src, shreg } => {
                let v = self.get_or_create_var(*src, types::I64);
                let sh = self.get_or_create_var(*shreg, types::I64);
                let val = self.builder.use_var(v);
                let shift = self.builder.use_var(sh);
                let res = self.builder.ins().ushr(val, shift);
                let v_dst = self.get_or_create_var(*dst, types::I64);
                self.builder.def_var(v_dst, res);
            }
            IROp::Sra { dst, src, shreg } => {
                let v = self.get_or_create_var(*src, types::I64);
                let sh = self.get_or_create_var(*shreg, types::I64);
                let val = self.builder.use_var(v);
                let shift = self.builder.use_var(sh);
                let res = self.builder.ins().sshr(val, shift);
                let v_dst = self.get_or_create_var(*dst, types::I64);
                self.builder.def_var(v_dst, res);
            }

            // Immediates
            IROp::AddImm { dst, src, imm } => {
                let v = self.get_or_create_var(*src, types::I64);
                let val = self.builder.use_var(v);
                let res = self.builder.ins().iadd_imm(val, *imm);
                let v_dst = self.get_or_create_var(*dst, types::I64);
                self.builder.def_var(v_dst, res);
            }
            IROp::MulImm { dst, src, imm } => {
                let v = self.get_or_create_var(*src, types::I64);
                let val = self.builder.use_var(v);
                let res = self.builder.ins().imul_imm(val, *imm);
                let v_dst = self.get_or_create_var(*dst, types::I64);
                self.builder.def_var(v_dst, res);
            }
            IROp::SllImm { dst, src, sh } => {
                let v = self.get_or_create_var(*src, types::I64);
                let val = self.builder.use_var(v);
                let res = self.builder.ins().ishl_imm(val, *sh as i64);
                let v_dst = self.get_or_create_var(*dst, types::I64);
                self.builder.def_var(v_dst, res);
            }
            IROp::SrlImm { dst, src, sh } => {
                let v = self.get_or_create_var(*src, types::I64);
                let val = self.builder.use_var(v);
                let res = self.builder.ins().ushr_imm(val, *sh as i64);
                let v_dst = self.get_or_create_var(*dst, types::I64);
                self.builder.def_var(v_dst, res);
            }
            IROp::SraImm { dst, src, sh } => {
                let v = self.get_or_create_var(*src, types::I64);
                let val = self.builder.use_var(v);
                let res = self.builder.ins().sshr_imm(val, *sh as i64);
                let v_dst = self.get_or_create_var(*dst, types::I64);
                self.builder.def_var(v_dst, res);
            }

            // Memory
            IROp::Load { dst, base, offset, size, flags: _ } => {
                let v_base = self.get_or_create_var(*base, types::I64);
                let base_val = self.builder.use_var(v_base);
                let mem_type = match size {
                    1 => types::I8,
                    2 => types::I16,
                    4 => types::I32,
                    8 => types::I64,
                    _ => return Err(TranslatorError::UnsupportedOperation(format!("Unsupported load size: {}", size))),
                };
                let flags = MemFlags::new();
                let res = self.builder.ins().load(mem_type, flags, base_val, *offset as i32);
                let res_ext = if *size < 8 {
                    self.builder.ins().uextend(types::I64, res)
                } else {
                    res
                };
                let v_dst = self.get_or_create_var(*dst, types::I64);
                self.builder.def_var(v_dst, res_ext);
            }
            IROp::Store { src, base, offset, size, flags: _ } => {
                let v_src = self.get_or_create_var(*src, types::I64);
                let v_base = self.get_or_create_var(*base, types::I64);
                let src_val = self.builder.use_var(v_src);
                let base_val = self.builder.use_var(v_base);
                
                let val_to_store = if *size < 8 {
                     self.builder.ins().ireduce(match size {
                        1 => types::I8,
                        2 => types::I16,
                        4 => types::I32,
                        _ => unreachable!(),
                     }, src_val)
                } else {
                    src_val
                };

                let flags = MemFlags::new();
                self.builder.ins().store(flags, val_to_store, base_val, *offset as i32);
            }

            // Comparisons
            IROp::CmpEq { dst, lhs, rhs } => {
                self.translate_cmp(IntCC::Equal, *dst, *lhs, *rhs)?;
            }
            IROp::CmpNe { dst, lhs, rhs } => {
                self.translate_cmp(IntCC::NotEqual, *dst, *lhs, *rhs)?;
            }
            IROp::CmpLt { dst, lhs, rhs } => {
                self.translate_cmp(IntCC::SignedLessThan, *dst, *lhs, *rhs)?;
            }
            IROp::CmpLtU { dst, lhs, rhs } => {
                self.translate_cmp(IntCC::UnsignedLessThan, *dst, *lhs, *rhs)?;
            }
            IROp::CmpGe { dst, lhs, rhs } => {
                self.translate_cmp(IntCC::SignedGreaterThanOrEqual, *dst, *lhs, *rhs)?;
            }
            IROp::CmpGeU { dst, lhs, rhs } => {
                self.translate_cmp(IntCC::UnsignedGreaterThanOrEqual, *dst, *lhs, *rhs)?;
            }

            // Select
            IROp::Select { dst, cond, true_val, false_val } => {
                let v_cond = self.get_or_create_var(*cond, types::I64);
                let v_true = self.get_or_create_var(*true_val, types::I64);
                let v_false = self.get_or_create_var(*false_val, types::I64);
                let cond_val = self.builder.use_var(v_cond);
                let true_val = self.builder.use_var(v_true);
                let false_val = self.builder.use_var(v_false);
                let cond_bool = self.builder.ins().icmp_imm(IntCC::NotEqual, cond_val, 0);
                let res = self.builder.ins().select(cond_bool, true_val, false_val);
                let v_dst = self.get_or_create_var(*dst, types::I64);
                self.builder.def_var(v_dst, res);
            }

            // Floating Point - Basic Operations (64-bit)
            IROp::Fadd { dst, src1, src2 } => {
                let v1 = self.get_or_create_var(*src1, types::F64);
                let v2 = self.get_or_create_var(*src2, types::F64);
                let val1 = self.builder.use_var(v1);
                let val2 = self.builder.use_var(v2);
                let res = self.builder.ins().fadd(val1, val2);
                let v_dst = self.get_or_create_var(*dst, types::F64);
                self.builder.def_var(v_dst, res);
            }
            IROp::Fsub { dst, src1, src2 } => {
                let v1 = self.get_or_create_var(*src1, types::F64);
                let v2 = self.get_or_create_var(*src2, types::F64);
                let val1 = self.builder.use_var(v1);
                let val2 = self.builder.use_var(v2);
                let res = self.builder.ins().fsub(val1, val2);
                let v_dst = self.get_or_create_var(*dst, types::F64);
                self.builder.def_var(v_dst, res);
            }
            IROp::Fmul { dst, src1, src2 } => {
                let v1 = self.get_or_create_var(*src1, types::F64);
                let v2 = self.get_or_create_var(*src2, types::F64);
                let val1 = self.builder.use_var(v1);
                let val2 = self.builder.use_var(v2);
                let res = self.builder.ins().fmul(val1, val2);
                let v_dst = self.get_or_create_var(*dst, types::F64);
                self.builder.def_var(v_dst, res);
            }
            IROp::Fdiv { dst, src1, src2 } => {
                let v1 = self.get_or_create_var(*src1, types::F64);
                let v2 = self.get_or_create_var(*src2, types::F64);
                let val1 = self.builder.use_var(v1);
                let val2 = self.builder.use_var(v2);
                let res = self.builder.ins().fdiv(val1, val2);
                let v_dst = self.get_or_create_var(*dst, types::F64);
                self.builder.def_var(v_dst, res);
            }
            IROp::Fsqrt { dst, src } => {
                let v = self.get_or_create_var(*src, types::F64);
                let val = self.builder.use_var(v);
                let res = self.builder.ins().sqrt(val);
                let v_dst = self.get_or_create_var(*dst, types::F64);
                self.builder.def_var(v_dst, res);
            }
            IROp::Fmin { dst, src1, src2 } => {
                let v1 = self.get_or_create_var(*src1, types::F64);
                let v2 = self.get_or_create_var(*src2, types::F64);
                let val1 = self.builder.use_var(v1);
                let val2 = self.builder.use_var(v2);
                let res = self.builder.ins().fmin(val1, val2);
                let v_dst = self.get_or_create_var(*dst, types::F64);
                self.builder.def_var(v_dst, res);
            }
            IROp::Fmax { dst, src1, src2 } => {
                let v1 = self.get_or_create_var(*src1, types::F64);
                let v2 = self.get_or_create_var(*src2, types::F64);
                let val1 = self.builder.use_var(v1);
                let val2 = self.builder.use_var(v2);
                let res = self.builder.ins().fmax(val1, val2);
                let v_dst = self.get_or_create_var(*dst, types::F64);
                self.builder.def_var(v_dst, res);
            }

            // Floating Point - Single Precision (32-bit)
            IROp::FaddS { dst, src1, src2 } => {
                let v1 = self.get_or_create_var(*src1, types::F32);
                let v2 = self.get_or_create_var(*src2, types::F32);
                let val1 = self.builder.use_var(v1);
                let val2 = self.builder.use_var(v2);
                let res = self.builder.ins().fadd(val1, val2);
                let v_dst = self.get_or_create_var(*dst, types::F32);
                self.builder.def_var(v_dst, res);
            }
            IROp::FsubS { dst, src1, src2 } => {
                let v1 = self.get_or_create_var(*src1, types::F32);
                let v2 = self.get_or_create_var(*src2, types::F32);
                let val1 = self.builder.use_var(v1);
                let val2 = self.builder.use_var(v2);
                let res = self.builder.ins().fsub(val1, val2);
                let v_dst = self.get_or_create_var(*dst, types::F32);
                self.builder.def_var(v_dst, res);
            }
            IROp::FmulS { dst, src1, src2 } => {
                let v1 = self.get_or_create_var(*src1, types::F32);
                let v2 = self.get_or_create_var(*src2, types::F32);
                let val1 = self.builder.use_var(v1);
                let val2 = self.builder.use_var(v2);
                let res = self.builder.ins().fmul(val1, val2);
                let v_dst = self.get_or_create_var(*dst, types::F32);
                self.builder.def_var(v_dst, res);
            }
            IROp::FdivS { dst, src1, src2 } => {
                let v1 = self.get_or_create_var(*src1, types::F32);
                let v2 = self.get_or_create_var(*src2, types::F32);
                let val1 = self.builder.use_var(v1);
                let val2 = self.builder.use_var(v2);
                let res = self.builder.ins().fdiv(val1, val2);
                let v_dst = self.get_or_create_var(*dst, types::F32);
                self.builder.def_var(v_dst, res);
            }
            IROp::FsqrtS { dst, src } => {
                let v = self.get_or_create_var(*src, types::F32);
                let val = self.builder.use_var(v);
                let res = self.builder.ins().sqrt(val);
                let v_dst = self.get_or_create_var(*dst, types::F32);
                self.builder.def_var(v_dst, res);
            }
            IROp::FminS { dst, src1, src2 } => {
                let v1 = self.get_or_create_var(*src1, types::F32);
                let v2 = self.get_or_create_var(*src2, types::F32);
                let val1 = self.builder.use_var(v1);
                let val2 = self.builder.use_var(v2);
                let res = self.builder.ins().fmin(val1, val2);
                let v_dst = self.get_or_create_var(*dst, types::F32);
                self.builder.def_var(v_dst, res);
            }
            IROp::FmaxS { dst, src1, src2 } => {
                let v1 = self.get_or_create_var(*src1, types::F32);
                let v2 = self.get_or_create_var(*src2, types::F32);
                let val1 = self.builder.use_var(v1);
                let val2 = self.builder.use_var(v2);
                let res = self.builder.ins().fmax(val1, val2);
                let v_dst = self.get_or_create_var(*dst, types::F32);
                self.builder.def_var(v_dst, res);
            }

            // Floating Point - Fused Multiply-Add (64-bit)
            IROp::Fmadd { dst, src1, src2, src3 } => {
                let v1 = self.get_or_create_var(*src1, types::F64);
                let v2 = self.get_or_create_var(*src2, types::F64);
                let v3 = self.get_or_create_var(*src3, types::F64);
                let val1 = self.builder.use_var(v1);
                let val2 = self.builder.use_var(v2);
                let val3 = self.builder.use_var(v3);
                let res = self.builder.ins().fma(val1, val2, val3);
                let v_dst = self.get_or_create_var(*dst, types::F64);
                self.builder.def_var(v_dst, res);
            }
            IROp::Fmsub { dst, src1, src2, src3 } => {
                let v1 = self.get_or_create_var(*src1, types::F64);
                let v2 = self.get_or_create_var(*src2, types::F64);
                let v3 = self.get_or_create_var(*src3, types::F64);
                let val1 = self.builder.use_var(v1);
                let val2 = self.builder.use_var(v2);
                let val3 = self.builder.use_var(v3);
                let neg_val3 = self.builder.ins().fneg(val3);
                let res = self.builder.ins().fma(val1, val2, neg_val3);
                let v_dst = self.get_or_create_var(*dst, types::F64);
                self.builder.def_var(v_dst, res);
            }

            // Floating Point - Comparisons (64-bit)
            IROp::Feq { dst, src1, src2 } => {
                let v1 = self.get_or_create_var(*src1, types::F64);
                let v2 = self.get_or_create_var(*src2, types::F64);
                let val1 = self.builder.use_var(v1);
                let val2 = self.builder.use_var(v2);
                let res_bool = self.builder.ins().fcmp(FloatCC::Equal, val1, val2);
                let res = self.builder.ins().bint(types::I64, res_bool);
                let v_dst = self.get_or_create_var(*dst, types::I64);
                self.builder.def_var(v_dst, res);
            }
            IROp::Flt { dst, src1, src2 } => {
                let v1 = self.get_or_create_var(*src1, types::F64);
                let v2 = self.get_or_create_var(*src2, types::F64);
                let val1 = self.builder.use_var(v1);
                let val2 = self.builder.use_var(v2);
                let res_bool = self.builder.ins().fcmp(FloatCC::LessThan, val1, val2);
                let res = self.builder.ins().bint(types::I64, res_bool);
                let v_dst = self.get_or_create_var(*dst, types::I64);
                self.builder.def_var(v_dst, res);
            }
            IROp::Fle { dst, src1, src2 } => {
                let v1 = self.get_or_create_var(*src1, types::F64);
                let v2 = self.get_or_create_var(*src2, types::F64);
                let val1 = self.builder.use_var(v1);
                let val2 = self.builder.use_var(v2);
                let res_bool = self.builder.ins().fcmp(FloatCC::LessThanOrEqual, val1, val2);
                let res = self.builder.ins().bint(types::I64, res_bool);
                let v_dst = self.get_or_create_var(*dst, types::I64);
                self.builder.def_var(v_dst, res);
            }

            // Floating Point - Conversions
            IROp::Fcvtws { dst, src } => {
                let v = self.get_or_create_var(*src, types::F32);
                let val = self.builder.use_var(v);
                let res = self.builder.ins().fcvt_to_sint(types::I32, val);
                let res_ext = self.builder.ins().sextend(types::I64, res);
                let v_dst = self.get_or_create_var(*dst, types::I64);
                self.builder.def_var(v_dst, res_ext);
            }
            IROp::Fcvtwus { dst, src } => {
                let v = self.get_or_create_var(*src, types::F32);
                let val = self.builder.use_var(v);
                let res = self.builder.ins().fcvt_to_uint(types::I32, val);
                let res_ext = self.builder.ins().uextend(types::I64, res);
                let v_dst = self.get_or_create_var(*dst, types::I64);
                self.builder.def_var(v_dst, res_ext);
            }
            IROp::Fcvtls { dst, src } => {
                let v = self.get_or_create_var(*src, types::F32);
                let val = self.builder.use_var(v);
                let res = self.builder.ins().fcvt_to_sint(types::I64, val);
                let v_dst = self.get_or_create_var(*dst, types::I64);
                self.builder.def_var(v_dst, res);
            }
            IROp::Fcvtsw { dst, src } => {
                let v = self.get_or_create_var(*src, types::I32);
                let val = self.builder.use_var(v);
                let res = self.builder.ins().fcvt_from_sint(types::F32, val);
                let v_dst = self.get_or_create_var(*dst, types::F32);
                self.builder.def_var(v_dst, res);
            }
            IROp::Fcvtsl { dst, src } => {
                let v = self.get_or_create_var(*src, types::I64);
                let val = self.builder.use_var(v);
                let res = self.builder.ins().fcvt_from_sint(types::F32, val);
                let v_dst = self.get_or_create_var(*dst, types::F32);
                self.builder.def_var(v_dst, res);
            }
            IROp::Fcvtwd { dst, src } => {
                let v = self.get_or_create_var(*src, types::F64);
                let val = self.builder.use_var(v);
                let res = self.builder.ins().fcvt_to_sint(types::I32, val);
                let res_ext = self.builder.ins().sextend(types::I64, res);
                let v_dst = self.get_or_create_var(*dst, types::I64);
                self.builder.def_var(v_dst, res_ext);
            }
            IROp::Fcvtdw { dst, src } => {
                let v = self.get_or_create_var(*src, types::I32);
                let val = self.builder.use_var(v);
                let res = self.builder.ins().fcvt_from_sint(types::F64, val);
                let v_dst = self.get_or_create_var(*dst, types::F64);
                self.builder.def_var(v_dst, res);
            }
            IROp::Fcvtdl { dst, src } => {
                let v = self.get_or_create_var(*src, types::I64);
                let val = self.builder.use_var(v);
                let res = self.builder.ins().fcvt_from_sint(types::F64, val);
                let v_dst = self.get_or_create_var(*dst, types::F64);
                self.builder.def_var(v_dst, res);
            }
            IROp::Fcvtsd { dst, src } => {
                let v = self.get_or_create_var(*src, types::F64);
                let val = self.builder.use_var(v);
                let res = self.builder.ins().fpromote(types::F32, val);
                let v_dst = self.get_or_create_var(*dst, types::F32);
                self.builder.def_var(v_dst, res);
            }
            IROp::Fcvtds { dst, src } => {
                let v = self.get_or_create_var(*src, types::F32);
                let val = self.builder.use_var(v);
                let res = self.builder.ins().fpromote(types::F64, val);
                let v_dst = self.get_or_create_var(*dst, types::F64);
                self.builder.def_var(v_dst, res);
            }

            // Floating Point - Absolute/Negate
            IROp::Fabs { dst, src } => {
                let v = self.get_or_create_var(*src, types::F64);
                let val = self.builder.use_var(v);
                let res = self.builder.ins().fabs(val);
                let v_dst = self.get_or_create_var(*dst, types::F64);
                self.builder.def_var(v_dst, res);
            }
            IROp::Fneg { dst, src } => {
                let v = self.get_or_create_var(*src, types::F64);
                let val = self.builder.use_var(v);
                let res = self.builder.ins().fneg(val);
                let v_dst = self.get_or_create_var(*dst, types::F64);
                self.builder.def_var(v_dst, res);
            }
            IROp::FabsS { dst, src } => {
                let v = self.get_or_create_var(*src, types::F32);
                let val = self.builder.use_var(v);
                let res = self.builder.ins().fabs(val);
                let v_dst = self.get_or_create_var(*dst, types::F32);
                self.builder.def_var(v_dst, res);
            }
            IROp::FnegS { dst, src } => {
                let v = self.get_or_create_var(*src, types::F32);
                let val = self.builder.use_var(v);
                let res = self.builder.ins().fneg(val);
                let v_dst = self.get_or_create_var(*dst, types::F32);
                self.builder.def_var(v_dst, res);
            }

            // Floating Point - Load/Store
            IROp::Fload { dst, base, offset, size } => {
                let v_base = self.get_or_create_var(*base, types::I64);
                let base_val = self.builder.use_var(v_base);
                let mem_type = match size {
                    4 => types::F32,
                    8 => types::F64,
                    _ => return Err(TranslatorError::UnsupportedOperation(format!("Unsupported float load size: {}", size))),
                };
                let flags = MemFlags::new();
                let res = self.builder.ins().load(mem_type, flags, base_val, *offset as i32);
                let v_dst = self.get_or_create_var(*dst, mem_type);
                self.builder.def_var(v_dst, res);
            }
            IROp::Fstore { src, base, offset, size } => {
                let v_src = self.get_or_create_var(*src, if *size == 4 { types::F32 } else { types::F64 });
                let v_base = self.get_or_create_var(*base, types::I64);
                let src_val = self.builder.use_var(v_src);
                let base_val = self.builder.use_var(v_base);
                let flags = MemFlags::new();
                self.builder.ins().store(flags, src_val, base_val, *offset as i32);
            }

            // Atomic Operations
            IROp::Atomic { dst, base, src, op, size } => {
                let v_base = self.get_or_create_var(*base, types::I64);
                let v_src = self.get_or_create_var(*src, types::I64);
                let base_val = self.builder.use_var(v_base);
                let src_val = self.builder.use_var(v_src);

                let mem_type = match size {
                    1 => types::I8,
                    2 => types::I16,
                    4 => types::I32,
                    8 => types::I64,
                    _ => return Err(TranslatorError::UnsupportedOperation(format!("Unsupported atomic size: {}", size))),
                };

                // 缩减值到正确大小
                let val_to_op = if *size < 8 {
                    self.builder.ins().ireduce(mem_type, src_val)
                } else {
                    src_val
                };

                // 映射操作
                let op_code = match op {
                    AtomicOp::Add => AtomicRmwOp::Add,
                    AtomicOp::Sub => AtomicRmwOp::Sub,
                    AtomicOp::And => AtomicRmwOp::BitwiseAnd,
                    AtomicOp::Or => AtomicRmwOp::BitwiseOr,
                    AtomicOp::Xor => AtomicRmwOp::BitwiseXor,
                    AtomicOp::Xchg => AtomicRmwOp::Xchg,
                    _ => return Err(TranslatorError::UnsupportedOperation(format!("Unsupported atomic op: {:?}", op))),
                };

                let flags = MemFlags::new();
                let res = self.builder.ins().atomic_rmw(op_code, flags, mem_type, base_val, 0, val_to_op);
                
                let res_ext = if *size < 8 {
                    self.builder.ins().uextend(types::I64, res)
                } else {
                    res
                };

                let v_dst = self.get_or_create_var(*dst, types::I64);
                self.builder.def_var(v_dst, res_ext);
            }

            // SIMD Operations - Vector Add
            IROp::VecAdd { dst, src1, src2, element_size } => {
                let vec_type = match element_size {
                    1 => types::I8X16,
                    2 => types::I16X8,
                    4 => types::I32X4,
                    8 => types::I64X2,
                    _ => return Err(TranslatorError::UnsupportedOperation(format!("Unsupported vector element size: {}", element_size))),
                };

                let v1 = self.get_or_create_var(*src1, vec_type);
                let v2 = self.get_or_create_var(*src2, vec_type);
                let val1 = self.builder.use_var(v1);
                let val2 = self.builder.use_var(v2);
                let res = self.builder.ins().iadd(val1, val2);
                let v_dst = self.get_or_create_var(*dst, vec_type);
                self.builder.def_var(v_dst, res);
            }

            // SIMD Operations - Vector Sub
            IROp::VecSub { dst, src1, src2, element_size } => {
                let vec_type = match element_size {
                    1 => types::I8X16,
                    2 => types::I16X8,
                    4 => types::I32X4,
                    8 => types::I64X2,
                    _ => return Err(TranslatorError::UnsupportedOperation(format!("Unsupported vector element size: {}", element_size))),
                };

                let v1 = self.get_or_create_var(*src1, vec_type);
                let v2 = self.get_or_create_var(*src2, vec_type);
                let val1 = self.builder.use_var(v1);
                let val2 = self.builder.use_var(v2);
                let res = self.builder.ins().isub(val1, val2);
                let v_dst = self.get_or_create_var(*dst, vec_type);
                self.builder.def_var(v_dst, res);
            }

            // SIMD Operations - Vector Mul
            IROp::VecMul { dst, src1, src2, element_size } => {
                let vec_type = match element_size {
                    1 => types::I8X16,
                    2 => types::I16X8,
                    4 => types::I32X4,
                    8 => types::I64X2,
                    _ => return Err(TranslatorError::UnsupportedOperation(format!("Unsupported vector element size: {}", element_size))),
                };

                let v1 = self.get_or_create_var(*src1, vec_type);
                let v2 = self.get_or_create_var(*src2, vec_type);
                let val1 = self.builder.use_var(v1);
                let val2 = self.builder.use_var(v2);
                let res = self.builder.ins().imul(val1, val2);
                let v_dst = self.get_or_create_var(*dst, vec_type);
                self.builder.def_var(v_dst, res);
            }

            // System Instructions - CPUId
            IROp::Cpuid { leaf, subleaf: _, dst_eax, dst_ebx, dst_ecx, dst_edx } => {
                // 简化实现：返回零
                // 真实实现需要调用实际的 CPUID 指令或预定义的返回值
                let zero = self.builder.ins().iconst(types::I64, 0);
                
                let v_eax = self.get_or_create_var(*dst_eax, types::I64);
                let v_ebx = self.get_or_create_var(*dst_ebx, types::I64);
                let v_ecx = self.get_or_create_var(*dst_ecx, types::I64);
                let v_edx = self.get_or_create_var(*dst_edx, types::I64);
                
                self.builder.def_var(v_eax, zero);
                self.builder.def_var(v_ebx, zero);
                self.builder.def_var(v_ecx, zero);
                self.builder.def_var(v_edx, zero);
            }

            // System Instructions - TLB Flush
            IROp::TlbFlush { vaddr: _ } => {
                // 空操作：Cranelift 不直接支持 TLB 操作
                // 在真实实现中，这应该生成平台特定的指令或跳过
            }

            // System Instructions - CSR Read/Write
            IROp::CsrRead { dst, csr: _ } => {
                // 简化实现：返回零
                let zero = self.builder.ins().iconst(types::I64, 0);
                let v_dst = self.get_or_create_var(*dst, types::I64);
                self.builder.def_var(v_dst, zero);
            }

            IROp::CsrWrite { csr: _, src: _ } => {
                // 空操作
            }

            IROp::CsrSet { csr: _, src: _ } => {
                // 空操作
            }

            IROp::CsrClear { csr: _, src: _ } => {
                // 空操作
            }

            // 不支持或暂未实现的操作
            IROp::VecAddSat { .. } | IROp::VecSubSat { .. } | IROp::VecMulSat { .. } |
            IROp::Vec128Add { .. } | IROp::Vec256Add { .. } | IROp::Vec256Sub { .. } | IROp::Vec256Mul { .. } |
            IROp::FmaddS { .. } | IROp::FmsubS { .. } | IROp::FnmaddS { .. } | IROp::FnmsubS { .. } |
            IROp::FeqS { .. } | IROp::FltS { .. } | IROp::FleS { .. } |
            IROp::Fcvtwud { .. } | IROp::Fcvtlud { .. } | IROp::Fcvtswu { .. } | IROp::Fcvtslu { .. } |
            IROp::Fsgnj { .. } | IROp::Fsgnjn { .. } | IROp::Fsgnjx { .. } |
            IROp::FsgnjS { .. } | IROp::FsgnjnS { .. } | IROp::FsgnjxS { .. } |
            IROp::Fclass { .. } | IROp::FclassS { .. } |
            IROp::FmvXW { .. } | IROp::FmvWX { .. } | IROp::FmvXD { .. } | IROp::FmvDX { .. } |
            IROp::AtomicRMW { .. } | IROp::AtomicRMWOrder { .. } |
            IROp::AtomicCmpXchg { .. } | IROp::AtomicCmpXchgOrder { .. } |
            IROp::AtomicLoadReserve { .. } | IROp::AtomicStoreCond { .. } |
            IROp::AtomicCmpXchgFlag { .. } | IROp::AtomicRmwFlag { .. } |
            IROp::Beq { .. } | IROp::Bne { .. } | IROp::Blt { .. } |
            IROp::Bge { .. } | IROp::Bltu { .. } | IROp::Bgeu { .. } |
            IROp::SysCall | IROp::DebugBreak |
            IROp::CsrSetImm { .. } | IROp::CsrClearImm { .. } |
            IROp::SysMret | IROp::SysSret | IROp::SysWfi |
            IROp::ReadPstateFlags { .. } | IROp::WritePstateFlags { .. } |
            IROp::EvalCondition { .. } |
            IROp::VendorLoad { .. } | IROp::VendorStore { .. } |
            IROp::VendorMatrixOp { .. } | IROp::VendorConfig { .. } |
            IROp::VendorVectorOp { .. } |
            IROp::Nop => {
                // 某些操作暂不支持，但 Nop 可以直接跳过
                if matches!(op, IROp::Nop) {
                    // No operation needed
                } else {
                    return Err(TranslatorError::UnsupportedOperation(format!("{:?}", op)));
                }
            }

            _ => return Err(TranslatorError::UnsupportedOperation(format!("{:?}", op))),
        }
        Ok(())
    }

    fn translate_terminator(&mut self, term: &Terminator) -> Result<(), TranslatorError> {
        match term {
            Terminator::Ret { value } => {
                if let Some(reg) = value {
                    let v = self.get_or_create_var(*reg, types::I64);
                    let val = self.builder.use_var(v);
                    self.builder.ins().return_(&[val]);
                } else {
                    self.builder.ins().return_(&[]);
                }
            }
            Terminator::Fault { cause: _ } => {
                self.builder.ins().trap(TrapCode::User(0));
            }
            // ... 其他终结符
            _ => return Err(TranslatorError::UnsupportedOperation(format!("{:?}", term))),
        }
        Ok(())
    }

    fn translate_cmp(&mut self, cc: IntCC, dst: RegId, lhs: RegId, rhs: RegId) -> Result<(), TranslatorError> {
        let v1 = self.get_or_create_var(lhs, types::I64);
        let v2 = self.get_or_create_var(rhs, types::I64);
        let val1 = self.builder.use_var(v1);
        let val2 = self.builder.use_var(v2);
        let res_bool = self.builder.ins().icmp(cc, val1, val2);
        let res = self.builder.ins().bint(types::I64, res_bool);
        let v_dst = self.get_or_create_var(dst, types::I64);
        self.builder.def_var(v_dst, res);
        Ok(())
    }
}
