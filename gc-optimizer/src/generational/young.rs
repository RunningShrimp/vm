use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use crate::GcError;
use super::config::YoungGenConfig;

/// 新生代
/// 
/// 使用复制回收算法：
/// - Eden 区：新分配的对象
/// - Survivor 区：经过一次 GC 仍存活的对象
pub struct YoungGeneration {
    /// Eden 区
    eden: Vec<u8>,
    /// Survivor 区 0
    survivor0: Vec<u8>,
    /// Survivor 区 1
    survivor1: Vec<u8>,
    /// 当前使用的 Survivor 区
    current_survivor: AtomicU64,
    /// Eden 区分配指针
    eden_ptr: AtomicUsize,
    /// 对象年龄映射
    object_ages: Vec<(u64, u64)>,
    /// 已回收对象数
    collected_count: AtomicU64,
    /// 回收次数
    collection_count: AtomicU64,
    /// 配置
    config: YoungGenConfig,
}

impl YoungGeneration {
    /// 创建新的新生代
    pub fn new(config: &YoungGenConfig) -> Self {
        Self {
            eden: vec![0; config.eden_size],
            survivor0: vec![0; config.survivor_size],
            survivor1: vec![0; config.survivor_size],
            current_survivor: AtomicU64::new(0),
            eden_ptr: AtomicUsize::new(0),
            object_ages: Vec::new(),
            collected_count: AtomicU64::new(0),
            collection_count: AtomicU64::new(0),
            config: config.clone(),
        }
    }

    /// 在 Eden 区分配对象
    pub fn allocate(&self, size: usize) -> Result<u64, GcError> {
        let current_ptr = self.eden_ptr.fetch_add(size, Ordering::Relaxed);

        if current_ptr + size > self.eden.len() {
            return Err(GcError::OutOfMemory {
                required: size,
                available: self.eden.len() - current_ptr,
            });
        }

        Ok(current_ptr as u64)
    }

    /// 执行 Minor GC
    pub fn collect(&self, roots: &[u64]) -> Result<Vec<u64>, GcError> {
        let mut promoted = Vec::new();

        for root in roots {
            if self.contains(*root) {
                promoted.push(*root);
            }
        }

        // 交换 Survivor 区
        self.current_survivor.fetch_xor(1, Ordering::Relaxed);

        // 重置 Eden 区
        self.eden_ptr.store(0, Ordering::Relaxed);

        // 更新回收统计
        self.collected_count.fetch_add(1, Ordering::Relaxed);
        self.collection_count.fetch_add(1, Ordering::Relaxed);

        Ok(promoted)
    }

    /// 检查地址是否在新生代
    pub fn contains(&self, addr: u64) -> bool {
        addr < self.eden.len() as u64
    }

    /// 获取所有对象地址（简化实现）
    pub fn get_all_objects(&self) -> Vec<u64> {
        let eden_ptr = self.eden_ptr.load(Ordering::Relaxed);
        let mut objects = Vec::new();

        for i in (0..eden_ptr).step_by(64) {
            objects.push(i as u64);
        }

        objects
    }

    /// 获取已用字节数
    pub fn used_bytes(&self) -> usize {
        self.eden_ptr.load(Ordering::Relaxed)
    }

    /// 获取容量
    pub fn capacity(&self) -> usize {
        self.config.young_gen_size
    }

    /// 获取回收的对象数
    pub fn collected_count(&self) -> u64 {
        self.collected_count.load(Ordering::Relaxed)
    }

    /// 获取回收次数
    pub fn collection_count(&self) -> u64 {
        self.collection_count.load(Ordering::Relaxed)
    }
}
