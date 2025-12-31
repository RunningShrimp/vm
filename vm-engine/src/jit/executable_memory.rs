//! 可执行内存管理
//!
//! 提供跨平台的可执行内存分配和管理功能

/// 可执行内存管理器
pub struct ExecutableMemory {
    data: Vec<u8>,
    ptr: *mut u8,
    size: usize,
}

unsafe impl Send for ExecutableMemory {}
unsafe impl Sync for ExecutableMemory {}

impl ExecutableMemory {
    /// 创建新的可执行内存
    pub fn new(size: usize) -> Self {
        let mut data = vec![0u8; size];
        let ptr = data.as_mut_ptr();

        Self { data, ptr, size }
    }

    /// 获取可变切片
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// 获取只读切片
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    /// 设置内存为可执行
    ///
    /// 在Unix系统上使用mprotect设置PROT_READ | PROT_EXEC
    /// 在Windows上使用VirtualProtect设置PAGE_EXECUTE_READ
    pub fn make_executable(&mut self) -> bool {
        #[cfg(unix)]
        unsafe {
            use libc::{PROT_EXEC, PROT_READ, c_void, mprotect};

            let page_size = libc::sysconf(libc::_SC_PAGESIZE) as usize;
            let aligned_addr = (self.ptr as usize) & !(page_size - 1);
            let aligned_size = self.size.div_ceil(page_size) * page_size;

            let result = mprotect(
                aligned_addr as *mut c_void,
                aligned_size,
                PROT_READ | PROT_EXEC,
            );

            result == 0
        }

        #[cfg(windows)]
        unsafe {
            use winapi::um::memoryapi::VirtualProtect;
            use winapi::um::winnt::{PAGE_EXECUTE_READ, PAGE_READWRITE};

            let mut old_protect = PAGE_READWRITE;
            let result = VirtualProtect(
                self.ptr as *mut _,
                self.size,
                PAGE_EXECUTE_READ,
                &mut old_protect,
            );

            result != 0
        }

        #[cfg(not(any(unix, windows)))]
        {
            // 对于其他平台，假设已经可执行
            true
        }
    }

    /// 设置内存为可写
    ///
    /// 在写入代码之前需要调用此方法
    pub fn make_writable(&mut self) -> bool {
        #[cfg(unix)]
        unsafe {
            use libc::{PROT_READ, PROT_WRITE, c_void, mprotect};

            let page_size = libc::sysconf(libc::_SC_PAGESIZE) as usize;
            let aligned_addr = (self.ptr as usize) & !(page_size - 1);
            let aligned_size = self.size.div_ceil(page_size) * page_size;

            let result = mprotect(
                aligned_addr as *mut c_void,
                aligned_size,
                PROT_READ | PROT_WRITE,
            );

            result == 0
        }

        #[cfg(windows)]
        unsafe {
            use winapi::um::memoryapi::VirtualProtect;
            use winapi::um::winnt::{PAGE_EXECUTE_READWRITE, PAGE_READWRITE};

            let mut old_protect = PAGE_READWRITE;
            let result = VirtualProtect(
                self.ptr as *mut _,
                self.size,
                PAGE_EXECUTE_READWRITE,
                &mut old_protect,
            );

            result != 0
        }

        #[cfg(not(any(unix, windows)))]
        {
            true
        }
    }

    /// 使指令缓存失效
    ///
    /// 在修改代码后需要调用此方法以确保CPU使用新代码
    pub fn invalidate_icache(&mut self) {
        #[cfg(target_arch = "x86_64")]
        {
            // x86_64不需要手动刷新指令缓存
        }

        #[cfg(target_arch = "aarch64")]
        unsafe {
            use std::arch::asm;

            // ARM64需要手动刷新指令缓存
            let addr = self.ptr as usize;
            let size = self.size;

            // 对齐到缓存行大小
            let cache_line_size = 64;
            let start_addr = addr & !(cache_line_size - 1);
            let end_addr = ((addr + size + cache_line_size - 1) & !(cache_line_size - 1)) as u64;

            let mut current = start_addr as u64;
            while current < end_addr {
                asm!(
                    "dc cvau, {0}",
                    "ic ivau, {0}",
                    in(reg) current,
                    options(nostack)
                );
                current += cache_line_size as u64;
            }

            // 数据同步屏障
            asm!("dsb ish", options(nostack));
            asm!("isb", options(nostack));
        }

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            // 其他架构：使用编译器内置函数
            unsafe {
                let addr = self.ptr as usize;
                let size = self.size;

                // 使用std::arch::asm的内置刷新
                #[cfg(feature = "compiler_builtins")]
                {
                    compiler_builtins::clear_cache(self.ptr, self.ptr.add(size));
                }
            }
        }
    }

    /// 获取内存指针
    pub fn as_ptr(&self) -> *const u8 {
        self.ptr
    }

    /// 获取可变内存指针
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.ptr
    }

    /// 获取内存大小
    pub fn size(&self) -> usize {
        self.size
    }

    /// 从数据创建可执行内存
    pub fn from_bytes(data: &[u8]) -> Self {
        let size = data.len();
        let mut exec_mem = Self::new(size);
        exec_mem.as_mut_slice().copy_from_slice(data);
        exec_mem
    }
}

impl Drop for ExecutableMemory {
    fn drop(&mut self) {
        // 在释放前恢复为可读写，这样操作系统才能正确释放内存
        let _ = self.make_writable();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executable_memory_creation() {
        let mem = ExecutableMemory::new(4096);
        assert_eq!(mem.size(), 4096);
    }

    #[test]
    fn test_executable_memory_write() {
        let mut mem = ExecutableMemory::new(100);
        let slice = mem.as_mut_slice();
        slice[0] = 0x90; // NOP on x86
        slice[1] = 0xC3; // RET on x86

        assert_eq!(mem.as_slice()[0], 0x90);
        assert_eq!(mem.as_slice()[1], 0xC3);
    }

    #[test]
    fn test_from_bytes() {
        let data = vec![0x90, 0xC3, 0x00, 0x00];
        let mem = ExecutableMemory::from_bytes(&data);
        assert_eq!(mem.size(), 4);
        assert_eq!(mem.as_slice(), &data[..]);
    }
}
