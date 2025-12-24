use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};

use crate::GcError;

pub mod config;
pub mod young;
pub mod old;
pub mod remembered_set;

use config::GenerationalGcConfig;
use young::YoungGeneration;
use old::OldGeneration;
use remembered_set::RememberedSet;

/// 分代垃圾回收器
/// 
/// 将堆分为新生代和老生代：
/// - 新生代：使用复制回收算法，快速回收短命对象
/// - 老生代：使用标记-清除算法，回收长寿命对象
pub struct GenerationalGc {
    /// 配置
    config: GenerationalGcConfig,
    /// 新生代
    young_gen: Arc<YoungGeneration>,
    /// 老生代
    old_gen: Arc<OldGeneration>,
    /// 记忆集（记录跨代引用）
    remembered_set: Arc<RememberedSet>,
    /// 当前代
    current_gen: AtomicU8,
    /// 回收计数器
    collection_count: AtomicU64,
}

impl GenerationalGc {
    /// 创建新的分代 GC
    pub fn new(config: GenerationalGcConfig) -> Self {
        let young_gen = Arc::new(YoungGeneration::new(&config.young_config));
        let old_gen = Arc::new(OldGeneration::new(&config.old_config));
        let remembered_set = Arc::new(RememberedSet::new(config.remembered_set_size));

        Self {
            config,
            young_gen,
            old_gen,
            remembered_set,
            current_gen: AtomicU8::new(0),
            collection_count: AtomicU64::new(0),
        }
    }

    /// 分配对象
    pub fn allocate(&self, size: usize) -> Result<u64, GcError> {
        let addr = self.young_gen.allocate(size)?;

        Ok(addr)
    }

    /// 执行 Minor GC（新生代回收）
    pub fn collect_minor(&self) -> Result<MinorGcStats, GcError> {
        let start_time = std::time::Instant::now();

        // 1. 从老生代扫描根
        let roots_from_old = self.remembered_set.get_roots();

        // 2. 标记新生代中的活跃对象
        let promoted = self.young_gen.collect(&roots_from_old)?;

        // 3. 将晋升的对象移动到老生代
        for obj_addr in &promoted {
            self.old_gen.add_object(*obj_addr, 64)?;
        }

        // 4. 清空记忆集
        self.remembered_set.clear();

        let duration_us = start_time.elapsed().as_micros() as u64;
        self.collection_count.fetch_add(1, Ordering::Relaxed);

        Ok(MinorGcStats {
            duration_us,
            objects_collected: self.young_gen.collected_count(),
            objects_promoted: promoted.len() as u64,
        })
    }

    /// 执行 Major GC（老生代回收）
    pub fn collect_major(&self) -> Result<MajorGcStats, GcError> {
        let start_time = std::time::Instant::now();

        // 1. 标记老生代中的活跃对象
        let roots = self.young_gen.get_all_objects();
        let roots_from_old = self.remembered_set.get_roots();

        let marked = self.old_gen.collect(&roots, &roots_from_old)?;

        let duration_us = start_time.elapsed().as_micros() as u64;

        Ok(MajorGcStats {
            duration_us,
            objects_marked: marked,
            objects_collected: self.old_gen.collected_count(),
        })
    }

    /// 记录跨代引用（写屏障）
    pub fn record_cross_gen_ref(&self, from_addr: u64, to_addr: u64) {
        if self.is_in_young_gen(from_addr) && self.is_in_old_gen(to_addr) {
            self.remembered_set.add(from_addr, to_addr);
        }
    }

    /// 检查地址是否在新生代
    fn is_in_young_gen(&self, addr: u64) -> bool {
        self.young_gen.contains(addr)
    }

    /// 检查地址是否在老生代
    fn is_in_old_gen(&self, addr: u64) -> bool {
        self.old_gen.contains(addr)
    }

    /// 获取内存使用统计
    pub fn get_memory_usage(&self) -> MemoryUsage {
        MemoryUsage {
            young_used: self.young_gen.used_bytes(),
            young_capacity: self.young_gen.capacity(),
            old_used: self.old_gen.used_bytes(),
            old_capacity: self.old_gen.capacity(),
        }
    }

    /// 获取回收统计
    pub fn get_collection_stats(&self) -> CollectionStats {
        CollectionStats {
            total_collections: self.collection_count.load(Ordering::Relaxed),
            young_collections: self.young_gen.collection_count(),
            old_collections: self.old_gen.collection_count(),
        }
    }

    /// 晋升阈值检查
    pub fn should_promote(&self, _obj_addr: u64, age: u64) -> bool {
        age >= self.config.promotion_threshold
    }
}

/// Minor GC 统计信息
#[derive(Debug, Clone)]
pub struct MinorGcStats {
    /// 持续时间（微秒）
    pub duration_us: u64,
    /// 回收的对象数
    pub objects_collected: u64,
    /// 晋升到老生代的对象数
    pub objects_promoted: u64,
}

/// Major GC 统计信息
#[derive(Debug, Clone)]
pub struct MajorGcStats {
    /// 持续时间（微秒）
    pub duration_us: u64,
    /// 标记的对象数
    pub objects_marked: u64,
    /// 回收的对象数
    pub objects_collected: u64,
}

/// 内存使用情况
#[derive(Debug, Clone)]
pub struct MemoryUsage {
    /// 新生代已用字节数
    pub young_used: usize,
    /// 新生代容量
    pub young_capacity: usize,
    /// 老生代已用字节数
    pub old_used: usize,
    /// 老生代容量
    pub old_capacity: usize,
}

/// 回收统计信息
#[derive(Debug, Clone)]
pub struct CollectionStats {
    /// 总回收次数
    pub total_collections: u64,
    /// 新生代回收次数
    pub young_collections: u64,
    /// 老生代回收次数
    pub old_collections: u64,
}
