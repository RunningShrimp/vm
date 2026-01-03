//! 跨架构执行示例
//!
//! 本示例演示虚拟机的跨架构翻译和执行能力，包括：
//! - 基础跨架构翻译
//! - 寄存器映射
//! - 指令模式匹配
//! - 内存访问优化
//! - 翻译管线使用
//! - 性能对比
//!
//! 运行方式：
//! ```bash
//! cargo run --example cross_arch_execution --package vm-cross-arch-support
//! ```

use std::time::Instant;

use vm_cross_arch_support::{
    TranslationRegId, // 使用translation_pipeline的RegId枚举
    encoding::{Architecture, EncodingContext, RegId},
    memory_access::{
        AccessType, AccessWidth, Alignment, DefaultMemoryAccessOptimizer, MemoryAccessAnalyzer,
        MemoryAccessOptimizer, MemoryAccessPattern,
    },
    register::{MappingStrategy, RegisterMapper, RegisterSet},
    translation_pipeline::RegisterMappingCache,
};

// ============================================================================
// 示例1：基础架构枚举和上下文
// ============================================================================

#[allow(dead_code)]
fn example1_architecture_basics() {
    println!("\n{}", "=".repeat(60));
    println!("示例1：基础架构枚举和上下文");
    println!("{}", "=".repeat(60));

    // 列出所有支持的架构
    println!("支持的架构:");
    println!("  - X86_64: {:?}", Architecture::X86_64);
    println!("  - ARM64:  {:?}", Architecture::ARM64);
    println!("  - RISC-V: {:?}", Architecture::RISCV64);

    // 创建编码上下文
    println!("\n创建编码上下文:");

    let ctx_x86 = EncodingContext::new(Architecture::X86_64);
    println!("  X86_64 上下文:");
    println!("    架构: {:?}", ctx_x86.architecture);
    println!("    地址大小: {} bits", ctx_x86.address_size);
    println!("    字节序: {:?}", ctx_x86.endianness);

    let ctx_arm = EncodingContext::new(Architecture::ARM64);
    println!("\n  ARM64 上下文:");
    println!("    架构: {:?}", ctx_arm.architecture);
    println!("    地址大小: {} bits", ctx_arm.address_size);
    println!("    字节序: {:?}", ctx_arm.endianness);

    let ctx_riscv = EncodingContext::new(Architecture::RISCV64);
    println!("\n  RISC-V 上下文:");
    println!("    架构: {:?}", ctx_riscv.architecture);
    println!("    地址大小: {} bits", ctx_riscv.address_size);
    println!("    字节序: {:?}", ctx_riscv.endianness);
}

// ============================================================================
// 示例2：寄存器映射
// ============================================================================

#[allow(dead_code)]
fn example2_register_mapping() {
    println!("\n{}", "=".repeat(60));
    println!("示例2：寄存器映射");
    println!("{}", "=".repeat(60));

    // 创建寄存器映射器
    let src_set = RegisterSet::new(Architecture::X86_64);
    let dst_set = RegisterSet::new(Architecture::RISCV64);
    let mut reg_mapper = RegisterMapper::new(src_set, dst_set, MappingStrategy::Direct);

    println!("X86_64 → RISC-V 寄存器映射:");
    println!("  {:<10} {:<10} {:<10}", "X86_64", "RISC-V", "说明");
    println!("  {}", "-".repeat(30));

    let x86_regs = vec![
        ("RAX", 0u16),
        ("RCX", 1),
        ("RDX", 2),
        ("RBX", 3),
        ("RSP", 4),
        ("RBP", 5),
        ("RSI", 6),
        ("RDI", 7),
    ];

    for (name, idx) in &x86_regs {
        let src_reg = RegId(*idx);
        match reg_mapper.map_register(src_reg) {
            Ok(dst_reg) => {
                println!(
                    "  {:<10} r{:>2}       r{:>2}       {:?}",
                    name, idx, dst_reg.0, name
                );
            }
            Err(e) => {
                println!("  {:<10} 映射失败: {:?}", name, e);
            }
        }
    }

    println!("\nARM64 → RISC-V 寄存器映射:");

    let src_set_arm = RegisterSet::new(Architecture::ARM64);
    let dst_set_riscv = RegisterSet::new(Architecture::RISCV64);
    let mut reg_mapper_arm =
        RegisterMapper::new(src_set_arm, dst_set_riscv, MappingStrategy::Direct);

    let arm_regs = vec![
        ("X0", 0u16),
        ("X1", 1),
        ("X2", 2),
        ("X3", 3),
        ("X4", 4),
        ("X5", 5),
        ("SP", 31),
        ("LR", 30),
    ];

    for (name, idx) in &arm_regs {
        let src_reg = RegId(*idx);
        match reg_mapper_arm.map_register(src_reg) {
            Ok(dst_reg) => {
                println!(
                    "  {:<10} x{:>2}       r{:>2}       {:?}",
                    name, idx, dst_reg.0, name
                );
            }
            Err(e) => {
                println!("  {:<10} 映射失败: {:?}", name, e);
            }
        }
    }
}

