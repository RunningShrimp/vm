pub mod gpu;
pub mod virtio;
pub mod block;
pub mod clint;
pub mod plic;
pub mod graphics;
pub mod network;

pub trait Device {
    fn reset(&mut self);
}

pub struct VirtioConsole {
    buffer: Vec<u8>,
}

impl VirtioConsole {
    pub fn new() -> Self { Self { buffer: Vec::new() } }
    pub fn send(&mut self, data: &[u8]) { self.buffer.extend_from_slice(data); println!("{}", String::from_utf8_lossy(&self.buffer)); }
}

impl Device for VirtioConsole {
    fn reset(&mut self) { self.buffer.clear(); }
}

use vm_core::MMU;
use vm_osal::{barrier_acquire, barrier_release};

pub trait Mmio {
    fn read(&mut self, mmu: &dyn MMU, offset: u64, size: u8) -> u64;
    fn write(&mut self, mmu: &mut dyn MMU, offset: u64, val: u64, size: u8);
}

pub struct VirtioConsoleMmio {
    pub dev: VirtioConsole,
    queue: Vec<Vec<u8>>, 
    selected: usize,
    qsize_arr: Vec<usize>,
    desc_addr_arr: Vec<u64>,
    avail_addr_arr: Vec<u64>,
    used_addr_arr: Vec<u64>,
    avail_event_addr_arr: Vec<u64>,
    used_event_addr_arr: Vec<u64>,
    irq_status: u32,
    irq_mask: u32,
    irq_cause_used: u64,
    irq_cause_evt: u64,
    dev_features: u32,
    driver_features: u32,
    woken: Vec<bool>,
}

impl VirtioConsoleMmio {
    const F_NEXT: u16 = 1;
    const F_WRITE: u16 = 2;
    const F_INDIRECT: u16 = 4;
    const AVAIL_NO_INTERRUPT: u16 = 1;
    const USED_NO_NOTIFY: u16 = 1;
    const EVT_NOTIFY_BASE: u8 = 0;
    const EVT_WAKE_BASE: u8 = 16;
    const EVT_IDX_BASE: u8 = 32;
    pub fn new() -> Self { Self { dev: VirtioConsole::new(), queue: Vec::new(), selected: 0, qsize_arr: vec![0; 8], desc_addr_arr: vec![0; 8], avail_addr_arr: vec![0; 8], used_addr_arr: vec![0; 8], avail_event_addr_arr: vec![0; 8], used_event_addr_arr: vec![0; 8], irq_status: 0, irq_mask: 0, irq_cause_used: 0, irq_cause_evt: 0, dev_features: 0x1, driver_features: 0, woken: vec![false; 8] } }
}

