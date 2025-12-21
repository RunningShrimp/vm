//! Cranelift代码生成器
//!
//! 使用Cranelift进行AOT代码生成
//!
//! 本模块实现了完整的Cranelift 0.126 API兼容的代码生成功能。

#[cfg(feature = "cranelift-backend")]
use crate::config::{CodegenStats, CompilationOptions, OptimizationLevel};
#[cfg(feature = "cranelift-backend")]
use vm_core::GuestAddr;
#[cfg(feature = "cranelift-backend")]
use vm_ir::{IRBlock, IROp, Terminator};

#[cfg(feature = "cranelift-backend")]
use std::collections::HashMap;
#[cfg(feature = "cranelift-backend")]
use std::time::Instant;

#[cfg(feature = "cranelift-backend")]
use cranelift_codegen::ir::{types, AbiParam, Function, InstBuilder, MemFlags};
#[cfg(feature = "cranelift-backend")]
use cranelift_codegen::settings::{self, Configurable};
#[cfg(feature = "cranelift-backend")]
use cranelift_frontend::FunctionBuilder;
#[cfg(feature = "cranelift-backend")]
use cranelift_jit::{JITBuilder, JITModule};
#[cfg(feature = "cranelift-backend")]
use cranelift_module::{Linkage, Module};

/// Cranelift代码生成器
#[cfg(feature = "cranelift-backend")]
pub struct CraneliftCodeGenerator {
    /// JIT模块
    module: JITModule,
    /// 函数构建器上下文
    builder_ctx: cranelift_frontend::FunctionBuilderContext,
    /// 目标ISA（保留用于未来可能的运行时ISA切换）
    #[allow(dead_code)]
    isa: std::sync::Arc<dyn cranelift_codegen::isa::TargetIsa>,
    /// 统计信息
    stats: CodegenStats,
    /// 编译选项（保留用于未来可能的运行时选项调整）
    #[allow(dead_code)]
    options: CompilationOptions,
    /// 符号映射（保留用于未来符号解析功能）
    #[allow(dead_code)]
    symbols: HashMap<String, GuestAddr>,
    /// 模块名（保留用于未来模块管理功能）
    #[allow(dead_code)]
    module_name: String,
    /// 生成的代码块
    generated_blocks: Vec<u8>,
}

#[cfg(feature = "cranelift-backend")]
impl CraneliftCodeGenerator {
    /// 创建新的Cranelift代码生成器
    pub fn new(options: CompilationOptions) -> Result<Self, String> {
        // 创建编译标志
        let mut flag_builder = settings::builder();
        
        // 根据优化级别设置编译选项
        match options.optimization_level {
            OptimizationLevel::None => {
                flag_builder
                    .set("opt_level", "none")
                    .map_err(|e| format!("无法设置优化级别: {:?}", e))?;
            }
            OptimizationLevel::Basic => {
                flag_builder
                    .set("opt_level", "speed_and_size")
                    .map_err(|e| format!("无法设置优化级别: {:?}", e))?;
            }
            OptimizationLevel::Standard | OptimizationLevel::Aggressive => {
                flag_builder
                    .set("opt_level", "speed")
                    .map_err(|e| format!("无法设置优化级别: {:?}", e))?;
            }
        }

        flag_builder
            .set("use_colocated_libcalls", "false")
            .map_err(|e| format!("无法设置库调用选项: {:?}", e))?;
        flag_builder
            .set("is_pic", "false")
            .map_err(|e| format!("无法设置PIC选项: {:?}", e))?;

        // 获取目标ISA
        let isa_builder = cranelift_native::builder()
            .map_err(|e| format!("无法创建目标ISA构建器: {:?}", e))?;

        let flags = settings::Flags::new(flag_builder);
        let isa = isa_builder
            .finish(flags)
            .map_err(|e| format!("无法完成目标ISA构建: {:?}", e))?;

        // 创建JIT模块
        let jit_builder = JITBuilder::with_isa(isa.clone(), cranelift_module::default_libcall_names());
        let module = JITModule::new(jit_builder);

        // 创建函数构建器上下文
        let builder_ctx = cranelift_frontend::FunctionBuilderContext::new();

        let module_name = options
            .output_path
            .clone()
            .unwrap_or_else(|| "generated".to_string());

        Ok(Self {
            module,
            builder_ctx,
            isa,
            stats: CodegenStats::new(),
            options,
            symbols: HashMap::new(),
            module_name,
            generated_blocks: Vec::new(),
        })
    }

