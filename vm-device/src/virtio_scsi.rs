use crate::virtio::{Queue, VirtioDevice};
use vm_core::MMU;

pub struct VirtioScsi {
    queues: Vec<Queue>,
}

impl VirtioScsi {
    pub fn new() -> Self {
        Self {
            queues: vec![Queue::new(256); 2],
        }
    }
}

impl VirtioDevice for VirtioScsi {
    fn device_id(&self) -> u32 {
        8 // VirtIO SCSI device ID
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
