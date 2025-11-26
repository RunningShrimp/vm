use vm_core::{MMU, GuestAddr, AccessType, Fault};
use std::cell::RefCell;

pub struct SoftMMU {
    mem: Vec<u8>,
    itlb: Vec<(u64, u64)>,
    dtlb: Vec<(u64, u64)>,
    fault_cb: Option<Box<dyn PageFaultHandler>>,
    mmio: Vec<(u64, u64, RefCell<Box<dyn crate_mmio::Mmio>>)>,
}

pub trait PageFaultHandler {
    fn on_fault(&mut self, va: u64, access: AccessType) -> Option<u64>;
}

impl SoftMMU {
    pub fn new() -> Self { Self { mem: vec![0; 1 << 16], itlb: Vec::new(), dtlb: Vec::new(), fault_cb: None, mmio: Vec::new() } }
    pub fn new_with_size(size: usize) -> Self { Self { mem: vec![0; size], itlb: Vec::new(), dtlb: Vec::new(), fault_cb: None, mmio: Vec::new() } }
    pub fn set_fault_handler(&mut self, h: Box<dyn PageFaultHandler>) { self.fault_cb = Some(h); }
    pub fn map_mmio(&mut self, base: u64, size: u64, dev: Box<dyn crate_mmio::Mmio>) { self.mmio.push((base, size, RefCell::new(dev))); }
}

