use vm_core::{ExecutionEngine, ExecResult, ExecStatus, ExecStats, MMU, GuestAddr, AccessType};
use vm_ir::{IRBlock, IROp, Terminator, AtomicOp, MemOrder};
use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module, FuncOrDataId};
use cranelift_native;
use cranelift::codegen::settings::{self, Configurable};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

mod advanced_ops;

/// Hot threshold for JIT compilation (execute count before compiling)
pub const HOT_THRESHOLD: u64 = 100;

/// Helper to create ExecStats with all required fields
fn make_stats(executed_ops: u64) -> ExecStats {
    ExecStats {
        executed_insns: executed_ops,
        executed_ops,
        tlb_hits: 0,
        tlb_misses: 0,
        jit_compiles: 0,
        jit_compile_time_ns: 0,
    }
}

/// Helper to create ExecResult
fn make_result(status: ExecStatus, executed_ops: u64, next_pc: GuestAddr) -> ExecResult {
    ExecResult {
        status,
        stats: make_stats(executed_ops),
        next_pc,
    }
}

/// JIT context passed to runtime helpers
pub struct JitContext<'a> {
    pub mmu: &'a mut dyn MMU,
}

/// Block execution statistics for hotspot tracking
#[derive(Default, Clone)]
pub struct BlockStats {
    pub exec_count: u64,
    pub is_compiled: bool,
}

unsafe extern "C" fn jit_read(ctx: *mut JitContext, addr: u64, size: u8) -> u64 {
    (*ctx).mmu.read(addr, size).unwrap_or(0)
}

unsafe extern "C" fn jit_write(ctx: *mut JitContext, addr: u64, val: u64, size: u8) {
    let _ = (*ctx).mmu.write(addr, val, size);
}

/// SIMD vector addition helper (element-wise)
unsafe extern "C" fn jit_vec_add(dst: *mut u64, src1: *const u64, src2: *const u64, element_size: u8) {
    let es = element_size as u64;
    let lane_bits = es * 8;
    let lanes = 64 / lane_bits;
    let mask = ((1u128 << lane_bits) - 1) as u64;
    
    let a = *src1;
    let b = *src2;
    let mut acc = 0u64;
    
    for i in 0..lanes {
        let shift = i * lane_bits;
        let av = (a >> shift) & mask;
        let bv = (b >> shift) & mask;
        let rv = av.wrapping_add(bv) & mask;
        acc |= rv << shift;
    }
    *dst = acc;
}

/// SIMD vector subtraction helper
unsafe extern "C" fn jit_vec_sub(dst: *mut u64, src1: *const u64, src2: *const u64, element_size: u8) {
    let es = element_size as u64;
    let lane_bits = es * 8;
    let lanes = 64 / lane_bits;
    let mask = ((1u128 << lane_bits) - 1) as u64;
    
    let a = *src1;
    let b = *src2;
    let mut acc = 0u64;
    
    for i in 0..lanes {
        let shift = i * lane_bits;
        let av = (a >> shift) & mask;
        let bv = (b >> shift) & mask;
        let rv = av.wrapping_sub(bv) & mask;
        acc |= rv << shift;
    }
    *dst = acc;
}

/// SIMD vector multiplication helper
unsafe extern "C" fn jit_vec_mul(dst: *mut u64, src1: *const u64, src2: *const u64, element_size: u8) {
    let es = element_size as u64;
    let lane_bits = es * 8;
    let lanes = 64 / lane_bits;
    let mask = ((1u128 << lane_bits) - 1) as u64;
    
    let a = *src1;
    let b = *src2;
    let mut acc = 0u64;
    
    for i in 0..lanes {
        let shift = i * lane_bits;
        let av = (a >> shift) & mask;
        let bv = (b >> shift) & mask;
        let rv = av.wrapping_mul(bv) & mask;
        acc |= rv << shift;
    }
    *dst = acc;
}

/// Atomic RMW helper - returns old value
unsafe extern "C" fn jit_atomic_rmw(ctx: *mut JitContext, addr: u64, val: u64, op: u8, size: u8) -> u64 {
    // Read current value
    let old = (*ctx).mmu.read(addr, size).unwrap_or(0);
    
    // Compute new value based on operation
    let new = match op {
        0 => old.wrapping_add(val), // Add
        1 => old.wrapping_sub(val), // Sub
        2 => old & val,              // And
        3 => old | val,              // Or
        4 => old ^ val,              // Xor
        5 => val,                    // Xchg
        6 => old.min(val),           // MinU
        7 => old.max(val),           // MaxU
        _ => old,
    };
    
    let _ = (*ctx).mmu.write(addr, new, size);
    old
}

/// Memory fence helper
unsafe extern "C" fn jit_fence(order: u8) {
    match order {
        1 => std::sync::atomic::fence(Ordering::Acquire),
        2 => std::sync::atomic::fence(Ordering::Release),
        3 => std::sync::atomic::fence(Ordering::AcqRel),
        4 => std::sync::atomic::fence(Ordering::SeqCst),
        _ => {}
    }
}

/// Wrapper for JIT code pointer that implements Send
/// SAFETY: JIT code pointers are safe to send between threads once generated
/// because the underlying code is immutable after compilation.
#[derive(Clone, Copy)]
struct CodePtr(*const u8);
unsafe impl Send for CodePtr {}

