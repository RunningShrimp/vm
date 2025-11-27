use crate::mmu_util::MmuUtil;

pub trait VirtioDevice {
    fn device_id(&self) -> u32;
    fn num_queues(&self) -> usize;
    fn get_queue(&mut self, index: usize) -> &mut Queue;
    fn process_queues(&mut self, mmu: &mut dyn MmuUtil);
}

#[derive(Clone)]
pub struct Queue {
    pub desc_addr: u64,
    pub avail_addr: u64,
    pub used_addr: u64,
    pub size: u16,
    last_avail_idx: u16,
}

impl Queue {
    pub fn new(size: u16) -> Self {
        Self {
            desc_addr: 0,
            avail_addr: 0,
            used_addr: 0,
            size,
            last_avail_idx: 0,
        }
    }

    pub fn pop(&mut self, mmu: &dyn MmuUtil) -> Option<DescChain> {
        let avail_idx = mmu.read_u16(self.avail_addr + 2).unwrap();
        if self.last_avail_idx == avail_idx {
            return None;
        }

        let desc_index = mmu.read_u16(self.avail_addr + 4 + (self.last_avail_idx % self.size) as u64 * 2).unwrap();
        self.last_avail_idx = self.last_avail_idx.wrapping_add(1);

        Some(DescChain::new(mmu, self.desc_addr, desc_index))
    }

    pub fn add_used(&mut self, mmu: &mut dyn MmuUtil, head_index: u16, len: u32) {
        let used_idx = mmu.read_u16(self.used_addr + 2).unwrap();
        let used_elem_addr = self.used_addr + 4 + (used_idx % self.size) as u64 * 8;
        mmu.write_u32(used_elem_addr, head_index as u32).unwrap();
        mmu.write_u32(used_elem_addr + 4, len).unwrap();
        mmu.write_u16(self.used_addr + 2, used_idx.wrapping_add(1)).unwrap();
    }

    pub fn signal_used(&self, _mmu: &mut dyn MmuUtil) {
        // For now, we don't support interrupts
    }
}

pub struct DescChain {
    pub head_index: u16,
    pub descs: Vec<Desc>,
}

impl DescChain {
    pub fn new(mmu: &dyn MmuUtil, desc_table: u64, head_index: u16) -> Self {
        let mut descs = Vec::new();
        let mut next_index = Some(head_index);

        while let Some(index) = next_index {
            let desc = Desc::from_memory(mmu, desc_table, index);
            next_index = desc.next;
            descs.push(desc);
        }

        Self { head_index, descs }
    }
}

#[derive(Clone)]
pub struct Desc {
    pub addr: u64,
    pub len: u32,
    pub flags: u16,
    pub next: Option<u16>,
}

impl Desc {
    pub fn from_memory(mmu: &dyn MmuUtil, desc_table: u64, index: u16) -> Self {
        let base = desc_table + index as u64 * 16;
        let addr = mmu.read_u64(base).unwrap();
        let len = mmu.read_u32(base + 8).unwrap();
        let flags = mmu.read_u16(base + 12).unwrap();
        let next = if (flags & 1) != 0 {
            Some(mmu.read_u16(base + 14).unwrap())
        } else {
            None
        };

        Self { addr, len, flags, next }
    }
}
