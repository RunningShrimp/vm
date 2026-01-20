//! JIT优化示例
//!
//! 本示例演示JIT编译器的各种优化特性，包括：
//! - 基础JIT编译
//! - 优化级别对比
//! - 热点检测
//! - 代码缓存
//! - 编译统计
//!
//! 运行方式：
//! ```bash
//! cargo run --example jit_optimization
//! ```

use std::sync::Arc;
use std::time::Instant;

use parking_lot::Mutex;
use vm_core::{GuestAddr, MMU, VmError};
use vm_engine::jit::core::{JITConfig, JITEngine};
use vm_ir::{IRBlock, IROp, Terminator};

// 简单的MMU实现用于测试
struct SimpleMMU {
    memory: Vec<u8>,
}

impl SimpleMMU {
    fn new(size: usize) -> Self {
        Self {
            memory: vec![0; size],
        }
    }
}

impl MMU for SimpleMMU {
    fn translate(&mut self, addr: GuestAddr, len: usize) -> Result<(GuestAddr, usize), VmError> {
        if addr.0 as usize + len <= self.memory.len() {
            Ok((addr, len))
        } else {
            Err(VmError::Core(vm_core::CoreError::InvalidState {
                message: "Out of memory bounds".to_string(),
                current: "".to_string(),
                expected: "".to_string(),
            }))
        }
    }

    fn read(&mut self, addr: GuestAddr, len: usize) -> Result<Vec<u8>, VmError> {
        let (translated, _) = self.translate(addr, len)?;
        let start = translated.0 as usize;
        let end = start + len;
        Ok(self.memory[start..end].to_vec())
    }

    fn write(&mut self, addr: GuestAddr, data: &[u8]) -> Result<(), VmError> {
        let (translated, _) = self.translate(addr, data.len())?;
        let start = translated.0 as usize;
        let end = start + data.len();
        self.memory[start..end].copy_from_slice(data);
        Ok(())
    }

    fn flush_tlb(&mut self) -> Result<(), VmError> {
        Ok(())
    }

    fn tlb_lookup(&mut self, _addr: GuestAddr) -> Option<Result<(GuestAddr, usize), VmError>> {
        None
    }
}

/// 创建测试IR块
fn create_test_block(base_pc: GuestAddr, instruction_count: usize) -> IRBlock {
    let mut ops = Vec::new();

    // 创建一系列算术运算指令
    for i in 0..instruction_count {
        ops.push(IROp::MovImm {
            dst: (i * 3) as u32,
            imm: (i * 2) as i64,
        });
        ops.push(IROp::MovImm {
            dst: (i * 3 + 1) as u32,
            imm: (i + 1) as i64,
        });
        ops.push(IROp::Add {
            dst: (i * 3 + 2) as u32,
            src1: (i * 3) as u32,
            src2: (i * 3 + 1) as u32,
        });
    }

    IRBlock {
        start_pc: base_pc,
        ops,
        term: Terminator::Jmp {
            target: base_pc + 100,
        },
    }
}

// ============================================================================
// 示例1：基础JIT编译
// ============================================================================

#[allow(dead_code)]
fn example1_basic_jit_compilation() {
    println!("\n{'=':=<60}");
    println!("示例1：基础JIT编译");
    println!("{}", "=".repeat(60));

    // 创建默认配置的JIT引擎
    let config = JITConfig::default();
    let mut jit_engine = JITEngine::new(config);

    // 创建测试IR块
    let block = create_test_block(GuestAddr(0x1000), 10);

    println!("原始IR块指令数量: {}", block.ops.len());

    // 编译IR块
    let start = Instant::now();
    match jit_engine.compile(&block) {
        Ok(result) => {
            let compile_time = start.elapsed();
            println!("✅ 编译成功！");
            println!("   编译耗时: {:?}", compile_time);
            println!("   生成代码大小: {} bytes", result.code_size);
            println!("   入口点: {:?}", result.entry_point);

            // 显示编译统计
            let stats = result.stats;
            println!("\n编译统计:");
            println!("   原始指令数: {}", stats.original_insn_count);
            println!("   优化后指令数: {}", stats.optimized_insn_count);
            println!("   机器码指令数: {}", stats.machine_insn_count);
            println!("   编译总耗时: {} ns", stats.compilation_time_ns);
            println!("   优化耗时: {} ns", stats.optimization_time_ns);
            println!(
                "   寄存器分配耗时: {} ns",
                stats.register_allocation_time_ns
            );
            println!("   代码生成耗时: {} ns", stats.code_generation_time_ns);
        }
        Err(e) => {
            println!("❌ 编译失败: {:?}", e);
        }
    }
}

