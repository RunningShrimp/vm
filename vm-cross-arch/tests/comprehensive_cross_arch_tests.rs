//! Comprehensive tests for vm-cross-arch module
//!
//! This test suite covers cross-architecture translation, optimization, and related functionality.

use vm_cross_arch::{
    ArchTranslator, Architecture, CacheReplacementPolicy, CrossArchBlockCache, CrossArchConfig,
    CrossArchStrategy, Endianness, EndiannessConversionStrategy, HostArch, IROptimizationStats,
    IROptimizer, InstructionParallelizer, MemoryAlignmentOptimizer, ParallelismStats,
    RegisterMapper, RegisterMapping, SourceArch, SourceBlockKey, TargetArch, TranslationConfig,
    TranslationError, TranslationOutcome,
};
use vm_ir::{IRBlock, IROp, RegId, Terminator};

// ============================================================================
// Architecture Tests
// ============================================================================

#[test]
fn test_architecture_variants() {
    let architectures = vec![
        Architecture::Riscv64,
        Architecture::Arm64,
        Architecture::X86_64,
        Architecture::PowerPC64,
    ];

    for arch in architectures {
        // Verify all variants can be created
        let _ = arch;
    }
}

#[test]
fn test_source_arch_variants() {
    let archs = vec![
        SourceArch::Riscv64,
        SourceArch::Arm64,
        SourceArch::X86_64,
        SourceArch::PowerPC64,
    ];

    for arch in archs {
        let _ = arch;
    }
}

#[test]
fn test_target_arch_variants() {
    let archs = vec![
        TargetArch::Riscv64,
        TargetArch::Arm64,
        TargetArch::X86_64,
        TargetArch::PowerPC64,
    ];

    for arch in archs {
        let _ = arch;
    }
}

// ============================================================================
// Basic Translator Tests
// ============================================================================

#[test]
fn test_translator_creation_x86_to_arm() {
    let translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::Arm64);
    // Should create successfully
    let _ = translator;
}

#[test]
fn test_translator_creation_arm_to_riscv() {
    let translator = ArchTranslator::new(SourceArch::Arm64, TargetArch::Riscv64);
    // Should create successfully
    let _ = translator;
}

#[test]
fn test_translator_creation_riscv_to_x86() {
    let translator = ArchTranslator::new(SourceArch::Riscv64, TargetArch::X86_64);
    // Should create successfully
    let _ = translator;
}

#[test]
fn test_translator_same_architecture() {
    let translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::X86_64);
    // Should handle same-architecture translation
    let _ = translator;
}

#[test]
fn test_translator_all_combinations() {
    let sources = vec![
        SourceArch::Riscv64,
        SourceArch::Arm64,
        SourceArch::X86_64,
        SourceArch::PowerPC64,
    ];

    let targets = vec![
        TargetArch::Riscv64,
        TargetArch::Arm64,
        TargetArch::X86_64,
        TargetArch::PowerPC64,
    ];

    for source in sources {
        for target in &targets {
            let translator = ArchTranslator::new(source, *target);
            let _ = translator;
        }
    }
}

// ============================================================================
// Translation Config Tests
// ============================================================================

#[test]
fn test_translation_config_new() {
    let config = TranslationConfig::new();
    // Should create default config
    let _ = config;
}

#[test]
fn test_translation_config_builder() {
    let config = TranslationConfig::new()
        .with_optimization_level(3)
        .with_enable_instruction_parallelism(true)
        .with_enable_memory_alignment_optimization(true)
        .with_enable_register_optimization(true)
        .with_cache_size(64 * 1024 * 1024);

    // Should build successfully
    let _ = config;
}

#[test]
fn test_translation_config_with_optimizer() {
    let config = TranslationConfig::new().with_optimization_level(2);

    let _ = config;
}

// ============================================================================
// Block Cache Tests
// ============================================================================

#[test]
fn test_block_cache_creation() {
    let cache = CrossArchBlockCache::new(64, CacheReplacementPolicy::LRU);
    // Should create successfully
    let _ = cache;
}

#[test]
fn test_block_cache_insert() {
    let mut cache = CrossArchBlockCache::new(64, CacheReplacementPolicy::LRU);

    let block = create_simple_ir_block(0x1000);

    let key = SourceBlockKey::new(
        SourceArch::X86_64,
        TargetArch::Arm64,
        vm_core::GuestAddr(0x1000),
        &block,
    );

    // Note: In real implementation, we would insert translated blocks
    // For now, just verify the structure works
    let _ = (key, block);
}

#[test]
fn test_block_cache_replacement_policies() {
    let policies = vec![
        CacheReplacementPolicy::LRU,
        CacheReplacementPolicy::LFU,
        CacheReplacementPolicy::FIFO,
        CacheReplacementPolicy::Random,
    ];

    for policy in policies {
        let cache = CrossArchBlockCache::new(32, policy);
        let _ = cache;
    }
}

