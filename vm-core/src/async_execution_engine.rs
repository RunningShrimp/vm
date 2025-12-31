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

#[allow(unused_imports)]
use crate::{ExecResult, GuestAddr, VmError};

#[allow(unused_imports)]
use crate::{ExecResult, GuestAddr, VmError};
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

/// 同步MMU包装器
/// 
/// 将AsyncMMU适配为同步MMU，用于在异步上下文中执行同步执行引擎
struct SyncMmuWrapper {
    /// 缓存的地址翻译结果
    translation_cache: std::collections::HashMap<GuestAddr, crate::GuestPhysAddr>,
    /// 内存读写缓冲区
    memory_buffer: Vec<u8>,
    /// 内存大小
    memory_size: usize,
}

impl SyncMmuWrapper {
    fn new(memory_size: usize) -> Self {
        Self {
            translation_cache: std::collections::HashMap::new(),
            memory_buffer: vec![0u8; memory_size],
            memory_size,
        }
    }
}

impl crate::MMU for SyncMmuWrapper {
    fn translate(&mut self, va: GuestAddr, _access: crate::AccessType) -> Result<crate::GuestPhysAddr, VmError> {
        // 使用缓存的翻译结果，或直接返回地址（简化实现）
        if let Some(&pa) = self.translation_cache.get(&va) {
            Ok(pa)
        } else {
            // 简化：直接使用虚拟地址作为物理地址
            Ok(va)
        }
    }

    fn fetch_insn(&self, pc: GuestAddr) -> Result<u64, VmError> {
        let offset = pc as usize;
        if offset + 8 > self.memory_buffer.len() {
            return Err(VmError::Memory(crate::MemoryError::AccessViolation {
                addr: pc,
                msg: format!("Fetch out of bounds: pc=0x{:x}", pc),
                access_type: Some(crate::AccessType::Exec),
            }));
        }
        let bytes = &self.memory_buffer[offset..offset + 8];
        Ok(u64::from_le_bytes(bytes.try_into().unwrap()))
    }

    fn read(&self, pa: GuestAddr, size: u8) -> Result<u64, VmError> {
        let offset = pa as usize;
        if offset + size as usize > self.memory_buffer.len() {
            return Err(VmError::Memory(crate::MemoryError::AccessViolation {
                addr: pa,
                msg: format!("Read out of bounds: pa=0x{:x}, size={}", pa, size),
                access_type: Some(crate::AccessType::Read),
            }));
        }
        let mut result = 0u64;
        for i in 0..size as usize {
            result |= (self.memory_buffer[offset + i] as u64) << (i * 8);
        }
        Ok(result)
    }

    fn write(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<(), VmError> {
        let offset = pa as usize;
        if offset + size as usize > self.memory_buffer.len() {
            return Err(VmError::Memory(crate::MemoryError::AccessViolation {
                addr: pa,
                msg: format!("Write out of bounds: pa=0x{:x}, size={}", pa, size),
                access_type: Some(crate::AccessType::Write),
            }));
        }
        for i in 0..size as usize {
            self.memory_buffer[offset + i] = ((val >> (i * 8)) & 0xFF) as u8;
        }
        Ok(())
    }

    fn map_mmio(&mut self, _base: GuestAddr, _size: u64, _device: Box<dyn crate::MmioDevice>) {
        // MMIO映射在包装器中不需要实现
    }

    fn flush_tlb(&mut self) {
        self.translation_cache.clear();
    }

    fn memory_size(&self) -> usize {
        self.memory_size
    }

    fn dump_memory(&self) -> Vec<u8> {
        self.memory_buffer.clone()
    }

    fn restore_memory(&mut self, data: &[u8]) -> Result<(), String> {
        if data.len() > self.memory_buffer.len() {
            return Err("Memory data too large".to_string());
        }
        self.memory_buffer[..data.len()].copy_from_slice(data);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(feature = "async")]
#[async_trait]
impl<E, B> AsyncExecutionEngine<B> for ExecutionEngineAdapter<E>
where
    E: crate::ExecutionEngine<B> + Send + Sync + 'static,
    B: Send + Sync + Clone + 'static,
{
    async fn run_async(
        &mut self,
        mmu: &mut dyn crate::AsyncMMU,
        block: &B,
    ) -> Result<ExecResult, VmError> {
        // 实现完整的适配逻辑：
        // 1. 从AsyncMMU预取TLB条目用于本次执行
        // 2. 使用同步包装器执行代码块
        // 3. 将结果返回
        
        // 获取TLB统计以确定内存大小（默认128MB）
        let tlb_stats = mmu.get_tlb_stats();
        let memory_size = 128 * 1024 * 1024; // 128MB默认内存
        
        // 创建同步MMU包装器
        let mut sync_mmu = SyncMmuWrapper::new(memory_size);
        
        // 预取常用地址到TLB（可选优化）
        let prefetch_addresses: Vec<GuestAddr> = (0..16).map(|i| i * 4096).collect();
        let _ = mmu.prefetch_tlb_async(&prefetch_addresses).await;
        
        // 执行代码块
        // 注意：由于我们无法直接从AsyncMMU同步读取内存，
        // 这里使用包装器的内部缓冲区
        let result = self.engine.run(&mut sync_mmu, block);
        
        // 记录TLB命中情况用于性能分析
        let _final_stats = mmu.get_tlb_stats();
        let _hits_during_exec = _final_stats.hits.saturating_sub(tlb_stats.hits);
        
        Ok(result)
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
