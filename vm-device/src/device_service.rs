use crate::block::VirtioBlockMmio;
use vm_core::{GuestAddr, MMU};

pub struct DeviceService {
    pub block_mmio: Option<VirtioBlockMmio>,
}

impl Default for DeviceService {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceService {
    pub fn new() -> Self {
        Self { block_mmio: None }
    }

    pub fn attach_block(&mut self, dev: VirtioBlockMmio) {
        self.block_mmio = Some(dev);
    }

    pub fn process_all(&mut self, mmu: &mut dyn MMU) {
        if let Some(dev) = &mut self.block_mmio {
            // 验证MMIO设备的内存地址有效性
            // 检查描述符表地址
            if dev.desc_addr != GuestAddr(0) {
                let _ = mmu.translate(dev.desc_addr, vm_core::AccessType::Read);
            }
            // 检查可用环形缓冲区地址
            if dev.avail_addr != GuestAddr(0) {
                let _ = mmu.translate(dev.avail_addr, vm_core::AccessType::Read);
            }
            // 检查已用环形缓冲区地址
            if dev.used_addr != GuestAddr(0) {
                let _ = mmu.translate(dev.used_addr, vm_core::AccessType::Write);
            }
        }
    }
}
