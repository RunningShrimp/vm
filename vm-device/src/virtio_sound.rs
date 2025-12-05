//! VirtIO Sound 设备实现
//!
//! 提供音频输入输出功能

use crate::virtio::{Queue, VirtioDevice};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use vm_core::{MMU, VmError};

/// 音频流方向
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamDirection {
    /// 输入（录音）
    Input,
    /// 输出（播放）
    Output,
}

/// 音频格式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    /// PCM 8位无符号
    PcmU8,
    /// PCM 16位有符号（小端）
    PcmS16Le,
    /// PCM 24位有符号（小端）
    PcmS24Le,
    /// PCM 32位有符号（小端）
    PcmS32Le,
    /// PCM 浮点32位（小端）
    PcmF32Le,
}

/// 音频流配置
#[derive(Debug, Clone)]
pub struct StreamConfig {
    /// 采样率（Hz）
    pub sample_rate: u32,
    /// 声道数
    pub channels: u8,
    /// 音频格式
    pub format: AudioFormat,
    /// 缓冲区大小（帧数）
    pub buffer_size: u32,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            channels: 2,
            format: AudioFormat::PcmS16Le,
            buffer_size: 1024,
        }
    }
}

/// VirtIO Sound 设备
pub struct VirtioSound {
    /// VirtIO队列（每个流一个队列）
    queues: Vec<Queue>,
    /// 输入流配置
    input_config: StreamConfig,
    /// 输出流配置
    output_config: StreamConfig,
    /// 输入音频缓冲区
    input_buffer: Arc<Mutex<VecDeque<u8>>>,
    /// 输出音频缓冲区
    output_buffer: Arc<Mutex<VecDeque<u8>>>,
    /// 设备状态
    device_status: u32,
    /// 是否启用输入
    input_enabled: bool,
    /// 是否启用输出
    output_enabled: bool,
}

impl VirtioSound {
    /// 创建新的VirtIO Sound设备
    pub fn new() -> Self {
        Self {
            queues: vec![Queue::new(256); 2], // 输入队列和输出队列
            input_config: StreamConfig::default(),
            output_config: StreamConfig::default(),
            input_buffer: Arc::new(Mutex::new(VecDeque::new())),
            output_buffer: Arc::new(Mutex::new(VecDeque::new())),
            device_status: 0,
            input_enabled: false,
            output_enabled: false,
        }
    }

    /// 设置输入流配置
    pub fn set_input_config(&mut self, config: StreamConfig) {
        self.input_config = config;
    }

    /// 获取输入流配置
    pub fn input_config(&self) -> &StreamConfig {
        &self.input_config
    }

    /// 设置输出流配置
    pub fn set_output_config(&mut self, config: StreamConfig) {
        self.output_config = config;
    }

    /// 获取输出流配置
    pub fn output_config(&self) -> &StreamConfig {
        &self.output_config
    }

    /// 启用输入
    pub fn enable_input(&mut self) {
        self.input_enabled = true;
    }

    /// 禁用输入
    pub fn disable_input(&mut self) {
        self.input_enabled = false;
    }

    /// 启用输出
    pub fn enable_output(&mut self) {
        self.output_enabled = true;
    }

    /// 禁用输出
    pub fn disable_output(&mut self) {
        self.output_enabled = false;
    }

