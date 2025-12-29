//! VirtIO Memory 设备实现
//!
//! 提供内存设备功能，支持内存热插拔和内存区域管理

use crate::virtio::{Queue, VirtioDevice};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use vm_core::{MMU, MemoryError, VmError, VmResult};

/// 内存区域类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryRegionType {
    /// 普通内存
    Normal,
    /// 设备内存
    Device,
    /// 持久化内存
    Persistent,
}

/// 内存区域
#[derive(Debug, Clone)]
pub struct MemoryRegion {
    /// 区域ID
    pub id: u64,
    /// 起始地址
    pub start_addr: u64,
    /// 大小
    pub size: u64,
    /// 区域类型
    pub region_type: MemoryRegionType,
    /// 是否可插拔
    pub pluggable: bool,
    /// 是否已插入
    pub plugged: bool,
}

/// VirtIO Memory 设备
pub struct VirtioMemory {
    /// VirtIO队列
    queues: Vec<Queue>,
    /// 内存区域列表
    regions: Arc<Mutex<HashMap<u64, MemoryRegion>>>,
    /// 下一个区域ID
    next_region_id: Arc<Mutex<u64>>,
    /// 设备状态
    device_status: u32,
    /// 总内存大小
    total_memory: u64,
    /// 可用内存大小
    available_memory: u64,
}

impl VirtioMemory {
    /// 创建新的VirtIO Memory设备
    pub fn new(total_memory: u64) -> Self {
        Self {
            queues: vec![Queue::new(256); 1],
            regions: Arc::new(Mutex::new(HashMap::new())),
            next_region_id: Arc::new(Mutex::new(1)),
            device_status: 0,
            total_memory,
            available_memory: total_memory,
        }
    }

    /// Helper to acquire next_region_id lock with error handling
    fn lock_next_region_id(&self) -> VmResult<std::sync::MutexGuard<'_, u64>> {
        self.next_region_id.lock().map_err(|_| {
            VmError::Memory(MemoryError::PageTableError {
                message: "VirtioMemory next_region_id lock is poisoned".to_string(),
                level: None,
            })
        })
    }

    /// 分配新的区域ID
    fn allocate_region_id(&self) -> u64 {
        match self.lock_next_region_id() {
            Ok(mut next) => {
                let id = *next;
                *next = next.wrapping_add(1);
                id
            }
            Err(_) => 0, // Fallback to 0 if lock is poisoned
        }
    }

    /// 添加内存区域
    pub fn add_region(
        &mut self,
        start_addr: u64,
        size: u64,
        region_type: MemoryRegionType,
        pluggable: bool,
    ) -> u64 {
        let id = self.allocate_region_id();
        let region = MemoryRegion {
            id,
            start_addr,
            size,
            region_type,
            pluggable,
            plugged: !pluggable, // 非可插拔区域默认已插入
        };

        if let Ok(mut regions) = self.regions.lock() {
            regions.insert(id, region);
        }

        if !pluggable {
            self.available_memory = self.available_memory.saturating_sub(size);
        }

        id
    }

    /// 插入内存区域（热插拔）
    pub fn plug_region(&mut self, region_id: u64) -> Result<(), VmError> {
        if let Ok(mut regions) = self.regions.lock() {
            if let Some(region) = regions.get_mut(&region_id) {
                if !region.pluggable {
                    return Err(VmError::Core(vm_core::CoreError::Internal {
                        message: "Region is not pluggable".to_string(),
                        module: "VirtIO Memory".to_string(),
                    }));
                }
                if region.plugged {
                    return Err(VmError::Core(vm_core::CoreError::Internal {
                        message: "Region is already plugged".to_string(),
                        module: "VirtIO Memory".to_string(),
                    }));
                }
                region.plugged = true;
                self.available_memory = self.available_memory.saturating_add(region.size);
                Ok(())
            } else {
                Err(VmError::Core(vm_core::CoreError::Internal {
                    message: "Region not found".to_string(),
                    module: "VirtIO Memory".to_string(),
                }))
            }
        } else {
            Err(VmError::Core(vm_core::CoreError::Internal {
                message: "Failed to lock regions".to_string(),
                module: "VirtIO Memory".to_string(),
            }))
        }
    }

    /// 拔出内存区域（热插拔）
    pub fn unplug_region(&mut self, region_id: u64) -> Result<(), VmError> {
        if let Ok(mut regions) = self.regions.lock() {
            if let Some(region) = regions.get_mut(&region_id) {
                if !region.pluggable {
                    return Err(VmError::Core(vm_core::CoreError::Internal {
                        message: "Region is not pluggable".to_string(),
                        module: "VirtIO Memory".to_string(),
                    }));
                }
                if !region.plugged {
                    return Err(VmError::Core(vm_core::CoreError::Internal {
                        message: "Region is not plugged".to_string(),
                        module: "VirtIO Memory".to_string(),
                    }));
                }
                region.plugged = false;
                self.available_memory = self.available_memory.saturating_sub(region.size);
                Ok(())
            } else {
                Err(VmError::Core(vm_core::CoreError::Internal {
                    message: "Region not found".to_string(),
                    module: "VirtIO Memory".to_string(),
                }))
            }
        } else {
            Err(VmError::Core(vm_core::CoreError::Internal {
                message: "Failed to lock regions".to_string(),
                module: "VirtIO Memory".to_string(),
            }))
        }
    }

    /// 获取内存区域
    pub fn get_region(&self, region_id: u64) -> Option<MemoryRegion> {
        if let Ok(regions) = self.regions.lock() {
            regions.get(&region_id).cloned()
        } else {
            None
        }
    }

    /// 获取所有内存区域
    pub fn get_all_regions(&self) -> Vec<MemoryRegion> {
        if let Ok(regions) = self.regions.lock() {
            regions.values().cloned().collect()
        } else {
            Vec::new()
        }
    }

    /// 获取总内存大小
    pub fn total_memory(&self) -> u64 {
        self.total_memory
    }

    /// 获取可用内存大小
    pub fn available_memory(&self) -> u64 {
        self.available_memory
    }

    /// 设置设备状态
    pub fn set_device_status(&mut self, status: u32) {
        self.device_status = status;
    }

    /// 获取设备状态
    pub fn device_status(&self) -> u32 {
        self.device_status
    }
}

