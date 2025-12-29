//! VirtIO Balloon 设备实现
//!
//! 提供内存气球功能，允许动态调整虚拟机内存大小

use crate::virtio::{Queue, VirtioDevice};
use std::sync::{Arc, Mutex};
use vm_core::{MMU, MemoryError, VmError, VmResult};

/// VirtIO Balloon 统计信息
#[derive(Debug, Clone, Default)]
pub struct BalloonStats {
    /// 已分配的页数
    pub pages_allocated: u64,
    /// 已释放的页数
    pub pages_freed: u64,
    /// 总内存页数
    pub total_pages: u64,
    /// 可用内存页数
    pub available_pages: u64,
    /// 已使用的内存页数
    pub used_pages: u64,
}

/// VirtIO Balloon 设备
pub struct VirtioBalloon {
    /// VirtIO队列（inflate和deflate各一个）
    queues: Vec<Queue>,
    /// 当前目标页数
    target_pages: u32,
    /// 实际页数
    actual_pages: u32,
    /// 统计信息
    stats: Arc<Mutex<BalloonStats>>,
    /// 设备状态
    device_status: u32,
    /// 页大小（通常为4KB）
    page_size: u64,
}

impl VirtioBalloon {
    /// 创建新的VirtIO Balloon设备
    pub fn new(page_size: u64) -> Self {
        Self {
            queues: vec![Queue::new(256); 2], // inflate和deflate队列
            target_pages: 0,
            actual_pages: 0,
            stats: Arc::new(Mutex::new(BalloonStats::default())),
            device_status: 0,
            page_size,
        }
    }

    /// Helper to acquire stats lock with error handling
    fn lock_stats(&self) -> VmResult<std::sync::MutexGuard<'_, BalloonStats>> {
        self.stats.lock().map_err(|_| {
            VmError::Memory(MemoryError::PageTableError {
                message: "VirtioBalloon stats lock is poisoned".to_string(),
                level: None,
            })
        })
    }

    /// 设置目标页数
    pub fn set_target_pages(&mut self, pages: u32) {
        self.target_pages = pages;
    }

    /// 获取目标页数
    pub fn target_pages(&self) -> u32 {
        self.target_pages
    }

    /// 获取实际页数
    pub fn actual_pages(&self) -> u32 {
        self.actual_pages
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> BalloonStats {
        match self.lock_stats() {
            Ok(stats) => stats.clone(),
            Err(_) => BalloonStats::default(),
        }
    }

    /// 更新统计信息
    pub fn update_stats(&self, stats: BalloonStats) {
        if let Ok(mut s) = self.stats.lock() {
            *s = stats;
        }
    }

    /// 设置设备状态
    pub fn set_device_status(&mut self, status: u32) {
        self.device_status = status;
    }

    /// 获取设备状态
    pub fn device_status(&self) -> u32 {
        self.device_status
    }

    /// 处理inflate请求（增加内存压力，释放内存）
    fn process_inflate(&mut self, mmu: &mut dyn MMU, chain: &crate::virtio::DescChain) {
        let mut pages_inflated = 0;

        // 读取要inflate的页地址列表
        for desc in &chain.descs {
            if desc.flags & 0x1 == 0 {
                // 可读
                let num_pages = (desc.len / 8) as usize;
                let mut page_data = vec![0u8; num_pages * 8];
                if mmu
                    .read_bulk(vm_core::GuestAddr(desc.addr), &mut page_data)
                    .is_ok()
                {
                    // 将字节数组转换为u64数组
                    let page_addrs: Vec<u64> = page_data
                        .chunks_exact(8)
                        .map(|chunk| {
                            u64::from_le_bytes([
                                chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5],
                                chunk[6], chunk[7],
                            ])
                        })
                        .collect();

                    // 标记这些页为已inflate（在实际实现中，需要与MMU协调释放内存）
                    pages_inflated += page_addrs.len() as u32;
                }
            }
        }

        // 更新统计信息
        if let Ok(mut stats) = self.stats.lock() {
            stats.pages_freed += pages_inflated as u64;
            stats.used_pages = stats.used_pages.saturating_sub(pages_inflated as u64);
        }

        self.actual_pages += pages_inflated;

        // 标记为已使用
        self.queues[0].add_used(
            mmu,
            chain.head_index,
            pages_inflated * self.page_size as u32,
        );
    }

    /// 处理deflate请求（减少内存压力，归还内存）
    fn process_deflate(&mut self, mmu: &mut dyn MMU, chain: &crate::virtio::DescChain) {
        let mut pages_deflated = 0;

        // 读取要deflate的页地址列表
        for desc in &chain.descs {
            if desc.flags & 0x1 == 0 {
                // 可读
                let num_pages = (desc.len / 8) as usize;
                let mut page_data = vec![0u8; num_pages * 8];
                if mmu
                    .read_bulk(vm_core::GuestAddr(desc.addr), &mut page_data)
                    .is_ok()
                {
                    // 将字节数组转换为u64数组
                    let page_addrs: Vec<u64> = page_data
                        .chunks_exact(8)
                        .map(|chunk| {
                            u64::from_le_bytes([
                                chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5],
                                chunk[6], chunk[7],
                            ])
                        })
                        .collect();

                    // 标记这些页为已deflate（在实际实现中，需要与MMU协调归还内存）
                    pages_deflated += page_addrs.len() as u32;
                }
            }
        }

        // 更新统计信息
        if let Ok(mut stats) = self.stats.lock() {
            stats.pages_allocated += pages_deflated as u64;
            stats.used_pages += pages_deflated as u64;
        }

        self.actual_pages = self.actual_pages.saturating_sub(pages_deflated);

        // 标记为已使用
        self.queues[1].add_used(
            mmu,
            chain.head_index,
            pages_deflated * self.page_size as u32,
        );
    }
}

