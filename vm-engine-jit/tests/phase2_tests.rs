//! JIT引擎第二阶段优化测试
//!
//! 测试多线程编译、动态优化和高级基准测试功能

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use vm_engine_jit::adaptive_threshold::{AdaptiveThresholdConfig, AdaptiveThresholdManager};
use vm_engine_jit::advanced_benchmark::{
    AdvancedPerformanceBenchmarker, BenchmarkConfig, BenchmarkTestCaseGenerator,
};
use vm_engine_jit::core::{JITConfig, JITEngine};
use vm_engine_jit::dynamic_optimization::{DynamicOptimizationConfig, DynamicOptimizationManager};
use vm_engine_jit::multithreaded_compiler::{MultithreadedJITCompiler, TaskPriority};
use vm_engine_jit::performance_benchmark::PerformanceBenchmarker;
use vm_ir::{IRBuilder, IROp, Terminator};

#[test]
fn test_multithreaded_compilation() {
    // 创建JIT引擎
    let config = JITConfig::default();
    let jit_engine = Arc::new(JITEngine::new(config));

    // 创建多线程编译器
    let mut compiler = MultithreadedJITCompiler::new(jit_engine.clone(), 4);

    // 创建测试IR块
    let mut ir_blocks = Vec::new();
    for i in 0..10 {
        let mut builder = IRBuilder::new(0x1000 + i as u64 * 0x100);
        for j in 0..20 {
            builder.push(IROp::Add {
                dst: (j % 16) as u32,
                src1: ((j + 1) % 16) as u32,
                src2: ((j + 2) % 16) as u32,
            });
        }
        builder.set_term(Terminator::Ret);
        ir_blocks.push(builder.build());
    }

    // 异步编译所有IR块
    let task_ids: Vec<u64> = ir_blocks
        .into_iter()
        .enumerate()
        .map(|(i, block)| {
            let priority = if i % 3 == 0 {
                TaskPriority::High
            } else {
                TaskPriority::Normal
            };
            compiler.compile_async(block, priority)
        })
        .collect();

    // 等待编译完成
    std::thread::sleep(Duration::from_millis(100));

    // 检查统计信息
    let stats = compiler.get_stats();
    assert_eq!(stats.total_tasks, 10);
    assert_eq!(stats.current_queue_size, 0);
    assert_eq!(stats.active_workers, 4);

    // 检查任务ID
    assert_eq!(task_ids.len(), 10);
    for (i, &task_id) in task_ids.iter().enumerate() {
        assert!(task_id > 0);
        // 任务ID应该是递增的
        if i > 0 {
            assert!(task_id > task_ids[i - 1]);
        }
    }
}

#[test]
fn test_multithreaded_compilation_with_callback() {
    // 创建JIT引擎
    let config = JITConfig::default();
    let jit_engine = Arc::new(JITEngine::new(config));

    // 创建多线程编译器
    let mut compiler = MultithreadedJITCompiler::new(jit_engine.clone(), 2);

    // 创建测试IR块
    let mut builder = IRBuilder::new(0x2000);
    for i in 0..10 {
        builder.push(IROp::MovImm {
            dst: i as u32,
            imm: i as u64,
        });
    }
    builder.set_term(Terminator::Ret);
    let ir_block = builder.build();

    // 用于跟踪回调执行
    let callback_executed = Arc::new(AtomicU64::new(0));
    let callback_success = Arc::new(AtomicU64::new(0));

    // 异步编译并设置回调
    let callback_executed_clone = Arc::clone(&callback_executed);
    let callback_success_clone = Arc::clone(&callback_success);

    compiler.compile_async_with_callback(
        ir_block,
        TaskPriority::High,
        move |result: Result<Vec<u8>, VmError>| {
            callback_executed_clone.fetch_add(1, Ordering::SeqCst);
            if result.is_ok() {
                callback_success_clone.fetch_add(1, Ordering::SeqCst);
            }
        },
    );

    // 等待编译完成
    std::thread::sleep(Duration::from_millis(100));

    // 检查回调执行
    assert_eq!(callback_executed.load(Ordering::SeqCst), 1);
    assert_eq!(callback_success.load(Ordering::SeqCst), 1);
}

