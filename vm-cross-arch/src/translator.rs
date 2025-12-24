mod types;
mod translation_impl;

pub use types::*;
pub use translation_impl::{TargetInstruction, TranslationResult, TranslationStats, ArchTranslator};

use vm_encoder::{Architecture, ArchEncoder, X86_64Encoder, Arm64Encoder, Riscv64Encoder};
use vm_register::SmartRegisterMapper;
use vm_optimizer::OptimizedRegisterMapper;
use std::sync::Arc;
use std::sync::Mutex;

impl ArchTranslator {
    pub fn new(source_arch: SourceArch, target_arch: TargetArch) -> Self {
        Self::with_cache(source_arch, target_arch, None)
    }

    pub fn with_cache(
        source_arch: SourceArch, 
        target_arch: TargetArch,
        cache_size: Option<usize>
    ) -> Self {
        Self::with_cache_and_optimization(source_arch, target_arch, cache_size, false)
    }

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
            true,
        )
    }

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

        let encoder: Box<dyn ArchEncoder> = match target {
            Architecture::X86_64 => Box::new(X86_64Encoder),
            Architecture::ARM64 => Box::new(Arm64Encoder),
            Architecture::RISCV64 => Box::new(Riscv64Encoder),
        };

        let register_mapper = SmartRegisterMapper::new(target);
        
        let optimized_mapper = if use_optimized_allocation {
            Some(OptimizedRegisterMapper::new(target))
        } else {
            None
        };
        
        let block_cache = cache_size.map(|size| {
            Arc::new(Mutex::new(translation_impl::CrossArchBlockCache::new(
                size, 
                translation_impl::CacheReplacementPolicy::LRU
            )))
        });
        
        let memory_optimizer = if use_memory_optimization {
            let source_endianness = match source {
                Architecture::X86_64 => translation_impl::Endianness::LittleEndian,
                Architecture::ARM64 => translation_impl::Endianness::LittleEndian,
                Architecture::RISCV64 => translation_impl::Endianness::LittleEndian,
            };
            
            let target_endianness = match target {
                Architecture::X86_64 => translation_impl::Endianness::LittleEndian,
                Architecture::ARM64 => translation_impl::Endianness::LittleEndian,
                Architecture::RISCV64 => translation_impl::Endianness::LittleEndian,
            };
            
            let conversion_strategy = if source_endianness == target_endianness {
                translation_impl::EndiannessConversionStrategy::None
            } else {
                translation_impl::EndiannessConversionStrategy::Hybrid
            };
            
            Some(translation_impl::MemoryAlignmentOptimizer::new(
                source_endianness,
                target_endianness,
                conversion_strategy,
            ))
        } else {
            None
        };
        
        let ir_optimizer = if use_ir_optimization {
            Some(translation_impl::IROptimizer::new())
        } else {
            None
        };
        
        let target_optimizer = if use_target_optimization {
            Some(translation_impl::TargetSpecificOptimizer::new(target))
        } else {
            None
        };
        
        let adaptive_optimizer = if use_adaptive_optimization {
            Some(translation_impl::AdaptiveOptimizer::new())
        } else {
            None
        };

        Self {
            source_arch,
            target_arch,
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
        let translator = ArchTranslator::with_cache(SourceArch::X86_64, TargetArch::ARM64, Some(100));
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
        let mut translator = ArchTranslator::with_cache(SourceArch::X86_64, TargetArch::ARM64, Some(10));
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
            true
        );
        let mut builder = IRBuilder::new(0x1000);
        builder.push(IROp::Const { dst: 0, value: 42 });
        builder.push(IROp::Mov { dst: 1, src: 0 });
        builder.push(IROp::Add { dst: 2, src1: 1, src2: 0 });
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
            true
        );
        let mut builder = IRBuilder::new(0x1000);
        
        builder.push(IROp::Load { dst: 1, base: 0, offset: 0, size: 4, flags: 0 });
        builder.push(IROp::Load { dst: 2, base: 0, offset: 4, size: 4, flags: 0 });
        builder.push(IROp::Load { dst: 3, base: 0, offset: 8, size: 4, flags: 0 });
        builder.push(IROp::Load { dst: 4, base: 0, offset: 12, size: 4, flags: 0 });
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
            true
        );
        let mut builder = IRBuilder::new(0x1000);
        
        builder.push(IROp::Const { dst: 1, value: 10 });
        builder.push(IROp::Const { dst: 2, value: 20 });
        builder.push(IROp::Add { dst: 3, src1: 1, src2: 2 });
        builder.push(IROp::Mul { dst: 4, src1: 3, src2: 8 });
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
            true
        );
        let mut builder = IRBuilder::new(0x1000);
        
        builder.push(IROp::Const { dst: 1, value: 10 });
        builder.push(IROp::Const { dst: 2, value: 20 });
        builder.push(IROp::Add { dst: 3, src1: 1, src2: 2 });
        builder.push(IROp::Mul { dst: 4, src1: 3, src2: 8 });
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
            true
        );
        let mut builder = IRBuilder::new(0x1000);
        
        builder.push(IROp::Const { dst: 1, value: 10 });
        builder.push(IROp::Add { dst: 2, src1: 1, src2: 1 });
        builder.push(IROp::Mul { dst: 3, src1: 2, src2: 8 });
        builder.set_term(vm_ir::Terminator::Ret);
        let block = builder.build();

        let result = translator.translate_block(&block);
        assert!(result.is_ok());
    }
}
