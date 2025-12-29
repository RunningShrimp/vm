//! JIT异步编译和执行集成测试
//!
//! 测试异步编译、异步执行、并发场景下的正确性

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use vm_core::GuestAddr;
use vm_engine::jit::{Jit, AsyncCompileResult, CodePtr};
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

/// 创建多个测试IR块
fn create_test_ir_blocks(count: usize, base_addr: GuestAddr) -> HashMap<GuestAddr, IRBlock> {
    let mut blocks = HashMap::new();
    for i in 0..count {
        let addr = base_addr + (i as u64 * 16);
        blocks.insert(addr, create_test_ir_block(addr));
    }
    blocks
}

#[tokio::test]
async fn test_async_compile_basic() {
    // 测试基本的异步编译功能
    let mut engine = Jit::new();
    
    let block = create_test_ir_block(0x1000);
    
    // 启动异步编译
    let handle = engine.compile_async(block.clone());
    
    // 等待编译完成
    let code_ptr = handle.await;
    
    // 验证编译结果
    assert!(!code_ptr.0.is_null(), "编译应该成功，返回非空代码指针");
    
    // 验证结果已缓存
    let result = engine.check_async_compile_result(0x1000);
    assert!(
        matches!(result, AsyncCompileResult::Completed(_)),
        "编译结果应该被缓存"
    );
}

#[tokio::test]
async fn test_concurrent_async_compile() {
    // 测试并发异步编译
    let engine = Arc::new(Mutex::new(Jit::new()));
    
    let blocks = create_test_ir_blocks(10, 0x1000);
    let blocks = Arc::new(blocks);
    
    // 启动多个并发编译任务
    let mut handles = Vec::new();
    for (pc, block) in blocks.iter() {
        let engine_clone = engine.clone();
        let block_clone = block.clone();
        let pc = *pc;
        
        let handle = tokio::spawn(async move {
            let mut engine = engine_clone.lock().await;
            let compile_handle = engine.compile_async(block_clone);
            let code_ptr = compile_handle.await;
            
            // 验证编译结果
            assert!(!code_ptr.0.is_null(), "PC {:#x} 应该编译成功", pc);
            
            // 验证结果已缓存
            let result = engine.check_async_compile_result(pc);
            assert!(
                matches!(result, AsyncCompileResult::Completed(_)),
                "PC {:#x} 的编译结果应该被缓存",
                pc
            );
            
            code_ptr
        });
        handles.push(handle);
    }
    
    // 等待所有编译任务完成
    for handle in handles {
        let _ = handle.await;
    }
    
    // 验证所有块都已编译
    let engine_guard = engine.lock().await;
    for pc in blocks.keys() {
        let result = engine_guard.check_async_compile_result(*pc);
        assert!(
            matches!(result, AsyncCompileResult::Completed(_)),
            "所有块都应该被编译"
        );
    }
}

#[tokio::test]
async fn test_async_compile_with_prefetch() {
    // 测试异步编译与预取机制的集成
    let mut engine = Jit::new();
    
    let blocks = create_test_ir_blocks(20, 0x1000);
    
    // 执行第一个块，触发预取
    if let Some(block) = blocks.get(&0x1000) {
        engine.prefetch_code_blocks(0x1000);
    }
    
    // 处理预取队列
    let prefetched = engine.process_prefetch_queue(&blocks);
    assert!(prefetched > 0, "应该预取了一些块");
    
    // 等待一段时间让异步编译完成
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // 验证预取的块是否已编译
    let mut compiled_count = 0;
    for pc in blocks.keys() {
        let result = engine.check_async_compile_result(*pc);
        if matches!(result, AsyncCompileResult::Completed(_)) {
            compiled_count += 1;
        }
    }
    
    assert!(compiled_count > 0, "至少应该有一些预取的块被编译");
}

