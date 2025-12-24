/// 分代 GC 配置
#[derive(Debug, Clone)]
pub struct GenerationalGcConfig {
    /// 新生代配置
    pub young_config: YoungGenConfig,
    /// 老生代配置
    pub old_config: OldGenConfig,
    /// 晋升阈值（对象年龄）
    pub promotion_threshold: u64,
    /// 记忆集大小
    pub remembered_set_size: usize,
}

/// 新生代配置
#[derive(Debug, Clone)]
pub struct YoungGenConfig {
    /// 新生代大小（字节）
    pub young_gen_size: usize,
    /// Eden 区大小（字节）
    pub eden_size: usize,
    /// Survivor 区大小（字节）
    pub survivor_size: usize,
    /// 复制算法的最大存活对象比例
    pub max_survivor_ratio: f64,
}

/// 老生代配置
#[derive(Debug, Clone)]
pub struct OldGenConfig {
    /// 老生代初始大小（字节）
    pub old_gen_size: usize,
    /// 老生代最大大小（字节）
    pub max_old_gen_size: usize,
    /// 标记-清除算法的最大停顿时间（微秒）
    pub max_pause_time_us: u64,
}

impl Default for GenerationalGcConfig {
    fn default() -> Self {
        Self {
            young_config: YoungGenConfig::default(),
            old_config: OldGenConfig::default(),
            promotion_threshold: 15,
            remembered_set_size: 1024,
        }
    }
}

impl Default for YoungGenConfig {
    fn default() -> Self {
        Self {
            young_gen_size: 16 * 1024 * 1024,
            eden_size: 8 * 1024 * 1024,
            survivor_size: 4 * 1024 * 1024,
            max_survivor_ratio: 0.5,
        }
    }
}

impl Default for OldGenConfig {
    fn default() -> Self {
        Self {
            old_gen_size: 64 * 1024 * 1024,
            max_old_gen_size: 512 * 1024 * 1024,
            max_pause_time_us: 10000,
        }
    }
}
