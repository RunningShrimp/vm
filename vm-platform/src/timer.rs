//! 高精度计时器
//!
//! 提供纳秒级时间戳和代码执行时间测量
//! 从 vm-osal 模块迁移而来

/// 高精度时间戳（纳秒）
pub fn timestamp_ns() -> u64 {
    #[cfg(unix)]
    {
        use libc;
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

/// 测量代码执行时间（纳秒）
pub fn measure<F, R>(f: F) -> (R, u64)
where
    F: FnOnce() -> R,
{
    let start = timestamp_ns();
    let result = f();
    let elapsed = timestamp_ns() - start;
    (result, elapsed)
}
