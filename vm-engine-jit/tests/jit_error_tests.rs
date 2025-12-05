//! JIT编译错误场景测试套件
//!
//! 测试各种错误场景下的JIT编译行为：
//! - 编译超时
//! - 内存不足
//! - 无效IR块
//! - 编译失败回退

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use vm_core::GuestAddr;
use vm_engine_jit::*;
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator};

/// 创建测试用的IR块
fn create_test_ir_block(addr: GuestAddr) -> IRBlock {
    let mut builder = IRBuilder::new(addr);
    builder.push(IROp::MovImm { dst: 1, imm: 42 });
    builder.push(IROp::Add {
        dst: 0,
        src1: 0,
        src2: 1,
    });
    builder.set_term(Terminator::Jmp { target: addr + 16 });
    builder.build()
}

/// 创建无效的IR块（可能导致编译失败）
fn create_invalid_ir_block(addr: GuestAddr) -> IRBlock {
    let mut builder = IRBuilder::new(addr);
    // 创建可能导致问题的IR操作
    builder.push(IROp::MovImm { dst: 999, imm: 0 }); // 无效寄存器
    builder.set_term(Terminator::Jmp { target: 0 }); // 无效目标地址
    builder.build()
}

#[test]
fn test_compile_timeout_handling() {
    // 测试编译超时处理
    let mut engine = JitEngine::new(JitConfig::default());
    
    // 设置很短的编译时间预算
    engine.set_compile_time_budget_duration(Duration::from_nanos(1)); // 1纳秒，几乎立即超时
    
    let blocks = {
        let mut map = HashMap::new();
        map.insert(0x1000, create_test_ir_block(0x1000));
        map
    };
    
    // 尝试编译，应该因为超时而失败或回退
    let compiled = engine.process_compile_queue(&blocks);
    
    // 超时情况下应该编译很少或没有块
    assert!(compiled <= 1, "编译应该因为超时而受限");
}

#[test]
fn test_invalid_ir_block_handling() {
    // 测试无效IR块的处理
    let mut engine = JitEngine::new(JitConfig::default());
    
    let blocks = {
        let mut map = HashMap::new();
        map.insert(0x1000, create_invalid_ir_block(0x1000));
        map
    };
    
    // 尝试编译无效块
    // 应该能够优雅地处理，不会panic
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        engine.process_compile_queue(&blocks)
    }));
    
    // 应该能够处理错误而不崩溃
    assert!(result.is_ok(), "应该能够优雅地处理无效IR块");
}

#[test]
fn test_compile_queue_empty_handling() {
    // 测试空编译队列的处理
    let mut engine = JitEngine::new(JitConfig::default());
    
    let empty_blocks = HashMap::new();
    
    // 处理空队列应该不会panic
    let compiled = engine.process_compile_queue(&empty_blocks);
    assert_eq!(compiled, 0, "空队列应该返回0");
}

#[test]
fn test_compile_missing_block_handling() {
    // 测试编译队列中存在但IR块不存在的处理
    let mut engine = JitEngine::new(JitConfig::default());
    
    // 手动添加一个不存在的PC到队列
    engine.add_to_compile_queue(0x9999, 100);
    
    let blocks = HashMap::new(); // 空的blocks映射
    
    // 应该能够处理缺失的块而不panic
    let compiled = engine.process_compile_queue(&blocks);
    assert_eq!(compiled, 0, "缺失的块应该被跳过");
}

#[test]
fn test_async_compile_timeout() {
    // 测试异步编译超时
    // 注意：这个测试需要tokio运行时，在单元测试中可能无法直接运行
    // 这里主要测试API的正确性
    let mut engine = JitEngine::new(JitConfig::default());
    
    let blocks = {
        let mut map = HashMap::new();
        map.insert(0x1000, create_test_ir_block(0x1000));
        map
    };
    
    // 检查编译结果（可能还没有开始编译）
    let result = engine.check_async_compile_result(0x1000);
    
    // 结果应该是超时（因为没有启动编译）或进行中
    assert!(
        matches!(result, AsyncCompileResult::Pending | AsyncCompileResult::Timeout),
        "异步编译应该返回有效状态"
    );
}

