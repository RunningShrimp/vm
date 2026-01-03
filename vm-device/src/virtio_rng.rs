//! VirtIO RNG (Random Number Generator) 设备实现
//!
//! 提供硬件随机数生成器功能

use vm_core::MMU;

use crate::virtio::{Queue, VirtioDevice};

/// VirtIO RNG 设备
pub struct VirtioRng {
    /// VirtIO队列
    queues: Vec<Queue>,
    /// 随机数种子
    seed: u64,
    /// 设备状态
    device_status: u32,
}

impl VirtioRng {
    /// 创建新的VirtIO RNG设备
    pub fn new() -> Self {
        Self {
            queues: vec![Queue::new(256); 1],
            seed: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0),
            device_status: 0,
        }
    }

    /// 生成随机字节（使用简单的线性同余生成器）
    fn generate_random_bytes(&self, len: usize) -> Vec<u8> {
        let mut bytes = vec![0u8; len];
        let mut state = self.seed;

        for byte in &mut bytes {
            // 简单的线性同余生成器
            state = state.wrapping_mul(1103515245).wrapping_add(12345);
            *byte = (state >> 24) as u8;
        }

        bytes
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

impl Default for VirtioRng {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtioDevice for VirtioRng {
    fn device_id(&self) -> u32 {
        4 // VirtIO RNG device ID
    }

    fn num_queues(&self) -> usize {
        self.queues.len()
    }

    fn get_queue(&mut self, index: usize) -> &mut Queue {
        &mut self.queues[index]
    }

    fn process_queues(&mut self, mmu: &mut dyn MMU) {
        // 批量处理随机数请求队列（优化性能）
        let chains = self.queues[0].pop_batch(mmu, 16);
        let mut entries = Vec::new();

        for chain in chains {
            let mut total_written = 0;

            // 遍历描述符链，写入随机数
            for desc in &chain.descs {
                if desc.flags & 0x2 != 0 {
                    // 可写
                    let len = desc.len as usize;
                    let random_bytes = self.generate_random_bytes(len);

                    if mmu
                        .write_bulk(vm_core::GuestAddr(desc.addr), &random_bytes)
                        .is_ok()
                    {
                        total_written += len;
                    }
                }
            }

            if total_written > 0 {
                entries.push((chain.head_index, total_written as u32));
            }
        }

        // 批量标记为已使用
        if !entries.is_empty() {
            self.queues[0].add_used_batch(mmu, &entries);
        }
    }
}

/// VirtIO RNG MMIO设备
pub struct VirtioRngMmio {
    device: VirtioRng,
}

impl VirtioRngMmio {
    pub fn new(device: VirtioRng) -> Self {
        Self { device }
    }

    pub fn device_mut(&mut self) -> &mut VirtioRng {
        &mut self.device
    }

    pub fn device(&self) -> &VirtioRng {
        &self.device
    }
}

#[cfg(test)]
mod tests {
    use vm_core::{AddressTranslator, GuestAddr, MemoryAccess, MmioManager, MmuAsAny, VmError};

    use super::*;

    struct MockMmu {
        memory: std::collections::HashMap<u64, u8>,
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

    #[test]
    fn test_virtio_rng_creation() {
        let rng = VirtioRng::new();
        assert_eq!(rng.device_status(), 0);
    }

    #[test]
    fn test_virtio_rng_device_id() {
        let mut rng = VirtioRng::new();
        let mut mmu = MockMmu {
            memory: std::collections::HashMap::new(),
        };

        assert_eq!(rng.device_id(), 4); // VirtIO RNG device ID
        assert_eq!(rng.num_queues(), 1); // 随机数请求队列
    }

    #[test]
    fn test_virtio_rng_generate_bytes() {
        let rng = VirtioRng::new();

        // 生成随机字节
        let bytes1 = rng.generate_random_bytes(16);
        let bytes2 = rng.generate_random_bytes(16);

        // 验证生成了正确数量的字节
        assert_eq!(bytes1.len(), 16);
        assert_eq!(bytes2.len(), 16);

        // 验证生成的字节不为全零（虽然理论上可能，但概率极低）
        // 这里只检查长度，不检查随机性
    }
}
