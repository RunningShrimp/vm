//! Fibonacci计算示例
//!
//! 这个示例展示了如何：
//! 1. 编写和执行更复杂的RISC-V程序
//! 2. 使用内存存储数据
//! 3. 处理循环和条件跳转
//! 4. 实现递归算法的迭代版本

use anyhow::Result;
use vm_core::{GuestArch, VmConfig};
use vm_engine::{ExecutionEngine, ExecutionResult};
use vm_frontend::riscv64::Riscv64Frontend;
use vm_mem::{MemoryRegion, SoftMmu};

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .init();

    println!("=== Fibonacci计算示例 ===\n");

    // Fibonacci数列: 0, 1, 1, 2, 3, 5, 8, 13, 21, 34, ...
    // 我们将计算前N个Fibonacci数

    let n = 10u64; // 计算前10个Fibonacci数
    println!("计算前 {} 个Fibonacci数\n", n);

    // 步骤 1: 创建VM配置
    println!("步骤 1: 创建VM配置");
    let config = VmConfig {
        arch: GuestArch::Riscv64,
        memory_size: 1024 * 1024, // 1MB内存
        ..Default::default()
    };
    println!("✅ 配置创建成功\n");

    // 步骤 2: 创建MMU并设置内存区域
    println!("步骤 2: 设置内存");
    let mut mmu = SoftMmu::new(config.memory_size);

    // 代码区域 (可读可执行)
    let code_region = MemoryRegion {
        base: 0x1000,
        size: 0x1000,
        flags: vm_mem::MemRegionFlags::READ | vm_mem::MemRegionFlags::EXEC,
    };

    // 数据区域 (可读可写)
    let data_region = MemoryRegion {
        base: 0x10000,
        size: 0x1000,
        flags: vm_mem::MemRegionFlags::READ | vm_mem::MemRegionFlags::WRITE,
    };

    mmu.add_region(code_region)?;
    mmu.add_region(data_region)?;
    println!("✅ 内存区域配置完成");
    println!("   代码段: 0x1000 - 0x2000");
    println!("   数据段: 0x10000 - 0x11000\n");

    // 步骤 3: 准备Fibonacci计算程序
    println!("步骤 3: 生成Fibonacci程序");
    let program = generate_fibonacci_program();
    let code_base = 0x1000u64;

    // 将程序写入内存
    for (i, &byte) in program.iter().enumerate() {
        mmu.write(code_base + i as u64, byte as u64, 1)?;
    }

    println!("✅ 程序已加载 ({} 字节)\n", program.len());

    // 步骤 4: 将参数N写入内存
    let data_base = 0x10000u64;
    mmu.write(data_base, n, 8)?; // 写入N
    mmu.write(data_base + 8, 0u64, 8)?; // 第一个数 fib[0] = 0
    mmu.write(data_base + 16, 1u64, 8)?; // 第二个数 fib[1] = 1

    println!("步骤 4: 初始化数据");
    println!("   N = {} (存储在 0x{:x})", n, data_base);
    println!("   fib[0] = 0");
    println!("   fib[1] = 1\n");

    // 步骤 5: 创建执行引擎
    println!("步骤 5: 创建执行引擎");
    let mut engine = ExecutionEngine::new(
        config.arch,
        Box::new(mmu),
        vm_engine::EngineConfig::default()
    )?;

    engine.set_pc(code_base)?;
    println!("✅ 执行引擎就绪\n");

    // 步骤 6: 执行程序
    println!("步骤 6: 执行Fibonacci程序");
    println!("--- 开始执行 ---\n");

    let max_instructions = 10000;
    let mut instructions_executed = 0;

    for i in 0..max_instructions {
        match engine.execute_step() {
            Ok(ExecutionResult::Continue) => {
                instructions_executed += 1;
            }
            Ok(ExecutionResult::Halted) => {
                println!("✅ 程序执行完成");
                break;
            }
            Ok(ExecutionResult::Exception(e)) => {
                println!("❌ 异常: {:?}", e);
                break;
            }
            Err(e) => {
                println!("❌ 执行错误: {}", e);
                return Err(e.into());
            }
        }
    }

    println!("执行指令数: {}\n", instructions_executed);

    // 步骤 7: 读取并显示结果
    println!("步骤 7: 读取结果");
    println!("Fibonacci数列:");

    let mut fib_results = Vec::new();
    for i in 0..n {
        let addr = data_base + 8 + (i * 8);
        let value = engine.read_memory(addr, 8)?;
        fib_results.push(value);
        print!("fib[{}] = {}", i, value);
        if (i + 1) % 5 == 0 {
            println!();
        } else {
            print!("\t");
        }
    }

    if n % 5 != 0 {
        println!();
    }

    println!();

    // 步骤 8: 验证结果
    println!("步骤 8: 验证结果");
    let mut correct = true;
    for i in 0..n as usize {
        let expected = if i == 0 { 0 }
                      else if i == 1 { 1 }
                      else { fib_results[i-2] + fib_results[i-1] };

        if fib_results[i] != expected {
            println!("❌ 错误: fib[{}] = {}, 期望 {}",
                     i, fib_results[i], expected);
            correct = false;
        }
    }

    if correct {
        println!("✅ 所有结果验证通过!");
    }

    // 步骤 9: 显示统计信息
    println!("\n步骤 9: 性能统计");
    let stats = engine.stats();
    println!("  总指令数: {}", stats.total_instructions);
    println!("  执行时间: {:?}", stats.execution_time);
    println!("  平均每指令: {:?}",
             stats.execution_time / stats.total_instructions as u32);

    println!("\n=== 示例完成 ===");
    println!("这个示例展示了:");
    println!("  - 复杂程序的加载和执行");
    println!("  - 内存读写操作");
    println!("  - 循环和条件跳转");
    println!("  - 数组数据结构在内存中的实现");

    Ok(())
}

