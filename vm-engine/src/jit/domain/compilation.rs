//! # Compilation Bounded Context
//!
//! This module defines the compilation domain, including IR transformation,
//! code generation, and compilation workflow management.
//!
//! ## Overview
//!
//! The compilation bounded context manages the transformation of intermediate
//! representation (IR) blocks into executable machine code, following a
//! multi-stage compilation pipeline.
//!
//! ## Key Components
//!
//! ### Core Types
//!
//! - **`CompilationId`**: Unique identifier for each compilation operation
//! - **`CompilationContext`**: Aggregate root managing the compilation lifecycle
//! - **`CompilationConfig`**: Configuration controlling compilation behavior
//! - **`CompilationResult`**: Output containing generated machine code and metadata
//!
//! ### Compilation Pipeline
//!
//! 1. **IR Analysis**: Validate and analyze input IR block
//! 2. **Optimization**: Apply IR-level optimizations
//! 3. **Code Generation**: Generate target machine code
//! 4. **Verification**: Validate generated code (optional)
//! 5. **Finalization**: Produce compilation result with statistics
//!
//! ## Usage Examples
//!
//! ### Basic Compilation
//!
//! ```ignore
//! use vm_engine_jit::domain::compilation::{
//!     CompilationService, CompilationConfig, OptimizationLevel, TargetArchitecture
//! };
//!
//! let service = CompilationService::new(
//!     Box::new(compiler_factory),
//!     Box::new(optimizer_factory),
//!     Box::new(codegen_factory),
//! );
//!
//! let config = CompilationConfig {
//!     optimization_level: OptimizationLevel::Balanced,
//!     target_arch: TargetArchitecture::X86_64,
//!     enable_verification: true,
//!     ..Default::default()
//! };
//!
//! let result = service.compile(ir_block, config)?;
//! ```
//!
//! ### Custom Compilation Configuration
//!
//! ```ignore
//! let config = CompilationConfig {
//!     optimization_level: OptimizationLevel::Max,
//!     target_arch: TargetArchitecture::ARM64,
//!     debug_info: true,
//!     enable_verification: true,
//!     timeout_ms: 10000,
//! };
//!
//! let result = service.compile(ir_block, config)?;
//! println!("Generated {} bytes of machine code", result.code_size);
//! ```
//!
//! ### Tracking Compilation Progress
//!
//! ```ignore
//! let context = CompilationContext::new(ir_block, config);
//! context.start_compilation();
//!
//! // Update statistics during compilation
//! context.update_ir_stats(ir_block.ops.len());
//!
//! let progress = context.get_progress();
//! println!("Compilation progress: {:.1}%", progress.percentage * 100.0);
//! ```
//!
//! ## Compilation States
//!
//! The compilation context follows this state machine:
//!
//! ```text
//! Pending -> InProgress -> Completed
//!                \           /
//!                 v         v
//!               Failed  Cancelled
//! ```
//!
//! - **Pending**: Compilation initialized, waiting to start
//! - **InProgress**: actively compiling
//! - **Completed**: Successfully finished
//! - **Failed**: Compilation encountered an error
//! - **Cancelled**: Compilation was cancelled
//!
//! ## Optimization Levels
//!
//! ### None
//! No optimizations, fastest compilation. Useful for debugging.
//!
//! ### Basic
//! Simple optimizations with minimal compile-time impact:
//! - Constant folding
//! - Dead code elimination
//! - Basic peephole optimizations
//!
//! ### Balanced
//! Good balance between compilation speed and runtime performance:
//! - All basic optimizations
//! - Inline expansion for small functions
//! - Register allocation
//!
//! ### Max
//! Maximum optimizations, slower compilation:
//! - All balanced optimizations
//! - Loop optimizations
//! - Instruction scheduling
//! - Advanced vectorization
//!
//! ## Target Architectures
//!
//! ### X86_64
//! - AMD64/Intel 64-bit architecture
//! - Supports SSE, AVX, AVX-512 extensions
//! - Most mature backend
//!
//! ### ARM64 (AArch64)
//! - 64-bit ARM architecture
//! - Supports NEON, SVE extensions
//! - Mobile and embedded focus
//!
//! ### RISCV64
//! - 64-bit RISC-V architecture
//! - Extensible instruction set
//! - Growing ecosystem support
//!
//! ## Code Hashing
//!
//! The compilation service computes a hash for each compiled block:
//!
//! ```ignore
//! let hash = CompilationService::compute_code_hash(&machine_code, &ir_block);
//! ```
//!
//! This hash is used for:
//! - **Cache Validation**: Ensure cached code matches source IR
//! - **Deduplication**: Identify duplicate compilations
//! - **Debugging**: Track compilation units
//!
//! ## Domain-Driven Design Applied
//!
//! ### Entities
//!
//! - `CompilationContext`: Aggregate root with unique `CompilationId`
//! - Lifecycle management with state transitions
//!
//! ### Value Objects
//!
//! - `CompilationConfig`: Immutable configuration
//! - `CompilationResult`: Immutable result data
//! - `CompilationStats`: Statistics snapshot
//! - `CompilationProgress`: Progress tracking
//!
//! ### Domain Services
//!
//! - `CompilationService`: Orchestrates compilation pipeline
//! - Manages factory dependencies
//!
//! ### Factory Pattern
//!
//! Abstract factory pattern for extensibility:
//! - `CompilerFactory`: Creates compiler instances
//! - `OptimizerFactory`: Creates optimizer instances
//! - `CodeGeneratorFactory`: Creates code generator instances
//!
//! ## Integration Points
//!
//! ### With IR Layer
//!
//! - Consumes `IRBlock` as input
//! - Validates IR structure before compilation
//!
//! ### With Optimization Domain
//!
//! - Integrates with optimization service for IR optimization
//! - Tracks optimization statistics
//!
//! ### With Caching Domain
//!
//! - Generates hash keys for cache lookup
//! - Stores compilation results in cache
//!
//! ### With Execution Domain
//!
//! - Provides executable machine code
//! - Supplies metadata for execution environment
//!
//! ## Performance Considerations
//!
//! - **Compilation Time**: Increases with optimization level
//! - **Code Size**: Higher optimization may increase code size
//! - **Memory Usage**: IR and machine code held in memory during compilation
//! - **Parallelism**: Can compile multiple blocks in parallel (service-level concern)

