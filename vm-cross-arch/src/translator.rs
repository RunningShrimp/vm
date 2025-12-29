pub use crate::translation_impl::ArchTranslator;
pub use crate::types::*;

use crate::Architecture;
use crate::encoder::{ArchEncoder, Arm64Encoder, Riscv64Encoder, X86_64Encoder};

/// 优化配置
///
/// 控制跨架构翻译的各种优化标志。
///
/// # 优化类型
/// - **寄存器分配优化**: 优化寄存器映射，减少溢出
/// - **内存优化**: 对齐优化、端序转换优化
/// - **IR优化**: 常量折叠、死代码消除等
/// - **目标特定优化**: 针对目标架构的优化
/// - **自适应优化**: 根据运行时反馈动态调整
///
/// # 示例
/// ```ignore
/// use vm_cross_arch::translator::OptimizationConfig;
///
/// // 启用所有优化
/// let config = OptimizationConfig::all_enabled();
///
/// // 只启用IR和目标优化
/// let config = OptimizationConfig::new()
///     .with_ir_optimization(true)
///     .with_target_optimization(true);
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct OptimizationConfig {
    /// 启用优化的寄存器分配
    pub use_optimized_allocation: bool,
    /// 启用内存对齐和端序优化
    pub use_memory_optimization: bool,
    /// 启用IR级别优化
    pub use_ir_optimization: bool,
    /// 启用目标架构特定优化
    pub use_target_optimization: bool,
    /// 启用自适应优化
    pub use_adaptive_optimization: bool,
}

impl OptimizationConfig {
    /// 创建新的优化配置（所有优化禁用）
    pub fn new() -> Self {
        Self::default()
    }

    /// 启用所有优化
    ///
    /// # 返回
    /// 所有优化标志都设置为true的配置
    pub fn all_enabled() -> Self {
        Self {
            use_optimized_allocation: true,
            use_memory_optimization: true,
            use_ir_optimization: true,
            use_target_optimization: true,
            use_adaptive_optimization: true,
        }
    }

    /// 设置优化的寄存器分配标志
    pub fn with_optimized_allocation(mut self, enabled: bool) -> Self {
        self.use_optimized_allocation = enabled;
        self
    }

    /// 设置内存优化标志
    pub fn with_memory_optimization(mut self, enabled: bool) -> Self {
        self.use_memory_optimization = enabled;
        self
    }

    /// 设置IR优化标志
    pub fn with_ir_optimization(mut self, enabled: bool) -> Self {
        self.use_ir_optimization = enabled;
        self
    }

    /// 设置目标优化标志
    pub fn with_target_optimization(mut self, enabled: bool) -> Self {
        self.use_target_optimization = enabled;
        self
    }

    /// 设置自适应优化标志
    pub fn with_adaptive_optimization(mut self, enabled: bool) -> Self {
        self.use_adaptive_optimization = enabled;
        self
    }
}
// Use types from translation_impl where the ArchTranslator struct is defined
use crate::optimized_register_allocator::OptimizedRegisterMapper;
use crate::smart_register_allocator::SmartRegisterMapper;
use crate::translation_impl::{
    AdaptiveOptimizer, CacheReplacementPolicy, CrossArchBlockCache, Endianness,
    EndiannessConversionStrategy, IROptimizer, MemoryAlignmentOptimizer, TargetSpecificOptimizer,
};
use std::sync::Arc;
use std::sync::Mutex;

impl ArchTranslator {
    /// 创建基本的架构转换器
    ///
    /// # 参数
    /// - `source_arch`: 源架构（x86_64/ARM64/RISC-V64等）
    /// - `target_arch`: 目标架构
    ///
    /// # 示例
    /// ```ignore
    /// use vm_cross_arch::{ArchTranslator, SourceArch, TargetArch};
    ///
    /// let translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
    /// ```
    pub fn new(source_arch: SourceArch, target_arch: TargetArch) -> Self {
        Self::with_cache(source_arch, target_arch, None)
    }

    /// 创建带缓存的转换器
    ///
    /// # 参数
    /// - `source_arch`: 源架构
    /// - `target_arch`: 目标架构
    /// - `cache_size`: 翻译缓存大小（条目数），None表示无限制
    pub fn with_cache(
        source_arch: SourceArch,
        target_arch: TargetArch,
        cache_size: Option<usize>,
    ) -> Self {
        Self::with_cache_and_optimization(source_arch, target_arch, cache_size, false)
    }