impl VirtioDevice for VirtioMemory {
    fn device_id(&self) -> u32 {
        24 // VirtIO Memory device ID (非标准，自定义)
    }

    fn num_queues(&self) -> usize {
        self.queues.len()
    }

    fn get_queue(&mut self, index: usize) -> &mut Queue {
        &mut self.queues[index]
    }

    fn process_queues(&mut self, mmu: &mut dyn MMU) {
        // 处理内存管理请求队列
        while let Some(chain) = self.queues[0].pop(mmu) {
            // 读取请求数据
            let mut request_data = Vec::new();
            for desc in &chain.descs {
                if desc.flags & 0x1 == 0 {
                    // 可读
                    let mut data = vec![0u8; desc.len as usize];
                    if mmu
                        .read_bulk(vm_core::GuestAddr(desc.addr), &mut data)
                        .is_ok()
                    {
                        request_data.extend_from_slice(&data);
                    }
                }
            }

            // 处理请求（简化实现）
            let response_len = if request_data.len() >= 8 {
                let op_code = request_data[0];
                match op_code {
                    0 => {
                        // 查询内存信息
                        16 // 返回总内存和可用内存
                    }
                    1 => {
                        // 插入内存区域
                        if request_data.len() >= 16 {
                            let region_id = u64::from_le_bytes([
                                request_data[8],
                                request_data[9],
                                request_data[10],
                                request_data[11],
                                request_data[12],
                                request_data[13],
                                request_data[14],
                                request_data[15],
                            ]);
                            if self.plug_region(region_id).is_ok() {
                                8
                            } else {
                                0
                            }
                        } else {
                            0
                        }
                    }
                    2 => {
                        // 拔出内存区域
                        if request_data.len() >= 16 {
                            let region_id = u64::from_le_bytes([
                                request_data[8],
                                request_data[9],
                                request_data[10],
                                request_data[11],
                                request_data[12],
                                request_data[13],
                                request_data[14],
                                request_data[15],
                            ]);
                            if self.unplug_region(region_id).is_ok() {
                                8
                            } else {
                                0
                            }
                        } else {
                            0
                        }
                    }
                    _ => 0,
                }
            } else {
                0
            };

            // 标记为已使用
            self.queues[0].add_used(mmu, chain.head_index, response_len);
        }
    }
}

/// VirtIO Memory MMIO设备
pub struct VirtioMemoryMmio {
    device: VirtioMemory,
}

impl VirtioMemoryMmio {
    pub fn new(device: VirtioMemory) -> Self {
        Self { device }
    }

    pub fn device_mut(&mut self) -> &mut VirtioMemory {
        &mut self.device
    }

    pub fn device(&self) -> &VirtioMemory {
        &self.device
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_core::{AddressTranslator, GuestAddr, MemoryAccess, MmioManager, MmuAsAny, VmError};

    struct MockMmu {
        memory: HashMap<u64, u8>,
    }

    // 实现AddressTranslator trait
    impl AddressTranslator for MockMmu {
        fn translate(
            &mut self,
            va: GuestAddr,
            _access: vm_core::AccessType,
        ) -> Result<vm_core::GuestPhysAddr, VmError> {
            Ok(va.into())
        }

        fn flush_tlb(&mut self) {}
    }

    // 实现MemoryAccess trait
    impl MemoryAccess for MockMmu {
        fn read(&self, pa: GuestAddr, size: u8) -> Result<u64, VmError> {
            let mut value = 0u64;
            for i in 0..size {
                let byte = self.memory.get(&(pa.0 + i as u64)).copied().unwrap_or(0);
                value |= (byte as u64) << (i * 8);
            }
            Ok(value)
        }

        fn write(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<(), VmError> {
            for i in 0..size {
                let byte = ((val >> (i * 8)) & 0xFF) as u8;
                self.memory.insert(pa.0 + i as u64, byte);
            }
            Ok(())
        }

        fn fetch_insn(&self, _pc: GuestAddr) -> Result<u64, VmError> {
            Ok(0)
        }

        fn read_bulk(&self, pa: GuestAddr, buf: &mut [u8]) -> Result<(), VmError> {
            for (i, byte) in buf.iter_mut().enumerate() {
                *byte = self.memory.get(&(pa.0 + i as u64)).copied().unwrap_or(0);
            }
            Ok(())
        }

        fn write_bulk(&mut self, pa: GuestAddr, buf: &[u8]) -> Result<(), VmError> {
            for (i, &byte) in buf.iter().enumerate() {
                self.memory.insert(pa.0 + i as u64, byte);
            }
            Ok(())
        }

        fn memory_size(&self) -> usize {
            0
        }

        fn dump_memory(&self) -> Vec<u8> {
            Vec::new()
        }

        fn restore_memory(&mut self, _data: &[u8]) -> Result<(), String> {
            Ok(())
        }
    }

    // 实现MmioManager trait
    impl MmioManager for MockMmu {
        fn map_mmio(&self, _base: GuestAddr, _size: u64, _device: Box<dyn vm_core::MmioDevice>) {}
    }

    // 实现MmuAsAny trait
    impl MmuAsAny for MockMmu {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }
}
