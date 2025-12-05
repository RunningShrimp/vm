//! mmap 基于的设备 I/O 模块
//!
//! 实现通过内存映射进行的零复制设备 I/O，支持：
//! - 文件 mmap 映射
//! - 设备内存映射
//! - 无复制数据访问
//! - 性能优化

use libc;
use std::fs::File;
use std::os::unix::io::AsRawFd;
use std::sync::{Arc, Mutex};

/// mmap I/O 错误
#[derive(Debug, Clone)]
pub enum MmapError {
    /// 内存映射失败
    MmapFailed(String),
    /// 访问越界
    OutOfBounds,
    /// 文件不存在
    FileNotFound,
    /// 权限不足
    PermissionDenied,
    /// 同步失败
    SyncFailed,
}

/// mmap 区域元数据
#[derive(Debug, Clone)]
pub struct MmapRegion {
    /// 映射地址
    pub base_addr: *const u8,
    /// 映射大小
    pub size: usize,
    /// 是否可写
    pub writable: bool,
    /// 是否已初始化
    pub initialized: bool,
}

unsafe impl Send for MmapRegion {}
unsafe impl Sync for MmapRegion {}

/// mmap 设备 I/O 管理器
pub struct MmapDeviceIo {
    /// 已映射的区域
    regions: Arc<Mutex<Vec<MmapRegion>>>,
    /// 页大小
    page_size: usize,
}

impl MmapDeviceIo {
    /// 创建新的 mmap 设备 I/O 管理器
    pub fn new() -> Self {
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize };

        Self {
            regions: Arc::new(Mutex::new(Vec::new())),
            page_size,
        }
    }

    /// 映射文件到内存
    pub fn mmap_file(
        &self,
        file: &File,
        offset: u64,
        size: usize,
        writable: bool,
    ) -> Result<MmapRegion, MmapError> {
        if size == 0 {
            return Err(MmapError::OutOfBounds);
        }

        let prot = if writable {
            libc::PROT_READ | libc::PROT_WRITE
        } else {
            libc::PROT_READ
        };

        let flags = libc::MAP_SHARED;
        let fd = file.as_raw_fd();

        let addr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                size,
                prot,
                flags,
                fd,
                offset as libc::off_t,
            )
        };

        if addr == libc::MAP_FAILED {
            return Err(MmapError::MmapFailed("mmap failed".into()));
        }

        let region = MmapRegion {
            base_addr: addr as *const u8,
            size,
            writable,
            initialized: true,
        };

        let mut regions = self
            .regions
            .lock()
            .map_err(|_| MmapError::MmapFailed("Lock failed".into()))?;
        regions.push(region.clone());

        Ok(region)
    }

    /// 从映射区域读取数据
    pub fn read(
        &self,
        region: &MmapRegion,
        offset: usize,
        len: usize,
    ) -> Result<Vec<u8>, MmapError> {
        if offset + len > region.size {
            return Err(MmapError::OutOfBounds);
        }

        if !region.initialized {
            return Err(MmapError::MmapFailed("Region not initialized".into()));
        }

        unsafe {
            let src = region.base_addr.add(offset);
            let slice = std::slice::from_raw_parts(src, len);
            Ok(slice.to_vec())
        }
    }

    /// 向映射区域写入数据
    pub fn write(&self, region: &MmapRegion, offset: usize, data: &[u8]) -> Result<(), MmapError> {
        if !region.writable {
            return Err(MmapError::PermissionDenied);
        }

        if offset + data.len() > region.size {
            return Err(MmapError::OutOfBounds);
        }

        if !region.initialized {
            return Err(MmapError::MmapFailed("Region not initialized".into()));
        }

        unsafe {
            let dst = region.base_addr.add(offset) as *mut u8;
            std::ptr::copy_nonoverlapping(data.as_ptr(), dst, data.len());
        }

        Ok(())
    }

    /// 获取映射区域的直接切片（零复制访问）
    pub fn get_slice<'a>(
        &self,
        region: &'a MmapRegion,
        offset: usize,
        len: usize,
    ) -> Result<&'a [u8], MmapError> {
        if offset + len > region.size {
            return Err(MmapError::OutOfBounds);
        }

        if !region.initialized {
            return Err(MmapError::MmapFailed("Region not initialized".into()));
        }

        unsafe {
            let src = region.base_addr.add(offset);
            Ok(std::slice::from_raw_parts(src, len))
        }
    }

    /// 获取映射区域的可变直接切片（零复制写入）
    pub fn get_slice_mut<'a>(
        &self,
        region: &'a MmapRegion,
        offset: usize,
        len: usize,
    ) -> Result<&'a mut [u8], MmapError> {
        if !region.writable {
            return Err(MmapError::PermissionDenied);
        }

        if offset + len > region.size {
            return Err(MmapError::OutOfBounds);
        }

        if !region.initialized {
            return Err(MmapError::MmapFailed("Region not initialized".into()));
        }

        unsafe {
            let src = region.base_addr.add(offset) as *mut u8;
            Ok(std::slice::from_raw_parts_mut(src, len))
        }
    }

    /// 同步映射区域的缓存
    pub fn msync(&self, region: &MmapRegion) -> Result<(), MmapError> {
        if !region.initialized {
            return Err(MmapError::MmapFailed("Region not initialized".into()));
        }

        let result = unsafe {
            libc::msync(
                region.base_addr as *mut libc::c_void,
                region.size,
                libc::MS_SYNC,
            )
        };

        if result == 0 {
            Ok(())
        } else {
            Err(MmapError::SyncFailed)
        }
    }

    /// 异步同步（使用 MS_ASYNC）
    pub fn msync_async(&self, region: &MmapRegion) -> Result<(), MmapError> {
        if !region.initialized {
            return Err(MmapError::MmapFailed("Region not initialized".into()));
        }

        let result = unsafe {
            libc::msync(
                region.base_addr as *mut libc::c_void,
                region.size,
                libc::MS_ASYNC,
            )
        };

        if result == 0 {
            Ok(())
        } else {
            Err(MmapError::SyncFailed)
        }
    }

    /// 取消映射
    pub fn munmap(&self, region: &MmapRegion) -> Result<(), MmapError> {
        if !region.initialized {
            return Err(MmapError::MmapFailed("Region not initialized".into()));
        }

        let result = unsafe { libc::munmap(region.base_addr as *mut libc::c_void, region.size) };

        if result == 0 {
            let mut regions = self
                .regions
                .lock()
                .map_err(|_| MmapError::MmapFailed("Lock failed".into()))?;
            regions.retain(|r| r.base_addr != region.base_addr);
            Ok(())
        } else {
            Err(MmapError::MmapFailed("munmap failed".into()))
        }
    }

    /// 预加载数据到内存
    pub fn madvise(&self, region: &MmapRegion, advice: MadviseAdvice) -> Result<(), MmapError> {
        if !region.initialized {
            return Err(MmapError::MmapFailed("Region not initialized".into()));
        }

        let advice_flag = match advice {
            MadviseAdvice::Normal => libc::MADV_NORMAL,
            MadviseAdvice::Random => libc::MADV_RANDOM,
            MadviseAdvice::Sequential => libc::MADV_SEQUENTIAL,
            MadviseAdvice::WillNeed => libc::MADV_WILLNEED,
            MadviseAdvice::DontNeed => libc::MADV_DONTNEED,
        };

        let result = unsafe {
            libc::madvise(
                region.base_addr as *mut libc::c_void,
                region.size,
                advice_flag,
            )
        };

        if result == 0 {
            Ok(())
        } else {
            Err(MmapError::SyncFailed)
        }
    }

    /// 获取页大小
    pub fn page_size(&self) -> usize {
        self.page_size
    }

    /// 获取所有已映射区域
    pub fn get_regions(&self) -> Result<Vec<MmapRegion>, MmapError> {
        let regions = self
            .regions
            .lock()
            .map_err(|_| MmapError::MmapFailed("Lock failed".into()))?;
        Ok(regions.clone())
    }

    /// 清除所有映射
    pub fn clear_all(&self) -> Result<(), MmapError> {
        let mut regions = self
            .regions
            .lock()
            .map_err(|_| MmapError::MmapFailed("Lock failed".into()))?;

        for region in regions.drain(..) {
            if region.initialized {
                let _result =
                    unsafe { libc::munmap(region.base_addr as *mut libc::c_void, region.size) };
            }
        }

        Ok(())
    }
}

/// madvise 建议类型
#[derive(Debug, Clone, Copy)]
pub enum MadviseAdvice {
    /// 默认行为
    Normal,
    /// 随机访问
    Random,
    /// 顺序访问
    Sequential,
    /// 即将访问
    WillNeed,
    /// 不再需要
    DontNeed,
}

impl Drop for MmapDeviceIo {
    fn drop(&mut self) {
        let _ = self.clear_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mmap_device_io_creation() {
        let mmap_io = MmapDeviceIo::new();
        assert!(mmap_io.page_size() > 0);
    }

    #[test]
    #[ignore] // 需要 tempfile 依赖
    fn test_mmap_file() {
        // 使用临时文件测试
    }

    #[test]
    #[ignore] // 需要 tempfile 依赖
    fn test_read_from_mmap() {
        // 使用临时文件测试
    }

    #[test]
    #[ignore] // 需要 tempfile 依赖
    fn test_out_of_bounds() {
        // 使用临时文件测试
    }
}
