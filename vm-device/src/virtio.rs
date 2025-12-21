use crate::mmu_util::MmuUtil;
use vm_core::{MMU, VmError};

pub trait VirtioDevice {
    fn device_id(&self) -> u32;
    fn num_queues(&self) -> usize;
    fn get_queue(&mut self, index: usize) -> &mut Queue;
    fn process_queues(&mut self, mmu: &mut dyn MMU);
}

pub use crate::block::VirtioBlockMmio as VirtioBlock;

#[derive(Clone)]
pub struct Queue {
    pub desc_addr: u64,
    pub avail_addr: u64,
    pub used_addr: u64,
    pub size: u16,
    last_avail_idx: u16,
    // 本地镜像用于批量操作优化
    avail_shadow: Vec<u16>,
    used_shadow: Vec<u16>,
}

impl Queue {
    pub fn new(size: u16) -> Self {
        Self {
            desc_addr: 0,
            avail_addr: 0,
            used_addr: 0,
            size,
            last_avail_idx: 0,
            avail_shadow: Vec::with_capacity(size as usize / 4), // 预分配合理的容量
            used_shadow: Vec::with_capacity(size as usize / 4),
        }
    }

    /// 添加本地avail索引到影子缓冲区
    pub fn add_avail_shadow(&mut self, idx: u16) {
        self.avail_shadow.push(idx);
        // 如果影子缓冲区满了，自动刷新
        if self.avail_shadow.len() >= (self.size as usize / 4) {
            let _ = self.flush_avail();
        }
    }

    /// 批量刷新avail索引到内存
    pub fn flush_avail(&mut self) -> Result<(), VmError> {
        if self.avail_shadow.is_empty() {
            return Ok(());
        }

        // 在这里实现批量写入avail ring的逻辑
        // 为了演示，我们记录操作但不实际写入
        println!(
            "Flushing {} avail indices: {:?}",
            self.avail_shadow.len(),
            self.avail_shadow
        );
        self.avail_shadow.clear();
        Ok(())
    }

    /// 添加本地used索引到影子缓冲区
    pub fn add_used_shadow(&mut self, idx: u16) {
        self.used_shadow.push(idx);
        // 如果影子缓冲区满了，自动刷新
        if self.used_shadow.len() >= (self.size as usize / 4) {
            let _ = self.flush_used();
        }
    }

    /// 批量刷新used索引到内存
    pub fn flush_used(&mut self) -> Result<(), VmError> {
        if self.used_shadow.is_empty() {
            return Ok(());
        }

        // 在这里实现批量写入used ring的逻辑
        // 为了演示，我们记录操作但不实际写入
        println!(
            "Flushing {} used indices: {:?}",
            self.used_shadow.len(),
            self.used_shadow
        );
        self.used_shadow.clear();
        Ok(())
    }

    /// 强制刷新所有影子缓冲区
    pub fn flush_all(&mut self) -> Result<(), VmError> {
        self.flush_avail()?;
        self.flush_used()?;
        Ok(())
    }

    /// 获取影子缓冲区统计信息
    pub fn shadow_stats(&self) -> (usize, usize) {
        (self.avail_shadow.len(), self.used_shadow.len())
    }

    pub fn pop(&mut self, mmu: &dyn MMU) -> Option<DescChain> {
        let avail_idx = mmu.read_u16(self.avail_addr + 2).ok()?;
        if self.last_avail_idx == avail_idx {
            return None;
        }

        let desc_index = mmu
            .read_u16(self.avail_addr + 4 + (self.last_avail_idx % self.size) as u64 * 2)
            .ok()?;
        self.last_avail_idx = self.last_avail_idx.wrapping_add(1);

        Some(DescChain::new(mmu, self.desc_addr, desc_index))
    }

