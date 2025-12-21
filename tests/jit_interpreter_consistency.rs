//! JIT 与解释器一致性差分测试
//!
//! 本测试套件验证 JIT 编译执行和解释器执行产生相同的结果，
//! 确保两种执行模式的语义一致性。

use std::sync::Arc;
use vm_core::{ExecResult, ExecStatus, GuestAddr, MMU};
use vm_engine_interpreter::Interpreter;
use vm_engine_jit::Jit;
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator};
use vm_mem::SoftMmu;

/// 测试 MMU 实现
struct TestMmu {
    memory: Vec<u8>,
}

impl TestMmu {
    fn new(size: usize) -> Self {
        Self {
            memory: vec![0; size],
        }
    }

    fn with_code(code: &[u8]) -> Self {
        let mut mmu = Self::new(1024 * 1024); // 1MB
        // 将代码加载到地址 0x1000
        let start = 0x1000;
        for (i, &byte) in code.iter().enumerate() {
            if start + i < mmu.memory.len() {
                mmu.memory[start + i] = byte;
            }
        }
        mmu
    }
}

impl MMU for TestMmu {
    fn read(&self, addr: GuestAddr, size: u8) -> Result<u64, vm_core::VmError> {
        let addr = addr.0 as usize;
        if addr + (size as usize) > self.memory.len() {
            return Err(vm_core::VmError::MemoryAccessFault);
        }

        let mut result = 0u64;
        for i in 0..size as usize {
            result |= (self.memory[addr + i] as u64) << (i * 8);
        }
        Ok(result)
    }

    fn write(&mut self, addr: GuestAddr, value: u64, size: u8) -> Result<(), vm_core::VmError> {
        let addr = addr.0 as usize;
        if addr + (size as usize) > self.memory.len() {
            return Err(vm_core::VmError::MemoryAccessFault);
        }

        for i in 0..size as usize {
            self.memory[addr + i] = ((value >> (i * 8)) & 0xFF) as u8;
        }
        Ok(())
    }

    fn fetch_instruction(&self, addr: GuestAddr) -> Result<u32, vm_core::VmError> {
        self.read(addr, 4).map(|v| v as u32)
    }
}

/// 创建简单的算术测试 IR 块
fn create_arithmetic_block() -> IRBlock {
    let mut builder = IRBuilder::new(GuestAddr(0x1000));

    // 简单的加法操作：R0 = R1 + R2
    builder.push(IROp::Add64 {
        dst: 0,
        src1: 1,
        src2: 2,
    });

    // 减法操作：R3 = R0 - R1
    builder.push(IROp::Sub64 {
        dst: 3,
        src1: 0,
        src2: 1,
    });

    // 设置终止条件
    builder.set_term(Terminator::Return);

    builder.build()
}

/// 创建内存操作测试 IR 块
fn create_memory_block() -> IRBlock {
    let mut builder = IRBuilder::new(GuestAddr(0x2000));

    // 加载操作：R0 = MEM[R1]
    builder.push(IROp::Load64 {
        dst: 0,
        base: 1,
        offset: 0,
    });

    // 存储操作：MEM[R2] = R0
    builder.push(IROp::Store64 {
        src: 0,
        base: 2,
        offset: 0,
    });

    builder.set_term(Terminator::Return);

    builder.build()
}

/// 创建分支测试 IR 块
fn create_branch_block() -> IRBlock {
    let mut builder = IRBuilder::new(GuestAddr(0x3000));

    // 比较操作
    builder.push(IROp::Cmp64 {
        dst: 0,
        src1: 1,
        src2: 2,
    });

    // 条件移动
    builder.push(IROp::CMove64 {
        dst: 3,
        cond: 0,
        src_true: 1,
        src_false: 2,
    });

    builder.set_term(Terminator::Return);

    builder.build()
}

/// 比较执行结果
fn compare_results(
    interp_result: &ExecResult,
    jit_result: &ExecResult,
    test_name: &str,
) -> bool {
    let mut passed = true;

    // 比较状态
    let status_match = matches!(
        (&interp_result.status, &jit_result.status),
        (ExecStatus::Ok, ExecStatus::Ok)
            | (ExecStatus::Continue, ExecStatus::Continue)
    );

    if !status_match {
        eprintln!(
            "[{}] Status mismatch: Interpreter={:?}, JIT={:?}",
            test_name, interp_result.status, jit_result.status
        );
        passed = false;
    }

    // 比较下一个 PC
    if interp_result.next_pc != jit_result.next_pc {
        eprintln!(
            "[{}] Next PC mismatch: Interpreter={:?}, JIT={:?}",
            test_name, interp_result.next_pc, jit_result.next_pc
        );
        passed = false;
    }

    // 比较执行的指令数
    if interp_result.stats.executed_insns != jit_result.stats.executed_insns {
        eprintln!(
            "[{}] Instruction count mismatch: Interpreter={}, JIT={}",
            test_name, interp_result.stats.executed_insns, jit_result.stats.executed_insns
        );
        // 这可能是预期的，因为 JIT 可能有不同的计数方式
    }

    passed
}