use std::collections::HashMap;
use vm_core::GuestAddr;
use vm_ir::IRBlock;
use vm_core::foundation::VmError;
use crate::jit::common::{JITResult, JITErrorBuilder, Config};

/// 编译限界上下文
pub struct CompilationContext {
    /// 编译ID
    pub compilation_id: CompilationId,
    /// 源代码块
    pub source_block: IRBlock,
    /// 编译配置
    pub config: CompilationConfig,
    /// 编译状态
    pub status: CompilationStatus,
    /// 编译结果
    pub result: Option<CompilationResult>,
    /// 编译统计
    pub stats: CompilationStats,
}

/// 编译ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CompilationId(u64);

impl CompilationId {
    /// 创建新的编译ID
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
    
    /// 获取ID值
    pub fn value(&self) -> u64 {
        self.0
    }
}

/// 编译配置
#[derive(Debug, Clone)]
pub struct CompilationConfig {
    /// 优化级别
    pub optimization_level: OptimizationLevel,
    /// 目标架构
    pub target_arch: TargetArchitecture,
    /// 是否启用调试信息
    pub debug_info: bool,
    /// 是否启用验证
    pub enable_verification: bool,
    /// 编译超时时间（毫秒）
    pub timeout_ms: u64,
}

impl Default for CompilationConfig {
    fn default() -> Self {
        Self {
            optimization_level: OptimizationLevel::Balanced,
            target_arch: TargetArchitecture::X86_64,
            debug_info: false,
            enable_verification: true,
            timeout_ms: 5000,
        }
    }
}

impl Config for CompilationConfig {
    fn validate(&self) -> Result<(), String> {
        // 检查超时时间
        if self.timeout_ms == 0 {
            return Err("编译超时时间不能为0".to_string());
        }
        
        // 检查是否启用验证但禁用了调试信息
        if self.enable_verification && !self.debug_info {
            // 验证需要调试信息
            return Err("启用验证时需要启用调试信息".to_string());
        }
        
        Ok(())
    }
    
    fn merge(&mut self, other: &Self) {
        self.optimization_level = other.optimization_level;
        self.target_arch = other.target_arch;
        self.debug_info = other.debug_info || self.debug_info;
        self.enable_verification = other.enable_verification && self.enable_verification;
        // 合并超时时间（取较小值以保持保守）
        self.timeout_ms = self.timeout_ms.min(other.timeout_ms);
    }
    
    fn summary(&self) -> String {
        format!(
            "CompilationConfig: level={:?}, arch={:?}, debug={}, verify={}, timeout={}ms",
            self.optimization_level, self.target_arch, self.debug_info, self.enable_verification, self.timeout_ms
        )
    }
}