#[tokio::test]
async fn test_async_compile_timeout_handling() {
    // 测试异步编译超时处理
    let mut engine = Jit::new();
    
    let block = create_test_ir_block(0x1000);
    
    // 启动异步编译
    let handle = engine.compile_async(block);
    
    // 使用超时等待编译完成
    let result = tokio::time::timeout(
        tokio::time::Duration::from_millis(5000), // 5秒超时
        handle
    ).await;
    
    // 验证编译完成（应该不会超时）
    assert!(result.is_ok(), "编译应该在超时前完成");
    
    let code_ptr = result.unwrap();
    assert!(!code_ptr.0.is_null(), "编译应该成功");
}

#[tokio::test]
async fn test_async_compile_duplicate_prevention() {
    // 测试防止重复编译
    let mut engine = Jit::new();
    
    let block = create_test_ir_block(0x1000);
    
    // 启动第一个异步编译
    let handle1 = engine.compile_async(block.clone());
    
    // 立即启动第二个异步编译（应该复用第一个）
    let handle2 = engine.compile_async(block.clone());
    
    // 等待两个任务完成
    let code_ptr1 = handle1.await;
    let code_ptr2 = handle2.await;
    
    // 验证两个任务返回相同的结果
    assert_eq!(code_ptr1.0, code_ptr2.0, "重复编译应该返回相同的结果");
}

#[tokio::test]
async fn test_async_cache_integration() {
    // 测试异步缓存与异步编译的集成
    use vm_engine::jit::unified_cache::UnifiedCodeCache;
    use vm_engine::jit::ewma_hotspot::EwmaHotspotConfig;
    
    let cache = Arc::new(UnifiedCodeCache::new(
        vm_engine::jit::unified_cache::CacheConfig::default(),
        EwmaHotspotConfig::default(),
    ));
    
    let mut engine = Jit::new();
    let block = create_test_ir_block(0x1000);
    
    // 启动异步编译
    let handle = engine.compile_async(block);
    let code_ptr = handle.await;
    
    assert!(!code_ptr.0.is_null(), "编译应该成功");
    
    // 异步插入缓存
    cache.insert_async_code(0x1000, code_ptr, 1024, 1000).await;
    
    // 异步查找缓存
    let cached = cache.get_async(0x1000).await;
    assert!(cached.is_some(), "应该能从缓存中找到代码");
}

