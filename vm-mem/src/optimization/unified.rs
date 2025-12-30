//! 统一内存管理接口
//!
//! 提供跨vm-mem、vm-core的统一内存管理API

use std::sync::Arc;
use vm_core::{GuestAddr, GuestPhysAddr, VmResult};
use crate::tlb::core::unified::UnifiedTlb;

/// 统一内存管理器
pub trait UnifiedMemoryManager: Send + Sync {
    /// 读取虚拟内存
    fn read(&self, addr: GuestAddr, size: u8) -> VmResult<u64>;

    /// 写入虚拟内存
    fn write(&self, addr: GuestAddr, value: u64, size: u8) -> VmResult<()>;

    /// 翻译虚拟地址到物理地址
    fn translate(&self, addr: GuestAddr) -> VmResult<GuestPhysAddr>;

    /// 获取内存大小
    fn size(&self) -> usize;

    /// 获取使用统计
    fn stats(&self) -> MemoryStats;
}

/// 内存统计信息
#[derive(Debug, Clone, Copy, Default)]
pub struct MemoryStats {
    /// 总内存大小
    pub total_size: usize,
    /// 已使用内存
    pub used_memory: usize,
    /// TLB命中次数
    pub tlb_hits: u64,
    /// TLB未命中次数
    pub tlb_misses: u64,
    /// 页面错误次数
    pub page_faults: u64,
}

/// 软件MMU包装器
pub struct SoftMmuWrapper {
    inner: Arc<crate::SoftMmu>,
    tlb: Arc<crate::tlb::unified::BasicTlb>,
}

impl SoftMmuWrapper {
    /// 创建新的包装器
    pub fn new(mmu: Arc<crate::SoftMmu>, tlb: Arc<crate::tlb::unified::BasicTlb>) -> Self {
        Self { inner: mmu, tlb }
    }

    /// 获取内部MMU
    pub fn inner(&self) -> &Arc<crate::SoftMmu> {
        &self.inner
    }

    /// 获取TLB
    pub fn tlb(&self) -> &Arc<crate::tlb::unified::BasicTlb> {
        &self.tlb
    }
}

impl UnifiedMemoryManager for SoftMmuWrapper {
    fn read(&self, addr: GuestAddr, size: u8) -> VmResult<u64> {
        // 简化实现：先查TLB
        if let Some(entry) = self.tlb.lookup(addr, vm_core::AccessType::Read) {
            let _phys_addr = entry.gpa;
            // 注意：无法通过Arc调用可变方法，这里返回占位值
            return Ok(size as u64);
        }

        // TLB未命中，返回占位值
        Ok(size as u64)
    }

    fn write(&self, addr: GuestAddr, _value: u64, _size: u8) -> VmResult<()> {
        // 先查TLB
        if let Some(entry) = self.tlb.lookup(addr, vm_core::AccessType::Write) {
            let _phys_addr = entry.gpa;
            // 注意：无法通过Arc调用可变方法，这里简化处理
            return Ok(());
        }

        // TLB未命中，简化处理
        Ok(())
    }

    fn translate(&self, addr: GuestAddr) -> VmResult<GuestPhysAddr> {
        // 先查TLB
        if let Some(entry) = self.tlb.lookup(addr, vm_core::AccessType::Read) {
            return Ok(entry.gpa);
        }

        // TLB未命中，返回占位值
        Ok(vm_core::GuestPhysAddr(0))
    }

    fn size(&self) -> usize {
        // 返回占位值
        0
    }

    fn stats(&self) -> MemoryStats {
        // 返回占位统计
        MemoryStats {
            total_size: 0,
            used_memory: 0,
            tlb_hits: 0,
            tlb_misses: 0,
            page_faults: 0,
        }
    }
}

/// 物理内存管理器
pub trait PhysicalMemoryManager: Send + Sync {
    /// 分配物理内存
    fn allocate(&mut self, size: usize) -> VmResult<GuestPhysAddr>;

    /// 释放物理内存
    fn deallocate(&mut self, addr: GuestPhysAddr) -> VmResult<()>;

    /// 读取物理内存
    fn read(&self, addr: GuestPhysAddr, size: u8) -> VmResult<u64>;

    /// 写入物理内存
    fn write(&self, addr: GuestPhysAddr, value: u64, size: u8) -> VmResult<()>;
}

/// 内存池管理器
///
/// 提供高效的内存分配和释放
pub struct MemoryPool {
    /// 内存块列表
    blocks: Vec<MemoryBlock>,
    /// 空闲块
    free_blocks: Vec<usize>,
    /// 下一个分配地址
    next_addr: GuestPhysAddr,
}

/// 内存块
#[derive(Debug, Clone)]
struct MemoryBlock {
    /// 物理地址
    addr: GuestPhysAddr,
    /// 大小
    size: usize,
    /// 是否已使用
    used: bool,
}

impl MemoryPool {
    /// 创建新的内存池
    pub fn new() -> Self {
        Self {
            blocks: vec![],
            free_blocks: vec![],
            next_addr: GuestPhysAddr(0),
        }
    }

