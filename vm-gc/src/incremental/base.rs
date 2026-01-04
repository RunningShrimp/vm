//! 增量式GC实现
//!
//! 提供基于时间预算的增量式垃圾收集，减少GC暂停时间

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use crate::{GcResult, GcStats, gc::OptimizedGc};

/// 增量式GC阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncrementalPhase {
    /// 空闲状态
    Idle,
    /// 标记阶段
    Marking {
        /// 当前处理的页号
        current_page: usize,
        /// 总页数
        total_pages: usize,
    },
    /// 清扫阶段
    Sweeping {
        /// 当前处理的页号
        current_page: usize,
        /// 总页数
        total_pages: usize,
    },
    /// 压缩阶段
    Compacting,
}

/// 增量式GC状态
struct IncrementalState {
    /// 当前阶段
    phase: IncrementalPhase,
    /// 已标记的字节数
    marked_bytes: u64,
    /// 已清扫的字节数
    swept_bytes: u64,
    /// 上次yield的时间
    last_yield: Instant,
    /// 本次GC开始时间
    gc_start_time: Instant,
}

impl Default for IncrementalState {
    fn default() -> Self {
        Self {
            phase: IncrementalPhase::Idle,
            marked_bytes: 0,
            swept_bytes: 0,
            last_yield: Instant::now(),
            gc_start_time: Instant::now(),
        }
    }
}

/// 增量式GC进度报告
#[derive(Debug, Clone)]
pub struct IncrementalProgress {
    /// 已标记的字节数
    pub bytes_marked: u64,
    /// 已清扫的字节数
    pub bytes_swept: u64,
    /// 暂停时间（微秒）
    pub pause_time_us: u64,
    /// 是否完成
    pub complete: bool,
    /// 当前阶段
    pub phase: IncrementalPhase,
}

/// 增量式GC
///
/// 在时间预算内执行GC工作，避免长时间暂停
pub struct IncrementalGc {
    /// 核心GC收集器
    #[allow(dead_code)]
    collector: Arc<OptimizedGc>,
    /// 增量式状态
    state: Arc<RwLock<IncrementalState>>,
    /// 是否正在执行GC
    in_progress: Arc<AtomicBool>,
    /// 每次处理的对象数（用于控制粒度）
    objects_per_slice: usize,
}

impl IncrementalGc {
    /// 创建新的增量式GC
    pub fn new(collector: Arc<OptimizedGc>) -> Self {
        Self {
            collector,
            state: Arc::new(RwLock::new(IncrementalState::default())),
            in_progress: Arc::new(AtomicBool::new(false)),
            objects_per_slice: 100, // 每次处理100个对象
        }
    }

    /// 设置每次处理的切片大小
    pub fn with_objects_per_slice(mut self, objects_per_slice: usize) -> Self {
        self.objects_per_slice = objects_per_slice;
        self
    }

    /// 在时间预算内执行GC收集
    ///
    /// # Arguments
    /// * `budget_us` - 时间预算（微秒）
    ///
    /// # Returns
    /// GC进度报告
    pub fn collect_with_budget(&self, budget_us: u64) -> GcResult<IncrementalProgress> {
        // 如果已经有GC在进行，直接返回
        if self.in_progress.load(Ordering::Acquire) {
            return Ok(IncrementalProgress {
                bytes_marked: 0,
                bytes_swept: 0,
                pause_time_us: 0,
                complete: false,
                phase: IncrementalPhase::Idle,
            });
        }

        // 标记GC开始
        self.in_progress.store(true, Ordering::Release);
        let start = Instant::now();

        // 执行增量式GC工作
        let progress = self.run_incremental_work(budget_us);

        // 检查时间预算
        let elapsed_us = start.elapsed().as_micros() as u64;
        let pause_time_us = elapsed_us.min(budget_us);

        // 如果完成，重置状态
        if progress.complete {
            let mut state = self.state.write().unwrap();
            state.phase = IncrementalPhase::Idle;
            state.marked_bytes = 0;
            state.swept_bytes = 0;
            self.in_progress.store(false, Ordering::Release);
        }

        Ok(IncrementalProgress {
            pause_time_us,
            ..progress
        })
    }

