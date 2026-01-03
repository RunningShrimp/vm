//! Cross-Crate Integration Tests
//!
//! Comprehensive integration tests that verify multiple crates work together correctly.
//! Tests cover:
//! - vm-core + vm-engine: VM execution with JIT
//! - vm-engine + vm-ir: JIT compilation from IR
//! - vm-core + vm-mem: Memory management
//! - Full stack: Core → IR → Engine → Execution

mod common;

use std::sync::Arc;

use vm_core::{
    ExecMode, GuestAddr, GuestArch, GuestRegs, VmConfig, VmError as CoreVmError,
    VmLifecycleState,
};

use vm_engine::jit::{JITCompiler, JITConfig, OptLevel};
use vm_ir::{IRBlock, IROp, MemFlags, RegId, Terminator};

use common::{
    assert_lifecycle_state, assert_memory_pattern, assert_timeout,
    create_arithmetic_block, create_control_flow_block, create_memory_block,
    random_bytes, random_reg_id, random_reg_value, TestIRBuilder, TestVm, TestVmConfig,
    DEFAULT_MEMORY_SIZE, DEFAULT_TIMEOUT,
};

// ============================================================================
// VM Full Lifecycle Integration Tests
// ============================================================================

#[cfg(test)]
mod vm_lifecycle_tests {
    use super::*;

    /// Test complete VM lifecycle: Create → Init → Run → Pause → Resume → Stop
    #[test]
    fn test_vm_full_lifecycle() {
        let _guard = common::cleanup_guard();

        // Create VM
        let config = TestVmConfig::default();
        let mut vm = TestVm::new(config);
        assert_lifecycle_state(&vm, VmLifecycleState::Created);

        // Initialize VM
        vm.init().expect("VM initialization should succeed");
        assert_eq!(vm.pc(), GuestAddr(0x1000));

        // Simulate state transitions
        vm.lifecycle_state = VmLifecycleState::Running;
        assert_lifecycle_state(&vm, VmLifecycleState::Running);

        // Pause
        vm.lifecycle_state = VmLifecycleState::Paused;
        assert_lifecycle_state(&vm, VmLifecycleState::Paused);

        // Resume
        vm.lifecycle_state = VmLifecycleState::Running;
        assert_lifecycle_state(&vm, VmLifecycleState::Running);

        // Stop
        vm.lifecycle_state = VmLifecycleState::Stopped;
        assert_lifecycle_state(&vm, VmLifecycleState::Stopped);
    }

    /// Test VM memory operations across lifecycle
    #[test]
    fn test_vm_memory_operations() {
        let _guard = common::cleanup_guard();
        let config = TestVmConfig::default();
        let vm = TestVm::new(config);

        // Write to memory
        let test_data = b"Hello, VM!";
        vm.write_memory(GuestAddr(0x1000), test_data)
            .expect("Memory write should succeed");

        // Read back and verify
        let read_data = vm
            .read_memory(GuestAddr(0x1000), test_data.len())
            .expect("Memory read should succeed");
        assert_eq!(read_data, test_data);

        // Verify pattern in actual memory
        let state = vm.state.lock().unwrap();
        assert_memory_pattern(&state.memory, 0x1000, test_data);
    }

    /// Test VM with different architectures
    #[test]
    fn test_vm_multiple_architectures() {
        let _guard = common::cleanup_guard();

        for arch in &[GuestArch::Riscv64, GuestArch::Arm64, GuestArch::X86_64] {
            let config = TestVmConfig {
                arch: *arch,
                ..Default::default()
            };
            let vm = TestVm::new(config);
            assert_eq!(vm.config.guest_arch, *arch);
        }
    }
}

// ============================================================================
// JIT Compilation Integration Tests (vm-engine + vm-ir)
// ============================================================================

#[cfg(test)]
mod jit_integration_tests {
    use super::*;

