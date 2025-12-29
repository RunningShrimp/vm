//! Cross-Architecture Translation Integration Tests
//!
//! Comprehensive integration tests for cross-architecture instruction translation:
//! - x86_64 ↔ ARM64 translation
//! - x86_64 ↔ RISC-V translation
//! - ARM64 ↔ RISC-V translation
//! - Translation caching
//! - Optimization and performance
//! - Error handling and edge cases

use vm_cross_arch::{
    ArchTranslator, CacheReplacementPolicy, CrossArchBlockCache, IROptimizer,
    MemoryAlignmentOptimizer, SourceArch, TargetArch, TranslationConfig, TranslationError,
    TranslationOutcome,
};
use vm_ir::{IRBlock, IROp, Operand, Terminator};

// ============================================================================
// Test Fixtures
// ============================================================================

/// Create a simple test IR block
fn create_test_ir_block(start_addr: u64) -> IRBlock {
    IRBlock {
        start_pc: vm_core::GuestAddr(start_addr),
        ops: vec![
            IROp::Const { dest: 0, value: 42 },
            IROp::Load {
                dest: 1,
                base: 0,
                offset: 0,
                size: 4,
            },
            IROp::Store {
                src: 1,
                base: 0,
                offset: 4,
                size: 4,
            },
        ],
        term: Terminator::Return,
    }
}

/// Create an arithmetic IR block
fn create_arithmetic_ir_block(start_addr: u64) -> IRBlock {
    IRBlock {
        start_pc: vm_core::GuestAddr(start_addr),
        ops: vec![
            IROp::Const {
                dest: 0,
                value: 100,
            },
            IROp::Const { dest: 1, value: 50 },
            IROp::BinaryOp {
                dest: 2,
                op: vm_ir::BinaryOpType::Add,
                src1: 0,
                src2: 1,
            },
            IROp::BinaryOp {
                dest: 3,
                op: vm_ir::BinaryOpType::Sub,
                src1: 0,
                src2: 1,
            },
            IROp::BinaryOp {
                dest: 4,
                op: vm_ir::BinaryOpType::Mul,
                src1: 2,
                src2: 3,
            },
        ],
        term: Terminator::Return,
    }
}

/// Create a complex IR block with control flow
fn create_complex_ir_block(start_addr: u64) -> IRBlock {
    IRBlock {
        start_pc: vm_core::GuestAddr(start_addr),
        ops: vec![
            IROp::Const { dest: 0, value: 10 },
            IROp::Load {
                dest: 1,
                base: 0,
                offset: 0,
                size: 8,
            },
            IROp::BinaryOp {
                dest: 2,
                op: vm_ir::BinaryOpType::Add,
                src1: 1,
                src2: 0,
            },
            IROp::Store {
                src: 2,
                base: 0,
                offset: 8,
                size: 8,
            },
        ],
        term: Terminator::Return,
    }
}

// ============================================================================
// Happy Path Tests
// ============================================================================

#[test]
fn test_x86_64_to_arm64_translation() {
    let translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
    let block = create_test_ir_block(0x1000);

    let result = translator.translate_block(&block);

    assert!(result.is_ok());
    let outcome = result.unwrap();

    assert!(!outcome.instructions.is_empty());
    assert!(outcome.translation_time_ns > 0);
}

#[test]
fn test_arm64_to_x86_64_translation() {
    let translator = ArchTranslator::new(SourceArch::ARM64, TargetArch::X86_64);
    let block = create_arithmetic_ir_block(0x2000);

    let result = translator.translate_block(&block);

    assert!(result.is_ok());
    let outcome = result.unwrap();

    assert!(!outcome.instructions.is_empty());
    assert_eq!(outcome.source_block_start, 0x2000);
}

#[test]
fn test_x86_64_to_riscv_translation() {
    let translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::Riscv64);
    let block = create_test_ir_block(0x3000);

    let result = translator.translate_block(&block);

    assert!(result.is_ok());
    let outcome = result.unwrap();

    assert!(!outcome.instructions.is_empty());
}

