//! 线程管理
//!
//! 提供线程亲和性设置和CPU绑定功能
//! 从 vm-osal 模块迁移而来

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
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        // 其他平台暂不支持
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
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        // 其他平台暂不支持
    }
}

/// 设置线程到指定 CPU
#[cfg(target_os = "linux")]
pub fn set_thread_cpu(cpu: usize) -> bool {
    unsafe {
        use libc;
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
