//! JIT块链接优化使用示例
//!
//! 本示例演示如何使用BlockChainer来优化JIT编译的块链接，
//! 减少间接跳转开销，提升性能。

use vm_core::GuestAddr;
use vm_engine_jit::block_chaining::{BlockChainer, ChainType};
use vm_ir::{IRBlock, IRBuilder, Terminator};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== JIT块链接优化示例 ===\n");

    // 创建块链接器
    let mut chainer = BlockChainer::with_config(16, true);
    println!("✅ 创建BlockChainer (最大链长度: 16, 热路径优化: 启用)\n");

    // 示例1: 创建线性执行的块序列
    println!("📦 示例1: 线性执行块序列");
    let blocks = create_linear_blocks();

    // 分析所有块
    for block in &blocks {
        chainer.analyze_block(block)?;
    }

    // 构建链
    chainer.build_chains();

    // 显示统计信息
    let stats = chainer.stats();
    println!("  总链接数: {}", stats.total_links);
    println!("  总链数: {}", stats.total_chains);
    println!("  总块数: {}", stats.total_blocks);
    println!("  平均链长度: {:.2}", stats.avg_chain_length);
    println!();

    // 获取并显示块链
    if let Some(chain) = chainer.get_chain(GuestAddr(0x1000)) {
        println!("  🔗 块链 (从0x1000):");
        for (i, addr) in chain.blocks.iter().enumerate() {
            println!("    {}: 0x{:x}", i + 1, addr);
        }
        println!("  总频率: {}", chain.frequency);
    }

    println!("\n📦 示例2: 条件分支块");
    let mut chainer2 = BlockChainer::new();
    let cond_blocks = create_conditional_blocks();

    for block in &cond_blocks {
        chainer2.analyze_block(block)?;
    }

    chainer2.build_chains();

    // 显示条件分支的链接信息
    println!("  条件分支链接:");
    for (from, to, link_type) in [
        (GuestAddr(0x1000), GuestAddr(0x2000)),
        (GuestAddr(0x1000), GuestAddr(0x3000)),
    ] {
        if let Some(link) = chainer2.get_link(from, to) {
            println!("    0x{:x} -> 0x{:x} ({:?})", from, to, link.link_type);
        }
    }

    println!("\n📦 示例3: 热路径优化");
    let mut chainer3 = BlockChainer::with_config(16, true);

    // 模拟多次执行块以增加频率
    let hot_block = create_hot_path_block();
    for _ in 0..10 {
        chainer3.analyze_block(&hot_block)?;
    }

    chainer3.build_chains();

    let stats3 = chainer3.stats();
    println!("  热路径块频率: {}", stats3.total_blocks);
    if let Some(chain) = chainer3.get_chain(GuestAddr(0x1000)) {
        println!("  热路径频率: {}", chain.frequency);
    }

    println!("\n=== 总结 ===");
    println!("✅ 块链接优化功能:");
    println!("  1. 识别可链接的连续块");
    println!("  2. 优化热路径（高频率块优先）");
    println!("  3. 减少间接跳转开销");
    println!("  4. 预期性能提升: 10-15%");

    println!("\n📚 详细文档:");
    println!("  - docs/BLOCK_CHAINING_IMPLEMENTATION.md");
    println!("  - docs/TODO_AUDIT.md");

    Ok(())
}

/// 创建线性执行的块序列
fn create_linear_blocks() -> Vec<IRBlock> {
    let mut blocks = Vec::new();

    // Block 1: 0x1000 -> 0x2000
    let mut builder1 = IRBuilder::new(GuestAddr(0x1000));
    // 添加一些操作...
    builder1.set_term(Terminator::Jmp {
        target: GuestAddr(0x2000),
    });
    blocks.push(builder1.build());

    // Block 2: 0x2000 -> 0x3000
    let mut builder2 = IRBuilder::new(GuestAddr(0x2000));
    // 添加一些操作...
    builder2.set_term(Terminator::Jmp {
        target: GuestAddr(0x3000),
    });
    blocks.push(builder2.build());

    // Block 3: 0x3000 -> 0x4000
    let mut builder3 = IRBuilder::new(GuestAddr(0x3000));
    // 添加一些操作...
    builder3.set_term(Terminator::Jmp {
        target: GuestAddr(0x4000),
    });
    blocks.push(builder3.build());

    // Block 4: 0x4000 (return)
    let mut builder4 = IRBuilder::new(GuestAddr(0x4000));
    builder4.set_term(Terminator::Ret);
    blocks.push(builder4.build());

    blocks
}

/// 创建条件分支块
fn create_conditional_blocks() -> Vec<IRBlock> {
    let mut blocks = Vec::new();

    // Block 1: 条件分支 0x1000 -> (0x2000, 0x3000)
    let mut builder1 = IRBuilder::new(GuestAddr(0x1000));
    builder1.set_term(Terminator::CondJmp {
        cond: 1,
        target_true: GuestAddr(0x2000),
        target_false: GuestAddr(0x3000),
    });
    blocks.push(builder1.build());

    // Block 2: True分支 0x2000 -> return
    let mut builder2 = IRBuilder::new(GuestAddr(0x2000));
    builder2.set_term(Terminator::Ret);
    blocks.push(builder2.build());

    // Block 3: False分支 0x3000 -> return
    let mut builder3 = IRBuilder::new(GuestAddr(0x3000));
    builder3.set_term(Terminator::Ret);
    blocks.push(builder3.build());

    blocks
}

/// 创建热路径块（会被多次执行）
fn create_hot_path_block() -> IRBlock {
    let mut builder = IRBuilder::new(GuestAddr(0x1000));
    builder.set_term(Terminator::Jmp {
        target: GuestAddr(0x2000),
    });
    builder.build()
}
