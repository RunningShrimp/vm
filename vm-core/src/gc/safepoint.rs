//! GC 安全点管理
//!
//! 提供 GC 安全点（Safepoint）功能，允许在程序执行过程中安全地触发垃圾回收。
//!
//! # 安全点类型
//!
//! - **局部安全点**: 在循环回边、方法调用等位置检查
//! - **全局安全点**: 在内存分配等关键位置检查
//! - **强制安全点**: 立即请求 GC 执行
//!
//! # 使用示例
//! ```rust,ignore
//! use vm_core::gc::safepoint::{Safepoint, SafepointManager};
//!
//! let manager = SafepointManager::new();
//!
//! // 在关键位置检查安全点
//! if manager.should_poll() {
//!     manager.poll_safepoint();
//! }
//! ```

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use super::error::GCError;

/// GC 安全点状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafepointState {
    /// 运行中（mutator 正在执行）
    Running,
    /// 在安全点暂停（等待 GC 完成）
    AtSafepoint,
}

/// GC 安全点管理器
///
/// 负责协调 mutator 线程和 GC 线程之间的同步。
#[derive(Clone)]
pub struct SafepointManager {
    /// 是否正在请求 GC
    gc_requested: Arc<AtomicBool>,
    /// 是否正在执行 GC
    gc_in_progress: Arc<AtomicBool>,
    /// 在安全点的线程数
    threads_at_safepoint: Arc<AtomicUsize>,
    /// 总线程数
    total_threads: usize,
    /// 上次检查时间（用于限流）
    last_check: Arc<Mutex<Instant>>,
    /// 检查间隔（避免过于频繁的检查）
    check_interval: Duration,
    /// 统计信息
    stats: Arc<SafepointStats>,
}

/// GC 安全点统计信息
#[derive(Debug, Default)]
pub struct SafepointStats {
    /// 安全点检查次数
    check_count: AtomicU64,
    /// 实际触发 GC 的次数
    gc_triggered: AtomicU64,
    /// 等待 GC 完成的次数
    wait_count: AtomicU64,
    /// 总等待时间（微秒）
    total_wait_us: AtomicU64,
}

impl SafepointStats {
    /// 获取检查次数
    pub fn check_count(&self) -> u64 {
        self.check_count.load(Ordering::Relaxed)
    }

    /// 获取触发 GC 的次数
    pub fn gc_triggered(&self) -> u64 {
        self.gc_triggered.load(Ordering::Relaxed)
    }

    /// 获取等待 GC 完成的次数
    pub fn wait_count(&self) -> u64 {
        self.wait_count.load(Ordering::Relaxed)
    }

    /// 获取平均等待时间（微秒）
    pub fn avg_wait_us(&self) -> f64 {
        let count = self.wait_count.load(Ordering::Relaxed);
        if count == 0 {
            return 0.0;
        }
        let total_us = self.total_wait_us.load(Ordering::Relaxed);
        total_us as f64 / count as f64
    }
}

impl SafepointManager {
    /// 创建新的安全点管理器
    ///
    /// # 参数
    /// - `total_threads`: mutator 线程数
    /// - `check_interval`: 检查间隔（避免过于频繁）
    pub fn new(total_threads: usize, check_interval: Duration) -> Self {
        Self {
            gc_requested: Arc::new(AtomicBool::new(false)),
            gc_in_progress: Arc::new(AtomicBool::new(false)),
            threads_at_safepoint: Arc::new(AtomicUsize::new(0)),
            total_threads,
            last_check: Arc::new(Mutex::new(Instant::now())),
            check_interval,
            stats: Arc::new(SafepointStats::default()),
        }
    }

