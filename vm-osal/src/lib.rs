use std::sync::atomic::{fence, Ordering};

pub struct OSMemory;

impl OSMemory {
    pub fn new() -> Self { Self }
}

pub fn barrier_acquire() { fence(Ordering::Acquire); }
pub fn barrier_release() { fence(Ordering::Release); }
pub fn barrier_full() { fence(Ordering::SeqCst); }

pub fn host_os() -> &'static str {
    #[cfg(target_os = "linux")]
    { return "linux"; }
    #[cfg(target_os = "macos")]
    { return "macos"; }
    #[cfg(target_os = "windows")]
    { return "windows"; }
    "unknown"
}

pub fn host_arch() -> &'static str {
    #[cfg(target_arch = "x86_64")]
    { return "x86_64"; }
    #[cfg(target_arch = "aarch64")]
    { return "aarch64"; }
    #[cfg(target_arch = "riscv64")]
    { return "riscv64"; }
    "unknown"
}

pub fn set_thread_affinity_big() { let _ = 0; }
pub fn set_thread_affinity_little() { let _ = 0; }