pub struct Jit {
    builder_context: FunctionBuilderContext,
    ctx: codegen::Context,
    module: JITModule,
    cache: HashMap<GuestAddr, CodePtr>,
    hot_counts: HashMap<GuestAddr, BlockStats>,
    pub regs: [u64; 32],
    pub pc: GuestAddr,
    /// Vector registers for SIMD (128-bit represented as two u64s)
    pub vec_regs: [[u64; 2]; 32],
    /// Statistics
    pub total_compiled: u64,
    pub total_interpreted: u64,
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
        builder.symbol("jit_vec_add", jit_vec_add as *const u8);
        builder.symbol("jit_vec_sub", jit_vec_sub as *const u8);
        builder.symbol("jit_vec_mul", jit_vec_mul as *const u8);
        builder.symbol("jit_atomic_rmw", jit_atomic_rmw as *const u8);
        builder.symbol("jit_fence", jit_fence as *const u8);
        let module = JITModule::new(builder);
        Self {
            builder_context: FunctionBuilderContext::new(),
            ctx: module.make_context(),
            module,
            cache: HashMap::new(),
            hot_counts: HashMap::new(),
            regs: [0; 32],
            pc: 0,
            vec_regs: [[0; 2]; 32],
            total_compiled: 0,
            total_interpreted: 0,
        }
    }
    
    /// Check if a block is hot enough to compile
    pub fn is_hot(&self, pc: GuestAddr) -> bool {
        self.hot_counts.get(&pc)
            .map(|s| s.exec_count >= HOT_THRESHOLD)
            .unwrap_or(false)
    }
    
    /// Increment execution count and return whether to compile
    pub fn record_execution(&mut self, pc: GuestAddr) -> bool {
        let stats = self.hot_counts.entry(pc).or_default();
        stats.exec_count += 1;
        if stats.exec_count >= HOT_THRESHOLD && !stats.is_compiled {
            stats.is_compiled = true;
            true
        } else {
            false
        }
    }
    
    /// Get execution statistics
    pub fn get_stats(&self, pc: GuestAddr) -> Option<&BlockStats> {
        self.hot_counts.get(&pc)
    }

    /// Load a register value into a Cranelift IR value
    fn load_reg(builder: &mut FunctionBuilder, regs_ptr: Value, idx: u32) -> Value {
        if idx == 0 {
            // x0 is always zero in RISC-V
            builder.ins().iconst(types::I64, 0)
        } else {
            let offset = (idx as i32) * 8;
            builder.ins().load(types::I64, MemFlags::trusted(), regs_ptr, offset)
        }
    }

    /// Store a value into a register
    fn store_reg(builder: &mut FunctionBuilder, regs_ptr: Value, idx: u32, val: Value) {
        if idx != 0 {
            let offset = (idx as i32) * 8;
            builder.ins().store(MemFlags::trusted(), val, regs_ptr, offset);
        }
    }

    fn compile(&mut self, block: &IRBlock) -> *const u8 {
        if let Some(&ptr) = self.cache.get(&block.start_pc) {
            return ptr.0;
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
                IROp::Mul { dst, src1, src2 } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *src1);
                    let v2 = Self::load_reg(&mut builder, regs_ptr, *src2);
                    let res = builder.ins().imul(v1, v2);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::Div { dst, src1, src2, signed } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *src1);
                    let v2 = Self::load_reg(&mut builder, regs_ptr, *src2);
                    let res = if *signed {
                        builder.ins().sdiv(v1, v2)
                    } else {
                        builder.ins().udiv(v1, v2)
                    };
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::Rem { dst, src1, src2, signed } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *src1);
                    let v2 = Self::load_reg(&mut builder, regs_ptr, *src2);
                    let res = if *signed {
                        builder.ins().srem(v1, v2)
                    } else {
                        builder.ins().urem(v1, v2)
                    };
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::AtomicRMW { dst, base, src, op, size } => {
                    // For now, use helper function for atomic operations
                    let addr = Self::load_reg(&mut builder, regs_ptr, *base);
                    let val = Self::load_reg(&mut builder, regs_ptr, *src);
                    let op_code = builder.ins().iconst(types::I32, *op as i64);
                    let sz = builder.ins().iconst(types::I32, *size as i64);
                    
                    // Call atomic RMW helper
                    let mut sig_atomic = self.module.make_signature();
                    sig_atomic.params.push(AbiParam::new(types::I64)); // ctx
                    sig_atomic.params.push(AbiParam::new(types::I64)); // addr
                    sig_atomic.params.push(AbiParam::new(types::I64)); // val
                    sig_atomic.params.push(AbiParam::new(types::I32)); // op
                    sig_atomic.params.push(AbiParam::new(types::I32)); // size
                    sig_atomic.returns.push(AbiParam::new(types::I64));
                    let sig_atomic_ref = builder.import_signature(sig_atomic);
                    
                    let atomic_fn = builder.ins().iconst(types::I64, jit_atomic_rmw as i64);
                    let call = builder.ins().call_indirect(sig_atomic_ref, atomic_fn, &[ctx_ptr, addr, val, op_code, sz]);
                    let old_val = builder.inst_results(call)[0];
                    Self::store_reg(&mut builder, regs_ptr, *dst, old_val);
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
        self.cache.insert(block.start_pc, CodePtr(code));
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

        for op in &block.ops {
            match op {
                IROp::Add { dst, src1, src2 } => {
                    let v1 = Self::get_reg_value(&mut builder, regs_ptr, *src1, &mut var_map);
                    let v2 = Self::get_reg_value(&mut builder, regs_ptr, *src2, &mut var_map);
                    let res = builder.ins().iadd(v1, v2);
                    Self::set_reg_value(&mut builder, regs_ptr, *dst, res, &mut var_map);
                }
                IROp::Sub { dst, src1, src2 } => {
                    let v1 = Self::get_reg_value(&mut builder, regs_ptr, *src1, &mut var_map);
                    let v2 = Self::get_reg_value(&mut builder, regs_ptr, *src2, &mut var_map);
                    let res = builder.ins().isub(v1, v2);
                    Self::set_reg_value(&mut builder, regs_ptr, *dst, res, &mut var_map);
                }
                IROp::Mul { dst, src1, src2 } => {
                    let v1 = Self::get_reg_value(&mut builder, regs_ptr, *src1, &mut var_map);
                    let v2 = Self::get_reg_value(&mut builder, regs_ptr, *src2, &mut var_map);
                    let res = builder.ins().imul(v1, v2);
                    Self::set_reg_value(&mut builder, regs_ptr, *dst, res, &mut var_map);
                }
                IROp::AddImm { dst, src, imm } => {
                    let v1 = Self::get_reg_value(&mut builder, regs_ptr, *src, &mut var_map);
                    let res = builder.ins().iadd_imm(v1, *imm);
                    Self::set_reg_value(&mut builder, regs_ptr, *dst, res, &mut var_map);
                }
                IROp::MovImm { dst, imm } => {
                    let res = builder.ins().iconst(types::I64, *imm as i64);
                    Self::set_reg_value(&mut builder, regs_ptr, *dst, res, &mut var_map);
                }
                IROp::Load { dst, base, offset, size, .. } => {
                    let b = Self::get_reg_value(&mut builder, regs_ptr, *base, &mut var_map);
                    let addr = builder.ins().iadd_imm(b, *offset);
                    
                    // Call jit_read(ctx, addr, size)
                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64));
                    sig.params.push(AbiParam::new(types::I64));
                    sig.params.push(AbiParam::new(types::I8));
                    sig.returns.push(AbiParam::new(types::I64));
                    
                    let callee = match self.module.declarations().get_name("jit_read") {
                        Some(FuncOrDataId::Func(id)) => id,
                        _ => panic!("jit_read not found"),
                    };
                    let local_callee = self.module.declare_func_in_func(callee, builder.func);
                    
                    let size_val = builder.ins().iconst(types::I8, *size as i64);
                    let call = builder.ins().call(local_callee, &[ctx_ptr, addr, size_val]);
                    let res = builder.inst_results(call)[0];
                    Self::set_reg_value(&mut builder, regs_ptr, *dst, res, &mut var_map);
                }
                IROp::Store { src, base, offset, size, .. } => {
                    let val = Self::get_reg_value(&mut builder, regs_ptr, *src, &mut var_map);
                    let b = Self::get_reg_value(&mut builder, regs_ptr, *base, &mut var_map);
                    let addr = builder.ins().iadd_imm(b, *offset);

                    // Call jit_write(ctx, addr, val, size)
                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64));
                    sig.params.push(AbiParam::new(types::I64));
                    sig.params.push(AbiParam::new(types::I64));
                    sig.params.push(AbiParam::new(types::I8));
                    
                    let callee = match self.module.declarations().get_name("jit_write") {
                        Some(FuncOrDataId::Func(id)) => id,
                        _ => panic!("jit_write not found"),
                    };
                    let local_callee = self.module.declare_func_in_func(callee, builder.func);
                    
                    let size_val = builder.ins().iconst(types::I8, *size as i64);
                    builder.ins().call(local_callee, &[ctx_ptr, addr, val, size_val]);
                }
                IROp::And { dst, src1, src2 } => {
                    let v1 = Self::get_reg_value(&mut builder, regs_ptr, *src1, &mut var_map);
                    let v2 = Self::get_reg_value(&mut builder, regs_ptr, *src2, &mut var_map);
                    let res = builder.ins().band(v1, v2);
                    Self::set_reg_value(&mut builder, regs_ptr, *dst, res, &mut var_map);
                }
                IROp::Or { dst, src1, src2 } => {
                    let v1 = Self::get_reg_value(&mut builder, regs_ptr, *src1, &mut var_map);
                    let v2 = Self::get_reg_value(&mut builder, regs_ptr, *src2, &mut var_map);
                    let res = builder.ins().bor(v1, v2);
                    Self::set_reg_value(&mut builder, regs_ptr, *dst, res, &mut var_map);
                }
                IROp::Xor { dst, src1, src2 } => {
                    let v1 = Self::get_reg_value(&mut builder, regs_ptr, *src1, &mut var_map);
                    let v2 = Self::get_reg_value(&mut builder, regs_ptr, *src2, &mut var_map);
                    let res = builder.ins().bxor(v1, v2);
                    Self::set_reg_value(&mut builder, regs_ptr, *dst, res, &mut var_map);
                }
                IROp::Not { dst, src } => {
                    let v1 = Self::get_reg_value(&mut builder, regs_ptr, *src, &mut var_map);
                    let res = builder.ins().bnot(v1);
                    Self::set_reg_value(&mut builder, regs_ptr, *dst, res, &mut var_map);
                }
                IROp::Sll { dst, src, shreg } => {
                    let v1 = Self::get_reg_value(&mut builder, regs_ptr, *src, &mut var_map);
                    let v2 = Self::get_reg_value(&mut builder, regs_ptr, *shreg, &mut var_map);
                    let res = builder.ins().ishl(v1, v2);
                    Self::set_reg_value(&mut builder, regs_ptr, *dst, res, &mut var_map);
                }
                IROp::Srl { dst, src, shreg } => {
                    let v1 = Self::get_reg_value(&mut builder, regs_ptr, *src, &mut var_map);
                    let v2 = Self::get_reg_value(&mut builder, regs_ptr, *shreg, &mut var_map);
                    let res = builder.ins().ushr(v1, v2);
                    Self::set_reg_value(&mut builder, regs_ptr, *dst, res, &mut var_map);
                }
                IROp::Sra { dst, src, shreg } => {
                    let v1 = Self::get_reg_value(&mut builder, regs_ptr, *src, &mut var_map);
                    let v2 = Self::get_reg_value(&mut builder, regs_ptr, *shreg, &mut var_map);
                    let res = builder.ins().sshr(v1, v2);
                    Self::set_reg_value(&mut builder, regs_ptr, *dst, res, &mut var_map);
                }
                IROp::SllImm { dst, src, sh } => {
                    let v1 = Self::get_reg_value(&mut builder, regs_ptr, *src, &mut var_map);
                    let v2 = builder.ins().iconst(types::I64, *sh as i64);
                    let res = builder.ins().ishl(v1, v2);
                    Self::set_reg_value(&mut builder, regs_ptr, *dst, res, &mut var_map);
                }
                IROp::SrlImm { dst, src, sh } => {
                    let v1 = Self::get_reg_value(&mut builder, regs_ptr, *src, &mut var_map);
                    let v2 = builder.ins().iconst(types::I64, *sh as i64);
                    let res = builder.ins().ushr(v1, v2);
                    Self::set_reg_value(&mut builder, regs_ptr, *dst, res, &mut var_map);
                }
                IROp::SraImm { dst, src, sh } => {
                    let v1 = Self::get_reg_value(&mut builder, regs_ptr, *src, &mut var_map);
                    let v2 = builder.ins().iconst(types::I64, *sh as i64);
                    let res = builder.ins().sshr(v1, v2);
                    Self::set_reg_value(&mut builder, regs_ptr, *dst, res, &mut var_map);
                }
                IROp::Div { dst, src1, src2, signed } => {
                    let v1 = Self::get_reg_value(&mut builder, regs_ptr, *src1, &mut var_map);
                    let v2 = Self::get_reg_value(&mut builder, regs_ptr, *src2, &mut var_map);
                    let res = if *signed {
                        builder.ins().sdiv(v1, v2)
                    } else {
                        builder.ins().udiv(v1, v2)
                    };
                    Self::set_reg_value(&mut builder, regs_ptr, *dst, res, &mut var_map);
                }
                IROp::Rem { dst, src1, src2, signed } => {
                    let v1 = Self::get_reg_value(&mut builder, regs_ptr, *src1, &mut var_map);
                    let v2 = Self::get_reg_value(&mut builder, regs_ptr, *src2, &mut var_map);
                    let res = if *signed {
                        builder.ins().srem(v1, v2)
                    } else {
                        builder.ins().urem(v1, v2)
                    };
                    Self::set_reg_value(&mut builder, regs_ptr, *dst, res, &mut var_map);
                }
                IROp::CmpEq { dst, lhs, rhs } => {
                    let v1 = Self::get_reg_value(&mut builder, regs_ptr, *lhs, &mut var_map);
                    let v2 = Self::get_reg_value(&mut builder, regs_ptr, *rhs, &mut var_map);
                    let cmp = builder.ins().icmp(IntCC::Equal, v1, v2);
                    let one = builder.ins().iconst(types::I64, 1);
                    let zero = builder.ins().iconst(types::I64, 0);
                    let res = builder.ins().select(cmp, one, zero);
                    Self::set_reg_value(&mut builder, regs_ptr, *dst, res, &mut var_map);
                }
                IROp::CmpNe { dst, lhs, rhs } => {
                    let v1 = Self::get_reg_value(&mut builder, regs_ptr, *lhs, &mut var_map);
                    let v2 = Self::get_reg_value(&mut builder, regs_ptr, *rhs, &mut var_map);
                    let cmp = builder.ins().icmp(IntCC::NotEqual, v1, v2);
                    let one = builder.ins().iconst(types::I64, 1);
                    let zero = builder.ins().iconst(types::I64, 0);
                    let res = builder.ins().select(cmp, one, zero);
                    Self::set_reg_value(&mut builder, regs_ptr, *dst, res, &mut var_map);
                }
                IROp::CmpLt { dst, lhs, rhs } => {
                    let v1 = Self::get_reg_value(&mut builder, regs_ptr, *lhs, &mut var_map);
                    let v2 = Self::get_reg_value(&mut builder, regs_ptr, *rhs, &mut var_map);
                    let cmp = builder.ins().icmp(IntCC::SignedLessThan, v1, v2);
                    let one = builder.ins().iconst(types::I64, 1);
                    let zero = builder.ins().iconst(types::I64, 0);
                    let res = builder.ins().select(cmp, one, zero);
                    Self::set_reg_value(&mut builder, regs_ptr, *dst, res, &mut var_map);
                }
                IROp::CmpLtU { dst, lhs, rhs } => {
                    let v1 = Self::get_reg_value(&mut builder, regs_ptr, *lhs, &mut var_map);
                    let v2 = Self::get_reg_value(&mut builder, regs_ptr, *rhs, &mut var_map);
                    let cmp = builder.ins().icmp(IntCC::UnsignedLessThan, v1, v2);
                    let one = builder.ins().iconst(types::I64, 1);
                    let zero = builder.ins().iconst(types::I64, 0);
                    let res = builder.ins().select(cmp, one, zero);
                    Self::set_reg_value(&mut builder, regs_ptr, *dst, res, &mut var_map);
                }
                IROp::CmpGe { dst, lhs, rhs } => {
                    let v1 = Self::get_reg_value(&mut builder, regs_ptr, *lhs, &mut var_map);
                    let v2 = Self::get_reg_value(&mut builder, regs_ptr, *rhs, &mut var_map);
                    let cmp = builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, v1, v2);
                    let one = builder.ins().iconst(types::I64, 1);
                    let zero = builder.ins().iconst(types::I64, 0);
                    let res = builder.ins().select(cmp, one, zero);
                    Self::set_reg_value(&mut builder, regs_ptr, *dst, res, &mut var_map);
                }
                IROp::CmpGeU { dst, lhs, rhs } => {
                    let v1 = Self::get_reg_value(&mut builder, regs_ptr, *lhs, &mut var_map);
                    let v2 = Self::get_reg_value(&mut builder, regs_ptr, *rhs, &mut var_map);
                    let cmp = builder.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, v1, v2);
                    let one = builder.ins().iconst(types::I64, 1);
                    let zero = builder.ins().iconst(types::I64, 0);
                    let res = builder.ins().select(cmp, one, zero);
                    Self::set_reg_value(&mut builder, regs_ptr, *dst, res, &mut var_map);
                }
                IROp::Select { dst, cond, true_val, false_val } => {
                    let c = Self::get_reg_value(&mut builder, regs_ptr, *cond, &mut var_map);
                    let t = Self::get_reg_value(&mut builder, regs_ptr, *true_val, &mut var_map);
                    let f = Self::get_reg_value(&mut builder, regs_ptr, *false_val, &mut var_map);
                    let zero = builder.ins().iconst(types::I64, 0);
                    let cond_bool = builder.ins().icmp(IntCC::NotEqual, c, zero);
                    let res = builder.ins().select(cond_bool, t, f);
                    Self::set_reg_value(&mut builder, regs_ptr, *dst, res, &mut var_map);
                }
                IROp::MulImm { dst, src, imm } => {
                    let v1 = Self::get_reg_value(&mut builder, regs_ptr, *src, &mut var_map);
                    let v2 = builder.ins().iconst(types::I64, *imm);
                    let res = builder.ins().imul(v1, v2);
                    Self::set_reg_value(&mut builder, regs_ptr, *dst, res, &mut var_map);
                }
                // SIMD operations - use helper functions via call
                IROp::VecAdd { dst, src1, src2, element_size } => {
                    // For SIMD, we store result on stack and call helper
                    let v1 = Self::get_reg_value(&mut builder, regs_ptr, *src1, &mut var_map);
                    let v2 = Self::get_reg_value(&mut builder, regs_ptr, *src2, &mut var_map);
                    // Simple lane-wise add emulated in scalar
                    let es = *element_size as u64;
                    let lane_bits = es * 8;
                    let lanes = 64 / lane_bits;
                    let mask = ((1u128 << lane_bits) - 1) as i64;
                    
                    let mut acc = builder.ins().iconst(types::I64, 0);
                    for i in 0..lanes {
                        let shift = builder.ins().iconst(types::I64, (i * lane_bits) as i64);
                        let mask_val = builder.ins().iconst(types::I64, mask);
                        let av = builder.ins().ushr(v1, shift);
                        let av = builder.ins().band(av, mask_val);
                        let bv = builder.ins().ushr(v2, shift);
                        let bv = builder.ins().band(bv, mask_val);
                        let rv = builder.ins().iadd(av, bv);
                        let rv = builder.ins().band(rv, mask_val);
                        let rv_shifted = builder.ins().ishl(rv, shift);
                        acc = builder.ins().bor(acc, rv_shifted);
                    }
                    Self::set_reg_value(&mut builder, regs_ptr, *dst, acc, &mut var_map);
                }
                IROp::VecSub { dst, src1, src2, element_size } => {
                    let v1 = Self::get_reg_value(&mut builder, regs_ptr, *src1, &mut var_map);
                    let v2 = Self::get_reg_value(&mut builder, regs_ptr, *src2, &mut var_map);
                    let es = *element_size as u64;
                    let lane_bits = es * 8;
                    let lanes = 64 / lane_bits;
                    let mask = ((1u128 << lane_bits) - 1) as i64;
                    
                    let mut acc = builder.ins().iconst(types::I64, 0);
                    for i in 0..lanes {
                        let shift = builder.ins().iconst(types::I64, (i * lane_bits) as i64);
                        let mask_val = builder.ins().iconst(types::I64, mask);
                        let av = builder.ins().ushr(v1, shift);
                        let av = builder.ins().band(av, mask_val);
                        let bv = builder.ins().ushr(v2, shift);
                        let bv = builder.ins().band(bv, mask_val);
                        let rv = builder.ins().isub(av, bv);
                        let rv = builder.ins().band(rv, mask_val);
                        let rv_shifted = builder.ins().ishl(rv, shift);
                        acc = builder.ins().bor(acc, rv_shifted);
                    }
                    Self::set_reg_value(&mut builder, regs_ptr, *dst, acc, &mut var_map);
                }
                IROp::VecMul { dst, src1, src2, element_size } => {
                    let v1 = Self::get_reg_value(&mut builder, regs_ptr, *src1, &mut var_map);
                    let v2 = Self::get_reg_value(&mut builder, regs_ptr, *src2, &mut var_map);
                    let es = *element_size as u64;
                    let lane_bits = es * 8;
                    let lanes = 64 / lane_bits;
                    let mask = ((1u128 << lane_bits) - 1) as i64;
                    
                    let mut acc = builder.ins().iconst(types::I64, 0);
                    for i in 0..lanes {
                        let shift = builder.ins().iconst(types::I64, (i * lane_bits) as i64);
                        let mask_val = builder.ins().iconst(types::I64, mask);
                        let av = builder.ins().ushr(v1, shift);
                        let av = builder.ins().band(av, mask_val);
                        let bv = builder.ins().ushr(v2, shift);
                        let bv = builder.ins().band(bv, mask_val);
                        let rv = builder.ins().imul(av, bv);
                        let rv = builder.ins().band(rv, mask_val);
                        let rv_shifted = builder.ins().ishl(rv, shift);
                        acc = builder.ins().bor(acc, rv_shifted);
                    }
                    Self::set_reg_value(&mut builder, regs_ptr, *dst, acc, &mut var_map);
                }
                IROp::Nop => {}
                IROp::SysCall | IROp::DebugBreak | IROp::TlbFlush { .. } => {
                    // These will cause early exit in actual implementation
                }
                _ => {
                    // Fallback for unimplemented ops - NOP
                }
            }
        }

        // Terminator
        let next_pc = match &block.term {
            Terminator::Jmp { target } => builder.ins().iconst(types::I64, *target as i64),
            Terminator::CondJmp { cond, target_true, target_false } => {
                let c = Self::get_reg_value(&mut builder, regs_ptr, *cond, &mut var_map);
                let t = builder.ins().iconst(types::I64, *target_true as i64);
                let f = builder.ins().iconst(types::I64, *target_false as i64);
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
        let pc = block.start_pc;
        
        // Hotspot tracking: record execution and check if should compile
        let should_compile = self.record_execution(pc);
        
        // Check if we have cached compiled code
        let func_ptr = if let Some(&ptr) = self.cache.get(&pc) {
            self.total_compiled += 1;
            ptr.0
        } else if should_compile {
            // Hot block - compile it
            match self.compile_block_full(block, pc) {
                Ok(ptr) => {
                    self.cache.insert(pc, CodePtr(ptr));
                    self.total_compiled += 1;
                    ptr
                }
                Err(e) => {
                    eprintln!("JIT Compile Error at {:#x}: {}", pc, e);
                    // Fall back to interpreter path
                    self.total_interpreted += 1;
                    return self.interpret_block(mmu, block);
                }
            }
        } else {
            // Cold block - interpret
            self.total_interpreted += 1;
            return self.interpret_block(mmu, block);
        };

        // Execute compiled code
        let mut ctx = JitContext { mmu };
        let run_fn: extern "C" fn(*mut JitContext, *mut u64) -> u64 = 
            unsafe { std::mem::transmute(func_ptr) };
        
        let next_pc = run_fn(&mut ctx as *mut _, self.regs.as_mut_ptr());
        self.pc = next_pc;
        
        ExecResult { 
            status: ExecStatus::Ok, 
            stats: ExecStats {
                executed_insns: block.ops.len() as u64,
                executed_ops: block.ops.len() as u64,
                tlb_hits: 0,
                tlb_misses: 0,
                jit_compiles: if should_compile { 1 } else { 0 },
                jit_compile_time_ns: 0,
            },
            next_pc,
        }
    }

    fn get_reg(&self, idx: usize) -> u64 {
        if idx < 32 { self.regs[idx] } else { 0 }
    }

    fn set_reg(&mut self, idx: usize, val: u64) {
        if idx > 0 && idx < 32 { self.regs[idx] = val; }
    }

    fn get_pc(&self) -> GuestAddr {
        self.pc
    }

    fn set_pc(&mut self, pc: GuestAddr) {
        self.pc = pc;
    }
}