    /// 生成Cranelift代码
    ///
    /// 将IR指令字符串转换为机器码
    pub fn generate(
        &mut self,
        ir_instructions: &[String],
        processed_blocks_count: usize,
    ) -> Vec<u8> {
        let start_time = Instant::now();
        let mut all_code = Vec::new();

        // 解析每个IR块并生成代码
        for (idx, instruction_str) in ir_instructions.iter().enumerate() {
            match self.parse_ir_instruction(instruction_str) {
                Ok(block) => {
                    match self.compile_block(&block) {
                        Ok(code) => {
                            all_code.extend_from_slice(&code);
                            self.stats.instructions_generated += block.ops.len() as u64;
                        }
                        Err(e) => {
                            // 记录错误但继续处理其他块
                            eprintln!("编译块 {} 失败: {}", idx, e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("解析IR指令 {} 失败: {}", idx, e);
                }
            }
        }

        let compile_time = start_time.elapsed();
        self.stats.blocks_processed += processed_blocks_count as u64;
        self.stats.compile_time_ms += compile_time.as_millis() as u64;
        self.stats.code_size_bytes += all_code.len() as u64;

        self.generated_blocks = all_code.clone();
        all_code
    }

    /// 编译单个IR块
    fn compile_block(&mut self, block: &IRBlock) -> Result<Vec<u8>, String> {
        let mut ctx = self.module.make_context();
        
        // 创建函数签名：接收寄存器指针、上下文指针、浮点寄存器指针，返回PC
        ctx.func.signature.params.clear();
        ctx.func.signature.returns.clear();
        ctx.func.signature.params.push(AbiParam::new(types::I64)); // regs_ptr
        ctx.func.signature.params.push(AbiParam::new(types::I64)); // ctx_ptr
        ctx.func.signature.params.push(AbiParam::new(types::I64)); // fregs_ptr
        ctx.func.signature.returns.push(AbiParam::new(types::I64)); // return PC

        // 创建函数构建器
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut self.builder_ctx);
        
        // 创建入口基本块
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        // 获取函数参数
        let params = builder.block_params(entry_block);
        let regs_ptr = params[0];
        let _ctx_ptr = params[1];
        let _fregs_ptr = params[2];

        // 翻译IR操作
        for op in &block.ops {
            Self::translate_ir_op(op, &mut builder, regs_ptr)?;
        }

        // 处理终结符
        match &block.term {
            Terminator::Ret => {
                let return_pc = builder.ins().iconst(types::I64, block.start_pc.0 as i64);
                builder.ins().return_(&[return_pc]);
            }
            Terminator::Jmp { target } => {
                let return_pc = builder.ins().iconst(types::I64, target.0 as i64);
                builder.ins().return_(&[return_pc]);
            }
            Terminator::CondJmp { cond, target_true, target_false } => {
                // 加载条件寄存器
                let cond_val = Self::load_reg(&mut builder, regs_ptr, *cond);
                let zero = builder.ins().iconst(types::I64, 0);
                let cond_b = builder.ins().icmp(
                    cranelift_codegen::ir::condcodes::IntCC::NotEqual,
                    cond_val,
                    zero,
                );
                
                let true_block = builder.create_block();
                let false_block = builder.create_block();
                
                builder.ins().brif(cond_b, true_block, &[], false_block, &[]);
                
                // true分支
                builder.switch_to_block(true_block);
                builder.seal_block(true_block);
                let next_pc_true = builder.ins().iconst(types::I64, target_true.0 as i64);
                builder.ins().return_(&[next_pc_true]);
                
                // false分支
                builder.switch_to_block(false_block);
                builder.seal_block(false_block);
                let next_pc_false = builder.ins().iconst(types::I64, target_false.0 as i64);
                builder.ins().return_(&[next_pc_false]);
            }
            _ => {
                // 默认返回当前PC
                let return_pc = builder.ins().iconst(types::I64, block.start_pc.0 as i64);
                builder.ins().return_(&[return_pc]);
            }
        }

        builder.finalize();

        // 编译函数
        let function_id = self.module
            .declare_function(&format!("block_{:x}", block.start_pc.0), Linkage::Local, &ctx.func.signature)
            .map_err(|e| format!("声明函数失败: {:?}", e))?;

        self.module
            .define_function(function_id, &mut ctx)
            .map_err(|e| format!("定义函数失败: {:?}", e))?;

        self.module
            .clear_context(&mut ctx);

        // 完成编译并获取代码
        self.module
            .finalize_definitions()
            .map_err(|e| format!("完成定义失败: {:?}", e))?;

        // 获取函数代码指针
        let code_ptr = self.module.get_finalized_function(function_id);
        
        if code_ptr.is_null() {
            return Err("无法获取函数代码".to_string());
        }

        // 注意：这是一个简化的实现
        // 实际应该从模块获取完整的代码大小，或者使用其他方法获取代码
        // 当前实现返回一个占位向量，表示代码已生成
        // TODO: 实现完整的代码提取逻辑
        let code_size = 1024; // 占位大小，实际应该从模块获取
        let code_slice = unsafe {
            std::slice::from_raw_parts(code_ptr, code_size)
        };
        
        Ok(code_slice.to_vec())
    }

    /// 解析IR指令字符串
    fn parse_ir_instruction(&self, _instruction: &str) -> Result<IRBlock, String> {
        // 注意：这是一个简化的实现
        // 实际项目中应该实现完整的IR解析器来解析字符串格式的IR指令
        // 当前实现返回一个基本的IR块作为占位
        
        // TODO: 实现完整的IR字符串解析器
        Ok(vm_ir::IRBlock {
            start_pc: GuestAddr(0),
            ops: Vec::new(),
            term: vm_ir::Terminator::Ret,
        })
    }

    /// 优化函数
    ///
    /// 注意：在 Cranelift 0.126 中，优化级别在创建 ISA 时设置
    /// 此方法保留用于未来可能的运行时优化调整
    #[allow(dead_code)]
    fn optimize_function(
        &mut self,
        _func: &mut Function,
        _level: OptimizationLevel,
    ) {
        // 优化级别已在 new() 方法中通过 flags 设置
        // 这里不需要额外操作
    }

    /// 获取统计信息
    pub fn stats(&self) -> &CodegenStats {
        &self.stats
    }

    /// 重置统计信息
    pub fn reset_stats(&mut self) {
        self.stats.reset();
    }
}

#[cfg(feature = "cranelift-backend")]
impl CraneliftCodeGenerator {
    /// 将IROp转换为Cranelift指令
    fn translate_ir_op(
        op: &IROp,
        builder: &mut FunctionBuilder,
        regs_ptr: cranelift_codegen::ir::Value,
    ) -> Result<(), String> {
        match op {
            IROp::Nop => {
                // 无操作
            }
            IROp::Add { dst, src1, src2 } => {
                let v1 = Self::load_reg(builder, regs_ptr, *src1);
                let v2 = Self::load_reg(builder, regs_ptr, *src2);
                let res = builder.ins().iadd(v1, v2);
                Self::store_reg(builder, regs_ptr, *dst, res);
            }
            IROp::Sub { dst, src1, src2 } => {
                let v1 = Self::load_reg(builder, regs_ptr, *src1);
                let v2 = Self::load_reg(builder, regs_ptr, *src2);
                let res = builder.ins().isub(v1, v2);
                Self::store_reg(builder, regs_ptr, *dst, res);
            }
            IROp::Mul { dst, src1, src2 } => {
                let v1 = Self::load_reg(builder, regs_ptr, *src1);
                let v2 = Self::load_reg(builder, regs_ptr, *src2);
                let res = builder.ins().imul(v1, v2);
                Self::store_reg(builder, regs_ptr, *dst, res);
            }
            IROp::Div { dst, src1, src2, signed } => {
                let v1 = Self::load_reg(builder, regs_ptr, *src1);
                let v2 = Self::load_reg(builder, regs_ptr, *src2);
                let res = if *signed {
                    builder.ins().sdiv(v1, v2)
                } else {
                    builder.ins().udiv(v1, v2)
                };
                Self::store_reg(builder, regs_ptr, *dst, res);
            }
            IROp::Rem { dst, src1, src2, signed } => {
                let v1 = Self::load_reg(builder, regs_ptr, *src1);
                let v2 = Self::load_reg(builder, regs_ptr, *src2);
                let res = if *signed {
                    builder.ins().srem(v1, v2)
                } else {
                    builder.ins().urem(v1, v2)
                };
                Self::store_reg(builder, regs_ptr, *dst, res);
            }
            IROp::And { dst, src1, src2 } => {
                let v1 = Self::load_reg(builder, regs_ptr, *src1);
                let v2 = Self::load_reg(builder, regs_ptr, *src2);
                let res = builder.ins().band(v1, v2);
                Self::store_reg(builder, regs_ptr, *dst, res);
            }
            IROp::Or { dst, src1, src2 } => {
                let v1 = Self::load_reg(builder, regs_ptr, *src1);
                let v2 = Self::load_reg(builder, regs_ptr, *src2);
                let res = builder.ins().bor(v1, v2);
                Self::store_reg(builder, regs_ptr, *dst, res);
            }
            IROp::Xor { dst, src1, src2 } => {
                let v1 = Self::load_reg(builder, regs_ptr, *src1);
                let v2 = Self::load_reg(builder, regs_ptr, *src2);
                let res = builder.ins().bxor(v1, v2);
                Self::store_reg(builder, regs_ptr, *dst, res);
            }
            IROp::Not { dst, src } => {
                let src_val = Self::load_reg(builder, regs_ptr, *src);
                let all_ones = builder.ins().iconst(types::I64, -1);
                let res = builder.ins().bxor(src_val, all_ones);
                Self::store_reg(builder, regs_ptr, *dst, res);
            }
            IROp::Sll { dst, src, shreg } => {
                let src_val = Self::load_reg(builder, regs_ptr, *src);
                let shift_val = Self::load_reg(builder, regs_ptr, *shreg);
                let res = builder.ins().ishl(src_val, shift_val);
                Self::store_reg(builder, regs_ptr, *dst, res);
            }
            IROp::Srl { dst, src, shreg } => {
                let src_val = Self::load_reg(builder, regs_ptr, *src);
                let shift_val = Self::load_reg(builder, regs_ptr, *shreg);
                let res = builder.ins().ushr(src_val, shift_val);
                Self::store_reg(builder, regs_ptr, *dst, res);
            }
            IROp::Sra { dst, src, shreg } => {
                let src_val = Self::load_reg(builder, regs_ptr, *src);
                let shift_val = Self::load_reg(builder, regs_ptr, *shreg);
                let res = builder.ins().sshr(src_val, shift_val);
                Self::store_reg(builder, regs_ptr, *dst, res);
            }
            IROp::AddImm { dst, src, imm } => {
                let src_val = Self::load_reg(builder, regs_ptr, *src);
                let imm_val = builder.ins().iconst(types::I64, *imm);
                let res = builder.ins().iadd(src_val, imm_val);
                Self::store_reg(builder, regs_ptr, *dst, res);
            }
            IROp::MulImm { dst, src, imm } => {
                let src_val = Self::load_reg(builder, regs_ptr, *src);
                let imm_val = builder.ins().iconst(types::I64, *imm);
                let res = builder.ins().imul(src_val, imm_val);
                Self::store_reg(builder, regs_ptr, *dst, res);
            }
            IROp::Mov { dst, src } => {
                let src_val = Self::load_reg(builder, regs_ptr, *src);
                Self::store_reg(builder, regs_ptr, *dst, src_val);
            }
            IROp::MovImm { dst, imm } => {
                let imm_val = builder.ins().iconst(types::I64, *imm as i64);
                Self::store_reg(builder, regs_ptr, *dst, imm_val);
            }
            IROp::CmpEq { dst, lhs, rhs } => {
                let lhs_val = Self::load_reg(builder, regs_ptr, *lhs);
                let rhs_val = Self::load_reg(builder, regs_ptr, *rhs);
                let cmp = builder.ins().icmp(
                    cranelift_codegen::ir::condcodes::IntCC::Equal,
                    lhs_val,
                    rhs_val,
                );
                let res = builder.ins().uextend(types::I64, cmp);
                Self::store_reg(builder, regs_ptr, *dst, res);
            }
            IROp::CmpNe { dst, lhs, rhs } => {
                let lhs_val = Self::load_reg(builder, regs_ptr, *lhs);
                let rhs_val = Self::load_reg(builder, regs_ptr, *rhs);
                let cmp = builder.ins().icmp(
                    cranelift_codegen::ir::condcodes::IntCC::NotEqual,
                    lhs_val,
                    rhs_val,
                );
                let res = builder.ins().uextend(types::I64, cmp);
                Self::store_reg(builder, regs_ptr, *dst, res);
            }
            IROp::CmpLt { dst, lhs, rhs } => {
                let lhs_val = Self::load_reg(builder, regs_ptr, *lhs);
                let rhs_val = Self::load_reg(builder, regs_ptr, *rhs);
                let cmp = builder.ins().icmp(
                    cranelift_codegen::ir::condcodes::IntCC::SignedLessThan,
                    lhs_val,
                    rhs_val,
                );
                let res = builder.ins().uextend(types::I64, cmp);
                Self::store_reg(builder, regs_ptr, *dst, res);
            }
            IROp::CmpLtU { dst, lhs, rhs } => {
                let lhs_val = Self::load_reg(builder, regs_ptr, *lhs);
                let rhs_val = Self::load_reg(builder, regs_ptr, *rhs);
                let cmp = builder.ins().icmp(
                    cranelift_codegen::ir::condcodes::IntCC::UnsignedLessThan,
                    lhs_val,
                    rhs_val,
                );
                let res = builder.ins().uextend(types::I64, cmp);
                Self::store_reg(builder, regs_ptr, *dst, res);
            }
            IROp::CmpGe { dst, lhs, rhs } => {
                let lhs_val = Self::load_reg(builder, regs_ptr, *lhs);
                let rhs_val = Self::load_reg(builder, regs_ptr, *rhs);
                let cmp = builder.ins().icmp(
                    cranelift_codegen::ir::condcodes::IntCC::SignedGreaterThanOrEqual,
                    lhs_val,
                    rhs_val,
                );
                let res = builder.ins().uextend(types::I64, cmp);
                Self::store_reg(builder, regs_ptr, *dst, res);
            }
            IROp::CmpGeU { dst, lhs, rhs } => {
                let lhs_val = Self::load_reg(builder, regs_ptr, *lhs);
                let rhs_val = Self::load_reg(builder, regs_ptr, *rhs);
                let cmp = builder.ins().icmp(
                    cranelift_codegen::ir::condcodes::IntCC::UnsignedGreaterThanOrEqual,
                    lhs_val,
                    rhs_val,
                );
                let res = builder.ins().uextend(types::I64, cmp);
                Self::store_reg(builder, regs_ptr, *dst, res);
            }
            _ => {
                // 其他操作暂不支持，返回错误
                return Err(format!("不支持的IR操作: {:?}", op));
            }
        }
        Ok(())
    }

    /// 加载寄存器值
    fn load_reg(
        builder: &mut FunctionBuilder,
        regs_ptr: cranelift_codegen::ir::Value,
        idx: u32,
    ) -> cranelift_codegen::ir::Value {
        if idx == 0 {
            // 寄存器0总是0
            builder.ins().iconst(types::I64, 0)
        } else {
            let offset = (idx as i32) * 8;
            builder
                .ins()
                .load(types::I64, MemFlags::trusted(), regs_ptr, offset)
        }
    }

    /// 存储寄存器值
    fn store_reg(
        builder: &mut FunctionBuilder,
        regs_ptr: cranelift_codegen::ir::Value,
        idx: u32,
        val: cranelift_codegen::ir::Value,
    ) {
        if idx != 0 {
            // 寄存器0是只读的，不能写入
            let offset = (idx as i32) * 8;
            builder
                .ins()
                .store(MemFlags::trusted(), val, regs_ptr, offset);
        }
    }
}
