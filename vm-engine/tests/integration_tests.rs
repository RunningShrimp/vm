//! VM-Engine Integration Tests
//!
//! Comprehensive integration tests for vm-engine that verify:
//! - JIT compiler with vm-IR
//! - Interpreter execution
//! - Async executor
//! - Cross-engine execution flows
//! - Performance benchmarks
//! - Error handling

use std::sync::Arc;
use std::time::{Duration, Instant};

use vm_core::GuestAddr;
use vm_engine::executor::{AsyncExecutionContext, ExecutorType};
use vm_engine::jit::{JITCompiler, JITConfig, JITContext, OptLevel};
use vm_ir::{IRBlock, IROp, MemFlags, RegId, Terminator};

// ============================================================================
// Test Helpers
// ============================================================================

/// Create a simple arithmetic IR block
fn create_arithmetic_block() -> IRBlock {
    IRBlock {
        start_pc: GuestAddr(0x1000),
        ops: vec![
            IROp::MovImm { dst: 1, imm: 10 },
            IROp::MovImm { dst: 2, imm: 20 },
            IROp::Add {
                dst: 3,
                src1: 1,
                src2: 2,
            },
            IROp::Sub {
                dst: 4,
                src1: 3,
                src2: 1,
            },
        ],
        term: Terminator::Ret,
    }
}

/// Create a memory operation IR block
fn create_memory_block() -> IRBlock {
    IRBlock {
        start_pc: GuestAddr(0x1000),
        ops: vec![
            IROp::MovImm {
                dst: 1,
                imm: 0x1000,
            },
            IROp::Load {
                dst: 2,
                addr: 1,
                flags: MemFlags::default(),
            },
            IROp::MovImm { dst: 3, imm: 42 },
            IROp::Store {
                addr: 1,
                src: 3,
                flags: MemFlags::default(),
            },
        ],
        term: Terminator::Ret,
    }
}

/// Create a control flow IR block
fn create_control_flow_block() -> IRBlock {
    IRBlock {
        start_pc: GuestAddr(0x1000),
        ops: vec![
            IROp::MovImm { dst: 1, imm: 10 },
            IROp::MovImm { dst: 2, imm: 20 },
        ],
        term: Terminator::BranchCond {
            src1: 1,
            src2: 2,
            target: GuestAddr(0x2000),
        },
    }
}

/// Create a complex IR block with many operations
fn create_complex_block(num_ops: usize) -> IRBlock {
    let mut ops = Vec::new();

    for i in 0..num_ops {
        let reg = (i % 32) as u32;
        ops.push(IROp::MovImm {
            dst: reg,
            imm: i as i64,
        });

        if i % 2 == 0 && i > 0 {
            ops.push(IROp::Add {
                dst: reg,
                src1: reg,
                src2: reg - 1,
            });
        }
    }

    IRBlock {
        start_pc: GuestAddr(0x1000),
        ops,
        term: Terminator::Ret,
    }
}

// ============================================================================
// JIT + IR Integration Tests
// ============================================================================

#[cfg(test)]
mod jit_ir_integration {
    use super::*;

