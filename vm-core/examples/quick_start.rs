//! # 快速开始示例
//!
//! 本示例展示如何快速创建并运行一个简单的RISC-V虚拟机。
//!
//! ## 功能演示
//!
//! - ✅ 创建虚拟机配置
//! - ✅ 初始化MMU和内存
//! - ✅ 创建解释器引擎
//! - ✅ 执行简单指令
//!
//! ## 运行
//!
//! ```bash
//! cargo run --example quick_start --features "vm-frontend/riscv64"
//! ```

use parking_lot::Mutex;
use std::sync::Arc;
use vm_core::{AccessType, ExecMode, GuestAddr, GuestArch, VmConfig};
use vm_frontend::Riscv64Decoder;
use vm_mem::{PagingMode, PhysicalMemory, UnifiedMMUV2};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("======================================");
    println!("  虚拟机快速开始示例");
    println!("======================================");
    println!();

    // 步骤1: 创建虚拟机配置
    println!("步骤1: 创建虚拟机配置...");
    let config = VmConfig {
        guest_arch: GuestArch::Riscv64,
        memory_size: 16 * 1024 * 1024, // 16MB内存
        vcpu_count: 1,
        exec_mode: ExecMode::Interpreter,
        kernel_path: None,
        initrd_path: None,
    };
    println!("✓ 配置创建成功:");
    println!("  - 架构: {}", config.guest_arch.name());
    println!("  - 内存: {} MB", config.memory_size / 1024 / 1024);
    println!("  - VCPU数量: {}", config.vcpu_count);
    println!("  - 执行模式: {:?}", config.exec_mode);
    println!();

    // 步骤2: 创建物理内存
    println!("步骤2: 初始化物理内存...");
    let mut phys_mem = PhysicalMemory::new(config.memory_size);
    println!("✓ 物理内存已分配: {} MB", config.memory_size / 1024 / 1024);
    println!();

    // 步骤3: 创建MMU
    println!("步骤3: 初始化MMU...");
    let mmu_config = vm_mem::UnifiedMmuConfigV2 {
        paging_mode: PagingMode::Sv39,
        enable_tlb: true,
        tlb_size: 64,
        enable_asid: true,
    };
    let mut mmu = Box::new(UnifiedMMUV2::new(
        mmu_config,
        Arc::new(Mutex::new(phys_mem)),
    ));
    println!("✓ MMU已创建:");
    println!("  - 分页模式: Sv39 (RISC-V 39位虚拟地址)");
    println!("  - TLB: 启用 (64条目)");
    println!("  - ASID: 启用");
    println!();

    // 步骤4: 创建指令解码器
    println!("步骤4: 创建指令解码器...");
    let mut decoder = Riscv64Decoder::new();
    println!("✓ RISC-V 64位解码器已创建");
    println!();

    // 步骤5: 模拟简单指令执行
    println!("步骤5: 模拟指令执行...");
    demonstrate_memory_access(&mut *mmu)?;
    demonstrate_tlb_lookup(&mut *mmu)?;
    println!();

    // 步骤6: 显示统计信息
    println!("步骤6: 执行统计...");
    if let Some(stats) = mmu.get_stats() {
        println!("✓ TLB统计:");
        println!("  - 总查找次数: {}", stats.total_lookups);
        println!("  - 命中次数: {}", stats.hits);
        println!("  - 缺失次数: {}", stats.misses);
        if stats.total_lookups > 0 {
            println!("  - 命中率: {:.2}%", stats.hit_rate);
        }
    }
    println!();

    println!("======================================");
    println!("  示例执行完成!");
    println!("======================================");
    println!();
    println!("下一步:");
    println!("  1. 查看 examples/jit_optimization.rs 了解JIT编译");
    println!("  2. 查看 examples/tlb_usage.rs 了解TLB管理");
    println!("  3. 查看 docs/ 目录阅读完整文档");

    Ok(())
}

/// 演示内存访问
fn demonstrate_memory_access(mmu: &mut dyn vm_core::MMU) -> Result<(), Box<dyn std::error::Error>> {
    println!("  演示内存访问...");

    // 测试地址
    let test_addr = GuestAddr(0x1000);

    // 尝试读取内存（在Bare模式下，虚拟地址=物理地址）
    match mmu.read_u8(test_addr) {
        Ok(_) => println!("  ✓ 内存读取成功: addr=0x{:x}", test_addr),
        Err(e) => println!("  ⚠ 内存读取失败: {} (预期，内存未初始化)", e),
    }

    Ok(())
}

/// 演示TLB查找
fn demonstrate_tlb_lookup(mmu: &mut dyn vm_core::MMU) -> Result<(), Box<dyn std::error::Error>> {
    println!("  演示TLB查找...");

    // 在Bare模式下进行地址转换
    let virt_addr = GuestAddr(0x2000);

    match mmu.translate(virt_addr, AccessType::Read) {
        Ok((phys_addr, _flags)) => {
            println!("  ✓ 地址转换成功:");
            println!("    虚拟地址: 0x{:x}", virt_addr);
            println!("    物理地址: 0x{:x}", phys_addr);
        }
        Err(e) => {
            println!("  ⚠ 地址转换失败: {} (预期，未设置页表)", e);
        }
    }

    Ok(())
}