// ============================================================================
// 示例3：寄存器映射缓存
// ============================================================================

#[allow(dead_code)]
fn example3_register_mapping_cache() {
    println!("\n{}", "=".repeat(60));
    println!("示例3：寄存器映射缓存");
    println!("{}", "=".repeat(60));

    let mut cache = RegisterMappingCache::new();

    println!("测试寄存器映射缓存性能...");
    println!("映射 X86_64 → RISC-V");

    let iterations = 1000;
    let start = Instant::now();

    for i in 0..iterations {
        let src_reg = TranslationRegId::X86((i % 16) as u8);
        let _dst_reg = cache.map_or_compute(
            vm_cross_arch_support::encoding_cache::Arch::X86_64,
            vm_cross_arch_support::encoding_cache::Arch::Riscv64,
            src_reg,
        );
    }

    let elapsed = start.elapsed();
    let hit_rate = cache.hit_rate() * 100.0;

    println!("  执行次数: {}", iterations);
    println!("  总耗时: {:?}", elapsed);
    println!("  平均耗时: {:?} /次", elapsed / iterations);
    println!("  缓存命中率: {:.1}%", hit_rate);
}

// ============================================================================
// 示例4：内存访问分析
// ============================================================================

#[allow(dead_code)]
fn example4_memory_access_analysis() {
    println!("\n{}", "=".repeat(60));
    println!("示例4：内存访问分析");
    println!("{}", "=".repeat(60));

    let mut analyzer = MemoryAccessAnalyzer::new();

    println!("分析不同内存访问模式:");

    // 创建内存访问模式
    let pattern1 = MemoryAccessPattern::new(
        RegId(0), // 使用encoding模块的RegId (u16 wrapper)
        0x1000,
        AccessWidth::DoubleWord,
    )
    .with_access_type(AccessType::Read);

    let pattern2 = MemoryAccessPattern::new(
        RegId(1), // 未对齐地址
        0x1003,
        AccessWidth::DoubleWord,
    )
    .with_access_type(AccessType::Write)
    .with_alignment(Alignment::Unaligned);

    let pattern3 = MemoryAccessPattern::new(RegId(2), 0x1010, AccessWidth::Word)
        .with_access_type(AccessType::ReadWrite);

    println!("\n添加访问模式到分析器:");
    println!(
        "  模式1: addr=0x{:04x}, size={}, align={:?}, type={:?}",
        pattern1.offset,
        pattern1.size(),
        pattern1.alignment,
        pattern1.access_type
    );
    analyzer.add_pattern(pattern1);

    println!(
        "  模式2: addr=0x{:04x}, size={}, align={:?}, type={:?}",
        pattern2.offset,
        pattern2.size(),
        pattern2.alignment,
        pattern2.access_type
    );
    analyzer.add_pattern(pattern2);

    println!(
        "  模式3: addr=0x{:04x}, size={}, align={:?}, type={:?}",
        pattern3.offset,
        pattern3.size(),
        pattern3.alignment,
        pattern3.access_type
    );
    analyzer.add_pattern(pattern3);

    // 执行分析
    println!("\n执行分析...");
    let result = analyzer.analyze();

    println!("  总访问次数: {}", result.total_accesses);
    println!("  未对齐访问: {}", result.unaligned_accesses);
    println!("  原子访问: {}", result.atomic_accesses);
    println!("  向量访问: {}", result.vector_accesses);
}