// ============================================================================
// 示例2：优化级别对比
// ============================================================================

#[allow(dead_code)]
fn example2_optimization_levels() {
    println!("\n{'=':=<60}");
    println!("示例2：优化级别对比");
    println!("{}", "=".repeat(60));

    let test_blocks = vec![
        (
            "小代码块 (10条指令)",
            create_test_block(GuestAddr(0x1000), 10),
        ),
        (
            "中等代码块 (50条指令)",
            create_test_block(GuestAddr(0x2000), 50),
        ),
        (
            "大代码块 (100条指令)",
            create_test_block(GuestAddr(0x3000), 100),
        ),
    ];

    let optimization_levels = vec![
        ("O0 (无优化)", 0u8),
        ("O1 (基本优化)", 1u8),
        ("O2 (标准优化)", 2u8),
        ("O3 (高级优化)", 3u8),
    ];

    for (block_name, block) in &test_blocks {
        println!("\n{}:", block_name);
        println!("原始指令数: {}", block.ops.len());

        for (level_name, opt_level) in &optimization_levels {
            let mut config = JITConfig::default();
            config.optimization_level = *opt_level;
            config.enable_optimization = *opt_level > 0;

            let mut jit_engine = JITEngine::new(config);

            let start = Instant::now();
            match jit_engine.compile(block) {
                Ok(result) => {
                    let compile_time = start.elapsed();
                    let stats = result.stats;

                    println!(
                        "  {}: 编译时间 {:>8.2?}, 优化后指令数: {:>4}, 代码大小: {:>5} bytes",
                        level_name, compile_time, stats.optimized_insn_count, result.code_size
                    );
                }
                Err(e) => {
                    println!("  {}: ❌ 编译失败: {:?}", level_name, e);
                }
            }
        }
    }
}

// ============================================================================
// 示例3：热点检测和缓存
// ============================================================================

#[allow(dead_code)]
fn example3_hotspot_detection_and_cache() {
    println!("\n{'=':=<60}");
    println!("示例3：热点检测和缓存");
    println!("{}", "=".repeat(60));

    let mut config = JITConfig::default();
    config.hotspot_threshold = 5; // 设置较低的热点阈值用于演示

    let mut jit_engine = JITEngine::new(config);
    let mut mmu = Box::new(SimpleMMU::new(1024 * 1024));

    let test_pc = GuestAddr(0x1000);

    println!("热点阈值: {}", config.hotspot_threshold);
    println!("\n模拟执行过程:");

    // 模拟多次执行同一地址
    for i in 1..=10 {
        // 更新热点计数
        jit_engine.update_hotspot_counter(test_pc);

        // 检查是否是热点
        let is_hotspot = jit_engine.is_hotspot(test_pc);

        // 检查缓存状态
        let cache_stats = jit_engine.get_cache_stats();

        println!(
            "执行 #{:2}: 热点={}, 缓存条目数={}",
            i, is_hotspot, cache_stats.size
        );

        // 尝试执行（热点会触发编译）
        let result = jit_engine.execute(&mut mmu, test_pc);
        if i == config.hotspot_threshold as usize {
            println!("        → 热点达到阈值，触发JIT编译");
            if let Ok(stats) = result {
                if stats.stats.jit_compiles > 0 {
                    println!("        → JIT编译完成");
                }
            }
        }
    }

    // 显示最终统计
    println!("\n最终缓存统计:");
    let cache_stats = jit_engine.get_cache_stats();
    println!(
        "  缓存大小: {} / {} bytes",
        cache_stats.size, cache_stats.capacity
    );
    println!(
        "  缓存命中率: {:.1}%",
        (cache_stats.hits as f64 / (cache_stats.hits + cache_stats.misses) as f64) * 100.0
    );
}

// ============================================================================
// 示例4：并行编译性能
// ============================================================================

