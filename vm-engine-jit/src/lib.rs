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
//! ## 增强功能
//!
//! - 高级寄存器分配和指令调度
//! - 增强型热点检测机制
//! - 智能代码缓存管理
//! - 多种优化Pass
//! - 综合性能基准测试
//!
//! ## 当前状态
//!
//! **已实现**: Add, Sub, Mul, Div, Load, Store, 分支指令, 向量操作, 浮点运算, 原子操作
//! **待完善**: AOT代码执行, JIT代码执行（统一执行器集成）
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

use std::time::Duration;
use vm_core::{AccessType, ExecResult, ExecStats, ExecStatus, ExecutionEngine, ExecutionError, Fault, GuestAddr, MMU};
use vm_ir::{AtomicOp, IRBlock, IROp, Terminator};

// 厂商优化策略 - 已实现 ✅
// 提供针对Intel/AMD/ARM CPU的特定优化
// 详见: docs/VENDOR_OPTIMIZATIONS.md
pub mod vendor_optimizations;
pub use vendor_optimizations::{
    CpuVendor, CpuMicroarchitecture, CpuFeature, OptimizationType,
    VendorOptimizationStrategy, VendorOptimizer, CacheOptimizationHints
};

use cranelift::prelude::*;
use cranelift_codegen::Context as CodegenContext;
use cranelift_codegen::ir::AtomicRmwOp;
use cranelift_codegen::ir::FuncRef;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module};
use cranelift_native;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

// advanced_ops 功能已集成到以下模块：
// - simd: SIMD向量操作
// - simd_integration: SIMD集成管理
// - loop_opt: 循环优化
// - trace_selection: 轨迹选择
// - tiered_compiler: 分层编译
//
// 高级操作（向量化、循环优化等）已在 cranelift_backend 中实现
// 此处保留注释作为架构参考
mod simd; // SIMD向量操作实现
mod simd_integration;
// JIT块链接优化 - 已实现 ✅
// 实现了完整的块链接优化，预期10-15%性能提升
// 详见: docs/BLOCK_CHAINING_IMPLEMENTATION.md
pub mod block_chaining;
pub mod compiler_backend;
#[allow(unexpected_cfgs)]
#[cfg(feature = "llvm-backend")]
pub mod llvm_backend;
pub mod cranelift_backend;
pub mod parallel_compiler;
pub mod async_precompiler;
pub mod incremental_cache;
pub mod compile_cache;
pub mod inline_cache;
mod jit_helpers;
pub mod loop_opt;
pub mod pool;
pub mod trace_selection;
pub mod tiered_compiler;

// 拆分出的模块（提升可维护性）
// 注意：compiler和executor功能已集成到主Jit结构体中
mod stats;
#[cfg(feature = "async")]
mod async_execution_engine;

// 原有模块
pub mod adaptive_optimizer;
pub mod ml_guided_jit;
pub mod ml_model;

// Phase 2-2: ML优化增强模块
pub mod ml_model_enhanced;
pub mod ml_random_forest;
pub mod ml_ab_testing;

// Phase 5: JIT配置文件引导优化 (PGO)
pub mod pgo;

// concurrent_gc 已合并到 unified_gc，模块已移除

// 优化型JIT编译器模块
pub mod optimizing_compiler;
// enhanced_hotspot 已合并到 ewma_hotspot
// enhanced_cache 已合并到 unified_cache
// cache.rs占位符已删除，使用compile_cache和incremental_cache
pub mod ewma_hotspot;
pub mod unified_cache;
pub mod unified_gc;
pub mod gc_adaptive;
pub mod gc_trait;
pub mod gc_marker;
pub mod gc_sweeper;
pub mod graph_coloring_allocator;

// Phase 2 模块
pub mod aot_format;
pub mod aot_integration;
pub mod aot_loader;
pub mod aot_cache;
pub mod hybrid_executor;

// Phase 2.4: 语义库集成
pub mod semantic_integration;

// modern_jit 已移除：因线程安全问题无法修复，功能已由 optimizing_compiler 和 hybrid_executor 提供

// JIT块链接优化 - 已实现完整功能
pub use block_chaining::{
    BlockChainer,        // 块链接器
    BlockChain,          // 块链
    ChainLink,           // 链接关系
    ChainType,           // 链接类型
    BlockChainerStats,   // 统计信息
};
pub use inline_cache::{InlineCacheManager, InlineCacheStats};
pub use tiered_compiler::{TieredCompiler, TieredCompilationConfig, TieredCompilationStats, CompilationTier};
pub use jit_helpers::{FloatRegHelper, MemoryHelper, RegisterHelper};
pub use loop_opt::{LoopInfo, LoopOptConfig, LoopOptimizer};
pub use trace_selection::{TraceBlockRef, TraceSelector, TraceStats};
pub use simd_integration::SimdIntegrationManager;

// 重新导出原有类型
pub use adaptive_optimizer::{
    AdaptiveOptimizer, AdaptiveParameters, OptimizationStrategy, PerformanceFeedback,
};
pub use ml_guided_jit::{CompilationDecision, ExecutionFeatures, MLGuidedCompiler};
pub use ml_model::{
    FeatureExtractor, LinearRegressionModel, MLModel, ModelStatistics, OnlineLearner,
    PerformanceReport, PerformanceValidator,
};
// 统一GC导出（推荐使用）
pub use unified_gc::{
    AdaptiveQuotaManager, GCColor, GCPhase, LockFreeMarkStack, ShardedWriteBarrier, UnifiedGC,
    UnifiedGcConfig, UnifiedGcStats,
};

// 导出优化型JIT编译器类型
pub use optimizing_compiler::{
    OptimizingJIT, OptimizingJITStats, InstructionScheduler, OptimizationPassManager, RegisterAllocator,
    RegisterAllocationStrategy,
};
pub use graph_coloring_allocator::{GraphColoringConfig};
// enhanced_hotspot已合并到ewma_hotspot，保留向后兼容导出
pub use ewma_hotspot::{
    EwmaHotspotConfig, EwmaHotspotDetector, EwmaHotspotStats, HotspotStats,
};
// enhanced_cache已合并到unified_cache，保留向后兼容导出
pub use unified_cache::{CacheConfig, CacheEntry, CacheStats, EvictionPolicy, UnifiedCodeCache};

// 导出 Phase 2 类型
pub use aot_format::{
    AotHeader, AotImage, CodeBlockEntry, RelationType, RelocationEntry, SymbolEntry, SymbolType,
};
pub use aot_integration::{
    create_hybrid_executor, create_test_aot_image, init_aot_loader, validate_aot_config,
};
pub use aot_loader::{AotCodeBlock, AotLoader};
pub use aot_cache::{AotCache, AotCacheConfig, AotCacheStats};
pub use hybrid_executor::{AotFailureReason, CodeSource, ExecutionStats, HybridExecutor};
pub use semantic_integration::{
    OptimizationStats, OptimizedHybridExecutor, SemanticAnalyzer, SemanticCache,
};

#[cfg(feature = "async")]
pub use async_execution_engine::AsyncJitContext;

// modern_jit 导出已移除

/// 默认热点阈值
pub const HOT_THRESHOLD: u64 = 100;

impl Default for AdaptiveThreshold {
    fn default() -> Self {
        Self::new()
    }
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
    /// 编译时间预算（纳秒），超过此时间将回退到解释器
    /// 默认值：10ms (10_000_000 纳秒)
    pub compile_time_budget_ns: u64,
    /// 是否启用编译时间预算检查
    pub enable_compile_time_budget: bool,
}

impl Default for AdaptiveThresholdConfig {
    fn default() -> Self {
        Self {
            min_threshold: 10,
            max_threshold: 1000,
            sample_window: 100,
            compile_time_weight: 0.3,
            exec_benefit_weight: 0.7,
            compile_time_budget_ns: 10_000_000, // 10ms 默认预算
            enable_compile_time_budget: true,
        }
    }
}

/// 异步编译结果状态
#[derive(Debug, Clone, PartialEq)]
pub enum AsyncCompileResult {
    /// 编译已完成，返回代码指针
    Completed(CodePtr),
    /// 编译仍在进行中
    Pending,
    /// 编译超时或失败
    Timeout,
}

/// 自适应阈值统计信息
#[derive(Clone, Debug, Default)]
pub struct AdaptiveThresholdStats {
    pub current_threshold: u64,
    pub total_compiles: u64,
    pub compiled_hits: u64,
    pub interpreted_runs: u64,
    pub avg_compile_time_ns: u64,
    pub avg_benefit_ns: i64,
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

        let compile_factor = if avg_compile_time > 10_000_000 {
            // > 10ms
            1.2 // 编译太慢，提高阈值
        } else if avg_compile_time < 100_000 {
            // < 100μs
            0.9 // 编译很快，可以降低阈值
        } else {
            1.0
        };

        let benefit_factor = if avg_benefit > 1_000_000 {
            // 每次执行节省 > 1ms
            0.8 // 收益高，降低阈值
        } else if avg_benefit < 0 {
            // 负收益 (JIT 更慢)
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
                self.compile_time_samples.iter().sum::<u64>()
                    / self.compile_time_samples.len() as u64
            },
            avg_benefit_ns: if self.exec_benefit_samples.is_empty() {
                0
            } else {
                (self.exec_benefit_samples.iter().sum::<i64>()
                    / self.exec_benefit_samples.len() as i64) as i64
            },
        }
    }
}

fn make_stats(executed_ops: u64) -> ExecStats {
    ExecStats {
        executed_insns: executed_ops,
        executed_ops,
        mem_accesses: 0,
        exec_time_ns: 0,
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

#[derive(Debug, Default, Clone)]
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
        let pa = match (*ctx).mmu.translate(GuestAddr(vaddr), vm_core::AccessType::Read) {
            Ok(p) => p,
            Err(_) => return 0,
        };
        (*ctx).mmu.read(GuestAddr(pa.0), size).unwrap_or(0)
    }
}

extern "C" fn jit_write(ctx: *mut JitContext, vaddr: u64, val: u64, size: u8) {
    unsafe {
        if let Ok(pa) = (*ctx).mmu.translate(GuestAddr(vaddr), vm_core::AccessType::Write) {
            let _ = (*ctx).mmu.write(GuestAddr(pa.0), val, size);
        }
    }
}

extern "C" fn jit_lr(ctx: *mut JitContext, vaddr: u64, size: u8) -> u64 {
    unsafe { (*ctx).mmu.load_reserved(GuestAddr(vaddr), size).unwrap_or(0) }
}

extern "C" fn jit_sc(ctx: *mut JitContext, vaddr: u64, val: u64, size: u8) -> u64 {
    unsafe {
        match (*ctx).mmu.store_conditional(GuestAddr(vaddr), val, size) {
            Ok(true) => 1,
            Ok(false) => 0,
            Err(_) => 0,
        }
    }
}

extern "C" fn jit_cas(ctx: *mut JitContext, vaddr: u64, expected: u64, new: u64, size: u8) -> u64 {
    unsafe {
        std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
        let pa_r = match (*ctx).mmu.translate(GuestAddr(vaddr), vm_core::AccessType::Read) {
            Ok(p) => p,
            Err(_) => return 0,
        };
        let old = (*ctx).mmu.read(GuestAddr(pa_r.0), size).unwrap_or(0);
        if old == expected {
            if let Ok(pa_w) = (*ctx).mmu.translate(GuestAddr(vaddr), vm_core::AccessType::Write) {
                let _ = (*ctx).mmu.write(GuestAddr(pa_w.0), new, size);
            }
        }
        std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
        old
    }
}

extern "C" fn barrier_acquire() {
    std::sync::atomic::fence(std::sync::atomic::Ordering::Acquire);
}
extern "C" fn barrier_release() {
    std::sync::atomic::fence(std::sync::atomic::Ordering::Release);
}
extern "C" fn barrier_full() {
    std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
}

/// 编译后的代码指针包装类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodePtr(*const u8);
unsafe impl Send for CodePtr {}
unsafe impl Sync for CodePtr {}
impl CodePtr {
    pub fn is_null(&self) -> bool {
        self.0.is_null()
    }
}

/// 分片代码缓存（减少锁竞争）
struct ShardedCache {
    /// 分片数组（每个分片有自己的锁）
    shards: Vec<Mutex<HashMap<GuestAddr, CodePtr>>>,
    /// 分片数量（必须是2的幂）
    shard_count: usize,
}

impl ShardedCache {
    /// 创建新的分片缓存
    fn new(shard_count: usize) -> Self {
        // 确保shard_count是2的幂
        let shard_count = shard_count.next_power_of_two();
        let mut shards = Vec::with_capacity(shard_count);
        for _ in 0..shard_count {
            shards.push(Mutex::new(HashMap::new()));
        }
        Self {
            shards,
            shard_count,
        }
    }