impl Jit {
    /// Interpret a block (fallback for cold code or compilation failures)
    fn interpret_block(&mut self, mmu: &mut dyn MMU, block: &IRBlock) -> ExecResult {
        use vm_ir::IROp;
        
        for op in &block.ops {
            match op {
                IROp::Add { dst, src1, src2 } => {
                    let v = self.regs[*src1 as usize].wrapping_add(self.regs[*src2 as usize]);
                    if *dst != 0 { self.regs[*dst as usize] = v; }
                }
                IROp::Sub { dst, src1, src2 } => {
                    let v = self.regs[*src1 as usize].wrapping_sub(self.regs[*src2 as usize]);
                    if *dst != 0 { self.regs[*dst as usize] = v; }
                }
                IROp::Mul { dst, src1, src2 } => {
                    let v = self.regs[*src1 as usize].wrapping_mul(self.regs[*src2 as usize]);
                    if *dst != 0 { self.regs[*dst as usize] = v; }
                }
                IROp::And { dst, src1, src2 } => {
                    let v = self.regs[*src1 as usize] & self.regs[*src2 as usize];
                    if *dst != 0 { self.regs[*dst as usize] = v; }
                }
                IROp::Or { dst, src1, src2 } => {
                    let v = self.regs[*src1 as usize] | self.regs[*src2 as usize];
                    if *dst != 0 { self.regs[*dst as usize] = v; }
                }
                IROp::Xor { dst, src1, src2 } => {
                    let v = self.regs[*src1 as usize] ^ self.regs[*src2 as usize];
                    if *dst != 0 { self.regs[*dst as usize] = v; }
                }
                IROp::AddImm { dst, src, imm } => {
                    let v = self.regs[*src as usize].wrapping_add(*imm as u64);
                    if *dst != 0 { self.regs[*dst as usize] = v; }
                }
                IROp::MovImm { dst, imm } => {
                    if *dst != 0 { self.regs[*dst as usize] = *imm; }
                }
                IROp::Load { dst, base, offset, size, .. } => {
                    let addr = self.regs[*base as usize].wrapping_add(*offset as u64);
                    match mmu.read(addr, *size) {
                        Ok(v) => { if *dst != 0 { self.regs[*dst as usize] = v; } }
                        Err(e) => return make_result(ExecStatus::Fault(e), 0, block.start_pc),
                    }
                }
                IROp::Store { src, base, offset, size, .. } => {
                    let addr = self.regs[*base as usize].wrapping_add(*offset as u64);
                    let val = self.regs[*src as usize];
                    if let Err(e) = mmu.write(addr, val, *size) {
                        return make_result(ExecStatus::Fault(e), 0, block.start_pc);
                    }
                }
                _ => {} // Skip unhandled ops in simple interpreter
            }
        }
        
        // Compute next_pc based on terminator
        let next_pc = match &block.term {
            Terminator::Jmp { target } => *target,
            Terminator::JmpReg { base, offset } => self.regs[*base as usize].wrapping_add(*offset as u64),
            Terminator::CondJmp { cond, target_true, target_false } => {
                if self.regs[*cond as usize] != 0 { *target_true } else { *target_false }
            }
            Terminator::Ret => block.start_pc,
            Terminator::Fault { cause: _ } => block.start_pc,
            Terminator::Interrupt { vector: _ } => block.start_pc.wrapping_add(4),
            Terminator::Call { target, ret_pc: _ } => *target,
        };
        self.pc = next_pc;
        
        make_result(ExecStatus::Ok, block.ops.len() as u64, next_pc)
    }
    
