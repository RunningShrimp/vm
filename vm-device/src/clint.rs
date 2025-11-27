//! CLINT - Core Local Interruptor
//!
//! 实现 RISC-V CLINT，提供时钟中断和软件中断功能

use vm_core::MmioDevice;
use std::sync::Arc;
use parking_lot::Mutex;
use std::time::Instant;

/// CLINT 寄存器偏移
pub mod offsets {
    /// 软件中断寄存器基址 (msip)
    pub const MSIP_BASE: u64 = 0x0000;
    /// 时钟比较寄存器基址 (mtimecmp)
    pub const MTIMECMP_BASE: u64 = 0x4000;
    /// 时钟寄存器 (mtime)
    pub const MTIME: u64 = 0xbff8;
}

/// CLINT 设备
pub struct Clint {
    /// 软件中断位（每个 hart 一个）
    msip: Vec<bool>,
    /// 时钟比较值（每个 hart 一个）
    mtimecmp: Vec<u64>,
    /// 当前时钟值
    mtime: u64,
    /// 时钟起始时间
    start_time: Instant,
    /// 时钟频率 (Hz)
    freq: u64,
    /// hart 数量
    num_harts: usize,
}

impl Clint {
    /// 创建新的 CLINT 设备
    pub fn new(num_harts: usize, freq: u64) -> Self {
        Self {
            msip: vec![false; num_harts],
            mtimecmp: vec![u64::MAX; num_harts],
            mtime: 0,
            start_time: Instant::now(),
            freq,
            num_harts,
        }
    }

    /// 更新时钟值
    pub fn update_time(&mut self) {
        let elapsed = self.start_time.elapsed();
        let ticks = (elapsed.as_nanos() as u64 * self.freq) / 1_000_000_000;
        self.mtime = ticks;
    }

    /// 检查是否有待处理的时钟中断
    pub fn has_timer_interrupt(&self, hart_id: usize) -> bool {
        if hart_id >= self.num_harts {
            return false;
        }
        self.mtime >= self.mtimecmp[hart_id]
    }

    /// 检查是否有待处理的软件中断
    pub fn has_software_interrupt(&self, hart_id: usize) -> bool {
        if hart_id >= self.num_harts {
            return false;
        }
        self.msip[hart_id]
    }

    /// 清除软件中断
    pub fn clear_software_interrupt(&mut self, hart_id: usize) {
        if hart_id < self.num_harts {
            self.msip[hart_id] = false;
        }
    }

    /// 设置软件中断
    pub fn set_software_interrupt(&mut self, hart_id: usize) {
        if hart_id < self.num_harts {
            self.msip[hart_id] = true;
        }
    }

    /// 读取寄存器
    pub fn read(&mut self, offset: u64, _size: u8) -> u64 {
        self.update_time();

        match offset {
            // MSIP 寄存器 (0x0000 - 0x3FFF)
            o if o >= offsets::MSIP_BASE && o < offsets::MTIMECMP_BASE => {
                let hart_id = ((o - offsets::MSIP_BASE) / 4) as usize;
                if hart_id < self.num_harts {
                    self.msip[hart_id] as u64
                } else {
                    0
                }
            }
            // MTIMECMP 寄存器 (0x4000 - 0xBFF7)
            o if o >= offsets::MTIMECMP_BASE && o < offsets::MTIME => {
                let hart_id = ((o - offsets::MTIMECMP_BASE) / 8) as usize;
                if hart_id < self.num_harts {
                    self.mtimecmp[hart_id]
                } else {
                    0
                }
            }
            // MTIME 寄存器 (0xBFF8)
            offsets::MTIME => self.mtime,
            _ => 0,
        }
    }

    /// 写入寄存器
    pub fn write(&mut self, offset: u64, val: u64, _size: u8) {
        match offset {
            // MSIP 寄存器
            o if o >= offsets::MSIP_BASE && o < offsets::MTIMECMP_BASE => {
                let hart_id = ((o - offsets::MSIP_BASE) / 4) as usize;
                if hart_id < self.num_harts {
                    self.msip[hart_id] = (val & 0x1) != 0;
                }
            }
            // MTIMECMP 寄存器
            o if o >= offsets::MTIMECMP_BASE && o < offsets::MTIME => {
                let hart_id = ((o - offsets::MTIMECMP_BASE) / 8) as usize;
                if hart_id < self.num_harts {
                    self.mtimecmp[hart_id] = val;
                }
            }
            // MTIME 寄存器（通常只读，但某些实现允许写入）
            offsets::MTIME => {
                self.mtime = val;
                self.start_time = Instant::now();
            }
            _ => {}
        }
    }
}

/// CLINT MMIO 设备包装器
pub struct ClintMmio {
    clint: Arc<Mutex<Clint>>,
}

impl ClintMmio {
    pub fn new(clint: Arc<Mutex<Clint>>) -> Self {
        Self { clint }
    }
}

impl MmioDevice for ClintMmio {
    fn read(&self, offset: u64, size: u8) -> u64 {
        let mut clint = self.clint.lock();
        clint.read(offset, size)
    }

    fn write(&mut self, offset: u64, val: u64, size: u8) {
        let mut clint = self.clint.lock();
        clint.write(offset, val, size);
    }

    fn poll(&mut self, _mmu: &mut dyn vm_core::MMU) {
        let mut clint = self.clint.lock();
        clint.update_time();
    }
}
