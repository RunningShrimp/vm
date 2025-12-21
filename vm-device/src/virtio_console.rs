//! VirtIO Console 设备实现
//!
//! 提供串口控制台功能，支持多端口和流控制

use crate::virtio::{Queue, VirtioDevice};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use vm_core::{MMU, VmError};

/// VirtIO Console 设备
pub struct VirtioConsole {
    /// VirtIO队列（接收和发送各一个）
    queues: Vec<Queue>,
    /// 接收缓冲区
    receive_buffer: Arc<Mutex<VecDeque<u8>>>,
    /// 发送缓冲区
    send_buffer: Arc<Mutex<VecDeque<u8>>>,
    /// 端口数量
    num_ports: u16,
    /// 设备状态
    device_status: u32,
}

impl VirtioConsole {
    /// 创建新的VirtIO Console设备
    pub fn new(num_ports: u16) -> Self {
        Self {
            queues: vec![Queue::new(256); 2], // 接收队列和发送队列
            receive_buffer: Arc::new(Mutex::new(VecDeque::new())),
            send_buffer: Arc::new(Mutex::new(VecDeque::new())),
            num_ports,
            device_status: 0,
        }
    }

    /// 写入数据到控制台（从主机到客户机）
    pub fn write_to_guest(&self, data: &[u8]) -> Result<usize, VmError> {
        let mut buffer = self.receive_buffer.lock().map_err(|_| {
            VmError::Platform(vm_core::PlatformError::AcceleratorUnavailable {
                platform: "VirtIO Console".to_string(),
                reason: "Failed to lock receive buffer".to_string(),
            })
        })?;

        let written = data.len().min(4096 - buffer.len()); // 限制缓冲区大小
        buffer.extend(data.iter().take(written));
        Ok(written)
    }

    /// 从控制台读取数据（从客户机到主机）
    pub fn read_from_guest(&self, buf: &mut [u8]) -> Result<usize, VmError> {
        let mut buffer = self.send_buffer.lock().map_err(|_| {
            VmError::Platform(vm_core::PlatformError::AcceleratorUnavailable {
                platform: "VirtIO Console".to_string(),
                reason: "Failed to lock send buffer".to_string(),
            })
        })?;

        let mut read = 0;
        while read < buf.len() && !buffer.is_empty() {
            if let Some(byte) = buffer.pop_front() {
                buf[read] = byte;
                read += 1;
            } else {
                break;
            }
        }

        Ok(read)
    }

    /// 获取端口数量
    pub fn num_ports(&self) -> u16 {
        self.num_ports
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

impl VirtioDevice for VirtioConsole {
    fn device_id(&self) -> u32 {
        3 // VirtIO Console device ID
    }

    fn num_queues(&self) -> usize {
        self.queues.len()
    }

    fn get_queue(&mut self, index: usize) -> &mut Queue {
        &mut self.queues[index]
    }

    fn process_queues(&mut self, mmu: &mut dyn MMU) {
        // 批量处理接收队列（索引0）：从主机到客户机
        let receive_chains = self.queues[0].pop_batch(mmu, 16); // 批量处理最多16个请求
        let mut receive_entries = Vec::new();

        for chain in receive_chains {
            let mut data = Vec::new();
            for desc in &chain.descs {
                if desc.flags & 0x1 == 0 {
                    // 可读
                    let mut bytes = vec![0u8; desc.len as usize];
                    if mmu
                        .read_bulk(vm_core::GuestAddr(desc.addr), &mut bytes)
                        .is_ok()
                    {
                        data.extend_from_slice(&bytes);
                    }
                }
            }

            // 将数据放入接收缓冲区
            let data_len = data.len();
            if let Ok(mut buffer) = self.receive_buffer.lock() {
                buffer.extend(data);
            }

            receive_entries.push((chain.head_index, data_len as u32));
        }

        // 批量标记为已使用
        if !receive_entries.is_empty() {
            self.queues[0].add_used_batch(mmu, &receive_entries);
        }

        // 批量处理发送队列（索引1）：从客户机到主机
        let send_chains = self.queues[1].pop_batch(mmu, 16); // 批量处理最多16个请求
        let mut send_entries = Vec::new();

        for chain in send_chains {
            let mut data = Vec::new();
            for desc in &chain.descs {
                if desc.flags & 0x1 == 0 {
                    // 可读
                    let mut bytes = vec![0u8; desc.len as usize];
                    if mmu
                        .read_bulk(vm_core::GuestAddr(desc.addr), &mut bytes)
                        .is_ok()
                    {
                        data.extend_from_slice(&bytes);
                    }
                }
            }

            // 将数据放入发送缓冲区
            let data_len = data.len();
            if let Ok(mut buffer) = self.send_buffer.lock() {
                buffer.extend(data);
            }

            send_entries.push((chain.head_index, data_len as u32));
        }

        // 批量标记为已使用
        if !send_entries.is_empty() {
            self.queues[1].add_used_batch(mmu, &send_entries);
        }
    }
}

/// VirtIO Console MMIO设备
pub struct VirtioConsoleMmio {
    device: std::sync::Arc<parking_lot::Mutex<VirtioConsole>>,
}

impl VirtioConsoleMmio {
    pub fn new(device: VirtioConsole) -> Self {
        Self {
            device: std::sync::Arc::new(parking_lot::Mutex::new(device)),
        }
    }

    pub fn from_arc(device: std::sync::Arc<parking_lot::Mutex<VirtioConsole>>) -> Self {
        Self { device }
    }

    pub fn device_mut(&mut self) -> parking_lot::MutexGuard<'_, VirtioConsole> {
        self.device.lock()
    }

    pub fn device(&self) -> parking_lot::MutexGuard<'_, VirtioConsole> {
        self.device.lock()
    }
}

