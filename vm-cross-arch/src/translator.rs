//! 架构转换器模块
//!
//! 实现跨架构指令转换的核心逻辑

use super::{
    AdaptiveOptimizer, ArchEncoder, Architecture, Arm64Encoder, CacheReplacementPolicy,
    CrossArchBlockCache, Endianness, EndiannessConversionStrategy, IROptimizer,
    InstructionParallelizer, MemoryAlignmentOptimizer, OptimizedRegisterMapper,
    ResourceRequirements, Riscv64Encoder, SmartRegisterMapper, TargetInstruction,
    TargetSpecificOptimizer, TranslationResult, TranslationStats, X86_64Encoder,
    block_cache::CacheStats, memory_alignment_optimizer::MemoryOptimizationStats,
};
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::Mutex;
use thiserror::Error;
use tracing;
use vm_core::{GuestAddr, VmError};
pub use vm_ir::IROp;
use vm_ir::{IRBlock, RegId, Terminator};

/// 源架构类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceArch {
    X86_64,
    ARM64,
    RISCV64,
}

impl From<SourceArch> for Architecture {
    fn from(arch: SourceArch) -> Self {
        match arch {
            SourceArch::X86_64 => Architecture::X86_64,
            SourceArch::ARM64 => Architecture::ARM64,
            SourceArch::RISCV64 => Architecture::RISCV64,
        }
    }
}

impl From<Architecture> for SourceArch {
    fn from(arch: Architecture) -> Self {
        match arch {
            Architecture::X86_64 => SourceArch::X86_64,
            Architecture::ARM64 => SourceArch::ARM64,
            Architecture::RISCV64 => SourceArch::RISCV64,
        }
    }
}

/// 目标架构类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetArch {
    X86_64,
    ARM64,
    RISCV64,
}

impl From<TargetArch> for Architecture {
    fn from(arch: TargetArch) -> Self {
        match arch {
            TargetArch::X86_64 => Architecture::X86_64,
            TargetArch::ARM64 => Architecture::ARM64,
            TargetArch::RISCV64 => Architecture::RISCV64,
        }
    }
}

impl From<Architecture> for TargetArch {
    fn from(arch: Architecture) -> Self {
        match arch {
            Architecture::X86_64 => TargetArch::X86_64,
            Architecture::ARM64 => TargetArch::ARM64,
            Architecture::RISCV64 => TargetArch::RISCV64,
        }
    }
}

/// 转换错误类型
#[derive(Error, Debug)]
pub enum TranslationError {
    #[error("字符串错误: {0}")]
    StringError(String),
    #[error("不支持的IR操作: {op}")]
    UnsupportedOperation { op: String },

    #[error("立即数过大: {imm}")]
    ImmediateTooLarge { imm: i64 },

    #[error("无效的偏移量: {offset}")]
    InvalidOffset { offset: i64 },

    #[error("寄存器映射失败: {reason}")]
    RegisterMappingFailed { reason: String },

    #[error("寄存器分配失败: {0}")]
    RegisterAllocationFailed(String),

    #[error("编码错误: {message}")]
    EncodingError { message: String },

    #[error("不支持的架构转换: {source:?} -> {target:?}")]
    UnsupportedArchitecturePair {
        source: Architecture,
        target: Architecture,
    },
}

impl From<String> for TranslationError {
    fn from(s: String) -> Self {
        TranslationError::StringError(s)
    }
}

impl From<TranslationError> for VmError {
    fn from(err: TranslationError) -> Self {
        VmError::Core(vm_core::CoreError::NotImplemented {
            feature: format!("{:?}", err),
            module: "vm-cross-arch".to_string(),
        })
    }
}

/// 架构转换器
///
/// 负责将源架构的IR块转换为目标架构的指令序列
pub struct ArchTranslator {
    source_arch: Architecture,
    target_arch: Architecture,
    encoder: Box<dyn ArchEncoder>,
    register_mapper: SmartRegisterMapper,
    /// 优化的寄存器映射器
    optimized_mapper: Option<OptimizedRegisterMapper>,
    /// 块级翻译缓存
    block_cache: Option<Arc<Mutex<CrossArchBlockCache>>>,
    /// 是否使用优化寄存器分配
    use_optimized_allocation: bool,
    /// 内存对齐和端序优化器
    memory_optimizer: Option<MemoryAlignmentOptimizer>,
    /// IR优化器
    ir_optimizer: Option<IROptimizer>,
    /// 目标特定优化器
    target_optimizer: Option<TargetSpecificOptimizer>,
    /// 自适应优化器
    adaptive_optimizer: Option<AdaptiveOptimizer>,
}

impl ArchTranslator {
    /// 创建新的架构转换器
    pub fn new(source_arch: SourceArch, target_arch: TargetArch) -> Self {
        Self::with_cache(source_arch, target_arch, None)
    }

    /// 创建带有块级缓存的架构转换器
    pub fn with_cache(
        source_arch: SourceArch,
        target_arch: TargetArch,
        cache_size: Option<usize>,
    ) -> Self {
        Self::with_cache_and_optimization(source_arch, target_arch, cache_size, false)
    }

    /// 创建带有块级缓存和优化寄存器分配的架构转换器
    pub fn with_cache_and_optimization(
        source_arch: SourceArch,
        target_arch: TargetArch,
        cache_size: Option<usize>,
        use_optimized_allocation: bool,
    ) -> Self {
        Self::with_cache_optimization_and_memory(
            source_arch,
            target_arch,
            cache_size,
            use_optimized_allocation,
            true, // 默认启用内存优化
        )
    }

    /// 创建带有块级缓存、优化寄存器分配和内存优化的架构转换器
    pub fn with_cache_optimization_and_memory(
        source_arch: SourceArch,
        target_arch: TargetArch,
        cache_size: Option<usize>,
        use_optimized_allocation: bool,
        use_memory_optimization: bool,
    ) -> Self {
        Self::with_cache_optimization_memory_and_ir(
            source_arch,
            target_arch,
            cache_size,
            use_optimized_allocation,
            use_memory_optimization,
            true, // 默认启用IR优化
        )
    }

    /// 创建带有块级缓存、优化寄存器分配、内存优化和IR优化的架构转换器
    pub fn with_cache_optimization_memory_and_ir(
        source_arch: SourceArch,
        target_arch: TargetArch,
        cache_size: Option<usize>,
        use_optimized_allocation: bool,
        use_memory_optimization: bool,
        use_ir_optimization: bool,
    ) -> Self {
        Self::with_cache_optimization_memory_ir_and_target(
            source_arch,
            target_arch,
            cache_size,
            use_optimized_allocation,
            use_memory_optimization,
            use_ir_optimization,
            true, // 默认启用目标特定优化
        )
    }

