use vm_core::{ExecutionEngine, ExecResult, ExecStatus, ExecStats, MMU, GuestAddr};
use vm_ir::{IRBlock, IROp, Terminator};
use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};
use cranelift_native;
use cranelift::codegen::settings::{self, Configurable};
use std::collections::HashMap;

pub struct JitContext<'a> {
    pub mmu: &'a mut dyn MMU,
}

unsafe extern "C" fn jit_read(ctx: *mut JitContext, addr: u64, size: u8) -> u64 {
    (*ctx).mmu.read(addr, size).unwrap_or(0)
}

unsafe extern "C" fn jit_write(ctx: *mut JitContext, addr: u64, val: u64, size: u8) {
    let _ = (*ctx).mmu.write(addr, val, size);
}

pub struct Jit {
    builder_context: FunctionBuilderContext,
    ctx: codegen::Context,
    module: JITModule,
    cache: HashMap<GuestAddr, *const u8>,
    hot_counts: HashMap<GuestAddr, u64>,
    pub regs: [u64; 32],
}

impl Jit {
    pub fn new() -> Self {
        let mut flag_builder = settings::builder();
        flag_builder.set("use_colocated_libcalls", "false").unwrap();
        flag_builder.set("is_pic", "false").unwrap();
        let isa_builder = cranelift_native::builder().unwrap();
        let isa = isa_builder.finish(settings::Flags::new(flag_builder)).unwrap();
        let mut builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
        
        builder.symbol("jit_read", jit_read as *const u8);
        builder.symbol("jit_write", jit_write as *const u8);
        let module = JITModule::new(builder);
        Self {
            builder_context: FunctionBuilderContext::new(),
            ctx: module.make_context(),
            module,
            cache: HashMap::new(),
            hot_counts: HashMap::new(),
            regs: [0; 32],
        }
    }