    /// 创建带缓存和寄存器优化的转换器
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
            true,
        )
    }

    /// 创建带缓存、寄存器和内存优化的转换器
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
            true,
        )
    }

    /// 创建带缓存、寄存器、内存和IR优化的转换器
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
            true,
        )
    }

    /// 创建全功能转换器（所有优化）
    ///
    /// 这是最高级的构造函数，启用所有优化选项。
    ///
    /// # 参数
    /// - `source_arch`: 源架构
    /// - `target_arch`: 目标架构
    /// - `cache_size`: 翻译缓存大小
    /// - `use_optimized_allocation`: 使用优化的寄存器分配
    /// - `use_memory_optimization`: 使用内存对齐和端序优化
    /// - `use_ir_optimization`: 使用IR级别优化
    /// - `use_target_optimization`: 使用目标架构特定优化
    ///
    /// # 示例
    /// ```ignore
    /// use vm_cross_arch::ArchTranslator;
    ///
    /// // 创建全功能转换器
    /// let translator = ArchTranslator::with_cache_optimization_memory_ir_and_target(
    ///     SourceArch::X86_64,
    ///     TargetArch::ARM64,
    ///     Some(1024),  // 1K条目缓存
    ///     true,  // 寄存器优化
    ///     true,  // 内存优化
    ///     true,  // IR优化
    ///     true,  // 目标优化
    /// );
    /// ```
    pub fn with_cache_optimization_memory_ir_and_target(
        source_arch: SourceArch,
        target_arch: TargetArch,
        cache_size: Option<usize>,
        use_optimized_allocation: bool,
        use_memory_optimization: bool,
        use_ir_optimization: bool,
        use_target_optimization: bool,
    ) -> Self {
        let config = OptimizationConfig {
            use_optimized_allocation,
            use_memory_optimization,
            use_ir_optimization,
            use_target_optimization,
            use_adaptive_optimization: true,
        };
        Self::with_all_optimizations(source_arch, target_arch, cache_size, config)
    }

    pub fn with_all_optimizations(
        source_arch: SourceArch,
        target_arch: TargetArch,
        cache_size: Option<usize>,
        optimization_config: OptimizationConfig,
    ) -> Self {
        let source: Architecture = source_arch.into();
        let target: Architecture = target_arch.into();

        let encoder: Box<dyn ArchEncoder> = match target {
            Architecture::X86_64 => Box::new(X86_64Encoder),
            Architecture::ARM64 => Box::new(Arm64Encoder),
            Architecture::RISCV64 => Box::new(Riscv64Encoder),
        };

        let register_mapper = SmartRegisterMapper::new(target);

        let optimized_mapper = if optimization_config.use_optimized_allocation {
            Some(OptimizedRegisterMapper::new(target))
        } else {
            None
        };

        let block_cache = cache_size.map(|size| {
            Arc::new(Mutex::new(CrossArchBlockCache::new(
                size,
                CacheReplacementPolicy::Lru,
            )))
        });

        let memory_optimizer = if optimization_config.use_memory_optimization {
            let source_endianness = match source {
                Architecture::X86_64 => Endianness::LittleEndian,
                Architecture::ARM64 => Endianness::LittleEndian,
                Architecture::RISCV64 => Endianness::LittleEndian,
            };

            let target_endianness = match target {
                Architecture::X86_64 => Endianness::LittleEndian,
                Architecture::ARM64 => Endianness::LittleEndian,
                Architecture::RISCV64 => Endianness::LittleEndian,
            };

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

        let ir_optimizer = if optimization_config.use_ir_optimization {
            Some(IROptimizer::new())
        } else {
            None
        };

        let target_optimizer = if optimization_config.use_target_optimization {
            Some(TargetSpecificOptimizer::new(target))
        } else {
            None
        };

        let adaptive_optimizer = if optimization_config.use_adaptive_optimization {
            Some(AdaptiveOptimizer::new())
        } else {
            None
        };

        Self {
            source_arch: source,
            target_arch: target,
            encoder,
            register_mapper,
            optimized_mapper,
            block_cache,
            use_optimized_allocation: optimization_config.use_optimized_allocation,
            memory_optimizer,
            ir_optimizer,
            target_optimizer,
            adaptive_optimizer,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_ir::{IRBuilder, IROp};

    #[test]
    fn test_translator_creation() {
        let translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
        assert_eq!(translator.source_arch(), Architecture::X86_64);
        assert_eq!(translator.target_arch(), Architecture::ARM64);
        assert!(translator.cache_stats().is_none());
    }

    #[test]
    fn test_translator_with_cache() {
        let translator =
            ArchTranslator::with_cache(SourceArch::X86_64, TargetArch::ARM64, Some(100));
        assert_eq!(translator.source_arch(), Architecture::X86_64);
        assert_eq!(translator.target_arch(), Architecture::ARM64);
        assert!(translator.cache_stats().is_some());
    }

    #[test]
    fn test_simple_translation() {
        let mut translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
        let mut builder = IRBuilder::new(0x1000);
        builder.push(IROp::Add {
            dst: 0,
            src1: 1,
            src2: 2,
        });
        builder.set_term(vm_ir::Terminator::Ret);
        let block = builder.build();

        let result = translator.translate_block(&block);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cached_translation() {
        let mut translator =
            ArchTranslator::with_cache(SourceArch::X86_64, TargetArch::ARM64, Some(10));
        let mut builder = IRBuilder::new(0x1000);
        builder.push(IROp::Add {
            dst: 0,
            src1: 1,
            src2: 2,
        });
        builder.set_term(vm_ir::Terminator::Ret);
        let block = builder.build();

        let result1 = translator.translate_block(&block);
        assert!(result1.is_ok());
        let stats1 = translator.cache_stats().unwrap();
        assert_eq!(stats1.misses, 1);
        assert_eq!(stats1.hits, 0);

        let result2 = translator.translate_block(&block);
        assert!(result2.is_ok());
        let stats2 = translator.cache_stats().unwrap();
        assert_eq!(stats2.misses, 1);
        assert_eq!(stats2.hits, 1);
    }

    #[test]
    fn test_optimized_register_allocation() {
        let mut translator = ArchTranslator::with_cache_and_optimization(
            SourceArch::X86_64,
            TargetArch::ARM64,
            Some(10),
            true,
        );
        let mut builder = IRBuilder::new(0x1000);
        builder.push(IROp::Const { dst: 0, value: 42 });
        builder.push(IROp::Mov { dst: 1, src: 0 });
        builder.push(IROp::Add {
            dst: 2,
            src1: 1,
            src2: 0,
        });
        builder.set_term(vm_ir::Terminator::Ret);
        let block = builder.build();

        let result = translator.translate_block(&block);
        assert!(result.is_ok());
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
        let mut builder = IRBuilder::new(0x1000);

        builder.push(IROp::Load {
            dst: 1,
            base: 0,
            offset: 0,
            size: 4,
            flags: 0,
        });
        builder.push(IROp::Load {
            dst: 2,
            base: 0,
            offset: 4,
            size: 4,
            flags: 0,
        });
        builder.push(IROp::Load {
            dst: 3,
            base: 0,
            offset: 8,
            size: 4,
            flags: 0,
        });
        builder.push(IROp::Load {
            dst: 4,
            base: 0,
            offset: 12,
            size: 4,
            flags: 0,
        });
        builder.set_term(vm_ir::Terminator::Ret);
        let block = builder.build();

        let result = translator.translate_block(&block);
        assert!(result.is_ok());
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
        let mut builder = IRBuilder::new(0x1000);

        builder.push(IROp::Const { dst: 1, value: 10 });
        builder.push(IROp::Const { dst: 2, value: 20 });
        builder.push(IROp::Add {
            dst: 3,
            src1: 1,
            src2: 2,
        });
        builder.push(IROp::Mul {
            dst: 4,
            src1: 3,
            src2: 8,
        });
        builder.set_term(vm_ir::Terminator::Ret);
        let block = builder.build();

        let result = translator.translate_block(&block);
        assert!(result.is_ok());
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
        let mut builder = IRBuilder::new(0x1000);

        builder.push(IROp::Const { dst: 1, value: 10 });
        builder.push(IROp::Const { dst: 2, value: 20 });
        builder.push(IROp::Add {
            dst: 3,
            src1: 1,
            src2: 2,
        });
        builder.push(IROp::Mul {
            dst: 4,
            src1: 3,
            src2: 8,
        });
        builder.set_term(vm_ir::Terminator::Ret);
        let block = builder.build();

        let result = translator.translate_block(&block);
        assert!(result.is_ok());
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
        let mut builder = IRBuilder::new(0x1000);

        builder.push(IROp::Const { dst: 1, value: 10 });
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
        builder.set_term(vm_ir::Terminator::Ret);
        let block = builder.build();

        let result = translator.translate_block(&block);
        assert!(result.is_ok());
    }
}
