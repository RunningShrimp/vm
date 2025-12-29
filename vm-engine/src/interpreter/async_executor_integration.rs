//! 异步执行引擎集成辅助模块
//!
//! 提供异步执行引擎与主循环集成的辅助功能

use crate::interpreter::Interpreter;
use crate::interpreter::async_executor::{AsyncExecStats, AsyncExecutor};
use std::sync::Arc;
use vm_core::ExecutionEngine;
use vm_core::{ExecResult, MMU};
use vm_ir::IRBlock;

#[cfg(test)]
use vm_ir::{IRBuilder, IROp, Terminator};

/// 异步执行引擎包装器
///
/// 提供统一的异步执行接口，支持配置化的异步执行策略
pub struct AsyncExecutorWrapper {
    /// 内部解释器
    interpreter: Arc<tokio::sync::Mutex<Interpreter>>,
    /// 统计信息
    stats: Arc<tokio::sync::Mutex<AsyncExecStats>>,
    /// Yield 间隔
    yield_interval: u64,
}

impl AsyncExecutorWrapper {
    /// 创建新的异步执行引擎包装器
    pub fn new(yield_interval: u64) -> Self {
        Self {
            interpreter: Arc::new(tokio::sync::Mutex::new(Interpreter::new())),
            stats: Arc::new(tokio::sync::Mutex::new(AsyncExecStats::default())),
            yield_interval,
        }
    }

    /// 异步执行单个 IR 块
    pub async fn execute_block_async(
        &self,
        mmu: &mut dyn MMU,
        block: &IRBlock,
    ) -> Result<ExecResult, String> {
        let mut interp = self.interpreter.lock().await;
        interp.execute_block_async(mmu, block).await
    }

    /// 异步执行多个步骤（带 yield）
    pub async fn run_steps_async(
        &self,
        mmu: &mut dyn MMU,
        block: &IRBlock,
        max_steps: u64,
    ) -> Result<ExecResult, String> {
        let mut interp = self.interpreter.lock().await;
        interp
            .run_steps_async(mmu, block, max_steps, self.yield_interval)
            .await
    }

    /// 获取统计信息
    pub fn stats(&self) -> AsyncExecStats {
        tokio::task::block_in_place(|| self.stats.blocking_lock().clone())
    }

    /// 重置统计信息
    pub fn reset_stats(&self) {
        tokio::task::block_in_place(|| {
            *self.stats.blocking_lock() = AsyncExecStats::default();
        });
    }

    /// 设置寄存器
    pub fn set_reg(&self, idx: usize, val: u64) {
        tokio::task::block_in_place(|| {
            self.interpreter.blocking_lock().set_reg(idx as u32, val);
        });
    }

    /// 获取寄存器值
    pub fn get_reg(&self, idx: usize) -> u64 {
        tokio::task::block_in_place(|| self.interpreter.blocking_lock().get_reg(idx as u32))
    }

    /// 设置 PC
    pub fn set_pc(&self, pc: u64) {
        tokio::task::block_in_place(|| {
            self.interpreter
                .blocking_lock()
                .set_pc(vm_core::GuestAddr(pc));
        });
    }

    /// 获取 PC
    pub fn get_pc(&self) -> u64 {
        tokio::task::block_in_place(|| self.interpreter.blocking_lock().get_pc().0)
    }
}

/// 性能对比结果
#[derive(Debug, Clone)]
pub struct PerformanceComparison {
    /// 同步执行时间（微秒）
    pub sync_time_us: u64,
    /// 异步执行时间（微秒）
    pub async_time_us: u64,
    /// 性能提升百分比
    pub improvement_percent: f64,
    /// 同步执行的指令数
    pub sync_ops: u64,
    /// 异步执行的指令数
    pub async_ops: u64,
}

impl PerformanceComparison {
    /// 计算性能提升
    pub fn calculate_improvement(&mut self) {
        if self.sync_time_us > 0 {
            let improvement = (self.sync_time_us as f64 - self.async_time_us as f64)
                / self.sync_time_us as f64
                * 100.0;
            self.improvement_percent = improvement;
        }
    }
}

/// 执行性能基准测试
///
/// 对比同步和异步执行的性能
pub async fn benchmark_async_vs_sync(
    mmu: &mut dyn MMU,
    block: &IRBlock,
    iterations: u64,
    yield_interval: u64,
) -> PerformanceComparison {
    // 同步执行基准测试
    let sync_start = std::time::Instant::now();
    let mut sync_interp = Interpreter::new();
    let mut sync_ops = 0u64;

    for _ in 0..iterations {
        let result = sync_interp.run(mmu, block);
        sync_ops += result.stats.executed_ops;
    }
    let sync_time_us = sync_start.elapsed().as_micros() as u64;

    // 异步执行基准测试
    let async_start = std::time::Instant::now();
    let async_executor = AsyncExecutorWrapper::new(yield_interval);
    let mut async_ops = 0u64;

    for _ in 0..iterations {
        match async_executor.execute_block_async(mmu, block).await {
            Ok(result) => {
                async_ops += result.stats.executed_ops;
            }
            Err(_) => break,
        }
    }
    let async_time_us = async_start.elapsed().as_micros() as u64;

    let mut comparison = PerformanceComparison {
        sync_time_us,
        async_time_us,
        improvement_percent: 0.0,
        sync_ops,
        async_ops,
    };
    comparison.calculate_improvement();
    comparison
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_mem::SoftMmu;

    #[tokio::test]
    async fn test_async_executor_wrapper() {
        let executor = AsyncExecutorWrapper::new(100);
        let mut mmu = SoftMmu::new(1024 * 1024, false);

        let mut builder = IRBuilder::new(vm_core::GuestAddr(0x1000));
        builder.push(IROp::MovImm { dst: 0, imm: 42 });
        builder.set_term(Terminator::Ret);
        let block = builder.build();

        let result = executor.execute_block_async(&mut mmu, &block).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_performance_comparison() {
        let mut mmu = SoftMmu::new(1024 * 1024, false);

        let mut builder = IRBuilder::new(vm_core::GuestAddr(0x1000));
        for i in 0..10 {
            builder.push(IROp::MovImm {
                dst: i,
                imm: i as u64,
            });
        }
        builder.set_term(Terminator::Ret);
        let block = builder.build();

        let comparison = benchmark_async_vs_sync(&mut mmu, &block, 100, 10).await;

        assert!(comparison.sync_time_us > 0);
        assert!(comparison.async_time_us > 0);
        assert_eq!(comparison.sync_ops, comparison.async_ops);
    }
}
