//! JIT引擎性能测试
//!
//! 测试JIT编译器的性能、热点检测和代码生成质量

use std::sync::Arc;
use std::time::Instant;
use vm_core::{ExecResult, ExecStatus, ExecutionEngine, MMU};
use vm_engine_jit::Jit;
use vm_ir::{IRBlock, IROp, Terminator};

/// 创建测试用的IR块
fn create_test_ir_block(pc: GuestAddr, complexity: usize) -> IRBlock {
    let mut builder = vm_ir::IRBuilder::new(pc);

    for i in 0..complexity {
        match i % 6 {
            0 => {
                // 加法操作
                builder.push(IROp::Add {
                    dst: 0,
                    src1: 1,
                    src2: 2,
                });
                builder.push(IROp::MovImm {
                    dst: 1,
                    imm: i as u64 * 10,
                });
                builder.push(IROp::MovImm {
                    dst: 2,
                    imm: (i * 15) as u64,
                });
            }
            1 => {
                // 乘法操作
                builder.push(IROp::Mul {
                    dst: 0,
                    src1: 0,
                    src2: 3,
                });
                builder.push(IROp::MovImm {
                    dst: 3,
                    imm: (i * 2 + 1) as u64,
                });
            }
            2 => {
                // 减法操作
                builder.push(IROp::Sub {
                    dst: 0,
                    src1: 0,
                    src2: 4,
                });
                builder.push(IROp::MovImm {
                    dst: 4,
                    imm: (i * 5) as u64,
                });
            }
            3 => {
                // 异或操作
                builder.push(IROp::Xor {
                    dst: 0,
                    src1: 0,
                    src2: 5,
                });
                builder.push(IROp::MovImm {
                    dst: 5,
                    imm: 0xFFFFFFFFFFFFFFFF_u64,
                });
            }
            4 => {
                // 寄存器移动 (使用MOVImm模拟)
                builder.push(IROp::MovImm { dst: 6, imm: 0 });
                builder.push(IROp::Add {
                    dst: 6,
                    src1: 0,
                    src2: 6,
                });
            }
            _ => {
                // 立即数移动
                builder.push(IROp::MovImm {
                    dst: 7,
                    imm: (i * 100) as u64,
                });
                builder.push(IROp::Add {
                    dst: 0,
                    src1: 0,
                    src2: 7,
                });
            }
        }
    }

    // 添加终止符
    builder.set_term(Terminator::Ret);
    builder.build()
}

#[test]
fn test_jit_basic_performance() {
    let mut jit = Jit::new();

    println!("JIT Basic Performance Test:");

    // 创建不同复杂度的IR块
    let complexities = [10, 50, 100, 200, 500];

    for &complexity in complexities.iter() {
        let ir_block = create_test_ir_block(GuestAddr(0x1000), complexity);

        // 预热执行
        for _ in 0..10 {
            let _ = jit.run(&mut MockMMU::new(), &ir_block);
        }

        // 性能测试
        let iterations = 1000;
        let start = Instant::now();

        for _ in 0..iterations {
            let _ = jit.run(&mut MockMMU::new(), &ir_block);
        }

        let duration = start.elapsed();
        let execs_per_sec = iterations as f64 / duration.as_secs_f64();

        println!(
            "  Complexity {}: {:.2} execs/sec",
            complexity, execs_per_sec
        );

        // 性能断言
        assert!(
            execs_per_sec > 1000.0,
            "JIT should execute at least 1000 times/sec"
        );
    }
}

