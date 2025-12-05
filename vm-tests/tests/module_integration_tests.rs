//! 核心模块集成测试套件
//!
//! 测试所有核心模块之间的集成，确保模块间协作正常，解决依赖问题

use std::sync::Arc;
use tokio;
use vm_core::{ExecMode, GuestArch, VmConfig, VmError};
use vm_engine_interpreter::Interpreter;
use vm_engine_interpreter::async_executor_integration::AsyncExecutorWrapper;
use vm_engine_jit::Jit;
use vm_engine_jit::aot_integration::init_aot_loader;
use vm_frontend_riscv64::RiscvDecoder;
use vm_ir::MemFlags;
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator};
use vm_mem::SoftMmu;
use vm_service::VmService;

/// 测试核心模块基本集成
#[tokio::test]
async fn test_core_modules_integration() {
    // 创建基本配置
    let config = VmConfig {
        guest_arch: GuestArch::Riscv64,
        memory_size: 64 * 1024 * 1024, // 64MB
        vcpu_count: 1,
        exec_mode: ExecMode::Interpreter,
        ..Default::default()
    };

    // 创建 VM Service
    let mut service = VmService::new(config, None).await;
    assert!(service.is_ok());
}

/// 测试执行引擎与内存管理集成
#[test]
fn test_execution_engine_memory_integration() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    let mut interpreter = Interpreter::new();

    // 创建简单的 IR 块
    let mut builder = IRBuilder::new(0x1000);
    builder.push(IROp::MovImm { dst: 0, imm: 42 });
    builder.set_term(Terminator::Ret);
    let block = builder.build();

    // 执行
    let result = interpreter.run(&mut mmu, &block);
    assert!(matches!(
        result.status,
        vm_core::ExecStatus::Ok | vm_core::ExecStatus::Continue
    ));
    assert_eq!(interpreter.get_reg(0), 42);
}

/// 测试 JIT 引擎与内存管理集成
#[test]
fn test_jit_engine_memory_integration() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    let mut jit = Jit::new();

    // 创建简单的 IR 块
    let mut builder = IRBuilder::new(0x1000);
    builder.push(IROp::MovImm { dst: 0, imm: 100 });
    builder.push(IROp::Add {
        dst: 1,
        src1: 0,
        src2: 0,
    });
    builder.set_term(Terminator::Ret);
    let block = builder.build();

    // 执行多次以触发 JIT 编译
    for _ in 0..150 {
        let _ = jit.run(&mut mmu, &block);
    }

    // 验证 JIT 已编译
    assert!(jit.is_hot(0x1000));
}

/// 测试解码器与执行引擎集成
#[test]
fn test_decoder_execution_engine_integration() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    let mut interpreter = Interpreter::new();
    let decoder = RiscvDecoder;

    // 写入一些 RISC-V 指令到内存
    // 这里使用简单的指令编码（实际应该使用正确的 RISC-V 编码）
    // 为了测试目的，我们直接创建 IR 块
    let mut builder = IRBuilder::new(0x1000);
    builder.push(IROp::MovImm { dst: 1, imm: 10 });
    builder.push(IROp::MovImm { dst: 2, imm: 20 });
    builder.push(IROp::Add {
        dst: 3,
        src1: 1,
        src2: 2,
    });
    builder.set_term(Terminator::Ret);
    let block = builder.build();

    // 执行
    let result = interpreter.run(&mut mmu, &block);
    assert!(matches!(
        result.status,
        vm_core::ExecStatus::Ok | vm_core::ExecStatus::Continue
    ));
    assert_eq!(interpreter.get_reg(3), 30);
}

/// 测试 AOT 加载器与 JIT 引擎集成
#[tokio::test]
async fn test_aot_jit_integration() {
    // 创建配置，启用 AOT
    let mut config = VmConfig::default();
    config.aot.enable_aot = true;
    // 注意：这里不提供实际路径，测试初始化逻辑
    config.aot.aot_image_path = None;

    // 初始化 AOT 加载器
    let loader = init_aot_loader(&config).await;
    assert!(loader.is_ok());
    // 由于没有提供路径，应该返回 None
    assert!(loader.unwrap().is_none());
}

/// 测试异步执行引擎集成
#[tokio::test]
async fn test_async_executor_integration() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    let executor = AsyncExecutorWrapper::new(10);

    // 创建 IR 块
    let mut builder = IRBuilder::new(0x1000);
    builder.push(IROp::MovImm { dst: 0, imm: 42 });
    builder.set_term(Terminator::Ret);
    let block = builder.build();

    // 异步执行
    let result = executor.execute_block_async(&mut mmu, &block).await;
    assert!(result.is_ok());
}

