//! JIT编译执行示例
//!
//! 这个示例展示了如何：
//! 1. 使用JIT编译器执行程序
//! 2. 配置JIT优化级别
//! 3. 比较JIT和解释器的性能
//! 4. 利用热点检测和编译缓存

use anyhow::Result;
use std::time::{Duration, Instant};
use vm_core::{GuestArch, VmConfig};
use vm_engine::{ExecutionEngine, ExecutionResult, EngineConfig};
use vm_frontend::riscv64::Riscv64Frontend;
use vm_mem::{MemoryRegion, SoftMmu, MemRegionFlags};

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .init();

    println!("=== JIT编译执行示例 ===\n");

    // 运行多个基准测试
    run_arithmetic_benchmark()?;
    run_loop_benchmark()?;
    run_memory_benchmark()?;

    println!("\n=== 所有测试完成 ===");
    println!("JIT编译的优势:");
    println!("  - 显著提升执行速度(10-100倍)");
    println!("  - 降低CPU开销");
    println!("  - 更好的内存局部性");
    println!("  - 支持激进的优化");

    Ok(())
}

/// 算术运算基准测试
fn run_arithmetic_benchmark() -> Result<()> {
    println!("--- 算术运算基准测试 ---\n");

    // 创建一个简单的算术运算程序
    let program = vec![
        0x93, 0x00, 0xA0, 0x00,  // li  x1, 10
        0x13, 0x01, 0x40, 0x01,  // li  x2, 20
        0xB3, 0x01, 0x21, 0x00,  // add x3, x1, x2
        0x93, 0x00, 0x60, 0x00,  // li  x1, 5
        0xB3, 0x01, 0x21, 0x00,  // sub x3, x3, x1
        0x67, 0x80, 0x00, 0x00,  // ret
    ];

    let iterations = 10000;

    // 测试解释器
    let interp_time = measure_execution(&program, iterations, false)?;
    println!("解释器: {:?} ({} 次迭代)", interp_time, iterations);

    // 测试JIT
    let jit_time = measure_execution(&program, iterations, true)?;
    println!("JIT:     {:?} ({} 次迭代)", jit_time, iterations);

    let speedup = interp_time.as_nanos() as f64 / jit_time.as_nanos() as f64;
    println!("加速比:  {:.2}x\n", speedup);

    Ok(())
}

/// 循环基准测试
fn run_loop_benchmark() -> Result<()> {
    println!("--- 循环基准测试 ---\n");

    // 创建一个包含循环的程序
    let program = vec![
        0x93, 0x00, 0x00, 0x00,  // li  x1, 0    # i = 0
        0x13, 0x01, 0x00, 0x01,  // li  x2, 100  # limit = 100
        0x93, 0x02, 0x00, 0x00,  // li  x3, 0    # sum = 0
        // loop:
        0xB3, 0x02, 0x23, 0x00,  // add x3, x3, x1  # sum += i
        0x93, 0x05, 0x05, 0x00,  // addi x1, x1, 1  # i++
        0x63, 0xEC, 0x25, 0xFE,  // blt x1, x2, loop
        0x67, 0x80, 0x00, 0x00,  // ret
    ];

    let iterations = 1000;

    // 测试解释器
    let interp_time = measure_execution(&program, iterations, false)?;
    println!("解释器: {:?} ({} 次迭代)", interp_time, iterations);

    // 测试JIT
    let jit_time = measure_execution(&program, iterations, true)?;
    println!("JIT:     {:?} ({} 次迭代)", jit_time, iterations);

    let speedup = interp_time.as_nanos() as f64 / jit_time.as_nanos() as f64;
    println!("加速比:  {:.2}x\n", speedup);

    Ok(())
}

/// 内存访问基准测试
fn run_memory_benchmark() -> Result<()> {
    println!("--- 内存访问基准测试 ---\n");

    // 创建一个包含内存访问的程序
    let program = vec![
        0x17, 0x11, 0x10, 0x10,  // lui x1, 0x10000  # 数据基址
        0x93, 0x00, 0x00, 0x00,  // li  x2, 0        # i = 0
        0x13, 0x01, 0x00, 0x01,  // li  x3, 100      # limit
        // loop:
        0x23, 0x34, 0x22, 0x00,  // sd x2, 0(x1)     # store i
        0x03, 0x23, 0x14, 0x00,  // ld x4, 0(x1)     # load back
        0x93, 0x05, 0x05, 0x00,  // addi x2, x2, 1   # i++
        0x63, 0xEC, 0x25, 0xFE,  // blt x2, x3, loop
        0x67, 0x80, 0x00, 0x00,  // ret
    ];

    let iterations = 1000;

    // 测试解释器
    let interp_time = measure_execution_with_memory(&program, iterations, false)?;
    println!("解释器: {:?} ({} 次迭代)", interp_time, iterations);

    // 测试JIT
    let jit_time = measure_execution_with_memory(&program, iterations, true)?;
    println!("JIT:     {:?} ({} 次迭代)", jit_time, iterations);

    let speedup = interp_time.as_nanos() as f64 / jit_time.as_nanos() as f64;
    println!("加速比:  {:.2}x\n", speedup);

    Ok(())
}

