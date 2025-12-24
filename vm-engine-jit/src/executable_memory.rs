cfg_if::cfg_if! {
    if #[cfg(target_os = "linux")] {
        pub struct ExecutableMemory {
            ptr: *mut u8,
            size: usize,
        }

        impl std::fmt::Debug for ExecutableMemory {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("ExecutableMemory")
                    .field("ptr", &self.ptr)
                    .field("size", &self.size)
                    .finish()
            }
        }

        impl ExecutableMemory {
            pub fn new(size: usize) -> Option<Self> {
                if size == 0 {
                    return None;
                }

                unsafe {
                    let ptr = libc::mmap(
                        std::ptr::null_mut(),
                        size,
                        libc::PROT_READ | libc::PROT_WRITE,
                        libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                        -1,
                        0,
                    );

                    if ptr == libc::MAP_FAILED {
                        return None;
                    }

                    Some(ExecutableMemory { ptr: ptr as *mut u8, size })
                }
            }

            pub fn as_mut_slice(&mut self) -> &mut [u8] {
                unsafe { std::slice::from_raw_parts_mut(self.ptr, self.size) }
            }

            pub fn make_executable(&mut self) -> bool {
                unsafe {
                    let result = libc::mprotect(
                        self.ptr as *mut libc::c_void,
                        self.size,
                        libc::PROT_READ | libc::PROT_EXEC,
                    );
                    result == 0
                }
            }

            pub fn make_writable(&mut self) -> bool {
                unsafe {
                    let result = libc::mprotect(
                        self.ptr as *mut libc::c_void,
                        self.size,
                        libc::PROT_READ | libc::PROT_WRITE,
                    );
                    result == 0
                }
            }

            pub fn invalidate_icache(&self) {
                let ptr = self.ptr as usize;
                let size = self.size;
                let cache_line_size = 64;

                #[cfg(target_arch = "x86_64")]
                {
                    use std::arch::asm;
                    unsafe {
                        for offset in (0..size).step_by(cache_line_size) {
                            let addr = (ptr + offset) as *const u8;
                            asm!("clflush ({0})", in(reg) addr, options(nostack));
                        }
                        asm!("mfence", options(nostack));
                    }
                }
                #[cfg(target_arch = "aarch64")]
                {
                    use std::arch::asm;
                    unsafe {
                        let start = ptr & !(cache_line_size - 1);
                        let end = ptr + size;
                        let mut addr = start;
                        while addr < end {
                            asm!("dc cvau, {0}", in(reg) addr, options(nostack));
                            asm!("ic ivau, {0}", in(reg) addr, options(nostack));
                            addr += cache_line_size;
                        }
                        asm!("dsb ish", options(nostack));
                        asm!("isb", options(nostack));
                    }
                }
                #[cfg(target_arch = "riscv64")]
                {
                    use std::arch::asm;
                    unsafe {
                        asm!("fence.i", options(nostack));
                    }
                }
            }

            pub fn fn_ptr(&self) -> extern "C" fn() {
                unsafe { std::mem::transmute(self.ptr) }
            }
        }

        impl Drop for ExecutableMemory {
            fn drop(&mut self) {
                unsafe {
                    libc::munmap(self.ptr as *mut libc::c_void, self.size);
                }
            }
        }

        unsafe impl Send for ExecutableMemory {}
        unsafe impl Sync for ExecutableMemory {}
    } else if #[cfg(target_os = "macos")] {
        pub struct ExecutableMemory {
            ptr: *mut u8,
            size: usize,
        }

        impl std::fmt::Debug for ExecutableMemory {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("ExecutableMemory")
                    .field("ptr", &self.ptr)
                    .field("size", &self.size)
                    .finish()
            }
        }

        impl ExecutableMemory {
            pub fn new(size: usize) -> Option<Self> {
                if size == 0 {
                    return None;
                }

                unsafe {
                    let ptr = libc::mmap(
                        std::ptr::null_mut(),
                        size,
                        libc::PROT_READ | libc::PROT_WRITE,
                        libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_JIT,
                        -1,
                        0,
                    );

                    if ptr == libc::MAP_FAILED {
                        return None;
                    }

                    Some(ExecutableMemory { ptr: ptr as *mut u8, size })
                }
            }

            pub fn as_mut_slice(&mut self) -> &mut [u8] {
                unsafe { std::slice::from_raw_parts_mut(self.ptr, self.size) }
            }

            pub fn make_executable(&mut self) -> bool {
                unsafe {
                    let result = libc::mprotect(
                        self.ptr as *mut libc::c_void,
                        self.size,
                        libc::PROT_READ | libc::PROT_EXEC,
                    );
                    result == 0
                }
            }

            pub fn make_writable(&mut self) -> bool {
                unsafe {
                    let result = libc::mprotect(
                        self.ptr as *mut libc::c_void,
                        self.size,
                        libc::PROT_READ | libc::PROT_WRITE,
                    );
                    result == 0
                }
            }

            pub fn invalidate_icache(&self) {
                let ptr = self.ptr as usize;
                let size = self.size;
                let cache_line_size = 64;

                use std::arch::asm;
                unsafe {
                    let start = ptr & !(cache_line_size - 1);
                    let end = ptr + size;
                    let mut addr = start;
                    while addr < end {
                        asm!("dc cvau, {0}", in(reg) addr, options(nostack));
                        asm!("ic ivau, {0}", in(reg) addr, options(nostack));
                        addr += cache_line_size;
                    }
                    asm!("dsb ish", options(nostack));
                    asm!("isb", options(nostack));
                }
            }

            pub fn fn_ptr(&self) -> extern "C" fn() {
                unsafe { std::mem::transmute(self.ptr) }
            }
        }

        impl Drop for ExecutableMemory {
            fn drop(&mut self) {
                unsafe {
                    libc::munmap(self.ptr as *mut libc::c_void, self.size);
                }
            }
        }

        unsafe impl Send for ExecutableMemory {}
        unsafe impl Sync for ExecutableMemory {}
    } else if #[cfg(target_os = "windows")] {
        use windows_sys::Win32::Foundation::{VirtualFree, MEM_RELEASE};
        use windows_sys::Win32::System::Memory::{
            VirtualAlloc, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, MEM_COMMIT, MEM_RESERVE,
        };

        pub struct ExecutableMemory {
            ptr: *mut u8,
            size: usize,
        }

        impl std::fmt::Debug for ExecutableMemory {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("ExecutableMemory")
                    .field("ptr", &self.ptr)
                    .field("size", &self.size)
                    .finish()
            }
        }

        impl ExecutableMemory {
            pub fn new(size: usize) -> Option<Self> {
                if size == 0 {
                    return None;
                }

                unsafe {
                    let ptr = VirtualAlloc(
                        std::ptr::null_mut(),
                        size,
                        MEM_COMMIT | MEM_RESERVE,
                        PAGE_EXECUTE_READWRITE,
                    );

                    if ptr.is_null() {
                        return None;
                    }

                    Some(ExecutableMemory { ptr: ptr as *mut u8, size })
                }
            }

            pub fn as_mut_slice(&mut self) -> &mut [u8] {
                unsafe { std::slice::from_raw_parts_mut(self.ptr, self.size) }
            }

            pub fn make_executable(&mut self) -> bool {
                true
            }

            pub fn make_writable(&mut self) -> bool {
                true
            }

            pub fn invalidate_icache(&self) {
                #[cfg(target_arch = "x86_64")]
                {
                    use std::arch::asm;
                    unsafe {
                        asm!("sfence", options(nostack, nomem));
                    }
                }
            }

            pub fn fn_ptr(&self) -> extern "C" fn() {
                unsafe { std::mem::transmute(self.ptr) }
            }
        }

        impl Drop for ExecutableMemory {
            fn drop(&mut self) {
                unsafe {
                    VirtualFree(self.ptr as *const _, 0, MEM_RELEASE);
                }
            }
        }

        unsafe impl Send for ExecutableMemory {}
        unsafe impl Sync for ExecutableMemory {}
    } else {
        compile_error!("Unsupported platform for executable memory");
    }
}
