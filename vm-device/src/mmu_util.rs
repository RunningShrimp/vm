use vm_core::{MMU, Fault};

pub trait MmuUtil: MMU {
    fn read_u16(&self, addr: u64) -> Result<u16, Fault> {
        self.read(addr, 2).map(|v| v as u16)
    }

    fn read_u32(&self, addr: u64) -> Result<u32, Fault> {
        self.read(addr, 4).map(|v| v as u32)
    }

    fn read_u64(&self, addr: u64) -> Result<u64, Fault> {
        self.read(addr, 8)
    }

    fn write_u16(&mut self, addr: u64, val: u16) -> Result<(), Fault> {
        self.write(addr, val as u64, 2)
    }

    fn write_u32(&mut self, addr: u64, val: u32) -> Result<(), Fault> {
        self.write(addr, val as u64, 4)
    }
}

impl<T: MMU + ?Sized> MmuUtil for T {}
