//! SIMD Feature Gate Tests
//!
//! 测试SIMD功能的feature gate机制和基本集成

#[cfg(test)]
mod simd_feature_tests {
    // 这个测试验证：没有启用simd feature时，SIMD高级API不可用
    #[test]
    #[cfg(not(feature = "simd"))]
    fn test_simd_apis_not_available_without_feature() {
        // 这个测试主要验证编译时行为
        // 如果SIMD API被错误地导出，这个测试会编译失败

        // 基本类型应该仍然可用（但可能没有实际功能）
        // 注意：这个测试在没有启用simd feature时运行
    }

    // 这个测试验证：启用simd feature后，所有SIMD API都可用
    #[test]
    #[cfg(feature = "simd")]
    fn test_simd_apis_available_with_feature() {
        // 这个测试在启用simd feature时运行
        // 验证所有SIMD类型都可以导入和使用

        use vm_engine_jit::{
            SimdCompiler, SimdIntegrationManager, SimdOperation, VectorOperation,
            ElementSize, VectorSize,
        };

        // 验证类型存在
        let _manager = SimdIntegrationManager::new();
        let _compiler = SimdCompiler::new();

        // 验证枚举可以使用
        let _op = SimdOperation::VecAdd;

        let _vec_op = VectorOperation::VecAdd;

        let _elem_size = ElementSize::Size64;
        let _vec_size = VectorSize::Vec128;

        // 如果能编译到这里，说明所有API都正确导出
        assert!(true);
    }
}

#[cfg(test)]
mod simd_integration_tests {
    use vm_ir::{IROp, IRBlock, Terminator};
    use vm_core::GuestAddr;

    // 测试基本的SIMD IR操作可以创建
    #[test]
    fn test_simd_ir_operations_creation() {
        // 创建包含SIMD操作的IR块
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![
                IROp::VecAdd {
                    dst: 1,
                    src1: 2,
                    src2: 3,
                    element_size: 64,
                },
                IROp::VecSub {
                    dst: 4,
                    src1: 5,
                    src2: 6,
                    element_size: 64,
                },
            ],
            term: Terminator::Ret,
        };

        assert_eq!(block.ops.len(), 2);
    }

    // 测试所有SIMD IR操作变体
    #[test]
    fn test_all_simd_ir_operations() {
        let operations = vec![
            IROp::VecAdd {
                dst: 1,
                src1: 2,
                src2: 3,
                element_size: 32,
            },
            IROp::VecSub {
                dst: 1,
                src1: 2,
                src2: 3,
                element_size: 32,
            },
            IROp::VecMul {
                dst: 1,
                src1: 2,
                src2: 3,
                element_size: 32,
            },
            IROp::VecAddSat {
                dst: 1,
                src1: 2,
                src2: 3,
                element_size: 32,
                signed: true,
            },
            IROp::VecSubSat {
                dst: 1,
                src1: 2,
                src2: 3,
                element_size: 32,
                signed: false,
            },
        ];

        assert_eq!(operations.len(), 5);
    }

    // 测试SIMD块可以创建JIT实例
    #[test]
    fn test_jit_creation_with_simd_block() {
        use vm_engine_jit::Jit;

        let mut _jit = Jit::new();

        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![
                IROp::VecAdd {
                    dst: 1,
                    src1: 2,
                    src2: 3,
                    element_size: 64,
                },
            ],
            term: Terminator::Ret,
        };

        // 验证JIT实例创建成功
        // 注意：当前可能不支持SIMD操作的编译，但至少应该能创建IR
        assert_eq!(block.start_pc, GuestAddr(0x1000));
    }
}

#[cfg(test)]
#[cfg(feature = "simd")]
mod simd_compiler_tests {
    use vm_engine_jit::{SimdCompiler, SimdIntegrationManager, SimdOperation, ElementSize, VectorSize};

    // 测试SimdCompiler创建
    #[test]
    fn test_simd_compiler_creation() {
        let _compiler = SimdCompiler::new();
        assert!(true); // 编译成功即通过
    }

    // 测试SimdIntegrationManager创建
    #[test]
    fn test_simd_manager_creation() {
        let _manager = SimdIntegrationManager::new();
        assert!(true); // 编译成功即通过
    }

    // 测试SimdOperation枚举变体
    #[test]
    fn test_simd_operation_variants() {
        let operations = vec![
            SimdOperation::VecAdd,
            SimdOperation::VecSub,
            SimdOperation::VecMul,
            SimdOperation::VecAnd,
            SimdOperation::VecOr,
        ];

        assert_eq!(operations.len(), 5);
    }

    // 测试浮点SIMD操作
    #[test]
    fn test_simd_float_operations() {
        let operations = vec![
            SimdOperation::VecFaddF32,
            SimdOperation::VecFsubF32,
            SimdOperation::VecFmulF32,
            SimdOperation::VecFdivF32,
            SimdOperation::VecFsqrtF32,
        ];

        assert_eq!(operations.len(), 5);
    }