    /// Full block compilation with all optimizations
    /// Note: Currently simplified - complex compilation disabled due to borrow checker issues
    fn compile_block_full(&mut self, block: &IRBlock, pc: GuestAddr) -> Result<*const u8, String> {
        // Use the simpler compile method for now
        // TODO: Fix borrow checker issues with compile_op
        let code = self.compile(block);
        if code.is_null() {
            Err("Compilation failed".to_string())
        } else {
            Ok(code)
        }
    }
    
    /// Compile a single IR operation (helper for full compilation path)
    /// Compile a single IR operation
    /// Note: Currently has borrow checker issues when called from compile_block_full
    #[allow(dead_code)]
    fn compile_op(
        &mut self,
        builder: &mut FunctionBuilder,
        op: &IROp,
        ctx_ptr: Value,
        regs_ptr: Value,
        var_map: &mut HashMap<u32, Value>,
    ) -> Result<(), String> {
        match op {
            IROp::Nop => {}
            IROp::Add { dst, src1, src2 } => {
                let v1 = Self::get_reg_value(builder, regs_ptr, *src1, var_map);
                let v2 = Self::get_reg_value(builder, regs_ptr, *src2, var_map);
                let res = builder.ins().iadd(v1, v2);
                Self::set_reg_value(builder, regs_ptr, *dst, res, var_map);
            }
            IROp::Sub { dst, src1, src2 } => {
                let v1 = Self::get_reg_value(builder, regs_ptr, *src1, var_map);
                let v2 = Self::get_reg_value(builder, regs_ptr, *src2, var_map);
                let res = builder.ins().isub(v1, v2);
                Self::set_reg_value(builder, regs_ptr, *dst, res, var_map);
            }
            IROp::Mul { dst, src1, src2 } => {
                let v1 = Self::get_reg_value(builder, regs_ptr, *src1, var_map);
                let v2 = Self::get_reg_value(builder, regs_ptr, *src2, var_map);
                let res = builder.ins().imul(v1, v2);
                Self::set_reg_value(builder, regs_ptr, *dst, res, var_map);
            }
            IROp::Div { dst, src1, src2, signed } => {
                let v1 = Self::get_reg_value(builder, regs_ptr, *src1, var_map);
                let v2 = Self::get_reg_value(builder, regs_ptr, *src2, var_map);
                let res = if *signed {
                    builder.ins().sdiv(v1, v2)
                } else {
                    builder.ins().udiv(v1, v2)
                };
                Self::set_reg_value(builder, regs_ptr, *dst, res, var_map);
            }
            IROp::Rem { dst, src1, src2, signed } => {
                let v1 = Self::get_reg_value(builder, regs_ptr, *src1, var_map);
                let v2 = Self::get_reg_value(builder, regs_ptr, *src2, var_map);
                let res = if *signed {
                    builder.ins().srem(v1, v2)
                } else {
                    builder.ins().urem(v1, v2)
                };
                Self::set_reg_value(builder, regs_ptr, *dst, res, var_map);
            }
            IROp::And { dst, src1, src2 } => {
                let v1 = Self::get_reg_value(builder, regs_ptr, *src1, var_map);
                let v2 = Self::get_reg_value(builder, regs_ptr, *src2, var_map);
                let res = builder.ins().band(v1, v2);
                Self::set_reg_value(builder, regs_ptr, *dst, res, var_map);
            }
            IROp::Or { dst, src1, src2 } => {
                let v1 = Self::get_reg_value(builder, regs_ptr, *src1, var_map);
                let v2 = Self::get_reg_value(builder, regs_ptr, *src2, var_map);
                let res = builder.ins().bor(v1, v2);
                Self::set_reg_value(builder, regs_ptr, *dst, res, var_map);
            }
            IROp::Xor { dst, src1, src2 } => {
                let v1 = Self::get_reg_value(builder, regs_ptr, *src1, var_map);
                let v2 = Self::get_reg_value(builder, regs_ptr, *src2, var_map);
                let res = builder.ins().bxor(v1, v2);
                Self::set_reg_value(builder, regs_ptr, *dst, res, var_map);
            }
            IROp::Not { dst, src } => {
                let v1 = Self::get_reg_value(builder, regs_ptr, *src, var_map);
                let res = builder.ins().bnot(v1);
                Self::set_reg_value(builder, regs_ptr, *dst, res, var_map);
            }
            IROp::Sll { dst, src, shreg } => {
                let v1 = Self::get_reg_value(builder, regs_ptr, *src, var_map);
                let v2 = Self::get_reg_value(builder, regs_ptr, *shreg, var_map);
                let res = builder.ins().ishl(v1, v2);
                Self::set_reg_value(builder, regs_ptr, *dst, res, var_map);
            }
            IROp::Srl { dst, src, shreg } => {
                let v1 = Self::get_reg_value(builder, regs_ptr, *src, var_map);
                let v2 = Self::get_reg_value(builder, regs_ptr, *shreg, var_map);
                let res = builder.ins().ushr(v1, v2);
                Self::set_reg_value(builder, regs_ptr, *dst, res, var_map);
            }
            IROp::Sra { dst, src, shreg } => {
                let v1 = Self::get_reg_value(builder, regs_ptr, *src, var_map);
                let v2 = Self::get_reg_value(builder, regs_ptr, *shreg, var_map);
                let res = builder.ins().sshr(v1, v2);
                Self::set_reg_value(builder, regs_ptr, *dst, res, var_map);
            }
            IROp::AddImm { dst, src, imm } => {
                let v1 = Self::get_reg_value(builder, regs_ptr, *src, var_map);
                let res = builder.ins().iadd_imm(v1, *imm);
                Self::set_reg_value(builder, regs_ptr, *dst, res, var_map);
            }
            IROp::MulImm { dst, src, imm } => {
                let v1 = Self::get_reg_value(builder, regs_ptr, *src, var_map);
                let v2 = builder.ins().iconst(types::I64, *imm);
                let res = builder.ins().imul(v1, v2);
                Self::set_reg_value(builder, regs_ptr, *dst, res, var_map);
            }
            IROp::MovImm { dst, imm } => {
                let res = builder.ins().iconst(types::I64, *imm as i64);
                Self::set_reg_value(builder, regs_ptr, *dst, res, var_map);
            }
            IROp::SllImm { dst, src, sh } => {
                let v1 = Self::get_reg_value(builder, regs_ptr, *src, var_map);
                let res = builder.ins().ishl_imm(v1, *sh as i64);
                Self::set_reg_value(builder, regs_ptr, *dst, res, var_map);
            }
            IROp::SrlImm { dst, src, sh } => {
                let v1 = Self::get_reg_value(builder, regs_ptr, *src, var_map);
                let res = builder.ins().ushr_imm(v1, *sh as i64);
                Self::set_reg_value(builder, regs_ptr, *dst, res, var_map);
            }
            IROp::SraImm { dst, src, sh } => {
                let v1 = Self::get_reg_value(builder, regs_ptr, *src, var_map);
                let res = builder.ins().sshr_imm(v1, *sh as i64);
                Self::set_reg_value(builder, regs_ptr, *dst, res, var_map);
            }
            IROp::CmpEq { dst, lhs, rhs } => {
                let v1 = Self::get_reg_value(builder, regs_ptr, *lhs, var_map);
                let v2 = Self::get_reg_value(builder, regs_ptr, *rhs, var_map);
                let cmp = builder.ins().icmp(IntCC::Equal, v1, v2);
                let one = builder.ins().iconst(types::I64, 1);
                let zero = builder.ins().iconst(types::I64, 0);
                let res = builder.ins().select(cmp, one, zero);
                Self::set_reg_value(builder, regs_ptr, *dst, res, var_map);
            }
            IROp::CmpNe { dst, lhs, rhs } => {
                let v1 = Self::get_reg_value(builder, regs_ptr, *lhs, var_map);
                let v2 = Self::get_reg_value(builder, regs_ptr, *rhs, var_map);
                let cmp = builder.ins().icmp(IntCC::NotEqual, v1, v2);
                let one = builder.ins().iconst(types::I64, 1);
                let zero = builder.ins().iconst(types::I64, 0);
                let res = builder.ins().select(cmp, one, zero);
                Self::set_reg_value(builder, regs_ptr, *dst, res, var_map);
            }
            IROp::CmpLt { dst, lhs, rhs } => {
                let v1 = Self::get_reg_value(builder, regs_ptr, *lhs, var_map);
                let v2 = Self::get_reg_value(builder, regs_ptr, *rhs, var_map);
                let cmp = builder.ins().icmp(IntCC::SignedLessThan, v1, v2);
                let one = builder.ins().iconst(types::I64, 1);
                let zero = builder.ins().iconst(types::I64, 0);
                let res = builder.ins().select(cmp, one, zero);
                Self::set_reg_value(builder, regs_ptr, *dst, res, var_map);
            }
            IROp::CmpLtU { dst, lhs, rhs } => {
                let v1 = Self::get_reg_value(builder, regs_ptr, *lhs, var_map);
                let v2 = Self::get_reg_value(builder, regs_ptr, *rhs, var_map);
                let cmp = builder.ins().icmp(IntCC::UnsignedLessThan, v1, v2);
                let one = builder.ins().iconst(types::I64, 1);
                let zero = builder.ins().iconst(types::I64, 0);
                let res = builder.ins().select(cmp, one, zero);
                Self::set_reg_value(builder, regs_ptr, *dst, res, var_map);
            }
            IROp::CmpGe { dst, lhs, rhs } => {
                let v1 = Self::get_reg_value(builder, regs_ptr, *lhs, var_map);
                let v2 = Self::get_reg_value(builder, regs_ptr, *rhs, var_map);
                let cmp = builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, v1, v2);
                let one = builder.ins().iconst(types::I64, 1);
                let zero = builder.ins().iconst(types::I64, 0);
                let res = builder.ins().select(cmp, one, zero);
                Self::set_reg_value(builder, regs_ptr, *dst, res, var_map);
            }
            IROp::CmpGeU { dst, lhs, rhs } => {
                let v1 = Self::get_reg_value(builder, regs_ptr, *lhs, var_map);
                let v2 = Self::get_reg_value(builder, regs_ptr, *rhs, var_map);
                let cmp = builder.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, v1, v2);
                let one = builder.ins().iconst(types::I64, 1);
                let zero = builder.ins().iconst(types::I64, 0);
                let res = builder.ins().select(cmp, one, zero);
                Self::set_reg_value(builder, regs_ptr, *dst, res, var_map);
            }
            IROp::Select { dst, cond, true_val, false_val } => {
                let c = Self::get_reg_value(builder, regs_ptr, *cond, var_map);
                let t = Self::get_reg_value(builder, regs_ptr, *true_val, var_map);
                let f = Self::get_reg_value(builder, regs_ptr, *false_val, var_map);
                let zero = builder.ins().iconst(types::I64, 0);
                let cond_bool = builder.ins().icmp(IntCC::NotEqual, c, zero);
                let res = builder.ins().select(cond_bool, t, f);
                Self::set_reg_value(builder, regs_ptr, *dst, res, var_map);
            }
            IROp::Load { dst, base, offset, size, .. } => {
                let b = Self::get_reg_value(builder, regs_ptr, *base, var_map);
                let addr = builder.ins().iadd_imm(b, *offset);
                
                let callee = match self.module.declarations().get_name("jit_read") {
                    Some(FuncOrDataId::Func(id)) => id,
                    _ => return Err("jit_read not found".to_string()),
                };
                let local_callee = self.module.declare_func_in_func(callee, builder.func);
                
                let size_val = builder.ins().iconst(types::I8, *size as i64);
                let call = builder.ins().call(local_callee, &[ctx_ptr, addr, size_val]);
                let res = builder.inst_results(call)[0];
                Self::set_reg_value(builder, regs_ptr, *dst, res, var_map);
            }
            IROp::Store { src, base, offset, size, .. } => {
                let val = Self::get_reg_value(builder, regs_ptr, *src, var_map);
                let b = Self::get_reg_value(builder, regs_ptr, *base, var_map);
                let addr = builder.ins().iadd_imm(b, *offset);
                
                let callee = match self.module.declarations().get_name("jit_write") {
                    Some(FuncOrDataId::Func(id)) => id,
                    _ => return Err("jit_write not found".to_string()),
                };
                let local_callee = self.module.declare_func_in_func(callee, builder.func);
                
                let size_val = builder.ins().iconst(types::I8, *size as i64);
                builder.ins().call(local_callee, &[ctx_ptr, addr, val, size_val]);
            }
            // SIMD operations - compile as scalar loop
            IROp::VecAdd { dst, src1, src2, element_size } => {
                Self::compile_vec_binop(builder, regs_ptr, *dst, *src1, *src2, *element_size, var_map, |b, a, c| b.ins().iadd(a, c));
            }
            IROp::VecSub { dst, src1, src2, element_size } => {
                Self::compile_vec_binop(builder, regs_ptr, *dst, *src1, *src2, *element_size, var_map, |b, a, c| b.ins().isub(a, c));
            }
            IROp::VecMul { dst, src1, src2, element_size } => {
                Self::compile_vec_binop(builder, regs_ptr, *dst, *src1, *src2, *element_size, var_map, |b, a, c| b.ins().imul(a, c));
            }
            _ => {
                // Unhandled operations - NOP for now
            }
        }
        Ok(())
    }
    
    /// Helper to get register value
    fn get_reg_value(
        builder: &mut FunctionBuilder,
        regs_ptr: Value,
        reg: u32,
        var_map: &mut HashMap<u32, Value>,
    ) -> Value {
        if reg == 0 {
            return builder.ins().iconst(types::I64, 0);
        }
        if let Some(&v) = var_map.get(&reg) {
            return v;
        }
        let offset = (reg as i32) * 8;
        let v = builder.ins().load(types::I64, MemFlags::new(), regs_ptr, offset);
        var_map.insert(reg, v);
        v
    }
    
    /// Helper to set register value
    fn set_reg_value(
        builder: &mut FunctionBuilder,
        regs_ptr: Value,
        reg: u32,
        value: Value,
        var_map: &mut HashMap<u32, Value>,
    ) {
        if reg == 0 {
            return;
        }
        var_map.insert(reg, value);
        let offset = (reg as i32) * 8;
        builder.ins().store(MemFlags::new(), value, regs_ptr, offset);
    }
    
    /// Compile a vector binary operation
    fn compile_vec_binop<F>(
        builder: &mut FunctionBuilder,
        regs_ptr: Value,
        dst: u32,
        src1: u32,
        src2: u32,
        element_size: u8,
        var_map: &mut HashMap<u32, Value>,
        op: F,
    ) where F: Fn(&mut FunctionBuilder, Value, Value) -> Value {
        let v1 = Self::get_reg_value(builder, regs_ptr, src1, var_map);
        let v2 = Self::get_reg_value(builder, regs_ptr, src2, var_map);
        
        let lane_bits = (element_size as u64) * 8;
        let lanes = 64 / lane_bits;
        let mask = ((1u128 << lane_bits) - 1) as i64;
        
        let mut acc = builder.ins().iconst(types::I64, 0);
        for i in 0..lanes {
            let shift = builder.ins().iconst(types::I64, (i * lane_bits) as i64);
            let mask_val = builder.ins().iconst(types::I64, mask);
            
            let av = builder.ins().ushr(v1, shift);
            let av = builder.ins().band(av, mask_val);
            let bv = builder.ins().ushr(v2, shift);
            let bv = builder.ins().band(bv, mask_val);
            
            let rv = op(builder, av, bv);
            let rv = builder.ins().band(rv, mask_val);
            let rv_shifted = builder.ins().ishl(rv, shift);
            acc = builder.ins().bor(acc, rv_shifted);
        }
        
        Self::set_reg_value(builder, regs_ptr, dst, acc, var_map);
    }
}