    /// 批量弹出多个描述符链（优化性能）
    ///
    /// 一次性读取多个可用索引，减少MMU调用次数
    pub fn pop_batch(&mut self, mmu: &dyn MMU, max_count: usize) -> Vec<DescChain> {
        let avail_idx = match mmu.read_u16(self.avail_addr + 2) {
            Ok(idx) => idx,
            Err(_) => return Vec::new(),
        };

        let available_count = (avail_idx.wrapping_sub(self.last_avail_idx) as usize)
            .min(max_count)
            .min(self.size as usize);

        if available_count == 0 {
            return Vec::new();
        }

        // 批量读取可用索引
        let mut chains = Vec::with_capacity(available_count);
        let ring_addr = self.avail_addr + 4;

        for i in 0..available_count {
            let ring_offset = ((self.last_avail_idx.wrapping_add(i as u16) % self.size) as u64) * 2;
            if let Ok(desc_index) = mmu.read_u16(ring_addr + ring_offset)
                && let Ok(chain) = DescChain::try_new(mmu, self.desc_addr, desc_index)
            {
                chains.push(chain);
            }
        }

        self.last_avail_idx = self.last_avail_idx.wrapping_add(chains.len() as u16);
        chains
    }

    pub fn add_used(&mut self, mmu: &mut dyn MMU, head_index: u16, len: u32) {
        if let Ok(used_idx) = mmu.read_u16(self.used_addr + 2) {
            let used_elem_addr = self.used_addr + 4 + (used_idx % self.size) as u64 * 8;
            let _ = mmu.write_u32(used_elem_addr, head_index as u32);
            let _ = mmu.write_u32(used_elem_addr + 4, len);
            let _ = mmu.write_u16(self.used_addr + 2, used_idx.wrapping_add(1));
        }
    }

    /// 批量添加已使用的描述符（优化性能）
    ///
    /// 一次性写入多个used元素，减少MMU调用次数
    pub fn add_used_batch(&mut self, mmu: &mut dyn MMU, entries: &[(u16, u32)]) {
        if entries.is_empty() {
            return;
        }

        if let Ok(mut used_idx) = mmu.read_u16(self.used_addr + 2) {
            let base_addr = self.used_addr + 4;

            for (head_index, len) in entries {
                let used_elem_addr = base_addr + (used_idx % self.size) as u64 * 8;
                let _ = mmu.write_u32(used_elem_addr, *head_index as u32);
                let _ = mmu.write_u32(used_elem_addr + 4, *len);
                used_idx = used_idx.wrapping_add(1);
            }

            // 最后更新used索引
            let _ = mmu.write_u16(self.used_addr + 2, used_idx);
        }
    }

    pub fn signal_used(&self, _mmu: &mut dyn MMU) {
        // For now, we don't support interrupts
    }
}

pub struct DescChain {
    pub head_index: u16,
    pub descs: Vec<Desc>,
}

impl DescChain {
    pub fn new(mmu: &dyn MMU, desc_table: u64, head_index: u16) -> Self {
        Self::try_new(mmu, desc_table, head_index).unwrap_or_else(|_| {
            // 如果失败，返回一个空的描述符链
            Self {
                head_index,
                descs: Vec::new(),
            }
        })
    }

    /// 尝试创建描述符链（返回Result以便错误处理）
    pub fn try_new(mmu: &dyn MMU, desc_table: u64, head_index: u16) -> Result<Self, VmError> {
        let mut descs = Vec::new();
        let mut next_index = Some(head_index);
        let mut visited = std::collections::HashSet::new();

        while let Some(index) = next_index {
            // 防止循环引用
            if !visited.insert(index) {
                return Err(VmError::Core(vm_core::CoreError::Internal {
                    message: "Circular descriptor chain detected".to_string(),
                    module: "VirtIO".to_string(),
                }));
            }

            let desc = Desc::from_memory(mmu, desc_table, index);
            next_index = desc.next;
            descs.push(desc);

            // 限制链长度，防止无限循环
            if descs.len() > 256 {
                return Err(VmError::Core(vm_core::CoreError::Internal {
                    message: "Descriptor chain too long".to_string(),
                    module: "VirtIO".to_string(),
                }));
            }
        }

        Ok(Self { head_index, descs })
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
    pub fn from_memory(mmu: &dyn MMU, desc_table: u64, index: u16) -> Self {
        let base = desc_table + index as u64 * 16;
        let addr = mmu.read_u64(base).unwrap_or(0);
        let len = mmu.read_u32(base + 8).unwrap_or(0);
        let flags = mmu.read_u16(base + 12).unwrap_or(0);
        let next = if (flags & 1) != 0 {
            mmu.read_u16(base + 14).ok()
        } else {
            None
        };

        Self {
            addr,
            len,
            flags,
            next,
        }
    }
}
