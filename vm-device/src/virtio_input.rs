//! VirtIO Input 设备实现
//!
//! 提供键盘和鼠标输入功能

use crate::virtio::{Queue, VirtioDevice};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use vm_core::{MMU, VmError};

/// 输入事件类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEventType {
    /// 键盘按键
    Key,
    /// 鼠标移动
    Rel,
    /// 鼠标按键
    Btn,
    /// 绝对坐标（触摸屏等）
    Abs,
}

/// 输入事件
#[derive(Debug, Clone)]
pub struct InputEvent {
    /// 事件类型
    pub event_type: InputEventType,
    /// 事件代码
    pub code: u16,
    /// 事件值
    pub value: i32,
}

/// VirtIO Input 设备
pub struct VirtioInput {
    /// VirtIO队列（事件队列和状态队列）
    queues: Vec<Queue>,
    /// 事件队列
    event_queue: Arc<Mutex<VecDeque<InputEvent>>>,
    /// 设备类型
    device_type: InputDeviceType,
    /// 设备状态
    device_status: u32,
}

/// 输入设备类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputDeviceType {
    /// 键盘
    Keyboard,
    /// 鼠标
    Mouse,
    /// 触摸屏
    Touchscreen,
    /// 通用输入设备
    Generic,
}

impl VirtioInput {
    /// 创建新的VirtIO Input设备
    pub fn new(device_type: InputDeviceType) -> Self {
        Self {
            queues: vec![Queue::new(64); 1], // 事件队列
            event_queue: Arc::new(Mutex::new(VecDeque::new())),
            device_type,
            device_status: 0,
        }
    }

    /// 发送输入事件
    pub fn send_event(&self, event: InputEvent) -> Result<(), VmError> {
        let mut queue = self.event_queue.lock().map_err(|_| {
            VmError::Platform(vm_core::PlatformError::AcceleratorUnavailable {
                platform: "VirtIO Input".to_string(),
                reason: "Failed to lock event queue".to_string(),
            })
        })?;

        if queue.len() < 1024 {
            // 限制队列大小
            queue.push_back(event);
            Ok(())
        } else {
            Err(VmError::Platform(
                vm_core::PlatformError::AcceleratorUnavailable {
                    platform: "VirtIO Input".to_string(),
                    reason: "Event queue full".to_string(),
                },
            ))
        }
    }

    /// 发送键盘按键事件
    pub fn send_key_event(&self, key_code: u16, pressed: bool) -> Result<(), VmError> {
        self.send_event(InputEvent {
            event_type: InputEventType::Key,
            code: key_code,
            value: if pressed { 1 } else { 0 },
        })
    }

    /// 发送鼠标移动事件
    pub fn send_mouse_move(&self, x: i32, y: i32) -> Result<(), VmError> {
        self.send_event(InputEvent {
            event_type: InputEventType::Rel,
            code: 0, // REL_X
            value: x,
        })?;
        self.send_event(InputEvent {
            event_type: InputEventType::Rel,
            code: 1, // REL_Y
            value: y,
        })
    }

    /// 发送鼠标按键事件
    pub fn send_mouse_button(&self, button: u16, pressed: bool) -> Result<(), VmError> {
        self.send_event(InputEvent {
            event_type: InputEventType::Btn,
            code: button,
            value: if pressed { 1 } else { 0 },
        })
    }

    /// 获取设备类型
    pub fn device_type(&self) -> InputDeviceType {
        self.device_type
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

impl VirtioDevice for VirtioInput {
    fn device_id(&self) -> u32 {
        18 // VirtIO Input device ID
    }

    fn num_queues(&self) -> usize {
        self.queues.len()
    }

    fn get_queue(&mut self, index: usize) -> &mut Queue {
        &mut self.queues[index]
    }

    fn process_queues(&mut self, mmu: &mut dyn MMU) {
        // 批量处理事件队列（优化性能）
        let chains = self.queues[0].pop_batch(mmu, 16);
        let event_queue = Arc::clone(&self.event_queue);
        let mut queue = match event_queue.lock() {
            Ok(q) => q,
            Err(_) => return,
        };

        let mut entries = Vec::new();

        for chain in chains {
            // 如果有待处理的事件，写入到描述符链
            if let Some(event) = queue.pop_front() {
                let mut written = 0;
                for desc in &chain.descs {
                    if desc.flags & 0x2 != 0 {
                        // 可写
                        // 写入事件数据（简化：只写入基本字段）
                        let event_data = [
                            event.event_type as u16 as u8,
                            (event.event_type as u16 >> 8) as u8,
                            event.code as u8,
                            (event.code >> 8) as u8,
                            event.value as u8,
                            (event.value >> 8) as u8,
                            (event.value >> 16) as u8,
                            (event.value >> 24) as u8,
                        ];

                        let to_write = event_data.len().min(desc.len as usize - written);
                        if mmu
                            .write_bulk(vm_core::GuestAddr(desc.addr), &event_data[..to_write])
                            .is_ok()
                        {
                            written += to_write;
                        }

                        if written >= event_data.len() {
                            break;
                        }
                    }
                }

                if written > 0 {
                    entries.push((chain.head_index, written as u32));
                }
            }
        }

        // 批量标记为已使用
        if !entries.is_empty() {
            self.queues[0].add_used_batch(mmu, &entries);
        }
    }
}

/// VirtIO Input MMIO设备
pub struct VirtioInputMmio {
    device: VirtioInput,
}

impl VirtioInputMmio {
    pub fn new(device: VirtioInput) -> Self {
        Self { device }
    }

    pub fn device_mut(&mut self) -> &mut VirtioInput {
        &mut self.device
    }

    pub fn device(&self) -> &VirtioInput {
        &self.device
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
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
