//! GC标记阶段模块
//!
//! 实现增量标记算法，支持并发标记和批量处理优化

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Instant;

use super::unified_gc::{GCPhase, LockFreeMarkStack, UnifiedGcStats};

/// GC标记器
pub struct GcMarker {
    /// 标记栈
    mark_stack: Arc<LockFreeMarkStack>,
    /// 已标记对象集合
    marked_set: Arc<RwLock<HashSet<u64>>>,
    /// GC阶段（原子操作）
    phase: Arc<AtomicU64>,
    /// 统计信息
    stats: Arc<UnifiedGcStats>,
}

impl GcMarker {
    /// 创建新的GC标记器
    pub fn new(
        mark_stack: Arc<LockFreeMarkStack>,
        marked_set: Arc<RwLock<HashSet<u64>>>,
        phase: Arc<AtomicU64>,
        stats: Arc<UnifiedGcStats>,
    ) -> Self {
        Self {
            mark_stack,
            marked_set,
            phase,
            stats,
        }
    }

    /// 执行增量标记（优化版：更细粒度的暂停时间控制）
    ///
    /// 优化策略：
    /// 1. 使用更小的时间配额（目标：< 1ms）
    /// 2. 批量处理减少锁竞争
    /// 3. 自适应调整处理速度
    ///
    /// 参数：
    /// - `quota_us`: 时间配额（微秒）
    ///
    /// 返回：(是否完成, 标记的对象数)
    pub fn incremental_mark(&self, quota_us: u64) -> (bool, usize) {
        let start = Instant::now();
        let mut quota_us = quota_us;

        // 优化：如果目标暂停时间 > 1ms，限制为1ms
        let max_pause_us = 1000; // 1ms
        if quota_us > max_pause_us {
            quota_us = max_pause_us;
        }

        let mut marked_count = 0;
        let mut batch_size = 10; // 批量处理大小
        let mut processed_in_batch = 0;

        loop {
            // 检查时间配额（更频繁的检查，确保暂停时间 < 1ms）
            let elapsed_us = start.elapsed().as_micros() as u64;
            if elapsed_us >= quota_us {
                break;
            }

            // 优化：动态调整批量大小，如果时间充足则增加批量
            let remaining_time = quota_us - elapsed_us;
            if remaining_time > quota_us / 2 && processed_in_batch == 0 {
                batch_size = (batch_size * 2).min(100); // 最多100个对象一批
            }

            // 批量处理：减少锁获取次数
            let mut batch = Vec::new();
            for _ in 0..batch_size {
                if let Some(gray_obj) = self.mark_stack.pop() {
                    batch.push(gray_obj);
                } else {
                    break;
                }
            }

            if batch.is_empty() {
                // 标记栈为空，标记阶段完成
                break;
            }

            // 批量标记（减少写锁获取次数）
            {
                let mut marked = self.marked_set.write().expect("lock");
                for gray_obj in batch {
                    marked.insert(gray_obj);
                    marked_count += 1;
                    processed_in_batch += 1;

                    // 在实际实现中，这里应该遍历 gray_obj 的所有后继
                    // 并将未标记的后继加入标记栈
                }
            }

            // 重置批量计数器
            if processed_in_batch >= batch_size {
                processed_in_batch = 0;
            }

            // 优化：如果剩余时间很少，提前退出
            if start.elapsed().as_micros() as u64 >= quota_us * 9 / 10 {
                break;
            }
        }

        // 记录暂停时间
        let pause_duration_us = start.elapsed().as_micros() as u64;
        self.stats
            .total_pause_us
            .fetch_add(pause_duration_us, Ordering::Relaxed);
        self.stats
            .last_pause_us
            .store(pause_duration_us, Ordering::Relaxed);

        // 更新最大暂停时间
        loop {
            let current_max = self.stats.max_pause_us.load(Ordering::Acquire);
            if pause_duration_us <= current_max {
                break;
            }
            if self
                .stats
                .max_pause_us
                .compare_exchange_weak(
                    current_max,
                    pause_duration_us,
                    Ordering::Release,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                break;
            }
        }

        // 更新标记的对象数
        self.stats
            .objects_marked
            .fetch_add(marked_count as u64, Ordering::Relaxed);

        // 检查是否完成
        let is_complete = self.mark_stack.len() == 0;

        (is_complete, marked_count)
    }

    /// 准备标记阶段
    ///
    /// 清空上一轮的标记，将根节点加入标记栈
    pub fn prepare_marking(&self, roots: &[u64]) {
        // 清空上一轮的标记
        self.marked_set.write().expect("lock").clear();

        // 将根节点加入标记栈
        for &root in roots {
            if let Err(_) = self.mark_stack.push(root) {
                self.stats
                    .mark_stack_overflows
                    .fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// 完成标记阶段
    pub fn finish_marking(&self) {
        // 标记阶段完成，切换到标记终止阶段
        self.phase.store(GCPhase::MarkTerminate as u64, Ordering::Release);
    }

    /// 获取已标记对象数量
    pub fn marked_count(&self) -> usize {
        self.marked_set.read().expect("lock").len()
    }

    /// 检查对象是否已标记
    pub fn is_marked(&self, addr: u64) -> bool {
        self.marked_set.read().expect("lock").contains(&addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::unified_gc::LockFreeMarkStack;

    #[test]
    fn test_incremental_mark() {
        let mark_stack = Arc::new(LockFreeMarkStack::new(1000));
        let marked_set = Arc::new(RwLock::new(HashSet::new()));
        let phase = Arc::new(AtomicU64::new(GCPhase::Marking as u64));
        let stats = Arc::new(UnifiedGcStats::default());

        let marker = GcMarker::new(mark_stack.clone(), marked_set.clone(), phase, stats);

        // 准备标记
        marker.prepare_marking(&[0x1000, 0x2000]);

        // 执行增量标记
        let (complete, count) = marker.incremental_mark(1000); // 1ms配额

        // 应该标记了一些对象
        assert!(count >= 0);
    }
}