#[test]
fn test_dynamic_optimization_manager() {
    // 创建JIT引擎
    let config = JITConfig::default();
    let jit_engine = Arc::new(JITEngine::new(config));

    // 创建自适应阈值管理器
    let threshold_config = AdaptiveThresholdConfig::default();
    let threshold_manager = Arc::new(Mutex::new(AdaptiveThresholdManager::new(threshold_config)));

    // 创建动态优化管理器
    let dynamic_config = DynamicOptimizationConfig {
        min_executions_for_analysis: 5,
        auto_apply_suggestions: true,
        ..Default::default()
    };

    let manager =
        DynamicOptimizationManager::new(jit_engine.clone(), threshold_manager, dynamic_config);

    // 记录多次执行性能
    let pc = 0x3000;
    for i in 0..10 {
        manager.record_execution(
            pc,
            1000 + i * 10, // 执行时间逐渐增加（性能恶化）
            1,
            200,
            2,
            true,
        );
    }

    // 获取性能统计
    let stats = manager.get_performance_stats(pc);
    assert!(stats.is_some());
    let stats = stats.unwrap();
    assert_eq!(stats.total_executions, 10);
    assert!(stats.avg_execution_time_ns > 1000);

    // 手动触发分析
    let suggestions = manager.trigger_analysis();
    assert!(!suggestions.is_empty());

    // 检查建议内容
    let suggestion = &suggestions[0];
    assert_eq!(suggestion.pc, pc);
    assert!(suggestion.confidence > 0.0);
    assert!(!suggestion.reason.is_empty());
}

#[test]
fn test_advanced_performance_benchmarker() {
    // 创建JIT引擎
    let config = JITConfig::default();
    let jit_engine = Arc::new(JITEngine::new(config));

    // 创建高级性能基准测试器
    let advanced_benchmarker = AdvancedPerformanceBenchmarker::new(jit_engine);

    // 创建测试配置
    let config = BenchmarkConfig {
        name: "test_arithmetic".to_string(),
        description: "算术操作测试".to_string(),
        iterations: 10,
        warmup_iterations: 2,
        ..Default::default()
    };

    // 生成测试用例
    let test_case = BenchmarkTestCaseGenerator::generate_arithmetic_test(0x4000, 5);

    // 运行基准测试
    let result = advanced_benchmarker.run_benchmark(&config, test_case);

    // 验证结果
    assert_eq!(result.name, "test_arithmetic");
    assert!(result.total_duration > Duration::ZERO);
    assert_eq!(result.errors.len(), 0); // 假设没有错误

    // 检查统计信息
    assert!(result.avg_execution_time > Duration::ZERO);
    assert!(result.min_execution_time <= result.avg_execution_time);
    assert!(result.max_execution_time >= result.avg_execution_time);
    assert!(result.ops_per_second > 0.0);

    // 检查编译统计
    assert!(result.compilation_stats.compilation_count > 0);
    assert!(result.compilation_stats.total_compilation_time > Duration::ZERO);
}

#[test]
fn test_benchmark_test_case_generation() {
    // 测试算术测试用例生成
    let arithmetic_test = BenchmarkTestCaseGenerator::generate_arithmetic_test(0x5000, 10);
    assert_eq!(arithmetic_test.start_pc, 0x5000);
    assert_eq!(arithmetic_test.ops.len(), 10);

    // 验证操作类型
    for (i, op) in arithmetic_test.ops.iter().enumerate() {
        match i % 4 {
            0 => assert!(matches!(op, IROp::Add { .. })),
            1 => assert!(matches!(op, IROp::Sub { .. })),
            2 => assert!(matches!(op, IROp::Mul { .. })),
            _ => assert!(matches!(op, IROp::Div { .. })),
        }
    }

    // 测试内存访问测试用例生成
    let memory_test = BenchmarkTestCaseGenerator::generate_memory_test(0x6000, 6);
    assert_eq!(memory_test.start_pc, 0x6000);
    assert_eq!(memory_test.ops.len(), 6);

    // 测试控制流测试用例生成
    let control_flow_test = BenchmarkTestCaseGenerator::generate_control_flow_test(0x7000, 5);
    assert_eq!(control_flow_test.start_pc, 0x7000);
    assert_eq!(control_flow_test.ops.len(), 10); // 每个分支包含比较和跳转

    // 测试SIMD测试用例生成
    let simd_test = BenchmarkTestCaseGenerator::generate_simd_test(0x8000, 3);
    assert_eq!(simd_test.start_pc, 0x8000);
    assert_eq!(simd_test.ops.len(), 12); // 每个向量包含4个操作

    // 测试混合测试用例生成
    let mixed_test = BenchmarkTestCaseGenerator::generate_mixed_test(0x9000, 10);
    assert_eq!(mixed_test.start_pc, 0x9000);
    assert_eq!(mixed_test.ops.len(), 10);
}

