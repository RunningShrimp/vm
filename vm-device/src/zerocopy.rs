//! 零拷贝技术实现
//!
//! 提供高性能的零拷贝数据传输，用于设备 I/O 和内存管理场景。
//!
//! 标识: 服务接口
//!
//! # 核心概念
//! - DirectBuffer: 直接内存视图，避免数据拷贝
//! - ScatterGatherList: 分散聚集列表，支持非连续内存
//! - MemoryMapping: 内存映射对象，支持零拷贝 I/O

/// 零拷贝缓冲区特质
///
/// 提供对内存的直接访问，避免数据拷贝。
///
/// 标识: 服务接口
pub trait ZeroCopyBuffer: Send + Sync {
    /// 获取缓冲区的物理地址
    fn phys_addr(&self) -> u64;

    /// 获取缓冲区的大小
    fn len(&self) -> usize;

    /// 获取底层数据指针（必须安全！）
    ///
    /// # Safety
    ///
    /// 调用者必须保证：
    /// - 指针指向的内存有效且已初始化
    /// - 缓冲区的生命周期内保持有效
    /// - 返回的指针不在缓冲区生命周期外使用
    unsafe fn as_ptr(&self) -> *const u8;

    /// 获取可变底层数据指针（必须安全！）
    ///
    /// # Safety
    ///
    /// 调用者必须保证：
    /// - 指针指向的内存有效且已初始化
    /// - 缓冲区的生命周期内保持有效
    /// - 返回的指针不在缓冲区生命周期外使用
    /// - 不存在通过其他引用的可变别名
    unsafe fn as_mut_ptr(&mut self) -> *mut u8;

    /// 检查缓冲区是否为空
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// 直接内存缓冲区
///
/// 对应一个连续的物理内存段，支持零拷贝访问。
///
/// 标识: 数据模型
pub struct DirectBuffer {
    /// 物理地址
    phys_addr: u64,
    /// 数据指针
    ptr: *mut u8,
    /// 大小
    size: usize,
}

impl DirectBuffer {
    /// 创建直接缓冲区
    ///
    /// # 参数
    /// - `phys_addr`: 物理地址
    /// - `ptr`: 数据指针
    /// - `size`: 大小
    ///
    /// # Safety
    ///
    /// 调用者必须保证：
    /// - ptr 指向有效的内存区域
    /// - ptr 指向的内存至少有 size 字节
    /// - 内存已正确对齐
    /// - 在缓冲区的生命周期内内存保持有效
    /// - 如果内存来自其他来源，确保没有未定义别名
    pub unsafe fn new(phys_addr: u64, ptr: *mut u8, size: usize) -> Self {
        Self {
            phys_addr,
            ptr,
            size,
        }
    }

    /// 创建从切片的直接缓冲区
    pub fn from_slice(phys_addr: u64, data: &mut [u8]) -> Self {
        unsafe { Self::new(phys_addr, data.as_mut_ptr(), data.len()) }
    }

    /// 转换为切片
    pub fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.size) }
    }

    /// 转换为可变切片
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.size) }
    }
}

unsafe impl Send for DirectBuffer {}
unsafe impl Sync for DirectBuffer {}

impl ZeroCopyBuffer for DirectBuffer {
    fn phys_addr(&self) -> u64 {
        self.phys_addr
    }

    fn len(&self) -> usize {
        self.size
    }

    unsafe fn as_ptr(&self) -> *const u8 {
        self.ptr
    }

    unsafe fn as_mut_ptr(&mut self) -> *mut u8 {
        self.ptr
    }
}

/// 分散聚集元素
///
/// 代表一个内存段。
///
/// 标识: 数据模型
pub struct ScatterGatherElement {
    /// 物理地址
    pub phys_addr: u64,
    /// 大小
    pub len: usize,
}

impl ScatterGatherElement {
    /// 创建新的分散聚集元素
    pub fn new(phys_addr: u64, len: usize) -> Self {
        Self { phys_addr, len }
    }
}

/// 分散聚集列表
///
/// 支持非连续内存的零拷贝传输。
///
/// 标识: 数据模型
pub struct ScatterGatherList {
    elements: Vec<ScatterGatherElement>,
    total_len: usize,
}

impl ScatterGatherList {
    /// 创建新的分散聚集列表
    pub fn new() -> Self {
        Self {
            elements: Vec::new(),
            total_len: 0,
        }
    }

    /// 添加一个元素
    pub fn add(&mut self, phys_addr: u64, len: usize) {
        self.elements
            .push(ScatterGatherElement::new(phys_addr, len));
        self.total_len += len;
    }

    /// 获取元素数量
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// 获取总大小
    pub fn total_len(&self) -> usize {
        self.total_len
    }

    /// 获取元素
    pub fn get(&self, index: usize) -> Option<&ScatterGatherElement> {
        self.elements.get(index)
    }

