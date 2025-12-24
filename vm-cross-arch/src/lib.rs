//! # vm-cross-arch - 跨架构指令转换层
//!
//! 实现不同架构之间等价转换层，支持AMD64、ARM64、RISC-V64架构指令之间的转换。
//!
//! ## 架构设计
//!
//! ```text
//! 源架构指令 (AMD64/ARM64/RISC-V64)
//!     ↓ (解码器)
//! 统一IR (vm-ir::IRBlock)
//!     ↓ (架构编码器)
//! 目标架构指令 (AMD64/ARM64/RISC-V64)
//! ```
//!
//! ## 核心功能
//!
//! - **指令解码**: 将源架构指令解码为统一中间表示(IR)
//! - **IR优化**: 对中间表示进行优化，提高转换效率
//! - **指令编码**: 将优化后的IR编码为目标架构指令
//! - **块级缓存**: 缓存已转换的代码块，避免重复转换
//! - **寄存器映射**: 智能映射源架构寄存器到目标架构寄存器
//! - **内存对齐优化**: 自动处理不同架构间的内存对齐差异
//! - **指令并行优化**: 识别和优化可并行执行的指令
//!
//! ## 性能优化
//!
//! - **块级翻译缓存**: 高效的翻译缓存机制，减少重复翻译开销
//! - **寄存器优化**: 智能寄存器分配算法，最小化寄存器拷贝
//! - **指令级并行**: 自动识别和优化可并行执行的指令
//! - **内存对齐优化**: 自动处理不同架构间的内存对齐和端序转换
//! - **IR优化**: 常量折叠、死代码消除、公共子表达式消除等优化技术
//!
//! ## 使用示例
//!
//! ### 基本使用
//!
//! ```rust
//! use vm_cross_arch::{ArchTranslator, SourceArch, TargetArch};
//! use vm_ir::IRBlock;
//!
//! // 创建转换器
//! let translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
//!
//! // 转换IR块
//! let source_block: IRBlock = ...;
//! let target_instructions = translator.translate_block(&source_block)?;
//! ```
//!
//! ### 高级配置
//!
//! ```rust
//! use vm_cross_arch::{ArchTranslator, SourceArch, TargetArch, TranslationConfig};
//! use vm_ir::IRBlock;
//!
//! // 创建转换配置
//! let config = TranslationConfig::new()
//!     .with_optimization_level(3)
//!     .with_enable_instruction_parallelism(true)
//!     .with_enable_memory_alignment_optimization(true)
//!     .with_enable_register_optimization(true)
//!     .with_cache_size(64 * 1024 * 1024); // 64MB缓存
//!
//! // 创建转换器
//! let translator = ArchTranslator::with_config(
//!     SourceArch::X86_64, 
//!     TargetArch::ARM64, 
//!     config
//! );
//!
//! // 转换IR块
//! let source_block: IRBlock = ...;
//! let target_instructions = translator.translate_block(&source_block)?;
//! ```
//!
//! ### 性能监控
//!
//! ```rust
//! use vm_cross_arch::{ArchTranslator, SourceArch, TargetArch};
//!
//! let translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
//!
//! // 执行一些转换...
//!
//! // 获取性能统计
//! let stats = translator.get_performance_stats();
//! println!("转换的代码块数: {}", stats.translated_blocks);
//! println!("缓存命中率: {:.2}%", stats.cache_hit_rate * 100.0);
//! println!("平均转换时间: {:?}", stats.avg_translation_time);
//! println!("指令并行优化数: {}", stats.parallel_optimizations);
//! ```
//!
//! ## 支持的转换
//!
//! - AMD64 ↔ ARM64
//! - AMD64 ↔ RISC-V64
//! - ARM64 ↔ RISC-V64
//!
//! ## 架构特性支持
//!
//! ### x86-64
//! - 基础指令集: 完整支持
//! - SIMD指令: SSE, AVX, AVX2, AVX-512
//! - 系统指令: SYSCALL, SYSRET
//! - 虚拟化指令: VMX, SVM
//!
//! ### ARM64
//! - 基础指令集: A64
//! - SIMD指令: NEON, SVE
//! - 系统指令: SVC, HVC, SMC
//! - 虚拟化指令: HVC
//!
//! ### RISC-V64
//! - 基础指令集: RV64I
//! - 扩展指令集: M, A, F, D, C
//! - SIMD指令: V (向量扩展)
//! - 系统指令: ECALL, EBREAK

