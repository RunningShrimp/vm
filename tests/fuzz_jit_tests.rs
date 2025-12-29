//! JIT编译模糊测试
//!
//! 针对JIT编译器的模糊测试，测试各种IR块和编译场景

use vm_engine::jit::Jit;
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator};
use vm_mem::SoftMmu;
use rand::Rng;

/// 生成随机IR块
fn generate_random_ir_block(rng: &mut impl Rng, pc: u64, max_ops: usize) -> IRBlock {
    let mut builder = IRBuilder::new(pc);
    let num_ops = rng.gen_range(1..=max_ops);

    for _ in 0..num_ops {
        match rng.gen_range(0..15) {
            0 => builder.push(IROp::Add {
                dst: rng.gen_range(0..32) as u8,
                src1: rng.gen_range(0..32) as u8,
                src2: rng.gen_range(0..32) as u8,
            }),
            1 => builder.push(IROp::Sub {
                dst: rng.gen_range(0..32) as u8,
                src1: rng.gen_range(0..32) as u8,
                src2: rng.gen_range(0..32) as u8,
            }),
            2 => builder.push(IROp::Mul {
                dst: rng.gen_range(0..32) as u8,
                src1: rng.gen_range(0..32) as u8,
                src2: rng.gen_range(0..32) as u8,
            }),
            3 => builder.push(IROp::Div {
                dst: rng.gen_range(0..32) as u8,
                src1: rng.gen_range(0..32) as u8,
                src2: rng.gen_range(0..32) as u8,
            }),
            4 => builder.push(IROp::MovImm {
                dst: rng.gen_range(0..32) as u8,
                imm: rng.gen::<i64>(),
            }),
            5 => builder.push(IROp::Load {
                dst: rng.gen_range(0..32) as u8,
                base: rng.gen_range(0..32) as u8,
                size: [1, 2, 4, 8][rng.gen_range(0..4)],
                offset: rng.gen_range(-10000..10000),
                flags: Default::default(),
            }),
            6 => builder.push(IROp::Store {
                src: rng.gen_range(0..32) as u8,
                base: rng.gen_range(0..32) as u8,
                size: [1, 2, 4, 8][rng.gen_range(0..4)],
                offset: rng.gen_range(-10000..10000),
                flags: Default::default(),
            }),
            7 => builder.push(IROp::Xor {
                dst: rng.gen_range(0..32) as u8,
                src1: rng.gen_range(0..32) as u8,
                src2: rng.gen_range(0..32) as u8,
            }),
            8 => builder.push(IROp::And {
                dst: rng.gen_range(0..32) as u8,
                src1: rng.gen_range(0..32) as u8,
                src2: rng.gen_range(0..32) as u8,
            }),
            9 => builder.push(IROp::Or {
                dst: rng.gen_range(0..32) as u8,
                src1: rng.gen_range(0..32) as u8,
                src2: rng.gen_range(0..32) as u8,
            }),
            10 => builder.push(IROp::Shl {
                dst: rng.gen_range(0..32) as u8,
                src: rng.gen_range(0..32) as u8,
                shift: rng.gen_range(0..64) as u8,
            }),
            11 => builder.push(IROp::Shr {
                dst: rng.gen_range(0..32) as u8,
                src: rng.gen_range(0..32) as u8,
                shift: rng.gen_range(0..64) as u8,
            }),
            12 => builder.push(IROp::Cmp {
                dst: rng.gen_range(0..32) as u8,
                src1: rng.gen_range(0..32) as u8,
                src2: rng.gen_range(0..32) as u8,
            }),
            13 => builder.push(IROp::Fadd {
                dst: rng.gen_range(0..32) as u8,
                src1: rng.gen_range(0..32) as u8,
                src2: rng.gen_range(0..32) as u8,
            }),
            _ => builder.push(IROp::Nop),
        }
    }

    // 随机终止符
    let term = match rng.gen_range(0..5) {
        0 => Terminator::Jmp {
            target: rng.gen_range(0x1000..0x100000),
        },
        1 => Terminator::CondJmp {
            cond: rng.gen_range(0..32) as u8,
            target_true: rng.gen_range(0x1000..0x100000),
            target_false: rng.gen_range(0x1000..0x100000),
        },
        2 => Terminator::JmpReg {
            base: rng.gen_range(0..32) as u8,
            offset: rng.gen_range(-10000..10000),
        },
        3 => Terminator::Ret,
        _ => Terminator::Fault {
            cause: rng.gen::<u64>(),
        },
    };

    builder.set_term(term);
    builder.build()
}

/// 模糊测试：JIT编译随机IR块
#[test]
fn fuzz_jit_compile_random_blocks() {
    let mut rng = rand::thread_rng();
    let mut jit = Jit::new();
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    for _ in 0..1000 {
        let pc = rng.gen_range(0x1000..0x100000);
        let block = generate_random_ir_block(&mut rng, pc, 50);

        // 执行多次以触发JIT编译
        jit.set_pc(block.start_pc);
        for _ in 0..150 {
            // 可能失败，但不应该panic
            let _ = jit.run(&mut mmu, &block);
        }
    }
}

