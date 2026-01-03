//! 硬件加速性能优化模块
//!
//! 提供 VM 切换优化和批量 MMU 更新功能，降低上下文切换开销。

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// 批量 MMU 更新请求
#[derive(Debug, Clone)]
pub struct MmuBatchUpdate {
    /// 客户端物理地址
    pub gpa: u64,
    /// 主机虚拟地址
    pub hva: u64,
    /// 大小（字节）
    pub size: u64,
    /// 权限标志（读/写/执行）
    pub flags: u32,
}

impl MmuBatchUpdate {
    /// 创建新的 MMU 更新请求
    pub fn new(gpa: u64, hva: u64, size: u64, flags: u32) -> Self {
        Self {
            gpa,
            hva,
            size,
            flags,
        }
    }

    /// 检查是否可以与另一个更新合并
    pub fn can_merge_with(&self, other: &Self) -> bool {
        // 检查是否相邻且有相同权限
        self.flags == other.flags
            && self.gpa + self.size == other.gpa
            && self.hva + self.size == other.hva
    }

    /// 合并两个更新请求
    pub fn merge(&mut self, other: &Self) {
        if self.can_merge_with(other) {
            self.size += other.size;
        }
    }
}

/// 批量 MMU 更新器
///
/// 通过批量处理和合并相邻的内存映射请求来减少 VM-EXIT 次数。
pub struct MmuBatchUpdater {
    /// 待处理的更新列表
    pending_updates: Vec<MmuBatchUpdate>,
    /// 批量大小
    batch_size: usize,
    /// 是否启用自动刷新
    auto_flush: bool,
    /// 统计信息
    stats: Arc<MmuBatchStats>,
}

/// MMU 批量更新统计信息
#[derive(Debug, Default)]
pub struct MmuBatchStats {
    /// 总更新次数
    pub total_updates: AtomicU64,
    /// 批量处理次数
    pub batch_flushes: AtomicU64,
    /// 合并的更新次数
    pub merged_updates: AtomicU64,
    /// 总处理时间（微秒）
    pub total_time_us: AtomicU64,
}

impl MmuBatchStats {
    /// 获取平均批量大小
    pub fn avg_batch_size(&self) -> f64 {
        let flushes = self.batch_flushes.load(Ordering::Relaxed);
        if flushes == 0 {
            return 0.0;
        }
        let updates = self.total_updates.load(Ordering::Relaxed);
        updates as f64 / flushes as f64
    }

    /// 获取合并效率（节省的调用次数 / 总更新次数）
    pub fn merge_efficiency(&self) -> f64 {
        let total = self.total_updates.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        let merged = self.merged_updates.load(Ordering::Relaxed);
        merged as f64 / total as f64
    }
}

impl MmuBatchUpdater {
    /// 创建新的批量更新器
    ///
    /// # 参数
    /// - `batch_size`: 批量大小（多少个更新后自动刷新）
    /// - `auto_flush`: 是否启用自动刷新
    pub fn new(batch_size: usize, auto_flush: bool) -> Self {
        Self {
            pending_updates: Vec::with_capacity(batch_size),
            batch_size,
            auto_flush,
            stats: Arc::new(MmuBatchStats::default()),
        }
    }

    /// 使用默认配置创建（批量大小 32，启用自动刷新）
    pub fn with_defaults() -> Self {
        Self::new(32, true)
    }

    /// 添加 MMU 更新请求
    pub fn add_update(&mut self, update: MmuBatchUpdate) {
        // 尝试与最后一个更新合并
        if let Some(last) = self.pending_updates.last_mut() {
            if last.can_merge_with(&update) {
                last.merge(&update);
                self.stats.merged_updates.fetch_add(1, Ordering::Relaxed);
            } else {
                self.pending_updates.push(update);
                self.stats.total_updates.fetch_add(1, Ordering::Relaxed);
            }
        } else {
            self.pending_updates.push(update);
            self.stats.total_updates.fetch_add(1, Ordering::Relaxed);
        }

        // 检查是否需要自动刷新
        if self.auto_flush && self.pending_updates.len() >= self.batch_size {
            self.flush();
        }
    }

