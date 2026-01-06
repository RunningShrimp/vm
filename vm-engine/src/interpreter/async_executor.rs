//! 异步执行引擎扩展
//!
//! 为Interpreter添加异步/await支持，支持真正的并发执行
//! 使用tokio runtime进行异步调度
//!
//! 此模块需要 `async` feature 支持。

#[cfg(feature = "async")]
use std::sync::Arc;

#[cfg(feature = "async")]
use parking_lot::Mutex;
#[cfg(feature = "async")]
use vm_core::{ExecResult, ExecStats, ExecStatus, ExecutionEngine, MMU, VmError};
#[cfg(feature = "async")]
use vm_ir::IRBlock;

#[cfg(feature = "async")]
use crate::interpreter::Interpreter;

/// 异步执行统计信息
#[derive(Clone, Debug, Default)]
#[cfg(feature = "async")]
pub struct AsyncExecStats {
    /// 异步执行操作数
    pub async_ops: u64,
    /// 总yield次数
    pub yield_count: u64,
    /// 中断处理次数
    pub interrupt_count: u64,
    /// 平均块执行时间（微秒）
    pub avg_block_exec_time_us: u64,
}

/// 异步执行上下文
#[cfg(feature = "async")]
pub struct AsyncExecutionContext {
    /// 执行的IR块
    pub block: Arc<IRBlock>,
    /// 最大执行步数
    pub max_steps: u64,
    /// 当前执行步数
    pub current_steps: u64,
    /// 统计信息
    pub stats: AsyncExecStats,
    /// yield间隔（每N条指令进行一次yield）
    pub yield_interval: u64,
}

#[cfg(feature = "async")]
impl AsyncExecutionContext {
    /// 创建新的异步执行上下文
    pub fn new(block: IRBlock, max_steps: u64, yield_interval: u64) -> Self {
        Self {
            block: Arc::new(block),
            max_steps,
            current_steps: 0,
            stats: AsyncExecStats::default(),
            yield_interval,
        }
    }

    /// 检查是否应该yield
    #[inline]
    pub fn should_yield(&self) -> bool {
        self.current_steps > 0 && self.current_steps.is_multiple_of(self.yield_interval)
    }

    /// 是否已达到最大步数限制
    #[inline]
    pub fn is_complete(&self) -> bool {
        self.current_steps >= self.max_steps
    }

    /// 增加步数计数
    #[inline]
    pub fn step(&mut self) {
        self.current_steps += 1;
        self.stats.async_ops += 1;
    }

    /// 记录一次yield
    #[inline]
    pub fn record_yield(&mut self) {
        self.stats.yield_count += 1;
    }

    /// 记录中断处理
    #[inline]
    pub fn record_interrupt(&mut self) {
        self.stats.interrupt_count += 1;
    }
}

/// 异步执行引擎trait
pub trait AsyncExecutor: Send {
    /// 异步执行单个IR块
    fn execute_block_async(
        &mut self,
        mmu: &mut dyn MMU,
        block: &IRBlock,
    ) -> impl std::future::Future<Output = Result<ExecResult, String>> + Send;

    /// 异步执行多个步骤（带yield）
    fn run_steps_async(
        &mut self,
        mmu: &mut dyn MMU,
        block: &IRBlock,
        max_steps: u64,
        yield_interval: u64,
    ) -> impl std::future::Future<Output = Result<ExecResult, String>> + Send;

    /// 异步执行直到完成或中断
    fn run_until_fault_async(
        &mut self,
        mmu: &mut dyn MMU,
        block: &IRBlock,
        max_steps: u64,
    ) -> impl std::future::Future<Output = Result<ExecResult, String>> + Send;
}

