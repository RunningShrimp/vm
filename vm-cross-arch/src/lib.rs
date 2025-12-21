//! # vm-cross-arch - 跨架构指令转换层
//!
//! 实现不同架构之间等价转换层，支持AMD64、ARM64、RISC-V64架构指令之间的转换。
//!
//! ## 快速开始
//!
//! 推荐使用 [`UnifiedExecutor`] 作为统一执行入口：
//!
//! ```rust,ignore
//! use vm_cross_arch::UnifiedExecutor;
//! use vm_core::{GuestArch, GuestAddr};
//!
//! // 创建统一执行器（自动选择最佳执行策略：AOT > JIT > 解释器）
//! let mut executor = UnifiedExecutor::auto_create(GuestArch::X86_64, 64 * 1024 * 1024)?;
//!
//! // 执行代码
//! let result = executor.execute(GuestAddr(0x1000))?;
//! ```
//!
//! ## 执行器说明
//!
//! - **[`UnifiedExecutor`]** (推荐): 统一执行入口，自动选择 AOT > JIT > 解释器
//! - [`AutoExecutor`]: 自动架构检测执行器，用于更细粒度的控制
//! - [`CrossArchRuntime`]: 底层跨架构运行时，供高级用户使用
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
//! ## OS级执行支持状态
//!
//! **⚠️ 当前状态（阶段1）**: 本项目当前**不支持**完整的OS级跨架构执行。
//!
//! ### 当前能力
//! - ✅ 跨架构指令翻译（AMD64 ↔ ARM64 ↔ RISC-V64）
//! - ✅ 基本块级执行（解释器模式）
//! - ✅ 最小系统调用支持（write/exit，仅用于测试）
//!
//! ### 未实现的关键能力（阶段2目标）
//! - ❌ **OS引导链**: 镜像加载、页表初始化、特权态设置
//! - ❌ **异常/中断处理**: 异常向量表、中断注入、异常返回
//! - ❌ **设备模型**: 最小设备集（console/timer/block/net）
//! - ❌ **完整系统调用**: 文件I/O、进程管理、内存管理等
//!
//! 详见 [`os_support`] 模块文档了解详细的能力清单和实现计划。
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

mod adaptive_optimizer;
mod auto_executor;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
mod hardware_accel_engine;
mod block_cache;
mod cache_optimizer;
mod direct_execution;
mod enhanced_block_cache;
mod hot_path_optimizer;
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
mod syscall_compat;
mod signal_compat;
mod filesystem_compat;
mod network_compat;
mod memory_model;
mod interrupt_handler;
mod performance_optimizer;
mod refactored_encoder;
mod register_mapping;
mod runtime;
mod smart_register_allocator;
mod target_specific_optimizer;
mod translator;
#[cfg(test)]
mod translation_coverage_tests;
#[cfg(test)]
mod riscv_vector_encoder_tests;
mod consistency_tests;
mod unified_executor;
mod vm_service_ext;