impl Mmio for VirtioConsoleMmio {
    fn read(&mut self, _mmu: &dyn MMU, offset: u64, _size: u8) -> u64 {
        match offset {
            0x30 => self.irq_status as u64,
            0x3C => self.irq_cause_used as u64,
            0x40 => self.dev_features as u64,
            0x48 => self.irq_cause_evt as u64,
            _ => 0
        }
    }
    fn write(&mut self, mmu: &mut dyn MMU, offset: u64, val: u64, _size: u8) {
        match offset {
            0x04 => { self.selected = val as usize % self.qsize_arr.len(); }
            0x00 => { self.qsize_arr[self.selected] = val as usize; self.queue.clear(); }
            0x08 => { self.desc_addr_arr[self.selected] = val as u64; }
            0x10 => { self.avail_addr_arr[self.selected] = val as u64; }
            0x18 => { self.used_addr_arr[self.selected] = val as u64; }
            0x24 => { self.avail_event_addr_arr[self.selected] = val as u64; }
            0x28 => { self.used_event_addr_arr[self.selected] = val as u64; }
            0x44 => { self.driver_features = val as u32; }
            0x20 => {
                let qsize = self.qsize_arr[self.selected];
                let desc_addr = self.desc_addr_arr[self.selected];
                let avail_addr = self.avail_addr_arr[self.selected];
                let used_addr = self.used_addr_arr[self.selected];
                if desc_addr != 0 && avail_addr != 0 {
                    self.irq_cause_evt |= 1u64 << (Self::EVT_NOTIFY_BASE as u64 + self.selected as u64);
                    barrier_acquire();
                    if self.driver_features & 0x1 != 0 {
                        let evt_addr = if self.avail_event_addr_arr[self.selected] != 0 { self.avail_event_addr_arr[self.selected] } else { self.default_avail_event_addr(used_addr, qsize) };
                        let evt = mmu.read(evt_addr, 2).unwrap_or(0) as u16;
                        let cur = mmu.read(avail_addr + 2, 2).unwrap_or(0) as u16;
                        if cur != evt { return; } else { self.irq_cause_evt |= 1u64 << (Self::EVT_IDX_BASE as u64 + self.selected as u64); }
                    }
                    let head = self.read_avail_head_idx(mmu, avail_addr, qsize);
                    let segs = self.gather_segments_idx(mmu, desc_addr, qsize, head);
                    let mut total = 0u32;
                    let mut buf = Vec::new();
                    for (addr, len, write) in segs {
                        total = total.wrapping_add(len);
                        if !write {
                            for i in 0..len { if let Ok(b) = mmu.read(addr + i as u64, 1) { buf.push(b as u8); } }
                        }
                    }
                    if !buf.is_empty() { self.dev.send(&buf); }
                    barrier_release();
                    self.write_used_idx(mmu, used_addr, head as u32, total);
                    let af = mmu.read(avail_addr + 0, 2).unwrap_or(0) as u16;
                    let uf = mmu.read(used_addr + 0, 2).unwrap_or(0) as u16;
                    if (af & Self::AVAIL_NO_INTERRUPT) == 0 && (uf & Self::USED_NO_NOTIFY) == 0 {
                        let used_event_addr = if self.used_event_addr_arr[self.selected] != 0 { self.used_event_addr_arr[self.selected] } else { self.default_used_event_addr(avail_addr, qsize) };
                        let evt = mmu.read(used_event_addr, 2).unwrap_or(0) as u16;
                        let cur = mmu.read(used_addr + 2, 2).unwrap_or(0) as u16;
                        let mask_bit = 1u64 << (self.selected as u64);
                        if cur == evt && (self.irq_mask & (mask_bit as u32)) == 0 { self.irq_status = 1; self.irq_cause_used |= mask_bit; }
                    }
                }
            }
            0x34 => { self.irq_status = 0; self.irq_cause_used = 0; self.irq_cause_evt = 0; }
            0x38 => { self.irq_mask = val as u32; }
            0x2C => { let q = val as usize % self.qsize_arr.len(); self.woken[q] = true; self.irq_cause_evt |= 1u64 << (Self::EVT_WAKE_BASE as u64 + q as u64); self.irq_status = 1; }
            _ => {}
        }
    }
}

impl VirtioConsoleMmio {
    fn read_avail_head_idx(&self, mmu: &dyn MMU, avail_addr: u64, qsize: usize) -> u16 {
        let idx = mmu.read(avail_addr + 2, 2).unwrap_or(0) as u16;
        let head_off = 4 + ((idx.wrapping_sub(1) % qsize as u16) as u64) * 2;
        mmu.read(avail_addr + head_off, 2).unwrap_or(0) as u16
    }
    fn read_desc_idx(&self, mmu: &dyn MMU, desc_addr: u64, i: u16) -> Option<(u64, u32, u16, u16)> {
        let base = desc_addr + (i as u64) * 16;
        let addr = mmu.read(base + 0, 8).ok()?;
        let len = mmu.read(base + 8, 4).ok()? as u32;
        let flags = mmu.read(base + 12, 2).ok()? as u16;
        let next = mmu.read(base + 14, 2).ok()? as u16;
        Some((addr, len, flags, next))
    }
    fn gather_segments_idx(&self, mmu: &dyn MMU, desc_addr: u64, qsize: usize, head: u16) -> Vec<(u64, u32, bool)> {
        let mut segs = Vec::new();
        let mut cur = head;
        let mut budget = qsize.max(8);
        while budget > 0 {
            budget -= 1;
            if let Some((addr, len, flags, next)) = self.read_desc_idx(mmu, desc_addr, cur) {
                if (flags & Self::F_INDIRECT) != 0 {
                    let mut off = addr;
                    let mut ibudget = 8u32;
                    loop {
                        if ibudget == 0 { break; }
                        ibudget -= 1;
                        let i_addr = mmu.read(off + 0, 8).unwrap_or(0);
                        let i_len = mmu.read(off + 8, 4).unwrap_or(0) as u32;
                        let i_flags = mmu.read(off + 12, 2).unwrap_or(0) as u16;
                        let _i_next = mmu.read(off + 14, 2).unwrap_or(0) as u16;
                        segs.push((i_addr, i_len, (i_flags & Self::F_WRITE) != 0));
                        off += 16;
                        if (i_flags & Self::F_NEXT) == 0 { break; }
                    }
                    break;
                } else {
                    segs.push((addr, len, (flags & Self::F_WRITE) != 0));
                    if (flags & Self::F_NEXT) != 0 { cur = next; continue; } else { break; }
                }
            } else { break; }
        }
        segs
    }
    fn write_used_idx(&self, mmu: &mut dyn MMU, used_addr: u64, id: u32, len: u32) {
        let idx = mmu.read(used_addr + 2, 2).unwrap_or(0) as u16;
        let off = 4 + (idx as u64) * 8;
        let _ = mmu.write(used_addr + off + 0, id as u64, 4);
        let _ = mmu.write(used_addr + off + 4, len as u64, 4);
        let _ = mmu.write(used_addr + 2, (idx.wrapping_add(1)) as u64, 2);
    }