impl VirtioDevice for VirtioBalloon {
    fn device_id(&self) -> u32 {
        5 // VirtIO Balloon device ID
    }

    fn num_queues(&self) -> usize {
        self.queues.len()
    }

    fn get_queue(&mut self, index: usize) -> &mut Queue {
        &mut self.queues[index]
    }

    fn process_queues(&mut self, mmu: &mut dyn MMU) {
        // 处理inflate队列（索引0）
        while let Some(chain) = self.queues[0].pop(mmu) {
            self.process_inflate(mmu, &chain);
        }

        // 处理deflate队列（索引1）
        while let Some(chain) = self.queues[1].pop(mmu) {
            self.process_deflate(mmu, &chain);
        }
    }
}

/// VirtIO Balloon MMIO设备
pub struct VirtioBalloonMmio {
    device: VirtioBalloon,
}

impl VirtioBalloonMmio {
    pub fn new(device: VirtioBalloon) -> Self {
        Self { device }
    }

    pub fn device_mut(&mut self) -> &mut VirtioBalloon {
        &mut self.device
    }

    pub fn device(&self) -> &VirtioBalloon {
        &self.device
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_core::{AddressTranslator, GuestAddr, MemoryAccess, MmioManager, MmuAsAny, VmError};

    struct MockMmu {
        memory: std::collections::HashMap<u64, u8>,
    }

    // 实现AddressTranslator trait
    impl AddressTranslator for MockMmu {
        fn translate(
            &mut self,
            va: GuestAddr,
            _access: vm_core::AccessType,
        ) -> Result<vm_core::GuestPhysAddr, VmError> {
            Ok(va.into())
        }

        fn flush_tlb(&mut self) {}
    }

    // 实现MemoryAccess trait
    impl MemoryAccess for MockMmu {
        fn read(&self, pa: GuestAddr, size: u8) -> Result<u64, VmError> {
            let mut value = 0u64;
            for i in 0..size {
                let byte = self.memory.get(&(pa.0 + i as u64)).copied().unwrap_or(0);
                value |= (byte as u64) << (i * 8);
            }
            Ok(value)
        }

        fn write(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<(), VmError> {
            for i in 0..size {
                let byte = ((val >> (i * 8)) & 0xFF) as u8;
                self.memory.insert(pa.0 + i as u64, byte);
            }
            Ok(())
        }

        fn fetch_insn(&self, _pc: GuestAddr) -> Result<u64, VmError> {
            Ok(0)
        }

        fn read_bulk(&self, pa: GuestAddr, buf: &mut [u8]) -> Result<(), VmError> {
            for (i, byte) in buf.iter_mut().enumerate() {
                *byte = self.memory.get(&(pa.0 + i as u64)).copied().unwrap_or(0);
            }
            Ok(())
        }

        fn write_bulk(&mut self, pa: GuestAddr, buf: &[u8]) -> Result<(), VmError> {
            for (i, &byte) in buf.iter().enumerate() {
                self.memory.insert(pa.0 + i as u64, byte);
            }
            Ok(())
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

    // 实现MmioManager trait
    impl MmioManager for MockMmu {
        fn map_mmio(&self, _base: GuestAddr, _size: u64, _device: Box<dyn vm_core::MmioDevice>) {}
    }

    // 实现MmuAsAny trait
    impl MmuAsAny for MockMmu {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }
}