    /// 分配内存块
    pub fn allocate(&mut self, size: usize) -> VmResult<GuestPhysAddr> {
        // 先尝试从空闲块中分配
        for &idx in &self.free_blocks {
            let block = &self.blocks[idx];
            if !block.used && block.size >= size {
                // 找到合适的块
                let addr = block.addr;
                self.blocks[idx].used = true;
                self.free_blocks.retain(|&i| i != idx);
                return Ok(addr);
            }
        }

        // 没有合适的空闲块，分配新块
        let addr = self.next_addr;
        self.blocks.push(MemoryBlock {
            addr,
            size,
            used: true,
        });

        self.next_addr = GuestPhysAddr(self.next_addr.0 + size as u64);
        Ok(addr)
    }

    /// 释放内存块
    pub fn deallocate(&mut self, addr: GuestPhysAddr) -> VmResult<()> {
        // 找到对应的块
        for (idx, block) in self.blocks.iter().enumerate() {
            if block.addr == addr {
                self.blocks[idx].used = false;
                self.free_blocks.push(idx);
                return Ok(());
            }
        }

        Err(vm_core::VmError::Memory(vm_core::error::MemoryError::InvalidAddress(
            vm_core::GuestAddr(addr.0),
        )))
    }
}

impl Default for MemoryPool {
    fn default() -> Self {
        Self::new()
    }
}

impl PhysicalMemoryManager for MemoryPool {
    fn allocate(&mut self, size: usize) -> VmResult<GuestPhysAddr> {
        // 先尝试从空闲块中分配
        for &idx in &self.free_blocks {
            let block = &self.blocks[idx];
            if !block.used && block.size >= size {
                // 找到合适的块
                let addr = block.addr;
                self.blocks[idx].used = true;
                self.free_blocks.retain(|&i| i != idx);
                return Ok(addr);
            }
        }

        // 没有合适的空闲块，分配新块
        let addr = self.next_addr;
        self.blocks.push(MemoryBlock {
            addr,
            size,
            used: true,
        });

        self.next_addr = GuestPhysAddr(self.next_addr.0 + size as u64);
        Ok(addr)
    }

    fn deallocate(&mut self, addr: GuestPhysAddr) -> VmResult<()> {
        // 找到对应的块
        for (idx, block) in self.blocks.iter().enumerate() {
            if block.addr == addr {
                self.blocks[idx].used = false;
                self.free_blocks.push(idx);
                return Ok(());
            }
        }

        Err(vm_core::VmError::Memory(vm_core::error::MemoryError::InvalidAddress(
            vm_core::GuestAddr(addr.0),
        )))
    }

    fn read(&self, _addr: GuestPhysAddr, _size: u8) -> VmResult<u64> {
        // TODO: 实现实际读取
        Ok(0)
    }

    fn write(&self, _addr: GuestPhysAddr, _value: u64, _size: u8) -> VmResult<()> {
        // TODO: 实现实际写入
        Ok(())
    }
}

/// 内存管理器工厂
pub struct MemoryManagerFactory;

impl MemoryManagerFactory {
    /// 创建软MMU包装器
    pub fn create_soft_mmu(
        mmu: Arc<crate::SoftMmu>,
        tlb: Arc<crate::tlb::unified::BasicTlb>,
    ) -> Box<dyn UnifiedMemoryManager> {
        Box::new(SoftMmuWrapper::new(mmu, tlb))
    }

    /// 创建内存池
    pub fn create_memory_pool() -> MemoryPool {
        MemoryPool::new()
    }
}

/// 批量内存操作
pub trait BatchMemoryOps {
    /// 批量读取
    fn read_batch(&self, addrs: &[GuestAddr], size: u8) -> VmResult<Vec<u64>>;

    /// 批量写入
    fn write_batch(&self, ops: &[(GuestAddr, u64, u8)]) -> VmResult<()>;

    /// 批量翻译
    fn translate_batch(&self, addrs: &[GuestAddr]) -> VmResult<Vec<GuestPhysAddr>>;
}

impl<T: UnifiedMemoryManager + ?Sized> BatchMemoryOps for T {
    fn read_batch(&self, addrs: &[GuestAddr], size: u8) -> VmResult<Vec<u64>> {
        let mut results = Vec::with_capacity(addrs.len());
        for &addr in addrs {
            results.push(self.read(addr, size)?);
        }
        Ok(results)
    }

    fn write_batch(&self, ops: &[(GuestAddr, u64, u8)]) -> VmResult<()> {
        for &(addr, value, size) in ops {
            self.write(addr, value, size)?;
        }
        Ok(())
    }

    fn translate_batch(&self, addrs: &[GuestAddr]) -> VmResult<Vec<GuestPhysAddr>> {
        let mut results = Vec::with_capacity(addrs.len());
        for &addr in addrs {
            results.push(self.translate(addr)?);
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_pool() {
        let mut pool = MemoryPool::new();

        let addr1 = pool.allocate(1024).unwrap();
        let addr2 = pool.allocate(2048).unwrap();

        assert_ne!(addr1, addr2);

        pool.deallocate(addr1).unwrap();

        // 释放后可以重新分配
        let addr3 = pool.allocate(512).unwrap();
        // addr3应该重用addr1或新的地址
    }

    #[test]
    fn test_memory_stats() {
        let stats = MemoryStats::default();
        assert_eq!(stats.total_size, 0);
        assert_eq!(stats.used_memory, 0);
    }

    #[test]
    fn test_batch_ops() {
        // TODO: 添加实际的批量操作测试
    }
}
