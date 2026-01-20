//! 热路径优化器使用示例
//!
//! 本示例演示如何使用热路径优化器进行各种优化

use crate::jit::hot_path_optimizer::{
    HotPathOptimizer, HotPathOptimizerConfig,
};
use vm_ir::{IRBlock, IROp, Terminator};
use vm_core::GuestAddr;

/// 创建一个包含循环的IR块示例
fn create_loop_block() -> IRBlock {
    IRBlock {
        start_pc: GuestAddr(0x1000),
        ops: vec![
            // 循环初始化
            IROp::MovImm { dst: 1, imm: 0 },       // i = 0
            IROp::MovImm { dst: 2, imm: 100 },     // n = 100
            // 循环体
            IROp::Add { dst: 1, src1: 1, src2: 2 }, // i = i + n
            IROp::MovImm { dst: 3, imm: 1 },
            IROp::Add { dst: 1, src1: 1, src2: 3 }, // i++
            IROp::MovImm { dst: 4, imm: 10 },
        ],
        term: Terminator::Jmp {
            target: GuestAddr(0x1000), // 跳回循环开始
        },
    }
}

/// 创建一个包含内存访问的IR块示例
fn create_memory_access_block() -> IRBlock {
    IRBlock {
        start_pc: GuestAddr(0x2000),
        ops: vec![
            IROp::MovImm { dst: 1, imm: 0x1000 }, // 基地址
            IROp::Load {
                dst: 2,
                base: 1,
                offset: 0,
                size: 8,
                flags: vm_ir::MemFlags::default(),
            },
            IROp::Load {
                dst: 3,
                base: 1,
                offset: 8,
                size: 8,
                flags: vm_ir::MemFlags::default(),
            },
            IROp::Load {
                dst: 4,
                base: 1,
                offset: 16,
                size: 8,
                flags: vm_ir::MemFlags::default(),
            },
        ],
        term: Terminator::Jmp {
            target: GuestAddr(0x2030),
        },
    }
}

/// 基本使用示例
pub fn basic_example() {
    println!("=== 热路径优化器基本使用示例 ===\n");

    // 1. 创建优化器
    let mut optimizer = HotPathOptimizer::new();

    // 2. 创建测试IR块
    let block = create_loop_block();
    println!("原始IR块指令数: {}", block.ops.len());

    // 3. 执行优化
    match optimizer.optimize(&block) {
        Ok(optimized_block) => {
            println!("优化后IR块指令数: {}", optimized_block.ops.len());

            // 4. 获取优化统计
            let stats = optimizer.get_stats();
            println!("\n优化统计:");
            println!("  原始指令数: {}", stats.original_insn_count);
            println!("  优化后指令数: {}", stats.optimized_insn_count);
            println!("  循环展开次数: {}", stats.loop_unrolling_count);
            println!("  函数内联次数: {}", stats.function_inlining_count);
            println!("  内存优化次数: {}", stats.memory_optimization_count);
            println!("  总优化时间: {} ns", stats.total_optimization_time_ns);
        }
        Err(e) => {
            eprintln!("优化失败: {:?}", e);
        }
    }
}

/// 自定义配置示例
pub fn custom_config_example() {
    println!("\n=== 自定义配置示例 ===\n");

    // 1. 创建自定义配置
    let config = HotPathOptimizerConfig {
        loop_unroll_factor: 8,           // 展开因子为8
        max_loop_body_size: 100,         // 最大循环体100条指令
        inline_size_threshold: 50,       // 内联阈值50条指令
        max_inline_depth: 5,             // 最大内联深度5
        enable_memory_optimization: true,
        enable_prefetch: true,
        prefetch_distance: 8,            // 预取距离8
        max_code_bloat_factor: 4.0,      // 最大代码膨胀4倍
    };

    // 2. 使用自定义配置创建优化器
    let mut optimizer = HotPathOptimizer::with_config(config);

    println!("使用自定义配置:");
    println!("  循环展开因子: {}", optimizer.get_config().loop_unroll_factor);
    println!("  内联大小阈值: {}", optimizer.get_config().inline_size_threshold);

    // 3. 执行优化
    let block = create_memory_access_block();
    match optimizer.optimize(&block) {
        Ok(_) => {
            println!("\n优化完成!");
            let stats = optimizer.get_stats();
            println!("  预取插入次数: {}", stats.prefetch_insertion_count);
            println!("  冗余访问消除: {}", stats.redundant_access_elimination);
        }
        Err(e) => {
            eprintln!("优化失败: {:?}", e);
        }
    }
}

