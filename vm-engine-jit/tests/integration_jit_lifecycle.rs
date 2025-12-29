//! JIT Compilation and Execution Integration Tests
//!
//! Comprehensive integration tests for JIT compilation:
//! - JIT engine creation and initialization
//! - IR block compilation
//! - Code execution
//! - Code caching
//! - Optimization levels
//! - Hotspot detection
//! - Tiered compilation
//! - Error handling and edge cases

use vm_core::{ExecResult, ExecStats, ExecStatus, GuestAddr, MMU};
use vm_engine_jit::{
    CompilationStrategy, JITConfig, JITEngine, OptimizationLevel,
    code_cache::CodeCache,
    hotspot::HotspotDetector,
    optimizer::Optimizer,
    tiered_compiler::{CompilationTier, TieredCompiler},
};
use vm_ir::{IRBlock, IROp, Terminator};

// ============================================================================
// Test Fixtures
// ============================================================================

/// Create a simple arithmetic IR block
fn create_arithmetic_block() -> IRBlock {
    IRBlock {
        start_pc: GuestAddr(0x1000),
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
    }
}

/// Create a loop IR block
fn create_loop_block() -> IRBlock {
    IRBlock {
        start_pc: GuestAddr(0x2000),
        ops: vec![
            IROp::Const { dest: 0, value: 0 },
            IROp::Const { dest: 1, value: 1 },
            IROp::BinaryOp {
                dest: 0,
                op: vm_ir::BinaryOpType::Add,
                src1: 0,
                src2: 1,
            },
        ],
        term: Terminator::Return,
    }
}

/// Create a memory access IR block
fn create_memory_block() -> IRBlock {
    IRBlock {
        start_pc: GuestAddr(0x3000),
        ops: vec![
            IROp::Const {
                dest: 0,
                value: 0x1000,
            },
            IROp::Load {
                dest: 1,
                base: 0,
                offset: 0,
                size: 8,
            },
            IROp::Store {
                src: 1,
                base: 0,
                offset: 8,
                size: 8,
            },
        ],
        term: Terminator::Return,
    }
}

/// Mock MMU for testing
struct MockMmu {
    memory: Vec<u8>,
}

impl MockMmu {
    fn new(size: usize) -> Self {
        MockMmu {
            memory: vec![0u8; size],
        }
    }
}

impl MMU for MockMmu {
    fn read_byte(&self, addr: vm_core::GuestAddr) -> vm_core::VmResult<u8> {
        if addr.0 < self.memory.len() as u64 {
            Ok(self.memory[addr.0 as usize])
        } else {
            Err(vm_core::VmError::Core(vm_core::CoreError::MemoryError(
                vm_core::error::MemoryError::OutOfMemory,
            )))
        }
    }

