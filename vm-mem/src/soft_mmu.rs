//! 软件MMU实现
//!
//! 实现高效的GVA -> GPA -> HVA的地址转换，并提供优化的批量内存访问

use std::ptr;
use vm_core::{GuestAddr, VmError, MmuUtil};

/// 软件MMU实现，提供地址转换和批量内存访问
pub struct SoftMmu {
    // 内存区域
    regions: Vec<MemoryRegion>,
    // 页面大小
    page_size: u64,
    // 对齐掩码
    page_mask: u64,
}

/// 内存区域
#[derive(Debug)]
struct MemoryRegion {
    /// 起始地址
    start: GuestAddr,
    /// 结束地址
    end: GuestAddr,
    /// 内存数据
    data: Vec<u8>,
}

impl SoftMmu {
    /// 创建新的软件MMU
    pub fn new(page_size: u64) -> Self {
        let page_mask = page_size - 1;
        Self {
            regions: Vec::new(),
            page_size,
            page_mask,
        }
    }
    
    /// 添加内存区域
    pub fn add_region(&mut self, start: GuestAddr, data: Vec<u8>) {
        let end = GuestAddr(start.0 + data.len() as u64);
        self.regions.push(MemoryRegion {
            start,
            end,
            data,
        });
    }
    
    /// 查找内存区域
    fn find_region(&self, addr: GuestAddr) -> Option<&MemoryRegion> {
        self.regions.iter().find(|region| addr >= region.start && addr < region.end)
    }
}

