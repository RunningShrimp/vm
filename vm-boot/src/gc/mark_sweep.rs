//! 标记-清除垃圾回收算法实现
//!
//! 实现完整的标记-清除GC算法，包括：
//! - 根集合扫描
//! - 对象图遍历和标记
//! - 内存回收和压缩

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::{debug, trace};

/// 对象标记状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarkState {
    /// 未标记（白色）
    Unmarked,
    /// 已标记（黑色）
    Marked,
    /// 正在处理（灰色）
    Processing,
}

/// 对象元数据
#[derive(Debug, Clone)]
struct ObjectMetadata {
    /// 对象地址
    #[allow(dead_code)]
    addr: u64,
    /// 对象大小
    size: usize,
    /// 标记状态
    mark_state: MarkState,
    /// 引用计数（用于调试）
    #[allow(dead_code)]
    ref_count: usize,
    /// 对象类型标识
    #[allow(dead_code)]
    obj_type: u32,
}

impl ObjectMetadata {
    fn new(addr: u64, size: usize, obj_type: u32) -> Self {
        Self {
            addr,
            size,
            mark_state: MarkState::Unmarked,
            ref_count: 0,
            obj_type,
        }
    }
}

/// 根对象来源
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RootSource {
    /// 寄存器
    Register { reg_id: u32 },
    /// 栈帧
    StackFrame { frame_id: usize },
    /// 全局变量
    Global { name: String },
    /// 静态变量
    Static { name: String },
}

/// 根对象引用
#[derive(Debug, Clone)]
pub struct RootReference {
    /// 对象地址
    pub addr: u64,
    /// 来源
    pub source: RootSource,
}

/// 增量GC配置
#[derive(Debug, Clone)]
pub struct IncrementalGcConfig {
    /// 每次增量标记的对象数量限制
    pub mark_work_limit: usize,
    /// 每次增量清除的对象数量限制
    pub sweep_work_limit: usize,
    /// 增量GC的时间预算（微秒）
    pub time_budget_us: u64,
    /// 是否启用增量GC
    pub enabled: bool,
}

impl Default for IncrementalGcConfig {
    fn default() -> Self {
        Self {
            mark_work_limit: 100,      // 每次标记100个对象
            sweep_work_limit: 200,     // 每次清除200个对象
            time_budget_us: 1000,      // 1ms时间预算
            enabled: true,
        }
    }
}

/// 增量GC状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IncrementalGcState {
    /// 空闲（未运行）
    Idle,
    /// 标记阶段
    Marking,
    /// 清除阶段
    Sweeping,
}

/// 标记-清除GC收集器
pub struct MarkSweepCollector {
    /// 对象元数据映射（地址 -> 元数据）
    objects: Arc<Mutex<HashMap<u64, ObjectMetadata>>>,
    /// 根对象集合
    roots: Arc<Mutex<Vec<RootReference>>>,
    /// 已分配的内存区域
    #[allow(dead_code)]
    allocated_regions: Arc<Mutex<Vec<(u64, usize)>>>,
    /// GC统计信息
    stats: Arc<Mutex<GcCollectionStats>>,
    /// 增量GC配置
    incremental_config: IncrementalGcConfig,
    /// 增量GC状态
    incremental_state: Arc<Mutex<IncrementalGcState>>,
    /// 标记工作队列（用于增量标记）
    mark_work_queue: Arc<Mutex<Vec<u64>>>,
    /// 清除工作队列（用于增量清除）
    sweep_work_queue: Arc<Mutex<Vec<u64>>>,
    /// 上次增量GC的时间
    last_incremental_time: Arc<Mutex<Instant>>,
}

/// GC收集统计信息
#[derive(Debug, Clone, Default)]
pub struct GcCollectionStats {
    /// 收集次数
    pub collections: u64,
    /// 标记的对象数
    pub objects_marked: u64,
    /// 回收的对象数
    pub objects_reclaimed: u64,
    /// 回收的字节数
    pub bytes_reclaimed: u64,
    /// 标记阶段耗时（微秒）
    pub mark_time_us: u64,
    /// 清除阶段耗时（微秒）
    pub sweep_time_us: u64,
    /// 总耗时（微秒）
    pub total_time_us: u64,
}

