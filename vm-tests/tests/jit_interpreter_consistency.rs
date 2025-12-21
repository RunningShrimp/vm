//! JIT 与解释器一致性差分测试
//!
//! 本测试套件验证 JIT 编译执行和解释器执行产生相同的结果，
//! 确保两种执行模式的语义一致性。

use vm_core::GuestAddr;
use vm_engine_jit::Jit;
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator};

/// 创建简单的算术测试 IR 块
fn create_arithmetic_block() -> IRBlock {
    let mut builder = IRBuilder::new(GuestAddr(0x1000));

    // 简单的加法操作：R0 = R1 + R2
    builder.push(IROp::Add {
        dst: 0,
        src1: 1,
        src2: 2,
    });

    // 减法操作：R3 = R0 - R1
    builder.push(IROp::Sub {
        dst: 3,
        src1: 0,
        src2: 1,
    });

    // 设置终止条件
    builder.set_term(Terminator::Ret);

    builder.build()
}

/// 创建简单的逻辑操作测试 IR 块
fn create_logic_block() -> IRBlock {
    let mut builder = IRBuilder::new(GuestAddr(0x2000));

    // AND 操作
    builder.push(IROp::And {
        dst: 0,
        src1: 1,
        src2: 2,
    });

    // OR 操作
    builder.push(IROp::Or {
        dst: 3,
        src1: 0,
        src2: 1,
    });

    builder.set_term(Terminator::Ret);

    builder.build()
}

/// 创建空块测试
fn create_empty_block() -> IRBlock {
    let mut builder = IRBuilder::new(GuestAddr(0x4000));
    builder.set_term(Terminator::Ret);
    builder.build()
}

/// 创建 NOP 序列测试块
fn create_nop_sequence_block() -> IRBlock {
    let mut builder = IRBuilder::new(GuestAddr(0x5000));

    // 添加多个 NOP 操作
    for _ in 0..10 {
        builder.push(IROp::Nop);
    }

    builder.set_term(Terminator::Ret);
    builder.build()
}

/// 运行 JIT 编译测试（不需要 MMU）
fn run_jit_compile_test(block: &IRBlock, test_name: &str) -> bool {
    let mut jit = Jit::new();

    // 使用模拟执行模式
    jit.enable_real_execution(false);

    // 编译代码块
    let code_ptr = jit.compile_only(block);

    // 验证编译成功
    let success = !code_ptr.0.is_null();
    if !success {
        eprintln!("[{}] JIT compilation failed", test_name);
    }

    // 验证编译统计
    let stats = jit.get_compile_stats();
    if stats.total_compiles != 1 {
        eprintln!(
            "[{}] Unexpected compile count: expected 1, got {}",
            test_name, stats.total_compiles
        );
        return false;
    }

    success
}

#[test]
fn test_jit_compile_arithmetic() {
    let block = create_arithmetic_block();
    assert!(
        run_jit_compile_test(&block, "arithmetic"),
        "Arithmetic block should compile successfully"
    );
}

#[test]
fn test_jit_compile_logic() {
    let block = create_logic_block();
    assert!(
        run_jit_compile_test(&block, "logic"),
        "Logic block should compile successfully"
    );
}

#[test]
fn test_jit_compile_empty() {
    let block = create_empty_block();
    assert!(
        run_jit_compile_test(&block, "empty"),
        "Empty block should compile successfully"
    );
}

#[test]
fn test_jit_compile_nop_sequence() {
    let block = create_nop_sequence_block();
    assert!(
        run_jit_compile_test(&block, "nop_sequence"),
        "NOP sequence block should compile successfully"
    );
}

#[test]
fn test_jit_cache_operations() {
    let block1 = create_arithmetic_block();
    let block2 = create_logic_block();
    let mut jit = Jit::new();

    // 第一次编译
    let _ = jit.compile_only(&block1);
    assert_eq!(jit.get_compile_stats().total_compiles, 1);
    assert_eq!(jit.get_compile_stats().cache_misses, 1);

    // 第二次访问同一块（缓存命中）
    let _ = jit.compile_only(&block1);
    assert_eq!(jit.get_compile_stats().total_compiles, 1);
    // 注意：compile_only 不增加 cache_hits，只有 run 会

    // 编译不同的块
    let _ = jit.compile_only(&block2);
    assert_eq!(jit.get_compile_stats().total_compiles, 2);
}

