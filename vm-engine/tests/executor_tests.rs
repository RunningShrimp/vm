//! Executor模块集成测试
//!
//! 测试执行器、协程和调度器的各种功能和场景

use vm_engine::executor::{
    AsyncExecutionContext, Coroutine, CoroutineId, CoroutineState, ExecutionResult, ExecutionStats,
    ExecutorType, HybridExecutor, InterpreterExecutor, JitExecutor, Scheduler, VCPU, VCPUState,
    VCPUStats,
};

// ============================================================================
// 基础执行器测试 (测试1-10)
// ============================================================================

#[cfg(test)]
mod jit_executor_tests {
    use super::*;

    /// 测试1: JIT执行器基本执行
    #[test]
    fn test_jit_single_execution() {
        let mut executor = JitExecutor::new();
        let result = executor.execute_block(1);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);

        let stats = executor.get_stats();
        assert_eq!(stats.total_executions, 1);
        assert_eq!(stats.cached_blocks, 1);
    }

    /// 测试2: JIT执行器缓存效益
    #[test]
    fn test_jit_caching_benefit() {
        let mut executor = JitExecutor::new();

        // 第一次执行会编译
        let _result1 = executor.execute_block(1);
        let stats1 = executor.get_stats();

        // 第二次执行使用缓存(更快)
        let _result2 = executor.execute_block(1);
        let stats2 = executor.get_stats();

        // 验证缓存工作
        assert_eq!(stats2.cached_blocks, 1);
        assert_eq!(stats2.total_executions, 2);
        assert!(stats2.avg_time_us < stats1.avg_time_us);
    }

    /// 测试3: JIT执行器批量执行
    #[test]
    fn test_jit_batch_execution() {
        let mut executor = JitExecutor::new();
        let block_ids = vec![1, 2, 3, 4, 5];

        let results = executor.execute_blocks(&block_ids);
        assert!(results.is_ok());
        assert_eq!(results.unwrap().len(), 5);

        let stats = executor.get_stats();
        assert_eq!(stats.total_executions, 5);
        assert_eq!(stats.cached_blocks, 5);
    }

    /// 测试4: JIT缓存清空
    #[test]
    fn test_jit_cache_flush() {
        let mut executor = JitExecutor::new();

        executor.execute_block(1).unwrap();
        executor.execute_block(2).unwrap();

        assert_eq!(executor.get_stats().cached_blocks, 2);

        executor.flush_cache();

        assert_eq!(executor.get_stats().cached_blocks, 0);
    }

    /// 测试5: JIT多次相同块执行
    #[test]
    fn test_jit_repeated_same_block() {
        let mut executor = JitExecutor::new();

        for _ in 0..10 {
            executor.execute_block(42).unwrap();
        }

        let stats = executor.get_stats();
        assert_eq!(stats.total_executions, 10);
        assert_eq!(stats.cached_blocks, 1);
    }

    /// 测试6: JIT大量不同块执行
    #[test]
    fn test_jit_many_different_blocks() {
        let mut executor = JitExecutor::new();

        for i in 1..=100 {
            executor.execute_block(i).unwrap();
        }

        let stats = executor.get_stats();
        assert_eq!(stats.total_executions, 100);
        assert_eq!(stats.cached_blocks, 100);
        assert_eq!(stats.compilation_count, 100);
    }

    /// 测试7: JIT执行器默认构造
    #[test]
    fn test_jit_default_constructor() {
        let executor: JitExecutor = Default::default();
        assert_eq!(executor.get_stats().total_executions, 0);
    }

    /// 测试8: JIT上下文类型
    #[test]
    fn test_jit_context_type() {
        let executor = JitExecutor::new();
        let context = AsyncExecutionContext::new(ExecutorType::Jit);
        assert_eq!(context.executor_type, ExecutorType::Jit);
    }

    /// 测试9: JIT统计信息准确性
    #[test]
    fn test_jit_stats_accuracy() {
        let mut executor = JitExecutor::new();

        executor.execute_block(1).unwrap();
        executor.execute_block(1).unwrap();
        executor.execute_block(2).unwrap();

        let stats = executor.get_stats();
        assert_eq!(stats.total_executions, 3);
        assert_eq!(stats.cached_blocks, 2);
        assert!(stats.avg_time_us > 0);
    }

    /// 测试10: JIT批量空列表
    #[test]
    fn test_jit_empty_batch() {
        let mut executor = JitExecutor::new();
        let block_ids = vec![];

        let results = executor.execute_blocks(&block_ids);
        assert!(results.is_ok());
        assert_eq!(results.unwrap().len(), 0);
    }
}

