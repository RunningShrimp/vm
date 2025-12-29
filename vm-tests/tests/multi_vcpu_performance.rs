//! 多vCPU并行执行性能测试
//!
//! 测试多vCPU执行器的并行性能和扩展效率

use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use vm_core::{ExecResult, ExecutionEngine, MMU};
use vm_engine_interpreter::Interpreter;
use vm_ir::{IRBuilder, IROp, MemFlags};
use vm_mem::SoftMmu;

use super::setup_utils::create_simple_ir_block;

/// 创建一个简单的执行引擎用于测试
fn create_test_engine() -> Box<dyn ExecutionEngine<vm_ir::IRBlock>> {
    Box::new(Interpreter::new())
}

/// 创建多vCPU执行器（使用默认配置）
fn create_executor<B: 'static + Send + Sync + Clone>(
    vcpu_count: u32,
    mmu_factory: impl Fn() -> Box<dyn MMU> + Clone + 'static,
    engine_factory: impl Fn() -> Box<dyn ExecutionEngine<B>> + Clone + 'static,
) -> vm_core::parallel::MultiVcpuExecutor<B> {
    vm_core::parallel::MultiVcpuExecutor::new(vcpu_count, mmu_factory, engine_factory)
}

/// 测试不同vCPU数量下的性能扩展
#[tokio::test]
async fn test_multi_vcpu_scaling() {
    // 测试不同数量的vCPU
    let vcpu_counts = [1, 2, 4, 8];

    for &vcpu_count in vcpu_counts.iter() {
        println!("Testing with {} vCPUs...", vcpu_count);

        let start = Instant::now();

        // 创建MMU工厂函数
        let mmu_factory = || Box::new(SoftMmu::new(64 * 1024 * 1024)) as Box<dyn MMU>;

        // 为每个vCPU创建相同的IR块（简单的算术运算）
        let blocks: Vec<vm_ir::IRBlock> = (0..vcpu_count)
            .map(|i| create_simple_ir_block(0x1000 + i as u64 * 0x100))
            .collect();

        // 创建多vCPU执行器
        let executor = create_executor(vcpu_count, mmu_factory, create_test_engine);

        // 执行并行运行（使用异步版本）
        let results = match executor.run_parallel_async(&blocks).await {
            Ok(results) => results,
            Err(e) => {
                panic!("Failed to run parallel execution: {}", e);
            }
        };

        let duration = start.elapsed();

        // 验证结果
        assert_eq!(
            results.len(),
            vcpu_count as usize,
            "Should have results for all vCPUs"
        );

        // 验证每个vCPU都成功执行
        for (i, result) in results.iter().enumerate() {
            assert!(
                matches!(result.status, vm_core::ExecStatus::Ok),
                "vCPU {} failed execution: {:?}",
                i,
                result.status
            );
        }

        // 打印性能指标
        let total_ops: u64 = results.iter().map(|r| r.stats.executed_insns).sum();

        println!(
            "  vCPUs: {}, Duration: {:?}, Total ops: {}, Ops/sec: {:.2}",
            vcpu_count,
            duration,
            total_ops,
            total_ops as f64 / duration.as_secs_f64()
        );

        // 性能断言：多vCPU应该比单vCPU快（至少不慢太多）
        if vcpu_count > 1 {
            let ops_per_vcpu = total_ops as f64 / vcpu_count as f64;
            assert!(
                ops_per_vcpu > 1000.0,
                "Each vCPU should execute at least 1000 ops/sec"
            );
        }
    }
}

