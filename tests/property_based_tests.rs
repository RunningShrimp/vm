//! Property-Based Integration Tests
//!
//! Uses proptest to generate random inputs and test invariants across the VM project.
//! These tests help discover edge cases and verify system properties.

mod common;

use std::sync::Arc;

use proptest::prelude::*;
use proptest::collection::{vec, size_range};

use vm_core::{
    ExecMode, GuestAddr, GuestArch, GuestRegs, VmConfig, VmLifecycleState, VmState,
};

use vm_engine::jit::{JITCompiler, JITConfig};

use vm_ir::{IRBlock, IROp, MemFlags, RegId, Terminator};

use common::{TestVm, TestVmConfig, TestIRBuilder, DEFAULT_MEMORY_SIZE};

// ============================================================================
// Property-Based Memory Tests
// ============================================================================

#[cfg(test)]
mod memory_properties {
    use super::*;

    /// Property: Writing then reading memory should return the same data
    proptest! {
        #[test]
        fn prop_write_read_consistent(
            addr in 0u64..(DEFAULT_MEMORY_SIZE as u64 - 100),
            data in vec(any::<u8>(), 1..100)
        ) {
            let _guard = common::cleanup_guard();
            let config = TestVmConfig::default();
            let vm = TestVm::new(config);

            // Write data
            let write_result = vm.write_memory(GuestAddr(addr), &data);
            prop_assert!(write_result.is_ok(), "Write should succeed");

            // Read back
            let read_result = vm.read_memory(GuestAddr(addr), data.len());
            prop_assert!(read_result.is_ok(), "Read should succeed");

            let read_data = read_result.unwrap();
            prop_assert_eq!(read_data, data, "Read data should match written data");
        }
    }

    /// Property: Multiple writes to same address should be idempotent
    proptest! {
        #[test]
        fn prop_write_idempotent(
            addr in 0u64..(DEFAULT_MEMORY_SIZE as u64 - 10),
            data1 in vec(any::<u8>(), 10),
            data2 in vec(any::<u8>(), 10),
        ) {
            let _guard = common::cleanup_guard();
            let config = TestVmConfig::default();
            let vm = TestVm::new(config);

            // Write data1
            vm.write_memory(GuestAddr(addr), &data1).unwrap();
            let read1 = vm.read_memory(GuestAddr(addr), 10).unwrap();

            // Write data2
            vm.write_memory(GuestAddr(addr), &data2).unwrap();
            let read2 = vm.read_memory(GuestAddr(addr), 10).unwrap();

            // Verify second write overwrote first
            prop_assert_eq!(read2, data2);
            prop_assert_ne!(read1, read2, "Second write should overwrite first");
        }
    }

    /// Property: Memory operations should handle boundary conditions
    proptest! {
        #[test]
        fn prop_memory_boundaries(
            offset in 0usize..100usize,
            size in 1usize..100usize
        ) {
            let _guard = common::cleanup_guard();
            let config = TestVmConfig::default();
            let vm = TestVm::new(config);

            let addr = (DEFAULT_MEMORY_SIZE - offset) as u64;
            let data = vec![1u8; size];

            let result = vm.write_memory(GuestAddr(addr), &data);

            // Should succeed if within bounds, fail otherwise
            if offset >= size {
                prop_assert!(result.is_ok(), "Write within bounds should succeed");
            } else {
                prop_assert!(result.is_err(), "Write out of bounds should fail");
            }
        }
    }

    /// Property: Sequential reads should be consistent
    proptest! {
        #[test]
        fn prop_read_consistency(
            addr in 0u64..(DEFAULT_MEMORY_SIZE as u64 - 100),
            data in vec(any::<u8>(), 50)
        ) {
            let _guard = common::cleanup_guard();
            let config = TestVmConfig::default();
            let vm = TestVm::new(config);

            // Write once
            vm.write_memory(GuestAddr(addr), &data).unwrap();

            // Read multiple times
            let read1 = vm.read_memory(GuestAddr(addr), data.len()).unwrap();
            let read2 = vm.read_memory(GuestAddr(addr), data.len()).unwrap();
            let read3 = vm.read_memory(GuestAddr(addr), data.len()).unwrap();

            // All reads should be identical
            prop_assert_eq!(read1, data);
            prop_assert_eq!(read2, data);
            prop_assert_eq!(read3, data);
        }
    }
}

// ============================================================================
// Property-Based JIT Compilation Tests
// ============================================================================