    /// 批量刷新所有待处理的更新
    ///
    /// 返回合并后的更新列表
    pub fn flush(&mut self) -> Vec<MmuBatchUpdate> {
        let updates = std::mem::take(&mut self.pending_updates);
        self.stats.batch_flushes.fetch_add(1, Ordering::Relaxed);
        updates
    }

    /// 执行批量更新（由后端实现）
    ///
    /// # 参数
    /// - `executor`: 实际执行更新的闭包
    pub fn execute_batch<F>(&mut self, mut executor: F) -> Result<(), String>
    where
        F: FnMut(&[MmuBatchUpdate]) -> Result<(), String>,
    {
        if self.pending_updates.is_empty() {
            return Ok(());
        }

        let start = Instant::now();
        let updates = self.flush();

        let result = executor(&updates);

        let elapsed = start.elapsed();
        self.stats
            .total_time_us
            .fetch_add(elapsed.as_micros() as u64, Ordering::Relaxed);

        result
    }

    /// 获取待处理的更新数量
    pub fn pending_count(&self) -> usize {
        self.pending_updates.len()
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> &MmuBatchStats {
        &self.stats
    }

    /// 清空待处理的更新
    pub fn clear(&mut self) {
        self.pending_updates.clear();
    }
}

/// VM 上下文切换优化器
///
/// 通过延迟和合并上下文切换操作来降低 VM-EXIT 开销。
pub struct VmContextOptimizer {
    /// 上下文切换缓存
    context_cache: HashMap<u32, CachedContext>,
    /// 优化策略
    strategy: VmContextStrategy,
    /// 统计信息
    stats: Arc<VmContextStats>,
}

/// VM 上下文优化策略
#[derive(Debug, Clone, Copy)]
pub enum VmContextStrategy {
    /// 无优化（立即切换）
    None,
    /// 延迟切换（累积多个切换后批量执行）
    Delayed,
    /// 自适应（根据切换频率自动调整）
    Adaptive,
}

/// 缓存的上下文信息
#[derive(Debug, Clone)]
#[allow(dead_code)] // Reserved for future use
struct CachedContext {
    /// 上下文 ID
    id: u32,
    /// 缓存的寄存器状态
    regs_hash: u64,
    /// 最后访问时间
    last_access: Instant,
    /// 访问频率
    access_count: u64,
}

/// VM 上下文切换统计信息
#[derive(Debug, Default)]
pub struct VmContextStats {
    /// 总切换次数
    pub total_switches: AtomicU64,
    /// 延迟切换次数
    pub delayed_switches: AtomicU64,
    /// 缓存命中次数
    pub cache_hits: AtomicU64,
    /// 缓存未命中次数
    pub cache_misses: AtomicU64,
    /// 总切换时间（微秒）
    pub total_time_us: AtomicU64,
}

impl VmContextStats {
    /// 获取缓存命中率
    pub fn cache_hit_rate(&self) -> f64 {
        let total =
            self.cache_hits.load(Ordering::Relaxed) + self.cache_misses.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        let hits = self.cache_hits.load(Ordering::Relaxed);
        hits as f64 / total as f64
    }

    /// 获取平均切换时间（微秒）
    pub fn avg_switch_time_us(&self) -> f64 {
        let total = self.total_switches.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        let time_us = self.total_time_us.load(Ordering::Relaxed);
        time_us as f64 / total as f64
    }

    /// 获取延迟切换比率
    pub fn delay_ratio(&self) -> f64 {
        let total = self.total_switches.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        let delayed = self.delayed_switches.load(Ordering::Relaxed);
        delayed as f64 / total as f64
    }
}

impl VmContextOptimizer {
    /// 创建新的上下文优化器
    pub fn new(strategy: VmContextStrategy) -> Self {
        Self {
            context_cache: HashMap::new(),
            strategy,
            stats: Arc::new(VmContextStats::default()),
        }
    }