impl MmuUtil for SoftMmu {
    fn read_u16(&self, addr: u64) -> Result<u16, VmError> {
        let region = self.find_region(GuestAddr(addr)).ok_or(VmError::InvalidAddress(GuestAddr(addr)))?;
        let offset = (addr - region.start.0) as usize;
        if offset + 2 > region.data.len() {
            return Err(VmError::InvalidAddress(GuestAddr(addr)));
        }
        let bytes = &region.data[offset..offset + 2];
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32(&self, addr: u64) -> Result<u32, VmError> {
        let region = self.find_region(GuestAddr(addr)).ok_or(VmError::InvalidAddress(GuestAddr(addr)))?;
        let offset = (addr - region.start.0) as usize;
        if offset + 4 > region.data.len() {
            return Err(VmError::InvalidAddress(GuestAddr(addr)));
        }
        let bytes = &region.data[offset..offset + 4];
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_u64(&self, addr: u64) -> Result<u64, VmError> {
        let region = self.find_region(GuestAddr(addr)).ok_or(VmError::InvalidAddress(GuestAddr(addr)))?;
        let offset = (addr - region.start.0) as usize;
        if offset + 8 > region.data.len() {
            return Err(VmError::InvalidAddress(GuestAddr(addr)));
        }
        let bytes = &region.data[offset..offset + 8];
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3],
            bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn write_u16(&mut self, addr: u64, val: u16) -> Result<(), VmError> {
        let region = self.find_region_mut(GuestAddr(addr)).ok_or(VmError::InvalidAddress(GuestAddr(addr)))?;
        let offset = (addr - region.start.0) as usize;
        if offset + 2 > region.data.len() {
            return Err(VmError::InvalidAddress(GuestAddr(addr)));
        }
        let bytes = val.to_le_bytes();
        region.data[offset..offset + 2].copy_from_slice(&bytes);
        Ok(())
    }

    fn write_u32(&mut self, addr: u64, val: u32) -> Result<(), VmError> {
        let region = self.find_region_mut(GuestAddr(addr)).ok_or(VmError::InvalidAddress(GuestAddr(addr)))?;
        let offset = (addr - region.start.0) as usize;
        if offset + 4 > region.data.len() {
            return Err(VmError::InvalidAddress(GuestAddr(addr)));
        }
        let bytes = val.to_le_bytes();
        region.data[offset..offset + 4].copy_from_slice(&bytes);
        Ok(())
    }

    /// 优化的批量读取，使用copy_nonoverlapping提高性能
    fn read_slice(&mut self, addr: u64, buf: &mut [u8]) -> Result<(), VmError> {
        let region = self.find_region_mut(GuestAddr(addr)).ok_or(VmError::InvalidAddress(GuestAddr(addr)))?;
        let offset = (addr - region.start.0) as usize;
        if offset + buf.len() > region.data.len() {
            return Err(VmError::InvalidAddress(GuestAddr(addr)));
        }
        
        // 使用copy_nonoverlapping实现高效批量读取
        unsafe {
            ptr::copy_nonoverlapping(
                region.data.as_ptr().add(offset),
                buf.as_mut_ptr(),
                buf.len(),
            );
        }
        Ok(())
    }

    /// 优化的批量写入，使用copy_nonoverlapping提高性能
    fn write_slice(&mut self, addr: u64, buf: &[u8]) -> Result<(), VmError> {
        let region = self.find_region_mut(GuestAddr(addr)).ok_or(VmError::InvalidAddress(GuestAddr(addr)))?;
        let offset = (addr - region.start.0) as usize;
        if offset + buf.len() > region.data.len() {
            return Err(VmError::InvalidAddress(GuestAddr(addr)));
        }
        
        // 使用copy_nonoverlapping实现高效批量写入
        unsafe {
            ptr::copy_nonoverlapping(
                buf.as_ptr(),
                region.data.as_mut_ptr().add(offset),
                buf.len(),
            );
        }
        Ok(())
    }

    /// 可变查找内存区域
    fn find_region_mut(&mut self, addr: GuestAddr) -> Option<&mut MemoryRegion> {
        self.regions.iter_mut().find(|region| addr >= region.start && addr < region.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_read_write_operations() {
        let mut mmu = SoftMmu::new(4096);
        
        // 添加一个4KB内存区域
        let mut data = vec![0u8; 4096];
        for i in 0..4096 {
            data[i] = (i % 256) as u8;
        }
        mmu.add_region(GuestAddr(0x1000), data);
        
        // 测试读取操作
        assert_eq!(mmu.read_u8(0x1000).unwrap(), 0);
        assert_eq!(mmu.read_u16(0x1000).unwrap(), 0x0100);
        assert_eq!(mmu.read_u32(0x1000).unwrap(), 0x03020100);
        assert_eq!(mmu.read_u64(0x1000).unwrap(), 0x0706050403020100);
        
        // 测试写入操作
        mmu.write_u8(0x1000, 0xFF).unwrap();
        assert_eq!(mmu.read_u8(0x1000).unwrap(), 0xFF);
        
        mmu.write_u16(0x1000, 0xBEEF).unwrap();
        assert_eq!(mmu.read_u16(0x1000).unwrap(), 0xBEEF);
        
        mmu.write_u32(0x1000, 0x12345678).unwrap();
        assert_eq!(mmu.read_u32(0x1000).unwrap(), 0x12345678);
        
        mmu.write_u64(0x1000, 0x87654321FEDCBA9).unwrap();
        assert_eq!(mmu.read_u64(0x1000).unwrap(), 0x87654321FEDCBA9);
    }
    
    #[test]
    fn test_bulk_operations() {
        let mut mmu = SoftMmu::new(4096);
        
        // 添加一个8KB内存区域
        let mut data = vec![0u8; 8192];
        for i in 0..8192 {
            data[i] = (i % 256) as u8;
        }
        mmu.add_region(GuestAddr(0x10000), data);
        
        // 测试批量读取
        let mut read_buf = [0u8; 1024];
        mmu.read_slice(0x10000, &mut read_buf).unwrap();
        
        // 验证数据
        for i in 0..1024 {
            assert_eq!(read_buf[i], data[i]);
        }
        
        // 测试批量写入
        let write_buf: Vec<u8> = (0..1024).map(|i| (255 - i) as u8).collect();
        mmu.write_slice(0x10000, &write_buf).unwrap();
        
        // 验证数据
        let mut verify_buf = [0u8; 1024];
        mmu.read_slice(0x10000, &mut verify_buf).unwrap();
        
        for i in 0..1024 {
            assert_eq!(verify_buf[i], (255 - i) as u8);
        }
    }
    
    #[test]
    fn test_cross_region_operations() {
        let mut mmu = SoftMmu::new(4096);
        
        // 添加两个不连续的内存区域
        let mut data1 = vec![1u8; 4096];
        let mut data2 = vec![2u8; 4096];
        mmu.add_region(GuestAddr(0x1000), data1);
        mmu.add_region(GuestAddr(0x10000), data2);
        
        // 测试跨区域访问（应该失败）
        assert!(mmu.read_u8(0x5000).is_err());
        assert!(mmu.write_u8(0x5000, 0x55).is_err());
        
        // 测试批量跨区域访问（应该失败）
        let mut buf = [0u8; 100];
        assert!(mmu.read_slice(0x5000, &mut buf[..50]).is_err());
        assert!(mmu.write_slice(0x5000, &buf[..50]).is_err());
    }
}

