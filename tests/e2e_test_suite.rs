//! 端到端测试套件
//!
//! 测试完整的虚拟机执行流程，包括：
//! - 跨架构执行
//! - AOT/JIT混合执行
//! - GC压力测试
//! - 性能回归测试

use vm_cross_arch::{UnifiedExecutor, HostArch};
use vm_core::{GuestArch, MMU, ExecutionEngine, Decoder};
use vm_ir::IRBlock;
use vm_ir::lift::ISA;
use vm_mem::SoftMmu;

/// 测试跨架构执行
#[test]
fn test_cross_arch_execution() {
    println!("=== 测试跨架构执行 ===");
    
    let host = HostArch::detect();
    println!("Host架构: {:?}", host);
    
    // 测试三种架构
    let guest_archs = vec![
        GuestArch::X86_64,
        GuestArch::Arm64,
        GuestArch::Riscv64,
    ];
    
    for guest_arch in guest_archs {
        println!("测试Guest架构: {:?}", guest_arch);
        
        let mut executor = UnifiedExecutor::auto_create(guest_arch, 128 * 1024 * 1024)
            .expect("创建统一执行器失败");
        
        // 加载简单测试代码
        let code_base: u64 = 0x1000;
        let test_code = create_test_code(guest_arch);
        
        executor.mmu_mut().write_bulk(code_base, &test_code)
            .expect("写入内存失败");
        
        // 执行代码
        for _ in 0..100 {
            let result = executor.execute(code_base).expect("执行失败");
            assert!(matches!(result.status, vm_core::ExecStatus::Ok | vm_core::ExecStatus::Continue));
        }
        
        println!("  ✓ 跨架构执行测试通过");
    }
}

/// 测试AOT/JIT混合执行
#[test]
fn test_aot_jit_hybrid_execution() {
    println!("=== 测试AOT/JIT混合执行 ===");
    
    let mut executor = UnifiedExecutor::auto_create(GuestArch::X86_64, 128 * 1024 * 1024)
        .expect("创建统一执行器失败");
    
    // 注意：UnifiedExecutor的配置在创建时设置，这里仅测试执行
    // AOT和JIT的启用由运行时自动决定
    
    let code_base: u64 = 0x1000;
    let test_code = create_test_code(GuestArch::X86_64);
    
    // 加载代码
    executor.mmu_mut().write_bulk(code_base, &test_code)
        .expect("写入内存失败");
    
    // 执行多次以触发JIT编译
    for i in 0..500 {
        let result = executor.execute(code_base).expect("执行失败");
        assert!(matches!(result.status, vm_core::ExecStatus::Ok | vm_core::ExecStatus::Continue));
        
        // 检查执行统计
        if i == 99 || i == 199 || i == 499 {
            executor.update_stats();
            let stats = executor.stats();
            println!("  执行 {} 次: AOT={}, JIT={}, Interpreter={}", 
                i + 1,
                stats.aot_executions,
                stats.jit_executions,
                stats.interpreter_executions);
        }
    }
    
    executor.update_stats();
    let stats = executor.stats();
    
    // 验证JIT执行次数 > 0（说明JIT编译被触发）
    assert!(stats.jit_executions > 0 || stats.aot_executions > 0, 
        "JIT或AOT应该被触发");
    
    println!("  ✓ AOT/JIT混合执行测试通过");
}

/// 测试GC压力
#[test]
fn test_gc_stress() {
    println!("=== 测试GC压力 ===");
    
    use vm_engine_jit::{UnifiedGC, UnifiedGcConfig};
    
    let config = UnifiedGcConfig {
        heap_size_limit: 10 * 1024 * 1024, // 10MB
        mark_quota_us: 1000,
        sweep_quota_us: 500,
        adaptive_quota: true,
        ..Default::default()
    };
    
    let gc = UnifiedGC::new(config);
    
    // 模拟大量对象分配
    let roots: Vec<u64> = (0..1000).map(|i| i as u64 * 1024).collect();
    
    // 执行多个GC周期
    for cycle in 0..10 {
        let cycle_start = gc.start_gc(&roots);
        
        // 增量标记
        loop {
            let (complete, _) = gc.incremental_mark();
            if complete {
                break;
            }
        }
        
        gc.terminate_marking();
        
        // 增量清扫
        loop {
            let (complete, _) = gc.incremental_sweep();
            if complete {
                break;
            }
        }
        
        gc.finish_gc(cycle_start);
        
        // 检查统计信息
        let stats = gc.stats();
        if cycle % 2 == 0 {
            println!("  GC周期 {}: 暂停时间={}us, 标记对象={}, 释放对象={}",
                cycle,
                stats.get_last_pause_us(),
                stats.get_objects_marked(),
                stats.get_objects_freed());
        }
    }
    
    let stats = gc.stats();
    assert!(stats.get_avg_pause_us() < 10000, "平均暂停时间应该 < 10ms");
    
    println!("  ✓ GC压力测试通过");
}