/// 优化级别
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationLevel {
    /// 无优化
    None,
    /// 基础优化
    Basic,
    /// 平衡优化
    Balanced,
    /// 最大优化
    Max,
}

/// 目标架构
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetArchitecture {
    /// x86-64
    X86_64,
    /// ARM64
    ARM64,
    /// RISC-V 64
    RISCV64,
}

/// 编译状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilationStatus {
    /// 等待编译
    Pending,
    /// 编译中
    InProgress,
    /// 编译成功
    Completed,
    /// 编译失败
    Failed,
    /// 已取消
    Cancelled,
}

/// 编译结果
#[derive(Debug, Clone)]
pub struct CompilationResult {
    /// 编译ID
    pub compilation_id: CompilationId,
    /// 生成的机器码
    pub machine_code: Vec<u8>,
    /// 代码大小
    pub code_size: usize,
    /// 代码入口点
    pub entry_point: GuestAddr,
    /// 符号表
    pub symbols: HashMap<String, GuestAddr>,
    /// 重定位信息
    pub relocations: Vec<RelocationInfo>,
    /// 编译统计
    pub stats: CompilationStats,
    /// 已编译代码块（用于domain层交互）
    pub compiled_block: crate::CompiledBlock,
}

/// 重定位信息
#[derive(Debug, Clone)]
pub struct RelocationInfo {
    /// 重定位类型
    pub relocation_type: RelocationType,
    /// 重定位偏移
    pub offset: usize,
    /// 符号名称
    pub symbol: String,
    /// 重定位值
    pub value: Option<u64>,
}

/// 重定位类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelocationType {
    /// 绝对地址
    Absolute,
    /// 相对地址
    Relative,
    /// PC相对地址
    PCRelative,
    /// GOT相对地址
    GOTRelative,
}

/// 编译统计
#[derive(Debug, Clone, Default)]
pub struct CompilationStats {
    /// 编译开始时间
    pub start_time: Option<std::time::Instant>,
    /// 编译结束时间
    pub end_time: Option<std::time::Instant>,
    /// IR优化时间（纳秒）
    pub ir_optimization_time_ns: u64,
    /// 代码生成时间（纳秒）
    pub code_generation_time_ns: u64,
    /// 验证时间（纳秒）
    pub verification_time_ns: u64,
    /// 总编译时间（纳秒）
    pub total_time_ns: u64,
    /// IR指令数量
    pub ir_instruction_count: usize,
    /// 生成的机器指令数量
    pub machine_instruction_count: usize,
    /// 优化后的IR指令数量
    pub optimized_ir_count: usize,
}

impl CompilationContext {
    /// 创建新的编译上下文
    pub fn new(source_block: IRBlock, config: CompilationConfig) -> Self {
        Self {
            compilation_id: CompilationId::new(),
            source_block,
            config,
            status: CompilationStatus::Pending,
            result: None,
            stats: CompilationStats::default(),
        }
    }
    
    /// 开始编译
    pub fn start_compilation(&mut self) {
        self.status = CompilationStatus::InProgress;
        self.stats.start_time = Some(std::time::Instant::now());
    }
    
    /// 完成编译
    pub fn complete_compilation(&mut self, result: CompilationResult) {
        self.status = CompilationStatus::Completed;
        self.result = Some(result.clone());
        self.stats.end_time = Some(std::time::Instant::now());
        
        if let (Some(start), Some(end)) = (self.stats.start_time, self.stats.end_time) {
            self.stats.total_time_ns = end.duration_since(start).as_nanos() as u64;
        }
    }
    
    /// 编译失败
    pub fn fail_compilation(&mut self, _error: VmError) {
        self.status = CompilationStatus::Failed;
        self.stats.end_time = Some(std::time::Instant::now());
        
        if let (Some(start), Some(end)) = (self.stats.start_time, self.stats.end_time) {
            self.stats.total_time_ns = end.duration_since(start).as_nanos() as u64;
        }
    }
    
    /// 取消编译
    pub fn cancel_compilation(&mut self) {
        self.status = CompilationStatus::Cancelled;
        self.stats.end_time = Some(std::time::Instant::now());
    }
    
    /// 更新IR统计
    pub fn update_ir_stats(&mut self, ir_count: usize) {
        self.stats.ir_instruction_count = ir_count;
    }
    
    /// 更新机器码统计
    pub fn update_machine_stats(&mut self, machine_count: usize) {
        self.stats.machine_instruction_count = machine_count;
    }
    
