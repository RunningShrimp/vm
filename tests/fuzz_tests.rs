//! 模糊测试套件
//!
//! 使用模糊测试发现边界条件和潜在bug

use vm_core::{MMU, GuestAddr, AccessType, Fault};
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator};
use vm_mem::SoftMmu;
use vm_engine::interpreter::Interpreter;
use vm_tests::test_utils::{MockMMU, IRBlockBuilder};

/// 模糊测试：随机IR块执行
#[test]
fn fuzz_random_ir_blocks() {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut mmu = MockMMU::at_zero();
    let mut engine = Interpreter::new();

    for _ in 0..1000 {
        // 生成随机IR块
        let pc = rng.gen_range(0x1000..0x100000);
        let mut builder = IRBuilder::new(pc);

        // 随机添加操作
        let num_ops = rng.gen_range(1..20);
        for _ in 0..num_ops {
            match rng.gen_range(0..5) {
                0 => builder.push(IROp::Add {
                    dst: rng.gen_range(0..32) as u8,
                    src1: rng.gen_range(0..32) as u8,
                    src2: rng.gen_range(0..32) as u8,
                }),
                1 => builder.push(IROp::MovImm {
                    dst: rng.gen_range(0..32) as u8,
                    imm: rng.gen::<i64>(),
                }),
                2 => builder.push(IROp::Load {
                    dst: rng.gen_range(0..32) as u8,
                    base: rng.gen_range(0..32) as u8,
                    size: [1, 2, 4, 8][rng.gen_range(0..4)],
                    offset: rng.gen_range(-1000..1000),
                    flags: Default::default(),
                }),
                3 => builder.push(IROp::Store {
                    src: rng.gen_range(0..32) as u8,
                    base: rng.gen_range(0..32) as u8,
                    size: [1, 2, 4, 8][rng.gen_range(0..4)],
                    offset: rng.gen_range(-1000..1000),
                    flags: Default::default(),
                }),
                _ => builder.push(IROp::Nop),
            }
        }

        // 设置终止符
        let target = rng.gen_range(0x1000..0x100000);
        builder.set_term(Terminator::Jmp { target });

        let block = builder.build();

        // 执行（可能失败，但不应该panic）
        let _result = engine.run(&mut mmu, &block);
    }
}

/// 模糊测试：随机内存访问
#[test]
fn fuzz_random_memory_access() {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut mmu = SoftMmu::new(0x1000000, false);

    for _ in 0..10000 {
        let addr = rng.gen_range(0..0x1000000);
        let size = [1, 2, 4, 8][rng.gen_range(0..4)];

        match rng.gen_range(0..2) {
            0 => {
                // 读取
                let _ = mmu.read(addr, size);
            }
            _ => {
                // 写入
                let value = rng.gen::<u64>();
                let _ = mmu.write(addr, value, size);
            }
        }
    }
}

/// 模糊测试：随机地址翻译
#[test]
fn fuzz_address_translation() {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut mmu = SoftMmu::new(0x1000000, true);

    for _ in 0..10000 {
        let va = rng.gen_range(0..0x1000000);
        let access = match rng.gen_range(0..3) {
            0 => AccessType::Read,
            1 => AccessType::Write,
            _ => AccessType::Exec,
        };

        // 翻译（可能失败，但不应该panic）
        let _ = mmu.translate(va, access);
    }
}

/// 模糊测试：边界条件
#[test]
fn fuzz_edge_cases() {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut mmu = MockMMU::at_zero();
    let mut engine = Interpreter::new();

    // 测试边界值
    let edge_values = [
        0u64,
        1,
        u8::MAX as u64,
        u16::MAX as u64,
        u32::MAX as u64,
        u64::MAX,
        i8::MAX as u64,
        i8::MIN as u64,
        i16::MAX as u64,
        i16::MIN as u64,
        i32::MAX as u64,
        i32::MIN as u64,
        i64::MAX as u64,
        i64::MIN as u64,
    ];

    for &value in &edge_values {
        // 设置寄存器
        engine.set_reg(1, value);

        // 创建简单IR块
        let mut builder = IRBuilder::new(0x1000);
        builder.push(IROp::Add {
            dst: 0,
            src1: 1,
            src2: 0,
        });
        builder.set_term(Terminator::Jmp { target: 0x1010 });

        let block = builder.build();
        let _result = engine.run(&mut mmu, &block);
    }
}

