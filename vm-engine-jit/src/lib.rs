//! # vm-engine-jit - JIT 编译执行引擎
//!
//! 基于 Cranelift 的即时编译执行引擎，将 IR 编译为本机代码执行。
//!
//! ## 架构
//!
//! ```text
//! IR Block -> Cranelift IR -> Native Code -> Execute
//!              (translate)    (compile)      (call)
//! ```
//!
//! ## 主要组件
//!
//! - [`Jit`]: JIT 编译器主结构体，实现 [`ExecutionEngine`] trait
//! - [`JitContext`]: JIT 执行上下文，包含 MMU 引用
//! - [`pool`]: 编译代码池管理
//!
//! ## 热点追踪
//!
//! 使用 [`HOT_THRESHOLD`] (默认100次) 来判断是否需要 JIT 编译。
//! 执行次数超过阈值的基本块会被编译为本机代码。
//!
//! ## 当前状态
//!
//! **已实现**: Add, MovImm
//! **待实现**: Sub, Mul, Div, Load, Store, 分支指令, 向量操作
//!
//! ## 示例
//!
//! ```rust,ignore
//! use vm_engine_jit::Jit;
//! use vm_core::ExecutionEngine;
//!
//! let mut jit = Jit::new();
//! let result = jit.run(&mut mmu, &ir_block);
//! ```

use vm_core::{ExecutionEngine, ExecResult, ExecStatus, ExecStats, MMU, GuestAddr};
use vm_ir::{IRBlock, IROp, Terminator, AtomicOp};
//! # vm-engine-jit - JIT 编译执行引擎
//!
//! 基于 Cranelift 的即时编译执行引擎，将 IR 编译为本机代码执行。
//!
//! ## 架构
//!
//! ```text
//! IR Block -> Cranelift IR -> Native Code -> Execute
//!              (translate)    (compile)      (call)
//! ```
//!
//! ## 主要组件
//!
//! - [`Jit`]: JIT 编译器主结构体，实现 [`ExecutionEngine`] trait
//! - [`JitContext`]: JIT 执行上下文，包含 MMU 引用
//! - [`pool`]: 编译代码池管理
//!
//! ## 热点追踪
//!
//! 使用 [`HOT_THRESHOLD`] (默认100次) 来判断是否需要 JIT 编译。
//! 执行次数超过阈值的基本块会被编译为本机代码。
//!
//! ## 当前状态
//!
//! **已实现**: Add, MovImm
//! **待实现**: Sub, Mul, Div, Load, Store, 分支指令, 向量操作
//!
//! ## 示例
//!
//! ```rust,ignore
//! use vm_engine_jit::Jit;
//! use vm_core::ExecutionEngine;
//!
//! let mut jit = Jit::new();
//! let result = jit.run(&mut mmu, &ir_block);
//! ```

use vm_core::{ExecutionEngine, ExecResult, ExecStatus, ExecStats, MMU, GuestAddr};
use vm_ir::{IRBlock, IROp, Terminator, AtomicOp};
use cranelift::prelude::*;
use cranelift_codegen::ir::AtomicRmwOp;
use cranelift_codegen::ir::AtomicRmwOp;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_codegen::Context as CodegenContext;
use cranelift_codegen::ir::FuncRef;
use cranelift_codegen::ir::FuncRef;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module, FuncId};
use cranelift_module::{Linkage, Module, FuncId};
use cranelift_native;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::{Arc, Mutex};

mod advanced_ops;
mod simd;
pub mod pool;
mod jit_helpers;

pub use jit_helpers::{RegisterHelper, FloatRegHelper, MemoryHelper};

/// 默认热点阈值
pub const HOT_THRESHOLD: u64 = 100;

/// 自适应阈值配置
#[derive(Clone, Debug)]
pub struct AdaptiveThresholdConfig {
    /// 最小阈值
    pub min_threshold: u64,
    /// 最大阈值
    pub max_threshold: u64,
    /// 采样窗口大小
    pub sample_window: usize,
    /// 编译时间权重
    pub compile_time_weight: f64,
    /// 执行收益权重
    pub exec_benefit_weight: f64,
}

impl Default for AdaptiveThresholdConfig {
    fn default() -> Self {
        Self {
            min_threshold: 10,
            max_threshold: 1000,
            sample_window: 100,
            compile_time_weight: 0.3,
            exec_benefit_weight: 0.7,
        }
    }
}

/// 自适应热点阈值管理器
#[derive(Clone)]
pub struct AdaptiveThreshold {
    /// 当前阈值
    current_threshold: u64,
    /// 配置
    config: AdaptiveThresholdConfig,
    /// 编译时间样本 (纳秒)
    compile_time_samples: Vec<u64>,
    /// 执行时间节省样本 (纳秒)
    exec_benefit_samples: Vec<i64>,
    /// 总编译次数
    total_compiles: u64,
    /// 命中编译代码的执行次数
    compiled_hits: u64,
    /// 解释器执行次数
    interpreted_runs: u64,
    /// 上次调整时的总执行数
    last_adjustment_total: u64,
}

impl AdaptiveThreshold {
    pub fn new() -> Self {
        Self::with_config(AdaptiveThresholdConfig::default())
    }

    pub fn with_config(config: AdaptiveThresholdConfig) -> Self {
        Self {
            current_threshold: HOT_THRESHOLD,
            config,
            compile_time_samples: Vec::with_capacity(100),
            exec_benefit_samples: Vec::with_capacity(100),
            total_compiles: 0,
            compiled_hits: 0,
            interpreted_runs: 0,
            last_adjustment_total: 0,
        }
    }

    /// 获取当前阈值
    pub fn threshold(&self) -> u64 {
        self.current_threshold
    }

    /// 记录编译事件
    pub fn record_compile(&mut self, compile_time_ns: u64) {
        self.total_compiles += 1;
        self.compile_time_samples.push(compile_time_ns);
        
        // 保持样本窗口大小
        if self.compile_time_samples.len() > self.config.sample_window {
            self.compile_time_samples.remove(0);
        }
    }

    /// 记录编译代码执行
    pub fn record_compiled_hit(&mut self, exec_time_ns: u64, estimated_interp_time_ns: u64) {
        self.compiled_hits += 1;
        let benefit = estimated_interp_time_ns as i64 - exec_time_ns as i64;
        self.exec_benefit_samples.push(benefit);
        
        if self.exec_benefit_samples.len() > self.config.sample_window {
            self.exec_benefit_samples.remove(0);
        }
    }

    /// 记录解释器执行
    pub fn record_interpreted(&mut self) {
        self.interpreted_runs += 1;
    }

    /// 调整阈值 (基于运行时性能数据)
    pub fn adjust(&mut self) {
        let total_runs = self.compiled_hits + self.interpreted_runs;
        
        // 每 1000 次执行调整一次
        if total_runs - self.last_adjustment_total < 1000 {
            return;
        }
        self.last_adjustment_total = total_runs;

        // 计算平均编译时间
        let avg_compile_time = if self.compile_time_samples.is_empty() {
            1000_u64 // 默认 1μs
        } else {
            self.compile_time_samples.iter().sum::<u64>() / self.compile_time_samples.len() as u64
        };

        // 计算平均执行收益
        let avg_benefit = if self.exec_benefit_samples.is_empty() {
            0_i64
        } else {
            self.exec_benefit_samples.iter().sum::<i64>() / self.exec_benefit_samples.len() as i64
        };

        // 计算编译命中率
        let hit_rate = if total_runs > 0 {
            self.compiled_hits as f64 / total_runs as f64
        } else {
            0.0
        };

        // 调整策略:
        // - 高编译时间 + 低收益 -> 提高阈值 (减少编译)
        // - 低编译时间 + 高收益 -> 降低阈值 (更积极编译)
        // - 低命中率 -> 提高阈值 (编译的代码没被充分利用)
        
        let compile_factor = if avg_compile_time > 10_000_000 { // > 10ms
            1.2 // 编译太慢，提高阈值
        } else if avg_compile_time < 100_000 { // < 100μs
            0.9 // 编译很快，可以降低阈值
        } else {
            1.0
        };

        let benefit_factor = if avg_benefit > 1_000_000 { // 每次执行节省 > 1ms
            0.8 // 收益高，降低阈值
        } else if avg_benefit < 0 { // 负收益 (JIT 更慢)
            1.3 // 提高阈值
        } else {
            1.0
        };

        let hit_factor = if hit_rate < 0.1 {
            1.2 // 命中率太低
        } else if hit_rate > 0.8 {
            0.95 // 命中率高，可以更积极
        } else {
            1.0
        };

        // 综合调整
        let adjustment = compile_factor * benefit_factor * hit_factor;
        let new_threshold = (self.current_threshold as f64 * adjustment) as u64;
        
        // 限制在范围内
        self.current_threshold = new_threshold
            .max(self.config.min_threshold)
            .min(self.config.max_threshold);
    }

    /// 获取统计信息
    pub fn stats(&self) -> AdaptiveThresholdStats {
        AdaptiveThresholdStats {
            current_threshold: self.current_threshold,
            total_compiles: self.total_compiles,
            compiled_hits: self.compiled_hits,
            interpreted_runs: self.interpreted_runs,
            avg_compile_time_ns: if self.compile_time_samples.is_empty() {
                0
            } else {
                self.compile_time_samples.iter().sum::<u64>() / self.compile_time_samples.len() as u64
            },
            avg_benefit_ns: if self.exec_benefit_samples.is_empty() {
                0
            } else {
                (self.exec_benefit_samples.iter().sum::<i64>() / self.exec_benefit_samples.len() as i64) as i64
            },
        }
    }
}

impl Default for AdaptiveThreshold {
    fn default() -> Self {
        Self::new()
    }
}

/// 自适应阈值统计信息
#[derive(Debug, Clone)]
pub struct AdaptiveThresholdStats {
    pub current_threshold: u64,
    pub total_compiles: u64,
    pub compiled_hits: u64,
    pub interpreted_runs: u64,
    pub avg_compile_time_ns: u64,
    pub avg_benefit_ns: i64,
}

/// 自适应阈值配置
#[derive(Clone, Debug)]
pub struct AdaptiveThresholdConfig {
    /// 最小阈值
    pub min_threshold: u64,
    /// 最大阈值
    pub max_threshold: u64,
    /// 采样窗口大小
    pub sample_window: usize,
    /// 编译时间权重
    pub compile_time_weight: f64,
    /// 执行收益权重
    pub exec_benefit_weight: f64,
}

impl Default for AdaptiveThresholdConfig {
    fn default() -> Self {
        Self {
            min_threshold: 10,
            max_threshold: 1000,
            sample_window: 100,
            compile_time_weight: 0.3,
            exec_benefit_weight: 0.7,
        }
    }
}