// ============================================================================
// 解释器执行器测试 (测试11-20)
// ============================================================================

#[cfg(test)]
mod interpreter_executor_tests {
    use super::*;

    /// 测试11: 解释器基本执行
    #[test]
    fn test_interpreter_single_execution() {
        let mut executor = InterpreterExecutor::new();
        let result = executor.execute_block(42);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);

        let stats = executor.get_stats();
        assert_eq!(stats.total_executions, 1);
    }

    /// 测试12: 解释器多次执行
    #[test]
    fn test_interpreter_multiple_executions() {
        let mut executor = InterpreterExecutor::new();

        for i in 1..=10 {
            executor.execute_block(i).unwrap();
        }

        let stats = executor.get_stats();
        assert_eq!(stats.total_executions, 10);
        assert!(stats.avg_time_us > 0);
    }

    /// 测试13: 解释器默认构造
    #[test]
    fn test_interpreter_default_constructor() {
        let executor: InterpreterExecutor = Default::default();
        assert_eq!(executor.get_stats().total_executions, 0);
    }

    /// 测试14: 解释器上下文类型
    #[test]
    fn test_interpreter_context_type() {
        let executor = InterpreterExecutor::new();
        let context = AsyncExecutionContext::new(ExecutorType::Interpreter);
        assert_eq!(context.executor_type, ExecutorType::Interpreter);
    }

    /// 测试15: 解释器统计信息
    #[test]
    fn test_interpreter_stats_tracking() {
        let mut executor = InterpreterExecutor::new();

        executor.execute_block(1).unwrap();
        executor.execute_block(2).unwrap();
        executor.execute_block(3).unwrap();

        let stats = executor.get_stats();
        assert_eq!(stats.total_executions, 3);
    }

    /// 测试16: 解释器零块ID
    #[test]
    fn test_interpreter_zero_block_id() {
        let mut executor = InterpreterExecutor::new();
        let result = executor.execute_block(0);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    /// 测试17: 解释器大块ID
    #[test]
    fn test_interpreter_large_block_id() {
        let mut executor = InterpreterExecutor::new();
        let result = executor.execute_block(u64::MAX);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), u64::MAX);
    }

    /// 测试18: 解释器连续执行相同块
    #[test]
    fn test_interpreter_repeated_same_block() {
        let mut executor = InterpreterExecutor::new();

        for _ in 0..5 {
            executor.execute_block(100).unwrap();
        }

        let stats = executor.get_stats();
        assert_eq!(stats.total_executions, 5);
    }

    /// 测试19: 解释器统计平均值
    #[test]
    fn test_interpreter_average_time() {
        let mut executor = InterpreterExecutor::new();

        for _ in 0..3 {
            executor.execute_block(1).unwrap();
        }

        let stats = executor.get_stats();
        // 解释器模拟执行时间为500微秒
        assert!(stats.avg_time_us >= 500);
    }

    /// 测试20: 解释器执行结果一致性
    #[test]
    fn test_interpreter_result_consistency() {
        let mut executor = InterpreterExecutor::new();

        let result1 = executor.execute_block(123).unwrap();
        let result2 = executor.execute_block(123).unwrap();

        assert_eq!(result1, result2);
        assert_eq!(result1, 123);
    }
}

// ============================================================================
// 混合执行器测试 (测试21-30)
// ============================================================================

#[cfg(test)]
mod hybrid_executor_tests {
    use super::*;

    /// 测试21: 混合执行器基本执行(JIT优先)
    #[test]
    fn test_hybrid_default_jit_preferred() {
        let mut executor = HybridExecutor::new();
        let result = executor.execute_block(1);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);

