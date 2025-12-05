//! 异步执行引擎接口
//!
//! 提供异步版本的ExecutionEngine trait，支持异步执行和异步内存访问。
//!
//! # 使用场景
//!
//! - 异步I/O操作
//! - 异步内存访问（使用AsyncMMU）
//! - 协程池执行
//! - 高并发场景
//!
//! # 示例
//!
//! ```rust,ignore
//! use vm_core::{AsyncExecutionEngine, AsyncMMU};
//! use vm_ir::IRBlock;
//!
//! async fn execute_block<E: AsyncExecutionEngine<IRBlock>>(
//!     engine: &mut E,
//!     mmu: &mut dyn AsyncMMU,
//!     block: &IRBlock,
//! ) -> Result<ExecResult, VmError> {
//!     engine.run_async(mmu, block).await
//! }
//! ```

use crate::{ExecResult, GuestAddr, VmError};

#[cfg(feature = "async")]
use async_trait::async_trait;

/// 异步执行引擎trait
///
/// 这是`ExecutionEngine`的异步版本，支持异步执行和异步内存访问。
/// 实现此trait的执行引擎可以使用`AsyncMMU`进行异步内存操作。
///
/// # 与同步版本的关系
///
/// - 同步`ExecutionEngine`：使用`&mut dyn MMU`，阻塞式执行
/// - 异步`AsyncExecutionEngine`：使用`&mut dyn AsyncMMU`，非阻塞式执行
///
/// # 实现建议
///
/// - 如果执行引擎支持异步，应该同时实现`ExecutionEngine`和`AsyncExecutionEngine`
/// - 异步版本可以避免阻塞，提高并发性能
/// - 对于I/O密集型操作，优先使用异步版本
#[cfg(feature = "async")]
#[async_trait]
pub trait AsyncExecutionEngine<B>: Send + Sync
where
    B: Send + Sync,
{
    /// 异步执行单个基本块
    ///
    /// # 参数
    /// - `mmu`: 异步内存管理单元
    /// - `block`: 要执行的基本块
    ///
    /// # 返回
    /// - `Ok(ExecResult)`: 执行成功，返回执行结果
    /// - `Err(VmError)`: 执行失败，返回错误信息
    ///
    /// # 异步行为
    ///
    /// 此方法应该：
    /// - 使用异步内存访问（通过`AsyncMMU`）
    /// - 避免阻塞线程
    /// - 支持并发执行多个基本块
    async fn run_async(
        &mut self,
        mmu: &mut dyn crate::AsyncMMU,
        block: &B,
    ) -> Result<ExecResult, VmError>;

    /// 异步获取指定编号的寄存器值
    ///
    /// # 参数
    /// - `idx`: 寄存器编号
    ///
    /// # 返回
    /// 寄存器值
    async fn get_reg_async(&self, idx: usize) -> u64;

    /// 异步设置指定编号的寄存器值
    ///
    /// # 参数
    /// - `idx`: 寄存器编号
    /// - `val`: 寄存器值
    async fn set_reg_async(&mut self, idx: usize, val: u64);

    /// 异步获取程序计数器（PC）
    ///
    /// # 返回
    /// 当前PC值
    async fn get_pc_async(&self) -> GuestAddr;

    /// 异步设置程序计数器（PC）
    ///
    /// # 参数
    /// - `pc`: 新的PC值
    async fn set_pc_async(&mut self, pc: GuestAddr);

    /// 异步批量执行多个基本块
    ///
    /// # 参数
    /// - `mmu`: 异步内存管理单元
    /// - `blocks`: 要执行的基本块列表
    ///
    /// # 返回
    /// 执行结果列表
    ///
    /// # 性能优化
    ///
    /// 实现应该尽可能并行执行多个基本块，提高吞吐量。
    async fn run_many_async(
        &mut self,
        mmu: &mut dyn crate::AsyncMMU,
        blocks: &[B],
    ) -> Result<Vec<ExecResult>, VmError> {
        let mut results = Vec::with_capacity(blocks.len());
        for block in blocks {
            let result = self.run_async(mmu, block).await?;
            results.push(result);
        }
        Ok(results)
    }
}

/// 执行引擎适配器
///
/// 将同步`ExecutionEngine`适配为异步`AsyncExecutionEngine`。
/// 使用`spawn_blocking`在异步上下文中执行同步代码。
///
/// # 性能考虑
///
/// 此适配器会在线程池中执行同步代码，可能增加延迟。
/// 如果可能，应该直接实现`AsyncExecutionEngine`以获得更好的性能。
pub struct ExecutionEngineAdapter<E> {
    engine: E,
}

impl<E> ExecutionEngineAdapter<E> {
    /// 创建新的适配器
    pub fn new(engine: E) -> Self {
        Self { engine }
    }

    /// 获取内部引擎的引用
    pub fn engine(&self) -> &E {
        &self.engine
    }

    /// 获取内部引擎的可变引用
    pub fn engine_mut(&mut self) -> &mut E {
        &mut self.engine
    }

    /// 消费适配器，返回内部引擎
    pub fn into_engine(self) -> E {
        self.engine
    }
}

#[cfg(feature = "async")]
#[async_trait]
impl<E, B> AsyncExecutionEngine<B> for ExecutionEngineAdapter<E>
where
    E: crate::ExecutionEngine<B> + Send + Sync,
    B: Send + Sync + Clone,
{
    async fn run_async(
        &mut self,
        _mmu: &mut dyn crate::AsyncMMU,
        block: &B,
    ) -> Result<ExecResult, VmError> {
        // 注意：这是一个占位符实现
        // 实际实现需要：
        // 1. 将AsyncMMU适配为同步MMU（如果AsyncMMU提供同步接口）
        // 2. 或者使用spawn_blocking在线程池中执行同步代码
        // 
        // 当前实现仅作为接口定义，具体适配逻辑需要根据AsyncMMU的实际接口实现
        
        // 临时实现：返回错误，提示需要实现适配器
        Err(VmError::Core(crate::CoreError::Internal {
            message: "ExecutionEngineAdapter需要实现AsyncMMU到MMU的适配".to_string(),
            module: "AsyncExecutionEngine".to_string(),
        }))
        
        // TODO: 实现完整的适配逻辑
        // 示例：
        // let block_clone = block.clone();
        // let mmu_sync = mmu.to_sync(); // 需要AsyncMMU提供此方法
        // tokio::task::spawn_blocking(move || {
        //     self.engine.run(&mut mmu_sync, &block_clone)
        // }).await.map_err(...)
    }

    async fn get_reg_async(&self, idx: usize) -> u64 {
        self.engine.get_reg(idx)
    }

    async fn set_reg_async(&mut self, idx: usize, val: u64) {
        self.engine.set_reg(idx, val);
    }

    async fn get_pc_async(&self) -> GuestAddr {
        self.engine.get_pc()
    }

    async fn set_pc_async(&mut self, pc: GuestAddr) {
        self.engine.set_pc(pc);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ExecResult, ExecStats, ExecStatus};

    // 测试用例将在实现具体的执行引擎后添加
}