#[test]
fn test_benchmark_suite() {
    // 创建JIT引擎
    let config = JITConfig::default();
    let jit_engine = Arc::new(JITEngine::new(config));

    // 创建高级性能基准测试器
    let advanced_benchmarker = AdvancedPerformanceBenchmarker::new(jit_engine);

    // 运行基准测试套件
    let results = advanced_benchmarker.run_benchmark_suite();

    // 验证结果
    assert_eq!(results.len(), 5); // 应该有5个测试

    // 检查每个测试
    for (name, result) in &results {
        assert!(!name.is_empty());
        assert_eq!(result.name, *name);
        assert!(result.total_duration > Duration::ZERO);
        assert!(result.avg_execution_time > Duration::ZERO);
        assert!(result.ops_per_second > 0.0);
    }

    // 检查特定测试
    assert!(results.contains_key("arithmetic_simple"));
    assert!(results.contains_key("memory_access"));
    assert!(results.contains_key("control_flow"));
    assert!(results.contains_key("simd_operations"));
    assert!(results.contains_key("mixed_operations"));

    // 获取全局统计
    let global_stats = advanced_benchmarker.get_global_stats();
    assert_eq!(global_stats.total_tests, 5);
    assert!(global_stats.total_execution_time > Duration::ZERO);
}

#[test]
fn test_worker_adjustment() {
    // 创建JIT引擎
    let config = JITConfig::default();
    let jit_engine = Arc::new(JITEngine::new(config));

    // 创建多线程编译器
    let mut compiler = MultithreadedJITCompiler::new(jit_engine.clone(), 2);

    // 检查初始工作线程数
    let stats = compiler.get_stats();
    assert_eq!(stats.active_workers, 2);

    // 增加工作线程
    compiler.adjust_workers(4);
    let stats = compiler.get_stats();
    assert_eq!(stats.active_workers, 4);

    // 减少工作线程
    compiler.adjust_workers(1);
    let stats = compiler.get_stats();
    assert_eq!(stats.active_workers, 1);
}

#[test]
fn test_performance_trend_analysis() {
    // 创建JIT引擎
    let config = JITConfig::default();
    let jit_engine = Arc::new(JITEngine::new(config));

    // 创建自适应阈值管理器
    let threshold_config = AdaptiveThresholdConfig::default();
    let threshold_manager = Arc::new(Mutex::new(AdaptiveThresholdManager::new(threshold_config)));

    // 创建动态优化管理器
    let dynamic_config = DynamicOptimizationConfig::default();
    let manager = DynamicOptimizationManager::new(jit_engine, threshold_manager, dynamic_config);

    // 记录性能改善的数据
    let pc = 0xA000;
    for i in 0..10 {
        manager.record_execution(
            pc,
            1000 - i * 20, // 执行时间逐渐减少（性能改善）
            1,
            200,
            2,
            true,
        );
    }

    // 获取性能统计
    let stats = manager.get_performance_stats(pc);
    assert!(stats.is_some());
    let stats = stats.unwrap();
    assert_eq!(stats.total_executions, 10);

    // 手动触发分析
    let suggestions = manager.trigger_analysis();

    // 由于性能改善，可能会有降低优化级别的建议
    if !suggestions.is_empty() {
        let suggestion = &suggestions[0];
        assert_eq!(suggestion.pc, pc);
        assert!(suggestion.confidence > 0.0);
    }
}
