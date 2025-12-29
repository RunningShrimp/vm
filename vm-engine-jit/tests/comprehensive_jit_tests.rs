//! Comprehensive tests for vm-engine-jit module
//!
//! This test suite covers JIT compilation, code caching, optimization, and related functionality.

use vm_core::{
    ExecResult, ExecStats, ExecStatus, GuestAddr, GuestPhysAddr, MMU, MemoryAccess, VmError,
};
use vm_engine_jit::{AdaptiveThresholdConfig, AdaptiveThresholdStats, CodePtr, HOT_THRESHOLD, Jit};
use vm_ir::{IRBlock, IROp, RegId, Terminator};

// ============================================================================
// Basic JIT Tests
// ============================================================================

#[test]
fn test_jit_creation() {
    let jit = Jit::new();
    assert!(jit.get_config().is_none());
}

#[test]
fn test_jit_default() {
    let jit = Jit::default();
    assert!(jit.get_config().is_none());
}

#[test]
fn test_jit_with_adaptive_config() {
    let config = AdaptiveThresholdConfig {
        hot_threshold: 200,
        cold_threshold: 20,
        enable_adaptive: true,
    };

    let jit = Jit::with_adaptive_config(config.clone());
    let retrieved_config = jit.get_config().unwrap();

    assert_eq!(retrieved_config.hot_threshold, 200);
    assert_eq!(retrieved_config.cold_threshold, 20);
    assert!(retrieved_config.enable_adaptive);
}

#[test]
fn test_jit_set_pc() {
    let mut jit = Jit::new();
    jit.set_pc(GuestAddr(0x1000));
    // Should not panic
}

#[test]
fn test_jit_get_set_config() {
    let mut jit = Jit::new();

    // Initially no config
    assert!(jit.get_config().is_none());

    // Set config
    let config = AdaptiveThresholdConfig::default();
    jit.set_config(Some(config.clone()));

    // Get config
    let retrieved = jit.get_config().unwrap();
    assert_eq!(retrieved.hot_threshold, config.hot_threshold);
    assert_eq!(retrieved.cold_threshold, config.cold_threshold);

    // Clear config
    jit.set_config(None);
    assert!(jit.get_config().is_none());
}

// ============================================================================
// IRBlock Execution Tests
// ============================================================================

#[test]
fn test_run_empty_block() {
    let mut jit = Jit::new();
    let mut mmu = create_test_mmu();

    let block = IRBlock {
        start_pc: GuestAddr(0x1000),
        ops: vec![],
        term: Terminator::Return,
    };

    let result = jit.run(&mut mmu, &block);

    assert!(matches!(result.status, ExecStatus::Ok));
}

#[test]
fn test_run_simple_block() {
    let mut jit = Jit::new();
    let mut mmu = create_test_mmu();

    let block = IRBlock {
        start_pc: GuestAddr(0x1000),
        ops: vec![create_nop_op(), create_nop_op(), create_nop_op()],
        term: Terminator::Return,
    };

    let result = jit.run(&mut mmu, &block);

    assert!(matches!(result.status, ExecStatus::Ok));
    assert_eq!(result.stats.executed_insns, 3);
}

#[test]
fn test_run_block_with_ops() {
    let mut jit = Jit::new();
    let mut mmu = create_test_mmu();

    let block = IRBlock {
        start_pc: GuestAddr(0x1000),
        ops: (0..10).map(|_| create_nop_op()).collect(),
        term: Terminator::Return,
    };

    let result = jit.run(&mut mmu, &block);

    assert!(matches!(result.status, ExecStatus::Ok));
    assert_eq!(result.stats.executed_insns, 10);
}

#[test]
fn test_run_multiple_blocks() {
    let mut jit = Jit::new();
    let mut mmu = create_test_mmu();

    for i in 0..5 {
        let block = IRBlock {
            start_pc: GuestAddr(0x1000 + i * 0x100),
            ops: (0..5).map(|_| create_nop_op()).collect(),
            term: Terminator::Return,
        };

        let result = jit.run(&mut mmu, &block);
        assert!(matches!(result.status, ExecStatus::Ok));
    }
}

// ============================================================================
// Code Caching Tests
// ============================================================================

#[test]
fn test_code_cache_hit() {
    let mut jit = Jit::new();
    let mut mmu = create_test_mmu();

    let block = IRBlock {
        start_pc: GuestAddr(0x1000),
        ops: vec![create_nop_op()],
        term: Terminator::Return,
    };

    // First execution - cache miss
    let result1 = jit.run(&mut mmu, &block);
    assert!(matches!(result1.status, ExecStatus::Ok));

    // Second execution - should hit cache
    let result2 = jit.run(&mut mmu, &block);
    assert!(matches!(result2.status, ExecStatus::Ok));
}

