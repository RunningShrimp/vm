//! Hello World - 最简单的VM示例
//!
//! 这个示例展示了如何：
//! 1. 创建一个基本的虚拟机
//! 2. 加载并执行简单的程序
//! 3. 访问寄存器和内存
//! 4. 处理执行结果

use anyhow::Result;
use vm_core::{GuestArch, VmConfig};
use vm_engine::{ExecutionEngine, ExecutionResult};
use vm_frontend::riscv64::Riscv64Frontend;
use vm_mem::{MemoryRegion, SoftMmu};

fn main() -> Result<()> {
    // 初始化日志
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .init();

    println!("=== VM Hello World 示例 ===\n");

    // 步骤 1: 创建VM配置
    println!("步骤 1: 创建VM配置");
    let config = VmConfig {
        arch: GuestArch::Riscv64,
        memory_size: 1024 * 1024, // 1MB内存
        ..Default::default()
    };
    println!("✅ 配置创建成功");
    println!("   架构: {:?}", config.arch);
    println!("   内存大小: {} MB\n", config.memory_size / (1024 * 1024));

    // 步骤 2: 创建内存管理单元
    println!("步骤 2: 创建内存管理单元(MMU)");
    let mut mmu = SoftMmu::new(config.memory_size);

    // 添加内存区域
    let code_region = MemoryRegion {
        base: 0x1000,
        size: 0x1000,
        flags: vm_mem::MemRegionFlags::READ | vm_mem::MemRegionFlags::EXEC,
    };

    mmu.add_region(code_region.clone())?;
    println!("✅ MMU创建成功");
    println!("   代码区域: 0x{:x} - 0x{:x}\n", code_region.base,
             code_region.base + code_region.size);

    // 步骤 3: 准备简单的RISC-V程序
    println!("步骤 3: 准备RISC-V程序");
    // 这个程序计算 10 + 20 并返回结果
    // RISC-V 64位汇编:
    //   li  x1, 10      # 将10加载到x1寄存器
    //   li  x2, 20      # 将20加载到x2寄存器
    //   add x3, x1, x2  # x3 = x1 + x2 (结果为30)
    //   ret             # 返回
    let program: Vec<u8> = vec![
        0x93, 0x00, 0xA0, 0x00,  // li  x1, 10
        0x13, 0x01, 0x40, 0x01,  // li  x2, 20
        0xB3, 0x01, 0x21, 0x00,  // add x3, x1, x2
        0x67, 0x80, 0x00, 0x00,  // ret
    ];

    let code_base = 0x1000u64;

    // 将程序写入内存
    for (i, &byte) in program.iter().enumerate() {
        mmu.write(code_base + i as u64, byte as u64, 1)?;
    }

    println!("✅ 程序已加载到内存");
    println!("   代码基地址: 0x{:x}", code_base);
    println!("   程序大小: {} 字节\n", program.len());

    // 步骤 4: 创建RISC-V前端
    println!("步骤 4: 创建指令集前端");
    let mut frontend = Riscv64Frontend::new();
    println!("✅ 前端创建成功\n");

    // 步骤 5: 创建执行引擎
    println!("步骤 5: 创建执行引擎");
    let mut engine = ExecutionEngine::new(
        config.arch,
        Box::new(mmu),
        vm_engine::EngineConfig::default()
    )?;
    println!("✅ 执行引擎创建成功\n");

    // 步骤 6: 设置程序计数器
    println!("步骤 6: 设置程序计数器(PC)");
    engine.set_pc(code_base)?;
    println!("✅ PC = 0x{:x}\n", code_base);

    // 步骤 7: 执行程序
    println!("步骤 7: 执行程序");
    println!("--- 开始执行 ---\n");

    let max_instructions = 100;
    let mut instructions_executed = 0;

    for i in 0..max_instructions {
        let pc = engine.pc();

        // 执行一条指令
        match engine.execute_step() {
            Ok(ExecutionResult::Continue) => {
                instructions_executed += 1;

                // 打印前几条指令的执行情况
                if i < 5 {
                    println!("执行第 {} 条指令: PC = 0x{:x}", i + 1, pc);
                }
            }
            Ok(ExecutionResult::Halted) => {
                println!("\n✅ 程序执行完成");
                break;
            }
            Ok(ExecutionResult::Exception(e)) => {
                println!("\n❌ 异常发生: {:?}", e);
                break;
            }
            Err(e) => {
                println!("\n❌ 执行错误: {}", e);
                return Err(e.into());
            }
        }
    }

    println!("总执行指令数: {}\n", instructions_executed);

    // 步骤 8: 检查结果
    println!("步骤 8: 检查执行结果");

    // 读取寄存器
    let x1 = engine.read_register(1)?;
    let x2 = engine.read_register(2)?;
    let x3 = engine.read_register(3)?;

    println!("寄存器状态:");
    println!("  x1 (a0) = {}", x1);
    println!("  x2 (a1) = {}", x2);
    println!("  x3 (a2) = {} (x1 + x2的结果)", x3);

    // 验证结果
    if x3 == 30 {
        println!("\n✅ 结果验证成功! 10 + 20 = {}", x3);
    } else {
        println!("\n❌ 结果错误: 期望 30, 实际 {}", x3);
    }

    // 步骤 9: 显示统计信息
    println!("\n步骤 9: 执行统计");
    let stats = engine.stats();
    println!("  总指令数: {}", stats.total_instructions);
    println!("  执行时间: {:?}", stats.execution_time);

    println!("\n=== 示例完成 ===");
    println!("这个示例展示了VM的基本使用流程:");
    println!("  1. 创建VM配置");
    println!("  2. 设置内存管理单元");
    println!("  3. 加载程序到内存");
    println!("  4. 创建执行引擎");
    println!("  5. 执行程序并检查结果");

    Ok(())
}
