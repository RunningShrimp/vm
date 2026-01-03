//! vm-engine JIT完整功能示例
//!
//! 本示例展示如何使用 `jit-full` feature 访问 vm-engine-jit 的高级功能
//!
//! 运行方式:
//! ```bash
//! cargo run --example jit_full_example --features jit-full
//! ```

#[cfg(feature = "jit-full")]
use vm_engine::{
    BlockChainer,
    // vm-engine-jit高级功能 (直接导入类型)
    CompileCache,
    InlineCache,
    // 基础JIT类型
    Jit,
    LoopOptimizer,
    VendorOptimizationStrategy,
    VendorOptimizer,
};

#[cfg(not(feature = "jit-full"))]
fn main() {
    eprintln!("错误: 此示例需要 jit-full feature");
    eprintln!("请运行: cargo run --example jit_full_example --features jit-full");
    std::process::exit(1);
}

#[cfg(feature = "jit-full")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== vm-engine JIT完整功能示例 ===\n");

    // 1. 基础JIT创建
    demo_basic_jit()?;

    // 2. 编译缓存演示
    demo_compile_cache()?;

    // 3. JIT优化passes演示
    demo_optimization_passes()?;

    // 4. CPU厂商优化演示
    demo_vendor_optimization()?;

    println!("\n=== 示例完成 ===");
    Ok(())
}

#[cfg(feature = "jit-full")]
fn demo_basic_jit() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- 1. 基础JIT创建 ---");

    // 创建基础JIT编译器
    let _jit = Jit::new();

    println!("✓ Jit编译器创建成功");
    println!("  - 基础JIT编译功能");
    println!("  - 支持即时编译");
    println!();

    Ok(())
}

#[cfg(feature = "jit-full")]
fn demo_compile_cache() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- 2. 编译缓存 ---");

    // 创建编译缓存，最多1000条目
    let _cache = CompileCache::new(1000);

    println!("✓ CompileCache创建成功");
    println!("  - 内存缓存: 最多1000条目");
    println!("  - 快速查找已编译代码");
    println!("  - 减少重复编译开销");
    println!();

    Ok(())
}

#[cfg(feature = "jit-full")]
fn demo_optimization_passes() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- 3. JIT优化Passes ---");

    // 创建优化器
    let _block_chainer = BlockChainer::new();
    let _loop_optimizer = LoopOptimizer::new();

    // 创建内联缓存 (ID为0)
    let _inline_cache = InlineCache::new(0);

    println!("✓ 优化passes初始化成功");
    println!("  - BlockChainer: 基本块链接优化");
    println!("  - LoopOptimizer: 循环优化");
    println!("  - InlineCache: 内联缓存 (ID=0)");
    println!();

    Ok(())
}

#[cfg(feature = "jit-full")]
fn demo_vendor_optimization() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- 4. CPU厂商优化 ---");

    // 创建厂商优化器（使用默认策略）
    let _vendor = VendorOptimizer::new(VendorOptimizationStrategy::default());

    println!("✓ VendorOptimizer创建成功");
    println!("  - 支持的厂商: Intel, AMD, ARM, Apple Silicon");
    println!("  - 特性检测: AVX2, AVX-512, NEON等");
    println!("  - 厂商特定优化");
    println!();

    Ok(())
}

#[cfg(feature = "jit-full")]
fn demonstrate_real_usage() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- 实际使用示例 ---\n");

    // 创建完整的JIT pipeline
    let _jit = Jit::new();
    let _cache = CompileCache::new(1000);
    let _chainer = BlockChainer::new();
    let _loopy = LoopOptimizer::new();
    let _inliner = InlineCache::new(0);
    let _vendor = VendorOptimizer::new(VendorOptimizationStrategy::default());

    println!("✓ 完整JIT pipeline已创建");
    println!("  - JIT编译器");
    println!("  - 编译缓存");
    println!("  - 优化passes");
    println!("  - 厂商优化");
    println!();

    Ok(())
}