    fn compile(&mut self, block: &IRBlock) -> *const u8 {
        if let Some(&ptr) = self.cache.get(&block.start_pc) {
            return ptr;
        }

        self.ctx.func.signature.params.clear();
        self.ctx.func.signature.returns.clear();
        self.ctx.func.signature.params.push(AbiParam::new(types::I64)); // regs
        self.ctx.func.signature.params.push(AbiParam::new(types::I64)); // ctx
        self.ctx.func.signature.params.push(AbiParam::new(types::I64)); // read_fn
        self.ctx.func.signature.params.push(AbiParam::new(types::I64)); // write_fn
        self.ctx.func.signature.returns.push(AbiParam::new(types::I64)); // next_pc

        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let regs_ptr = builder.block_params(entry_block)[0];
        let ctx_ptr = builder.block_params(entry_block)[1];
        let read_fn_ptr = builder.block_params(entry_block)[2];
        let write_fn_ptr = builder.block_params(entry_block)[3];

        let mut sig_read = self.module.make_signature();
        sig_read.params.push(AbiParam::new(types::I64));
        sig_read.params.push(AbiParam::new(types::I64));
        sig_read.params.push(AbiParam::new(types::I8));
        sig_read.returns.push(AbiParam::new(types::I64));
        let sig_read_ref = builder.import_signature(sig_read);

        let mut sig_write = self.module.make_signature();
        sig_write.params.push(AbiParam::new(types::I64));
        sig_write.params.push(AbiParam::new(types::I64));
        sig_write.params.push(AbiParam::new(types::I64));
        sig_write.params.push(AbiParam::new(types::I8));
        let sig_write_ref = builder.import_signature(sig_write);

        for op in &block.ops {
            match op {
                IROp::Add { dst, src1, src2 } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *src1);
                    let v2 = Self::load_reg(&mut builder, regs_ptr, *src2);
                    let res = builder.ins().iadd(v1, v2);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::Sub { dst, src1, src2 } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *src1);
                    let v2 = Self::load_reg(&mut builder, regs_ptr, *src2);
                    let res = builder.ins().isub(v1, v2);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::And { dst, src1, src2 } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *src1);
                    let v2 = Self::load_reg(&mut builder, regs_ptr, *src2);
                    let res = builder.ins().band(v1, v2);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::Or { dst, src1, src2 } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *src1);
                    let v2 = Self::load_reg(&mut builder, regs_ptr, *src2);
                    let res = builder.ins().bor(v1, v2);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::Xor { dst, src1, src2 } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *src1);
                    let v2 = Self::load_reg(&mut builder, regs_ptr, *src2);
                    let res = builder.ins().bxor(v1, v2);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::AddImm { dst, src, imm } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *src);
                    let v2 = builder.ins().iconst(types::I64, *imm);
                    let res = builder.ins().iadd(v1, v2);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::MovImm { dst, imm } => {
                    let val = builder.ins().iconst(types::I64, *imm as i64);
                    Self::store_reg(&mut builder, regs_ptr, *dst, val);
                }
                IROp::Load { dst, base, offset, size, flags: _ } => {
                    let b = Self::load_reg(&mut builder, regs_ptr, *base);
                    let off = builder.ins().iconst(types::I64, *offset);
                    let addr = builder.ins().iadd(b, off);
                    let sz = builder.ins().iconst(types::I8, *size as i64);
                    let call = builder.ins().call_indirect(sig_read_ref, read_fn_ptr, &[ctx_ptr, addr, sz]);
                    let val = builder.inst_results(call)[0];
                    Self::store_reg(&mut builder, regs_ptr, *dst, val);
                }
                IROp::Store { src, base, offset, size, flags: _ } => {
                    let val = Self::load_reg(&mut builder, regs_ptr, *src);
                    let b = Self::load_reg(&mut builder, regs_ptr, *base);
                    let off = builder.ins().iconst(types::I64, *offset);
                    let addr = builder.ins().iadd(b, off);
                    let sz = builder.ins().iconst(types::I8, *size as i64);
                    builder.ins().call_indirect(sig_write_ref, write_fn_ptr, &[ctx_ptr, addr, val, sz]);
                }
                IROp::Sll { dst, src, shreg } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *src);
                    let v2 = Self::load_reg(&mut builder, regs_ptr, *shreg);
                    let res = builder.ins().ishl(v1, v2);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::Srl { dst, src, shreg } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *src);
                    let v2 = Self::load_reg(&mut builder, regs_ptr, *shreg);
                    let res = builder.ins().ushr(v1, v2);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::Sra { dst, src, shreg } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *src);
                    let v2 = Self::load_reg(&mut builder, regs_ptr, *shreg);
                    let res = builder.ins().sshr(v1, v2);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::SllImm { dst, src, sh } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *src);
                    let v2 = builder.ins().iconst(types::I64, *sh as i64);
                    let res = builder.ins().ishl(v1, v2);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::SrlImm { dst, src, sh } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *src);
                    let v2 = builder.ins().iconst(types::I64, *sh as i64);
                    let res = builder.ins().ushr(v1, v2);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::SraImm { dst, src, sh } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *src);
                    let v2 = builder.ins().iconst(types::I64, *sh as i64);
                    let res = builder.ins().sshr(v1, v2);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::CmpEq { dst, lhs, rhs } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *lhs);
                    let v2 = Self::load_reg(&mut builder, regs_ptr, *rhs);
                    let cmp = builder.ins().icmp(IntCC::Equal, v1, v2);
                    let one = builder.ins().iconst(types::I64, 1);
                    let zero = builder.ins().iconst(types::I64, 0);
                    let res = builder.ins().select(cmp, one, zero);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::CmpNe { dst, lhs, rhs } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *lhs);
                    let v2 = Self::load_reg(&mut builder, regs_ptr, *rhs);
                    let cmp = builder.ins().icmp(IntCC::NotEqual, v1, v2);
                    let one = builder.ins().iconst(types::I64, 1);
                    let zero = builder.ins().iconst(types::I64, 0);
                    let res = builder.ins().select(cmp, one, zero);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::CmpLt { dst, lhs, rhs } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *lhs);
                    let v2 = Self::load_reg(&mut builder, regs_ptr, *rhs);
                    let cmp = builder.ins().icmp(IntCC::SignedLessThan, v1, v2);
                    let one = builder.ins().iconst(types::I64, 1);
                    let zero = builder.ins().iconst(types::I64, 0);
                    let res = builder.ins().select(cmp, one, zero);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::CmpLtU { dst, lhs, rhs } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *lhs);
                    let v2 = Self::load_reg(&mut builder, regs_ptr, *rhs);
                    let cmp = builder.ins().icmp(IntCC::UnsignedLessThan, v1, v2);
                    let one = builder.ins().iconst(types::I64, 1);
                    let zero = builder.ins().iconst(types::I64, 0);
                    let res = builder.ins().select(cmp, one, zero);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::CmpGe { dst, lhs, rhs } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *lhs);
                    let v2 = Self::load_reg(&mut builder, regs_ptr, *rhs);
                    let cmp = builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, v1, v2);
                    let one = builder.ins().iconst(types::I64, 1);
                    let zero = builder.ins().iconst(types::I64, 0);
                    let res = builder.ins().select(cmp, one, zero);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::CmpGeU { dst, lhs, rhs } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *lhs);
                    let v2 = Self::load_reg(&mut builder, regs_ptr, *rhs);
                    let cmp = builder.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, v1, v2);
                    let one = builder.ins().iconst(types::I64, 1);
                    let zero = builder.ins().iconst(types::I64, 0);
                    let res = builder.ins().select(cmp, one, zero);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                _ => {}
            }
        }

        let next_pc = match &block.term {
            Terminator::Jmp { target } => *target,
            _ => block.start_pc + 4,
        };
        let ret_val = builder.ins().iconst(types::I64, next_pc as i64);
        builder.ins().return_(&[ret_val]);

        builder.finalize();

        let id = self.module.declare_function(&format!("func_{}", block.start_pc), Linkage::Export, &self.ctx.func.signature).unwrap();
        self.module.define_function(id, &mut self.ctx).unwrap();
        self.module.clear_context(&mut self.ctx);
        self.module.finalize_definitions().unwrap();

        let code = self.module.get_finalized_function(id);
        self.cache.insert(block.start_pc, code);
        code
    }

    fn compile_block(&mut self, block: &IRBlock, pc: GuestAddr) -> Result<*const u8, String> {
        self.ctx.func.signature.params.clear();
        self.ctx.func.signature.returns.clear();
        
        // Params: ctx_ptr, regs_ptr
        self.ctx.func.signature.params.push(AbiParam::new(types::I64));
        self.ctx.func.signature.params.push(AbiParam::new(types::I64));
        // Returns: next_pc
        self.ctx.func.signature.returns.push(AbiParam::new(types::I64));

        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let ctx_ptr = builder.block_params(entry_block)[0];
        let regs_ptr = builder.block_params(entry_block)[1];

        let mut var_map = HashMap::new();
        
        // Helper to get reg value
        let mut get_reg = |b: &mut FunctionBuilder, r: u32| -> Value {
            if r == 0 { return b.ins().iconst(types::I64, 0); }
            if let Some(v) = var_map.get(&r) { return *v; }
            let offset = (r as i32) * 8;
            let v = b.ins().load(types::I64, MemFlags::new(), regs_ptr, offset);
            var_map.insert(r, v);
            v
        };

        // Helper to set reg value
        let mut set_reg = |b: &mut FunctionBuilder, r: u32, v: Value| {
            if r == 0 { return; }
            var_map.insert(r, v);
            let offset = (r as i32) * 8;
            b.ins().store(MemFlags::new(), v, regs_ptr, offset);
        };

        for op in &block.ops {
            match op {
                IROp::Add { dst, src1, src2 } => {
                    let v1 = get_reg(&mut builder, *src1);
                    let v2 = get_reg(&mut builder, *src2);
                    let res = builder.ins().iadd(v1, v2);
                    set_reg(&mut builder, *dst, res);
                }
                IROp::Sub { dst, src1, src2 } => {
                    let v1 = get_reg(&mut builder, *src1);
                    let v2 = get_reg(&mut builder, *src2);
                    let res = builder.ins().isub(v1, v2);
                    set_reg(&mut builder, *dst, res);
                }
                IROp::Mul { dst, src1, src2 } => {
                    let v1 = get_reg(&mut builder, *src1);
                    let v2 = get_reg(&mut builder, *src2);
                    let res = builder.ins().imul(v1, v2);
                    set_reg(&mut builder, *dst, res);
                }
                IROp::AddImm { dst, src, imm } => {
                    let v1 = get_reg(&mut builder, *src);
                    let res = builder.ins().iadd_imm(v1, *imm);
                    set_reg(&mut builder, *dst, res);
                }
                IROp::MovImm { dst, imm } => {
                    let res = builder.ins().iconst(types::I64, *imm as i64);
                    set_reg(&mut builder, *dst, res);
                }
                IROp::Load { dst, base, offset, size, .. } => {
                    let b = get_reg(&mut builder, *base);
                    let addr = builder.ins().iadd_imm(b, *offset);
                    
                    // Call jit_read(ctx, addr, size)
                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64));
                    sig.params.push(AbiParam::new(types::I64));
                    sig.params.push(AbiParam::new(types::I8));
                    sig.returns.push(AbiParam::new(types::I64));
                    
                    let callee = self.module.declarations().get_name("jit_read").unwrap();
                    let local_callee = self.module.declare_func_in_func(callee, builder.func);
                    
                    let size_val = builder.ins().iconst(types::I8, *size as i64);
                    let call = builder.ins().call(local_callee, &[ctx_ptr, addr, size_val]);
                    let res = builder.inst_results(call)[0];
                    set_reg(&mut builder, *dst, res);
                }
                IROp::Store { src, base, offset, size, .. } => {
                    let val = get_reg(&mut builder, *src);
                    let b = get_reg(&mut builder, *base);
                    let addr = builder.ins().iadd_imm(b, *offset);

                    // Call jit_write(ctx, addr, val, size)
                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64));
                    sig.params.push(AbiParam::new(types::I64));
                    sig.params.push(AbiParam::new(types::I64));
                    sig.params.push(AbiParam::new(types::I8));
                    
                    let callee = self.module.declarations().get_name("jit_write").unwrap();
                    let local_callee = self.module.declare_func_in_func(callee, builder.func);
                    
                    let size_val = builder.ins().iconst(types::I8, *size as i64);
                    builder.ins().call(local_callee, &[ctx_ptr, addr, val, size_val]);
                }
                // ... Implement other ops ...
                _ => {}
            }
        }

        // Terminator
        let next_pc = match block.terminator {
            Terminator::Jmp { target } => builder.ins().iconst(types::I64, target as i64),
            Terminator::CondJmp { cond, target_true, target_false } => {
                let c = get_reg(&mut builder, cond);
                let t = builder.ins().iconst(types::I64, target_true as i64);
                let f = builder.ins().iconst(types::I64, target_false as i64);
                builder.ins().select(c, t, f)
            }
            _ => builder.ins().iconst(types::I64, (pc + 4) as i64), // Fallback
        };
        
        builder.ins().return_(&[next_pc]);
        builder.finalize();

        let id = self.module.declare_function(&format!("block_{}", pc), Linkage::Export, &self.ctx.func.signature).map_err(|e| e.to_string())?;
        self.module.define_function(id, &mut self.ctx).map_err(|e| e.to_string())?;
        self.module.clear_context(&mut self.ctx);
        self.module.finalize_definitions().unwrap();
        
        let code = self.module.get_finalized_function(id);
        Ok(code)
    }
}