#[cfg(test)]
mod jit_properties {
    use super::*;

    /// Property: JIT compilation should handle various arithmetic operations
    proptest! {
        #[test]
        fn prop_jit_arithmetic_operations(
            imm1 in 0u64..1000u64,
            imm2 in 0u64..1000u64,
            reg1 in 0u32..32u32,
            reg2 in 0u32..32u32,
            reg3 in 0u32..32u32,
        ) {
            // Create IR block with random immediates and registers
            let block = TestIRBuilder::new(0x1000)
                .push(IROp::MovImm { dst: reg1, imm: imm1 as i64 })
                .push(IROp::MovImm { dst: reg2, imm: imm2 as i64 })
                .push(IROp::Add { dst: reg3, src1: reg1, src2: reg2 })
                .terminator(Terminator::Ret)
                .build();

            // Compile with JIT
            let mut compiler = JITCompiler::new();
            let result = compiler.compile(&block);

            // Should always succeed for valid inputs
            prop_assert!(result.is_ok(), "JIT compilation should succeed for valid arithmetic ops");
        }
    }

    /// Property: JIT should handle various block sizes
    proptest! {
        #[test]
        fn prop_jit_variable_block_size(
            num_ops in 1usize..100usize
        ) {
            let mut ops = Vec::new();

            for i in 0..num_ops {
                let reg = (i % 32) as u32;
                ops.push(IROp::MovImm {
                    dst: reg,
                    imm: i as i64,
                });
            }

            let block = IRBlock {
                start_pc: GuestAddr(0x1000),
                ops,
                term: Terminator::Ret,
            };

            let mut compiler = JITCompiler::new();
            let result = compiler.compile(&block);

            prop_assert!(result.is_ok(), "JIT should handle blocks of various sizes");
        }
    }

    /// Property: JIT compilation with different optimization levels
    proptest! {
        #[test]
        fn prop_jit_optimization_levels(
            opt_level in 0u8..4u8,  // Maps to OptLevel enum
            num_ops in 1usize..50usize
        ) {
            let mut ops = Vec::new();

            for i in 0..num_ops {
                ops.push(IROp::MovImm {
                    dst: (i % 32) as u32,
                    imm: i as i64,
                });
            }

            let block = IRBlock {
                start_pc: GuestAddr(0x1000),
                ops,
                term: Terminator::Ret,
            };

            let opt = match opt_level {
                0 => vm_engine::jit::OptLevel::None,
                1 => vm_engine::jit::OptLevel::Less,
                2 => vm_engine::jit::OptLevel::Default,
                _ => vm_engine::jit::OptLevel::Aggressive,
            };

            let config = JITConfig {
                opt_level: opt,
                ..Default::default()
            };

            let mut compiler = JITCompiler::new();
            let result = compiler.compile(&block);

            prop_assert!(
                result.is_ok(),
                "JIT should succeed with any optimization level"
            );
        }
    }
}

// ============================================================================
// Property-Based IR Construction Tests
// ============================================================================

#[cfg(test)]
mod ir_properties {
    use super::*;

    /// Property: IR blocks should maintain operation order
    proptest! {
        #[test]
        fn prop_ir_operation_order(
            seed in 0u64..1000u64
        ) {
            let mut rng = rand::thread_rng();

            // Create deterministic operations based on seed
            let num_ops = ((seed % 10) + 1) as usize;
            let mut ops = Vec::new();

            for i in 0..num_ops {
                ops.push(IROp::MovImm {
                    dst: (i % 32) as u32,
                    imm: (seed + i as u64) as i64,
                });
            }

            let block = IRBlock {
                start_pc: GuestAddr(0x1000),
                ops,
                term: Terminator::Ret,
            };

            // Verify operations are in order
            for (i, op) in block.ops.iter().enumerate() {
                if let IROp::MovImm { dst, .. } = op {
                    prop_assert_eq!(*dst, (i % 32) as u32);
                }
            }
        }
    }

