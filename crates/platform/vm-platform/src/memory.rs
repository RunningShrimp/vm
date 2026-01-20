//! 内存管理
//!
//! 提供跨平台的内存映射、内存保护和管理功能
//! 从 vm-osal 模块迁移而来
//!
//! This module now uses the unified error handling framework from vm-core.

use std::sync::atomic::{Ordering, fence};

use vm_core::{MemoryError as VmMemoryError, VmError};

// ============================================================================
// 内存屏障
// ============================================================================

/// 内存屏障:获取语义
pub fn barrier_acquire() {
    fence(Ordering::Acquire);
}

/// 内存屏障:释放语义
pub fn barrier_release() {
    fence(Ordering::Release);
}

/// 内存屏障:完全屏障
pub fn barrier_full() {
    fence(Ordering::SeqCst);
}

// ============================================================================
// 内存权限
// ============================================================================

/// 内存保护标志
#[derive(Debug, Clone, Copy)]
pub struct MemoryProtection {
    pub read: bool,
    pub write: bool,
    pub exec: bool,
}

impl MemoryProtection {
    pub const NONE: Self = Self {
        read: false,
        write: false,
        exec: false,
    };
    pub const READ: Self = Self {
        read: true,
        write: false,
        exec: false,
    };
    pub const READ_WRITE: Self = Self {
        read: true,
        write: true,
        exec: false,
    };
    pub const READ_EXEC: Self = Self {
        read: true,
        write: false,
        exec: true,
    };
    pub const READ_WRITE_EXEC: Self = Self {
        read: true,
        write: true,
        exec: true,
    };
}

// ============================================================================
// 内存映射抽象
// ============================================================================

/// 内存映射结果 - using unified VmError
pub type MemoryResult<T> = Result<T, VmError>;

/// 跨平台内存映射结构
pub struct MappedMemory {
    ptr: *mut u8,
    size: usize,
}

unsafe impl Send for MappedMemory {}
unsafe impl Sync for MappedMemory {}

impl MappedMemory {
    /// 分配匿名内存映射
    pub fn allocate(size: usize, prot: MemoryProtection) -> MemoryResult<Self> {
        #[cfg(unix)]
        {
            Self::allocate_unix(size, prot)
        }
        #[cfg(windows)]
        {
            Self::allocate_windows(size, prot)
        }
    }

    #[cfg(unix)]
    fn allocate_unix(size: usize, prot: MemoryProtection) -> MemoryResult<Self> {
        use std::ptr;

        let mut flags = libc::PROT_NONE;
        if prot.read {
            flags |= libc::PROT_READ;
        }
        if prot.write {
            flags |= libc::PROT_WRITE;
        }
        if prot.exec {
            flags |= libc::PROT_EXEC;
        }

        let ptr = unsafe {
            libc::mmap(
                ptr::null_mut(),
                size,
                flags,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };

        if ptr == libc::MAP_FAILED {
            return Err(VmMemoryError::AllocationFailed {
                message: "mmap failed".to_string(),
                size: Some(size),
            }
            .into());
        }

        Ok(Self {
            ptr: ptr as *mut u8,
            size,
        })
    }

    #[cfg(windows)]
    fn allocate_windows(size: usize, prot: MemoryProtection) -> MemoryResult<Self> {
        use std::ptr;

        let protect = match (prot.read, prot.write, prot.exec) {
            (false, false, false) => 0x01, // PAGE_NOACCESS
            (true, false, false) => 0x02,  // PAGE_READONLY
            (true, true, false) => 0x04,   // PAGE_READWRITE
            (true, false, true) => 0x20,   // PAGE_EXECUTE_READ
            (true, true, true) => 0x40,    // PAGE_EXECUTE_READWRITE
            _ => 0x04,                     // Default to PAGE_READWRITE
        };

        extern "system" {
            fn VirtualAlloc(
                lpAddress: *mut std::ffi::c_void,
                dwSize: usize,
                flAllocationType: u32,
                flProtect: u32,
            ) -> *mut std::ffi::c_void;
        }

        let ptr = unsafe {
            VirtualAlloc(
                ptr::null_mut(),
                size,
                0x1000 | 0x2000, // MEM_COMMIT | MEM_RESERVE
                protect,
            )
        };

        if ptr.is_null() {
            return Err(VmMemoryError::AllocationFailed {
                message: "VirtualAlloc failed".to_string(),
                size: Some(size),
            }
            .into());
        }

        Ok(Self {
            ptr: ptr as *mut u8,
            size,
        })
    }

    /// 更改内存保护
    pub fn protect(&self, prot: MemoryProtection) -> MemoryResult<()> {
        #[cfg(unix)]
        {
            self.protect_unix(prot)
        }
        #[cfg(windows)]
        {
            self.protect_windows(prot)
        }
    }

    #[cfg(unix)]
    fn protect_unix(&self, prot: MemoryProtection) -> MemoryResult<()> {
        let mut flags = libc::PROT_NONE;
        if prot.read {
            flags |= libc::PROT_READ;
        }
        if prot.write {
            flags |= libc::PROT_WRITE;
        }
        if prot.exec {
            flags |= libc::PROT_EXEC;
        }

        let ret = unsafe { libc::mprotect(self.ptr as *mut _, self.size, flags) };
        if ret != 0 {
            return Err(VmMemoryError::ProtectionFailed {
                message: "mprotect failed".to_string(),
                addr: None,
            }
            .into());
        }
        Ok(())
    }

    #[cfg(windows)]
    fn protect_windows(&self, prot: MemoryProtection) -> MemoryResult<()> {
        let protect = match (prot.read, prot.write, prot.exec) {
            (false, false, false) => 0x01,
            (true, false, false) => 0x02,
            (true, true, false) => 0x04,
            (true, false, true) => 0x20,
            (true, true, true) => 0x40,
            _ => 0x04,
        };

        extern "system" {
            fn VirtualProtect(
                lpAddress: *mut std::ffi::c_void,
                dwSize: usize,
                flNewProtect: u32,
                lpflOldProtect: *mut u32,
            ) -> i32;
        }

        let mut old_protect: u32 = 0;
        let ret =
            unsafe { VirtualProtect(self.ptr as *mut _, self.size, protect, &mut old_protect) };

        if ret == 0 {
            return Err(VmMemoryError::ProtectionFailed {
                message: "VirtualProtect failed".to_string(),
                addr: None,
            }
            .into());
        }
        Ok(())
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

    /// 作为切片访问
    ///
    /// # Safety
    ///
    /// 调用者必须确保:
    /// 1. `self.ptr` 指针在整个生命周期内保持有效
    /// 2. `self.size` 的大小是正确的
    /// 3. 指针指向的内存范围是可读的
    /// 4. 在此切片存在期间,不会调用任何可能改变 `self.ptr` 的方法(如 `protect`)
    pub unsafe fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.size) }
    }

    /// 作为可变切片访问
    ///
    /// # Safety
    ///
    /// 调用者必须确保:
    /// 1. `self.ptr` 指针在整个生命周期内保持有效
    /// 2. `self.size` 的大小是正确的
    /// 3. 指针指向的内存范围是可读写的
    /// 4. 内存当前处于可写状态(通过 `protect` 方法已设置适当的权限)
    /// 5. 在此切片存在期间,不会调用任何可能改变 `self.ptr` 的方法
    /// 6. 没有其他可变引用指向此内存
    pub unsafe fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.size) }
    }
}