// ============================================================================
// 示例5：内存访问优化
// ============================================================================

#[allow(dead_code)]
fn example5_memory_access_optimization() {
    println!("\n{}", "=".repeat(60));
    println!("示例5：内存访问优化");
    println!("{}", "=".repeat(60));

    let optimizer = DefaultMemoryAccessOptimizer::new(Architecture::X86_64);

    println!("优化不同内存访问模式:");

    // 创建几个内存访问模式进行优化
    let patterns = vec![
        MemoryAccessPattern::new(
            RegId(0), // RAX
            0x1000,
            AccessWidth::DoubleWord,
        )
        .with_access_type(AccessType::Read),
        MemoryAccessPattern::new(
            RegId(1), // RCX
            0x1008,
            AccessWidth::Word,
        )
        .with_access_type(AccessType::Write),
        MemoryAccessPattern::new(
            RegId(2), // RDX (未对齐地址)
            0x1003,
            AccessWidth::DoubleWord,
        )
        .with_access_type(AccessType::Read)
        .with_alignment(Alignment::Unaligned),
    ];

    println!("\n原始访问模式:");
    for (i, pattern) in patterns.iter().enumerate() {
        println!(
            "  {} addr=0x{:04x}, size={}, type={:?}, align={:?}",
            i + 1,
            pattern.offset,
            pattern.size(),
            pattern.access_type,
            pattern.alignment
        );
    }

    // 分析并优化每个模式
    println!("\n优化结果:");
    for (i, pattern) in patterns.iter().enumerate() {
        let start = Instant::now();
        let optimized = optimizer.optimize_access_pattern(pattern);
        let elapsed = start.elapsed();

        println!("  模式{}:", i + 1);
        println!(
            "    原始: addr=0x{:04x}, size={}, align={:?}",
            pattern.offset,
            pattern.size(),
            pattern.alignment
        );
        println!(
            "    优化: addr=0x{:04x}, size={}, align={:?}",
            optimized.optimized.offset,
            optimized.optimized.size(),
            optimized.optimized.alignment
        );
        println!("    优化类型: {:?}", optimized.optimization_type);
        println!("    性能提升: {:.1}%", optimized.performance_gain * 100.0);
        println!("    优化耗时: {:?}", elapsed);
    }
}

// ============================================================================
// 示例6：架构对比分析
// ============================================================================

#[allow(dead_code)]
fn example6_architecture_comparison() {
    println!("\n{}", "=".repeat(60));
    println!("示例6：架构特性对比");
    println!("{}", "=".repeat(60));

    println!("架构特性对比:\n");

    let architectures = vec![
        ("X86_64", Architecture::X86_64),
        ("ARM64", Architecture::ARM64),
        ("RISC-V", Architecture::RISCV64),
    ];

    println!(
        "  {:<10} {:<15} {:<15} {:<15}",
        "架构", "寄存器数", "寻址模式", "特色特性"
    );
    println!("  {}", "-".repeat(55));

    for (name, arch) in &architectures {
        let ctx = EncodingContext::new(*arch);

        let (reg_count, addr_modes, features) = match arch {
            Architecture::X86_64 => (
                "16 (GPR)",
                "复杂 (寻址模式丰富)",
                "SIMD (AVX/AVX2/AVX-512), 向量寄存器",
            ),
            Architecture::ARM64 => (
                "31 (X0-X30) + SP",
                "灵活 (基址+偏移)",
                "NEON, SVE (可变长向量), 条件执行",
            ),
            Architecture::RISCV64 => (
                "31 (x1-x31) + x0",
                "简洁 (Load/Store架构)",
                "模块化扩展 (M/A/F/D/C/V), 指令格式统一",
            ),
        };

        println!(
            "  {:<10} {:<15} {:<15} {:<15}",
            name, reg_count, addr_modes, features
        );
    }

    println!("\n性能考虑因素:");
    println!("  X86_64:");
    println!("    - 优势: CISC, 指令密度高, 成熟的工具链");
    println!("    - 劣势: 指令复杂, 解码开销大");
    println!("\n  ARM64:");
    println!("    - 优势: RISC, 指令简单, 能效高");
    println!("    - 劣势: 指令密度相对较低");
    println!("\n  RISC-V:");
    println!("    - 优势: 开放, 模块化, 可扩展性强");
    println!("    - 劣势: 生态系统相对较新");
}