/// 测量执行时间
fn measure_execution(program: &[u8], iterations: usize, use_jit: bool) -> Result<Duration> {
    let config = VmConfig {
        arch: GuestArch::Riscv64,
        memory_size: 1024 * 1024,
        ..Default::default()
    };

    let code_base = 0x1000u64;

    let mut total_time = Duration::ZERO;

    for _ in 0..iterations {
        let mut mmu = SoftMmu::new(config.memory_size);

        mmu.add_region(MemoryRegion {
            base: code_base,
            size: 0x1000,
            flags: MemRegionFlags::READ | MemRegionFlags::EXEC,
        })?;

        for (i, &byte) in program.iter().enumerate() {
            mmu.write(code_base + i as u64, byte as u64, 1)?;
        }

        let engine_config = if use_jit {
            EngineConfig {
                enable_jit: true,
                jit_threshold: 1,  // 立即编译
                optimization_level: 2,
                ..Default::default()
            }
        } else {
            EngineConfig {
                enable_jit: false,
                ..Default::default()
            }
        };

        let mut engine = ExecutionEngine::new(
            config.arch,
            Box::new(mmu),
            engine_config
        )?;

        engine.set_pc(code_base)?;

        let start = Instant::now();

        loop {
            match engine.execute_step()? {
                ExecutionResult::Continue => {}
                ExecutionResult::Halted => break,
                ExecutionResult::Exception(_) => break,
            }
        }

        total_time += start.elapsed();
    }

    Ok(total_time)
}

/// 测量执行时间(带内存区域)
fn measure_execution_with_memory(
    program: &[u8],
    iterations: usize,
    use_jit: bool
) -> Result<Duration> {
    let config = VmConfig {
        arch: GuestArch::Riscv64,
        memory_size: 1024 * 1024,
        ..Default::default()
    };

    let code_base = 0x1000u64;
    let data_base = 0x10000u64;

    let mut total_time = Duration::ZERO;

    for _ in 0..iterations {
        let mut mmu = SoftMmu::new(config.memory_size);

        mmu.add_region(MemoryRegion {
            base: code_base,
            size: 0x1000,
            flags: MemRegionFlags::READ | MemRegionFlags::EXEC,
        })?;

        mmu.add_region(MemoryRegion {
            base: data_base,
            size: 0x1000,
            flags: MemRegionFlags::READ | MemRegionFlags::WRITE,
        })?;

        for (i, &byte) in program.iter().enumerate() {
            mmu.write(code_base + i as u64, byte as u64, 1)?;
        }

        let engine_config = if use_jit {
            EngineConfig {
                enable_jit: true,
                jit_threshold: 1,
                optimization_level: 2,
                ..Default::default()
            }
        } else {
            EngineConfig {
                enable_jit: false,
                ..Default::default()
            }
        };

        let mut engine = ExecutionEngine::new(
            config.arch,
            Box::new(mmu),
            engine_config
        )?;

        engine.set_pc(code_base)?;

        let start = Instant::now();

        loop {
            match engine.execute_step()? {
                ExecutionResult::Continue => {}
                ExecutionResult::Halted => break,
                ExecutionResult::Exception(_) => break,
            }
        }

        total_time += start.elapsed();
    }

    Ok(total_time)
}

/// JIT配置演示
fn demonstrate_jit_config() -> Result<()> {
    println!("=== JIT配置选项 ===\n");

    // 基础JIT配置
    let basic_config = EngineConfig {
        enable_jit: true,
        jit_threshold: 100,      // 执行100次后编译
        optimization_level: 0,   // 无优化
        ..Default::default()
    };

    println!("基础配置:");
    println!("  JIT启用: {}", basic_config.enable_jit);
    println!("  编译阈值: {}", basic_config.jit_threshold);
    println!("  优化级别: {}", basic_config.optimization_level);

    // 优化配置
    let optimized_config = EngineConfig {
        enable_jit: true,
        jit_threshold: 10,       // 更激进的编译
        optimization_level: 2,   // 高度优化
        inline_functions: true,
        ..Default::default()
    };

    println!("\n优化配置:");
    println!("  JIT启用: {}", optimized_config.enable_jit);
    println!("  编译阈值: {}", optimized_config.jit_threshold);
    println!("  优化级别: {}", optimized_config.optimization_level);
    println!("  内联函数: {}", optimized_config.inline_functions);

    Ok(())
}
