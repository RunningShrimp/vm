//! 统一测试工具库
//!
//! 提供公共的测试辅助函数、Mock对象和测试模式，减少重复代码

use std::collections::HashMap;
use vm_core::{AccessType, GuestAddr, GuestPhysAddr, MMU, MmioDevice, VmError};
use vm_core::{AddressTranslator, MemoryAccess, MmioManager, MmuAsAny};
use vm_ir::{IRBlock, IRBuilder, IROp, MemFlags};
use vm_mem::SoftMmu;

/// Mock MMU实现
///
/// 用于测试的简化MMU实现，支持恒等映射和基本内存操作
pub struct MockMMU {
    /// 内存数据
    memory: HashMap<GuestAddr, u8>,
    /// 基址（用于相对地址计算）
    base: GuestAddr,
}

impl MockMMU {
    /// 创建新的Mock MMU
    pub fn new(base: GuestAddr) -> Self {
        Self {
            memory: HashMap::new(),
            base,
        }
    }

    /// 创建默认的Mock MMU（基址为0）
    pub fn default() -> Self {
        Self::new(GuestAddr(0))
    }

    /// 预填充内存数据
    pub fn with_data(mut self, data: &[u8], offset: GuestAddr) -> Self {
        for (i, &byte) in data.iter().enumerate() {
            self.memory.insert(GuestAddr(offset.0 + i as u64), byte);
        }
        self
    }
}

impl AddressTranslator for MockMMU {
    fn translate(&mut self, va: GuestAddr, _access: AccessType) -> Result<GuestPhysAddr, VmError> {
        // 恒等映射
        Ok(GuestPhysAddr(va.0))
    }

    fn flush_tlb(&mut self) {
        // Mock实现：无操作
    }
}

impl MemoryAccess for MockMMU {
    fn read(&self, addr: GuestAddr, size: u8) -> Result<u64, VmError> {
        let offset = (addr.0 - self.base.0) as usize;
        let mut val = 0u64;

        for i in 0..size as usize {
            let byte = self.memory.get(&GuestAddr(self.base.0 + (offset + i) as u64)).copied().unwrap_or(0);
            val |= (byte as u64) << (i * 8);
        }

        Ok(val)
    }

    fn write(&mut self, addr: GuestAddr, val: u64, size: u8) -> Result<(), VmError> {
        for i in 0..size as usize {
            let byte = ((val >> (i * 8)) & 0xFF) as u8;
            self.memory.insert(GuestAddr(addr.0 + i as u64), byte);
        }
        Ok(())
    }

    fn fetch_insn(&self, pc: GuestAddr) -> Result<u64, VmError> {
        self.read(pc, 8)
    }

    fn read_bulk(&self, pa: GuestAddr, buf: &mut [u8]) -> Result<(), VmError> {
        for (i, byte) in buf.iter_mut().enumerate() {
            *byte = self.memory.get(&GuestAddr(pa.0 + i as u64)).copied().unwrap_or(0);
        }
        Ok(())
    }

    fn write_bulk(&mut self, pa: GuestAddr, buf: &[u8]) -> Result<(), VmError> {
        for (i, &byte) in buf.iter().enumerate() {
            self.memory.insert(vm_core::GuestAddr(pa.0 + i as u64), byte);
        }
        Ok(())
    }

    fn memory_size(&self) -> usize {
        self.memory.len()
    }

    fn dump_memory(&self) -> Vec<u8> {
        let max_addr = self.memory.keys().max().copied().unwrap_or(GuestAddr(0));
        let mut result = vec![0u8; (max_addr.0 + 1) as usize];
        for (&addr, &byte) in &self.memory {
            if addr < GuestAddr(result.len() as u64) {
                result[addr.0 as usize] = byte;
            }
        }
        result
    }

    fn restore_memory(&mut self, data: &[u8]) -> Result<(), String> {
        self.memory.clear();
        for (i, &byte) in data.iter().enumerate() {
            self.memory.insert(GuestAddr(i as u64), byte);
        }
        Ok(())
    }
}