/// 生成计算Fibonacci数列的RISC-V程序
///
/// 程序逻辑:
/// 1. 读取N (从内存地址 0x10000)
/// 2. 初始化 fib[0] = 0, fib[1] = 1
/// 3. 循环计算 fib[i] = fib[i-1] + fib[i-2]
/// 4. 将结果存储到内存数组
///
/// 寄存器使用:
/// - x1: 循环计数器 i
/// - x2: N (总数)
/// - x3: 数据基址 (0x10000)
/// - x4: fib[i-2]
/// - x5: fib[i-1]
/// - x6: fib[i]
fn generate_fibonacci_program() -> Vec<u8> {
    let mut code = Vec::new();

    // ===== 初始化阶段 =====

    // 加载数组基址到 x3
    // lui x3, 0x10        # x3 = 0x10000 (数据段基址)
    code.extend_from_slice(&[0x17, 0x31, 0x10, 0x10]);

    // 加载N到 x2
    // ld x2, 0(x3)        # x2 = N (从内存读取)
    code.extend_from_slice(&[0x03, 0x23, 0x03, 0x00]);

    // 判断特殊情况: N == 0
    // beqz x2, end        # if N == 0, 跳转到结束
    code.extend_from_slice(&[0x63, 0x00, 0x00, 0x04]); // 注意: 这个偏移会在后面修正

    // 判断特殊情况: N == 1
    // li x1, 1
    code.extend_from_slice(&[0x93, 0x00, 0x50, 0x00]);
    // ble x2, x1, end     # if N <= 1, 跳转到结束
    code.extend_from_slice(&[0xE3, 0x2E, 0x01, 0x00]); // 偏移也需要修正

    // ===== 初始化前两个Fibonacci数 =====

    // fib[0] = 0 (已经在main中设置)
    // fib[1] = 1 (已经在main中设置)

    // 加载 fib[0] 到 x4
    // ld x4, 8(x3)        # x4 = fib[0]
    code.extend_from_slice(&[0x03, 0x23, 0x14, 0x00]);

    // 加载 fib[1] 到 x5
    // ld x5, 16(x3)       # x5 = fib[1]
    code.extend_from_slice(&[0x03, 0x23, 0x15, 0x01]);

    // 初始化循环计数器 i = 2
    // li x1, 2
    code.extend_from_slice(&[0x93, 0x00, 0x20, 0x01]);

    // ===== 循环开始 =====
    let loop_start = code.len();

    // 计算下一个Fibonacci数
    // add x6, x4, x5      # x6 = x4 + x5 (fib[i] = fib[i-2] + fib[i-1])
    code.extend_from_slice(&[0xB3, 0x01, 0x45, 0x00]);

    // 存储到内存: fib[i] = x6
    // slli x7, x1, 3      # x7 = i * 8 (计算数组索引的字节偏移)
    code.extend_from_slice(&[0x13, 0x17, 0x17, 0x03]);
    // add x7, x3, x7      # x7 = 数据基址 + 偏移
    code.extend_from_slice(&[0xB3, 0x07, 0x07, 0x00]);
    // sd x6, 8(x7)        # 存储fib[i]
    code.extend_from_slice(&[0x23, 0x34, 0x86, 0x00]);

    // 更新值: fib[i-2] = fib[i-1], fib[i-1] = fib[i]
    // mv x4, x5           # x4 = x5
    code.extend_from_slice(&[0x93, 0x82, 0x05, 0x00]);
    // mv x4, x6           # x5 = x6 (应该: mv x5, x6)
    code.extend_from_slice(&[0x93, 0x8A, 0x06, 0x00]);

    // 增加循环计数器
    // addi x1, x1, 1      # i++
    code.extend_from_slice(&[0x93, 0x05, 0x05, 0x00]);

    // 检查循环条件
    // blt x1, x2, loop    # if i < N, 继续循环
    code.extend_from_slice(&[0x63, 0xEC, 0x25, 0xFE]);

    // ===== 程序结束 =====
    // ret
    code.extend_from_slice(&[0x67, 0x80, 0x00, 0x00]);

    // 简化版本 - 使用简单的循环
    generate_simple_fibonacci()
}