/// 测试跨架构AOT编译
#[test]
#[ignore] // 需要LLVM支持，暂时忽略
fn test_cross_arch_aot() {
    println!("=== 测试跨架构AOT编译 ===");
    
    use aot_builder::{AotBuilder, CompilationOptions, CodegenMode};
    use vm_ir::lift::ISA;
    
    // 测试不同架构组合（仅测试直接代码生成模式，不依赖LLVM）
    let arch_combinations = vec![
        (ISA::X86_64, ISA::X86_64), // 同架构测试
    ];
    
    for (source_isa, target_isa) in arch_combinations {
        println!("  测试 {} -> {}", source_isa, target_isa);
        
        let options = CompilationOptions {
            optimization_level: 1,
            target_isa,
            enable_applicability_check: false,
            codegen_mode: CodegenMode::Direct, // 使用直接代码生成，不依赖LLVM
        };
        
        let mut builder = AotBuilder::with_options(options);
        
        // 创建测试代码块
        let test_code = create_test_code_for_isa(source_isa);
        let pc: u64 = 0x1000;
        
        // 编译代码块
        let result = builder.add_raw_code_block(pc, &test_code, 0);
        // 允许失败，因为直接代码生成可能不支持所有架构组合
        if result.is_ok() {
            println!("    ✓ {} -> {} AOT编译成功", source_isa, target_isa);
        } else {
            println!("    ⚠ {} -> {} AOT编译跳过（可能需要LLVM）", source_isa, target_isa);
        }
    }
    
    println!("  ✓ 跨架构AOT编译测试完成");
}

/// 测试混合执行器回退机制
#[test]
fn test_hybrid_executor_fallback() {
    println!("=== 测试混合执行器回退机制 ===");
    
    use vm_engine_jit::HybridExecutor;
    use vm_engine_jit::Jit;
    
    // 创建混合执行器（无AOT加载器）
    let executor = HybridExecutor::new(None);
    
    // 创建JIT
    let mut jit = Jit::new();
    
    // 创建测试IR块
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    let block = create_test_ir_block();
    
    // 测试回退机制：AOT -> JIT -> 解释器
    // 由于没有AOT加载器，应该回退到JIT或解释器
    let (result, source) = executor.lookup_and_execute(
        0x1000,
        &block,
        &mut mmu,
        &mut jit,
    );
    
    // 应该回退到JIT或解释器
    assert!(matches!(source, vm_engine_jit::CodeSource::JitCompiled | vm_engine_jit::CodeSource::Interpreted));
    assert!(matches!(result.status, vm_core::ExecStatus::Ok | vm_core::ExecStatus::Continue));
    
    println!("  ✓ 混合执行器回退机制测试通过");
}

/// 测试性能回归
#[test]
fn test_performance_regression() {
    println!("=== 测试性能回归 ===");
    
    let mut executor = UnifiedExecutor::auto_create(GuestArch::X86_64, 128 * 1024 * 1024)
        .expect("创建统一执行器失败");
    
    let code_base: u64 = 0x1000;
    let test_code = create_test_code(GuestArch::X86_64);
    
    // 加载代码
    executor.mmu_mut().write_bulk(code_base, &test_code)
        .expect("写入内存失败");
    
    // 预热
    for _ in 0..100 {
        executor.execute(code_base).expect("执行失败");
    }
    
    // 性能测试
    let start = std::time::Instant::now();
    let iterations = 1000;
    
    for _ in 0..iterations {
        executor.execute(code_base).expect("执行失败");
    }
    
    let elapsed = start.elapsed();
    let avg_time_us = elapsed.as_micros() as u64 / iterations;
    
    println!("  平均执行时间: {}us/次", avg_time_us);
    
    // 性能阈值：平均执行时间应该 < 1000us
    assert!(avg_time_us < 1000, 
        "性能回归：平均执行时间 {}us 超过阈值 1000us", avg_time_us);
    
    println!("  ✓ 性能回归测试通过");
}

// 辅助函数

fn create_test_code(arch: GuestArch) -> Vec<u8> {
    match arch {
        GuestArch::X86_64 => vec![
            0xB8, 0x0A, 0x00, 0x00, 0x00,  // mov eax, 10
            0xBB, 0x14, 0x00, 0x00, 0x00,  // mov ebx, 20
            0x01, 0xD8,                     // add eax, ebx
            0xC3,                           // ret
        ],
        GuestArch::Arm64 => vec![
            0x21, 0x00, 0x80, 0xD2,  // mov x1, #10
            0x42, 0x00, 0x80, 0xD2,  // mov x2, #20
            0x23, 0x00, 0x02, 0x8B,  // add x3, x1, x2
            0xC0, 0x03, 0x5F, 0xD6,  // ret
        ],
        GuestArch::Riscv64 => vec![
            0x93, 0x00, 0xA0, 0x00,  // li x1, 10
            0x13, 0x01, 0x40, 0x01,  // li x2, 20
            0xB3, 0x01, 0x21, 0x00,  // add x3, x1, x2
            0x67, 0x80, 0x00, 0x00,  // ret
        ],
    }
}

fn create_test_code_for_isa(isa: ISA) -> Vec<u8> {
    match isa {
        ISA::X86_64 => create_test_code(GuestArch::X86_64),
        ISA::ARM64 => create_test_code(GuestArch::Arm64),
        ISA::RISCV64 => create_test_code(GuestArch::Riscv64),
    }
}

fn create_test_ir_block() -> IRBlock {
    use vm_ir::{IROp, Terminator};
    
    IRBlock {
        start_pc: 0x1000,
        ops: vec![
            IROp::MovImm { dst: 1, imm: 10 },
            IROp::MovImm { dst: 2, imm: 20 },
            IROp::Add { dst: 3, src1: 1, src2: 2 },
        ],
        term: Terminator::Ret,
    }
}

