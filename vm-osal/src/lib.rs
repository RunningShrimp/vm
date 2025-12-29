//! vm-osal: 操作系统抽象层
//!
//! 提供跨平台的内存映射、线程亲和性、信号处理等抽象

use std::sync::atomic::{Ordering, fence};

// ============================================================================
// 内存屏障
// ============================================================================

pub fn barrier_acquire() {
    fence(Ordering::Acquire);
}
pub fn barrier_release() {
    fence(Ordering::Release);
}
pub fn barrier_full() {
    fence(Ordering::SeqCst);
}

// ============================================================================
// 平台检测
// ============================================================================

pub fn host_os() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        // HarmonyOS 基于 Linux 内核，可通过系统属性检测
        if is_harmonyos() {
            return "harmonyos";
        }
        return "linux";
    }
    #[cfg(target_os = "macos")]
    {
        return "macos";
    }
    #[cfg(target_os = "windows")]
    {
        return "windows";
    }
    #[cfg(target_os = "android")]
    {
        return "android";
    }
    #[cfg(target_os = "ios")]
    {
        return "ios";
    }
    #[allow(unreachable_code)]
    "unknown"
}

/// 检测是否为 HarmonyOS
#[allow(dead_code)]
fn is_harmonyos() -> bool {
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        // 检查 /etc/os-release 或系统属性
        if let Ok(content) = fs::read_to_string("/etc/os-release") {
            return content.to_lowercase().contains("harmonyos")
                || content.to_lowercase().contains("openharmony");
        }
        false
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

pub fn host_arch() -> &'static str {
    #[cfg(target_arch = "x86_64")]
    {
        return "x86_64";
    }
    #[cfg(target_arch = "aarch64")]
    {
        return "aarch64";
    }
    #[cfg(target_arch = "riscv64")]
    {
        return "riscv64";
    }
    #[allow(unreachable_code)]
    "unknown"
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

/// 内存映射结果
pub type MemoryResult<T> = Result<T, MemoryError>;

/// 内存错误类型
#[derive(Debug)]
pub enum MemoryError {
    AllocationFailed,
    ProtectionFailed,
    InvalidAddress,
    InvalidSize,
    OsError(i32),
}

impl std::fmt::Display for MemoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryError::AllocationFailed => write!(f, "Memory allocation failed"),
            MemoryError::ProtectionFailed => write!(f, "Memory protection change failed"),
            MemoryError::InvalidAddress => write!(f, "Invalid memory address"),
            MemoryError::InvalidSize => write!(f, "Invalid memory size"),
            MemoryError::OsError(code) => write!(f, "OS error: {}", code),
        }
    }
}

impl std::error::Error for MemoryError {}

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
            return Err(MemoryError::AllocationFailed);
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
            return Err(MemoryError::AllocationFailed);
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
            return Err(MemoryError::ProtectionFailed);
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
            return Err(MemoryError::ProtectionFailed);
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
    /// 调用者必须确保：
    /// 1. `self.ptr` 指针在整个生命周期内保持有效
    /// 2. `self.size` 的大小是正确的
    /// 3. 指针指向的内存范围是可读的
    /// 4. 在此切片存在期间，不会调用任何可能改变 `self.ptr` 的方法（如 `make_writable`）
    pub unsafe fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.size) }
    }

    /// 作为可变切片访问
    ///
    /// # Safety
    ///
    /// 调用者必须确保：
    /// 1. `self.ptr` 指针在整个生命周期内保持有效
    /// 2. `self.size` 的大小是正确的
    /// 3. 指针指向的内存范围是可读写的
    /// 4. 内存当前处于可写状态（通过 `make_writable()` 切换）
    /// 5. 在此切片存在期间，不会调用任何可能改变 `self.ptr` 的方法
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

/// JIT 代码内存（支持 W^X）
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
            return Err(MemoryError::InvalidSize);
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
    /// 调用者必须确保：
    /// 1. `offset` 是一个有效的偏移量，不会超出 `self.mem` 的范围
    /// 2. `offset + size_of::<T>()` 不会超出内存区域
    /// 3. 内存当前处于可执行状态（通过 `make_executable()` 切换）
    /// 4. 指向的内存包含有效的类型 `T` 的函数
    /// 5. 返回的函数指针在调用时内存仍然有效且可执行
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