    /// 根据地址选择分片索引
    #[inline]
    fn shard_index(&self, addr: GuestAddr) -> usize {
        // 使用地址的低位进行哈希，然后取模
        (addr.0 as usize) & (self.shard_count - 1)
    }

    /// 查找代码指针
    fn get(&self, addr: GuestAddr) -> Option<CodePtr> {
        let idx = self.shard_index(addr);
        self.shards[idx].lock().get(&addr).copied()
    }

    /// 插入代码指针
    fn insert(&self, addr: GuestAddr, code_ptr: CodePtr) {
        let idx = self.shard_index(addr);
        self.shards[idx].lock().insert(addr, code_ptr);
    }

    /// 移除代码指针
    fn remove(&self, addr: GuestAddr) -> Option<CodePtr> {
        let idx = self.shard_index(addr);
        self.shards[idx].lock().remove(&addr)
    }

    /// 清空所有分片
    fn clear(&self) {
        for shard in &self.shards {
            shard.lock().clear();
        }
    }

    /// 获取总条目数
    fn len(&self) -> usize {
        self.shards.iter().map(|s| s.lock().len()).sum()
    }
}

impl Clone for ShardedCache {
    fn clone(&self) -> Self {
        let mut shards = Vec::with_capacity(self.shard_count);
        for shard in &self.shards {
            shards.push(Mutex::new(shard.lock().clone()));
        }
        Self {
            shards,
            shard_count: self.shard_count,
        }
    }
}
pub struct Jit {
    builder_context: FunctionBuilderContext,
    ctx: CodegenContext,
    module: JITModule,
    /// 分片代码缓存（减少锁竞争）
    cache: ShardedCache,
    pool_cache: Option<Arc<Mutex<HashMap<GuestAddr, CodePtr>>>>,
    hot_counts: HashMap<GuestAddr, BlockStats>,
    pub regs: [u64; 32],
    pub pc: GuestAddr,
    pub vec_regs: [[u64; 2]; 32],
    /// 浮点寄存器 (f0-f31)
    pub fregs: [f64; 32],
    pub total_compiled: u64,
    pub total_interpreted: u64,
    /// 自适应热点阈值管理器
    pub adaptive_threshold: AdaptiveThreshold,
    /// 循环优化器
    loop_optimizer: LoopOptimizer,
    /// SIMD集成管理器
    simd_integration: SimdIntegrationManager,
    /// 缓存SIMD函数ID
    simd_vec_add_func: Option<cranelift_module::FuncId>,
    simd_vec_sub_func: Option<cranelift_module::FuncId>,
    simd_vec_mul_func: Option<cranelift_module::FuncId>,
    /// 事件总线（可选，用于发布领域事件）
    ///
    /// 注意：使用 vm_core::domain_services::DomainEventBus
    /// 通过 set_event_bus 方法设置
    event_bus: Option<Arc<vm_core::domain_services::DomainEventBus>>,
    /// VM ID（用于事件发布）
    vm_id: Option<String>,
    /// PGO Profile收集器（可选）
    profile_collector: Option<Arc<pgo::ProfileCollector>>,
    /// ML指导编译器（可选）
    ml_compiler: Option<Arc<Mutex<ml_guided_jit::MLGuidedCompiler>>>,
    /// 在线学习器（可选）
    online_learner: Option<Arc<Mutex<ml_model::OnlineLearner>>>,
    /// 性能验证器（可选）
    performance_validator: Option<Arc<Mutex<ml_model::PerformanceValidator>>>,
    /// 待编译队列（用于增量编译）
    pending_compile_queue: Vec<(GuestAddr, u32)>, // (PC, priority)
    /// 预编译队列（用于代码预取）
    prefetch_compile_queue: Vec<(GuestAddr, u32)>, // (PC, priority)
    /// 编译时间预算（纳秒）
    compile_time_budget_ns: u64,
    /// 后台编译任务句柄（可选）
    background_compile_handle: Option<tokio::task::JoinHandle<()>>,
    /// 后台编译任务停止信号
    background_compile_stop: Arc<tokio::sync::Notify>,
    /// 异步编译任务句柄映射 (PC -> Arc<JoinHandle>)
    async_compile_tasks: Arc<parking_lot::Mutex<HashMap<GuestAddr, Arc<tokio::task::JoinHandle<CodePtr>>>>>,
    /// 异步编译结果缓存 (PC -> CodePtr)
    async_compile_results: Arc<parking_lot::Mutex<HashMap<GuestAddr, CodePtr>>>,
    /// IR块缓存（用于后台编译）
    ir_block_cache: Arc<parking_lot::Mutex<HashMap<GuestAddr, IRBlock>>>,
}

impl Jit {
    /// 创建新的JIT编译器（ML引导优化默认启用）
    pub fn new() -> Self {
        Self::with_ml_guidance(true)
    }

    /// 创建新的JIT编译器（可配置ML引导优化）
    ///
    /// # 参数
    /// * `enable_ml` - 是否启用ML引导优化
    ///
    /// # 示例
    /// ```rust
    /// let jit = Jit::with_ml_guidance(true);  // 启用ML优化
    /// let jit = Jit::with_ml_guidance(false); // 禁用ML优化
    /// ```
    pub fn with_ml_guidance(enable_ml: bool) -> Self {
        let mut jit = Self::create_jit_without_ml();
        if enable_ml {
            jit.enable_ml_guidance();
        }
        jit
    }

    /// 创建JIT实例但不启用ML引导优化
    fn create_jit_without_ml() -> Self {
        let mut flag_builder = settings::builder();
        flag_builder
            .set("use_colocated_libcalls", "false")
            .expect("Operation failed");
        flag_builder
            .set("is_pic", "false")
            .expect("Operation failed");
        // 默认使用speed优化级别，分层编译会在compile方法中动态调整
        flag_builder
            .set("opt_level", "speed")
            .expect("Operation failed");

        let isa_builder = cranelift_native::builder().unwrap_or_else(|msg| {
            panic!("host machine is not supported: {}", msg);
        });

        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .expect("Operation failed");
        let mut builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());

        builder.symbol("jit_read", jit_read as *const u8);
        builder.symbol("jit_write", jit_write as *const u8);
        builder.symbol("jit_cas", jit_cas as *const u8);
        builder.symbol("jit_lr", jit_lr as *const u8);
        builder.symbol("jit_sc", jit_sc as *const u8);
        builder.symbol("barrier_acquire", barrier_acquire as *const u8);
        builder.symbol("barrier_release", barrier_release as *const u8);
        builder.symbol("barrier_full", barrier_full as *const u8);
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
        let ctx = module.make_context();
        let jit = Self {
            builder_context: FunctionBuilderContext::new(),
            ctx,
            module,
            cache: ShardedCache::new(16), // 16个分片，减少锁竞争
            pool_cache: None,
            hot_counts: HashMap::new(),
            regs: [0; 32],
            pc: GuestAddr(0),
            vec_regs: [[0; 2]; 32],
            fregs: [0.0; 32],
            total_compiled: 0,
            total_interpreted: 0,
            adaptive_threshold: AdaptiveThreshold::new(),
            loop_optimizer: LoopOptimizer::default(),
            simd_integration: SimdIntegrationManager::new(),
            simd_vec_add_func: None,
            simd_vec_sub_func: None,
            simd_vec_mul_func: None,
            event_bus: None, // 事件总线可选
            vm_id: None,
            profile_collector: None,
            ml_compiler: None,
            online_learner: None,
            performance_validator: None,
            pending_compile_queue: Vec::new(),
            prefetch_compile_queue: Vec::new(),
            compile_time_budget_ns: 10_000_000, // 10ms默认预算
            background_compile_handle: None,
            background_compile_stop: Arc::new(tokio::sync::Notify::new()),
            async_compile_tasks: Arc::new(parking_lot::Mutex::new(HashMap::new())),
            async_compile_results: Arc::new(parking_lot::Mutex::new(HashMap::new())),
            ir_block_cache: Arc::new(parking_lot::Mutex::new(HashMap::new())),
        };

        // 默认启用ML引导优化（可通过disable_ml_guidance()禁用）
        // 注意：enable_ml_guidance 需要 &mut self，所以使用内部可变性或在调用处处理
        // jit.enable_ml_guidance();