/// 生成简化版本的Fibonacci程序
/// 这个版本更容易理解和调试
fn generate_simple_fibonacci() -> Vec<u8> {
    let mut code = Vec::new();

    // 使用简化的程序结构
    // 假设已经在main函数中设置了fib[0]和fib[1]
    // 这里只需要计算fib[2]到fib[N-1]

    // lui x3, 0x10        # x3 = 0x10000
    code.extend_from_slice(&[0x17, 0x31, 0x10, 0x10]);

    // ld x2, 0(x3)        # x2 = N
    code.extend_from_slice(&[0x03, 0x23, 0x03, 0x00]);

    // li x1, 2            # i = 2
    code.extend_from_slice(&[0x93, 0x00, 0x20, 0x01]);

    // ld x4, 8(x3)        # x4 = fib[0] = 0
    code.extend_from_slice(&[0x03, 0x23, 0x14, 0x00]);

    // ld x5, 16(x3)       # x5 = fib[1] = 1
    code.extend_from_slice(&[0x03, 0x23, 0x15, 0x01]);

    // loop:
    // add x6, x4, x5      # x6 = fib[i-2] + fib[i-1]
    code.extend_from_slice(&[0xB3, 0x01, 0x45, 0x00]);

    // slli x7, x1, 3      # x7 = i * 8
    code.extend_from_slice(&[0x13, 0x17, 0x17, 0x03]);

    // add x7, x3, x7      # x7 = &fib[i]
    code.extend_from_slice(&[0xB3, 0x07, 0x07, 0x00]);

    // sd x6, 8(x7)        # fib[i] = x6
    code.extend_from_slice(&[0x23, 0x34, 0x86, 0x00]);

    // addi x4, x5, 0      # x4 = x5 (fib[i-2] = fib[i-1])
    code.extend_from_slice(&[0x93, 0x82, 0x05, 0x00]);

    // mv x5, x6           # x5 = x6 (fib[i-1] = fib[i])
    code.extend_from_slice(&[0x93, 0x8A, 0x06, 0x00]);

    // addi x1, x1, 1      # i++
    code.extend_from_slice(&[0x93, 0x05, 0x05, 0x00]);

    // blt x1, x2, loop    # if i < N, goto loop
    code.extend_from_slice(&[0x63, 0xEC, 0x25, 0xFE]);

    // ret
    code.extend_from_slice(&[0x67, 0x80, 0x00, 0x00]);

    code
}