    fn default_used_event_addr(&self, avail_addr: u64, qsize: usize) -> u64 {
        avail_addr + 4 + (qsize as u64) * 2
    }
    fn default_avail_event_addr(&self, used_addr: u64, qsize: usize) -> u64 {
        used_addr + 4 + (qsize as u64) * 8
    }
}

pub struct VirtioBlockMmio {
    pub capacity: u64,
    queue: Vec<Vec<u8>>, 
    selected: usize,
    qsize_arr: Vec<usize>,
    desc_addr_arr: Vec<u64>,
    avail_addr_arr: Vec<u64>,
    used_addr_arr: Vec<u64>,
    avail_event_addr_arr: Vec<u64>,
    used_event_addr_arr: Vec<u64>,
    irq_status: u32,
    irq_mask: u32,
    irq_cause_used: u64,
    irq_cause_evt: u64,
    dev_features: u32,
    driver_features: u32,
    woken: Vec<bool>,
    backing: Vec<u8>,
}
impl VirtioBlockMmio { 
    pub fn new() -> Self { 
        Self { capacity: 0, queue: Vec::new(), selected: 0, qsize_arr: vec![0; 8], desc_addr_arr: vec![0; 8], avail_addr_arr: vec![0; 8], used_addr_arr: vec![0; 8], avail_event_addr_arr: vec![0; 8], used_event_addr_arr: vec![0; 8], irq_status: 0, irq_mask: 0, irq_cause_used: 0, irq_cause_evt: 0, dev_features: 0x1, driver_features: 0, woken: vec![false; 8], backing: Vec::new() } 
    }
    pub fn with_backing(data: Vec<u8>) -> Self { let mut s = Self::new(); s.capacity = (data.len() as u64) / 512; s.backing = data; s }
}
impl Mmio for VirtioBlockMmio {
    fn read(&mut self, _mmu: &dyn MMU, offset: u64, _size: u8) -> u64 {
        match offset { 0x40 => self.capacity, 0x30 => self.irq_status as u64, 0x3C => self.irq_cause_used as u64, 0x48 => self.irq_cause_evt as u64, _ => 0 }
    }
    fn write(&mut self, mmu: &mut dyn MMU, offset: u64, val: u64, _size: u8) {
        match offset {
            0x04 => { self.selected = val as usize % self.qsize_arr.len(); }
            0x00 => { self.qsize_arr[self.selected] = val as usize; self.queue.clear(); }
            0x08 => { self.desc_addr_arr[self.selected] = val as u64; }
            0x10 => { self.avail_addr_arr[self.selected] = val as u64; }
            0x18 => { self.used_addr_arr[self.selected] = val as u64; }
            0x24 => { self.avail_event_addr_arr[self.selected] = val as u64; }
            0x28 => { self.used_event_addr_arr[self.selected] = val as u64; }
            0x44 => { self.driver_features = val as u32; }
            0x20 => {
                let qsize = self.qsize_arr[self.selected];
                let desc_addr = self.desc_addr_arr[self.selected];
                let avail_addr = self.avail_addr_arr[self.selected];
                let used_addr = self.used_addr_arr[self.selected];
                if desc_addr != 0 && avail_addr != 0 {
                    barrier_acquire();
                    let head = Self::read_avail_head_idx(mmu, avail_addr, qsize);
                    let segs = Self::gather_segments_idx(mmu, desc_addr, qsize, head);
                    let mut total = 0u32;
                    let mut req_type = 0u32; let mut sector = 0u64; let mut data_addr = 0u64; let mut data_len = 0u32; let mut status_addr = 0u64;
                    if !segs.is_empty() {
                        let hdr_addr = segs[0].0; // VBlkReq header
                        req_type = mmu.read(hdr_addr + 0, 4).unwrap_or(0) as u32;
                        sector = mmu.read(hdr_addr + 8, 8).unwrap_or(0);
                        if segs.len() > 1 { data_addr = segs[1].0; data_len = segs[1].1; }
                        if segs.len() > 2 { status_addr = segs[2].0; }
                    }
                    if req_type == 0 { // READ
                        let off = (sector * 512) as usize;
                        for i in 0..data_len {
                            let b = if off + (i as usize) < self.backing.len() { self.backing[off + (i as usize)] as u64 } else { 0 };
                            let _ = mmu.write(data_addr + i as u64, b, 1);
                            total = total.wrapping_add(1);
                        }
                        let _ = mmu.write(status_addr, 0, 1);
                    }
                    barrier_release();
                    Self::write_used_idx(mmu, used_addr, head as u32, total);
                    self.irq_status = 1; self.irq_cause_used |= 1u64 << (self.selected as u64);
                }
            }
            0x34 => { self.irq_status = 0; self.irq_cause_used = 0; self.irq_cause_evt = 0; }
            0x38 => { self.irq_mask = val as u32; }
            _ => {}
        }
    }
}