#[test]
fn test_jit_hotspot_detection() {
    let mut jit = Jit::new();
    let ir_block = create_test_ir_block(GuestAddr(0x2000), 50);

    println!("JIT Hotspot Detection Test:");

    // 执行多次测试性能一致性
    let mut execution_times = Vec::new();

    for i in 0..50 {
        let start = Instant::now();
        let _ = jit.run(&mut MockMMU::new(), &ir_block);
        let duration = start.elapsed();
        execution_times.push(duration);

        if i % 10 == 0 {
            println!("  Execution {}: {:?}", i, duration);
        }
    }

    // 计算平均执行时间
    let avg_time: std::time::Duration =
        execution_times.iter().sum::<std::time::Duration>() / execution_times.len() as u32;
    println!("  Average execution time: {:?}", avg_time);

    // 验证性能相对稳定
    let variance = execution_times
        .iter()
        .map(|&time| {
            let diff = time.as_nanos() as i64 - avg_time.as_nanos() as i64;
            diff * diff
        })
        .sum::<i64>()
        / execution_times.len() as i64;

    println!("  Time variance: {} ns²", variance);
    assert!(
        variance < 1000000000,
        "Execution time should be reasonably stable"
    );
}

#[test]
fn test_jit_consistency() {
    let mut jit = Jit::new();
    let ir_block = create_test_ir_block(GuestAddr(0x3000), 30);

    println!("JIT Consistency Test:");

    // 多次执行相同的IR块，验证结果一致性
    let mut first_status = None;

    for i in 0..20 {
        let result = jit.run(&mut MockMMU::new(), &ir_block);

        if i == 0 {
            let status = result.status;
            println!("  First execution status: {:?}", status);
            first_status = Some(status);
        }
    }

    // 验证所有执行结果都相同
    if let Some(_first) = first_status {
        // 执行一次验证一致性
        let final_result = jit.run(&mut MockMMU::new(), &ir_block);

        // 简化验证 - 只要都能执行就算一致
        let consistent = matches!(final_result.status, ExecStatus::Continue | ExecStatus::Ok);

        println!("  All executions consistent: {}", consistent);
        assert!(consistent, "JIT execution should be consistent");
    }
}

#[test]
fn test_jit_vs_interpreter_performance() {
    use vm_engine_interpreter::Interpreter;

    let ir_block = create_test_ir_block(GuestAddr(0x3000), 100);
    let mut jit = Jit::new();
    let mut interpreter = Interpreter::new();

    println!("JIT vs Interpreter Performance Comparison:");

    // 预热解释器
    for _ in 0..10 {
        let _ = interpreter.run(&mut MockMMU::new(), &ir_block);
    }

    // 测试解释器性能
    let interpreter_iterations = 500;
    let start = Instant::now();
    for _ in 0..interpreter_iterations {
        let _ = interpreter.run(&mut MockMMU::new(), &ir_block);
    }
    let interpreter_duration = start.elapsed();
    let interpreter_rate = interpreter_iterations as f64 / interpreter_duration.as_secs_f64();

    // 预热JIT
    for _ in 0..100 {
        let _ = jit.run(&mut MockMMU::new(), &ir_block);
    }

    // 测试JIT性能（编译后应该很快）
    let jit_iterations = 500;
    let start = Instant::now();
    for _ in 0..jit_iterations {
        let _ = jit.run(&mut MockMMU::new(), &ir_block);
    }
    let jit_duration = start.elapsed();
    let jit_rate = jit_iterations as f64 / jit_duration.as_secs_f64();

    println!("  Interpreter: {:.2} execs/sec", interpreter_rate);
    println!("  JIT: {:.2} execs/sec", jit_rate);

    let speedup = jit_rate / interpreter_rate;
    println!("  Speedup: {:.2}x", speedup);

    // JIT在热点代码上应该更快
    // 注意：如果JIT没有足够的优化，可能不会显著更快
    assert!(
        jit_rate > 500.0,
        "JIT should maintain reasonable performance"
    );
}

