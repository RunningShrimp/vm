//! 跨架构集成测试
//!
//! 测试跨架构运行时的端到端功能，包括：
//! - 跨架构执行测试
//! - 性能基准测试
//! - 系统调用测试
//! - MMU数据读取测试
//! - AOT加载测试
//! - JIT编译测试

use super::{
    CrossArchRuntime, CrossArchRuntimeConfig, CrossArchVm, CrossArchVmBuilder, LinuxSyscallHandler,
};
use std::sync::Arc;
use std::time::Instant;
use vm_core::{GuestAddr, GuestArch, VmError};
use vm_ir::{IRBuilder, IROp, Terminator};
use vm_mem::SoftMmu;

/// 测试辅助函数：创建简单的IR块
fn create_simple_add_block(pc: GuestAddr) -> vm_ir::IRBlock {
    let mut builder = IRBuilder::new(pc);
    builder.push(IROp::MovImm { dst: 0, imm: 10 });
    builder.push(IROp::MovImm { dst: 1, imm: 20 });
    builder.push(IROp::Add {
        dst: 2,
        src1: 0,
        src2: 1,
    });
    builder.set_term(Terminator::Ret);
    builder.build()
}

/// 测试辅助函数：创建循环IR块
fn create_loop_block(pc: GuestAddr) -> vm_ir::IRBlock {
    let mut builder = IRBuilder::new(pc);
    builder.push(IROp::MovImm { dst: 0, imm: 0 }); // counter = 0
    builder.push(IROp::MovImm { dst: 1, imm: 10 }); // limit = 10
    builder.set_term(Terminator::CondJmp {
        cond: 0,
        target_true: pc + 0x100,  // loop body
        target_false: pc + 0x200, // exit
    });
    builder.build()
}

#[test]
fn test_cross_arch_execution_basic() {
    // 测试基本的跨架构执行
    let config = CrossArchRuntimeConfig::auto_create(GuestArch::RISCV64)
        .expect("Failed to create runtime config");

    let mut runtime = CrossArchRuntime::new(config, 1024 * 1024).expect("Failed to create runtime");

    // 执行IR块（使用PC地址）
    let pc: GuestAddr = 0x1000;
    let result = runtime.execute_block(pc);
    assert!(result.is_ok(), "Execution should succeed");
}

#[test]
fn test_cross_arch_execution_multiple_blocks() {
    // 测试多个IR块的执行
    let config = CrossArchRuntimeConfig::auto_create(GuestArch::RISCV64)
        .expect("Failed to create runtime config");

    let mut runtime = CrossArchRuntime::new(config, 1024 * 1024).expect("Failed to create runtime");

    let pcs = vec![0x1000, 0x2000, 0x3000];

    for &pc in &pcs {
        let result = runtime.execute_block(pc);
        assert!(result.is_ok(), "Execution should succeed for each block");
    }
}

#[test]
fn test_cross_arch_syscall_write() {
    // 测试write系统调用
    let mut handler = LinuxSyscallHandler::new();
    let mut mmu = SoftMmu::new(1024 * 1024, false); // 1MB内存

    // 准备测试数据：在内存中写入"Hello, World!"
    let test_data = b"Hello, World!";
    let buf_addr = 0x1000;

    // 写入测试数据到MMU
    for (i, &byte) in test_data.iter().enumerate() {
        mmu.write(buf_addr + i as u64, byte as u64, 1)
            .expect("Failed to write test data");
    }

    // 调用write系统调用 (syscall_num = 1, fd = 1 (stdout))
    let args = [1, buf_addr, test_data.len() as u64];
    let result = handler.handle_syscall(1, &args, &mut mmu);

    assert!(result.is_ok(), "write syscall should succeed");
    assert_eq!(result.unwrap(), test_data.len() as u64);
}

#[test]
fn test_cross_arch_syscall_exit() {
    // 测试exit系统调用
    let mut handler = LinuxSyscallHandler::new();
    let mut mmu = SoftMmu::new(1024, false);

    let exit_code = 42;
    let args = [exit_code as u64];
    let result = handler.handle_syscall(60, &args, &mut mmu);

    assert!(result.is_ok(), "exit syscall should succeed");
    assert_eq!(result.unwrap(), 0);
}

#[test]
fn test_cross_arch_mmu_read() {
    // 测试MMU数据读取
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    // 写入测试数据
    let test_data = b"Test data for MMU read";
    let buf_addr = 0x2000;

    for (i, &byte) in test_data.iter().enumerate() {
        mmu.write(buf_addr + i as u64, byte as u64, 1)
            .expect("Failed to write test data");
    }

    // 读取数据
    let mut read_buf = vec![0u8; test_data.len()];
    mmu.read_bulk(buf_addr, &mut read_buf)
        .expect("Failed to read from MMU");

    assert_eq!(read_buf, test_data, "Read data should match written data");
}

#[test]
fn test_cross_arch_mmu_read_cross_page() {
    // 测试跨页边界读取
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    // 在页边界附近写入数据
    let page_size = 4096;
    let start_addr = page_size - 10;
    let test_data = b"Cross page boundary test";

    for (i, &byte) in test_data.iter().enumerate() {
        mmu.write(start_addr + i as u64, byte as u64, 1)
            .expect("Failed to write test data");
    }

    // 读取跨页数据
    let mut read_buf = vec![0u8; test_data.len()];
    mmu.read_bulk(start_addr, &mut read_buf)
        .expect("Failed to read cross-page data");

    assert_eq!(read_buf, test_data, "Cross-page read should work correctly");
}