impl MmioManager for MockMMU {
    fn map_mmio(&self, _base: GuestAddr, _size: u64, _device: Box<dyn MmioDevice>) {
        // Mock实现：无操作
    }
}

impl MmuAsAny for MockMMU {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// 故障注入Mock MMU
///
/// 用于测试错误处理路径
pub struct FaultyMMU;

impl AddressTranslator for FaultyMMU {
    fn translate(&mut self, va: GuestAddr, access: AccessType) -> Result<GuestPhysAddr, VmError> {
        Err(vm_core::VmError::Execution(vm_core::ExecutionError::Fault(
            vm_core::Fault::PageFault { 
                addr: va, 
                access_type: access, 
                is_write: false, 
                is_user: false 
            },
        )))
    }

    fn flush_tlb(&mut self) {}
}

impl MemoryAccess for FaultyMMU {
    fn read(&self, _pa: GuestAddr, _size: u8) -> Result<u64, VmError> {
        Err(VmError::Execution(vm_core::ExecutionError::FetchFailed {
            pc: _pa,
            message: "Faulty MMU always fails".to_string(),
        }))
    }

    fn write(&mut self, _pa: GuestAddr, _val: u64, _size: u8) -> Result<(), VmError> {
        Err(VmError::Execution(vm_core::ExecutionError::FetchFailed {
            pc: _pa,
            message: "Faulty MMU always fails".to_string(),
        }))
    }

    fn fetch_insn(&self, _pc: GuestAddr) -> Result<u64, VmError> {
        Err(VmError::Execution(vm_core::ExecutionError::FetchFailed {
            pc: _pc,
            message: "Faulty MMU always fails".to_string(),
        }))
    }

    fn read_bulk(&self, _pa: GuestAddr, _buf: &mut [u8]) -> Result<(), VmError> {
        Err(VmError::Execution(vm_core::ExecutionError::FetchFailed {
            pc: _pa,
            message: "Faulty MMU always fails".to_string(),
        }))
    }

    fn write_bulk(&mut self, _pa: GuestAddr, _buf: &[u8]) -> Result<(), VmError> {
        Err(VmError::Execution(vm_core::ExecutionError::FetchFailed {
            pc: _pa,
            message: "Faulty MMU always fails".to_string(),
        }))
    }

    fn memory_size(&self) -> usize {
        0
    }

    fn dump_memory(&self) -> Vec<u8> {
        Vec::new()
    }

    fn restore_memory(&mut self, _data: &[u8]) -> Result<(), String> {
        Err("Not supported".to_string())
    }
}

impl MmioManager for FaultyMMU {
    fn map_mmio(&self, _base: GuestAddr, _size: u64, _device: Box<dyn MmioDevice>) {}
}

impl MmuAsAny for FaultyMMU {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// IR块构建器工具
pub struct IRBlockBuilder;

impl IRBlockBuilder {
    /// 创建简单的算术运算IR块
    pub fn create_simple_arithmetic(pc: GuestAddr) -> IRBlock {
        let mut builder = IRBuilder::new(pc);
        builder.push(IROp::MovImm { dst: 1, imm: 42 });
        builder.push(IROp::MovImm { dst: 2, imm: 24 });
        builder.push(IROp::Add {
            dst: 0,
            src1: 1,
            src2: 2,
        });
        builder.push(IROp::Mul {
            dst: 0,
            src1: 0,
            src2: 3,
        });
        builder.push(IROp::Sub {
            dst: 0,
            src1: 0,
            src2: 1,
        });
        builder.build()
    }