    /// 使用默认策略（自适应）创建
    pub fn with_defaults() -> Self {
        Self::new(VmContextStrategy::Adaptive)
    }

    /// 请求上下文切换
    ///
    /// 根据优化策略决定是否立即切换或延迟切换
    pub fn request_switch(&mut self, context_id: u32) -> SwitchAction {
        self.stats.total_switches.fetch_add(1, Ordering::Relaxed);

        match self.strategy {
            VmContextStrategy::None => SwitchAction::SwitchNow,
            VmContextStrategy::Delayed => {
                self.stats.delayed_switches.fetch_add(1, Ordering::Relaxed);
                SwitchAction::Delay
            }
            VmContextStrategy::Adaptive => {
                // 检查缓存
                if let Some(cached) = self.context_cache.get(&context_id) {
                    self.stats.cache_hits.fetch_add(1, Ordering::Relaxed);

                    // 高频访问的上下文立即切换
                    if cached.access_count > 10 {
                        SwitchAction::SwitchNow
                    } else {
                        self.stats.delayed_switches.fetch_add(1, Ordering::Relaxed);
                        SwitchAction::Delay
                    }
                } else {
                    self.stats.cache_misses.fetch_add(1, Ordering::Relaxed);
                    SwitchAction::SwitchNow
                }
            }
        }
    }

    /// 记录上下文访问
    pub fn record_access(&mut self, context_id: u32, regs_hash: u64) {
        let now = Instant::now();
        let entry = self
            .context_cache
            .entry(context_id)
            .or_insert(CachedContext {
                id: context_id,
                regs_hash,
                last_access: now,
                access_count: 0,
            });

        entry.last_access = now;
        entry.access_count += 1;
        entry.regs_hash = regs_hash;

        // 清理过期缓存（超过 1 分钟未访问）
        self.cleanup_cache();
    }

    /// 执行延迟的切换操作
    pub fn flush_delayed(&mut self, context_ids: &[u32]) -> Vec<u32> {
        // 根据访问频率排序
        let mut sorted: Vec<_> = context_ids
            .iter()
            .filter_map(|&id| {
                self.context_cache
                    .get(&id)
                    .map(|ctx| (id, ctx.access_count))
            })
            .collect();

        sorted.sort_by(|a, b| b.1.cmp(&a.1));

        sorted.into_iter().map(|(id, _)| id).collect()
    }

    /// 清理过期缓存
    fn cleanup_cache(&mut self) {
        let now = Instant::now();
        let timeout = Duration::from_secs(60);

        self.context_cache
            .retain(|_, ctx| now.duration_since(ctx.last_access) < timeout);
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> &VmContextStats {
        &self.stats
    }

    /// 重置统计信息
    pub fn reset_stats(&self) {
        self.stats.total_switches.store(0, Ordering::Relaxed);
        self.stats.delayed_switches.store(0, Ordering::Relaxed);
        self.stats.cache_hits.store(0, Ordering::Relaxed);
        self.stats.cache_misses.store(0, Ordering::Relaxed);
        self.stats.total_time_us.store(0, Ordering::Relaxed);
    }
}

/// 上下文切换动作
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchAction {
    /// 立即切换
    SwitchNow,
    /// 延迟切换
    Delay,
}

/// 硬件加速性能统计
#[derive(Debug, Default)]
pub struct AccelPerformanceStats {
    /// VM 切换次数
    pub vm_exits: AtomicU64,
    /// MMU 更新次数
    pub mmu_updates: AtomicU64,
    /// 寄存器访问次数
    pub register_accesses: AtomicU64,
    /// 中断注入次数
    pub interrupt_injections: AtomicU64,
    /// 总执行时间（微秒）
    pub total_exec_time_us: AtomicU64,
}

impl AccelPerformanceStats {
    /// 计算 VM-EXIT 率（每秒退出次数）
    pub fn exit_rate(&self, exec_time_us: u64) -> f64 {
        if exec_time_us == 0 {
            return 0.0;
        }
        let exits = self.vm_exits.load(Ordering::Relaxed);
        let exec_time_sec = exec_time_us as f64 / 1_000_000.0;
        exits as f64 / exec_time_sec
    }