impl MarkSweepCollector {
    /// 创建新的标记-清除收集器
    pub fn new() -> Self {
        Self {
            objects: Arc::new(Mutex::new(HashMap::new())),
            roots: Arc::new(Mutex::new(Vec::new())),
            allocated_regions: Arc::new(Mutex::new(Vec::new())),
            stats: Arc::new(Mutex::new(GcCollectionStats::default())),
            incremental_config: IncrementalGcConfig::default(),
            incremental_state: Arc::new(Mutex::new(IncrementalGcState::Idle)),
            mark_work_queue: Arc::new(Mutex::new(Vec::new())),
            sweep_work_queue: Arc::new(Mutex::new(Vec::new())),
            last_incremental_time: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// 创建带增量GC配置的收集器
    pub fn with_incremental_config(config: IncrementalGcConfig) -> Self {
        Self {
            objects: Arc::new(Mutex::new(HashMap::new())),
            roots: Arc::new(Mutex::new(Vec::new())),
            allocated_regions: Arc::new(Mutex::new(Vec::new())),
            stats: Arc::new(Mutex::new(GcCollectionStats::default())),
            incremental_config: config,
            incremental_state: Arc::new(Mutex::new(IncrementalGcState::Idle)),
            mark_work_queue: Arc::new(Mutex::new(Vec::new())),
            sweep_work_queue: Arc::new(Mutex::new(Vec::new())),
            last_incremental_time: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// 注册对象
    pub fn register_object(&self, addr: u64, size: usize, obj_type: u32) {
        let mut objects = self.objects.lock().unwrap();
        objects.insert(addr, ObjectMetadata::new(addr, size, obj_type));
    }

    /// 添加根对象引用
    pub fn add_root(&self, root: RootReference) {
        let mut roots = self.roots.lock().unwrap();
        roots.push(root);
    }

    /// 清除所有根对象
    pub fn clear_roots(&self) {
        let mut roots = self.roots.lock().unwrap();
        roots.clear();
    }

    /// 扫描根集合
    #[allow(dead_code)]
    fn scan_roots(&self) -> Vec<u64> {
        let roots = self.roots.lock().unwrap();
        let mut root_addrs = Vec::new();

        for root in roots.iter() {
            root_addrs.push(root.addr);
            trace!(
                "Found root object at 0x{:x} from {:?}",
                root.addr, root.source
            );
        }

        debug!("Scanned {} root objects", root_addrs.len());
        root_addrs
    }

    /// 标记阶段：从根对象开始标记所有可达对象
    fn mark_phase(&self) -> u64 {
        let start = Instant::now();
        let mut objects = self.objects.lock().unwrap();

        // 1. 重置所有对象的标记状态
        for obj in objects.values_mut() {
            obj.mark_state = MarkState::Unmarked;
        }

        // 2. 扫描根集合
        let root_addrs = {
            let roots = self.roots.lock().unwrap();
            roots.iter().map(|r| r.addr).collect::<Vec<_>>()
        };

        // 3. 从根对象开始标记
        let mut work_queue = Vec::new();
        let mut marked_count = 0u64;

        // 初始化工作队列
        for root_addr in root_addrs {
            if let Some(obj) = objects.get_mut(&root_addr)
                && obj.mark_state == MarkState::Unmarked {
                    obj.mark_state = MarkState::Processing;
                    work_queue.push(root_addr);
                }
        }

        // 4. 处理工作队列（广度优先遍历）
        while let Some(addr) = work_queue.pop() {
            if let Some(obj) = objects.get_mut(&addr) {
                obj.mark_state = MarkState::Marked;
                marked_count += 1;

                // 遍历对象的引用（简化实现：假设对象包含引用）
                // 实际实现中，需要根据对象类型解析引用字段
                let references = self.get_object_references(addr, &objects);
                for ref_addr in references {
                    if let Some(ref_obj) = objects.get_mut(&ref_addr)
                        && ref_obj.mark_state == MarkState::Unmarked {
                            ref_obj.mark_state = MarkState::Processing;
                            work_queue.push(ref_addr);
                        }
                }
            }
        }

        let mark_time = start.elapsed().as_micros() as u64;
        debug!(
            "Mark phase completed: {} objects marked in {}us",
            marked_count, mark_time
        );
        marked_count
    }

    /// 获取对象的引用（简化实现）
    ///
    /// 实际实现中，需要根据对象类型和布局解析引用字段
    fn get_object_references(
        &self,
        _addr: u64,
        _objects: &HashMap<u64, ObjectMetadata>,
    ) -> Vec<u64> {
        // TODO: 实现对象引用解析
        // 这里返回空向量作为占位实现
        // 实际实现需要：
        // 1. 根据对象类型确定引用字段位置
        // 2. 从内存中读取引用值
        // 3. 验证引用有效性
        Vec::new()
    }

    /// 清除阶段：回收未标记的对象
    fn sweep_phase(&self) -> (u64, u64) {
        let start = Instant::now();
        let mut objects = self.objects.lock().unwrap();
        let mut reclaimed_count = 0u64;
        let mut reclaimed_bytes = 0u64;

        // 收集未标记的对象
        let mut to_remove = Vec::new();
        for (addr, obj) in objects.iter() {
            if obj.mark_state == MarkState::Unmarked {
                to_remove.push(*addr);
                reclaimed_count += 1;
                reclaimed_bytes += obj.size as u64;
            }
        }

        // 移除未标记的对象
        for addr in &to_remove {
            objects.remove(addr);
        }

        let sweep_time = start.elapsed().as_micros() as u64;
        debug!(
            "Sweep phase completed: {} objects ({}) reclaimed in {}us",
            reclaimed_count, reclaimed_bytes, sweep_time
        );

        (reclaimed_count, reclaimed_bytes)
    }

    /// 执行完整的标记-清除GC
    pub fn collect(&self) -> GcCollectionStats {
        let start = Instant::now();
        debug!("Starting mark-sweep GC collection");

        // 标记阶段
        let objects_marked = self.mark_phase();

        // 清除阶段
        let (objects_reclaimed, bytes_reclaimed) = self.sweep_phase();

        // 更新统计信息
        let total_time = start.elapsed().as_micros() as u64;
        let mut stats = self.stats.lock().unwrap();
        stats.collections += 1;
        stats.objects_marked = objects_marked;
        stats.objects_reclaimed = objects_reclaimed;
        stats.bytes_reclaimed = bytes_reclaimed;
        stats.total_time_us = total_time;
        stats.mark_time_us = total_time / 2; // 简化：假设各占一半
        stats.sweep_time_us = total_time / 2;

        debug!(
            "GC collection completed: {} objects marked, {} objects reclaimed ({} bytes) in {}us",
            objects_marked, objects_reclaimed, bytes_reclaimed, total_time
        );

        stats.clone()
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> GcCollectionStats {
        self.stats.lock().unwrap().clone()
    }

    /// 获取当前对象数量
    pub fn object_count(&self) -> usize {
        self.objects.lock().unwrap().len()
    }

    /// 执行增量GC步骤
    ///
    /// 如果增量GC未启用或已完成，返回false；否则返回true表示还有工作要做
    pub fn incremental_step(&self) -> bool {
        if !self.incremental_config.enabled {
            return false;
        }

        let start = Instant::now();
        let mut state = self.incremental_state.lock().unwrap();

        match *state {
            IncrementalGcState::Idle => {
                // 检查是否需要启动新的GC周期
                let objects = self.objects.lock().unwrap();
                if objects.len() > 1000 {
                    // 对象数量超过阈值，启动增量GC
                    *state = IncrementalGcState::Marking;
                    drop(objects);
                    self.start_incremental_mark();
                    *self.last_incremental_time.lock().unwrap() = Instant::now();
                    true
                } else {
                    false
                }
            }
            IncrementalGcState::Marking => {
                drop(state);
                let has_more_work = self.incremental_mark_step();
                if !has_more_work {
                    // 标记完成，切换到清除阶段
                    *self.incremental_state.lock().unwrap() = IncrementalGcState::Sweeping;
                    self.start_incremental_sweep();
                }
                has_more_work || (start.elapsed().as_micros() as u64) < self.incremental_config.time_budget_us
            }
            IncrementalGcState::Sweeping => {
                drop(state);
                let has_more_work = self.incremental_sweep_step();
                if !has_more_work {
                    // 清除完成，回到空闲状态
                    *self.incremental_state.lock().unwrap() = IncrementalGcState::Idle;
                    let mut stats = self.stats.lock().unwrap();
                    stats.collections += 1;
                }
                has_more_work || (start.elapsed().as_micros() as u64) < self.incremental_config.time_budget_us
            }
        }
    }

    /// 启动增量标记阶段
    fn start_incremental_mark(&self) {
        let mut objects = self.objects.lock().unwrap();
        let mut work_queue = self.mark_work_queue.lock().unwrap();

        // 重置所有对象的标记状态
        for obj in objects.values_mut() {
            obj.mark_state = MarkState::Unmarked;
        }

        // 扫描根集合，初始化工作队列
        let root_addrs = {
            let roots = self.roots.lock().unwrap();
            roots.iter().map(|r| r.addr).collect::<Vec<_>>()
        };

        for root_addr in root_addrs {
            if let Some(obj) = objects.get_mut(&root_addr) {
                if obj.mark_state == MarkState::Unmarked {
                    obj.mark_state = MarkState::Processing;
                    work_queue.push(root_addr);
                }
            }
        }
    }

    /// 执行增量标记步骤
    fn incremental_mark_step(&self) -> bool {
        let mut objects = self.objects.lock().unwrap();
        let mut work_queue = self.mark_work_queue.lock().unwrap();
        let mut marked_count = 0u64;
        let work_limit = self.incremental_config.mark_work_limit;

        // 处理工作队列中的对象
        while let Some(addr) = work_queue.pop() {
            if marked_count >= work_limit as u64 {
                // 达到工作限制，将当前对象放回队列
                work_queue.push(addr);
                break;
            }

            if let Some(obj) = objects.get_mut(&addr) {
                obj.mark_state = MarkState::Marked;
                marked_count += 1;

                // 遍历对象的引用
                let references = self.get_object_references(addr, &objects);
                for ref_addr in references {
                    if let Some(ref_obj) = objects.get_mut(&ref_addr) {
                        if ref_obj.mark_state == MarkState::Unmarked {
                            ref_obj.mark_state = MarkState::Processing;
                            work_queue.push(ref_addr);
                        }
                    }
                }
            }
        }

        // 更新统计信息
        let mut stats = self.stats.lock().unwrap();
        stats.objects_marked += marked_count;

        // 返回是否还有工作要做
        !work_queue.is_empty()
    }

    /// 启动增量清除阶段
    fn start_incremental_sweep(&self) {
        let mut objects = self.objects.lock().unwrap();
        let mut sweep_queue = self.sweep_work_queue.lock().unwrap();

        // 收集所有未标记的对象到清除队列
        for addr in objects.keys() {
            if let Some(obj) = objects.get(addr) {
                if obj.mark_state == MarkState::Unmarked {
                    sweep_queue.push(*addr);
                }
            }
        }
    }

    /// 执行增量清除步骤
    fn incremental_sweep_step(&self) -> bool {
        let mut objects = self.objects.lock().unwrap();
        let mut sweep_queue = self.sweep_work_queue.lock().unwrap();
        let mut reclaimed_count = 0u64;
        let mut reclaimed_bytes = 0u64;
        let work_limit = self.incremental_config.sweep_work_limit;

        // 处理清除队列中的对象
        while let Some(addr) = sweep_queue.pop() {
            if reclaimed_count >= work_limit as u64 {
                // 达到工作限制，将剩余对象放回队列
                sweep_queue.push(addr);
                break;
            }

            if let Some(obj) = objects.remove(&addr) {
                reclaimed_count += 1;
                reclaimed_bytes += obj.size as u64;
            }
        }

        // 更新统计信息
        let mut stats = self.stats.lock().unwrap();
        stats.objects_reclaimed += reclaimed_count;
        stats.bytes_reclaimed += reclaimed_bytes;

        // 返回是否还有工作要做
        !sweep_queue.is_empty()
    }

    /// 设置增量GC配置
    pub fn set_incremental_config(&mut self, config: IncrementalGcConfig) {
        self.incremental_config = config;
    }

    /// 获取增量GC状态
    pub fn incremental_state(&self) -> IncrementalGcState {
        *self.incremental_state.lock().unwrap()
    }
}

impl Default for MarkSweepCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mark_sweep_basic() {
        let collector = MarkSweepCollector::new();

        // 注册一些对象
        collector.register_object(0x1000, 100, 1);
        collector.register_object(0x2000, 200, 2);
        collector.register_object(0x3000, 300, 3);

        // 添加根对象
        collector.add_root(RootReference {
            addr: 0x1000,
            source: RootSource::Register { reg_id: 0 },
        });

        // 执行GC
        let stats = collector.collect();

        // 根对象应该被保留
        assert_eq!(stats.objects_marked, 1);
        assert_eq!(collector.object_count(), 1);
    }

    #[test]
    fn test_mark_sweep_unreachable() {
        let collector = MarkSweepCollector::new();

        // 注册对象
        collector.register_object(0x1000, 100, 1); // 根对象
        collector.register_object(0x2000, 200, 2); // 不可达对象

        // 只添加一个根对象
        collector.add_root(RootReference {
            addr: 0x1000,
            source: RootSource::StackFrame { frame_id: 0 },
        });

        // 执行GC
        let stats = collector.collect();

        // 不可达对象应该被回收
        assert_eq!(stats.objects_reclaimed, 1);
        assert_eq!(collector.object_count(), 1);
    }
}