    /// 使用默认配置创建安全点管理器
    pub fn with_defaults() -> Self {
        Self::new(
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4),
            Duration::from_micros(100), // 每 100 微秒检查一次
        )
    }

    /// 检查是否应该进行安全点轮询
    ///
    /// 返回 true 表示应该调用 `poll_safepoint()`
    #[inline]
    pub fn should_poll(&self) -> bool {
        // 检查时间间隔限制（避免过于频繁的检查）
        let now = Instant::now();
        let last = *self.last_check.lock();

        if now.duration_since(last) < self.check_interval {
            return false;
        }

        // 更新检查计数（即使没有 GC 请求）
        self.stats.check_count.fetch_add(1, Ordering::Relaxed);

        // 检查是否有 GC 请求
        self.gc_requested.load(Ordering::Relaxed)
    }

    /// 在安全点轮询
    ///
    /// 如果有 GC 请求，暂停当前线程直到 GC 完成
    pub fn poll_safepoint(&self) {
        // 更新上次检查时间
        *self.last_check.lock() = Instant::now();

        // 检查是否有 GC 请求
        if !self.gc_requested.load(Ordering::Relaxed) {
            return;
        }

        // 标记当前线程到达安全点
        let count = self.threads_at_safepoint.fetch_add(1, Ordering::Acquire);

        // 如果所有线程都到达安全点，触发 GC
        if count + 1 >= self.total_threads {
            self.gc_in_progress.store(true, Ordering::Release);
            self.stats.gc_triggered.fetch_add(1, Ordering::Relaxed);
        }

        // 等待 GC 完成
        let start = Instant::now();
        while self.gc_in_progress.load(Ordering::Acquire) {
            std::hint::spin_loop();
        }
        let wait_us = start.elapsed().as_micros() as u64;

        // 更新统计信息
        self.stats.wait_count.fetch_add(1, Ordering::Relaxed);
        self.stats
            .total_wait_us
            .fetch_add(wait_us, Ordering::Relaxed);

        // 离开安全点
        self.threads_at_safepoint.fetch_sub(1, Ordering::Release);
    }

    /// 请求 GC
    ///
    /// 由 GC 线程调用，请求所有 mutator 线程到达安全点
    pub fn request_gc(&self) {
        self.gc_requested.store(true, Ordering::Release);
    }

    /// 等待所有线程到达安全点
    ///
    /// 由 GC 线程调用，等待所有 mutator 线程暂停
    pub fn wait_for_safepoint(&self) -> Result<(), GCError> {
        let timeout = Duration::from_secs(5); // 5 秒超时
        let start = Instant::now();

        while self.threads_at_safepoint.load(Ordering::Acquire) < self.total_threads {
            if start.elapsed() > timeout {
                return Err(GCError::GCFailed("Safepoint timeout".into()));
            }
            std::hint::spin_loop();
        }

        Ok(())
    }

    /// 完成 GC
    ///
    /// 由 GC 线程调用，通知所有 mutator 线程恢复执行
    pub fn gc_complete(&self) {
        self.gc_requested.store(false, Ordering::Release);
        self.gc_in_progress.store(false, Ordering::Release);
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> &SafepointStats {
        &self.stats
    }

    /// 重置统计信息
    pub fn reset_stats(&self) {
        self.stats.check_count.store(0, Ordering::Relaxed);
        self.stats.gc_triggered.store(0, Ordering::Relaxed);
        self.stats.wait_count.store(0, Ordering::Relaxed);
        self.stats.total_wait_us.store(0, Ordering::Relaxed);
    }
}

/// GC 安全点（线程本地）
///
/// 每个 mutator 线程应该有一个 Safepoint 实例
pub struct Safepoint {
    /// 安全点管理器
    manager: Arc<SafepointManager>,
    /// 当前状态
    state: SafepointState,
    /// 本地计数器（用于优化检查频率）
    local_counter: u64,
    /// 检查阈值（每 N 次操作检查一次）
    check_threshold: u64,
}

impl Safepoint {
    /// 创建新的安全点
    pub fn new(manager: Arc<SafepointManager>) -> Self {
        Self {
            manager,
            state: SafepointState::Running,
            local_counter: 0,
            check_threshold: 1000, // 每 1000 次操作检查一次
        }
    }

    /// 使用默认配置创建
    pub fn with_manager(manager: &SafepointManager) -> Self {
        Self {
            // 需要将 &SafepointManager 转换为 Arc<SafepointManager>
            // 由于 SafepointManager 现在实现了 Clone，我们可以创建一个新的 Arc
            manager: Arc::new(manager.clone()),
            state: SafepointState::Running,
            local_counter: 0,
            check_threshold: 1000,
        }
    }

    /// 在关键位置检查安全点
    ///
    /// 应该在以下位置调用：
    /// - 循环回边
    /// - 方法调用/返回
    /// - 内存分配前
    #[inline]
    pub fn poll(&mut self) {
        self.local_counter = self.local_counter.wrapping_add(1);

        // 优化：不是每次都检查，使用计数器减少检查频率
        if !self.local_counter.is_multiple_of(self.check_threshold) {
            return;
        }

        // 检查是否需要轮询（这会更新统计信息）
        if self.manager.should_poll() {
            self.state = SafepointState::AtSafepoint;
            self.manager.poll_safepoint();
            self.state = SafepointState::Running;
        } else {
            // 即使不需要 GC，也要记录检查次数
            // 注意：这里我们假设检查次数由 should_poll 内部管理
            // 如果需要更精确的统计，可以在这里调用一个专门的方法
        }
    }