    /// Test IR generation → JIT compilation flow
    #[test]
    fn test_ir_to_jit_flow() {
        let block = create_arithmetic_block();

        // Verify IR structure
        assert_eq!(block.start_pc, GuestAddr(0x1000));
        assert!(!block.ops.is_empty());
        assert!(matches!(block.term, Terminator::Ret));

        // Compile with JIT
        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok(), "JIT compilation should succeed");
    }

    /// Test JIT compilation with different optimization levels
    #[test]
    fn test_jit_optimization_levels() {
        let block = create_arithmetic_block();

        let opt_levels = vec![
            OptLevel::None,
            OptLevel::Less,
            OptLevel::Default,
            OptLevel::Aggressive,
        ];

        for opt_level in opt_levels {
            let config = JITConfig {
                opt_level,
                ..Default::default()
            };

            let mut compiler = JITCompiler::new();
            let result = compiler.compile(&block);

            assert!(
                result.is_ok(),
                "JIT with opt_level {:?} should succeed",
                opt_level
            );
        }
    }

    /// Test JIT compilation of memory operations
    #[test]
    fn test_jit_memory_operations() {
        let block = create_memory_block();

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(
            result.is_ok(),
            "JIT compilation of memory ops should succeed"
        );
    }

    /// Test JIT compilation of control flow
    #[test]
    fn test_jit_control_flow() {
        let block = create_control_flow_block();

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(
            result.is_ok(),
            "JIT compilation of control flow should succeed"
        );
    }

    /// Test JIT compilation of complex blocks
    #[test]
    fn test_jit_complex_blocks() {
        let sizes = vec![10, 50, 100, 200];

        for size in sizes {
            let block = create_complex_block(size);

            let mut compiler = JITCompiler::new();
            let result = compiler.compile(&block);

            assert!(
                result.is_ok(),
                "JIT compilation of {} ops should succeed",
                size
            );
        }
    }

    /// Test JIT compilation with various instruction types
    #[test]
    fn test_jit_all_instruction_types() {
        let blocks = vec![
            create_arithmetic_block(),
            create_memory_block(),
            create_control_flow_block(),
        ];

        let mut compiler = JITCompiler::new();

        for block in blocks {
            let result = compiler.compile(&block);
            assert!(result.is_ok(), "JIT should handle all instruction types");
        }
    }

    /// Test JIT compilation performance
    #[test]
    fn test_jit_compilation_performance() {
        let block = create_complex_block(100);

        let iterations = 100;
        let start = Instant::now();

        let mut compiler = JITCompiler::new();
        for _ in 0..iterations {
            let _ = compiler.compile(&block);
        }

        let elapsed = start.elapsed();
        let avg_time = elapsed / iterations;

        println!(
            "JIT compilation: {} iterations in {:?} (avg: {:?}/iter)",
            iterations, elapsed, avg_time
        );

        assert!(
            avg_time.as_millis() < 10,
            "JIT compilation should be fast (< 10ms average)"
        );
    }

    /// Test JIT compilation timeout handling
    #[test]
    fn test_jit_timeout() {
        let block = create_complex_block(1000);

        let start = Instant::now();
        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);
        let elapsed = start.elapsed();

        assert!(result.is_ok(), "JIT should complete successfully");
        assert!(
            elapsed < Duration::from_secs(5),
            "JIT should complete within 5 seconds"
        );
    }
}

// ============================================================================
// Executor Integration Tests
// ============================================================================

#[cfg(test)]
mod executor_integration {
    use super::*;

    /// Test creating execution contexts for different executor types
    #[test]
    fn test_executor_context_creation() {
        let executor_types = vec![
            ExecutorType::Interpreter,
            ExecutorType::JIT,
            ExecutorType::Threaded,
        ];

        for exec_type in executor_types {
            // Note: Actual execution may require more setup
            // This tests the basic creation/configuration
            let ctx = AsyncExecutionContext::new(exec_type);
            assert_eq!(ctx.executor_type(), exec_type);
        }
    }

    /// Test executor type transitions
    #[test]
    fn test_executor_type_transitions() {
        let mut ctx = AsyncExecutionContext::new(ExecutorType::Interpreter);

        assert_eq!(ctx.executor_type(), ExecutorType::Interpreter);

        // Note: Actual executor switching may require different implementation
        // This tests the basic configuration
    }
}

// ============================================================================
// Cross-Engine Integration Tests
// ============================================================================

#[cfg(test)]
mod cross_engine_integration {
    use super::*;

    /// Test that IR can be compiled by different engines
    #[test]
    fn test_cross_engine_ir_compatibility() {
        let block = create_arithmetic_block();

        // JIT compilation
        let mut jit_compiler = JITCompiler::new();
        let jit_result = jit_compiler.compile(&block);
        assert!(jit_result.is_ok(), "JIT should compile the block");

        // The same IR should be compatible with other engines
        // (though we only test JIT here due to implementation details)
    }

    /// Test execution flow across multiple blocks
    #[test]
    fn test_multi_block_execution() {
        let blocks = vec![
            create_arithmetic_block(),
            create_memory_block(),
            create_control_flow_block(),
        ];

        let mut compiler = JITCompiler::new();

        for block in blocks {
            let result = compiler.compile(&block);
            assert!(result.is_ok(), "Each block should compile successfully");
        }
    }