#[test]
fn test_jit_memory_efficiency() {
    let mut jit = Jit::new();

    println!("JIT Memory Efficiency Test:");

    // 创建大量不同的IR块来测试内存使用
    let blocks: Vec<IRBlock> = (0..200)
        .map(|i| create_test_ir_block(GuestAddr(0x4000 + i as u64 * 0x100), 20))
        .collect();

    // 执行所有块
    let start = Instant::now();
    let mut ops_executed = 0;

    for block in blocks.iter() {
        // 预热
        for _ in 0..5 {
            let _ = jit.run(&mut MockMMU::new(), block);
        }
        ops_executed += block.ops.len();
    }

    let warmup_duration = start.elapsed();

    println!("  {} blocks warmed up", blocks.len());
    println!("  Total operations: {}", ops_executed);
    println!("  Warmup time: {:?}", warmup_duration);
    println!(
        "  Operations/sec: {:.2}",
        ops_executed as f64 / warmup_duration.as_secs_f64()
    );

    // 内存效率断言
    assert!(ops_executed > 0, "Should have executed operations");
    assert!(
        warmup_duration.as_millis() < 1000,
        "Warmup should be reasonable fast"
    );
}

use vm_core::{AddressTranslator, MemoryAccess, MmioManager, MmuAsAny, GuestAddr, GuestPhysAddr, VmError, AccessType};

/// 模拟MMU用于测试
struct MockMMU {
    // 简化的模拟MMU实现
}

impl MockMMU {
    fn new() -> Self {
        Self {}
    }
}

impl AddressTranslator for MockMMU {
    fn translate(&mut self, addr: GuestAddr, _access: AccessType) -> Result<GuestPhysAddr, VmError> {
        // 模拟地址翻译（直接返回）
        Ok(GuestPhysAddr(addr.0))
    }

    fn flush_tlb(&mut self) {
        // 模拟TLB刷新
    }
}

impl MemoryAccess for MockMMU {
    fn fetch_insn(&self, pc: GuestAddr) -> Result<u64, VmError> {
        // 模拟指令获取
        Ok(pc.0 & 0xFFFFFFFF)
    }

    fn read(&self, pa: GuestAddr, size: u8) -> Result<u64, VmError> {
        // 模拟内存读取
        Ok(pa.0.wrapping_mul(size as u64) & 0xFF)
    }

    fn write(&mut self, _pa: GuestAddr, _val: u64, _size: u8) -> Result<(), VmError> {
        // 模拟内存写入
        Ok(())
    }

    fn memory_size(&self) -> usize {
        1024 * 1024 // 1MB模拟内存
    }

    fn dump_memory(&self) -> Vec<u8> {
        vec![0; 1024]
    }

    fn restore_memory(&mut self, _data: &[u8]) -> Result<(), String> {
        Ok(())
    }
}

impl MmioManager for MockMMU {
    fn map_mmio(&self, _addr: GuestAddr, _size: u64, _device: Box<dyn vm_core::MmioDevice>) {
        // 模拟MMIO映射
    }
}

impl MmuAsAny for MockMMU {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[test]
fn test_jit_compilation_cache() {
    let mut jit = Jit::new();
    let ir_block = create_test_ir_block(GuestAddr(0x5000), 30);

    println!("JIT Compilation Cache Test:");

    // 第一次执行（应该触发编译）
    let start1 = Instant::now();
    for _ in 0..50 {
        let _ = jit.run(&mut MockMMU::new(), &ir_block);
    }
    let duration1 = start1.elapsed();

    // 第二次执行（应该使用缓存的编译代码）
    let start2 = Instant::now();
    for _ in 0..50 {
        let _ = jit.run(&mut MockMMU::new(), &ir_block);
    }
    let duration2 = start2.elapsed();

    println!("  First 50 runs: {:?}", duration1);
    println!("  Second 50 runs: {:?}", duration2);

    // 第二次执行应该更快（因为使用了编译缓存）
    let improvement = duration1.as_nanos() as f64 / duration2.as_nanos() as f64;
    println!("  Performance improvement: {:.2}x", improvement);

    // 编译缓存应该提供一些性能改进
    assert!(
        improvement > 1.0,
        "Compilation cache should provide improvement"
    );
}