// ============================================================================
// 示例7：完整翻译演示
// ============================================================================

#[allow(dead_code)]
fn example7_full_translation_demo() {
    println!("\n{}", "=".repeat(60));
    println!("示例7：完整跨架构翻译演示");
    println!("{}", "=".repeat(60));

    println!("演示 X86_64 → ARM64 翻译流程\n");

    // 创建寄存器映射缓存
    let mut reg_cache = RegisterMappingCache::new();

    println!("步骤1: 寄存器映射");
    println!("  源寄存器: RAX (X86_64 GPR #0)");
    let src_reg = TranslationRegId::X86(0);
    println!("  映射到: {:?}", src_reg);

    let dst_reg = reg_cache.map_or_compute(
        vm_cross_arch_support::encoding_cache::Arch::X86_64,
        vm_cross_arch_support::encoding_cache::Arch::ARM64,
        src_reg,
    );
    println!("  目标寄存器: {:?} (ARM64)", dst_reg);

    println!("\n步骤2: 内存访问分析");
    let mut analyzer = MemoryAccessAnalyzer::new();

    let pattern = MemoryAccessPattern::new(RegId(0), 0x1000, AccessWidth::DoubleWord)
        .with_access_type(AccessType::Read);

    println!("  地址: 0x{:04x}", pattern.offset);
    println!("  大小: {} 字节", pattern.size());
    println!("  访问类型: {:?}", pattern.access_type);
    println!("  对齐状态: {:?}", pattern.alignment);

    analyzer.add_pattern(pattern);

    let result = analyzer.analyze();
    println!(
        "  分析结果: 总访问数={}, 未对齐={}",
        result.total_accesses, result.unaligned_accesses
    );

    println!("\n步骤3: 内存访问优化");
    let optimizer = DefaultMemoryAccessOptimizer::new(Architecture::X86_64);

    let pattern_to_optimize = MemoryAccessPattern::new(RegId(0), 0x1000, AccessWidth::DoubleWord)
        .with_access_type(AccessType::Read)
        .with_alignment(Alignment::Unaligned);

    let optimized = optimizer.optimize_access_pattern(&pattern_to_optimize);

    println!(
        "  原始模式: addr=0x{:04x}, align={:?}",
        pattern_to_optimize.offset, pattern_to_optimize.alignment
    );
    println!(
        "  优化模式: addr=0x{:04x}, align={:?}",
        optimized.optimized.offset, optimized.optimized.alignment
    );
    println!("  优化类型: {:?}", optimized.optimization_type);
    println!("  性能提升: {:.1}%", optimized.performance_gain * 100.0);

    println!("\n步骤4: 翻译统计");
    println!(
        "  寄存器映射缓存命中率: {:.1}%",
        reg_cache.hit_rate() * 100.0
    );
}

// ============================================================================
// 主函数
// ============================================================================

fn main() {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║       跨架构执行示例 - 虚拟机跨架构翻译演示               ║");
    println!("╚════════════════════════════════════════════════════════════╝");

    // 运行所有示例
    example1_architecture_basics();
    example2_register_mapping();
    example3_register_mapping_cache();
    example4_memory_access_analysis();
    example5_memory_access_optimization();
    example6_architecture_comparison();
    example7_full_translation_demo();

    println!("\n{}", "=".repeat(60));
    println!("所有示例运行完成！");
    println!("{}", "=".repeat(60));

    println!("\n关键要点:");
    println!("  ✓ 支持X86_64、ARM64、RISC-V64三种架构");
    println!("  ✓ 自动寄存器映射和缓存");
    println!("  ✓ 内存访问分析和优化");
    println!("  ✓ 完整的跨架构翻译管线");
    println!("  ✓ 高效的缓存机制");
}