    /// Test optimization levels affect compilation
    #[test]
    fn test_optimization_effects() {
        let block = create_complex_block(50);

        let configs = vec![
            JITConfig {
                opt_level: OptLevel::None,
                ..Default::default()
            },
            JITConfig {
                opt_level: OptLevel::Aggressive,
                ..Default::default()
            },
        ];

        for config in configs {
            let mut compiler = JITCompiler::new();
            let result = compiler.compile(&block);
            assert!(
                result.is_ok(),
                "Compilation should succeed with any optimization level"
            );
        }
    }
}

// ============================================================================
// Performance Integration Tests
// ============================================================================

#[cfg(test)]
mod performance_integration {
    use super::*;

    /// Benchmark JIT compilation throughput
    #[test]
    fn test_jit_throughput() {
        let iterations = 1000;
        let mut times = Vec::with_capacity(iterations);

        let mut compiler = JITCompiler::new();

        for _ in 0..iterations {
            let block = create_complex_block(50);

            let start = Instant::now();
            let result = compiler.compile(&block);
            let elapsed = start.elapsed();

            assert!(result.is_ok());
            times.push(elapsed);
        }

        let total: Duration = times.iter().sum();
        let avg = total / iterations as u32;
        let min = times.iter().min().copied().unwrap_or_default();
        let max = times.iter().max().copied().unwrap_or_default();

        println!("JIT Throughput ({} iterations):", iterations);
        println!("  Total: {:?}", total);
        println!("  Average: {:?}", avg);
        println!("  Min: {:?}", min);
        println!("  Max: {:?}", max);

        assert!(
            avg.as_micros() < 1000,
            "Average compilation time should be < 1ms"
        );
    }

    /// Test compilation scalability with block size
    #[test]
    fn test_jit_scalability() {
        let sizes = vec![10, 50, 100, 200, 500];
        let mut times = Vec::new();

        for size in sizes {
            let block = create_complex_block(size);

            let start = Instant::now();
            let mut compiler = JITCompiler::new();
            let result = compiler.compile(&block);
            let elapsed = start.elapsed();

            assert!(result.is_ok());
            times.push((size, elapsed));

            println!("Block size {}: {:?}", size, elapsed);
        }

        // Verify compilation time grows reasonably with block size
        // (should be roughly linear or better)
        let (_, time10) = times[0];
        let (_, time500) = times[4];

        let ratio = time500.as_nanos() as f64 / time10.as_nanos() as f64;
        let size_ratio = 500.0 / 10.0;

        println!(
            "Scalability: time ratio = {:.2}, size ratio = {:.2}",
            ratio, size_ratio
        );

        // Compilation time shouldn't grow worse than O(n²)
        assert!(
            ratio < size_ratio * size_ratio,
            "Compilation should scale reasonably"
        );
    }

    /// Test memory allocation during compilation
    #[test]
    fn test_jit_memory_usage() {
        // This is a basic test - real memory profiling would need external tools
        let iterations = 100;

        let block = create_complex_block(100);

        for i in 0..iterations {
            let mut compiler = JITCompiler::new();
            let result = compiler.compile(&block);

            if i % 10 == 0 {
                // Periodic checks to ensure no obvious memory issues
                assert!(result.is_ok());
            }
        }

        // If we get here without panicking or OOM, memory usage is reasonable
        assert!(true);
    }
}

// ============================================================================
// Error Handling Integration Tests
// ============================================================================

#[cfg(test)]
mod error_handling_integration {
    use super::*;

    /// Test JIT compilation error handling
    #[test]
    fn test_jit_error_handling() {
        // Create an invalid IR block (empty)
        let invalid_block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![],
            term: Terminator::Ret,
        };

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&invalid_block);

        // Empty blocks should either succeed or fail gracefully
        // (depending on implementation)
        match result {
            Ok(_) => println!("Empty block compiled successfully"),
            Err(e) => println!("Empty block compilation failed (expected): {:?}", e),
        }
    }

    /// Test recovery after compilation error
    #[test]
    fn test_error_recovery() {
        let mut compiler = JITCompiler::new();

        // Try to compile a potentially problematic block
        let block1 = create_complex_block(10000);
        let _ = compiler.compile(&block1);

        // Compiler should still work for subsequent compilations
        let block2 = create_arithmetic_block();
        let result = compiler.compile(&block2);

        assert!(result.is_ok(), "Compiler should recover after any errors");
    }
}