    /// 获取 MMU 更新占总操作的百分比
    pub fn mmu_update_ratio(&self) -> f64 {
        let total = self.vm_exits.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        let mmu = self.mmu_updates.load(Ordering::Relaxed);
        mmu as f64 / total as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mmubatch_update_creation() {
        let update = MmuBatchUpdate::new(0x1000, 0x7000, 0x1000, 0x7);
        assert_eq!(update.gpa, 0x1000);
        assert_eq!(update.size, 0x1000);
    }

    #[test]
    fn test_mmubatch_update_merge() {
        let mut update1 = MmuBatchUpdate::new(0x1000, 0x7000, 0x1000, 0x7);
        let update2 = MmuBatchUpdate::new(0x2000, 0x8000, 0x1000, 0x7);

        assert!(update1.can_merge_with(&update2));
        update1.merge(&update2);
        assert_eq!(update1.size, 0x2000);
    }

    #[test]
    fn test_mmubatch_updater() {
        let mut updater = MmuBatchUpdater::with_defaults();

        updater.add_update(MmuBatchUpdate::new(0x1000, 0x7000, 0x1000, 0x7));
        updater.add_update(MmuBatchUpdate::new(0x2000, 0x8000, 0x1000, 0x7));

        // 第二个更新应该与第一个合并
        assert_eq!(updater.pending_count(), 1);

        let updates = updater.flush();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].size, 0x2000);
    }

    #[test]
    fn test_mmubatch_stats() {
        let mut updater = MmuBatchUpdater::new(10, false);

        for i in 0..20 {
            let gpa = 0x1000 + i * 0x1000;
            let hva = 0x7000 + i * 0x1000;
            updater.add_update(MmuBatchUpdate::new(gpa, hva, 0x1000, 0x7));
        }

        updater.flush();

        let stats = updater.get_stats();
        // 应该有合并（相邻且有相同权限）
        assert!(stats.merged_updates.load(Ordering::Relaxed) > 0);
    }

    #[test]
    fn test_vmcontext_optimizer() {
        let mut optimizer = VmContextOptimizer::with_defaults();

        // 记录一些访问
        optimizer.record_access(0, 12345);
        optimizer.record_access(1, 67890);

        // 高频访问的上下文应该立即切换
        for _ in 0..15 {
            optimizer.record_access(0, 12345);
        }

        let action = optimizer.request_switch(0);
        assert_eq!(action, SwitchAction::SwitchNow);
    }

    #[test]
    fn test_vmcontext_stats() {
        let optimizer = VmContextOptimizer::new(VmContextStrategy::Adaptive);
        let stats = optimizer.get_stats();

        assert_eq!(stats.total_switches.load(Ordering::Relaxed), 0);
        assert_eq!(stats.cache_hit_rate(), 0.0);
    }

    #[test]
    fn test_performance_stats() {
        let stats = AccelPerformanceStats::default();

        stats.vm_exits.store(1000, Ordering::Relaxed);
        stats.mmu_updates.store(100, Ordering::Relaxed);
        stats.total_exec_time_us.store(1_000_000, Ordering::Relaxed);

        // 1 秒内 1000 次退出 = 1000 exits/sec
        let exit_rate = stats.exit_rate(1_000_000);
        assert!((exit_rate - 1000.0).abs() < 0.1);

        // MMU 更新占 10%
        let mmu_ratio = stats.mmu_update_ratio();
        assert!((mmu_ratio - 0.1).abs() < 0.001);
    }
}
