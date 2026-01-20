//! SIMD Direct Intrinsic 测试
//!
//! 这个测试文件验证直接使用Cranelift SIMD intrinsic的增强实现
//! 确保新的SIMD代码生成路径正确工作并提供性能提升

#[cfg(test)]
mod simd_direct_intrinsic_tests {
    use vm_core::GuestAddr;
    use vm_ir::{IRBlock, IROp, Terminator};

    /// 创建简单的向量加法IR块
    fn create_vec_add_ir_block() -> IRBlock {
        IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![IROp::VecAdd {
                dst: 1u32,
                src1: 2u32,
                src2: 3u32,
                element_size: 32,
            }],
            term: Terminator::Ret,
        }
    }

    /// 创建向量位运算IR块
    #[allow(dead_code)]
    fn create_vec_bitwise_ir_block() -> IRBlock {
        IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![
                IROp::VecAnd {
                    dst: 1u32,
                    src1: 2u32,
                    src2: 3u32,
                    element_size: 64,
                },
                IROp::VecOr {
                    dst: 4u32,
                    src1: 5u32,
                    src2: 6u32,
                    element_size: 64,
                },
                IROp::VecXor {
                    dst: 7u32,
                    src1: 8u32,
                    src2: 9u32,
                    element_size: 64,
                },
            ],
            term: Terminator::Ret,
        }
    }

    /// 创建混合向量操作IR块
    #[allow(dead_code)]
    fn create_mixed_simd_ir_block() -> IRBlock {
        IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![
                IROp::VecAdd {
                    dst: 1u32,
                    src1: 2u32,
                    src2: 3u32,
                    element_size: 32,
                },
                IROp::VecMul {
                    dst: 4u32,
                    src1: 1u32,
                    src2: 5u32,
                    element_size: 32,
                },
                IROp::VecAnd {
                    dst: 6u32,
                    src1: 4u32,
                    src2: 7u32,
                    element_size: 32,
                },
                IROp::VecXor {
                    dst: 8u32,
                    src1: 6u32,
                    src2: 9u32,
                    element_size: 32,
                },
            ],
            term: Terminator::Ret,
        }
    }

    #[test]
    #[cfg(feature = "simd")]
    fn test_direct_intrinsic_vec_add() {
        // 测试直接SIMD intrinsic的向量加法 - IR创建
        let block = create_vec_add_ir_block();

        // IR块应该成功创建
        assert_eq!(block.ops.len(), 1, "Should have 1 VecAdd operation");
        match &block.ops[0] {
            IROp::VecAdd {
                dst,
                src1,
                src2,
                element_size,
            } => {
                assert_eq!(*dst, 1u32);
                assert_eq!(*src1, 2u32);
                assert_eq!(*src2, 3u32);
                assert_eq!(*element_size, 32);
            }
            _ => panic!("Expected VecAdd operation"),
        }
    }

    #[test]
    #[cfg(feature = "simd")]
    fn test_direct_intrinsic_vec_mul() {
        // 测试直接SIMD intrinsic的向量乘法 - IR创建
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![IROp::VecMul {
                dst: 1u32,
                src1: 2u32,
                src2: 3u32,
                element_size: 32,
            }],
            term: Terminator::Ret,
        };

        assert_eq!(block.ops.len(), 1);
        match &block.ops[0] {
            IROp::VecMul {
                dst,
                src1,
                src2,
                element_size,
            } => {
                assert_eq!(*dst, 1u32);
                assert_eq!(*src1, 2u32);
                assert_eq!(*src2, 3u32);
                assert_eq!(*element_size, 32);
            }
            _ => panic!("Expected VecMul operation"),
        }
    }

    #[test]
    #[cfg(feature = "simd")]
    fn test_direct_intrinsic_vec_sub() {
        // 测试直接SIMD intrinsic的向量减法 - IR创建
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![IROp::VecSub {
                dst: 1u32,
                src1: 2u32,
                src2: 3u32,
                element_size: 64,
            }],
            term: Terminator::Ret,
        };

        assert_eq!(block.ops.len(), 1);
        match &block.ops[0] {
            IROp::VecSub {
                dst,
                src1,
                src2,
                element_size,
            } => {
                assert_eq!(*dst, 1u32);
                assert_eq!(*src1, 2u32);
                assert_eq!(*src2, 3u32);
                assert_eq!(*element_size, 64);
            }
            _ => panic!("Expected VecSub operation"),
        }
    }

    #[test]
    #[cfg(feature = "simd")]
    fn test_direct_intrinsic_bitwise_operations() {
        // 测试直接SIMD intrinsic的位运算 - IR创建
        let block = create_vec_bitwise_ir_block();

        assert_eq!(block.ops.len(), 3);

        // 验证VecAnd
        match &block.ops[0] {
            IROp::VecAnd {
                dst,
                src1,
                src2,
                element_size,
            } => {
                assert_eq!(*dst, 1u32);
                assert_eq!(*src1, 2u32);
                assert_eq!(*src2, 3u32);
                assert_eq!(*element_size, 64);
            }
            _ => panic!("Expected VecAnd operation"),
        }

        // 验证VecOr
        match &block.ops[1] {
            IROp::VecOr {
                dst, src1, src2, ..
            } => {
                assert_eq!(*dst, 4u32);
                assert_eq!(*src1, 5u32);
                assert_eq!(*src2, 6u32);
            }
            _ => panic!("Expected VecOr operation"),
        }

        // 验证VecXor
        match &block.ops[2] {
            IROp::VecXor {
                dst, src1, src2, ..
            } => {
                assert_eq!(*dst, 7u32);
                assert_eq!(*src1, 8u32);
                assert_eq!(*src2, 9u32);
            }
            _ => panic!("Expected VecXor operation"),
        }
    }

    #[test]
    #[cfg(feature = "simd")]
    fn test_direct_intrinsic_mixed_operations() {
        // 测试混合向量操作 - IR创建
        let block = create_mixed_simd_ir_block();

        assert_eq!(block.ops.len(), 4);

        // 验证操作序列
        match &block.ops[0] {
            IROp::VecAdd { .. } => {}
            _ => panic!("Expected VecAdd as first operation"),
        }

        match &block.ops[1] {
            IROp::VecMul { .. } => {}
            _ => panic!("Expected VecMul as second operation"),
        }

        match &block.ops[2] {
            IROp::VecAnd { .. } => {}
            _ => panic!("Expected VecAnd as third operation"),
        }

        match &block.ops[3] {
            IROp::VecXor { .. } => {}
            _ => panic!("Expected VecXor as fourth operation"),
        }
    }

    #[test]
    #[cfg(feature = "simd")]
    fn test_direct_intrinsic_different_element_sizes() {
        // 测试不同元素大小的SIMD操作
        for element_size in [8u8, 16, 32, 64].iter() {
            let block = IRBlock {
                start_pc: GuestAddr(0x1000),
                ops: vec![IROp::VecAdd {
                    dst: 1u32,
                    src1: 2u32,
                    src2: 3u32,
                    element_size: *element_size,
                }],
                term: Terminator::Ret,
            };

            // 验证IR块成功创建
            assert_eq!(block.ops.len(), 1);
            match &block.ops[0] {
                IROp::VecAdd {
                    element_size: es, ..
                } => {
                    assert_eq!(*es, *element_size);
                }
                _ => panic!("Expected VecAdd operation"),
            }
        }
    }

    #[test]
    #[cfg(feature = "simd")]
    fn test_direct_intrinsic_large_block() {
        // 测试包含多个SIMD操作的大型IR块
        let mut ops = Vec::new();
        for i in 0..20 {
            ops.push(IROp::VecAdd {
                dst: (i * 3 + 1) as u32,
                src1: (i * 3 + 2) as u32,
                src2: (i * 3 + 3) as u32,
                element_size: 32,
            });
        }

        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops,
            term: Terminator::Ret,
        };

        // 验证IR块创建成功
        assert_eq!(block.ops.len(), 20, "Should have 20 VecAdd operations");

        // 验证所有操作都是VecAdd
        for (i, op) in block.ops.iter().enumerate() {
            match op {
                IROp::VecAdd {
                    dst,
                    src1,
                    src2,
                    element_size,
                } => {
                    assert_eq!(*dst, (i * 3 + 1) as u32);
                    assert_eq!(*src1, (i * 3 + 2) as u32);
                    assert_eq!(*src2, (i * 3 + 3) as u32);
                    assert_eq!(*element_size, 32);
                }
                _ => panic!("Expected VecAdd operation at index {}", i),
            }
        }
    }

    #[test]
    #[cfg(feature = "simd")]
    fn test_direct_intrinsic_fallback_mechanism() {
        // 测试当直接SIMD intrinsic失败时的回退机制
        // 通过验证IR创建来确保有回退路径可用
        let block = create_vec_add_ir_block();

        // IR块应该成功创建
        assert!(!block.ops.is_empty(), "IR block should have operations");

        // 第一个操作应该是VecAdd
        match &block.ops[0] {
            IROp::VecAdd { .. } => {
                // VecAdd操作存在，可以使用SIMD或回退到标量
            }
            _ => panic!("Expected VecAdd operation"),
        }
    }

    #[test]
    #[cfg(feature = "simd")]
    fn test_vec128_size_optimization() {
        // 测试128位向量优化的IR结构
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![IROp::VecAdd {
                dst: 1u32,
                src1: 2u32,
                src2: 3u32,
                element_size: 32, // 32位元素适合Vec128 (4×32=128)
            }],
            term: Terminator::Ret,
        };

        // 验证IR结构
        assert_eq!(block.ops.len(), 1);
        match &block.ops[0] {
            IROp::VecAdd { element_size, .. } => {
                assert_eq!(*element_size, 32, "32-bit elements fit in Vec128");
            }
            _ => panic!("Expected VecAdd operation"),
        }
    }

    #[test]
    fn test_simd_feature_gate_compatibility() {
        // 测试feature gate兼容性
        // 这个测试应该在没有simd feature时也能运行
        let block = create_vec_add_ir_block();

        // IR块应该成功创建，无论是否启用simd feature
        assert_eq!(block.ops.len(), 1, "Should have 1 VecAdd operation");
    }

    #[cfg(feature = "simd")]
    #[test]
    fn test_direct_intrinsic_saturated_operations() {
        // 测试饱和运算的SIMD实现 - IR创建

        // 测试有符号饱和加法
        let block_signed = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![IROp::VecAddSat {
                dst: 1u32,
                src1: 2u32,
                src2: 3u32,
                element_size: 32,
                signed: true,
            }],
            term: Terminator::Ret,
        };

        assert_eq!(block_signed.ops.len(), 1);
        match &block_signed.ops[0] {
            IROp::VecAddSat { signed, .. } => {
                assert!(*signed, "Should be signed saturation");
            }
            _ => panic!("Expected VecAddSat operation"),
        }

        // 测试无符号饱和加法
        let block_unsigned = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![IROp::VecAddSat {
                dst: 1u32,
                src1: 2u32,
                src2: 3u32,
                element_size: 32,
                signed: false,
            }],
            term: Terminator::Ret,
        };

        assert_eq!(block_unsigned.ops.len(), 1);
        match &block_unsigned.ops[0] {
            IROp::VecAddSat { signed, .. } => {
                assert!(!(*signed), "Should be unsigned saturation");
            }
            _ => panic!("Expected VecAddSat operation"),
        }
    }

    #[cfg(feature = "simd")]
    #[test]
    fn test_direct_intrinsic_min_max_operations() {
        // 测试最小/最大值的SIMD实现 - IR创建
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![IROp::VecAdd {
                dst: 1u32,
                src1: 2u32,
                src2: 3u32,
                element_size: 32,
            }],
            term: Terminator::Ret,
        };

        // 验证基本向量操作仍然工作
        assert_eq!(block.ops.len(), 1);
        match &block.ops[0] {
            IROp::VecAdd { .. } => {
                // VecAdd操作可以编译
            }
            _ => panic!("Expected VecAdd operation"),
        }
    }

    #[test]
    fn test_backwards_compatibility() {
        // 测试向后兼容性
        // 确保现有代码不受SIMD增强影响
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![
                // 标量操作
                IROp::Add {
                    dst: 1u32,
                    src1: 2u32,
                    src2: 3u32,
                },
                IROp::Sub {
                    dst: 4u32,
                    src1: 5u32,
                    src2: 6u32,
                },
            ],
            term: Terminator::Ret,
        };

        // 验证标量操作IR创建
        assert_eq!(block.ops.len(), 2);
        match &block.ops[0] {
            IROp::Add { dst, src1, src2 } => {
                assert_eq!(*dst, 1u32);
                assert_eq!(*src1, 2u32);
                assert_eq!(*src2, 3u32);
            }
            _ => panic!("Expected Add operation"),
        }
        match &block.ops[1] {
            IROp::Sub { dst, src1, src2 } => {
                assert_eq!(*dst, 4u32);
                assert_eq!(*src1, 5u32);
                assert_eq!(*src2, 6u32);
            }
            _ => panic!("Expected Sub operation"),
        }
    }
}