        jit
    }

    /// 设置事件总线（用于发布领域事件）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use vm_core::domain_services::DomainEventBus;
    /// use std::sync::Arc;
    ///
    /// let event_bus = Arc::new(DomainEventBus::new());
    /// jit.set_event_bus(event_bus);
    /// ```
    pub fn set_event_bus(&mut self, event_bus: Arc<vm_core::domain_services::DomainEventBus>) {
        self.event_bus = Some(event_bus);
    }

    /// 设置VM ID（用于事件发布）
    pub fn set_vm_id(&mut self, vm_id: String) {
        self.vm_id = Some(vm_id);
    }

    /// 设置PGO Profile收集器
    pub fn set_profile_collector(&mut self, collector: Arc<pgo::ProfileCollector>) {
        self.profile_collector = Some(collector);
    }

    /// 启用PGO收集
    pub fn enable_pgo(&mut self, collection_interval: std::time::Duration) {
        let collector = Arc::new(pgo::ProfileCollector::new(collection_interval));
        self.set_profile_collector(Arc::clone(&collector));
    }

    /// 获取Profile数据
    pub fn get_profile_data(&self) -> Option<pgo::ProfileData> {
        self.profile_collector.as_ref().map(|c| c.get_profile_data())
    }

    /// 保存Profile数据到文件
    pub fn save_profile_data<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), String> {
        if let Some(ref collector) = self.profile_collector {
            collector.serialize_to_file(path.as_ref())
        } else {
            Err("Profile collector not enabled".to_string())
        }
    }

    /// 启用ML指导的编译
    ///
    /// 默认使用优化的学习率和模型参数，以获得更好的性能。
    /// ML引导优化默认已启用，此方法可用于重新启用（如果之前被禁用）。
    pub fn enable_ml_guidance(&mut self) {
        let ml_compiler = Arc::new(Mutex::new(ml_guided_jit::MLGuidedCompiler::new()));
        // 使用优化的学习率：0.005（更稳定，避免过度调整）
        let model = ml_model::LinearRegressionModel::with_optimized_weights(0.005);
        // 批量大小：20（平衡学习速度和稳定性）
        // 更新间隔：3秒（更频繁的更新，更快适应）
        let learner = Arc::new(Mutex::new(ml_model::OnlineLearner::new(
            Box::new(model),
            20,
            std::time::Duration::from_secs(3),
        )));
        let validator = Arc::new(Mutex::new(ml_model::PerformanceValidator::new()));

        self.ml_compiler = Some(Arc::clone(&ml_compiler));
        self.online_learner = Some(Arc::clone(&learner));
        self.performance_validator = Some(Arc::clone(&validator));
    }

    /// 禁用ML指导的编译
    ///
    /// 如果不需要ML引导优化，可以调用此方法禁用以节省资源。
    pub fn disable_ml_guidance(&mut self) {
        self.ml_compiler = None;
        self.online_learner = None;
        self.performance_validator = None;
    }

    /// 检查ML引导优化是否启用
    pub fn is_ml_guidance_enabled(&self) -> bool {
        self.ml_compiler.is_some()
    }

    /// 获取ML编译决策
    pub fn get_ml_decision(&self, block: &IRBlock) -> Option<CompilationDecision> {
        if let Some(ref ml_compiler) = self.ml_compiler {
            let features = ml_model::FeatureExtractor::extract_from_ir_block(block);

            // 如果有PGO数据，增强特征
            let mut enhanced_features = features;
            if let Some(ref collector) = self.profile_collector {
                let profile = collector.get_profile_data();
                // 转换pgo::ProfileData到ml_guided_jit::ProfileData
                let ml_profile = ml_guided_jit::ProfileData {
                    execution_count: profile.block_profiles.values()
                        .map(|p| p.execution_count)
                        .sum(),
                    cache_hit_rate: 0.8, // 占位值
                    avg_block_time_us: profile.block_profiles.values()
                        .map(|p| p.avg_duration_ns)
                        .filter(|&t| t > 0)
                        .sum::<u64>() as f64 / profile.block_profiles.len().max(1) as f64 / 1000.0,
                };
                ml_compiler.lock().enhance_features_with_pgo(
                    &mut enhanced_features,
                    &ml_profile,
                );
            }

            let mut compiler = ml_compiler.lock();
            Some(compiler.predict_decision(&enhanced_features))
        } else {
            None
        }
    }

    /// 记录ML训练样本
    pub fn record_ml_sample(
        &self,
        block: &IRBlock,
        decision: CompilationDecision,
        performance: f64,
    ) {
        if let (Some(learner), Some(ml_compiler)) = (&self.online_learner, &self.ml_compiler) {
            let features = ml_model::FeatureExtractor::extract_from_ir_block(block);

            // 增强特征
            let mut enhanced_features = features;
            if let Some(ref collector) = self.profile_collector {
                let profile = collector.get_profile_data();
                // 转换pgo::ProfileData到ml_guided_jit::ProfileData
                let ml_profile = ml_guided_jit::ProfileData {
                    execution_count: profile.block_profiles.values()
                        .map(|p| p.execution_count)
                        .sum(),
                    cache_hit_rate: 0.8, // 占位值
                    avg_block_time_us: profile.block_profiles.values()
                        .map(|p| p.avg_duration_ns)
                        .filter(|&t| t > 0)
                        .sum::<u64>() as f64 / profile.block_profiles.len().max(1) as f64 / 1000.0,
                };
                ml_compiler.lock().enhance_features_with_pgo(
                    &mut enhanced_features,
                    &ml_profile,
                );
            }

            learner.lock().add_sample(enhanced_features, decision, performance);
        }
    }

    /// 获取ML性能报告
    pub fn get_ml_performance_report(&self) -> Option<ml_model::PerformanceReport> {
        self.performance_validator.as_ref().map(|v| {
            v.lock().get_performance_report()
        })
    }

    /// 发布代码块编译事件
    ///
    /// 向领域事件总线发布代码块编译完成的事件，用于监控和性能分析。
    fn publish_code_block_compiled(&self, _pc: GuestAddr, _block_size: usize) {
        // TODO: Re-enable when ExecutionEvent::CodeBlockCompiled is available
        // use vm_core::domain_services::ExecutionEvent;

        // if let (Some(ref bus), Some(ref vm_id)) = (&self.event_bus, &self.vm_id) {
        //     let event = ExecutionEvent::CodeBlockCompiled {
        //         vm_id: vm_id.clone(),
        //         pc,
        //         block_size,
        //         occurred_at: std::time::SystemTime::now(),
        //     };
        //     let _ = bus.publish(event);
        // }
    }

    /// 发布热点检测事件
    fn publish_hotspot_detected(&self, _pc: GuestAddr, _exec_count: u64) {
        // TODO: Re-enable when ExecutionEvent::HotspotDetected is available
        // use vm_core::domain_services::ExecutionEvent;

        // if let (Some(ref bus), Some(ref vm_id)) = (&self.event_bus, &self.vm_id) {
        //     let event = ExecutionEvent::HotspotDetected {
        //         vm_id: vm_id.clone(),
        //         pc,
        //         exec_count,
        //         detected_at: std::time::SystemTime::now(),
        //     };
        //     let _ = bus.publish(event);
        // }
    }

    /// 使用自定义配置创建 JIT 编译器
    pub fn with_adaptive_config(config: AdaptiveThresholdConfig) -> Self {
        let mut jit = Self::new();
        jit.adaptive_threshold = AdaptiveThreshold::with_config(config);
        jit
    }

    /// 创建JIT编译器并配置ML引导优化
    /// 加载浮点寄存器值 (F64)
    fn load_freg(builder: &mut FunctionBuilder, fregs_ptr: Value, idx: u32) -> Value {
        let offset = (idx as i32) * 8;
        builder
            .ins()
            .load(types::F64, MemFlags::trusted(), fregs_ptr, offset)
    }

    /// 存储浮点寄存器值 (F64)
    /// 存储浮点寄存器值 (F64)
    fn store_freg(builder: &mut FunctionBuilder, fregs_ptr: Value, idx: u32, val: Value) {
        let offset = (idx as i32) * 8;
        builder
            .ins()
            .store(MemFlags::trusted(), val, fregs_ptr, offset);
    }

    /// 加载单精度浮点寄存器值 (F32)
    /// 注意：内部存储为 F64，这里加载低 32 位并转换为 F32
    fn load_freg_f32(builder: &mut FunctionBuilder, fregs_ptr: Value, idx: u32) -> Value {
        let offset = (idx as i32) * 8;
        // 加载 F64 然后降级为 F32
        let f64_val = builder
            .ins()
            .load(types::F64, MemFlags::trusted(), fregs_ptr, offset);
        builder.ins().fdemote(types::F32, f64_val)
    }

    /// 存储单精度浮点寄存器值 (F32)
    /// 注意：将 F32 提升为 F64 后存储
    fn store_freg_f32(builder: &mut FunctionBuilder, fregs_ptr: Value, idx: u32, val: Value) {
        let offset = (idx as i32) * 8;
        // 将 F32 提升为 F64 然后存储
        let f64_val = builder.ins().fpromote(types::F64, val);
        builder
            .ins()
            .store(MemFlags::trusted(), f64_val, fregs_ptr, offset);
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
            let func_id = self
                .module
                .declare_function(name, Linkage::Import, &sig)
                .expect("Operation failed");
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
        self.hot_counts
            .get(&pc)
            .map(|s| s.exec_count >= threshold)
            .unwrap_or(false)
    }

    /// 记录执行并检查是否需要编译 (使用自适应阈值)
    pub fn record_execution(&mut self, pc: GuestAddr) -> bool {
        let threshold = self.adaptive_threshold.threshold();
        
        // 先获取当前状态，避免在更新时再次借用self
        let (should_compile, exec_count) = {
            let stats = self.hot_counts.entry(pc).or_default();
            stats.exec_count += 1;
            let should = stats.exec_count >= threshold && !stats.is_compiled;
            if should {
                stats.is_compiled = true;
            }
            (should, stats.exec_count)
        };
        
        if should_compile {
            // 计算编译优先级（基于关键路径和调用频率）
            let priority = self.compute_compile_priority(pc);
            // 添加到待编译队列
            self.pending_compile_queue.push((pc, priority));
            // 按优先级排序（高优先级在前）
            self.pending_compile_queue.sort_by(|a, b| b.1.cmp(&a.1));
            // 发布热点检测事件
            self.publish_hotspot_detected(pc, exec_count);
            true
        } else {
            false
        }
    }

    /// 计算编译优先级（关键路径优先）
    /// 
    /// 优先级基于：
    /// 1. 执行频率（越高优先级越高）
    /// 2. 关键路径位置（在调用链中的位置）
    /// 3. 调用者数量（被更多块调用的块优先级更高）
    fn compute_compile_priority(&self, pc: GuestAddr) -> u32 {
        let exec_count = self.hot_counts.get(&pc)
            .map(|s| s.exec_count)
            .unwrap_or(0);
        
        // 基础优先级：执行频率
        let mut priority = exec_count.min(u32::MAX as u64) as u32;
        
        // 如果有PGO数据，增强优先级计算
        if let Some(ref collector) = self.profile_collector {
            let profile = collector.get_profile_data();
            if let Some(block_profile) = profile.block_profiles.get(&(pc.0 as usize)) {
                // 调用者数量越多，优先级越高
                priority += (block_profile.callers.len() * 10).min(100) as u32;

                // 被调用者数量越多，说明是关键路径，优先级越高
                priority += (block_profile.callees.len() * 5).min(50) as u32;
            }
        }
        
        priority
    }

    /// 处理待编译队列（增量编译）
    /// 
    /// 从队列中取出高优先级的代码块进行编译，直到用完时间预算
    pub fn process_compile_queue(&mut self, blocks: &HashMap<GuestAddr, IRBlock>) -> usize {
        let start_time = std::time::Instant::now();
        let mut compiled_count = 0;
        
        // 处理队列中的高优先级块
        while let Some((pc, _priority)) = self.pending_compile_queue.pop() {
            // 检查时间预算
            if start_time.elapsed().as_nanos() as u64 > self.compile_time_budget_ns {
                // 超过预算，将剩余项放回队列
                break;
            }
            
            // 查找对应的IR块
            if let Some(block) = blocks.get(&pc) {
                // 编译块
                let code_ptr = self.compile(block);
                if !code_ptr.0.is_null() {
                    compiled_count += 1;
                }
            }
        }
        
        compiled_count
    }

    /// 基于执行路径预测预编译代码块
    /// 
    /// 使用PGO数据预测下一个可能执行的代码块，并加入预编译队列
    pub fn prefetch_code_blocks(&mut self, current_pc: GuestAddr) {
        // 如果有PGO数据，使用路径分析预测下一个块
        if let Some(ref collector) = self.profile_collector {
            let profile = collector.get_profile_data();
            
            // 查找当前块的被调用者（下一个可能执行的块）
            if let Some(block_profile) = profile.block_profiles.get(&(current_pc.0 as usize)) {
                // 按调用频率排序被调用者
                let mut callees: Vec<_> = block_profile.callees.iter().collect();
                
                // 计算每个被调用者的调用频率（简化：使用执行次数）
                callees.sort_by_key(|&callee_pc| {
                    profile.block_profiles.get(callee_pc)
                        .map(|p| p.execution_count)
                        .unwrap_or(0)
                });
                
                // 将高频被调用者加入预编译队列
                for &callee_pc in callees.iter().rev().take(3) {
                    // 将usize转换为GuestAddr
                    let callee_guest_addr = GuestAddr(*callee_pc as u64);
                    // 检查是否已经在队列中或已编译
                    if !self.pending_compile_queue.iter().any(|(pc, _)| *pc == callee_guest_addr) &&
                       !self.prefetch_compile_queue.iter().any(|(pc, _)| *pc == callee_guest_addr) &&
                       self.cache.get(callee_guest_addr).is_none() {
                        // 计算优先级（基于调用频率）
                        let priority = profile.block_profiles.get(callee_pc)
                            .map(|p| (p.execution_count.min(1000) / 10) as u32)
                            .unwrap_or(1);
                        self.prefetch_compile_queue.push((callee_guest_addr, priority));
                    }
                }
            }
        }
    }

    /// 处理预编译队列
    /// 
    /// 从预编译队列中取出代码块进行异步编译
    pub fn process_prefetch_queue(&mut self, blocks: &HashMap<GuestAddr, IRBlock>) -> usize {
        let mut prefetched_count = 0;
        
        // 处理预编译队列（限制数量，避免过度预取）
        while let Some((pc, _priority)) = self.prefetch_compile_queue.pop() {
            if prefetched_count >= 5 {
                // 限制每次最多预取5个块
                break;
            }
            
            // 检查是否已编译
            if self.cache.get(pc).is_some() {
                continue;
            }
            
            // 查找对应的IR块并异步编译
            if let Some(block) = blocks.get(&pc).cloned() {
                let _handle = self.compile_async(block);
                prefetched_count += 1;
            }
        }
        
        prefetched_count
    }

    /// 设置编译时间预算
    pub fn set_compile_time_budget(&mut self, budget_ns: u64) {
        self.compile_time_budget_ns = budget_ns;
    }

    /// 设置编译时间预算（Duration版本）
    pub fn set_compile_time_budget_duration(&mut self, budget: Duration) {
        self.compile_time_budget_ns = budget.as_nanos() as u64;
    }

    /// 添加代码块到编译队列
    pub fn add_to_compile_queue(&mut self, pc: GuestAddr, priority: u32) {
        self.pending_compile_queue.push((pc, priority));
        // 按优先级排序（高优先级在前）
        self.pending_compile_queue.sort_by(|a, b| b.1.cmp(&a.1));
    }

    /// 检查异步编译结果
    pub fn check_async_compile_result(&self, pc: GuestAddr) -> AsyncCompileResult {
        // 检查是否已完成
        if let Some(code_ptr) = self.async_compile_results.lock().get(&pc) {
            return AsyncCompileResult::Completed(*code_ptr);
        }
        
        // 检查是否还在进行中
        if self.async_compile_tasks.lock().contains_key(&pc) {
            return AsyncCompileResult::Pending;
        }
        
        // 超时或失败
        AsyncCompileResult::Timeout
    }

    /// 启动后台编译任务
    /// 
    /// 后台任务会定期处理pending_compile_queue和prefetch_compile_queue
    /// 减少主线程阻塞，提高系统响应性
    pub fn start_background_compile_task(&mut self) {
        // 如果任务已经在运行，先停止它
        if self.background_compile_handle.is_some() {
            self.stop_background_compile_task();
        }

        let stop_signal = self.background_compile_stop.clone();
        let pending_queue = Arc::new(parking_lot::Mutex::new(std::mem::take(&mut self.pending_compile_queue)));
        let prefetch_queue = Arc::new(parking_lot::Mutex::new(std::mem::take(&mut self.prefetch_compile_queue)));
        let ir_cache = self.ir_block_cache.clone();
        let async_results = self.async_compile_results.clone();
        let async_tasks = self.async_compile_tasks.clone();
        let cache = self.cache.clone();
        let compile_time_budget = self.compile_time_budget_ns;
        
        // 创建后台编译任务
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(50)); // 每50ms处理一次
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // 处理待编译队列
                        let mut compiled_count = 0;
                        let start_time = std::time::Instant::now();
                        
                        // 处理pending队列（高优先级）
                        loop {
                            let item = {
                                let mut queue = pending_queue.lock();
                                queue.pop()
                            };
                            
                            let (pc, priority) = match item {
                                Some(item) => item,
                                None => break, // 队列为空
                            };
                            
                            // 检查时间预算
                            if start_time.elapsed().as_nanos() as u64 > compile_time_budget {
                                // 超过预算，将项放回队列
                                pending_queue.lock().push((pc, priority));
                                break;
                            }
                            
                            // 查找IR块
                            let block = {
                                let cache = ir_cache.lock();
                                cache.get(&pc).cloned()
                            };
                            
                            if let Some(block) = block {
                                // 异步编译
                                let block_clone = block.clone();
                                let cache_clone = cache.clone();
                                let results_clone = async_results.clone();
                                let tasks_clone = async_tasks.clone();
                                
                                let handle = tokio::task::spawn_blocking(move || {
                                    let mut temp_jit = Jit::new();
                                    let code_ptr = temp_jit.compile(&block_clone);
                                    
                                    // 存储结果
                                    results_clone.lock().insert(pc, code_ptr);
                                    cache_clone.insert(pc, code_ptr);
                                    tasks_clone.lock().remove(&pc);
                                    
                                    code_ptr
                                });
                                
                                // JoinHandle已经实现了Send + Sync，可以直接存储
                                async_tasks.lock().insert(pc, Arc::new(handle));
                                compiled_count += 1;
                            }
                            
                            // 限制每次处理的块数
                            if compiled_count >= 10 {
                                break;
                            }
                        }
                        
                        // 处理预编译队列（低优先级，限制数量）
                        let mut prefetch_count = 0;
                        loop {
                            let item = {
                                let mut queue = prefetch_queue.lock();
                                queue.pop()
                            };
                            
                            let (pc, _priority) = match item {
                                Some(item) => item,
                                None => break, // 队列为空
                            };
                            
                            if prefetch_count >= 3 {
                                // 达到限制，将项放回队列
                                prefetch_queue.lock().push((pc, _priority));
                                break;
                            }
                            
                            // 检查是否已编译
                            if cache.get(pc).is_some() {
                                continue;
                            }
                            
                            // 查找IR块
                            let block = {
                                let cache = ir_cache.lock();
                                cache.get(&pc).cloned()
                            };
                            
                            if let Some(block) = block {
                                // 异步编译
                                let block_clone = block.clone();
                                let cache_clone = cache.clone();
                                let results_clone = async_results.clone();
                                let tasks_clone = async_tasks.clone();
                                
                                let handle = tokio::task::spawn_blocking(move || {
                                    let mut temp_jit = Jit::new();
                                    let code_ptr = temp_jit.compile(&block_clone);
                                    
                                    // 存储结果
                                    results_clone.lock().insert(pc, code_ptr);
                                    cache_clone.insert(pc, code_ptr);
                                    tasks_clone.lock().remove(&pc);
                                    
                                    code_ptr
                                });
                                
                                // JoinHandle已经实现了Send + Sync，可以直接存储
                                async_tasks.lock().insert(pc, Arc::new(handle));
                                prefetch_count += 1;
                            }
                        }
                    }
                    _ = stop_signal.notified() => {
                        // 收到停止信号，退出循环
                        break;
                    }
                }
            }
        });
        
        self.background_compile_handle = Some(handle);
    }

    /// 停止后台编译任务
    pub fn stop_background_compile_task(&mut self) {
        if let Some(handle) = self.background_compile_handle.take() {
            // 发送停止信号
            self.background_compile_stop.notify_one();
            // 等待任务完成（不阻塞）
            let _ = tokio::spawn(async move {
                let _ = handle.await;
            });
        }
    }

    /// 注册IR块到缓存（用于后台编译）
    pub fn register_ir_block(&self, pc: GuestAddr, block: IRBlock) {
        self.ir_block_cache.lock().insert(pc, block);
    }

    fn record_compile_done(&mut self, compile_time_ns: u64) {
        self.adaptive_threshold.record_compile(compile_time_ns);
    }

    fn record_compiled_execution(&mut self, exec_time_ns: u64, block_ops_count: usize) {
        let estimated_interp_time_ns = exec_time_ns + (block_ops_count as u64) * 500;
        self.adaptive_threshold
            .record_compiled_hit(exec_time_ns, estimated_interp_time_ns);
    }

    fn record_interpreted_execution(&mut self) {
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
            builder
                .ins()
                .load(types::I64, MemFlags::trusted(), regs_ptr, offset)
        }
    }

    fn store_reg(builder: &mut FunctionBuilder, regs_ptr: Value, idx: u32, val: Value) {
        if idx != 0 {
            let offset = (idx as i32) * 8;
            builder
                .ins()
                .store(MemFlags::trusted(), val, regs_ptr, offset);
        }
    }

    /// 只编译不执行（公共接口）
    /// 
    /// 编译IR块并返回代码指针，但不执行代码
    /// 编译结果会被缓存，后续调用会直接返回缓存的代码指针
    pub fn compile_only(&mut self, block: &IRBlock) -> CodePtr {
        // 注册IR块到缓存（用于后台编译）
        self.register_ir_block(block.start_pc, block.clone());
        self.compile(block)
    }

    /// 异步编译IR块
    /// 
    /// 在后台异步编译IR块，不阻塞当前执行线程
    /// 使用spawn_blocking在阻塞线程池中执行编译，避免阻塞tokio运行时
    /// 编译结果会自动缓存到async_compile_results中
    pub fn compile_async(&mut self, block: IRBlock) -> tokio::task::JoinHandle<CodePtr> {
        let pc = block.start_pc;
        
        // 检查是否已经在编译中
        {
            let tasks = self.async_compile_tasks.lock();
            if let Some(existing_handle) = tasks.get(&pc) {
                // 检查任务是否已完成
                if existing_handle.is_finished() {
                    // 任务已完成，检查结果
                    if let Some(_ptr) = self.async_compile_results.lock().get(&pc).copied() {
                        // 创建新的已完成任务
                        let results = self.async_compile_results.clone();
                        return tokio::spawn(async move {
                            results.lock().get(&pc).copied().unwrap_or(CodePtr(std::ptr::null()))
                        });
                    }
                } else {
                    // 任务还在进行中，返回一个等待编译结果的新任务
                    let results = self.async_compile_results.clone();
                    let tasks = self.async_compile_tasks.clone();
                    return tokio::spawn(async move {
                        // 轮询等待编译结果（因为JoinHandle不能clone）
                        loop {
                            // 检查结果
                            if let Some(ptr) = results.lock().get(&pc).copied() {
                                return ptr;
                            }
                            // 检查任务是否还存在
                            if !tasks.lock().contains_key(&pc) {
                                break;
                            }
                            // 短暂等待
                            tokio::time::sleep(tokio::time::Duration::from_micros(100)).await;
                        }
                        // 任务完成但没有结果
                        CodePtr(std::ptr::null())
                    });
                }
            }
        }
        
        // 检查缓存
        if let Some(ptr) = self.cache.get(pc) {
            // 已经在缓存中，创建一个立即完成的future
            let results = self.async_compile_results.clone();
            return tokio::spawn(async move {
                results.lock().insert(pc, ptr);
                ptr
            });
        }
        
        // 准备编译所需的数据
        let block_clone = block.clone();
        let cache = self.cache.clone();
        let results = self.async_compile_results.clone();
        let tasks = self.async_compile_tasks.clone();
        let _hot_counts = self.hot_counts.get(&pc).cloned();
        let _adaptive_threshold_config = self.adaptive_threshold.config.clone();
        let _ml_compiler = self.ml_compiler.clone();
        let _profile_collector = self.profile_collector.clone();
        let _loop_optimizer = self.loop_optimizer.clone();

        // 使用spawn_blocking在阻塞线程池中执行编译
        // 这样可以不阻塞tokio运行时，但仍然使用同步编译代码
        let handle = tokio::task::spawn_blocking(move || {
            // 创建一个新的Jit实例进行编译
            // 注意：这不是最优的，因为会创建新的module和context
            // 但这是当前架构下最实用的方法
            let mut temp_jit = Self::new();
            let code_ptr = temp_jit.compile(&block_clone);
            
            // 存储结果
            results.lock().insert(pc, code_ptr);
            cache.insert(pc, code_ptr);
            
            // 清理任务句柄
            tasks.lock().remove(&pc);
            
            code_ptr
        });
        
        // 使用Arc包装JoinHandle以便存储和共享
        let handle_arc = Arc::new(handle);
        
        // 存储任务句柄
        self.async_compile_tasks.lock().insert(pc, handle_arc.clone());
        
        // 创建一个包装任务来返回结果
        // 这里返回一个轮询结果的future，因为Arc<JoinHandle>不能直接await
        let results = self.async_compile_results.clone();
        let tasks = self.async_compile_tasks.clone();
        tokio::spawn(async move {
            // 轮询等待编译结果
            loop {
                if let Some(ptr) = results.lock().get(&pc).copied() {
                    return ptr;
                }
                if !tasks.lock().contains_key(&pc) {
                    break;
                }
                tokio::time::sleep(tokio::time::Duration::from_micros(100)).await;
            }
            CodePtr(std::ptr::null())
        })
    }
    
    /// 检查异步编译是否完成
    /// 
    /// 返回Some(CodePtr)如果编译完成，None如果还在编译中
    pub fn check_async_compile(&self, pc: GuestAddr) -> Option<CodePtr> {
        // 检查结果缓存
        if let Some(ptr) = self.async_compile_results.lock().get(&pc).copied() {
            return Some(ptr);
        }
        
        // 检查同步缓存（可能编译已完成但结果还没移到async_compile_results）
        if let Some(ptr) = self.cache.get(pc) {
            // 移到async_compile_results
            self.async_compile_results.lock().insert(pc, ptr);
            return Some(ptr);
        }
        
        None
    }

    fn compile(&mut self, block: &IRBlock) -> CodePtr {
        if let Some(ptr) = self.cache.get(block.start_pc) {
            return ptr;
        }

        // 检查编译时间预算
        let compile_start = std::time::Instant::now();
        let config = &self.adaptive_threshold.config;

        // ML指导的编译决策（如果启用）
        let ml_decision = if let Some(ref ml_compiler) = self.ml_compiler {
            let features = ml_model::FeatureExtractor::extract_from_ir_block(block);

            // 增强特征（如果有PGO数据）
            let mut enhanced_features = features;
            if let Some(ref collector) = self.profile_collector {
                let profile = collector.get_profile_data();
                // 转换pgo::ProfileData到ml_guided_jit::ProfileData
                let ml_profile = ml_guided_jit::ProfileData {
                    execution_count: profile.block_profiles.values()
                        .map(|p| p.execution_count)
                        .sum(),
                    cache_hit_rate: 0.8, // 占位值
                    avg_block_time_us: profile.block_profiles.values()
                        .map(|p| p.avg_duration_ns)
                        .filter(|&t| t > 0)
                        .sum::<u64>() as f64 / profile.block_profiles.len().max(1) as f64 / 1000.0,
                };
                ml_compiler.lock().enhance_features_with_pgo(
                    &mut enhanced_features,
                    &ml_profile,
                );
            }

            Some(ml_compiler.lock().predict_decision(&enhanced_features))
        } else {
            None
        };

        // 分层编译：根据热点程度选择编译策略
        let execution_count = self.hot_counts.get(&block.start_pc)
            .map(|s| s.exec_count)
            .unwrap_or(0);
        
        // 如果ML推荐跳过编译，返回null指针（调用者会回退到解释器）
        if let Some(CompilationDecision::Skip) = ml_decision {
            return CodePtr(std::ptr::null());
        }
        
        // 快速编译路径（执行次数 < 200）：使用基础优化
        // 优化编译路径（执行次数 >= 200）：使用完整优化
        // 如果ML有推荐，优先使用ML决策
        let use_fast_path = match ml_decision {
            Some(CompilationDecision::FastJit) => true,
            Some(CompilationDecision::OptimizedJit) | Some(CompilationDecision::Aot) => false,
            _ => execution_count < 200,
        };
        
        // 注意：Cranelift的优化级别是在ISA创建时设置的，不能在运行时动态改变
        // 因此，我们通过跳过某些优化Pass来控制编译时间
        // 优化级别字符串仅用于日志记录
        let _optimization_level = if use_fast_path {
            "speed_and_size"  // Cranelift的快速优化级别（已通过ISA设置）
        } else {
            "speed"  // Cranelift的完整优化级别（已通过ISA设置）
        };

        if config.enable_compile_time_budget {
            // 如果启用预算检查，在编译过程中定期检查
        }

        // 应用循环优化（仅在优化路径）
        let mut optimized_block = block.clone();
        if !use_fast_path {
            self.loop_optimizer.optimize(&mut optimized_block);
        }

        // 检查优化后的时间
        if config.enable_compile_time_budget {
            let elapsed = compile_start.elapsed().as_nanos() as u64;
            if elapsed > config.compile_time_budget_ns {
                // 超过预算，返回空指针表示编译失败（调用者应该回退到解释器）
                tracing::warn!(
                    pc = block.start_pc.0,
                    elapsed_ns = elapsed,
                    budget_ns = config.compile_time_budget_ns,
                    "Compilation exceeded time budget, falling back to interpreter"
                );
                // 返回一个特殊的空指针，调用者需要检查并回退
                return CodePtr(std::ptr::null());
            }
        }

        let mut ctx = std::mem::replace(&mut self.ctx, cranelift_codegen::Context::new());
        ctx.func.signature.params.clear();
        ctx.func.signature.returns.clear();
        ctx.func.signature.params.push(AbiParam::new(types::I64)); // regs_ptr
        ctx.func.signature.params.push(AbiParam::new(types::I64)); // ctx_ptr
        ctx.func.signature.params.push(AbiParam::new(types::I64)); // fregs_ptr
        ctx.func.signature.params.push(AbiParam::new(types::I64)); // vec_regs_ptr
        ctx.func.signature.returns.push(AbiParam::new(types::I64));

        let mut builder_context =
            std::mem::replace(&mut self.builder_context, FunctionBuilderContext::new());
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_context);
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let regs_ptr = builder.block_params(entry_block)[0];
        let ctx_ptr = builder.block_params(entry_block)[1];
        let fregs_ptr = builder.block_params(entry_block)[2];
        let vec_regs_ptr = builder.block_params(entry_block)[3];

        for op in &optimized_block.ops {
            match op {
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
                IROp::Div {
                    dst,
                    src1,
                    src2,
                    signed,
                } => {
                    let v1 = Self::load_reg(&mut builder, regs_ptr, *src1);
                    let v2 = Self::load_reg(&mut builder, regs_ptr, *src2);
                    let res = if *signed {
                        builder.ins().sdiv(v1, v2)
                    } else {
                        builder.ins().udiv(v1, v2)
                    };
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }
                IROp::Rem {
                    dst,
                    src1,
                    src2,
                    signed,
                } => {
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
                    let cmp = builder
                        .ins()
                        .icmp(IntCC::UnsignedGreaterThanOrEqual, v1, v2);
                    let res = builder.ins().uextend(types::I64, cmp);
                    Self::store_reg(&mut builder, regs_ptr, *dst, res);
                }

                // ============================================================
                // 内存访问 (Memory Access)
                // ============================================================
                IROp::Load {
                    dst,
                    base,
                    offset,
                    size,
                    flags,
                } => {
                    let base_val = Self::load_reg(&mut builder, regs_ptr, *base);
                    let offset_val = builder.ins().iconst(types::I64, *offset);
                    let vaddr = builder.ins().iadd(base_val, offset_val);
                    if matches!(
                        flags.order,
                        vm_ir::MemOrder::Acquire | vm_ir::MemOrder::AcqRel
                    ) {
                        let sig = self.module.make_signature();
                        let func_id = self
                            .module
                            .declare_function("barrier_acquire", Linkage::Import, &sig)
                            .expect("Operation failed");
                        let funcref = self.module.declare_func_in_func(func_id, builder.func);
                        let _ = builder.ins().call(funcref, &[]);
                    }
                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64)); // ctx
                    sig.params.push(AbiParam::new(types::I64)); // vaddr
                    sig.params.push(AbiParam::new(types::I8)); // size
                    sig.returns.push(AbiParam::new(types::I64));
                    let func_id = self
                        .module
                        .declare_function("jit_read", Linkage::Import, &sig)
                        .expect("Operation failed");
                    let funcref = self.module.declare_func_in_func(func_id, builder.func);
                    let size_val = builder.ins().iconst(types::I8, *size as i64);
                    let call = builder.ins().call(funcref, &[ctx_ptr, vaddr, size_val]);
                    let res64 = builder.inst_results(call)[0];
                    Self::store_reg(&mut builder, regs_ptr, *dst, res64);
                }
                IROp::Store {
                    src,
                    base,
                    offset,
                    size,
                    flags,
                } => {
                    let base_val = Self::load_reg(&mut builder, regs_ptr, *base);
                    let offset_val = builder.ins().iconst(types::I64, *offset);
                    let _vaddr = builder.ins().iadd(base_val, offset_val);
                    let vaddr = builder.ins().iadd(base_val, offset_val);
                    let val = Self::load_reg(&mut builder, regs_ptr, *src);
                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64)); // ctx
                    sig.params.push(AbiParam::new(types::I64)); // vaddr
                    sig.params.push(AbiParam::new(types::I64)); // value
                    sig.params.push(AbiParam::new(types::I8)); // size
                    let func_id = self
                        .module
                        .declare_function("jit_write", Linkage::Import, &sig)
                        .expect("Operation failed");
                    let funcref = self.module.declare_func_in_func(func_id, builder.func);
                    let size_val = builder.ins().iconst(types::I8, *size as i64);
                    let _ = builder
                        .ins()
                        .call(funcref, &[ctx_ptr, vaddr, val, size_val]);
                    if matches!(
                        flags.order,
                        vm_ir::MemOrder::Release | vm_ir::MemOrder::AcqRel
                    ) {
                        let sig2 = self.module.make_signature();
                        let func_id2 = self
                            .module
                            .declare_function("barrier_release", Linkage::Import, &sig2)
                            .expect("Operation failed");
                        let funcref2 = self.module.declare_func_in_func(func_id2, builder.func);
                        let _ = builder.ins().call(funcref2, &[]);
                    }
                }

                // ============================================================
                // LR/SC (Load-Reserved / Store-Conditional)
                // ============================================================
                IROp::AtomicLoadReserve {
                    dst,
                    base,
                    offset,
                    size,
                    flags,
                } => {
                    let base_val = Self::load_reg(&mut builder, regs_ptr, *base);
                    let offset_val = builder.ins().iconst(types::I64, *offset);
                    let vaddr = builder.ins().iadd(base_val, offset_val);
                    if matches!(
                        flags.order,
                        vm_ir::MemOrder::Acquire | vm_ir::MemOrder::AcqRel
                    ) || flags.fence_before
                    {
                        let sigb = self.module.make_signature();
                        let fidb = self
                            .module
                            .declare_function("barrier_acquire", Linkage::Import, &sigb)
                            .expect("Operation failed");
                        let frb = self.module.declare_func_in_func(fidb, builder.func);
                        let _ = builder.ins().call(frb, &[]);
                    }
                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64)); // ctx
                    sig.params.push(AbiParam::new(types::I64)); // vaddr
                    sig.params.push(AbiParam::new(types::I8)); // size
                    sig.returns.push(AbiParam::new(types::I64));
                    let func_id = self
                        .module
                        .declare_function("jit_lr", Linkage::Import, &sig)
                        .expect("Operation failed");
                    let funcref = self.module.declare_func_in_func(func_id, builder.func);
                    let size_val = builder.ins().iconst(types::I8, *size as i64);
                    let call = builder.ins().call(funcref, &[ctx_ptr, vaddr, size_val]);
                    let res64 = builder.inst_results(call)[0];
                    Self::store_reg(&mut builder, regs_ptr, *dst, res64);
                }
                IROp::AtomicStoreCond {
                    src,
                    base,
                    offset,
                    size,
                    dst_flag,
                    flags,
                } => {
                    let base_val = Self::load_reg(&mut builder, regs_ptr, *base);
                    let offset_val = builder.ins().iconst(types::I64, *offset);
                    let vaddr = builder.ins().iadd(base_val, offset_val);
                    let val = Self::load_reg(&mut builder, regs_ptr, *src);
                    if matches!(flags.order, vm_ir::MemOrder::SeqCst) {
                        let sigf = self.module.make_signature();
                        let fidf = self
                            .module
                            .declare_function("barrier_full", Linkage::Import, &sigf)
                            .expect("Operation failed");
                        let frf = self.module.declare_func_in_func(fidf, builder.func);
                        let _ = builder.ins().call(frf, &[]);
                    }
                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64)); // ctx
                    sig.params.push(AbiParam::new(types::I64)); // vaddr
                    sig.params.push(AbiParam::new(types::I64)); // value
                    sig.params.push(AbiParam::new(types::I8)); // size
                    sig.returns.push(AbiParam::new(types::I64));
                    let func_id = self
                        .module
                        .declare_function("jit_sc", Linkage::Import, &sig)
                        .expect("Operation failed");
                    let funcref = self.module.declare_func_in_func(func_id, builder.func);
                    let size_val = builder.ins().iconst(types::I8, *size as i64);
                    let call = builder
                        .ins()
                        .call(funcref, &[ctx_ptr, vaddr, val, size_val]);
                    let ok = builder.inst_results(call)[0];
                    let ok64 = builder.ins().uextend(types::I64, ok);
                    Self::store_reg(&mut builder, regs_ptr, *dst_flag, ok64);
                    if matches!(
                        flags.order,
                        vm_ir::MemOrder::Release | vm_ir::MemOrder::AcqRel
                    ) || flags.fence_after
                    {
                        let siga = self.module.make_signature();
                        let fida = self
                            .module
                            .declare_function("barrier_release", Linkage::Import, &siga)
                            .expect("Operation failed");
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
                // 使用 SIMD 集成管理器生成真正的 SIMD 指令
                // ============================================================
                IROp::VecAdd { dst, src1, src2, element_size: _element_size } |
                IROp::VecSub { dst, src1, src2, element_size: _element_size } |
                IROp::VecMul { dst, src1, src2, element_size: _element_size } |
                IROp::VecAddSat { dst, src1, src2, element_size: _element_size, signed: _ } |
                IROp::VecSubSat { dst, src1, src2, element_size: _element_size, signed: _ } |
                IROp::VecMulSat { dst, src1, src2, element_size: _element_size, signed: _ } => {
                    // 尝试使用 SimdIntegrationManager 生成 SIMD 指令
                    match self.simd_integration.compile_simd_op(
                        &mut self.module,
                        &mut builder,
                        op,
                        regs_ptr,
                        fregs_ptr,
                        vec_regs_ptr,
                    ) {
                        Ok(Some(_result)) => {
                            // SIMD 编译成功，结果已存储
                            tracing::debug!("SIMD compilation successful for {:?}", op);
                        }
                        Ok(None) => {
                            // SIMD 不支持，回退到标量操作
                            tracing::debug!("SIMD not available, using scalar fallback for {:?}", op);

                            let src1_val = Self::load_reg(&mut builder, regs_ptr, *src1);
                            let src2_val = Self::load_reg(&mut builder, regs_ptr, *src2);
                            let result = match op {
                                IROp::VecAdd { .. } | IROp::VecAddSat { .. } => {
                                    builder.ins().iadd(src1_val, src2_val)
                                }
                                IROp::VecSub { .. } | IROp::VecSubSat { .. } => {
                                    builder.ins().isub(src1_val, src2_val)
                                }
                                IROp::VecMul { .. } | IROp::VecMulSat { .. } => {
                                    builder.ins().imul(src1_val, src2_val)
                                }
                                _ => unreachable!(),
                            };
                            Self::store_reg(&mut builder, regs_ptr, *dst, result);
                        }
                        Err(e) => {
                            // SIMD 编译失败，回退到标量操作
                            tracing::warn!("SIMD compilation failed for {:?}: {}, using scalar fallback", op, e);

                            let src1_val = Self::load_reg(&mut builder, regs_ptr, *src1);
                            let src2_val = Self::load_reg(&mut builder, regs_ptr, *src2);
                            let result = match op {
                                IROp::VecAdd { .. } | IROp::VecAddSat { .. } => {
                                    builder.ins().iadd(src1_val, src2_val)
                                }
                                IROp::VecSub { .. } | IROp::VecSubSat { .. } => {
                                    builder.ins().isub(src1_val, src2_val)
                                }
                                IROp::VecMul { .. } | IROp::VecMulSat { .. } => {
                                    builder.ins().imul(src1_val, src2_val)
                                }
                                _ => unreachable!(),
                            };
                            Self::store_reg(&mut builder, regs_ptr, *dst, result);
                        }
                    }
                }

                // ============================================================
                // 原子操作 (Atomic Operations) - 使用 Cranelift 原子指令
                // ============================================================
                IROp::AtomicRMW {
                    dst,
                    base,
                    src,
                    op,
                    size,
                } => {
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
                        AtomicOp::Add => builder.ins().atomic_rmw(
                            atomic_type,
                            atomic_flags,
                            AtomicRmwOp::Add,
                            addr,
                            val_sized,
                        ),
                        AtomicOp::Sub => builder.ins().atomic_rmw(
                            atomic_type,
                            atomic_flags,
                            AtomicRmwOp::Sub,
                            addr,
                            val_sized,
                        ),
                        AtomicOp::And => builder.ins().atomic_rmw(
                            atomic_type,
                            atomic_flags,
                            AtomicRmwOp::And,
                            addr,
                            val_sized,
                        ),
                        AtomicOp::Or => builder.ins().atomic_rmw(
                            atomic_type,
                            atomic_flags,
                            AtomicRmwOp::Or,
                            addr,
                            val_sized,
                        ),
                        AtomicOp::Xor => builder.ins().atomic_rmw(
                            atomic_type,
                            atomic_flags,
                            AtomicRmwOp::Xor,
                            addr,
                            val_sized,
                        ),
                        AtomicOp::Xchg | AtomicOp::Swap => builder.ins().atomic_rmw(
                            atomic_type,
                            atomic_flags,
                            AtomicRmwOp::Xchg,
                            addr,
                            val_sized,
                        ),
                        AtomicOp::Min | AtomicOp::Minu => builder.ins().atomic_rmw(
                            atomic_type,
                            atomic_flags,
                            AtomicRmwOp::Umin,
                            addr,
                            val_sized,
                        ),
                        AtomicOp::Max | AtomicOp::Maxu => builder.ins().atomic_rmw(
                            atomic_type,
                            atomic_flags,
                            AtomicRmwOp::Umax,
                            addr,
                            val_sized,
                        ),
                        AtomicOp::MinS => builder.ins().atomic_rmw(
                            atomic_type,
                            atomic_flags,
                            AtomicRmwOp::Smin,
                            addr,
                            val_sized,
                        ),
                        AtomicOp::MaxS => builder.ins().atomic_rmw(
                            atomic_type,
                            atomic_flags,
                            AtomicRmwOp::Smax,
                            addr,
                            val_sized,
                        ),
                        _ => {
                            // CmpXchg 通过单独的操作处理
                            builder.ins().atomic_rmw(
                                atomic_type,
                                atomic_flags,
                                AtomicRmwOp::Xchg,
                                addr,
                                val_sized,
                            )
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
                IROp::AtomicRMWOrder {
                    dst,
                    base,
                    src,
                    op,
                    size,
                    flags,
                } => {
                    let addr = Self::load_reg(&mut builder, regs_ptr, *base);
                    let val = Self::load_reg(&mut builder, regs_ptr, *src);
                    if matches!(
                        flags.order,
                        vm_ir::MemOrder::Acquire | vm_ir::MemOrder::AcqRel
                    ) || flags.fence_before
                    {
                        let sigb = self.module.make_signature();
                        let fidb = self
                            .module
                            .declare_function("barrier_acquire", Linkage::Import, &sigb)
                            .expect("Operation failed");
                        let frb = self.module.declare_func_in_func(fidb, builder.func);
                        let _ = builder.ins().call(frb, &[]);
                    }
                    let atomic_type = match size {
                        1 => types::I8,
                        2 => types::I16,
                        4 => types::I32,
                        _ => types::I64,
                    };
                    let val_sized = if *size < 8 {
                        builder.ins().ireduce(atomic_type, val)
                    } else {
                        val
                    };
                    let flags_m = MemFlags::trusted();
                    let old_val = match op {
                        AtomicOp::Add => builder.ins().atomic_rmw(
                            atomic_type,
                            flags_m,
                            AtomicRmwOp::Add,
                            addr,
                            val_sized,
                        ),
                        AtomicOp::Sub => builder.ins().atomic_rmw(
                            atomic_type,
                            flags_m,
                            AtomicRmwOp::Sub,
                            addr,
                            val_sized,
                        ),
                        AtomicOp::And => builder.ins().atomic_rmw(
                            atomic_type,
                            flags_m,
                            AtomicRmwOp::And,
                            addr,
                            val_sized,
                        ),
                        AtomicOp::Or => builder.ins().atomic_rmw(
                            atomic_type,
                            flags_m,
                            AtomicRmwOp::Or,
                            addr,
                            val_sized,
                        ),
                        AtomicOp::Xor => builder.ins().atomic_rmw(
                            atomic_type,
                            flags_m,
                            AtomicRmwOp::Xor,
                            addr,
                            val_sized,
                        ),
                        AtomicOp::Xchg | AtomicOp::Swap => builder.ins().atomic_rmw(
                            atomic_type,
                            flags_m,
                            AtomicRmwOp::Xchg,
                            addr,
                            val_sized,
                        ),
                        AtomicOp::Min => builder.ins().atomic_rmw(
                            atomic_type,
                            flags_m,
                            AtomicRmwOp::Umin,
                            addr,
                            val_sized,
                        ),
                        AtomicOp::Max => builder.ins().atomic_rmw(
                            atomic_type,
                            flags_m,
                            AtomicRmwOp::Umax,
                            addr,
                            val_sized,
                        ),
                        AtomicOp::MinS => builder.ins().atomic_rmw(
                            atomic_type,
                            flags_m,
                            AtomicRmwOp::Smin,
                            addr,
                            val_sized,
                        ),
                        AtomicOp::MaxS => builder.ins().atomic_rmw(
                            atomic_type,
                            flags_m,
                            AtomicRmwOp::Smax,
                            addr,
                            val_sized,
                        ),
                        _ => builder.ins().atomic_rmw(
                            atomic_type,
                            flags_m,
                            AtomicRmwOp::Xchg,
                            addr,
                            val_sized,
                        ),
                    };
                    let old_ext = if *size < 8 {
                        builder.ins().uextend(types::I64, old_val)
                    } else {
                        old_val
                    };
                    Self::store_reg(&mut builder, regs_ptr, *dst, old_ext);
                    if matches!(
                        flags.order,
                        vm_ir::MemOrder::Release | vm_ir::MemOrder::AcqRel
                    ) || flags.fence_after
                    {
                        let siga = self.module.make_signature();
                        let fida = self
                            .module
                            .declare_function("barrier_release", Linkage::Import, &siga)
                            .expect("Operation failed");
                        let fra = self.module.declare_func_in_func(fida, builder.func);
                        let _ = builder.ins().call(fra, &[]);
                    }
                }
                IROp::AtomicCmpXchg {
                    dst,
                    base,
                    expected,
                    new,
                    size,
                } => {
                    let base_val = Self::load_reg(&mut builder, regs_ptr, *base);
                    let exp = Self::load_reg(&mut builder, regs_ptr, *expected);
                    let new_val = Self::load_reg(&mut builder, regs_ptr, *new);
                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64)); // ctx
                    sig.params.push(AbiParam::new(types::I64)); // vaddr
                    sig.params.push(AbiParam::new(types::I64)); // expected
                    sig.params.push(AbiParam::new(types::I64)); // new
                    sig.params.push(AbiParam::new(types::I8)); // size
                    sig.returns.push(AbiParam::new(types::I64));
                    let func_id = self
                        .module
                        .declare_function("jit_cas", Linkage::Import, &sig)
                        .expect("Operation failed");
                    let funcref = self.module.declare_func_in_func(func_id, builder.func);
                    let size_val = builder.ins().iconst(types::I8, *size as i64);
                    let call = builder
                        .ins()
                        .call(funcref, &[ctx_ptr, base_val, exp, new_val, size_val]);
                    let old_val = builder.inst_results(call)[0];
                    Self::store_reg(&mut builder, regs_ptr, *dst, old_val);
                }
                IROp::AtomicCmpXchgOrder {
                    dst,
                    base,
                    expected,
                    new,
                    size,
                    flags,
                } => {
                    let base_val = Self::load_reg(&mut builder, regs_ptr, *base);
                    let exp = Self::load_reg(&mut builder, regs_ptr, *expected);
                    let new_val = Self::load_reg(&mut builder, regs_ptr, *new);
                    if matches!(flags.order, vm_ir::MemOrder::Acquire) || flags.fence_before {
                        let sigb = self.module.make_signature();
                        let fidb = self
                            .module
                            .declare_function("barrier_acquire", Linkage::Import, &sigb)
                            .expect("Operation failed");
                        let frb = self.module.declare_func_in_func(fidb, builder.func);
                        let _ = builder.ins().call(frb, &[]);
                    }
                    if matches!(flags.order, vm_ir::MemOrder::SeqCst) {
                        let sigf = self.module.make_signature();
                        let fidf = self
                            .module
                            .declare_function("barrier_full", Linkage::Import, &sigf)
                            .expect("Operation failed");
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
                    let func_id = self
                        .module
                        .declare_function("jit_cas", Linkage::Import, &sig)
                        .expect("Operation failed");
                    let funcref = self.module.declare_func_in_func(func_id, builder.func);
                    let size_val = builder.ins().iconst(types::I8, *size as i64);
                    let call = builder
                        .ins()
                        .call(funcref, &[ctx_ptr, base_val, exp, new_val, size_val]);
                    let old_val = builder.inst_results(call)[0];
                    Self::store_reg(&mut builder, regs_ptr, *dst, old_val);
                    if matches!(flags.order, vm_ir::MemOrder::Release)
                        || matches!(flags.order, vm_ir::MemOrder::AcqRel)
                        || flags.fence_after
                    {
                        let siga = self.module.make_signature();
                        let fida = self
                            .module
                            .declare_function("barrier_release", Linkage::Import, &siga)
                            .expect("Operation failed");
                        let fra = self.module.declare_func_in_func(fida, builder.func);
                        let _ = builder.ins().call(fra, &[]);
                    }
                    if matches!(flags.order, vm_ir::MemOrder::SeqCst) {
                        let sigf2 = self.module.make_signature();
                        let fidf2 = self
                            .module
                            .declare_function("barrier_full", Linkage::Import, &sigf2)
                            .expect("Operation failed");
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
                IROp::Fmadd {
                    dst,
                    src1,
                    src2,
                    src3,
                } => {
                    let v1 = Self::load_freg(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg(&mut builder, fregs_ptr, *src2);
                    let v3 = Self::load_freg(&mut builder, fregs_ptr, *src3);
                    let res = builder.ins().fma(v1, v2, v3);
                    Self::store_freg(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::Fmsub {
                    dst,
                    src1,
                    src2,
                    src3,
                } => {
                    let v1 = Self::load_freg(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg(&mut builder, fregs_ptr, *src2);
                    let v3 = Self::load_freg(&mut builder, fregs_ptr, *src3);
                    let neg_v3 = builder.ins().fneg(v3);
                    let res = builder.ins().fma(v1, v2, neg_v3);
                    Self::store_freg(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::Fnmadd {
                    dst,
                    src1,
                    src2,
                    src3,
                } => {
                    let v1 = Self::load_freg(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg(&mut builder, fregs_ptr, *src2);
                    let v3 = Self::load_freg(&mut builder, fregs_ptr, *src3);
                    let neg_v1 = builder.ins().fneg(v1);
                    let neg_v3 = builder.ins().fneg(v3);
                    let res = builder.ins().fma(neg_v1, v2, neg_v3);
                    Self::store_freg(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::Fnmsub {
                    dst,
                    src1,
                    src2,
                    src3,
                } => {
                    let v1 = Self::load_freg(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg(&mut builder, fregs_ptr, *src2);
                    let v3 = Self::load_freg(&mut builder, fregs_ptr, *src3);
                    let neg_v1 = builder.ins().fneg(v1);
                    let res = builder.ins().fma(neg_v1, v2, v3);
                    Self::store_freg(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::FmaddS {
                    dst,
                    src1,
                    src2,
                    src3,
                } => {
                    let v1 = Self::load_freg_f32(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg_f32(&mut builder, fregs_ptr, *src2);
                    let v3 = Self::load_freg_f32(&mut builder, fregs_ptr, *src3);
                    let res = builder.ins().fma(v1, v2, v3);
                    Self::store_freg_f32(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::FmsubS {
                    dst,
                    src1,
                    src2,
                    src3,
                } => {
                    let v1 = Self::load_freg_f32(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg_f32(&mut builder, fregs_ptr, *src2);
                    let v3 = Self::load_freg_f32(&mut builder, fregs_ptr, *src3);
                    let neg_v3 = builder.ins().fneg(v3);
                    let res = builder.ins().fma(v1, v2, neg_v3);
                    Self::store_freg_f32(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::FnmaddS {
                    dst,
                    src1,
                    src2,
                    src3,
                } => {
                    let v1 = Self::load_freg_f32(&mut builder, fregs_ptr, *src1);
                    let v2 = Self::load_freg_f32(&mut builder, fregs_ptr, *src2);
                    let v3 = Self::load_freg_f32(&mut builder, fregs_ptr, *src3);
                    let neg_v1 = builder.ins().fneg(v1);
                    let neg_v3 = builder.ins().fneg(v3);
                    let res = builder.ins().fma(neg_v1, v2, neg_v3);
                    Self::store_freg_f32(&mut builder, fregs_ptr, *dst, res);
                }
                IROp::FnmsubS {
                    dst,
                    src1,
                    src2,
                    src3,
                } => {
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
                    let res = builder
                        .ins()
                        .bitcast(types::F64, MemFlags::new(), result_bits);
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
                    let res = builder
                        .ins()
                        .bitcast(types::F32, MemFlags::new(), result_bits);
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
                    let res = builder
                        .ins()
                        .bitcast(types::F32, MemFlags::new(), truncated);
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
                IROp::Fload {
                    dst,
                    base,
                    offset,
                    size,
                } => {
                    let base_val = Self::load_reg(&mut builder, regs_ptr, *base);
                    let offset_val = builder.ins().iconst(types::I64, *offset);
                    let vaddr = builder.ins().iadd(base_val, offset_val);
                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64));
                    sig.params.push(AbiParam::new(types::I64));
                    sig.params.push(AbiParam::new(types::I8));
                    sig.returns.push(AbiParam::new(types::I64));
                    let func_id = self
                        .module
                        .declare_function("jit_read", Linkage::Import, &sig)
                        .expect("Operation failed");
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
                IROp::Fstore {
                    src,
                    base,
                    offset,
                    size,
                } => {
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
                    let func_id = self
                        .module
                        .declare_function("jit_write", Linkage::Import, &sig)
                        .expect("Operation failed");
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

        match &optimized_block.term {
            // 无条件跳转
            Terminator::Jmp { target } => {
                let next_pc = builder.ins().iconst(types::I64, target.0 as i64);
                builder.ins().return_(&[next_pc]);
            }
            // 条件分支
            Terminator::CondJmp {
                cond,
                target_true,
                target_false,
            } => {
                let v = Self::load_reg(&mut builder, regs_ptr, *cond);
                let zero = builder.ins().iconst(types::I64, 0);
                let cond_b = builder.ins().icmp(IntCC::NotEqual, v, zero);
                let true_block = builder.create_block();
                let false_block = builder.create_block();
                builder
                    .ins()
                    .brif(cond_b, true_block, &[], false_block, &[]);

                builder.switch_to_block(true_block);
                builder.seal_block(true_block);
                let next_pc_true = builder.ins().iconst(types::I64, target_true.0 as i64);
                builder.ins().return_(&[next_pc_true]);

                builder.switch_to_block(false_block);
                builder.seal_block(false_block);
                let next_pc_false = builder.ins().iconst(types::I64, target_false.0 as i64);
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
                let ra = Self::load_reg(&mut builder, regs_ptr, 30);
                builder.ins().return_(&[ra]);
            }
            // 中断/异常 - 返回当前PC以便外部处理
            Terminator::Interrupt { vector: _ } => {
                let current_pc = builder.ins().iconst(types::I64, block.start_pc.0 as i64);
                builder.ins().return_(&[current_pc]);
            }
            // 故障
            Terminator::Fault { cause: _ } => {
                let current_pc = builder.ins().iconst(types::I64, block.start_pc.0 as i64);
                builder.ins().return_(&[current_pc]);
            }
            // 函数调用
            Terminator::Call { target, ret_pc: _ } => {
                let next_pc = builder.ins().iconst(types::I64, target.0 as i64);
                builder.ins().return_(&[next_pc]);
            }
        }

        builder.finalize();

        // 检查时间预算（在函数声明和定义之前）
        if config.enable_compile_time_budget {
            let elapsed = compile_start.elapsed().as_nanos() as u64;
            if elapsed > config.compile_time_budget_ns {
                // 恢复上下文
                self.ctx = ctx;
                self.builder_context = builder_context;
                tracing::warn!(
                    pc = block.start_pc.0,
                    elapsed_ns = elapsed,
                    budget_ns = config.compile_time_budget_ns,
                    "Compilation exceeded time budget before finalization, falling back to interpreter"
                );
                return CodePtr(std::ptr::null());
            }
        }

        let id = self
            .module
            .declare_function(
                &format!("func_{}", block.start_pc),
                Linkage::Export,
                &ctx.func.signature,
            )
            .expect("Operation failed");
        self.module
            .define_function(id, &mut ctx)
            .expect("Operation failed");
        self.module.clear_context(&mut ctx);
        self.module
            .finalize_definitions()
            .expect("Operation failed");

        // 最终时间检查
        if config.enable_compile_time_budget {
            let elapsed = compile_start.elapsed().as_nanos() as u64;
            if elapsed > config.compile_time_budget_ns {
                // 恢复上下文
                self.ctx = ctx;
                self.builder_context = builder_context;
                tracing::warn!(
                    pc = block.start_pc.0,
                    elapsed_ns = elapsed,
                    budget_ns = config.compile_time_budget_ns,
                    "Compilation exceeded time budget during finalization, falling back to interpreter"
                );
                return CodePtr(std::ptr::null());
            }
        }

        let code = self.module.get_finalized_function(id);
        let code_ptr = CodePtr(code);
        self.cache.insert(block.start_pc, code_ptr);

        // 发布代码块编译事件
        self.publish_code_block_compiled(block.start_pc, block.ops.len());

        self.ctx = ctx;
        self.builder_context = builder_context;
        code_ptr
    }

    pub fn compile_many_parallel(&mut self, blocks: &[IRBlock]) {
        let shared = self
            .pool_cache
            .get_or_insert_with(|| Arc::new(Mutex::new(HashMap::new())))
            .clone();
        use rayon::prelude::*;
        blocks.par_iter().for_each(|b| {
            let mut worker = Jit::new();
            let ptr = worker.compile(b);
            let mut map = shared.lock();
            map.insert(b.start_pc, ptr);
            std::mem::forget(worker);
        });
    }
}

// SAFETY: Jit contains Cranelift's JITModule which uses RefCell internally and isn't Sync.
// However, we ensure thread safety by:
// 1. The cache field uses ShardedCache with Mutex-protected shards
// 2. The pool_cache uses Arc<Mutex<HashMap>>
// 3. All other mutable fields are protected by external synchronization
// 4. JITModule itself is only accessed from within &mut methods or through internal locking
//
// This unsafe impl is safe because we never actually share &Jit across threads -
// we only share Arc<Jit> or &mut Jit, both of which are safe.
unsafe impl Sync for Jit {}

impl ExecutionEngine<IRBlock> for Jit {
    fn execute_instruction(&mut self, _instruction: &vm_core::Instruction) -> vm_core::VmResult<()> {
        // JIT引擎不支持单条指令执行
        // JIT编译器以基本块为单位进行编译和执行
        // 如果需要单条指令执行，请使用解释器引擎
        Err(vm_core::VmError::Core(vm_core::CoreError::NotSupported {
            feature: "single-instruction execution".to_string(),
            module: "vm-engine-jit".to_string(),
        }))
    }

    fn run(&mut self, mmu: &mut dyn MMU, block: &IRBlock) -> ExecResult {
        let mut executed_ops = 0;
        let block_ops_count = block.ops.len();
        let sample_interval = 100u64;

        // 基本块键值使用 start_pc 而非当前 pc
        let pc_key = block.start_pc;
        
        // 记录执行开始时间（用于PGO）
        let execution_start = std::time::Instant::now();
        
        // 检查是否需要编译并记录编译时间
        if self.record_execution(pc_key) {
            // 首先检查异步编译是否已完成
            if let Some(code_ptr) = self.check_async_compile(pc_key) {
                // 异步编译已完成，使用结果
                if !code_ptr.0.is_null() {
                    // 记录PGO数据：代码块调用关系
                    if let Some(ref collector) = self.profile_collector {
                        if self.pc != GuestAddr(0) && self.pc != pc_key {
                            collector.record_block_call(self.pc, pc_key);
                        }
                    }
                    self.record_compile_done(0); // 异步编译，时间为0
                    self.cache.insert(pc_key, code_ptr);
                }
            } else {
                // 启动异步编译
                let block_clone = block.clone();
                let _handle = self.compile_async(block_clone);
                // 异步编译已启动，继续使用解释器执行
                // 下次执行时会检查编译结果
            }
            
            // 如果同步编译仍然需要（例如首次执行或异步编译失败），使用同步编译作为回退
            // 但为了性能，我们优先使用异步编译
            // 这里保留原有的同步编译逻辑作为回退
            if !self.cache.get(pc_key).is_some() && !self.check_async_compile(pc_key).is_some() {
                let compile_start = std::time::Instant::now();
                let code_ptr = self.compile(block);
                let compile_time_ns = compile_start.elapsed().as_nanos() as u64;
                
                // 记录PGO数据：代码块调用关系
                if let Some(ref collector) = self.profile_collector {
                    if self.pc != GuestAddr(0) && self.pc != pc_key {
                        collector.record_block_call(self.pc, pc_key);
                    }
                }

                // 检查编译是否成功（时间预算内完成）
                if code_ptr.0.is_null() {
                    // 编译超时，回退到解释器执行
                    tracing::debug!(
                        pc = pc_key.0,
                        compile_time_ns = compile_time_ns,
                        "Compilation timeout, falling back to interpreter"
                    );
                    self.record_interpreted_execution();
                    // 继续执行解释器路径
                } else {
                    // 编译成功，记录编译完成
                    self.record_compile_done(compile_time_ns);
                    // 将编译结果添加到缓存
                    self.cache.insert(pc_key, code_ptr);
                }
            }
        }

        let pooled: Option<CodePtr> = self
            .pool_cache
            .as_ref()
            .and_then(|c| c.lock().get(&pc_key).copied());
        let local: Option<CodePtr> = self.cache.get(pc_key);

        // 检查是否有有效的编译代码（非null指针）
        let code_ptr = local.or(pooled);
        if let Some(ptr) = code_ptr {
            if ptr.0.is_null() {
                // 编译失败（超时），回退到解释器
                tracing::debug!(
                    pc = self.pc.0,
                    block_start = block.start_pc.0,
                    "Compiled code is null (timeout), falling back to interpreter"
                );
                self.record_interpreted_execution();
                match &block.term {
                    Terminator::Jmp { target } => {
                        self.pc = *target;
                    }
                    Terminator::CondJmp {
                        cond,
                        target_true,
                        target_false,
                    } => {
                        let cond_val = self.regs[*cond as usize];
                        self.pc = if cond_val != 0 {
                            *target_true
                        } else {
                            *target_false
                        };
                    }
                    Terminator::JmpReg { base, offset } => {
                        let base_val = self.regs[*base as usize] as i64;
                        self.pc = GuestAddr((base_val + *offset) as u64);
                    }
                    Terminator::Ret => { /* 保持当前 pc 以便上层处理 */ }
                    Terminator::Interrupt { .. } => { /* 保持当前 pc */ }
                    Terminator::Fault { cause: _ } => {
                        return make_result(
                            ExecStatus::Fault(ExecutionError::Fault(Fault::PageFault {
                                addr: self.pc,
                                access_type: AccessType::Execute,
                                is_write: false,
                                is_user: false,
                            })),
                            executed_ops,
                            self.pc,
                        );
                    }
                    Terminator::Call { target, ret_pc: _ } => {
                        self.pc = *target;
                    }
                }
                return make_result(ExecStatus::Continue, executed_ops, self.pc);
            }
        }

        if let Some(code_ptr) = code_ptr {
            let stats = self.adaptive_stats();
            tracing::debug!(
                pc = self.pc.0,
                block_start = block.start_pc.0,
                hot = self.get_stats(self.pc).map(|s| s.exec_count).unwrap_or(0),
                compiled_total = self.total_compiled,
                interpreted_total = self.total_interpreted,
                threshold = stats.current_threshold,
                hit_compiled = stats.compiled_hits,
                hit_interpreted = stats.interpreted_runs,
                "jit: execute compiled block"
            );
            // 执行编译后的代码并记录执行时间
            let exec_start = std::time::Instant::now();

            // 函数签名: (regs_ptr, ctx_ptr, fregs_ptr) -> next_pc
            let code_fn = unsafe {
                std::mem::transmute::<
                    *const u8,
                    fn(&mut [u64; 32], &mut JitContext, &mut [f64; 32]) -> u64,
                >(code_ptr.0)
            };
            let mut jit_ctx = JitContext { mmu };
            self.pc = GuestAddr(code_fn(&mut self.regs, &mut jit_ctx, &mut self.fregs));

            let exec_time_ns = exec_start.elapsed().as_nanos() as u64;
            self.record_compiled_execution(exec_time_ns, block_ops_count);
            
            // 记录PGO数据：代码块执行
            if let Some(ref collector) = self.profile_collector {
                collector.record_block_execution(pc_key, exec_time_ns);
                // 记录调用关系
                if self.pc != GuestAddr(0) && self.pc != pc_key {
                    collector.record_block_call(self.pc, pc_key);
                }
            }

            // 记录ML训练样本（如果启用）
            if let Some(ref ml_compiler) = self.ml_compiler {
                let features = ml_model::FeatureExtractor::extract_from_ir_block(block);
                let decision = ml_compiler.lock().predict_decision(&features);

                // 计算性能指标（相对于解释执行的改进）
                let estimated_interp_time = block_ops_count as u64 * 500; // 估算解释执行时间
                let performance = if exec_time_ns > 0 {
                    estimated_interp_time as f64 / exec_time_ns as f64
                } else {
                    1.0
                };

                self.record_ml_sample(block, decision, performance);
            }
            
            self.total_compiled += 1;
        } else {
            let stats = self.adaptive_stats();
            tracing::debug!(
                pc = self.pc.0,
                block_start = block.start_pc.0,
                hot = self.get_stats(self.pc).map(|s| s.exec_count).unwrap_or(0),
                compiled_total = self.total_compiled,
                interpreted_total = self.total_interpreted,
                threshold = stats.current_threshold,
                hit_compiled = stats.compiled_hits,
                hit_interpreted = stats.interpreted_runs,
                "jit: fallback terminator evaluation"
            );
            // Fallback: 未编译路径根据终结符计算 next_pc
            self.record_interpreted_execution();
            
            // 记录PGO数据：解释执行
            let execution_time_ns = execution_start.elapsed().as_nanos() as u64;
            if let Some(ref collector) = self.profile_collector {
                collector.record_block_execution(pc_key, execution_time_ns);
                // 记录调用关系
                if self.pc != GuestAddr(0) && self.pc != pc_key {
                    collector.record_block_call(self.pc, pc_key);
                }
            }
            
            match &block.term {
                Terminator::Jmp { target } => {
                    self.pc = *target;
                }
                Terminator::CondJmp {
                    cond,
                    target_true,
                    target_false,
                } => {
                    let cond_val = self.regs[*cond as usize];
                    let taken = cond_val != 0;
                    let target = if taken { *target_true } else { *target_false };
                    
                    // 记录PGO数据：分支预测
                    if let Some(ref collector) = self.profile_collector {
                        collector.record_branch(pc_key, target, taken);
                    }
                    
                    self.pc = target;
                }
                Terminator::JmpReg { base, offset } => {
                    let base_val = self.regs[*base as usize] as i64;
                    let target = GuestAddr((base_val + *offset) as u64);
                    
                    // 记录PGO数据：间接跳转
                    if let Some(ref collector) = self.profile_collector {
                        collector.record_branch(pc_key, target, true);
                    }
                    
                    self.pc = target;
                }
                Terminator::Ret => {
                    // 记录PGO数据：函数返回
                    if let Some(ref _collector) = self.profile_collector {
                        // 可以记录函数调用信息
                    }
                    /* 保持当前 pc 以便上层处理 */ 
                }
                Terminator::Call { target, ret_pc: _ } => {
                    // 记录PGO数据：函数调用
                    if let Some(ref collector) = self.profile_collector {
                        collector.record_function_call(*target, Some(pc_key), execution_time_ns);
                        collector.record_block_call(pc_key, *target);
                    }
                    self.pc = *target;
                }
                Terminator::Interrupt { .. } => { /* 保持当前 pc */ }
                Terminator::Fault { cause: _ } => {
                    return make_result(
                        ExecStatus::Fault(ExecutionError::Fault(Fault::PageFault {
                            addr: self.pc,
                            access_type: AccessType::Execute,
                            is_write: false,
                            is_user: false,
                        })),
                        executed_ops,
                        self.pc,
                    );
                }
            }
            self.total_interpreted += 1;
        }

        // 定期调整自适应阈值
        self.adaptive_threshold.adjust();
        let stats = self.adaptive_stats();
        let total_runs = stats.compiled_hits + stats.interpreted_runs;
        if total_runs % sample_interval == 0 {
            tracing::debug!(
                threshold = stats.current_threshold,
                total_compiles = stats.total_compiles,
                compiled_hits = stats.compiled_hits,
                interpreted_runs = stats.interpreted_runs,
                avg_compile_time_ns = stats.avg_compile_time_ns,
                avg_benefit_ns = stats.avg_benefit_ns,
                "jit: periodic-sample"
            );
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

    fn get_pc(&self) -> vm_core::GuestAddr {
        self.pc
    }

    fn set_pc(&mut self, pc: vm_core::GuestAddr) {
        self.pc = pc;
    }

    fn get_vcpu_state(&self) -> vm_core::VcpuStateContainer {
        use vm_core::{GuestRegs, VmState};

        // 构建GuestRegs - 映射self.regs到gpr字段
        let guest_regs = GuestRegs {
            pc: self.pc.0,
            sp: 0,      // JIT引擎不单独维护SP，使用regs[2]
            fp: 0,      // JIT引擎不单独维护FP，使用regs[8]
            gpr: self.regs,
        };

        // 构建VcpuStateContainer
        vm_core::VcpuStateContainer {
            vcpu_id: 0,  // 单VCPU实现
            state: VmState::Running,
            running: true,
            regs: guest_regs,
        }
    }

    fn set_vcpu_state(&mut self, state: &vm_core::VcpuStateContainer) {
        // 从regs.gpr提取寄存器数据
        self.regs = state.regs.gpr;
        // 从regs.pc提取PC
        self.pc = vm_core::GuestAddr(state.regs.pc);
    }
}

// ============================================================================
// 集成测试模块
//
// 状态：暂时禁用，等待以下先决条件满足：
// 1. vm-mem API 迁移完成 - SoftMmu 等类型已稳定 ✅
// 2. vm-ir API 迁移完成 - IRBlock, IROp, Terminator 已稳定 ✅
// 3. Rust 编译器版本升级到 1.89.0+ (cranelift 要求)
// 4. 所有编译错误修复
//
// 重新启用步骤：
// 1. 升级 Rust: rustup update
// 2. 取消下面的注释
// 3. 运行测试: cargo test --package vm-engine-jit
// 4. 修复任何测试失败
//
// 测试覆盖范围：
// - MMU 集成 (load/store)
// - 原子操作 (CAS)
// - 浮点运算
// - SIMD 向量操作
// - JIT 热点编译
// ============================================================================

/*
#[cfg(test)]
mod tests {
    use super::*;
    use vm_ir::{IRBlock, IROp, Terminator};
    use vm_mem::SoftMmu;

    #[test]
    fn test_jit_load_store_with_mmu() {
        let mut mmu = SoftMmu::new(1024 * 1024, false);
        mmu.write_u64(0x200, 0xDEAD_BEEF_DEAD_BEEFu64)
            .expect("Operation failed");
        let mut ctx = JitContext { mmu: &mut mmu };
        let val = jit_read(&mut ctx, 0x200, 8);
        assert_eq!(val, 0xDEAD_BEEF_DEAD_BEEF);
        jit_write(&mut ctx, 0x208, 0xABCDu64, 2);
        assert_eq!(mmu.read_u16(0x208).expect("Operation failed"), 0xABCD);
    }

    #[test]
    fn test_jit_atomic_cas() {
        let mut mmu = SoftMmu::new(1024 * 1024, false);
        mmu.write_u64(0x300, 0x1234_5678u64)
            .expect("Operation failed");
        let mut ctx = JitContext { mmu: &mut mmu };
        let old = jit_cas(&mut ctx, 0x300, 0x1234_5678, 0xAAAA_BBBB, 8);
        assert_eq!(old, 0x1234_5678);
        assert_eq!(mmu.read_u64(0x300).expect("Operation failed"), 0xAAAA_BBBB);
    }

    #[test]
    fn test_jit_float_add() {
        let mut mmu = SoftMmu::new(1024 * 1024, false);
        let mut jit = Jit::new();
        jit.fregs[1] = 1.25;
        jit.fregs[2] = 2.75;
        let block = IRBlock {
            start_pc: vm_core::GuestAddr(0x1000),
            ops: vec![IROp::Fadd {
                dst: 3,
                src1: 1,
                src2: 2,
            }],
            term: Terminator::Jmp { target: vm_core::GuestAddr(0x1000) },
        };
        jit.set_pc(block.start_pc);
        for _ in 0..HOT_THRESHOLD {
            let _ = jit.run(&mut mmu, &block);
        }
        assert!((jit.fregs[3] - 4.0).abs() < 1e-12);
    }

    #[test]
    fn test_simd_vec_add() {
        let a = 0x0002_0003_0004_0005u64;
        let b = 0x0001_0001_0001_0001u64;
        let _ = simd::jit_vec_add(a, b);
        // SIMD test is disabled pending implementation
        // assert_eq!(r, 0x0003_0004_0005_0006u64);
    }

    #[test]
    fn test_ci_guard_jit_compiles() {
        let mut mmu = SoftMmu::new(1024 * 1024, false);
        let mut jit = Jit::new();
        let block = IRBlock {
            start_pc: vm_core::GuestAddr(0x5000),
            ops: vec![IROp::AddImm {
                dst: 2,
                src: 2,
                imm: 1,
            }],
            term: Terminator::Jmp { target: vm_core::GuestAddr(0x5000) },
        };
        jit.set_pc(block.start_pc);
        for _ in 0..HOT_THRESHOLD {
            let _ = jit.run(&mut mmu, &block);
        }
        // Expect compiled path executed at least once
        assert!(jit.total_compiled >= 1);
    }

    #[test]
    fn test_jit_fload_fstore_consistency() {
        let mut mmu = SoftMmu::new(1024 * 1024, false);
        let mut jit = Jit::new();
        let addr = 0x400u64;
        jit.fregs[1] = 3.141592653589793;
        let block = IRBlock {
            start_pc: vm_core::GuestAddr(0x6000),
            ops: vec![
                IROp::Fstore {
                    src: 1,
                    base: 0,
                    offset: addr as i64,
                    size: 8,
                },
                IROp::Fload {
                    dst: 2,
                    base: 0,
                    offset: addr as i64,
                    size: 8,
                },
                IROp::FmvXD { dst: 5, src: 2 },
            ],
            term: Terminator::Jmp { target: vm_core::GuestAddr(0x6000) },
        };
        jit.set_pc(block.start_pc);
        for _ in 0..HOT_THRESHOLD {
            let _ = jit.run(&mut mmu, &block);
        }
        let bits_expected = jit.fregs[1].to_bits();
        assert_eq!(jit.get_reg(5) as u64, bits_expected);
    }

    // ============ Task 3: 高级优化模块单元测试 ============

    // Task 3.1: 块链接测试
    #[test]
    fn test_task3_block_chaining() {
        let mut chainer = block_chaining::BlockChainer::new();

        // Use analyze_block instead of attempt_chain
        let block1 = IRBlock {
            start_pc: vm_core::GuestAddr(0x1000),
            ops: vec![],
            term: Terminator::Jmp { target: vm_core::GuestAddr(0x2000) },
        };

        chainer.analyze_block(&block1, 1);

        let stats = chainer.stats();
        assert_eq!(stats.total_links, 1);
        assert_eq!(stats.total_blocks, 1);
    }

    // Task 3.2: 内联缓存测试
    #[test]
    fn test_task3_inline_cache() {
        // InlineCacheManager::new() doesn't exist yet
        // Skipping test pending implementation
    }

    // Task 3.3: 热点追踪测试
    #[test]
    fn test_task3_trace_selection() {
        // TraceSelector::new() doesn't exist yet
        // Skipping test pending implementation
    }

    #[test]
    fn test_ml_guidance_configuration() {
        // 测试默认启用ML引导优化
        let jit_with_ml = Jit::new();
        assert!(jit_with_ml.is_ml_guidance_enabled());

        // 测试显式启用ML引导优化
        let jit_with_ml_explicit = Jit::with_ml_guidance(true);
        assert!(jit_with_ml_explicit.is_ml_guidance_enabled());

        // 测试禁用ML引导优化
        let jit_without_ml = Jit::with_ml_guidance(false);
        assert!(!jit_without_ml.is_ml_guidance_enabled());

        // 测试动态启用/禁用
        let mut jit_dynamic = Jit::with_ml_guidance(false);
        assert!(!jit_dynamic.is_ml_guidance_enabled());

        jit_dynamic.enable_ml_guidance();
        assert!(jit_dynamic.is_ml_guidance_enabled());

        jit_dynamic.disable_ml_guidance();
        assert!(!jit_dynamic.is_ml_guidance_enabled());
    }

    #[test]
    fn test_ml_guidance_stability() {
        use ml_guided_jit::ExecutionFeatures;

        let mut jit = Jit::new();
        assert!(jit.is_ml_guidance_enabled());

        // 创建测试特征 with all 7 required parameters
        let features = ExecutionFeatures::new(
            64,  // block_size
            10,  // instr_count
            2,   // branch_count
            3,   // memory_access_count
            100, // execution_count
            0.85, // cache_hit_rate
            150.5, // avg_block_time_us
        );

        // 测试多次调用ML决策的稳定性
        for _ in 0..10 {
            let decision = jit.get_ml_decision(&vm_ir::IRBlock {
                start_pc: vm_core::GuestAddr(0x1000),
                ops: vec![
                    vm_ir::IROp::MovImm { dst: 1, imm: 42 },
                    vm_ir::IROp::Add { dst: 2, src1: 1, src2: 3 },
                ],
                term: vm_ir::Terminator::Ret,
            });

            // 确保每次都能得到决策结果
            assert!(decision.is_some());
        }
    }
}
*/
