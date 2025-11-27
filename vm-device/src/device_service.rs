use crate::block::VirtioBlockMmio;
use vm_core::MMU;

pub struct DeviceService {
    pub block_mmio: Option<VirtioBlockMmio>,
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
            dev.handle_queue_notify(mmu, 0);
        }
    }
}