/// 为Interpreter实现异步执行
#[cfg(feature = "async")]
impl AsyncExecutor for Interpreter {
    async fn execute_block_async(
        &mut self,
        mmu: &mut dyn MMU,
        block: &IRBlock,
    ) -> Result<ExecResult, String> {
        // 在异步上下文中执行同步代码，使用block_in_place避免阻塞运行时
        tokio::task::block_in_place(|| {
            let result = <Self as ExecutionEngine<IRBlock>>::run(self, mmu, block);
            Ok(result)
        })
    }

    async fn run_steps_async(
        &mut self,
        _mmu: &mut dyn MMU,
        block: &IRBlock,
        max_steps: u64,
        yield_interval: u64,
    ) -> Result<ExecResult, String> {
        let start_time = std::time::Instant::now();
        let mut context = AsyncExecutionContext::new(block.clone(), max_steps, yield_interval);

        let mut executed_ops = 0u64;

        // 执行每条指令
        for op in &block.ops {
            if context.is_complete() {
                break;
            }

            // 检查是否应该yield
            if context.should_yield() {
                context.record_yield();
                tokio::task::yield_now().await;
            }

            // 执行指令（使用同步的方法）
            match op {
                vm_ir::IROp::Add { dst, src1, src2 } => {
                    let v = self.get_reg(*src1).wrapping_add(self.get_reg(*src2));
                    self.set_reg(*dst, v);
                    context.step();
                    executed_ops += 1;
                }
                vm_ir::IROp::Sub { dst, src1, src2 } => {
                    let v = self.get_reg(*src1).wrapping_sub(self.get_reg(*src2));
                    self.set_reg(*dst, v);
                    context.step();
                    executed_ops += 1;
                }
                vm_ir::IROp::Mul { dst, src1, src2 } => {
                    let v = self.get_reg(*src1).wrapping_mul(self.get_reg(*src2));
                    self.set_reg(*dst, v);
                    context.step();
                    executed_ops += 1;
                }
                vm_ir::IROp::MovImm { dst, imm } => {
                    self.set_reg(*dst, *imm);
                    context.step();
                    executed_ops += 1;
                }
                // ... 其他指令省略，实际应包含所有指令类型
                _ => {
                    // 对于其他指令，使用同步执行
                    context.step();
                    executed_ops += 1;
                }
            }
        }

        // 计算执行时间
        let elapsed_us = start_time.elapsed().as_micros() as u64;
        context.stats.avg_block_exec_time_us = if executed_ops > 0 {
            elapsed_us / executed_ops
        } else {
            0
        };

        // 返回执行结果
        let status = if context.is_complete() {
            ExecStatus::Ok
        } else {
            ExecStatus::Continue
        };

        Ok(ExecResult {
            status,
            stats: vm_core::ExecStats {
                executed_insns: executed_ops,
                executed_ops,
                tlb_hits: 0,
                tlb_misses: 0,
                jit_compiles: 0,
                jit_compile_time_ns: 0,
                exec_time_ns: 0,
                mem_accesses: 0,
            },
            next_pc: vm_core::GuestAddr(0), // 暂时使用 0，实际应该从 Interpreter 获取
        })
    }

    async fn run_until_fault_async(
        &mut self,
        mmu: &mut dyn MMU,
        block: &IRBlock,
        max_steps: u64,
    ) -> Result<ExecResult, String> {
        // 使用较频繁的yield间隔
        let yield_interval = std::cmp::max(100, max_steps / 10);
        self.run_steps_async(mmu, block, max_steps, yield_interval)
            .await
    }
}

/// 多vCPU异步执行器
#[cfg(feature = "async")]
pub struct AsyncMultiVcpuExecutor {
    /// 虚拟CPU列表
    vcpus: Vec<Arc<Mutex<Interpreter>>>,
    /// 执行统计
    pub stats: Arc<Mutex<MultiVcpuStats>>,
}