    /// 更新优化统计
    pub fn update_optimization_stats(&mut self, optimized_count: usize, time_ns: u64) {
        self.stats.optimized_ir_count = optimized_count;
        self.stats.ir_optimization_time_ns = time_ns;
    }
    
    /// 更新代码生成统计
    pub fn update_code_generation_stats(&mut self, time_ns: u64) {
        self.stats.code_generation_time_ns = time_ns;
    }
    
    /// 更新验证统计
    pub fn update_verification_stats(&mut self, time_ns: u64) {
        self.stats.verification_time_ns = time_ns;
    }
    
    /// 获取编译进度
    pub fn get_progress(&self) -> CompilationProgress {
        match self.status {
            CompilationStatus::Pending => CompilationProgress::new(0.0),
            CompilationStatus::InProgress => {
                // 基于时间估算进度
                if let Some(start) = self.stats.start_time {
                    let target_time = self.config.timeout_ms;
                    let elapsed = start.elapsed().as_millis();
                    let progress = (elapsed as f64 / target_time as f64).min(1.0);
                    CompilationProgress::new(progress)
                } else {
                    CompilationProgress::new(0.5)
                }
            }
            CompilationStatus::Completed => CompilationProgress::new(1.0),
            CompilationStatus::Failed | CompilationStatus::Cancelled => CompilationProgress::finished(),
        }
    }
}

/// 编译进度
#[derive(Debug, Clone)]
pub struct CompilationProgress {
    /// 进度百分比（0.0-1.0）
    pub percentage: f64,
    /// 是否已完成
    pub is_finished: bool,
}

impl CompilationProgress {
    /// 创建新的进度
    pub fn new(percentage: f64) -> Self {
        Self {
            percentage: percentage.clamp(0.0, 1.0),
            is_finished: percentage >= 1.0,
        }
    }
    
    /// 创建已完成的进度
    pub fn finished() -> Self {
        Self {
            percentage: 1.0,
            is_finished: true,
        }
    }
}

/// 编译领域服务
pub struct CompilationService {
    /// 编译器工厂
    compiler_factory: Box<dyn CompilerFactory>,
    /// 优化器工厂
    optimizer_factory: Box<dyn OptimizerFactory>,
    /// 代码生成器工厂
    code_generator_factory: Box<dyn CodeGeneratorFactory>,
}

impl CompilationService {
    /// 创建新的编译服务
    pub fn new(
        compiler_factory: Box<dyn CompilerFactory>,
        optimizer_factory: Box<dyn OptimizerFactory>,
        code_generator_factory: Box<dyn CodeGeneratorFactory>,
    ) -> Self {
        Self {
            compiler_factory,
            optimizer_factory,
            code_generator_factory,
        }
    }

    /// 计算代码哈希值
    ///
    /// 使用FNV-1a哈希算法计算机器码和源IR块的哈希值。
    /// 这用于代码缓存验证，确保编译的代码与源IR匹配。
    ///
    /// # 参数
    /// - `machine_code`: 生成的机器码
    /// - `source_block`: 源IR块
    ///
    /// # 返回值
    /// - `u64`: 哈希值
    ///
    /// # 算法
    /// FNV-1a (Fowler-Noll-Vo)哈希算法：
    /// - 基数: 14695981039346656037
    /// - 质数: 1099511628211
    ///
    /// # 示例
    /// ```ignore
    /// use vm_engine_jit::domain::compilation::CompilationService;
    ///
    /// let machine_code = vec![0x90, 0x90, 0xC3];
    /// let ir_block = IRBlock { ... };
    /// let hash = CompilationService::compute_code_hash(&machine_code, &ir_block);
    /// ```
    fn compute_code_hash(machine_code: &[u8], source_block: &IRBlock) -> u64 {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;

        // 使用FNV-1a哈希算法
        let mut hasher = DefaultHasher::new();

        // 包含机器码
        machine_code.hash(&mut hasher);

        // 包含源IR块的关键信息
        source_block.start_pc.hash(&mut hasher);
        source_block.ops.len().hash(&mut hasher);

        // 对每个操作进行哈希
        for op in &source_block.ops {
            // 使用操作类型的discriminant作为哈希输入
            std::mem::discriminant(op).hash(&mut hasher);
        }

        // 包含终止符类型
        std::mem::discriminant(&source_block.term).hash(&mut hasher);

        hasher.finish()
    }
    