pub use adaptive_optimizer::{
    AdaptiveOptimizer, CachedCode, CompilationRecord, CompilationTier, DynamicRecompiler, Hotspot,
    HotspotDetector, OptimizationStats as AdaptiveOptimizationStats, PerformanceData,
    PerformanceProfiler, PerformanceSample, PerformanceTrend, ProfilingConfig, ProfilingSession,
    RecompilationRecord, RecompilationStrategy, TierTriggerCondition, TieredCompilationStrategy,
    TieredCompiler,
};
pub use auto_executor::{AutoExecutor, UnifiedDecoder};
pub use block_cache::{
    CacheReplacementPolicy, CrossArchBlockCache, SourceBlockKey, TranslatedBlock,
};
pub use cache_optimizer::{CacheConfig, CacheOptimizer, CachePolicy};
pub use direct_execution::DirectExecutionOptimizer;
pub use enhanced_block_cache::{EnhancedBlockCache, EnhancedReplacementPolicy};
pub use hot_path_optimizer::{HotPathOptimizer, HotPathStats};
pub use direct_execution::DirectExecutionOptimizer;
pub use cross_arch_aot::{CrossArchAotCompiler, CrossArchAotConfig, CrossArchAotStats};
pub use cross_arch_runtime::{
    AotIntegrationConfig, CrossArchRuntime, CrossArchRuntimeConfig, GcIntegrationConfig,
    JitIntegrationConfig,
};
pub use encoder::{ArchEncoder, Arm64Encoder, Riscv64Encoder, X86_64Encoder};
pub use instruction_parallelism::{
    DependencyEdge, DependencyType, InstructionNode, InstructionParallelizer, ParallelGroup,
    ParallelismStats, ResourceRequirements,
};
pub use integration::{CrossArchVm, CrossArchVmBuilder};
pub use ir_optimizer::{
    BinaryOp, IROptimizer, Operand, OptimizationStats as IROptimizationStats, SubExpression,
    UnaryOp,
};
pub use memory_alignment_optimizer::{
    AlignmentInfo, Endianness, EndiannessConversionStrategy, IROpExt, MemoryAccessPattern,
    MemoryAlignmentOptimizer, MemoryOptimizationStats as OptimizationStats,
};
pub use optimized_register_allocator::{
    OptimizedRegisterMapper, RegisterCopy, RegisterLifetime, TempRegisterUsage,
};
pub use os_support::{
    DeviceEmulator, DeviceType, InterruptController, LinuxSyscallHandler, SyscallHandler,
};
pub use signal_compat::{
    Signal, SignalAction, SignalCompatibilityLayer,
};
pub use filesystem_compat::{
    FileInfo, FileMode, FilesystemCompatibilityLayer, FilesystemOperations, OpenFlags,
};
pub use network_compat::{
    NetworkCompatibilityLayer, NetworkStackOperations, SocketAddress, SocketDomain,
    SocketInfo, SocketOption, SocketOptionLevel, SocketProtocol, SocketType,
};
pub use syscall_compat::{
    SyscallArgConverter, SyscallCompatibilityLayer, SyscallNumberMapper,
};
pub use performance_optimizer::{PerformanceConfig, PerformanceOptimizer};
pub use register_mapping::{RegisterMapper, RegisterMapping};
pub use runtime::{CrossArchConfig, CrossArchStrategy, HostArch};
pub use smart_register_allocator::{
    CallingConvention, InterferenceNode, RegisterAllocationStats, RegisterClass, RegisterInfo,
    SmartRegisterMapper,
};
pub use target_specific_optimizer::{
    OptimizationStats as TargetOptimizationStats, TargetSpecificOptimizer,
};
pub use translator::{ArchTranslator, SourceArch, TargetArch, TranslationError};
pub use unified_executor::{ExecutionStats, UnifiedExecutor};
pub use vm_service_ext::{VmConfigExt, create_auto_vm_config};

// use vm_core::VmError;
// use vm_ir::{IRBlock, IROp};

/// 架构类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Architecture {
    /// x86-64 (AMD64/Intel64)
    X86_64,
    /// ARM64 (AArch64)
    ARM64,
    /// RISC-V 64-bit
    RISCV64,
}

impl std::fmt::Display for Architecture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Architecture::X86_64 => write!(f, "x86_64"),
            Architecture::ARM64 => write!(f, "arm64"),
            Architecture::RISCV64 => write!(f, "riscv64"),
        }
    }
}

impl std::error::Error for Architecture {}

impl Architecture {
    /// 获取架构的寄存器数量
    pub fn register_count(&self) -> usize {
        match self {
            Architecture::X86_64 => 16,  // 通用寄存器
            Architecture::ARM64 => 32,   // X0-X30 + SP
            Architecture::RISCV64 => 32, // X0-X31
        }
    }

    /// 获取架构的指针大小（字节）
    pub fn pointer_size(&self) -> usize {
        8
    }

    /// 获取架构的指令对齐（字节）
    pub fn instruction_alignment(&self) -> usize {
        match self {
            Architecture::X86_64 => 1,  // x86-64指令可变长
            Architecture::ARM64 => 4,   // ARM64指令固定4字节
            Architecture::RISCV64 => 2, // RISC-V支持16位压缩指令
        }
    }
}

/// 目标架构指令表示
#[derive(Debug, Clone)]
pub struct TargetInstruction {
    /// 指令字节码
    pub bytes: Vec<u8>,
    /// 指令长度（字节）
    pub length: usize,
    /// 指令助记符（用于调试）
    pub mnemonic: String,
    /// 是否影响控制流
    pub is_control_flow: bool,
    /// 是否访问内存
    pub is_memory_op: bool,
}

/// 转换结果
#[derive(Debug, Clone)]
pub struct TranslationResult {
    /// 转换后的指令序列
    pub instructions: Vec<TargetInstruction>,
    /// 转换统计信息
    pub stats: TranslationStats,
}

/// 转换统计信息
#[derive(Debug, Clone, Default)]
pub struct TranslationStats {
    /// 转换的IR操作数
    pub ir_ops_translated: usize,
    /// 生成的目标指令数
    pub target_instructions_generated: usize,
    /// 需要多指令序列的复杂操作数
    pub complex_operations: usize,
    /// 寄存器映射次数
    pub register_mappings: usize,
    /// 消除的寄存器拷贝数
    pub copies_eliminated: usize,
    /// 寄存器重用次数
    pub registers_reused: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_architecture_properties() {
        assert_eq!(Architecture::X86_64.register_count(), 16);
        assert_eq!(Architecture::ARM64.register_count(), 32);
        assert_eq!(Architecture::RISCV64.register_count(), 32);
    }
}

#[cfg(test)]
mod integration_tests;
#[cfg(test)]
mod os_compat_tests;
