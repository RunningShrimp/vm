use vm_core::{ExecutionEngine, ExecResult, ExecStatus, ExecStats, MMU, GuestAddr, AccessType};
use vm_ir::{IRBlock, IROp, Terminator};
use cranelift::prelude::*;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_codegen::Context as CodegenContext;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};
use cranelift_native;
use std::collections::HashMap;

mod advanced_ops;

pub const HOT_THRESHOLD: u64 = 100;

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

fn make_result(status: ExecStatus, executed_ops: u64, next_pc: GuestAddr) -> ExecResult {
    ExecResult {
        status,
        stats: make_stats(executed_ops),
        next_pc,
    }
}

pub struct JitContext<'a> {
    pub mmu: &'a mut dyn MMU,
}

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

#[derive(Clone, Copy)]
struct CodePtr(*const u8);
unsafe impl Send for CodePtr {}

pub struct Jit {
    builder_context: FunctionBuilderContext,
    ctx: CodegenContext,
    module: JITModule,
    cache: HashMap<GuestAddr, CodePtr>,
    hot_counts: HashMap<GuestAddr, BlockStats>,
    pub regs: [u64; 32],
    pub pc: GuestAddr,
    pub vec_regs: [[u64; 2]; 32],
    pub total_compiled: u64,
    pub total_interpreted: u64,
}

impl Jit {
    pub fn new() -> Self {
        let mut flag_builder = settings::builder();
        flag_builder.set("use_colocated_libcalls", "false").unwrap();
        flag_builder.set("is_pic", "false").unwrap();
        flag_builder.set("opt_level", "speed").unwrap();

        let isa_builder = cranelift_native::builder().unwrap_or_else(|msg| {
            panic!("host machine is not supported: {}", msg);
        });

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
            pc: 0,
            vec_regs: [[0; 2]; 32],
            total_compiled: 0,
            total_interpreted: 0,
        }
    }
    
    pub fn is_hot(&self, pc: GuestAddr) -> bool {
        self.hot_counts.get(&pc)
            .map(|s| s.exec_count >= HOT_THRESHOLD)
            .unwrap_or(false)
    }
    
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
    
    pub fn get_stats(&self, pc: GuestAddr) -> Option<&BlockStats> {
        self.hot_counts.get(&pc)
    }

    fn load_reg(builder: &mut FunctionBuilder, regs_ptr: Value, idx: u32) -> Value {
        if idx == 0 {
            builder.ins().iconst(types::I64, 0)
        } else {
            let offset = (idx as i32) * 8;
            builder.ins().load(types::I64, MemFlags::trusted(), regs_ptr, offset)
        }
    }

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
        self.ctx.func.signature.params.push(AbiParam::new(types::I64));
        self.ctx.func.signature.params.push(AbiParam::new(types::I64));
        self.ctx.func.signature.returns.push(AbiParam::new(types::I64));

        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let regs_ptr = builder.block_params(entry_block)[0];
        let _ctx_ptr = builder.block_params(entry_block)[1];

        for op in &block.ops {
            match op {
                IROp::Add { dst, src1, src2 } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *src1);
                    let v2 = Self::load_reg(&mut builder, regs_ptr, *src2);
                    let res = builder.ins().iadd(v1, v2);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                _ => {}
            }
        }

        match &block.term {
            Terminator::Jmp { target } => {
                let next_pc = builder.ins().iconst(types::I64, *target as i64);
                builder.ins().return_(&[next_pc]);
            }
            Terminator::CondJmp { cond, target_true, target_false } => {
                let v = Self::load_reg(&mut builder, regs_ptr, *cond);
                let true_block = builder.create_block();
                let false_block = builder.create_block();
                builder.ins().brif(v, true_block, &[], false_block, &[]);

                builder.switch_to_block(true_block);
                builder.seal_block(true_block);
                let next_pc_true = builder.ins().iconst(types::I64, *target_true as i64);
                builder.ins().return_(&[next_pc_true]);

                builder.switch_to_block(false_block);
                builder.seal_block(false_block);
                let next_pc_false = builder.ins().iconst(types::I64, *target_false as i64);
                builder.ins().return_(&[next_pc_false]);
            }
            _ => {}
        }

        builder.finalize();

        let id = self.module.declare_function(&format!("func_{}", block.start_pc), Linkage::Export, &self.ctx.func.signature).unwrap();
        self.module.define_function(id, &mut self.ctx).unwrap();
        self.module.clear_context(&mut self.ctx);
        self.module.finalize_definitions().unwrap();

        let code = self.module.get_finalized_function(id);
        self.cache.insert(block.start_pc, CodePtr(code));
        code
    }
}

impl ExecutionEngine<IRBlock> for Jit {
    fn run(&mut self, mmu: &mut dyn MMU, block: &IRBlock) -> ExecResult {
        let mut executed_ops = 0;
        if self.record_execution(self.pc) {
            self.compile(block);
        }

        if let Some(code_ptr) = self.cache.get(&self.pc) {
            let code_fn = unsafe { std::mem::transmute::<*const u8, fn(&mut [u64; 32], &mut JitContext) -> u64>(code_ptr.0) };
            let mut jit_ctx = JitContext { mmu };
            self.pc = code_fn(&mut self.regs, &mut jit_ctx);
            self.total_compiled += 1;
        } else {
            // Fallback to interpreter if not compiled
            self.pc += 4; // Simple increment for placeholder
            self.total_interpreted += 1;
        }
        executed_ops += 1;
        make_result(ExecStatus::Continue, executed_ops, self.pc)
    }

    fn get_reg(&self, reg: usize) -> u64 {
        self.regs[reg as usize]
    }

    fn set_reg(&mut self, reg: usize, val: u64) {
        if reg != 0 {
            self.regs[reg as usize] = val;
        }
    }

    fn get_pc(&self) -> u64 {
        self.pc
    }

    fn set_pc(&mut self, pc: u64) {
        self.pc = pc;
    }

    fn get_vcpu_state(&self) -> vm_core::VcpuStateContainer {
        vm_core::VcpuStateContainer {
            regs: self.regs,
            pc: self.pc,
        }
    }

    fn set_vcpu_state(&mut self, state: &vm_core::VcpuStateContainer) {
        self.regs = state.regs;
        self.pc = state.pc;
    }
}
