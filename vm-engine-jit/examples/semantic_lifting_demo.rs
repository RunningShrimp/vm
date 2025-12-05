//! 语义库与 LLVM IR 提升演示
//!
//! 展示如何使用 vm-ir-lift 进行指令抬升和优化

use vm_ir_lift::{
    LiftingContext,
    decoder::{ISA, create_decoder},
    optimizer::{OptimizationLevel, PassManager},
    semantics::create_semantics,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔════════════════════════════════════════════════════════╗");
    println!("║      语义库与 LLVM IR 提升演示 (Semantic Lifting)      ║");
    println!("╚════════════════════════════════════════════════════════╝\n");

    // 演示 1: x86-64 指令解码与提升
    demo_x86_64()?;

    // 演示 2: ARM64 指令解码与提升
    demo_arm64()?;

    // 演示 3: RISC-V 指令解码与提升
    demo_riscv64()?;

    // 演示 4: 优化管线
    demo_optimization_pipeline()?;

    println!("\n✅ 所有演示完成！\n");

    Ok(())
}

fn demo_x86_64() -> Result<(), Box<dyn std::error::Error>> {
    println!("【演示 1】x86-64 指令提升");
    println!("═══════════════════════════════════════\n");

    let mut ctx = LiftingContext::new(ISA::X86_64);
    let decoder = create_decoder(ISA::X86_64);
    let semantics = create_semantics(ISA::X86_64);

    // 指令 1: NOP (0x90)
    {
        println!("指令: NOP (0x90)");
        let bytes = vec![0x90];
        match decoder.decode(&bytes) {
            Ok((instr, len)) => {
                println!("  ✓ 解码成功");
                println!("    - 指令: {}", instr.mnemonic);
                println!("    - 长度: {} 字节", len);

                match semantics.lift(&instr, &mut ctx) {
                    Ok(ir) => {
                        println!("  ✓ 语义提升成功");
                        println!("    - IR: {}", ir);
                        println!("    - 缓存统计: {} 条", ctx.cache_stats());
                    }
                    Err(e) => println!("  ✗ 提升失败: {}", e),
                }
            }
            Err(e) => println!("  ✗ 解码失败: {}", e),
        }
    }

    println!();

    // 指令 2: MOV rbx, rax (0x48 0x89 0xC3)
    {
        println!("指令: MOV rbx, rax (0x48 0x89 0xC3)");
        let bytes = vec![0x48, 0x89, 0xC3];
        match decoder.decode(&bytes) {
            Ok((instr, len)) => {
                println!("  ✓ 解码成功");
                println!("    - 指令: {}", instr.mnemonic);
                println!("    - 操作数: {:?}", instr.operands);
                println!("    - 长度: {} 字节", len);

                match semantics.lift(&instr, &mut ctx) {
                    Ok(ir) => {
                        println!("  ✓ 语义提升成功");
                        println!("    - IR: {}", ir);
                    }
                    Err(e) => println!("  ✗ 提升失败: {}", e),
                }
            }
            Err(e) => println!("  ✗ 解码失败: {}", e),
        }
    }

    println!();

    // 指令 3: ADD rax, rbx (0x48 0x01 0xD8)
    {
        println!("指令: ADD rax, rbx (0x48 0x01 0xD8)");
        let bytes = vec![0x48, 0x01, 0xD8];
        match decoder.decode(&bytes) {
            Ok((instr, len)) => {
                println!("  ✓ 解码成功");
                println!("    - 指令: {}", instr.mnemonic);
                println!("    - 操作数: {:?}", instr.operands);
                println!("    - 长度: {} 字节", len);
                println!("    - 隐式写入: {:?}", instr.implicit_writes);

                match semantics.lift(&instr, &mut ctx) {
                    Ok(ir) => {
                        println!("  ✓ 语义提升成功");
                        println!("    - IR:");
                        for line in ir.lines() {
                            println!("      {}", line);
                        }
                    }
                    Err(e) => println!("  ✗ 提升失败: {}", e),
                }
            }
            Err(e) => println!("  ✗ 解码失败: {}", e),
        }
    }

    println!("\n缓存统计: {} 条指令已缓存\n", ctx.cache_stats());
    Ok(())
}

