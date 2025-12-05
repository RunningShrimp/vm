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
        if let Some(_dev) = &mut self.block_mmio {
            // 业务逻辑已迁移至服务层，MMIO容器不处理队列通知
        }
    }
}