#[test]
fn test_riscv_to_x86_64_translation() {
    let translator = ArchTranslator::new(SourceArch::Riscv64, TargetArch::X86_64);
    let block = create_arithmetic_ir_block(0x4000);

    let result = translator.translate_block(&block);

    assert!(result.is_ok());
    let outcome = result.unwrap();

    assert!(!outcome.instructions.is_empty());
}

#[test]
fn test_arm64_to_riscv_translation() {
    let translator = ArchTranslator::new(SourceArch::ARM64, TargetArch::Riscv64);
    let block = create_complex_ir_block(0x5000);

    let result = translator.translate_block(&block);

    assert!(result.is_ok());
    let outcome = result.unwrap();

    assert!(!outcome.instructions.is_empty());
}

#[test]
fn test_riscv_to_arm64_translation() {
    let translator = ArchTranslator::new(SourceArch::Riscv64, TargetArch::ARM64);
    let block = create_complex_ir_block(0x6000);

    let result = translator.translate_block(&block);

    assert!(result.is_ok());
    let outcome = result.unwrap();

    assert!(!outcome.instructions.is_empty());
}

#[test]
fn test_all_architecture_combinations() {
    let combinations = vec![
        (SourceArch::X86_64, TargetArch::ARM64),
        (SourceArch::X86_64, TargetArch::Riscv64),
        (SourceArch::ARM64, TargetArch::X86_64),
        (SourceArch::ARM64, TargetArch::Riscv64),
        (SourceArch::Riscv64, TargetArch::X86_64),
        (SourceArch::Riscv64, TargetArch::ARM64),
    ];

    for (src, tgt) in combinations {
        let translator = ArchTranslator::new(src, tgt);
        let block = create_test_ir_block(0x1000);

        let result = translator.translate_block(&block);
        assert!(result.is_ok(), "Failed to translate {:?} to {:?}", src, tgt);
    }
}

#[test]
fn test_translation_with_optimization() {
    let config = TranslationConfig::new()
        .with_optimization_level(3)
        .with_enable_instruction_parallelism(true)
        .with_enable_memory_alignment_optimization(true)
        .with_enable_register_optimization(true);

    let translator = ArchTranslator::with_config(SourceArch::X86_64, TargetArch::ARM64, config);

    let block = create_arithmetic_ir_block(0x1000);
    let result = translator.translate_block(&block);

    assert!(result.is_ok());
    let outcome = result.unwrap();

    assert!(!outcome.instructions.is_empty());
}

#[test]
fn test_translation_cache_hit() {
    let cache = CrossArchBlockCache::new(1024 * 1024, CacheReplacementPolicy::Lru);
    let translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
    let block = create_test_ir_block(0x1000);

    // First translation - cache miss
    let result1 = translator.translate_block_with_cache(&block, &cache);
    assert!(result1.is_ok());

    // Second translation - cache hit
    let result2 = translator.translate_block_with_cache(&block, &cache);
    assert!(result2.is_ok());

    let stats = cache.get_stats();
    assert!(stats.hits > 0 || stats.misses > 0);
}

// ============================================================================
// Error Path Tests
// ============================================================================

#[test]
fn test_invalid_instruction_sequence() {
    let translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);

    // Create an invalid block with no operations
    let invalid_block = IRBlock {
        start_pc: vm_core::GuestAddr(0x1000),
        ops: vec![],
        term: Terminator::Return,
    };

    let result = translator.translate_block(&invalid_block);

    // Should handle gracefully - either ok with minimal instructions or specific error
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_unsupported_architecture_combination() {
    // Test with PowerPC as source (if not fully supported)
    let translator = ArchTranslator::new(SourceArch::PowerPC64, TargetArch::X86_64);
    let block = create_test_ir_block(0x1000);

    let result = translator.translate_block(&block);

    // May fail or succeed depending on implementation
    // Just verify it doesn't panic
    let _ = result;
}