/// 模糊测试：并发执行
#[test]
fn fuzz_concurrent_execution() {
    use std::sync::Arc;
    use std::thread;
    use rand::Rng;

    let mut rng = rand::thread_rng();
    let mmu = Arc::new(std::sync::Mutex::new(MockMMU::at_zero()));

    let mut handles = Vec::new();

    for _ in 0..10 {
        let mmu_clone = Arc::clone(&mmu);
        let handle = thread::spawn(move || {
            let mut engine = Interpreter::new();
            let mut mmu = mmu_clone.lock().unwrap();

            for _ in 0..100 {
                let pc = rng.gen_range(0x1000..0x10000);
                let block = IRBlockBuilder::create_simple_arithmetic(pc);
                let _result = engine.run(&mut *mmu, &block);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

/// 模糊测试：大块执行
#[test]
fn fuzz_large_blocks() {
    let mut mmu = MockMMU::at_zero();
    let mut engine = Interpreter::new();

    // 创建大IR块
    let block = IRBlockBuilder::create_complex(0x1000, 1000);

    // 执行（可能慢，但不应该panic）
    let _result = engine.run(&mut mmu, &block);
}

/// 模糊测试：IR生成边界条件
#[test]
fn fuzz_ir_generation_edge_cases() {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut builder = IRBuilder::new(0x1000);

    // 测试边界值
    let edge_values = [
        0i64,
        1,
        -1,
        i8::MAX as i64,
        i8::MIN as i64,
        i16::MAX as i64,
        i16::MIN as i64,
        i32::MAX as i64,
        i32::MIN as i64,
        i64::MAX,
        i64::MIN,
    ];

    for &value in &edge_values {
        builder.push(IROp::MovImm {
            dst: 1,
            imm: value,
        });
    }

    let block = builder.build();
    assert!(!block.ops.is_empty());
}

/// 模糊测试：IR操作组合
#[test]
fn fuzz_ir_operation_combinations() {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut mmu = MockMMU::at_zero();
    let mut engine = Interpreter::new();

    for _ in 0..1000 {
        let mut builder = IRBuilder::new(0x1000);
        
        // 生成复杂的操作序列
        let num_ops = rng.gen_range(1..100);
        for i in 0..num_ops {
            // 根据位置选择不同的操作类型
            match i % 10 {
                0..=3 => {
                    // 算术操作
                    builder.push(IROp::Add {
                        dst: (i % 32) as u8,
                        src1: ((i + 1) % 32) as u8,
                        src2: ((i + 2) % 32) as u8,
                    });
                }
                4..=5 => {
                    // 内存操作
                    builder.push(IROp::Load {
                        dst: (i % 32) as u8,
                        base: ((i + 1) % 32) as u8,
                        size: [1, 2, 4, 8][(i % 4) as usize],
                        offset: (i as i32 * 4) - 1000,
                        flags: Default::default(),
                    });
                }
                6..=7 => {
                    // 立即数操作
                    builder.push(IROp::MovImm {
                        dst: (i % 32) as u8,
                        imm: i as i64,
                    });
                }
                _ => {
                    // 位操作
                    builder.push(IROp::Xor {
                        dst: (i % 32) as u8,
                        src1: ((i + 1) % 32) as u8,
                        src2: ((i + 2) % 32) as u8,
                    });
                }
            }
        }

        builder.set_term(Terminator::Ret);
        let block = builder.build();
        let _result = engine.run(&mut mmu, &block);
    }
}

/// 模糊测试：IR终止符组合
#[test]
fn fuzz_ir_terminator_combinations() {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut mmu = MockMMU::at_zero();
    let mut engine = Interpreter::new();

    for _ in 0..500 {
        let mut builder = IRBuilder::new(0x1000);
        
        // 添加一些操作
        for i in 0..10 {
            builder.push(IROp::Add {
                dst: (i % 32) as u8,
                src1: ((i + 1) % 32) as u8,
                src2: ((i + 2) % 32) as u8,
            });
        }

        // 测试各种终止符
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
        let block = builder.build();
        let _result = engine.run(&mut mmu, &block);
    }
}