/// 自适应热点阈值管理器
#[derive(Clone)]
pub struct AdaptiveThreshold {
    /// 当前阈值
    current_threshold: u64,
    /// 配置
    config: AdaptiveThresholdConfig,
    /// 编译时间样本 (纳秒)
    compile_time_samples: Vec<u64>,
    /// 执行时间节省样本 (纳秒)
    exec_benefit_samples: Vec<i64>,
    /// 总编译次数
    total_compiles: u64,
    /// 命中编译代码的执行次数
    compiled_hits: u64,
    /// 解释器执行次数
    interpreted_runs: u64,
    /// 上次调整时的总执行数
    last_adjustment_total: u64,
}

impl AdaptiveThreshold {
    pub fn new() -> Self {
        Self::with_config(AdaptiveThresholdConfig::default())
    }

    pub fn with_config(config: AdaptiveThresholdConfig) -> Self {
        Self {
            current_threshold: HOT_THRESHOLD,
            config,
            compile_time_samples: Vec::with_capacity(100),
            exec_benefit_samples: Vec::with_capacity(100),
            total_compiles: 0,
            compiled_hits: 0,
            interpreted_runs: 0,
            last_adjustment_total: 0,
        }
    }

    /// 获取当前阈值
    pub fn threshold(&self) -> u64 {
        self.current_threshold
    }

    /// 记录编译事件
    pub fn record_compile(&mut self, compile_time_ns: u64) {
        self.total_compiles += 1;
        self.compile_time_samples.push(compile_time_ns);
        
        // 保持样本窗口大小
        if self.compile_time_samples.len() > self.config.sample_window {
            self.compile_time_samples.remove(0);
        }
    }

    /// 记录编译代码执行
    pub fn record_compiled_hit(&mut self, exec_time_ns: u64, estimated_interp_time_ns: u64) {
        self.compiled_hits += 1;
        let benefit = estimated_interp_time_ns as i64 - exec_time_ns as i64;
        self.exec_benefit_samples.push(benefit);
        
        if self.exec_benefit_samples.len() > self.config.sample_window {
            self.exec_benefit_samples.remove(0);
        }
    }

    /// 记录解释器执行
    pub fn record_interpreted(&mut self) {
        self.interpreted_runs += 1;
    }

    /// 调整阈值 (基于运行时性能数据)
    pub fn adjust(&mut self) {
        let total_runs = self.compiled_hits + self.interpreted_runs;
        
        // 每 1000 次执行调整一次
        if total_runs - self.last_adjustment_total < 1000 {
            return;
        }
        self.last_adjustment_total = total_runs;

        // 计算平均编译时间
        let avg_compile_time = if self.compile_time_samples.is_empty() {
            1000_u64 // 默认 1μs
        } else {
            self.compile_time_samples.iter().sum::<u64>() / self.compile_time_samples.len() as u64
        };

        // 计算平均执行收益
        let avg_benefit = if self.exec_benefit_samples.is_empty() {
            0_i64
        } else {
            self.exec_benefit_samples.iter().sum::<i64>() / self.exec_benefit_samples.len() as i64
        };

        // 计算编译命中率
        let hit_rate = if total_runs > 0 {
            self.compiled_hits as f64 / total_runs as f64
        } else {
            0.0
        };

        // 调整策略:
        // - 高编译时间 + 低收益 -> 提高阈值 (减少编译)
        // - 低编译时间 + 高收益 -> 降低阈值 (更积极编译)
        // - 低命中率 -> 提高阈值 (编译的代码没被充分利用)
        
        let compile_factor = if avg_compile_time > 10_000_000 { // > 10ms
            1.2 // 编译太慢，提高阈值
        } else if avg_compile_time < 100_000 { // < 100μs
            0.9 // 编译很快，可以降低阈值
        } else {
            1.0
        };

        let benefit_factor = if avg_benefit > 1_000_000 { // 每次执行节省 > 1ms
            0.8 // 收益高，降低阈值
        } else if avg_benefit < 0 { // 负收益 (JIT 更慢)
            1.3 // 提高阈值
        } else {
            1.0
        };

        let hit_factor = if hit_rate < 0.1 {
            1.2 // 命中率太低
        } else if hit_rate > 0.8 {
            0.95 // 命中率高，可以更积极
        } else {
            1.0
        };

        // 综合调整
        let adjustment = compile_factor * benefit_factor * hit_factor;
        let new_threshold = (self.current_threshold as f64 * adjustment) as u64;
        
        // 限制在范围内
        self.current_threshold = new_threshold
            .max(self.config.min_threshold)
            .min(self.config.max_threshold);
    }

    /// 获取统计信息
    pub fn stats(&self) -> AdaptiveThresholdStats {
        AdaptiveThresholdStats {
            current_threshold: self.current_threshold,
            total_compiles: self.total_compiles,
            compiled_hits: self.compiled_hits,
            interpreted_runs: self.interpreted_runs,
            avg_compile_time_ns: if self.compile_time_samples.is_empty() {
                0
            } else {
                self.compile_time_samples.iter().sum::<u64>() / self.compile_time_samples.len() as u64
            },
            avg_benefit_ns: if self.exec_benefit_samples.is_empty() {
                0
            } else {
                (self.exec_benefit_samples.iter().sum::<i64>() / self.exec_benefit_samples.len() as i64) as i64
            },
        }
    }
}

impl Default for AdaptiveThreshold {
    fn default() -> Self {
        Self::new()
    }
}

/// 自适应阈值统计信息
#[derive(Debug, Clone)]
pub struct AdaptiveThresholdStats {
    pub current_threshold: u64,
    pub total_compiles: u64,
    pub compiled_hits: u64,
    pub interpreted_runs: u64,
    pub avg_compile_time_ns: u64,
    pub avg_benefit_ns: i64,
}

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

#[derive(Clone, Copy)]
enum SimdIntrinsic {
    Add,
    Sub,
    Mul,
}

extern "C" fn jit_read(ctx: *mut JitContext, vaddr: u64, size: u8) -> u64 {
    unsafe {
        let pa = match (*ctx).mmu.translate(vaddr, vm_core::AccessType::Read) {
            Ok(p) => p,
            Err(_) => return 0,
        };
        (*ctx).mmu.read(pa, size).unwrap_or(0)
    }
}

extern "C" fn jit_write(ctx: *mut JitContext, vaddr: u64, val: u64, size: u8) {
    unsafe {
        if let Ok(pa) = (*ctx).mmu.translate(vaddr, vm_core::AccessType::Write) {
            let _ = (*ctx).mmu.write(pa, val, size);
        }
    }
}

extern "C" fn jit_lr(ctx: *mut JitContext, vaddr: u64, size: u8) -> u64 {
    unsafe { (*ctx).mmu.load_reserved(vaddr, size).unwrap_or(0) }
}

extern "C" fn jit_sc(ctx: *mut JitContext, vaddr: u64, val: u64, size: u8) -> u64 {
    unsafe {
        match (*ctx).mmu.store_conditional(vaddr, val, size) {
            Ok(true) => 1,
            Ok(false) => 0,
            Err(_) => 0,
        }
    }
}

extern "C" fn jit_cas(ctx: *mut JitContext, vaddr: u64, expected: u64, new: u64, size: u8) -> u64 {
    unsafe {
        std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
        let pa_r = match (*ctx).mmu.translate(vaddr, vm_core::AccessType::Read) { Ok(p) => p, Err(_) => return 0 };
        let old = (*ctx).mmu.read(pa_r, size).unwrap_or(0);
        if old == expected {
            if let Ok(pa_w) = (*ctx).mmu.translate(vaddr, vm_core::AccessType::Write) {
                let _ = (*ctx).mmu.write(pa_w, new, size);
            }
        }
        std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
        old
    }
}

extern "C" fn barrier_acquire() { std::sync::atomic::fence(std::sync::atomic::Ordering::Acquire); }
extern "C" fn barrier_release() { std::sync::atomic::fence(std::sync::atomic::Ordering::Release); }
extern "C" fn barrier_full() { std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst); }

/// 编译后的代码指针包装类型
#[derive(Clone, Copy)]
pub struct CodePtr(*const u8);
pub struct CodePtr(*const u8);
unsafe impl Send for CodePtr {}

pub struct Jit {
    builder_context: FunctionBuilderContext,
    ctx: CodegenContext,
    module: JITModule,
    cache: HashMap<GuestAddr, CodePtr>,
    pool_cache: Option<Arc<Mutex<HashMap<GuestAddr, CodePtr>>>>,
    pool_cache: Option<Arc<Mutex<HashMap<GuestAddr, CodePtr>>>>,
    hot_counts: HashMap<GuestAddr, BlockStats>,
    pub regs: [u64; 32],
    pub pc: GuestAddr,
    pub vec_regs: [[u64; 2]; 32],
    /// 浮点寄存器 (f0-f31)
    pub fregs: [f64; 32],
    /// 浮点寄存器 (f0-f31)
    pub fregs: [f64; 32],
    pub total_compiled: u64,
    pub total_interpreted: u64,
    /// 自适应热点阈值管理器
    pub adaptive_threshold: AdaptiveThreshold,
    simd_vec_add_func: Option<FuncId>,
    simd_vec_sub_func: Option<FuncId>,
    simd_vec_mul_func: Option<FuncId>,
    /// 自适应热点阈值管理器
    pub adaptive_threshold: AdaptiveThreshold,
    simd_vec_add_func: Option<FuncId>,
    simd_vec_sub_func: Option<FuncId>,
    simd_vec_mul_func: Option<FuncId>,
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
        builder.symbol("jit_cas", jit_cas as *const u8);
        builder.symbol("jit_lr", jit_lr as *const u8);
        builder.symbol("jit_sc", jit_sc as *const u8);
        builder.symbol("barrier_acquire", barrier_acquire as *const u8);
        builder.symbol("barrier_release", barrier_release as *const u8);
        builder.symbol("barrier_full", barrier_full as *const u8);
        builder.symbol("jit_vec_add", simd::jit_vec_add as *const u8);
        builder.symbol("jit_vec_sub", simd::jit_vec_sub as *const u8);
        builder.symbol("jit_vec_mul", simd::jit_vec_mul as *const u8);