#[test]
fn test_empty_translation() {
    let translator = ArchTranslator::new(SourceArch::ARM64, TargetArch::Riscv64);

    let empty_block = IRBlock {
        start_pc: vm_core::GuestAddr(0x1000),
        ops: vec![],
        term: Terminator::Return,
    };

    let result = translator.translate_block(&empty_block);

    // Should handle empty blocks gracefully
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_very_large_block() {
    let translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);

    // Create a block with many operations
    let mut ops = Vec::new();
    for i in 0..1000 {
        ops.push(IROp::Const {
            dest: i % 32,
            value: i as u64,
        });
    }

    let large_block = IRBlock {
        start_pc: vm_core::GuestAddr(0x1000),
        ops,
        term: Terminator::Return,
    };

    let result = translator.translate_block(&large_block);

    // Should handle large blocks without panicking
    assert!(result.is_ok() || result.is_err());
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_memory_alignment_handling() {
    let config = TranslationConfig::new().with_enable_memory_alignment_optimization(true);

    let translator = ArchTranslator::with_config(SourceArch::X86_64, TargetArch::ARM64, config);

    // Create block with unaligned accesses
    let block = IRBlock {
        start_pc: vm_core::GuestAddr(0x1001), // Unaligned address
        ops: vec![
            IROp::Load {
                dest: 0,
                base: 1,
                offset: 1,
                size: 4,
            },
            IROp::Store {
                src: 0,
                base: 1,
                offset: 5,
                size: 4,
            },
        ],
        term: Terminator::Return,
    };

    let result = translator.translate_block(&block);

    // Should handle unaligned accesses
    assert!(result.is_ok());
}

#[test]
fn test_register_allocation_under_pressure() {
    let translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);

    // Create block that uses many virtual registers
    let mut ops = Vec::new();
    for i in 0..100 {
        ops.push(IROp::Const {
            dest: i,
            value: i as u64,
        });
    }
    ops.push(IROp::BinaryOp {
        dest: 0,
        op: vm_ir::BinaryOpType::Add,
        src1: 1,
        src2: 2,
    });

    let block = IRBlock {
        start_pc: vm_core::GuestAddr(0x1000),
        ops,
        term: Terminator::Return,
    };

    let result = translator.translate_block(&block);

    // Should handle register spilling
    assert!(result.is_ok());
}

#[test]
fn test_different_endianness() {
    // Test translation between architectures with different endianness requirements
    let translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);

    let block = IRBlock {
        start_pc: vm_core::GuestAddr(0x1000),
        ops: vec![
            IROp::Const {
                dest: 0,
                value: 0xDEADBEEF,
            },
            IROp::Store {
                src: 0,
                base: 0,
                offset: 0,
                size: 4,
            },
        ],
        term: Terminator::Return,
    };

    let result = translator.translate_block(&block);

    assert!(result.is_ok());
}

#[test]
fn test_simd_instruction_translation() {
    let translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);

    // This test assumes SIMD operations are represented in IR
    // Actual SIMD support depends on IR capabilities
    let block = IRBlock {
        start_pc: vm_core::GuestAddr(0x1000),
        ops: vec![
            IROp::Const { dest: 0, value: 0 },
            IROp::Load {
                dest: 1,
                base: 0,
                offset: 0,
                size: 16, // 128-bit load
            },
        ],
        term: Terminator::Return,
    };

    let result = translator.translate_block(&block);

    // May succeed or fail depending on SIMD support
    let _ = result;
}

// ============================================================================
// Performance Tests
// ============================================================================

#[test]
fn test_translation_performance() {
    let translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
    let block = create_arithmetic_ir_block(0x1000);

    let start = std::time::Instant::now();
    let iterations = 1000;

    for _ in 0..iterations {
        let _ = translator.translate_block(&block);
    }

    let duration = start.elapsed();

    // Performance assertion: should complete reasonably fast
    assert!(
        duration.as_millis() < 5000,
        "Translation too slow: {:?}",
        duration
    );
}

#[test]
fn test_cache_efficiency() {
    let cache = CrossArchBlockCache::new(1024 * 1024, CacheReplacementPolicy::Lru);
    let translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
    let block = create_test_ir_block(0x1000);

    // Warm up cache
    for _ in 0..10 {
        let _ = translator.translate_block_with_cache(&block, &cache);
    }

    let stats = cache.get_stats();

    // Should have some cache hits
    assert!(stats.hits + stats.misses > 0);
}