        let jit_stats = executor.get_jit_stats();
        assert_eq!(jit_stats.total_executions, 1);
    }

    /// 测试22: 混合执行器切换到解释器
    #[test]
    fn test_hybrid_switch_to_interpreter() {
        let mut executor = HybridExecutor::new();
        executor.set_prefer_jit(false);

        executor.execute_block(1).unwrap();

        let interp_stats = executor.get_interpreter_stats();
        assert_eq!(interp_stats.total_executions, 1);

        let jit_stats = executor.get_jit_stats();
        assert_eq!(jit_stats.total_executions, 0);
    }

    /// 测试23: 混合执行器切换回JIT
    #[test]
    fn test_hybrid_switch_back_to_jit() {
        let mut executor = HybridExecutor::new();
        executor.set_prefer_jit(false);

        executor.execute_block(1).unwrap();
        executor.set_prefer_jit(true);
        executor.execute_block(2).unwrap();

        let interp_stats = executor.get_interpreter_stats();
        let jit_stats = executor.get_jit_stats();

        assert_eq!(interp_stats.total_executions, 1);
        assert_eq!(jit_stats.total_executions, 1);
    }

    /// 测试24: 混合执行器默认构造
    #[test]
    fn test_hybrid_default_constructor() {
        let executor: HybridExecutor = Default::default();
        assert_eq!(executor.get_jit_stats().total_executions, 0);
        assert_eq!(executor.get_interpreter_stats().total_executions, 0);
    }

    /// 测试25: 混合执行器JIT和解释器统计分离
    #[test]
    fn test_hybrid_stats_separation() {
        let mut executor = HybridExecutor::new();

        executor.set_prefer_jit(true);
        executor.execute_block(1).unwrap();
        executor.execute_block(2).unwrap();

        executor.set_prefer_jit(false);
        executor.execute_block(3).unwrap();

        let jit_stats = executor.get_jit_stats();
        let interp_stats = executor.get_interpreter_stats();

        assert_eq!(jit_stats.total_executions, 2);
        assert_eq!(interp_stats.total_executions, 1);
    }

    /// 测试26: 混合执行器多次切换
    #[test]
    fn test_hybrid_multiple_switches() {
        let mut executor = HybridExecutor::new();

        for i in 0..10 {
            executor.set_prefer_jit(i % 2 == 0);
            executor.execute_block(i).unwrap();
        }

        let jit_stats = executor.get_jit_stats();
        let interp_stats = executor.get_interpreter_stats();

        assert_eq!(jit_stats.total_executions, 5);
        assert_eq!(interp_stats.total_executions, 5);
    }

    /// 测试27: 混合执行器JIT缓存
    #[test]
    fn test_hybrid_jit_caching() {
        let mut executor = HybridExecutor::new();

        executor.execute_block(1).unwrap();
        executor.execute_block(1).unwrap();

        let jit_stats = executor.get_jit_stats();
        assert_eq!(jit_stats.total_executions, 2);
        assert_eq!(jit_stats.cached_blocks, 1);
    }

    /// 测试28: 混合执行器同时执行
    #[test]
    fn test_hybrid_concurrent_execution() {
        let mut executor = HybridExecutor::new();

        executor.set_prefer_jit(true);
        executor.execute_block(1).unwrap();

        executor.set_prefer_jit(false);
        executor.execute_block(2).unwrap();

        let jit_stats = executor.get_jit_stats();
        let interp_stats = executor.get_interpreter_stats();

        assert!(jit_stats.total_executions > 0);
        assert!(interp_stats.total_executions > 0);
    }

    /// 测试29: 混合执行器性能差异
    #[test]
    fn test_hybrid_performance_difference() {
        let mut executor = HybridExecutor::new();

        executor.set_prefer_jit(true);
        executor.execute_block(1).unwrap();
        let jit_stats = executor.get_jit_stats();

        executor.set_prefer_jit(false);
        executor.execute_block(2).unwrap();
        let interp_stats = executor.get_interpreter_stats();

        // JIT应该比解释器快
        assert!(jit_stats.avg_time_us < interp_stats.avg_time_us);
    }

    /// 测试30: 混合执行器独立统计
    #[test]
    fn test_hybrid_independent_stats() {
        let mut executor = HybridExecutor::new();

        executor.set_prefer_jit(true);
        executor.execute_block(1).unwrap();

        let jit_stats_before = executor.get_jit_stats();
        let interp_stats_before = executor.get_interpreter_stats();

        executor.set_prefer_jit(false);
        executor.execute_block(2).unwrap();

        let jit_stats_after = executor.get_jit_stats();
        let interp_stats_after = executor.get_interpreter_stats();

        // JIT统计不应因解释器执行而改变
        assert_eq!(
            jit_stats_before.total_executions,
            jit_stats_after.total_executions
        );

        // 解释器统计应该增加
        assert!(interp_stats_after.total_executions > interp_stats_before.total_executions);
    }
}