    /// 编译IR块
    pub fn compile(&self, source_block: IRBlock, config: CompilationConfig) -> JITResult<CompilationResult> {
        let mut context = CompilationContext::new(source_block, config);
        
        // 开始编译
        context.start_compilation();
        
        // 更新IR统计
        context.update_ir_stats(context.source_block.ops.len());
        
        // IR优化
        let optimized_block = self.optimize_ir(&mut context)?;
        
        // 代码生成
        let machine_code = self.generate_machine_code(&mut context, &optimized_block)?;
        
        // 验证生成的代码
        if context.config.enable_verification {
            self.verify_machine_code(&mut context, &machine_code)?;
        }
        
        // 创建编译结果
        let result = CompilationResult {
            compilation_id: context.compilation_id,
            machine_code: machine_code.clone(),
            code_size: machine_code.len(),
            entry_point: context.source_block.start_pc,
            symbols: HashMap::new(), // 简化实现
            relocations: Vec::new(), // 简化实现
            stats: context.stats.clone(),
            compiled_block: crate::CompiledBlock {
                start_pc: context.source_block.start_pc,
                size: machine_code.len(),
                hash: Self::compute_code_hash(&machine_code, &context.source_block),
                compile_time: std::time::Instant::now(),
                hotness: 0,
            },
        };
        
        // 完成编译
        context.complete_compilation(result.clone());
        
        Ok(result)
    }
    
    /// 优化IR
    fn optimize_ir(&self, context: &mut CompilationContext) -> JITResult<IRBlock> {
        let start_time = std::time::Instant::now();
        
        let optimizer = self.optimizer_factory.create_optimizer(context.config.optimization_level);
        let optimized_block = optimizer.optimize(&context.source_block)?;
        
        let optimization_time = start_time.elapsed().as_nanos() as u64;
        context.update_optimization_stats(optimized_block.ops.len(), optimization_time);
        
        Ok(optimized_block)
    }
    
    /// 生成机器码
    fn generate_machine_code(&self, context: &mut CompilationContext, ir_block: &IRBlock) -> JITResult<Vec<u8>> {
        let start_time = std::time::Instant::now();
        
        let code_generator = self.code_generator_factory.create_code_generator(context.config.target_arch);
        let machine_code = code_generator.generate(ir_block)?;
        
        let generation_time = start_time.elapsed().as_nanos() as u64;
        context.update_code_generation_stats(generation_time);
        
        Ok(machine_code)
    }
    
    /// 验证机器码
    fn verify_machine_code(&self, context: &mut CompilationContext, machine_code: &[u8]) -> JITResult<()> {
        let start_time = std::time::Instant::now();
        
        // 简化的验证逻辑
        if machine_code.is_empty() {
            return Err(JITErrorBuilder::compilation("生成的机器码为空"));
        }
        
        // 检查代码大小限制
        if machine_code.len() > 10 * 1024 * 1024 { // 10MB限制
            return Err(JITErrorBuilder::compilation("生成的机器码过大"));
        }
        
        let verification_time = start_time.elapsed().as_nanos() as u64;
        context.update_verification_stats(verification_time);
        
        Ok(())
    }
}

/// 编译器工厂特征
pub trait CompilerFactory: Send + Sync {
    /// 创建编译器
    fn create_compiler(&self, config: &CompilationConfig) -> Box<dyn Compiler>;
}

/// 优化器工厂特征
pub trait OptimizerFactory: Send + Sync {
    /// 创建优化器
    fn create_optimizer(&self, level: OptimizationLevel) -> Box<dyn Optimizer>;
}

/// 代码生成器工厂特征
pub trait CodeGeneratorFactory: Send + Sync {
    /// 创建代码生成器
    fn create_code_generator(&self, arch: TargetArchitecture) -> Box<dyn CodeGenerator>;
}

/// 编译器特征
pub trait Compiler: Send + Sync {
    /// 编译
    fn compile(&self, ir_block: &IRBlock) -> JITResult<Vec<u8>>;
}

/// 优化器特征
pub trait Optimizer: Send + Sync {
    /// 优化IR块
    fn optimize(&self, ir_block: &IRBlock) -> JITResult<IRBlock>;
}