fn demo_arm64() -> Result<(), Box<dyn std::error::Error>> {
    println!("【演示 2】ARM64 指令提升");
    println!("═══════════════════════════════════════\n");

    let mut ctx = LiftingContext::new(ISA::ARM64);
    let decoder = create_decoder(ISA::ARM64);
    let semantics = create_semantics(ISA::ARM64);

    // 指令: NOP
    {
        println!("指令: NOP");
        let bytes = vec![0x1F, 0x20, 0x03, 0xD5];
        match decoder.decode(&bytes) {
            Ok((instr, len)) => {
                println!("  ✓ 解码成功");
                println!("    - 指令: {}", instr.mnemonic);
                println!("    - 长度: {} 字节", len);

                match semantics.lift(&instr, &mut ctx) {
                    Ok(ir) => {
                        println!("  ✓ 语义提升成功");
                        println!("    - IR: {}", ir);
                    }
                    Err(e) => println!("  ✗ 提升失败: {}", e),
                }
            }
            Err(e) => println!("  ✗ 解码失败: {}", e),
        }
    }

    println!();

    // 指令: ADD x0, x1, x2
    {
        println!("指令: ADD x0, x1, x2");
        let bytes = vec![0x20, 0x00, 0x02, 0x8B];
        match decoder.decode(&bytes) {
            Ok((instr, len)) => {
                println!("  ✓ 解码成功");
                println!("    - 指令: {}", instr.mnemonic);
                println!("    - 操作数: {:?}", instr.operands);
                println!("    - 长度: {} 字节", len);

                match semantics.lift(&instr, &mut ctx) {
                    Ok(ir) => {
                        println!("  ✓ 语义提升成功");
                        println!("    - IR: {}", ir);
                    }
                    Err(e) => println!("  ✗ 提升失败: {}", e),
                }
            }
            Err(e) => println!("  ✗ 解码失败: {}", e),
        }
    }

    println!();
    Ok(())
}

fn demo_riscv64() -> Result<(), Box<dyn std::error::Error>> {
    println!("【演示 3】RISC-V 指令提升");
    println!("═══════════════════════════════════════\n");

    let mut ctx = LiftingContext::new(ISA::RISCV64);
    let decoder = create_decoder(ISA::RISCV64);
    let semantics = create_semantics(ISA::RISCV64);

    // 指令: ADDI x1, x1, 8
    {
        println!("指令: ADDI x1, x1, 8");
        let bytes = vec![0x93, 0x05, 0x80, 0x00]; // ADDI x1, x1, 8
        match decoder.decode(&bytes) {
            Ok((instr, len)) => {
                println!("  ✓ 解码成功");
                println!("    - 指令: {}", instr.mnemonic);
                println!("    - 操作数: {:?}", instr.operands);
                println!("    - 长度: {} 字节", len);

                match semantics.lift(&instr, &mut ctx) {
                    Ok(ir) => {
                        println!("  ✓ 语义提升成功");
                        println!("    - IR: {}", ir);
                    }
                    Err(e) => println!("  ✗ 提升失败: {}", e),
                }
            }
            Err(e) => println!("  ✗ 解码失败: {}", e),
        }
    }

    println!();
    Ok(())
}

fn demo_optimization_pipeline() -> Result<(), Box<dyn std::error::Error>> {
    println!("【演示 4】LLVM 优化管线");
    println!("═══════════════════════════════════════\n");

    let pm_o0 = PassManager::new(OptimizationLevel::O0);
    let pm_o1 = PassManager::new(OptimizationLevel::O1);
    let pm_o2 = PassManager::new(OptimizationLevel::O2);

    println!("优化等级 O0 (快速 JIT 编译):");
    println!("  通过数: {}", pm_o0.passes().len());
    for (i, pass) in pm_o0.passes().iter().enumerate() {
        println!("    {}. {:?}", i + 1, pass);
    }

    println!("\n优化等级 O1 (平衡编译和执行):");
    println!("  通过数: {}", pm_o1.passes().len());
    for (i, pass) in pm_o1.passes().iter().enumerate() {
        println!("    {}. {:?}", i + 1, pass);
    }

    println!("\n优化等级 O2 (激进优化 AOT):");
    println!("  通过数: {}", pm_o2.passes().len());
    for (i, pass) in pm_o2.passes().iter().enumerate() {
        println!("    {}. {:?}", i + 1, pass);
    }

    println!("\n✅ 优化管线配置就绪\n");

    Ok(())
}