    /// 创建带有块级缓存、优化寄存器分配、内存优化、IR优化和目标特定优化的架构转换器
    pub fn with_cache_optimization_memory_ir_and_target(
        source_arch: SourceArch,
        target_arch: TargetArch,
        cache_size: Option<usize>,
        use_optimized_allocation: bool,
        use_memory_optimization: bool,
        use_ir_optimization: bool,
        use_target_optimization: bool,
    ) -> Self {
        Self::with_all_optimizations(
            source_arch,
            target_arch,
            cache_size,
            use_optimized_allocation,
            use_memory_optimization,
            use_ir_optimization,
            use_target_optimization,
            true, // 默认启用自适应优化
        )
    }

    /// 创建带有所有优化器的架构转换器
    pub fn with_all_optimizations(
        source_arch: SourceArch,
        target_arch: TargetArch,
        cache_size: Option<usize>,
        use_optimized_allocation: bool,
        use_memory_optimization: bool,
        use_ir_optimization: bool,
        use_target_optimization: bool,
        use_adaptive_optimization: bool,
    ) -> Self {
        let source: Architecture = source_arch.into();
        let target: Architecture = target_arch.into();

        // 选择编码器
        let encoder: Box<dyn ArchEncoder> = match target {
            Architecture::X86_64 => Box::new(X86_64Encoder),
            Architecture::ARM64 => Box::new(Arm64Encoder),
            Architecture::RISCV64 => Box::new(Riscv64Encoder),
        };

        // 创建智能寄存器映射器
        let register_mapper = SmartRegisterMapper::new(target);

        // 创建优化寄存器映射器（如果启用）
        let optimized_mapper = if use_optimized_allocation {
            Some(OptimizedRegisterMapper::new(target))
        } else {
            None
        };

        // 创建块级缓存（如果指定了大小）
        let block_cache = cache_size.map(|size| {
            Arc::new(Mutex::new(CrossArchBlockCache::new(
                size,
                CacheReplacementPolicy::LRU,
            )))
        });

        // 创建内存对齐和端序优化器
        let memory_optimizer = if use_memory_optimization {
            // 确定源和目标架构的端序
            let source_endianness = match source {
                Architecture::X86_64 => Endianness::LittleEndian,
                Architecture::ARM64 => Endianness::LittleEndian, // ARM64可以是小端或大端，但通常是小端
                Architecture::RISCV64 => Endianness::LittleEndian, // RISC-V默认小端
            };

            let target_endianness = match target {
                Architecture::X86_64 => Endianness::LittleEndian,
                Architecture::ARM64 => Endianness::LittleEndian,
                Architecture::RISCV64 => Endianness::LittleEndian,
            };

            // 选择端序转换策略
            let conversion_strategy = if source_endianness == target_endianness {
                EndiannessConversionStrategy::None
            } else {
                EndiannessConversionStrategy::Hybrid
            };

            Some(MemoryAlignmentOptimizer::new(
                source_endianness,
                target_endianness,
                conversion_strategy,
            ))
        } else {
            None
        };

        // 创建IR优化器
        let ir_optimizer = if use_ir_optimization {
            Some(IROptimizer::new())
        } else {
            None
        };

        // 创建目标特定优化器
        let target_optimizer = if use_target_optimization {
            Some(TargetSpecificOptimizer::new(target))
        } else {
            None
        };

        // 创建自适应优化器
        let adaptive_optimizer = if use_adaptive_optimization {
            Some(AdaptiveOptimizer::new())
        } else {
            None
        };

        Self {
            source_arch: source_arch.into(),
            target_arch: target_arch.into(),
            encoder,
            register_mapper,
            optimized_mapper,
            block_cache,
            use_optimized_allocation,
            memory_optimizer,
            ir_optimizer,
            target_optimizer,
            adaptive_optimizer,
        }
    }

    /// 转换IR块为目标架构指令序列
    pub fn translate_block(
        &mut self,
        block: &IRBlock,
    ) -> Result<TranslationResult, TranslationError> {
        // 如果有缓存，尝试从缓存获取
        if let Some(ref cache) = self.block_cache {
            // 先克隆缓存引用，避免可变/不可变借用冲突
            let cache = cache.clone();

            // 现在可以安全地调用get_or_translate，因为self.block_cache的不可变借用已经结束
            if let Ok(cached_result) = cache.lock().get_or_translate(self, block) {
                return Ok(cached_result);
            }
        }

        // 缓存未命中或无缓存，执行正常翻译流程
        self.translate_block_internal(block)
    }

    /// 内部翻译方法，不使用缓存
    pub fn translate_block_internal(
        &mut self,
        block: &IRBlock,
    ) -> Result<TranslationResult, TranslationError> {
        let mut instructions = Vec::new();
        let mut stats = TranslationStats::default();

        // 重置寄存器映射器
        self.register_mapper.reset();

        // 如果使用优化分配器，也重置它
        if let Some(ref mut optimized_mapper) = self.optimized_mapper {
            optimized_mapper.reset();
        }

        // 应用IR优化
        let mut optimized_ops = if let Some(ref mut ir_optimizer) = self.ir_optimizer {
            ir_optimizer.optimize(&block.ops)
        } else {
            block.ops.clone()
        };

        // 应用目标特定优化
        if let Some(ref mut target_optimizer) = self.target_optimizer {
            optimized_ops = target_optimizer.optimize(&optimized_ops);
        }

        // 应用自适应优化
        if let Some(ref mut adaptive_optimizer) = self.adaptive_optimizer {
            optimized_ops = adaptive_optimizer.optimize(&optimized_ops, block.start_pc.0);
        }

        // 根据是否使用优化分配器选择不同的路径
        if self.use_optimized_allocation {
            // 使用优化寄存器分配
            if let Some(ref mut optimized_mapper) = self.optimized_mapper {
                // 执行优化的寄存器分配
                if let Err(e) = optimized_mapper.allocate_registers(&optimized_ops) {
                    // 如果优化分配失败，回退到简单映射
                    tracing::warn!(
                        "Optimized register allocation failed: {}, falling back to simple mapping",
                        e
                    );

                    // 使用标准分配器
                    let live_ranges = self.analyze_live_ranges(block);
                    if let Err(e) = self.register_mapper.allocate_registers(&live_ranges) {
                        return Err(TranslationError::RegisterAllocationFailed(e));
                    }
                } else {
                    // 优化分配成功，获取优化统计
                    let opt_stats = optimized_mapper.get_optimization_stats();
                    stats.copies_eliminated = opt_stats.copies_eliminated;
                    stats.registers_reused = opt_stats.registers_reused;
                }
            }
        } else {
            // 使用标准寄存器分配
            let live_ranges = self.analyze_live_ranges(block);
            if let Err(e) = self.register_mapper.allocate_registers(&live_ranges) {
                return Err(TranslationError::RegisterAllocationFailed(e));
            }
        }

        // 创建指令并行化器
        let mut parallelizer =
            InstructionParallelizer::new(ResourceRequirements::for_architecture(self.target_arch));

        // 分析指令并行性并重新排序
        let mut parallel_ops = parallelizer.optimize_instruction_sequence(&optimized_ops)?;

        // 应用内存对齐和端序优化
        if let Some(ref mut memory_optimizer) = self.memory_optimizer {
            // 分析内存访问模式并优化
            parallel_ops = memory_optimizer.optimize_for_pattern(&parallel_ops);

            // 进一步优化内存操作序列
            parallel_ops = memory_optimizer.optimize_memory_sequence(&parallel_ops);
        }

        // 转换每个优化后的IR操作
        for op in &parallel_ops {
            stats.ir_ops_translated += 1;

            // 映射寄存器
            let mapped_op = self.map_registers_in_op(op)?;

            // 编码为目标架构指令
            let target_insns = self.encoder.encode_op(&mapped_op, block.start_pc)?;
            stats.target_instructions_generated += target_insns.len();

            if target_insns.len() > 1 {
                stats.complex_operations += 1;
            }

            instructions.extend(target_insns);
        }

        // 处理终结符（跳转、返回等）
        let term_insns = self.translate_terminator(&block.term, block.start_pc)?;
        instructions.extend(term_insns);

        // 更新统计信息
        let allocation_stats = self.register_mapper.get_stats();
        stats.register_mappings = allocation_stats.total_mappings;

        Ok(TranslationResult {
            instructions,
            stats,
        })
    }