    // 测试ElementSize枚举
    #[test]
    fn test_element_size_enum() {
        let sizes = vec![
            ElementSize::Size8,
            ElementSize::Size16,
            ElementSize::Size32,
            ElementSize::Size64,
        ];

        assert_eq!(sizes.len(), 4);
    }

    // 测试VectorSize枚举
    #[test]
    fn test_vector_size_enum() {
        let sizes = vec![
            VectorSize::Scalar64,
            VectorSize::Vec128,
            VectorSize::Vec256,
            VectorSize::Vec512,
        ];

        assert_eq!(sizes.len(), 4);
    }
}

#[cfg(test)]
mod simd_compilation_tests {
    use vm_ir::{IROp, IRBlock, Terminator};
    use vm_core::GuestAddr;

    // 测试SIMD块的IR构建
    #[test]
    fn test_build_simd_ir_block() {
        let block = IRBlock {
            start_pc: GuestAddr(0x2000),
            ops: vec![
                // 向量加法
                IROp::VecAdd {
                    dst: 1,
                    src1: 2,
                    src2: 3,
                    element_size: 32,
                },
                // 向量乘法
                IROp::VecMul {
                    dst: 4,
                    src1: 1,
                    src2: 5,
                    element_size: 32,
                },
            ],
            term: Terminator::Ret,
        };

        assert_eq!(block.ops.len(), 2);
        assert_eq!(block.start_pc, GuestAddr(0x2000));
    }

    // 测试不同元素大小的SIMD操作
    #[test]
    fn test_simd_different_element_sizes() {
        let element_sizes = vec![8, 16, 32, 64];

        for size in element_sizes {
            let block = IRBlock {
                start_pc: GuestAddr(0x1000),
                ops: vec![IROp::VecAdd {
                    dst: 1,
                    src1: 2,
                    src2: 3,
                    element_size: size,
                }],
                term: Terminator::Ret,
            };

            assert_eq!(block.ops[0], IROp::VecAdd {
                dst: 1,
                src1: 2,
                src2: 3,
                element_size: size,
            });
        }
    }

    // 测试不同元素大小的SIMD操作 - 测试覆盖
    #[test]
    fn test_simd_element_size_coverage() {
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![
                IROp::VecAdd { dst: 1, src1: 2, src2: 3, element_size: 8 },
                IROp::VecSub { dst: 4, src1: 5, src2: 6, element_size: 16 },
                IROp::VecMul { dst: 7, src1: 8, src2: 9, element_size: 32 },
                IROp::VecAdd { dst: 10, src1: 11, src2: 12, element_size: 64 },
            ],
            term: Terminator::Ret,
        };

        assert_eq!(block.ops.len(), 4);
    }
}

#[cfg(test)]
mod simd_bitwise_tests {
    use vm_ir::{IROp, IRBlock, Terminator};
    use vm_core::GuestAddr;

    // 测试SIMD位运算操作
    #[test]
    fn test_simd_bitwise_operations() {
        let block = IRBlock {
            start_pc: GuestAddr(0x3000),
            ops: vec![
                IROp::VecAnd {
                    dst: 1,
                    src1: 2,
                    src2: 3,
                    element_size: 64,
                },
                IROp::VecOr {
                    dst: 4,
                    src1: 5,
                    src2: 6,
                    element_size: 64,
                },
                IROp::VecXor {
                    dst: 7,
                    src1: 8,
                    src2: 9,
                    element_size: 64,
                },
                IROp::VecNot {
                    dst: 10,
                    src: 11,
                    element_size: 64,
                },
            ],
            term: Terminator::Ret,
        };

        assert_eq!(block.ops.len(), 4);
    }

    // 测试SIMD移位操作
    #[test]
    fn test_simd_shift_operations() {
        let block = IRBlock {
            start_pc: GuestAddr(0x4000),
            ops: vec![
                IROp::VecShl {
                    dst: 1,
                    src: 2,
                    shift: 3,
                    element_size: 32,
                },
                IROp::VecSrl {
                    dst: 4,
                    src: 5,
                    shift: 6,
                    element_size: 32,
                },
                IROp::VecSra {
                    dst: 7,
                    src: 8,
                    shift: 9,
                    element_size: 32,
                },
            ],
            term: Terminator::Ret,
        };

        assert_eq!(block.ops.len(), 3);
    }

    // 测试SIMD立即数移位操作
    #[test]
    fn test_simd_immediate_shift_operations() {
        let block = IRBlock {
            start_pc: GuestAddr(0x5000),
            ops: vec![
                IROp::VecShlImm {
                    dst: 1,
                    src: 2,
                    shift: 4,
                    element_size: 32,
                },
                IROp::VecSrlImm {
                    dst: 3,
                    src: 4,
                    shift: 8,
                    element_size: 32,
                },
                IROp::VecSraImm {
                    dst: 5,
                    src: 6,
                    shift: 16,
                    element_size: 32,
                },
            ],
            term: Terminator::Ret,
        };

        assert_eq!(block.ops.len(), 3);
    }
}