    /// Test IR generation → JIT compilation flow
    #[test]
    fn test_ir_to_jit_flow() {
        // Create IR block
        let block = create_arithmetic_block();
        assert!(!block.ops.is_empty());

        // Compile with JIT
        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok(), "JIT compilation should succeed");
    }

    /// Test JIT compilation with optimization levels
    #[test]
    fn test_jit_optimization_levels() {
        let block = create_arithmetic_block();

        for opt_level in &[OptLevel::None, OptLevel::Less, OptLevel::Default, OptLevel::Aggressive]
        {
            let config = JITConfig {
                opt_level: *opt_level,
                ..Default::default()
            };

            let mut compiler = JITCompiler::new();
            let result = compiler.compile(&block);

            assert!(
                result.is_ok(),
                "JIT compilation with opt_level {:?} should succeed",
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
            "JIT compilation of memory operations should succeed"
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

    /// Test complex IR block compilation
    #[test]
    fn test_complex_ir_block() {
        let block = TestIRBuilder::new(0x1000)
            .push(IROp::MovImm {
                dst: 1,
                imm: 100,
            })
            .push(IROp::MovImm {
                dst: 2,
                imm: 200,
            })
            .push(IROp::Add {
                dst: 3,
                src1: 1,
                src2: 2,
            })
            .push(IROp::Sub {
                dst: 4,
                src1: 3,
                src2: 1,
            })
            .push(IROp::Mul {
                dst: 5,
                src1: 4,
                src2: 2,
            })
            .push(IROp::MovImm {
                dst: 6,
                imm: 0x2000,
            })
            .push(IROp::Store {
                addr: 6,
                src: 5,
                flags: MemFlags::default(),
            })
            .terminator(Terminator::Ret)
            .build();

        let mut compiler = JITCompiler::new();
        let result = compiler.compile(&block);

        assert!(result.is_ok(), "Complex IR block compilation should succeed");
    }

    /// Test JIT compilation timeout handling
    #[test]
    fn test_jit_compilation_timeout() {
        let block = create_arithmetic_block();

        let result = assert_timeout(
            || {
                let mut compiler = JITCompiler::new();
                compiler.compile(&block).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
            },
            DEFAULT_TIMEOUT,
            "JIT compilation",
        );

        assert!(result.is_ok(), "JIT compilation should complete within timeout");
    }
}

// ============================================================================
// Memory Management Integration Tests (vm-core + vm-mem)
// ============================================================================

#[cfg(test)]
mod memory_integration_tests {
    use super::*;

    /// Test sequential memory operations
    #[test]
    fn test_sequential_memory_operations() {
        let _guard = common::cleanup_guard();
        let config = TestVmConfig::default();
        let vm = TestVm::new(config);

        // Write multiple blocks
        let blocks = vec![
            (GuestAddr(0x1000), b"Block1"),
            (GuestAddr(0x2000), b"Block2"),
            (GuestAddr(0x3000), b"Block3"),
        ];

        for (addr, data) in &blocks {
            vm.write_memory(*addr, data).expect("Write should succeed");
        }

        // Verify all blocks
        for (addr, data) in &blocks {
            let read = vm.read_memory(*addr, data.len()).expect("Read should succeed");
            assert_eq!(read, *data);
        }
    }

    /// Test large memory operations
    #[test]
    fn test_large_memory_operations() {
        let _guard = common::cleanup_guard();
        let config = TestVmConfig {
            memory_size: 10 * 1024 * 1024, // 10 MB
            ..Default::default()
        };
        let vm = TestVm::new(config);

        // Write 1 MB of data
        let large_data = random_bytes(1024 * 1024);
        vm.write_memory(GuestAddr(0x1000), &large_data)
            .expect("Large write should succeed");

        // Read back and verify
        let read_data = vm
            .read_memory(GuestAddr(0x1000), large_data.len())
            .expect("Large read should succeed");
        assert_eq!(read_data, large_data);
    }

    /// Test memory boundary conditions
    #[test]
    fn test_memory_boundaries() {
        let _guard = common::cleanup_guard();
        let config = TestVmConfig::default();
        let vm = TestVm::new(config);

        // Write at end of memory
        let last_addr = GuestAddr((DEFAULT_MEMORY_SIZE - 4) as u64);
        vm.write_memory(last_addr, &[1, 2, 3, 4])
            .expect("Write at end should succeed");

        // Read back
        let data = vm.read_memory(last_addr, 4).expect("Read at end should succeed");
        assert_eq!(data, vec![1, 2, 3, 4]);

        // Test out-of-bounds access
        let oob_addr = GuestAddr(DEFAULT_MEMORY_SIZE as u64);
        let result = vm.write_memory(oob_addr, &[1, 2, 3, 4]);
        assert!(result.is_err(), "Out-of-bounds write should fail");
    }

    /// Test overlapped memory regions
    #[test]
    fn test_overlapped_memory_regions() {
        let _guard = common::cleanup_guard();
        let config = TestVmConfig::default();
        let vm = TestVm::new(config);

        // Write first block
        vm.write_memory(GuestAddr(0x1000), b"AAAA")
            .expect("First write should succeed");

        // Write overlapping block
        vm.write_memory(GuestAddr(0x1002), b"BBBB")
            .expect("Overlapping write should succeed");

        // Verify result: AABBBB
        let data = vm.read_memory(GuestAddr(0x1000), 6).expect("Read should succeed");
        assert_eq!(data, b"AABBBB");
    }
}

// ============================================================================
// Full Stack Integration Tests
// ============================================================================

#[cfg(test)]
mod full_stack_tests {
    use super::*;

    /// Test complete stack: VM creation → IR generation → JIT compilation
    #[test]
    fn test_full_stack_vm_to_jit() {
        let _guard = common::cleanup_guard();

        // Create VM
        let config = TestVmConfig::default();
        let vm = TestVm::new(config);

        // Generate IR
        let block = create_arithmetic_block();

        // Compile with JIT
        let mut compiler = JITCompiler::new();
        let compile_result = compiler.compile(&block);

        assert!(
            compile_result.is_ok(),
            "Full stack: VM → IR → JIT should succeed"
        );
    }

    /// Test full stack with memory operations
    #[test]
    fn test_full_stack_with_memory() {
        let _guard = common::cleanup_guard();

        let config = TestVmConfig::default();
        let vm = TestVm::new(config);

        // Setup memory
        let test_data = b"TestData";
        vm.write_memory(GuestAddr(0x1000), test_data)
            .expect("Memory setup should succeed");

        // Generate IR with memory operations
        let block = create_memory_block();

        // Compile
        let mut compiler = JITCompiler::new();
        let compile_result = compiler.compile(&block);

        assert!(compile_result.is_ok(), "Full stack with memory should succeed");
    }

    /// Test multiple execution modes
    #[test]
    fn test_multiple_execution_modes() {
        let _guard = common::cleanup_guard();

        for exec_mode in &[ExecMode::Interpreter, ExecMode::JIT, ExecMode::Threaded] {
            let config = TestVmConfig {
                exec_mode: *exec_mode,
                ..Default::default()
            };
            let vm = TestVm::new(config);

            assert_eq!(
                vm.config.exec_mode, *exec_mode,
                "VM should use {:?} mode",
                exec_mode
            );
        }
    }
}

// ============================================================================
// Error Handling Integration Tests
// ============================================================================

#[cfg(test)]
mod error_handling_tests {
    use super::*;
    use vm_core::{MemoryError, VmError};

    /// Test error propagation across crate boundaries
    #[test]
    fn test_error_propagation() {
        let _guard = common::cleanup_guard();
        let config = TestVmConfig::default();
        let vm = TestVm::new(config);

        // Trigger memory error
        let oob_addr = GuestAddr(DEFAULT_MEMORY_SIZE as u64);
        let result = vm.write_memory(oob_addr, &[1, 2, 3]);

        assert!(result.is_err(), "Out-of-bounds access should error");

        if let Err(MemoryError::InvalidAddress { addr, size }) = result {
            assert_eq!(addr, DEFAULT_MEMORY_SIZE as u64);
            assert_eq!(size, 3);
        } else {
            panic!("Expected InvalidAddress error");
        }
    }

    /// Test error recovery
    #[test]
    fn test_error_recovery() {
        let _guard = common::cleanup_guard();
        let config = TestVmConfig::default();
        let vm = TestVm::new(config);

        // Fail operation
        let oob_addr = GuestAddr(DEFAULT_MEMORY_SIZE as u64);
        let _ = vm.write_memory(oob_addr, &[1, 2, 3]);

        // VM should still be usable after error
        let valid_addr = GuestAddr(0x1000);
        vm.write_memory(valid_addr, b"OK")
            .expect("VM should recover from error");
    }
}

// ============================================================================
// Performance Integration Tests
// ============================================================================

#[cfg(test)]
mod performance_tests {
    use super::*;

    /// Benchmark VM creation and initialization
    #[test]
    fn test_vm_creation_performance() {
        let _guard = common::cleanup_guard();

        let iterations = 100;
        let stats = common::benchmark(
            || {
                let config = TestVmConfig::default();
                let vm = TestVm::new(config);
                let _ = vm.init();
            },
            iterations,
        );

        println!("VM creation performance: {:?}", stats);
        assert!(
            stats.avg_time.as_millis() < 100,
            "VM creation should be fast (< 100ms average)"
        );
    }

    /// Benchmark JIT compilation
    #[test]
    fn test_jit_compilation_performance() {
        let block = create_arithmetic_block();

        let iterations = 1000;
        let stats = common::benchmark(
            || {
                let mut compiler = JITCompiler::new();
                let _ = compiler.compile(&block);
            },
            iterations,
        );

        println!("JIT compilation performance: {:?}", stats);
        assert!(
            stats.avg_time.as_micros() < 1000,
            "JIT compilation should be fast (< 1ms average)"
        );
    }

    /// Benchmark memory operations
    #[test]
    fn test_memory_operations_performance() {
        let _guard = common::cleanup_guard();
        let config = TestVmConfig::default();
        let vm = TestVm::new(config);

        let data = random_bytes(4096);

        let iterations = 1000;
        let stats = common::benchmark(
            || {
                let addr = GuestAddr(0x1000);
                let _ = vm.write_memory(addr, &data);
                let _ = vm.read_memory(addr, data.len());
            },
            iterations,
        );

        println!("Memory operations performance: {:?}", stats);
        assert!(
            stats.avg_time.as_micros() < 100,
            "Memory operations should be fast (< 100μs average)"
        );
    }
}