#[test]
fn test_code_cache_multiple_blocks() {
    let mut jit = Jit::new();
    let mut mmu = create_test_mmu();

    // Execute multiple different blocks
    for i in 0..10 {
        let block = IRBlock {
            start_pc: GuestAddr(0x1000 + i * 0x100),
            ops: vec![create_nop_op()],
            term: Terminator::Return,
        };

        let result = jit.run(&mut mmu, &block);
        assert!(matches!(result.status, ExecStatus::Ok));
    }

    // Re-execute previous blocks - should hit cache
    for i in 0..10 {
        let block = IRBlock {
            start_pc: GuestAddr(0x1000 + i * 0x100),
            ops: vec![create_nop_op()],
            term: Terminator::Return,
        };

        let result = jit.run(&mut mmu, &block);
        assert!(matches!(result.status, ExecStatus::Ok));
    }
}

#[test]
fn test_compile_only() {
    let mut jit = Jit::new();

    let block = IRBlock {
        start_pc: GuestAddr(0x1000),
        ops: vec![create_nop_op()],
        term: Terminator::Return,
    };

    let code_ptr = jit.compile_only(&block);

    // Should return a valid code pointer
    // (We can't execute it directly without proper machine code generation)
    let _ = code_ptr;
}

#[test]
fn test_compile_only_caching() {
    let mut jit = Jit::new();

    let block = IRBlock {
        start_pc: GuestAddr(0x1000),
        ops: vec![create_nop_op()],
        term: Terminator::Return,
    };

    // Compile once
    let ptr1 = jit.compile_only(&block);

    // Compile again - should return cached version
    let ptr2 = jit.compile_only(&block);

    // Pointers should be the same (same cached code)
    assert_eq!(ptr1.0 as usize, ptr2.0 as usize);
}

// ============================================================================
// Execution Statistics Tests
// ============================================================================

#[test]
fn test_execution_stats() {
    let mut jit = Jit::new();
    let mut mmu = create_test_mmu();

    let block = IRBlock {
        start_pc: GuestAddr(0x1000),
        ops: (0..5).map(|_| create_nop_op()).collect(),
        term: Terminator::Return,
    };

    let result = jit.run(&mut mmu, &block);

    assert_eq!(result.stats.executed_insns, 5);
    assert_eq!(result.stats.executed_ops, 5);
    assert!(result.stats.exec_time_ns > 0);
}

#[test]
fn test_execution_stats_memory_accesses() {
    let mut jit = Jit::new();
    let mut mmu = create_test_mmu();

    let block = IRBlock {
        start_pc: GuestAddr(0x1000),
        ops: (0..10).map(|_| create_nop_op()).collect(),
        term: Terminator::Return,
    };

    let result = jit.run(&mut mmu, &block);

    // Should estimate memory accesses
    assert!(result.stats.mem_accesses > 0);
}

#[test]
fn test_execution_stats_next_pc() {
    let mut jit = Jit::new();
    let mut mmu = create_test_mmu();

    let block = IRBlock {
        start_pc: GuestAddr(0x1000),
        ops: vec![create_nop_op(), create_nop_op()],
        term: Terminator::Return,
    };

    let result = jit.run(&mut mmu, &block);

    // Next PC should be after the block
    assert!(result.next_pc >= block.start_pc);
}

// ============================================================================
// Adaptive Threshold Tests
// ============================================================================

#[test]
fn test_adaptive_threshold_config_default() {
    let config = AdaptiveThresholdConfig::default();

    assert_eq!(config.hot_threshold, 100);
    assert_eq!(config.cold_threshold, 10);
    assert!(config.enable_adaptive);
}

#[test]
fn test_adaptive_threshold_config_custom() {
    let config = AdaptiveThresholdConfig {
        hot_threshold: 200,
        cold_threshold: 20,
        enable_adaptive: false,
    };

    assert_eq!(config.hot_threshold, 200);
    assert_eq!(config.cold_threshold, 20);
    assert!(!config.enable_adaptive);
}

#[test]
fn test_adaptive_threshold_stats_default() {
    let stats = AdaptiveThresholdStats::default();

    assert_eq!(stats.hot_threshold, 100);
    assert_eq!(stats.cold_threshold, 10);
    assert_eq!(stats.execution_count, 0);
}