/// 模糊测试：JIT热点检测
#[test]
fn fuzz_jit_hotspot_detection() {
    let mut rng = rand::thread_rng();
    let mut jit = Jit::new();
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    // 创建多个不同的IR块
    let mut blocks = Vec::new();
    for i in 0..100 {
        let pc = 0x1000 + (i * 0x1000);
        blocks.push(generate_random_ir_block(&mut rng, pc, 20));
    }

    // 随机执行不同的块，测试热点检测
    for _ in 0..10000 {
        let block_idx = rng.gen_range(0..blocks.len());
        let block = &blocks[block_idx];
        jit.set_pc(block.start_pc);
        let _ = jit.run(&mut mmu, block);
    }
}

/// 模糊测试：JIT编译边界条件
#[test]
fn fuzz_jit_edge_cases() {
    let mut jit = Jit::new();
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    // 测试空块
    let empty_block = IRBlock {
        start_pc: 0x1000,
        ops: Vec::new(),
        term: Terminator::Ret,
    };
    jit.set_pc(empty_block.start_pc);
    for _ in 0..150 {
        let _ = jit.run(&mut mmu, &empty_block);
    }

    // 测试单操作块
    let single_op_block = IRBlock {
        start_pc: 0x2000,
        ops: vec![IROp::Nop],
        term: Terminator::Ret,
    };
    jit.set_pc(single_op_block.start_pc);
    for _ in 0..150 {
        let _ = jit.run(&mut mmu, &single_op_block);
    }

    // 测试大块（1000个操作）
    let mut builder = IRBuilder::new(0x3000);
    for _ in 0..1000 {
        builder.push(IROp::Nop);
    }
    builder.set_term(Terminator::Ret);
    let large_block = builder.build();
    jit.set_pc(large_block.start_pc);
    for _ in 0..150 {
        let _ = jit.run(&mut mmu, &large_block);
    }
}

/// 模糊测试：JIT寄存器分配
#[test]
fn fuzz_jit_register_allocation() {
    let mut rng = rand::thread_rng();
    let mut jit = Jit::new();
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    // 创建大量使用寄存器的IR块
    for _ in 0..100 {
        let mut builder = IRBuilder::new(0x1000);
        
        // 使用所有32个寄存器
        for i in 0..32 {
            builder.push(IROp::MovImm {
                dst: i as u8,
                imm: rng.gen::<i64>(),
            });
        }
        
        // 执行大量寄存器操作
        for _ in 0..100 {
            let dst = rng.gen_range(0..32) as u8;
            let src1 = rng.gen_range(0..32) as u8;
            let src2 = rng.gen_range(0..32) as u8;
            builder.push(IROp::Add { dst, src1, src2 });
        }
        
        builder.set_term(Terminator::Ret);
        let block = builder.build();
        
        jit.set_pc(block.start_pc);
        for _ in 0..150 {
            let _ = jit.run(&mut mmu, &block);
        }
    }
}

/// 模糊测试：JIT内存操作
#[test]
fn fuzz_jit_memory_operations() {
    let mut rng = rand::thread_rng();
    let mut jit = Jit::new();
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    for _ in 0..500 {
        let mut builder = IRBuilder::new(0x1000);
        
        // 生成大量内存操作
        for _ in 0..50 {
            match rng.gen_range(0..2) {
                0 => {
                    builder.push(IROp::Load {
                        dst: rng.gen_range(0..32) as u8,
                        base: rng.gen_range(0..32) as u8,
                        size: [1, 2, 4, 8][rng.gen_range(0..4)],
                        offset: rng.gen_range(-10000..10000),
                        flags: Default::default(),
                    });
                }
                _ => {
                    builder.push(IROp::Store {
                        src: rng.gen_range(0..32) as u8,
                        base: rng.gen_range(0..32) as u8,
                        size: [1, 2, 4, 8][rng.gen_range(0..4)],
                        offset: rng.gen_range(-10000..10000),
                        flags: Default::default(),
                    });
                }
            }
        }
        
        builder.set_term(Terminator::Ret);
        let block = builder.build();
        
        jit.set_pc(block.start_pc);
        for _ in 0..150 {
            let _ = jit.run(&mut mmu, &block);
        }
    }
}

/// 模糊测试：JIT控制流
#[test]
fn fuzz_jit_control_flow() {
    let mut rng = rand::thread_rng();
    let mut jit = Jit::new();
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    for _ in 0..500 {
        let pc = rng.gen_range(0x1000..0x100000);
        let mut builder = IRBuilder::new(pc);
        
        // 添加一些操作
        for _ in 0..20 {
            builder.push(IROp::Add {
                dst: rng.gen_range(0..32) as u8,
                src1: rng.gen_range(0..32) as u8,
                src2: rng.gen_range(0..32) as u8,
            });
        }
        
        // 测试各种控制流终止符
        let term = match rng.gen_range(0..4) {
            0 => Terminator::Jmp {
                target: rng.gen_range(0x1000..0x100000),
            },
            1 => Terminator::CondJmp {
                cond: rng.gen_range(0..32) as u8,
                target_true: rng.gen_range(0x1000..0x100000),
                target_false: rng.gen_range(0x1000..0x100000),
            },
            2 => Terminator::JmpReg {
                base: rng.gen_range(0..32) as u8,
                offset: rng.gen_range(-10000..10000),
            },
            _ => Terminator::Ret,
        };
        
        builder.set_term(term);
        let block = builder.build();
        
        jit.set_pc(block.start_pc);
        for _ in 0..150 {
            let _ = jit.run(&mut mmu, &block);
        }
    }
}