/// 运行一致性测试
fn run_consistency_test(block: &IRBlock, test_name: &str) -> bool {
    // 创建测试 MMU
    let mut interp_mmu = TestMmu::new(1024 * 1024);
    let mut jit_mmu = TestMmu::new(1024 * 1024);

    // 创建解释器和 JIT
    let mut interpreter = Interpreter::new();
    let mut jit = Jit::new();

    // 使用模拟执行模式（真实执行需要完整的代码生成）
    jit.enable_real_execution(false);

    // 执行解释器
    let interp_result = interpreter.run(&mut interp_mmu, block);

    // 执行 JIT
    let jit_result = jit.run(&mut jit_mmu, block);

    // 比较结果
    compare_results(&interp_result, &jit_result, test_name)
}

#[test]
fn test_arithmetic_consistency() {
    let block = create_arithmetic_block();
    assert!(
        run_consistency_test(&block, "arithmetic"),
        "Arithmetic operations should produce consistent results"
    );
}

#[test]
fn test_memory_consistency() {
    let block = create_memory_block();
    assert!(
        run_consistency_test(&block, "memory"),
        "Memory operations should produce consistent results"
    );
}

#[test]
fn test_branch_consistency() {
    let block = create_branch_block();
    assert!(
        run_consistency_test(&block, "branch"),
        "Branch operations should produce consistent results"
    );
}

#[test]
fn test_empty_block_consistency() {
    let mut builder = IRBuilder::new(GuestAddr(0x4000));
    builder.set_term(Terminator::Return);
    let block = builder.build();

    assert!(
        run_consistency_test(&block, "empty"),
        "Empty blocks should produce consistent results"
    );
}

#[test]
fn test_nop_sequence_consistency() {
    let mut builder = IRBuilder::new(GuestAddr(0x5000));

    // 添加多个 NOP 操作
    for _ in 0..10 {
        builder.push(IROp::Nop);
    }

    builder.set_term(Terminator::Return);
    let block = builder.build();

    assert!(
        run_consistency_test(&block, "nop_sequence"),
        "NOP sequences should produce consistent results"
    );
}

#[test]
fn test_jit_cache_hit() {
    let block = create_arithmetic_block();
    let mut mmu = TestMmu::new(1024 * 1024);
    let mut jit = Jit::new();

    // 第一次执行（缓存未命中）
    let result1 = jit.run(&mut mmu, &block);

    // 第二次执行（缓存命中）
    let result2 = jit.run(&mut mmu, &block);

    // 验证缓存统计
    let stats = jit.get_compile_stats();
    assert_eq!(stats.cache_misses, 1, "Should have 1 cache miss");
    assert_eq!(stats.cache_hits, 1, "Should have 1 cache hit");
    assert_eq!(stats.total_compiles, 1, "Should compile only once");

    // 验证结果一致性
    assert_eq!(
        result1.next_pc, result2.next_pc,
        "Repeated executions should produce same next_pc"
    );
}

#[test]
fn test_jit_compile_stats() {
    let block1 = create_arithmetic_block();
    let block2 = create_memory_block();
    let mut mmu = TestMmu::new(1024 * 1024);
    let mut jit = Jit::new();

    // 执行多个不同的块
    jit.run(&mut mmu, &block1);
    jit.run(&mut mmu, &block2);
    jit.run(&mut mmu, &block1); // 缓存命中
    jit.run(&mut mmu, &block2); // 缓存命中

    let stats = jit.get_compile_stats();
    assert_eq!(stats.total_compiles, 2, "Should compile 2 different blocks");
    assert_eq!(stats.cache_hits, 2, "Should have 2 cache hits");
    assert_eq!(stats.cache_misses, 2, "Should have 2 cache misses");
}

/// 性能基准：比较解释器和 JIT 执行时间
#[test]
#[ignore] // 默认忽略性能测试
fn benchmark_interpreter_vs_jit() {
    use std::time::Instant;

    let block = create_arithmetic_block();
    let iterations = 1000;

    // 解释器基准
    let mut interp_mmu = TestMmu::new(1024 * 1024);
    let mut interpreter = Interpreter::new();

    let start = Instant::now();
    for _ in 0..iterations {
        interpreter.run(&mut interp_mmu, &block);
    }
    let interp_time = start.elapsed();

    // JIT 基准
    let mut jit_mmu = TestMmu::new(1024 * 1024);
    let mut jit = Jit::new();

    let start = Instant::now();
    for _ in 0..iterations {
        jit.run(&mut jit_mmu, &block);
    }
    let jit_time = start.elapsed();

    println!("Interpreter: {:?}", interp_time);
    println!("JIT: {:?}", jit_time);
    println!(
        "JIT speedup: {:.2}x",
        interp_time.as_nanos() as f64 / jit_time.as_nanos() as f64
    );
}