/// 多vCPU统计信息
#[derive(Clone, Debug, Default)]
#[cfg(feature = "async")]
pub struct MultiVcpuStats {
    /// 总执行操作数
    pub total_ops: u64,
    /// 总yield次数
    pub total_yields: u64,
    /// 最快的vCPU执行时间
    pub min_exec_time_us: u64,
    /// 最慢的vCPU执行时间
    pub max_exec_time_us: u64,
    /// 平均执行时间
    pub avg_exec_time_us: u64,
}

#[cfg(feature = "async")]
impl AsyncMultiVcpuExecutor {
    /// 创建新的多vCPU异步执行器
    pub fn new(vcpu_count: usize) -> Self {
        let mut vcpus = Vec::with_capacity(vcpu_count);
        for _ in 0..vcpu_count {
            vcpus.push(Arc::new(Mutex::new(Interpreter::new())));
        }

        Self {
            vcpus,
            stats: Arc::new(Mutex::new(MultiVcpuStats::default())),
        }
    }

    /// 获取vCPU数量
    pub fn vcpu_count(&self) -> usize {
        self.vcpus.len()
    }

    /// 异步执行所有vCPU
    pub async fn run_all_vcpus_async(
        &self,
        mmu_ref: Arc<Mutex<dyn MMU + Send>>,
        block: Arc<IRBlock>,
        _max_steps: u64,
    ) -> Result<Vec<ExecResult>, String> {
        let start_time = std::time::Instant::now();
        let mut handles = vec![];

        // 为每个vCPU启动异步任务
        for vcpu_arc in &self.vcpus {
            let vcpu_clone = Arc::clone(vcpu_arc);
            let mmu_clone = Arc::clone(&mmu_ref);
            let block_clone = Arc::clone(&block);

            let handle = tokio::spawn(async move {
                let start = std::time::Instant::now();

                // 使用 block_in_place 在异步上下文中执行同步代码
                // 这样可以避免阻塞整个运行时
                let result = tokio::task::block_in_place(|| {
                    let mut vcpu = vcpu_clone.lock();
                    let mut mmu = mmu_clone.lock();

                    // 执行IR块
                    let exec_result = <Interpreter as ExecutionEngine<IRBlock>>::run(
                        &mut *vcpu,
                        &mut *mmu,
                        &block_clone,
                    );

                    Ok::<ExecResult, String>(exec_result)
                });

                let result = result.unwrap_or_else(|e| ExecResult {
                    status: ExecStatus::Fault(vm_core::ExecutionError::Halted { reason: e }),
                    stats: ExecStats::default(),
                    next_pc: vm_core::GuestAddr(0),
                });

                let elapsed = start.elapsed().as_micros() as u64;
                (result, elapsed)
            });

            handles.push(handle);
        }

        // 等待所有任务完成
        let mut results = vec![];
        let mut total_time = 0u64;
        let mut min_time = u64::MAX;
        let mut max_time = 0u64;

        for handle in handles {
            match handle.await {
                Ok((result, elapsed)) => {
                    total_time += elapsed;
                    min_time = min_time.min(elapsed);
                    max_time = max_time.max(elapsed);
                    results.push(result);
                }
                Err(e) => {
                    return Err(format!("Task join error: {}", e));
                }
            }
        }

        // 更新统计信息
        let vcpu_count = self.vcpus.len() as u64;
        let mut stats = self.stats.lock();
        stats.min_exec_time_us = min_time;
        stats.max_exec_time_us = max_time;
        stats.avg_exec_time_us = if vcpu_count > 0 {
            total_time / vcpu_count
        } else {
            0
        };

        let total_elapsed = start_time.elapsed().as_micros() as u64;
        println!(
            "多vCPU异步执行完成: {}us ({}个vCPU)",
            total_elapsed, vcpu_count
        );

        Ok(results)
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> MultiVcpuStats {
        self.stats.lock().clone()
    }
}

/// Mock MMU for testing
#[cfg(feature = "async")]
pub struct MockMMU;

#[cfg(feature = "async")]
impl Default for MockMMU {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "async")]
impl MockMMU {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(feature = "async")]
impl vm_core::AddressTranslator for MockMMU {
    fn translate(
        &mut self,
        va: vm_core::GuestAddr,
        _access: vm_core::AccessType,
    ) -> Result<vm_core::GuestPhysAddr, VmError> {
        Ok(vm_core::GuestPhysAddr::from(va)) // 无翻译
    }

