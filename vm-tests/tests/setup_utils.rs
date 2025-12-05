//! 测试设置和工具函数

use vm_ir::{IRBuilder, IROp, MemFlags};

/// 创建一个简单的IR块用于测试
pub fn create_simple_ir_block(pc: vm_core::GuestAddr) -> vm_ir::IRBlock {
    let mut builder = IRBuilder::new(pc);

    // 添加一些简单的算术运算
    builder.push(IROp::Add {
        dst: 0,
        src1: 1,
        src2: 2,
    });
    builder.push(IROp::Mul {
        dst: 0,
        src1: 0,
        src2: 3,
    });
    builder.push(IROp::Sub {
        dst: 0,
        src1: 0,
        src2: 1,
    });

    // 设置一些初始寄存器值
    builder.push(IROp::MovImm { dst: 1, value: 42 });
    builder.push(IROp::MovImm { dst: 2, value: 24 });
    builder.push(IROp::MovImm { dst: 3, value: 10 });

    builder.build()
}

/// 创建一个复杂的IR块用于性能测试
pub fn create_complex_ir_block(pc: vm_core::GuestAddr, num_ops: usize) -> vm_ir::IRBlock {
    let mut builder = IRBuilder::new(pc);

    for i in 0..num_ops {
        // 交替进行不同类型的操作来模拟真实工作负载
        match i % 4 {
            0 => {
                builder.push(IROp::Add {
                    dst: 0,
                    src1: 1,
                    src2: 2,
                });
                builder.push(IROp::MovImm {
                    dst: i as u8,
                    value: (i * 10) as u64,
                });
            }
            1 => {
                builder.push(IROp::Sub {
                    dst: 0,
                    src1: 0,
                    src2: 3,
                });
                builder.push(IROp::MovImm {
                    dst: (i + 1) as u8,
                    value: (i * 15) as u64,
                });
            }
            2 => {
                builder.push(IROp::Mul {
                    dst: 0,
                    src1: 0,
                    src2: 2,
                });
                builder.push(IROp::MovImm {
                    dst: (i + 2) as u8,
                    value: (i * 20) as u64,
                });
            }
            3 => {
                builder.push(IROp::Xor {
                    dst: 0,
                    src1: 1,
                    src2: 3,
                });
                builder.push(IROp::MovImm {
                    dst: (i + 3) as u8,
                    value: (i * 25) as u64,
                });
            }
        }
    }

    builder.build()
}

/// 创建带有内存访问的IR块
pub fn create_memory_ir_block(
    pc: vm_core::GuestAddr,
    base_addr: vm_core::GuestAddr,
) -> vm_ir::IRBlock {
    let mut builder = IRBuilder::new(pc);

    // 内存操作序列
    for i in 0..10 {
        builder.push(IROp::Load {
            dst: 0,
            base: base_addr,
            size: 8,
            offset: (i * 8) as i64,
            flags: MemFlags {
                volatile: false,
                atomic: false,
                align: 8,
                fence_before: false,
                fence_after: false,
                order: vm_ir::MemOrder::Relaxed,
            },
        });
        builder.push(IROp::Add {
            dst: 0,
            src1: 0,
            src2: 1,
        });
        builder.push(IROp::Store {
            base: base_addr,
            src: 0,
            size: 8,
            offset: (i * 8) as i64,
            flags: MemFlags {
                volatile: false,
                atomic: false,
                align: 8,
                fence_before: false,
                fence_after: false,
                order: vm_ir::MemOrder::Relaxed,
            },
        });
    }

    builder.build()
}
