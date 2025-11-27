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

    fn read_slice(&self, addr: u64, buf: &mut [u8]) -> Result<(), Fault> {
        let mut offset = 0u64;
        // 先对齐到 8 字节
        while (addr + offset) % 8 != 0 && (offset as usize) < buf.len() {
            buf[offset as usize] = self.read(addr + offset, 1)? as u8;
            offset += 1;
        }
        // 8 字节块
        while (offset as usize) + 8 <= buf.len() {
            let v = self.read(addr + offset, 8)?;
            buf[offset as usize + 0] = (v & 0xFF) as u8;
            buf[offset as usize + 1] = ((v >> 8) & 0xFF) as u8;
            buf[offset as usize + 2] = ((v >> 16) & 0xFF) as u8;
            buf[offset as usize + 3] = ((v >> 24) & 0xFF) as u8;
            buf[offset as usize + 4] = ((v >> 32) & 0xFF) as u8;
            buf[offset as usize + 5] = ((v >> 40) & 0xFF) as u8;
            buf[offset as usize + 6] = ((v >> 48) & 0xFF) as u8;
            buf[offset as usize + 7] = ((v >> 56) & 0xFF) as u8;
            offset += 8;
        }
        // 余数部分
        while (offset as usize) < buf.len() {
            buf[offset as usize] = self.read(addr + offset, 1)? as u8;
            offset += 1;
        }
        Ok(())
    }

    fn write_slice(&mut self, addr: u64, data: &[u8]) -> Result<(), Fault> {
        let mut offset = 0u64;
        // 先对齐到 8 字节
        while (addr + offset) % 8 != 0 && (offset as usize) < data.len() {
            self.write(addr + offset, data[offset as usize] as u64, 1)?;
            offset += 1;
        }
        // 8 字节块
        while (offset as usize) + 8 <= data.len() {
            let v = (data[offset as usize + 0] as u64)
                | ((data[offset as usize + 1] as u64) << 8)
                | ((data[offset as usize + 2] as u64) << 16)
                | ((data[offset as usize + 3] as u64) << 24)
                | ((data[offset as usize + 4] as u64) << 32)
                | ((data[offset as usize + 5] as u64) << 40)
                | ((data[offset as usize + 6] as u64) << 48)
                | ((data[offset as usize + 7] as u64) << 56);
            self.write(addr + offset, v, 8)?;
            offset += 8;
        }
        // 余数部分
        while (offset as usize) < data.len() {
            self.write(addr + offset, data[offset as usize] as u64, 1)?;
            offset += 1;
        }
        Ok(())
    }
}

impl<T: MMU + ?Sized> MmuUtil for T {}