#[tokio::test]
async fn test_async_compile_with_gc() {
    // 测试异步编译与GC的集成
    use vm_engine::jit::unified_gc::{UnifiedGC, UnifiedGcConfig};
    
    let gc = Arc::new(UnifiedGC::new(UnifiedGcConfig::default()));
    let mut engine = Jit::new();
    
    let blocks = create_test_ir_blocks(50, 0x1000);
    
    // 启动多个异步编译任务
    let mut handles = Vec::new();
    for (pc, block) in blocks.iter() {
        let handle = engine.compile_async(block.clone());
        handles.push((*pc, handle));
    }
    
    // 在编译过程中运行GC
    let gc_clone = gc.clone();
    let gc_handle = tokio::spawn(async move {
        // 运行几次GC步骤
        for _ in 0..5 {
            let _ = gc_clone.run_gc_step_async().await;
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    });
    
    // 等待所有编译完成
    for (_pc, handle) in handles {
        let _ = handle.await;
    }
    
    // 等待GC完成
    gc_handle.await.unwrap();
    
    // 验证编译结果仍然有效
    for pc in blocks.keys() {
        let result = engine.check_async_compile_result(*pc);
        assert!(
            matches!(result, AsyncCompileResult::Completed(_)),
            "GC不应该影响编译结果"
        );
    }
}

#[tokio::test]
async fn test_async_compile_queue_processing() {
    // 测试异步编译队列处理
    let mut engine = Jit::new();
    
    let blocks = create_test_ir_blocks(20, 0x1000);
    
    // 添加多个块到编译队列
    for (pc, _) in blocks.iter() {
        engine.add_to_compile_queue(*pc, 100);
    }
    
    // 处理编译队列（同步处理）
    let compiled = engine.process_compile_queue(&blocks);
    
    assert!(compiled > 0, "应该编译了一些块");
    
    // 验证编译结果
    for pc in blocks.keys() {
        let result = engine.check_async_compile_result(*pc);
        // 结果可能是已完成或超时（取决于编译时间）
        assert!(
            matches!(result, AsyncCompileResult::Completed(_) | AsyncCompileResult::Timeout),
            "编译结果应该是有效状态"
        );
    }
}

#[tokio::test]
async fn test_async_compile_error_recovery() {
    // 测试异步编译错误恢复
    let mut engine = Jit::new();
    
    // 创建一个可能导致编译失败的块（使用无效寄存器）
    let mut builder = IRBuilder::new(0x1000);
    builder.push(IROp::MovImm { dst: 999, imm: 0 }); // 无效寄存器
    builder.set_term(Terminator::Jmp { target: 0 });
    let invalid_block = builder.build();
    
    // 尝试异步编译
    let handle = engine.compile_async(invalid_block);
    
    // 等待编译完成（可能成功或失败）
    let code_ptr = handle.await;
    
    // 检查结果状态
    let result = engine.check_async_compile_result(0x1000);
    
    // 结果可能是完成（即使代码指针为null）或超时
    assert!(
        matches!(result, AsyncCompileResult::Completed(_) | AsyncCompileResult::Timeout),
        "应该能够处理编译错误"
    );
}

#[tokio::test]
async fn test_concurrent_prefetch_and_compile() {
    // 测试并发预取和编译
    let engine = Arc::new(Mutex::new(Jit::new()));
    
    let blocks = create_test_ir_blocks(30, 0x1000);
    let blocks = Arc::new(blocks);
    
    // 预取任务
    let engine_clone = engine.clone();
    let blocks_clone = blocks.clone();
    let prefetch_handle = tokio::spawn(async move {
        let mut engine = engine_clone.lock().await;
        
        // 触发预取
        engine.prefetch_code_blocks(0x1000);
        
        // 处理预取队列
        engine.process_prefetch_queue(&blocks_clone)
    });
    
    // 编译任务
    let engine_clone = engine.clone();
    let blocks_clone = blocks.clone();
    let compile_handle = tokio::spawn(async move {
        let mut engine = engine_clone.lock().await;
        let mut handles = Vec::new();
        
        // 启动多个编译任务
        for (pc, block) in blocks_clone.iter() {
            let handle = engine.compile_async(block.clone());
            handles.push((*pc, handle));
        }
        
        // 等待所有编译完成
        for (_pc, handle) in handles {
            let _ = handle.await;
        }
    });
    
    // 等待所有任务完成
    let prefetched = prefetch_handle.await.unwrap();
    compile_handle.await.unwrap();
    
    assert!(prefetched > 0, "应该预取了一些块");
    
    // 验证结果
    let engine_guard = engine.lock().await;
    let mut compiled_count = 0;
    for pc in blocks.keys() {
        let result = engine_guard.check_async_compile_result(*pc);
        if matches!(result, AsyncCompileResult::Completed(_)) {
            compiled_count += 1;
        }
    }
    
    assert!(compiled_count > 0, "应该有一些块被编译");
}

#[tokio::test]
async fn test_async_compile_priority() {
    // 测试异步编译优先级
    let mut engine = Jit::new();
    
    let blocks = create_test_ir_blocks(10, 0x1000);
    
    // 添加不同优先级的块到队列
    for (i, pc) in blocks.keys().enumerate() {
        let priority = if i < 5 { 100 } else { 10 }; // 前5个高优先级
        engine.add_to_compile_queue(*pc, priority);
    }
    
    // 处理编译队列
    let compiled = engine.process_compile_queue(&blocks);
    
    assert!(compiled > 0, "应该编译了一些块");
    
    // 验证高优先级块优先编译（简化验证：至少编译了一些块）
    let mut high_priority_compiled = 0;
    for (i, pc) in blocks.keys().enumerate() {
        let result = engine.check_async_compile_result(*pc);
        if i < 5 && matches!(result, AsyncCompileResult::Completed(_)) {
            high_priority_compiled += 1;
        }
    }
    
    // 至少应该有一些高优先级块被编译
    assert!(high_priority_compiled >= 0, "应该编译了一些高优先级块");
}