// ============================================================================
// 协程测试 (测试31-40)
// ============================================================================

#[cfg(test)]
mod coroutine_tests {
    use super::*;

    /// 测试31: 协程基本创建
    #[test]
    fn test_coroutine_creation() {
        let coro = Coroutine::new(1);

        assert_eq!(coro.id, 1);
        assert_eq!(coro.state, CoroutineState::Created);
        assert_eq!(coro.execution_count, 0);
        assert_eq!(coro.total_time_us, 0);
    }

    /// 测试32: 协程状态转换
    #[test]
    fn test_coroutine_state_transitions() {
        let mut coro = Coroutine::new(1);

        assert_eq!(coro.state, CoroutineState::Created);

        coro.mark_ready();
        assert_eq!(coro.state, CoroutineState::Ready);

        coro.mark_running();
        assert_eq!(coro.state, CoroutineState::Running);
    }

    /// 测试33: 协程执行记录
    #[test]
    fn test_coroutine_execution_recording() {
        let mut coro = Coroutine::new(1);

        coro.record_execution(100);
        assert_eq!(coro.execution_count, 1);
        assert_eq!(coro.total_time_us, 100);

        coro.record_execution(200);
        assert_eq!(coro.execution_count, 2);
        assert_eq!(coro.total_time_us, 300);
    }

    /// 测试34: 协程平均执行时间
    #[test]
    fn test_coroutine_average_time() {
        let mut coro = Coroutine::new(1);

        assert_eq!(coro.avg_exec_time(), 0);

        coro.record_execution(100);
        assert_eq!(coro.avg_exec_time(), 100);

        coro.record_execution(200);
        assert_eq!(coro.avg_exec_time(), 150);
    }

    /// 测试35: 协程克隆
    #[test]
    fn test_coroutine_clone() {
        let coro1 = Coroutine::new(1);
        let coro2 = coro1.clone();

        assert_eq!(coro1.id, coro2.id);
        assert_eq!(coro1.state, coro2.state);
    }

    /// 测试36: 协程多次执行
    #[test]
    fn test_coroutine_multiple_executions() {
        let mut coro = Coroutine::new(1);

        for i in 1..=10 {
            coro.record_execution(i * 10);
        }

        assert_eq!(coro.execution_count, 10);
        assert_eq!(coro.total_time_us, 550);
    }

    /// 测试37: 协程零执行时间
    #[test]
    fn test_coroutine_zero_execution_time() {
        let mut coro = Coroutine::new(1);

        coro.record_execution(0);
        assert_eq!(coro.avg_exec_time(), 0);
    }

    /// 测试38: 协程大执行时间
    #[test]
    fn test_coroutine_large_execution_time() {
        let mut coro = Coroutine::new(1);

        coro.record_execution(u64::MAX);
        assert_eq!(coro.avg_exec_time(), u64::MAX);
    }

    /// 测试39: 协程不同ID
    #[test]
    fn test_coroutine_different_ids() {
        let coro1 = Coroutine::new(100);
        let coro2 = Coroutine::new(200);

        assert_eq!(coro1.id, 100);
        assert_eq!(coro2.id, 200);
        assert_ne!(coro1.id, coro2.id);
    }

    /// 测试40: 协程状态相等性
    #[test]
    fn test_coroutine_state_equality() {
        assert_eq!(CoroutineState::Created, CoroutineState::Created);
        assert_eq!(CoroutineState::Ready, CoroutineState::Ready);
        assert_ne!(CoroutineState::Created, CoroutineState::Running);
    }
}

// ============================================================================
// VCPU测试 (测试41-50)
// ============================================================================

#[cfg(test)]
mod vcpu_tests {
    use std::collections::VecDeque;

    use super::*;

