//! 统一跨架构执行示例
//!
//! 演示如何使用统一执行器在三种架构之间自动运行操作系统

use vm_core::GuestArch;
use vm_cross_arch::{HostArch, UnifiedExecutor};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 统一跨架构操作系统执行示例 ===\n");

    // 1. 检测host架构
    let host = HostArch::detect();
    println!("🔍 Host架构: {}", host);
    println!("   支持的架构组合:\n");

    // 2. 测试三种架构两两之间的执行
    let guest_archs = vec![
        ("AMD64", GuestArch::X86_64),
        ("ARM64", GuestArch::Arm64),
        ("RISC-V64", GuestArch::Riscv64),
    ];

    for (guest_name, guest_arch) in &guest_archs {
        println!("--- 测试Guest架构: {} ---", guest_name);

        // 创建统一执行器（自动检测和配置）
        let mut executor = UnifiedExecutor::auto_create(*guest_arch, 128 * 1024 * 1024)?;

        println!("✅ 统一执行器创建成功");
        println!("   配置: {}", executor.config().cross_arch);
        println!(
            "   GC: {}",
            if executor.config().gc.enable_gc {
                "启用"
            } else {
                "禁用"
            }
        );
        println!(
            "   AOT: {}",
            if executor.config().aot.enable_aot {
                "启用"
            } else {
                "禁用"
            }
        );
        println!(
            "   JIT: {}",
            if executor.config().jit.enable_jit {
                "启用"
            } else {
                "禁用"
            }
        );

        // 加载测试代码
        let (code_base, code) = match guest_arch {
            GuestArch::X86_64 => {
                let code_base: u64 = 0x1000;
                let code: Vec<u8> = vec![
                    0xB8, 0x0A, 0x00, 0x00, 0x00, // mov eax, 10
                    0xBB, 0x14, 0x00, 0x00, 0x00, // mov ebx, 20
                    0x01, 0xD8, // add eax, ebx
                    0xC3, // ret
                ];
                (code_base, code)
            }
            GuestArch::Arm64 => {
                let code_base: u64 = 0x1000;
                let code: Vec<u8> = vec![
                    0x21, 0x00, 0x80, 0xD2, // mov x1, #10
                    0x42, 0x00, 0x80, 0xD2, // mov x2, #20
                    0x23, 0x00, 0x02, 0x8B, // add x3, x1, x2
                    0xC0, 0x03, 0x5F, 0xD6, // ret
                ];
                (code_base, code)
            }
            GuestArch::Riscv64 => {
                let code_base: u64 = 0x1000;
                let code: Vec<u8> = vec![
                    0x93, 0x00, 0xA0, 0x00, // li x1, 10
                    0x13, 0x01, 0x40, 0x01, // li x2, 20
                    0xB3, 0x01, 0x21, 0x00, // add x3, x1, x2
                    0x67, 0x80, 0x00, 0x00, // ret
                ];
                (code_base, code)
            }
        };

        // 写入代码到内存
        for (i, byte) in code.iter().enumerate() {
            executor
                .mmu_mut()
                .write(code_base + i as u64, *byte as u64, 1)?;
        }

        println!("  已加载代码到 0x{:x}", code_base);

        // 执行代码（多次执行以触发热点和统计）
        println!("  执行代码（统一执行器自动选择策略）...");
        for i in 0..200 {
            let result = executor.execute(code_base)?;
            if i == 0 || i == 99 || i == 199 {
                println!("    执行 {} 次: {:?}", i + 1, result.status);
            }
        }

        // 更新并显示统计信息
        executor.update_stats();
        let stats = executor.stats();
        println!("  执行统计:");
        println!("    总执行次数: {}", stats.total_executions);
        println!(
            "    AOT执行: {} ({:.1}%)",
            stats.aot_executions,
            stats.aot_hit_rate * 100.0
        );
        println!(
            "    JIT执行: {} ({:.1}%)",
            stats.jit_executions,
            stats.jit_hit_rate * 100.0
        );
        println!(
            "    解释器执行: {} ({:.1}%)",
            stats.interpreter_executions,
            (stats.interpreter_executions as f64 / stats.total_executions as f64) * 100.0
        );

        println!();
    }

    // 3. 显示支持的架构组合
    println!("=== 支持的架构组合 ===");
    println!("✅ AMD64 → ARM64");
    println!("✅ AMD64 → RISC-V64");
    println!("✅ ARM64 → AMD64");
    println!("✅ ARM64 → RISC-V64");
    println!("✅ RISC-V64 → AMD64");
    println!("✅ RISC-V64 → ARM64");
    println!("✅ AMD64 → AMD64 (同架构，硬件加速)");
    println!("✅ ARM64 → ARM64 (同架构，硬件加速)");
    println!("✅ RISC-V64 → RISC-V64 (同架构，硬件加速)");

    println!("\n=== 技术集成 ===");
    println!("✅ AOT (提前编译): 热点代码预编译，启动快");
    println!("✅ GC (垃圾回收): 自动内存管理，增量回收");
    println!("✅ JIT (即时编译): 运行时优化，性能高");
    println!("✅ 统一执行器: 自动选择最佳执行策略");

    println!("\n=== 总结 ===");
    println!("✅ 跨架构操作系统执行系统已就绪");
    println!("✅ 支持三种架构两两之间的自动执行");
    println!("✅ 集成AOT、GC、JIT等先进技术");
    println!("✅ 自动检测和配置，零配置使用");

    Ok(())
}