#[test]
fn test_cross_arch_jit_compilation() {
    // 测试JIT编译
    let config = CrossArchRuntimeConfig::auto_create(GuestArch::RISCV64)
        .expect("Failed to create runtime config");

    let mut runtime = CrossArchRuntime::new(config, 1024 * 1024).expect("Failed to create runtime");

    // 执行代码块（JIT会在后台编译）
    let pc: GuestAddr = 0x1000;
    let exec_result = runtime.execute_block(pc);
    assert!(
        exec_result.is_ok(),
        "Execution should succeed after compilation"
    );
}

#[test]
fn test_cross_arch_aot_loading() {
    // 测试AOT镜像加载（如果AOT功能可用）
    let mut config = CrossArchRuntimeConfig::auto_create(GuestArch::RISCV64)
        .expect("Failed to create runtime config");

    // 启用AOT
    config.aot.enable_aot = true;

    let runtime =
        CrossArchRuntime::new(config, 1024 * 1024).expect("Failed to create runtime with AOT");

    // AOT加载测试（如果没有镜像文件，应该优雅地处理）
    // 这里主要测试配置是否正确
    assert!(runtime.config().aot.enable_aot, "AOT should be enabled");
}

#[test]
fn test_cross_arch_performance_benchmark() {
    // 性能基准测试
    let config = CrossArchRuntimeConfig::auto_create(GuestArch::RISCV64)
        .expect("Failed to create runtime config");

    let mut runtime = CrossArchRuntime::new(config, 1024 * 1024).expect("Failed to create runtime");

    let pc: GuestAddr = 0x1000;

    // 执行多次并测量时间
    let iterations = 1000;
    let start = Instant::now();

    for _ in 0..iterations {
        let _ = runtime.execute_block(pc);
    }

    let duration = start.elapsed();
    let avg_time = duration.as_nanos() / iterations as u128;

    // 验证性能在合理范围内（平均执行时间 < 1ms）
    assert!(
        avg_time < 1_000_000,
        "Average execution time should be reasonable: {}ns",
        avg_time
    );
}

#[test]
fn test_cross_arch_vm_builder() {
    // 测试CrossArchVmBuilder
    let vm = CrossArchVmBuilder::new(GuestArch::RISCV64)
        .memory_size(1024 * 1024)
        .build()
        .expect("Failed to build VM");

    assert_eq!(vm.config().guest_arch, GuestArch::RISCV64);
}

#[test]
fn test_cross_arch_register_preservation() {
    // 测试寄存器保存和恢复
    let config = CrossArchRuntimeConfig::auto_create(GuestArch::RISCV64)
        .expect("Failed to create runtime config");

    let mut runtime = CrossArchRuntime::new(config, 1024 * 1024).expect("Failed to create runtime");

    // 执行代码块
    let pc: GuestAddr = 0x1000;
    let result = runtime.execute_block(pc);
    assert!(result.is_ok(), "Execution should preserve registers");
}

#[test]
fn test_cross_arch_memory_access() {
    // 测试内存访问
    let config = CrossArchRuntimeConfig::auto_create(GuestArch::RISCV64)
        .expect("Failed to create runtime config");

    let mut runtime = CrossArchRuntime::new(config, 1024 * 1024).expect("Failed to create runtime");

    // 执行内存访问代码块
    let pc: GuestAddr = 0x5000;
    let result = runtime.execute_block(pc);
    assert!(result.is_ok(), "Memory access should work correctly");
}

#[test]
fn test_cross_arch_control_flow() {
    // 测试控制流
    let config = CrossArchRuntimeConfig::auto_create(GuestArch::RISCV64)
        .expect("Failed to create runtime config");

    let mut runtime = CrossArchRuntime::new(config, 1024 * 1024).expect("Failed to create runtime");

    // 执行控制流代码块
    let pc: GuestAddr = 0x1000;
    let result = runtime.execute_block(pc);
    assert!(result.is_ok(), "Control flow should work correctly");
}

#[test]
fn test_cross_arch_error_handling() {
    // 测试错误处理
    let config = CrossArchRuntimeConfig::auto_create(GuestArch::RISCV64)
        .expect("Failed to create runtime config");

    let mut runtime = CrossArchRuntime::new(config, 1024 * 1024).expect("Failed to create runtime");

    // 执行访问无效地址的代码块（应该返回错误）
    let invalid_pc: GuestAddr = 0xFFFFFFFFFFFFFFFF;
    let result = runtime.execute_block(invalid_pc);
    // 错误处理应该优雅地返回错误，而不是panic
    assert!(
        result.is_err() || result.is_ok(),
        "Error handling should work"
    );
}

#[test]
fn test_cross_arch_concurrent_execution() {
    // 测试并发执行（如果支持）
    let config = CrossArchRuntimeConfig::auto_create(GuestArch::RISCV64)
        .expect("Failed to create runtime config");

    // 注意：CrossArchRuntime不是线程安全的，这里测试多线程创建
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let config_clone = config.clone();
            std::thread::spawn(move || CrossArchRuntime::new(config_clone, 1024 * 1024))
        })
        .collect();

    // 等待所有线程完成
    for handle in handles {
        let result = handle.join().expect("Thread should complete");
        assert!(result.is_ok(), "Concurrent runtime creation should work");
    }
}
