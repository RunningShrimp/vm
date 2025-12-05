/// Week 3 - 异步执行引擎集成测试
///
/// 测试异步执行、多vCPU并发、中断处理等功能

#[cfg(test)]
mod async_execution_engine_tests {
    use vm_engine_interpreter::async_executor::{
        AsyncExecutor, AsyncMultiVcpuExecutor, AsyncExecutionContext, MockMMU,
    };
    use vm_engine_interpreter::async_interrupt_handler::{
        AsyncInterruptQueue, Interrupt, InterruptType, InterruptPriority,
    };
    use vm_ir::IRBlock;

    #[tokio::test]
    async fn test_async_execution_context_workflow() {
        let block = IRBlock { ops: vec![] };
        let mut ctx = AsyncExecutionContext::new(block, 1000, 100);
        
        // 初始状态检查
        assert_eq!(ctx.current_steps, 0);
        assert_eq!(ctx.max_steps, 1000);
        assert!(!ctx.is_complete());
        
        // 执行步骤
        for _ in 0..1000 {
            if ctx.should_yield() {
                ctx.record_yield();
            }
            ctx.step();
        }
        
        // 完成状态检查
        assert!(ctx.is_complete());
        assert_eq!(ctx.stats.async_ops, 1000);
        assert!(ctx.stats.yield_count > 0);
    }

    #[tokio::test]
    async fn test_multi_vcpu_executor_creation() {
        let executor = AsyncMultiVcpuExecutor::new(4);
        
        assert_eq!(executor.vcpu_count(), 4);
        
        let stats = executor.get_stats();
        assert_eq!(stats.total_ops, 0);
    }

    #[tokio::test]
    async fn test_multi_vcpu_stats_update() {
        let executor = AsyncMultiVcpuExecutor::new(2);
        let stats = executor.get_stats();
        
        assert!(stats.min_exec_time_us == 0 || stats.min_exec_time_us <= stats.max_exec_time_us);
    }

    #[tokio::test]
    async fn test_interrupt_type_variants() {
        let syscall = InterruptType::Syscall(1);
        let io = InterruptType::IoInterrupt(2);
        let timer = InterruptType::Timer;
        let page_fault = InterruptType::PageFault(0x1000);
        
        assert_ne!(syscall, io);
        assert_ne!(timer, page_fault);
    }

    #[tokio::test]
    async fn test_interrupt_priority_ordering() {
        let critical = InterruptPriority::Critical;
        let high = InterruptPriority::High;
        let normal = InterruptPriority::Normal;
        let low = InterruptPriority::Low;
        
        assert!(critical > high);
        assert!(high > normal);
        assert!(normal > low);
    }

    #[tokio::test]
    async fn test_async_interrupt_queue_basic() {
        let queue = AsyncInterruptQueue::new();
        
        assert_eq!(queue.queue_length(), 0);
        
        let intr = Interrupt::new(
            InterruptType::Timer,
            InterruptPriority::Normal,
        );
        
        queue.dispatch_interrupt(intr).await.unwrap();
        assert_eq!(queue.queue_length(), 1);
    }

    #[tokio::test]
    async fn test_async_interrupt_queue_priority() {
        let queue = AsyncInterruptQueue::new();
        
        // 按非优先级顺序投递中断
        queue.dispatch_interrupt(Interrupt::new(
            InterruptType::Timer,
            InterruptPriority::Low,
        )).await.unwrap();

        queue.dispatch_interrupt(Interrupt::new(
            InterruptType::External(1),
            InterruptPriority::Critical,
        )).await.unwrap();

        queue.dispatch_interrupt(Interrupt::new(
            InterruptType::IoInterrupt(0),
            InterruptPriority::High,
        )).await.unwrap();

        // 验证优先级顺序
        let first = queue.pop_next().unwrap();
        assert_eq!(first.priority, InterruptPriority::Critical);

        let second = queue.pop_next().unwrap();
        assert_eq!(second.priority, InterruptPriority::High);

        let third = queue.pop_next().unwrap();
        assert_eq!(third.priority, InterruptPriority::Low);
    }

