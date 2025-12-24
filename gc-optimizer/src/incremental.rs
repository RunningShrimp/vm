use parking_lot::RwLock;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::time::Instant;

use crate::{GcPhase, GcResult};

pub use config::*;
pub use state::*;
pub use marker::*;
pub use sweeper::*;

mod config;
mod state;
mod marker;
mod sweeper;

/// 增量式垃圾回收器
/// 
/// 将标记和清除阶段分片执行，减少单次暂停时间。
/// 核心特性：
/// - 标记阶段分片：每次最多处理 N 个对象
/// - 清除阶段分片：每次最多释放 M 个对象
/// - 自适应片大小：根据暂停时间动态调整
/// - 写屏障集成：增量过程中记录修改
pub struct IncrementalGc {
    /// GC 配置
    config: IncrementalGcConfig,
    /// 当前 GC 状态
    state: Arc<RwLock<IncrementalState>>,
    /// 增量标记器
    marker: Arc<IncrementalMarker>,
    /// 增量清除器
    sweeper: Arc<IncrementalSweeper>,
    /// 当前阶段
    phase: Arc<AtomicU8>,
}

impl IncrementalGc {
    /// 创建新的增量式 GC
    pub fn new(config: IncrementalGcConfig) -> Self {
        let config_ref = config.clone();
        Self {
            config,
            state: Arc::new(RwLock::new(IncrementalState::new())),
            marker: Arc::new(IncrementalMarker::new(&config_ref)),
            sweeper: Arc::new(IncrementalSweeper::new(&config_ref)),
            phase: Arc::new(AtomicU8::new(GcPhase::Idle as u8)),
        }
    }

    /// 执行增量回收步骤
    /// 
    /// 根据当前阶段执行相应的增量操作。
    /// 返回本次步骤处理的字节数。
    pub fn step(&self, max_time_us: u64) -> Result<u64, crate::GcError> {
        let start = Instant::now();
        let target_time = Duration::from_micros(max_time_us);
        
        let phase = self.current_phase();
        let mut processed_bytes = 0u64;
        
        loop {
            if Instant::now().duration_since(start) >= target_time {
                break;
            }
            
            match phase {
                GcPhase::Idle => {
                    self.start_collection();
                    break;
                },
                GcPhase::Marking => {
                    let bytes = self.marker.step(&self.config)?;
                    processed_bytes += bytes;
                    if self.marker.is_complete() {
                        self.transition_to_sweeping();
                        break;
                    }
                },
                GcPhase::Sweeping => {
                    let bytes = self.sweeper.step(&self.config)?;
                    processed_bytes += bytes;
                    if self.sweeper.is_complete() {
                        self.finish_collection();
                        break;
                    }
                },
                _ => {
                    break;
                }
            }
        }
        
        Ok(processed_bytes)
    }

    /// 开始新的回收周期
    pub fn start_collection(&self) {
        self.phase.store(GcPhase::Marking as u8, Ordering::Release);
        self.state.write().start_collection();
        self.marker.reset();
        self.sweeper.reset();
    }

    /// 记录对象写操作（写屏障）
    pub fn record_write(&self, obj_addr: u64) {
        self.marker.record_write(obj_addr);
    }

    /// 获取当前阶段
    pub fn current_phase(&self) -> GcPhase {
        match self.phase.load(Ordering::Acquire) {
            0 => GcPhase::Idle,
            1 => GcPhase::Marking,
            2 => GcPhase::Sweeping,
            3 => GcPhase::Compacting,
            _ => GcPhase::Idle,
        }
    }

    /// 检查是否处于回收中
    pub fn is_collecting(&self) -> bool {
        self.phase.load(Ordering::Acquire) != GcPhase::Idle as u8
    }

    /// 获取回收进度（0.0 - 1.0）
    pub fn progress(&self) -> f64 {
        match self.current_phase() {
            GcPhase::Idle => 0.0,
            GcPhase::Marking => self.marker.progress(),
            GcPhase::Sweeping => {
                0.5 + self.sweeper.progress() * 0.5
            },
            _ => 1.0,
        }
    }

    /// 转换到清除阶段
    fn transition_to_sweeping(&self) {
        self.phase.store(GcPhase::Sweeping as u8, Ordering::Release);
        self.sweeper.set_marked_objects(self.marker.get_marked_objects());
    }

    /// 完成回收周期
    fn finish_collection(&self) {
        self.phase.store(GcPhase::Idle as u8, Ordering::Release);
        self.state.write().finish_collection();
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> IncrementalGcStats {
        let state = self.state.read();
        IncrementalGcStats {
            total_collections: state.total_collections,
            total_pause_time_us: state.total_pause_time_us,
            marking_steps: state.marking_steps,
            sweeping_steps: state.sweeping_steps,
            objects_marked: state.objects_marked,
            objects_swept: state.objects_swept,
            bytes_collected: state.bytes_collected,
            avg_step_time_us: if state.marking_steps + state.sweeping_steps > 0 {
                state.total_pause_time_us as f64 / (state.marking_steps + state.sweeping_steps) as f64
            } else {
                0.0
            },
        }
    }

    /// 重置统计信息
    pub fn reset_stats(&self) {
        self.state.write().reset_stats();
    }

    /// 强制完成当前回收（不考虑时间限制）
    pub fn finish_current_collection(&self) -> GcResult<()> {
        let phase = self.current_phase();
        
        match phase {
            GcPhase::Marking => {
                while !self.marker.is_complete() {
                    self.marker.step(&self.config)?;
                }
                self.transition_to_sweeping();
                self.finish_collection();
            },
            GcPhase::Sweeping => {
                while !self.sweeper.is_complete() {
                    self.sweeper.step(&self.config)?;
                }
                self.finish_collection();
            },
            _ => {}
        }
        
        Ok(())
    }
}

impl Default for IncrementalGc {
    fn default() -> Self {
        Self::new(IncrementalGcConfig::default())
    }
}

use std::time::Duration;

/// 增量式 GC 统计信息
#[derive(Debug, Clone, Default)]
pub struct IncrementalGcStats {
    /// 总回收次数
    pub total_collections: u64,
    /// 总暂停时间（微秒）
    pub total_pause_time_us: u64,
    /// 标记步骤数
    pub marking_steps: u64,
    /// 清除步骤数
    pub sweeping_steps: u64,
    /// 标记的对象数
    pub objects_marked: u64,
    /// 清除的对象数
    pub objects_swept: u64,
    /// 收集的字节数
    pub bytes_collected: u64,
    /// 平均步骤时间（微秒）
    pub avg_step_time_us: f64,
}

impl IncrementalGcStats {
    /// 计算平均暂停时间（每次回收）
    pub fn avg_pause_time_us(&self) -> f64 {
        if self.total_collections == 0 {
            0.0
        } else {
            self.total_pause_time_us as f64 / self.total_collections as f64
        }
    }

    /// 计算回收效率（字节/微秒）
    pub fn efficiency(&self) -> f64 {
        if self.total_pause_time_us == 0 {
            0.0
        } else {
            self.bytes_collected as f64 / self.total_pause_time_us as f64
        }
    }

    /// 计算平均每步骤处理的字节数
    pub fn avg_bytes_per_step(&self) -> f64 {
        let total_steps = self.marking_steps + self.sweeping_steps;
        if total_steps == 0 {
            0.0
        } else {
            self.bytes_collected as f64 / total_steps as f64
        }
    }
}