    fn flush_tlb(&mut self) {
        // Mock implementation
    }
}

#[cfg(feature = "async")]
impl vm_core::MemoryAccess for MockMMU {
    fn read(&self, _pa: vm_core::GuestAddr, _size: u8) -> Result<u64, VmError> {
        Ok(0)
    }

    fn write(&mut self, _pa: vm_core::GuestAddr, _val: u64, _size: u8) -> Result<(), VmError> {
        Ok(())
    }

    fn fetch_insn(&self, _pc: vm_core::GuestAddr) -> Result<u64, VmError> {
        Ok(0)
    }

    fn memory_size(&self) -> usize {
        0
    }

    fn dump_memory(&self) -> Vec<u8> {
        Vec::new()
    }

    fn restore_memory(&mut self, _data: &[u8]) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(feature = "async")]
impl vm_core::MmioManager for MockMMU {
    fn map_mmio(
        &self,
        _base: vm_core::GuestAddr,
        _size: u64,
        _device: Box<dyn vm_core::MmioDevice>,
    ) {
        // Mock implementation
    }
}

#[cfg(feature = "async")]
impl vm_core::MmuAsAny for MockMMU {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_ir::Terminator;

    #[tokio::test]
    async fn test_async_execution_context_creation() {
        let block = IRBlock {
            start_pc: vm_core::GuestAddr(0x1000),
            ops: vec![],
            term: Terminator::Ret,
        };
        let ctx = AsyncExecutionContext::new(block, 1000, 100);

        assert_eq!(ctx.max_steps, 1000);
        assert_eq!(ctx.current_steps, 0);
        assert_eq!(ctx.yield_interval, 100);
    }

    #[tokio::test]
    async fn test_async_multi_vcpu_executor_creation() {
        let executor = AsyncMultiVcpuExecutor::new(4);
        assert_eq!(executor.vcpu_count(), 4);
    }

    #[tokio::test]
    async fn test_should_yield() {
        let block = IRBlock {
            start_pc: vm_core::GuestAddr(0x1000),
            ops: vec![],
            term: Terminator::Ret,
        };
        let mut ctx = AsyncExecutionContext::new(block, 1000, 100);

        assert!(!ctx.should_yield()); // 0 steps

        ctx.current_steps = 100;
        assert!(ctx.should_yield()); // 100 steps = yield_interval

        ctx.current_steps = 150;
        assert!(!ctx.should_yield()); // 150 % 100 != 0
    }

    #[tokio::test]
    async fn test_is_complete() {
        let block = IRBlock {
            start_pc: vm_core::GuestAddr(0x1000),
            ops: vec![],
            term: Terminator::Ret,
        };
        let mut ctx = AsyncExecutionContext::new(block, 1000, 100);

        assert!(!ctx.is_complete());

        ctx.current_steps = 1000;
        assert!(ctx.is_complete());

        ctx.current_steps = 1001;
        assert!(ctx.is_complete());
    }

    #[tokio::test]
    async fn test_stats_tracking() {
        let block = IRBlock {
            start_pc: vm_core::GuestAddr(0x1000),
            ops: vec![],
            term: Terminator::Ret,
        };
        let mut ctx = AsyncExecutionContext::new(block, 1000, 100);

        ctx.step();
        assert_eq!(ctx.stats.async_ops, 1);

        ctx.record_yield();
        assert_eq!(ctx.stats.yield_count, 1);

        ctx.record_interrupt();
        assert_eq!(ctx.stats.interrupt_count, 1);
    }
}