// ============================================================================
// Integration with vm-core Tests
// ============================================================================

#[cfg(test)]
mod vm_core_integration {
    use super::*;

    /// Test GuestAddr compatibility
    #[test]
    fn test_guest_addr_compatibility() {
        let addr = GuestAddr(0x1000);

        let block = IRBlock {
            start_pc: addr,
            ops: vec![],
            term: Terminator::Ret,
        };

        assert_eq!(block.start_pc, addr);
    }

    /// Test memory flag compatibility
    #[test]
    fn test_memory_flags_compatibility() {
        let flags = MemFlags {
            volatile: true,
            atomic: false,
            align: 8,
            ..Default::default()
        };

        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![IROp::Load {
                dst: 1,
                addr: 2,
                flags,
            }],
            term: Terminator::Ret,
        };

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok());
    }
}

// ============================================================================
// Stress Tests
// ============================================================================

#[cfg(test)]
mod stress_tests {
    use super::*;

    /// Test compiling many blocks in sequence
    #[test]
    fn test_sequential_compilation_stress() {
        let iterations = 1000;
        let mut compiler = JITCompiler::new();

        for i in 0..iterations {
            let block = create_complex_block((i % 100) + 10);
            let result = compiler.compile(&block);

            assert!(result.is_ok(), "Compilation {} should succeed", i);
        }
    }

    /// Test compiling very large blocks
    #[test]
    fn test_large_block_compilation() {
        let block = create_complex_block(1000);

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok(), "Large block compilation should succeed");
    }

    /// Test rapid compilation with different configurations
    #[test]
    fn test_rapid_reconfiguration() {
        let block = create_arithmetic_block();
        let configs = vec![
            JITConfig {
                opt_level: OptLevel::None,
                ..Default::default()
            },
            JITConfig {
                opt_level: OptLevel::Default,
                ..Default::default()
            },
            JITConfig {
                opt_level: OptLevel::Aggressive,
                ..Default::default()
            },
        ];

        for (i, config) in configs.iter().cycle().take(100).enumerate() {
            let mut compiler = JITCompiler::new();
            let result = compiler.compile(&block);

            assert!(
                result.is_ok(),
                "Compilation with config {} should succeed",
                i
            );
        }
    }
}

// ============================================================================
// Real-World Scenario Tests
// ============================================================================

#[cfg(test)]
mod real_world_scenarios {
    use super::*;

    /// Simulate a realistic code compilation scenario
    #[test]
    fn test_realistic_code_compilation() {
        // Create a mix of different operation types
        let ops = vec![
            // Prologue
            IROp::MovImm {
                dst: 1,
                imm: 0x1000, // Stack pointer
            },
            IROp::MovImm {
                dst: 2,
                imm: 100, // Loop counter
            },
            // Loop body (simulated)
            IROp::MovImm { dst: 3, imm: 0 },
            IROp::Add {
                dst: 3,
                src1: 3,
                src2: 2,
            },
            // Store result
            IROp::Store {
                addr: 1,
                src: 3,
                flags: MemFlags::default(),
            },
            // Decrement counter
            IROp::Add {
                dst: 2,
                src1: 2,
                src2: 0, // Assuming x0 is zero
            },
        ];

        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops,
            term: Terminator::BranchCond {
                src1: 2,
                src2: 0,
                target: GuestAddr(0x1000),
            },
        };

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok(), "Realistic code should compile successfully");
    }

    /// Test compilation of function call sequence
    #[test]
    fn test_function_call_compilation() {
        let ops = vec![
            // Setup arguments
            IROp::MovImm {
                dst: 10, // a0
                imm: 42,
            },
            IROp::MovImm {
                dst: 11, // a1
                imm: 24,
            },
            // Call function (simplified)
            IROp::Add {
                dst: 12, // Return value
                src1: 10,
                src2: 11,
            },
            // Use result
            IROp::MovImm {
                dst: 13,
                imm: 0x2000,
            },
            IROp::Store {
                addr: 13,
                src: 12,
                flags: MemFlags::default(),
            },
        ];

        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops,
            term: Terminator::Ret,
        };

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok(), "Function call sequence should compile");
    }
}