    #[tokio::test]
    async fn test_interrupt_queue_peek_and_pop() {
        let queue = AsyncInterruptQueue::new();
        
        let intr1 = Interrupt::new(InterruptType::Timer, InterruptPriority::Normal);
        queue.dispatch_interrupt(intr1.clone()).await.unwrap();
        
        // Peek不应该删除
        let peeked = queue.peek_next();
        assert!(peeked.is_some());
        assert_eq!(queue.queue_length(), 1);
        
        // Pop应该删除
        let popped = queue.pop_next();
        assert!(popped.is_some());
        assert_eq!(queue.queue_length(), 0);
    }

    #[tokio::test]
    async fn test_interrupt_queue_clear() {
        let queue = AsyncInterruptQueue::new();
        
        queue.dispatch_interrupt(Interrupt::new(
            InterruptType::Timer,
            InterruptPriority::Normal,
        )).await.unwrap();

        queue.dispatch_interrupt(Interrupt::new(
            InterruptType::IoInterrupt(0),
            InterruptPriority::Normal,
        )).await.unwrap();

        assert_eq!(queue.queue_length(), 2);
        
        queue.clear();
        assert_eq!(queue.queue_length(), 0);
    }

    #[tokio::test]
    async fn test_interrupt_stats_tracking() {
        let queue = AsyncInterruptQueue::new();
        
        for _ in 0..5 {
            queue.dispatch_interrupt(Interrupt::new(
                InterruptType::Timer,
                InterruptPriority::Normal,
            )).await.unwrap();
        }

        let stats = queue.get_stats();
        assert!(stats.avg_latency_ns > 0);
    }

    #[tokio::test]
    async fn test_multiple_interrupt_dispatches() {
        let queue = AsyncInterruptQueue::new();
        
        let mut handles = vec![];
        for i in 0..10 {
            let queue_clone = Arc::new(queue.clone_for_async());
            let handle = tokio::spawn(async move {
                let intr = Interrupt::new(
                    InterruptType::IoInterrupt(i),
                    InterruptPriority::Normal,
                );
                queue_clone.dispatch_interrupt(intr).await
            });
            handles.push(handle);
        }
        
        for handle in handles {
            assert!(handle.await.unwrap().is_ok());
        }

        assert_eq!(queue.queue_length(), 10);
    }

    #[tokio::test]
    async fn test_interrupt_type_syscall() {
        let syscall = InterruptType::Syscall(42);
        match syscall {
            InterruptType::Syscall(id) => assert_eq!(id, 42),
            _ => panic!("Expected Syscall variant"),
        }
    }

    #[tokio::test]
    async fn test_interrupt_type_page_fault() {
        let pf = InterruptType::PageFault(0xdeadbeef);
        match pf {
            InterruptType::PageFault(addr) => assert_eq!(addr, 0xdeadbeef),
            _ => panic!("Expected PageFault variant"),
        }
    }

    #[tokio::test]
    async fn test_interrupt_context_field() {
        let mut intr = Interrupt::new(
            InterruptType::Timer,
            InterruptPriority::Normal,
        );
        
        assert!(intr.context.is_none());
        
        intr.context = Some(vec![1, 2, 3, 4]);
        assert!(intr.context.is_some());
        assert_eq!(intr.context.unwrap().len(), 4);
    }

    #[tokio::test]
    async fn test_async_execution_yield_behavior() {
        let block = IRBlock { ops: vec![] };
        let mut ctx = AsyncExecutionContext::new(block, 1000, 50);
        
        let mut yield_count = 0;
        for _ in 0..1000 {
            if ctx.should_yield() {
                yield_count += 1;
                ctx.record_yield();
            }
            ctx.step();
        }
        
        // 在1000步中，每50步yield一次，应该有20次yield
        assert_eq!(yield_count, 20);
        assert_eq!(ctx.stats.yield_count, 20);
    }

    #[tokio::test]
    async fn test_interrupt_priority_comparisons() {
        let c1 = Interrupt::new(InterruptType::Timer, InterruptPriority::Critical);
        let c2 = Interrupt::new(InterruptType::Timer, InterruptPriority::High);
        
        assert!(c1 > c2);
    }
}

// 辅助函数和模拟
use std::sync::Arc;

// 为AsyncInterruptQueue实现克隆功能（仅用于测试）
trait AsyncInterruptQueueClone {
    fn clone_for_async(&self) -> Self;
}