    /// 测试41: VCPU基本创建
    #[test]
    fn test_vcpu_creation() {
        let vcpu = VCPU::new(0);

        assert_eq!(vcpu.id, 0);
        assert_eq!(vcpu.state, VCPUState::Idle);
        assert!(vcpu.current_coroutine.is_none());
        assert_eq!(vcpu.queue_length(), 0);
    }

    /// 测试42: VCPU入队协程
    #[test]
    fn test_vcpu_enqueue() {
        let mut vcpu = VCPU::new(0);
        let coro = Coroutine::new(1);

        vcpu.enqueue(coro);
        assert_eq!(vcpu.queue_length(), 1);
    }

    /// 测试43: VCPU出队协程
    #[test]
    fn test_vcpu_dequeue() {
        let mut vcpu = VCPU::new(0);
        let coro = Coroutine::new(1);

        vcpu.enqueue(coro);
        let dequeued = vcpu.dequeue();

        assert!(dequeued.is_some());
        assert_eq!(dequeued.unwrap().id, 1);
        assert_eq!(vcpu.queue_length(), 0);
    }

    /// 测试44: VCPU设置当前协程
    #[test]
    fn test_vcpu_set_current() {
        let mut vcpu = VCPU::new(0);
        let coro = Coroutine::new(1);

        vcpu.set_current(coro);

        assert!(vcpu.current_coroutine.is_some());
        assert_eq!(vcpu.state, VCPUState::Running);
        assert_eq!(vcpu.stats.executions, 1);
    }

    /// 测试45: VCPU清除当前协程
    #[test]
    fn test_vcpu_clear_current() {
        let mut vcpu = VCPU::new(0);
        let coro = Coroutine::new(1);

        vcpu.set_current(coro);
        vcpu.clear_current();

        assert!(vcpu.current_coroutine.is_none());
        assert_eq!(vcpu.state, VCPUState::Idle);
    }

    /// 测试46: VCPU多个协程入队
    #[test]
    fn test_vcpu_multiple_enqueue() {
        let mut vcpu = VCPU::new(0);

        for i in 1..=10 {
            vcpu.enqueue(Coroutine::new(i));
        }

        assert_eq!(vcpu.queue_length(), 10);
    }

    /// 测试47: VCPU FIFO顺序
    #[test]
    fn test_vcpu_fifo_order() {
        let mut vcpu = VCPU::new(0);

        for i in 1..=5 {
            vcpu.enqueue(Coroutine::new(i));
        }

        for i in 1..=5 {
            let coro = vcpu.dequeue().unwrap();
            assert_eq!(coro.id, i);
        }
    }

    /// 测试48: VCPU统计信息初始化
    #[test]
    fn test_vcpu_stats_initialization() {
        let vcpu = VCPU::new(0);

        assert_eq!(vcpu.stats.executions, 0);
        assert_eq!(vcpu.stats.context_switches, 0);
        assert_eq!(vcpu.stats.idle_time_us, 0);
        assert_eq!(vcpu.stats.busy_time_us, 0);
    }

    /// 测试49: VCPU记录时间
    #[test]
    fn test_vcpu_record_time() {
        let mut vcpu = VCPU::new(0);

        vcpu.record_busy_time(1000);
        vcpu.record_idle_time(500);

        assert_eq!(vcpu.stats.busy_time_us, 1000);
        assert_eq!(vcpu.stats.idle_time_us, 500);
    }

    /// 测试50: VCPU利用率计算
    #[test]
    fn test_vcpu_utilization() {
        let mut vcpu = VCPU::new(0);

        // 无时间时利用率为0
        assert_eq!(vcpu.stats.utilization(), 0.0);

        vcpu.record_busy_time(500);
        vcpu.record_idle_time(500);

        // 50%利用率
        assert!((vcpu.stats.utilization() - 0.5).abs() < 0.01);
    }
}

// ============================================================================
// 调度器测试 (测试51-60)
// ============================================================================

#[cfg(test)]
mod scheduler_tests {
    use super::*;

    /// 测试51: 调度器基本创建
    #[test]
    fn test_scheduler_creation() {
        let scheduler = Scheduler::new(4);

        assert_eq!(scheduler.vcpu_count(), 4);
        assert_eq!(scheduler.create_coroutine().id, 0);
        assert_eq!(scheduler.create_coroutine().id, 1);
    }