impl MMU for SoftMMU {
    fn translate(&mut self, va: GuestAddr, access: AccessType) -> Result<GuestAddr, Fault> {
        let tlb = match access { AccessType::Exec => &mut self.itlb, _ => &mut self.dtlb };
        if let Some(&(_, pa)) = tlb.iter().find(|e| e.0 == va) { return Ok(pa); }
        let pa = va; // Identity mapping for now
        tlb.push((va, pa));
        if tlb.len() > 64 { tlb.remove(0); }
        Ok(pa)
    }
    fn fetch_insn(&self, pc: GuestAddr) -> Result<u64, Fault> {
        self.read(pc, 4)
    }
    fn read(&self, pa: GuestAddr, size: u8) -> Result<u64, Fault> {
        let a = pa as usize;
        for i in 0..self.mmio.len() {
            let (base, size_bytes, _) = &self.mmio[i];
            if pa >= *base && pa < *base + *size_bytes {
                let mut dev = self.mmio[i].2.borrow_mut();
                let mmu: &dyn vm_core::MMU = self;
                return Ok(dev.read(mmu, pa - *base, size));
            }
        }
        match size {
            1 => self.mem.get(a).map(|&b| b as u64).ok_or(Fault::AccessViolation),
            2 => {
                if a >= self.mem.len().saturating_sub(1) { return Err(Fault::AccessViolation); }
                Ok(u16::from_le_bytes([self.mem[a], self.mem[a + 1]]) as u64)
            }
            4 => {
                if a >= self.mem.len().saturating_sub(3) { return Err(Fault::AccessViolation); }
                Ok(u32::from_le_bytes([self.mem[a], self.mem[a + 1], self.mem[a + 2], self.mem[a + 3]]) as u64)
            }
            8 => {
                if a >= self.mem.len().saturating_sub(7) { return Err(Fault::AccessViolation); }
                Ok(u64::from_le_bytes([
                    self.mem[a], self.mem[a + 1], self.mem[a + 2], self.mem[a + 3],
                    self.mem[a + 4], self.mem[a + 5], self.mem[a + 6], self.mem[a + 7],
                ]))
            }
            _ => Err(Fault::AccessViolation),
        }
    }
    fn write(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<(), Fault> {
        let a = pa as usize;
        for i in 0..self.mmio.len() {
            let base = self.mmio[i].0;
            let size_bytes = self.mmio[i].1;
            if pa >= base && pa < base + size_bytes {
                let dev_ptr = self.mmio[i].2.as_ptr();
                let mmu: &mut dyn vm_core::MMU = self;
                unsafe { (*dev_ptr).write(mmu, pa - base, val, size); }
                return Ok(());
            }
        }
        match size {
            1 => {
                if a >= self.mem.len() { return Err(Fault::AccessViolation); }
                self.mem[a] = val as u8;
                Ok(())
            }
            2 => {
                if a >= self.mem.len().saturating_sub(1) { return Err(Fault::AccessViolation); }
                let b = (val as u16).to_le_bytes();
                self.mem[a] = b[0]; self.mem[a + 1] = b[1];
                Ok(())
            }
            4 => {
                if a >= self.mem.len().saturating_sub(3) { return Err(Fault::AccessViolation); }
                let b = (val as u32).to_le_bytes();
                self.mem[a] = b[0]; self.mem[a + 1] = b[1]; self.mem[a + 2] = b[2]; self.mem[a + 3] = b[3];
                Ok(())
            }
            8 => {
                if a >= self.mem.len().saturating_sub(7) { return Err(Fault::AccessViolation); }
                let b = val.to_le_bytes();
                self.mem[a] = b[0]; self.mem[a + 1] = b[1]; self.mem[a + 2] = b[2]; self.mem[a + 3] = b[3];
                self.mem[a + 4] = b[4]; self.mem[a + 5] = b[5]; self.mem[a + 6] = b[6]; self.mem[a + 7] = b[7];
                Ok(())
            }
            _ => Err(Fault::AccessViolation),
        }
    }
}

pub mod crate_mmio {
    pub use vm_device::Mmio;
}

pub struct PagedMMU {
    mem: Vec<u8>,
}

impl PagedMMU {
    pub fn new(size: usize) -> Self { Self { mem: vec![0; size] } }
}

impl MMU for PagedMMU {
    fn translate(&mut self, va: GuestAddr, _access: AccessType) -> Result<GuestAddr, Fault> { Ok(va) }
    fn fetch_insn(&self, pc: GuestAddr) -> Result<u64, Fault> { self.read(pc, 4) }
    fn read(&self, pa: GuestAddr, size: u8) -> Result<u64, Fault> {
        let a = pa as usize;
        match size {
            1 => self.mem.get(a).map(|&b| b as u64).ok_or(Fault::AccessViolation),
            2 => if a + 1 < self.mem.len() { Ok(u16::from_le_bytes([self.mem[a], self.mem[a+1]]) as u64) } else { Err(Fault::AccessViolation) },
            4 => if a + 3 < self.mem.len() { Ok(u32::from_le_bytes([self.mem[a], self.mem[a+1], self.mem[a+2], self.mem[a+3]]) as u64) } else { Err(Fault::AccessViolation) },
            8 => if a + 7 < self.mem.len() { Ok(u64::from_le_bytes([self.mem[a], self.mem[a+1], self.mem[a+2], self.mem[a+3], self.mem[a+4], self.mem[a+5], self.mem[a+6], self.mem[a+7]])) } else { Err(Fault::AccessViolation) },
            _ => Err(Fault::AccessViolation)
        }
    }
    fn write(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<(), Fault> {
        let a = pa as usize;
        match size {
            1 => if a < self.mem.len() { self.mem[a] = val as u8; Ok(()) } else { Err(Fault::AccessViolation) },
            2 => if a + 1 < self.mem.len() { let b = (val as u16).to_le_bytes(); self.mem[a]=b[0]; self.mem[a+1]=b[1]; Ok(()) } else { Err(Fault::AccessViolation) },
            4 => if a + 3 < self.mem.len() { let b = (val as u32).to_le_bytes(); self.mem[a]=b[0]; self.mem[a+1]=b[1]; self.mem[a+2]=b[2]; self.mem[a+3]=b[3]; Ok(()) } else { Err(Fault::AccessViolation) },
            8 => if a + 7 < self.mem.len() { let b = val.to_le_bytes(); self.mem[a]=b[0]; self.mem[a+1]=b[1]; self.mem[a+2]=b[2]; self.mem[a+3]=b[3]; self.mem[a+4]=b[4]; self.mem[a+5]=b[5]; self.mem[a+6]=b[6]; self.mem[a+7]=b[7]; Ok(()) } else { Err(Fault::AccessViolation) },
            _ => Err(Fault::AccessViolation)
        }
    }
}
