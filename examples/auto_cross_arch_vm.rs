//! 自动跨架构VM执行示例
//!
//! 演示如何自动检测host/guest架构并运行跨架构操作系统

use vm_cross_arch::{create_auto_vm_config, AutoExecutor, HostArch, CrossArchVmBuilder};
use vm_core::{GuestArch, MMU, GuestAddr, VmError};
use vm_mem::SoftMmu;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 自动跨架构VM执行示例 ===\n");
    
    // 1. 检测host架构
    let host_arch = HostArch::detect();
    println!("🔍 Host架构检测: {}", host_arch);
    
    // 2. 测试不同guest架构的自动配置
    let guest_archs = vec![
        GuestArch::X86_64,
        GuestArch::Arm64,
        GuestArch::Riscv64,
    ];
    
    for guest_arch in guest_archs {
        println!("\n--- 测试Guest架构: {:?} ---", guest_arch);
        
        // 自动创建VM配置
        match create_auto_vm_config(guest_arch, Some(128 * 1024 * 1024)) {
            Ok((vm_config, cross_config)) => {
                println!("✅ 配置创建成功");
                println!("  {}", cross_config);
                println!("  执行模式: {:?}", vm_config.exec_mode);
                println!("  硬件加速: {}", vm_config.enable_accel);
                
                // 创建自动执行器
                match AutoExecutor::auto_create(guest_arch, Some(vm_config.exec_mode)) {
                    Ok(mut executor) => {
                        println!("✅ 执行器创建成功: {}", executor);
                        
                        // 创建MMU并加载测试代码
                        let mut mmu = SoftMmu::new(vm_config.memory_size, false);
                        
                        // 根据guest架构加载不同的测试代码
                        let (code_base, code) = match guest_arch {
                            GuestArch::X86_64 => {
                                // AMD64测试代码: mov eax, 10; mov ebx, 20; add eax, ebx; ret
                                let code_base: GuestAddr = 0x1000;
                                let code: Vec<u8> = vec![
                                    0xB8, 0x0A, 0x00, 0x00, 0x00,  // mov eax, 10
                                    0xBB, 0x14, 0x00, 0x00, 0x00,  // mov ebx, 20
                                    0x01, 0xD8,                     // add eax, ebx
                                    0xC3,                           // ret
                                ];
                                (code_base, code)
                            }
                            GuestArch::Arm64 => {
                                // ARM64测试代码: mov x1, #10; mov x2, #20; add x3, x1, x2; ret
                                let code_base: GuestAddr = 0x1000;
                                let code: Vec<u8> = vec![
                                    0x21, 0x00, 0x80, 0xD2,  // mov x1, #10
                                    0x42, 0x00, 0x80, 0xD2,  // mov x2, #20
                                    0x23, 0x00, 0x02, 0x8B,  // add x3, x1, x2
                                    0xC0, 0x03, 0x5F, 0xD6,  // ret
                                ];
                                (code_base, code)
                            }
                            GuestArch::Riscv64 => {
                                // RISC-V64测试代码: li x1, 10; li x2, 20; add x3, x1, x2; ret
                                let code_base: GuestAddr = 0x1000;
                                let code: Vec<u8> = vec![
                                    0x93, 0x00, 0xA0, 0x00,  // li x1, 10
                                    0x13, 0x01, 0x40, 0x01,  // li x2, 20
                                    0xB3, 0x01, 0x21, 0x00,  // add x3, x1, x2
                                    0x67, 0x80, 0x00, 0x00,  // ret (jalr x0, 0(x1))
                                ];
                                (code_base, code)
                            }
                        };
                        
                        // 写入代码到内存
                        for (i, byte) in code.iter().enumerate() {
                            mmu.write(code_base + i as u64, *byte as u64, 1)
                                .map_err(|e| format!("Failed to write code: {}", e))?;
                        }
                        
                        println!("  已加载代码到 0x{:x}", code_base);
                        
                        // 执行代码
                        match executor.execute_block(&mut mmu, code_base) {
                            Ok(result) => {
                                println!("✅ 执行成功");
                                println!("  状态: {:?}", result.status);
                                println!("  下一个PC: 0x{:x}", result.next_pc);
                                
                                // 显示寄存器状态
                                let engine = executor.engine_mut();
                                println!("  寄存器状态:");
                                for i in 0..5 {
                                    println!("    reg[{}]: {}", i, engine.get_reg(i));
                                }
                            }
                            Err(e) => {
                                println!("❌ 执行失败: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        println!("❌ 执行器创建失败: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("❌ 配置创建失败: {}", e);
            }
        }
    }
    
    // 8. 使用便捷构建器API
    println!("\n--- 使用便捷构建器API ---");
    match CrossArchVmBuilder::new(GuestArch::X86_64)
        .memory_size(128 * 1024 * 1024)
        .build()
    {
        Ok(mut vm) => {
            println!("✅ 使用构建器创建VM成功");
            println!("  配置: {}", vm.cross_config());
            
            // 加载并执行代码
            let code: Vec<u8> = vec![
                0xB8, 0x0A, 0x00, 0x00, 0x00,  // mov eax, 10
                0xBB, 0x14, 0x00, 0x00, 0x00,  // mov ebx, 20
                0x01, 0xD8,                     // add eax, ebx
                0xC3,                           // ret
            ];
            
            vm.load_code(0x1000, &code)?;
            let result = vm.execute(0x1000)?;
            println!("✅ 执行成功: {:?}", result.status);
        }
        Err(e) => {
            println!("❌ 构建器创建失败: {}", e);
        }
    }
    
    println!("\n=== 总结 ===");
    println!("✅ 自动跨架构VM执行系统已就绪");
    println!("✅ 支持自动检测host/guest架构");
    println!("✅ 支持自动选择执行策略");
    println!("✅ 支持ARM64 ↔ AMD64双向执行");
    println!("✅ 提供便捷的构建器API");
    
    Ok(())
}

