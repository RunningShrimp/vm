//! 异步执行引擎性能测试
//!
//! 测试异步执行引擎的性能，对比同步和异步执行的性能差异

use vm_core::GuestAddr;
use vm_engine_interpreter::async_executor_integration::{
    AsyncExecutorWrapper, benchmark_async_vs_sync,
};
use vm_ir::{IRBuilder, IROp, Terminator};
use vm_mem::SoftMmu;

#[tokio::test]
async fn test_async_executor_basic_performance() {
    use tokio::time::{Duration, timeout};

    let test_future = async {
        let mut mmu = SoftMmu::new(1024 * 1024, false);

        // 创建一个简单的 IR 块
        let mut builder = IRBuilder::new(GuestAddr(0x1000));
        for i in 0..20 {
            builder.push(IROp::MovImm {
                dst: i,
                imm: i as u64,
            });
            builder.push(IROp::Add {
                dst: i,
                src1: i,
                src2: (i + 1) % 20,
            });
        }
        builder.set_term(Terminator::Ret);
        let block = builder.build();

        // 测试异步执行
        let executor = AsyncExecutorWrapper::new(10);
        let start = std::time::Instant::now();

        for _ in 0..100 {
            let _ = executor.execute_block_async(&mut mmu, &block).await;
        }

        let async_time = start.elapsed();
        assert!(async_time.as_millis() > 0);
    };

    timeout(Duration::from_secs(60), test_future)
        .await
        .expect("异步执行器基本性能测试超时（超过60秒）");
}

#[tokio::test]
async fn test_async_vs_sync_performance_comparison() {
    use tokio::time::{Duration, timeout};

    let test_future = async {
        let mut mmu = SoftMmu::new(1024 * 1024, false);

        // 创建一个中等复杂度的 IR 块
        let mut builder = IRBuilder::new(GuestAddr(0x1000));
        for i in 0..50 {
            builder.push(IROp::MovImm {
                dst: i % 32,
                imm: i as u64,
            });
            if i > 0 {
                builder.push(IROp::Add {
                    dst: (i - 1) % 32,
                    src1: (i - 1) % 32,
                    src2: i % 32,
                });
            }
        }
        builder.set_term(Terminator::Ret);
        let block = builder.build();

        // 执行性能对比测试
        let comparison = benchmark_async_vs_sync(&mut mmu, &block, 50, 10).await;

        // 验证结果
        assert!(comparison.sync_time_us > 0);
        assert!(comparison.async_time_us > 0);
        assert_eq!(comparison.sync_ops, comparison.async_ops);

        // 打印性能对比结果
        println!("=== 性能对比结果 ===");
        println!("同步执行时间: {} 微秒", comparison.sync_time_us);
        println!("异步执行时间: {} 微秒", comparison.async_time_us);
        println!("性能提升: {:.2}%", comparison.improvement_percent);
        println!("执行操作数: {}", comparison.sync_ops);
    };

    timeout(Duration::from_secs(90), test_future)
        .await
        .expect("异步vs同步性能对比测试超时（超过90秒）");
}

#[tokio::test]
async fn test_async_executor_yield_behavior() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    // 创建一个较大的 IR 块
    let mut builder = IRBuilder::new(GuestAddr(0x1000));
    for i in 0..100 {
        builder.push(IROp::MovImm {
            dst: i % 32,
            imm: i as u64,
        });
    }
    builder.set_term(Terminator::Ret);
    let block = builder.build();

    // 测试不同的 yield 间隔
    for yield_interval in [10, 50, 100] {
        let executor = AsyncExecutorWrapper::new(yield_interval);
        let start = std::time::Instant::now();

        let result = executor.run_steps_async(&mut mmu, &block, 1000).await;
        assert!(result.is_ok());

        let elapsed = start.elapsed();
        println!(
            "Yield 间隔 {}: {} 微秒",
            yield_interval,
            elapsed.as_micros()
        );
    }
}

#[tokio::test]
async fn test_async_executor_stats() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    let mut builder = IRBuilder::new(GuestAddr(0x1000));
    builder.push(IROp::MovImm { dst: 0, imm: 42 });
    builder.set_term(Terminator::Ret);
    let block = builder.build();

    let executor = AsyncExecutorWrapper::new(10);

    // 执行几次
    for _ in 0..10 {
        let _ = executor.execute_block_async(&mut mmu, &block).await;
    }

    // 获取统计信息
    let stats = executor.stats();
    assert!(stats.async_ops > 0);

    // 重置统计
    executor.reset_stats();
    let stats_after_reset = executor.stats();
    assert_eq!(stats_after_reset.async_ops, 0);
}

#[tokio::test]
async fn test_async_executor_concurrent_execution() {
    use tokio::time::{Duration, timeout};

    let test_future = async {
        let mut _mmu = SoftMmu::new(1024 * 1024, false);

        let mut builder = IRBuilder::new(GuestAddr(0x1000));
        for i in 0..10 {
            builder.push(IROp::MovImm {
                dst: i,
                imm: i as u64,
            });
        }
        builder.set_term(Terminator::Ret);
        let block = builder.build();

        // 创建多个执行器并发执行
        let mut handles = Vec::new();

        for i in 0..5 {
            let executor = AsyncExecutorWrapper::new(10);
            let mut local_mmu = SoftMmu::new(1024 * 1024, false);
            let block_clone = block.clone();

            handles.push(tokio::spawn(async move {
                let start = std::time::Instant::now();
                for _ in 0..20 {
                    let _ = executor
                        .execute_block_async(&mut local_mmu, &block_clone)
                        .await;
                }
                (i, start.elapsed())
            }));
        }

        // 等待所有任务完成
        let mut results = Vec::new();
        for handle in handles {
            if let Ok(result) = handle.await {
                results.push(result);
            }
        }

        assert_eq!(results.len(), 5);
        for (id, elapsed) in results {
            println!("执行器 {} 完成，耗时: {:?}", id, elapsed);
            assert!(elapsed.as_millis() > 0);
        }
    };

    timeout(Duration::from_secs(120), test_future)
        .await
        .expect("异步执行器并发执行测试超时（超过120秒）");
}