    /// 强制立即检查（忽略计数器）
    ///
    /// 用于关键位置如内存分配前
    #[inline]
    pub fn poll_now(&mut self) {
        if self.manager.should_poll() {
            self.state = SafepointState::AtSafepoint;
            self.manager.poll_safepoint();
            self.state = SafepointState::Running;
        }
    }

    /// 获取当前状态
    pub fn state(&self) -> SafepointState {
        self.state
    }

    /// 设置检查阈值
    pub fn set_check_threshold(&mut self, threshold: u64) {
        self.check_threshold = threshold;
    }
}

/// 自动安全点轮询器
///
/// 使用 RAII 模式，在作用域结束时自动轮询安全点
pub struct AutoSafepoint<'a> {
    safepoint: &'a mut Safepoint,
}

impl<'a> AutoSafepoint<'a> {
    /// 创建自动轮询器
    pub fn new(safepoint: &'a mut Safepoint) -> Self {
        Self { safepoint }
    }
}

impl<'a> Drop for AutoSafepoint<'a> {
    fn drop(&mut self) {
        self.safepoint.poll();
    }
}

impl Safepoint {
    /// 创建自动轮询器
    pub fn auto_poll(&mut self) -> AutoSafepoint<'_> {
        AutoSafepoint::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safepoint_manager_creation() {
        let manager = SafepointManager::new(2, Duration::from_micros(100));
        assert!(!manager.gc_requested.load(Ordering::Relaxed));
        assert!(!manager.gc_in_progress.load(Ordering::Relaxed));
    }

    #[test]
    fn test_safepoint_creation() {
        let manager = SafepointManager::with_defaults();
        let safepoint = Safepoint::with_manager(&manager);

        assert_eq!(safepoint.state(), SafepointState::Running);
        assert_eq!(safepoint.check_threshold, 1000);
    }

    #[test]
    fn test_safepoint_poll() {
        let manager = SafepointManager::new(1, Duration::from_micros(10));
        let mut safepoint = Safepoint::with_manager(&manager);

        // 正常轮询（不应该触发 GC）
        for _ in 0..10 {
            safepoint.poll();
        }

        assert_eq!(safepoint.state(), SafepointState::Running);
    }

    #[test]
    fn test_safepoint_stats() {
        // 使用更短的检查间隔以便测试
        let manager = SafepointManager::new(1, Duration::from_micros(1));
        let mut safepoint = Safepoint::with_manager(&manager);

        // 降低阈值以便测试
        safepoint.set_check_threshold(10);

        // 执行足够的轮询以触发检查
        for _ in 0..20 {
            safepoint.poll();
            // 添加小延迟确保时间间隔触发
            std::thread::sleep(Duration::from_micros(10));
        }

        let stats = manager.get_stats();
        // 检查计数应该大于 0（至少触发 2 次检查：20 / 10 = 2）
        assert!(stats.check_count() > 0);
    }

    #[test]
    fn test_set_check_threshold() {
        let manager = SafepointManager::with_defaults();
        let mut safepoint = Safepoint::with_manager(&manager);

        safepoint.set_check_threshold(100);
        assert_eq!(safepoint.check_threshold, 100);
    }

    #[test]
    fn test_auto_safepoint() {
        // 使用更短的检查间隔以便测试
        let manager = SafepointManager::new(1, Duration::from_micros(1));
        let mut safepoint = Safepoint::with_manager(&manager);

        // 降低阈值以便测试
        safepoint.set_check_threshold(10);

        // 执行足够的轮询操作
        for _ in 0..25 {
            safepoint.poll();
            // 添加小延迟确保时间间隔触发
            std::thread::sleep(Duration::from_micros(10));
        }

        // 验证状态仍然是 Running
        assert_eq!(safepoint.state(), SafepointState::Running);

        // 验证轮询发生了
        let stats = manager.get_stats();
        // 应该至少有 2 次检查（25 / 10 = 2.5，向上取整是 3）
        assert!(stats.check_count() >= 2);
    }
}