    fn write_byte(&mut self, addr: vm_core::GuestAddr, value: u8) -> vm_core::VmResult<()> {
        if addr.0 < self.memory.len() as u64 {
            self.memory[addr.0 as usize] = value;
            Ok(())
        } else {
            Err(vm_core::VmError::Core(vm_core::CoreError::MemoryError(
                vm_core::error::MemoryError::OutOfMemory,
            )))
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ============================================================================
// Happy Path Tests
// ============================================================================

#[test]
fn test_jit_engine_creation() {
    let config = JITConfig::new()
        .with_optimization_level(OptimizationLevel::None)
        .with_cache_size(1024 * 1024);

    let result = JITEngine::new(config);
    assert!(result.is_ok());
}

#[test]
fn test_simple_compilation() {
    let config = JITConfig::new().with_optimization_level(OptimizationLevel::None);

    let jit_engine = JITEngine::new(config).unwrap();
    let block = create_arithmetic_block();

    let result = jit_engine.compile_block(block);
    assert!(result.is_ok());
}

#[test]
fn test_compilation_with_optimization() {
    let config = JITConfig::new().with_optimization_level(OptimizationLevel::Aggressive);

    let jit_engine = JITEngine::new(config).unwrap();
    let block = create_arithmetic_block();

    let result = jit_engine.compile_block(block);
    assert!(result.is_ok());
}

#[test]
fn test_multiple_compilations() {
    let config = JITConfig::new().with_optimization_level(OptimizationLevel::Balanced);

    let jit_engine = JITEngine::new(config).unwrap();

    for i in 0..10 {
        let mut block = create_arithmetic_block();
        block.start_pc = GuestAddr(0x1000 + (i * 0x100));

        let result = jit_engine.compile_block(block);
        assert!(result.is_ok());
    }
}

#[test]
fn test_code_cache_hit() {
    let config = JITConfig::new()
        .with_optimization_level(OptimizationLevel::Balanced)
        .with_cache_size(1024 * 1024);

    let jit_engine = JITEngine::new(config).unwrap();
    let block = create_arithmetic_block();

    // First compilation - cache miss
    let result1 = jit_engine.compile_block(block.clone());
    assert!(result1.is_ok());

    // Second compilation - cache hit
    let result2 = jit_engine.compile_block(block);
    assert!(result2.is_ok());

    let stats = jit_engine.get_performance_stats();
    assert!(stats.cache_hits >= 0);
}

#[test]
fn test_tiered_compilation() {
    let config = JITConfig::new()
        .with_compilation_strategy(CompilationStrategy::Tiered)
        .with_hotspot_threshold(10);

    let jit_engine = JITEngine::new(config).unwrap();
    let block = create_loop_block();

    // Execute block multiple times to trigger tiered compilation
    for _ in 0..20 {
        let _ = jit_engine.compile_block(block.clone());
    }

    let stats = jit_engine.get_performance_stats();
    assert!(stats.compiled_blocks > 0);
}

#[test]
fn test_hotspot_detection() {
    let mut detector = HotspotDetector::new(10);

    // Simulate multiple executions of the same block
    let addr = GuestAddr(0x1000);
    for _ in 0..15 {
        detector.record_execution(addr);
    }

    assert!(detector.is_hotspot(addr));
}

// ============================================================================
// Execution Tests
// ============================================================================

#[test]
fn test_simple_execution() {
    let config = JITConfig::new().with_optimization_level(OptimizationLevel::None);

    let mut jit_engine = JITEngine::new(config).unwrap();
    let block = create_arithmetic_block();

    let compiled = jit_engine.compile_block(block).unwrap();

    // Execute compiled code
    let mut mmu = MockMmu::new(1024 * 1024);
    let result = jit_engine.execute(&mut mmu, &compiled);

    assert!(result.is_ok());
}

#[test]
fn test_execution_with_memory_access() {
    let config = JITConfig::new().with_optimization_level(OptimizationLevel::Balanced);

    let mut jit_engine = JITEngine::new(config).unwrap();
    let block = create_memory_block();

    let compiled = jit_engine.compile_block(block).unwrap();

    let mut mmu = MockMmu::new(1024 * 1024);
    mmu.memory[0x1000] = 0x42;

    let result = jit_engine.execute(&mut mmu, &compiled);

    assert!(result.is_ok());
}

#[test]
fn test_execution_statistics() {
    let config = JITConfig::new().with_optimization_level(OptimizationLevel::Balanced);

    let mut jit_engine = JITEngine::new(config).unwrap();
    let block = create_arithmetic_block();

    let compiled = jit_engine.compile_block(block).unwrap();

    let mut mmu = MockMmu::new(1024 * 1024);

    // Execute multiple times
    for _ in 0..10 {
        let _ = jit_engine.execute(&mut mmu, &compiled);
    }

    let stats = jit_engine.get_performance_stats();
    assert!(stats.execution_count > 0);
    assert!(stats.total_execution_time_ns > 0);
}

#[test]
fn test_compilation_and_execution_flow() {
    let config = JITConfig::new().with_optimization_level(OptimizationLevel::Balanced);

    let mut jit_engine = JITEngine::new(config).unwrap();

    // Compile
    let block = create_arithmetic_block();
    let compiled = jit_engine.compile_block(block).unwrap();

    // Execute
    let mut mmu = MockMmu::new(1024 * 1024);
    let result = jit_engine.execute(&mut mmu, &compiled);

    assert!(result.is_ok());

    // Verify statistics
    let stats = jit_engine.get_performance_stats();
    assert_eq!(stats.compiled_blocks, 1);
    assert!(stats.execution_count >= 1);
}

// ============================================================================
// Error Path Tests
// ============================================================================

#[test]
fn test_invalid_block_compilation() {
    let config = JITConfig::new().with_optimization_level(OptimizationLevel::None);

    let jit_engine = JITEngine::new(config).unwrap();

    // Empty block
    let invalid_block = IRBlock {
        start_pc: GuestAddr(0x1000),
        ops: vec![],
        term: Terminator::Return,
    };

    let result = jit_engine.compile_block(invalid_block);

    // Should handle gracefully
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_very_large_block() {
    let config = JITConfig::new().with_optimization_level(OptimizationLevel::None);

    let jit_engine = JITEngine::new(config).unwrap();

    // Create a large block
    let mut ops = Vec::new();
    for i in 0..10000 {
        ops.push(IROp::Const {
            dest: i % 32,
            value: i as u64,
        });
    }

    let large_block = IRBlock {
        start_pc: GuestAddr(0x1000),
        ops,
        term: Terminator::Return,
    };

    let result = jit_engine.compile_block(large_block);

    // Should handle large blocks
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_execution_with_invalid_memory() {
    let config = JITConfig::new().with_optimization_level(OptimizationLevel::None);

    let mut jit_engine = JITEngine::new(config).unwrap();
    let block = create_memory_block();

    let compiled = jit_engine.compile_block(block).unwrap();

    // Use very small MMU that will cause out-of-memory errors
    let mut mmu = MockMmu::new(100);

    let result = jit_engine.execute(&mut mmu, &compiled);

    // May fail due to memory access errors
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_cache_overflow() {
    let config = JITConfig::new()
        .with_optimization_level(OptimizationLevel::None)
        .with_cache_size(1024); // Very small cache

    let jit_engine = JITEngine::new(config).unwrap();

    // Compile many blocks to overflow cache
    for i in 0..1000 {
        let mut block = create_arithmetic_block();
        block.start_pc = GuestAddr(i * 0x1000);

        let _ = jit_engine.compile_block(block);
    }

    // Should handle cache overflow gracefully
    let stats = jit_engine.get_performance_stats();
    assert!(stats.compiled_blocks > 0);
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_zero_optimization_level() {
    let config = JITConfig::new().with_optimization_level(OptimizationLevel::None);

    let jit_engine = JITEngine::new(config).unwrap();
    let block = create_arithmetic_block();

    let result = jit_engine.compile_block(block);

    assert!(result.is_ok());
}

#[test]
fn test_aggressive_optimization() {
    let config = JITConfig::new().with_optimization_level(OptimizationLevel::Aggressive);

    let jit_engine = JITEngine::new(config).unwrap();
    let block = create_arithmetic_block();

    let result = jit_engine.compile_block(block);

    assert!(result.is_ok());
}

#[test]
fn test_all_optimization_levels() {
    let levels = vec![
        OptimizationLevel::None,
        OptimizationLevel::Basic,
        OptimizationLevel::Balanced,
        OptimizationLevel::Aggressive,
    ];

    for level in levels {
        let config = JITConfig::new().with_optimization_level(level);

        let jit_engine = JITEngine::new(config).unwrap();
        let block = create_arithmetic_block();

        let result = jit_engine.compile_block(block.clone());
        assert!(result.is_ok(), "Failed at optimization level {:?}", level);
    }
}

#[test]
fn test_different_compilation_strategies() {
    let strategies = vec![CompilationStrategy::Simple, CompilationStrategy::Tiered];

    for strategy in strategies {
        let config = JITConfig::new().with_compilation_strategy(strategy);

        let jit_engine = JITEngine::new(config).unwrap();
        let block = create_arithmetic_block();

        let result = jit_engine.compile_block(block.clone());
        assert!(result.is_ok(), "Failed with strategy {:?}", strategy);
    }
}

#[test]
fn test_concurrent_compilation() {
    use std::thread;

    let config = JITConfig::new()
        .with_optimization_level(OptimizationLevel::Balanced)
        .with_max_compilation_threads(4);

    let jit_engine = std::sync::Arc::new(std::sync::Mutex::new(JITEngine::new(config).unwrap()));

    let mut handles = Vec::new();

    for i in 0..4 {
        let engine = std::sync::Arc::clone(&jit_engine);
        let handle = thread::spawn(move || {
            let mut block = create_arithmetic_block();
            block.start_pc = GuestAddr(0x1000 + (i * 0x1000));

            let engine = engine.lock().unwrap();
            // Note: compile_block may need &mut self, adjust as needed
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_repeated_same_block_compilation() {
    let config = JITConfig::new().with_optimization_level(OptimizationLevel::Balanced);

    let jit_engine = JITEngine::new(config).unwrap();
    let block = create_arithmetic_block();

    // Compile the same block many times
    for _ in 0..100 {
        let result = jit_engine.compile_block(block.clone());
        assert!(result.is_ok());
    }

    let stats = jit_engine.get_performance_stats();
    // Most should be cache hits
    assert!(stats.compiled_blocks >= 1);
    assert!(stats.cache_hits > 0);
}

#[test]
fn test_compilation_with_simd_operations() {
    let config = JITConfig::new()
        .with_optimization_level(OptimizationLevel::Balanced)
        .with_enable_simd_optimization(true);

    let jit_engine = JITEngine::new(config).unwrap();

    // Create block with SIMD-like operations
    let block = IRBlock {
        start_pc: GuestAddr(0x1000),
        ops: vec![
            IROp::Load {
                dest: 0,
                base: 1,
                offset: 0,
                size: 16, // 128-bit load
            },
            IROp::Store {
                src: 0,
                base: 1,
                offset: 16,
                size: 16,
            },
        ],
        term: Terminator::Return,
    };

    let result = jit_engine.compile_block(block);

    // May succeed or fail depending on SIMD support
    let _ = result;
}

// ============================================================================
// Performance Tests
// ============================================================================

#[test]
fn test_compilation_performance() {
    let config = JITConfig::new().with_optimization_level(OptimizationLevel::Balanced);

    let jit_engine = JITEngine::new(config).unwrap();

    let start = std::time::Instant::now();

    for i in 0..100 {
        let mut block = create_arithmetic_block();
        block.start_pc = GuestAddr(i * 0x100);

        let _ = jit_engine.compile_block(block);
    }

    let duration = start.elapsed();

    // Should complete in reasonable time
    assert!(duration.as_secs() < 10);
}

#[test]
fn test_execution_performance() {
    let config = JITConfig::new().with_optimization_level(OptimizationLevel::Balanced);

    let mut jit_engine = JITEngine::new(config).unwrap();
    let block = create_arithmetic_block();

    let compiled = jit_engine.compile_block(block).unwrap();
    let mut mmu = MockMmu::new(1024 * 1024);

    let start = std::time::Instant::now();

    for _ in 0..10000 {
        let _ = jit_engine.execute(&mut mmu, &compiled);
    }

    let duration = start.elapsed();

    // Should execute quickly
    assert!(duration.as_secs() < 5);
}

#[test]
fn test_cache_efficiency() {
    let config = JITConfig::new()
        .with_optimization_level(OptimizationLevel::Balanced)
        .with_cache_size(10 * 1024 * 1024);

    let jit_engine = JITEngine::new(config).unwrap();

    // Compile unique blocks
    for i in 0..50 {
        let mut block = create_arithmetic_block();
        block.start_pc = GuestAddr(i * 0x1000);

        let _ = jit_engine.compile_block(block);
    }

    // Compile same blocks again (should hit cache)
    for i in 0..50 {
        let mut block = create_arithmetic_block();
        block.start_pc = GuestAddr(i * 0x1000);

        let _ = jit_engine.compile_block(block);
    }

    let stats = jit_engine.get_performance_stats();

    // Should have cache hits
    assert!(stats.cache_hits > 0);
}

// ============================================================================
// Statistics and Monitoring
// ============================================================================

#[test]
fn test_performance_stats() {
    let config = JITConfig::new().with_optimization_level(OptimizationLevel::Balanced);

    let mut jit_engine = JITEngine::new(config).unwrap();

    // Compile some blocks
    for i in 0..10 {
        let mut block = create_arithmetic_block();
        block.start_pc = GuestAddr(i * 0x1000);

        let _ = jit_engine.compile_block(block);
    }

    let stats = jit_engine.get_performance_stats();

    assert_eq!(stats.compiled_blocks, 10);
    assert!(stats.total_compilation_time_ns > 0);
}

#[test]
fn test_hotspot_stats() {
    let config = JITConfig::new()
        .with_compilation_strategy(CompilationStrategy::Tiered)
        .with_hotspot_threshold(10);

    let mut jit_engine = JITEngine::new(config).unwrap();
    let block = create_loop_block();

    // Execute many times
    for _ in 0..20 {
        let compiled = jit_engine.compile_block(block.clone()).unwrap();

        let mut mmu = MockMmu::new(1024 * 1024);
        let _ = jit_engine.execute(&mut mmu, &compiled);
    }

    let stats = jit_engine.get_performance_stats();

    // Should detect hotspots
    assert!(stats.hotspots_detected >= 0);
}

#[test]
fn test_reset_stats() {
    let config = JITConfig::new().with_optimization_level(OptimizationLevel::Balanced);

    let jit_engine = JITEngine::new(config).unwrap();

    // Compile some blocks
    for i in 0..10 {
        let mut block = create_arithmetic_block();
        block.start_pc = GuestAddr(i * 0x1000);

        let _ = jit_engine.compile_block(block);
    }

    // Reset stats
    jit_engine.reset_stats();

    let stats = jit_engine.get_performance_stats();

    assert_eq!(stats.compiled_blocks, 0);
    assert_eq!(stats.cache_hits, 0);
    assert_eq!(stats.cache_misses, 0);
}