/// 动态配置更新示例
pub fn config_update_example() {
    println!("\n=== 动态配置更新示例 ===\n");

    let mut optimizer = HotPathOptimizer::new();

    // 初始配置
    println!("初始配置:");
    println!("  循环展开因子: {}", optimizer.get_config().loop_unroll_factor);

    // 更新配置
    let new_config = HotPathOptimizerConfig {
        loop_unroll_factor: 2,
        ..Default::default()
    };
    optimizer.update_config(new_config);

    println!("更新后配置:");
    println!("  循环展开因子: {}", optimizer.get_config().loop_unroll_factor);
}

/// 多次优化示例
pub fn multiple_optimization_example() {
    println!("\n=== 多次优化示例 ===\n");

    let mut optimizer = HotPathOptimizer::new();

    // 优化多个块
    let blocks = vec![create_loop_block(), create_memory_access_block()];

    for (i, block) in blocks.iter().enumerate() {
        println!("优化块 #{}", i + 1);
        match optimizer.optimize(block) {
            Ok(_) => {
                let stats = optimizer.get_stats();
                println!("  成功! 累计循环展开: {}", stats.loop_unrolling_count);
            }
            Err(e) => {
                eprintln!("  失败: {:?}", e);
            }
        }
    }

    // 显示总体统计
    let final_stats = optimizer.get_stats();
    println!("\n总体统计:");
    println!("  总优化次数: {}", final_stats.loop_unrolling_count + final_stats.function_inlining_count);
    println!("  总优化时间: {} ns", final_stats.total_optimization_time_ns);
}

/// 性能分析示例
pub fn performance_analysis_example() {
    println!("\n=== 性能分析示例 ===\n");

    let mut optimizer = HotPathOptimizer::new();
    let block = create_loop_block();

    // 执行优化并测量时间
    let start = std::time::Instant::now();
    let result = optimizer.optimize(&block);
    let duration = start.elapsed();

    match result {
        Ok(optimized_block) => {
            println!("优化成功!");
            println!("  优化时间: {:?}", duration);
            println!("  指令数变化: {} -> {}", block.ops.len(), optimized_block.ops.len());

            let stats = optimizer.get_stats();
            if stats.optimized_insn_count > 0 {
                let speedup = block.ops.len() as f64 / optimized_block.ops.len() as f64;
                println!("  理论加速比: {:.2}x", speedup);
            }
        }
        Err(e) => {
            eprintln!("优化失败: {:?}", e);
        }
    }
}

/// 统计重置示例
pub fn stats_reset_example() {
    println!("\n=== 统计重置示例 ===\n");

    let mut optimizer = HotPathOptimizer::new();

    // 第一次优化
    let block = create_loop_block();
    optimizer.optimize(&block).unwrap();
    println!("第一次优化后的统计:");
    println!("  循环展开次数: {}", optimizer.get_stats().loop_unrolling_count);

    // 重置统计
    optimizer.reset_stats();
    println!("\n重置后的统计:");
    println!("  循环展开次数: {}", optimizer.get_stats().loop_unrolling_count);

    // 第二次优化
    optimizer.optimize(&block).unwrap();
    println!("\n第二次优化后的统计:");
    println!("  循环展开次数: {}", optimizer.get_stats().loop_unrolling_count);
}

/// 主函数示例
#[allow(dead_code)]
pub fn main() {
    basic_example();
    custom_config_example();
    config_update_example();
    multiple_optimization_example();
    performance_analysis_example();
    stats_reset_example();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_example() {
        basic_example();
    }

    #[test]
    fn test_custom_config() {
        custom_config_example();
    }

    #[test]
    fn test_config_update() {
        config_update_example();
    }

    #[test]
    fn test_multiple_optimizations() {
        multiple_optimization_example();
    }

    #[test]
    fn test_performance_analysis() {
        performance_analysis_example();
    }

    #[test]
    fn test_stats_reset() {
        stats_reset_example();
    }
}
