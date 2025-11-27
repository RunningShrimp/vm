//! PLIC - Platform-Level Interrupt Controller
//!
//! 实现 RISC-V PLIC，管理外部中断的优先级和路由

use vm_core::MmioDevice;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::time::Instant;

/// PLIC 寄存器偏移
pub mod offsets {
    /// 中断优先级寄存器基址
    pub const PRIORITY_BASE: u64 = 0x0000_0000;
    /// 中断待处理寄存器基址
    pub const PENDING_BASE: u64 = 0x0000_1000;
    /// 中断使能寄存器基址（每个 context）
    pub const ENABLE_BASE: u64 = 0x0000_2000;
    /// 优先级阈值和声明/完成寄存器基址
    pub const CONTEXT_BASE: u64 = 0x0020_0000;
}

/// 每个 context 的寄存器偏移
pub mod context_offsets {
    /// 优先级阈值
    pub const THRESHOLD: u64 = 0x0000;
    /// 声明寄存器
    pub const CLAIM: u64 = 0x0004;
    /// 完成寄存器（与 CLAIM 相同地址）
    pub const COMPLETE: u64 = 0x0004;
}

/// PLIC 设备
pub struct Plic {
    /// 中断源数量（不包括 0）
    num_sources: usize,
    /// context 数量（通常是 hart_count * 2，M 和 S 模式各一个）
    num_contexts: usize,
    /// 每个中断源的优先级 (1-7, 0 表示禁用)
    priorities: Vec<u32>,
    /// 待处理的中断位图
    pending: Vec<bool>,
    /// 每个 context 的中断使能位图
    enables: Vec<Vec<bool>>,
    /// 每个 context 的优先级阈值
    thresholds: Vec<u32>,
    /// 每个 context 当前声明的中断
    claimed: Vec<Option<usize>>,
    last_tick: Instant,
    tick_interval_ms: u64,
    virtio_queue_source_base: usize,
    source_map: HashMap<String, (usize, usize)>,
}

impl Plic {
    /// 创建新的 PLIC 设备
    pub fn new(num_sources: usize, num_contexts: usize) -> Self {
        Self {
            num_sources,
            num_contexts,
            priorities: vec![0; num_sources + 1], // +1 for source 0 (unused)
            pending: vec![false; num_sources + 1],
            enables: vec![vec![false; num_sources + 1]; num_contexts],
            thresholds: vec![0; num_contexts],
            claimed: vec![None; num_contexts],
            last_tick: Instant::now(),
            tick_interval_ms: 100,
            virtio_queue_source_base: 16,
            source_map: HashMap::new(),
        }
    }

    /// 设置中断待处理
    pub fn set_pending(&mut self, source: usize) {
        if source > 0 && source <= self.num_sources {
            self.pending[source] = true;
        }
    }

    /// 清除中断待处理
    pub fn clear_pending(&mut self, source: usize) {
        if source > 0 && source <= self.num_sources {
            self.pending[source] = false;
        }
    }

    /// 检查 context 是否有待处理的中断
    pub fn has_interrupt(&self, context: usize) -> bool {
        if context >= self.num_contexts {
            return false;
        }

        // 如果已经有声明的中断，则有中断待处理
        if self.claimed[context].is_some() {
            return true;
        }

        let threshold = self.thresholds[context];

        // 查找优先级最高的待处理中断
        for source in 1..=self.num_sources {
            if self.pending[source]
                && self.enables[context][source]
                && self.priorities[source] > threshold
            {
                return true;
            }
        }

        false
    }

    /// 声明中断（返回中断源 ID）
    fn claim(&mut self, context: usize) -> u32 {
        if context >= self.num_contexts {
            return 0;
        }

        // 如果已经有声明的中断，返回它
        if let Some(source) = self.claimed[context] {
            return source as u32;
        }

        let threshold = self.thresholds[context];
        let mut best_source = 0;
        let mut best_priority = 0;

        // 查找优先级最高的待处理中断
        for source in 1..=self.num_sources {
            if self.pending[source]
                && self.enables[context][source]
                && self.priorities[source] > threshold
                && self.priorities[source] > best_priority
            {
                best_source = source;
                best_priority = self.priorities[source];
            }
        }

        if best_source > 0 {
            self.claimed[context] = Some(best_source);
            self.pending[best_source] = false;
        }

        best_source as u32
    }

    /// 完成中断
    fn complete(&mut self, context: usize, source: u32) {
        if context >= self.num_contexts {
            return;
        }

        let source = source as usize;
        if source > 0 && source <= self.num_sources {
            if self.claimed[context] == Some(source) {
                self.claimed[context] = None;
            }
        }
    }