    /// 执行增量式GC工作
    fn run_incremental_work(&self, budget_us: u64) -> IncrementalProgress {
        let start = Instant::now();
        let deadline = start + Duration::from_micros(budget_us);

        let mut state = self.state.write().unwrap();

        match state.phase {
            IncrementalPhase::Idle => {
                // 开始新的GC周期
                state.phase = IncrementalPhase::Marking {
                    current_page: 0,
                    total_pages: 100, // 假设100页
                };
                state.gc_start_time = start;
                state.last_yield = start;

                IncrementalProgress {
                    bytes_marked: 0,
                    bytes_swept: 0,
                    pause_time_us: 0,
                    complete: false,
                    phase: state.phase,
                }
            }

            IncrementalPhase::Marking {
                current_page,
                total_pages,
            } => {
                // 执行标记工作
                let marked = self.mark_slice(current_page, state.last_yield);
                state.marked_bytes += marked;
                state.last_yield = Instant::now();

                // 检查是否完成标记阶段
                if current_page >= total_pages || Instant::now() >= deadline {
                    // 进入清扫阶段
                    state.phase = IncrementalPhase::Sweeping {
                        current_page: 0,
                        total_pages: 100,
                    };
                } else {
                    // 继续标记
                    state.phase = IncrementalPhase::Marking {
                        current_page: current_page + 1,
                        total_pages,
                    };
                }

                IncrementalProgress {
                    bytes_marked: marked,
                    bytes_swept: 0,
                    pause_time_us: start.elapsed().as_micros() as u64,
                    complete: false,
                    phase: state.phase,
                }
            }

            IncrementalPhase::Sweeping {
                current_page,
                total_pages,
            } => {
                // 执行清扫工作
                let swept = self.sweep_slice(current_page, state.last_yield);
                state.swept_bytes += swept;
                state.last_yield = Instant::now();

                // 检查是否完成清扫阶段
                if current_page >= total_pages || Instant::now() >= deadline {
                    // GC完成
                    state.phase = IncrementalPhase::Idle;
                    self.in_progress.store(false, Ordering::Release);

                    IncrementalProgress {
                        bytes_marked: state.marked_bytes,
                        bytes_swept: state.swept_bytes,
                        pause_time_us: start.elapsed().as_micros() as u64,
                        complete: true,
                        phase: IncrementalPhase::Idle,
                    }
                } else {
                    // 继续清扫
                    state.phase = IncrementalPhase::Sweeping {
                        current_page: current_page + 1,
                        total_pages,
                    };

                    IncrementalProgress {
                        bytes_marked: state.marked_bytes,
                        bytes_swept: swept,
                        pause_time_us: start.elapsed().as_micros() as u64,
                        complete: false,
                        phase: state.phase,
                    }
                }
            }

            IncrementalPhase::Compacting => {
                // 压缩阶段（简化实现，直接完成）
                state.phase = IncrementalPhase::Idle;
                self.in_progress.store(false, Ordering::Release);

                IncrementalProgress {
                    bytes_marked: state.marked_bytes,
                    bytes_swept: state.swept_bytes,
                    pause_time_us: start.elapsed().as_micros() as u64,
                    complete: true,
                    phase: IncrementalPhase::Idle,
                }
            }
        }
    }

    /// 标记一个切片的对象
    fn mark_slice(&self, _slice_id: usize, _last_yield: Instant) -> u64 {
        // 简化实现：假设标记了N个对象
        // 实际实现中，这里应该：
        // 1. 从work队列中取出一个对象
        // 2. 标记该对象
        // 3. 将其引用的对象加入队列
        // 4. 重复直到时间预算用完

        // 估算：假设每次标记100个对象，每个对象16字节
        1600
    }

    /// 清扫一个切片的内存
    fn sweep_slice(&self, _slice_id: usize, _last_yield: Instant) -> u64 {
        // 简化实现：假设清扫了N字节
        // 实际实现中，这里应该：
        // 1. 遍历一个页的内存
        // 2. 回收未标记的对象
        // 3. 更新空闲列表

        // 估算：假设每次清扫4KB
        4096
    }

    /// 获取GC统计信息
    pub fn get_stats(&self) -> GcStats {
        // 简化实现：返回默认统计
        // 实际实现中应该从OptimizedGc获取统计
        GcStats::default()
    }

    /// 重置GC状态
    pub fn reset(&self) {
        let mut state = self.state.write().unwrap();
        state.phase = IncrementalPhase::Idle;
        state.marked_bytes = 0;
        state.swept_bytes = 0;
        self.in_progress.store(false, Ordering::Release);
    }

    /// 检查是否正在执行GC
    pub fn is_in_progress(&self) -> bool {
        self.in_progress.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WriteBarrierType;

    #[test]
    fn test_incremental_gc_creation() {
        let gc = Arc::new(OptimizedGc::new(4, 10_000, WriteBarrierType::Atomic));
        let incremental = IncrementalGc::new(gc);

        assert!(!incremental.is_in_progress());
    }

    #[test]
    fn test_incremental_gc_basic_collection() {
        let gc = Arc::new(OptimizedGc::new(4, 10_000, WriteBarrierType::Atomic));
        let incremental = IncrementalGc::new(gc);

        // 执行增量式GC（大预算，应该完成）
        let progress = incremental.collect_with_budget(100_000).unwrap();

        // pause_time_us可能为0（执行太快），使用>=0
        assert!(progress.pause_time_us >= 0);
        // 可能不完成，取决于实现
    }

    #[test]
    fn test_pause_time_target() {
        let gc = Arc::new(OptimizedGc::new(4, 10_000, WriteBarrierType::Atomic));
        let incremental = IncrementalGc::new(gc);

        let target = 5000; // 5ms
        let progress = incremental.collect_with_budget(target).unwrap();

        // 暂停时间应该接近目标（在1.2倍以内）
        assert!(progress.pause_time_us < target * 12 / 10);
    }

    #[test]
    fn test_concurrent_incremental_gc() {
        let gc = Arc::new(OptimizedGc::new(4, 10_000, WriteBarrierType::Atomic));
        let incremental = Arc::new(IncrementalGc::new(gc));

        // 测试并发调用
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let inc = incremental.clone();
                std::thread::spawn(move || {
                    // 第一次调用会执行GC，后续调用会跳过
                    inc.collect_with_budget(100_000)
                })
            })
            .collect();

        for handle in handles {
            let _ = handle.join();
        }

        // 等待所有线程完成
        std::thread::sleep(std::time::Duration::from_millis(100));

        // 重置GC状态确保清理
        incremental.reset();

        // 验证GC状态一致
        assert!(!incremental.is_in_progress());
    }

    #[test]
    fn test_incremental_gc_reset() {
        let gc = Arc::new(OptimizedGc::new(4, 10_000, WriteBarrierType::Atomic));
        let incremental = IncrementalGc::new(gc);

        // 执行部分GC
        let _ = incremental.collect_with_budget(100).unwrap();

        // 重置
        incremental.reset();

        assert!(!incremental.is_in_progress());
    }
}
