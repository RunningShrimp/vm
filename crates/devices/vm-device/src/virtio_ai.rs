use vm_core::{MmioDevice, VmResult};

pub struct VirtioAi;

impl Default for VirtioAi {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtioAi {
    pub fn new() -> Self {
        Self
    }
}

pub struct VirtioAiMmio;

impl VirtioAiMmio {
    pub fn new(_device: VirtioAi) -> Self {
        Self
    }
}

impl MmioDevice for VirtioAiMmio {
    fn read(&self, _offset: u64, _size: u8) -> VmResult<u64> {
        Ok(0)
    }

    fn write(&mut self, _offset: u64, _val: u64, _size: u8) -> VmResult<()> {
        Ok(())
    }
}