/// 测试负载均衡策略的效果
#[test]
fn test_load_balancing_strategies() {
    use vm_core::parallel::{LoadBalancePolicy, VcpuLoadBalancer};

    let policies = [
        LoadBalancePolicy::RoundRobin,
        LoadBalancePolicy::LeastLoaded,
        LoadBalancePolicy::WeightedRoundRobin,
    ];

    for policy in policies.iter() {
        let mut balancer = VcpuLoadBalancer::new(4, *policy);

        // 模拟负载
        balancer.update_load(0, 1000); // vCPU 0 高负载
        balancer.update_load(1, 500); // vCPU 1 中负载
        balancer.update_load(2, 200); // vCPU 2 低负载
        balancer.update_load(3, 100); // vCPU 3 最低负载

        let mut selected_counts = std::collections::HashMap::new();

        // 测试选择分布
        for _ in 0..1000 {
            let selected = balancer.select_vcpu();
            *selected_counts.entry(selected).or_insert(0) += 1;
        }

        // 验证负载均衡效果
        println!("Load Balancing Policy: {:?}", policy);
        for (vcpu_id, count) in selected_counts.iter() {
            println!("  vCPU {}: {} selections", vcpu_id, count);
        }

        // RoundRobin应该均匀分布
        if *policy == LoadBalancePolicy::RoundRobin {
            let min_count = selected_counts.values().min().unwrap();
            let max_count = selected_counts.values().max().unwrap();
            assert!(
                max_count - min_count <= 1,
                "RoundRobin should distribute evenly"
            );
        }
    }
}

/// 测试并行执行的一致性
#[tokio::test]
async fn test_parallel_consistency() {
    let vcpu_count = 4;

    // 创建MMU工厂函数
    let mmu_factory = || Box::new(SoftMmu::new(16 * 1024 * 1024)) as Box<dyn MMU>;

    // 创建带有内存共享操作的IR块
    let blocks: Vec<vm_ir::IRBlock> = (0..vcpu_count)
        .map(|i| {
            let mut builder = IRBuilder::new(0x1000 + i as u64 * 0x100);
            let base_addr = 0x10000 + i as u64 * 0x1000;

            builder.push(IROp::Load {
                dst: 0,
                base: base_addr as u32,
                size: 8,
                offset: 0,
            });
            builder.push(IROp::Add {
                dst: 0,
                src1: 0,
                src2: 1,
            });
            builder.push(IROp::Store {
                base: base_addr as u32,
                src: 0,
                size: 8,
                offset: 0,
            });

            builder.build()
        })
        .collect();

    // 创建多vCPU执行器
    let executor = create_executor(vcpu_count, mmu_factory, create_test_engine);

    // 执行并行运行
    let results = executor.run_parallel_async(&blocks).await.unwrap();

    // 验证结果
    assert_eq!(results.len(), vcpu_count as usize);
}

/// 压力测试大量vCPU
#[tokio::test]
async fn test_many_vcpu_stress() {
    let large_vcpu_count = 16;

    let start = Instant::now();

    // 创建MMU工厂函数
    let mmu_factory = || Box::new(SoftMmu::new(256 * 1024 * 1024)) as Box<dyn MMU>;

    // 创建简单的IR块
    let blocks: Vec<vm_ir::IRBlock> = (0..large_vcpu_count)
        .map(|i| create_simple_ir_block(0x1000 + i as u64 * 0x100))
        .collect();

    // 创建大容量多vCPU执行器
    let executor = create_executor(large_vcpu_count, mmu_factory, create_test_engine);

    // 执行压力测试
    let results = executor.run_parallel_async(&blocks).await.unwrap();

    let duration = start.elapsed();

    // 验证结果
    assert_eq!(results.len(), large_vcpu_count as usize);

    // 计算性能指标
    let total_ops: u64 = results.iter().map(|r| r.stats.executed_insns).sum();

    println!(
        "Stress Test - {} vCPUs: Duration: {:?}, Total ops: {}, Ops/sec: {:.2}",
        large_vcpu_count,
        duration,
        total_ops,
        total_ops as f64 / duration.as_secs_f64()
    );

    // 性能断言
    let ops_per_vcpu_per_sec =
        total_ops as f64 / (large_vcpu_count as f64) / duration.as_secs_f64();
    assert!(
        ops_per_vcpu_per_sec > 500.0,
        "Each vCPU should maintain at least 500 ops/sec under stress"
    );
}