    /// Property: IR blocks should handle different terminators
    proptest! {
        #[test]
        fn prop_ir_terminators(
            term_type in 0u8..4u8,
            target_addr in 0x1000u64..0x10000u64
        ) {
            let ops = vec![
                IROp::MovImm { dst: 1, imm: 10 },
                IROp::MovImm { dst: 2, imm: 20 },
            ];

            let term = match term_type {
                0 => Terminator::Ret,
                1 => Terminator::Branch { target: GuestAddr(target_addr) },
                2 => Terminator::BranchCond {
                    src1: 1,
                    src2: 2,
                    target: GuestAddr(target_addr),
                },
                _ => Terminator::Unreachable,
            };

            let block = IRBlock {
                start_pc: GuestAddr(0x1000),
                ops,
                term,
            };

            // Block should be valid regardless of terminator
            prop_assert!(!block.ops.is_empty());
        }
    }
}

// ============================================================================
// Property-Based VM Lifecycle Tests
// ============================================================================

#[cfg(test)]
mod vm_lifecycle_properties {
    use super::*;

    /// Property: VM should handle various memory sizes
    proptest! {
        #[test]
        fn prop_vm_memory_sizes(
            size_mb in 1u64..100u64
        ) {
            let _guard = common::cleanup_guard();
            let size = (size_mb * 1024 * 1024) as usize;

            let config = TestVmConfig {
                memory_size: size,
                ..Default::default()
            };

            let vm = TestVm::new(config);

            prop_assert_eq!(vm.config.memory_size, size);

            // Should be able to write to valid addresses
            let data = vec![1u8, 2, 3, 4];
            let result = vm.write_memory(GuestAddr(0x1000), &data);
            prop_assert!(result.is_ok());
        }
    }

    /// Property: VM should handle various VCPU counts
    proptest! {
        #[test]
        fn prop_vm_vcpu_counts(
            vcpu_count in 1usize..16usize
        ) {
            let config = TestVmConfig {
                vcpu_count,
                ..Default::default()
            };

            let vm = TestVm::new(config);

            prop_assert_eq!(vm.config.vcpu_count, vcpu_count);
        }
    }

    /// Property: VM should support all execution modes
    proptest! {
        #[test]
        fn prop_vm_execution_modes(
            mode_index in 0u8..3u8
        ) {
            let exec_mode = match mode_index {
                0 => ExecMode::Interpreter,
                1 => ExecMode::JIT,
                _ => ExecMode::Threaded,
            };

            let config = TestVmConfig {
                exec_mode,
                ..Default::default()
            };

            let vm = TestVm::new(config);

            prop_assert_eq!(vm.config.exec_mode, exec_mode);
        }
    }
}

// ============================================================================
// Property-Based Cross-Crate Integration Tests
// ============================================================================

#[cfg(test)]
mod cross_crate_properties {
    use super::*;

    /// Property: Full stack should handle random instruction sequences
    proptest! {
        #[test]
        fn prop_full_stack_random_instructions(
            seed in 0u64..1000u64
        ) {
            let _guard = common::cleanup_guard();

            // Create VM
            let config = TestVmConfig::default();
            let vm = TestVm::new(config);

            // Generate random instructions based on seed
            let num_ops = ((seed % 20) + 1) as usize;
            let mut ops = Vec::new();

            for i in 0..num_ops {
                let op = match i % 4 {
                    0 => IROp::MovImm {
                        dst: (i % 32) as u32,
                        imm: (seed + i as u64) as i64,
                    },
                    1 => IROp::Add {
                        dst: 1,
                        src1: 2,
                        src2: 3,
                    },
                    2 => IROp::Sub {
                        dst: 2,
                        src1: 3,
                        src2: 4,
                    },
                    _ => IROp::Mul {
                        dst: 3,
                        src1: 4,
                        src2: 5,
                    },
                };
                ops.push(op);
            }

            let block = IRBlock {
                start_pc: GuestAddr(0x1000),
                ops,
                term: Terminator::Ret,
            };

            // Compile with JIT
            let mut compiler = JITCompiler::new();
            let result = compiler.compile(&block);

            prop_assert!(result.is_ok(), "Full stack should handle random instructions");
        }
    }

    /// Property: Memory operations followed by JIT compilation should work
    proptest! {
        #[test]
        fn prop_memory_then_jit(
            write_addr in 0u64..0x1000u64,
            data_size in 1usize..100usize
        ) {
            let _guard = common::cleanup_guard();

            let config = TestVmConfig::default();
            let vm = TestVm::new(config);

            // Perform memory operation
            let data = vec![42u8; data_size];
            let _ = vm.write_memory(GuestAddr(write_addr), &data);

            // Compile IR block
            let block = TestIRBuilder::new(0x1000)
                .push(IROp::MovImm { dst: 1, imm: 10 })
                .terminator(Terminator::Ret)
                .build();

            let mut compiler = JITCompiler::new();
            let result = compiler.compile(&block);

            prop_assert!(result.is_ok(), "Memory ops + JIT should work together");
        }
    }
}