#[allow(dead_code)]
fn example4_parallel_compilation() {
    println!("\n{'=':=<60}");
    println!("示例4：并行编译性能对比");
    println!("{}", "=".repeat(60));

    let blocks: Vec<_> = (0..10)
        .map(|i| create_test_block(GuestAddr(0x1000 + i as u64 * 0x100), 50))
        .collect();

    // 测试顺序编译
    let mut config_sequential = JITConfig::default();
    config_sequential.enable_parallel_compilation = false;
    let mut jit_sequential = JITEngine::new(config_sequential);

    let start = Instant::now();
    let mut seq_success = 0;
    for block in &blocks {
        if jit_sequential.compile(block).is_ok() {
            seq_success += 1;
        }
    }
    let sequential_time = start.elapsed();

    println!("顺序编译:");
    println!("  成功编译: {} / {} 个代码块", seq_success, blocks.len());
    println!("  总耗时: {:?}", sequential_time);

    // 测试并行编译
    let mut config_parallel = JITConfig::default();
    config_parallel.enable_parallel_compilation = true;
    config_parallel.parallel_compilation_threads = 4;
    let mut jit_parallel = JITEngine::new(config_parallel);

    let start = Instant::now();
    let mut par_success = 0;
    for block in &blocks {
        if jit_parallel.compile(block).is_ok() {
            par_success += 1;
        }
    }
    let parallel_time = start.elapsed();

    println!("\n并行编译 (4线程):");
    println!("  成功编译: {} / {} 个代码块", par_success, blocks.len());
    println!("  总耗时: {:?}", parallel_time);

    if sequential_time > parallel_time {
        let speedup = sequential_time.as_nanos() as f64 / parallel_time.as_nanos() as f64;
        println!("  加速比: {:.2}x", speedup);
    }
}

// ============================================================================
// 示例5：优化策略对比
// ============================================================================

#[allow(dead_code)]
fn example5_optimization_strategies() {
    println!("\n{'=':=<60}");
    println!("示例5：优化策略对比");
    println!("{}", "=".repeat(60));

    let block = create_test_block(GuestAddr(0x1000), 50);

    struct StrategyConfig {
        name: &'static str,
        enable_optimization: bool,
        enable_simd: bool,
        optimization_level: u8,
    }

    let strategies = vec![
        StrategyConfig {
            name: "无优化",
            enable_optimization: false,
            enable_simd: false,
            optimization_level: 0,
        },
        StrategyConfig {
            name: "仅常量折叠",
            enable_optimization: true,
            enable_simd: false,
            optimization_level: 1,
        },
        StrategyConfig {
            name: "标准优化 + SIMD",
            enable_optimization: true,
            enable_simd: true,
            optimization_level: 2,
        },
        StrategyConfig {
            name: "激进优化 + SIMD",
            enable_optimization: true,
            enable_simd: true,
            optimization_level: 3,
        },
    ];

    println!("原始指令数: {}\n", block.ops.len());

    for strategy in strategies {
        let mut config = JITConfig::default();
        config.enable_optimization = strategy.enable_optimization;
        config.enable_simd = strategy.enable_simd;
        config.optimization_level = strategy.optimization_level;

        let mut jit_engine = JITEngine::new(config);

        let start = Instant::now();
        match jit_engine.compile(&block) {
            Ok(result) => {
                let compile_time = start.elapsed();
                let stats = result.stats;

                println!("{}:", strategy.name);
                println!("  编译时间: {:?}", compile_time);
                println!(
                    "  优化后指令数: {} (减少: {:.1}%)",
                    stats.optimized_insn_count,
                    ((stats.original_insn_count - stats.optimized_insn_count) as f64
                        / stats.original_insn_count as f64)
                        * 100.0
                );
                println!("  代码大小: {} bytes", result.code_size);
                println!("  优化耗时: {} ns", stats.optimization_time_ns);
                println!("  总耗时: {} ns", stats.compilation_time_ns);
                println!();
            }
            Err(e) => {
                println!("{}: ❌ 编译失败: {:?}\n", strategy.name, e);
            }
        }
    }
}

// ============================================================================
// 示例6：完整编译流程演示
// ============================================================================