impl ExecutionEngine<IRBlock> for Jit {
    fn run(&mut self, mmu: &mut dyn MMU, block: &IRBlock) -> ExecResult {
        // Simple hotness check
        // let count = self.hot_counts.entry(0).or_insert(0); // Need PC here, but interface doesn't pass it easily. 
        // Assuming block generation passes PC or we change interface. 
        // For now, just compile everything.
        
        // We need a way to map block to PC or cache key. 
        // Since ExecutionEngine trait takes &IRBlock, we might need to hash it or change trait.
        // But wait, the user code calls decode then run.
        // Let's assume we compile every time for this demo or use a hacky cache key.
        
        // Actually, to properly support JIT, we should probably change the ExecutionEngine trait 
        // or have the JIT manage the decoding loop itself.
        // For this step, I'll implement a basic compile-and-run.
        
        let pc = 0; // Placeholder
        
        let func_ptr = if let Some(ptr) = self.cache.get(&pc) {
            *ptr
        } else {
            match self.compile_block(block, pc) {
                Ok(ptr) => {
                    // self.cache.insert(pc, ptr); // Can't cache with 0 PC
                    ptr
                }
                Err(e) => {
                    println!("JIT Compile Error: {}", e);
                    return ExecResult { status: ExecStatus::Fault(vm_core::Fault::InvalidOpcode), stats: ExecStats { executed_ops: 0 } };
                }
            }
        };

        let mut ctx = JitContext { mmu };
        let run_fn: extern "C" fn(*mut JitContext, *mut u64) -> u64 = unsafe { std::mem::transmute(func_ptr) };
        
        let next_pc = run_fn(&mut ctx as *mut _, self.regs.as_mut_ptr());
        
        ExecResult { status: ExecStatus::Ok, stats: ExecStats { executed_ops: block.ops.len() as u64 } }
    }
}