    /// 读取寄存器
    pub fn read(&self, offset: u64, _size: u8) -> u64 {
        match offset {
            // 优先级寄存器 (0x000000 - 0x000FFF)
            o if o >= offsets::PRIORITY_BASE && o < offsets::PENDING_BASE => {
                let source = (o / 4) as usize;
                if source <= self.num_sources {
                    self.priorities[source] as u64
                } else {
                    0
                }
            }
            // 待处理寄存器 (0x001000 - 0x001FFF)
            o if o >= offsets::PENDING_BASE && o < offsets::ENABLE_BASE => {
                let word_idx = ((o - offsets::PENDING_BASE) / 4) as usize;
                let mut val = 0u32;
                for i in 0..32 {
                    let source = word_idx * 32 + i;
                    if source <= self.num_sources && self.pending[source] {
                        val |= 1 << i;
                    }
                }
                val as u64
            }
            // 使能寄存器 (0x002000 - 0x1FFFFF)
            o if o >= offsets::ENABLE_BASE && o < offsets::CONTEXT_BASE => {
                let context = ((o - offsets::ENABLE_BASE) / 0x80) as usize;
                let word_idx = (((o - offsets::ENABLE_BASE) % 0x80) / 4) as usize;
                
                if context < self.num_contexts {
                    let mut val = 0u32;
                    for i in 0..32 {
                        let source = word_idx * 32 + i;
                        if source <= self.num_sources && self.enables[context][source] {
                            val |= 1 << i;
                        }
                    }
                    val as u64
                } else {
                    0
                }
            }
            // Context 寄存器 (0x200000+)
            o if o >= offsets::CONTEXT_BASE => {
                let context = ((o - offsets::CONTEXT_BASE) / 0x1000) as usize;
                let reg_offset = (o - offsets::CONTEXT_BASE) % 0x1000;
                
                if context < self.num_contexts {
                    match reg_offset {
                        context_offsets::THRESHOLD => self.thresholds[context] as u64,
                        context_offsets::CLAIM => {
                            // 注意：这里应该调用 claim，但由于是不可变引用，我们只返回 0
                            // 实际使用时需要通过 Arc<Mutex<>> 来处理
                            0
                        }
                        _ => 0,
                    }
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    /// 写入寄存器
    pub fn write(&mut self, offset: u64, val: u64, _size: u8) {
        match offset {
            // 优先级寄存器
            o if o >= offsets::PRIORITY_BASE && o < offsets::PENDING_BASE => {
                let source = (o / 4) as usize;
                if source <= self.num_sources {
                    self.priorities[source] = (val & 0x7) as u32; // 3-bit priority
                }
            }
            // 待处理寄存器（只读）
            o if o >= offsets::PENDING_BASE && o < offsets::ENABLE_BASE => {
                // 只读，忽略写入
            }
            // 使能寄存器
            o if o >= offsets::ENABLE_BASE && o < offsets::CONTEXT_BASE => {
                let context = ((o - offsets::ENABLE_BASE) / 0x80) as usize;
                let word_idx = (((o - offsets::ENABLE_BASE) % 0x80) / 4) as usize;
                
                if context < self.num_contexts {
                    let val = val as u32;
                    for i in 0..32 {
                        let source = word_idx * 32 + i;
                        if source <= self.num_sources {
                            self.enables[context][source] = (val & (1 << i)) != 0;
                        }
                    }
                }
            }
            // Context 寄存器
            o if o >= offsets::CONTEXT_BASE => {
                let context = ((o - offsets::CONTEXT_BASE) / 0x1000) as usize;
                let reg_offset = (o - offsets::CONTEXT_BASE) % 0x1000;
                
                if context < self.num_contexts {
                    match reg_offset {
                        context_offsets::THRESHOLD => {
                            self.thresholds[context] = (val & 0x7) as u32;
                        }
                        context_offsets::COMPLETE => {
                            self.complete(context, val as u32);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    /// 声明中断（可变版本，用于 MMIO 包装器）
    pub fn claim_mut(&mut self, context: usize) -> u32 {
        self.claim(context)
    }
}

/// PLIC MMIO 设备包装器
pub struct PlicMmio {
    plic: Arc<Mutex<Plic>>,
}

impl PlicMmio {
    pub fn new(plic: Arc<Mutex<Plic>>) -> Self {
        Self { plic }
    }

    /// 获取 PLIC 的共享引用
    pub fn plic(&self) -> Arc<Mutex<Plic>> {
        Arc::clone(&self.plic)
    }

    pub fn set_virtio_queue_source_base(&self, base: usize) {
        let mut p = self.plic.lock().unwrap();
        p.virtio_queue_source_base = base;
    }

    pub fn register_source_range(&self, name: &str, base: usize, len: usize) {
        let mut p = self.plic.lock().unwrap();
        p.source_map.insert(name.to_string(), (base, len));
    }
    pub fn unregister_source(&self, name: &str) {
        let mut p = self.plic.lock().unwrap();
        p.source_map.remove(name);
    }
}

impl MmioDevice for PlicMmio {
    fn read(&self, offset: u64, size: u8) -> u64 {
        // 特殊处理 CLAIM 寄存器
        if offset >= offsets::CONTEXT_BASE {
            let context = ((offset - offsets::CONTEXT_BASE) / 0x1000) as usize;
            let reg_offset = (offset - offsets::CONTEXT_BASE) % 0x1000;
            
            if reg_offset == context_offsets::CLAIM {
                let mut plic = self.plic.lock().unwrap();
                return plic.claim_mut(context) as u64;
            }
        }

        let plic = self.plic.lock().unwrap();
        plic.read(offset, size)
    }

    fn write(&mut self, offset: u64, val: u64, size: u8) {
        let mut plic = self.plic.lock().unwrap();
        plic.write(offset, val, size);
    }

    fn poll(&mut self, _mmu: &mut dyn vm_core::MMU) {
        let mut plic = self.plic.lock().unwrap();
        let elapsed = plic.last_tick.elapsed();
        if elapsed.as_millis() as u64 >= plic.tick_interval_ms {
            plic.set_pending(1);
            plic.last_tick = Instant::now();
        }
        if let Ok(status) = _mmu.read(0x1000_0000 + 0x030, 4) {
            if status != 0 { plic.set_pending(1); }
        }
        let cause = _mmu.read(0x1000_0000 + 0x048, 8).unwrap_or(0);
        let base = plic.virtio_queue_source_base;
        for i in 0..32 {
            if ((cause >> i) & 1) != 0 { plic.set_pending(base + i as usize); }
        }
    }
}