    /// 创建复杂IR块（用于性能测试）
    pub fn create_complex(pc: GuestAddr, num_ops: usize) -> IRBlock {
        let mut builder = IRBuilder::new(pc);

        for i in 0..num_ops {
            match i % 4 {
                0 => {
                    builder.push(IROp::Add {
                        dst: 0,
                        src1: 1,
                        src2: 2,
                    });
                    builder.push(IROp::MovImm {
                        dst: i as u32,
                        imm: (i * 10) as u64,
                    });
                }
                1 => {
                    builder.push(IROp::Sub {
                        dst: 0,
                        src1: 0,
                        src2: 3,
                    });
                    builder.push(IROp::MovImm {
                        dst: (i + 1) as u32,
                        imm: (i * 15) as u64,
                    });
                }
                2 => {
                    builder.push(IROp::Mul {
                        dst: 0,
                        src1: 0,
                        src2: 2,
                    });
                    builder.push(IROp::MovImm {
                        dst: (i + 2) as u32,
                        imm: (i * 20) as u64,
                    });
                }
                3 => {
                    builder.push(IROp::Xor {
                        dst: 0,
                        src1: 1,
                        src2: 3,
                    });
                    builder.push(IROp::MovImm {
                        dst: (i + 3) as u32,
                        imm: (i * 25) as u64,
                    });
                }
                _ => unreachable!(),
            }
        }

        builder.build()
    }

    /// 创建带内存访问的IR块
    pub fn create_with_memory(pc: GuestAddr, base_addr: vm_ir::RegId) -> IRBlock {
        let mut builder = IRBuilder::new(pc);

        for i in 0..10 {
            builder.push(IROp::Load {
                dst: 0,
                base: base_addr,
                size: 8,
                offset: (i * 8) as i64,
                flags: MemFlags {
                    volatile: false,
                    atomic: false,
                    align: 8,
                    fence_before: false,
                    fence_after: false,
                    order: vm_ir::MemOrder::None,
                },
            });
            builder.push(IROp::Add {
                dst: 0,
                src1: 0,
                src2: 1,
            });
            builder.push(IROp::Store {
                base: base_addr,
                src: 0,
                size: 8,
                offset: (i * 8) as i64,
                flags: MemFlags {
                    volatile: false,
                    atomic: false,
                    align: 8,
                    fence_before: false,
                    fence_after: false,
                    order: vm_ir::MemOrder::None,
                },
            });
        }

        builder.build()
    }

    /// 创建带分支的IR块
    pub fn create_with_branch(pc: GuestAddr, target: GuestAddr) -> IRBlock {
        let mut builder = IRBuilder::new(pc);
        builder.push(IROp::MovImm { dst: 1, imm: 1 });
        builder.push(IROp::CmpEq {
            dst: 31,
            lhs: 1,
            rhs: 1,
        });
        builder.set_term(vm_ir::Terminator::CondJmp {
            cond: 31,
            target_true: target,
            target_false: pc + 16,
        });
        builder.build()
    }
}

/// 测试VM配置构建器
pub struct TestVmConfigBuilder {
    memory_size: usize,
    vcpu_count: usize,
    use_hugepages: bool,
}

impl TestVmConfigBuilder {
    pub fn new() -> Self {
        Self {
            memory_size: 0x1000000, // 16MB
            vcpu_count: 1,
            use_hugepages: false,
        }
    }

    pub fn with_memory_size(mut self, size: usize) -> Self {
        self.memory_size = size;
        self
    }

    pub fn with_vcpu_count(mut self, count: usize) -> Self {
        self.vcpu_count = count;
        self
    }

    pub fn with_hugepages(mut self, enable: bool) -> Self {
        self.use_hugepages = enable;
        self
    }

    pub fn build_mmu(&self) -> SoftMmu {
        SoftMmu::new(self.memory_size, self.use_hugepages)
    }
}

impl Default for TestVmConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// 并发测试工具
pub struct ConcurrentTestHelper;

impl ConcurrentTestHelper {
    /// 运行并发测试模式
    ///
    /// 标准化的并发测试模式，用于测试多线程/协程安全性
    pub async fn run_concurrent_test<F, Fut, R>(
        num_threads: usize,
        iterations_per_thread: usize,
        test_fn: F,
    ) -> Vec<R>
    where
        F: Fn(usize, usize) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        use tokio::task;