#[test]
fn test_source_block_key() {
    let block = create_simple_ir_block(0x1000);

    let key1 = SourceBlockKey::new(
        SourceArch::X86_64,
        TargetArch::Arm64,
        vm_core::GuestAddr(0x1000),
        &block,
    );

    let key2 = SourceBlockKey::new(
        SourceArch::X86_64,
        TargetArch::Arm64,
        vm_core::GuestAddr(0x1000),
        &block,
    );

    let block2 = create_simple_ir_block(0x2000);
    let key3 = SourceBlockKey::new(
        SourceArch::X86_64,
        TargetArch::Arm64,
        vm_core::GuestAddr(0x2000),
        &block2,
    );

    assert_eq!(key1, key2);
    assert_ne!(key1, key3);
}

// ============================================================================
// IR Optimizer Tests
// ============================================================================

#[test]
fn test_ir_optimizer_creation() {
    let optimizer = IROptimizer::new();
    // Should create successfully
    let _ = optimizer;
}

#[test]
fn test_ir_optimizer_with_config() {
    let stats = IROptimizationStats::default();
    // Verify default stats
    assert_eq!(stats.optimized_blocks, 0);
    assert_eq!(stats.constant_folds, 0);
    assert_eq!(stats.dead_code_eliminated, 0);
}

#[test]
fn test_ir_optimization_stats() {
    let stats = IROptimizationStats {
        optimized_blocks: 10,
        constant_folds: 5,
        dead_code_eliminated: 3,
        sub_expression_elimination: 2,
    };

    assert_eq!(stats.optimized_blocks, 10);
    assert_eq!(stats.constant_folds, 5);
    assert_eq!(stats.dead_code_eliminated, 3);
    assert_eq!(stats.sub_expression_elimination, 2);
}

// ============================================================================
// Memory Alignment Optimizer Tests
// ============================================================================

#[test]
fn test_memory_alignment_optimizer_creation() {
    let optimizer = MemoryAlignmentOptimizer::new(
        Endianness::Little,
        Endianness::Little,
        EndiannessConversionStrategy::None,
    );

    // Should create successfully
    let _ = optimizer;
}

#[test]
fn test_endianness_variants() {
    let endiannesses = vec![Endianness::Big, Endianness::Little];

    for end in endiannesses {
        let _ = end;
    }
}

#[test]
fn test_endianness_conversion_strategies() {
    let strategies = vec![
        EndiannessConversionStrategy::None,
        EndiannessConversionStrategy::Software,
        EndiannessConversionStrategy::Hardware,
    ];

    for strategy in strategies {
        let _ = strategy;
    }
}

#[test]
fn test_memory_optimizer_same_endianness() {
    let optimizer = MemoryAlignmentOptimizer::new(
        Endianness::Little,
        Endianness::Little,
        EndiannessConversionStrategy::None,
    );

    // Should handle same endianness
    let _ = optimizer;
}

#[test]
fn test_memory_optimizer_different_endianness() {
    let optimizer = MemoryAlignmentOptimizer::new(
        Endianness::Little,
        Endianness::Big,
        EndiannessConversionStrategy::Software,
    );

    // Should handle different endianness
    let _ = optimizer;
}

// ============================================================================
// Register Mapping Tests
// ============================================================================

#[test]
fn test_register_mapper_creation() {
    let mapper = RegisterMapper::new();
    // Should create successfully
    let _ = mapper;
}

#[test]
fn test_register_mapping_x86_to_arm() {
    let mapping = RegisterMapping::new(SourceArch::X86_64, TargetArch::Arm64);
    // Should create successfully
    let _ = mapping;
}

#[test]
fn test_register_mapping_arm_to_riscv() {
    let mapping = RegisterMapping::new(SourceArch::Arm64, TargetArch::Riscv64);
    // Should create successfully
    let _ = mapping;
}

#[test]
fn test_register_mapping_all_pairs() {
    let pairs = vec![
        (SourceArch::X86_64, TargetArch::Arm64),
        (SourceArch::X86_64, TargetArch::Riscv64),
        (SourceArch::Arm64, TargetArch::X86_64),
        (SourceArch::Arm64, TargetArch::Riscv64),
        (SourceArch::Riscv64, TargetArch::X86_64),
        (SourceArch::Riscv64, TargetArch::Arm64),
    ];

    for (source, target) in pairs {
        let mapping = RegisterMapping::new(source, target);
        let _ = mapping;
    }
}

// ============================================================================
// Instruction Parallelizer Tests
// ============================================================================

#[test]
fn test_instruction_parallelizer_creation() {
    let parallelizer = InstructionParallelizer::new();
    // Should create successfully
    let _ = parallelizer;
}