impl vm_core::MmioDevice for VirtioConsoleMmio {
    fn read(&self, offset: u64, _size: u8) -> vm_core::VmResult<u64> {
        let device = self.device.lock();

        // VirtIO MMIO 寄存器布局
        // 0x000: Magic value ("virt")
        // 0x004: Version
        // 0x008: Device ID
        // 0x00c: Vendor ID
        // 0x010: Device features
        // 0x014: Device features sel
        // 0x020: Driver features
        // 0x024: Driver features sel
        // 0x028: Queue sel
        // 0x030: Queue num max
        // 0x034: Queue num
        // 0x038: Queue ready
        // 0x044: Queue notify
        // 0x060: Interrupt status
        // 0x064: Status

        match offset {
            0x000 => Ok(0x74726976), // "virt" in little-endian
            0x004 => Ok(2),          // Version 2
            0x008 => Ok(device.device_id() as u64),
            0x00c => Ok(0x554d4551), // QEMU vendor ID
            0x010 => Ok(0),          // Device features (简化实现)
            0x014 => Ok(0),
            0x060 => Ok(0), // Interrupt status
            0x064 => Ok(device.device_status() as u64),
            _ => Ok(0),
        }
    }

    fn write(&mut self, offset: u64, value: u64, _size: u8) -> vm_core::VmResult<()> {
        let mut device = self.device.lock();

        match offset {
            0x020 => {
                // Driver features (忽略)
            }
            0x024 => {
                // Driver features sel (忽略)
            }
            0x028 => {
                // Queue sel (忽略)
            }
            0x034 => {
                // Queue num (忽略)
            }
            0x038 => {
                // Queue ready (忽略)
            }
            0x044 => {
                // Queue notify - 触发队列处理
                // 这里应该处理队列，但需要 MMU 访问，所以暂时忽略
            }
            0x064 => {
                // Status
                device.set_device_status(value as u32);
            }
            _ => {
                // 其他寄存器写入忽略
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_core::{
        AccessType, AddressTranslator, GuestAddr, GuestPhysAddr, MemoryAccess, MmioManager,
        MmuAsAny, VmError,
    };

    struct MockMmu {
        memory: std::collections::HashMap<u64, u8>,
    }

    impl AddressTranslator for MockMmu {
        fn translate(
            &mut self,
            va: GuestAddr,
            _access: AccessType,
        ) -> Result<GuestPhysAddr, VmError> {
            Ok(GuestPhysAddr(va.0))
        }

        fn flush_tlb(&mut self) {}
    }

    impl MemoryAccess for MockMmu {
        fn fetch_insn(&self, _pc: GuestAddr) -> Result<u64, VmError> {
            Ok(0)
        }

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

    impl MmioManager for MockMmu {
        fn map_mmio(&self, _base: GuestAddr, _size: u64, _device: Box<dyn vm_core::MmioDevice>) {}
    }

    impl MmuAsAny for MockMmu {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    #[test]
    fn test_virtio_console_creation() {
        let console = VirtioConsole::new(1);
        assert_eq!(console.num_ports(), 1);
        assert_eq!(console.device_status(), 0);
    }

    #[test]
    fn test_virtio_console_write_read() {
        let console = VirtioConsole::new(1);

        // 写入数据到控制台
        let data = b"Hello, World!";
        let written = console.write_to_guest(data).unwrap();
        assert_eq!(written, data.len());

        // 从控制台读取数据
        let mut buf = vec![0u8; 100];
        let read = console.read_from_guest(&mut buf).unwrap();
        assert_eq!(read, 0); // 发送缓冲区为空（数据在接收缓冲区）
    }

    #[test]
    fn test_virtio_console_device_id() {
        let console = VirtioConsole::new(1);
        let _mmu = MockMmu {
            memory: std::collections::HashMap::new(),
        };

        assert_eq!(console.device_id(), 3); // VirtIO Console device ID
        assert_eq!(console.num_queues(), 2); // 接收和发送队列
    }
}
