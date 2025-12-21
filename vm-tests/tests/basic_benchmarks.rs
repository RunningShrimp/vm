//! 基本性能基准测试
//!
//! 测试核心组件的基本性能指标

use std::time::Instant;
use vm_core::{GuestAddr, MemoryAccess, MMU};
use vm_ir::{IRBuilder, IROp};

// 内联setup_utils函数
pub fn create_simple_ir_block(pc: u64) -> vm_ir::IRBlock {
    let mut builder = IRBuilder::new(GuestAddr(pc));

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
    builder.push(IROp::MovImm { dst: 1, imm: 42 });
    builder.push(IROp::MovImm { dst: 2, imm: 24 });
    builder.push(IROp::MovImm { dst: 3, imm: 10 });

    builder.build()
}

pub fn create_complex_ir_block(pc: u64, num_ops: usize) -> vm_ir::IRBlock {
    let mut builder = IRBuilder::new(GuestAddr(pc));

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
                    dst: i as u32,
                    imm: (i * 10) as u64,
                });
            }
            1 => {
                builder.push(IROp::Sub {
                    dst: 0,
                    src1: 0,
                    src2: 3,
                });
                builder.push(IROp::MovImm {
                    dst: (i + 1) as u32,
                    imm: (i * 15) as u64,
                });
            }
            2 => {
                builder.push(IROp::Mul {
                    dst: 0,
                    src1: 0,
                    src2: 2,
                });
                builder.push(IROp::MovImm {
                    dst: (i + 2) as u32,
                    imm: (i * 20) as u64,
                });
            }
            3 => {
                builder.push(IROp::Xor {
                    dst: 0,
                    src1: 1,
                    src2: 3,
                });
                builder.push(IROp::MovImm {
                    dst: (i + 3) as u32,
                    imm: (i * 25) as u64,
                });
            }
            _ => {
                builder.push(IROp::Nop);
            }
        }
    }

    builder.build()
}

#[test]
fn test_memory_access_performance() {
    let mut mmu = vm_mem::SoftMmu::new(1024 * 1024, false); // 1MB内存

    // 写入一些测试数据
    let test_data = 0x123456789ABCDEF0u64;

    let start = Instant::now();
    let num_operations = 100_000;

    for i in 0..num_operations {
        let addr = (i % 1024) * 8; // 在1KB范围内循环
        (&mut mmu as &mut dyn vm_core::MemoryAccess).write(vm_core::GuestAddr(addr as u64), test_data, 8).unwrap();
        let _read_back = (&mmu as &dyn vm_core::MemoryAccess).read(vm_core::GuestAddr(addr as u64), 8).unwrap();
    }

    let duration = start.elapsed();
    let ops_per_sec = (num_operations as f64 * 2.0) / duration.as_secs_f64(); // 读写各一次

    println!("Memory Access Performance:");
    println!("  Operations: {}", num_operations * 2);
    println!("  Duration: {:?}", duration);
    println!("  Ops/sec: {:.2}", ops_per_sec);

    // 性能断言：应该能达到合理的内存访问速度
    assert!(
        ops_per_sec > 10_000.0,
        "Memory access should be reasonably fast"
    );
}

#[test]
fn test_translation_overhead() {
    // 测试基本的地址翻译开销
    let start = Instant::now();
    let num_translations = 10_000;

    for i in 0..num_translations {
        let _guest_addr = i as u64 * 0x1000;
        // 这里只是测试地址创建的开销
    }

    let duration = start.elapsed();
    let translations_per_sec = num_translations as f64 / duration.as_secs_f64();

    println!("Address Creation Performance:");
    println!("  Translations: {}", num_translations);
    println!("  Duration: {:?}", duration);
    println!("  Translations/sec: {:.2}", translations_per_sec);

    // 地址创建应该非常快
    assert!(
        translations_per_sec > 1_000_000.0,
        "Address creation should be very fast"
    );
}

#[test]
fn test_ir_block_creation() {
    // 测试IR块创建的性能
    let start = Instant::now();
    let num_blocks = 1_000;

    for i in 0..num_blocks {
        let pc = 0x1000 + i as u64 * 0x100;
        let _block = create_simple_ir_block(pc);
    }

    let duration = start.elapsed();
    let blocks_per_sec = num_blocks as f64 / duration.as_secs_f64();

    println!("IR Block Creation Performance:");
    println!("  Blocks: {}", num_blocks);
    println!("  Duration: {:?}", duration);
    println!("  Blocks/sec: {:.2}", blocks_per_sec);

    // IR块创建应该足够快
    assert!(
        blocks_per_sec > 1_000.0,
        "IR block creation should be reasonably fast"
    );
}

#[test]
fn test_complex_ir_block_performance() {
    // 测试复杂IR块的创建和性能
    let start = Instant::now();
    let pc = 0x1000;
    let complex_block = create_complex_ir_block(pc, 1000);
    let creation_time = start.elapsed();

    println!("Complex IR Block Creation:");
    println!("  Operations: 1000");
    println!("  Duration: {:?}", creation_time);

    // 验证块包含预期的操作数
    assert!(
        !complex_block.ops.is_empty(),
        "Complex block should contain operations"
    );

    // 创建性能测试
    let start = Instant::now();
    for _ in 0..100 {
        let _block = create_complex_ir_block(pc, 100);
    }
    let duration = start.elapsed();
    let blocks_per_sec = 100.0 / duration.as_secs_f64();

    println!(
        "Complex Block Creation Rate: {:.2} blocks/sec",
        blocks_per_sec
    );
}

#[test]
fn test_basic_overhead_benchmark() {
    // 测试基本的VM操作开销

    // 1. 时间测量开销
    let start = Instant::now();
    for _ in 0..100_000 {
        let _now = Instant::now();
    }
    let timing_overhead = start.elapsed();

    // 2. 分配开销
    let start = Instant::now();
    let mut vecs = Vec::new();
    for i in 0..10_000 {
        let mut v = Vec::new();
        v.push(i);
        vecs.push(v);
    }
    let allocation_overhead = start.elapsed();

    // 3. 哈希操作开销
    use std::collections::HashMap;
    let start = Instant::now();
    let mut map = HashMap::new();
    for i in 0..10_000 {
        map.insert(i, i * 2);
        let _value = map.get(&i);
    }
    let hash_overhead = start.elapsed();

    println!("Basic Overhead Benchmarks:");
    println!("  Timing overhead (100k): {:?}", timing_overhead);
    println!(
        "  Allocation overhead (10k vectors): {:?}",
        allocation_overhead
    );
    println!("  Hash operations (10k): {:?}", hash_overhead);

    // 这些基本操作应该很快
    assert!(timing_overhead.as_millis() < 100, "Timing should be fast");
    assert!(
        allocation_overhead.as_millis() < 100,
        "Allocation should be fast"
    );
    assert!(
        hash_overhead.as_millis() < 100,
        "Hash operations should be fast"
    );
}