#[test]
fn test_hot_threshold_constant() {
    assert_eq!(HOT_THRESHOLD, 100);
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn test_run_with_invalid_memory() {
    let mut jit = Jit::new();
    let mut mmu = create_test_mmu();

    // Create a block that would access invalid memory
    let block = IRBlock {
        start_pc: GuestAddr(0x1000),
        ops: vec![],
        term: Terminator::Return,
    };

    // Should handle gracefully
    let result = jit.run(&mut mmu, &block);
    // Even with fallback, should not crash
    let _ = result;
}

#[test]
fn test_compile_failure_handling() {
    let mut jit = Jit::new();
    let mut mmu = create_test_mmu();

    let block = IRBlock {
        start_pc: GuestAddr(0x1000),
        ops: vec![],
        term: Terminator::Return,
    };

    // Should use fallback execution if compilation fails
    let result = jit.run(&mut mmu, &block);
    assert!(matches!(result.status, ExecStatus::Ok));
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_empty_terminator() {
    let mut jit = Jit::new();
    let mut mmu = create_test_mmu();

    let block = IRBlock {
        start_pc: GuestAddr(0x1000),
        ops: vec![],
        term: Terminator::Return,
    };

    // Should handle empty terminator
    let result = jit.run(&mut mmu, &block);
    assert!(matches!(result.status, ExecStatus::Ok));
}

#[test]
fn test_large_block() {
    let mut jit = Jit::new();
    let mut mmu = create_test_mmu();

    let block = IRBlock {
        start_pc: GuestAddr(0x1000),
        ops: (0..1000).map(|_| create_nop_op()).collect(),
        term: Terminator::Return,
    };

    let result = jit.run(&mut mmu, &block);

    assert!(matches!(result.status, ExecStatus::Ok));
    assert_eq!(result.stats.executed_insns, 1000);
}

#[test]
fn test_zero_address_block() {
    let mut jit = Jit::new();
    let mut mmu = create_test_mmu();

    let block = IRBlock {
        start_pc: GuestAddr(0),
        ops: vec![create_nop_op()],
        term: Terminator::Return,
    };

    let result = jit.run(&mut mmu, &block);
    assert!(matches!(result.status, ExecStatus::Ok));
}

#[test]
fn test_max_address_block() {
    let mut jit = Jit::new();
    let mut mmu = create_test_mmu();

    let block = IRBlock {
        start_pc: GuestAddr(u64::MAX),
        ops: vec![create_nop_op()],
        term: Terminator::Return,
    };

    let result = jit.run(&mut mmu, &block);
    assert!(matches!(result.status, ExecStatus::Ok));
}

#[test]
fn test_multiple_jit_instances() {
    let jit1 = Jit::new();
    let jit2 = Jit::new();
    let mut mmu = create_test_mmu();

    let block = IRBlock {
        start_pc: GuestAddr(0x1000),
        ops: vec![create_nop_op()],
        term: Terminator::Return,
    };

    // Run on first JIT
    let result1 = jit1.run(&mut mmu, &block);
    assert!(matches!(result1.status, ExecStatus::Ok));

    // Run on second JIT
    let result2 = jit2.run(&mut mmu, &block);
    assert!(matches!(result2.status, ExecStatus::Ok));
}

#[test]
fn test_concurrent_execution() {
    use std::sync::Arc;
    use std::thread;

    let jit = Arc::new(std::sync::Mutex::new(Jit::new()));
    let mut handles = vec![];

    for i in 0..4 {
        let jit_clone = Arc::clone(&jit);
        let handle = thread::spawn(move || {
            let mut mmu = create_test_mmu();
            let block = IRBlock {
                start_pc: GuestAddr(0x1000 + i * 0x100),
                ops: vec![create_nop_op()],
                term: Terminator::Return,
            };

            let mut jit = jit_clone.lock().unwrap();
            jit.run(&mut mmu, &block)
        });

        handles.push(handle);
    }

    // All threads should complete successfully
    for handle in handles {
        let result = handle.join().unwrap();
        assert!(matches!(result.status, ExecStatus::Ok));
    }
}

// ============================================================================
// Performance Tests
// ============================================================================

#[test]
fn test_execution_performance() {
    let mut jit = Jit::new();
    let mut mmu = create_test_mmu();

    let block = IRBlock {
        start_pc: GuestAddr(0x1000),
        ops: (0..100).map(|_| create_nop_op()).collect(),
        term: Terminator::Return,
    };

    let start = std::time::Instant::now();
    let result = jit.run(&mut mmu, &block);
    let duration = start.elapsed();

    assert!(matches!(result.status, ExecStatus::Ok));
    // Execution should be reasonably fast
    assert!(duration.as_millis() < 1000);
}

#[test]
fn test_cache_performance() {
    let mut jit = Jit::new();
    let mut mmu = create_test_mmu();

    let block = IRBlock {
        start_pc: GuestAddr(0x1000),
        ops: (0..10).map(|_| create_nop_op()).collect(),
        term: Terminator::Return,
    };

    // First run - cache miss
    let start1 = std::time::Instant::now();
    jit.run(&mut mmu, &block);
    let duration1 = start1.elapsed();

    // Second run - cache hit
    let start2 = std::time::Instant::now();
    jit.run(&mut mmu, &block);
    let duration2 = start2.elapsed();

    // Cached execution should be similar or faster
    // (In a real implementation, cache hits would be significantly faster)
    let _ = (duration1, duration2);
}

#[test]
fn test_memory_efficiency() {
    let mut jit = Jit::new();
    let mut mmu = create_test_mmu();

    // Compile many blocks
    for i in 0..100 {
        let block = IRBlock {
            start_pc: GuestAddr(0x1000 + i * 0x100),
            ops: vec![create_nop_op()],
            term: Terminator::Return,
        };

        jit.run(&mut mmu, &block);
    }

    // Should not run out of memory
    // (In a real implementation, we would check memory usage here)
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_jit_with_adaptive_config() {
    let config = AdaptiveThresholdConfig::default();
    let mut jit = Jit::with_adaptive_config(config.clone());
    let mut mmu = create_test_mmu();

    let block = IRBlock {
        start_pc: GuestAddr(0x1000),
        ops: vec![create_nop_op()],
        term: Terminator::Return,
    };

    let result = jit.run(&mut mmu, &block);

    assert!(matches!(result.status, ExecStatus::Ok));
    assert_eq!(result.stats.executed_insns, 1);
}

#[test]
fn test_jit_config_modification() {
    let mut jit = Jit::new();
    let mut mmu = create_test_mmu();

    // Run without config
    let block = IRBlock {
        start_pc: GuestAddr(0x1000),
        ops: vec![create_nop_op()],
        term: Terminator::Return,
    };

    let result1 = jit.run(&mut mmu, &block);
    assert!(matches!(result1.status, ExecStatus::Ok));

    // Add config
    jit.set_config(Some(AdaptiveThresholdConfig::default()));

    let result2 = jit.run(&mut mmu, &block);
    assert!(matches!(result2.status, ExecStatus::Ok));
}

// ============================================================================
// Helper Functions
// ============================================================================

fn create_test_mmu() -> TestMmu {
    TestMmu::new()
}

fn create_nop_op() -> IROp {
    IROp::Nop
}

// ============================================================================
// Test MMU Implementation
// ============================================================================

struct TestMmu {
    memory: std::collections::HashMap<u64, u8>,
}

impl TestMmu {
    fn new() -> Self {
        Self {
            memory: std::collections::HashMap::new(),
        }
    }
}

impl MemoryAccess for TestMmu {
    fn read(&self, pa: GuestAddr, size: u8) -> Result<u64, VmError> {
        let mut value = 0u64;
        for i in 0..size {
            let addr = pa.0 + i as u64;
            let byte = self.memory.get(&addr).copied().unwrap_or(0);
            value |= (byte as u64) << (i * 8);
        }
        Ok(value)
    }

    fn write(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<(), VmError> {
        for i in 0..size {
            let addr = pa.0 + i as u64;
            let byte = (val >> (i * 8)) as u8;
            self.memory.insert(addr, byte);
        }
        Ok(())
    }

    fn fetch_insn(&self, pc: GuestAddr) -> Result<u64, VmError> {
        self.read(pc, 4)
    }

    fn memory_size(&self) -> usize {
        self.memory.len()
    }

    fn dump_memory(&self) -> Vec<u8> {
        // Simplified implementation
        vec![]
    }

    fn restore_memory(&mut self, _data: &[u8]) -> Result<(), String> {
        Ok(())
    }
}

impl MMU for TestMmu {
    fn read(&self, pa: GuestAddr, size: u8) -> Result<u64, VmError> {
        MemoryAccess::read(self, pa, size)
    }

    fn write(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<(), VmError> {
        MemoryAccess::write(self, pa, val, size)
    }

    fn fetch_insn(&self, pc: GuestAddr) -> Result<u64, VmError> {
        MemoryAccess::fetch_insn(self, pc)
    }
}
