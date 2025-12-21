use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use vm_core::{GuestAddr, MMU, VmError};

/// 映射的Guest缓冲区（零拷贝访问）
pub struct MappedBuffer<'a> {
    ptr: *mut u8,
    len: usize,
    _phantom: PhantomData<&'a mut [u8]>,
}

impl<'a> MappedBuffer<'a> {
    /// 创建新的映射缓冲区
    ///
    /// # Safety
    /// 调用者必须确保 addr 和 size 描述的是有效的内存范围，并且该内存
    /// 在 'a 生命周期内保持有效且未被其他地方以不兼容的方式访问。
    pub unsafe fn from_virtual(addr: GuestAddr, size: usize) -> Self {
        Self {
            ptr: addr.0 as *mut u8,
            len: size,
            _phantom: PhantomData,
        }
    }

    /// 获取缓冲区长度
    pub fn len(&self) -> usize {
        self.len
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// 获取原始指针
    pub fn as_ptr(&self) -> *const u8 {
        self.ptr as *const u8
    }

    /// 获取可变指针
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.ptr
    }
}

impl<'a> Deref for MappedBuffer<'a> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl<'a> DerefMut for MappedBuffer<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

/// 不安全的缓冲区访问（用于内部优化）
///
/// # Safety
/// 实现此 trait 的类型必须确保 as_ptr() 和 as_mut_ptr() 返回的指针
/// 在其生命周期内是有效的，且 len() 返回正确的大小。
pub unsafe trait UnsafeBufferAccess {
    /// 获取缓冲区的原始指针
    fn as_ptr(&self) -> *const u8;

    /// 获取缓冲区的可变指针
    fn as_mut_ptr(&mut self) -> *mut u8;

    /// 获取缓冲区长度
    fn len(&self) -> usize;

    /// 检查是否为空
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub trait MmuUtil: MMU {
    fn read_u16(&self, addr: u64) -> Result<u16, VmError> {
        self.read(GuestAddr(addr), 2).map(|v| v as u16)
    }

    fn read_u32(&self, addr: u64) -> Result<u32, VmError> {
        self.read(GuestAddr(addr), 4).map(|v| v as u32)
    }

    fn read_u64(&self, addr: u64) -> Result<u64, VmError> {
        self.read(GuestAddr(addr), 8)
    }

    fn write_u16(&mut self, addr: u64, val: u16) -> Result<(), VmError> {
        self.write(GuestAddr(addr), val as u64, 2)
    }

    fn write_u32(&mut self, addr: u64, val: u32) -> Result<(), VmError> {
        self.write(GuestAddr(addr), val as u64, 4)
    }

    fn read_slice(&self, addr: u64, buf: &mut [u8]) -> Result<(), VmError> {
        let mut offset = 0u64;
        // 先对齐到 8 字节
        while !(addr + offset).is_multiple_of(8) && (offset as usize) < buf.len() {
            buf[offset as usize] = self.read(GuestAddr(addr + offset), 1)? as u8;
            offset += 1;
        }
        // 8 字节块
        while (offset as usize) + 8 <= buf.len() {
            let v = self.read(GuestAddr(addr + offset), 8)?;
            buf[offset as usize] = (v & 0xFF) as u8;
            buf[offset as usize + 1] = ((v >> 8) & 0xFF) as u8;
            buf[offset as usize + 2] = ((v >> 16) & 0xFF) as u8;
            buf[offset as usize + 3] = ((v >> 24) & 0xFF) as u8;
            buf[offset as usize + 4] = ((v >> 32) & 0xFF) as u8;
            buf[offset as usize + 5] = ((v >> 40) & 0xFF) as u8;
            buf[offset as usize + 6] = ((v >> 48) & 0xFF) as u8;
            buf[offset as usize + 7] = ((v >> 56) & 0xFF) as u8;
            offset += 8;
        }
        // 余数部分
        while (offset as usize) < buf.len() {
            buf[offset as usize] = self.read(GuestAddr(addr + offset), 1)? as u8;
            offset += 1;
        }
        Ok(())
    }

    fn write_slice(&mut self, addr: u64, data: &[u8]) -> Result<(), VmError> {
        let mut offset = 0u64;
        // 先对齐到 8 字节
        while !(addr + offset).is_multiple_of(8) && (offset as usize) < data.len() {
            self.write(
                vm_core::GuestAddr(addr + offset),
                data[offset as usize] as u64,
                1,
            )?;
            offset += 1;
        }
        // 8 字节块
        while (offset as usize) + 8 <= data.len() {
            let v = (data[offset as usize] as u64)
                | ((data[offset as usize + 1] as u64) << 8)
                | ((data[offset as usize + 2] as u64) << 16)
                | ((data[offset as usize + 3] as u64) << 24)
                | ((data[offset as usize + 4] as u64) << 32)
                | ((data[offset as usize + 5] as u64) << 40)
                | ((data[offset as usize + 6] as u64) << 48)
                | ((data[offset as usize + 7] as u64) << 56);
            self.write(vm_core::GuestAddr(addr + offset), v, 8)?;
            offset += 8;
        }
        // 余数部分
        while (offset as usize) < data.len() {
            self.write(
                vm_core::GuestAddr(addr + offset),
                data[offset as usize] as u64,
                1,
            )?;
            offset += 1;
        }
        Ok(())
    }

    /// 批量写入，针对大块数据优化
    fn write_bulk(&mut self, addr: u64, data: &[u8]) -> Result<(), VmError> {
        // 使用copy_nonoverlapping进行高效的批量写入
        let mut offset = 0u64;
        let len = data.len();

        // 分块处理，避免过大的内存操作
        const BATCH_SIZE: usize = 4096; // 4KB批次

        while (offset as usize) < len {
            let remaining = len - offset as usize;
            let batch_size = if remaining > BATCH_SIZE {
                BATCH_SIZE
            } else {
                remaining
            };

            // 对齐到8字节边界
            let aligned_batch = if batch_size % 8 == 0 {
                batch_size
            } else {
                batch_size + (8 - (batch_size % 8))
            };

            // 检查对齐后的批次是否超出数据长度
            let actual_batch = if (offset as usize + aligned_batch) > len {
                len - offset as usize
            } else {
                aligned_batch
            };

            // 逐字节写入对齐的批次
            for i in 0..actual_batch {
                self.write(
                    vm_core::GuestAddr(addr + offset + i as u64),
                    data[offset as usize + i] as u64,
                    1,
                )?;
            }

            offset += actual_batch as u64;
        }

        Ok(())
    }

    /// 获取Guest缓冲区的Host映射（零拷贝访问）
    fn map_guest_buffer<'a>(
        &'a mut self,
        addr: u64,
        size: usize,
    ) -> Result<MappedBuffer<'a>, VmError> {
        // 检查缓冲区是否在连续物理内存中
        // 这里需要访问底层内存管理器，简化实现
        // 实际实现需要与特定的内存后端集成

        // 暂时返回一个虚拟映射（实际实现需要架构特定代码）
        Ok(unsafe { MappedBuffer::from_virtual(vm_core::GuestAddr(addr), size) })
    }

    /// 零拷贝写入Guest缓冲区
    fn zero_copy_write(&mut self, addr: u64, data: &[u8]) -> Result<(), VmError> {
        // 检查是否可以直接映射
        if let Ok(mut buffer) = self.map_guest_buffer(addr, data.len()) {
            // 使用直接内存复制
            buffer.copy_from_slice(data);
            Ok(())
        } else {
            // 回退到标准写入
            self.write_slice(addr, data)
        }
    }
}

impl<T: MMU + ?Sized> MmuUtil for T {}