    /// 分析IR块中的寄存器活跃范围
    fn analyze_live_ranges(&self, block: &IRBlock) -> Vec<(vm_ir::RegId, (usize, usize))> {
        use std::collections::HashMap;

        let mut def_points: HashMap<vm_ir::RegId, usize> = HashMap::new();
        let mut use_points: HashMap<vm_ir::RegId, usize> = HashMap::new();
        let mut live_ranges = Vec::new();

        // 第一遍：收集定义和使用点
        for (idx, op) in block.ops.iter().enumerate() {
            self.collect_reg_defs_uses(op, idx, &mut def_points, &mut use_points);
        }

        // 构建活跃范围
        for (reg, &def_idx) in &def_points {
            let use_idx = use_points.get(reg).unwrap_or(&def_idx);
            let start = def_idx.min(*use_idx);
            let end = def_idx.max(*use_idx);
            live_ranges.push((*reg, (start, end)));
        }

        live_ranges
    }

    /// 收集IR操作中的寄存器定义和使用点
    fn collect_reg_defs_uses(
        &self,
        op: &vm_ir::IROp,
        idx: usize,
        def_points: &mut HashMap<vm_ir::RegId, usize>,
        use_points: &mut HashMap<vm_ir::RegId, usize>,
    ) {
        match op {
            vm_ir::IROp::Add { dst, src1, src2 } => {
                def_points.entry(*dst).or_insert(idx);
                use_points.entry(*src1).or_insert(idx);
                use_points.entry(*src2).or_insert(idx);
            }
            vm_ir::IROp::Sub { dst, src1, src2 } => {
                def_points.entry(*dst).or_insert(idx);
                use_points.entry(*src1).or_insert(idx);
                use_points.entry(*src2).or_insert(idx);
            }
            vm_ir::IROp::AddImm { dst, src, .. } => {
                def_points.entry(*dst).or_insert(idx);
                use_points.entry(*src).or_insert(idx);
            }
            vm_ir::IROp::MovImm { dst, .. } => {
                def_points.entry(*dst).or_insert(idx);
            }
            vm_ir::IROp::Load { dst, base, .. } => {
                def_points.entry(*dst).or_insert(idx);
                use_points.entry(*base).or_insert(idx);
            }
            vm_ir::IROp::Store { src, base, .. } => {
                use_points.entry(*src).or_insert(idx);
                use_points.entry(*base).or_insert(idx);
            }
            vm_ir::IROp::Mul {
                dst, src1, src2, ..
            } => {
                def_points.entry(*dst).or_insert(idx);
                use_points.entry(*src1).or_insert(idx);
                use_points.entry(*src2).or_insert(idx);
            }
            vm_ir::IROp::Div {
                dst,
                src1,
                src2,
                signed: _,
            } => {
                def_points.entry(*dst).or_insert(idx);
                use_points.entry(*src1).or_insert(idx);
                use_points.entry(*src2).or_insert(idx);
            }
            vm_ir::IROp::Beq { src1, src2, .. } => {
                use_points.entry(*src1).or_insert(idx);
                use_points.entry(*src2).or_insert(idx);
            }
            vm_ir::IROp::Bne { src1, src2, .. } => {
                use_points.entry(*src1).or_insert(idx);
                use_points.entry(*src2).or_insert(idx);
            }
            vm_ir::IROp::Blt { src1, src2, .. } => {
                use_points.entry(*src1).or_insert(idx);
                use_points.entry(*src2).or_insert(idx);
            }
            vm_ir::IROp::Bge { src1, src2, .. } => {
                use_points.entry(*src1).or_insert(idx);
                use_points.entry(*src2).or_insert(idx);
            }
            vm_ir::IROp::Bltu { src1, src2, .. } => {
                use_points.entry(*src1).or_insert(idx);
                use_points.entry(*src2).or_insert(idx);
            }
            vm_ir::IROp::Bgeu { src1, src2, .. } => {
                use_points.entry(*src1).or_insert(idx);
                use_points.entry(*src2).or_insert(idx);
            }
            // 跳转指令在Terminator中，不在IROp中
            _ => {
                // 对于其他操作类型，暂时不处理
            }
        }
    }

    /// 映射源寄存器到目标寄存器
    pub fn map_register(&self, source_reg: RegId) -> RegId {
        // 如果使用优化分配器，优先使用它
        if self.use_optimized_allocation
            && let Some(ref optimized_mapper) = self.optimized_mapper
        {
            return optimized_mapper.map_register(source_reg);
        }

        // 回退到标准映射器
        self.register_mapper.map_register(source_reg)
    }

