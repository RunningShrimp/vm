use vm_core::MmioDevice;

pub struct VirtioBlock {
    pub capacity: u64,
    pub data: Vec<u8>,
}

impl VirtioBlock {
    pub fn new(size: usize) -> Self {
        Self {
            capacity: size as u64,
            data: vec![0; size],
        }
    }
}

impl MmioDevice for VirtioBlock {
    fn read(&self, offset: u64, _size: u8) -> u64 {
        match offset {
            0x00 => 0x74726976, // Magic "virt"
            0x04 => 2,          // Version
            0x08 => 2,          // Device ID (Block)
            0x0C => 0x554D4551, // Vendor ID
            _ => 0,
        }
    }

    fn write(&mut self, offset: u64, val: u64, _size: u8) {
        // Handle queue notifications etc.
        println!("VirtioBlock write: offset={:#x} val={:#x}", offset, val);
    }
}

pub struct VirtioNet {
    pub mac: [u8; 6],
}

impl VirtioNet {
    pub fn new() -> Self {
        Self {
            mac: [0x52, 0x54, 0x00, 0x12, 0x34, 0x56],
        }
    }
}

impl MmioDevice for VirtioNet {
    fn read(&self, offset: u64, _size: u8) -> u64 {
        match offset {
            0x00 => 0x74726976, // Magic "virt"
            0x04 => 2,          // Version
            0x08 => 1,          // Device ID (Net)
            0x0C => 0x554D4551, // Vendor ID
            _ => 0,
        }
    }

    fn write(&mut self, offset: u64, val: u64, _size: u8) {
        println!("VirtioNet write: offset={:#x} val={:#x}", offset, val);
    }
}