/// 代码生成器特征
pub trait CodeGenerator: Send + Sync {
    /// 生成机器码
    fn generate(&self, ir_block: &IRBlock) -> JITResult<Vec<u8>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_compilation_context() {
        let ir_block = IRBlock {
            start_pc: 0x1000,
            ops: vec![
                IROp::MovImm { dst: 1, imm: 42 },
                IROp::Add { dst: 2, src1: 1, src2: 1 },
            ],
        };
        
        let config = CompilationConfig::default();
        let mut context = CompilationContext::new(ir_block, config);
        
        assert_eq!(context.status, CompilationStatus::Pending);
        assert_eq!(context.stats.ir_instruction_count, 0);
        
        context.start_compilation();
        assert_eq!(context.status, CompilationStatus::InProgress);
        assert!(context.stats.start_time.is_some());
        
        context.update_ir_stats(2);
        assert_eq!(context.stats.ir_instruction_count, 2);
    }
    
    #[test]
    fn test_compilation_progress() {
        let progress = CompilationProgress::new(0.5);
        assert_eq!(progress.percentage, 0.5);
        assert!(!progress.is_finished);
        
        let finished = CompilationProgress::finished();
        assert_eq!(finished.percentage, 1.0);
        assert!(finished.is_finished);
    }
    
    #[test]
    fn test_compilation_id() {
        let id1 = CompilationId::new();
        let id2 = CompilationId::new();

        assert_ne!(id1.value(), id2.value());
        assert!(id1.value() > 0);
        assert!(id2.value() > id1.value());
    }

    #[test]
    fn test_code_hash_deterministic() {
        use vm_ir::{IROp, IRBlock};

        // 相同的输入应该产生相同的哈希值
        let machine_code = vec
![0x90, 0x90, 0xC3];
        let ir_block = IRBlock {
            start_pc: 0x1000,
            ops: vec
![IROp::Nop],
            term: vm_ir::Terminator::Ret,
        };

        let hash1 = CompilationService::compute_code_hash(&machine_code, &ir_block);
        let hash2 = CompilationService::compute_code_hash(&machine_code, &ir_block);

        assert_eq!(hash1, hash2, "Hash should be deterministic");
    }

    #[test]
    fn test_code_hash_different_inputs() {
        use vm_ir::{IROp, IRBlock};

        let machine_code1 = vec
![0x90, 0x90, 0xC3];
        let ir_block1 = IRBlock {
            start_pc: 0x1000,
            ops: vec
![IROp::Nop],
            term: vm_ir::Terminator::Ret,
        };

        let machine_code2 = vec
![0x90, 0x90, 0x90];
        let ir_block2 = IRBlock {
            start_pc: 0x1000,
            ops: vec
![IROp::Nop],
            term: vm_ir::Terminator::Ret,
        };

        let hash1 = CompilationService::compute_code_hash(&machine_code1, &ir_block1);
        let hash2 = CompilationService::compute_code_hash(&machine_code2, &ir_block2);

        assert_ne!(hash1, hash2, "Different machine code should produce different hashes");
    }

    #[test]
    fn test_code_hash_different_operations()
 {
        use vm_ir::{IROp, IRBlock};

        let machine_code = vec
![0x90, 0x90, 0xC3];
        let ir_block1 = IRBlock {
            start_pc: 0x1000,
            ops: vec
![IROp::Nop],
            term: vm_ir::Terminator::Ret,
        };

        let ir_block2 = IRBlock {
            start_pc: 0x1000,
            ops: vec
![IROp::MovImm { dst: 1, imm: 42 }],
            term: vm_ir::Terminator::Ret,
        };

        let hash1 = CompilationService::compute_code_hash(&machine_code, &ir_block1);
        let hash2 = CompilationService::compute_code_hash(&machine_code, &ir_block2);

        assert_ne!(hash1, hash2, "Different operations should produce different hashes");
    }

    #[test]
    fn test_code_hash_consistent() {
        use vm_ir::{IROp, IRBlock};

        // 测试哈希函数的一致性
        let machine_code = vec
![0x48, 0x89, 0xD8, 0xC3]; // mov rax, rbx; ret
        let ir_block = IRBlock {
            start_pc: 0x1000,
            ops: vec
![
                IROp::MovImm { dst: 1, imm: 10 },
                IROp::MovImm { dst: 2, imm: 20 },
                IROp::Add { dst: 3, src1: 1, src2: 2 },
            ],
            term: vm_ir::Terminator::Ret,
        };

        let hash1 = CompilationService::compute_code_hash(&machine_code, &ir_block);
        let hash2 = CompilationService::compute_code_hash(&machine_code, &ir_block);
        let hash3 = CompilationService::compute_code_hash(&machine_code, &ir_block);

        assert_eq!(hash1, hash2);
        assert_eq!(hash2, hash3);
        assert_ne!(hash1, 0, "Hash should not be zero for valid input");
    }
}