        let module = JITModule::new(builder);
        Self {
            builder_context: FunctionBuilderContext::new(),
            ctx: module.make_context(),
            module,
            cache: HashMap::new(),
            pool_cache: None,
            pool_cache: None,
            hot_counts: HashMap::new(),
            regs: [0; 32],
            pc: 0,
            vec_regs: [[0; 2]; 32],
            fregs: [0.0; 32],
            fregs: [0.0; 32],
            total_compiled: 0,
            total_interpreted: 0,
            adaptive_threshold: AdaptiveThreshold::new(),
            simd_vec_add_func: None,
            simd_vec_sub_func: None,
            simd_vec_mul_func: None,
            adaptive_threshold: AdaptiveThreshold::new(),
            simd_vec_add_func: None,
            simd_vec_sub_func: None,
            simd_vec_mul_func: None,
        }
    }

    /// 使用自定义配置创建 JIT 编译器
    pub fn with_adaptive_config(config: AdaptiveThresholdConfig) -> Self {
        let mut jit = Self::new();
        jit.adaptive_threshold = AdaptiveThreshold::with_config(config);
        jit
    }

    /// 加载浮点寄存器值 (F64)
    fn load_freg(builder: &mut FunctionBuilder, fregs_ptr: Value, idx: u32) -> Value {
        let offset = (idx as i32) * 8;
        builder.ins().load(types::F64, MemFlags::trusted(), fregs_ptr, offset)
    }

    /// 存储浮点寄存器值 (F64)
    fn store_freg(builder: &mut FunctionBuilder, fregs_ptr: Value, idx: u32, val: Value) {
        let offset = (idx as i32) * 8;
        builder.ins().store(MemFlags::trusted(), val, fregs_ptr, offset);
    }

    /// 加载单精度浮点寄存器值 (F32)
    /// 注意：内部存储为 F64，这里加载低 32 位并转换为 F32
    fn load_freg_f32(builder: &mut FunctionBuilder, fregs_ptr: Value, idx: u32) -> Value {
        let offset = (idx as i32) * 8;
        // 加载 F64 然后降级为 F32
        let f64_val = builder.ins().load(types::F64, MemFlags::trusted(), fregs_ptr, offset);
        builder.ins().fdemote(types::F32, f64_val)
    }

    /// 存储单精度浮点寄存器值 (F32)
    /// 注意：将 F32 提升为 F64 后存储
    fn store_freg_f32(builder: &mut FunctionBuilder, fregs_ptr: Value, idx: u32, val: Value) {
        let offset = (idx as i32) * 8;
        // 将 F32 提升为 F64 然后存储
        let f64_val = builder.ins().fpromote(types::F64, val);
        builder.ins().store(MemFlags::trusted(), f64_val, fregs_ptr, offset);
    }

    fn ensure_simd_func_id(&mut self, op: SimdIntrinsic) -> FuncId {
        let (slot, name) = match op {
            SimdIntrinsic::Add => (&mut self.simd_vec_add_func, "jit_vec_add"),
            SimdIntrinsic::Sub => (&mut self.simd_vec_sub_func, "jit_vec_sub"),
            SimdIntrinsic::Mul => (&mut self.simd_vec_mul_func, "jit_vec_mul"),
        };

        if let Some(id) = slot {
            *id
        } else {
            let mut sig = self.module.make_signature();
            sig.params.push(AbiParam::new(types::I64));
            sig.params.push(AbiParam::new(types::I64));
            sig.params.push(AbiParam::new(types::I64));
            sig.returns.push(AbiParam::new(types::I64));
            let func_id = self.module.declare_function(name, Linkage::Import, &sig).unwrap();
            *slot = Some(func_id);
            func_id
        }
    }

    fn get_simd_funcref(&mut self, builder: &mut FunctionBuilder, op: SimdIntrinsic) -> FuncRef {
        let func_id = self.ensure_simd_func_id(op);
        self.module.declare_func_in_func(func_id, builder.func)
    }

    fn call_simd_intrinsic(
        &mut self,
        builder: &mut FunctionBuilder,
        op: SimdIntrinsic,
        lhs: Value,
        rhs: Value,
        element_size: u8,
    ) -> Value {
        let func_ref = self.get_simd_funcref(builder, op);
        let es = builder.ins().iconst(types::I64, element_size as i64);
        let call = builder.ins().call(func_ref, &[lhs, rhs, es]);
        builder.inst_results(call)[0]
    }

    pub fn with_pool_cache(mut self, cache: Arc<Mutex<HashMap<GuestAddr, CodePtr>>>) -> Self {
        self.pool_cache = Some(cache);
        self
    }
    
    /// 检查基本块是否足够热以触发编译 (使用自适应阈值)
    pub fn is_hot(&self, pc: GuestAddr) -> bool {
        let threshold = self.adaptive_threshold.threshold();
        let threshold = self.adaptive_threshold.threshold();
        self.hot_counts.get(&pc)
            .map(|s| s.exec_count >= threshold)
            .map(|s| s.exec_count >= threshold)
            .unwrap_or(false)
    }
    
    /// 记录执行并检查是否需要编译 (使用自适应阈值)
    /// 记录执行并检查是否需要编译 (使用自适应阈值)
    pub fn record_execution(&mut self, pc: GuestAddr) -> bool {
        let threshold = self.adaptive_threshold.threshold();
        let threshold = self.adaptive_threshold.threshold();
        let stats = self.hot_counts.entry(pc).or_default();
        stats.exec_count += 1;
        
        if stats.exec_count >= threshold && !stats.is_compiled {
        
        if stats.exec_count >= threshold && !stats.is_compiled {
            stats.is_compiled = true;
            true
        } else {
            false
        }
    }
    
    /// 记录编译完成并更新自适应阈值
    pub fn record_compile_done(&mut self, compile_time_ns: u64) {
        self.adaptive_threshold.record_compile(compile_time_ns);
        self.adaptive_threshold.adjust();
    }

    /// 记录编译代码执行并更新统计
    pub fn record_compiled_execution(&mut self, exec_time_ns: u64, block_ops: usize) {
        // 估计解释器执行时间 (假设每条操作约 50ns)
        let estimated_interp_time = (block_ops as u64) * 50;
        self.adaptive_threshold.record_compiled_hit(exec_time_ns, estimated_interp_time);
    }

    /// 记录解释器执行
    pub fn record_interpreted_execution(&mut self) {
        self.adaptive_threshold.record_interpreted();
    }

    /// 获取自适应阈值统计信息
    pub fn adaptive_stats(&self) -> AdaptiveThresholdStats {
        self.adaptive_threshold.stats()
    }
    
    /// 记录编译完成并更新自适应阈值
    pub fn record_compile_done(&mut self, compile_time_ns: u64) {
        self.adaptive_threshold.record_compile(compile_time_ns);
        self.adaptive_threshold.adjust();
    }

    /// 记录编译代码执行并更新统计
    pub fn record_compiled_execution(&mut self, exec_time_ns: u64, block_ops: usize) {
        // 估计解释器执行时间 (假设每条操作约 50ns)
        let estimated_interp_time = (block_ops as u64) * 50;
        self.adaptive_threshold.record_compiled_hit(exec_time_ns, estimated_interp_time);
    }

    /// 记录解释器执行
    pub fn record_interpreted_execution(&mut self) {
        self.adaptive_threshold.record_interpreted();
    }

    /// 获取自适应阈值统计信息
    pub fn adaptive_stats(&self) -> AdaptiveThresholdStats {
        self.adaptive_threshold.stats()
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

        let mut ctx = std::mem::replace(&mut self.ctx, cranelift_codegen::Context::new());
        ctx.func.signature.params.clear();
        ctx.func.signature.returns.clear();
        ctx.func.signature.params.push(AbiParam::new(types::I64));
        ctx.func.signature.params.push(AbiParam::new(types::I64));
        ctx.func.signature.params.push(AbiParam::new(types::I64));
        ctx.func.signature.returns.push(AbiParam::new(types::I64));
        let mut ctx = std::mem::replace(&mut self.ctx, cranelift_codegen::Context::new());
        ctx.func.signature.params.clear();
        ctx.func.signature.returns.clear();
        ctx.func.signature.params.push(AbiParam::new(types::I64));
        ctx.func.signature.params.push(AbiParam::new(types::I64));
        ctx.func.signature.params.push(AbiParam::new(types::I64));
        ctx.func.signature.returns.push(AbiParam::new(types::I64));

        let mut builder_context = std::mem::replace(&mut self.builder_context, FunctionBuilderContext::new());
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_context);
        let mut builder_context = std::mem::replace(&mut self.builder_context, FunctionBuilderContext::new());
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_context);
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let regs_ptr = builder.block_params(entry_block)[0];
        let ctx_ptr = builder.block_params(entry_block)[1];
        let fregs_ptr = builder.block_params(entry_block)[2];

        for op in &block.ops {
            match op {
                // ============================================================
                // 算术运算 (Arithmetic Operations)
                // ============================================================
                // ============================================================
                // 算术运算 (Arithmetic Operations)
                // ============================================================
                IROp::Add { dst, src1, src2 } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *src1);
                    let v2 = Self::load_reg(&mut builder, regs_ptr, *src2);
                    let res = builder.ins().iadd(v1, v2);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::AddImm { dst, src, imm } => {
                    let v = Self::load_reg(&mut builder, regs_ptr, *src);
                    let imm_val = builder.ins().iconst(types::I64, *imm);
                    let res = builder.ins().iadd(v, imm_val);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::Sub { dst, src1, src2 } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *src1);
                    let v2 = Self::load_reg(&mut builder, regs_ptr, *src2);
                    let res = builder.ins().isub(v1, v2);
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

                // ============================================================
                // 逻辑运算 (Logical Operations)
                // ============================================================
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
                IROp::Not { dst, src } => {
                    let v = Self::load_reg(&mut builder, regs_ptr, *src);
                    let res = builder.ins().bnot(v);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }

                // ============================================================
                // 移位运算 (Shift Operations)
                // ============================================================
                IROp::Sll { dst, src, shreg } => {
                    let v = Self::load_reg(&mut builder, regs_ptr, *src);
                    let amt = Self::load_reg(&mut builder, regs_ptr, *shreg);
                    let res = builder.ins().ishl(v, amt);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::Srl { dst, src, shreg } => {
                    let v = Self::load_reg(&mut builder, regs_ptr, *src);
                    let amt = Self::load_reg(&mut builder, regs_ptr, *shreg);
                    let res = builder.ins().ushr(v, amt);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::Sra { dst, src, shreg } => {
                    let v = Self::load_reg(&mut builder, regs_ptr, *src);
                    let amt = Self::load_reg(&mut builder, regs_ptr, *shreg);
                    let res = builder.ins().sshr(v, amt);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::SllImm { dst, src, sh } => {
                    let v = Self::load_reg(&mut builder, regs_ptr, *src);
                    let amt = builder.ins().iconst(types::I64, *sh as i64);
                    let res = builder.ins().ishl(v, amt);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::SrlImm { dst, src, sh } => {
                    let v = Self::load_reg(&mut builder, regs_ptr, *src);
                    let amt = builder.ins().iconst(types::I64, *sh as i64);
                    let res = builder.ins().ushr(v, amt);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::SraImm { dst, src, sh } => {
                    let v = Self::load_reg(&mut builder, regs_ptr, *src);
                    let amt = builder.ins().iconst(types::I64, *sh as i64);
                    let res = builder.ins().sshr(v, amt);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }

                // ============================================================
                // 数据移动 (Data Movement)
                // ============================================================
                IROp::MovImm { dst, imm } => {
                    let val = builder.ins().iconst(types::I64, *imm as i64);
                    Self::store_reg(&mut builder, regs_ptr, *dst, val);
                }

                // ============================================================
                // 比较运算 (Comparison Operations)
                // ============================================================
                IROp::CmpEq { dst, lhs, rhs } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *lhs);
                    let v2 = Self::load_reg(&mut builder, regs_ptr, *rhs);
                    let cmp = builder.ins().icmp(IntCC::Equal, v1, v2);
                    let res = builder.ins().uextend(types::I64, cmp);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::CmpNe { dst, lhs, rhs } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *lhs);
                    let v2 = Self::load_reg(&mut builder, regs_ptr, *rhs);
                    let cmp = builder.ins().icmp(IntCC::NotEqual, v1, v2);
                    let res = builder.ins().uextend(types::I64, cmp);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::CmpLt { dst, lhs, rhs } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *lhs);
                    let v2 = Self::load_reg(&mut builder, regs_ptr, *rhs);
                    let cmp = builder.ins().icmp(IntCC::SignedLessThan, v1, v2);
                    let res = builder.ins().uextend(types::I64, cmp);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::CmpLtU { dst, lhs, rhs } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *lhs);
                    let v2 = Self::load_reg(&mut builder, regs_ptr, *rhs);
                    let cmp = builder.ins().icmp(IntCC::UnsignedLessThan, v1, v2);
                    let res = builder.ins().uextend(types::I64, cmp);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::CmpGe { dst, lhs, rhs } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *lhs);
                    let v2 = Self::load_reg(&mut builder, regs_ptr, *rhs);
                    let cmp = builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, v1, v2);
                    let res = builder.ins().uextend(types::I64, cmp);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::CmpGeU { dst, lhs, rhs } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *lhs);
                    let v2 = Self::load_reg(&mut builder, regs_ptr, *rhs);
                    let cmp = builder.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, v1, v2);
                    let res = builder.ins().uextend(types::I64, cmp);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }

                // ============================================================
                // 内存访问 (Memory Access)
                // ============================================================
                IROp::Load { dst, base, offset, size, flags } => {
                    let base_val = Self::load_reg(&mut builder, regs_ptr, *base);
                    let offset_val = builder.ins().iconst(types::I64, *offset);
                    let vaddr = builder.ins().iadd(base_val, offset_val);
                    if matches!(flags.order, vm_ir::MemOrder::Acquire | vm_ir::MemOrder::AcqRel) {
                        let sig = self.module.make_signature();
                        let func_id = self.module.declare_function("barrier_acquire", Linkage::Import, &sig).unwrap();
                        let funcref = self.module.declare_func_in_func(func_id, builder.func);
                        let _ = builder.ins().call(funcref, &[]);
                    }
                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64)); // ctx
                    sig.params.push(AbiParam::new(types::I64)); // vaddr
                    sig.params.push(AbiParam::new(types::I8));  // size
                    sig.returns.push(AbiParam::new(types::I64));
                    let func_id = self.module.declare_function("jit_read", Linkage::Import, &sig).unwrap();
                    let funcref = self.module.declare_func_in_func(func_id, builder.func);
                    let size_val = builder.ins().iconst(types::I8, *size as i64);
                    let call = builder.ins().call(funcref, &[ctx_ptr, vaddr, size_val]);
                    let res64 = builder.inst_results(call)[0];
                    Self::store_reg(&mut builder, regs_ptr, *dst, res64);
                }
                IROp::Store { src, base, offset, size, flags } => {
                    let base_val = Self::load_reg(&mut builder, regs_ptr, *base);
                    let offset_val = builder.ins().iconst(types::I64, *offset);
                    let vaddr = builder.ins().iadd(base_val, offset_val);
                    let val = Self::load_reg(&mut builder, regs_ptr, *src);
                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64)); // ctx
                    sig.params.push(AbiParam::new(types::I64)); // vaddr
                    sig.params.push(AbiParam::new(types::I64)); // value
                    sig.params.push(AbiParam::new(types::I8));  // size
                    let func_id = self.module.declare_function("jit_write", Linkage::Import, &sig).unwrap();
                    let funcref = self.module.declare_func_in_func(func_id, builder.func);
                    let size_val = builder.ins().iconst(types::I8, *size as i64);
                    let _ = builder.ins().call(funcref, &[ctx_ptr, vaddr, val, size_val]);
                    if matches!(flags.order, vm_ir::MemOrder::Release | vm_ir::MemOrder::AcqRel) {
                        let sig2 = self.module.make_signature();
                        let func_id2 = self.module.declare_function("barrier_release", Linkage::Import, &sig2).unwrap();
                        let funcref2 = self.module.declare_func_in_func(func_id2, builder.func);
                        let _ = builder.ins().call(funcref2, &[]);
                    }
                }

                // ============================================================
                // LR/SC (Load-Reserved / Store-Conditional)
                // ============================================================
                IROp::AtomicLoadReserve { dst, base, offset, size, flags } => {
                    let base_val = Self::load_reg(&mut builder, regs_ptr, *base);
                    let offset_val = builder.ins().iconst(types::I64, *offset);
                    let vaddr = builder.ins().iadd(base_val, offset_val);
                    if matches!(flags.order, vm_ir::MemOrder::Acquire | vm_ir::MemOrder::AcqRel) || flags.fence_before {
                        let sigb = self.module.make_signature();
                        let fidb = self.module.declare_function("barrier_acquire", Linkage::Import, &sigb).unwrap();
                        let frb = self.module.declare_func_in_func(fidb, builder.func);
                        let _ = builder.ins().call(frb, &[]);
                    }
                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64)); // ctx
                    sig.params.push(AbiParam::new(types::I64)); // vaddr
                    sig.params.push(AbiParam::new(types::I8));  // size
                    sig.returns.push(AbiParam::new(types::I64));
                    let func_id = self.module.declare_function("jit_lr", Linkage::Import, &sig).unwrap();
                    let funcref = self.module.declare_func_in_func(func_id, builder.func);
                    let size_val = builder.ins().iconst(types::I8, *size as i64);
                    let call = builder.ins().call(funcref, &[ctx_ptr, vaddr, size_val]);
                    let res64 = builder.inst_results(call)[0];
                    Self::store_reg(&mut builder, regs_ptr, *dst, res64);
                }
                IROp::AtomicStoreCond { src, base, offset, size, dst_flag, flags } => {
                    let base_val = Self::load_reg(&mut builder, regs_ptr, *base);
                    let offset_val = builder.ins().iconst(types::I64, *offset);
                    let vaddr = builder.ins().iadd(base_val, offset_val);
                    let val = Self::load_reg(&mut builder, regs_ptr, *src);
                    if matches!(flags.order, vm_ir::MemOrder::SeqCst) {
                        let sigf = self.module.make_signature();
                        let fidf = self.module.declare_function("barrier_full", Linkage::Import, &sigf).unwrap();
                        let frf = self.module.declare_func_in_func(fidf, builder.func);
                        let _ = builder.ins().call(frf, &[]);
                    }
                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64)); // ctx
                    sig.params.push(AbiParam::new(types::I64)); // vaddr
                    sig.params.push(AbiParam::new(types::I64)); // value
                    sig.params.push(AbiParam::new(types::I8));  // size
                    sig.returns.push(AbiParam::new(types::I64));
                    let func_id = self.module.declare_function("jit_sc", Linkage::Import, &sig).unwrap();
                    let funcref = self.module.declare_func_in_func(func_id, builder.func);
                    let size_val = builder.ins().iconst(types::I8, *size as i64);
                    let call = builder.ins().call(funcref, &[ctx_ptr, vaddr, val, size_val]);
                    let ok = builder.inst_results(call)[0];
                    let ok64 = builder.ins().uextend(types::I64, ok);
                    Self::store_reg(&mut builder, regs_ptr, *dst_flag, ok64);
                    if matches!(flags.order, vm_ir::MemOrder::Release | vm_ir::MemOrder::AcqRel) || flags.fence_after {
                        let siga = self.module.make_signature();
                        let fida = self.module.declare_function("barrier_release", Linkage::Import, &siga).unwrap();
                        let fra = self.module.declare_func_in_func(fida, builder.func);
                        let _ = builder.ins().call(fra, &[]);
                    }
                }

                // ============================================================
                // NOP 和其他
                // ============================================================
                IROp::Nop => {
                    // 不执行任何操作
                }

                // ============================================================
                // 向量操作 (64-bit packed)
                // 注意：Cranelift 原生向量支持有限，这里使用标量模拟
                // ============================================================
                IROp::VecAdd { dst, src1, src2, element_size } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *src1);
                    let v2 = Self::load_reg(&mut builder, regs_ptr, *src2);
                    let res = self.call_simd_intrinsic(&mut builder, SimdIntrinsic::Add, v1, v2, *element_size);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::VecSub { dst, src1, src2, element_size } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *src1);
                    let v2 = Self::load_reg(&mut builder, regs_ptr, *src2);
                    let res = self.call_simd_intrinsic(&mut builder, SimdIntrinsic::Sub, v1, v2, *element_size);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::VecMul { dst, src1, src2, element_size } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *src1);
                    let v2 = Self::load_reg(&mut builder, regs_ptr, *src2);
                    let res = self.call_simd_intrinsic(&mut builder, SimdIntrinsic::Mul, v1, v2, *element_size);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }

                // ============================================================
                // 原子操作 (Atomic Operations) - 使用 Cranelift 原子指令
                // ============================================================
                IROp::AtomicRMW { dst, base, src, op, size } => {
                    let addr = Self::load_reg(&mut builder, regs_ptr, *base);
                    let val = Self::load_reg(&mut builder, regs_ptr, *src);
                    
                    // 确定操作类型
                    let atomic_type = match size {
                        1 => types::I8,
                        2 => types::I16,
                        4 => types::I32,
                        _ => types::I64,
                    };
                    
                    // 截断值到正确大小
                    let val_sized = if *size < 8 {
                        builder.ins().ireduce(atomic_type, val)
                    } else {
                        val
                    };
                    
                    // 设置原子内存标志 (SeqCst 内存序)
                    let atomic_flags = MemFlags::trusted();
                    
                    // 使用 Cranelift 原子 RMW 指令
                    let old_val = match op {
                        AtomicOp::Add => {
                            builder.ins().atomic_rmw(atomic_type, atomic_flags, AtomicRmwOp::Add, addr, val_sized)
                        }
                        AtomicOp::Sub => {
                            builder.ins().atomic_rmw(atomic_type, atomic_flags, AtomicRmwOp::Sub, addr, val_sized)
                        }
                        AtomicOp::And => {
                            builder.ins().atomic_rmw(atomic_type, atomic_flags, AtomicRmwOp::And, addr, val_sized)
                        }
                        AtomicOp::Or => {
                            builder.ins().atomic_rmw(atomic_type, atomic_flags, AtomicRmwOp::Or, addr, val_sized)
                        }
                        AtomicOp::Xor => {
                            builder.ins().atomic_rmw(atomic_type, atomic_flags, AtomicRmwOp::Xor, addr, val_sized)
                        }
                        AtomicOp::Xchg | AtomicOp::Swap => {
                            builder.ins().atomic_rmw(atomic_type, atomic_flags, AtomicRmwOp::Xchg, addr, val_sized)
                        }
                        AtomicOp::Min | AtomicOp::Minu => {
                            builder.ins().atomic_rmw(atomic_type, atomic_flags, AtomicRmwOp::Umin, addr, val_sized)
                        }
                        AtomicOp::Max | AtomicOp::Maxu => {
                            builder.ins().atomic_rmw(atomic_type, atomic_flags, AtomicRmwOp::Umax, addr, val_sized)
                        }
                        AtomicOp::MinS => {
                            builder.ins().atomic_rmw(atomic_type, atomic_flags, AtomicRmwOp::Smin, addr, val_sized)
                        }
                        AtomicOp::MaxS => {
                            builder.ins().atomic_rmw(atomic_type, atomic_flags, AtomicRmwOp::Smax, addr, val_sized)
                        }
                        _ => {
                            // CmpXchg 通过单独的操作处理
                            builder.ins().atomic_rmw(atomic_type, atomic_flags, AtomicRmwOp::Xchg, addr, val_sized)
                        }
                    };
                    
                    // 扩展返回值到 64 位
                    let old_val_ext = if *size < 8 {
                        builder.ins().uextend(types::I64, old_val)
                    } else {
                        old_val
                    };
                    
                    Self::store_reg(&mut builder, regs_ptr, *dst, old_val_ext);
                }
                IROp::AtomicRMWOrder { dst, base, src, op, size, flags } => {
                    let addr = Self::load_reg(&mut builder, regs_ptr, *base);
                    let val = Self::load_reg(&mut builder, regs_ptr, *src);
                    if matches!(flags.order, vm_ir::MemOrder::Acquire | vm_ir::MemOrder::AcqRel) || flags.fence_before {
                        let sigb = self.module.make_signature();
                        let fidb = self.module.declare_function("barrier_acquire", Linkage::Import, &sigb).unwrap();
                        let frb = self.module.declare_func_in_func(fidb, builder.func);
                        let _ = builder.ins().call(frb, &[]);
                    }
                    let atomic_type = match size {
                        1 => types::I8,
                        2 => types::I16,
                        4 => types::I32,
                        _ => types::I64,
                    };
                    let val_sized = if *size < 8 { builder.ins().ireduce(atomic_type, val) } else { val };
                    let flags_m = MemFlags::trusted();
                    let old_val = match op {
                        AtomicOp::Add => builder.ins().atomic_rmw(atomic_type, flags_m, AtomicRmwOp::Add, addr, val_sized),
                        AtomicOp::Sub => builder.ins().atomic_rmw(atomic_type, flags_m, AtomicRmwOp::Sub, addr, val_sized),
                        AtomicOp::And => builder.ins().atomic_rmw(atomic_type, flags_m, AtomicRmwOp::And, addr, val_sized),
                        AtomicOp::Or  => builder.ins().atomic_rmw(atomic_type, flags_m, AtomicRmwOp::Or, addr, val_sized),
                        AtomicOp::Xor => builder.ins().atomic_rmw(atomic_type, flags_m, AtomicRmwOp::Xor, addr, val_sized),
                        AtomicOp::Xchg | AtomicOp::Swap => builder.ins().atomic_rmw(atomic_type, flags_m, AtomicRmwOp::Xchg, addr, val_sized),
                        AtomicOp::Min  => builder.ins().atomic_rmw(atomic_type, flags_m, AtomicRmwOp::Umin, addr, val_sized),
                        AtomicOp::Max  => builder.ins().atomic_rmw(atomic_type, flags_m, AtomicRmwOp::Umax, addr, val_sized),
                        AtomicOp::MinS => builder.ins().atomic_rmw(atomic_type, flags_m, AtomicRmwOp::Smin, addr, val_sized),
                        AtomicOp::MaxS => builder.ins().atomic_rmw(atomic_type, flags_m, AtomicRmwOp::Smax, addr, val_sized),
                        _ => builder.ins().atomic_rmw(atomic_type, flags_m, AtomicRmwOp::Xchg, addr, val_sized),
                    };
                    let old_ext = if *size < 8 { builder.ins().uextend(types::I64, old_val) } else { old_val };
                    Self::store_reg(&mut builder, regs_ptr, *dst, old_ext);
                    if matches!(flags.order, vm_ir::MemOrder::Release | vm_ir::MemOrder::AcqRel) || flags.fence_after {
                        let siga = self.module.make_signature();
                        let fida = self.module.declare_function("barrier_release", Linkage::Import, &siga).unwrap();
                        let fra = self.module.declare_func_in_func(fida, builder.func);
                        let _ = builder.ins().call(fra, &[]);
                    }
                }
                IROp::AtomicCmpXchg { dst, base, expected, new, size } => {
                    let base_val = Self::load_reg(&mut builder, regs_ptr, *base);
                    let exp = Self::load_reg(&mut builder, regs_ptr, *expected);
                    let new_val = Self::load_reg(&mut builder, regs_ptr, *new);
                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64)); // ctx
                    sig.params.push(AbiParam::new(types::I64)); // vaddr
                    sig.params.push(AbiParam::new(types::I64)); // expected
                    sig.params.push(AbiParam::new(types::I64)); // new
                    sig.params.push(AbiParam::new(types::I8));  // size
                    sig.returns.push(AbiParam::new(types::I64));
                    let func_id = self.module.declare_function("jit_cas", Linkage::Import, &sig).unwrap();
                    let funcref = self.module.declare_func_in_func(func_id, builder.func);
                    let size_val = builder.ins().iconst(types::I8, *size as i64);
                    let call = builder.ins().call(funcref, &[ctx_ptr, base_val, exp, new_val, size_val]);
                    let old_val = builder.inst_results(call)[0];
                    Self::store_reg(&mut builder, regs_ptr, *dst, old_val);
                }
                IROp::AtomicCmpXchgOrder { dst, base, expected, new, size, flags } => {
                    let base_val = Self::load_reg(&mut builder, regs_ptr, *base);
                    let exp = Self::load_reg(&mut builder, regs_ptr, *expected);
                    let new_val = Self::load_reg(&mut builder, regs_ptr, *new);
                    if matches!(flags.order, vm_ir::MemOrder::Acquire) || flags.fence_before {
                        let sigb = self.module.make_signature();
                        let fidb = self.module.declare_function("barrier_acquire", Linkage::Import, &sigb).unwrap();
                        let frb = self.module.declare_func_in_func(fidb, builder.func);
                        let _ = builder.ins().call(frb, &[]);
                    }
                    if matches!(flags.order, vm_ir::MemOrder::SeqCst) {
                        let sigf = self.module.make_signature();
                        let fidf = self.module.declare_function("barrier_full", Linkage::Import, &sigf).unwrap();
                        let frf = self.module.declare_func_in_func(fidf, builder.func);
                        let _ = builder.ins().call(frf, &[]);
                    }
                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64));
                    sig.params.push(AbiParam::new(types::I64));
                    sig.params.push(AbiParam::new(types::I64));
                    sig.params.push(AbiParam::new(types::I64));
                    sig.params.push(AbiParam::new(types::I8));
                    sig.returns.push(AbiParam::new(types::I64));
                    let func_id = self.module.declare_function("jit_cas", Linkage::Import, &sig).unwrap();
                    let funcref = self.module.declare_func_in_func(func_id, builder.func);
                    let size_val = builder.ins().iconst(types::I8, *size as i64);
                    let call = builder.ins().call(funcref, &[ctx_ptr, base_val, exp, new_val, size_val]);
                    let old_val = builder.inst_results(call)[0];
                    Self::store_reg(&mut builder, regs_ptr, *dst, old_val);
                    if matches!(flags.order, vm_ir::MemOrder::Release) || matches!(flags.order, vm_ir::MemOrder::AcqRel) || flags.fence_after {
                        let siga = self.module.make_signature();
                        let fida = self.module.declare_function("barrier_release", Linkage::Import, &siga).unwrap();
                        let fra = self.module.declare_func_in_func(fida, builder.func);
                        let _ = builder.ins().call(fra, &[]);
                    }
                    if matches!(flags.order, vm_ir::MemOrder::SeqCst) {
                        let sigf2 = self.module.make_signature();
                        let fidf2 = self.module.declare_function("barrier_full", Linkage::Import, &sigf2).unwrap();
                        let frf2 = self.module.declare_func_in_func(fidf2, builder.func);
                        let _ = builder.ins().call(frf2, &[]);
                    }
                }

                // ============================================================
                // 浮点运算 (Floating Point Operations)
                // ============================================================
                IROp::Fadd { dst, src1, src2 } => {
                    let v1 = Self::load_freg(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg(&mut builder, fregs_ptr, *src2);
                    let res = builder.ins().fadd(v1, v2);
                    Self::store_freg(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::Fsub { dst, src1, src2 } => {
                    let v1 = Self::load_freg(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg(&mut builder, fregs_ptr, *src2);
                    let res = builder.ins().fsub(v1, v2);
                    Self::store_freg(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::Fmul { dst, src1, src2 } => {
                    let v1 = Self::load_freg(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg(&mut builder, fregs_ptr, *src2);
                    let res = builder.ins().fmul(v1, v2);
                    Self::store_freg(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::Fdiv { dst, src1, src2 } => {
                    let v1 = Self::load_freg(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg(&mut builder, fregs_ptr, *src2);
                    let res = builder.ins().fdiv(v1, v2);
                    Self::store_freg(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::Fsqrt { dst, src } => {
                    let v = Self::load_freg(&mut builder, fregs_ptr, *src);
                    let res = builder.ins().sqrt(v);
                    Self::store_freg(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::Fmin { dst, src1, src2 } => {
                    let v1 = Self::load_freg(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg(&mut builder, fregs_ptr, *src2);
                    let res = builder.ins().fmin(v1, v2);
                    Self::store_freg(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::Fmax { dst, src1, src2 } => {
                    let v1 = Self::load_freg(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg(&mut builder, fregs_ptr, *src2);
                    let res = builder.ins().fmax(v1, v2);
                    Self::store_freg(&mut builder, fregs_ptr, *dst, res);
                }
                
                // ============================================================
                // 单精度浮点运算 (Single Precision FP Operations)
                // ============================================================
                IROp::FaddS { dst, src1, src2 } => {
                    let v1 = Self::load_freg_f32(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg_f32(&mut builder, fregs_ptr, *src2);
                    let res = builder.ins().fadd(v1, v2);
                    Self::store_freg_f32(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::FsubS { dst, src1, src2 } => {
                    let v1 = Self::load_freg_f32(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg_f32(&mut builder, fregs_ptr, *src2);
                    let res = builder.ins().fsub(v1, v2);
                    Self::store_freg_f32(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::FmulS { dst, src1, src2 } => {
                    let v1 = Self::load_freg_f32(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg_f32(&mut builder, fregs_ptr, *src2);
                    let res = builder.ins().fmul(v1, v2);
                    Self::store_freg_f32(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::FdivS { dst, src1, src2 } => {
                    let v1 = Self::load_freg_f32(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg_f32(&mut builder, fregs_ptr, *src2);
                    let res = builder.ins().fdiv(v1, v2);
                    Self::store_freg_f32(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::FsqrtS { dst, src } => {
                    let v = Self::load_freg_f32(&mut builder, fregs_ptr, *src);
                    let res = builder.ins().sqrt(v);
                    Self::store_freg_f32(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::FminS { dst, src1, src2 } => {
                    let v1 = Self::load_freg_f32(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg_f32(&mut builder, fregs_ptr, *src2);
                    let res = builder.ins().fmin(v1, v2);
                    Self::store_freg_f32(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::FmaxS { dst, src1, src2 } => {
                    let v1 = Self::load_freg_f32(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg_f32(&mut builder, fregs_ptr, *src2);
                    let res = builder.ins().fmax(v1, v2);
                    Self::store_freg_f32(&mut builder, fregs_ptr, *dst, res);
                }
                
                // ============================================================
                // 融合乘加运算 (Fused Multiply-Add Operations)
                // ============================================================
                IROp::Fmadd { dst, src1, src2, src3 } => {
                    let v1 = Self::load_freg(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg(&mut builder, fregs_ptr, *src2);
                    let v3 = Self::load_freg(&mut builder, fregs_ptr, *src3);
                    let res = builder.ins().fma(v1, v2, v3);
                    Self::store_freg(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::Fmsub { dst, src1, src2, src3 } => {
                    let v1 = Self::load_freg(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg(&mut builder, fregs_ptr, *src2);
                    let v3 = Self::load_freg(&mut builder, fregs_ptr, *src3);
                    let neg_v3 = builder.ins().fneg(v3);
                    let res = builder.ins().fma(v1, v2, neg_v3);
                    Self::store_freg(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::Fnmadd { dst, src1, src2, src3 } => {
                    let v1 = Self::load_freg(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg(&mut builder, fregs_ptr, *src2);
                    let v3 = Self::load_freg(&mut builder, fregs_ptr, *src3);
                    let neg_v1 = builder.ins().fneg(v1);
                    let neg_v3 = builder.ins().fneg(v3);
                    let res = builder.ins().fma(neg_v1, v2, neg_v3);
                    Self::store_freg(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::Fnmsub { dst, src1, src2, src3 } => {
                    let v1 = Self::load_freg(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg(&mut builder, fregs_ptr, *src2);
                    let v3 = Self::load_freg(&mut builder, fregs_ptr, *src3);
                    let neg_v1 = builder.ins().fneg(v1);
                    let res = builder.ins().fma(neg_v1, v2, v3);
                    Self::store_freg(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::FmaddS { dst, src1, src2, src3 } => {
                    let v1 = Self::load_freg_f32(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg_f32(&mut builder, fregs_ptr, *src2);
                    let v3 = Self::load_freg_f32(&mut builder, fregs_ptr, *src3);
                    let res = builder.ins().fma(v1, v2, v3);
                    Self::store_freg_f32(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::FmsubS { dst, src1, src2, src3 } => {
                    let v1 = Self::load_freg_f32(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg_f32(&mut builder, fregs_ptr, *src2);
                    let v3 = Self::load_freg_f32(&mut builder, fregs_ptr, *src3);
                    let neg_v3 = builder.ins().fneg(v3);
                    let res = builder.ins().fma(v1, v2, neg_v3);
                    Self::store_freg_f32(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::FnmaddS { dst, src1, src2, src3 } => {
                    let v1 = Self::load_freg_f32(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg_f32(&mut builder, fregs_ptr, *src2);
                    let v3 = Self::load_freg_f32(&mut builder, fregs_ptr, *src3);
                    let neg_v1 = builder.ins().fneg(v1);
                    let neg_v3 = builder.ins().fneg(v3);
                    let res = builder.ins().fma(neg_v1, v2, neg_v3);
                    Self::store_freg_f32(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::FnmsubS { dst, src1, src2, src3 } => {
                    let v1 = Self::load_freg_f32(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg_f32(&mut builder, fregs_ptr, *src2);
                    let v3 = Self::load_freg_f32(&mut builder, fregs_ptr, *src3);
                    let neg_v1 = builder.ins().fneg(v1);
                    let res = builder.ins().fma(neg_v1, v2, v3);
                    Self::store_freg_f32(&mut builder, fregs_ptr, *dst, res);
                }
                
                // ============================================================
                // 浮点比较运算 (Floating Point Comparisons)
                // ============================================================
                IROp::Feq { dst, src1, src2 } => {
                    let v1 = Self::load_freg(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg(&mut builder, fregs_ptr, *src2);
                    let cmp = builder.ins().fcmp(FloatCC::Equal, v1, v2);
                    let res = builder.ins().uextend(types::I64, cmp);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::Flt { dst, src1, src2 } => {
                    let v1 = Self::load_freg(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg(&mut builder, fregs_ptr, *src2);
                    let cmp = builder.ins().fcmp(FloatCC::LessThan, v1, v2);
                    let res = builder.ins().uextend(types::I64, cmp);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::Fle { dst, src1, src2 } => {
                    let v1 = Self::load_freg(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg(&mut builder, fregs_ptr, *src2);
                    let cmp = builder.ins().fcmp(FloatCC::LessThanOrEqual, v1, v2);
                    let res = builder.ins().uextend(types::I64, cmp);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::FeqS { dst, src1, src2 } => {
                    let v1 = Self::load_freg_f32(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg_f32(&mut builder, fregs_ptr, *src2);
                    let cmp = builder.ins().fcmp(FloatCC::Equal, v1, v2);
                    let res = builder.ins().uextend(types::I64, cmp);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::FltS { dst, src1, src2 } => {
                    let v1 = Self::load_freg_f32(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg_f32(&mut builder, fregs_ptr, *src2);
                    let cmp = builder.ins().fcmp(FloatCC::LessThan, v1, v2);
                    let res = builder.ins().uextend(types::I64, cmp);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::FleS { dst, src1, src2 } => {
                    let v1 = Self::load_freg_f32(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg_f32(&mut builder, fregs_ptr, *src2);
                    let cmp = builder.ins().fcmp(FloatCC::LessThanOrEqual, v1, v2);
                    let res = builder.ins().uextend(types::I64, cmp);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                
                // ============================================================
                // 浮点转换运算 (Floating Point Conversions)
                // ============================================================
                IROp::Fcvtws { dst, src } => {
                    let v = Self::load_freg_f32(&mut builder, fregs_ptr, *src);
                    let res = builder.ins().fcvt_to_sint(types::I32, v);
                    let ext = builder.ins().sextend(types::I64, res);
                    Self::store_reg(&mut builder, regs_ptr, *dst, ext);
                }
                IROp::Fcvtwus { dst, src } => {
                    let v = Self::load_freg_f32(&mut builder, fregs_ptr, *src);
                    let res = builder.ins().fcvt_to_uint(types::I32, v);
                    let ext = builder.ins().uextend(types::I64, res);
                    Self::store_reg(&mut builder, regs_ptr, *dst, ext);
                }
                IROp::Fcvtls { dst, src } => {
                    let v = Self::load_freg_f32(&mut builder, fregs_ptr, *src);
                    let res = builder.ins().fcvt_to_sint(types::I64, v);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::Fcvtlus { dst, src } => {
                    let v = Self::load_freg_f32(&mut builder, fregs_ptr, *src);
                    let res = builder.ins().fcvt_to_uint(types::I64, v);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::Fcvtsw { dst, src } => {
                    let v = Self::load_reg(&mut builder, regs_ptr, *src);
                    let i32_val = builder.ins().ireduce(types::I32, v);
                    let res = builder.ins().fcvt_from_sint(types::F32, i32_val);
                    Self::store_freg_f32(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::Fcvtswu { dst, src } => {
                    let v = Self::load_reg(&mut builder, regs_ptr, *src);
                    let i32_val = builder.ins().ireduce(types::I32, v);
                    let res = builder.ins().fcvt_from_uint(types::F32, i32_val);
                    Self::store_freg_f32(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::Fcvtsl { dst, src } => {
                    let v = Self::load_reg(&mut builder, regs_ptr, *src);
                    let res = builder.ins().fcvt_from_sint(types::F32, v);
                    Self::store_freg_f32(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::Fcvtslu { dst, src } => {
                    let v = Self::load_reg(&mut builder, regs_ptr, *src);
                    let res = builder.ins().fcvt_from_uint(types::F32, v);
                    Self::store_freg_f32(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::Fcvtwd { dst, src } => {
                    let v = Self::load_freg(&mut builder, fregs_ptr, *src);
                    let res = builder.ins().fcvt_to_sint(types::I32, v);
                    let ext = builder.ins().sextend(types::I64, res);
                    Self::store_reg(&mut builder, regs_ptr, *dst, ext);
                }
                IROp::Fcvtwud { dst, src } => {
                    let v = Self::load_freg(&mut builder, fregs_ptr, *src);
                    let res = builder.ins().fcvt_to_uint(types::I32, v);
                    let ext = builder.ins().uextend(types::I64, res);
                    Self::store_reg(&mut builder, regs_ptr, *dst, ext);
                }
                IROp::Fcvtld { dst, src } => {
                    let v = Self::load_freg(&mut builder, fregs_ptr, *src);
                    let res = builder.ins().fcvt_to_sint(types::I64, v);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::Fcvtlud { dst, src } => {
                    let v = Self::load_freg(&mut builder, fregs_ptr, *src);
                    let res = builder.ins().fcvt_to_uint(types::I64, v);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::Fcvtdw { dst, src } => {
                    let v = Self::load_reg(&mut builder, regs_ptr, *src);
                    let i32_val = builder.ins().ireduce(types::I32, v);
                    let res = builder.ins().fcvt_from_sint(types::F64, i32_val);
                    Self::store_freg(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::Fcvtdwu { dst, src } => {
                    let v = Self::load_reg(&mut builder, regs_ptr, *src);
                    let i32_val = builder.ins().ireduce(types::I32, v);
                    let res = builder.ins().fcvt_from_uint(types::F64, i32_val);
                    Self::store_freg(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::Fcvtdl { dst, src } => {
                    let v = Self::load_reg(&mut builder, regs_ptr, *src);
                    let res = builder.ins().fcvt_from_sint(types::F64, v);
                    Self::store_freg(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::Fcvtdlu { dst, src } => {
                    let v = Self::load_reg(&mut builder, regs_ptr, *src);
                    let res = builder.ins().fcvt_from_uint(types::F64, v);
                    Self::store_freg(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::Fcvtsd { dst, src } => {
                    let v = Self::load_freg(&mut builder, fregs_ptr, *src);
                    let res = builder.ins().fdemote(types::F32, v);
                    Self::store_freg_f32(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::Fcvtds { dst, src } => {
                    let v = Self::load_freg_f32(&mut builder, fregs_ptr, *src);
                    let res = builder.ins().fpromote(types::F64, v);
                    Self::store_freg(&mut builder, fregs_ptr, *dst, res);
                }
                
                // ============================================================
                // 浮点符号操作 (Floating Point Sign Operations)
                // ============================================================
                IROp::Fabs { dst, src } => {
                    let v = Self::load_freg(&mut builder, fregs_ptr, *src);
                    let res = builder.ins().fabs(v);
                    Self::store_freg(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::Fneg { dst, src } => {
                    let v = Self::load_freg(&mut builder, fregs_ptr, *src);
                    let res = builder.ins().fneg(v);
                    Self::store_freg(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::FabsS { dst, src } => {
                    let v = Self::load_freg_f32(&mut builder, fregs_ptr, *src);
                    let res = builder.ins().fabs(v);
                    Self::store_freg_f32(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::FnegS { dst, src } => {
                    let v = Self::load_freg_f32(&mut builder, fregs_ptr, *src);
                    let res = builder.ins().fneg(v);
                    Self::store_freg_f32(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::Fsgnj { dst, src1, src2 } => {
                    let v1 = Self::load_freg(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg(&mut builder, fregs_ptr, *src2);
                    let res = builder.ins().fcopysign(v1, v2);
                    Self::store_freg(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::Fsgnjn { dst, src1, src2 } => {
                    let v1 = Self::load_freg(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg(&mut builder, fregs_ptr, *src2);
                    let neg_v2 = builder.ins().fneg(v2);
                    let res = builder.ins().fcopysign(v1, neg_v2);
                    Self::store_freg(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::Fsgnjx { dst, src1, src2 } => {
                    // XOR of signs: if signs differ, negate v1
                    let v1 = Self::load_freg(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg(&mut builder, fregs_ptr, *src2);
                    // Convert to bits, XOR sign bits, convert back
                    let bits1 = builder.ins().bitcast(types::I64, MemFlags::new(), v1);
                    let bits2 = builder.ins().bitcast(types::I64, MemFlags::new(), v2);
                    let sign_mask = builder.ins().iconst(types::I64, 1i64 << 63);
                    let xor_bits = builder.ins().bxor(bits1, bits2);
                    let sign_xor = builder.ins().band(xor_bits, sign_mask);
                    let result_bits = builder.ins().bxor(bits1, sign_xor);
                    let res = builder.ins().bitcast(types::F64, MemFlags::new(), result_bits);
                    Self::store_freg(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::FsgnjS { dst, src1, src2 } => {
                    let v1 = Self::load_freg_f32(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg_f32(&mut builder, fregs_ptr, *src2);
                    let res = builder.ins().fcopysign(v1, v2);
                    Self::store_freg_f32(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::FsgnjnS { dst, src1, src2 } => {
                    let v1 = Self::load_freg_f32(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg_f32(&mut builder, fregs_ptr, *src2);
                    let neg_v2 = builder.ins().fneg(v2);
                    let res = builder.ins().fcopysign(v1, neg_v2);
                    Self::store_freg_f32(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::FsgnjxS { dst, src1, src2 } => {
                    let v1 = Self::load_freg_f32(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg_f32(&mut builder, fregs_ptr, *src2);
                    let bits1 = builder.ins().bitcast(types::I32, MemFlags::new(), v1);
                    let bits2 = builder.ins().bitcast(types::I32, MemFlags::new(), v2);
                    let sign_mask = builder.ins().iconst(types::I32, 1i64 << 31);
                    let xor_bits = builder.ins().bxor(bits1, bits2);
                    let sign_xor = builder.ins().band(xor_bits, sign_mask);
                    let result_bits = builder.ins().bxor(bits1, sign_xor);
                    let res = builder.ins().bitcast(types::F32, MemFlags::new(), result_bits);
                    Self::store_freg_f32(&mut builder, fregs_ptr, *dst, res);
                }
                
                // ============================================================
                // 浮点寄存器移动 (Float-Integer Move Operations)
                // ============================================================
                IROp::FmvXW { dst, src } => {
                    let v = Self::load_freg_f32(&mut builder, fregs_ptr, *src);
                    let bits = builder.ins().bitcast(types::I32, MemFlags::new(), v);
                    let ext = builder.ins().sextend(types::I64, bits);
                    Self::store_reg(&mut builder, regs_ptr, *dst, ext);
                }
                IROp::FmvWX { dst, src } => {
                    let v = Self::load_reg(&mut builder, regs_ptr, *src);
                    let truncated = builder.ins().ireduce(types::I32, v);
                    let res = builder.ins().bitcast(types::F32, MemFlags::new(), truncated);
                    Self::store_freg_f32(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::FmvXD { dst, src } => {
                    let v = Self::load_freg(&mut builder, fregs_ptr, *src);
                    let bits = builder.ins().bitcast(types::I64, MemFlags::new(), v);
                    Self::store_reg(&mut builder, regs_ptr, *dst, bits);
                }
                IROp::FmvDX { dst, src } => {
                    let v = Self::load_reg(&mut builder, regs_ptr, *src);
                    let res = builder.ins().bitcast(types::F64, MemFlags::new(), v);
                    Self::store_freg(&mut builder, fregs_ptr, *dst, res);
                }
                
                // ============================================================
                // 浮点加载/存储 (Floating Point Load/Store)
                // ============================================================
                IROp::Fload { dst, base, offset, size } => {
                    let base_val = Self::load_reg(&mut builder, regs_ptr, *base);
                    let offset_val = builder.ins().iconst(types::I64, *offset);
                    let vaddr = builder.ins().iadd(base_val, offset_val);
                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64));
                    sig.params.push(AbiParam::new(types::I64));
                    sig.params.push(AbiParam::new(types::I8));
                    sig.returns.push(AbiParam::new(types::I64));
                    let func_id = self.module.declare_function("jit_read", Linkage::Import, &sig).unwrap();
                    let funcref = self.module.declare_func_in_func(func_id, builder.func);
                    let size_val = builder.ins().iconst(types::I8, *size as i64);
                    let call = builder.ins().call(funcref, &[ctx_ptr, vaddr, size_val]);
                    let res64 = builder.inst_results(call)[0];
                    if *size <= 4 {
                        let i32v = builder.ins().ireduce(types::I32, res64);
                        let fv = builder.ins().bitcast(types::F32, MemFlags::new(), i32v);
                        Self::store_freg_f32(&mut builder, fregs_ptr, *dst, fv);
                    } else {
                        let fv = builder.ins().bitcast(types::F64, MemFlags::new(), res64);
                        Self::store_freg(&mut builder, fregs_ptr, *dst, fv);
                    }
                }
                IROp::Fstore { src, base, offset, size } => {
                    let base_val = Self::load_reg(&mut builder, regs_ptr, *base);
                    let offset_val = builder.ins().iconst(types::I64, *offset);
                    let vaddr = builder.ins().iadd(base_val, offset_val);
                    let iv = if *size <= 4 {
                        let fv = Self::load_freg_f32(&mut builder, fregs_ptr, *src);
                        let bits = builder.ins().bitcast(types::I32, MemFlags::new(), fv);
                        builder.ins().uextend(types::I64, bits)
                    } else {
                        let fv = Self::load_freg(&mut builder, fregs_ptr, *src);
                        builder.ins().bitcast(types::I64, MemFlags::new(), fv)
                    };
                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64));
                    sig.params.push(AbiParam::new(types::I64));
                    sig.params.push(AbiParam::new(types::I64));
                    sig.params.push(AbiParam::new(types::I8));
                    let func_id = self.module.declare_function("jit_write", Linkage::Import, &sig).unwrap();
                    let funcref = self.module.declare_func_in_func(func_id, builder.func);
                    let size_val = builder.ins().iconst(types::I8, *size as i64);
                    let _ = builder.ins().call(funcref, &[ctx_ptr, vaddr, iv, size_val]);
                }
                
                // ============================================================
                // 浮点分类 (Floating Point Classification) - 简化实现
                // ============================================================
                IROp::Fclass { dst, src } => {
                    // 简化实现：返回基本分类位
                    let v = Self::load_freg(&mut builder, fregs_ptr, *src);
                    let bits = builder.ins().bitcast(types::I64, MemFlags::new(), v);
                    // 基本分类：检查是否为零
                    let zero = builder.ins().iconst(types::I64, 0);
                    let is_zero = builder.ins().icmp(IntCC::Equal, bits, zero);
                    let result = builder.ins().uextend(types::I64, is_zero);
                    Self::store_reg(&mut builder, regs_ptr, *dst, result);
                }
                IROp::FclassS { dst, src } => {
                    let v = Self::load_freg_f32(&mut builder, fregs_ptr, *src);
                    let bits = builder.ins().bitcast(types::I32, MemFlags::new(), v);
                    let zero = builder.ins().iconst(types::I32, 0);
                    let is_zero = builder.ins().icmp(IntCC::Equal, bits, zero);
                    let result = builder.ins().uextend(types::I64, is_zero);
                    Self::store_reg(&mut builder, regs_ptr, *dst, result);
                }

                // 其他未实现的操作暂时跳过
                _ => {}
            }
        }

        match &block.term {
            // 无条件跳转
            // 无条件跳转
            Terminator::Jmp { target } => {
                let next_pc = builder.ins().iconst(types::I64, *target as i64);
                builder.ins().return_(&[next_pc]);
            }
            // 条件分支
            // 条件分支
            Terminator::CondJmp { cond, target_true, target_false } => {
                let v = Self::load_reg(&mut builder, regs_ptr, *cond);
                let zero = builder.ins().iconst(types::I64, 0);
                let cond_b = builder.ins().icmp(IntCC::NotEqual, v, zero);
                let true_block = builder.create_block();
                let false_block = builder.create_block();
                builder.ins().brif(cond_b, true_block, &[], false_block, &[]);

                builder.switch_to_block(true_block);
                builder.seal_block(true_block);
                let next_pc_true = builder.ins().iconst(types::I64, *target_true as i64);
                builder.ins().return_(&[next_pc_true]);

                builder.switch_to_block(false_block);
                builder.seal_block(false_block);
                let next_pc_false = builder.ins().iconst(types::I64, *target_false as i64);
                builder.ins().return_(&[next_pc_false]);
            }
            // 寄存器间接跳转
            Terminator::JmpReg { base, offset } => {
                let base_val = Self::load_reg(&mut builder, regs_ptr, *base);
                let offset_val = builder.ins().iconst(types::I64, *offset);
                let next_pc = builder.ins().iadd(base_val, offset_val);
                builder.ins().return_(&[next_pc]);
            }
            // 返回指令 (使用 x30/ra 寄存器作为返回地址)
            Terminator::Ret => {
                let ra = Self::load_reg(&mut builder, regs_ptr, 30); // x30 = ra
                builder.ins().return_(&[ra]);
            }
            // 中断/异常 - 返回当前PC以便外部处理
            Terminator::Interrupt { vector: _ } => {
                let current_pc = builder.ins().iconst(types::I64, block.start_pc as i64);
                builder.ins().return_(&[current_pc]);
            }
            // 故障
            Terminator::Fault { cause: _ } => {
                let current_pc = builder.ins().iconst(types::I64, block.start_pc as i64);
                builder.ins().return_(&[current_pc]);
            }
            // 函数调用
            Terminator::Call { target, ret_pc: _ } => {
                let next_pc = builder.ins().iconst(types::I64, *target as i64);
                builder.ins().return_(&[next_pc]);
            }
            // 寄存器间接跳转
            Terminator::JmpReg { base, offset } => {
                let base_val = Self::load_reg(&mut builder, regs_ptr, *base);
                let offset_val = builder.ins().iconst(types::I64, *offset);
                let next_pc = builder.ins().iadd(base_val, offset_val);
                builder.ins().return_(&[next_pc]);
            }
            // 返回指令 (使用 x30/ra 寄存器作为返回地址)
            Terminator::Ret => {
                let ra = Self::load_reg(&mut builder, regs_ptr, 30); // x30 = ra
                builder.ins().return_(&[ra]);
            }
            // 中断/异常 - 返回当前PC以便外部处理
            Terminator::Interrupt { vector: _ } => {
                let current_pc = builder.ins().iconst(types::I64, block.start_pc as i64);
                builder.ins().return_(&[current_pc]);
            }
            // 故障
            Terminator::Fault { cause: _ } => {
                let current_pc = builder.ins().iconst(types::I64, block.start_pc as i64);
                builder.ins().return_(&[current_pc]);
            }
            // 函数调用
            Terminator::Call { target, ret_pc: _ } => {
                let next_pc = builder.ins().iconst(types::I64, *target as i64);
                builder.ins().return_(&[next_pc]);
            }
        }

        builder.finalize();

        let id = self.module.declare_function(&format!("func_{}", block.start_pc), Linkage::Export, &ctx.func.signature).unwrap();
        self.module.define_function(id, &mut ctx).unwrap();
        self.module.clear_context(&mut ctx);
        let id = self.module.declare_function(&format!("func_{}", block.start_pc), Linkage::Export, &ctx.func.signature).unwrap();
        self.module.define_function(id, &mut ctx).unwrap();
        self.module.clear_context(&mut ctx);
        self.module.finalize_definitions().unwrap();

        let code = self.module.get_finalized_function(id);
        self.cache.insert(block.start_pc, CodePtr(code));

        self.ctx = ctx;
        self.builder_context = builder_context;

        self.ctx = ctx;
        self.builder_context = builder_context;
        code
    }

    pub fn compile_many_parallel(&mut self, blocks: &[IRBlock]) {
        let shared = self.pool_cache.get_or_insert_with(|| Arc::new(Mutex::new(HashMap::new()))).clone();
        use rayon::prelude::*;
        blocks.par_iter().for_each(|b| {
            let mut worker = Jit::new();
            let ptr = worker.compile(b);
            if let Ok(mut map) = shared.lock() {
                map.insert(b.start_pc, CodePtr(ptr));
            }
            std::mem::forget(worker);
        });
    }

    pub fn compile_many_parallel(&mut self, blocks: &[IRBlock]) {
        let shared = self.pool_cache.get_or_insert_with(|| Arc::new(Mutex::new(HashMap::new()))).clone();
        use rayon::prelude::*;
        blocks.par_iter().for_each(|b| {
            let mut worker = Jit::new();
            let ptr = worker.compile(b);
            if let Ok(mut map) = shared.lock() {
                map.insert(b.start_pc, CodePtr(ptr));
            }
            std::mem::forget(worker);
        });
    }
}

impl ExecutionEngine<IRBlock> for Jit {
    fn run(&mut self, mmu: &mut dyn MMU, block: &IRBlock) -> ExecResult {
        let mut executed_ops = 0;
        let block_ops_count = block.ops.len();
        let sample_interval = 100u64;
        
        // 基本块键值使用 start_pc 而非当前 pc
        let pc_key = block.start_pc;
        // 检查是否需要编译并记录编译时间
        if self.record_execution(pc_key) {
            let compile_start = std::time::Instant::now();
            self.compile(block);
            let compile_time_ns = compile_start.elapsed().as_nanos() as u64;
            self.record_compile_done(compile_time_ns);
        }

        let pooled: Option<CodePtr> = self.pool_cache.as_ref()
            .and_then(|c| c.lock().ok().and_then(|h| h.get(&pc_key).copied()));
        let local: Option<CodePtr> = self.cache.get(&pc_key).copied();
        
        if let Some(code_ptr) = local.or(pooled) {
            let stats = self.adaptive_stats();
            tracing::debug!(pc = self.pc, block_start = block.start_pc, hot = self.get_stats(self.pc).map(|s| s.exec_count).unwrap_or(0), compiled_total = self.total_compiled, interpreted_total = self.total_interpreted, threshold = stats.current_threshold, hit_compiled = stats.compiled_hits, hit_interpreted = stats.interpreted_runs, "jit: execute compiled block");
            // 执行编译后的代码并记录执行时间
            let exec_start = std::time::Instant::now();
            
            // 函数签名: (regs_ptr, ctx_ptr, fregs_ptr) -> next_pc
            let code_fn = unsafe { 
                std::mem::transmute::<*const u8, fn(&mut [u64; 32], &mut JitContext, &mut [f64; 32]) -> u64>(code_ptr.0) 
            };
            let mut jit_ctx = JitContext { mmu };
            self.pc = code_fn(&mut self.regs, &mut jit_ctx, &mut self.fregs);
            
            let exec_time_ns = exec_start.elapsed().as_nanos() as u64;
            self.record_compiled_execution(exec_time_ns, block_ops_count);
            self.pc = code_fn(&mut self.regs, &mut jit_ctx, &mut self.fregs);
            
            let exec_time_ns = exec_start.elapsed().as_nanos() as u64;
            self.record_compiled_execution(exec_time_ns, block_ops_count);
            self.total_compiled += 1;
        } else {
            let stats = self.adaptive_stats();
            tracing::debug!(pc = self.pc, block_start = block.start_pc, hot = self.get_stats(self.pc).map(|s| s.exec_count).unwrap_or(0), compiled_total = self.total_compiled, interpreted_total = self.total_interpreted, threshold = stats.current_threshold, hit_compiled = stats.compiled_hits, hit_interpreted = stats.interpreted_runs, "jit: fallback terminator evaluation");
            // Fallback: 未编译路径根据终结符计算 next_pc
            self.record_interpreted_execution();
            match &block.term {
                Terminator::Jmp { target } => { self.pc = *target; }
                Terminator::CondJmp { cond, target_true, target_false } => {
                    let cond_val = self.regs[*cond as usize];
                    self.pc = if cond_val != 0 { *target_true } else { *target_false };
                }
                Terminator::JmpReg { base, offset } => {
                    let base_val = self.regs[*base as usize] as i64;
                    self.pc = (base_val + *offset) as u64;
                }
                Terminator::Ret => { /* 保持当前 pc 以便上层处理 */ }
                Terminator::Interrupt { .. } | Terminator::Fault { .. } => { /* 保持当前 pc */ }
                Terminator::Call { target, .. } => { self.pc = *target; }
            }
            self.total_interpreted += 1;
        }
        
        // 定期调整自适应阈值
        self.adaptive_threshold.adjust();
        let stats = self.adaptive_stats();
        let total_runs = stats.compiled_hits + stats.interpreted_runs;
        if total_runs % sample_interval == 0 {
            tracing::debug!(threshold = stats.current_threshold, total_compiles = stats.total_compiles, compiled_hits = stats.compiled_hits, interpreted_runs = stats.interpreted_runs, avg_compile_time_ns = stats.avg_compile_time_ns, avg_benefit_ns = stats.avg_benefit_ns, "jit: periodic-sample");
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

#[cfg(test)]
mod tests {
    use super::*;
    use vm_mem::SoftMmu;
    use vm_ir::{MemFlags, IRBlock, IROp, Terminator};

    #[test]
    fn test_jit_load_store_with_mmu() {
        let mut mmu = SoftMmu::new(1024 * 1024, false);
        mmu.write(0x200, 0xDEAD_BEEF_DEAD_BEEFu64, 8).unwrap();
        let mut ctx = JitContext { mmu: &mut mmu };
        let val = unsafe { jit_read(&mut ctx, 0x200, 8) };
        assert_eq!(val, 0xDEAD_BEEF_DEAD_BEEF);
        unsafe { jit_write(&mut ctx, 0x208, 0xABCDu64, 2) };
        assert_eq!(mmu.read(0x208, 2).unwrap(), 0xABCD);
    }

    #[test]
    fn test_jit_atomic_cas() {
        let mut mmu = SoftMmu::new(1024 * 1024, false);
        mmu.write(0x300, 0x1234_5678u64, 8).unwrap();
        let mut ctx = JitContext { mmu: &mut mmu };
        let old = unsafe { jit_cas(&mut ctx, 0x300, 0x1234_5678, 0xAAAA_BBBB, 8) };
        assert_eq!(old, 0x1234_5678);
        assert_eq!(mmu.read(0x300, 8).unwrap(), 0xAAAA_BBBB);
    }

    #[test]
    fn test_jit_float_add() {
        let mut mmu = SoftMmu::new(1024 * 1024, false);
        let mut jit = Jit::new();
        jit.fregs[1] = 1.25;
        jit.fregs[2] = 2.75;
        let block = IRBlock { start_pc: 0x1000, ops: vec![
            IROp::Fadd { dst: 3, src1: 1, src2: 2 },
        ], term: Terminator::Jmp { target: 0x1000 } };
        jit.set_pc(block.start_pc);
        for _ in 0..HOT_THRESHOLD { let _ = jit.run(&mut mmu, &block); }
        assert!((jit.fregs[3] - 4.0).abs() < 1e-12);
    }


    #[test]
    fn test_simd_vec_add() {
        let a = 0x0002_0003_0004_0005u64;
        let b = 0x0001_0001_0001_0001u64;
        let r = simd::jit_vec_add(a, b, 2);
        // lane size 2 bytes: expect per-lane add
        assert_eq!(r, 0x0003_0004_0005_0006u64);
    }

    #[test]
    fn test_ci_guard_jit_compiles() {
        let mut mmu = SoftMmu::new(1024 * 1024, false);
        let mut jit = Jit::new();
        let block = IRBlock { start_pc: 0x5000, ops: vec![
            IROp::AddImm { dst: 2, src: 2, imm: 1 },
        ], term: Terminator::Jmp { target: 0x5000 } };
        jit.set_pc(block.start_pc);
        for _ in 0..HOT_THRESHOLD { let _ = jit.run(&mut mmu, &block); }
        // Expect compiled path executed at least once
        assert!(jit.total_compiled >= 1);
    }

    #[test]
    fn test_jit_fload_fstore_consistency() {
        let mut mmu = SoftMmu::new(1024 * 1024, false);
        let mut jit = Jit::new();
        let addr = 0x400u64;
        jit.fregs[1] = 3.141592653589793;
        let block = IRBlock { start_pc: 0x6000, ops: vec![
            IROp::Fstore { src: 1, base: 0, offset: addr as i64, size: 8 },
            IROp::Fload { dst: 2, base: 0, offset: addr as i64, size: 8 },
            IROp::FmvXD { dst: 5, src: 2 },
        ], term: Terminator::Jmp { target: 0x6000 } };
        jit.set_pc(block.start_pc);
        for _ in 0..HOT_THRESHOLD { let _ = jit.run(&mut mmu, &block); }
        let bits_expected = jit.fregs[1].to_bits();
        assert_eq!(jit.get_reg(5) as u64, bits_expected);
    }
}