impl VirtioBlockMmio {
    const F_NEXT: u16 = 1;
    const F_WRITE: u16 = 2;
    const F_INDIRECT: u16 = 4;
    fn read_avail_head_idx(mmu: &dyn MMU, avail_addr: u64, qsize: usize) -> u16 {
        let idx = mmu.read(avail_addr + 2, 2).unwrap_or(0) as u16;
        let head_off = 4 + ((idx.wrapping_sub(1) % qsize as u16) as u64) * 2;
        mmu.read(avail_addr + head_off, 2).unwrap_or(0) as u16
    }
    fn read_desc_idx(mmu: &dyn MMU, desc_addr: u64, i: u16) -> Option<(u64, u32, u16, u16)> {
        let base = desc_addr + (i as u64) * 16;
        let addr = mmu.read(base + 0, 8).ok()?;
        let len = mmu.read(base + 8, 4).ok()? as u32;
        let flags = mmu.read(base + 12, 2).ok()? as u16;
        let next = mmu.read(base + 14, 2).ok()? as u16;
        Some((addr, len, flags, next))
    }
    fn gather_segments_idx(mmu: &dyn MMU, desc_addr: u64, qsize: usize, head: u16) -> Vec<(u64, u32, bool)> {
        let mut segs = Vec::new();
        let mut cur = head;
        let mut budget = qsize.max(8);
        while budget > 0 {
            budget -= 1;
            if let Some((addr, len, flags, next)) = Self::read_desc_idx(mmu, desc_addr, cur) {
                segs.push((addr, len, (flags & Self::F_WRITE) != 0));
                if (flags & Self::F_NEXT) != 0 { cur = next; continue; } else { break; }
            } else { break; }
        }
        segs
    }
    fn write_used_idx(mmu: &mut dyn MMU, used_addr: u64, id: u32, len: u32) {
        let idx = mmu.read(used_addr + 2, 2).unwrap_or(0) as u16;
        let off = 4 + (idx as u64) * 8;
        let _ = mmu.write(used_addr + off + 0, id as u64, 4);
        let _ = mmu.write(used_addr + off + 4, len as u64, 4);
        let _ = mmu.write(used_addr + 2, (idx.wrapping_add(1)) as u64, 2);
    }
}

pub struct VirtioNetMmio {
    pub mtu: u32,
}
impl VirtioNetMmio { pub fn new() -> Self { Self { mtu: 1500 } } }
impl Mmio for VirtioNetMmio {
    fn read(&mut self, _mmu: &dyn MMU, _offset: u64, _size: u8) -> u64 { 0 }
    fn write(&mut self, _mmu: &mut dyn MMU, _offset: u64, _val: u64, _size: u8) {}
}

pub struct VirtioInputMmio;
impl VirtioInputMmio { pub fn new() -> Self { Self } }
impl Mmio for VirtioInputMmio {
    fn read(&mut self, _mmu: &dyn MMU, _offset: u64, _size: u8) -> u64 { 0 }
    fn write(&mut self, _mmu: &mut dyn MMU, _offset: u64, _val: u64, _size: u8) {}
}

pub struct VirtioGpuMmio;
impl VirtioGpuMmio { pub fn new() -> Self { Self } }
impl Mmio for VirtioGpuMmio {
    fn read(&mut self, _mmu: &dyn MMU, _offset: u64, _size: u8) -> u64 { 0 }
    fn write(&mut self, _mmu: &mut dyn MMU, _offset: u64, _val: u64, _size: u8) {}
}

pub trait Renderer {
    fn init(&mut self) -> bool;
    fn present(&mut self) {}
}

pub struct NullRenderer;
impl Renderer for NullRenderer { fn init(&mut self) -> bool { true } }

pub trait NetBackend {
    fn init(&mut self) -> bool;
    fn send(&mut self, _buf: &[u8]) {}
    fn recv(&mut self, _buf: &mut [u8]) -> usize { 0 }
}

pub struct UserModeNet;
impl NetBackend for UserModeNet { fn init(&mut self) -> bool { true } }

pub struct TapTunNet;
impl NetBackend for TapTunNet { fn init(&mut self) -> bool { false } }