    /// 从输入缓冲区读取音频数据
    pub fn read_audio(&self, buf: &mut [u8]) -> Result<usize, VmError> {
        let mut buffer = self.input_buffer.lock().map_err(|_| {
            VmError::Platform(vm_core::PlatformError::AcceleratorUnavailable {
                platform: "VirtIO Sound".to_string(),
                reason: "Failed to lock input buffer".to_string(),
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

    /// 写入音频数据到输出缓冲区
    pub fn write_audio(&self, data: &[u8]) -> Result<usize, VmError> {
        let mut buffer = self.output_buffer.lock().map_err(|_| {
            VmError::Platform(vm_core::PlatformError::AcceleratorUnavailable {
                platform: "VirtIO Sound".to_string(),
                reason: "Failed to lock output buffer".to_string(),
            })
        })?;

        let max_size = 65536; // 限制缓冲区大小
        let to_write = data.len().min(max_size - buffer.len());
        buffer.extend(data.iter().take(to_write));
        Ok(to_write)
    }

    /// 处理输入队列（从客户机接收音频数据）
    fn process_input_queue(&mut self, mmu: &mut dyn MMU) {
        while let Some(chain) = self.queues[0].pop(mmu) {
            let mut total_read = 0;

            for desc in &chain.descs {
                if desc.flags & 0x1 == 0 {
                    // 可读
                    let mut data = vec![0u8; desc.len as usize];
                    if mmu.read_bulk(desc.addr, &mut data).is_ok() {
                        // 将数据放入输入缓冲区
                        if let Ok(mut buffer) = self.input_buffer.lock() {
                            buffer.extend(&data);
                            total_read += data.len();
                        }
                    }
                }
            }

            // 标记为已使用
            self.queues[0].add_used(mmu, chain.head_index, total_read as u32);
        }
    }

    /// 处理输出队列（向客户机发送音频数据）
    fn process_output_queue(&mut self, mmu: &mut dyn MMU) {
        while let Some(chain) = self.queues[1].pop(mmu) {
            let mut total_written = 0;

            if let Ok(mut buffer) = self.output_buffer.lock() {
                for desc in &chain.descs {
                    if desc.flags & 0x2 != 0 {
                        // 可写
                        let to_write = desc.len as usize;
                        let mut data = vec![0u8; to_write];

                        // 从输出缓冲区读取数据
                        let mut read = 0;
                        while read < to_write && !buffer.is_empty() {
                            if let Some(byte) = buffer.pop_front() {
                                data[read] = byte;
                                read += 1;
                            } else {
                                break;
                            }
                        }

                        if read > 0 {
                            if mmu.write_bulk(desc.addr, &data[..read]).is_ok() {
                                total_written += read;
                            }
                        }
                    }
                }
            }

            // 标记为已使用
            self.queues[1].add_used(mmu, chain.head_index, total_written as u32);
        }
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

impl Default for VirtioSound {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtioDevice for VirtioSound {
    fn device_id(&self) -> u32 {
        25 // VirtIO Sound device ID (非标准，自定义)
    }

    fn num_queues(&self) -> usize {
        self.queues.len()
    }

    fn get_queue(&mut self, index: usize) -> &mut Queue {
        &mut self.queues[index]
    }

    fn process_queues(&mut self, mmu: &mut dyn MMU) {
        // 处理输入队列
        if self.input_enabled {
            self.process_input_queue(mmu);
        }

        // 处理输出队列
        if self.output_enabled {
            self.process_output_queue(mmu);
        }
    }
}

/// VirtIO Sound MMIO设备
pub struct VirtioSoundMmio {
    device: VirtioSound,
}

impl VirtioSoundMmio {
    pub fn new(device: VirtioSound) -> Self {
        Self { device }
    }

    pub fn device_mut(&mut self) -> &mut VirtioSound {
        &mut self.device
    }

    pub fn device(&self) -> &VirtioSound {
        &self.device
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_core::GuestAddr;

    #[test]
    fn test_virtio_sound_creation() {
        let sound = VirtioSound::new();
        assert_eq!(sound.input_config().sample_rate, 44100);
        assert_eq!(sound.output_config().sample_rate, 44100);
        assert!(!sound.input_enabled);
        assert!(!sound.output_enabled);
    }

    #[test]
    fn test_virtio_sound_config() {
        let mut sound = VirtioSound::new();
        let mut config = StreamConfig::default();
        config.sample_rate = 48000;
        sound.set_input_config(config.clone());
        sound.set_output_config(config);

        assert_eq!(sound.input_config().sample_rate, 48000);
        assert_eq!(sound.output_config().sample_rate, 48000);
    }

    #[test]
    fn test_virtio_sound_device_id() {
        let mut sound = VirtioSound::new();
        let mut mmu = MockMmu {
            memory: std::collections::HashMap::new(),
        };

        assert_eq!(sound.device_id(), 25); // VirtIO Sound device ID
        assert_eq!(sound.num_queues(), 2); // 输入队列和输出队列
    }

    struct MockMmu {
        memory: std::collections::HashMap<u64, u8>,
    }

    impl MMU for MockMmu {
        fn translate(
            &mut self,
            va: GuestAddr,
            _access: vm_core::AccessType,
        ) -> Result<vm_core::GuestPhysAddr, VmError> {
            Ok(va)
        }

        fn fetch_insn(&self, _pc: GuestAddr) -> Result<u64, VmError> {
            Ok(0)
        }

        fn read(&self, pa: GuestAddr, size: u8) -> Result<u64, VmError> {
            let mut value = 0u64;
            for i in 0..size {
                let byte = self.memory.get(&(pa + i as u64)).copied().unwrap_or(0);
                value |= (byte as u64) << (i * 8);
            }
            Ok(value)
        }

        fn write(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<(), VmError> {
            for i in 0..size {
                let byte = ((val >> (i * 8)) & 0xFF) as u8;
                self.memory.insert(pa + i as u64, byte);
            }
            Ok(())
        }

        fn read_bulk(&self, pa: GuestAddr, buf: &mut [u8]) -> Result<(), VmError> {
            for (i, byte) in buf.iter_mut().enumerate() {
                *byte = self.memory.get(&(pa + i as u64)).copied().unwrap_or(0);
            }
            Ok(())
        }

        fn write_bulk(&mut self, pa: GuestAddr, buf: &[u8]) -> Result<(), VmError> {
            for (i, &byte) in buf.iter().enumerate() {
                self.memory.insert(pa + i as u64, byte);
            }
            Ok(())
        }

        fn map_mmio(
            &mut self,
            _base: GuestAddr,
            _size: u64,
            _device: Box<dyn vm_core::MmioDevice>,
        ) {
        }
        fn flush_tlb(&mut self) {}
        fn memory_size(&self) -> usize {
            0
        }
        fn dump_memory(&self) -> Vec<u8> {
            Vec::new()
        }
        fn restore_memory(&mut self, _data: &[u8]) -> Result<(), String> {
            Ok(())
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }
}
