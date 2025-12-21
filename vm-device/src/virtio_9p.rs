//! VirtIO 9P 设备实现
//!
//! 提供9P文件系统协议支持，允许客户机访问主机文件系统

use crate::virtio::{Queue, VirtioDevice};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use vm_core::{GuestAddr, MMU};

/// 9P文件系统标签
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Tag(u16);

/// 9P文件句柄
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fid(u32);

/// 9P文件类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    /// 目录
    Directory,
    /// 普通文件
    Regular,
    /// 符号链接
    Symlink,
}

/// 9P文件信息
#[derive(Debug, Clone)]
pub struct FileInfo {
    /// 文件类型
    pub file_type: FileType,
    /// 文件大小
    pub size: u64,
    /// 访问权限
    pub mode: u32,
    /// 修改时间
    pub mtime: u64,
}

/// VirtIO 9P 设备
pub struct Virtio9P {
    /// VirtIO队列（请求队列）
    queues: Vec<Queue>,
    /// 挂载点路径
    mount_point: PathBuf,
    /// 文件句柄映射
    fids: Arc<Mutex<HashMap<Fid, PathBuf>>>,
    /// 标签到FID的映射
    tag_to_fid: Arc<Mutex<HashMap<Tag, Fid>>>,
    /// 下一个FID
    next_fid: Arc<Mutex<u32>>,
    /// 设备状态
    device_status: u32,
    /// 最大消息大小
    max_message_size: u32,
}

impl Virtio9P {
    /// 创建新的VirtIO 9P设备
    pub fn new<P: AsRef<Path>>(mount_point: P) -> Self {
        Self {
            queues: vec![Queue::new(256); 1],
            mount_point: mount_point.as_ref().to_path_buf(),
            fids: Arc::new(Mutex::new(HashMap::new())),
            tag_to_fid: Arc::new(Mutex::new(HashMap::new())),
            next_fid: Arc::new(Mutex::new(1)),
            device_status: 0,
            max_message_size: 4096,
        }
    }

    /// 分配新的FID
    fn allocate_fid(&self) -> Fid {
        let mut next = self.next_fid.lock().unwrap();
        let fid = Fid(*next);
        *next = next.wrapping_add(1);
        fid
    }

    /// 处理9P请求
    fn process_request(&mut self, mmu: &mut dyn MMU, chain: &crate::virtio::DescChain) -> u32 {
        let mut request_data = Vec::new();

        // 读取请求数据
        for desc in &chain.descs {
            if desc.flags & 0x1 == 0 {
                // 可读
                let mut data = vec![0u8; desc.len as usize];
                if mmu.read_bulk(GuestAddr(desc.addr), &mut data).is_ok() {
                    request_data.extend_from_slice(&data);
                }
            }
        }

        if request_data.len() < 4 {
            return 0;
        }

        // 解析9P消息类型（简化实现）
        let message_type = u8::from_le_bytes([request_data[0]]);
        let tag = u16::from_le_bytes([request_data[1], request_data[2]]);

        // 处理不同类型的9P消息

        match message_type {
            6 => self.handle_tversion(mmu, &request_data, tag), // TVERSION
            7 => self.handle_rversion(mmu, &request_data, tag), // RVERSION
            100 => self.handle_tattach(mmu, &request_data, tag), // TATTACH
            101 => self.handle_rattach(mmu, &request_data, tag), // RATTACH
            108 => self.handle_tread(mmu, &request_data, tag),  // TREAD
            109 => self.handle_rread(mmu, &request_data, tag),  // RREAD
            _ => {
                // 未知消息类型，返回错误
                0
            }
        }
    }

    /// 处理TVERSION请求
    fn handle_tversion(&mut self, _mmu: &mut dyn MMU, data: &[u8], _tag: u16) -> u32 {
        // TVERSION请求格式：message_type(1) + tag(2) + msize(4) + version string
        if data.len() >= 7 {
            let client_msize = u32::from_le_bytes([data[3], data[4], data[5], data[6]]);
            // 使用较小的消息大小作为协商结果
            self.max_message_size = std::cmp::min(self.max_message_size, client_msize);
            log::debug!("9P: TVersion negotiated msize: {}", self.max_message_size);
        }
        // 返回版本响应长度（简化）
        0
    }

    /// 处理RVERSION响应
    fn handle_rversion(&mut self, _mmu: &mut dyn MMU, _data: &[u8], _tag: u16) -> u32 {
        0
    }

    /// 处理TATTACH请求
    fn handle_tattach(&mut self, _mmu: &mut dyn MMU, _data: &[u8], _tag: u16) -> u32 {
        // 分配FID并关联到根目录
        let fid = self.allocate_fid();
        if let Ok(mut fids) = self.fids.lock() {
            fids.insert(fid, self.mount_point.clone());
        }
        if let Ok(mut tag_map) = self.tag_to_fid.lock() {
            tag_map.insert(Tag(_tag), fid);
        }
        0
    }

    /// 处理RATTACH响应
    fn handle_rattach(&mut self, _mmu: &mut dyn MMU, _data: &[u8], _tag: u16) -> u32 {
        0
    }

    /// 处理TREAD请求
    fn handle_tread(&mut self, _mmu: &mut dyn MMU, _data: &[u8], _tag: u16) -> u32 {
        // 简化实现：读取文件数据
        0
    }

    /// 处理RREAD响应
    fn handle_rread(&mut self, _mmu: &mut dyn MMU, _data: &[u8], _tag: u16) -> u32 {
        0
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

impl VirtioDevice for Virtio9P {
    fn device_id(&self) -> u32 {
        9 // VirtIO 9P device ID
    }

    fn num_queues(&self) -> usize {
        self.queues.len()
    }

    fn get_queue(&mut self, index: usize) -> &mut Queue {
        &mut self.queues[index]
    }

    fn process_queues(&mut self, mmu: &mut dyn MMU) {
        // 处理请求队列
        while let Some(chain) = self.queues[0].pop(mmu) {
            let response_len = self.process_request(mmu, &chain);

            // 标记为已使用
            self.queues[0].add_used(mmu, chain.head_index, response_len);
        }
    }
}

/// VirtIO 9P MMIO设备
pub struct Virtio9PMmio {
    device: Virtio9P,
}

impl Virtio9PMmio {
    pub fn new(device: Virtio9P) -> Self {
        Self { device }
    }

    pub fn device_mut(&mut self) -> &mut Virtio9P {
        &mut self.device
    }

    pub fn device(&self) -> &Virtio9P {
        &self.device
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_core::{
        AccessType, AddressTranslator, GuestAddr, GuestPhysAddr, MemoryAccess, MmioManager,
        MmuAsAny, VmError,
    };

    #[test]
    fn test_virtio_9p_creation() {
        let fs = Virtio9P::new("/tmp");
        assert_eq!(fs.device_status(), 0);
    }

    #[test]
    fn test_virtio_9p_device_id() {
        let fs = Virtio9P::new("/tmp");
        let _mmu = MockMmu {
            memory: std::collections::HashMap::new(),
        };

        assert_eq!(fs.device_id(), 9); // VirtIO 9P device ID
        assert_eq!(fs.num_queues(), 1); // 请求队列
    }

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
}