impl Drop for MappedMemory {
    fn drop(&mut self) {
        #[cfg(unix)]
        unsafe {
            libc::munmap(self.ptr as *mut _, self.size);
        }

        #[cfg(windows)]
        unsafe {
            extern "system" {
                fn VirtualFree(
                    lpAddress: *mut std::ffi::c_void,
                    dwSize: usize,
                    dwFreeType: u32,
                ) -> i32;
            }
            VirtualFree(self.ptr as *mut _, 0, 0x8000); // MEM_RELEASE
        }
    }
}

// ============================================================================
// JIT 代码内存
// ============================================================================

/// JIT 代码内存(支持 W^X)
pub struct JitMemory {
    mem: MappedMemory,
    is_executable: bool,
}

impl JitMemory {
    /// 分配 JIT 代码内存
    pub fn allocate(size: usize) -> MemoryResult<Self> {
        // 初始分配为可读写
        let mem = MappedMemory::allocate(size, MemoryProtection::READ_WRITE)?;
        Ok(Self {
            mem,
            is_executable: false,
        })
    }

    /// 切换到写模式
    pub fn make_writable(&mut self) -> MemoryResult<()> {
        if self.is_executable {
            self.mem.protect(MemoryProtection::READ_WRITE)?;
            self.is_executable = false;
        }
        Ok(())
    }

    /// 切换到执行模式
    pub fn make_executable(&mut self) -> MemoryResult<()> {
        if !self.is_executable {
            self.mem.protect(MemoryProtection::READ_EXEC)?;
            self.is_executable = true;
        }
        Ok(())
    }

    /// 写入代码
    pub fn write(&mut self, offset: usize, data: &[u8]) -> MemoryResult<()> {
        self.make_writable()?;

        if offset + data.len() > self.mem.size() {
            return Err(VmMemoryError::AccessViolation {
                addr: vm_core::GuestAddr(offset as u64),
                msg: format!(
                    "Write exceeds memory size: offset={}, len={}, size={}",
                    offset,
                    data.len(),
                    self.mem.size()
                ),
                access_type: None,
            }
            .into());
        }

        unsafe {
            let dst = self.mem.as_mut_ptr().add(offset);
            std::ptr::copy_nonoverlapping(data.as_ptr(), dst, data.len());
        }

        Ok(())
    }

    /// 获取函数指针
    ///
    /// # Safety
    ///
    /// 调用者必须确保:
    /// 1. `offset` 指向的位置已写入有效的机器代码
    /// 2. 代码大小至少为 `std::mem::size_of::<T>()` 字节
    /// 3. 目标代码与类型 `T` 兼容
    /// 4. 调用返回的函数指针前,必须先调用 `make_executable()` 确保代码区域可执行
    /// 5. 在函数指针使用期间,JitMemory 必须保持有效,不会被释放或修改
    pub unsafe fn get_fn<T>(&self, offset: usize) -> T {
        unsafe {
            let ptr = self.mem.as_ptr().add(offset);
            std::mem::transmute_copy(&ptr)
        }
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.mem.as_ptr()
    }

    pub fn size(&self) -> usize {
        self.mem.size()
    }
}