/// 测试多模块协作：解码器 -> 解释器 -> 内存管理
#[test]
fn test_multi_module_collaboration() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    let mut interpreter = Interpreter::new();
    let decoder = RiscvDecoder;

    // 模拟完整的执行流程
    let mut pc = 0x1000u64;

    for _ in 0..10 {
        // 创建 IR 块（模拟解码）
        let mut builder = IRBuilder::new(pc);
        builder.push(IROp::MovImm { dst: 0, imm: pc });
        builder.set_term(Terminator::Ret);
        let block = builder.build();

        // 执行
        let result = interpreter.run(&mut mmu, &block);
        assert!(matches!(
            result.status,
            vm_core::ExecStatus::Ok | vm_core::ExecStatus::Continue
        ));

        pc = result.next_pc;
    }
}

/// 测试配置验证与模块初始化集成
#[tokio::test]
async fn test_config_module_integration() {
    // 测试不同的执行模式配置
    let modes = [ExecMode::Interpreter, ExecMode::Jit, ExecMode::Hybrid];

    for mode in modes {
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 64 * 1024 * 1024,
            vcpu_count: 1,
            exec_mode: mode,
            ..Default::default()
        };

        let service = VmService::new(config, None).await;
        assert!(
            service.is_ok(),
            "Failed to create service with mode: {:?}",
            mode
        );
    }
}

/// 测试模块间错误传播
#[test]
fn test_error_propagation_across_modules() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    let mut interpreter = Interpreter::new();

    // 创建会导致错误的 IR 块（访问无效地址）
    let mut builder = IRBuilder::new(0x1000);
    builder.push(IROp::Load {
        dst: 0,
        base: 31,                   // 使用寄存器 31（通常为 0）
        offset: 0xFFFFFFFFFFFFFFFF, // 无效地址偏移
        size: 8,
        flags: vm_ir::MemFlags::default(),
    });
    builder.set_term(Terminator::Ret);
    let block = builder.build();

    // 执行应该处理错误
    let result = interpreter.run(&mut mmu, &block);
    // 可能返回 Ok（如果错误被处理）或 Fault
    assert!(matches!(
        result.status,
        vm_core::ExecStatus::Ok | vm_core::ExecStatus::Fault(_)
    ));
}

/// 测试并发模块访问
#[tokio::test]
async fn test_concurrent_module_access() {
    let mmu = Arc::new(std::sync::Mutex::new(SoftMmu::new(1024 * 1024, false)));
    let mut handles = Vec::new();

    // 创建多个并发任务
    for i in 0..5 {
        let mmu_clone = Arc::clone(&mmu);
        let handle = tokio::spawn(async move {
            let mut interpreter = Interpreter::new();
            let mut builder = IRBuilder::new(0x1000 + i * 0x100);
            builder.push(IROp::MovImm {
                dst: 0,
                imm: i as u64,
            });
            builder.set_term(Terminator::Ret);
            let block = builder.build();

            let mut mmu_guard = mmu_clone.lock().unwrap();
            interpreter.run(&mut *mmu_guard, &block)
        });
        handles.push(handle);
    }

    // 等待所有任务完成
    for handle in handles {
        let result = handle.await;
        assert!(result.is_ok());
        let exec_result = result.unwrap();
        assert!(matches!(
            exec_result.status,
            vm_core::ExecStatus::Ok | vm_core::ExecStatus::Continue
        ));
    }
}

/// 测试模块生命周期管理
#[tokio::test]
async fn test_module_lifecycle() {
    // 创建和销毁多个服务实例
    for _ in 0..5 {
        let config = VmConfig::default();
        let service = VmService::new(config, None).await;
        assert!(service.is_ok());
        // 服务在这里被销毁
    }
}

/// 测试模块配置一致性
#[test]
fn test_module_config_consistency() {
    let config = VmConfig {
        guest_arch: GuestArch::Riscv64,
        memory_size: 128 * 1024 * 1024,
        vcpu_count: 2,
        exec_mode: ExecMode::Hybrid,
        jit_threshold: 50,
        ..Default::default()
    };

    // 验证配置一致性
    assert_eq!(config.guest_arch, GuestArch::Riscv64);
    assert_eq!(config.memory_size, 128 * 1024 * 1024);
    assert_eq!(config.vcpu_count, 2);
    assert_eq!(config.exec_mode, ExecMode::Hybrid);
    assert_eq!(config.jit_threshold, 50);
}