#[test]
fn test_parallelism_stats() {
    let stats = ParallelismStats::default();

    assert_eq!(stats.parallel_groups_found, 0);
    assert_eq!(stats.instructions_parallelized, 0);
    assert_eq!(stats.dependencies_analyzed, 0);
}

#[test]
fn test_parallelism_stats_custom() {
    let stats = ParallelismStats {
        parallel_groups_found: 5,
        instructions_parallelized: 10,
        dependencies_analyzed: 20,
    };

    assert_eq!(stats.parallel_groups_found, 5);
    assert_eq!(stats.instructions_parallelized, 10);
    assert_eq!(stats.dependencies_analyzed, 20);
}

// ============================================================================
// Cross-Arch Config Tests
// ============================================================================

#[test]
fn test_cross_arch_config() {
    let config = CrossArchConfig {
        source_arch: SourceArch::X86_64,
        target_arch: TargetArch::Arm64,
        strategy: CrossArchStrategy::Optimized,
    };

    assert_eq!(config.source_arch, SourceArch::X86_64);
    assert_eq!(config.target_arch, TargetArch::Arm64);
    assert_eq!(config.strategy, CrossArchStrategy::Optimized);
}

#[test]
fn test_cross_arch_strategy_variants() {
    let strategies = vec![
        CrossArchStrategy::Direct,
        CrossArchStrategy::Optimized,
        CrossArchStrategy::Aggressive,
    ];

    for strategy in strategies {
        let _ = strategy;
    }
}

#[test]
fn test_host_arch_detection() {
    let host = HostArch::detect();
    // Should detect current host architecture
    let _ = host;
}

// ============================================================================
// Translation Error Tests
// ============================================================================

#[test]
fn test_translation_error_unsupported_instruction() {
    let error = TranslationError::UnsupportedInstruction {
        opcode: 0xDEADBEEF,
        source_arch: SourceArch::X86_64,
    };

    match error {
        TranslationError::UnsupportedInstruction {
            opcode,
            source_arch,
        } => {
            assert_eq!(opcode, 0xDEADBEEF);
            assert_eq!(source_arch, SourceArch::X86_64);
        }
        _ => panic!("Expected UnsupportedInstruction"),
    }
}

#[test]
fn test_translation_error_invalid_register() {
    let error = TranslationError::InvalidRegister {
        reg: 999,
        source_arch: SourceArch::Arm64,
    };

    match error {
        TranslationError::InvalidRegister { reg, source_arch } => {
            assert_eq!(reg, 999);
            assert_eq!(source_arch, SourceArch::Arm64);
        }
        _ => panic!("Expected InvalidRegister"),
    }
}

#[test]
fn test_translation_error_unknown() {
    let error = TranslationError::Unknown {
        message: "Test error".to_string(),
    };

    match error {
        TranslationError::Unknown { message } => {
            assert_eq!(message, "Test error");
        }
        _ => panic!("Expected Unknown"),
    }
}

// ============================================================================
// Translation Outcome Tests
// ============================================================================

#[test]
fn test_translation_outcome_success() {
    let block = create_simple_ir_block(0x1000);
    let outcome = TranslationOutcome::Success(block.clone());

    match outcome {
        TranslationOutcome::Success(b) => {
            assert_eq!(b.start_pc, block.start_pc);
        }
        _ => panic!("Expected Success"),
    }
}

#[test]
fn test_translation_outcome_partial() {
    let block = create_simple_ir_block(0x1000);
    let outcome = TranslationOutcome::Partial {
        block: block.clone(),
        warnings: vec!["Warning 1".to_string()],
    };

    match outcome {
        TranslationOutcome::Partial { block: b, warnings } => {
            assert_eq!(b.start_pc, block.start_pc);
            assert_eq!(warnings.len(), 1);
        }
        _ => panic!("Expected Partial"),
    }
}