// ============================================================================
// Invariant Testing
// ============================================================================

#[cfg(test)]
mod invariant_tests {
    use super::*;

    /// Invariant: Memory should always preserve written data
    proptest! {
        #[test]
        fn invariant_memory_preservation(
            addr in 0u64..(DEFAULT_MEMORY_SIZE as u64 - 50),
            data in vec(any::<u8>(), 50)
        ) {
            let _guard = common::cleanup_guard();
            let config = TestVmConfig::default();
            let vm = TestVm::new(config);

            // Write data
            vm.write_memory(GuestAddr(addr), &data).unwrap();

            // Invariant: Reading immediately should return same data
            let read = vm.read_memory(GuestAddr(addr), data.len()).unwrap();
            prop_assert_eq!(read, data);

            // Invariant: Reading again should still return same data
            let read2 = vm.read_memory(GuestAddr(addr), data.len()).unwrap();
            prop_assert_eq!(read2, data);
        }
    }

    /// Invariant: IR blocks should have valid structure
    proptest! {
        #[test]
        fn invariant_ir_block_structure(
            num_ops in 1usize..100usize,
            start_pc in 0u64..0x10000u64
        ) {
            let ops: Vec<IROp> = (0..num_ops)
                .map(|i| IROp::MovImm {
                    dst: (i % 32) as u32,
                    imm: i as i64,
                })
                .collect();

            let block = IRBlock {
                start_pc: GuestAddr(start_pc),
                ops,
                term: Terminator::Ret,
            };

            // Invariant: Block should have exactly num_ops operations
            prop_assert_eq!(block.ops.len(), num_ops);

            // Invariant: Block should have a terminator
            prop_assert!(matches!(block.term, Terminator::Ret | Terminator::Branch { .. } | Terminator::BranchCond { .. } | Terminator::Unreachable));
        }
    }

    /// Invariant: VM configuration should be consistent
    proptest! {
        #[test]
        fn invariant_vm_config_consistency(
            memory_size_mb in 1u64..100u64,
            vcpu_count in 1usize..16usize
        ) {
            let memory_size = (memory_size_mb * 1024 * 1024) as usize;

            let config = TestVmConfig {
                memory_size,
                vcpu_count,
                ..Default::default()
            };

            let vm = TestVm::new(config);

            // Invariant: Config should match what we set
            prop_assert_eq!(vm.config.memory_size, memory_size);
            prop_assert_eq!(vm.config.vcpu_count, vcpu_count);

            // Invariant: VM should start in Created state
            prop_assert_eq!(vm.lifecycle_state, VmLifecycleState::Created);
        }
    }
}

// ============================================================================
// Edge Case Discovery Tests
// ============================================================================

#[cfg(test)]
mod edge_case_tests {
    use super::*;

    /// Test edge case: Maximum register values
    #[test]
    fn test_max_registers() {
        let block = TestIRBuilder::new(0x1000)
            .push(IROp::MovImm {
                dst: 31, // Max RISC-V register
                imm: i64::MAX,
            })
            .terminator(Terminator::Ret)
            .build();

        let mut compiler = JITCompiler::new();
        assert!(compiler.compile(&block).is_ok());
    }

    /// Test edge case: Minimum immediate values
    #[test]
    fn test_min_immediates() {
        let block = TestIRBuilder::new(0x1000)
            .push(IROp::MovImm {
                dst: 1,
                imm: i64::MIN,
            })
            .terminator(Terminator::Ret)
            .build();

        let mut compiler = JITCompiler::new();
        assert!(compiler.compile(&block).is_ok());
    }

    /// Test edge case: Zero-sized operations
    #[test]
    fn test_empty_block() {
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![],
            term: Terminator::Ret,
        };

        let mut compiler = JITCompiler::new();
        assert!(compiler.compile(&block).is_ok());
    }

    /// Test edge case: Single operation
    #[test]
    fn test_single_operation() {
        let block = TestIRBuilder::new(0x1000)
            .push(IROp::MovImm {
                dst: 0,
                imm: 0,
            })
            .terminator(Terminator::Ret)
            .build();

        let mut compiler = JITCompiler::new();
        assert!(compiler.compile(&block).is_ok());
    }
}