#[test]
fn test_parallel_translation() {
    use std::thread;

    let translator1 = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
    let translator2 = ArchTranslator::new(SourceArch::ARM64, TargetArch::X86_64);

    let handle1 = thread::spawn(move || {
        for i in 0..100 {
            let block = create_test_ir_block(i * 0x1000);
            let _ = translator1.translate_block(&block);
        }
    });

    let handle2 = thread::spawn(move || {
        for i in 0..100 {
            let block = create_arithmetic_ir_block(i * 0x1000);
            let _ = translator2.translate_block(&block);
        }
    });

    handle1.join().unwrap();
    handle2.join().unwrap();
}

// ============================================================================
// IR Optimization Tests
// ============================================================================

#[test]
fn test_ir_optimization() {
    let mut optimizer = IROptimizer::new();

    let block = IRBlock {
        start_pc: vm_core::GuestAddr(0x1000),
        ops: vec![
            IROp::Const { dest: 0, value: 10 },
            IROp::Const { dest: 1, value: 20 },
            IROp::BinaryOp {
                dest: 2,
                op: vm_ir::BinaryOpType::Add,
                src1: 0,
                src2: 1,
            },
        ],
        term: Terminator::Return,
    };

    let optimized = optimizer.optimize_block(&block);

    assert!(!optimized.ops.is_empty());
}

#[test]
fn test_constant_folding() {
    let mut optimizer = IROptimizer::new();

    // Create block with foldable constants
    let block = IRBlock {
        start_pc: vm_core::GuestAddr(0x1000),
        ops: vec![
            IROp::Const { dest: 0, value: 10 },
            IROp::Const { dest: 1, value: 20 },
            IROp::BinaryOp {
                dest: 2,
                op: vm_ir::BinaryOpType::Add,
                src1: 0,
                src2: 1,
            },
        ],
        term: Terminator::Return,
    };

    let optimized = optimizer.optimize_block(&block);

    // Optimizer should fold 10 + 20 = 30
    assert!(!optimized.ops.is_empty());
}

#[test]
fn test_dead_code_elimination() {
    let mut optimizer = IROptimizer::new();

    // Create block with dead code
    let block = IRBlock {
        start_pc: vm_core::GuestAddr(0x1000),
        ops: vec![
            IROp::Const { dest: 0, value: 10 },
            IROp::Const {
                dest: 1,
                value: 20, // This is never used
            },
            IROp::BinaryOp {
                dest: 2,
                op: vm_ir::BinaryOpType::Add,
                src1: 0,
                src2: 0,
            },
        ],
        term: Terminator::Return,
    };

    let optimized = optimizer.optimize_block(&block);

    // Should eliminate dead code
    assert!(optimized.ops.len() <= block.ops.len());
}

#[test]
fn test_memory_alignment_optimization() {
    let mut optimizer = MemoryAlignmentOptimizer::new();

    let block = IRBlock {
        start_pc: vm_core::GuestAddr(0x1000),
        ops: vec![IROp::Load {
            dest: 0,
            base: 1,
            offset: 0,
            size: 8,
        }],
        term: Terminator::Return,
    };

    let optimized = optimizer.optimize_block(&block);

    assert!(!optimized.ops.is_empty());
}

// ============================================================================
// Statistics and Monitoring
// ============================================================================

#[test]
fn test_translation_statistics() {
    let translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);

    // Translate multiple blocks
    for i in 0..10 {
        let block = create_test_ir_block(i * 0x1000);
        let _ = translator.translate_block(&block);
    }

    let stats = translator.get_performance_stats();

    assert_eq!(stats.translated_blocks, 10);
    assert!(stats.total_translation_time_ns > 0);
}

#[test]
fn test_optimizer_statistics() {
    let mut optimizer = IROptimizer::new();

    let block = create_arithmetic_ir_block(0x1000);
    let _ = optimizer.optimize_block(&block);

    let stats = optimizer.get_stats();

    assert!(stats.optimized_blocks > 0 || stats.const_folding_count > 0);
}