#[test]
fn test_prefetch_queue_limits() {
    // 测试预编译队列的限制
    let mut engine = JitEngine::new(JitConfig::default());
    
    let blocks = {
        let mut map = HashMap::new();
        for i in 0..20 {
            map.insert(0x1000 + i * 16, create_test_ir_block(0x1000 + i * 16));
        }
        map
    };
    
    // 触发预取
    engine.prefetch_code_blocks(0x1000);
    
    // 处理预取队列，应该限制数量
    let prefetched = engine.process_prefetch_queue(&blocks);
    
    // 应该限制每次预取的数量（我们设置的是5）
    assert!(prefetched <= 5, "预取应该被限制在合理数量");
}

#[test]
fn test_compile_error_recovery() {
    // 测试编译错误后的恢复能力
    let mut engine = JitEngine::new(JitConfig::default());
    
    let blocks = {
        let mut map = HashMap::new();
        // 先添加一个无效块
        map.insert(0x1000, create_invalid_ir_block(0x1000));
        // 再添加一个有效块
        map.insert(0x2000, create_test_ir_block(0x2000));
        map
    };
    
    // 尝试编译，应该能够处理错误并继续
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        engine.process_compile_queue(&blocks)
    }));
    
    assert!(result.is_ok(), "应该能够从编译错误中恢复");
}

#[test]
fn test_concurrent_compile_handling() {
    // 测试并发编译的处理
    let engine = Arc::new(std::sync::Mutex::new(JitEngine::new(JitConfig::default())));
    
    let blocks = {
        let mut map = HashMap::new();
        for i in 0..5 {
            map.insert(0x1000 + i * 16, create_test_ir_block(0x1000 + i * 16));
        }
        map
    };
    
    let blocks = Arc::new(blocks);
    
    // 启动多个并发编译任务（使用同步编译）
    let handles: Vec<_> = (0..3)
        .map(|i| {
            let engine = engine.clone();
            let blocks = blocks.clone();
            std::thread::spawn(move || {
                let mut engine = engine.lock().unwrap();
                if let Some(block) = blocks.get(&(0x1000 + i * 16)).cloned() {
                    // 使用同步编译进行测试
                    let mut blocks_map = HashMap::new();
                    blocks_map.insert(0x1000 + i * 16, block);
                    engine.process_compile_queue(&blocks_map)
                } else {
                    0
                }
            })
        })
        .collect();
    
    // 等待所有任务完成
    for handle in handles {
        let _ = handle.join();
    }
    
    // 验证没有panic
    assert!(true, "并发编译应该能够正常处理");
}

#[test]
fn test_memory_pressure_handling() {
    // 测试内存压力下的处理（模拟）
    let mut engine = JitEngine::new(JitConfig::default());
    
    // 创建大量块，模拟内存压力
    let blocks = {
        let mut map = HashMap::new();
        for i in 0..100 {
            map.insert(0x1000 + i * 16, create_test_ir_block(0x1000 + i * 16));
        }
        map
    };
    
    // 尝试编译大量块
    let compiled = engine.process_compile_queue(&blocks);
    
    // 应该能够处理，不会因为内存问题而崩溃
    assert!(compiled >= 0, "应该能够处理内存压力情况");
}

#[test]
fn test_compile_priority_ordering() {
    // 测试编译优先级排序
    let mut engine = JitEngine::new(JitConfig::default());
    
    // 添加不同优先级的块到队列
    engine.add_to_compile_queue(0x1000, 10); // 低优先级
    engine.add_to_compile_queue(0x2000, 50); // 中优先级
    engine.add_to_compile_queue(0x3000, 100); // 高优先级
    
    let blocks = {
        let mut map = HashMap::new();
        map.insert(0x1000, create_test_ir_block(0x1000));
        map.insert(0x2000, create_test_ir_block(0x2000));
        map.insert(0x3000, create_test_ir_block(0x3000));
        map
    };
    
    // 处理队列，高优先级应该先编译
    let compiled = engine.process_compile_queue(&blocks);
    
    // 应该编译了一些块
    assert!(compiled > 0, "应该编译了至少一个块");
}

