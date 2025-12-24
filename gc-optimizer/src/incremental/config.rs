/// 增量式 GC 配置
#[derive(Debug, Clone)]
pub struct IncrementalGcConfig {
    /// 单次标记步骤的最大对象数
    pub max_mark_per_step: usize,
    /// 单次清除步骤的最大对象数
    pub max_sweep_per_step: usize,
    /// 初始堆大小（字节）
    pub initial_heap_size: usize,
    /// 增长因子
    pub growth_factor: f64,
    /// 目标暂停时间（微秒）
    pub target_pause_us: u64,
    /// 最大暂停时间（微秒）
    pub max_pause_us: u64,
    /// 最小暂停时间（微秒）
    pub min_pause_us: u64,
    /// 自适应调整步长（百分比）
    pub adaptive_adjustment: f64,
    /// 启用写屏障优化
    pub enable_write_barrier: bool,
    /// 启用三色标记
    pub enable_tricolor_marking: bool,
    /// 标记位图大小
    pub mark_bitmap_size: usize,
}

impl Default for IncrementalGcConfig {
    fn default() -> Self {
        Self {
            max_mark_per_step: 1000,
            max_sweep_per_step: 2000,
            initial_heap_size: 1024 * 1024, // 1MB
            growth_factor: 1.5,
            target_pause_us: 5_000, // 5ms
            max_pause_us: 10_000, // 10ms
            min_pause_us: 1_000, // 1ms
            adaptive_adjustment: 0.1,
            enable_write_barrier: true,
            enable_tricolor_marking: true,
            mark_bitmap_size: 1024 * 1024 * 8, // 1MB * 8 bits/byte
        }
    }
}
