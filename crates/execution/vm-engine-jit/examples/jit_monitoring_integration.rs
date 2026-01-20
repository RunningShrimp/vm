//! JIT监控集成示例
//!
//! 本示例展示如何将vm-engine-jit与事件系统集成，
//! 实现JIT编译的实时性能监控。
//!
//! 注意：本示例展示了如何设置事件总线，实际的JitPerformanceMonitor
//! 需要从vm-monitor包中创建和订阅事件。
//!
//! 运行示例:
//! ```bash
//! cargo run --example jit_monitoring_integration --package vm-engine-jit
//! ```

use std::sync::Arc;
use vm_core::domain_services::DomainEventBus;
use vm_engine_jit::Jit;
use vm_ir::{IRBlock, IROp, Terminator};

fn main() {
    // 1. 创建DomainEventBus
    let event_bus = Arc::new(DomainEventBus::new());
    println!("✅ Created DomainEventBus");

    // 2. 创建JIT引擎并设置event_bus和vm_id
    let mut jit = Jit::new();
    jit.set_event_bus(event_bus.clone());
    jit.set_vm_id("example-vm".to_string());
    println!("✅ Configured JIT engine with event bus");

    // 3. 模拟一些JIT编译活动
    println!("\n📊 Simulating JIT compilation activity...\n");

    // 创建一些测试代码块
    let blocks = create_test_blocks();

    // 编译代码块
    for (i, block) in blocks.iter().enumerate() {
        println!(
            "Compiling block {}: PC=0x{:x}, ops={}",
            i + 1,
            block.start_pc.0,
            block.ops.len()
        );

        // 编译代码块（这会触发CodeBlockCompiled事件）
        // 使用compile_only方法：只编译不执行，返回代码指针
        let code_ptr = jit.compile_only(block);
        if !code_ptr.is_null() {
            println!("  ✅ Block compiled successfully (ptr={:?})", code_ptr);
        } else {
            println!("  ❌ Block compilation failed (null pointer)");
        }

        // 模拟热点检测（多次执行会触发HotspotDetected事件）
        for _ in 0..10 {
            jit.record_execution(block.start_pc);
        }
        println!("  ✅ Recorded executions");
    }

    println!("\n📊 Integration test completed successfully!");
    println!("\n💡 To use JitPerformanceMonitor, create a vm-monitor instance");
    println!("   and subscribe it to the event_bus to receive JIT events.");
}

/// 创建测试用的IR块
fn create_test_blocks() -> Vec<IRBlock> {
    vec![
        // Block 1: 简单的加法操作
        IRBlock {
            start_pc: vm_core::GuestAddr(0x1000),
            ops: vec![
                IROp::AddImm {
                    dst: 1,
                    src: 0,
                    imm: 42,
                },
                IROp::AddImm {
                    dst: 2,
                    src: 1,
                    imm: 10,
                },
            ],
            term: Terminator::Jmp {
                target: vm_core::GuestAddr(0x1004),
            },
        },
        // Block 2: 寄存器移动
        IRBlock {
            start_pc: vm_core::GuestAddr(0x1004),
            ops: vec![IROp::Mov { dst: 3, src: 1 }, IROp::Mov { dst: 4, src: 2 }],
            term: Terminator::Jmp {
                target: vm_core::GuestAddr(0x1008),
            },
        },
        // Block 3: 立即数加载
        IRBlock {
            start_pc: vm_core::GuestAddr(0x1008),
            ops: vec![
                IROp::MovImm { dst: 5, imm: 100 },
                IROp::MovImm { dst: 6, imm: 200 },
            ],
            term: Terminator::Ret,
        },
    ]
}