    /// 迭代元素
    pub fn iter(&self) -> impl Iterator<Item = &ScatterGatherElement> {
        self.elements.iter()
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }
}

impl Default for ScatterGatherList {
    fn default() -> Self {
        Self::new()
    }
}

/// 零拷贝 I/O 操作结果
///
/// 标识: 数据模型
pub struct ZeroCopyIoResult {
    /// 传输的字节数
    pub bytes_transferred: usize,
    /// 是否成功
    pub success: bool,
    /// 错误信息（如果失败）
    pub error: Option<String>,
}

impl ZeroCopyIoResult {
    /// 创建成功结果
    pub fn success(bytes_transferred: usize) -> Self {
        Self {
            bytes_transferred,
            success: true,
            error: None,
        }
    }

    /// 创建失败结果
    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            bytes_transferred: 0,
            success: false,
            error: Some(msg.into()),
        }
    }
}

/// 零拷贝 I/O 管理器
///
/// 负责管理零拷贝数据传输。
///
/// 标识: 服务接口
pub trait ZeroCopyIoManager: Send + Sync {
    /// 执行零拷贝读操作
    ///
    /// # 参数
    /// - `sg_list`: 分散聚集列表（目标）
    /// - `offset`: 源偏移
    ///
    /// # 返回
    /// 操作结果
    fn read_sg(&self, sg_list: &ScatterGatherList, offset: u64) -> ZeroCopyIoResult;

    /// 执行零拷贝写操作
    ///
    /// # 参数
    /// - `sg_list`: 分散聚集列表（源）
    /// - `offset`: 目标偏移
    ///
    /// # 返回
    /// 操作结果
    fn write_sg(&mut self, sg_list: &ScatterGatherList, offset: u64) -> ZeroCopyIoResult;

    /// 批量零拷贝操作
    ///
    /// 适用于大数据块传输。
    fn bulk_transfer(&mut self, src_addr: u64, dst_addr: u64, len: usize) -> ZeroCopyIoResult;
}

/// 内存映射对象
///
/// 使用内存映射文件进行零拷贝 I/O。
///
/// 标识: 数据模型
#[cfg(target_os = "linux")]
pub struct MemoryMappedBuffer {
    /// mmap 指针
    ptr: *mut u8,
    /// 大小
    size: usize,
    /// 文件描述符
    fd: i32,
}

#[cfg(target_os = "linux")]
impl MemoryMappedBuffer {
    /// 创建内存映射缓冲区
    ///
    /// # 参数
    /// - `fd`: 文件描述符
    /// - `offset`: 文件偏移
    /// - `size`: 映射大小
    pub unsafe fn new(fd: i32, offset: u64, size: usize) -> Result<Self, String> {
        use libc::{MAP_SHARED, PROT_READ, PROT_WRITE, mmap};

        let ptr = mmap(
            std::ptr::null_mut(),
            size,
            PROT_READ | PROT_WRITE,
            MAP_SHARED,
            fd,
            offset as libc::off_t,
        );

        if ptr == libc::MAP_FAILED {
            return Err("Failed to mmap".to_string());
        }

        Ok(Self {
            ptr: ptr as *mut u8,
            size,
            fd,
        })
    }

    /// 获取指针
    pub fn ptr(&self) -> *mut u8 {
        self.ptr
    }

    /// 获取大小
    pub fn size(&self) -> usize {
        self.size
    }

    /// 同步映射内存到磁盘
    pub fn sync(&self) -> Result<(), String> {
        unsafe {
            if libc::msync(self.ptr as *mut libc::c_void, self.size, libc::MS_SYNC) != 0 {
                return Err("Failed to msync".to_string());
            }
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
impl Drop for MemoryMappedBuffer {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.ptr as *mut libc::c_void, self.size);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direct_buffer() {
        let mut data = vec![1, 2, 3, 4, 5];
        let buffer = DirectBuffer::from_slice(0x1000, &mut data);

        assert_eq!(buffer.phys_addr(), 0x1000);
        assert_eq!(buffer.len(), 5);
        assert_eq!(buffer.as_slice(), &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_scatter_gather_list() {
        let mut sg = ScatterGatherList::new();
        sg.add(0x1000, 0x1000);
        sg.add(0x3000, 0x1000);
        sg.add(0x5000, 0x1000);

        assert_eq!(sg.len(), 3);
        assert_eq!(sg.total_len(), 0x3000);
        assert!(!sg.is_empty());

        let first = sg
            .get(0)
            .expect("Failed to get first scatter-gather element");
        assert_eq!(first.phys_addr, 0x1000);
        assert_eq!(first.len, 0x1000);
    }

    #[test]
    fn test_io_result() {
        let success = ZeroCopyIoResult::success(512);
        assert!(success.success);
        assert_eq!(success.bytes_transferred, 512);
        assert!(success.error.is_none());

        let error = ZeroCopyIoResult::error("Test error");
        assert!(!error.success);
        assert!(error.error.is_some());
    }
}