        let mut handles = Vec::new();

        for thread_id in 0..num_threads {
            let test_fn_clone = test_fn.clone();
            let handle = task::spawn(async move {
                let mut results = Vec::new();
                for iteration in 0..iterations_per_thread {
                    let result = test_fn_clone(thread_id, iteration).await;
                    results.push(result);
                }
                results
            });
            handles.push(handle);
        }

        let mut all_results = Vec::new();
        for handle in handles {
            let results = handle.await.unwrap();
            all_results.extend(results);
        }

        all_results
    }

    /// 运行同步并发测试（使用线程）
    pub fn run_sync_concurrent_test<F, R>(
        num_threads: usize,
        iterations_per_thread: usize,
        test_fn: F,
    ) -> Vec<R>
    where
        F: Fn(usize, usize) -> R + Send + Sync + Clone + 'static,
        R: Send + 'static,
    {
        use std::sync::Arc;
        use std::thread;

        let test_fn = Arc::new(test_fn);
        let mut handles = Vec::new();

        for thread_id in 0..num_threads {
            let test_fn_clone = Arc::clone(&test_fn);
            let handle = thread::spawn(move || {
                let mut results = Vec::new();
                for iteration in 0..iterations_per_thread {
                    let result = test_fn_clone(thread_id, iteration);
                    results.push(result);
                }
                results
            });
            handles.push(handle);
        }

        let mut all_results = Vec::new();
        for handle in handles {
            let results = handle.join().unwrap();
            all_results.extend(results);
        }

        all_results
    }
}

/// 内存操作工具函数
pub struct MemoryTestUtils;

impl MemoryTestUtils {
    /// 统一的write_bulk实现
    pub fn write_bulk(mmu: &mut dyn MMU, addr: GuestAddr, data: &[u8]) -> Result<(), VmError> {
        mmu.write_bulk(addr, data)
    }

    /// 统一的read_bulk实现
    pub fn read_bulk(mmu: &dyn MMU, addr: GuestAddr, len: usize) -> Result<Vec<u8>, VmError> {
        let mut buf = vec![0u8; len];
        mmu.read_bulk(addr, &mut buf)?;
        Ok(buf)
    }

    /// 写入对齐的数据
    pub fn write_aligned(
        mmu: &mut dyn MMU,
        addr: GuestAddr,
        val: u64,
        size: u8,
    ) -> Result<(), VmError> {
        mmu.write(addr, val, size)
    }

    /// 读取对齐的数据
    pub fn read_aligned(mmu: &dyn MMU, addr: GuestAddr, size: u8) -> Result<u64, VmError> {
        mmu.read(addr, size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_mmu_basic() {
        let mut mmu = MockMMU::default();
        mmu.write(GuestAddr(0x100), 0x12345678, 4).unwrap();
        let val = mmu.read(GuestAddr(0x100), 4).unwrap();
        assert_eq!(val, 0x12345678);
    }

    #[test]
    fn test_ir_block_builder() {
        let block = IRBlockBuilder::create_simple_arithmetic(GuestAddr(0x1000));
        assert_eq!(block.start_pc, GuestAddr(0x1000));
        assert!(!block.ops.is_empty());
    }

    #[test]
    fn test_vm_config_builder() {
        let config = TestVmConfigBuilder::new()
            .with_memory_size(0x2000000)
            .with_vcpu_count(4);
        let mmu = config.build_mmu();
        assert_eq!(mmu.memory_size(), 0x2000000);
    }

    #[tokio::test]
    async fn test_concurrent_helper() {
        let results =
            ConcurrentTestHelper::run_concurrent_test(4, 10, |thread_id, iteration| async move {
                thread_id * 1000 + iteration
            })
            .await;
        assert_eq!(results.len(), 40);
    }
}