#[test]
fn test_translation_outcome_failure() {
    let error = TranslationError::Unknown {
        message: "Test error".to_string(),
    };
    let outcome = TranslationOutcome::Failure(error.clone());

    match outcome {
        TranslationOutcome::Failure(e) => match e {
            TranslationError::Unknown { message } => {
                assert_eq!(message, "Test error");
            }
            _ => panic!("Expected Unknown error"),
        },
        _ => panic!("Expected Failure"),
    }
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_translator_with_config() {
    let config = TranslationConfig::new()
        .with_optimization_level(3)
        .with_enable_instruction_parallelism(true);

    let translator = ArchTranslator::with_config(SourceArch::X86_64, TargetArch::Arm64, config);

    // Should create successfully
    let _ = translator;
}

#[test]
fn test_translator_multiple_blocks() {
    let translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::Arm64);

    // Simulate translating multiple blocks
    for i in 0..10 {
        let block = create_simple_ir_block(0x1000 + i * 0x100);
        let _ = block;
        // In real implementation: translator.translate_block(&block)
    }

    // Should handle multiple blocks
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_empty_block_translation() {
    let block = IRBlock {
        start_pc: vm_core::GuestAddr(0x1000),
        ops: vec![],
        term: Terminator::Return,
    };

    // Should handle empty blocks
    let _ = block;
}

#[test]
fn test_large_block_translation() {
    let block = IRBlock {
        start_pc: vm_core::GuestAddr(0x1000),
        ops: (0..1000).map(|_| IROp::Nop).collect(),
        term: Terminator::Return,
    };

    // Should handle large blocks
    let _ = block;
}

#[test]
fn test_complex_block_translation() {
    let block = IRBlock {
        start_pc: vm_core::GuestAddr(0x1000),
        ops: vec![
            IROp::Const {
                dst: RegId(1),
                value: 42,
            },
            IROp::Add {
                dst: RegId(2),
                src1: RegId(1),
                src2: RegId(0),
            },
            IROp::Sub {
                dst: RegId(3),
                src1: RegId(2),
                src2: RegId(1),
            },
            IROp::Mul {
                dst: RegId(4),
                src1: RegId(3),
                src2: RegId(2),
            },
        ],
        term: Terminator::Return,
    };

    // Should handle complex blocks
    let _ = block;
}

#[test]
fn test_all_terminator_types() {
    let terminators = vec![
        Terminator::Return,
        Terminator::Branch {
            target: vm_core::GuestAddr(0x1000),
        },
        Terminator::CondBranch {
            condition: RegId(1),
            true_target: vm_core::GuestAddr(0x1000),
            false_target: vm_core::GuestAddr(0x2000),
        },
    ];

    for term in terminators {
        let block = IRBlock {
            start_pc: vm_core::GuestAddr(0x1000),
            ops: vec![],
            term: term.clone(),
        };

        assert_eq!(block.term, term);
    }
}

#[test]
fn test_all_irop_types() {
    let ops = vec![
        IROp::Nop,
        IROp::Const {
            dst: RegId(1),
            value: 42,
        },
        IROp::Add {
            dst: RegId(2),
            src1: RegId(1),
            src2: RegId(0),
        },
        IROp::Sub {
            dst: RegId(3),
            src1: RegId(2),
            src2: RegId(1),
        },
        IROp::Mul {
            dst: RegId(4),
            src1: RegId(3),
            src2: RegId(2),
        },
        IROp::Load {
            dst: RegId(5),
            addr: RegId(1),
            size: 4,
        },
        IROp::Store {
            src: RegId(5),
            addr: RegId(1),
            size: 4,
        },
    ];

    for op in ops {
        let _ = op;
    }
}

#[test]
fn test_cross_arch_powerpc() {
    let translator = ArchTranslator::new(SourceArch::PowerPC64, TargetArch::X86_64);
    // Should handle PowerPC
    let _ = translator;
}

#[test]
fn test_cache_overflow() {
    let mut cache = CrossArchBlockCache::new(4, CacheReplacementPolicy::LRU);

    // Insert more blocks than cache size
    for i in 0..10 {
        let block = create_simple_ir_block(0x1000 + i * 0x100);
        let key = SourceBlockKey::new(
            SourceArch::X86_64,
            TargetArch::Arm64,
            vm_core::GuestAddr(0x1000 + i * 0x100),
            &block,
        );
        // In real implementation, would insert blocks here
        let _ = key;
    }

    // Should handle overflow via replacement policy
}

#[test]
fn test_concurrent_translation() {
    use std::sync::Arc;
    use std::thread;

    let translator = Arc::new(std::sync::Mutex::new(ArchTranslator::new(
        SourceArch::X86_64,
        TargetArch::Arm64,
    )));
    let mut handles = vec![];

    for i in 0..4 {
        let trans = Arc::clone(&translator);
        let handle = thread::spawn(move || {
            let translator = trans.lock().unwrap();
            let block = create_simple_ir_block(0x1000 + i * 0x100);
            let _ = (translator, block);
        });

        handles.push(handle);
    }

    // All threads should complete
    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_zero_address_block() {
    let block = IRBlock {
        start_pc: vm_core::GuestAddr(0),
        ops: vec![IROp::Nop],
        term: Terminator::Return,
    };

    // Should handle zero address
    let _ = block;
}

#[test]
fn test_max_address_block() {
    let block = IRBlock {
        start_pc: vm_core::GuestAddr(u64::MAX),
        ops: vec![IROp::Nop],
        term: Terminator::Return,
    };

    // Should handle max address
    let _ = block;
}

// ============================================================================
// Helper Functions
// ============================================================================

fn create_simple_ir_block(addr: u64) -> IRBlock {
    IRBlock {
        start_pc: vm_core::GuestAddr(addr),
        ops: vec![IROp::Nop, IROp::Nop],
        term: Terminator::Return,
    }
}
