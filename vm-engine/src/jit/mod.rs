//! # JIT编译器
//!
//! 提供即时编译(JIT)功能，将中间表示(IR)转换为本机机器码以提升执行性能。
//!
//! ## 主要组件
//!
//! - **编译器**: [`JITCompiler`] 主编译器，管理整个编译流程
//! - **后端**: [`JITBackend`] 定义JIT后端接口，支持多种代码生成后端
//! - **代码缓存**: [`code_cache`] 模块管理编译后的代码缓存
//! - **寄存器分配**: [`register_allocator`] 提供寄存器分配算法
//! - **优化器**: [`optimizer`] 和 [`branch_prediction`] 提供各种优化
//!
//! ## 架构
//!
//! JIT编译器采用多层架构：
//! 1. **前端**: 接收IR块
//! 2. **优化器**: 应用各种优化（常量折叠、死代码消除等）
//! 3. **寄存器分配**: 将虚拟寄存器映射到物理寄存器
//! 4. **代码生成**: 生成本机机器码
//! 5. **缓存**: 缓存编译结果以重用
//!
//! ## 示例
//!
//! ```rust,ignore
//! use vm_engine::jit::{JITCompiler, JITConfig};
//! use vm_ir::IRBlock;
//!
//! // 创建JIT编译器
//! let config = JITConfig::default();
//! let mut jit = JITCompiler::new(config);
//!
//! // 编译IR块
//! let block = IRBlock::new(vm_ir::GuestAddr(0x1000));
//! let compiled = jit.compile(&block)?;
//!
//! // 执行编译后的代码
//! let result = compiled.execute()?;
//! ```

pub mod backend;
mod compiler;
pub mod core; // 导出以支持JITEngine公共API（形成逻辑闭环）

// JIT 并行编译模块（工作窃取调度器）
pub mod parallel;

// 声明所有JIT子模块
pub mod branch_target_cache; // 导出以支持分支目标缓存公共API（形成逻辑闭环）
mod code_cache;
pub mod codegen; // 导出以支持代码生成公共API（形成逻辑闭环）
pub mod executable_memory;
pub mod instruction_scheduler; // 导出以支持指令调度公共API（形成逻辑闭环）
pub mod optimizer; // 导出以支持优化器公共API（形成逻辑闭环）
// 优化策略模块（基础设施层实现）
pub mod optimizer_strategy;
mod tiered_cache;
pub mod translation_optimizer;

// 缓存管理模块（基础设施层实现）
pub mod cache;

pub mod branch_prediction;
pub mod register_allocator; // 导出以支持寄存器分配公共API（形成逻辑闭环）
// 寄存器分配器适配器（基础设施层实现）
pub mod register_allocator_adapter;
pub mod tiered_translation_cache;
pub mod translation_cache;

pub use backend::cranelift::CraneliftBackend;
pub use backend::{
    BackendType, CompiledCode, JITBackend, JITBackendImpl, JITConfig, JITStats, OptLevel,
};
pub use compiler::JITCompiler;

/// 自适应阈值配置
///
/// 用于JIT编译器的自适应优化策略，定义热点代码检测阈值。
///
/// # 字段
///
/// - `hot_threshold`: 热点阈值，执行次数超过此值触发JIT编译
/// - `cold_threshold`: 冷点阈值，执行次数低于此值不进行优化
/// - `enable_adaptive`: 是否启用自适应优化
#[derive(Debug, Clone, Default)]
pub struct AdaptiveThresholdConfig {
    /// 热点阈值（执行次数）
    pub hot_threshold: u64,
    /// 冷点阈值（执行次数）
    pub cold_threshold: u64,
    /// 是否启用自适应优化
    pub enable_adaptive: bool,
}

/// 自适应阈值统计信息
///
/// 记录自适应优化策略的统计信息，用于性能分析和调优。
///
/// # 字段
///
/// - `hits`: 缓存命中次数
/// - `misses`: 缓存未命中次数
/// - `execution_count`: 累计执行次数
#[derive(Debug, Clone, Default)]
pub struct AdaptiveThresholdStats {
    /// 缓存命中次数
    pub hits: usize,
    /// 缓存未命中次数
    pub misses: usize,
    /// 累计执行次数
    pub execution_count: u64,
}

/// 代码指针类型
///
/// 表示编译后的机器码在内存中的地址。
pub type CodePtr = u64;

/// JIT编译器类型别名
///
/// [`JITCompiler`]的简短别名，用于简化代码。
pub type Jit = JITCompiler;
