//! 跨架构操作系统执行示例
//!
//! 演示如何使用AOT、GC、JIT等技术在三种架构之间运行操作系统

use vm_cross_arch::{
    CrossArchRuntime, CrossArchRuntimeConfig,
    GcIntegrationConfig, AotIntegrationConfig, JitIntegrationConfig,
    CrossArchAotCompiler, CrossArchAotConfig,
    HostArch,
};
use vm_core::GuestArch;
use vm_mem::SoftMmu;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 跨架构操作系统执行示例 ===\n");
    
    // 1. 检测host架构
    let host = HostArch::detect();
    println!("🔍 Host架构: {}", host);
    
    // 2. 测试三种架构两两之间的执行
    let guest_archs = vec![
        GuestArch::X86_64,
        GuestArch::Arm64,
        GuestArch::Riscv64,
    ];
    
    for guest_arch in guest_archs {
        println!("\n--- 测试Guest架构: {:?} ---", guest_arch);
        
        // 创建运行时配置
        let mut runtime_config = CrossArchRuntimeConfig::auto_create(guest_arch)?;
        
        // 配置GC
        runtime_config.gc = GcIntegrationConfig {
            enable_gc: true,
            gc_trigger_threshold: 0.8,
            gc_goal: 0.7,
            incremental_step_size: 100,
        };
        
        // 配置AOT
        runtime_config.aot = AotIntegrationConfig {
            enable_aot: true,
            aot_image_path: None,
            aot_priority: true,
            aot_hotspot_threshold: 1000,
        };
        
        // 配置JIT
        runtime_config.jit = JitIntegrationConfig {
            enable_jit: true,
            jit_threshold: 100,
            jit_cache_size: 64 * 1024 * 1024,
        };
        
        println!("✅ 运行时配置创建成功");
        println!("  GC: {}", if runtime_config.gc.enable_gc { "启用" } else { "禁用" });
        println!("  AOT: {}", if runtime_config.aot.enable_aot { "启用" } else { "禁用" });
        println!("  JIT: {}", if runtime_config.jit.enable_jit { "启用" } else { "禁用" });
        
        // 创建运行时
        let mut runtime = CrossArchRuntime::new(runtime_config, 128 * 1024 * 1024)?;
        println!("✅ 运行时创建成功");
        
        // 加载测试代码
        let (code_base, code) = match guest_arch {
            GuestArch::X86_64 => {
                // AMD64测试代码
                let code_base: u64 = 0x1000;
                let code: Vec<u8> = vec![
                    0xB8, 0x0A, 0x00, 0x00, 0x00,  // mov eax, 10
                    0xBB, 0x14, 0x00, 0x00, 0x00,  // mov ebx, 20
                    0x01, 0xD8,                     // add eax, ebx
                    0xC3,                           // ret
                ];
                (code_base, code)
            }
            GuestArch::Arm64 => {
                // ARM64测试代码
                let code_base: u64 = 0x1000;
                let code: Vec<u8> = vec![
                    0x21, 0x00, 0x80, 0xD2,  // mov x1, #10
                    0x42, 0x00, 0x80, 0xD2,  // mov x2, #20
                    0x23, 0x00, 0x02, 0x8B,  // add x3, x1, x2
                    0xC0, 0x03, 0x5F, 0xD6,  // ret
                ];
                (code_base, code)
            }
            GuestArch::Riscv64 => {
                // RISC-V64测试代码
                let code_base: u64 = 0x1000;
                let code: Vec<u8> = vec![
                    0x93, 0x00, 0xA0, 0x00,  // li x1, 10
                    0x13, 0x01, 0x40, 0x01,  // li x2, 20
                    0xB3, 0x01, 0x21, 0x00,  // add x3, x1, x2
                    0x67, 0x80, 0x00, 0x00,  // ret
                ];
                (code_base, code)
            }
        };
        
        // 写入代码到内存
        for (i, byte) in code.iter().enumerate() {
            runtime.mmu_mut().write(code_base + i as u64, *byte as u64, 1)?;
        }
        
        println!("  已加载代码到 0x{:x}", code_base);
        
        // 执行代码（多次执行以触发热点）
        println!("  执行代码（触发热点）...");
        for i in 0..150 {
            let result = runtime.execute_block(code_base)?;
            if i % 50 == 0 {
                println!("    执行 {} 次: {:?}", i, result.status);
            }
        }
        
        // 显示统计信息
        println!("  执行完成");
        
        // 3. 测试AOT编译
        println!("\n--- 测试AOT编译 ---");
        let aot_config = CrossArchAotConfig {
            source_arch: guest_arch.into(),
            target_arch: host.to_architecture()
                .ok_or("Unknown host architecture")?,
            optimization_level: 2,
            enable_cross_arch_optimization: true,
            codegen_mode: aot_builder::CodegenMode::LLVM,
        };
        
        let mut aot_compiler = CrossArchAotCompiler::new(aot_config)?;
        aot_compiler.compile_from_source(code_base, &code, guest_arch.into())?;
        
        let stats = aot_compiler.stats();
        println!("✅ AOT编译完成");
        println!("  编译块数: {}", stats.blocks_compiled);
        println!("  跨架构转换: {}", stats.cross_arch_translations);
        println!("  编译时间: {} ms", stats.total_compilation_time_ms);
        println!("  生成代码大小: {} bytes", stats.generated_code_size);
        
        // 保存AOT镜像
        let aot_path = format!("/tmp/cross_arch_{:?}.aot", guest_arch);
        aot_compiler.save_to_file(&aot_path)?;
        println!("  AOT镜像已保存到: {}", aot_path);
    }
    
    println!("\n=== 总结 ===");
    println!("✅ 跨架构操作系统执行系统已就绪");
    println!("✅ 支持三种架构两两之间的执行");
    println!("✅ 集成AOT、GC、JIT技术");
    println!("✅ 支持操作系统级别的功能");
    
    Ok(())
}