#[allow(dead_code)]
fn example6_full_compilation_pipeline() {
    println!("\n{'=':=<60}");
    println!("示例6：完整JIT编译流程");
    println!("{}", "=".repeat(60));

    // 创建优化的JIT配置
    let mut config = JITConfig::default();
    config.optimization_level = 2;
    config.enable_optimization = true;
    config.enable_simd = true;
    config.hotspot_threshold = 10;

    println!("JIT配置:");
    println!("  优化级别: O{}", config.optimization_level);
    println!("  启用优化: {}", config.enable_optimization);
    println!("  启用SIMD: {}", config.enable_simd);
    println!("  热点阈值: {}", config.hotspot_threshold);
    println!("  并行编译: {}", config.enable_parallel_compilation);
    println!("  代码缓存限制: {} bytes", config.code_cache_size_limit);

    // 创建JIT引擎
    let mut jit_engine = JITEngine::new(config);

    // 创建多个测试代码块
    println!("\n创建测试代码块...");
    let blocks: Vec<_> = (0..5)
        .map(|i| {
            let pc = GuestAddr(0x1000 + i as u64 * 0x1000);
            (pc, create_test_block(pc, 30))
        })
        .collect();

    println!("创建了 {} 个代码块", blocks.len());

    // 编译所有代码块
    println!("\n开始编译...");
    let mut compiled = 0;
    let mut failed = 0;

    for (pc, block) in &blocks {
        print!("  编译地址 {:?}... ", pc);
        let start = Instant::now();

        match jit_engine.compile(block) {
            Ok(result) => {
                let time = start.elapsed();
                println!("✅ ({:?}, {} bytes)", time, result.code_size);
                compiled += 1;
            }
            Err(e) => {
                println!("❌ {:?}", e);
                failed += 1;
            }
        }
    }

    println!("\n编译结果:");
    println!("  成功: {} / {}", compiled, blocks.len());
    println!("  失败: {} / {}", failed, blocks.len());

    // 显示总体统计
    println!("\n总体编译统计:");
    let stats = jit_engine.get_compilation_stats();
    println!("  原始指令总数: {}", stats.original_insn_count);
    println!("  优化后指令总数: {}", stats.optimized_insn_count);
    println!("  机器码指令总数: {}", stats.machine_insn_count);
    println!("  总编译时间: {} ms", stats.compilation_time_ns / 1_000_000);
    println!(
        "  总优化时间: {} ms",
        stats.optimization_time_ns / 1_000_000
    );
    println!(
        "  总寄存器分配时间: {} ms",
        stats.register_allocation_time_ns / 1_000_000
    );
    println!(
        "  总代码生成时间: {} ms",
        stats.code_generation_time_ns / 1_000_000
    );

    // 显示缓存统计
    println!("\n代码缓存统计:");
    let cache_stats = jit_engine.get_cache_stats();
    println!(
        "  缓存条目数: {} / {}",
        cache_stats.size, cache_stats.capacity
    );
    println!("  缓存命中次数: {}", cache_stats.hits);
    println!("  缓存未命中次数: {}", cache_stats.misses);
    println!(
        "  命中率: {:.1}%",
        (cache_stats.hits as f64 / (cache_stats.hits + cache_stats.misses) as f64) * 100.0
    );

    // 模拟执行
    println!("\n模拟执行...");
    let mut mmu = Box::new(SimpleMMU::new(1024 * 1024));

    for (pc, _) in &blocks {
        // 多次执行以触发热点编译
        for _ in 0..15 {
            jit_engine.update_hotspot_counter(*pc);
            let _ = jit_engine.execute(&mut mmu, *pc);
        }
    }

    let final_cache_stats = jit_engine.get_cache_stats();
    println!("  执行后缓存条目数: {}", final_cache_stats.size);
}

// ============================================================================
// 主函数
// ============================================================================

fn main() {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║         JIT优化示例 - 虚拟机JIT编译器特性演示             ║");
    println!("╚════════════════════════════════════════════════════════════╝");

    // 运行所有示例
    example1_basic_jit_compilation();
    example2_optimization_levels();
    example3_hotspot_detection_and_cache();
    example4_parallel_compilation();
    example5_optimization_strategies();
    example6_full_compilation_pipeline();

    println!("\n{'=':=<60}");
    println!("所有示例运行完成！");
    println!("{}", "=".repeat(60));
}
