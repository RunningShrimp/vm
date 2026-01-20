//! 信号处理
//!
//! 提供SIGSEGV处理器注册和异常信号捕获
//! 从 vm-osal 模块迁移而来

/// 信号处理器类型
pub type SignalHandler = extern "C" fn(i32);

/// 注册 SIGSEGV 处理器（Unix）
#[cfg(unix)]
pub fn register_sigsegv_handler(handler: SignalHandler) -> bool {
    unsafe {
        use libc;
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
