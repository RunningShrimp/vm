//! 示例：在ARM64芯片上运行AMD64代码
//!
//! 这个示例演示了如何使用VM在ARM64 host上运行AMD64 guest代码。
//!
//! 执行流程：
//! 1. AMD64指令 → IR（通过vm-frontend-x86_64解码）
//! 2. IR → ARM64机器码（通过vm-engine-jit自动编译）或直接解释执行
//!
//! 注意：这个示例需要：
//! - 在ARM64 host上编译和运行
//! - 确保vm-frontend-x86_64和vm-engine-jit已启用

use vm_core::{GuestArch, ExecMode, VmConfig, MMU, GuestAddr};
use vm_mem::SoftMmu;
use vm_ir::IRBlock;
use vm_frontend_x86_64::X86_64Decoder;
use vm_engine_interpreter::Interpreter;
use vm_engine_jit::Jit;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 在ARM64上运行AMD64代码示例 ===");
    
    // 检测host架构
    #[cfg(target_arch = "aarch64")]
    {
        println!("✅ 检测到ARM64 host架构");
    }
    
    #[cfg(not(target_arch = "aarch64"))]
    {
        println!("⚠️  警告：当前不是ARM64架构，此示例在ARM64上运行效果最佳");
    }
    
    // 1. 创建VM配置
    let config = VmConfig {
        guest_arch: GuestArch::X86_64,  // Guest架构：AMD64
        memory_size: 128 * 1024 * 1024,  // 128MB内存
        vcpu_count: 1,
        exec_mode: ExecMode::Hybrid,     // 使用混合模式（解释器+JIT）
        enable_accel: false,             // 跨架构不支持硬件加速
        jit_threshold: 100,              // JIT热点阈值
        ..Default::default()
    };
    
    println!("配置：");
    println!("  Guest架构: {:?}", config.guest_arch);
    println!("  执行模式: {:?}", config.exec_mode);
    println!("  内存大小: {} MB", config.memory_size / (1024 * 1024));
    
    // 2. 创建内存管理单元
    let mut mmu = SoftMmu::new(config.memory_size, false);
    
    // 3. 加载AMD64机器码示例
    // 示例：简单的ADD指令
    // mov eax, 10      => 0xB8 0x0A 0x00 0x00 0x00
    // mov ebx, 20      => 0xBB 0x14 0x00 0x00 0x00
    // add eax, ebx     => 0x01 0xD8
    // ret              => 0xC3
    let code_base: GuestAddr = 0x1000;
    let amd64_code: Vec<u8> = vec![
        0xB8, 0x0A, 0x00, 0x00, 0x00,  // mov eax, 10
        0xBB, 0x14, 0x00, 0x00, 0x00,  // mov ebx, 20
        0x01, 0xD8,                     // add eax, ebx
        0xC3,                           // ret
    ];
    
    // 写入内存
    for (i, byte) in amd64_code.iter().enumerate() {
        mmu.write(code_base + i as u64, *byte as u64, 1)
            .map_err(|e| format!("Failed to write code: {}", e))?;
    }
    
    println!("\n已加载AMD64代码到地址 0x{:x}", code_base);
    
    // 4. 创建解码器（AMD64 → IR）
    let mut decoder = X86_64Decoder::new();
    
    // 5. 解码AMD64指令为IR
    println!("\n解码AMD64指令...");
    let ir_block = decoder.decode(&mut mmu, code_base)
        .map_err(|e| format!("Failed to decode AMD64 instruction: {}", e))?;
    
    println!("✅ 解码成功，生成IR块：");
    println!("  PC: 0x{:x}", ir_block.start_pc);
    println!("  操作数: {}", ir_block.ops.len());
    
    // 6. 执行IR（解释器模式）
    println!("\n=== 使用解释器执行 ===");
    let mut interpreter = Interpreter::new();
    
    // 设置初始寄存器状态
    interpreter.set_reg(0, 0); // EAX
    interpreter.set_reg(1, 0); // EBX
    interpreter.set_pc(code_base);
    
    let result = interpreter.run(&mut mmu, &ir_block)
        .map_err(|e| format!("Failed to execute IR: {}", e))?;
    
    println!("执行结果：");
    println!("  状态: {:?}", result.status);
    println!("  下一个PC: 0x{:x}", result.next_pc);
    println!("  EAX (reg 0): {}", interpreter.get_reg(0));
    println!("  EBX (reg 1): {}", interpreter.get_reg(1));
    
    // 7. 执行IR（JIT模式，如果可用）
    #[cfg(feature = "jit")]
    {
        println!("\n=== 使用JIT执行 ===");
        let mut jit = Jit::new();
        
        // 重置寄存器状态
        jit.set_reg(0, 0);
        jit.set_reg(1, 0);
        jit.set_pc(code_base);
        
        // JIT会自动将IR编译到ARM64机器码
        let jit_result = jit.run(&mut mmu, &ir_block)
            .map_err(|e| format!("Failed to execute with JIT: {}", e))?;
        
        println!("JIT执行结果：");
        println!("  状态: {:?}", jit_result.status);
        println!("  下一个PC: 0x{:x}", jit_result.next_pc);
        println!("  EAX (reg 0): {}", jit.get_reg(0));
        println!("  EBX (reg 1): {}", jit.get_reg(1));
        
        println!("\n✅ JIT已自动将IR编译为ARM64机器码并执行");
    }
    
    #[cfg(not(feature = "jit"))]
    {
        println!("\n⚠️  JIT功能未启用，跳过JIT测试");
    }
    
    println!("\n=== 总结 ===");
    println!("✅ 成功在ARM64 host上运行AMD64 guest代码");
    println!("✅ 执行流程：AMD64指令 → IR → ARM64机器码（JIT）或解释执行");
    println!("\n关键点：");
    println!("  1. vm-frontend-x86_64将AMD64指令解码为统一IR");
    println!("  2. vm-engine-jit自动将IR编译到host架构（ARM64）");
    println!("  3. 解释器直接执行IR，不依赖host架构");
    println!("  4. 因此无需vm-cross-arch进行运行时转换");
    
    Ok(())
}