    /// 测试52: 调度器创建协程
    #[test]
    fn test_scheduler_create_coroutine() {
        let scheduler = Scheduler::new(2);

        let coro1 = scheduler.create_coroutine();
        let coro2 = scheduler.create_coroutine();

        assert_eq!(coro1.id, 0);
        assert_eq!(coro2.id, 1);
        assert_eq!(coro1.state, CoroutineState::Created);
    }

    /// 测试53: 调度器提交协程
    #[test]
    fn test_scheduler_submit_coroutine() {
        let scheduler = Scheduler::new(2);
        let coro = scheduler.create_coroutine();

        scheduler.submit_coroutine(coro);

        // 协程已提交到全局队列
        // 验证通过next_coroutine获取
    }

    /// 测试54: 调度器分配到vCPU
    #[test]
    fn test_scheduler_assign_to_vcpu() {
        let mut scheduler = Scheduler::new(4);
        let coro = scheduler.create_coroutine();

        let result = scheduler.assign_to_vcpu(0, coro);

        assert!(result.is_ok());

        // 验证协程在vCPU 0的本地队列中
        let next_coro = scheduler.next_coroutine(0);
        assert!(next_coro.is_some());
        assert_eq!(next_coro.unwrap().id, 0);
    }

    /// 测试55: 调度器无效vCPU分配
    #[test]
    fn test_scheduler_invalid_vcpu_assignment() {
        let mut scheduler = Scheduler::new(2);
        let coro = scheduler.create_coroutine();

        let result = scheduler.assign_to_vcpu(10, coro);

        assert!(result.is_err());
    }

    /// 测试56: 调度器本地队列优先
    #[test]
    fn test_scheduler_local_queue_priority() {
        let mut scheduler = Scheduler::new(2);

        // 添加到本地队列
        let coro1 = scheduler.create_coroutine();
        scheduler.assign_to_vcpu(0, coro1).unwrap();

        // 添加到全局队列
        let coro2 = scheduler.create_coroutine();
        scheduler.submit_coroutine(coro2);

        // 应该先从本地队列获取
        let next_coro = scheduler.next_coroutine(0);
        assert!(next_coro.is_some());
        assert_eq!(next_coro.unwrap().id, 0);
    }

    /// 测试57: 调度器全局队列获取
    #[test]
    fn test_scheduler_global_queue() {
        let mut scheduler = Scheduler::new(2);

        // 只添加到全局队列
        let coro = scheduler.create_coroutine();
        scheduler.submit_coroutine(coro);

        let next_coro = scheduler.next_coroutine(0);
        assert!(next_coro.is_some());
        assert_eq!(next_coro.unwrap().id, 0);
    }

    /// 测试58: 调度器work stealing
    #[test]
    fn test_scheduler_work_stealing() {
        let mut scheduler = Scheduler::new(2);

        // 只在vCPU 1添加协程
        let coro = scheduler.create_coroutine();
        scheduler.assign_to_vcpu(1, coro).unwrap();

        // vCPU 0应该能从vCPU 1窃取
        let stolen = scheduler.try_steal_work(0);
        assert!(stolen.is_some());
        assert_eq!(stolen.unwrap().id, 0);
    }

    /// 测试59: 调度器空队列处理
    #[test]
    fn test_scheduler_empty_queue_handling() {
        let mut scheduler = Scheduler::new(2);

        // 无可用协程
        let next_coro = scheduler.next_coroutine(0);
        assert!(next_coro.is_none());

        let stolen = scheduler.try_steal_work(0);
        assert!(stolen.is_none());
    }

    /// 测试60: 调度器多vCPU分配
    #[test]
    fn test_scheduler_multiple_vcpu_distribution() {
        let mut scheduler = Scheduler::new(4);

        // 分配协程到不同vCPU
        for i in 0..4 {
            let coro = scheduler.create_coroutine();
            scheduler.assign_to_vcpu(i, coro).unwrap();
        }

        // 每个vCPU都应该有一个协程
        for i in 0..4 {
            let next_coro = scheduler.next_coroutine(i);
            assert!(next_coro.is_some());
            assert_eq!(next_coro.unwrap().id, i as u64);
        }
    }
}
