//! # vm-engine - 虚拟机执行引擎
//!
//! 提供虚拟机的执行引擎实现，包括解释器、JIT编译器和异步执行框架。
//!
//! ## 主要组件
//!
//! - **JIT编译**: [`JITCompiler`] 提供即时编译功能，将IR转换为本机机器码
//! - **解释器**: [`interpreter`] 模块提供解释执行引擎
//! - **异步执行**: [`executor`] 模块提供异步执行和协程调度
//! - **执行上下文**: [`AsyncExecutionContext`] 异步执行上下文
//! - **执行器类型**: [`ExecutorType`] 枚举支持多种执行模式
//!
//! ## 特性标志
//!
//! - `async`: 启用异步执行和分布式虚拟机支持
//! - `jit-full`: 启用完整JIT引擎，包含vm-engine-jit的高级功能
//!
//! ## 示例
//!
//! ### 使用JIT编译器
//!
//! ```rust,ignore
//! use vm_engine::{JITCompiler, JITConfig};
//! use vm_ir::IRBlock;
//!
//! let config = JITConfig::default();
//! let mut jit = JITCompiler::new(config);
//!
//! let block = IRBlock::new(vm_ir::GuestAddr(0x1000));
//! let compiled_code = jit.compile(&block)?;
//! ```
//!
//! ### 使用异步执行器
//!
//! ```rust,ignore
//! use vm_engine::{ExecutorType, AsyncExecutionContext};
//!
//! let ctx = AsyncExecutionContext::new(ExecutorType::JIT);
//! let result = ctx.execute_block(block_id).await?;
//! ```

pub mod executor;
pub mod interpreter;
pub mod jit;

// 重新导出常用类型
pub use executor::{AsyncExecutionContext, ExecutorType};
pub use jit::{JITCompiler, JITConfig};

// 当启用jit-full feature时，重新导出vm-engine-jit的高级功能
#[cfg(feature = "jit-full")]
pub use vm_engine_jit::{
    // 核心JIT编译器
    Jit,
    JitContext,
    // 性能分析
    adaptive_optimizer::{AdaptiveOptimizer, AdaptiveParameters},
    // AOT相关
    aot_cache::AotCache,
    aot_format::AotFormat,
    aot_loader::AotLoader,
    // 优化passes
    block_chaining::{BlockChain, BlockChainer},
    // 编译缓存
    compile_cache::CompileCache,
    ewma_hotspot::EwmaHotspotDetector,
    inline_cache::InlineCache,
    loop_opt::LoopOptimizer,
    // ML引导的JIT
    ml_model::MLModel,
    // 分层编译
    tiered_compiler::TieredCompiler,
    // GC相关
    unified_gc::UnifiedGC,
    // 厂商优化
    vendor_optimizations::{CpuFeature, CpuVendor, VendorOptimizationStrategy, VendorOptimizer},
};
