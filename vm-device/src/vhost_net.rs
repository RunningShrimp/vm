use crate::virtio::{Queue, VirtioDevice};
use vm_core::MMU;

pub struct VhostNet {
    queues: Vec<Queue>,
}

impl Default for VhostNet {
    fn default() -> Self {
        Self::new()
    }
}

impl VhostNet {
    pub fn new() -> Self {
        Self {
            queues: vec![Queue::new(256); 2],
        }
    }
}

impl VirtioDevice for VhostNet {
    fn device_id(&self) -> u32 {
        1 // VirtIO Net device ID
    }

    fn num_queues(&self) -> usize {
        self.queues.len()
    }

    fn get_queue(&mut self, index: usize) -> &mut Queue {
        &mut self.queues[index]
    }

    fn process_queues(&mut self, _mmu: &mut dyn MMU) {
        for i in 0..self.num_queues() {
            let _queue = &mut self.queues[i];
            // ...
        }
    }
}