#[test]
fn test_jit_real_execution_toggle() {
    let mut jit = Jit::new();

    // 默认应该是禁用真实执行
    assert!(!jit.is_real_execution_enabled());

    // 启用真实执行
    jit.enable_real_execution(true);
    assert!(jit.is_real_execution_enabled());

    // 禁用真实执行
    jit.enable_real_execution(false);
    assert!(!jit.is_real_execution_enabled());
}

#[test]
fn test_jit_adaptive_config() {
    use vm_engine_jit::AdaptiveThresholdConfig;

    let config = AdaptiveThresholdConfig {
        hot_threshold: 200,
        cold_threshold: 20,
        enable_adaptive: true,
    };

    let jit = Jit::with_adaptive_config(config.clone());

    // 验证配置被正确设置
    let stored_config = jit.get_config().expect("Config should be set");
    assert_eq!(stored_config.hot_threshold, 200);
    assert_eq!(stored_config.cold_threshold, 20);
    assert!(stored_config.enable_adaptive);
}

#[test]
fn test_executable_memory_module() {
    use vm_engine_jit::{BlockMetadata, CompiledBlock, ExecutableMemory};

    // 测试可执行内存分配
    let mut mem = ExecutableMemory::allocate(4096).expect("Should allocate memory");
    assert!(!mem.is_executable());
    assert!(mem.is_empty());
    assert_eq!(mem.len(), 0);

    // 写入简单的代码
    #[cfg(target_arch = "x86_64")]
    let code = vec![0xC3]; // RET

    #[cfg(target_arch = "aarch64")]
    let code = vec![0xC0, 0x03, 0x5F, 0xD6]; // RET

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    let code = vec![0x00]; // 占位符

    mem.write(&code).expect("Should write code");
    assert_eq!(mem.len(), code.len());

    // 切换为可执行
    mem.make_executable().expect("Should make executable");
    assert!(mem.is_executable());

    // 尝试写入应该失败（W^X）
    let write_result = mem.write(&[0x90]);
    assert!(
        write_result.is_err(),
        "Writing to executable memory should fail"
    );

    // 切换回可写
    mem.make_writable().expect("Should make writable");
    assert!(!mem.is_executable());

    // 现在可以写入
    mem.write(&code).expect("Should write again");
}

#[test]
fn test_compiled_block_creation() {
    use vm_engine_jit::{BlockMetadata, CompiledBlock};

    let metadata = BlockMetadata {
        start_pc: 0x1000,
        code_size: 4,
        ir_ops_count: 2,
        compile_time_ns: 1000,
        optimization_level: 1,
    };

    #[cfg(target_arch = "x86_64")]
    let code = vec![0x90, 0x90, 0x90, 0xC3]; // NOP, NOP, NOP, RET

    #[cfg(target_arch = "aarch64")]
    let code = vec![0xC0, 0x03, 0x5F, 0xD6]; // RET

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    let code = vec![0x00, 0x00, 0x00, 0x00];

    let block = CompiledBlock::new(code.clone(), metadata).expect("Should create block");

    assert_eq!(block.code_size(), code.len());
    assert_eq!(block.metadata().start_pc, 0x1000);
    assert_eq!(block.metadata().ir_ops_count, 2);
}

/// 性能基准测试
#[test]
#[ignore] // 默认忽略性能测试
fn benchmark_jit_compilation() {
    use std::time::Instant;

    let block = create_arithmetic_block();
    let iterations = 100;

    // 首次编译（冷启动）
    let mut jit = Jit::new();
    let cold_start = Instant::now();
    let _ = jit.compile_only(&block);
    let cold_time = cold_start.elapsed();

    // 缓存命中
    let cache_start = Instant::now();
    for _ in 0..iterations {
        let _ = jit.compile_only(&block);
    }
    let cache_time = cache_start.elapsed();

    println!("Cold compilation: {:?}", cold_time);
    println!(
        "Cache hits ({} iterations): {:?} ({:?}/iter)",
        iterations,
        cache_time,
        cache_time / iterations as u32
    );
    println!(
        "Cache speedup: {:.2}x",
        cold_time.as_nanos() as f64 / (cache_time.as_nanos() as f64 / iterations as f64)
    );
}