// ============================================================================
// 线程亲和性
// ============================================================================

/// 设置线程亲和性到大核（性能核心）
pub fn set_thread_affinity_big() {
    #[cfg(target_os = "linux")]
    {
        // 在 Linux 上尝试绑定到性能核心
        // 具体实现依赖于硬件拓扑
    }
    #[cfg(target_os = "macos")]
    {
        // macOS 使用 QoS 类
        // pthread_set_qos_class_self_np
    }
}

/// 设置线程亲和性到小核（能效核心）
pub fn set_thread_affinity_little() {
    #[cfg(target_os = "linux")]
    {
        // 绑定到能效核心
    }
    #[cfg(target_os = "macos")]
    {
        // 设置低 QoS
    }
}

/// 设置线程到指定 CPU
#[cfg(target_os = "linux")]
pub fn set_thread_cpu(cpu: usize) -> bool {
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        libc::CPU_ZERO(&mut set);
        libc::CPU_SET(cpu, &mut set);
        libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set) == 0
    }
}

#[cfg(not(target_os = "linux"))]
pub fn set_thread_cpu(_cpu: usize) -> bool {
    false
}

// ============================================================================
// 高精度计时器
// ============================================================================

/// 高精度时间戳
pub fn timestamp_ns() -> u64 {
    #[cfg(unix)]
    {
        let mut ts = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        unsafe {
            libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts);
        }
        (ts.tv_sec as u64) * 1_000_000_000 + (ts.tv_nsec as u64)
    }
    #[cfg(windows)]
    {
        use std::time::Instant;
        static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
        let start = START.get_or_init(Instant::now);
        start.elapsed().as_nanos() as u64
    }
}

/// 测量代码执行时间
pub fn measure<F, R>(f: F) -> (R, u64)
where
    F: FnOnce() -> R,
{
    let start = timestamp_ns();
    let result = f();
    let elapsed = timestamp_ns() - start;
    (result, elapsed)
}

// ============================================================================
// 信号处理（用于 JIT 和 MMU 异常捕获）
// ============================================================================

/// 信号处理器类型
pub type SignalHandler = extern "C" fn(i32);

/// 注册 SIGSEGV 处理器（Unix）
#[cfg(unix)]
pub fn register_sigsegv_handler(handler: SignalHandler) -> bool {
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = handler as usize;
        action.sa_flags = libc::SA_SIGINFO;

        libc::sigaction(libc::SIGSEGV, &action, std::ptr::null_mut()) == 0
    }
}

#[cfg(not(unix))]
pub fn register_sigsegv_handler(_handler: SignalHandler) -> bool {
    false
}

// ============================================================================
// 平台特定集成
// ============================================================================

pub mod platform;

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_detection() {
        let os = host_os();
        let arch = host_arch();
        assert!(!os.is_empty());
        assert!(!arch.is_empty());
        println!("Running on {} / {}", os, arch);
    }

    #[test]
    fn test_memory_allocation() {
        let mem = MappedMemory::allocate(4096, MemoryProtection::READ_WRITE)
            .expect("Failed to allocate memory");
        assert_eq!(mem.size(), 4096);

        unsafe {
            let slice = std::slice::from_raw_parts_mut(mem.ptr, 4096);
            slice[0] = 42;
            assert_eq!(slice[0], 42);
        }
    }

    #[test]
    fn test_jit_memory() {
        let mut jit = JitMemory::allocate(4096).expect("Failed to allocate JIT memory");

        // 写入一些"代码"
        jit.write(0, &[0x90, 0x90, 0x90, 0xC3])
            .expect("Failed to write JIT code"); // NOP NOP NOP RET

        // 切换到可执行
        jit.make_executable()
            .expect("Failed to make memory executable");
    }

    #[test]
    fn test_timestamp() {
        let t1 = timestamp_ns();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let t2 = timestamp_ns();
        assert!(t2 > t1);
    }
}