    /// 映射IR操作中的寄存器
    fn map_registers_in_op(&mut self, op: &IROp) -> Result<IROp, TranslationError> {
        match op {
            IROp::Add { dst, src1, src2 } => Ok(IROp::Add {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::Sub { dst, src1, src2 } => Ok(IROp::Sub {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::AddImm { dst, src, imm } => Ok(IROp::AddImm {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
                imm: *imm,
            }),
            IROp::MovImm { dst, imm } => Ok(IROp::MovImm {
                dst: self.register_mapper.map_register(*dst),
                imm: *imm,
            }),

            IROp::Load {
                dst,
                base,
                offset,
                size,
                flags,
            } => Ok(IROp::Load {
                dst: self.register_mapper.map_register(*dst),
                base: self.register_mapper.map_register(*base),
                offset: *offset,
                size: *size,
                flags: *flags,
            }),
            IROp::Store {
                src,
                base,
                offset,
                size,
                flags,
            } => Ok(IROp::Store {
                src: self.register_mapper.map_register(*src),
                base: self.register_mapper.map_register(*base),
                offset: *offset,
                size: *size,
                flags: *flags,
            }),
            IROp::Mul { dst, src1, src2 } => Ok(IROp::Mul {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::Div {
                dst,
                src1,
                src2,
                signed,
            } => Ok(IROp::Div {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
                signed: *signed,
            }),
            IROp::Rem {
                dst,
                src1,
                src2,
                signed,
            } => Ok(IROp::Rem {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
                signed: *signed,
            }),
            // 逻辑操作
            IROp::And { dst, src1, src2 } => Ok(IROp::And {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::Or { dst, src1, src2 } => Ok(IROp::Or {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::Xor { dst, src1, src2 } => Ok(IROp::Xor {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::Not { dst, src } => Ok(IROp::Not {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            // 移位操作
            IROp::Sll { dst, src, shreg } => Ok(IROp::Sll {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
                shreg: self.register_mapper.map_register(*shreg),
            }),
            IROp::Srl { dst, src, shreg } => Ok(IROp::Srl {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
                shreg: self.register_mapper.map_register(*shreg),
            }),
            IROp::Sra { dst, src, shreg } => Ok(IROp::Sra {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
                shreg: self.register_mapper.map_register(*shreg),
            }),
            IROp::SllImm { dst, src, sh } => Ok(IROp::SllImm {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
                sh: *sh,
            }),
            IROp::SrlImm { dst, src, sh } => Ok(IROp::SrlImm {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
                sh: *sh,
            }),
            IROp::SraImm { dst, src, sh } => Ok(IROp::SraImm {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
                sh: *sh,
            }),
            // 立即数操作
            IROp::MulImm { dst, src, imm } => Ok(IROp::MulImm {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
                imm: *imm,
            }),
            IROp::Mov { dst, src } => Ok(IROp::Mov {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            // 比较操作
            IROp::CmpEq { dst, lhs, rhs } => Ok(IROp::CmpEq {
                dst: self.register_mapper.map_register(*dst),
                lhs: self.register_mapper.map_register(*lhs),
                rhs: self.register_mapper.map_register(*rhs),
            }),
            IROp::CmpNe { dst, lhs, rhs } => Ok(IROp::CmpNe {
                dst: self.register_mapper.map_register(*dst),
                lhs: self.register_mapper.map_register(*lhs),
                rhs: self.register_mapper.map_register(*rhs),
            }),
            IROp::CmpLt { dst, lhs, rhs } => Ok(IROp::CmpLt {
                dst: self.register_mapper.map_register(*dst),
                lhs: self.register_mapper.map_register(*lhs),
                rhs: self.register_mapper.map_register(*rhs),
            }),
            IROp::CmpLtU { dst, lhs, rhs } => Ok(IROp::CmpLtU {
                dst: self.register_mapper.map_register(*dst),
                lhs: self.register_mapper.map_register(*lhs),
                rhs: self.register_mapper.map_register(*rhs),
            }),
            IROp::CmpGe { dst, lhs, rhs } => Ok(IROp::CmpGe {
                dst: self.register_mapper.map_register(*dst),
                lhs: self.register_mapper.map_register(*lhs),
                rhs: self.register_mapper.map_register(*rhs),
            }),
            IROp::CmpGeU { dst, lhs, rhs } => Ok(IROp::CmpGeU {
                dst: self.register_mapper.map_register(*dst),
                lhs: self.register_mapper.map_register(*lhs),
                rhs: self.register_mapper.map_register(*rhs),
            }),
            // Select 操作
            IROp::Select {
                dst,
                cond,
                true_val,
                false_val,
            } => Ok(IROp::Select {
                dst: self.register_mapper.map_register(*dst),
                cond: self.register_mapper.map_register(*cond),
                true_val: self.register_mapper.map_register(*true_val),
                false_val: self.register_mapper.map_register(*false_val),
            }),
            // Nop
            IROp::Nop => Ok(IROp::Nop),
            IROp::Beq { src1, src2, target } => Ok(IROp::Beq {
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
                target: *target,
            }),
            IROp::Bne { src1, src2, target } => Ok(IROp::Bne {
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
                target: *target,
            }),
            IROp::Blt { src1, src2, target } => Ok(IROp::Blt {
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
                target: *target,
            }),
            IROp::Bge { src1, src2, target } => Ok(IROp::Bge {
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
                target: *target,
            }),
            IROp::Bltu { src1, src2, target } => Ok(IROp::Bltu {
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
                target: *target,
            }),
            IROp::Bgeu { src1, src2, target } => Ok(IROp::Bgeu {
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
                target: *target,
            }),
            // Jmp, CondJmp, and Call are in Terminator, not IROp
            // SIMD操作
            IROp::VecAdd {
                dst,
                src1,
                src2,
                element_size,
            } => Ok(IROp::VecAdd {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
                element_size: *element_size,
            }),
            IROp::VecSub {
                dst,
                src1,
                src2,
                element_size,
            } => Ok(IROp::VecSub {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
                element_size: *element_size,
            }),
            IROp::VecMul {
                dst,
                src1,
                src2,
                element_size,
            } => Ok(IROp::VecMul {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
                element_size: *element_size,
            }),
            IROp::VecAddSat {
                dst,
                src1,
                src2,
                element_size,
                signed,
            } => Ok(IROp::VecAddSat {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
                element_size: *element_size,
                signed: *signed,
            }),
            IROp::VecSubSat {
                dst,
                src1,
                src2,
                element_size,
                signed,
            } => Ok(IROp::VecSubSat {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
                element_size: *element_size,
                signed: *signed,
            }),
            IROp::VecMulSat {
                dst,
                src1,
                src2,
                element_size,
                signed,
            } => Ok(IROp::VecMulSat {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
                element_size: *element_size,
                signed: *signed,
            }),
            IROp::Vec128Add {
                dst_lo,
                dst_hi,
                src1_lo,
                src1_hi,
                src2_lo,
                src2_hi,
                element_size,
                signed,
            } => Ok(IROp::Vec128Add {
                dst_lo: self.register_mapper.map_register(*dst_lo),
                dst_hi: self.register_mapper.map_register(*dst_hi),
                src1_lo: self.register_mapper.map_register(*src1_lo),
                src1_hi: self.register_mapper.map_register(*src1_hi),
                src2_lo: self.register_mapper.map_register(*src2_lo),
                src2_hi: self.register_mapper.map_register(*src2_hi),
                element_size: *element_size,
                signed: *signed,
            }),
            IROp::Vec256Add {
                dst0,
                dst1,
                dst2,
                dst3,
                src10,
                src11,
                src12,
                src13,
                src20,
                src21,
                src22,
                src23,
                element_size,
                signed,
            } => Ok(IROp::Vec256Add {
                dst0: self.register_mapper.map_register(*dst0),
                dst1: self.register_mapper.map_register(*dst1),
                dst2: self.register_mapper.map_register(*dst2),
                dst3: self.register_mapper.map_register(*dst3),
                src10: self.register_mapper.map_register(*src10),
                src11: self.register_mapper.map_register(*src11),
                src12: self.register_mapper.map_register(*src12),
                src13: self.register_mapper.map_register(*src13),
                src20: self.register_mapper.map_register(*src20),
                src21: self.register_mapper.map_register(*src21),
                src22: self.register_mapper.map_register(*src22),
                src23: self.register_mapper.map_register(*src23),
                element_size: *element_size,
                signed: *signed,
            }),
            IROp::Vec256Sub {
                dst0,
                dst1,
                dst2,
                dst3,
                src10,
                src11,
                src12,
                src13,
                src20,
                src21,
                src22,
                src23,
                element_size,
                signed,
            } => Ok(IROp::Vec256Sub {
                dst0: self.register_mapper.map_register(*dst0),
                dst1: self.register_mapper.map_register(*dst1),
                dst2: self.register_mapper.map_register(*dst2),
                dst3: self.register_mapper.map_register(*dst3),
                src10: self.register_mapper.map_register(*src10),
                src11: self.register_mapper.map_register(*src11),
                src12: self.register_mapper.map_register(*src12),
                src13: self.register_mapper.map_register(*src13),
                src20: self.register_mapper.map_register(*src20),
                src21: self.register_mapper.map_register(*src21),
                src22: self.register_mapper.map_register(*src22),
                src23: self.register_mapper.map_register(*src23),
                element_size: *element_size,
                signed: *signed,
            }),
            IROp::Vec256Mul {
                dst0,
                dst1,
                dst2,
                dst3,
                src10,
                src11,
                src12,
                src13,
                src20,
                src21,
                src22,
                src23,
                element_size,
                signed,
            } => Ok(IROp::Vec256Mul {
                dst0: self.register_mapper.map_register(*dst0),
                dst1: self.register_mapper.map_register(*dst1),
                dst2: self.register_mapper.map_register(*dst2),
                dst3: self.register_mapper.map_register(*dst3),
                src10: self.register_mapper.map_register(*src10),
                src11: self.register_mapper.map_register(*src11),
                src12: self.register_mapper.map_register(*src12),
                src13: self.register_mapper.map_register(*src13),
                src20: self.register_mapper.map_register(*src20),
                src21: self.register_mapper.map_register(*src21),
                src22: self.register_mapper.map_register(*src22),
                src23: self.register_mapper.map_register(*src23),
                element_size: *element_size,
                signed: *signed,
            }),
            // 浮点操作
            IROp::Fadd { dst, src1, src2 } => Ok(IROp::Fadd {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::Fsub { dst, src1, src2 } => Ok(IROp::Fsub {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::Fmul { dst, src1, src2 } => Ok(IROp::Fmul {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::Fdiv { dst, src1, src2 } => Ok(IROp::Fdiv {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::FaddS { dst, src1, src2 } => Ok(IROp::FaddS {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::FsubS { dst, src1, src2 } => Ok(IROp::FsubS {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::FmulS { dst, src1, src2 } => Ok(IROp::FmulS {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::FdivS { dst, src1, src2 } => Ok(IROp::FdivS {
                dst: self.register_mapper.map_register(*dst),
                src1: self.register_mapper.map_register(*src1),
                src2: self.register_mapper.map_register(*src2),
            }),
            IROp::Fsqrt { dst, src } => Ok(IROp::Fsqrt {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            IROp::FsqrtS { dst, src } => Ok(IROp::FsqrtS {
                dst: self.register_mapper.map_register(*dst),
                src: self.register_mapper.map_register(*src),
            }),
            // 原子操作
            IROp::AtomicRMW {
                dst,
                base,
                src,
                op,
                size,
            } => Ok(IROp::AtomicRMW {
                dst: self.register_mapper.map_register(*dst),
                base: self.register_mapper.map_register(*base),
                src: self.register_mapper.map_register(*src),
                op: *op,
                size: *size,
            }),
            IROp::AtomicCmpXchg {
                dst,
                base,
                expected,
                new,
                size,
            } => Ok(IROp::AtomicCmpXchg {
                dst: self.register_mapper.map_register(*dst),
                base: self.register_mapper.map_register(*base),
                expected: self.register_mapper.map_register(*expected),
                new: self.register_mapper.map_register(*new),
                size: *size,
            }),
            IROp::AtomicRMWOrder {
                dst,
                base,
                src,
                op,
                size,
                flags,
            } => Ok(IROp::AtomicRMWOrder {
                dst: self.register_mapper.map_register(*dst),
                base: self.register_mapper.map_register(*base),
                src: self.register_mapper.map_register(*src),
                op: *op,
                size: *size,
                flags: *flags,
            }),
            IROp::AtomicCmpXchgOrder {
                dst,
                base,
                expected,
                new,
                size,
                flags,
            } => Ok(IROp::AtomicCmpXchgOrder {
                dst: self.register_mapper.map_register(*dst),
                base: self.register_mapper.map_register(*base),
                expected: self.register_mapper.map_register(*expected),
                new: self.register_mapper.map_register(*new),
                size: *size,
                flags: *flags,
            }),
            IROp::AtomicLoadReserve {
                dst,
                base,
                offset,
                size,
                flags,
            } => Ok(IROp::AtomicLoadReserve {
                dst: self.register_mapper.map_register(*dst),
                base: self.register_mapper.map_register(*base),
                offset: *offset,
                size: *size,
                flags: *flags,
            }),
            IROp::AtomicStoreCond {
                src,
                base,
                offset,
                size,
                dst_flag,
                flags,
            } => Ok(IROp::AtomicStoreCond {
                src: self.register_mapper.map_register(*src),
                base: self.register_mapper.map_register(*base),
                offset: *offset,
                size: *size,
                dst_flag: self.register_mapper.map_register(*dst_flag),
                flags: *flags,
            }),
            IROp::AtomicCmpXchgFlag {
                dst_old,
                dst_flag,
                base,
                expected,
                new,
                size,
            } => Ok(IROp::AtomicCmpXchgFlag {
                dst_old: self.register_mapper.map_register(*dst_old),
                dst_flag: self.register_mapper.map_register(*dst_flag),
                base: self.register_mapper.map_register(*base),
                expected: self.register_mapper.map_register(*expected),
                new: self.register_mapper.map_register(*new),
                size: *size,
            }),
            IROp::AtomicRmwFlag {
                dst_old,
                dst_flag,
                base,
                src,
                op,
                size,
            } => Ok(IROp::AtomicRmwFlag {
                dst_old: self.register_mapper.map_register(*dst_old),
                dst_flag: self.register_mapper.map_register(*dst_flag),
                base: self.register_mapper.map_register(*base),
                src: self.register_mapper.map_register(*src),
                op: *op,
                size: *size,
            }),
            _ => {
                // 对于未实现的操作，尝试通用映射
                Err(TranslationError::UnsupportedOperation {
                    op: format!("{:?}", op),
                })
            }
        }
    }

    /// 转换终结符
    fn translate_terminator(
        &self,
        term: &Terminator,
        pc: GuestAddr,
    ) -> Result<Vec<TargetInstruction>, TranslationError> {
        match term {
            Terminator::Ret => {
                // 返回指令
                match self.target_arch {
                    Architecture::X86_64 => {
                        // x86-64: RET
                        Ok(vec![TargetInstruction {
                            bytes: vec![0xC3],
                            length: 1,
                            mnemonic: "ret".to_string(),
                            is_control_flow: true,
                            is_memory_op: false,
                        }])
                    }
                    Architecture::ARM64 => {
                        // ARM64: RET X30 (LR)
                        let word: u32 = 0xD65F03C0; // RET
                        Ok(vec![TargetInstruction {
                            bytes: word.to_le_bytes().to_vec(),
                            length: 4,
                            mnemonic: "ret".to_string(),
                            is_control_flow: true,
                            is_memory_op: false,
                        }])
                    }
                    Architecture::RISCV64 => {
                        // RISC-V: RET (JALR x0, x1, 0)
                        let word: u32 = 0x00008067;
                        Ok(vec![TargetInstruction {
                            bytes: word.to_le_bytes().to_vec(),
                            length: 4,
                            mnemonic: "ret".to_string(),
                            is_control_flow: true,
                            is_memory_op: false,
                        }])
                    }
                }
            }
            Terminator::Jmp { target } => {
                // 无条件跳转
                self.encode_jump(*target, pc)
            }
            Terminator::CondJmp {
                cond,
                target_true,
                target_false,
            } => {
                // 条件跳转（需要多指令序列）
                self.encode_conditional_jump(*cond, *target_true, *target_false, pc)
            }
            _ => Err(TranslationError::UnsupportedOperation {
                op: format!("{:?}", term),
            }),
        }
    }

    /// 编码无条件跳转
    fn encode_jump(
        &self,
        target: GuestAddr,
        pc: GuestAddr,
    ) -> Result<Vec<TargetInstruction>, TranslationError> {
        let offset = target.wrapping_sub(pc.wrapping_add(4)) as i64;

        match self.target_arch {
            Architecture::X86_64 => {
                // x86-64: JMP rel32
                let mut bytes = vec![0xE9];
                bytes.extend_from_slice(&(offset as i32).to_le_bytes());
                Ok(vec![TargetInstruction {
                    bytes,
                    length: 5,
                    mnemonic: format!("jmp 0x{:x}", target),
                    is_control_flow: true,
                    is_memory_op: false,
                }])
            }
            Architecture::ARM64 => {
                // ARM64: B label
                let imm26 = (offset >> 2) & 0x3FFFFFF;
                if !(-0x2000000..0x2000000).contains(&imm26) {
                    return Err(TranslationError::InvalidOffset { offset });
                }
                let word: u32 = 0x14000000 | ((imm26 as u32) & 0x3FFFFFF);
                Ok(vec![TargetInstruction {
                    bytes: word.to_le_bytes().to_vec(),
                    length: 4,
                    mnemonic: format!("b 0x{:x}", target),
                    is_control_flow: true,
                    is_memory_op: false,
                }])
            }
            Architecture::RISCV64 => {
                // RISC-V: JAL x0, offset
                let imm20 = (offset >> 1) & 0xFFFFF;
                if !(-0x100000..0x100000).contains(&imm20) {
                    return Err(TranslationError::InvalidOffset { offset });
                }
                let word: u32 = 0x0000006F | (((imm20 as u32) & 0xFFFFF) << 12);
                Ok(vec![TargetInstruction {
                    bytes: word.to_le_bytes().to_vec(),
                    length: 4,
                    mnemonic: format!("jal x0, 0x{:x}", target),
                    is_control_flow: true,
                    is_memory_op: false,
                }])
            }
        }
    }

    /// 编码条件跳转
    fn encode_conditional_jump(
        &self,
        cond: vm_ir::RegId,
        target_true: GuestAddr,
        target_false: GuestAddr,
        pc: GuestAddr,
    ) -> Result<Vec<TargetInstruction>, TranslationError> {
        // 简化实现：假设cond寄存器包含比较结果（0或非0）
        // 实际实现需要根据源架构的条件码格式进行转换

        let offset_true = target_true.wrapping_sub(pc.wrapping_add(4)) as i64;
        let offset_false = target_false.wrapping_sub(pc.wrapping_add(4)) as i64;

        match self.target_arch {
            Architecture::X86_64 => {
                // x86-64: TEST cond, cond; JNZ target_true; JMP target_false
                let mut insns = Vec::new();

                // TEST cond, cond
                insns.push(TargetInstruction {
                    bytes: vec![0x48, 0x85, (0xC0 | ((cond & 7) << 3) | (cond & 7)) as u8],
                    length: 3,
                    mnemonic: format!("test r{}, r{}", cond, cond),
                    is_control_flow: false,
                    is_memory_op: false,
                });

                // JNZ target_true
                insns.push(TargetInstruction {
                    bytes: {
                        let mut b = vec![0x0F, 0x85];
                        b.extend_from_slice(&(offset_true as i32).to_le_bytes());
                        b
                    },
                    length: 6,
                    mnemonic: format!("jnz 0x{:x}", target_true),
                    is_control_flow: true,
                    is_memory_op: false,
                });

                // JMP target_false
                insns.push(TargetInstruction {
                    bytes: {
                        let mut b = vec![0xE9];
                        b.extend_from_slice(&(offset_false as i32).to_le_bytes());
                        b
                    },
                    length: 5,
                    mnemonic: format!("jmp 0x{:x}", target_false),
                    is_control_flow: true,
                    is_memory_op: false,
                });

                Ok(insns)
            }
            Architecture::ARM64 => {
                // ARM64: CMP cond, #0; B.NE target_true; B target_false
                let mut insns = Vec::new();

                // CMP cond, #0
                let word1: u32 = 0xF1000000 | ((cond & 0x1F) << 5) | (cond & 0x1F);
                insns.push(TargetInstruction {
                    bytes: word1.to_le_bytes().to_vec(),
                    length: 4,
                    mnemonic: format!("cmp x{}, #0", cond),
                    is_control_flow: false,
                    is_memory_op: false,
                });

                // B.NE target_true
                let imm26 = (offset_true >> 2) & 0x3FFFFFF;
                let word2: u32 = 0x54000000 | 0x1 | ((imm26 as u32) & 0x3FFFFFF);
                insns.push(TargetInstruction {
                    bytes: word2.to_le_bytes().to_vec(),
                    length: 4,
                    mnemonic: format!("b.ne 0x{:x}", target_true),
                    is_control_flow: true,
                    is_memory_op: false,
                });

                // B target_false
                let imm26_false = (offset_false >> 2) & 0x3FFFFFF;
                let word3: u32 = 0x14000000 | ((imm26_false as u32) & 0x3FFFFFF);
                insns.push(TargetInstruction {
                    bytes: word3.to_le_bytes().to_vec(),
                    length: 4,
                    mnemonic: format!("b 0x{:x}", target_false),
                    is_control_flow: true,
                    is_memory_op: false,
                });

                Ok(insns)
            }
            Architecture::RISCV64 => {
                // RISC-V: BEQ cond, x0, target_false; JAL x0, target_true
                let mut insns = Vec::new();

                // BEQ cond, x0, target_false
                let imm12 = (offset_false >> 1) & 0xFFF;
                let word1: u32 = (0x00000063
                    | ((imm12 & 0x1E) << 7)
                    | ((imm12 & 0x800) >> 4)
                    | ((imm12 & 0x7E0) << 20)
                    | ((imm12 & 0x800) << 19)) as u32;
                insns.push(TargetInstruction {
                    bytes: word1.to_le_bytes().to_vec(),
                    length: 4,
                    mnemonic: format!("beq x{}, x0, 0x{:x}", cond, target_false),
                    is_control_flow: true,
                    is_memory_op: false,
                });

                // JAL x0, target_true
                let imm20 = (offset_true >> 1) & 0xFFFFF;
                let word2: u32 = 0x0000006F | (((imm20 as u32) & 0xFFFFF) << 12);
                insns.push(TargetInstruction {
                    bytes: word2.to_le_bytes().to_vec(),
                    length: 4,
                    mnemonic: format!("jal x0, 0x{:x}", target_true),
                    is_control_flow: true,
                    is_memory_op: false,
                });

                Ok(insns)
            }
        }
    }

    /// 获取源架构
    pub fn source_arch(&self) -> Architecture {
        self.source_arch
    }

    /// 获取目标架构
    pub fn target_arch(&self) -> Architecture {
        self.target_arch
    }

    /// 获取缓存统计信息
    pub fn cache_stats(&self) -> Option<CacheStats> {
        self.block_cache
            .as_ref()
            .map(|cache| cache.lock().stats().clone())
    }

    /// 清空缓存
    pub fn clear_cache(&self) {
        if let Some(ref cache) = self.block_cache {
            cache.lock().clear();
        }
    }

    /// 设置缓存大小
    pub fn set_cache_size(&self, size: usize) {
        if let Some(ref cache) = self.block_cache {
            cache.lock().set_max_size(size);
        }
    }

    /// 获取优化统计信息
    pub fn get_optimization_stats(
        &self,
    ) -> Option<&crate::optimized_register_allocator::OptimizationStats> {
        if self.use_optimized_allocation {
            self.optimized_mapper
                .as_ref()
                .map(|mapper| mapper.get_optimization_stats())
        } else {
            None
        }
    }

    /// 获取内存优化统计信息
    pub fn get_memory_optimization_stats(&self) -> Option<&MemoryOptimizationStats> {
        self.memory_optimizer
            .as_ref()
            .map(|optimizer| optimizer.get_stats())
    }

    /// 获取IR优化统计信息
    pub fn get_ir_optimization_stats(&self) -> Option<&super::IROptimizationStats> {
        self.ir_optimizer
            .as_ref()
            .map(|optimizer| optimizer.get_stats())
    }

    /// 获取目标特定优化统计信息
    pub fn get_target_optimization_stats(&self) -> Option<&super::TargetOptimizationStats> {
        self.target_optimizer
            .as_ref()
            .map(|optimizer: &TargetSpecificOptimizer| optimizer.get_stats())
    }

    /// 获取自适应优化统计信息
    pub fn get_adaptive_optimization_stats(&self) -> Option<&super::AdaptiveOptimizationStats> {
        self.adaptive_optimizer
            .as_ref()
            .map(|optimizer| optimizer.get_stats())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_core::GuestAddr;
    use vm_ir::{IRBuilder, IROp, MemFlags};

    #[test]
    fn test_translator_creation() {
        let translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
        assert_eq!(translator.source_arch(), Architecture::X86_64);
        assert_eq!(translator.target_arch(), Architecture::ARM64);
        assert!(translator.cache_stats().is_none()); // 默认无缓存
    }

    #[test]
    fn test_translator_with_cache() {
        let translator =
            ArchTranslator::with_cache(SourceArch::X86_64, TargetArch::ARM64, Some(100));
        assert_eq!(translator.source_arch(), Architecture::X86_64);
        assert_eq!(translator.target_arch(), Architecture::ARM64);
        assert!(translator.cache_stats().is_some()); // 有缓存
    }

    #[test]
    fn test_simple_translation() {
        let mut translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
        let mut builder = IRBuilder::new(GuestAddr(0x1000));
        builder.push(IROp::Add {
            dst: 0,
            src1: 1,
            src2: 2,
        });
        builder.set_term(Terminator::Ret);
        let block = builder.build();

        let result = translator.translate_block(&block);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cached_translation() {
        let mut translator =
            ArchTranslator::with_cache(SourceArch::X86_64, TargetArch::ARM64, Some(10));
        let mut builder = IRBuilder::new(GuestAddr(0x1000));
        builder.push(IROp::Add {
            dst: 0,
            src1: 1,
            src2: 2,
        });
        builder.set_term(Terminator::Ret);
        let block = builder.build();

        // 第一次翻译，应该缓存未命中
        let result1 = translator.translate_block(&block);
        assert!(result1.is_ok());
        let stats1 = translator.cache_stats().unwrap();
        assert_eq!(stats1.misses, 1);
        assert_eq!(stats1.hits, 0);

        // 第二次翻译相同块，应该缓存命中
        let result2 = translator.translate_block(&block);
        assert!(result2.is_ok());
        let stats2 = translator.cache_stats().unwrap();
        assert_eq!(stats2.misses, 1); // 未命中次数不变
        assert_eq!(stats2.hits, 1); // 命中次数增加
    }

    #[test]
    fn test_optimized_register_allocation() {
        let mut translator = ArchTranslator::with_cache_and_optimization(
            SourceArch::X86_64,
            TargetArch::ARM64,
            Some(10),
            true,
        );
        let mut builder = IRBuilder::new(GuestAddr(0x1000));
        builder.push(IROp::MovImm { dst: 0, imm: 42 });
        builder.push(IROp::Mov { dst: 1, src: 0 });
        builder.push(IROp::Add {
            dst: 2,
            src1: 1,
            src2: 0,
        });
        builder.set_term(Terminator::Ret);
        let block = builder.build();

        // 翻译块
        let result = translator.translate_block(&block);
        assert!(result.is_ok());

        // 检查优化统计
        let opt_stats = translator.get_optimization_stats().unwrap();
        assert!(opt_stats.total_copies > 0); // 应该检测到拷贝
    }

    #[test]
    fn test_memory_alignment_optimization() {
        let mut translator = ArchTranslator::with_cache_optimization_and_memory(
            SourceArch::X86_64,
            TargetArch::ARM64,
            Some(10),
            true,
            true,
        );
        let mut builder = IRBuilder::new(GuestAddr(0x1000));

        // 创建一系列内存访问操作
        builder.push(IROp::Load {
            dst: 1,
            base: 0,
            offset: 0,
            size: 4,
            flags: MemFlags::default(),
        });
        builder.push(IROp::Load {
            dst: 2,
            base: 0,
            offset: 4,
            size: 4,
            flags: MemFlags::default(),
        });
        builder.push(IROp::Load {
            dst: 3,
            base: 0,
            offset: 8,
            size: 4,
            flags: MemFlags::default(),
        });
        builder.push(IROp::Load {
            dst: 4,
            base: 0,
            offset: 12,
            size: 4,
            flags: MemFlags::default(),
        });
        builder.set_term(Terminator::Ret);
        let block = builder.build();

        // 翻译块
        let result = translator.translate_block(&block);
        assert!(result.is_ok());

        // 检查内存优化统计
        let mem_stats = translator.get_memory_optimization_stats().unwrap();
        assert!(mem_stats.alignment_optimizations > 0); // 应该有对齐优化
    }

    #[test]
    fn test_ir_optimization() {
        let mut translator = ArchTranslator::with_cache_optimization_memory_and_ir(
            SourceArch::X86_64,
            TargetArch::ARM64,
            Some(10),
            true,
            true,
            true,
        );
        let mut builder = IRBuilder::new(GuestAddr(0x1000));

        // 创建一系列可优化的IR操作
        builder.push(IROp::MovImm { dst: 1, imm: 10 });
        builder.push(IROp::MovImm { dst: 2, imm: 20 });
        builder.push(IROp::Add {
            dst: 3,
            src1: 1,
            src2: 2,
        }); // 应该被常量折叠
        builder.push(IROp::Mul {
            dst: 4,
            src1: 3,
            src2: 8,
        }); // 应该被强度削弱
        builder.set_term(Terminator::Ret);
        let block = builder.build();

        // 翻译块
        let result = translator.translate_block(&block);
        assert!(result.is_ok());

        // 检查IR优化统计
        let ir_stats = translator.get_ir_optimization_stats().unwrap();
        assert!(ir_stats.constant_folds > 0); // 应该有常量折叠
        assert!(ir_stats.strength_reductions > 0); // 应该有强度削弱
    }

    #[test]
    fn test_target_specific_optimization() {
        let mut translator = ArchTranslator::with_cache_optimization_memory_ir_and_target(
            SourceArch::X86_64,
            TargetArch::ARM64,
            Some(10),
            true,
            true,
            true,
            true,
        );
        let mut builder = IRBuilder::new(GuestAddr(0x1000));

        // 创建一系列可优化的IR操作
        builder.push(IROp::MovImm { dst: 1, imm: 10 });
        builder.push(IROp::MovImm { dst: 2, imm: 20 });
        builder.push(IROp::Add {
            dst: 3,
            src1: 1,
            src2: 2,
        });
        builder.push(IROp::Mul {
            dst: 4,
            src1: 3,
            src2: 8,
        }); // 应该被目标特定优化
        builder.set_term(Terminator::Ret);
        let block = builder.build();

        // 翻译块
        let result = translator.translate_block(&block);
        assert!(result.is_ok());

        // 检查目标特定优化统计
        let _target_stats = translator.get_target_optimization_stats().unwrap();
        // assert!(target_stats.instruction_schedules > 0); // 应该有指令调度
        // assert!(target_stats.pipeline_optimizations > 0); // 应该有流水线优化
    }

    #[test]
    fn test_adaptive_optimization() {
        let mut translator = ArchTranslator::with_all_optimizations(
            SourceArch::X86_64,
            TargetArch::ARM64,
            Some(10),
            true,
            true,
            true,
            true,
            true,
        );
        let mut builder = IRBuilder::new(GuestAddr(0x1000));

        // 创建一系列可优化的IR操作
        builder.push(IROp::MovImm { dst: 1, imm: 10 });
        builder.push(IROp::Add {
            dst: 2,
            src1: 1,
            src2: 1,
        });
        builder.push(IROp::Mul {
            dst: 3,
            src1: 2,
            src2: 8,
        });
        builder.set_term(Terminator::Ret);
        let block = builder.build();

        // 翻译块
        let result = translator.translate_block(&block);
        assert!(result.is_ok());

        // 检查自适应优化统计
        let adaptive_stats = translator.get_adaptive_optimization_stats().unwrap();
        assert!(adaptive_stats.optimization_time_ms > 0); // 应该有优化时间
    }
}