mod auto_executor;
mod block_cache;
mod refactored_encoder;
mod cache_optimizer;
mod cross_arch_aot;
mod cross_arch_runtime;
mod encoder;
mod instruction_parallelism;
mod instruction_patterns;
mod integration;
mod ir_optimizer;
mod memory_alignment_optimizer;
mod optimized_register_allocator;
mod os_support;
mod performance_optimizer;
mod register_mapping;
mod runtime;
mod smart_register_allocator;
mod target_specific_optimizer;
mod adaptive_optimizer;
mod translator;
mod types;
mod translation_impl;
mod unified_executor;
mod vm_service_ext;

pub use auto_executor::{AutoExecutor, UnifiedDecoder};
pub use block_cache::{CacheReplacementPolicy, CrossArchBlockCache, SourceBlockKey, TranslatedBlock};
pub use cache_optimizer::{CacheConfig, CacheOptimizer, CachePolicy};
pub use cross_arch_aot::{CrossArchAotCompiler, CrossArchAotConfig, CrossArchAotStats};
pub use instruction_parallelism::{
    DependencyType, DependencyEdge, InstructionNode, ParallelGroup, ResourceRequirements,
    ParallelismStats, InstructionParallelizer
};
pub use memory_alignment_optimizer::{
    AlignmentInfo, Endianness, EndiannessConversionStrategy, MemoryAccessPattern,
    MemoryAlignmentOptimizer, MemoryOptimizationStats as OptimizationStats, IROpExt
};
pub use ir_optimizer::{
    IROptimizer, OptimizationStats as IROptimizationStats, SubExpression, BinaryOp, UnaryOp, Operand
};
pub use target_specific_optimizer::{
    TargetSpecificOptimizer, OptimizationStats as TargetOptimizationStats
};
pub use adaptive_optimizer::{
    AdaptiveOptimizer, OptimizationStats as AdaptiveOptimizationStats, HotspotDetector, Hotspot,
    PerformanceProfiler, PerformanceData, PerformanceTrend, ProfilingConfig, ProfilingSession,
    PerformanceSample, TieredCompiler, CompilationTier, TieredCompilationStrategy,
    TierTriggerCondition, CompilationRecord, DynamicRecompiler, RecompilationStrategy,
    RecompilationRecord, CachedCode
};
pub use optimized_register_allocator::{OptimizedRegisterMapper, RegisterLifetime, RegisterCopy, TempRegisterUsage};
pub use cross_arch_runtime::{
    AotIntegrationConfig, CrossArchRuntime, CrossArchRuntimeConfig, GcIntegrationConfig,
    JitIntegrationConfig,
};
pub use encoder::{ArchEncoder, Arm64Encoder, Riscv64Encoder, X86_64Encoder};
pub use integration::{CrossArchVm, CrossArchVmBuilder};
pub use os_support::{
    DeviceEmulator, DeviceType, InterruptController, LinuxSyscallHandler, SyscallHandler,
};
pub use performance_optimizer::{PerformanceConfig, PerformanceOptimizer};
pub use register_mapping::{RegisterMapper, RegisterMapping};
pub use smart_register_allocator::{
    CallingConvention, InterferenceNode, RegisterAllocationStats, RegisterClass, RegisterInfo,
    SmartRegisterMapper,
};
pub use runtime::{CrossArchConfig, CrossArchStrategy, HostArch};
pub use translator::{ArchTranslator};
pub use types::{SourceArch, TargetArch, TranslationError};
pub use translation_impl::{TargetInstruction, TranslationResult, TranslationStats};
pub use unified_executor::{ExecutionStats, UnifiedExecutor};
pub use vm_service_ext::{VmConfigExt, create_auto_vm_config};

pub use vm_encoder::Architecture;
